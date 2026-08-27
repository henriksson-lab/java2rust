use std::collections::{BTreeSet, HashMap};

use crate::dependency_mode::DependencyMode;
use crate::effective_facts::EffectiveFacts;
use crate::facts::{FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy};
use crate::java_ir::{
    Block, Expr, JavaType, LambdaBody, Member, Program, Stmt, SwitchLabel, TypeDecl,
};
pub use crate::object_model::{
    ClosedEnumInferencePolicy, ClosedEnumProof, ClosedEnumProofPolicy,
    HierarchyEdgeDiagnosticOutcome, HierarchyEdgeDiagnosticPolicy,
    HierarchyOverrideApplicationPolicy, HierarchyOverrideRepresentationPolicy,
    HierarchyOverrideValidationPolicy, ObjectHierarchyContext, ObjectHierarchyEdge,
    ObjectHierarchyEdgeDiagnosticInput, ObjectHierarchyEdgeKind, ObjectModelInput,
    ObjectRepresentation, ObjectRepresentationPolicy, ObjectRepresentationRequest,
    ObjectSafetyFactPolicy, ObjectSafetyPolicy, RepresentationDiagnostic,
    RepresentationLookupFallback, RepresentationLookupPolicy, closed_enum_inference_policy,
    closed_enum_proof_policy, hierarchy_override_application_policy,
    hierarchy_override_representation_policy, hierarchy_override_validation_policy,
    object_hierarchy_edge_diagnostic_policy, object_hierarchy_edge_representation_policy,
    object_representation_is_user_closed_enum, object_representation_lookup_policy,
    object_representation_policy, object_safety_fact_policy, plan_object_model_from_input,
};
use crate::slot_shape::DispatchShape;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepresentationPlan {
    types: HashMap<String, TypeRepresentation>,
    hierarchy_edges_by_subtype: HashMap<String, Vec<ObjectHierarchyEdge>>,
    diagnostics: Vec<RepresentationDiagnostic>,
    /// How each type's values are *held* — by value, or behind a shared handle.
    ///
    /// Carried here because this plan is the one both signature solving and
    /// lowering already consult, and a storage decision that only lowering can
    /// see would let the two disagree about a type's Rust type. Step 2 of
    /// [0002b](../claude_audits/0002b-shared-representation-design.md) computed
    /// this plan and reported it *after* lowering; nothing reads it yet, and
    /// moving it in front of representation planning is the step that makes it
    /// readable at all.
    storage: crate::storage_strategy::StoragePlan,
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
pub struct JavaEnumVariantPath {
    pub enum_name: String,
    pub variant_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSummarySuperclassBoundaryPolicy {
    pub boundary_types: Vec<String>,
    pub facts: Vec<HierarchyOverride>,
}

pub fn plan_representations(program: &Program, facts: &FactSet) -> RepresentationPlan {
    let mut plan = RepresentationPlan::default();
    let effective_facts = EffectiveFacts::new(facts);
    let usage = crate::usage::analyze_usage_with_facts(program, facts);
    plan.storage = crate::storage_strategy::plan_storage_strategies(program, &usage);
    let object_plan = plan_object_model_from_input(ObjectModelInput {
        program,
        facts,
        effective_facts: &effective_facts,
        usage: Some(&usage),
    });
    plan.diagnostics.extend(object_plan.diagnostics);
    for strategy in object_plan.strategies.into_values() {
        let primary_provenance = strategy.primary_provenance();
        let provenance = representation_provenance_for_fact(primary_provenance);
        let representation = representation_for_strategy_with_external_final_facade_policy(
            &strategy.root,
            strategy.strategy,
            primary_provenance,
            &effective_facts,
        );
        plan.types.insert(
            strategy.root.clone(),
            TypeRepresentation {
                java_name: strategy.root,
                representation,
                provenance,
                direct_subtypes: strategy.direct_subtypes,
            },
        );
    }
    for edge in object_plan.hierarchy_edges {
        record_hierarchy_edge(&mut plan, edge);
    }
    keep_source_platform_facade_subclasses_concrete(program, &mut plan);
    expand_source_hierarchy_carrier_variants(&mut plan);
    collect_anonymous_erased_boundary_representations(program, &effective_facts, &mut plan);
    plan
}

fn keep_source_platform_facade_subclasses_concrete(
    program: &Program,
    plan: &mut RepresentationPlan,
) {
    for ty in &program.types {
        keep_source_platform_facade_subclass_concrete(ty, plan);
    }
}

fn keep_source_platform_facade_subclass_concrete(ty: &TypeDecl, plan: &mut RepresentationPlan) {
    if ty
        .extends
        .as_deref()
        .is_some_and(crate::stdlib::is_java_platform_facade_type_name)
    {
        plan.types.insert(
            ty.name.clone(),
            TypeRepresentation {
                java_name: ty.name.clone(),
                representation: ObjectRepresentation::ConcreteStruct,
                provenance: RepresentationProvenance::CorpusAnalysis,
                direct_subtypes: plan
                    .type_representation(&ty.name)
                    .map(|entry| entry.direct_subtypes.clone())
                    .unwrap_or_default(),
            },
        );
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            keep_source_platform_facade_subclass_concrete(nested, plan);
        }
    }
}

fn expand_source_hierarchy_carrier_variants(plan: &mut RepresentationPlan) {
    let roots = plan
        .types
        .iter()
        .filter_map(|(name, representation)| {
            matches!(
                representation.representation,
                ObjectRepresentation::SourceHierarchyCarrier { .. }
            )
            .then_some(name.clone())
        })
        .collect::<Vec<_>>();
    for root in roots {
        let variants = source_hierarchy_carrier_variants_for_root(plan, &root);
        if let Some(entry) = plan.types.get_mut(&root)
            && let ObjectRepresentation::SourceHierarchyCarrier {
                variants: existing, ..
            } = &mut entry.representation
        {
            *existing = variants;
        }
    }
}

fn source_hierarchy_carrier_variants_for_root(
    plan: &RepresentationPlan,
    root: &str,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut names = Vec::new();
    collect_source_hierarchy_carrier_variants(plan, root, &mut seen, &mut names);
    names
        .into_iter()
        .map(|name| crate::slot_render::rust_name_for_java_named_type(&name))
        .collect()
}

fn collect_source_hierarchy_carrier_variants(
    plan: &RepresentationPlan,
    type_name: &str,
    seen: &mut BTreeSet<String>,
    names: &mut Vec<String>,
) {
    if !seen.insert(type_name.to_string()) {
        return;
    }
    names.push(type_name.to_string());
    let direct_subtypes = plan
        .type_representation(type_name)
        .map(|entry| entry.direct_subtypes.clone())
        .unwrap_or_default();
    for subtype in direct_subtypes {
        collect_source_hierarchy_carrier_variants(plan, &subtype, seen, names);
    }
}

fn representation_for_strategy_with_external_final_facade_policy(
    type_name: &str,
    representation: ObjectRepresentation,
    provenance: FactProvenance,
    facts: &EffectiveFacts<'_>,
) -> ObjectRepresentation {
    if matches!(representation, ObjectRepresentation::ErasedObject)
        && provenance == FactProvenance::ExternalLibraryPack
        && external_final_facade_should_keep_concrete_dispatch(
            &JavaType::Named {
                name: type_name.to_string(),
                args: Vec::new(),
            },
            facts,
        )
    {
        ObjectRepresentation::ConcreteStruct
    } else {
        representation
    }
}

fn external_final_facade_should_keep_concrete_dispatch(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    facts.is_final_type(java_ty) && crate::stdlib::java_collection_kind_for_type(java_ty).is_some()
}

impl RepresentationPlan {
    pub fn type_representations(&self) -> impl Iterator<Item = &TypeRepresentation> {
        self.types.values()
    }

    /// How values of `type_name` are held.
    ///
    /// `StorageStrategy::Value` is both the default and the answer for a type
    /// the plan has never heard of, so a caller cannot accidentally treat
    /// "unknown" as "shared".
    pub fn storage_strategy(&self, type_name: &str) -> crate::storage_strategy::StorageStrategy {
        self.storage.strategy(type_name)
    }

    pub fn storage(&self) -> &crate::storage_strategy::StoragePlan {
        &self.storage
    }

    pub fn type_representation(&self, type_name: &str) -> Option<&TypeRepresentation> {
        self.types
            .get(type_name)
            .or_else(|| self.unique_type_representation_by_simple_name(type_name))
    }

    pub fn representation_for(&self, java_ty: &JavaType) -> ObjectRepresentation {
        self.representation_lookup_policy(java_ty).representation
    }

    pub fn representation_lookup_policy(&self, java_ty: &JavaType) -> RepresentationLookupPolicy {
        let planned_representation = match java_ty {
            JavaType::Named { name, .. } => self
                .type_representation(name)
                .map(|entry| entry.representation.clone()),
            _ => None,
        };
        object_representation_lookup_policy(java_ty, planned_representation)
    }

    pub fn direct_subtypes_of(&self, type_name: &str) -> &[String] {
        self.type_representation(type_name)
            .map(|entry| entry.direct_subtypes.as_slice())
            .unwrap_or(&[])
    }

    pub fn dispatch_for(&self, java_ty: &JavaType) -> DispatchShape {
        representation_dispatch_policy(self.representation_for(java_ty)).dispatch
    }

    pub fn java_enum_variant_path_for_label(
        &self,
        type_name: &str,
        label_name: &str,
    ) -> Option<JavaEnumVariantPath> {
        let representation = self.type_representation(type_name)?;
        let ObjectRepresentation::JavaEnum {
            enum_name,
            variants,
        } = &representation.representation
        else {
            return None;
        };
        let variant = crate::object_model::rust_enum_variant_name_from_parsed_constant(label_name);
        let rust_enum_name = crate::slot_render::rust_name_for_java_named_type(enum_name);
        variants
            .iter()
            .any(|known| known == &variant)
            .then(|| JavaEnumVariantPath {
                enum_name: rust_enum_name,
                variant_name: variant,
            })
    }

    fn unique_type_representation_by_simple_name(
        &self,
        type_name: &str,
    ) -> Option<&TypeRepresentation> {
        if type_name.contains('.') {
            return None;
        }
        let mut matches = self
            .types
            .values()
            .filter(|entry| entry.java_name.rsplit('.').next() == Some(type_name));
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
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
            .chain(anonymous_class_diagnostics_with_plan(program, Some(self)))
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
        self.hierarchy_type_is_represented(super_name)
            || self.hierarchy_type_has_user_closed_enum_ancestor(super_name, &mut BTreeSet::new())
    }

    fn hierarchy_type_is_represented(&self, type_name: &str) -> bool {
        self.types.get(type_name).is_some_and(|entry| {
            object_hierarchy_edge_representation_policy(entry.representation.clone()).represented
        })
    }

    fn hierarchy_type_is_user_closed_enum(&self, type_name: &str) -> bool {
        self.types.get(type_name).is_some_and(|entry| {
            object_representation_is_user_closed_enum(
                &entry.representation,
                fact_provenance_for_representation(entry.provenance),
            )
        })
    }

    fn hierarchy_type_has_user_closed_enum_ancestor(
        &self,
        type_name: &str,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if !visited.insert(type_name.to_string()) {
            return false;
        }
        self.hierarchy_edges_for_subtype(type_name)
            .iter()
            .any(|edge| {
                self.hierarchy_type_is_user_closed_enum(&edge.super_name)
                    || self.hierarchy_type_has_user_closed_enum_ancestor(&edge.super_name, visited)
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
        ObjectRepresentation::JavaEnum { .. } => DispatchShape::Concrete,
        ObjectRepresentation::ClosedEnum { enum_name, .. } => DispatchShape::ClosedEnum {
            enum_name: crate::slot_render::rust_name_for_java_named_type(enum_name),
        },
        ObjectRepresentation::SourceHierarchyCarrier { carrier_name, .. } => {
            DispatchShape::ClosedEnum {
                enum_name: crate::slot_render::rust_name_for_java_named_type(carrier_name),
            }
        }
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

pub fn extend_representation_facts_for_source_summary_superclass_boundaries<'a>(
    emitted_programs: impl IntoIterator<Item = &'a Program>,
    source_type_modes: &HashMap<String, DependencyMode>,
    facts: &mut FactSet,
) -> SourceSummarySuperclassBoundaryPolicy {
    let policy =
        source_summary_superclass_boundary_policy(emitted_programs, source_type_modes, facts);
    facts
        .hierarchy_overrides
        .extend(policy.facts.iter().cloned());
    policy
}

pub fn source_summary_superclass_boundary_policy<'a>(
    emitted_programs: impl IntoIterator<Item = &'a Program>,
    source_type_modes: &HashMap<String, DependencyMode>,
    facts: &FactSet,
) -> SourceSummarySuperclassBoundaryPolicy {
    let mut boundary_types = BTreeSet::new();
    for program in emitted_programs {
        for ty in &program.types {
            collect_source_summary_superclasses_for_type(
                ty,
                source_type_modes,
                &mut boundary_types,
            );
        }
    }
    let facts = boundary_types
        .iter()
        .filter(|type_name| {
            !facts
                .hierarchy_overrides
                .iter()
                .any(|existing| existing.type_name == **type_name)
        })
        .map(|type_name| HierarchyOverride {
            type_name: type_name.clone(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::CorpusAnalysis,
        })
        .collect::<Vec<_>>();
    SourceSummarySuperclassBoundaryPolicy {
        boundary_types: boundary_types.into_iter().collect(),
        facts,
    }
}

fn collect_source_summary_superclasses_for_type(
    ty: &TypeDecl,
    source_type_modes: &HashMap<String, DependencyMode>,
    boundary_types: &mut BTreeSet<String>,
) {
    if let Some(super_name) = ty.extends.as_deref() {
        if let Some(boundary_name) =
            source_type_name_with_mode(super_name, source_type_modes, DependencyMode::SourceSummary)
        {
            boundary_types.insert(boundary_name);
        }
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_source_summary_superclasses_for_type(nested, source_type_modes, boundary_types);
        }
    }
}

fn source_type_name_with_mode(
    type_name: &str,
    source_type_modes: &HashMap<String, DependencyMode>,
    mode: DependencyMode,
) -> Option<String> {
    hierarchy_type_name_lookup_aliases(type_name)
        .into_iter()
        .find(|alias| source_type_modes.get(alias) == Some(&mode))
}

fn hierarchy_type_name_lookup_aliases(type_name: &str) -> Vec<String> {
    let raw_name = type_name
        .split_once('<')
        .map(|(raw, _)| raw)
        .unwrap_or(type_name)
        .trim();
    let parsed = crate::parser::parse_hierarchy_type(type_name);
    let mut aliases = Vec::new();
    push_unique_alias(&mut aliases, type_name.trim());
    push_unique_alias(&mut aliases, raw_name);
    if let JavaType::Named { name, .. } = parsed {
        push_unique_alias(&mut aliases, &name);
        push_unique_alias(&mut aliases, &crate::parser::to_snake_case(&name));
    }
    push_unique_alias(
        &mut aliases,
        &crate::parser::to_snake_case(type_name.trim()),
    );
    push_unique_alias(&mut aliases, &crate::parser::to_snake_case(raw_name));
    aliases
}

fn push_unique_alias(aliases: &mut Vec<String>, alias: &str) {
    if alias.is_empty() {
        return;
    }
    if !aliases.iter().any(|existing| existing == alias) {
        aliases.push(alias.to_string());
    }
}

fn anonymous_class_diagnostics_with_plan(
    program: &Program,
    plan: Option<&RepresentationPlan>,
) -> Vec<RepresentationDiagnostic> {
    let mut diagnostics = Vec::new();
    for ty in all_types(program) {
        collect_anonymous_class_diagnostics_for_type(ty, plan, &mut diagnostics);
    }
    diagnostics
}

fn collect_anonymous_class_diagnostics_for_type(
    ty: &TypeDecl,
    plan: Option<&RepresentationPlan>,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_anonymous_class_diagnostics_for_expr(initializer, plan, diagnostics);
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_anonymous_class_diagnostics_for_block(body, plan, diagnostics);
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_anonymous_class_diagnostics_for_block(body, plan, diagnostics);
                }
            }
            Member::Type(nested) => {
                collect_anonymous_class_diagnostics_for_type(nested, plan, diagnostics);
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_anonymous_class_diagnostics_for_block(
    block: &Block,
    plan: Option<&RepresentationPlan>,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    for stmt in &block.statements {
        collect_anonymous_class_diagnostics_for_stmt(stmt, plan, diagnostics);
    }
}

fn collect_anonymous_class_diagnostics_for_stmt(
    stmt: &Stmt,
    plan: Option<&RepresentationPlan>,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_anonymous_class_diagnostics_for_expr(initializer, plan, diagnostics);
            }
        }
        Stmt::Assign { target, value } => {
            collect_anonymous_class_diagnostics_for_expr(target, plan, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(value, plan, diagnostics);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_anonymous_class_diagnostics_for_expr(condition, plan, diagnostics);
            collect_anonymous_class_diagnostics_for_block(then_block, plan, diagnostics);
            if let Some(else_block) = else_block {
                collect_anonymous_class_diagnostics_for_block(else_block, plan, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_anonymous_class_diagnostics_for_expr(condition, plan, diagnostics);
            collect_anonymous_class_diagnostics_for_block(body, plan, diagnostics);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_anonymous_class_diagnostics_for_stmt(stmt, plan, diagnostics);
            }
            if let Some(condition) = condition {
                collect_anonymous_class_diagnostics_for_expr(condition, plan, diagnostics);
            }
            for expr in update {
                collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
            }
            collect_anonymous_class_diagnostics_for_block(body, plan, diagnostics);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_anonymous_class_diagnostics_for_expr(iterable, plan, diagnostics);
            collect_anonymous_class_diagnostics_for_block(body, plan, diagnostics);
        }
        Stmt::Switch { selector, cases } => {
            collect_anonymous_class_diagnostics_for_expr(selector, plan, diagnostics);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
                    }
                }
                for stmt in &case.statements {
                    collect_anonymous_class_diagnostics_for_stmt(stmt, plan, diagnostics);
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
                collect_anonymous_class_diagnostics_for_stmt(resource, plan, diagnostics);
            }
            collect_anonymous_class_diagnostics_for_block(body, plan, diagnostics);
            for catch in catches {
                collect_anonymous_class_diagnostics_for_block(&catch.body, plan, diagnostics);
            }
            if let Some(finally_block) = finally_block {
                collect_anonymous_class_diagnostics_for_block(finally_block, plan, diagnostics);
            }
        }
        Stmt::LocalType(ty) => collect_anonymous_class_diagnostics_for_type(ty, plan, diagnostics),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, plan, diagnostics);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_anonymous_class_diagnostics_for_block(block, plan, diagnostics)
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_anonymous_class_diagnostics_for_expr(
    expr: &Expr,
    plan: Option<&RepresentationPlan>,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            collect_anonymous_class_diagnostics_for_expr(receiver, plan, diagnostics);
        }
        Expr::ArrayAccess(array, index) => {
            collect_anonymous_class_diagnostics_for_expr(array, plan, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(index, plan, diagnostics);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_anonymous_class_diagnostics_for_expr(left, plan, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(right, plan, diagnostics);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_anonymous_class_diagnostics_for_expr(condition, plan, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(then_expr, plan, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(else_expr, plan, diagnostics);
        }
        Expr::Switch { selector, cases } => {
            collect_anonymous_class_diagnostics_for_expr(selector, plan, diagnostics);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
                    }
                }
                collect_anonymous_class_diagnostics_for_expr(&case.value, plan, diagnostics);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_anonymous_class_diagnostics_for_expr(receiver, plan, diagnostics);
            }
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, plan, diagnostics);
            }
        }
        Expr::New { ty, args, body, .. } => {
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, plan, diagnostics);
            }
            if body.is_some() && !anonymous_class_target_is_erased_boundary(ty, plan) {
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
            collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_anonymous_class_diagnostics_for_expr(expr, plan, diagnostics)
            }
            LambdaBody::Block(block) => {
                collect_anonymous_class_diagnostics_for_block(block, plan, diagnostics);
            }
        },
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

