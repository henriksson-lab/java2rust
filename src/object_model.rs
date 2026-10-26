use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::effective_facts::EffectiveFacts;
use crate::facts::{
    DispatchRequirementKind, FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy,
    ObjectSafetyViolation,
};
use crate::java_ir::{Block, Expr, JavaType, Member, Program, Stmt, TypeDecl, TypeKind};
use crate::usage::{BindingUse, BindingUseFacts, CallArgumentBoundary, FieldUse, UsageFacts};

#[derive(Debug, Clone, Copy)]
pub struct ObjectModelInput<'a> {
    program: &'a Program,
    facts: &'a FactSet,
    effective_facts: &'a EffectiveFacts<'a>,
    usage: Option<&'a UsageFacts>,
}

impl<'a> ObjectModelInput<'a> {
    pub fn new(
        program: &'a Program,
        facts: &'a FactSet,
        effective_facts: &'a EffectiveFacts<'a>,
        usage: Option<&'a UsageFacts>,
    ) -> Self {
        Self {
            program,
            facts,
            effective_facts,
            usage,
        }
    }

    pub fn program(&self) -> &'a Program {
        self.program
    }

    pub fn facts(&self) -> &'a FactSet {
        self.facts
    }

    pub fn effective_facts(&self) -> &'a EffectiveFacts<'a> {
        self.effective_facts
    }

    pub fn usage(&self) -> Option<&'a UsageFacts> {
        self.usage
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectModelPlan {
    strategies: BTreeMap<String, ObjectStrategyPlan>,
    hierarchy_edges: Vec<ObjectHierarchyEdge>,
    diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStrategyPlan {
    root: String,
    strategy: ObjectRepresentation,
    requirements: Vec<DispatchRequirement>,
    direct_subtypes: Vec<String>,
    source_methods_access_instance_fields: bool,
    rejected: Vec<RejectedObjectStrategy>,
    provenance: Vec<FactProvenance>,
    diagnostics: Vec<RepresentationDiagnostic>,
}

pub type DispatchRequirement = DispatchRequirementKind;
pub type UsageDispatchRequirements = BTreeMap<String, Vec<DispatchRequirement>>;
type UsageRequirementAliases = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedObjectStrategy {
    strategy: ObjectRepresentation,
    reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectHierarchyContext {
    known_types: BTreeSet<String>,
    direct_subtypes: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHierarchyEdge {
    super_name: String,
    sub_name: String,
    kind: ObjectHierarchyEdgeKind,
}

impl ObjectHierarchyEdge {
    pub fn new(
        super_name: impl Into<String>,
        sub_name: impl Into<String>,
        kind: ObjectHierarchyEdgeKind,
    ) -> Self {
        Self {
            super_name: super_name.into(),
            sub_name: sub_name.into(),
            kind,
        }
    }

    pub fn super_name(&self) -> &str {
        &self.super_name
    }

    pub fn sub_name(&self) -> &str {
        &self.sub_name
    }

    pub fn kind(&self) -> ObjectHierarchyEdgeKind {
        self.kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHierarchyEdgeRepresentationPolicy {
    representation: ObjectRepresentation,
    represented: bool,
}

impl ObjectHierarchyEdgeRepresentationPolicy {
    pub fn new(representation: ObjectRepresentation, represented: bool) -> Self {
        Self {
            representation,
            represented,
        }
    }

    pub fn representation(&self) -> &ObjectRepresentation {
        &self.representation
    }

    pub fn represented(&self) -> bool {
        self.represented
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHierarchyEdgeDiagnosticInput {
    type_name: String,
    declared_extends: Option<String>,
    declared_implements: Vec<String>,
    edges: Vec<ObjectHierarchyEdge>,
    represented_super_names: BTreeSet<String>,
}

impl ObjectHierarchyEdgeDiagnosticInput {
    pub fn new(
        type_name: String,
        declared_extends: Option<String>,
        declared_implements: Vec<String>,
        edges: Vec<ObjectHierarchyEdge>,
        represented_super_names: BTreeSet<String>,
    ) -> Self {
        Self {
            type_name,
            declared_extends,
            declared_implements,
            edges,
            represented_super_names,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyEdgeDiagnosticPolicy {
    type_name: String,
    outcomes: Vec<HierarchyEdgeDiagnosticOutcome>,
    diagnostics: Vec<RepresentationDiagnostic>,
}

impl HierarchyEdgeDiagnosticPolicy {
    pub fn new(
        type_name: String,
        outcomes: Vec<HierarchyEdgeDiagnosticOutcome>,
        diagnostics: Vec<RepresentationDiagnostic>,
    ) -> Self {
        Self {
            type_name,
            outcomes,
            diagnostics,
        }
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn outcomes(&self) -> &[HierarchyEdgeDiagnosticOutcome] {
        &self.outcomes
    }

    pub fn diagnostics(&self) -> &[RepresentationDiagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<RepresentationDiagnostic> {
        self.diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HierarchyEdgeDiagnosticOutcome {
    UnsupportedInheritance,
    UnsupportedInterfaceImpl { interfaces: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectHierarchyEdgeKind {
    Extends,
    Implements,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyOverrideValidationPolicy {
    type_name: String,
    diagnostics: Vec<RepresentationDiagnostic>,
}

impl HierarchyOverrideValidationPolicy {
    pub fn new(type_name: String, diagnostics: Vec<RepresentationDiagnostic>) -> Self {
        Self {
            type_name,
            diagnostics,
        }
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn diagnostics(&self) -> &[RepresentationDiagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<RepresentationDiagnostic> {
        self.diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyOverrideApplicationPolicy {
    type_name: String,
    representation: Option<ObjectRepresentation>,
    provenance: FactProvenance,
    direct_subtypes: Vec<String>,
    diagnostics: Vec<RepresentationDiagnostic>,
}

impl HierarchyOverrideApplicationPolicy {
    pub fn new(
        type_name: String,
        representation: Option<ObjectRepresentation>,
        provenance: FactProvenance,
        direct_subtypes: Vec<String>,
        diagnostics: Vec<RepresentationDiagnostic>,
    ) -> Self {
        Self {
            type_name,
            representation,
            provenance,
            direct_subtypes,
            diagnostics,
        }
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn representation(&self) -> Option<&ObjectRepresentation> {
        self.representation.as_ref()
    }

    pub fn provenance(&self) -> FactProvenance {
        self.provenance
    }

    pub fn direct_subtypes(&self) -> &[String] {
        &self.direct_subtypes
    }

    pub fn diagnostics(&self) -> &[RepresentationDiagnostic] {
        &self.diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyOverrideRepresentationPolicy {
    representation: ObjectRepresentation,
    object_safety: ObjectSafetyPolicy,
    diagnostics: Vec<RepresentationDiagnostic>,
}

impl HierarchyOverrideRepresentationPolicy {
    pub fn new(
        representation: ObjectRepresentation,
        object_safety: ObjectSafetyPolicy,
        diagnostics: Vec<RepresentationDiagnostic>,
    ) -> Self {
        Self {
            representation,
            object_safety,
            diagnostics,
        }
    }

    pub fn representation(&self) -> &ObjectRepresentation {
        &self.representation
    }

    pub fn object_safety(&self) -> ObjectSafetyPolicy {
        self.object_safety
    }

    pub fn into_representation_diagnostics(
        self,
    ) -> (ObjectRepresentation, Vec<RepresentationDiagnostic>) {
        (self.representation, self.diagnostics)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedEnumInferencePolicy {
    type_name: String,
    representation: Option<ObjectRepresentation>,
    provenance: Option<FactProvenance>,
}

impl ClosedEnumInferencePolicy {
    pub fn new(
        type_name: String,
        representation: Option<ObjectRepresentation>,
        provenance: Option<FactProvenance>,
    ) -> Self {
        Self {
            type_name,
            representation,
            provenance,
        }
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn representation(&self) -> Option<&ObjectRepresentation> {
        self.representation.as_ref()
    }

    pub fn provenance(&self) -> Option<FactProvenance> {
        self.provenance
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedEnumProofPolicy {
    type_name: String,
    proof: ClosedEnumProof,
}

impl ClosedEnumProofPolicy {
    pub fn new(type_name: String, proof: ClosedEnumProof) -> Self {
        Self { type_name, proof }
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn proof(&self) -> &ClosedEnumProof {
        &self.proof
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedEnumProof {
    Proven {
        variants: Vec<String>,
        provenance: FactProvenance,
    },
    OpenWorld,
    NoKnownDirectSubtypes,
    MissingPermittedSubtypes {
        missing: Vec<String>,
    },
    NonFinalVariants {
        variants: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectRepresentation {
    ConcreteStruct,
    DynTrait {
        trait_name: String,
    },
    JavaEnum {
        enum_name: String,
        variants: Vec<String>,
    },
    ClosedEnum {
        enum_name: String,
        variants: Vec<String>,
    },
    SourceHierarchyCarrier {
        carrier_name: String,
        variants: Vec<String>,
    },
    ErasedObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectRepresentationRequest {
    DefaultInterface {
        type_name: String,
    },
    DynTraitOverride {
        type_name: String,
        trait_name: String,
    },
    ErasedObjectOverride {
        type_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRepresentationPolicy {
    object_safety: ObjectSafetyPolicy,
    representation: ObjectRepresentation,
    diagnostics: Vec<RepresentationDiagnostic>,
}

impl ObjectRepresentationPolicy {
    pub fn new(
        object_safety: ObjectSafetyPolicy,
        representation: ObjectRepresentation,
        diagnostics: Vec<RepresentationDiagnostic>,
    ) -> Self {
        Self {
            object_safety,
            representation,
            diagnostics,
        }
    }

    pub fn object_safety(&self) -> ObjectSafetyPolicy {
        self.object_safety
    }

    pub fn representation(&self) -> &ObjectRepresentation {
        &self.representation
    }

    pub fn diagnostics(&self) -> &[RepresentationDiagnostic] {
        &self.diagnostics
    }

    pub fn into_parts(
        self,
    ) -> (
        ObjectSafetyPolicy,
        ObjectRepresentation,
        Vec<RepresentationDiagnostic>,
    ) {
        (self.object_safety, self.representation, self.diagnostics)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationLookupPolicy {
    java_ty: JavaType,
    fallback: RepresentationLookupFallback,
    representation: ObjectRepresentation,
}

impl RepresentationLookupPolicy {
    pub fn new(
        java_ty: JavaType,
        fallback: RepresentationLookupFallback,
        representation: ObjectRepresentation,
    ) -> Self {
        Self {
            java_ty,
            fallback,
            representation,
        }
    }

    pub fn java_ty(&self) -> &JavaType {
        &self.java_ty
    }

    pub fn fallback(&self) -> RepresentationLookupFallback {
        self.fallback
    }

    pub fn representation(&self) -> &ObjectRepresentation {
        &self.representation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentationLookupFallback {
    PlannedType,
    ConservativeConcrete,
    ConservativeErasedObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSafetyPolicy {
    NotRequired,
    ProvenObjectSafe,
    AssumedObjectSafe,
    RejectedNonObjectSafe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSafetyFactPolicy {
    type_name: String,
    object_safe_trait: Option<bool>,
    violation: Option<ObjectSafetyViolation>,
}

impl ObjectSafetyFactPolicy {
    pub fn new(
        type_name: String,
        object_safe_trait: Option<bool>,
        violation: Option<ObjectSafetyViolation>,
    ) -> Self {
        Self {
            type_name,
            object_safe_trait,
            violation,
        }
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn object_safe_trait(&self) -> Option<bool> {
        self.object_safe_trait
    }

    pub fn violation(&self) -> Option<ObjectSafetyViolation> {
        self.violation.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationDiagnostic {
    code: &'static str,
    message: String,
}

impl RepresentationDiagnostic {
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

pub fn plan_object_model_with_effective_facts(
    program: &Program,
    facts: &EffectiveFacts<'_>,
) -> ObjectModelPlan {
    plan_object_model_from_effective_facts(program, facts)
}

pub fn plan_object_model_from_input(input: ObjectModelInput<'_>) -> ObjectModelPlan {
    let _facts = input.facts();
    let usage_requirements = input
        .usage()
        .map(dispatch_requirements_from_usage)
        .unwrap_or_default();
    plan_object_model_from_effective_facts_and_usage(
        input.program(),
        input.effective_facts(),
        &usage_requirements,
    )
}

fn plan_object_model_from_effective_facts(
    program: &Program,
    facts: &EffectiveFacts<'_>,
) -> ObjectModelPlan {
    plan_object_model_from_effective_facts_and_usage(
        program,
        facts,
        &UsageDispatchRequirements::new(),
    )
}

fn plan_object_model_from_effective_facts_and_usage(
    program: &Program,
    facts: &EffectiveFacts<'_>,
    usage_requirements: &UsageDispatchRequirements,
) -> ObjectModelPlan {
    let mut plan = ObjectModelPlan::default();
    let usage_aliases = usage_requirement_aliases(program);
    for ty in &program.types {
        collect_type_strategy(ty, facts, usage_requirements, &usage_aliases, &mut plan);
    }
    collect_hierarchy_edges_for_strategy_plan(facts, usage_requirements, &usage_aliases, &mut plan);
    apply_hierarchy_overrides_to_strategy_plan(facts, &mut plan);
    apply_closed_enum_strategy_selection(facts, &mut plan);
    plan
}

pub fn dispatch_requirements_from_usage(usage: &UsageFacts) -> UsageDispatchRequirements {
    let mut requirements = UsageDispatchRequirements::new();
    for method in usage.method_usages().chain(usage.constructor_usages()) {
        for field_use in method.field_uses() {
            let subtype_target = match field_use {
                FieldUse::AssignedSubtypeToField { field_type, .. } => Some(field_type),
                FieldUse::AssignedSubtypeToCallArgument { target_type, .. } => Some(target_type),
                FieldUse::AssignedSubtypeToReturn { target_type, .. } => Some(target_type),
                FieldUse::AssignedSubtypeToCollectionElement { element_type, .. } => {
                    Some(element_type)
                }
                _ => None,
            };
            let Some(subtype_target) = subtype_target else {
                continue;
            };
            let Some(type_name) = dispatch_requirement_type_name(subtype_target) else {
                continue;
            };
            push_usage_dispatch_requirement(
                &mut requirements,
                type_name,
                DispatchRequirementKind::NeedsSubtypeDispatch,
            );
        }
        for binding in method.all_binding_facts() {
            for use_fact in binding.uses() {
                if let BindingUse::AssignedSubtypeToCollectionElement { element_type, .. } =
                    use_fact
                {
                    if let Some(type_name) = dispatch_requirement_type_name(element_type) {
                        push_usage_dispatch_requirement(
                            &mut requirements,
                            type_name,
                            DispatchRequirementKind::NeedsSubtypeDispatch,
                        );
                    }
                    continue;
                }
                if let BindingUse::AssignedSubtypeToCallArgument { target_type, .. } = use_fact {
                    if let Some(type_name) = dispatch_requirement_type_name(target_type) {
                        push_usage_dispatch_requirement(
                            &mut requirements,
                            type_name,
                            DispatchRequirementKind::NeedsSubtypeDispatch,
                        );
                    }
                    continue;
                }
                let Some(type_name) = dispatch_requirement_type_name(binding.java_ty()) else {
                    continue;
                };
                for requirement in dispatch_requirements_for_binding_use(
                    binding,
                    use_fact,
                    method.type_params_in_scope(),
                ) {
                    push_usage_dispatch_requirement(&mut requirements, type_name, requirement);
                }
            }
        }
    }
    requirements
}

fn dispatch_requirements_for_binding_use(
    binding: &BindingUseFacts,
    use_fact: &BindingUse,
    type_params_in_scope: &[String],
) -> Vec<DispatchRequirement> {
    match use_fact {
        BindingUse::AssignedSubtypeToLocal { .. } => {
            vec![DispatchRequirementKind::NeedsSubtypeDispatch]
        }
        BindingUse::AssignedSubtypeToCollectionElement { .. }
        | BindingUse::AssignedSubtypeToCallArgument { .. } => Vec::new(),
        BindingUse::StoredInField { .. }
        | BindingUse::RetainedByCall
        | BindingUse::Returned
        | BindingUse::ReturnedNullableSequenceElements => {
            vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage]
        }
        BindingUse::InstanceOfTarget { .. } => vec![DispatchRequirementKind::NeedsInstanceof],
        BindingUse::PassedToCall {
            name,
            receiver_type,
            arg_index,
            boundary,
            ..
        } => match boundary {
            CallArgumentBoundary::InternalSameType | CallArgumentBoundary::KnownContentRead => {
                vec![DispatchRequirementKind::NeedsBorrowedTraitView]
            }
            CallArgumentBoundary::Unknown => {
                if collection_call_stores_binding_value(
                    name,
                    receiver_type.as_ref(),
                    *arg_index,
                    binding.java_ty(),
                ) {
                    vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage]
                } else if java_type_is_type_param_in_scope(binding.java_ty(), type_params_in_scope)
                {
                    Vec::new()
                } else {
                    vec![DispatchRequirementKind::NeedsUnknownRuntimeObject]
                }
            }
        },
        BindingUse::AssignedFromCall { boundary, .. } => match boundary {
            CallArgumentBoundary::InternalSameType | CallArgumentBoundary::KnownContentRead => {
                vec![DispatchRequirementKind::NeedsBorrowedTraitView]
            }
            CallArgumentBoundary::Unknown
                if java_type_is_type_param_in_scope(binding.java_ty(), type_params_in_scope) =>
            {
                Vec::new()
            }
            CallArgumentBoundary::Unknown => {
                vec![DispatchRequirementKind::NeedsUnknownRuntimeObject]
            }
        },
        BindingUse::ReceiverForCall { boundary, .. } => match boundary {
            CallArgumentBoundary::InternalSameType | CallArgumentBoundary::KnownContentRead => {
                vec![DispatchRequirementKind::NeedsBorrowedTraitView]
            }
            CallArgumentBoundary::Unknown => vec![DispatchRequirementKind::RejectTraitObject],
        },
        BindingUse::ContentRead
        | BindingUse::IndexedElementComparedToNull { .. }
        | BindingUse::AssignedToLocal { .. }
        | BindingUse::Mutated => Vec::new(),
        BindingUse::CastTarget { .. } => Vec::new(),
    }
}

fn java_type_is_type_param_in_scope(java_ty: &JavaType, type_params_in_scope: &[String]) -> bool {
    let name = match java_ty {
        JavaType::Named { name, args } if args.is_empty() => name,
        JavaType::Unknown(name) => name,
        _ => return false,
    };
    let simple = name.rsplit('.').next().unwrap_or(name);
    type_params_in_scope.iter().any(|param| param == simple)
}

fn collection_call_stores_binding_value(
    name: &str,
    receiver_type: Option<&JavaType>,
    arg_index: usize,
    binding_ty: &JavaType,
) -> bool {
    let Some(receiver_type) = receiver_type else {
        return false;
    };
    match crate::stdlib::java_collection_kind_for_type(receiver_type) {
        Some(crate::stdlib::JavaCollectionKind::List)
            if matches!(name, "add" | "set") && matches!(arg_index, 0 | 1) =>
        {
            collection_type_arg_matches_binding(receiver_type, 0, binding_ty)
        }
        Some(kind) if kind.is_set_like() && name == "add" && arg_index == 0 => {
            collection_type_arg_matches_binding(receiver_type, 0, binding_ty)
        }
        Some(kind) if kind.is_map_like() && name == "put" && matches!(arg_index, 0 | 1) => {
            collection_type_arg_matches_binding(receiver_type, arg_index, binding_ty)
        }
        _ => false,
    }
}

fn collection_type_arg_matches_binding(
    receiver_type: &JavaType,
    arg_index: usize,
    binding_ty: &JavaType,
) -> bool {
    let JavaType::Named { args, .. } = receiver_type else {
        return false;
    };
    args.get(arg_index)
        .is_some_and(|arg_ty| collection_storage_type_matches(arg_ty, binding_ty))
}

fn collection_storage_type_matches(left: &JavaType, right: &JavaType) -> bool {
    match (left, right) {
        (
            JavaType::Named {
                name: left_name,
                args: left_args,
            },
            JavaType::Named {
                name: right_name,
                args: right_args,
            },
        ) => {
            left_args.len() == right_args.len()
                && (left_name == right_name
                    || left_name.rsplit('.').next() == Some(right_name.as_str())
                    || right_name.rsplit('.').next() == Some(left_name.as_str()))
        }
        _ => left == right,
    }
}

fn dispatch_requirement_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::Intersection(bounds) => bounds.iter().find_map(dispatch_requirement_type_name),
        JavaType::String => Some("String"),
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_) | JavaType::Wildcard(_) => {
            None
        }
    }
}

fn push_usage_dispatch_requirement(
    requirements: &mut UsageDispatchRequirements,
    type_name: &str,
    requirement: DispatchRequirement,
) {
    let entry = requirements.entry(type_name.to_string()).or_default();
    if !entry.contains(&requirement) {
        entry.push(requirement);
    }
}

fn merged_dispatch_requirements(
    type_name: &str,
    facts: &EffectiveFacts<'_>,
    usage_requirements: &UsageDispatchRequirements,
    usage_aliases: &UsageRequirementAliases,
) -> Vec<DispatchRequirement> {
    let mut requirements = facts.dispatch_requirements_for(type_name);
    merge_usage_dispatch_requirements(&mut requirements, usage_requirements.get(type_name));
    if let Some(alias) = usage_aliases.get(type_name) {
        merge_usage_dispatch_requirements(&mut requirements, usage_requirements.get(alias));
    }
    requirements
}

fn merge_usage_dispatch_requirements(
    requirements: &mut Vec<DispatchRequirement>,
    usage: Option<&Vec<DispatchRequirement>>,
) {
    if let Some(usage) = usage {
        for requirement in usage {
            if !requirements.contains(requirement) {
                requirements.push(*requirement);
            }
        }
    }
}

fn usage_requirement_aliases(program: &Program) -> UsageRequirementAliases {
    let mut names = Vec::new();
    for ty in &program.types {
        collect_usage_requirement_type_names(ty, &mut names);
    }
    let mut simple_counts = BTreeMap::<String, usize>::new();
    for name in &names {
        *simple_counts
            .entry(simple_type_name(name).to_string())
            .or_default() += 1;
    }
    names
        .into_iter()
        .filter_map(|name| {
            let simple = simple_type_name(&name).to_string();
            (simple != name && simple_counts.get(&simple) == Some(&1)).then(|| (name, simple))
        })
        .collect()
}

fn collect_usage_requirement_type_names(ty: &TypeDecl, out: &mut Vec<String>) {
    out.push(ty.name.clone());
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_usage_requirement_type_names(nested, out);
        }
    }
}

fn simple_type_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

impl ObjectModelPlan {
    pub fn strategies(&self) -> &BTreeMap<String, ObjectStrategyPlan> {
        &self.strategies
    }

    pub fn hierarchy_edges(&self) -> &[ObjectHierarchyEdge] {
        &self.hierarchy_edges
    }

    pub fn diagnostics(&self) -> &[RepresentationDiagnostic] {
        &self.diagnostics
    }

    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<String, ObjectStrategyPlan>,
        Vec<ObjectHierarchyEdge>,
        Vec<RepresentationDiagnostic>,
    ) {
        (self.strategies, self.hierarchy_edges, self.diagnostics)
    }

    pub fn strategy_for(&self, root: &str) -> Option<&ObjectStrategyPlan> {
        self.strategies.get(root)
    }

    fn strategy_for_mut(&mut self, root: &str) -> Option<&mut ObjectStrategyPlan> {
        self.strategies.get_mut(root)
    }

    fn insert_strategy(&mut self, strategy: ObjectStrategyPlan) {
        self.strategies.insert(strategy.root.clone(), strategy);
    }
}

impl ObjectStrategyPlan {
    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn strategy(&self) -> ObjectRepresentation {
        self.strategy.clone()
    }

    pub fn requirements(&self) -> &[DispatchRequirement] {
        &self.requirements
    }

    pub fn direct_subtypes(&self) -> &[String] {
        &self.direct_subtypes
    }

    pub fn source_methods_access_instance_fields(&self) -> bool {
        self.source_methods_access_instance_fields
    }

    pub fn rejected(&self) -> &[RejectedObjectStrategy] {
        &self.rejected
    }

    pub fn provenance(&self) -> &[FactProvenance] {
        &self.provenance
    }

    pub fn diagnostics(&self) -> &[RepresentationDiagnostic] {
        &self.diagnostics
    }

    pub fn into_parts(
        self,
    ) -> (
        String,
        ObjectRepresentation,
        Vec<DispatchRequirement>,
        Vec<String>,
        bool,
        Vec<RejectedObjectStrategy>,
        Vec<FactProvenance>,
        Vec<RepresentationDiagnostic>,
    ) {
        (
            self.root,
            self.strategy,
            self.requirements,
            self.direct_subtypes,
            self.source_methods_access_instance_fields,
            self.rejected,
            self.provenance,
            self.diagnostics,
        )
    }

    pub fn primary_provenance(&self) -> FactProvenance {
        self.provenance
            .last()
            .copied()
            .unwrap_or(FactProvenance::ConservativeFallback)
    }

    pub fn has_provenance(&self, provenance: FactProvenance) -> bool {
        self.provenance.contains(&provenance)
    }

    fn replace_provenance(&mut self, provenance: FactProvenance) {
        self.provenance = vec![provenance];
    }

    fn add_provenance(&mut self, provenance: FactProvenance) {
        if !self.provenance.contains(&provenance) {
            self.provenance.push(provenance);
        }
    }
}

impl RejectedObjectStrategy {
    pub fn strategy(&self) -> ObjectRepresentation {
        self.strategy.clone()
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

fn object_strategy_for_type_with_effective_facts_and_requirements(
    ty: &TypeDecl,
    facts: &EffectiveFacts<'_>,
    requirements: Vec<DispatchRequirement>,
) -> ObjectStrategyPlan {
    let explicit_hierarchy_override = facts.hierarchy_overrides().iter().find(|override_fact| {
        override_fact.type_name == ty.name && !override_fact.is_source_fallback()
    });
    let (strategy, diagnostics) = match ty.kind {
        TypeKind::Class
            if ty.modifiers.contains(&crate::java_ir::Modifier::Abstract)
                && !type_has_instance_fields(ty)
                && facts.closed_hierarchy_fact_for_name(&ty.name) == Some(true) =>
        {
            (ObjectRepresentation::ErasedObject, Vec::new())
        }
        TypeKind::Class | TypeKind::Record => (ObjectRepresentation::ConcreteStruct, Vec::new()),
        TypeKind::Interface if explicit_hierarchy_override.is_some() => {
            let override_fact =
                explicit_hierarchy_override.expect("interface override was checked by match guard");
            let policy =
                hierarchy_override_representation_policy_with_effective_facts(override_fact, facts);
            policy.into_representation_diagnostics()
        }
        TypeKind::Interface => {
            let policy = object_representation_policy_with_effective_facts(
                ObjectRepresentationRequest::DefaultInterface {
                    type_name: ty.name.clone(),
                },
                facts,
            );
            let (_, representation, diagnostics) = policy.into_parts();
            (representation, diagnostics)
        }
        TypeKind::Enum => (
            ObjectRepresentation::JavaEnum {
                enum_name: ty.name.clone(),
                variants: java_enum_variant_names(ty),
            },
            Vec::new(),
        ),
    };
    ObjectStrategyPlan {
        root: ty.name.clone(),
        strategy,
        requirements,
        direct_subtypes: Vec::new(),
        source_methods_access_instance_fields: type_methods_access_instance_fields(ty),
        rejected: Vec::new(),
        provenance: vec![FactProvenance::ParsedSource],
        diagnostics,
    }
}

pub fn java_enum_variant_names(ty: &TypeDecl) -> Vec<String> {
    if ty.kind != TypeKind::Enum {
        return Vec::new();
    }
    crate::constructor_plan::enum_constant_fields(ty)
        .map(|field| rust_enum_variant_name_from_parsed_constant(&field.name))
        .collect()
}

fn type_has_instance_fields(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| {
        matches!(
            member,
            Member::Field(field) if !field.modifiers.contains(&crate::java_ir::Modifier::Static)
        )
    })
}

fn type_methods_access_instance_fields(ty: &TypeDecl) -> bool {
    let field_names = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field)
                if !field.modifiers.contains(&crate::java_ir::Modifier::Static) =>
            {
                Some(field.name.clone())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();
    if field_names.is_empty() {
        return false;
    }
    ty.members.iter().any(|member| match member {
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(|body| block_accesses_instance_field(body, &field_names)),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(|body| block_accesses_instance_field(body, &field_names)),
        _ => false,
    })
}

fn block_accesses_instance_field(block: &Block, field_names: &HashSet<String>) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_accesses_instance_field(stmt, field_names))
}

fn stmt_accesses_instance_field(stmt: &Stmt, field_names: &HashSet<String>) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => initializer
            .as_ref()
            .is_some_and(|expr| expr_accesses_instance_field(expr, field_names)),
        Stmt::Assign { target, value } => {
            expr_accesses_instance_field(target, field_names)
                || expr_accesses_instance_field(value, field_names)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => expr_accesses_instance_field(expr, field_names),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_accesses_instance_field(condition, field_names)
                || block_accesses_instance_field(then_block, field_names)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_accesses_instance_field(block, field_names))
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_accesses_instance_field(condition, field_names)
                || block_accesses_instance_field(body, field_names)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_accesses_instance_field(stmt, field_names))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_accesses_instance_field(expr, field_names))
                || update
                    .iter()
                    .any(|expr| expr_accesses_instance_field(expr, field_names))
                || block_accesses_instance_field(body, field_names)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_accesses_instance_field(iterable, field_names)
                || block_accesses_instance_field(body, field_names)
        }
        Stmt::Switch { selector, cases } => {
            expr_accesses_instance_field(selector, field_names)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_accesses_instance_field(expr, field_names)
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || case
                        .statements
                        .iter()
                        .any(|stmt| stmt_accesses_instance_field(stmt, field_names))
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|stmt| stmt_accesses_instance_field(stmt, field_names))
                || block_accesses_instance_field(body, field_names)
                || catches
                    .iter()
                    .any(|catch| block_accesses_instance_field(&catch.body, field_names))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_accesses_instance_field(block, field_names))
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_accesses_instance_field(expr, field_names)),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_accesses_instance_field(block, field_names)
        }
        Stmt::LocalType(_) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_accesses_instance_field(expr: &Expr, field_names: &HashSet<String>) -> bool {
    match expr {
        Expr::MethodReference {
            receiver: Some(receiver),
            ..
        } => expr_accesses_instance_field(receiver, field_names),
        Expr::Var(name) => field_names.contains(name),
        Expr::FieldAccess(receiver, field_name) => {
            matches!(receiver.as_ref(), Expr::This) && field_names.contains(field_name)
                || expr_accesses_instance_field(receiver, field_names)
        }
        Expr::ArrayAccess(array, index) => {
            expr_accesses_instance_field(array, field_names)
                || expr_accesses_instance_field(index, field_names)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_accesses_instance_field(expr, field_names))
                || initializer.as_ref().is_some_and(|items| {
                    items
                        .iter()
                        .any(|expr| expr_accesses_instance_field(expr, field_names))
                })
        }
        Expr::ArrayInitializer(items) => items
            .iter()
            .any(|expr| expr_accesses_instance_field(expr, field_names)),
        Expr::Binary { left, right, .. } => {
            expr_accesses_instance_field(left, field_names)
                || expr_accesses_instance_field(right, field_names)
        }
        Expr::Unary { expr, .. } => expr_accesses_instance_field(expr, field_names),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_accesses_instance_field(condition, field_names)
                || expr_accesses_instance_field(then_expr, field_names)
                || expr_accesses_instance_field(else_expr, field_names)
        }
        Expr::Switch { selector, cases } => {
            expr_accesses_instance_field(selector, field_names)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_accesses_instance_field(expr, field_names)
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || expr_accesses_instance_field(&case.value, field_names)
                })
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|expr| expr_accesses_instance_field(expr, field_names))
                || args
                    .iter()
                    .any(|expr| expr_accesses_instance_field(expr, field_names))
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|expr| expr_accesses_instance_field(expr, field_names))
                || args
                    .iter()
                    .any(|expr| expr_accesses_instance_field(expr, field_names))
                || body.as_ref().is_some_and(|members| {
                    members.iter().any(|member| match member {
                        Member::Method(method) => method
                            .body
                            .as_ref()
                            .is_some_and(|body| block_accesses_instance_field(body, field_names)),
                        _ => false,
                    })
                })
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_accesses_instance_field(expr, field_names)
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                expr_accesses_instance_field(expr, field_names)
            }
            crate::java_ir::LambdaBody::Block(block) => {
                block_accesses_instance_field(block, field_names)
            }
        },
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => false,
    }
}

pub(crate) fn rust_enum_variant_name_from_parsed_constant(name: &str) -> String {
    if parsed_constant_name_is_valid_rust_variant(name) {
        return name.to_string();
    }
    let mut out = String::new();
    for part in name.split('_').filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        "Unknown".to_string()
    } else {
        out
    }
}

fn parsed_constant_name_is_valid_rust_variant(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_uppercase())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn collect_type_strategy(
    ty: &TypeDecl,
    facts: &EffectiveFacts<'_>,
    usage_requirements: &UsageDispatchRequirements,
    usage_aliases: &UsageRequirementAliases,
    plan: &mut ObjectModelPlan,
) {
    let strategy = object_strategy_for_type_with_effective_facts_and_requirements(
        ty,
        facts,
        merged_dispatch_requirements(&ty.name, facts, usage_requirements, usage_aliases),
    );
    plan.diagnostics.extend(strategy.diagnostics.clone());
    plan.insert_strategy(strategy);
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_strategy(nested, facts, usage_requirements, usage_aliases, plan);
        }
    }
}

fn collect_hierarchy_edges_for_strategy_plan(
    facts: &EffectiveFacts<'_>,
    usage_requirements: &UsageDispatchRequirements,
    usage_aliases: &UsageRequirementAliases,
    plan: &mut ObjectModelPlan,
) {
    let roots = plan.strategies.keys().cloned().collect::<Vec<_>>();
    for root in roots {
        let java_ty = JavaType::Named {
            name: root,
            args: Vec::new(),
        };
        record_object_hierarchy_edges_for_type(facts, plan, &java_ty);
        for edge in facts.hierarchy_edges_for_type(&java_ty) {
            add_direct_subtype_to_strategy(
                plan,
                &edge.super_name,
                &edge.sub_name,
                facts,
                usage_requirements,
                usage_aliases,
            );
        }
    }
}

fn record_object_hierarchy_edges_for_type(
    facts: &EffectiveFacts<'_>,
    plan: &mut ObjectModelPlan,
    java_ty: &JavaType,
) {
    let Some(sub_name) = java_type_name_for_object_model(java_ty).map(str::to_string) else {
        return;
    };
    if let Some(super_name) = facts
        .extends_type_for(java_ty)
        .as_ref()
        .and_then(java_type_name_for_object_model)
    {
        record_object_hierarchy_edge(
            plan,
            ObjectHierarchyEdge::new(
                super_name,
                sub_name.clone(),
                ObjectHierarchyEdgeKind::Extends,
            ),
        );
    }
    for super_name in facts
        .implemented_types_for(java_ty)
        .iter()
        .filter_map(java_type_name_for_object_model)
    {
        record_object_hierarchy_edge(
            plan,
            ObjectHierarchyEdge::new(
                super_name,
                sub_name.clone(),
                ObjectHierarchyEdgeKind::Implements,
            ),
        );
    }
}

fn record_object_hierarchy_edge(plan: &mut ObjectModelPlan, edge: ObjectHierarchyEdge) {
    if !plan.hierarchy_edges.contains(&edge) {
        plan.hierarchy_edges.push(edge);
    }
}

fn java_type_name_for_object_model(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::String => Some("String"),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

pub fn object_hierarchy_edge_representation_policy(
    representation: ObjectRepresentation,
) -> ObjectHierarchyEdgeRepresentationPolicy {
    let represented = matches!(
        representation,
        ObjectRepresentation::ClosedEnum { .. }
            | ObjectRepresentation::SourceHierarchyCarrier { .. }
            | ObjectRepresentation::DynTrait { .. }
            | ObjectRepresentation::ErasedObject
    );
    ObjectHierarchyEdgeRepresentationPolicy::new(representation, represented)
}

pub fn object_representation_is_user_closed_enum(
    representation: &ObjectRepresentation,
    provenance: FactProvenance,
) -> bool {
    provenance == FactProvenance::UserOverride
        && matches!(representation, ObjectRepresentation::ClosedEnum { .. })
}

pub fn object_hierarchy_edge_diagnostic_policy(
    input: ObjectHierarchyEdgeDiagnosticInput,
) -> HierarchyEdgeDiagnosticPolicy {
    let mut outcomes = Vec::new();
    let has_unsupported_extends = input.edges.iter().any(|edge| {
        edge.kind() == ObjectHierarchyEdgeKind::Extends
            && !input.represented_super_names.contains(edge.super_name())
    }) || (input.edges.is_empty()
        && input
            .declared_extends
            .as_ref()
            .is_some_and(|super_name| !input.represented_super_names.contains(super_name)));
    if has_unsupported_extends {
        outcomes.push(HierarchyEdgeDiagnosticOutcome::UnsupportedInheritance);
    }
    let unsupported_interfaces = input
        .edges
        .iter()
        .filter(|edge| edge.kind() == ObjectHierarchyEdgeKind::Implements)
        .map(|edge| edge.super_name())
        .chain(
            (input.edges.is_empty())
                .then_some(&input.declared_implements)
                .into_iter()
                .flatten()
                .map(String::as_str),
        )
        .filter(|interface_name| !input.represented_super_names.contains(*interface_name))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !unsupported_interfaces.is_empty() {
        outcomes.push(HierarchyEdgeDiagnosticOutcome::UnsupportedInterfaceImpl {
            interfaces: unsupported_interfaces,
        });
    }
    let diagnostics = outcomes
        .iter()
        .map(|outcome| render_hierarchy_edge_diagnostic_outcome(&input.type_name, outcome))
        .collect();
    HierarchyEdgeDiagnosticPolicy::new(input.type_name, outcomes, diagnostics)
}

fn render_hierarchy_edge_diagnostic_outcome(
    type_name: &str,
    outcome: &HierarchyEdgeDiagnosticOutcome,
) -> RepresentationDiagnostic {
    match outcome {
        HierarchyEdgeDiagnosticOutcome::UnsupportedInheritance => RepresentationDiagnostic::new(
            "unsupported_inheritance",
            format!(
                "{type_name} participates in an extends hierarchy, but hierarchy lowering is not implemented yet"
            ),
        ),
        HierarchyEdgeDiagnosticOutcome::UnsupportedInterfaceImpl { interfaces } => {
            RepresentationDiagnostic::new(
                "unsupported_interface_impl",
                format!(
                    "{type_name} implements interfaces without a supported representation: {}",
                    interfaces.join(", ")
                ),
            )
        }
    }
}

pub fn object_representation_lookup_policy(
    java_ty: &JavaType,
    planned_representation: Option<ObjectRepresentation>,
) -> RepresentationLookupPolicy {
    if let Some(representation) = planned_representation {
        return RepresentationLookupPolicy::new(
            java_ty.clone(),
            RepresentationLookupFallback::PlannedType,
            representation,
        );
    }
    match java_ty {
        JavaType::Named { name, .. } if name == "Object" => RepresentationLookupPolicy::new(
            java_ty.clone(),
            RepresentationLookupFallback::ConservativeErasedObject,
            ObjectRepresentation::ErasedObject,
        ),
        JavaType::Unknown(_) | JavaType::Wildcard(_) => RepresentationLookupPolicy::new(
            java_ty.clone(),
            RepresentationLookupFallback::ConservativeErasedObject,
            ObjectRepresentation::ErasedObject,
        ),
        _ => RepresentationLookupPolicy::new(
            java_ty.clone(),
            RepresentationLookupFallback::ConservativeConcrete,
            ObjectRepresentation::ConcreteStruct,
        ),
    }
}

fn add_direct_subtype_to_strategy(
    plan: &mut ObjectModelPlan,
    super_name: &str,
    subtype_name: &str,
    facts: &EffectiveFacts<'_>,
    usage_requirements: &UsageDispatchRequirements,
    usage_aliases: &UsageRequirementAliases,
) {
    let strategy = if let Some(strategy) = plan.strategy_for_mut(super_name) {
        strategy
    } else {
        plan.insert_strategy(ObjectStrategyPlan {
            root: super_name.to_string(),
            strategy: ObjectRepresentation::ErasedObject,
            requirements: merged_dispatch_requirements(
                super_name,
                facts,
                usage_requirements,
                usage_aliases,
            ),
            direct_subtypes: Vec::new(),
            source_methods_access_instance_fields: false,
            rejected: Vec::new(),
            provenance: vec![FactProvenance::ConservativeFallback],
            diagnostics: Vec::new(),
        });
        plan.strategy_for_mut(super_name)
            .expect("fallback strategy was just inserted")
    };
    if !strategy
        .direct_subtypes
        .iter()
        .any(|existing| existing == subtype_name)
    {
        strategy.direct_subtypes.push(subtype_name.to_string());
        refresh_closed_enum_strategy_variants(strategy);
    }
}

fn refresh_closed_enum_strategy_variants(strategy: &mut ObjectStrategyPlan) {
    if let ObjectRepresentation::ClosedEnum { variants, .. } = &mut strategy.strategy {
        *variants = closed_enum_variant_names_for_subtypes(&strategy.direct_subtypes);
    }
}

fn apply_closed_enum_strategy_selection(facts: &EffectiveFacts<'_>, plan: &mut ObjectModelPlan) {
    for strategy in plan.strategies.values_mut() {
        if strategy.primary_provenance() == FactProvenance::UserOverride {
            continue;
        }
        let policy = closed_enum_inference_policy_with_effective_facts_and_requirements(
            &strategy.root,
            &strategy.strategy,
            &strategy.direct_subtypes,
            facts,
            &strategy.requirements,
        );
        if let Some(representation) = policy.representation().cloned() {
            if closed_enum_is_rejected_by_requirements(&strategy.requirements) {
                strategy.rejected.push(RejectedObjectStrategy {
                    strategy: representation,
                    reason: closed_enum_requirement_rejection_reason(&strategy.root),
                });
            } else {
                strategy.strategy =
                    source_hierarchy_carrier_strategy_if_needed(strategy, representation);
                if let Some(provenance) = policy.provenance() {
                    strategy.add_provenance(provenance);
                }
                continue;
            }
        }
        if closed_runtime_case_split_is_useful(&strategy.requirements) {
            strategy.rejected.push(RejectedObjectStrategy {
                strategy: ObjectRepresentation::ClosedEnum {
                    enum_name: runtime_case_split_type_name(&strategy.root),
                    variants: closed_enum_variant_names_for_subtypes(&strategy.direct_subtypes),
                },
                reason: closed_enum_rejection_reason(
                    &strategy.root,
                    &strategy.direct_subtypes,
                    facts,
                ),
            });
        }
        apply_erased_object_requirement_fallback(strategy);
    }
}

fn closed_enum_is_rejected_by_requirements(requirements: &[DispatchRequirement]) -> bool {
    requirements.iter().any(|requirement| {
        matches!(
            requirement,
            DispatchRequirementKind::NeedsPublicExtensibleApi
                | DispatchRequirementKind::NeedsUnknownRuntimeObject
        )
    })
}

fn closed_enum_requirement_rejection_reason(type_name: &str) -> String {
    format!(
        "closed enum for {type_name} is rejected because this hierarchy has a public extensible API or unknown runtime object requirement"
    )
}

fn closed_runtime_case_split_is_useful(requirements: &[DispatchRequirement]) -> bool {
    requirements.iter().any(|requirement| {
        matches!(
            requirement,
            DispatchRequirementKind::NeedsSubtypeDispatch
                | DispatchRequirementKind::NeedsClosedRuntimeCaseSplit
                | DispatchRequirementKind::NeedsInstanceof
                | DispatchRequirementKind::NeedsDowncast
                | DispatchRequirementKind::NeedsUnknownRuntimeObject
        )
    })
}

fn closed_enum_rejection_reason(
    type_name: &str,
    direct_subtypes: &[String],
    facts: &EffectiveFacts<'_>,
) -> String {
    match closed_enum_proof_policy_with_effective_facts(type_name, direct_subtypes, facts).proof() {
        ClosedEnumProof::Proven { .. } => {
            format!("closed enum for {type_name} was not applicable to the current representation")
        }
        ClosedEnumProof::OpenWorld => format!(
            "closed enum for {type_name} requires a closed hierarchy fact before runtime case-split requirements can use named variants"
        ),
        ClosedEnumProof::NoKnownDirectSubtypes => {
            format!("closed enum for {type_name} requires at least one known direct subtype")
        }
        ClosedEnumProof::MissingPermittedSubtypes { missing } => format!(
            "closed enum for {type_name} is missing permitted subtype evidence for: {}",
            missing.join(", ")
        ),
        ClosedEnumProof::NonFinalVariants { variants } => format!(
            "closed enum for {type_name} requires final direct variants; non-final variants: {}",
            variants.join(", ")
        ),
    }
}

fn apply_erased_object_requirement_fallback(strategy: &mut ObjectStrategyPlan) {
    let Some(reason) = erased_object_requirement_reason(
        &strategy.strategy,
        &strategy.requirements,
        &strategy.direct_subtypes,
    ) else {
        return;
    };
    if matches!(strategy.strategy, ObjectRepresentation::ErasedObject) {
        return;
    }
    if parsed_source_erased_fallback_should_remain_rejected(strategy) {
        if let Some(carrier) = open_source_hierarchy_carrier_fallback(strategy) {
            strategy.rejected.push(RejectedObjectStrategy {
                strategy: ObjectRepresentation::ErasedObject,
                reason,
            });
            strategy.strategy = carrier;
            strategy.add_provenance(FactProvenance::ConservativeFallback);
            return;
        }
        strategy.rejected.push(RejectedObjectStrategy {
            strategy: ObjectRepresentation::ErasedObject,
            reason,
        });
        return;
    }
    if matches!(
        strategy.strategy,
        ObjectRepresentation::DynTrait { .. } | ObjectRepresentation::ConcreteStruct
    ) {
        strategy.rejected.push(RejectedObjectStrategy {
            strategy: strategy.strategy.clone(),
            reason,
        });
        strategy.strategy = ObjectRepresentation::ErasedObject;
        strategy.add_provenance(FactProvenance::ConservativeFallback);
    }
}

fn open_source_hierarchy_carrier_fallback(
    strategy: &ObjectStrategyPlan,
) -> Option<ObjectRepresentation> {
    if !(matches!(strategy.strategy, ObjectRepresentation::ConcreteStruct)
        && strategy.has_provenance(FactProvenance::ParsedSource)
        && source_runtime_case_split_needs_erased_carrier(strategy))
    {
        return None;
    }
    Some(ObjectRepresentation::SourceHierarchyCarrier {
        carrier_name: runtime_case_split_type_name(&strategy.root),
        variants: source_hierarchy_carrier_variant_names(strategy),
    })
}

fn source_hierarchy_carrier_strategy_if_needed(
    strategy: &ObjectStrategyPlan,
    representation: ObjectRepresentation,
) -> ObjectRepresentation {
    let ObjectRepresentation::ClosedEnum {
        enum_name,
        mut variants,
    } = representation
    else {
        return representation;
    };
    if !(strategy.has_provenance(FactProvenance::ParsedSource)
        && matches!(strategy.strategy, ObjectRepresentation::ConcreteStruct)
        && strategy.source_methods_access_instance_fields)
    {
        return ObjectRepresentation::ClosedEnum {
            enum_name,
            variants,
        };
    }
    for variant in source_hierarchy_carrier_variant_names(strategy)
        .into_iter()
        .rev()
    {
        if !variants.iter().any(|existing| existing == &variant) {
            variants.insert(0, variant);
        }
    }
    ObjectRepresentation::SourceHierarchyCarrier {
        carrier_name: enum_name,
        variants,
    }
}

fn source_hierarchy_carrier_variant_names(strategy: &ObjectStrategyPlan) -> Vec<String> {
    std::iter::once(strategy.root.as_str())
        .chain(strategy.direct_subtypes.iter().map(String::as_str))
        .map(crate::java_naming::rust_name_for_java_named_type)
        .collect()
}

fn runtime_case_split_type_name(type_name: &str) -> String {
    format!(
        "{}Value",
        crate::java_naming::rust_name_for_java_named_type(type_name)
    )
}

fn parsed_source_erased_fallback_should_remain_rejected(strategy: &ObjectStrategyPlan) -> bool {
    matches!(
        strategy.strategy,
        ObjectRepresentation::ConcreteStruct | ObjectRepresentation::DynTrait { .. }
    ) && strategy.has_provenance(FactProvenance::ParsedSource)
}

fn source_runtime_case_split_needs_erased_carrier(strategy: &ObjectStrategyPlan) -> bool {
    !strategy.direct_subtypes.is_empty()
        && strategy.requirements.iter().any(|requirement| {
            matches!(
                requirement,
                DispatchRequirementKind::NeedsSubtypeDispatch
                    | DispatchRequirementKind::NeedsInstanceof
                    | DispatchRequirementKind::NeedsDowncast
            )
        })
}

fn erased_object_requirement_reason(
    strategy: &ObjectRepresentation,
    requirements: &[DispatchRequirement],
    direct_subtypes: &[String],
) -> Option<String> {
    if matches!(strategy, ObjectRepresentation::ConcreteStruct)
        && !direct_subtypes.is_empty()
        && requirements.iter().any(|requirement| {
            matches!(
                requirement,
                DispatchRequirementKind::NeedsSubtypeDispatch
                    | DispatchRequirementKind::NeedsInstanceof
                    | DispatchRequirementKind::NeedsDowncast
            )
        })
    {
        return Some(
            "open subtype-dispatch/runtime case-split requirement needs erased object fallback when closed enum is unavailable"
                .to_string(),
        );
    }
    if requirements
        .iter()
        .any(|requirement| *requirement == DispatchRequirementKind::NeedsUnknownRuntimeObject)
    {
        return Some(
            "dyn trait representation rejected because an unknown runtime object requirement needs erased object fallback"
                .to_string(),
        );
    }
    if requirements
        .iter()
        .any(|requirement| *requirement == DispatchRequirementKind::RejectTraitObject)
    {
        return Some(
            "dyn trait representation rejected by RejectTraitObject requirement".to_string(),
        );
    }
    None
}

fn apply_hierarchy_overrides_to_strategy_plan(
    facts: &EffectiveFacts<'_>,
    plan: &mut ObjectModelPlan,
) {
    let context = object_hierarchy_context_for_strategy_plan(plan);
    for override_fact in facts.hierarchy_overrides() {
        let policy = hierarchy_override_application_policy_with_effective_facts(
            override_fact,
            &context,
            facts,
        );
        plan.diagnostics
            .extend(policy.diagnostics().iter().cloned());
        let Some(representation) = policy.representation().cloned() else {
            continue;
        };
        if let Some(strategy) = plan.strategy_for_mut(policy.type_name()) {
            if fallback_override_should_not_replace_parsed_source(override_fact, strategy) {
                continue;
            }
            strategy.strategy = representation;
            strategy.replace_provenance(policy.provenance());
            strategy.direct_subtypes = policy.direct_subtypes().to_vec();
            strategy.rejected.clear();
            continue;
        }
        if fallback_override_should_not_insert_beside_parsed_source(override_fact, plan) {
            continue;
        }
        plan.insert_strategy(ObjectStrategyPlan {
            root: policy.type_name().to_string(),
            strategy: representation,
            requirements: facts.dispatch_requirements_for(policy.type_name()),
            direct_subtypes: policy.direct_subtypes().to_vec(),
            source_methods_access_instance_fields: false,
            rejected: Vec::new(),
            provenance: vec![policy.provenance()],
            diagnostics: Vec::new(),
        });
    }
}

fn fallback_override_should_not_replace_parsed_source(
    override_fact: &HierarchyOverride,
    strategy: &ObjectStrategyPlan,
) -> bool {
    (override_fact.is_source_fallback()
        || crate::effective_facts::fact_precedence_key(override_fact.provenance)
            < crate::effective_facts::fact_precedence_key(FactProvenance::ParsedSource))
        && strategy.has_provenance(FactProvenance::ParsedSource)
}

fn fallback_override_should_not_insert_beside_parsed_source(
    override_fact: &HierarchyOverride,
    plan: &ObjectModelPlan,
) -> bool {
    override_fact.is_source_fallback()
        && plan.strategies.values().any(|strategy| {
            strategy.has_provenance(FactProvenance::ParsedSource)
                && crate::facts::external_type_name_matches(
                    &override_fact.type_name,
                    &strategy.root,
                )
        })
}

fn object_hierarchy_context_for_strategy_plan(plan: &ObjectModelPlan) -> ObjectHierarchyContext {
    let mut context = ObjectHierarchyContext::default();
    for strategy in plan.strategies.values() {
        context.insert_known_type(strategy.root.clone());
        context.insert_direct_subtypes(strategy.root.clone(), strategy.direct_subtypes.clone());
    }
    context
}

pub fn object_representation_policy_with_effective_facts(
    request: ObjectRepresentationRequest,
    facts: &EffectiveFacts<'_>,
) -> ObjectRepresentationPolicy {
    let object_safety = object_safety_policy_for_request(&request, facts);
    if object_safety == ObjectSafetyPolicy::RejectedNonObjectSafe {
        return ObjectRepresentationPolicy::new(
            object_safety,
            ObjectRepresentation::ErasedObject,
            vec![non_object_safe_trait_fallback_diagnostic(&request, facts)],
        );
    }
    let representation = match &request {
        ObjectRepresentationRequest::DefaultInterface { type_name } => {
            ObjectRepresentation::DynTrait {
                trait_name: type_name.clone(),
            }
        }
        ObjectRepresentationRequest::DynTraitOverride { trait_name, .. } => {
            ObjectRepresentation::DynTrait {
                trait_name: trait_name.clone(),
            }
        }
        ObjectRepresentationRequest::ErasedObjectOverride { .. } => {
            ObjectRepresentation::ErasedObject
        }
    };
    ObjectRepresentationPolicy::new(object_safety, representation, Vec::new())
}

pub fn object_safety_fact_policy_with_effective_facts(
    type_name: &str,
    facts: &EffectiveFacts<'_>,
) -> ObjectSafetyFactPolicy {
    ObjectSafetyFactPolicy::new(
        type_name.to_string(),
        facts.object_safe_trait_fact_for_name(type_name),
        facts.object_safety_violation_for_name(type_name),
    )
}

impl ObjectHierarchyContext {
    pub fn insert_known_type(&mut self, type_name: impl Into<String>) {
        self.known_types.insert(type_name.into());
    }

    pub fn insert_direct_subtypes(
        &mut self,
        type_name: impl Into<String>,
        direct_subtypes: Vec<String>,
    ) {
        self.direct_subtypes
            .insert(type_name.into(), direct_subtypes);
    }

    pub fn contains_type(&self, type_name: &str) -> bool {
        self.known_types.contains(type_name)
    }

    pub fn direct_subtypes_of(&self, type_name: &str) -> Vec<String> {
        self.direct_subtypes
            .get(type_name)
            .cloned()
            .unwrap_or_default()
    }

    pub fn transitive_subtypes_of(&self, type_name: &str) -> Vec<String> {
        let mut out = BTreeSet::new();
        self.collect_transitive_subtypes(type_name, &mut out);
        out.into_iter().collect()
    }

    pub fn terminal_subtypes_of(&self, type_name: &str) -> Vec<String> {
        self.transitive_subtypes_of(type_name)
            .into_iter()
            .filter(|subtype| self.direct_subtypes_of(subtype).is_empty())
            .collect()
    }

    fn collect_transitive_subtypes(&self, type_name: &str, out: &mut BTreeSet<String>) {
        for subtype in self.direct_subtypes_of(type_name) {
            if out.insert(subtype.clone()) {
                self.collect_transitive_subtypes(&subtype, out);
            }
        }
    }
}

pub fn hierarchy_override_application_policy_with_effective_facts(
    override_fact: &HierarchyOverride,
    context: &ObjectHierarchyContext,
    facts: &EffectiveFacts<'_>,
) -> HierarchyOverrideApplicationPolicy {
    let validation = hierarchy_override_validation_policy(override_fact, context);
    let direct_subtypes = hierarchy_override_represented_subtypes(override_fact, context);
    let mut diagnostics = validation.into_diagnostics();
    let representation = if context.contains_type(&override_fact.type_name) {
        let policy =
            hierarchy_override_representation_policy_with_effective_facts(override_fact, facts);
        let (representation, policy_diagnostics) = policy.into_representation_diagnostics();
        diagnostics.extend(policy_diagnostics);
        Some(representation)
    } else if hierarchy_override_introduces_boundary_root(override_fact) {
        let policy =
            hierarchy_override_representation_policy_with_effective_facts(override_fact, facts);
        let (representation, policy_diagnostics) = policy.into_representation_diagnostics();
        diagnostics.extend(policy_diagnostics);
        Some(representation)
    } else {
        None
    };

    HierarchyOverrideApplicationPolicy::new(
        override_fact.type_name.clone(),
        representation,
        override_fact.provenance,
        direct_subtypes,
        diagnostics,
    )
}

fn hierarchy_override_represented_subtypes(
    override_fact: &HierarchyOverride,
    context: &ObjectHierarchyContext,
) -> Vec<String> {
    match &override_fact.strategy {
        HierarchyStrategy::ClosedEnum { variants, .. } => {
            let known_subtypes = context.transitive_subtypes_of(&override_fact.type_name);
            variants
                .iter()
                .map(|variant| {
                    known_subtypes
                        .iter()
                        .find(|subtype| hierarchy_name_matches(variant, subtype))
                        .cloned()
                        .unwrap_or_else(|| variant.clone())
                })
                .collect()
        }
        _ => context.direct_subtypes_of(&override_fact.type_name),
    }
}

pub fn hierarchy_override_validation_policy(
    override_fact: &HierarchyOverride,
    context: &ObjectHierarchyContext,
) -> HierarchyOverrideValidationPolicy {
    let mut diagnostics = Vec::new();
    if !context.contains_type(&override_fact.type_name)
        && !hierarchy_override_introduces_boundary_root(override_fact)
    {
        diagnostics.push(representation_override_conflict_diagnostic(format!(
            "hierarchy override references unknown Java type {}",
            override_fact.type_name
        )));
    }
    if let HierarchyStrategy::ClosedEnum { variants, .. } = &override_fact.strategy {
        push_closed_enum_override_validation_diagnostics(
            &mut diagnostics,
            override_fact,
            variants,
            context,
        );
    }
    HierarchyOverrideValidationPolicy::new(override_fact.type_name.clone(), diagnostics)
}

fn hierarchy_override_introduces_boundary_root(override_fact: &HierarchyOverride) -> bool {
    match override_fact.provenance {
        FactProvenance::ExternalLibraryPack => matches!(
            override_fact.strategy,
            HierarchyStrategy::ConcreteStruct
                | HierarchyStrategy::DynTrait { .. }
                | HierarchyStrategy::ErasedObject
        ),
        FactProvenance::CorpusAnalysis => {
            matches!(override_fact.strategy, HierarchyStrategy::ErasedObject)
        }
        FactProvenance::ParsedSource
        | FactProvenance::BuiltinExternalModel
        | FactProvenance::UserOverride
        | FactProvenance::ConservativeFallback => false,
    }
}

pub fn hierarchy_override_representation_policy_with_effective_facts(
    override_fact: &HierarchyOverride,
    facts: &EffectiveFacts<'_>,
) -> HierarchyOverrideRepresentationPolicy {
    match &override_fact.strategy {
        HierarchyStrategy::ConcreteStruct => HierarchyOverrideRepresentationPolicy::new(
            ObjectRepresentation::ConcreteStruct,
            ObjectSafetyPolicy::NotRequired,
            Vec::new(),
        ),
        HierarchyStrategy::ClosedEnum {
            enum_name,
            variants,
        } => HierarchyOverrideRepresentationPolicy::new(
            ObjectRepresentation::ClosedEnum {
                enum_name: enum_name.clone(),
                variants: variants.clone(),
            },
            ObjectSafetyPolicy::NotRequired,
            Vec::new(),
        ),
        HierarchyStrategy::DynTrait { trait_name } => {
            hierarchy_override_representation_from_object_policy(
                object_representation_policy_with_effective_facts(
                    ObjectRepresentationRequest::DynTraitOverride {
                        type_name: override_fact.type_name.clone(),
                        trait_name: trait_name.clone(),
                    },
                    facts,
                ),
            )
        }
        HierarchyStrategy::ErasedObject => hierarchy_override_representation_from_object_policy(
            object_representation_policy_with_effective_facts(
                ObjectRepresentationRequest::ErasedObjectOverride {
                    type_name: override_fact.type_name.clone(),
                },
                facts,
            ),
        ),
    }
}

pub fn closed_enum_inference_policy_with_effective_facts(
    type_name: &str,
    current_representation: &ObjectRepresentation,
    direct_subtypes: &[String],
    facts: &EffectiveFacts<'_>,
) -> ClosedEnumInferencePolicy {
    closed_enum_inference_policy_with_effective_facts_and_requirements(
        type_name,
        current_representation,
        direct_subtypes,
        facts,
        &facts.dispatch_requirements_for(type_name),
    )
}

fn closed_enum_inference_policy_with_effective_facts_and_requirements(
    type_name: &str,
    current_representation: &ObjectRepresentation,
    direct_subtypes: &[String],
    facts: &EffectiveFacts<'_>,
    requirements: &[DispatchRequirement],
) -> ClosedEnumInferencePolicy {
    let proof_policy =
        closed_enum_proof_policy_with_effective_facts(type_name, direct_subtypes, facts);
    let variants = match proof_policy.proof() {
        ClosedEnumProof::Proven { variants, .. } => {
            Some(closed_enum_variant_names_for_subtypes(variants))
        }
        _ => None,
    };
    let representation = variants.and_then(|variants| match current_representation {
        ObjectRepresentation::DynTrait { .. } | ObjectRepresentation::ErasedObject => {
            Some(ObjectRepresentation::ClosedEnum {
                enum_name: runtime_case_split_type_name(type_name),
                variants,
            })
        }
        ObjectRepresentation::ConcreteStruct
            if closed_runtime_case_split_is_useful(requirements) =>
        {
            Some(ObjectRepresentation::ClosedEnum {
                enum_name: runtime_case_split_type_name(type_name),
                variants,
            })
        }
        ObjectRepresentation::ConcreteStruct
        | ObjectRepresentation::JavaEnum { .. }
        | ObjectRepresentation::ClosedEnum { .. }
        | ObjectRepresentation::SourceHierarchyCarrier { .. } => None,
    });
    let provenance = representation
        .is_some()
        .then_some(FactProvenance::CorpusAnalysis);
    ClosedEnumInferencePolicy::new(type_name.to_string(), representation, provenance)
}

pub(crate) fn closed_enum_variant_names_for_subtypes(subtypes: &[String]) -> Vec<String> {
    subtypes
        .iter()
        .map(|subtype| crate::java_naming::rust_name_for_java_named_type(subtype))
        .collect()
}

pub fn closed_enum_proof_policy_with_effective_facts(
    type_name: &str,
    direct_subtypes: &[String],
    facts: &EffectiveFacts<'_>,
) -> ClosedEnumProofPolicy {
    let proof = match facts.closed_hierarchy_fact_for_name_with_provenance(type_name) {
        Some(closed_fact) if *closed_fact.value() => {
            if let Some(permits_fact) = facts.sealed_permitted_subtypes_for(type_name) {
                let missing = missing_permitted_subtypes(permits_fact.value(), direct_subtypes);
                if !missing.is_empty() {
                    ClosedEnumProof::MissingPermittedSubtypes { missing }
                } else {
                    closed_enum_terminal_variant_proof(
                        direct_subtypes,
                        facts,
                        closed_fact.provenance(),
                    )
                }
            } else {
                closed_enum_terminal_variant_proof(direct_subtypes, facts, closed_fact.provenance())
            }
        }
        _ => ClosedEnumProof::OpenWorld,
    };
    ClosedEnumProofPolicy::new(type_name.to_string(), proof)
}

fn closed_enum_terminal_variant_proof(
    direct_subtypes: &[String],
    facts: &EffectiveFacts<'_>,
    provenance: FactProvenance,
) -> ClosedEnumProof {
    if direct_subtypes.is_empty() {
        return ClosedEnumProof::NoKnownDirectSubtypes;
    }
    let non_final_variants = direct_subtypes
        .iter()
        .filter(|subtype| {
            !facts.is_final_type(&JavaType::Named {
                name: (*subtype).clone(),
                args: Vec::new(),
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    if non_final_variants.is_empty() {
        ClosedEnumProof::Proven {
            variants: direct_subtypes.to_vec(),
            provenance,
        }
    } else {
        ClosedEnumProof::NonFinalVariants {
            variants: non_final_variants,
        }
    }
}

fn missing_permitted_subtypes(permits: &[String], direct_subtypes: &[String]) -> Vec<String> {
    permits
        .iter()
        .filter(|permitted| {
            !direct_subtypes
                .iter()
                .any(|direct| hierarchy_name_matches(permitted, direct))
        })
        .cloned()
        .collect()
}

fn hierarchy_name_matches(left: &str, right: &str) -> bool {
    left == right || simple_hierarchy_name(left) == simple_hierarchy_name(right)
}

fn simple_hierarchy_name(name: &str) -> &str {
    let without_generics = name.split_once('<').map_or(name, |(prefix, _)| prefix);
    without_generics
        .rsplit(['.', '$'])
        .next()
        .unwrap_or(without_generics)
}

fn hierarchy_override_representation_from_object_policy(
    policy: ObjectRepresentationPolicy,
) -> HierarchyOverrideRepresentationPolicy {
    let (object_safety, representation, diagnostics) = policy.into_parts();
    HierarchyOverrideRepresentationPolicy::new(representation, object_safety, diagnostics)
}

fn push_closed_enum_override_validation_diagnostics(
    diagnostics: &mut Vec<RepresentationDiagnostic>,
    override_fact: &HierarchyOverride,
    variants: &[String],
    context: &ObjectHierarchyContext,
) {
    let known_subtypes = context.transitive_subtypes_of(&override_fact.type_name);
    for variant in variants {
        let known_variant = known_subtypes
            .iter()
            .any(|subtype| hierarchy_name_matches(variant, subtype));
        if !context.contains_type(variant) && !known_variant {
            diagnostics.push(representation_override_conflict_diagnostic(format!(
                "closed enum override for {} references unknown variant {}",
                override_fact.type_name, variant
            )));
        } else if !known_variant {
            diagnostics.push(representation_override_conflict_diagnostic(format!(
                "closed enum override for {} includes {}, but source facts do not record it as a subtype",
                override_fact.type_name, variant
            )));
        }
    }
    for subtype in context.terminal_subtypes_of(&override_fact.type_name) {
        if !variants
            .iter()
            .any(|variant| hierarchy_name_matches(variant, &subtype))
        {
            diagnostics.push(representation_override_conflict_diagnostic(format!(
                "closed enum override for {} omits terminal source subtype {}",
                override_fact.type_name, subtype
            )));
        }
    }
}

fn representation_override_conflict_diagnostic(message: String) -> RepresentationDiagnostic {
    RepresentationDiagnostic::new("representation_override_conflict", message)
}

fn object_safety_policy_for_request(
    request: &ObjectRepresentationRequest,
    facts: &EffectiveFacts<'_>,
) -> ObjectSafetyPolicy {
    if !object_representation_request_requires_object_safety(request) {
        return ObjectSafetyPolicy::NotRequired;
    }
    let type_name = object_representation_request_type_name(request);
    let fact_policy = object_safety_fact_policy_with_effective_facts(type_name, facts);
    match fact_policy.object_safe_trait {
        Some(true) => ObjectSafetyPolicy::ProvenObjectSafe,
        Some(false) => ObjectSafetyPolicy::RejectedNonObjectSafe,
        None => ObjectSafetyPolicy::AssumedObjectSafe,
    }
}

fn object_representation_request_requires_object_safety(
    request: &ObjectRepresentationRequest,
) -> bool {
    matches!(
        request,
        ObjectRepresentationRequest::DefaultInterface { .. }
            | ObjectRepresentationRequest::DynTraitOverride { .. }
    )
}

fn object_representation_request_type_name(request: &ObjectRepresentationRequest) -> &str {
    match request {
        ObjectRepresentationRequest::DefaultInterface { type_name }
        | ObjectRepresentationRequest::DynTraitOverride { type_name, .. }
        | ObjectRepresentationRequest::ErasedObjectOverride { type_name } => type_name,
    }
}

fn non_object_safe_trait_fallback_diagnostic(
    request: &ObjectRepresentationRequest,
    facts: &EffectiveFacts<'_>,
) -> RepresentationDiagnostic {
    let type_name = object_representation_request_type_name(request);
    let fact_policy = object_safety_fact_policy_with_effective_facts(type_name, facts);
    let reason = fact_policy
        .violation
        .as_ref()
        .map(object_safety_violation_message);
    let message = match request {
        ObjectRepresentationRequest::DefaultInterface { type_name } => {
            let type_name = object_safety_diagnostic_type_name(type_name);
            let mut message = format!(
                "interface {type_name} is marked non-object-safe; falling back to erased object representation"
            );
            if let Some(reason) = reason {
                message.push_str(&format!(" ({reason})"));
            }
            message
        }
        ObjectRepresentationRequest::DynTraitOverride { type_name, .. } => {
            let mut message = format!(
                "dyn trait representation for {type_name} requires object safety; falling back to erased object representation"
            );
            if let Some(reason) = reason {
                message.push_str(&format!(" ({reason})"));
            }
            message
        }
        ObjectRepresentationRequest::ErasedObjectOverride { type_name } => {
            format!("erased object representation for {type_name} does not require object safety")
        }
    };
    RepresentationDiagnostic::new("non_object_safe_trait_fallback", message)
}

fn object_safety_diagnostic_type_name(type_name: &str) -> &str {
    let Some((prefix, simple)) = type_name.rsplit_once('.') else {
        return type_name;
    };
    if prefix
        .split('.')
        .any(|segment| segment.chars().next().is_some_and(char::is_lowercase))
    {
        type_name
    } else {
        simple
    }
}

fn object_safety_violation_message(violation: &ObjectSafetyViolation) -> String {
    match violation {
        ObjectSafetyViolation::ExplicitNonObjectSafeFact { provenance } => {
            format!("explicit non-object-safe trait fact from {provenance:?}")
        }
        ObjectSafetyViolation::StaticMethod { method_name } => {
            format!("static interface method {method_name}")
        }
        ObjectSafetyViolation::SelfReferentialReturn { method_name } => {
            format!("method {method_name} returns the interface type")
        }
        ObjectSafetyViolation::NestedSelfReferentialReturn { method_name } => {
            format!("method {method_name} returns a type containing the interface type")
        }
        ObjectSafetyViolation::SelfReferentialParameter {
            method_name,
            param_name,
        } => format!("method {method_name} parameter {param_name} mentions the interface type"),
        ObjectSafetyViolation::NestedSelfReferentialParameter {
            method_name,
            param_name,
        } => format!(
            "method {method_name} parameter {param_name} contains the interface type in a nested type"
        ),
        ObjectSafetyViolation::FieldMember { field_name } => {
            format!("interface field {field_name}")
        }
        ObjectSafetyViolation::ConstructorMember => "interface constructor member".to_string(),
        ObjectSafetyViolation::UnsupportedMember => "unsupported interface member".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding_id::{BindingId, BindingKind};
    use crate::usage::{MethodUsageFacts, UsageMethodKey};

    fn qualified_new_expr(qualifier: Expr, args: Vec<Expr>) -> Expr {
        Expr::New {
            ty: JavaType::Unknown("Inner".to_string()),
            infer_type_arguments: false,
            qualifier: Some(Box::new(qualifier)),
            args,
            body: None,
            resolved: None,
        }
    }

    #[test]
    fn instance_field_access_scan_sees_qualified_new_receiver() {
        let expr = qualified_new_expr(
            Expr::FieldAccess(Box::new(Expr::This), "state".to_string()),
            Vec::new(),
        );
        let field_names = HashSet::from(["state".to_string()]);

        assert!(expr_accesses_instance_field(&expr, &field_names));
    }

    #[test]
    fn dispatch_requirements_include_shadowed_binding_id_facts() {
        let hidden_binding = BindingId::new(BindingKind::Local, "value", 0);
        let visible_binding = BindingId::new(BindingKind::Local, "value", 1);
        let method = MethodUsageFacts::from_exact_binding_facts([
            BindingUseFacts::new(
                "value",
                Some(hidden_binding),
                JavaType::Named {
                    name: "Hidden".to_string(),
                    args: Vec::new(),
                },
                vec![BindingUse::Returned],
            ),
            BindingUseFacts::new(
                "value",
                Some(visible_binding),
                JavaType::Named {
                    name: "Visible".to_string(),
                    args: Vec::new(),
                },
                Vec::new(),
            ),
        ]);

        let mut usage = UsageFacts::default();
        usage.insert_method_usage(
            UsageMethodKey::current_rust_method_name("Owner", "run"),
            method,
        );

        let requirements = dispatch_requirements_from_usage(&usage);
        assert_eq!(
            requirements.get("Hidden"),
            Some(&vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage])
        );
        assert!(
            !requirements.contains_key("Visible"),
            "visible binding has no dispatch-relevant use"
        );
    }

    #[test]
    fn parsed_java_enum_constant_names_preserve_rust_valid_uppercase_constants() {
        assert_eq!(rust_enum_variant_name_from_parsed_constant("s_n_p"), "SNP");
        assert_eq!(
            rust_enum_variant_name_from_parsed_constant("MAX_COUNT_EXCEEDED"),
            "MAX_COUNT_EXCEEDED"
        );
    }
}
