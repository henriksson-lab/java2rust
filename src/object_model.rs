use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::effective_facts::EffectiveFacts;
use crate::facts::{
    DispatchRequirementKind, FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy,
    ObjectSafetyViolation,
};
use crate::java_ir::{Block, Expr, JavaType, Member, Program, Stmt, TypeDecl, TypeKind};
use crate::usage::{BindingUse, BindingUseFacts, CallArgumentBoundary, UsageFacts};

#[derive(Debug, Clone, Copy)]
pub struct ObjectModelInput<'a> {
    pub program: &'a Program,
    pub facts: &'a FactSet,
    pub effective_facts: &'a EffectiveFacts<'a>,
    pub usage: Option<&'a UsageFacts>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectModelPlan {
    pub strategies: BTreeMap<String, ObjectStrategyPlan>,
    pub hierarchy_edges: Vec<ObjectHierarchyEdge>,
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStrategyPlan {
    pub root: String,
    pub strategy: ObjectRepresentation,
    pub requirements: Vec<DispatchRequirement>,
    pub direct_subtypes: Vec<String>,
    pub source_methods_access_instance_fields: bool,
    pub rejected: Vec<RejectedObjectStrategy>,
    pub provenance: Vec<FactProvenance>,
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

pub type DispatchRequirement = DispatchRequirementKind;
pub type UsageDispatchRequirements = BTreeMap<String, Vec<DispatchRequirement>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedObjectStrategy {
    pub strategy: ObjectRepresentation,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectHierarchyContext {
    known_types: BTreeSet<String>,
    direct_subtypes: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHierarchyEdge {
    pub super_name: String,
    pub sub_name: String,
    pub kind: ObjectHierarchyEdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHierarchyEdgeRepresentationPolicy {
    pub representation: ObjectRepresentation,
    pub represented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHierarchyEdgeDiagnosticInput {
    pub type_name: String,
    pub declared_extends: Option<String>,
    pub declared_implements: Vec<String>,
    pub edges: Vec<ObjectHierarchyEdge>,
    pub represented_super_names: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyEdgeDiagnosticPolicy {
    pub type_name: String,
    pub outcomes: Vec<HierarchyEdgeDiagnosticOutcome>,
    pub diagnostics: Vec<RepresentationDiagnostic>,
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
    pub type_name: String,
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyOverrideApplicationPolicy {
    pub type_name: String,
    pub representation: Option<ObjectRepresentation>,
    pub provenance: FactProvenance,
    pub direct_subtypes: Vec<String>,
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyOverrideRepresentationPolicy {
    pub representation: ObjectRepresentation,
    pub object_safety: ObjectSafetyPolicy,
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedEnumInferencePolicy {
    pub type_name: String,
    pub representation: Option<ObjectRepresentation>,
    pub provenance: Option<FactProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedEnumProofPolicy {
    pub type_name: String,
    pub proof: ClosedEnumProof,
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
    pub object_safety: ObjectSafetyPolicy,
    pub representation: ObjectRepresentation,
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationLookupPolicy {
    pub java_ty: JavaType,
    pub fallback: RepresentationLookupFallback,
    pub representation: ObjectRepresentation,
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
    pub type_name: String,
    pub object_safe_trait: Option<bool>,
    pub violation: Option<ObjectSafetyViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationDiagnostic {
    pub code: &'static str,
    pub message: String,
}

pub fn plan_object_model_with_effective_facts(
    program: &Program,
    facts: &EffectiveFacts<'_>,
) -> ObjectModelPlan {
    plan_object_model_from_effective_facts(program, facts)
}

pub fn plan_object_model_from_input(input: ObjectModelInput<'_>) -> ObjectModelPlan {
    let _facts = input.facts;
    let usage_requirements = input
        .usage
        .map(dispatch_requirements_from_usage)
        .unwrap_or_default();
    plan_object_model_from_effective_facts_and_usage(
        input.program,
        input.effective_facts,
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
    for ty in &program.types {
        collect_type_strategy(ty, facts, usage_requirements, &mut plan);
    }
    collect_hierarchy_edges_for_strategy_plan(facts, usage_requirements, &mut plan);
    apply_hierarchy_overrides_to_strategy_plan(facts, &mut plan);
    apply_closed_enum_strategy_selection(facts, &mut plan);
    plan
}

pub fn dispatch_requirements_from_usage(usage: &UsageFacts) -> UsageDispatchRequirements {
    let mut requirements = UsageDispatchRequirements::new();
    for method in usage.methods.values().chain(usage.constructors.values()) {
        for binding in method.all_binding_facts() {
            let Some(type_name) = dispatch_requirement_type_name(&binding.java_ty) else {
                continue;
            };
            for use_fact in &binding.uses {
                for requirement in dispatch_requirements_for_binding_use(binding, use_fact) {
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
) -> Vec<DispatchRequirement> {
    match use_fact {
        BindingUse::StoredInField { .. } | BindingUse::Returned => {
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
                    &binding.java_ty,
                ) {
                    vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage]
                } else {
                    vec![DispatchRequirementKind::NeedsUnknownRuntimeObject]
                }
            }
        },
        BindingUse::AssignedFromCall { boundary, .. } => match boundary {
            CallArgumentBoundary::InternalSameType | CallArgumentBoundary::KnownContentRead => {
                vec![DispatchRequirementKind::NeedsBorrowedTraitView]
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
        BindingUse::ContentRead | BindingUse::AssignedToLocal { .. } | BindingUse::Mutated => {
            Vec::new()
        }
    }
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
) -> Vec<DispatchRequirement> {
    let mut requirements = facts.dispatch_requirements_for(type_name);
    if let Some(usage) = usage_requirements.get(type_name) {
        for requirement in usage {
            if !requirements.contains(requirement) {
                requirements.push(*requirement);
            }
        }
    }
    requirements
}

impl ObjectModelPlan {
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

fn object_strategy_for_type_with_effective_facts_and_requirements(
    ty: &TypeDecl,
    facts: &EffectiveFacts<'_>,
    requirements: Vec<DispatchRequirement>,
) -> ObjectStrategyPlan {
    let explicit_hierarchy_override = facts
        .hierarchy_overrides()
        .iter()
        .find(|override_fact| override_fact.type_name == ty.name);
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
            (policy.representation, policy.diagnostics)
        }
        TypeKind::Interface => {
            let policy = object_representation_policy_with_effective_facts(
                ObjectRepresentationRequest::DefaultInterface {
                    type_name: ty.name.clone(),
                },
                facts,
            );
            (policy.representation, policy.diagnostics)
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
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if field.ty == named_type(&ty.name) => {
                Some(rust_enum_variant_name_from_parsed_constant(&field.name))
            }
            _ => None,
        })
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
        Expr::New { args, body, .. } => {
            args.iter()
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

pub fn rust_enum_variant_name_from_parsed_constant(name: &str) -> String {
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

fn named_type(name: &str) -> JavaType {
    JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    }
}

fn collect_type_strategy(
    ty: &TypeDecl,
    facts: &EffectiveFacts<'_>,
    usage_requirements: &UsageDispatchRequirements,
    plan: &mut ObjectModelPlan,
) {
    let strategy = object_strategy_for_type_with_effective_facts_and_requirements(
        ty,
        facts,
        merged_dispatch_requirements(&ty.name, facts, usage_requirements),
    );
    plan.diagnostics.extend(strategy.diagnostics.clone());
    plan.insert_strategy(strategy);
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_strategy(nested, facts, usage_requirements, plan);
        }
    }
}

fn collect_hierarchy_edges_for_strategy_plan(
    facts: &EffectiveFacts<'_>,
    usage_requirements: &UsageDispatchRequirements,
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
            ObjectHierarchyEdge {
                super_name: super_name.to_string(),
                sub_name: sub_name.clone(),
                kind: ObjectHierarchyEdgeKind::Extends,
            },
        );
    }
    for super_name in facts
        .implemented_types_for(java_ty)
        .iter()
        .filter_map(java_type_name_for_object_model)
    {
        record_object_hierarchy_edge(
            plan,
            ObjectHierarchyEdge {
                super_name: super_name.to_string(),
                sub_name: sub_name.clone(),
                kind: ObjectHierarchyEdgeKind::Implements,
            },
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
    ObjectHierarchyEdgeRepresentationPolicy {
        representation,
        represented,
    }
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
        edge.kind == ObjectHierarchyEdgeKind::Extends
            && !input.represented_super_names.contains(&edge.super_name)
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
        .filter(|edge| edge.kind == ObjectHierarchyEdgeKind::Implements)
        .map(|edge| &edge.super_name)
        .chain(
            (input.edges.is_empty())
                .then_some(&input.declared_implements)
                .into_iter()
                .flatten(),
        )
        .filter(|interface_name| !input.represented_super_names.contains(*interface_name))
        .cloned()
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
    HierarchyEdgeDiagnosticPolicy {
        type_name: input.type_name,
        outcomes,
        diagnostics,
    }
}

fn render_hierarchy_edge_diagnostic_outcome(
    type_name: &str,
    outcome: &HierarchyEdgeDiagnosticOutcome,
) -> RepresentationDiagnostic {
    match outcome {
        HierarchyEdgeDiagnosticOutcome::UnsupportedInheritance => RepresentationDiagnostic {
            code: "unsupported_inheritance",
            message: format!(
                "{type_name} participates in an extends hierarchy, but hierarchy lowering is not implemented yet"
            ),
        },
        HierarchyEdgeDiagnosticOutcome::UnsupportedInterfaceImpl { interfaces } => {
            RepresentationDiagnostic {
                code: "unsupported_interface_impl",
                message: format!(
                    "{type_name} implements interfaces without a supported representation: {}",
                    interfaces.join(", ")
                ),
            }
        }
    }
}

pub fn object_representation_lookup_policy(
    java_ty: &JavaType,
    planned_representation: Option<ObjectRepresentation>,
) -> RepresentationLookupPolicy {
    if let Some(representation) = planned_representation {
        return RepresentationLookupPolicy {
            java_ty: java_ty.clone(),
            fallback: RepresentationLookupFallback::PlannedType,
            representation,
        };
    }
    match java_ty {
        JavaType::Named { name, .. } if name == "Object" => RepresentationLookupPolicy {
            java_ty: java_ty.clone(),
            fallback: RepresentationLookupFallback::ConservativeErasedObject,
            representation: ObjectRepresentation::ErasedObject,
        },
        JavaType::Unknown(_) | JavaType::Wildcard(_) => RepresentationLookupPolicy {
            java_ty: java_ty.clone(),
            fallback: RepresentationLookupFallback::ConservativeErasedObject,
            representation: ObjectRepresentation::ErasedObject,
        },
        _ => RepresentationLookupPolicy {
            java_ty: java_ty.clone(),
            fallback: RepresentationLookupFallback::ConservativeConcrete,
            representation: ObjectRepresentation::ConcreteStruct,
        },
    }
}

fn add_direct_subtype_to_strategy(
    plan: &mut ObjectModelPlan,
    super_name: &str,
    subtype_name: &str,
    facts: &EffectiveFacts<'_>,
    usage_requirements: &UsageDispatchRequirements,
) {
    let strategy = if let Some(strategy) = plan.strategy_for_mut(super_name) {
        strategy
    } else {
        plan.insert_strategy(ObjectStrategyPlan {
            root: super_name.to_string(),
            strategy: ObjectRepresentation::ErasedObject,
            requirements: merged_dispatch_requirements(super_name, facts, usage_requirements),
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
        if let Some(representation) = policy.representation {
            if closed_enum_is_rejected_by_requirements(&strategy.requirements) {
                strategy.rejected.push(RejectedObjectStrategy {
                    strategy: representation,
                    reason: closed_enum_requirement_rejection_reason(&strategy.root),
                });
            } else {
                strategy.strategy =
                    source_hierarchy_carrier_strategy_if_needed(strategy, representation);
                if let Some(provenance) = policy.provenance {
                    strategy.add_provenance(provenance);
                }
                continue;
            }
        }
        if closed_runtime_case_split_is_useful(&strategy.requirements) {
            strategy.rejected.push(RejectedObjectStrategy {
                strategy: ObjectRepresentation::ClosedEnum {
                    enum_name: format!("{}Value", strategy.root),
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
            DispatchRequirementKind::NeedsClosedRuntimeCaseSplit
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
    match closed_enum_proof_policy_with_effective_facts(type_name, direct_subtypes, facts).proof {
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
        && strategy.source_methods_access_instance_fields
        && source_runtime_case_split_needs_erased_carrier(strategy))
    {
        return None;
    }
    Some(ObjectRepresentation::SourceHierarchyCarrier {
        carrier_name: format!("{}Value", strategy.root),
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
        .map(crate::slot_render::rust_name_for_java_named_type)
        .collect()
}

fn parsed_source_erased_fallback_should_remain_rejected(strategy: &ObjectStrategyPlan) -> bool {
    matches!(strategy.strategy, ObjectRepresentation::ConcreteStruct)
        && strategy.has_provenance(FactProvenance::ParsedSource)
        && (strategy.source_methods_access_instance_fields
            || !source_runtime_case_split_needs_erased_carrier(strategy))
}

fn source_runtime_case_split_needs_erased_carrier(strategy: &ObjectStrategyPlan) -> bool {
    !strategy.direct_subtypes.is_empty()
        && strategy.requirements.iter().any(|requirement| {
            matches!(
                requirement,
                DispatchRequirementKind::NeedsInstanceof | DispatchRequirementKind::NeedsDowncast
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
                DispatchRequirementKind::NeedsInstanceof | DispatchRequirementKind::NeedsDowncast
            )
        })
    {
        return Some(
            "open runtime case-split requirement needs erased object fallback when closed enum is unavailable"
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
        plan.diagnostics.extend(policy.diagnostics.clone());
        let Some(representation) = policy.representation else {
            continue;
        };
        if let Some(strategy) = plan.strategy_for_mut(&policy.type_name) {
            if erased_override_should_not_replace_parsed_source(override_fact, strategy) {
                continue;
            }
            strategy.strategy = representation;
            strategy.replace_provenance(policy.provenance);
            strategy.direct_subtypes = policy.direct_subtypes;
            strategy.rejected.clear();
            continue;
        }
        if erased_override_should_not_insert_beside_parsed_source(override_fact, plan) {
            continue;
        }
        plan.insert_strategy(ObjectStrategyPlan {
            root: policy.type_name.clone(),
            strategy: representation,
            requirements: facts.dispatch_requirements_for(&policy.type_name),
            direct_subtypes: policy.direct_subtypes,
            source_methods_access_instance_fields: false,
            rejected: Vec::new(),
            provenance: vec![policy.provenance],
            diagnostics: Vec::new(),
        });
    }
}

fn erased_override_should_not_replace_parsed_source(
    override_fact: &HierarchyOverride,
    strategy: &ObjectStrategyPlan,
) -> bool {
    !matches!(override_fact.provenance, FactProvenance::UserOverride)
        && matches!(override_fact.strategy, HierarchyStrategy::ErasedObject)
        && strategy.has_provenance(FactProvenance::ParsedSource)
}

fn erased_override_should_not_insert_beside_parsed_source(
    override_fact: &HierarchyOverride,
    plan: &ObjectModelPlan,
) -> bool {
    !matches!(override_fact.provenance, FactProvenance::UserOverride)
        && matches!(override_fact.strategy, HierarchyStrategy::ErasedObject)
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
        return ObjectRepresentationPolicy {
            diagnostics: vec![non_object_safe_trait_fallback_diagnostic(&request, facts)],
            object_safety,
            representation: ObjectRepresentation::ErasedObject,
        };
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
    ObjectRepresentationPolicy {
        object_safety,
        representation,
        diagnostics: Vec::new(),
    }
}

pub fn object_safety_fact_policy_with_effective_facts(
    type_name: &str,
    facts: &EffectiveFacts<'_>,
) -> ObjectSafetyFactPolicy {
    ObjectSafetyFactPolicy {
        type_name: type_name.to_string(),
        object_safe_trait: facts.object_safe_trait_fact_for_name(type_name),
        violation: facts.object_safety_violation_for_name(type_name),
    }
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
    let mut diagnostics = validation.diagnostics;
    let representation = if context.contains_type(&override_fact.type_name) {
        let policy =
            hierarchy_override_representation_policy_with_effective_facts(override_fact, facts);
        diagnostics.extend(policy.diagnostics);
        Some(policy.representation)
    } else if hierarchy_override_introduces_boundary_root(override_fact) {
        let policy =
            hierarchy_override_representation_policy_with_effective_facts(override_fact, facts);
        diagnostics.extend(policy.diagnostics);
        Some(policy.representation)
    } else {
        None
    };

    HierarchyOverrideApplicationPolicy {
        type_name: override_fact.type_name.clone(),
        representation,
        provenance: override_fact.provenance,
        direct_subtypes,
        diagnostics,
    }
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
    HierarchyOverrideValidationPolicy {
        type_name: override_fact.type_name.clone(),
        diagnostics,
    }
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
        HierarchyStrategy::ConcreteStruct => HierarchyOverrideRepresentationPolicy {
            object_safety: ObjectSafetyPolicy::NotRequired,
            representation: ObjectRepresentation::ConcreteStruct,
            diagnostics: Vec::new(),
        },
        HierarchyStrategy::ClosedEnum {
            enum_name,
            variants,
        } => HierarchyOverrideRepresentationPolicy {
            object_safety: ObjectSafetyPolicy::NotRequired,
            representation: ObjectRepresentation::ClosedEnum {
                enum_name: enum_name.clone(),
                variants: variants.clone(),
            },
            diagnostics: Vec::new(),
        },
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
    let variants = match &proof_policy.proof {
        ClosedEnumProof::Proven { variants, .. } => {
            Some(closed_enum_variant_names_for_subtypes(variants))
        }
        _ => None,
    };
    let representation = variants.and_then(|variants| match current_representation {
        ObjectRepresentation::DynTrait { .. } | ObjectRepresentation::ErasedObject => {
            Some(ObjectRepresentation::ClosedEnum {
                enum_name: format!("{type_name}Value"),
                variants,
            })
        }
        ObjectRepresentation::ConcreteStruct
            if closed_runtime_case_split_is_useful(requirements) =>
        {
            Some(ObjectRepresentation::ClosedEnum {
                enum_name: format!("{type_name}Value"),
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
    ClosedEnumInferencePolicy {
        type_name: type_name.to_string(),
        representation,
        provenance,
    }
}

pub(crate) fn closed_enum_variant_names_for_subtypes(subtypes: &[String]) -> Vec<String> {
    subtypes
        .iter()
        .map(|subtype| crate::slot_render::rust_name_for_java_named_type(subtype))
        .collect()
}

pub fn closed_enum_proof_policy_with_effective_facts(
    type_name: &str,
    direct_subtypes: &[String],
    facts: &EffectiveFacts<'_>,
) -> ClosedEnumProofPolicy {
    let proof = match facts.closed_hierarchy_fact_for_name_with_provenance(type_name) {
        Some(closed_fact) if closed_fact.value => {
            if let Some(permits_fact) = facts.sealed_permitted_subtypes_for(type_name) {
                let missing = missing_permitted_subtypes(&permits_fact.value, direct_subtypes);
                if !missing.is_empty() {
                    ClosedEnumProof::MissingPermittedSubtypes { missing }
                } else {
                    closed_enum_terminal_variant_proof(
                        direct_subtypes,
                        facts,
                        closed_fact.provenance,
                    )
                }
            } else {
                closed_enum_terminal_variant_proof(direct_subtypes, facts, closed_fact.provenance)
            }
        }
        _ => ClosedEnumProof::OpenWorld,
    };
    ClosedEnumProofPolicy {
        type_name: type_name.to_string(),
        proof,
    }
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
    HierarchyOverrideRepresentationPolicy {
        representation: policy.representation,
        object_safety: policy.object_safety,
        diagnostics: policy.diagnostics,
    }
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
    RepresentationDiagnostic {
        code: "representation_override_conflict",
        message,
    }
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
    RepresentationDiagnostic {
        code: "non_object_safe_trait_fallback",
        message,
    }
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

    #[test]
    fn dispatch_requirements_include_shadowed_binding_id_facts() {
        let hidden_binding = BindingId::new(BindingKind::Local, "value", 0);
        let visible_binding = BindingId::new(BindingKind::Local, "value", 1);
        let mut method = MethodUsageFacts::default();
        method.bindings_by_id.insert(
            hidden_binding.clone(),
            BindingUseFacts {
                name: "value".to_string(),
                binding: Some(hidden_binding),
                java_ty: JavaType::Named {
                    name: "Hidden".to_string(),
                    args: Vec::new(),
                },
                uses: vec![BindingUse::Returned],
            },
        );
        let visible = BindingUseFacts {
            name: "value".to_string(),
            binding: Some(visible_binding.clone()),
            java_ty: JavaType::Named {
                name: "Visible".to_string(),
                args: Vec::new(),
            },
            uses: Vec::new(),
        };
        method
            .bindings_by_id
            .insert(visible_binding, visible.clone());
        method.bindings.insert("value".to_string(), visible);

        let mut usage = UsageFacts::default();
        usage.methods.insert(
            UsageMethodKey {
                type_name: "Owner".to_string(),
                method_name: "run".to_string(),
            },
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
}
