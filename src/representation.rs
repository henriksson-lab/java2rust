use std::collections::HashMap;

use crate::facts::{
    FactProvenance, FactSet, HierarchyEdgeFact, HierarchyOverride, HierarchyStrategy,
    ObjectSafetyViolation,
};
use crate::java_ir::{
    Block, Expr, JavaType, LambdaBody, Member, Program, Stmt, TypeDecl, TypeKind,
};
use crate::slot_shape::DispatchShape;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepresentationPlan {
    types: HashMap<String, TypeRepresentation>,
    hierarchy_edges_by_subtype: HashMap<String, Vec<RepresentationHierarchyEdge>>,
    diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRepresentation {
    pub java_name: String,
    pub representation: ObjectRepresentation,
    pub provenance: RepresentationProvenance,
    pub direct_subtypes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepresentationHierarchyEdge {
    super_name: String,
    sub_name: String,
    kind: RepresentationHierarchyEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepresentationHierarchyEdgeKind {
    Extends,
    Implements,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectRepresentation {
    ConcreteStruct,
    DynTrait {
        trait_name: String,
    },
    BoxedDynTrait {
        trait_name: String,
    },
    ClosedEnum {
        enum_name: String,
        variants: Vec<String>,
    },
    ErasedObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentationProvenance {
    ParsedSource,
    CorpusAnalysis,
    UserOverride,
    ConservativeFallback,
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
    BoxedDynTraitOverride {
        type_name: String,
        trait_name: String,
    },
    ErasedObjectOverride {
        type_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRepresentationPolicy {
    pub request: ObjectRepresentationRequest,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationDispatchPolicy {
    pub representation: ObjectRepresentation,
    pub dispatch: DispatchShape,
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
pub struct HierarchyOverrideValidationPolicy {
    pub type_name: String,
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyOverrideApplicationPolicy {
    pub type_name: String,
    pub representation: Option<ObjectRepresentation>,
    pub provenance: RepresentationProvenance,
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
pub struct RepresentationHierarchyFactPolicy {
    pub java_ty: JavaType,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub edges: Vec<HierarchyEdgeFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedEnumInferencePolicy {
    pub type_name: String,
    pub representation: Option<ObjectRepresentation>,
    pub provenance: Option<RepresentationProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedEnumFactPolicy {
    pub type_name: String,
    pub variants: Option<Vec<String>>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousClassDiagnosticPolicy {
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationDiagnostic {
    pub code: &'static str,
    pub message: String,
}

pub fn plan_representations(program: &Program, facts: &FactSet) -> RepresentationPlan {
    let mut plan = RepresentationPlan::default();
    for ty in &program.types {
        collect_type_representation(ty, facts, &mut plan);
    }
    for ty in all_types(program) {
        collect_hierarchy_edges(ty, facts, &mut plan);
    }
    apply_inferred_closed_enum_representations(facts, &mut plan);
    apply_hierarchy_overrides(facts, &mut plan);
    plan
}

impl RepresentationPlan {
    pub fn type_representation(&self, type_name: &str) -> Option<&TypeRepresentation> {
        self.types.get(type_name)
    }

    pub fn representation_for(&self, java_ty: &JavaType) -> ObjectRepresentation {
        self.representation_lookup_policy(java_ty).representation
    }

    pub fn representation_lookup_policy(&self, java_ty: &JavaType) -> RepresentationLookupPolicy {
        match java_ty {
            JavaType::Named { name, .. } => {
                if let Some(entry) = self.types.get(name) {
                    RepresentationLookupPolicy {
                        java_ty: java_ty.clone(),
                        fallback: RepresentationLookupFallback::PlannedType,
                        representation: entry.representation.clone(),
                    }
                } else {
                    RepresentationLookupPolicy {
                        java_ty: java_ty.clone(),
                        fallback: RepresentationLookupFallback::ConservativeConcrete,
                        representation: ObjectRepresentation::ConcreteStruct,
                    }
                }
            }
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

    pub fn direct_subtypes_of(&self, type_name: &str) -> &[String] {
        self.types
            .get(type_name)
            .map(|entry| entry.direct_subtypes.as_slice())
            .unwrap_or(&[])
    }

    pub fn dispatch_for(&self, java_ty: &JavaType) -> DispatchShape {
        representation_dispatch_policy(self.representation_for(java_ty)).dispatch
    }

    pub fn diagnostics_for_program(&self, program: &Program) -> Vec<RepresentationDiagnostic> {
        self.diagnostics
            .iter()
            .cloned()
            .chain(
                all_types(program)
                    .into_iter()
                    .flat_map(|ty| self.diagnostics_for_type(ty)),
            )
            .chain(anonymous_class_diagnostic_policy(program).diagnostics)
            .collect()
    }

    pub fn diagnostics_for_type(&self, ty: &TypeDecl) -> Vec<RepresentationDiagnostic> {
        hierarchy_edge_diagnostic_policy(ty, self).diagnostics
    }

    fn hierarchy_edges_for_subtype(&self, type_name: &str) -> &[RepresentationHierarchyEdge] {
        self.hierarchy_edges_by_subtype
            .get(type_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn hierarchy_edge_is_represented(&self, super_name: &str) -> bool {
        matches!(
            self.types
                .get(super_name)
                .map(|entry| &entry.representation),
            Some(
                ObjectRepresentation::ClosedEnum { .. }
                    | ObjectRepresentation::DynTrait { .. }
                    | ObjectRepresentation::BoxedDynTrait { .. }
                    | ObjectRepresentation::ErasedObject
            )
        )
    }
}

pub fn representation_dispatch_policy(
    representation: ObjectRepresentation,
) -> RepresentationDispatchPolicy {
    let projected_dispatch = match &representation {
        ObjectRepresentation::ConcreteStruct => DispatchShape::Concrete,
        ObjectRepresentation::DynTrait { trait_name } => DispatchShape::DynTrait {
            trait_name: trait_name.clone(),
        },
        ObjectRepresentation::BoxedDynTrait { trait_name } => DispatchShape::BoxedDynTrait {
            trait_name: trait_name.clone(),
        },
        ObjectRepresentation::ClosedEnum { enum_name, .. } => DispatchShape::ClosedEnum {
            enum_name: enum_name.clone(),
        },
        ObjectRepresentation::ErasedObject => DispatchShape::ErasedObject,
    };
    RepresentationDispatchPolicy {
        representation,
        dispatch: projected_dispatch,
    }
}

pub fn hierarchy_edge_diagnostic_policy(
    ty: &TypeDecl,
    plan: &RepresentationPlan,
) -> HierarchyEdgeDiagnosticPolicy {
    let plan_edges = plan.hierarchy_edges_for_subtype(&ty.name);
    let mut outcomes = Vec::new();
    let has_unsupported_extends = plan_edges.iter().any(|edge| {
        edge.kind == RepresentationHierarchyEdgeKind::Extends
            && !plan.hierarchy_edge_is_represented(&edge.super_name)
    }) || (plan_edges.is_empty()
        && ty
            .extends
            .as_ref()
            .is_some_and(|super_name| !plan.hierarchy_edge_is_represented(super_name)));
    if has_unsupported_extends {
        outcomes.push(HierarchyEdgeDiagnosticOutcome::UnsupportedInheritance);
    }
    let unsupported_interfaces = plan_edges
        .iter()
        .filter(|edge| edge.kind == RepresentationHierarchyEdgeKind::Implements)
        .map(|edge| &edge.super_name)
        .chain(
            (plan_edges.is_empty())
                .then_some(&ty.implements)
                .into_iter()
                .flatten(),
        )
        .filter(|interface_name| !plan.hierarchy_edge_is_represented(interface_name))
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported_interfaces.is_empty() {
        outcomes.push(HierarchyEdgeDiagnosticOutcome::UnsupportedInterfaceImpl {
            interfaces: unsupported_interfaces,
        });
    }
    let diagnostics = outcomes
        .iter()
        .map(|outcome| render_hierarchy_edge_diagnostic_outcome(&ty.name, outcome))
        .collect();
    HierarchyEdgeDiagnosticPolicy {
        type_name: ty.name.clone(),
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

pub fn anonymous_class_diagnostic_policy(program: &Program) -> AnonymousClassDiagnosticPolicy {
    let mut diagnostics = Vec::new();
    for ty in all_types(program) {
        collect_anonymous_class_diagnostics_for_type(ty, &mut diagnostics);
    }
    AnonymousClassDiagnosticPolicy { diagnostics }
}

fn collect_anonymous_class_diagnostics_for_type(
    ty: &TypeDecl,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_anonymous_class_diagnostics_for_expr(initializer, diagnostics);
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_anonymous_class_diagnostics_for_block(body, diagnostics);
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_anonymous_class_diagnostics_for_block(body, diagnostics);
                }
            }
            Member::Type(nested) => {
                collect_anonymous_class_diagnostics_for_type(nested, diagnostics);
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_anonymous_class_diagnostics_for_block(
    block: &Block,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    for stmt in &block.statements {
        collect_anonymous_class_diagnostics_for_stmt(stmt, diagnostics);
    }
}

fn collect_anonymous_class_diagnostics_for_stmt(
    stmt: &Stmt,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_anonymous_class_diagnostics_for_expr(initializer, diagnostics);
            }
        }
        Stmt::Assign { target, value } => {
            collect_anonymous_class_diagnostics_for_expr(target, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(value, diagnostics);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_anonymous_class_diagnostics_for_expr(condition, diagnostics);
            collect_anonymous_class_diagnostics_for_block(then_block, diagnostics);
            if let Some(else_block) = else_block {
                collect_anonymous_class_diagnostics_for_block(else_block, diagnostics);
            }
        }
        Stmt::While { condition, body } => {
            collect_anonymous_class_diagnostics_for_expr(condition, diagnostics);
            collect_anonymous_class_diagnostics_for_block(body, diagnostics);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_anonymous_class_diagnostics_for_stmt(stmt, diagnostics);
            }
            if let Some(condition) = condition {
                collect_anonymous_class_diagnostics_for_expr(condition, diagnostics);
            }
            for expr in update {
                collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
            }
            collect_anonymous_class_diagnostics_for_block(body, diagnostics);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_anonymous_class_diagnostics_for_expr(iterable, diagnostics);
            collect_anonymous_class_diagnostics_for_block(body, diagnostics);
        }
        Stmt::Switch { selector, cases } => {
            collect_anonymous_class_diagnostics_for_expr(selector, diagnostics);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
                    }
                }
                for stmt in &case.statements {
                    collect_anonymous_class_diagnostics_for_stmt(stmt, diagnostics);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                collect_anonymous_class_diagnostics_for_stmt(resource, diagnostics);
            }
            collect_anonymous_class_diagnostics_for_block(body, diagnostics);
            for catch in catches {
                collect_anonymous_class_diagnostics_for_block(&catch.body, diagnostics);
            }
            if let Some(finally_block) = finally_block {
                collect_anonymous_class_diagnostics_for_block(finally_block, diagnostics);
            }
        }
        Stmt::LocalType(ty) => collect_anonymous_class_diagnostics_for_type(ty, diagnostics),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, diagnostics);
            }
        }
        Stmt::Block(block) => collect_anonymous_class_diagnostics_for_block(block, diagnostics),
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_anonymous_class_diagnostics_for_expr(
    expr: &Expr,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            collect_anonymous_class_diagnostics_for_expr(receiver, diagnostics);
        }
        Expr::ArrayAccess(array, index) => {
            collect_anonymous_class_diagnostics_for_expr(array, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(index, diagnostics);
        }
        Expr::ArrayCreation { dimensions, .. } | Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_anonymous_class_diagnostics_for_expr(left, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(right, diagnostics);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_anonymous_class_diagnostics_for_expr(condition, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(then_expr, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(else_expr, diagnostics);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_anonymous_class_diagnostics_for_expr(receiver, diagnostics);
            }
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, diagnostics);
            }
        }
        Expr::New { ty, args, body, .. } => {
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, diagnostics);
            }
            if body.is_some() {
                diagnostics.push(RepresentationDiagnostic {
                    code: "unsupported_anonymous_class",
                    message: format!(
                        "anonymous Java class for {:?} requires object representation lowering",
                        ty
                    ),
                });
            }
        }
        Expr::InstanceOf { expr, .. } => {
            collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_anonymous_class_diagnostics_for_expr(expr, diagnostics)
            }
            LambdaBody::Block(block) => {
                collect_anonymous_class_diagnostics_for_block(block, diagnostics);
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

fn collect_type_representation(ty: &TypeDecl, facts: &FactSet, plan: &mut RepresentationPlan) {
    let representation = match ty.kind {
        TypeKind::Class => ObjectRepresentation::ConcreteStruct,
        TypeKind::Interface if has_explicit_hierarchy_override(facts, &ty.name) => {
            ObjectRepresentation::DynTrait {
                trait_name: ty.name.clone(),
            }
        }
        TypeKind::Interface => {
            let policy = object_representation_policy(
                ObjectRepresentationRequest::DefaultInterface {
                    type_name: ty.name.clone(),
                },
                facts,
            );
            plan.diagnostics.extend(policy.diagnostics);
            policy.representation
        }
        TypeKind::Enum => ObjectRepresentation::ClosedEnum {
            enum_name: ty.name.clone(),
            variants: Vec::new(),
        },
    };
    plan.types.insert(
        ty.name.clone(),
        TypeRepresentation {
            java_name: ty.name.clone(),
            representation,
            provenance: RepresentationProvenance::ParsedSource,
            direct_subtypes: Vec::new(),
        },
    );
    for member in &ty.members {
        if let crate::java_ir::Member::Type(nested) = member {
            collect_type_representation(nested, facts, plan);
        }
    }
}

fn has_explicit_hierarchy_override(facts: &FactSet, type_name: &str) -> bool {
    facts
        .hierarchy_overrides
        .iter()
        .any(|override_fact| override_fact.type_name == type_name)
}

fn collect_hierarchy_edges(ty: &TypeDecl, facts: &FactSet, plan: &mut RepresentationPlan) {
    let java_ty = JavaType::Named {
        name: ty.name.clone(),
        args: Vec::new(),
    };
    let policy = representation_hierarchy_fact_policy(&java_ty, facts);
    if let Some(super_name) = policy.extends {
        record_hierarchy_edge(
            plan,
            RepresentationHierarchyEdge {
                super_name,
                sub_name: ty.name.clone(),
                kind: RepresentationHierarchyEdgeKind::Extends,
            },
        );
    }
    for interface_name in policy.implements {
        record_hierarchy_edge(
            plan,
            RepresentationHierarchyEdge {
                super_name: interface_name,
                sub_name: ty.name.clone(),
                kind: RepresentationHierarchyEdgeKind::Implements,
            },
        );
    }
    for edge in policy.edges {
        add_direct_subtype(plan, &edge.super_name, &edge.sub_name);
    }
}

pub fn representation_hierarchy_fact_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> RepresentationHierarchyFactPolicy {
    RepresentationHierarchyFactPolicy {
        java_ty: java_ty.clone(),
        extends: facts
            .extends_type_for(java_ty)
            .as_ref()
            .and_then(java_type_name_for_representation)
            .map(str::to_string),
        implements: facts
            .implemented_types_for(java_ty)
            .iter()
            .filter_map(java_type_name_for_representation)
            .map(str::to_string)
            .collect(),
        edges: facts.hierarchy_edges_for_type(java_ty),
    }
}

fn apply_hierarchy_overrides(facts: &FactSet, plan: &mut RepresentationPlan) {
    for override_fact in &facts.hierarchy_overrides {
        let policy = hierarchy_override_application_policy(override_fact, plan, facts);
        plan.diagnostics.extend(policy.diagnostics);
        let Some(representation) = policy.representation else {
            continue;
        };
        plan.types.insert(
            policy.type_name.clone(),
            TypeRepresentation {
                java_name: policy.type_name,
                representation,
                provenance: policy.provenance,
                direct_subtypes: policy.direct_subtypes,
            },
        );
    }
}

pub fn hierarchy_override_application_policy(
    override_fact: &HierarchyOverride,
    plan: &RepresentationPlan,
    facts: &FactSet,
) -> HierarchyOverrideApplicationPolicy {
    let validation = hierarchy_override_validation_policy(override_fact, plan);
    let direct_subtypes = plan
        .types
        .get(&override_fact.type_name)
        .map(|entry| entry.direct_subtypes.clone())
        .unwrap_or_default();
    let mut diagnostics = validation.diagnostics;
    let representation = if plan.types.contains_key(&override_fact.type_name) {
        let policy = hierarchy_override_representation_policy(override_fact, facts);
        diagnostics.extend(policy.diagnostics);
        Some(policy.representation)
    } else {
        None
    };

    HierarchyOverrideApplicationPolicy {
        type_name: override_fact.type_name.clone(),
        representation,
        provenance: representation_provenance_for_fact(override_fact.provenance),
        direct_subtypes,
        diagnostics,
    }
}

fn hierarchy_override_representation_policy(
    override_fact: &HierarchyOverride,
    facts: &FactSet,
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
            hierarchy_override_representation_from_object_policy(object_representation_policy(
                ObjectRepresentationRequest::DynTraitOverride {
                    type_name: override_fact.type_name.clone(),
                    trait_name: trait_name.clone(),
                },
                facts,
            ))
        }
        HierarchyStrategy::BoxedDynTrait { trait_name } => {
            hierarchy_override_representation_from_object_policy(object_representation_policy(
                ObjectRepresentationRequest::BoxedDynTraitOverride {
                    type_name: override_fact.type_name.clone(),
                    trait_name: trait_name.clone(),
                },
                facts,
            ))
        }
        HierarchyStrategy::ErasedObject => {
            hierarchy_override_representation_from_object_policy(object_representation_policy(
                ObjectRepresentationRequest::ErasedObjectOverride {
                    type_name: override_fact.type_name.clone(),
                },
                facts,
            ))
        }
    }
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

fn representation_provenance_for_fact(provenance: FactProvenance) -> RepresentationProvenance {
    match provenance {
        FactProvenance::ParsedSource => RepresentationProvenance::ParsedSource,
        FactProvenance::CorpusAnalysis => RepresentationProvenance::CorpusAnalysis,
        FactProvenance::UserOverride => RepresentationProvenance::UserOverride,
        FactProvenance::ConservativeFallback => RepresentationProvenance::ConservativeFallback,
    }
}

fn apply_inferred_closed_enum_representations(facts: &FactSet, plan: &mut RepresentationPlan) {
    let inferred = plan
        .types
        .iter()
        .map(|(type_name, entry)| closed_enum_inference_policy(type_name, entry, facts))
        .filter(|policy| policy.representation.is_some())
        .collect::<Vec<_>>();

    for policy in inferred {
        if let Some(entry) = plan.types.get_mut(&policy.type_name) {
            if let Some(representation) = policy.representation {
                entry.representation = representation;
            }
            if let Some(provenance) = policy.provenance {
                entry.provenance = provenance;
            }
        }
    }
}

pub fn closed_enum_inference_policy(
    type_name: &str,
    entry: &TypeRepresentation,
    facts: &FactSet,
) -> ClosedEnumInferencePolicy {
    let fact_policy = closed_enum_fact_policy(type_name, entry, facts);
    let representation = fact_policy
        .variants
        .and_then(|variants| match entry.representation {
            ObjectRepresentation::DynTrait { .. } | ObjectRepresentation::ErasedObject => {
                Some(ObjectRepresentation::ClosedEnum {
                    enum_name: format!("{type_name}Value"),
                    variants,
                })
            }
            ObjectRepresentation::ConcreteStruct
            | ObjectRepresentation::BoxedDynTrait { .. }
            | ObjectRepresentation::ClosedEnum { .. } => None,
        });
    let provenance = representation
        .is_some()
        .then_some(RepresentationProvenance::CorpusAnalysis);
    ClosedEnumInferencePolicy {
        type_name: type_name.to_string(),
        representation,
        provenance,
    }
}

pub fn closed_enum_fact_policy(
    type_name: &str,
    entry: &TypeRepresentation,
    facts: &FactSet,
) -> ClosedEnumFactPolicy {
    ClosedEnumFactPolicy {
        type_name: type_name.to_string(),
        variants: facts.closed_hierarchy_variants_for(type_name, &entry.direct_subtypes),
    }
}

pub fn object_representation_policy(
    request: ObjectRepresentationRequest,
    facts: &FactSet,
) -> ObjectRepresentationPolicy {
    let object_safety = object_safety_policy_for_request(&request, facts);
    if object_safety == ObjectSafetyPolicy::RejectedNonObjectSafe {
        return ObjectRepresentationPolicy {
            diagnostics: vec![non_object_safe_trait_fallback_diagnostic(&request, facts)],
            request,
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
        ObjectRepresentationRequest::BoxedDynTraitOverride { trait_name, .. } => {
            ObjectRepresentation::BoxedDynTrait {
                trait_name: trait_name.clone(),
            }
        }
        ObjectRepresentationRequest::ErasedObjectOverride { .. } => {
            ObjectRepresentation::ErasedObject
        }
    };
    ObjectRepresentationPolicy {
        request,
        object_safety,
        representation,
        diagnostics: Vec::new(),
    }
}

fn object_safety_policy_for_request(
    request: &ObjectRepresentationRequest,
    facts: &FactSet,
) -> ObjectSafetyPolicy {
    if !object_representation_request_requires_object_safety(request) {
        return ObjectSafetyPolicy::NotRequired;
    }
    let type_name = object_representation_request_type_name(request);
    let fact_policy = object_safety_fact_policy(type_name, facts);
    match fact_policy.object_safe_trait {
        Some(true) => ObjectSafetyPolicy::ProvenObjectSafe,
        Some(false) => ObjectSafetyPolicy::RejectedNonObjectSafe,
        None => ObjectSafetyPolicy::AssumedObjectSafe,
    }
}

pub fn object_safety_fact_policy(type_name: &str, facts: &FactSet) -> ObjectSafetyFactPolicy {
    ObjectSafetyFactPolicy {
        type_name: type_name.to_string(),
        object_safe_trait: facts.object_safe_trait_fact_for_name(type_name),
        violation: facts.object_safety_violation_for_name(type_name),
    }
}

fn object_representation_request_requires_object_safety(
    request: &ObjectRepresentationRequest,
) -> bool {
    matches!(
        request,
        ObjectRepresentationRequest::DefaultInterface { .. }
            | ObjectRepresentationRequest::DynTraitOverride { .. }
            | ObjectRepresentationRequest::BoxedDynTraitOverride { .. }
    )
}

fn object_representation_request_type_name(request: &ObjectRepresentationRequest) -> &str {
    match request {
        ObjectRepresentationRequest::DefaultInterface { type_name }
        | ObjectRepresentationRequest::DynTraitOverride { type_name, .. }
        | ObjectRepresentationRequest::BoxedDynTraitOverride { type_name, .. }
        | ObjectRepresentationRequest::ErasedObjectOverride { type_name } => type_name,
    }
}

fn non_object_safe_trait_fallback_diagnostic(
    request: &ObjectRepresentationRequest,
    facts: &FactSet,
) -> RepresentationDiagnostic {
    let type_name = object_representation_request_type_name(request);
    let fact_policy = object_safety_fact_policy(type_name, facts);
    let reason = fact_policy
        .violation
        .as_ref()
        .map(object_safety_violation_message);
    let message = match request {
        ObjectRepresentationRequest::DefaultInterface { type_name } => {
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
        ObjectRepresentationRequest::BoxedDynTraitOverride { type_name, .. } => {
            let mut message = format!(
                "boxed dyn trait representation for {type_name} requires object safety; falling back to erased object representation"
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

pub fn hierarchy_override_validation_policy(
    override_fact: &crate::facts::HierarchyOverride,
    plan: &RepresentationPlan,
) -> HierarchyOverrideValidationPolicy {
    let mut diagnostics = Vec::new();
    if !plan.types.contains_key(&override_fact.type_name) {
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
            plan,
        );
    }
    HierarchyOverrideValidationPolicy {
        type_name: override_fact.type_name.clone(),
        diagnostics,
    }
}

fn push_closed_enum_override_validation_diagnostics(
    diagnostics: &mut Vec<RepresentationDiagnostic>,
    override_fact: &crate::facts::HierarchyOverride,
    variants: &[String],
    plan: &RepresentationPlan,
) {
    let known_subtypes = plan
        .types
        .get(&override_fact.type_name)
        .map(|entry| entry.direct_subtypes.clone())
        .unwrap_or_default();
    for variant in variants {
        if !plan.types.contains_key(variant) {
            diagnostics.push(representation_override_conflict_diagnostic(format!(
                "closed enum override for {} references unknown variant {}",
                override_fact.type_name, variant
            )));
        } else if !known_subtypes.iter().any(|subtype| subtype == variant) {
            diagnostics.push(representation_override_conflict_diagnostic(format!(
                "closed enum override for {} includes {}, but source facts do not record it as a direct subtype",
                override_fact.type_name, variant
            )));
        }
    }
    for subtype in known_subtypes {
        if !variants.iter().any(|variant| variant == &subtype) {
            diagnostics.push(representation_override_conflict_diagnostic(format!(
                "closed enum override for {} omits source subtype {}",
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

fn java_type_name_for_representation(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::String => Some("String"),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn record_hierarchy_edge(plan: &mut RepresentationPlan, edge: RepresentationHierarchyEdge) {
    let edges = plan
        .hierarchy_edges_by_subtype
        .entry(edge.sub_name.clone())
        .or_default();
    if !edges.contains(&edge) {
        edges.push(edge);
    }
}

fn add_direct_subtype(plan: &mut RepresentationPlan, super_name: &str, subtype_name: &str) {
    let entry = plan
        .types
        .entry(super_name.to_string())
        .or_insert_with(|| TypeRepresentation {
            java_name: super_name.to_string(),
            representation: ObjectRepresentation::ErasedObject,
            provenance: RepresentationProvenance::ConservativeFallback,
            direct_subtypes: Vec::new(),
        });
    if !entry
        .direct_subtypes
        .iter()
        .any(|existing| existing == subtype_name)
    {
        entry.direct_subtypes.push(subtype_name.to_string());
    }
}

fn all_types(program: &Program) -> Vec<&TypeDecl> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_types(ty, &mut types);
    }
    types
}

fn collect_types<'a>(ty: &'a TypeDecl, out: &mut Vec<&'a TypeDecl>) {
    out.push(ty);
    for member in &ty.members {
        if let crate::java_ir::Member::Type(nested) = member {
            collect_types(nested, out);
        }
    }
}
