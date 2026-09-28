use std::collections::{BTreeSet, HashMap, HashSet};

use crate::dependency_mode::DependencyMode;
use crate::effective_facts::EffectiveFacts;
use crate::facts::{FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy};
use crate::java_ir::{
    Block, Expr, JavaType, LambdaBody, Member, Modifier, Program, Stmt, SwitchLabel, TypeDecl,
    TypeKind,
};
pub use crate::object_model::{
    ClosedEnumInferencePolicy, ClosedEnumProof, ClosedEnumProofPolicy,
    HierarchyEdgeDiagnosticOutcome, HierarchyEdgeDiagnosticPolicy,
    HierarchyOverrideApplicationPolicy, HierarchyOverrideRepresentationPolicy,
    HierarchyOverrideValidationPolicy, ObjectHierarchyContext, ObjectHierarchyEdge,
    ObjectHierarchyEdgeDiagnosticInput, ObjectHierarchyEdgeKind, ObjectModelInput,
    ObjectRepresentation, ObjectRepresentationPolicy, ObjectRepresentationRequest,
    ObjectSafetyFactPolicy, ObjectSafetyPolicy, RepresentationDiagnostic,
    RepresentationLookupFallback, RepresentationLookupPolicy,
    closed_enum_inference_policy_with_effective_facts,
    closed_enum_proof_policy_with_effective_facts,
    hierarchy_override_application_policy_with_effective_facts,
    hierarchy_override_validation_policy, object_hierarchy_edge_diagnostic_policy,
    object_hierarchy_edge_representation_policy, object_representation_is_user_closed_enum,
    object_representation_lookup_policy, object_representation_policy_with_effective_facts,
    object_safety_fact_policy_with_effective_facts, plan_object_model_from_input,
};
use crate::slot_shape::DispatchShape;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepresentationPlan {
    types: HashMap<String, TypeRepresentation>,
    /// A source `java.util.Map.Entry` implementation needs a virtual value
    /// carrier; tuple snapshots cannot retain its override or shared state.
    source_map_entry_value_carrier: bool,
    /// Concrete native-map fields whose entries escape as Java `Map.Entry`
    /// values. These fields need a shared tracked-map handle so an entry and
    /// subsequent field accesses observe the same map node.
    live_native_map_fields: HashSet<(String, String)>,
    live_static_native_map_fields: HashSet<(String, String)>,
    nullable_live_native_map_field_keys: HashSet<(String, String)>,
    nullable_live_native_map_field_values: HashSet<(String, String)>,
    nullable_live_static_native_map_field_keys: HashSet<(String, String)>,
    nullable_live_static_native_map_field_values: HashSet<(String, String)>,
    hierarchy_edges_by_subtype: HashMap<String, Vec<ObjectHierarchyEdge>>,
    /// Concrete inheritance edges proven safe by the existing source-member
    /// flattening path. Keep this separate from polymorphic hierarchy
    /// representations: these edges are represented only for the concrete,
    /// statically constructed leaf shape recorded below.
    concretely_lowered_hierarchy_edges: BTreeSet<(String, String)>,
    diagnostics: Vec<RepresentationDiagnostic>,
    /// Source classes anonymously subclassed somewhere in the translated
    /// program. Unlike a named subtype, an anonymous subtype cannot appear as
    /// a statically named payload in `SourceHierarchyCarrier`; class lowering
    /// must instead plan a virtual carrier for this base.
    anonymous_class_bases: BTreeSet<String>,
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
    java_name: String,
    representation: ObjectRepresentation,
    provenance: RepresentationProvenance,
    direct_subtypes: Vec<String>,
}

impl TypeRepresentation {
    pub fn new(
        java_name: impl Into<String>,
        representation: ObjectRepresentation,
        provenance: RepresentationProvenance,
        direct_subtypes: Vec<String>,
    ) -> Self {
        Self {
            java_name: java_name.into(),
            representation,
            provenance,
            direct_subtypes,
        }
    }

    pub fn java_name(&self) -> &str {
        &self.java_name
    }

    pub fn representation(&self) -> &ObjectRepresentation {
        &self.representation
    }

    pub fn provenance(&self) -> RepresentationProvenance {
        self.provenance
    }

    pub fn direct_subtypes(&self) -> &[String] {
        &self.direct_subtypes
    }
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
    representation: ObjectRepresentation,
    dispatch: DispatchShape,
}

impl RepresentationDispatchPolicy {
    pub fn new(representation: ObjectRepresentation, dispatch: DispatchShape) -> Self {
        Self {
            representation,
            dispatch,
        }
    }

    pub fn representation(&self) -> &ObjectRepresentation {
        &self.representation
    }

    pub fn dispatch(&self) -> &DispatchShape {
        &self.dispatch
    }

    pub fn into_dispatch(self) -> DispatchShape {
        self.dispatch
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaEnumVariantPath {
    enum_name: String,
    variant_name: String,
}

impl JavaEnumVariantPath {
    pub fn new(enum_name: impl Into<String>, variant_name: impl Into<String>) -> Self {
        Self {
            enum_name: enum_name.into(),
            variant_name: variant_name.into(),
        }
    }

    pub fn enum_name(&self) -> &str {
        &self.enum_name
    }

    pub fn variant_name(&self) -> &str {
        &self.variant_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSummarySuperclassBoundaryPolicy {
    boundary_types: Vec<String>,
    facts: Vec<HierarchyOverride>,
}

impl SourceSummarySuperclassBoundaryPolicy {
    pub fn new(boundary_types: Vec<String>, facts: Vec<HierarchyOverride>) -> Self {
        Self {
            boundary_types,
            facts,
        }
    }

    pub fn boundary_types(&self) -> &[String] {
        &self.boundary_types
    }

    pub fn facts(&self) -> &[HierarchyOverride] {
        &self.facts
    }
}

pub fn plan_representations(program: &Program, facts: &FactSet) -> RepresentationPlan {
    let mut plan = RepresentationPlan::default();
    plan.live_native_map_fields = live_native_map_fields(program, false);
    plan.live_static_native_map_fields = live_native_map_fields(program, true);
    let (nullable_keys, nullable_values) =
        nullable_live_native_map_field_slots(program, &plan.live_native_map_fields);
    plan.nullable_live_native_map_field_keys = nullable_keys;
    plan.nullable_live_native_map_field_values = nullable_values;
    let (nullable_static_keys, nullable_static_values) =
        nullable_live_native_map_field_slots(program, &plan.live_static_native_map_fields);
    plan.nullable_live_static_native_map_field_keys = nullable_static_keys;
    plan.nullable_live_static_native_map_field_values = nullable_static_values;
    plan.source_map_entry_value_carrier =
        crate::stdlib::program_uses_source_map_entry_values(program)
            || !plan.live_native_map_fields.is_empty()
            || !plan.live_static_native_map_fields.is_empty()
            || program_has_live_native_map_parameter(program)
            || program_has_mutated_live_native_map_local(program)
            || program_has_escaping_inline_native_map_entry(program);
    let effective_facts = EffectiveFacts::new(facts);
    let usage = crate::usage::analyze_usage_with_facts(program, facts);
    plan.storage =
        crate::storage_strategy::plan_storage_strategies_from_facts(program, &usage, facts);
    let object_plan = plan_object_model_from_input(ObjectModelInput::new(
        program,
        facts,
        &effective_facts,
        Some(&usage),
    ));
    let (strategies, hierarchy_edges, diagnostics) = object_plan.into_parts();
    plan.diagnostics.extend(diagnostics);
    for strategy in strategies.into_values() {
        let (
            root,
            strategy_representation,
            _requirements,
            direct_subtypes,
            _source_methods_access_instance_fields,
            _rejected,
            provenance_chain,
            _diagnostics,
        ) = strategy.into_parts();
        let primary_provenance = provenance_chain
            .last()
            .copied()
            .unwrap_or(FactProvenance::ConservativeFallback);
        let provenance = representation_provenance_for_fact(primary_provenance);
        let representation = representation_for_strategy_with_external_final_facade_policy(
            &root,
            strategy_representation,
            primary_provenance,
            &effective_facts,
        );
        plan.types.insert(
            root.clone(),
            TypeRepresentation {
                java_name: root,
                representation,
                provenance,
                direct_subtypes,
            },
        );
    }
    for edge in hierarchy_edges {
        record_hierarchy_edge(&mut plan, edge);
    }
    keep_source_platform_facade_subclasses_concrete(program, &mut plan);
    expand_source_hierarchy_carrier_variants(&mut plan);
    collect_anonymous_erased_boundary_representations(program, &effective_facts, &mut plan);
    promote_anonymous_class_bases_to_source_hierarchy_carriers(&mut plan);
    record_statically_constructed_concrete_leaf_hierarchy_edges(program, &mut plan);
    record_simple_super_copyback_hierarchy_edges(program, &mut plan);
    plan
}

/// Record the deliberately small concrete `super.m()` subset whose lowering
/// reconstructs the superclass shell, invokes a field-local mutator, and
/// copies the superclass fields back.  This proof is intentionally narrower
/// than merely seeing `extends` or `super`: arbitrary calls and summary-only
/// ancestors must retain the unsupported diagnostic.
fn record_simple_super_copyback_hierarchy_edges(program: &Program, plan: &mut RepresentationPlan) {
    for subtype in all_types(program) {
        let Some(super_name) = subtype.extends.as_deref() else {
            continue;
        };
        let Some(superclass) = find_type_by_erased_name(program, super_name) else {
            continue;
        };
        if !matches!(
            plan.type_representation(&subtype.name)
                .map(TypeRepresentation::representation),
            Some(ObjectRepresentation::ConcreteStruct)
        ) || !matches!(
            plan.type_representation(&superclass.name)
                .map(TypeRepresentation::representation),
            Some(ObjectRepresentation::ConcreteStruct)
        ) {
            continue;
        }
        let mut super_fields = superclass
            .members
            .iter()
            .filter_map(|member| match member {
                Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => {
                    Some(field.name.clone())
                }
                _ => None,
            })
            .collect::<HashSet<_>>();
        super_fields.extend(
            crate::inherited_members::inherited_instance_field_refs_for_type_in_programs(
                superclass,
                &[program],
            )
            .into_iter()
            .map(|inherited| inherited.field().name.clone()),
        );
        let proven = subtype.members.iter().any(|member| {
            let Member::Method(method) = member else {
                return false;
            };
            let Some(body) = method.body.as_ref() else {
                return false;
            };
            body.statements.iter().any(|stmt| {
                let Stmt::Expr(Expr::Call {
                    receiver: Some(receiver),
                    name,
                    args,
                    ..
                }) = stmt
                else {
                    return false;
                };
                if !matches!(receiver.as_ref(), Expr::Super) {
                    return false;
                }
                superclass.members.iter().any(|member| {
                    let Member::Method(super_method) = member else {
                        return false;
                    };
                    super_method.name == *name
                        && super_method.params.len() == args.len()
                        && super_method.body.as_ref().is_some_and(|body| {
                            !body.statements.is_empty()
                                && body.statements.iter().all(|stmt| {
                                    simple_super_copyback_mutation_stmt(stmt, &super_fields)
                                })
                        })
                })
            })
        });
        if proven {
            plan.concretely_lowered_hierarchy_edges
                .insert((subtype.name.clone(), superclass.name.clone()));
        }
    }
}

fn simple_super_copyback_mutation_stmt(stmt: &Stmt, fields: &HashSet<String>) -> bool {
    match stmt {
        Stmt::Assign { target, value } => {
            simple_super_copyback_field_target(target, fields)
                && simple_super_copyback_value_expr(value, fields)
        }
        Stmt::Expr(Expr::Unary { expr, .. }) => simple_super_copyback_field_target(expr, fields),
        _ => false,
    }
}

fn simple_super_copyback_field_target(expr: &Expr, fields: &HashSet<String>) -> bool {
    match expr {
        Expr::Var(name) => fields.contains(name.as_str()),
        Expr::FieldAccess(receiver, name) => {
            matches!(receiver.as_ref(), Expr::This) && fields.contains(name.as_str())
        }
        _ => false,
    }
}

fn simple_super_copyback_value_expr(expr: &Expr, fields: &HashSet<String>) -> bool {
    match expr {
        Expr::Literal(_) => true,
        Expr::Var(name) => fields.contains(name.as_str()),
        Expr::FieldAccess(receiver, name) => {
            matches!(receiver.as_ref(), Expr::This) && fields.contains(name.as_str())
        }
        Expr::Binary { left, right, .. } => {
            simple_super_copyback_value_expr(left, fields)
                && simple_super_copyback_value_expr(right, fields)
        }
        Expr::Unary { expr, .. } => simple_super_copyback_value_expr(expr, fields),
        _ => false,
    }
}

fn record_statically_constructed_concrete_leaf_hierarchy_edges(
    program: &Program,
    plan: &mut RepresentationPlan,
) {
    let statically_constructed_types = all_types(program)
        .into_iter()
        .flat_map(|ty| &ty.members)
        .filter_map(|member| match member {
            Member::Field(field) if field.modifiers.contains(&Modifier::Static) => {
                let JavaType::Named { name: field_ty, .. } = &field.ty else {
                    return None;
                };
                let Some(Expr::New {
                    ty: JavaType::Named { name: new_ty, .. },
                    body: None,
                    ..
                }) = field.initializer.as_ref()
                else {
                    return None;
                };
                crate::facts::external_type_name_matches(field_ty, new_ty).then(|| field_ty.clone())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    for subtype in all_types(program) {
        let Some(super_name) = subtype.extends.as_deref() else {
            continue;
        };
        if subtype.kind != TypeKind::Class
            || !subtype.implements.is_empty()
            || !subtype.members.is_empty()
            || !statically_constructed_types
                .iter()
                .any(|name| crate::facts::external_type_name_matches(name, &subtype.name))
        {
            continue;
        }
        let Some(superclass) = find_type_by_erased_name(program, super_name) else {
            continue;
        };
        if superclass.kind != TypeKind::Class
            || superclass.modifiers.contains(&Modifier::Abstract)
            || !matches!(
                plan.type_representation(&subtype.name)
                    .map(|entry| &entry.representation),
                Some(ObjectRepresentation::ConcreteStruct)
            )
            || !matches!(
                plan.type_representation(super_name)
                    .map(|entry| &entry.representation),
                Some(ObjectRepresentation::ConcreteStruct)
            )
        {
            continue;
        }
        plan.concretely_lowered_hierarchy_edges
            .insert((subtype.name.clone(), super_name.to_string()));
    }
}

fn live_native_map_fields(program: &Program, static_fields: bool) -> HashSet<(String, String)> {
    let mut fields = HashSet::new();
    for ty in &program.types {
        for member in &ty.members {
            let Member::Field(field) = member else {
                continue;
            };
            if field.modifiers.contains(&Modifier::Static) != static_fields
                || !matches!(&field.ty, JavaType::Named { name, .. }
                    if matches!(name.as_str(), "Map" | "java.util.Map" | "SortedMap" | "java.util.SortedMap" | "NavigableMap" | "java.util.NavigableMap" | "HashMap" | "java.util.HashMap" | "LinkedHashMap" | "java.util.LinkedHashMap" | "TreeMap" | "java.util.TreeMap"))
            {
                continue;
            }
            let source_name = field.name.as_str();
            let rust_name = crate::parser::to_snake_case(source_name);
            let used = ty.members.iter().any(|member| match member {
                Member::Method(method) => method.body.as_ref().is_some_and(|body| {
                    body.statements.iter().any(|stmt| {
                        stmt_uses_native_map_entry_next_on_field(stmt, source_name, &rust_name)
                    })
                }),
                Member::Constructor(ctor) => ctor.body.as_ref().is_some_and(|body| {
                    body.statements.iter().any(|stmt| {
                        stmt_uses_native_map_entry_next_on_field(stmt, source_name, &rust_name)
                    })
                }),
                _ => false,
            });
            if used {
                fields.insert((ty.name.clone(), field.name.clone()));
            }
        }
    }
    fields
}

fn program_has_live_native_map_parameter(program: &Program) -> bool {
    program.types.iter().any(type_has_live_native_map_parameter)
}

fn program_has_mutated_live_native_map_local(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(type_has_mutated_live_native_map_local)
}

fn program_has_escaping_inline_native_map_entry(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(type_has_escaping_inline_native_map_entry)
}

fn type_has_escaping_inline_native_map_entry(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(expr_has_escaping_inline_native_map_entry),
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_has_escaping_inline_native_map_entry),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_has_escaping_inline_native_map_entry),
        Member::Type(nested) => type_has_escaping_inline_native_map_entry(nested),
        _ => false,
    })
}

fn block_has_escaping_inline_native_map_entry(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(stmt_has_escaping_inline_native_map_entry)
}

fn stmt_has_escaping_inline_native_map_entry(stmt: &Stmt) -> bool {
    let in_expr = |expr: &Expr| expr_has_escaping_inline_native_map_entry(expr);
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            initializer.as_ref().is_some_and(in_expr)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => in_expr(expr),
        Stmt::Assign { target, value } => in_expr(target) || in_expr(value),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            in_expr(condition)
                || block_has_escaping_inline_native_map_entry(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_escaping_inline_native_map_entry)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            body, condition, ..
        } => in_expr(condition) || block_has_escaping_inline_native_map_entry(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_has_escaping_inline_native_map_entry)
                || condition.as_ref().is_some_and(in_expr)
                || update.iter().any(in_expr)
                || block_has_escaping_inline_native_map_entry(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            in_expr(iterable) || block_has_escaping_inline_native_map_entry(body)
        }
        Stmt::Switch { selector, cases } => {
            in_expr(selector)
                || cases.iter().any(|case| {
                    case.statements
                        .iter()
                        .any(stmt_has_escaping_inline_native_map_entry)
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
                .any(stmt_has_escaping_inline_native_map_entry)
                || block_has_escaping_inline_native_map_entry(body)
                || catches
                    .iter()
                    .any(|catch| block_has_escaping_inline_native_map_entry(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_escaping_inline_native_map_entry)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_has_escaping_inline_native_map_entry(block)
        }
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(in_expr),
        _ => false,
    }
}

fn expr_has_escaping_inline_native_map_entry(expr: &Expr) -> bool {
    let nested = |expr: &Expr| expr_has_escaping_inline_native_map_entry(expr);
    match expr {
        Expr::Call { receiver, args, .. } => {
            args.iter()
                .any(|arg| map_entry_next_backing_field(arg).is_some())
                || receiver.as_deref().is_some_and(nested)
                || args.iter().any(nested)
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            args.iter()
                .any(|arg| map_entry_next_backing_field(arg).is_some())
                || qualifier.as_deref().is_some_and(nested)
                || args.iter().any(nested)
                || body.as_ref().is_some_and(|members| {
                    members.iter().any(|member| match member {
                        Member::Field(field) => field.initializer.as_ref().is_some_and(nested),
                        Member::Method(method) => method
                            .body
                            .as_ref()
                            .is_some_and(block_has_escaping_inline_native_map_entry),
                        Member::Constructor(constructor) => constructor
                            .body
                            .as_ref()
                            .is_some_and(block_has_escaping_inline_native_map_entry),
                        Member::Type(nested) => type_has_escaping_inline_native_map_entry(nested),
                        _ => false,
                    })
                })
        }
        Expr::FieldAccess(inner, _)
        | Expr::Unary { expr: inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::InstanceOf { expr: inner, .. } => nested(inner),
        Expr::ArrayAccess(left, right) | Expr::Binary { left, right, .. } => {
            nested(left) || nested(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => nested(condition) || nested(then_expr) || nested(else_expr),
        Expr::Switch { selector, cases } => {
            nested(selector) || cases.iter().any(|case| nested(&case.value))
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(nested)
                || initializer
                    .as_ref()
                    .is_some_and(|values| values.iter().any(nested))
        }
        Expr::ArrayInitializer(values) => values.iter().any(nested),
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => nested(expr),
            LambdaBody::Block(block) => block_has_escaping_inline_native_map_entry(block),
        },
        _ => false,
    }
}

fn type_has_mutated_live_native_map_local(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_has_mutated_live_native_map_local),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_has_mutated_live_native_map_local),
        Member::Type(nested) => type_has_mutated_live_native_map_local(nested),
        _ => false,
    })
}

fn block_has_mutated_live_native_map_local(block: &Block) -> bool {
    for (index, stmt) in block.statements.iter().enumerate() {
        if let Stmt::VarDecl {
            source_name,
            initializer: Some(initializer),
            ..
        } = stmt
            && map_entry_next_backing_field(initializer).is_some()
        {
            let mut aliases = HashSet::from([source_name.clone()]);
            if statements_require_live_map_entry_alias(&block.statements[index + 1..], &mut aliases)
            {
                return true;
            }
        }
        if stmt_has_nested_mutated_live_native_map_local(stmt) {
            return true;
        }
    }
    false
}

fn statements_require_live_map_entry_alias(
    statements: &[Stmt],
    aliases: &mut HashSet<String>,
) -> bool {
    for stmt in statements {
        if aliases.iter().any(|alias| {
            stmt_mutates_map_entry_local(stmt, alias)
                || stmt_passes_map_entry_local_as_argument(stmt, alias)
        }) {
            return true;
        }
        match stmt {
            Stmt::VarDecl {
                source_name: alias,
                initializer: Some(Expr::Var(source)),
                ..
            }
            | Stmt::Assign {
                target: Expr::Var(alias),
                value: Expr::Var(source),
            } if aliases.contains(source) => {
                aliases.insert(alias.clone());
            }
            _ => {}
        }
        if nested_stmt_requires_live_map_entry_alias(stmt, aliases) {
            return true;
        }
    }
    false
}

fn nested_stmt_requires_live_map_entry_alias(stmt: &Stmt, aliases: &HashSet<String>) -> bool {
    let in_block = |block: &Block| {
        statements_require_live_map_entry_alias(&block.statements, &mut aliases.clone())
    };
    match stmt {
        Stmt::If {
            then_block,
            else_block,
            ..
        } => in_block(then_block) || else_block.as_ref().is_some_and(in_block),
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => in_block(body),
        Stmt::For { init, body, .. } => {
            let mut loop_aliases = aliases.clone();
            statements_require_live_map_entry_alias(init, &mut loop_aliases)
                || statements_require_live_map_entry_alias(&body.statements, &mut loop_aliases)
        }
        Stmt::Switch { cases, .. } => cases.iter().any(|case| {
            statements_require_live_map_entry_alias(&case.statements, &mut aliases.clone())
        }),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut try_aliases = aliases.clone();
            statements_require_live_map_entry_alias(resources, &mut try_aliases)
                || statements_require_live_map_entry_alias(&body.statements, &mut try_aliases)
                || catches.iter().any(|catch| in_block(&catch.body))
                || finally_block.as_ref().is_some_and(in_block)
        }
        _ => false,
    }
}

fn stmt_has_nested_mutated_live_native_map_local(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_has_mutated_live_native_map_local(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_mutated_live_native_map_local)
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => block_has_mutated_live_native_map_local(body),
        Stmt::For { init, body, .. } => {
            block_has_mutated_live_native_map_local(&Block {
                statements: init.clone(),
            }) || block_has_mutated_live_native_map_local(body)
        }
        Stmt::Switch { cases, .. } => cases.iter().any(|case| {
            block_has_mutated_live_native_map_local(&Block {
                statements: case.statements.clone(),
            })
        }),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            block_has_mutated_live_native_map_local(&Block {
                statements: resources.clone(),
            }) || block_has_mutated_live_native_map_local(body)
                || catches
                    .iter()
                    .any(|catch| block_has_mutated_live_native_map_local(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_mutated_live_native_map_local)
        }
        _ => false,
    }
}