fn anonymous_class_target_is_erased_boundary(
    ty: &JavaType,
    plan: Option<&RepresentationPlan>,
) -> bool {
    let Some(plan) = plan else {
        return false;
    };
    let policy = plan.representation_lookup_policy(ty);
    matches!(
        (&policy.fallback, &policy.representation),
        (
            RepresentationLookupFallback::PlannedType,
            ObjectRepresentation::ErasedObject
        )
    )
}

fn collect_anonymous_erased_boundary_representations(
    program: &Program,
    facts: &EffectiveFacts<'_>,
    plan: &mut RepresentationPlan,
) {
    let mut type_names = BTreeSet::new();
    for ty in all_types(program) {
        collect_anonymous_erased_boundary_type_names_for_type(ty, facts, &mut type_names);
    }
    for type_name in type_names {
        plan.types
            .entry(type_name.clone())
            .or_insert(TypeRepresentation {
                java_name: type_name,
                representation: ObjectRepresentation::ErasedObject,
                provenance: RepresentationProvenance::BuiltinExternalModel,
                direct_subtypes: Vec::new(),
            });
    }
}

fn collect_anonymous_erased_boundary_type_names_for_type(
    ty: &TypeDecl,
    facts: &EffectiveFacts<'_>,
    type_names: &mut BTreeSet<String>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_anonymous_erased_boundary_type_names_for_expr(
                        initializer,
                        facts,
                        type_names,
                    );
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_anonymous_erased_boundary_type_names_for_block(body, facts, type_names);
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_anonymous_erased_boundary_type_names_for_block(body, facts, type_names);
                }
            }
            Member::Type(nested) => {
                collect_anonymous_erased_boundary_type_names_for_type(nested, facts, type_names);
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_anonymous_erased_boundary_type_names_for_block(
    block: &Block,
    facts: &EffectiveFacts<'_>,
    type_names: &mut BTreeSet<String>,
) {
    for stmt in &block.statements {
        collect_anonymous_erased_boundary_type_names_for_stmt(stmt, facts, type_names);
    }
}

fn collect_anonymous_erased_boundary_type_names_for_stmt(
    stmt: &Stmt,
    facts: &EffectiveFacts<'_>,
    type_names: &mut BTreeSet<String>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_anonymous_erased_boundary_type_names_for_expr(
                    initializer,
                    facts,
                    type_names,
                );
            }
        }
        Stmt::Assign { target, value } => {
            collect_anonymous_erased_boundary_type_names_for_expr(target, facts, type_names);
            collect_anonymous_erased_boundary_type_names_for_expr(value, facts, type_names);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_anonymous_erased_boundary_type_names_for_expr(condition, facts, type_names);
            collect_anonymous_erased_boundary_type_names_for_block(then_block, facts, type_names);
            if let Some(else_block) = else_block {
                collect_anonymous_erased_boundary_type_names_for_block(
                    else_block, facts, type_names,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_anonymous_erased_boundary_type_names_for_expr(condition, facts, type_names);
            collect_anonymous_erased_boundary_type_names_for_block(body, facts, type_names);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_anonymous_erased_boundary_type_names_for_stmt(stmt, facts, type_names);
            }
            if let Some(condition) = condition {
                collect_anonymous_erased_boundary_type_names_for_expr(condition, facts, type_names);
            }
            for expr in update {
                collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
            }
            collect_anonymous_erased_boundary_type_names_for_block(body, facts, type_names);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_anonymous_erased_boundary_type_names_for_expr(iterable, facts, type_names);
            collect_anonymous_erased_boundary_type_names_for_block(body, facts, type_names);
        }
        Stmt::Switch { selector, cases } => {
            collect_anonymous_erased_boundary_type_names_for_expr(selector, facts, type_names);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_anonymous_erased_boundary_type_names_for_expr(
                            expr, facts, type_names,
                        );
                    }
                }
                for stmt in &case.statements {
                    collect_anonymous_erased_boundary_type_names_for_stmt(stmt, facts, type_names);
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
                collect_anonymous_erased_boundary_type_names_for_stmt(resource, facts, type_names);
            }
            collect_anonymous_erased_boundary_type_names_for_block(body, facts, type_names);
            for catch in catches {
                collect_anonymous_erased_boundary_type_names_for_block(
                    &catch.body,
                    facts,
                    type_names,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_anonymous_erased_boundary_type_names_for_block(
                    finally_block,
                    facts,
                    type_names,
                );
            }
        }
        Stmt::LocalType(ty) => {
            collect_anonymous_erased_boundary_type_names_for_type(ty, facts, type_names);
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_anonymous_erased_boundary_type_names_for_expr(arg, facts, type_names);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_anonymous_erased_boundary_type_names_for_block(block, facts, type_names);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_anonymous_erased_boundary_type_names_for_expr(
    expr: &Expr,
    facts: &EffectiveFacts<'_>,
    type_names: &mut BTreeSet<String>,
) {
    match expr {
        Expr::New { ty, args, body, .. } => {
            for arg in args {
                collect_anonymous_erased_boundary_type_names_for_expr(arg, facts, type_names);
            }
            if body.is_some() {
                if let Some(type_name) = explicit_erased_object_fact_type_name(ty, facts) {
                    type_names.insert(type_name.to_string());
                }
            }
        }
        Expr::FieldAccess(receiver, _) => {
            collect_anonymous_erased_boundary_type_names_for_expr(receiver, facts, type_names);
        }
        Expr::ArrayAccess(array, index) => {
            collect_anonymous_erased_boundary_type_names_for_expr(array, facts, type_names);
            collect_anonymous_erased_boundary_type_names_for_expr(index, facts, type_names);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_anonymous_erased_boundary_type_names_for_expr(left, facts, type_names);
            collect_anonymous_erased_boundary_type_names_for_expr(right, facts, type_names);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_anonymous_erased_boundary_type_names_for_expr(condition, facts, type_names);
            collect_anonymous_erased_boundary_type_names_for_expr(then_expr, facts, type_names);
            collect_anonymous_erased_boundary_type_names_for_expr(else_expr, facts, type_names);
        }
        Expr::Switch { selector, cases } => {
            collect_anonymous_erased_boundary_type_names_for_expr(selector, facts, type_names);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_anonymous_erased_boundary_type_names_for_expr(
                            expr, facts, type_names,
                        );
                    }
                }
                collect_anonymous_erased_boundary_type_names_for_expr(
                    &case.value,
                    facts,
                    type_names,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_anonymous_erased_boundary_type_names_for_expr(receiver, facts, type_names);
            }
            for arg in args {
                collect_anonymous_erased_boundary_type_names_for_expr(arg, facts, type_names);
            }
        }
        Expr::InstanceOf { expr, .. } => {
            collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_anonymous_erased_boundary_type_names_for_expr(expr, facts, type_names);
            }
            LambdaBody::Block(block) => {
                collect_anonymous_erased_boundary_type_names_for_block(block, facts, type_names);
            }
        },
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

