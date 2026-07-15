use std::collections::{BTreeSet, HashMap};

use crate::effective_facts::EffectiveFacts;
use crate::facts::{FactProvenance, FactSet, HierarchyEdgeFact};
use crate::java_ir::{Block, Expr, JavaType, LambdaBody, Member, Program, Stmt, TypeDecl};
pub use crate::object_model::{
    ClosedEnumFactPolicy, ClosedEnumInferencePolicy, HierarchyEdgeDiagnosticOutcome,
    HierarchyEdgeDiagnosticPolicy, HierarchyOverrideApplicationPolicy,
    HierarchyOverrideRepresentationPolicy, HierarchyOverrideValidationPolicy,
    ObjectHierarchyContext, ObjectHierarchyEdge, ObjectHierarchyEdgeDiagnosticInput,
    ObjectHierarchyEdgeKind, ObjectModelInput, ObjectRepresentation, ObjectRepresentationPolicy,
    ObjectRepresentationRequest, ObjectSafetyFactPolicy, ObjectSafetyPolicy,
    RepresentationDiagnostic, RepresentationLookupFallback, RepresentationLookupPolicy,
    closed_enum_fact_policy, closed_enum_inference_policy, hierarchy_override_application_policy,
    hierarchy_override_representation_policy, hierarchy_override_validation_policy,
    object_hierarchy_edge_diagnostic_policy, object_hierarchy_edge_representation_policy,
    object_representation_lookup_policy, object_representation_policy, object_safety_fact_policy,
    plan_object_model_from_input,
};
use crate::slot_shape::DispatchShape;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepresentationPlan {
    types: HashMap<String, TypeRepresentation>,
    hierarchy_edges_by_subtype: HashMap<String, Vec<ObjectHierarchyEdge>>,
    diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRepresentation {
    pub java_name: String,
    pub representation: ObjectRepresentation,
    pub provenance: RepresentationProvenance,
    pub direct_subtypes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentationProvenance {
    ParsedSource,
    BuiltinExternalModel,
    CorpusAnalysis,
    UserOverride,
    ConservativeFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationDispatchPolicy {
    pub representation: ObjectRepresentation,
    pub dispatch: DispatchShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationHierarchyFactPolicy {
    pub java_ty: JavaType,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub edges: Vec<HierarchyEdgeFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousClassDiagnosticPolicy {
    pub diagnostics: Vec<RepresentationDiagnostic>,
}

pub fn plan_representations(program: &Program, facts: &FactSet) -> RepresentationPlan {
    let mut plan = RepresentationPlan::default();
    let effective_facts = EffectiveFacts::new(facts);
    let object_plan = plan_object_model_from_input(ObjectModelInput {
        program,
        facts,
        effective_facts: &effective_facts,
        usage: None,
    });
    plan.diagnostics.extend(object_plan.diagnostics);
    for strategy in object_plan.strategies.into_values() {
        let provenance = representation_provenance_for_fact(strategy.primary_provenance());
        plan.types.insert(
            strategy.root.clone(),
            TypeRepresentation {
                java_name: strategy.root,
                representation: strategy.strategy,
                provenance,
                direct_subtypes: strategy.direct_subtypes,
            },
        );
    }
    for edge in object_plan.hierarchy_edges {
        record_hierarchy_edge(&mut plan, edge);
    }
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
        let planned_representation = match java_ty {
            JavaType::Named { name, .. } => self
                .types
                .get(name)
                .map(|entry| entry.representation.clone()),
            _ => None,
        };
        object_representation_lookup_policy(java_ty, planned_representation)
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

    fn hierarchy_edges_for_subtype(&self, type_name: &str) -> &[ObjectHierarchyEdge] {
        self.hierarchy_edges_by_subtype
            .get(type_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn hierarchy_edge_is_represented(&self, super_name: &str) -> bool {
        self.types.get(super_name).is_some_and(|entry| {
            object_hierarchy_edge_representation_policy(entry.representation.clone()).represented
        })
    }

    fn hierarchy_edge_diagnostic_input(&self, ty: &TypeDecl) -> ObjectHierarchyEdgeDiagnosticInput {
        let edges = self.hierarchy_edges_for_subtype(&ty.name).to_vec();
        let represented_super_names = edges
            .iter()
            .map(|edge| &edge.super_name)
            .chain(ty.extends.iter())
            .chain(ty.implements.iter())
            .filter(|super_name| self.hierarchy_edge_is_represented(super_name))
            .cloned()
            .collect::<BTreeSet<_>>();
        ObjectHierarchyEdgeDiagnosticInput {
            type_name: ty.name.clone(),
            declared_extends: ty.extends.clone(),
            declared_implements: ty.implements.clone(),
            edges,
            represented_super_names,
        }
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
    object_hierarchy_edge_diagnostic_policy(plan.hierarchy_edge_diagnostic_input(ty))
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

pub fn representation_hierarchy_fact_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> RepresentationHierarchyFactPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    representation_hierarchy_fact_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn representation_hierarchy_fact_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
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

fn representation_provenance_for_fact(provenance: FactProvenance) -> RepresentationProvenance {
    match provenance {
        FactProvenance::ParsedSource => RepresentationProvenance::ParsedSource,
        FactProvenance::BuiltinExternalModel | FactProvenance::ExternalLibraryPack => {
            RepresentationProvenance::BuiltinExternalModel
        }
        FactProvenance::CorpusAnalysis => RepresentationProvenance::CorpusAnalysis,
        FactProvenance::UserOverride => RepresentationProvenance::UserOverride,
        FactProvenance::ConservativeFallback => RepresentationProvenance::ConservativeFallback,
    }
}

fn java_type_name_for_representation(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::String => Some("String"),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

pub fn object_hierarchy_context(plan: &RepresentationPlan) -> ObjectHierarchyContext {
    let mut context = ObjectHierarchyContext::default();
    for (type_name, entry) in &plan.types {
        context.insert_known_type(type_name.clone());
        context.insert_direct_subtypes(type_name.clone(), entry.direct_subtypes.clone());
    }
    context
}

fn record_hierarchy_edge(plan: &mut RepresentationPlan, edge: ObjectHierarchyEdge) {
    add_direct_subtype(plan, &edge.super_name, &edge.sub_name);
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