fn stmt_mutates_map_entry_local(stmt: &Stmt, local: &str) -> bool {
    let in_expr = |expr: &Expr| expr_mutates_map_entry_local(expr, local);
    let in_block = |block: &Block| {
        block
            .statements
            .iter()
            .any(|stmt| stmt_mutates_map_entry_local(stmt, local))
    };
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            initializer.as_ref().is_some_and(in_expr)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => in_expr(expr),
        Stmt::Assign { target, value } => in_expr(target) || in_expr(value),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            in_expr(condition) || in_block(then_block) || else_block.as_ref().is_some_and(in_block)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            body, condition, ..
        } => in_expr(condition) || in_block(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_mutates_map_entry_local(stmt, local))
                || condition.as_ref().is_some_and(in_expr)
                || update.iter().any(in_expr)
                || in_block(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => in_expr(iterable) || in_block(body),
        Stmt::Switch { selector, cases } => {
            in_expr(selector)
                || cases.iter().any(|case| {
                    case.statements
                        .iter()
                        .any(|stmt| stmt_mutates_map_entry_local(stmt, local))
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
                .any(|stmt| stmt_mutates_map_entry_local(stmt, local))
                || in_block(body)
                || catches.iter().any(|catch| in_block(&catch.body))
                || finally_block.as_ref().is_some_and(in_block)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => in_block(block),
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(in_expr),
        _ => false,
    }
}

fn expr_mutates_map_entry_local(expr: &Expr, local: &str) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            (matches!(name.as_str(), "setValue" | "set_value")
                && matches!(receiver.as_deref(), Some(Expr::Var(name)) if name == local))
                || receiver
                    .as_deref()
                    .is_some_and(|expr| expr_mutates_map_entry_local(expr, local))
                || args
                    .iter()
                    .any(|expr| expr_mutates_map_entry_local(expr, local))
        }
        Expr::FieldAccess(inner, _)
        | Expr::Unary { expr: inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::InstanceOf { expr: inner, .. } => expr_mutates_map_entry_local(inner, local),
        Expr::ArrayAccess(left, right) | Expr::Binary { left, right, .. } => {
            expr_mutates_map_entry_local(left, local) || expr_mutates_map_entry_local(right, local)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_mutates_map_entry_local(condition, local)
                || expr_mutates_map_entry_local(then_expr, local)
                || expr_mutates_map_entry_local(else_expr, local)
        }
        _ => false,
    }
}