fn explicit_erased_object_fact_type_name<'a>(
    ty: &'a JavaType,
    facts: &EffectiveFacts<'_>,
) -> Option<&'a str> {
    let JavaType::Named { name, .. } = ty else {
        return None;
    };
    matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(crate::facts::HierarchyStrategy::ErasedObject)
    )
    .then_some(name.as_str())
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

fn fact_provenance_for_representation(provenance: RepresentationProvenance) -> FactProvenance {
    match provenance {
        RepresentationProvenance::ParsedSource => FactProvenance::ParsedSource,
        RepresentationProvenance::BuiltinExternalModel => FactProvenance::BuiltinExternalModel,
        RepresentationProvenance::CorpusAnalysis => FactProvenance::CorpusAnalysis,
        RepresentationProvenance::UserOverride => FactProvenance::UserOverride,
        RepresentationProvenance::ConservativeFallback => FactProvenance::ConservativeFallback,
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
        refresh_closed_enum_representation_variants(entry);
    }
}

fn refresh_closed_enum_representation_variants(entry: &mut TypeRepresentation) {
    if let ObjectRepresentation::ClosedEnum { variants, .. } = &mut entry.representation {
        *variants =
            crate::object_model::closed_enum_variant_names_for_subtypes(&entry.direct_subtypes);
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