fn stmt_passes_map_entry_local_as_argument(stmt: &Stmt, local: &str) -> bool {
    let in_expr = |expr: &Expr| expr_passes_map_entry_local_as_argument(expr, local);
    let in_block = |block: &Block| {
        block
            .statements
            .iter()
            .any(|stmt| stmt_passes_map_entry_local_as_argument(stmt, local))
    };
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            initializer.as_ref().is_some_and(in_expr)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => in_expr(expr),
        Stmt::Assign { target, value } => in_expr(target) || in_expr(value),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            in_expr(condition) || in_block(then_block) || else_block.as_ref().is_some_and(in_block)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            body, condition, ..
        } => in_expr(condition) || in_block(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_passes_map_entry_local_as_argument(stmt, local))
                || condition.as_ref().is_some_and(in_expr)
                || update.iter().any(in_expr)
                || in_block(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => in_expr(iterable) || in_block(body),
        Stmt::Switch { selector, cases } => {
            in_expr(selector)
                || cases.iter().any(|case| {
                    case.statements
                        .iter()
                        .any(|stmt| stmt_passes_map_entry_local_as_argument(stmt, local))
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
                .any(|stmt| stmt_passes_map_entry_local_as_argument(stmt, local))
                || in_block(body)
                || catches.iter().any(|catch| in_block(&catch.body))
                || finally_block.as_ref().is_some_and(in_block)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => in_block(block),
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(in_expr),
        _ => false,
    }
}

fn expr_passes_map_entry_local_as_argument(expr: &Expr, local: &str) -> bool {
    let nested = |expr: &Expr| expr_passes_map_entry_local_as_argument(expr, local);
    match expr {
        Expr::Call { receiver, args, .. } => {
            args.iter()
                .any(|arg| matches!(arg, Expr::Var(name) if name == local))
                || receiver.as_deref().is_some_and(nested)
                || args.iter().any(nested)
        }
        Expr::New {
            qualifier, args, ..
        } => {
            args.iter()
                .any(|arg| matches!(arg, Expr::Var(name) if name == local))
                || qualifier.as_deref().is_some_and(nested)
                || args.iter().any(nested)
        }
        Expr::FieldAccess(inner, _)
        | Expr::Unary { expr: inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::InstanceOf { expr: inner, .. } => nested(inner),
        Expr::ArrayAccess(left, right) | Expr::Binary { left, right, .. } => {
            nested(left) || nested(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => nested(condition) || nested(then_expr) || nested(else_expr),
        Expr::Switch { selector, cases } => {
            nested(selector) || cases.iter().any(|case| nested(&case.value))
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(nested)
                || initializer
                    .as_ref()
                    .is_some_and(|values| values.iter().any(nested))
        }
        Expr::ArrayInitializer(values) => values.iter().any(nested),
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => nested(expr),
            LambdaBody::Block(block) => block
                .statements
                .iter()
                .any(|stmt| stmt_passes_map_entry_local_as_argument(stmt, local)),
        },
        _ => false,
    }
}

fn type_has_live_native_map_parameter(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => method.body.as_ref().is_some_and(|body| {
            method.params.iter().any(|param| {
                matches!(&param.ty, JavaType::Named { name, .. }
                    if matches!(name.as_str(),
                        "Map" | "java.util.Map"
                            | "SortedMap" | "java.util.SortedMap"
                            | "NavigableMap" | "java.util.NavigableMap"
                            | "HashMap" | "java.util.HashMap"
                            | "LinkedHashMap" | "java.util.LinkedHashMap"
                            | "TreeMap" | "java.util.TreeMap"))
                    && body.statements.iter().any(|stmt| {
                        stmt_uses_native_map_entry_next_on_field(
                            stmt,
                            &param.source_name,
                            &crate::parser::to_snake_case(&param.source_name),
                        )
                    })
            })
        }),
        Member::Type(nested) => type_has_live_native_map_parameter(nested),
        _ => false,
    })
}

fn nullable_live_native_map_field_slots(
    program: &Program,
    live_fields: &HashSet<(String, String)>,
) -> (HashSet<(String, String)>, HashSet<(String, String)>) {
    let mut nullable_keys = HashSet::new();
    let mut nullable_values = HashSet::new();
    for ty in &program.types {
        for member in &ty.members {
            let Member::Field(field) = member else {
                continue;
            };
            let key = (ty.name.clone(), field.name.clone());
            if !live_fields.contains(&key) {
                continue;
            }
            let rust_name = crate::parser::to_snake_case(&field.name);
            let mut slots = (false, false);
            for member in &ty.members {
                match member {
                    Member::Method(method) => {
                        if let Some(body) = &method.body {
                            merge_nullable_map_slots(
                                &mut slots,
                                block_nullable_map_field_put_slots(body, &field.name, &rust_name),
                            );
                        }
                    }
                    Member::Constructor(ctor) => {
                        if let Some(body) = &ctor.body {
                            merge_nullable_map_slots(
                                &mut slots,
                                block_nullable_map_field_put_slots(body, &field.name, &rust_name),
                            );
                        }
                    }
                    _ => {}
                }
            }
            if slots.0 {
                nullable_keys.insert(key.clone());
            }
            if slots.1 {
                nullable_values.insert(key);
            }
        }
    }
    (nullable_keys, nullable_values)
}

fn merge_nullable_map_slots(target: &mut (bool, bool), next: (bool, bool)) {
    target.0 |= next.0;
    target.1 |= next.1;
}

fn block_nullable_map_field_put_slots(
    block: &Block,
    source_name: &str,
    rust_name: &str,
) -> (bool, bool) {
    let mut slots = (false, false);
    for stmt in &block.statements {
        merge_nullable_map_slots(
            &mut slots,
            stmt_nullable_map_field_put_slots(stmt, source_name, rust_name),
        );
    }
    slots
}

fn stmt_nullable_map_field_put_slots(
    stmt: &Stmt,
    source_name: &str,
    rust_name: &str,
) -> (bool, bool) {
    let expr_slots = |expr: &Expr| expr_nullable_map_field_put_slots(expr, source_name, rust_name);
    let block_slots =
        |block: &Block| block_nullable_map_field_put_slots(block, source_name, rust_name);
    let mut slots = (false, false);
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            if let Some(expr) = initializer {
                merge_nullable_map_slots(&mut slots, expr_slots(expr));
            }
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            merge_nullable_map_slots(&mut slots, expr_slots(expr));
        }
        Stmt::Assign { target, value } => {
            merge_nullable_map_slots(&mut slots, expr_slots(target));
            merge_nullable_map_slots(&mut slots, expr_slots(value));
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            merge_nullable_map_slots(&mut slots, expr_slots(condition));
            merge_nullable_map_slots(&mut slots, block_slots(then_block));
            if let Some(block) = else_block {
                merge_nullable_map_slots(&mut slots, block_slots(block));
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            merge_nullable_map_slots(&mut slots, expr_slots(condition));
            merge_nullable_map_slots(&mut slots, block_slots(body));
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                merge_nullable_map_slots(
                    &mut slots,
                    stmt_nullable_map_field_put_slots(stmt, source_name, rust_name),
                );
            }
            if let Some(condition) = condition {
                merge_nullable_map_slots(&mut slots, expr_slots(condition));
            }
            for expr in update {
                merge_nullable_map_slots(&mut slots, expr_slots(expr));
            }
            merge_nullable_map_slots(&mut slots, block_slots(body));
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            merge_nullable_map_slots(&mut slots, expr_slots(iterable));
            merge_nullable_map_slots(&mut slots, block_slots(body));
        }
        Stmt::Switch { selector, cases } => {
            merge_nullable_map_slots(&mut slots, expr_slots(selector));
            for case in cases {
                for stmt in &case.statements {
                    merge_nullable_map_slots(
                        &mut slots,
                        stmt_nullable_map_field_put_slots(stmt, source_name, rust_name),
                    );
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for stmt in resources {
                merge_nullable_map_slots(
                    &mut slots,
                    stmt_nullable_map_field_put_slots(stmt, source_name, rust_name),
                );
            }
            merge_nullable_map_slots(&mut slots, block_slots(body));
            for catch in catches {
                merge_nullable_map_slots(&mut slots, block_slots(&catch.body));
            }
            if let Some(block) = finally_block {
                merge_nullable_map_slots(&mut slots, block_slots(block));
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            merge_nullable_map_slots(&mut slots, block_slots(block));
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for expr in args {
                merge_nullable_map_slots(&mut slots, expr_slots(expr));
            }
        }
        _ => {}
    }
    slots
}

fn expr_nullable_map_field_put_slots(
    expr: &Expr,
    source_name: &str,
    rust_name: &str,
) -> (bool, bool) {
    let nested = |expr: &Expr| expr_nullable_map_field_put_slots(expr, source_name, rust_name);
    let mut slots = (false, false);
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            if matches!(name.as_str(), "put" | "putIfAbsent" | "put_if_absent")
                && args.len() == 2
                && receiver.as_deref().is_some_and(|receiver| {
                    expr_is_named_instance_field(receiver, source_name, rust_name)
                })
            {
                slots.0 |= matches!(args[0], Expr::Literal(crate::java_ir::Literal::Null));
                slots.1 |= matches!(args[1], Expr::Literal(crate::java_ir::Literal::Null));
            }
            if let Some(receiver) = receiver {
                merge_nullable_map_slots(&mut slots, nested(receiver));
            }
            for arg in args {
                merge_nullable_map_slots(&mut slots, nested(arg));
            }
        }
        Expr::New {
            qualifier, args, ..
        } => {
            if let Some(qualifier) = qualifier {
                merge_nullable_map_slots(&mut slots, nested(qualifier));
            }
            for arg in args {
                merge_nullable_map_slots(&mut slots, nested(arg));
            }
        }
        Expr::FieldAccess(inner, _)
        | Expr::Unary { expr: inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::InstanceOf { expr: inner, .. } => {
            merge_nullable_map_slots(&mut slots, nested(inner));
        }
        Expr::ArrayAccess(left, right) | Expr::Binary { left, right, .. } => {
            merge_nullable_map_slots(&mut slots, nested(left));
            merge_nullable_map_slots(&mut slots, nested(right));
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            merge_nullable_map_slots(&mut slots, nested(condition));
            merge_nullable_map_slots(&mut slots, nested(then_expr));
            merge_nullable_map_slots(&mut slots, nested(else_expr));
        }
        Expr::Switch { selector, cases } => {
            merge_nullable_map_slots(&mut slots, nested(selector));
            for case in cases {
                merge_nullable_map_slots(&mut slots, nested(&case.value));
            }
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                merge_nullable_map_slots(&mut slots, nested(dimension));
            }
            if let Some(values) = initializer {
                for value in values {
                    merge_nullable_map_slots(&mut slots, nested(value));
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for value in values {
                merge_nullable_map_slots(&mut slots, nested(value));
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => merge_nullable_map_slots(&mut slots, nested(expr)),
            LambdaBody::Block(block) => merge_nullable_map_slots(
                &mut slots,
                block_nullable_map_field_put_slots(block, source_name, rust_name),
            ),
        },
        _ => {}
    }
    slots
}

fn expr_is_named_instance_field(expr: &Expr, source_name: &str, rust_name: &str) -> bool {
    matches!(expr, Expr::Var(name) if name == source_name || name == rust_name)
        || matches!(expr, Expr::FieldAccess(receiver, name)
            if (name == source_name || name == rust_name)
                && (matches!(receiver.as_ref(), Expr::This)
                    || matches!(receiver.as_ref(), Expr::Var(value) if value == "this")))
}

fn map_entry_next_backing_field<'a>(expr: &'a Expr) -> Option<&'a Expr> {
    let Expr::Call {
        receiver: Some(iterator),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if name != "next" || !args.is_empty() {
        return None;
    }
    let Expr::Call {
        receiver: Some(entry_set),
        name,
        args,
        ..
    } = iterator.as_ref()
    else {
        return None;
    };
    if name != "iterator" || !args.is_empty() {
        return None;
    }
    let Expr::Call {
        receiver: Some(map),
        name,
        args,
        ..
    } = entry_set.as_ref()
    else {
        return None;
    };
    (matches!(name.as_str(), "entrySet" | "entry_set") && args.is_empty()).then_some(map.as_ref())
}

fn expr_uses_native_map_entry_next_on_field(
    expr: &Expr,
    source_name: &str,
    rust_name: &str,
) -> bool {
    if map_entry_next_backing_field(expr)
        .is_some_and(|map| expr_is_named_instance_field(map, source_name, rust_name))
    {
        return true;
    }
    let nested =
        |expr: &Expr| expr_uses_native_map_entry_next_on_field(expr, source_name, rust_name);
    match expr {
        Expr::Call { receiver, args, .. } => {
            receiver.as_deref().is_some_and(nested) || args.iter().any(nested)
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier.as_deref().is_some_and(nested)
                || args.iter().any(nested)
                || body.as_ref().is_some_and(|members| {
                    members.iter().any(|member| match member {
                        Member::Field(field) => field.initializer.as_ref().is_some_and(nested),
                        Member::Method(method) => method.body.as_ref().is_some_and(|body| {
                            body.statements.iter().any(|stmt| {
                                stmt_uses_native_map_entry_next_on_field(
                                    stmt,
                                    source_name,
                                    rust_name,
                                )
                            })
                        }),
                        _ => false,
                    })
                })
        }
        Expr::FieldAccess(inner, _)
        | Expr::Unary { expr: inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::InstanceOf { expr: inner, .. } => nested(inner),
        Expr::ArrayAccess(left, right) | Expr::Binary { left, right, .. } => {
            nested(left) || nested(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => nested(condition) || nested(then_expr) || nested(else_expr),
        Expr::Switch { selector, cases } => {
            nested(selector) || cases.iter().any(|case| nested(&case.value))
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(nested)
                || initializer
                    .as_ref()
                    .is_some_and(|values| values.iter().any(nested))
        }
        Expr::ArrayInitializer(values) => values.iter().any(nested),
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => nested(expr),
            LambdaBody::Block(block) => block
                .statements
                .iter()
                .any(|stmt| stmt_uses_native_map_entry_next_on_field(stmt, source_name, rust_name)),
        },
        _ => false,
    }
}

fn stmt_uses_native_map_entry_next_on_field(
    stmt: &Stmt,
    source_name: &str,
    rust_name: &str,
) -> bool {
    let in_expr =
        |expr: &Expr| expr_uses_native_map_entry_next_on_field(expr, source_name, rust_name);
    let in_block = |block: &Block| {
        block
            .statements
            .iter()
            .any(|stmt| stmt_uses_native_map_entry_next_on_field(stmt, source_name, rust_name))
    };
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            initializer.as_ref().is_some_and(in_expr)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => in_expr(expr),
        Stmt::Assign { target, value } => in_expr(target) || in_expr(value),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            in_expr(condition) || in_block(then_block) || else_block.as_ref().is_some_and(in_block)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => in_expr(condition) || in_block(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_uses_native_map_entry_next_on_field(stmt, source_name, rust_name))
                || condition.as_ref().is_some_and(in_expr)
                || update.iter().any(in_expr)
                || in_block(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => in_expr(iterable) || in_block(body),
        Stmt::Switch { selector, cases } => {
            in_expr(selector)
                || cases.iter().any(|case| {
                    case.statements.iter().any(|stmt| {
                        stmt_uses_native_map_entry_next_on_field(stmt, source_name, rust_name)
                    })
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
                .any(|stmt| stmt_uses_native_map_entry_next_on_field(stmt, source_name, rust_name))
                || in_block(body)
                || catches.iter().any(|catch| in_block(&catch.body))
                || finally_block.as_ref().is_some_and(in_block)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => in_block(block),
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(in_expr),
        _ => false,
    }
}

fn promote_anonymous_class_bases_to_source_hierarchy_carriers(plan: &mut RepresentationPlan) {
    let bases = plan
        .anonymous_class_bases
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    for base in bases {
        let variants = source_hierarchy_carrier_variants_for_root(plan, &base);
        let Some(entry) = plan.types.get_mut(&base) else {
            continue;
        };
        if !matches!(entry.representation, ObjectRepresentation::ConcreteStruct) {
            continue;
        }
        entry.representation = ObjectRepresentation::SourceHierarchyCarrier {
            carrier_name: format!(
                "{}Value",
                crate::java_naming::rust_name_for_java_named_type(&base)
            ),
            variants,
        };
    }
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
    if crate::stdlib::is_java_platform_facade_type_name(&ty.name)
        || ty
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
        .map(|name| crate::java_naming::rust_name_for_java_named_type(&name))
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
    pub fn source_map_entry_uses_value_carrier(&self) -> bool {
        self.source_map_entry_value_carrier
    }

    pub(crate) fn field_needs_live_native_map(&self, owner: &str, field: &str) -> bool {
        field_key_set_contains(&self.live_native_map_fields, owner, field)
    }

    pub(crate) fn static_field_needs_live_native_map(&self, owner: &str, field: &str) -> bool {
        field_key_set_contains(&self.live_static_native_map_fields, owner, field)
    }

    pub(crate) fn live_static_native_map_field_has_nullable_key(
        &self,
        owner: &str,
        field: &str,
    ) -> bool {
        field_key_set_contains(
            &self.nullable_live_static_native_map_field_keys,
            owner,
            field,
        )
    }

    pub(crate) fn live_static_native_map_field_has_nullable_value(
        &self,
        owner: &str,
        field: &str,
    ) -> bool {
        field_key_set_contains(
            &self.nullable_live_static_native_map_field_values,
            owner,
            field,
        )
    }

    pub(crate) fn live_native_map_field_has_nullable_key(&self, owner: &str, field: &str) -> bool {
        field_key_set_contains(&self.nullable_live_native_map_field_keys, owner, field)
    }

    pub(crate) fn live_native_map_field_has_nullable_value(
        &self,
        owner: &str,
        field: &str,
    ) -> bool {
        field_key_set_contains(&self.nullable_live_native_map_field_values, owner, field)
    }

    pub fn type_representations(&self) -> impl Iterator<Item = &TypeRepresentation> {
        self.types.values()
    }

    pub fn type_representation(&self, type_name: &str) -> Option<&TypeRepresentation> {
        self.types
            .get(type_name)
            .or_else(|| self.unique_type_representation_by_simple_name(type_name))
    }

    /// Returns the Rust carrier and variant used to construct a source value
    /// whose declared type is the root of a hierarchy carrier.
    ///
    /// Lowering layers consume this projection instead of reopening the
    /// representation axis and matching `ObjectRepresentation` themselves.
    pub(crate) fn source_hierarchy_carrier_constructor_parts(
        &self,
        type_name: &str,
    ) -> Option<(&str, String)> {
        let representation = self.type_representation(type_name)?;
        let ObjectRepresentation::SourceHierarchyCarrier {
            carrier_name,
            variants,
        } = representation.representation()
        else {
            return None;
        };
        let variant = crate::java_naming::rust_name_for_java_named_type(type_name);
        variants
            .iter()
            .any(|candidate| candidate == &variant)
            .then_some((carrier_name.as_str(), variant))
    }

    pub fn representation_for(&self, java_ty: &JavaType) -> ObjectRepresentation {
        self.representation_lookup_policy(java_ty)
            .representation()
            .clone()
    }

    pub fn storage_placement_for(
        &self,
        type_name: &str,
    ) -> crate::storage_strategy::StoragePlacement {
        self.storage.placement(type_name)
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
        representation_dispatch_policy(self.representation_for(java_ty)).into_dispatch()
    }

    pub fn anonymous_class_base_requires_virtual_carrier(&self, type_name: &str) -> bool {
        self.anonymous_class_bases.contains(type_name)
            || (!type_name.contains('.')
                && self
                    .anonymous_class_bases
                    .iter()
                    .filter(|candidate| candidate.rsplit('.').next() == Some(type_name))
                    .take(2)
                    .count()
                    == 1)
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
        let rust_enum_name = crate::java_naming::rust_name_for_java_named_type(enum_name);
        variants
            .iter()
            .any(|known| known == &variant)
            .then(|| JavaEnumVariantPath::new(rust_enum_name, variant))
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
        self.diagnostics_for_program_with_anonymous_context(program, program)
    }

    pub fn diagnostics_for_program_with_anonymous_context(
        &self,
        program: &Program,
        anonymous_context: &Program,
    ) -> Vec<RepresentationDiagnostic> {
        self.diagnostics
            .iter()
            .cloned()
            .chain(
                all_types(program)
                    .into_iter()
                    .flat_map(|ty| self.diagnostics_for_type(ty)),
            )
            .chain(anonymous_class_diagnostics_with_plan(
                program,
                anonymous_context,
                Some(self),
            ))
            .collect()
    }

    pub fn diagnostics_for_type(&self, ty: &TypeDecl) -> Vec<RepresentationDiagnostic> {
        hierarchy_edge_diagnostic_policy(ty, self).into_diagnostics()
    }

    fn hierarchy_edges_for_subtype(&self, type_name: &str) -> Vec<ObjectHierarchyEdge> {
        if let Some(edges) = self.hierarchy_edges_by_subtype.get(type_name) {
            return edges.clone();
        }
        self.hierarchy_edges_by_subtype
            .values()
            .flat_map(|edges| edges.iter())
            .filter(|edge| crate::facts::external_type_name_matches(edge.sub_name(), type_name))
            .cloned()
            .collect()
    }

    fn hierarchy_edge_is_represented(&self, super_name: &str) -> bool {
        hierarchy_type_name_lookup_aliases(super_name)
            .iter()
            .any(|alias| {
                self.hierarchy_type_is_represented(alias)
                    || self
                        .hierarchy_type_has_user_closed_enum_ancestor(alias, &mut BTreeSet::new())
            })
    }

    fn hierarchy_type_is_represented(&self, type_name: &str) -> bool {
        self.type_representation(type_name).is_some_and(|entry| {
            object_hierarchy_edge_representation_policy(entry.representation.clone()).represented()
        })
    }

    fn hierarchy_type_is_user_closed_enum(&self, type_name: &str) -> bool {
        self.type_representation(type_name).is_some_and(|entry| {
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
            .into_iter()
            .any(|edge| {
                self.hierarchy_type_is_user_closed_enum(edge.super_name())
                    || self.hierarchy_type_has_user_closed_enum_ancestor(edge.super_name(), visited)
            })
    }

    fn hierarchy_edge_diagnostic_input(&self, ty: &TypeDecl) -> ObjectHierarchyEdgeDiagnosticInput {
        let edges = self.hierarchy_edges_for_subtype(&ty.name);
        let subtype_owns_source_carrier =
            self.type_representation(&ty.name)
                .is_some_and(|representation| {
                    matches!(
                        representation.representation(),
                        ObjectRepresentation::SourceHierarchyCarrier { .. }
                    )
                });
        let represented_super_names = edges
            .iter()
            .map(|edge| edge.super_name())
            .chain(ty.extends.iter().map(String::as_str))
            .chain(ty.implements.iter().map(String::as_str))
            .filter(|super_name| {
                // A selected source carrier owns the complete concrete shell
                // of its root, including flattened inherited state and direct
                // `super` behavior.  Its own extends edge is therefore part of
                // the represented carrier, even when the ancestor does not
                // independently need a polymorphic carrier.
                subtype_owns_source_carrier
                    || self.hierarchy_edge_is_represented(super_name)
                    || self.concretely_lowered_hierarchy_edges.iter().any(
                        |(subtype, represented_super)| {
                            crate::facts::external_type_name_matches(subtype, &ty.name)
                                && crate::facts::external_type_name_matches(
                                    represented_super,
                                    super_name,
                                )
                        },
                    )
            })
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        ObjectHierarchyEdgeDiagnosticInput::new(
            ty.name.clone(),
            ty.extends.clone(),
            ty.implements.clone(),
            edges,
            represented_super_names,
        )
    }
}

fn field_key_set_contains(fields: &HashSet<(String, String)>, owner: &str, field: &str) -> bool {
    fields.contains(&(owner.to_string(), field.to_string()))
        || (!owner.contains('.')
            && fields.iter().any(|(candidate, candidate_field)| {
                candidate.rsplit('.').next() == Some(owner) && candidate_field == field
            }))
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
            enum_name: crate::java_naming::rust_name_for_java_named_type(enum_name),
        },
        ObjectRepresentation::SourceHierarchyCarrier { carrier_name, .. } => {
            DispatchShape::ClosedEnum {
                enum_name: crate::java_naming::rust_name_for_java_named_type(carrier_name),
            }
        }
        ObjectRepresentation::ErasedObject => DispatchShape::ErasedObject,
    };
    RepresentationDispatchPolicy::new(representation, projected_dispatch)
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
        .extend(policy.facts().iter().cloned());
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
    SourceSummarySuperclassBoundaryPolicy::new(boundary_types.into_iter().collect(), facts)
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
    anonymous_context: &Program,
    plan: Option<&RepresentationPlan>,
) -> Vec<RepresentationDiagnostic> {
    let mut diagnostics = Vec::new();
    for ty in all_types(program) {
        collect_anonymous_class_diagnostics_for_type(ty, anonymous_context, plan, &mut diagnostics);
    }
    diagnostics
}

fn collect_anonymous_class_diagnostics_for_type(
    ty: &TypeDecl,
    anonymous_context: &Program,
    plan: Option<&RepresentationPlan>,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_anonymous_class_diagnostics_for_expr(
                        initializer,
                        anonymous_context,
                        plan,
                        diagnostics,
                    );
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_anonymous_class_diagnostics_for_block(
                        body,
                        anonymous_context,
                        plan,
                        diagnostics,
                    );
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_anonymous_class_diagnostics_for_block(
                        body,
                        anonymous_context,
                        plan,
                        diagnostics,
                    );
                }
            }
            Member::Type(nested) => {
                collect_anonymous_class_diagnostics_for_type(
                    nested,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_anonymous_class_diagnostics_for_block(
    block: &Block,
    anonymous_context: &Program,
    plan: Option<&RepresentationPlan>,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    for stmt in &block.statements {
        collect_anonymous_class_diagnostics_for_stmt(stmt, anonymous_context, plan, diagnostics);
    }
}

fn collect_anonymous_class_diagnostics_for_stmt(
    stmt: &Stmt,
    anonymous_context: &Program,
    plan: Option<&RepresentationPlan>,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_anonymous_class_diagnostics_for_expr(
                    initializer,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
        }
        Stmt::Assign { target, value } => {
            collect_anonymous_class_diagnostics_for_expr(
                target,
                anonymous_context,
                plan,
                diagnostics,
            );
            collect_anonymous_class_diagnostics_for_expr(
                value,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_anonymous_class_diagnostics_for_expr(
                expr,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_anonymous_class_diagnostics_for_expr(
                    expr,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_anonymous_class_diagnostics_for_expr(
                condition,
                anonymous_context,
                plan,
                diagnostics,
            );
            collect_anonymous_class_diagnostics_for_block(
                then_block,
                anonymous_context,
                plan,
                diagnostics,
            );
            if let Some(else_block) = else_block {
                collect_anonymous_class_diagnostics_for_block(
                    else_block,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_anonymous_class_diagnostics_for_expr(
                condition,
                anonymous_context,
                plan,
                diagnostics,
            );
            collect_anonymous_class_diagnostics_for_block(
                body,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_anonymous_class_diagnostics_for_stmt(
                    stmt,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            if let Some(condition) = condition {
                collect_anonymous_class_diagnostics_for_expr(
                    condition,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            for expr in update {
                collect_anonymous_class_diagnostics_for_expr(
                    expr,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            collect_anonymous_class_diagnostics_for_block(
                body,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_anonymous_class_diagnostics_for_expr(
                iterable,
                anonymous_context,
                plan,
                diagnostics,
            );
            collect_anonymous_class_diagnostics_for_block(
                body,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Stmt::Switch { selector, cases } => {
            collect_anonymous_class_diagnostics_for_expr(
                selector,
                anonymous_context,
                plan,
                diagnostics,
            );
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_anonymous_class_diagnostics_for_expr(
                            expr,
                            anonymous_context,
                            plan,
                            diagnostics,
                        );
                    }
                }
                for stmt in &case.statements {
                    collect_anonymous_class_diagnostics_for_stmt(
                        stmt,
                        anonymous_context,
                        plan,
                        diagnostics,
                    );
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
                collect_anonymous_class_diagnostics_for_stmt(
                    resource,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            collect_anonymous_class_diagnostics_for_block(
                body,
                anonymous_context,
                plan,
                diagnostics,
            );
            for catch in catches {
                collect_anonymous_class_diagnostics_for_block(
                    &catch.body,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_anonymous_class_diagnostics_for_block(
                    finally_block,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
        }
        Stmt::LocalType(ty) => {
            collect_anonymous_class_diagnostics_for_type(ty, anonymous_context, plan, diagnostics)
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(
                    arg,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_anonymous_class_diagnostics_for_block(
                block,
                anonymous_context,
                plan,
                diagnostics,
            )
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_anonymous_class_diagnostics_for_expr(
    expr: &Expr,
    anonymous_context: &Program,
    plan: Option<&RepresentationPlan>,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            collect_anonymous_class_diagnostics_for_expr(
                receiver,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Expr::ArrayAccess(array, index) => {
            collect_anonymous_class_diagnostics_for_expr(
                array,
                anonymous_context,
                plan,
                diagnostics,
            );
            collect_anonymous_class_diagnostics_for_expr(
                index,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_anonymous_class_diagnostics_for_expr(
                    expr,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_anonymous_class_diagnostics_for_expr(
                        expr,
                        anonymous_context,
                        plan,
                        diagnostics,
                    );
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_anonymous_class_diagnostics_for_expr(
                    expr,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_anonymous_class_diagnostics_for_expr(
                left,
                anonymous_context,
                plan,
                diagnostics,
            );
            collect_anonymous_class_diagnostics_for_expr(
                right,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            collect_anonymous_class_diagnostics_for_expr(
                expr,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_anonymous_class_diagnostics_for_expr(
                condition,
                anonymous_context,
                plan,
                diagnostics,
            );
            collect_anonymous_class_diagnostics_for_expr(
                then_expr,
                anonymous_context,
                plan,
                diagnostics,
            );
            collect_anonymous_class_diagnostics_for_expr(
                else_expr,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Expr::Switch { selector, cases } => {
            collect_anonymous_class_diagnostics_for_expr(
                selector,
                anonymous_context,
                plan,
                diagnostics,
            );
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_anonymous_class_diagnostics_for_expr(
                            expr,
                            anonymous_context,
                            plan,
                            diagnostics,
                        );
                    }
                }
                collect_anonymous_class_diagnostics_for_expr(
                    &case.value,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_anonymous_class_diagnostics_for_expr(
                    receiver,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(
                    arg,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
        }
        Expr::New { ty, args, body, .. } => {
            if anonymous_priority_queue_comparator_constructor_is_supported(ty, args)
                || anonymous_functional_interface_is_supported(
                    ty,
                    args,
                    body.as_deref(),
                    anonymous_context,
                )
                || anonymous_fieldless_interface_object_is_supported(
                    ty,
                    args,
                    body.as_deref(),
                    anonymous_context,
                )
                || anonymous_source_class_super_object_is_supported(
                    ty,
                    body.as_deref(),
                    anonymous_context,
                    plan,
                )
            {
                return;
            }
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(
                    arg,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
            }
            if body.is_some() && !anonymous_class_target_is_erased_boundary(ty, plan) {
                diagnostics.push(RepresentationDiagnostic::new(
                    "unsupported_anonymous_class",
                    format!(
                        "anonymous Java class for {:?} requires object representation lowering",
                        ty
                    ),
                ));
            }
        }
        Expr::InstanceOf { expr, .. } => {
            collect_anonymous_class_diagnostics_for_expr(
                expr,
                anonymous_context,
                plan,
                diagnostics,
            );
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_anonymous_class_diagnostics_for_expr(
                expr,
                anonymous_context,
                plan,
                diagnostics,
            ),
            LambdaBody::Block(block) => {
                collect_anonymous_class_diagnostics_for_block(
                    block,
                    anonymous_context,
                    plan,
                    diagnostics,
                );
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

fn anonymous_source_class_super_object_is_supported(
    ty: &JavaType,
    body: Option<&[Member]>,
    anonymous_context: &Program,
    plan: Option<&RepresentationPlan>,
) -> bool {
    let JavaType::Named { name, .. } = ty else {
        return false;
    };
    let Some(plan) = plan else {
        return false;
    };
    if !plan.anonymous_class_base_requires_virtual_carrier(name) {
        return false;
    }
    let Some(base) = find_type_by_erased_name(anonymous_context, name) else {
        return false;
    };
    if base.kind != TypeKind::Class {
        return false;
    }
    let Some(body) = body else {
        return false;
    };
    if body.iter().any(|member| match member {
        Member::Method(_) => false,
        Member::Field(field) => {
            field.modifiers.contains(&Modifier::Static)
                || field.initializer.is_none()
                || !matches!(
                    field.ty,
                    JavaType::Primitive(_) | JavaType::String | JavaType::Named { .. }
                )
        }
        _ => true,
    }) {
        return false;
    }
    let field_names = body
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(crate::parser::to_snake_case(&field.name)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mutable_field_names = body
        .iter()
        .filter_map(|member| match member {
            Member::Field(field)
                if matches!(
                    field.ty,
                    JavaType::Primitive(_) | JavaType::String | JavaType::Named { .. }
                ) =>
            {
                Some(crate::parser::to_snake_case(&field.name))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let methods = body
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .collect::<Vec<_>>();
    methods.into_iter().all(|method| {
        let is_base_override = base.members.iter().any(|member| {
            matches!(member, Member::Method(candidate)
                    if !candidate.modifiers.contains(&Modifier::Static)
                        && candidate.name == method.name
                        && crate::java_signature::erased_param_types(&candidate.params)
                            == crate::java_signature::erased_param_types(&method.params))
        })
            || crate::inherited_members::inherited_instance_method_refs_for_type(
                base,
                anonymous_context,
            )
            .iter()
            .any(|inherited| {
                inherited.method().name == method.name
                    && crate::java_signature::erased_param_types(&inherited.method().params)
                        == crate::java_signature::erased_param_types(&method.params)
            });
        if is_base_override {
            anonymous_source_class_super_method_body_is_supported(
                method,
                &field_names,
                &mutable_field_names,
            )
        } else {
            anonymous_source_class_helper_method_is_supported(method, &field_names)
        }
    })
}

fn anonymous_source_class_helper_method_is_supported(
    method: &crate::java_ir::MethodDecl,
    field_names: &[String],
) -> bool {
    if method.modifiers.contains(&Modifier::Static) || !method.type_params.is_empty() {
        return false;
    }
    matches!(method.body.as_ref().map(|body| body.statements.as_slice()),
        Some([Stmt::Return(Some(expr))])
            if anonymous_source_class_super_return_is_supported(expr, field_names)
                && !matches!(expr, Expr::Call { receiver: Some(receiver), .. } if matches!(receiver.as_ref(), Expr::Super)))
}

fn anonymous_source_class_super_method_body_is_supported(
    method: &crate::java_ir::MethodDecl,
    field_names: &[String],
    mutable_field_names: &[String],
) -> bool {
    if method.modifiers.contains(&Modifier::Static) || !method.type_params.is_empty() {
        return false;
    }
    let Some(Block { statements }) = &method.body else {
        return false;
    };
    if anonymous_source_class_local_statement_body_is_supported(
        statements,
        field_names,
        false,
        &HashSet::new(),
        &HashSet::new(),
        &method
            .params
            .iter()
            .map(|param| (param.name.clone(), param.ty.clone()))
            .collect(),
        &mut HashSet::new(),
    ) {
        return true;
    }
    if anonymous_source_class_direct_reference_member_write_body_is_supported(
        statements,
        field_names,
    ) {
        return true;
    }
    // The class-adapter lowerer preserves a sequential run of initialized
    // locals as Rust locals, after rebinding any anonymous field/capture read
    // in each initializer.  Do this before the historical one/two-local
    // shapes below so adding an intermediate local does not silently lose the
    // anonymous representation.
    if let Some((Stmt::Return(Some(returned)), locals)) = statements.split_last()
        && !locals.is_empty()
        && locals.iter().all(|statement| {
            matches!(statement, Stmt::VarDecl { initializer: Some(initializer), .. }
                if anonymous_source_class_super_return_is_supported(initializer, field_names))
        })
        && anonymous_source_class_super_return_is_supported(returned, field_names)
    {
        return true;
    }
    match statements.as_slice() {
        [
            Stmt::VarDecl {
                initializer: Some(first),
                ..
            },
            Stmt::VarDecl {
                initializer: Some(second),
                ..
            },
            Stmt::Return(Some(Expr::Var(_) | Expr::Binary { .. })),
        ] => {
            anonymous_source_class_mutable_assignment_value_is_supported(first, field_names)
                && anonymous_source_class_mutable_assignment_value_is_supported(second, field_names)
        }
        [
            Stmt::Expr(Expr::Call {
                receiver: Some(receiver),
                ..
            }),
            Stmt::Return(Some(Expr::Call {
                receiver: Some(return_receiver),
                ..
            })),
        ] => matches!(
            (receiver.as_ref(), return_receiver.as_ref()),
            (Expr::Var(capture), Expr::Var(return_capture)) if capture == return_capture
        ),
        [
            Stmt::Assign {
                target: Expr::FieldAccess(receiver, assigned),
                value,
            },
            Stmt::Return(Some(Expr::FieldAccess(return_receiver, returned))),
        ] if matches!(receiver.as_ref(), Expr::Var(capture)
            if matches!(return_receiver.as_ref(), Expr::Var(return_capture) if return_capture == capture)
                && assigned == returned) =>
        {
            anonymous_source_class_mutable_assignment_value_is_supported(value, field_names)
        }
        [Stmt::Return(Some(expr))] => {
            anonymous_source_class_super_return_is_supported(expr, field_names)
        }
        [
            Stmt::Assign {
                target: Expr::FieldAccess(receiver, assigned),
                value,
            },
            Stmt::Return(Some(returned)),
        ] => {
            let assigned = crate::parser::to_snake_case(assigned);
            matches!(receiver.as_ref(), Expr::This)
                && mutable_field_names.iter().any(|field| field == &assigned)
                && anonymous_source_class_mutable_assignment_value_is_supported(value, field_names)
                && match returned {
                    Expr::Var(name) => crate::parser::to_snake_case(name) == assigned,
                    Expr::FieldAccess(receiver, name) => {
                        matches!(receiver.as_ref(), Expr::This)
                            && crate::parser::to_snake_case(name) == assigned
                    }
                    _ => false,
                }
        }
        [
            Stmt::VarDecl {
                name,
                initializer: Some(initializer),
                ..
            },
            Stmt::Assign {
                target: Expr::FieldAccess(receiver, assigned),
                value: Expr::Var(value_name),
            },
            Stmt::Return(Some(returned)),
        ] => {
            let assigned = crate::parser::to_snake_case(assigned);
            matches!(receiver.as_ref(), Expr::This)
                && mutable_field_names.iter().any(|field| field == &assigned)
                && anonymous_source_class_mutable_assignment_value_is_supported(
                    initializer,
                    field_names,
                )
                && crate::parser::to_snake_case(value_name) == crate::parser::to_snake_case(name)
                && match returned {
                    Expr::Var(name) => crate::parser::to_snake_case(name) == assigned,
                    Expr::FieldAccess(receiver, name) => {
                        matches!(receiver.as_ref(), Expr::This)
                            && crate::parser::to_snake_case(name) == assigned
                    }
                    _ => false,
                }
        }
        [
            Stmt::If {
                condition,
                then_block,
                else_block: Some(else_block),
            },
            Stmt::Return(Some(returned)),
        ] => {
            let [
                Stmt::Assign {
                    target: Expr::FieldAccess(then_receiver, then_assigned),
                    value: then_value,
                },
            ] = then_block.statements.as_slice()
            else {
                return false;
            };
            let [
                Stmt::Assign {
                    target: Expr::FieldAccess(else_receiver, else_assigned),
                    value: else_value,
                },
            ] = else_block.statements.as_slice()
            else {
                return false;
            };
            let assigned = crate::parser::to_snake_case(then_assigned);
            matches!(then_receiver.as_ref(), Expr::This)
                && matches!(else_receiver.as_ref(), Expr::This)
                && assigned == crate::parser::to_snake_case(else_assigned)
                && anonymous_source_class_guard_condition_is_supported(condition)
                && mutable_field_names.iter().any(|field| field == &assigned)
                && anonymous_source_class_mutable_assignment_value_is_supported(
                    then_value,
                    field_names,
                )
                && anonymous_source_class_mutable_assignment_value_is_supported(
                    else_value,
                    field_names,
                )
                && match returned {
                    Expr::Var(name) => crate::parser::to_snake_case(name) == assigned,
                    Expr::FieldAccess(receiver, name) => {
                        matches!(receiver.as_ref(), Expr::This)
                            && crate::parser::to_snake_case(name) == assigned
                    }
                    _ => false,
                }
        }
        [
            Stmt::If {
                condition,
                then_block,
                else_block: None,
            },
            Stmt::Return(Some(returned)),
        ] => {
            let [
                Stmt::Assign {
                    target: Expr::FieldAccess(receiver, assigned),
                    value,
                },
            ] = then_block.statements.as_slice()
            else {
                return false;
            };
            let assigned = crate::parser::to_snake_case(assigned);
            matches!(receiver.as_ref(), Expr::This)
                && anonymous_source_class_guard_condition_is_supported(condition)
                && mutable_field_names.iter().any(|field| field == &assigned)
                && anonymous_source_class_mutable_assignment_value_is_supported(value, field_names)
                && match returned {
                    Expr::Var(name) => crate::parser::to_snake_case(name) == assigned,
                    Expr::FieldAccess(receiver, name) => {
                        matches!(receiver.as_ref(), Expr::This)
                            && crate::parser::to_snake_case(name) == assigned
                    }
                    _ => false,
                }
        }
        [
            Stmt::VarDecl {
                initializer: Some(initializer),
                ..
            },
            Stmt::Return(Some(returned)),
        ] => {
            (matches!(initializer, Expr::Call { receiver: Some(receiver), .. } if matches!(receiver.as_ref(), Expr::Super))
                && matches!(returned, Expr::Var(_) | Expr::Binary { .. }))
                || (anonymous_source_class_mutable_assignment_value_is_supported(
                    initializer,
                    field_names,
                ) && matches!(returned, Expr::Var(_) | Expr::Binary { .. }))
        }
        _ => false,
    }
}

fn anonymous_source_class_direct_reference_member_write_body_is_supported(
    statements: &[Stmt],
    field_names: &[String],
) -> bool {
    let Some((returned, before_return)) = statements.split_last() else {
        return false;
    };
    let Stmt::Return(Some(Expr::FieldAccess(return_receiver, _))) = returned else {
        return false;
    };
    let Some((assignment, locals)) = before_return.split_last() else {
        return false;
    };
    let Stmt::Assign {
        target: Expr::FieldAccess(receiver, _),
        ..
    } = assignment
    else {
        return false;
    };
    locals.iter().all(|statement| {
        matches!(statement, Stmt::VarDecl {
            ty: JavaType::Primitive(_) | JavaType::String,
            initializer: Some(initializer),
            ..
        } if anonymous_source_class_super_return_is_supported(initializer, field_names))
    }) && matches!(receiver.as_ref(), Expr::FieldAccess(this, field)
        if matches!(this.as_ref(), Expr::This)
            && field_names.iter().any(|candidate| candidate == &crate::parser::to_snake_case(field)))
        && matches!(return_receiver.as_ref(), Expr::FieldAccess(return_this, return_adapter_field)
            if matches!(return_this.as_ref(), Expr::This)
                && field_names.iter().any(|candidate| candidate == &crate::parser::to_snake_case(return_adapter_field)))
}

/// Keep the class-carrier eligibility check aligned with the adapter-local
/// statement renderer.  This deliberately excludes field writes: those need
/// the separate `Cell`/`RefCell` lowering tier, while ordinary locals and
/// nested conditionals are safe once every receiver-aware expression can be
/// rebound through the adapter.
fn anonymous_source_class_local_statement_body_is_supported(
    statements: &[Stmt],
    field_names: &[String],
    may_continue: bool,
    active_break_labels: &HashSet<String>,
    active_continue_labels: &HashSet<String>,
    params: &HashMap<String, JavaType>,
    aliases: &mut HashSet<String>,
) -> bool {
    statements.iter().all(|statement| {
        let admitted = match statement {
        Stmt::VarDecl {
            ty: JavaType::Primitive(_) | JavaType::String | JavaType::Named { .. },
            initializer: Some(initializer),
            ..
        } => anonymous_source_class_super_return_is_supported(initializer, field_names),
        Stmt::Expr(Expr::Unary {
            op: crate::java_ir::UnaryOp::PreInc
                | crate::java_ir::UnaryOp::PostInc
                | crate::java_ir::UnaryOp::PreDec
                | crate::java_ir::UnaryOp::PostDec,
            expr,
        }) if matches!(expr.as_ref(), Expr::Var(_)) => true,
        Stmt::Expr(Expr::Binary {
            op: crate::java_ir::BinaryOp::Assign,
            left,
            right,
        }) if matches!(left.as_ref(), Expr::Var(_)) => {
            anonymous_source_class_super_return_is_supported(right, field_names)
        }
        Stmt::Assign {
            target: Expr::Var(_),
            value,
        }
        | Stmt::Expr(value)
        | Stmt::Return(Some(value)) => {
            anonymous_source_class_super_return_is_supported(value, field_names)
                || anonymous_source_class_primitive_array_access_is_supported(value, params)
        }
        Stmt::Assign {
            target: Expr::FieldAccess(receiver, _),
            value,
        } if matches!(receiver.as_ref(), Expr::Var(name) if aliases.contains(name)) => {
            anonymous_source_class_alias_write_value_is_supported(value, aliases, params)
        }
        Stmt::Assign {
            target: Expr::FieldAccess(receiver, _),
            value,
        } if matches!(receiver.as_ref(), Expr::FieldAccess(this, field)
            if matches!(this.as_ref(), Expr::This)
                && field_names.iter().any(|candidate| candidate == &crate::parser::to_snake_case(field))) => {
            anonymous_source_class_super_return_is_supported(value, field_names)
        }
        Stmt::Assign {
            target: Expr::FieldAccess(receiver, field),
            value,
        } => {
            matches!(receiver.as_ref(), Expr::This)
                && field_names
                    .iter()
                    .any(|candidate| candidate == &crate::parser::to_snake_case(field))
                && anonymous_source_class_super_return_is_supported(value, field_names)
        }
        Stmt::Return(None) => true,
        Stmt::Throw(Expr::New { body: None, args, .. }) => {
            args.iter().all(|arg| matches!(arg, Expr::Literal(_)))
        }
        Stmt::Break(None) => true,
        Stmt::Continue(None) => may_continue,
        Stmt::Break(Some(label)) => active_break_labels.contains(label),
        Stmt::Continue(Some(label)) => active_continue_labels.contains(label),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            anonymous_source_class_super_return_is_supported(condition, field_names)
                && anonymous_source_class_local_statement_body_is_supported(
                    &then_block.statements,
                    field_names,
                    may_continue,
                    active_break_labels,
                    active_continue_labels,
                    params,
                    &mut aliases.clone(),
                )
                && else_block.as_ref().is_none_or(|else_block| {
                    anonymous_source_class_local_statement_body_is_supported(
                        &else_block.statements,
                        field_names,
                        may_continue,
                        active_break_labels,
                        active_continue_labels,
                        params,
                        &mut aliases.clone(),
                    )
                })
        }
        Stmt::Block(body) => anonymous_source_class_local_statement_body_is_supported(
            &body.statements,
            field_names,
            may_continue,
            active_break_labels,
            active_continue_labels,
            params,
            &mut aliases.clone(),
        ),
        Stmt::LabeledBlock { label, body } => {
            let mut block_break_labels = active_break_labels.clone();
            block_break_labels.insert(label.clone());
            anonymous_source_class_local_statement_body_is_supported(
                &body.statements,
                field_names,
                may_continue,
                &block_break_labels,
                active_continue_labels,
                params,
                &mut aliases.clone(),
            )
        }
        Stmt::While {
            condition,
            body,
            label,
        } => {
            let mut loop_break_labels = active_break_labels.clone();
            let mut loop_continue_labels = active_continue_labels.clone();
            if let Some(label) = label {
                loop_break_labels.insert(label.clone());
                loop_continue_labels.insert(label.clone());
            }
            anonymous_source_class_super_return_is_supported(condition, field_names)
                && anonymous_source_class_local_statement_body_is_supported(
                    &body.statements,
                    field_names,
                    true,
                    &loop_break_labels,
                    &loop_continue_labels,
                    params,
                    &mut aliases.clone(),
                )
        }
        Stmt::DoWhile {
            condition,
            body,
            label,
        } => {
            let mut loop_break_labels = active_break_labels.clone();
            let mut loop_continue_labels = active_continue_labels.clone();
            if let Some(label) = label {
                loop_break_labels.insert(label.clone());
                loop_continue_labels.insert(label.clone());
            }
            anonymous_source_class_super_return_is_supported(condition, field_names)
                && anonymous_source_class_local_statement_body_is_supported(
                    &body.statements,
                    field_names,
                    true,
                    &loop_break_labels,
                    &loop_continue_labels,
                    params,
                    &mut aliases.clone(),
                )
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            label,
        } => {
            let mut loop_break_labels = active_break_labels.clone();
            let mut loop_continue_labels = active_continue_labels.clone();
            let mut loop_aliases = aliases.clone();
            if let Some(label) = label {
                loop_break_labels.insert(label.clone());
                loop_continue_labels.insert(label.clone());
            }
            anonymous_source_class_local_statement_body_is_supported(
                init,
                field_names,
                false,
                active_break_labels,
                active_continue_labels,
                params,
                &mut loop_aliases,
            )
                && condition.as_ref().is_none_or(|condition| {
                    anonymous_source_class_super_return_is_supported(condition, field_names)
                })
                && update.iter().all(|update| {
                    matches!(update,
                        Expr::Unary { op: crate::java_ir::UnaryOp::PreInc
                            | crate::java_ir::UnaryOp::PostInc
                            | crate::java_ir::UnaryOp::PreDec
                            | crate::java_ir::UnaryOp::PostDec,
                            expr } if matches!(expr.as_ref(), Expr::Var(_)))
                        || matches!(update,
                            Expr::Binary { op: crate::java_ir::BinaryOp::Assign, left, right }
                            if matches!(left.as_ref(), Expr::Var(_))
                                && anonymous_source_class_super_return_is_supported(right, field_names))
                })
                && anonymous_source_class_local_statement_body_is_supported(
                    &body.statements,
                    field_names,
                    true,
                    &loop_break_labels,
                    &loop_continue_labels,
                    params,
                    &mut loop_aliases,
                )
        }
        Stmt::EnhancedFor {
            var_type,
            iterable: Expr::Var(name),
            body,
            label,
            ..
        } => {
            let Some(JavaType::Array(element)) = params.get(name) else {
                return false;
            };
            if element.as_ref() != var_type || !matches!(var_type, JavaType::Primitive(_)) {
                return false;
            }
            let mut loop_break_labels = active_break_labels.clone();
            let mut loop_continue_labels = active_continue_labels.clone();
            if let Some(label) = label {
                loop_break_labels.insert(label.clone());
                loop_continue_labels.insert(label.clone());
            }
            anonymous_source_class_local_statement_body_is_supported(
                &body.statements,
                field_names,
                true,
                &loop_break_labels,
                &loop_continue_labels,
                params,
                &mut aliases.clone(),
            )
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.is_empty()
                && !catches.is_empty()
                && finally_block.is_none()
                && anonymous_source_class_local_statement_body_is_supported(
                    &body.statements,
                    field_names,
                    may_continue,
                    active_break_labels,
                    active_continue_labels,
                    params,
                    &mut aliases.clone(),
                )
                && catches.iter().all(|catch| {
                    anonymous_source_class_local_statement_body_is_supported(
                        &catch.body.statements,
                        field_names,
                        may_continue,
                        active_break_labels,
                        active_continue_labels,
                        params,
                        &mut aliases.clone(),
                    )
                })
        }
        _ => false,
    };
        if admitted {
            match statement {
                Stmt::VarDecl {
                    name,
                    source_name,
                    ty: JavaType::Named { .. },
                    initializer: Some(Expr::FieldAccess(receiver, field)),
                    ..
                } if matches!(receiver.as_ref(), Expr::This)
                    && field_names
                        .iter()
                        .any(|candidate| candidate == &crate::parser::to_snake_case(field)) =>
                {
                    aliases.insert(name.clone());
                    aliases.insert(source_name.clone());
                }
                Stmt::VarDecl {
                    name, source_name, ..
                } => {
                    aliases.remove(name);
                    aliases.remove(source_name);
                }
                Stmt::Assign {
                    target: Expr::Var(name),
                    ..
                } => {
                    aliases.remove(name);
                }
                Stmt::Expr(Expr::Binary {
                    op: crate::java_ir::BinaryOp::Assign,
                    left,
                    ..
                }) if let Expr::Var(name) = left.as_ref() => {
                    aliases.remove(name);
                }
                Stmt::Try { .. } => {
                    // A path inside the try or one catch may reassign a
                    // reference local. Forget field-alias provenance at the
                    // join rather than accepting a later member write based
                    // on a path-sensitive fact this bounded checker does not
                    // retain.
                    aliases.clear();
                }
                _ => {}
            }
        }
        admitted
    })
}

fn anonymous_source_class_alias_write_value_is_supported(
    expr: &Expr,
    aliases: &HashSet<String>,
    params: &HashMap<String, JavaType>,
) -> bool {
    match expr {
        Expr::Literal(_) => true,
        Expr::Var(name) => aliases.contains(name) || params.contains_key(name),
        Expr::FieldAccess(receiver, _) => {
            matches!(receiver.as_ref(), Expr::Var(name) if aliases.contains(name))
        }
        Expr::Binary { left, right, .. } => {
            anonymous_source_class_alias_write_value_is_supported(left, aliases, params)
                && anonymous_source_class_alias_write_value_is_supported(right, aliases, params)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            anonymous_source_class_alias_write_value_is_supported(expr, aliases, params)
        }
        _ => false,
    }
}

fn anonymous_source_class_primitive_array_access_is_supported(
    expr: &Expr,
    params: &HashMap<String, JavaType>,
) -> bool {
    let Expr::ArrayAccess(array, index) = expr else {
        return false;
    };
    let Expr::Var(name) = array.as_ref() else {
        return false;
    };
    matches!(params.get(name), Some(JavaType::Array(element)) if matches!(element.as_ref(), JavaType::Primitive(_)))
        && matches!(
            index.as_ref(),
            Expr::Literal(crate::java_ir::Literal::Int(_))
        )
}

fn anonymous_source_class_guard_condition_is_supported(expr: &Expr) -> bool {
    matches!(expr, Expr::Var(_) | Expr::Literal(_))
}

/// The first mutable class-carrier tier rewrites only primitive arithmetic and
/// direct adapter-field reads. Keep eligibility aligned with that renderer:
/// admitting a `super` or arbitrary receiver call here would suppress the
/// diagnostic even though no composed-base rewrite exists for assignment RHSs.
fn anonymous_source_class_mutable_assignment_value_is_supported(
    expr: &Expr,
    field_names: &[String],
) -> bool {
    match expr {
        Expr::Literal(_) | Expr::Var(_) => true,
        Expr::FieldAccess(receiver, field) => {
            matches!(receiver.as_ref(), Expr::This)
                && field_names
                    .iter()
                    .any(|candidate| candidate == &crate::parser::to_snake_case(field))
        }
        Expr::Binary { op, left, right } => {
            matches!(
                op,
                crate::java_ir::BinaryOp::Add
                    | crate::java_ir::BinaryOp::Sub
                    | crate::java_ir::BinaryOp::Mul
                    | crate::java_ir::BinaryOp::Div
                    | crate::java_ir::BinaryOp::Rem
            ) && anonymous_source_class_mutable_assignment_value_is_supported(left, field_names)
                && anonymous_source_class_mutable_assignment_value_is_supported(right, field_names)
        }
        _ => false,
    }
}

fn anonymous_source_class_super_return_is_supported(expr: &Expr, field_names: &[String]) -> bool {
    if let Expr::Binary {
        op:
            crate::java_ir::BinaryOp::Add
            | crate::java_ir::BinaryOp::Sub
            | crate::java_ir::BinaryOp::Mul
            | crate::java_ir::BinaryOp::Div
            | crate::java_ir::BinaryOp::Rem,
        left,
        right,
    } = expr
    {
        return anonymous_source_class_super_return_is_supported(left, field_names)
            && anonymous_source_class_super_return_is_supported(right, field_names);
    }
    if let Expr::Unary {
        op:
            crate::java_ir::UnaryOp::Not | crate::java_ir::UnaryOp::Neg | crate::java_ir::UnaryOp::Pos,
        expr,
    } = expr
    {
        return anonymous_source_class_super_return_is_supported(expr, field_names);
    }
    if matches!(expr, Expr::Var(_)) {
        // A lexical outer value is materialized as an adapter field by class
        // anonymous lowering; method parameters use the same direct path.
        return true;
    }
    if matches!(expr, Expr::New { body: None, .. }) {
        // The adapter assignment lowerer routes the construction into the
        // destination field's storage slot, where shared-handle construction
        // is decided.  It is therefore safe to admit a plain named `new`.
        return true;
    }
    if matches!(expr, Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::Var(_))) {
        return true;
    }
    if matches!(expr, Expr::Call { receiver: Some(receiver), .. } if matches!(receiver.as_ref(), Expr::Var(_)))
    {
        return true;
    }
    if matches!(expr, Expr::Call { receiver: Some(receiver), .. } if matches!(receiver.as_ref(), Expr::Super))
    {
        return true;
    }
    if matches!(expr, Expr::Call { receiver: Some(receiver), .. } if matches!(receiver.as_ref(), Expr::This))
    {
        return true;
    }
    if matches!(expr, Expr::Call { receiver: Some(receiver), .. }
        if matches!(receiver.as_ref(), Expr::FieldAccess(this, field)
            if matches!(this.as_ref(), Expr::This)
                && field_names.iter().any(|candidate| candidate == &crate::parser::to_snake_case(field))))
    {
        // The adapter field itself is rendered through its storage slot and
        // the receiver lowering path supplies any shared-handle borrow.
        return true;
    }
    if matches!(expr, Expr::FieldAccess(receiver, field)
        if matches!(receiver.as_ref(), Expr::This)
            && field_names.iter().any(|candidate| candidate == &crate::parser::to_snake_case(field)))
    {
        return true;
    }
    if matches!(expr, Expr::FieldAccess(receiver, _)
        if matches!(receiver.as_ref(), Expr::FieldAccess(this, field)
            if matches!(this.as_ref(), Expr::This)
                && field_names.iter().any(|candidate| candidate == &crate::parser::to_snake_case(field))))
    {
        return true;
    }
    anonymous_source_class_mutable_assignment_value_is_supported(expr, field_names)
        || matches!(expr, Expr::Binary { op: crate::java_ir::BinaryOp::Eq
            | crate::java_ir::BinaryOp::Ne
            | crate::java_ir::BinaryOp::Lt
            | crate::java_ir::BinaryOp::Le
            | crate::java_ir::BinaryOp::Gt
            | crate::java_ir::BinaryOp::Ge
            | crate::java_ir::BinaryOp::And
            | crate::java_ir::BinaryOp::Or
            | crate::java_ir::BinaryOp::BitAnd
            | crate::java_ir::BinaryOp::BitOr
            | crate::java_ir::BinaryOp::BitXor
            | crate::java_ir::BinaryOp::Shl
            | crate::java_ir::BinaryOp::Shr
            | crate::java_ir::BinaryOp::UShr, left, right }
            if anonymous_source_class_super_return_is_supported(left, field_names)
                && anonymous_source_class_super_return_is_supported(right, field_names))
}

/// A non-functional source interface still needs an object, not a closure. The
/// first general tier has no anonymous instance fields, but implements every
/// abstract interface method with an ordinary body that does not require an
/// anonymous `this` or `super` receiver.
fn anonymous_fieldless_interface_object_is_supported(
    ty: &JavaType,
    args: &[Expr],
    body: Option<&[Member]>,
    anonymous_context: &Program,
) -> bool {
    if !args.is_empty() {
        return false;
    }
    let JavaType::Named { name, .. } = ty else {
        return false;
    };
    let Some(interface) = find_type_by_erased_name(anonymous_context, name) else {
        return false;
    };
    if interface.kind != TypeKind::Interface {
        return false;
    }
    // Use the same canonical, specialized contracts as adapter lowering.  A
    // raw lineage walk leaves `T` unsubstituted in contracts such as
    // `Relay<String> extends Source<String>` and retains duplicate inherited
    // declarations.  That made an otherwise supported adapter look as though
    // it had omitted `echo(Object)` or supplied an extra helper method.
    let expected = crate::representation_lowering::specialized_interface_abstract_contracts(
        anonymous_context,
        interface,
    );
    if expected.is_empty() {
        return false;
    }
    let Some(body) = body else {
        return false;
    };
    let fields = body
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    if fields.iter().any(|field| field.initializer.is_none())
        || (expected.len() == 1
            && interface
                .members
                .iter()
                .filter(|member| matches!(member, Member::Method(method) if method.body.is_none()))
                .count()
                == 1
            && fields.is_empty()
            && body
                .iter()
                .filter(|member| matches!(member, Member::Method(_)))
                .count()
                == 1)
    {
        return false;
    }
    let implementations = body
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .collect::<Vec<_>>();
    expected.iter().all(|expected_contract| {
            implementations.iter().any(|implementation| {
                implementation.name == expected_contract.method.name
                    && implementation.params.len() == expected_contract.param_types.len()
                    && implementation.params.iter().zip(&expected_contract.param_types).all(
                        |(implementation_param, expected_ty)| {
                            crate::java_signature::erased_type_name(
                                &implementation_param.ty,
                                implementation_param.is_varargs,
                            ) == crate::java_signature::erased_type_name(
                                expected_ty,
                                implementation_param.is_varargs,
                            )
                        },
                    )
                    && implementation.type_params.is_empty()
                    && !implementation.modifiers.contains(&Modifier::Static)
                    && !fields.iter().any(|field| {
                        implementation
                            .params
                            .iter()
                            .any(|param| param.name == field.name)
                    })
                    && implementation.body.as_ref().is_some_and(|body| {
                        if fields.is_empty() {
                            stateful_anonymous_read_only_block_is_supported(body)
                        } else {
                            stateful_anonymous_read_only_block_is_supported(body)
                                || stateful_anonymous_counter_mutator_body(
                                    body,
                                    &expected_contract.return_type,
                                    &fields,
                                )
                                || stateful_anonymous_sequential_mutator_body(
                                    body,
                                    &expected_contract.return_type,
                                    &fields,
                                )
                                .is_some()
                                || stateful_anonymous_receiver_statement_body_is_supported(
                                    body,
                                    &expected_contract.return_type,
                                    &fields,
                                    &implementation.params,
                                )
                                || stateful_anonymous_guarded_counter_mutator_body(
                                    body,
                                    &expected_contract.return_type,
                                    &fields,
                                )
                                || stateful_anonymous_reference_member_write_body(
                                    body,
                                    &expected_contract.return_type,
                                    &fields,
                                )
                        }
                    })
            })
        })
        && implementations
            .iter()
            .filter(|method| {
                !expected.iter().any(|expected_contract| {
                    method.name == expected_contract.method.name
                        && method.params.len() == expected_contract.param_types.len()
                        && method.params.iter().zip(&expected_contract.param_types).all(
                            |(implementation_param, expected_ty)| {
                                crate::java_signature::erased_type_name(
                                    &implementation_param.ty,
                                    implementation_param.is_varargs,
                                ) == crate::java_signature::erased_type_name(
                                    expected_ty,
                                    implementation_param.is_varargs,
                                )
                            },
                        )
                })
            })
            .all(|helper| {
                helper
                    .params
                    .iter()
                    .all(|param| matches!(param.ty, JavaType::Primitive(_)))
                    && helper.type_params.is_empty()
                    && !helper.modifiers.contains(&Modifier::Static)
                    && matches!(helper.return_type, JavaType::Primitive(_))
                && helper.body.as_ref().is_some_and(|body| {
                    matches!(body.statements.as_slice(), [Stmt::Return(Some(expr))]
                        if !stateful_anonymous_return_expr_has_super(expr))
                        || matches!(body.statements.as_slice(), [
                            Stmt::Assign { target: Expr::FieldAccess(receiver, assigned), .. },
                            Stmt::Return(Some(returned)),
                        ] if matches!(receiver.as_ref(), Expr::This)
                            && fields.iter().any(|field| field.name == crate::parser::to_snake_case_unescaped(assigned)
                                && matches!(field.ty, JavaType::Primitive(_))
                                && stateful_anonymous_counter_return_reads_field(returned, &field.name)))
                })
            })
}

/// A direct write through a named anonymous-object reference field is safe to
/// route through ordinary lowering: the field's slot shape supplies the
/// shared-handle receiver projection (`borrow_mut` when required).  Keep the
/// capability deliberately exact until the adapter has a general statement IR.
fn stateful_anonymous_reference_member_write_body(
    body: &Block,
    return_type: &JavaType,
    fields: &[&crate::java_ir::FieldDecl],
) -> bool {
    matches!(
        body.statements.as_slice(),
        [Stmt::Assign {
            target: Expr::FieldAccess(receiver, _),
            ..
        }]
            if matches!(return_type, JavaType::Void)
                && matches!(receiver.as_ref(), Expr::FieldAccess(owner, field)
                    if matches!(owner.as_ref(), Expr::This)
                        && fields.iter().any(|candidate| {
                            candidate.name == crate::parser::to_snake_case_unescaped(field)
                                && matches!(candidate.ty, JavaType::Named { .. })
                        }))
    )
}

/// A guarded primitive update is still ordinary anonymous-instance state: the
/// condition is read through the adapter and the assignment updates its `Cell`
/// only when that condition holds. Keep this deliberately narrower than a
/// general mutable control-flow grammar: each present branch must update the
/// same field. A following return is accepted only when it returns that field.
fn stateful_anonymous_guarded_counter_mutator_body(
    body: &Block,
    return_type: &JavaType,
    fields: &[&crate::java_ir::FieldDecl],
) -> bool {
    let (condition, then_block, else_block, returned) = match body.statements.as_slice() {
        [
            Stmt::If {
                condition,
                then_block,
                else_block,
            },
        ] => (condition, then_block, else_block, None),
        [
            Stmt::If {
                condition,
                then_block,
                else_block,
            },
            Stmt::Return(Some(returned)),
        ] => (condition, then_block, else_block, Some(returned)),
        _ => return false,
    };
    let [
        Stmt::Assign {
            target: Expr::FieldAccess(receiver, assigned),
            ..
        },
    ] = then_block.statements.as_slice()
    else {
        return false;
    };
    let else_assigned = else_block.as_ref().map(|else_block| {
        let [
            Stmt::Assign {
                target: Expr::FieldAccess(receiver, assigned),
                ..
            },
        ] = else_block.statements.as_slice()
        else {
            return None;
        };
        matches!(receiver.as_ref(), Expr::This)
            .then(|| crate::parser::to_snake_case_unescaped(assigned))
    });
    !stateful_anonymous_return_expr_has_super(condition)
        && matches!(receiver.as_ref(), Expr::This)
        && else_assigned.as_ref().is_none_or(|assigned_else| {
            assigned_else.as_deref()
                == Some(crate::parser::to_snake_case_unescaped(assigned).as_str())
        })
        && fields.iter().any(|field| {
            field.name == crate::parser::to_snake_case_unescaped(assigned)
                && (matches!(field.ty, JavaType::Primitive(_) | JavaType::String)
                    || matches!(&field.ty, JavaType::Named { name, .. }
                        if name.rsplit('.').next() == Some("String")))
                && match returned {
                    None => *return_type == JavaType::Void,
                    Some(returned) => {
                        anonymous_mutable_field_return_type_matches(return_type, &field.ty)
                            && stateful_anonymous_counter_return_reads_field(returned, &field.name)
                    }
                }
        })
}

/// A narrow mutable anonymous-object method: update one `int` instance field,
/// optionally returning that same field. The latter is the Java idiom behind
/// `int add(int n) { this.total += n; return total; }`.
fn stateful_anonymous_counter_mutator_body(
    body: &Block,
    return_type: &JavaType,
    fields: &[&crate::java_ir::FieldDecl],
) -> bool {
    let (assigned, returned) = match body.statements.as_slice() {
        [
            Stmt::Assign {
                target: Expr::FieldAccess(receiver, assigned),
                ..
            },
        ] if matches!(receiver.as_ref(), Expr::This) => (assigned, None),
        [
            Stmt::Assign {
                target: Expr::FieldAccess(receiver, assigned),
                ..
            },
            Stmt::Return(Some(returned)),
        ] if matches!(receiver.as_ref(), Expr::This) => (assigned, Some(returned)),
        [
            prefix @ ..,
            Stmt::Assign {
                target: Expr::FieldAccess(receiver, assigned),
                ..
            },
            Stmt::Return(Some(returned)),
        ] if !prefix.is_empty()
            && stateful_anonymous_local_prefix_is_supported(prefix)
            && matches!(receiver.as_ref(), Expr::This) =>
        {
            (assigned, Some(returned))
        }
        _ => return false,
    };
    fields.iter().any(|field| {
        field.name == crate::parser::to_snake_case_unescaped(assigned)
            && (matches!(field.ty, JavaType::Primitive(_) | JavaType::String)
                || matches!(&field.ty, JavaType::Named { name, .. }
                    if name.rsplit('.').next() == Some("String")))
            && match returned {
                None => *return_type == JavaType::Void,
                Some(returned) => {
                    anonymous_mutable_field_return_type_matches(return_type, &field.ty)
                        && !stateful_anonymous_return_expr_has_super(returned)
                }
            }
    })
}

/// Ordered receiver-field writes separated by ordinary local statements. The
/// existing counter shortcut owns the direct single-write forms; this path
/// admits post-write locals and multiple top-level writes while still refusing
/// nested writes until the general receiver-aware statement pass handles them.
pub(crate) fn stateful_anonymous_sequential_mutator_body(
    body: &Block,
    return_type: &JavaType,
    fields: &[&crate::java_ir::FieldDecl],
) -> Option<Vec<(usize, String)>> {
    let statements = &body.statements;
    let (last, _) = statements.split_last()?;
    let normal_end = if matches!(return_type, JavaType::Void) {
        if matches!(last, Stmt::Return(None)) {
            statements.len() - 1
        } else {
            statements.len()
        }
    } else if matches!(last, Stmt::Return(Some(expr)) if !stateful_anonymous_return_expr_has_super(expr))
    {
        statements.len() - 1
    } else {
        return None;
    };
    let mut writes = Vec::new();
    let mut segment_start = 0;
    for (index, stmt) in statements[..normal_end].iter().enumerate() {
        let Stmt::Assign {
            target: Expr::FieldAccess(receiver, assigned),
            value,
        } = stmt
        else {
            continue;
        };
        if !matches!(receiver.as_ref(), Expr::This)
            || stateful_anonymous_return_expr_has_super(value)
            || !stateful_anonymous_local_prefix_is_supported(&statements[segment_start..index])
        {
            return None;
        }
        let field_name = crate::parser::to_snake_case_unescaped(assigned);
        if !fields.iter().any(|field| {
            field.name == field_name
                && (matches!(field.ty, JavaType::Primitive(_) | JavaType::String)
                    || matches!(&field.ty, JavaType::Named { name, .. }
                        if name.rsplit('.').next() == Some("String")))
        }) {
            return None;
        }
        writes.push((index, field_name));
        segment_start = index + 1;
    }
    if writes.is_empty()
        || (writes.len() == 1 && normal_end == segment_start)
        || !stateful_anonymous_read_only_block_is_supported(&Block {
            statements: statements[segment_start..normal_end].to_vec(),
        })
    {
        return None;
    }
    Some(writes)
}

/// Receiver-aware statement lowering for mutable interface adapters. This is
/// intentionally a capability check over the same recursive grammar consumed
/// by the adapter renderer, rather than another exact counter-body pattern.
/// It admits branch and loop field writes, including labels on supported
/// loops, but not arbitrary expression statements.
pub(crate) fn stateful_anonymous_receiver_statement_body_is_supported(
    body: &Block,
    return_type: &JavaType,
    fields: &[&crate::java_ir::FieldDecl],
    method_params: &[crate::java_ir::Param],
) -> bool {
    fn supported(
        block: &Block,
        fields: &[&crate::java_ir::FieldDecl],
        params: &HashMap<String, JavaType>,
        in_loop: bool,
        active_break_labels: &HashSet<String>,
        active_continue_labels: &HashSet<String>,
        inherited_aliases: &HashSet<String>,
        writes: &mut usize,
    ) -> bool {
        let mut aliases = inherited_aliases.clone();
        block.statements.iter().all(|stmt| {
            let admitted = match stmt {
                Stmt::VarDecl {
                    ty: JavaType::Primitive(_) | JavaType::String,
                    initializer: Some(expr),
                    ..
                }
                | Stmt::Assign {
                    target: Expr::Var(_),
                    value: expr,
                }
                | Stmt::Return(Some(expr)) => !stateful_anonymous_return_expr_has_super(expr),
                Stmt::Throw(Expr::New {
                    args, body: None, ..
                }) => args
                    .iter()
                    .all(|arg| !stateful_anonymous_return_expr_has_super(arg)),
                Stmt::Throw(expr) => !stateful_anonymous_return_expr_has_super(expr),
                Stmt::VarDecl {
                    ty: ty @ JavaType::Named { .. },
                    initializer: Some(Expr::FieldAccess(receiver, name)),
                    ..
                } if matches!(receiver.as_ref(), Expr::This)
                    && fields.iter().any(|field| {
                        field.name == crate::parser::to_snake_case_unescaped(name)
                            && field.ty == *ty
                    }) =>
                {
                    true
                }
                Stmt::Expr(Expr::Binary {
                    op: crate::java_ir::BinaryOp::Assign,
                    left,
                    right,
                }) if matches!(left.as_ref(), Expr::Var(_)) => {
                    !stateful_anonymous_return_expr_has_super(right)
                }
                Stmt::Expr(expr @ Expr::Call { .. }) => {
                    !stateful_anonymous_return_expr_has_super(expr)
                }
                Stmt::Expr(Expr::Unary {
                    op:
                        crate::java_ir::UnaryOp::PreInc
                        | crate::java_ir::UnaryOp::PostInc
                        | crate::java_ir::UnaryOp::PreDec
                        | crate::java_ir::UnaryOp::PostDec,
                    expr,
                }) if matches!(expr.as_ref(), Expr::FieldAccess(receiver, field)
                if matches!(receiver.as_ref(), Expr::This)
                    && stateful_anonymous_incrementable_cell_field(field, fields)) =>
                {
                    *writes += 1;
                    true
                }
                Stmt::Expr(Expr::Unary {
                    op:
                        crate::java_ir::UnaryOp::PreInc
                        | crate::java_ir::UnaryOp::PostInc
                        | crate::java_ir::UnaryOp::PreDec
                        | crate::java_ir::UnaryOp::PostDec,
                    expr,
                }) if matches!(expr.as_ref(), Expr::Var(_)) => true,
                Stmt::Return(None) => true,
                Stmt::Assign {
                    target: Expr::FieldAccess(receiver, _),
                    value,
                } if matches!(receiver.as_ref(), Expr::Var(name) if aliases.contains(name))
                    && !stateful_anonymous_return_expr_has_super(value) =>
                {
                    *writes += 1;
                    true
                }
                Stmt::Assign {
                    target: Expr::FieldAccess(receiver, assigned),
                    value,
                } if matches!(receiver.as_ref(), Expr::This)
                    && !stateful_anonymous_return_expr_has_super(value)
                    && fields.iter().any(|field| {
                        field.name == crate::parser::to_snake_case_unescaped(assigned)
                            && matches!(
                                field.ty,
                                JavaType::Primitive(_) | JavaType::String | JavaType::Named { .. }
                            )
                    }) =>
                {
                    *writes += 1;
                    true
                }
                Stmt::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    !stateful_anonymous_return_expr_has_super(condition)
                        && supported(
                            then_block,
                            fields,
                            params,
                            in_loop,
                            active_break_labels,
                            active_continue_labels,
                            &aliases,
                            writes,
                        )
                        && else_block.as_ref().is_none_or(|else_block| {
                            supported(
                                else_block,
                                fields,
                                params,
                                in_loop,
                                active_break_labels,
                                active_continue_labels,
                                &aliases,
                                writes,
                            )
                        })
                }
                Stmt::Block(body) => supported(
                    body,
                    fields,
                    params,
                    in_loop,
                    active_break_labels,
                    active_continue_labels,
                    &aliases,
                    writes,
                ),
                Stmt::LabeledBlock { label, body } => {
                    let mut block_break_labels = active_break_labels.clone();
                    block_break_labels.insert(label.clone());
                    supported(
                        body,
                        fields,
                        params,
                        in_loop,
                        &block_break_labels,
                        active_continue_labels,
                        &aliases,
                        writes,
                    )
                }
                Stmt::While {
                    condition,
                    body,
                    label,
                } => {
                    let mut loop_break_labels = active_break_labels.clone();
                    let mut loop_continue_labels = active_continue_labels.clone();
                    if let Some(label) = label {
                        loop_break_labels.insert(label.clone());
                        loop_continue_labels.insert(label.clone());
                    }
                    !stateful_anonymous_return_expr_has_super(condition)
                        && supported(
                            body,
                            fields,
                            params,
                            true,
                            &loop_break_labels,
                            &loop_continue_labels,
                            &aliases,
                            writes,
                        )
                }
                Stmt::DoWhile {
                    body,
                    condition,
                    label,
                } => {
                    let mut loop_break_labels = active_break_labels.clone();
                    let mut loop_continue_labels = active_continue_labels.clone();
                    if let Some(label) = label {
                        loop_break_labels.insert(label.clone());
                        loop_continue_labels.insert(label.clone());
                    }
                    !stateful_anonymous_return_expr_has_super(condition)
                        && supported(
                            body,
                            fields,
                            params,
                            true,
                            &loop_break_labels,
                            &loop_continue_labels,
                            &aliases,
                            writes,
                        )
                }
                Stmt::For {
                    init,
                    condition,
                    update,
                    body,
                    label,
                } => {
                    let mut loop_break_labels = active_break_labels.clone();
                    let mut loop_continue_labels = active_continue_labels.clone();
                    if let Some(label) = label {
                        loop_break_labels.insert(label.clone());
                        loop_continue_labels.insert(label.clone());
                    }
                    let init_block = Block {
                        statements: init.clone(),
                    };
                    supported(
                        &init_block,
                        fields,
                        params,
                        false,
                        active_break_labels,
                        active_continue_labels,
                        &aliases,
                        writes,
                    ) && condition.as_ref().is_none_or(|condition| {
                        !stateful_anonymous_return_expr_has_super(condition)
                    }) && update.iter().all(|expr| match expr {
                        Expr::Unary {
                            op:
                                crate::java_ir::UnaryOp::PreInc
                                | crate::java_ir::UnaryOp::PostInc
                                | crate::java_ir::UnaryOp::PreDec
                                | crate::java_ir::UnaryOp::PostDec,
                            expr,
                        } => matches!(expr.as_ref(), Expr::Var(_)),
                        Expr::Binary {
                            op: crate::java_ir::BinaryOp::Assign,
                            left,
                            right,
                        } => {
                            matches!(left.as_ref(), Expr::Var(_))
                                && !stateful_anonymous_return_expr_has_super(right)
                        }
                        _ => false,
                    }) && supported(
                        body,
                        fields,
                        params,
                        true,
                        &loop_break_labels,
                        &loop_continue_labels,
                        &aliases,
                        writes,
                    )
                }
                Stmt::EnhancedFor {
                    var_type,
                    var_name,
                    source_var_name,
                    iterable: Expr::Var(iterable),
                    body,
                    label,
                    ..
                } if matches!(var_type, JavaType::Primitive(_))
                    && matches!(params.get(iterable), Some(JavaType::Array(element))
                        if element.as_ref() == var_type) =>
                {
                    let mut loop_break_labels = active_break_labels.clone();
                    let mut loop_continue_labels = active_continue_labels.clone();
                    if let Some(label) = label {
                        loop_break_labels.insert(label.clone());
                        loop_continue_labels.insert(label.clone());
                    }
                    let mut loop_aliases = aliases.clone();
                    loop_aliases.remove(var_name);
                    loop_aliases.remove(source_var_name);
                    supported(
                        body,
                        fields,
                        params,
                        true,
                        &loop_break_labels,
                        &loop_continue_labels,
                        &loop_aliases,
                        writes,
                    )
                }
                Stmt::Break(None) | Stmt::Continue(None) => in_loop,
                Stmt::Break(Some(label)) => active_break_labels.contains(label),
                Stmt::Continue(Some(label)) => active_continue_labels.contains(label),
                _ => false,
            };
            if admitted {
                match stmt {
                    Stmt::VarDecl {
                        ty: JavaType::Named { .. },
                        name,
                        source_name,
                        initializer: Some(Expr::FieldAccess(receiver, _)),
                        ..
                    } if matches!(receiver.as_ref(), Expr::This) => {
                        aliases.insert(name.clone());
                        aliases.insert(source_name.clone());
                    }
                    Stmt::VarDecl {
                        name, source_name, ..
                    } => {
                        aliases.remove(name);
                        aliases.remove(source_name);
                    }
                    Stmt::Assign {
                        target: Expr::Var(name),
                        ..
                    } => {
                        aliases.remove(name);
                    }
                    Stmt::Expr(Expr::Binary {
                        op: crate::java_ir::BinaryOp::Assign,
                        left,
                        ..
                    }) if let Expr::Var(name) = left.as_ref() => {
                        aliases.remove(name);
                    }
                    _ => {}
                }
            }
            admitted
        })
    }

    if matches!(return_type, JavaType::Void) {
        if matches!(body.statements.last(), Some(Stmt::Return(Some(_)))) {
            return false;
        }
    } else if !matches!(body.statements.last(), Some(Stmt::Return(Some(_)))) {
        return false;
    }
    let mut writes = 0;
    let params = method_params
        .iter()
        .flat_map(|param| {
            [
                (param.name.clone(), param.ty.clone()),
                (param.source_name.clone(), param.ty.clone()),
            ]
        })
        .collect::<HashMap<_, _>>();
    supported(
        body,
        fields,
        &params,
        false,
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &mut writes,
    ) && writes > 0
}

pub(crate) fn stateful_anonymous_incrementable_cell_field(
    name: &str,
    fields: &[&crate::java_ir::FieldDecl],
) -> bool {
    let name = crate::parser::to_snake_case_unescaped(name);
    fields.iter().any(|field| {
        field.name == name
            && matches!(
                field.ty,
                JavaType::Primitive(
                    crate::java_ir::PrimitiveType::Byte
                        | crate::java_ir::PrimitiveType::Short
                        | crate::java_ir::PrimitiveType::Int
                        | crate::java_ir::PrimitiveType::Long
                )
            )
    })
}

/// Local statements allowed before the single receiver-field write in the
/// bounded mutable anonymous-object tier.
fn stateful_anonymous_local_prefix_is_supported(prefix: &[Stmt]) -> bool {
    prefix.iter().all(|stmt| match stmt {
        Stmt::VarDecl {
            ty: JavaType::Primitive(_) | JavaType::String,
            initializer: Some(initializer),
            ..
        } => !stateful_anonymous_return_expr_has_super(initializer),
        Stmt::Assign {
            target: Expr::Var(_),
            value,
        } => !stateful_anonymous_return_expr_has_super(value),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            !stateful_anonymous_return_expr_has_super(condition)
                && stateful_anonymous_local_prefix_is_supported(&then_block.statements)
                && else_block.as_ref().is_none_or(|block| {
                    stateful_anonymous_local_prefix_is_supported(&block.statements)
                })
        }
        _ => false,
    })
}

fn anonymous_mutable_field_return_type_matches(
    return_type: &JavaType,
    field_type: &JavaType,
) -> bool {
    return_type == field_type
        || matches!(return_type, JavaType::String)
            && matches!(field_type, JavaType::Named { name, .. }
                if name.rsplit('.').next() == Some("String"))
        || matches!(field_type, JavaType::String)
            && matches!(return_type, JavaType::Named { name, .. }
                if name.rsplit('.').next() == Some("String"))
}

fn stateful_anonymous_read_only_block_is_supported(body: &Block) -> bool {
    fn supported(body: &Block, locals: &mut BTreeSet<String>, in_loop: bool) -> bool {
        for stmt in &body.statements {
            match stmt {
                Stmt::VarDecl {
                    name,
                    source_name,
                    initializer: Some(expr),
                    ..
                } if !stateful_anonymous_return_expr_has_super(expr) => {
                    locals.insert(name.clone());
                    locals.insert(source_name.clone());
                }
                Stmt::Return(Some(expr)) if !stateful_anonymous_return_expr_has_super(expr) => {}
                Stmt::Throw(Expr::New {
                    args, body: None, ..
                }) if args
                    .iter()
                    .all(|arg| !stateful_anonymous_return_expr_has_super(arg)) => {}
                Stmt::Throw(expr) if !stateful_anonymous_return_expr_has_super(expr) => {}
                Stmt::Expr(expr @ Expr::Call { .. })
                    if !stateful_anonymous_return_expr_has_super(expr) => {}
                Stmt::Assign {
                    target: Expr::Var(name),
                    value,
                } if locals.contains(name) && !stateful_anonymous_return_expr_has_super(value) => {}
                Stmt::If {
                    condition,
                    then_block,
                    else_block,
                } if !stateful_anonymous_return_expr_has_super(condition)
                    && supported(then_block, &mut locals.clone(), in_loop)
                    && else_block
                        .as_ref()
                        .is_none_or(|block| supported(block, &mut locals.clone(), in_loop)) => {}
                Stmt::While {
                    condition,
                    body,
                    label: None,
                }
                | Stmt::DoWhile {
                    condition,
                    body,
                    label: None,
                } if !stateful_anonymous_return_expr_has_super(condition)
                    && supported(body, &mut locals.clone(), true) => {}
                Stmt::For {
                    init,
                    condition,
                    update,
                    body,
                    label: None,
                } => {
                    let mut loop_locals = locals.clone();
                    let init_block = Block {
                        statements: init.clone(),
                    };
                    if !supported(&init_block, &mut loop_locals, false)
                        || condition.as_ref().is_some_and(stateful_anonymous_return_expr_has_super)
                        || !update.iter().all(|expr| match expr {
                            Expr::Unary { op: crate::java_ir::UnaryOp::PreInc
                                | crate::java_ir::UnaryOp::PostInc
                                | crate::java_ir::UnaryOp::PreDec
                                | crate::java_ir::UnaryOp::PostDec, expr }
                                if matches!(expr.as_ref(), Expr::Var(name) if loop_locals.contains(name)) => true,
                            Expr::Binary { op: crate::java_ir::BinaryOp::Assign, left, right }
                                if matches!(left.as_ref(), Expr::Var(name) if loop_locals.contains(name))
                                    && !stateful_anonymous_return_expr_has_super(right) => true,
                            _ => false,
                        })
                        || !supported(body, &mut loop_locals, true)
                    {
                        return false;
                    }
                }
                Stmt::Break(None) | Stmt::Continue(None) if in_loop => {}
                Stmt::Try {
                    resources,
                    body,
                    catches,
                    finally_block,
                } if resources.is_empty()
                    && !catches.is_empty()
                    && finally_block.is_none()
                    && supported(body, &mut locals.clone(), in_loop)
                    && catches
                        .iter()
                        .all(|catch| supported(&catch.body, &mut locals.clone(), in_loop)) => {}
                _ => return false,
            }
        }
        true
    }
    supported(body, &mut BTreeSet::new(), false)
}

fn anonymous_functional_interface_is_supported(
    ty: &JavaType,
    args: &[Expr],
    body: Option<&[Member]>,
    anonymous_context: &Program,
) -> bool {
    if !args.is_empty() {
        return false;
    }
    let JavaType::Named { name, .. } = ty else {
        return false;
    };
    let Some(interface) = find_type_by_erased_name(anonymous_context, name) else {
        return false;
    };
    if interface.kind != TypeKind::Interface || interface.extends.is_some() {
        return false;
    }
    let mut methods = interface.members.iter().filter_map(|member| match member {
        Member::Method(method) if method.body.is_none() => Some(method),
        _ => None,
    });
    let Some(interface_method) = methods.next() else {
        return false;
    };
    if methods.next().is_some()
        || !interface_method.type_params.is_empty()
        || !interface_method.throws.is_empty()
        || interface_method.modifiers.contains(&Modifier::Static)
    {
        return false;
    }
    let Some(body) = body else {
        return false;
    };
    let methods = body
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [implementation] = methods.as_slice() else {
        return false;
    };
    let stateful_field_slice = body
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    let stateful = !stateful_field_slice.is_empty()
        && body.len() == stateful_field_slice.len() + 1
        && stateful_field_slice
            .iter()
            .all(|field| field.initializer.is_some())
        && implementation.body.as_ref().is_some_and(|body| {
            matches!(body.statements.as_slice(), [Stmt::Return(Some(expr))]
                if !stateful_anonymous_return_expr_has_super(expr))
                || stateful_anonymous_counter_body(body, &stateful_field_slice)
        });
    if !stateful && body.len() != 1 {
        return false;
    }
    implementation.name == interface_method.name
        && implementation.params.len() == interface_method.params.len()
        && implementation.type_params.is_empty()
        && !implementation.modifiers.contains(&Modifier::Static)
        && !stateful_field_slice.iter().any(|field| {
            implementation
                .params
                .iter()
                .any(|param| param.name == field.name)
        })
        && implementation
            .body
            .as_ref()
            .is_some_and(|body| stateful || !crate::java_ir::block_mentions_this_or_super(body))
}

fn stateful_anonymous_counter_body(body: &Block, fields: &[&crate::java_ir::FieldDecl]) -> bool {
    matches!(body.statements.as_slice(), [
        Stmt::Assign { target: Expr::FieldAccess(receiver, assigned), .. },
        Stmt::Return(Some(returned_expr)),
    ] if matches!(receiver.as_ref(), Expr::This)
        && fields.iter().any(|field| field.name == crate::parser::to_snake_case_unescaped(assigned)
            && matches!(field.ty, JavaType::Primitive(crate::java_ir::PrimitiveType::Int))
            && stateful_anonymous_counter_return_reads_field(returned_expr, &field.name)))
}

fn stateful_anonymous_counter_return_reads_field(expr: &Expr, field: &str) -> bool {
    matches!(expr, Expr::Var(name) if name == field)
        || matches!(expr, Expr::FieldAccess(receiver, name)
            if matches!(receiver.as_ref(), Expr::This) && name == field)
}

/// The stateful anonymous adapter does not yet have a superclass. Treat nested
/// anonymous construction and lambdas conservatively as unsupported here; that
/// keeps diagnostics aligned with the lowering boundary.
fn stateful_anonymous_return_expr_has_super(expr: &Expr) -> bool {
    match expr {
        Expr::Super | Expr::Lambda { .. } => true,
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            body.is_some()
                || qualifier
                    .as_ref()
                    .is_some_and(|qualifier| stateful_anonymous_return_expr_has_super(qualifier))
                || args.iter().any(stateful_anonymous_return_expr_has_super)
        }
        Expr::This
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
        Expr::FieldAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            stateful_anonymous_return_expr_has_super(receiver)
        }
        Expr::ArrayAccess(array, index) => {
            stateful_anonymous_return_expr_has_super(array)
                || stateful_anonymous_return_expr_has_super(index)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(stateful_anonymous_return_expr_has_super)
                || initializer
                    .as_ref()
                    .is_some_and(|items| items.iter().any(stateful_anonymous_return_expr_has_super))
        }
        Expr::ArrayInitializer(items) => items.iter().any(stateful_anonymous_return_expr_has_super),
        Expr::Binary { left, right, .. } => {
            stateful_anonymous_return_expr_has_super(left)
                || stateful_anonymous_return_expr_has_super(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            stateful_anonymous_return_expr_has_super(condition)
                || stateful_anonymous_return_expr_has_super(then_expr)
                || stateful_anonymous_return_expr_has_super(else_expr)
        }
        Expr::Switch { selector, cases } => {
            stateful_anonymous_return_expr_has_super(selector)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| {
                        matches!(label, SwitchLabel::Expr(expr)
                        if stateful_anonymous_return_expr_has_super(expr))
                    }) || stateful_anonymous_return_expr_has_super(&case.value)
                })
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| stateful_anonymous_return_expr_has_super(receiver))
                || args.iter().any(stateful_anonymous_return_expr_has_super)
        }
    }
}

fn anonymous_priority_queue_comparator_constructor_is_supported(
    ty: &JavaType,
    args: &[Expr],
) -> bool {
    if !crate::stdlib::java_type_is_priority_queue(ty) {
        return false;
    }
    let [
        Expr::New {
            ty: comparator_ty,
            args: comparator_args,
            body: Some(body),
            ..
        },
    ] = args
    else {
        return false;
    };
    comparator_args.is_empty()
        && anonymous_comparator_type(comparator_ty)
        && anonymous_comparator_body_has_single_returning_compare(body)
}

fn anonymous_comparator_type(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::canonical_external_receiver_type_key(name)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Comparator")
        }
        _ => false,
    }
}

fn anonymous_comparator_body_has_single_returning_compare(body: &[Member]) -> bool {
    let [Member::Method(method)] = body else {
        return false;
    };
    method.name == "compare"
        && method.params.len() == 2
        && matches!(
            method.body.as_ref().map(|body| body.statements.as_slice()),
            Some([Stmt::Return(Some(_))])
        )
}

fn find_type_by_erased_name<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    let types = all_types(program);
    let exact = types
        .iter()
        .copied()
        .filter(|ty| ty.name == type_name)
        .collect::<Vec<_>>();
    if !exact.is_empty() {
        return unique_type_decl_match(exact.as_slice());
    }
    if type_name.contains('.') || type_name.contains('$') {
        return None;
    }
    let mut matches = types.into_iter().filter(|ty| {
        ty.name
            .rsplit(['.', '$'])
            .next()
            .is_some_and(|simple| simple == type_name)
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn unique_type_decl_match<'a>(matches: &[&'a TypeDecl]) -> Option<&'a TypeDecl> {
    match matches {
        [ty] => Some(*ty),
        _ => None,
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
        (policy.fallback(), policy.representation()),
        (
            RepresentationLookupFallback::PlannedType,
            &ObjectRepresentation::ErasedObject
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
        let java_ty = JavaType::Named {
            name: type_name.clone(),
            args: Vec::new(),
        };
        if matches!(
            facts.object_representation_override_strategy_for(&java_ty),
            Some(crate::facts::HierarchyStrategy::ErasedObject)
        ) {
            plan.types
                .entry(type_name.clone())
                .or_insert(TypeRepresentation {
                    java_name: type_name.clone(),
                    representation: ObjectRepresentation::ErasedObject,
                    provenance: RepresentationProvenance::BuiltinExternalModel,
                    direct_subtypes: Vec::new(),
                });
        }
        if find_type_by_erased_name(program, &type_name)
            .is_some_and(|ty| ty.kind == TypeKind::Class)
        {
            plan.anonymous_class_bases.insert(type_name);
        }
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
                if let JavaType::Named { name, .. } = ty {
                    type_names.insert(name.clone());
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
    add_direct_subtype(plan, edge.super_name(), edge.sub_name());
    let edges = plan
        .hierarchy_edges_by_subtype
        .entry(edge.sub_name().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Span;
    use crate::java_ir::{MethodDecl, PrimitiveType, TypeKind};

    #[test]
    fn map_field_entry_escape_requires_live_native_map_storage() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.LinkedHashMap;
import java.util.Map;
class FieldEntries {
    LinkedHashMap<String, Integer> values = new LinkedHashMap<>();
    Integer mutate() {
        values.put(null, null);
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        return entry.setValue(7);
    }
}
"#,
        );
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(plan.field_needs_live_native_map("FieldEntries", "values"));
        assert!(plan.live_native_map_field_has_nullable_key("FieldEntries", "values"));
        assert!(plan.live_native_map_field_has_nullable_value("FieldEntries", "values"));
        assert!(plan.source_map_entry_uses_value_carrier());
    }

    #[test]
    fn static_map_field_entry_escape_requires_synchronized_live_storage() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.LinkedHashMap;
import java.util.Map;
class StaticFieldEntries {
    static LinkedHashMap<String, Integer> values = new LinkedHashMap<>();
    static Integer mutate() {
        values.put(null, null);
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        return entry.setValue(7);
    }
}
"#,
        );
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(plan.static_field_needs_live_native_map("StaticFieldEntries", "values"));
        assert!(plan.live_static_native_map_field_has_nullable_key("StaticFieldEntries", "values"));
        assert!(
            plan.live_static_native_map_field_has_nullable_value("StaticFieldEntries", "values")
        );
        assert!(plan.source_map_entry_uses_value_carrier());
    }

    #[test]
    fn interface_map_parameter_entry_escape_requires_live_value_carrier() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.Map;
class InterfaceMapEntries {
    static Integer mutate(Map<String, Integer> values) {
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        return entry.setValue(7);
    }
}
"#,
        );
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(plan.source_map_entry_uses_value_carrier());
    }

    #[test]
    fn mutated_interface_map_local_entry_requires_live_value_carrier() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.HashMap;
import java.util.Map;
class InterfaceMapLocalEntries {
    static Integer mutate() {
        Map<String, Integer> values = new HashMap<>();
        values.put("a", 1);
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        return entry.setValue(7);
    }
}
"#,
        );
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(plan.source_map_entry_uses_value_carrier());
    }

    #[test]
    fn inline_native_map_entry_argument_requires_live_value_carrier() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.HashMap;
import java.util.Map;
class InlineMapEntryArgument {
    static Integer mutate(Map.Entry<String, Integer> entry) {
        return entry.setValue(7);
    }
    static Integer call() {
        HashMap<String, Integer> values = new HashMap<>();
        values.put("a", 1);
        return mutate(values.entrySet().iterator().next());
    }
}
"#,
        );
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(plan.source_map_entry_uses_value_carrier());
    }

    #[test]
    fn read_only_native_map_entry_local_keeps_snapshot_representation() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.HashMap;
import java.util.Map;
class ReadOnlyMapLocalEntries {
    static Integer read() {
        Map<String, Integer> values = new HashMap<>();
        values.put("a", 1);
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        return entry.getValue();
    }
}
"#,
        );
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(!plan.source_map_entry_uses_value_carrier());
    }

    #[test]
    fn ordinary_map_field_does_not_require_live_entry_storage() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.LinkedHashMap;
class PlainFieldMap {
    LinkedHashMap<String, Integer> values = new LinkedHashMap<>();
    Integer read() { return values.get("a"); }
}
"#,
        );
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(!plan.field_needs_live_native_map("PlainFieldMap", "values"));
    }

    #[test]
    fn receiver_statement_reference_alias_loses_field_provenance_on_reassignment() {
        let node_type = JavaType::Named {
            name: "Node".to_string(),
            args: Vec::new(),
        };
        let field = crate::java_ir::FieldDecl {
            name: "node".to_string(),
            ty: node_type.clone(),
            modifiers: Vec::new(),
            initializer: None,
            span: Span::default(),
        };
        let alias = Stmt::VarDecl {
            ty: node_type,
            name: "alias".to_string(),
            source_name: "alias".to_string(),
            initializer: Some(Expr::FieldAccess(Box::new(Expr::This), "node".to_string())),
            binding: None,
        };
        let write = Stmt::Assign {
            target: Expr::FieldAccess(
                Box::new(Expr::Var("alias".to_string())),
                "value".to_string(),
            ),
            value: Expr::Literal(crate::java_ir::Literal::Int(1)),
        };
        let returned = Stmt::Return(Some(Expr::Literal(crate::java_ir::Literal::Int(1))));
        let return_type = JavaType::Primitive(PrimitiveType::Int);
        let fields = [&field];

        assert!(stateful_anonymous_receiver_statement_body_is_supported(
            &Block {
                statements: vec![alias.clone(), write.clone(), returned.clone()],
            },
            &return_type,
            &fields,
            &[],
        ));
        assert!(!stateful_anonymous_receiver_statement_body_is_supported(
            &Block {
                statements: vec![
                    alias,
                    Stmt::Assign {
                        target: Expr::Var("alias".to_string()),
                        value: Expr::Var("other".to_string()),
                    },
                    write,
                    returned,
                ],
            },
            &return_type,
            &fields,
            &[],
        ));
    }

    #[test]
    fn class_statement_reference_alias_loses_field_provenance_on_reassignment() {
        let alias = Stmt::VarDecl {
            ty: JavaType::Named {
                name: "Node".to_string(),
                args: Vec::new(),
            },
            name: "alias".to_string(),
            source_name: "alias".to_string(),
            initializer: Some(Expr::FieldAccess(Box::new(Expr::This), "node".to_string())),
            binding: None,
        };
        let write = Stmt::Assign {
            target: Expr::FieldAccess(
                Box::new(Expr::Var("alias".to_string())),
                "value".to_string(),
            ),
            value: Expr::Literal(crate::java_ir::Literal::Int(1)),
        };
        let supports = |statements: Vec<Stmt>| {
            anonymous_source_class_local_statement_body_is_supported(
                &statements,
                &["node".to_string()],
                false,
                &HashSet::new(),
                &HashSet::new(),
                &HashMap::new(),
                &mut HashSet::new(),
            )
        };
        assert!(supports(vec![alias.clone(), write.clone()]));
        assert!(!supports(vec![
            alias,
            Stmt::Assign {
                target: Expr::Var("alias".to_string()),
                value: Expr::Var("other".to_string()),
            },
            write,
        ]));
    }

    #[test]
    fn source_type_named_like_platform_facade_stays_concrete() {
        let parsed = crate::parser::parse_java_source("class Path {}");
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(matches!(
            plan.type_representation("Path")
                .map(|entry| entry.representation()),
            Some(ObjectRepresentation::ConcreteStruct)
        ));
    }

    #[test]
    fn anonymous_source_class_base_requires_virtual_carrier() {
        let parsed = crate::parser::parse_java_source(
            "class Base {} class Use { Base make() { return new Base() {}; } }",
        );
        let plan = plan_representations(&parsed.program, &FactSet::default());

        assert!(plan.anonymous_class_base_requires_virtual_carrier("Base"));
        assert!(matches!(
            plan.type_representation("Base")
                .map(|entry| entry.representation()),
            Some(ObjectRepresentation::SourceHierarchyCarrier { .. })
        ));
    }

    #[test]
    fn concrete_signature_shape_renders_source_carrier_representation() {
        let mut plan = RepresentationPlan::default();
        plan.types.insert(
            "Base".to_string(),
            TypeRepresentation::new(
                "Base",
                ObjectRepresentation::SourceHierarchyCarrier {
                    carrier_name: "BaseValue".to_string(),
                    variants: vec!["Base".to_string()],
                },
                RepresentationProvenance::ParsedSource,
                Vec::new(),
            ),
        );
        let shape = crate::slot_shape::RustSlotShape::conservative(JavaType::Named {
            name: "Base".to_string(),
            args: Vec::new(),
        });

        assert_eq!(
            crate::emit::emit_type(
                &crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &FactSet::default(),
                    &plan,
                )
            ),
            "BaseValue"
        );
    }

    #[test]
    fn hierarchy_carrier_construction_uses_its_variant_payload_storage_placement() {
        let mut plan = RepresentationPlan::default();
        plan.types.insert(
            "Base".to_string(),
            TypeRepresentation::new(
                "Base",
                ObjectRepresentation::SourceHierarchyCarrier {
                    carrier_name: "BaseValue".to_string(),
                    variants: vec!["Base".to_string()],
                },
                RepresentationProvenance::ParsedSource,
                Vec::new(),
            ),
        );
        plan.storage.insert(
            "Base".to_string(),
            crate::storage_strategy::TypeStorageFacts::default()
                .with_escape(crate::storage_strategy::Escape::Stored)
                .with_mutated(true),
        );

        let constructed = crate::call_lowering::wrap_source_hierarchy_carrier_value(
            crate::rust_ir::RustExpr::Call {
                function: Box::new(crate::rust_ir::RustExpr::path(["Base", "new"])),
                args: Vec::new(),
            },
            "Base",
            &plan,
        );

        assert_eq!(
            crate::emit::emit_expr(&constructed),
            "BaseValue::Base(std::rc::Rc::new(std::cell::RefCell::new(Base::new())))"
        );
    }

    #[test]
    fn hierarchy_carrier_construction_preserves_result_propagation() {
        let mut plan = RepresentationPlan::default();
        plan.types.insert(
            "Child".to_string(),
            TypeRepresentation::new(
                "Child",
                ObjectRepresentation::SourceHierarchyCarrier {
                    carrier_name: "BaseValue".to_string(),
                    variants: vec!["Child".to_string()],
                },
                RepresentationProvenance::ParsedSource,
                Vec::new(),
            ),
        );
        let constructor = crate::coercion::propagate_result_projection(
            crate::coercion::function_call_projection(
                crate::rust_ir::RustExpr::path(["Child", "new"]),
                Vec::new(),
            ),
        );

        let wrapped =
            crate::call_lowering::wrap_source_hierarchy_carrier_value(constructor, "Child", &plan);

        assert_eq!(
            crate::emit::emit_expr(&wrapped),
            "BaseValue::Child(Child::new()?)"
        );
        assert!(crate::coercion::expr_is_result_propagated(&wrapped));
    }

    fn interface_type(name: &str, method: &str) -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Interface,
            name: name.to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: vec![Member::Method(MethodDecl {
                name: method.to_string(),
                rust_name: None,
                type_params: Vec::new(),
                type_param_bounds: Vec::new(),
                return_type: JavaType::Void,
                throws: Vec::new(),
                params: Vec::new(),
                modifiers: Vec::new(),
                body: None,
                span: Span::default(),
            })],
            static_init_blocks: Vec::new(),
            span: Span::default(),
        }
    }

    fn class_type(name: &str, members: Vec<Member>) -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Class,
            name: name.to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members,
            static_init_blocks: Vec::new(),
            span: Span::default(),
        }
    }

    fn plan_with_dyn_trait(type_name: &str) -> RepresentationPlan {
        let mut plan = RepresentationPlan::default();
        plan.types.insert(
            type_name.to_string(),
            TypeRepresentation::new(
                type_name,
                ObjectRepresentation::DynTrait {
                    trait_name: type_name.to_string(),
                },
                RepresentationProvenance::ParsedSource,
                Vec::new(),
            ),
        );
        plan
    }

    #[test]
    fn erased_name_type_lookup_refuses_ambiguous_simple_package_peer() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                interface_type("a.Task", "run"),
                interface_type("b.Task", "close"),
                interface_type("c.Unique", "run"),
            ],
        };

        assert_eq!(
            find_type_by_erased_name(&program, "a.Task").map(|ty| ty.name.as_str()),
            Some("a.Task")
        );
        assert_eq!(
            find_type_by_erased_name(&program, "Unique").map(|ty| ty.name.as_str()),
            Some("c.Unique")
        );
        assert_eq!(find_type_by_erased_name(&program, "Task"), None);
        assert_eq!(find_type_by_erased_name(&program, "missing.Task"), None);
    }

    #[test]
    fn erased_name_type_lookup_refuses_duplicate_exact_owner() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                interface_type("a.Task", "run"),
                interface_type("a.Task", "close"),
                interface_type("c.Unique", "run"),
            ],
        };

        assert_eq!(
            find_type_by_erased_name(&program, "a.Task").map(|ty| ty.name.as_str()),
            None,
            "duplicate exact erased-name owners must fail closed"
        );
        assert_eq!(
            find_type_by_erased_name(&program, "Unique").map(|ty| ty.name.as_str()),
            Some("c.Unique")
        );
    }

    #[test]
    fn anonymous_zero_arg_interface_support_refuses_ambiguous_simple_owner() {
        let anonymous = Expr::New {
            ty: JavaType::Named {
                name: "Task".to_string(),
                args: Vec::new(),
            },
            qualifier: None,
            args: Vec::new(),
            body: Some(vec![Member::Method(MethodDecl {
                name: "run".to_string(),
                rust_name: None,
                type_params: Vec::new(),
                type_param_bounds: Vec::new(),
                return_type: JavaType::Void,
                throws: Vec::new(),
                params: Vec::new(),
                modifiers: Vec::new(),
                body: Some(Block {
                    statements: Vec::new(),
                }),
                span: Span::default(),
            })]),
            resolved: None,
        };
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                interface_type("a.Task", "run"),
                interface_type("b.Task", "close"),
                class_type(
                    "UseTask",
                    vec![Member::Method(MethodDecl {
                        name: "make".to_string(),
                        rust_name: None,
                        type_params: Vec::new(),
                        type_param_bounds: Vec::new(),
                        return_type: JavaType::Void,
                        throws: Vec::new(),
                        params: Vec::new(),
                        modifiers: Vec::new(),
                        body: Some(Block {
                            statements: vec![Stmt::Expr(anonymous)],
                        }),
                        span: Span::default(),
                    })],
                ),
            ],
        };
        let plan = plan_with_dyn_trait("Task");
        let diagnostics = anonymous_class_diagnostics_with_plan(&program, &program, Some(&plan));

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code() == "unsupported_anonymous_class"),
            "ambiguous simple Task must not borrow the first package peer"
        );
    }

    #[test]
    fn anonymous_zero_arg_interface_body_using_this_still_reports_unsupported() {
        let anonymous = Expr::New {
            ty: JavaType::Named {
                name: "Task".to_string(),
                args: Vec::new(),
            },
            qualifier: None,
            args: Vec::new(),
            body: Some(vec![Member::Method(MethodDecl {
                name: "run".to_string(),
                rust_name: None,
                type_params: Vec::new(),
                type_param_bounds: Vec::new(),
                return_type: JavaType::Void,
                throws: Vec::new(),
                params: Vec::new(),
                modifiers: Vec::new(),
                body: Some(Block {
                    statements: vec![Stmt::Expr(Expr::Call {
                        receiver: Some(Box::new(Expr::This)),
                        name: "touch".to_string(),
                        args: Vec::new(),
                        resolved: None,
                    })],
                }),
                span: Span::default(),
            })]),
            resolved: None,
        };
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                interface_type("Task", "run"),
                class_type(
                    "UseTask",
                    vec![Member::Method(MethodDecl {
                        name: "make".to_string(),
                        rust_name: None,
                        type_params: Vec::new(),
                        type_param_bounds: Vec::new(),
                        return_type: JavaType::Void,
                        throws: Vec::new(),
                        params: Vec::new(),
                        modifiers: Vec::new(),
                        body: Some(Block {
                            statements: vec![Stmt::Expr(anonymous)],
                        }),
                        span: Span::default(),
                    })],
                ),
            ],
        };
        let plan = plan_with_dyn_trait("Task");
        let diagnostics = anonymous_class_diagnostics_with_plan(&program, &program, Some(&plan));

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code() == "unsupported_anonymous_class"),
            "anonymous bodies that use this are rejected by lowering and must stay diagnostic-visible"
        );
    }
}
