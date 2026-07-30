use std::collections::{HashMap, HashSet};

use crate::fact_saturation::{FiniteSaturationPolicy, SaturationStep};
use crate::facts::{
    AccessorInlinePolicy, CatchFact, ConstructorKey, ConstructorRecursionFact, ControlFlowFact,
    ControlFlowFactKind, DispatchRequirementFact, DispatchRequirementKind, ExceptionFact,
    ExceptionKind, ExternalApiEffectFact, ExternalConstructorFact, ExternalFieldFact,
    ExternalFieldKind, FactProvenance, FactSet, FieldKey, FieldNullabilityFact,
    FieldNullabilityProvenance, HierarchyFactLookupIndex, MethodEffectFact, MethodKey,
    NullabilityState, ReceiverMutabilityFact, RecursionFact, RecursionKind, TrivialAccessorFact,
    TrivialAccessorKind, UnresolvedTargetKind,
};
use crate::graph::strongly_connected_components;
use crate::java_ir::{
    BinaryOp, Block, CatchClause, ConstructorDecl, ConstructorInvocationKind, Expr, FieldDecl,
    JavaType, LambdaBody, Literal, Member, MethodDecl, Modifier, Param, Program, Stmt, SwitchLabel,
    TypeDecl,
};
use crate::lattice::field_nullability_join;
use crate::rewrite::{
    AssignmentConditionNormalizedShape, AssignmentInConditionPlan, ControlFlowRewritePlan,
    MethodControlFlowPlan, NullTestKind, TailRecursiveLoopPlan, TailRecursiveNormalizedShape,
};
use crate::types::{TypeEnv, infer_expr_type};
use crate::usage::{self, CallGraphEdgeKind, CallableKey, UsageFacts, UsageMethodKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticFacts {
    pub facts: FactSet,
    pub usage: UsageFacts,
    pub control_flow_plans: Vec<MethodControlFlowPlan>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    pub iterations: SemanticAnalysisIterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticAnalysisIterations {
    pub field_nullability: usize,
    pub external_api_effects: usize,
    pub control_flow: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticAnalysisConfig {
    pub max_external_guess_iterations: Option<usize>,
    pub enable_external_api_guesses: bool,
    pub enable_control_flow_facts: bool,
    pub enable_builtin_external_packs: bool,
}

impl Default for SemanticAnalysisConfig {
    fn default() -> Self {
        Self {
            max_external_guess_iterations: None,
            enable_external_api_guesses: true,
            enable_control_flow_facts: true,
            enable_builtin_external_packs: true,
        }
    }
}

pub fn analyze_program(
    program: &Program,
    resolved_facts: &FactSet,
    external_facts: &FactSet,
) -> SemanticFacts {
    analyze_program_with_config(
        program,
        resolved_facts,
        external_facts,
        SemanticAnalysisConfig::default(),
    )
}

pub fn analyze_program_with_config(
    program: &Program,
    resolved_facts: &FactSet,
    external_facts: &FactSet,
    config: SemanticAnalysisConfig,
) -> SemanticFacts {
    let mut facts = resolved_facts.clone();
    facts.extend_from(external_facts.clone());
    let mut diagnostics = Vec::new();

    collect_builtin_external_representation_facts(&mut facts, config.enable_builtin_external_packs);
    collect_trivial_accessor_facts(program, &mut facts);
    if config.enable_external_api_guesses {
        collect_java_platform_api_argument_effect_facts(&mut facts);
        collect_builtin_external_api_argument_effect_facts(
            &mut facts,
            config.enable_builtin_external_packs,
        );
    }
    let usage = usage::analyze_usage_with_facts(program, &facts);
    let field_nullability_iterations =
        collect_field_nullability_facts(program, &mut facts, &mut diagnostics);
    if config.enable_external_api_guesses {
        collect_external_api_effect_facts(program, &mut facts, &config, &mut diagnostics);
        collect_java_platform_api_argument_effect_facts(&mut facts);
        collect_java_platform_exception_facts(&mut facts);
        collect_builtin_external_exception_facts(&mut facts, config.enable_builtin_external_packs);
        collect_builtin_external_api_argument_effect_facts(
            &mut facts,
            config.enable_builtin_external_packs,
        );
        collect_external_constructor_facts(
            program,
            &mut facts,
            config.enable_builtin_external_packs,
            &mut diagnostics,
        );
        collect_external_field_facts(
            program,
            &mut facts,
            config.enable_builtin_external_packs,
            &mut diagnostics,
        );
    }
    collect_method_effect_facts(program, &usage, &mut facts);
    propagate_source_call_thrown_effects(program, &mut facts);
    collect_recursion_facts(program, &mut facts);
    collect_dispatch_requirement_facts(program, &mut facts);
    let control_flow_plans = if config.enable_control_flow_facts {
        collect_control_flow_facts(program, &mut facts);
        collect_control_flow_plans(program)
    } else {
        Vec::new()
    };
    let hierarchy_index = facts.hierarchy_lookup_index();
    collect_exception_facts(program, &mut facts, &hierarchy_index, &mut diagnostics);
    collect_catch_facts(program, &mut facts, &hierarchy_index);
    collect_recursion_fallback_diagnostics(program, &facts, &control_flow_plans, &mut diagnostics);

    SemanticFacts {
        facts,
        usage,
        control_flow_plans,
        diagnostics,
        iterations: SemanticAnalysisIterations {
            field_nullability: field_nullability_iterations,
            external_api_effects: usize::from(config.enable_external_api_guesses),
            control_flow: usize::from(config.enable_control_flow_facts),
        },
    }
}

fn collect_builtin_external_representation_facts(facts: &mut FactSet, enabled: bool) {
    if !enabled {
        return;
    }

    let builtin = crate::external_packs::builtin_external_library_packs();
    for fact in builtin.hierarchy_overrides {
        if !facts.hierarchy_overrides.contains(&fact) {
            facts.hierarchy_overrides.push(fact);
        }
    }
    for fact in builtin.closed_hierarchy_facts {
        if !facts.closed_hierarchy_facts.contains(&fact) {
            facts.closed_hierarchy_facts.push(fact);
        }
    }
    for fact in builtin.object_safety_facts {
        if !facts.object_safety_facts.contains(&fact) {
            facts.object_safety_facts.push(fact);
        }
    }
    for fact in builtin.type_capability_overrides {
        if !facts.type_capability_overrides.contains(&fact) {
            facts.type_capability_overrides.push(fact);
        }
    }
    for fact in builtin.owned_recovery_capability_facts {
        if !facts.owned_recovery_capability_facts.contains(&fact) {
            facts.owned_recovery_capability_facts.push(fact);
        }
    }
    for fact in builtin.external_conversion_adapter_facts {
        if !facts.external_conversion_adapter_facts.contains(&fact) {
            facts.external_conversion_adapter_facts.push(fact);
        }
    }
}

fn collect_trivial_accessor_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_trivial_accessor_facts(ty, facts);
    }
}

fn collect_type_trivial_accessor_facts(ty: &TypeDecl, facts: &mut FactSet) {
    let field_types = field_types(ty);
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(fact) = trivial_accessor_fact(ty, method, &field_types) {
                    facts.trivial_accessor_facts.push(fact);
                }
            }
            Member::Type(nested) => collect_type_trivial_accessor_facts(nested, facts),
            _ => {}
        }
    }
}

fn trivial_accessor_fact(
    ty: &TypeDecl,
    method: &MethodDecl,
    field_types: &HashMap<String, JavaType>,
) -> Option<TrivialAccessorFact> {
    trivial_getter_fact(ty, method, field_types)
        .or_else(|| trivial_setter_fact(ty, method, field_types))
}

fn trivial_getter_fact(
    ty: &TypeDecl,
    method: &MethodDecl,
    field_types: &HashMap<String, JavaType>,
) -> Option<TrivialAccessorFact> {
    if !method.params.is_empty() {
        return None;
    }
    let body = method.body.as_ref()?;
    let [Stmt::Return(Some(expr))] = body.statements.as_slice() else {
        return None;
    };
    let field_name = returned_field_name(expr)?;
    let field_ty = field_types.get(field_name)?;
    if !types_compatible(&method.return_type, field_ty) {
        return None;
    }
    Some(TrivialAccessorFact {
        method: MethodKey {
            type_name: ty.name.clone(),
            method_name: method.name.clone(),
        },
        kind: TrivialAccessorKind::Getter {
            field: FieldKey {
                type_name: ty.name.clone(),
                field_name: field_name.to_string(),
            },
        },
        inline_policy: accessor_inline_policy(&method.modifiers),
        provenance: FactProvenance::ParsedSource,
    })
}

fn trivial_setter_fact(
    ty: &TypeDecl,
    method: &MethodDecl,
    field_types: &HashMap<String, JavaType>,
) -> Option<TrivialAccessorFact> {
    if method.params.len() != 1 {
        return None;
    }
    let param = method.params.first()?;
    let body = method.body.as_ref()?;
    let (assign, fluent) = match body.statements.as_slice() {
        [Stmt::Assign { target, value }] => ((target, value), false),
        [
            Stmt::Assign { target, value },
            Stmt::Return(Some(Expr::This)),
        ] => ((target, value), true),
        _ => return None,
    };
    let field_name = assigned_this_field_name(assign.0)?;
    if !matches!(assign.1, Expr::Var(name) if name == &param.name) {
        return None;
    }
    let field_ty = field_types.get(field_name)?;
    if !types_compatible(&param.ty, field_ty) {
        return None;
    }
    Some(TrivialAccessorFact {
        method: MethodKey {
            type_name: ty.name.clone(),
            method_name: method.name.clone(),
        },
        kind: TrivialAccessorKind::Setter {
            field: FieldKey {
                type_name: ty.name.clone(),
                field_name: field_name.to_string(),
            },
            param_name: param.name.clone(),
            fluent,
        },
        inline_policy: accessor_inline_policy(&method.modifiers),
        provenance: FactProvenance::ParsedSource,
    })
}

fn collect_field_nullability_facts(
    program: &Program,
    facts: &mut FactSet,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) -> usize {
    let mut max_iterations = 1;
    for ty in &program.types {
        max_iterations =
            max_iterations.max(collect_type_field_nullability_facts(ty, facts, diagnostics));
    }
    max_iterations
}

fn collect_external_api_effect_facts(
    program: &Program,
    facts: &mut FactSet,
    config: &SemanticAnalysisConfig,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let mut seen = HashSet::new();
    let mut budget_exhausted = false;
    let mut guessed_count = 0usize;
    for ty in &program.types {
        collect_type_external_api_effect_facts(
            ty,
            facts,
            &mut seen,
            config.max_external_guess_iterations,
            config.enable_builtin_external_packs,
            &mut guessed_count,
            &mut budget_exhausted,
            diagnostics,
        );
    }
    collect_external_api_effect_facts_from_unresolved_targets(
        program,
        facts,
        &mut seen,
        config.max_external_guess_iterations,
        config.enable_builtin_external_packs,
        &mut guessed_count,
        &mut budget_exhausted,
        diagnostics,
    );
    if budget_exhausted {
        diagnostics.push(SemanticDiagnostic {
            code: "semantic_external_api_effect_budget_exhausted",
            message: "external API effect guessing stopped at configured finite budget".to_string(),
        });
    }
}

fn collect_java_platform_api_argument_effect_facts(facts: &mut FactSet) {
    for fact in crate::external_packs::java_platform_api_argument_effect_facts() {
        let already_present = facts
            .external_api_argument_effect_facts
            .iter()
            .any(|existing| {
                existing.receiver_type == fact.receiver_type
                    && existing.method_name == fact.method_name
                    && existing.parameter_index == fact.parameter_index
                    && existing.provenance == fact.provenance
            });
        if !already_present {
            facts.external_api_argument_effect_facts.push(fact);
        }
    }
}

fn collect_java_platform_exception_facts(facts: &mut FactSet) {
    for fact in crate::external_packs::java_platform_exception_facts() {
        let already_present = facts.exception_facts.iter().any(|existing| {
            existing.type_name == fact.type_name && existing.provenance == fact.provenance
        });
        if !already_present {
            facts.exception_facts.push(fact);
        }
    }
}

fn collect_builtin_external_exception_facts(facts: &mut FactSet, enabled: bool) {
    if !enabled {
        return;
    }

    for fact in crate::external_packs::builtin_external_library_packs().exception_facts {
        let already_present = facts.exception_facts.iter().any(|existing| {
            existing.type_name == fact.type_name && existing.provenance == fact.provenance
        });
        if !already_present {
            facts.exception_facts.push(fact);
        }
    }
}

fn collect_builtin_external_api_argument_effect_facts(facts: &mut FactSet, enabled: bool) {
    if !enabled {
        return;
    }

    for fact in crate::external_packs::builtin_external_api_argument_effect_facts() {
        let already_present = facts
            .external_api_argument_effect_facts
            .iter()
            .any(|existing| {
                existing.receiver_type == fact.receiver_type
                    && existing.method_name == fact.method_name
                    && existing.parameter_index == fact.parameter_index
                    && existing.provenance == fact.provenance
            });
        if !already_present {
            facts.external_api_argument_effect_facts.push(fact);
        }
    }
}

fn collect_external_constructor_facts(
    program: &Program,
    facts: &mut FactSet,
    enable_builtin_external_packs: bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let source_types = program_type_names(program);
    let unresolved_targets = facts.unresolved_target_facts.clone();
    for fact in unresolved_targets {
        if fact.kind != UnresolvedTargetKind::ConstructorCall || fact.is_anonymous_class {
            continue;
        }
        let Some(type_name) = constructor_target_type_name(&fact) else {
            continue;
        };
        if source_types.contains(type_name) {
            continue;
        }
        if facts
            .external_constructor_for(type_name, &fact.arg_types)
            .is_some()
        {
            continue;
        }
        if let Some(unsupported) =
            facts.unsupported_external_constructor_for(type_name, fact.arg_types.len())
        {
            diagnostics.push(SemanticDiagnostic {
                code: "external_pack_unsupported_constructor",
                message: format!(
                    "{} constructor with {} argument(s) is unsupported by external pack {}: {}",
                    type_name,
                    fact.arg_types.len(),
                    unsupported.pack_name,
                    unsupported.reason
                ),
            });
            continue;
        }
        if let Some(pack) = facts.external_library_pack_for_type(type_name) {
            diagnostics.push(SemanticDiagnostic {
                code: "external_pack_missing_constructor",
                message: format!(
                    "{} constructor with {} argument(s) is inside external pack {} but has no modeled coverage entry",
                    type_name,
                    fact.arg_types.len(),
                    pack.pack_name
                ),
            });
            continue;
        }
        if let Some(pack_fact) = builtin_external_constructor_for(
            type_name,
            &fact.arg_types,
            enable_builtin_external_packs,
        ) {
            facts.external_constructor_facts.push(pack_fact);
            continue;
        }
        if let Some(unsupported) = builtin_unsupported_external_constructor_for(
            type_name,
            fact.arg_types.len(),
            enable_builtin_external_packs,
        ) {
            diagnostics.push(SemanticDiagnostic {
                code: "external_pack_unsupported_constructor",
                message: format!(
                    "{} constructor with {} argument(s) is unsupported by external pack {}: {}",
                    type_name,
                    fact.arg_types.len(),
                    unsupported.pack_name,
                    unsupported.reason
                ),
            });
            continue;
        }
        if let Some(pack) =
            builtin_external_library_pack_for_type(type_name, enable_builtin_external_packs)
        {
            diagnostics.push(SemanticDiagnostic {
                code: "external_pack_missing_constructor",
                message: format!(
                    "{} constructor with {} argument(s) is inside external pack {} but has no modeled coverage entry",
                    type_name,
                    fact.arg_types.len(),
                    pack.pack_name
                ),
            });
            continue;
        }
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: type_name.to_string(),
                arg_types: fact.arg_types.clone(),
                provenance: crate::stdlib::external_constructor_provenance_with_builtin_packs(
                    type_name,
                    &fact.arg_types,
                    enable_builtin_external_packs,
                ),
            });
    }
}

fn collect_external_field_facts(
    program: &Program,
    facts: &mut FactSet,
    enable_builtin_external_packs: bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let source_types = program_type_names(program);
    let external_superclasses = external_superclasses_by_source_type(program, &source_types);
    let unresolved_targets = facts.unresolved_target_facts.clone();
    for fact in unresolved_targets {
        if fact.kind != UnresolvedTargetKind::FieldAccess {
            continue;
        }
        let Some(type_name) = fact.receiver_type.as_ref().and_then(java_type_name) else {
            continue;
        };
        let type_name = crate::stdlib::canonical_external_receiver_type_key(type_name);
        if program_contains_type_named(&source_types, &type_name)
            && !external_superclasses.contains_key(&type_name)
        {
            continue;
        }
        if facts
            .external_field_for(&type_name, &fact.target_name)
            .is_some()
        {
            continue;
        }
        if let Some(unsupported) =
            facts.unsupported_external_field_for(&type_name, &fact.target_name)
        {
            diagnostics.push(SemanticDiagnostic {
                code: "external_pack_unsupported_field",
                message: format!(
                    "{}.{} is unsupported by external pack {}: {}",
                    type_name, fact.target_name, unsupported.pack_name, unsupported.reason
                ),
            });
            continue;
        }
        if let Some(pack_fact) =
            builtin_external_field_for(&type_name, &fact.target_name, enable_builtin_external_packs)
        {
            facts.external_field_facts.push(pack_fact);
            continue;
        }
        if let Some(unsupported) = builtin_unsupported_external_field_for(
            &type_name,
            &fact.target_name,
            enable_builtin_external_packs,
        ) {
            diagnostics.push(SemanticDiagnostic {
                code: "external_pack_unsupported_field",
                message: format!(
                    "{}.{} is unsupported by external pack {}: {}",
                    type_name, fact.target_name, unsupported.pack_name, unsupported.reason
                ),
            });
            continue;
        }
        if let Some(pack) = facts.external_library_pack_for_type(&type_name) {
            diagnostics.push(SemanticDiagnostic {
                code: "external_pack_missing_field",
                message: format!(
                    "{}.{} is inside external pack {} but has no modeled coverage entry",
                    type_name, fact.target_name, pack.pack_name
                ),
            });
            continue;
        }
        if let Some(pack) =
            builtin_external_library_pack_for_type(&type_name, enable_builtin_external_packs)
        {
            diagnostics.push(SemanticDiagnostic {
                code: "external_pack_missing_field",
                message: format!(
                    "{}.{} is inside external pack {} but has no modeled coverage entry",
                    type_name, fact.target_name, pack.pack_name
                ),
            });
            continue;
        }
        facts.external_field_facts.push(ExternalFieldFact {
            field: FieldKey {
                type_name: type_name.clone(),
                field_name: fact.target_name.clone(),
            },
            field_type: crate::stdlib::known_external_field_type_for_receiver(
                &JavaType::Named {
                    name: type_name.clone(),
                    args: Vec::new(),
                },
                &fact.target_name,
            ),
            field_kind: ExternalFieldKind::Unknown,
            provenance: crate::stdlib::external_field_provenance_with_builtin_packs(
                &type_name,
                &fact.target_name,
                enable_builtin_external_packs,
            ),
        });
    }
}

fn external_superclasses_by_source_type(
    program: &Program,
    source_types: &HashSet<String>,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for ty in &program.types {
        collect_type_external_superclasses(ty, source_types, &mut out);
    }
    out
}

fn collect_type_external_superclasses(
    ty: &TypeDecl,
    source_types: &HashSet<String>,
    out: &mut HashMap<String, String>,
) {
    if let Some(extends) = &ty.extends {
        let super_ty = crate::parser::parse_type(extends);
        if let Some(super_name) = java_type_name(&super_ty) {
            let canonical_super = crate::stdlib::canonical_external_receiver_type_key(super_name);
            if !program_contains_type_named(source_types, &canonical_super) {
                out.insert(ty.name.clone(), canonical_super);
            }
        }
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_external_superclasses(nested, source_types, out);
        }
    }
}

fn program_contains_type_named(source_types: &HashSet<String>, type_name: &str) -> bool {
    source_types.contains(type_name)
        || source_types
            .iter()
            .any(|source_type| crate::parser::to_snake_case(source_type) == type_name)
}

fn constructor_target_type_name(fact: &crate::facts::UnresolvedTargetFact) -> Option<&str> {
    fact.receiver_type
        .as_ref()
        .and_then(java_type_name)
        .or(Some(fact.target_name.as_str()))
}

fn program_type_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for ty in &program.types {
        collect_program_type_names(ty, &mut names);
    }
    names
}

fn collect_program_type_names(ty: &TypeDecl, names: &mut HashSet<String>) {
    names.insert(ty.name.clone());
    for member in &ty.members {
        collect_member_type_names(member, names);
    }
}

fn collect_member_type_names(member: &Member, names: &mut HashSet<String>) {
    match member {
        Member::Type(nested) => collect_program_type_names(nested, names),
        Member::Method(method) => {
            if let Some(body) = &method.body {
                collect_block_type_names(body, names);
            }
        }
        Member::Constructor(constructor) => {
            if let Some(body) = &constructor.body {
                collect_block_type_names(body, names);
            }
        }
        _ => {}
    }
}

fn collect_block_type_names(block: &Block, names: &mut HashSet<String>) {
    for stmt in &block.statements {
        collect_stmt_type_names(stmt, names);
    }
}

fn collect_stmt_type_names(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::LocalType(local) => collect_program_type_names(local, names),
        Stmt::VarDecl {
            initializer: Some(initializer),
            ..
        } => collect_expr_type_names(initializer, names),
        Stmt::Assign { target, value } => {
            collect_expr_type_names(target, names);
            collect_expr_type_names(value, names);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_expr_type_names(expr, names),
        Stmt::Return(Some(expr)) => collect_expr_type_names(expr, names),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_type_names(condition, names);
            collect_block_type_names(then_block, names);
            if let Some(else_block) = else_block {
                collect_block_type_names(else_block, names);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_expr_type_names(condition, names);
            collect_block_type_names(body, names);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_stmt_type_names(stmt, names);
            }
            if let Some(condition) = condition {
                collect_expr_type_names(condition, names);
            }
            for expr in update {
                collect_expr_type_names(expr, names);
            }
            collect_block_type_names(body, names);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_expr_type_names(iterable, names);
            collect_block_type_names(body, names);
        }
        Stmt::Switch {
            selector, cases, ..
        } => {
            collect_expr_type_names(selector, names);
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_type_names(stmt, names);
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
                collect_stmt_type_names(resource, names);
            }
            collect_block_type_names(body, names);
            for catch in catches {
                collect_block_type_names(&catch.body, names);
            }
            if let Some(finally_block) = finally_block {
                collect_block_type_names(finally_block, names);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_type_names(arg, names);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_block_type_names(block, names)
        }
        Stmt::VarDecl {
            initializer: None, ..
        }
        | Stmt::Return(None)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn collect_expr_type_names(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_expr_type_names(receiver, names);
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            collect_expr_type_names(array, names);
            collect_expr_type_names(index, names);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_expr_type_names(expr, names);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_expr_type_names(expr, names);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_expr_type_names(expr, names);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_type_names(condition, names);
            collect_expr_type_names(then_expr, names);
            collect_expr_type_names(else_expr, names);
        }
        Expr::Switch { selector, cases } => {
            collect_expr_type_names(selector, names);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_expr_type_names(expr, names);
                    }
                }
                collect_expr_type_names(&case.value, names);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_type_names(receiver, names);
            }
            for arg in args {
                collect_expr_type_names(arg, names);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_type_names(arg, names);
            }
            if let Some(members) = body {
                for member in members {
                    collect_member_type_names(member, names);
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_type_names(expr, names);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_expr_type_names(expr, names),
            LambdaBody::Block(block) => collect_block_type_names(block, names),
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

fn collect_type_external_api_effect_facts(
    ty: &TypeDecl,
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    enable_builtin_external_packs: bool,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_external_api_effects_from_block(
                        ty,
                        &method.type_params,
                        &method.type_param_bounds,
                        &method.params,
                        body,
                        facts,
                        seen,
                        budget,
                        enable_builtin_external_packs,
                        guessed_count,
                        budget_exhausted,
                        diagnostics,
                    );
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_external_api_effects_from_block(
                        ty,
                        &ctor.type_params,
                        &ctor.type_param_bounds,
                        &ctor.params,
                        body,
                        facts,
                        seen,
                        budget,
                        enable_builtin_external_packs,
                        guessed_count,
                        budget_exhausted,
                        diagnostics,
                    );
                }
            }
            Member::Type(nested) => collect_type_external_api_effect_facts(
                nested,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
            ),
            _ => {}
        }
    }
}

fn collect_external_api_effect_facts_from_unresolved_targets(
    program: &Program,
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    enable_builtin_external_packs: bool,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let unresolved_targets = facts.unresolved_target_facts.clone();
    for fact in unresolved_targets {
        if fact.kind != UnresolvedTargetKind::MethodCall {
            continue;
        }
        let receiver_type_key = unresolved_target_receiver_type_key(program, &fact);
        if receiver_type_key.is_none()
            && !crate::stdlib::is_known_receiverless_static_utility_method(
                &fact.target_name,
                fact.arg_types.len(),
            )
            && !crate::stdlib::has_receiver_agnostic_external_return_type(
                &fact.target_name,
                fact.arg_types.len(),
            )
        {
            continue;
        }
        push_external_api_effect_guess(
            facts,
            seen,
            budget,
            enable_builtin_external_packs,
            guessed_count,
            budget_exhausted,
            diagnostics,
            receiver_type_key,
            &fact.target_name,
            fact.arg_types.len(),
        );
    }
}

fn unresolved_target_receiver_type_key(
    program: &Program,
    fact: &crate::facts::UnresolvedTargetFact,
) -> Option<String> {
    let receiver_type = fact.receiver_type.as_ref()?;
    if let JavaType::Named { name, args } = receiver_type
        && args.is_empty()
        && let Some(bound) = unresolved_target_type_param_bound(program, fact, name)
    {
        return external_api_receiver_type_key(&bound);
    }
    external_api_receiver_type_key(receiver_type)
}

fn unresolved_target_type_param_bound(
    program: &Program,
    fact: &crate::facts::UnresolvedTargetFact,
    type_param: &str,
) -> Option<JavaType> {
    let ty = find_type_decl(program, &fact.enclosing_type)?;
    type_param_bound_from_decl(&ty.type_params, &ty.type_param_bounds, type_param).or_else(|| {
        let member_name = fact.enclosing_member.as_deref()?;
        type_param_bound_from_member(ty, member_name, type_param)
    })
}

fn type_param_bound_from_member(
    ty: &TypeDecl,
    member_name: &str,
    type_param: &str,
) -> Option<JavaType> {
    for member in &ty.members {
        match member {
            Member::Method(method) if method.name == member_name => {
                if let Some(bound) = type_param_bound_from_decl(
                    &method.type_params,
                    &method.type_param_bounds,
                    type_param,
                ) {
                    return Some(bound);
                }
            }
            Member::Constructor(constructor) if member_name == "<init>" => {
                if let Some(bound) = type_param_bound_from_decl(
                    &constructor.type_params,
                    &constructor.type_param_bounds,
                    type_param,
                ) {
                    return Some(bound);
                }
            }
            _ => {}
        }
    }
    None
}

fn type_param_bound_from_decl(
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
    type_param: &str,
) -> Option<JavaType> {
    type_params
        .iter()
        .zip(type_param_bounds.iter())
        .find_map(|(name, bound)| (name == type_param).then(|| bound.clone()).flatten())
}

fn find_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_type_decl_in_type(ty, type_name))
}

fn find_type_decl_in_type<'a>(ty: &'a TypeDecl, type_name: &str) -> Option<&'a TypeDecl> {
    if ty.name == type_name {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_type_decl_in_type(nested, type_name),
        _ => None,
    })
}

fn collect_external_api_effects_from_block(
    ty: &TypeDecl,
    member_type_params: &[String],
    member_type_param_bounds: &[Option<JavaType>],
    params: &[crate::java_ir::Param],
    body: &Block,
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    enable_builtin_external_packs: bool,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let mut env =
        collect_external_api_type_env(ty, member_type_params, member_type_param_bounds, params);
    collect_external_api_effects_from_block_with_env(
        body,
        facts,
        seen,
        budget,
        enable_builtin_external_packs,
        guessed_count,
        budget_exhausted,
        diagnostics,
        &mut env,
        &ty.name,
    );
}

fn collect_external_api_effects_from_block_with_env(
    body: &Block,
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    enable_builtin_external_packs: bool,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
    env: &mut TypeEnv,
    current_type: &str,
) {
    for stmt in &body.statements {
        collect_external_api_effects_from_stmt_with_env(
            stmt,
            facts,
            seen,
            budget,
            enable_builtin_external_packs,
            guessed_count,
            budget_exhausted,
            diagnostics,
            env,
            current_type,
        );
        env.learn_stmt(stmt);
    }
}

fn collect_external_api_effects_from_stmt_with_env(
    stmt: &Stmt,
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    enable_builtin_external_packs: bool,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
    env: &mut TypeEnv,
    current_type: &str,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                collect_external_api_effects_from_expr_with_env(
                    expr,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    env,
                    current_type,
                );
            }
        }
        Stmt::Assign { target, value } => {
            collect_external_api_effects_from_expr_with_env(
                target,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                env,
                current_type,
            );
            collect_external_api_effects_from_expr_with_env(
                value,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                env,
                current_type,
            );
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_external_api_effects_from_expr_with_env(
            expr,
            facts,
            seen,
            budget,
            enable_builtin_external_packs,
            guessed_count,
            budget_exhausted,
            diagnostics,
            env,
            current_type,
        ),
        Stmt::Return(Some(expr)) => collect_external_api_effects_from_expr_with_env(
            expr,
            facts,
            seen,
            budget,
            enable_builtin_external_packs,
            guessed_count,
            budget_exhausted,
            diagnostics,
            env,
            current_type,
        ),
        Stmt::Return(None)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::LocalType(_)
        | Stmt::Unsupported(_) => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_external_api_effects_from_expr_with_env(
                condition,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                env,
                current_type,
            );
            collect_external_api_effects_from_block_with_env(
                then_block,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                &mut env.clone(),
                current_type,
            );
            if let Some(else_block) = else_block {
                collect_external_api_effects_from_block_with_env(
                    else_block,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    &mut env.clone(),
                    current_type,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_external_api_effects_from_expr_with_env(
                condition,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                env,
                current_type,
            );
            collect_external_api_effects_from_block_with_env(
                body,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                &mut env.clone(),
                current_type,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut loop_env = env.clone();
            for stmt in init {
                collect_external_api_effects_from_stmt_with_env(
                    stmt,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    &mut loop_env,
                    current_type,
                );
                loop_env.learn_stmt(stmt);
            }
            if let Some(condition) = condition {
                collect_external_api_effects_from_expr_with_env(
                    condition,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    &mut loop_env,
                    current_type,
                );
            }
            for expr in update {
                collect_external_api_effects_from_expr_with_env(
                    expr,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    &mut loop_env,
                    current_type,
                );
            }
            collect_external_api_effects_from_block_with_env(
                body,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                &mut loop_env,
                current_type,
            );
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
            ..
        } => {
            collect_external_api_effects_from_expr_with_env(
                iterable,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                env,
                current_type,
            );
            let mut body_env = env.clone();
            body_env.insert_local(var_name.clone(), var_type.clone());
            collect_external_api_effects_from_block_with_env(
                body,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                &mut body_env,
                current_type,
            );
        }
        Stmt::Switch { selector, cases } => {
            collect_external_api_effects_from_expr_with_env(
                selector,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                env,
                current_type,
            );
            for case in cases {
                let mut case_env = env.clone();
                for stmt in &case.statements {
                    collect_external_api_effects_from_stmt_with_env(
                        stmt,
                        facts,
                        seen,
                        budget,
                        enable_builtin_external_packs,
                        guessed_count,
                        budget_exhausted,
                        diagnostics,
                        &mut case_env,
                        current_type,
                    );
                    case_env.learn_stmt(stmt);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut try_env = env.clone();
            for resource in resources {
                collect_external_api_effects_from_stmt_with_env(
                    resource,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    &mut try_env,
                    current_type,
                );
                try_env.learn_stmt(resource);
            }
            collect_external_api_effects_from_block_with_env(
                body,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                &mut try_env.clone(),
                current_type,
            );
            for catch in catches {
                let mut catch_env = env.clone();
                for catch_type in &catch.types {
                    catch_env.insert_local(catch.name.clone(), catch_type.clone());
                }
                collect_external_api_effects_from_block_with_env(
                    &catch.body,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    &mut catch_env,
                    current_type,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_external_api_effects_from_block_with_env(
                    finally_block,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    &mut env.clone(),
                    current_type,
                );
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_external_api_effects_from_expr_with_env(
                    arg,
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    env,
                    current_type,
                );
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_external_api_effects_from_block_with_env(
                block,
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                &mut env.clone(),
                current_type,
            )
        }
    }
}

fn collect_external_api_effects_from_expr_with_env(
    expr: &Expr,
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    enable_builtin_external_packs: bool,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
    env: &TypeEnv,
    current_type: &str,
) {
    walk_expr_outside_lambdas(expr, &mut |expr| {
        if let Expr::MethodReference { target, method } = expr {
            if !is_non_external_method_reference(target, method) {
                let receiver = method_reference_receiver(target, env);
                let arg_count = usize::from(receiver.bound_receiver);
                push_external_api_effect_guess(
                    facts,
                    seen,
                    budget,
                    enable_builtin_external_packs,
                    guessed_count,
                    budget_exhausted,
                    diagnostics,
                    receiver.type_key,
                    method,
                    arg_count,
                );
            }
        }
        if let Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } = expr
        {
            let Some(receiver_type_key) = crate::call_plan::external_call_receiver_type_key(
                receiver.as_deref(),
                name,
                args.len(),
                facts,
                env,
                Some(current_type),
            ) else {
                return;
            };
            let is_modeled_external_api =
                receiver_type_key.as_deref().is_some_and(|receiver_type| {
                    crate::external_packs::java_platform_api_effect_for_receiver_with_arity(
                        receiver_type,
                        name,
                        Some(args.len()),
                    )
                    .is_some()
                        || crate::external_packs::external_api_effect_for_receiver_with_arity(
                            Some(receiver_type),
                            name,
                            Some(args.len()),
                        )
                        .is_some()
                });
            if resolved.is_some() && !is_modeled_external_api {
                return;
            }
            if receiver_type_key.is_none()
                && !crate::stdlib::is_known_receiverless_static_utility_method(name, args.len())
                && !crate::stdlib::has_receiver_agnostic_external_return_type(name, args.len())
            {
                return;
            }
            push_external_api_effect_guess(
                facts,
                seen,
                budget,
                enable_builtin_external_packs,
                guessed_count,
                budget_exhausted,
                diagnostics,
                receiver_type_key,
                name,
                args.len(),
            );
        }
        if let Expr::FieldAccess(receiver, field) = expr
            && let Some(receiver_type_key) = external_api_receiver_expr_type_key(receiver, env)
        {
            push_external_field_fact_guess(
                facts,
                enable_builtin_external_packs,
                diagnostics,
                &receiver_type_key,
                field,
            );
        }
    });
}

fn walk_expr_outside_lambdas(expr: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(expr);
    match expr {
        Expr::FieldAccess(receiver, _) => walk_expr_outside_lambdas(receiver, visit),
        Expr::ArrayAccess(array, index) => {
            walk_expr_outside_lambdas(array, visit);
            walk_expr_outside_lambdas(index, visit);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                walk_expr_outside_lambdas(expr, visit);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    walk_expr_outside_lambdas(expr, visit);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                walk_expr_outside_lambdas(expr, visit);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr_outside_lambdas(left, visit);
            walk_expr_outside_lambdas(right, visit);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            walk_expr_outside_lambdas(expr, visit);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            walk_expr_outside_lambdas(condition, visit);
            walk_expr_outside_lambdas(then_expr, visit);
            walk_expr_outside_lambdas(else_expr, visit);
        }
        Expr::Switch { selector, cases } => {
            walk_expr_outside_lambdas(selector, visit);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        walk_expr_outside_lambdas(expr, visit);
                    }
                }
                walk_expr_outside_lambdas(&case.value, visit);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                walk_expr_outside_lambdas(receiver, visit);
            }
            for arg in args {
                walk_expr_outside_lambdas(arg, visit);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                walk_expr_outside_lambdas(arg, visit);
            }
        }
        Expr::Lambda { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn is_non_external_method_reference(target: &str, method: &str) -> bool {
    is_source_receiver_method_reference(target)
        || is_array_constructor_method_reference(target, method)
}

fn is_source_receiver_method_reference(target: &str) -> bool {
    matches!(target, "this" | "super")
}

fn is_array_constructor_method_reference(target: &str, method: &str) -> bool {
    method == "new" && matches!(crate::parser::parse_type(target), JavaType::Array(_))
}

struct MethodReferenceReceiver {
    type_key: Option<String>,
    bound_receiver: bool,
}

fn method_reference_receiver(target: &str, env: &TypeEnv) -> MethodReferenceReceiver {
    if let Some(java_ty) = env.local_type(target).or_else(|| env.field_type(target)) {
        return MethodReferenceReceiver {
            type_key: external_api_receiver_type_key_with_env(java_ty, env),
            bound_receiver: true,
        };
    }
    let normalized = crate::parser::to_snake_case(target);
    if normalized != target
        && let Some(java_ty) = env
            .local_type(&normalized)
            .or_else(|| env.field_type(&normalized))
    {
        return MethodReferenceReceiver {
            type_key: external_api_receiver_type_key_with_env(java_ty, env),
            bound_receiver: true,
        };
    }
    let parsed = crate::parser::parse_type(target);
    let type_key = java_type_name(&parsed)
        .map(crate::stdlib::canonical_external_receiver_type_key)
        .or_else(|| Some(crate::stdlib::canonical_external_receiver_type_key(target)));
    MethodReferenceReceiver {
        type_key,
        bound_receiver: false,
    }
}

fn push_external_api_effect_guess(
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    enable_builtin_external_packs: bool,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
    receiver_type_key: Option<String>,
    name: &str,
    arg_count: usize,
) {
    if facts
        .external_api_effect_for_receiver_with_arity(
            receiver_type_key.as_deref(),
            name,
            Some(arg_count),
        )
        .is_some_and(|fact| fact.provenance == FactProvenance::UserOverride)
    {
        return;
    }
    if facts
        .external_api_effect_for_receiver_with_arity(
            receiver_type_key.as_deref(),
            name,
            Some(arg_count),
        )
        .is_some()
    {
        return;
    }
    if let Some(receiver_type) = receiver_type_key.as_deref()
        && let Some(unsupported) = facts.unsupported_external_api_for_receiver(receiver_type, name)
    {
        diagnostics.push(SemanticDiagnostic {
            code: "external_pack_unsupported_api",
            message: format!(
                "{}.{} is unsupported by external pack {}: {}",
                unsupported.receiver_type,
                unsupported.method_name,
                unsupported.pack_name,
                unsupported.reason
            ),
        });
        return;
    }
    if let Some(receiver_type) = receiver_type_key.as_deref()
        && let Some(unsupported) = builtin_unsupported_external_api_for_receiver(
            Some(receiver_type),
            name,
            enable_builtin_external_packs,
        )
    {
        diagnostics.push(SemanticDiagnostic {
            code: "external_pack_unsupported_api",
            message: format!(
                "{}.{} is unsupported by external pack {}: {}",
                unsupported.receiver_type,
                unsupported.method_name,
                unsupported.pack_name,
                unsupported.reason
            ),
        });
        return;
    }
    if let Some(pack_fact) = builtin_external_api_effect_for_receiver(
        receiver_type_key.as_deref(),
        name,
        arg_count,
        enable_builtin_external_packs,
    ) {
        facts.external_api_effect_facts.push(pack_fact);
        return;
    }
    let policy = crate::stdlib::external_api_method_policy_with_builtin_packs(
        receiver_type_key.as_deref(),
        name,
        arg_count,
        enable_builtin_external_packs,
    );
    if policy.provenance == FactProvenance::BuiltinExternalModel {
        facts.external_api_effect_facts.push(ExternalApiEffectFact {
            method_name: name.to_string(),
            receiver_type: receiver_type_key,
            arg_count: Some(arg_count),
            role: policy.role,
            receiver_mutability: policy.receiver_mutability,
            return_type: None,
            return_nullability: policy.return_nullability,
            thrown: Vec::new(),
            provenance: policy.provenance,
        });
        *guessed_count += 1;
        return;
    }
    if enable_builtin_external_packs
        && receiver_type_key.as_deref().is_some_and(|receiver_type| {
            crate::external_packs::builtin_external_pack_models_api(receiver_type, name)
        })
    {
        return;
    }
    if let Some(receiver_type) = receiver_type_key.as_deref()
        && let Some(pack) = facts.external_library_pack_for_type(receiver_type)
    {
        diagnostics.push(SemanticDiagnostic {
            code: "external_pack_missing_api",
            message: format!(
                "{}.{} is inside external pack {} but has no modeled coverage entry",
                receiver_type, name, pack.pack_name
            ),
        });
        return;
    }
    if let Some(receiver_type) = receiver_type_key.as_deref()
        && let Some(pack) =
            builtin_external_library_pack_for_type(receiver_type, enable_builtin_external_packs)
    {
        diagnostics.push(SemanticDiagnostic {
            code: "external_pack_missing_api",
            message: format!(
                "{}.{} is inside external pack {} but has no modeled coverage entry",
                receiver_type, name, pack.pack_name
            ),
        });
        return;
    }
    let seen_key = format!(
        "{}::{}::{}",
        receiver_type_key.as_deref().unwrap_or("*"),
        name,
        arg_count
    );
    if !seen.insert(seen_key) {
        return;
    }
    if budget.is_some_and(|budget| *guessed_count >= budget) {
        *budget_exhausted = true;
        return;
    }
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: name.to_string(),
        receiver_type: receiver_type_key,
        arg_count: Some(arg_count),
        role: policy.role,
        receiver_mutability: policy.receiver_mutability,
        return_type: None,
        return_nullability: policy.return_nullability,
        thrown: Vec::new(),
        provenance: policy.provenance,
    });
    *guessed_count += 1;
}

fn push_external_field_fact_guess(
    facts: &mut FactSet,
    enable_builtin_external_packs: bool,
    diagnostics: &mut Vec<SemanticDiagnostic>,
    type_name: &str,
    field_name: &str,
) {
    let type_name = crate::stdlib::canonical_external_receiver_type_key(type_name);
    if facts
        .external_field_for(&type_name, field_name)
        .is_some_and(|fact| fact.provenance == FactProvenance::UserOverride)
    {
        return;
    }
    if facts.external_field_for(&type_name, field_name).is_some() {
        return;
    }
    if let Some(unsupported) = facts.unsupported_external_field_for(&type_name, field_name) {
        push_semantic_diagnostic_once(
            diagnostics,
            "external_pack_unsupported_field",
            format!(
                "{}.{} is unsupported by external pack {}: {}",
                type_name, field_name, unsupported.pack_name, unsupported.reason
            ),
        );
        return;
    }
    if let Some(pack_fact) =
        builtin_external_field_for(&type_name, field_name, enable_builtin_external_packs)
    {
        facts.external_field_facts.push(pack_fact);
        return;
    }
    if let Some(unsupported) = builtin_unsupported_external_field_for(
        &type_name,
        field_name,
        enable_builtin_external_packs,
    ) {
        push_semantic_diagnostic_once(
            diagnostics,
            "external_pack_unsupported_field",
            format!(
                "{}.{} is unsupported by external pack {}: {}",
                type_name, field_name, unsupported.pack_name, unsupported.reason
            ),
        );
        return;
    }
    if let Some(pack) = facts.external_library_pack_for_type(&type_name) {
        push_semantic_diagnostic_once(
            diagnostics,
            "external_pack_missing_field",
            format!(
                "{}.{} is inside external pack {} but has no modeled coverage entry",
                type_name, field_name, pack.pack_name
            ),
        );
        return;
    }
    if let Some(pack) =
        builtin_external_library_pack_for_type(&type_name, enable_builtin_external_packs)
    {
        push_semantic_diagnostic_once(
            diagnostics,
            "external_pack_missing_field",
            format!(
                "{}.{} is inside external pack {} but has no modeled coverage entry",
                type_name, field_name, pack.pack_name
            ),
        );
    }
}

fn push_semantic_diagnostic_once(
    diagnostics: &mut Vec<SemanticDiagnostic>,
    code: &'static str,
    message: String,
) {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == code && diagnostic.message == message)
    {
        return;
    }
    diagnostics.push(SemanticDiagnostic { code, message });
}

fn builtin_external_api_effect_for_receiver(
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: usize,
    enabled: bool,
) -> Option<ExternalApiEffectFact> {
    if let Some(receiver_type) = receiver_type
        && let Some(effect) =
            crate::external_packs::java_platform_api_effect_for_receiver_with_arity(
                receiver_type,
                method_name,
                Some(arg_count),
            )
    {
        return Some(effect);
    }
    enabled
        .then(|| {
            crate::external_packs::external_api_effect_for_receiver_with_arity(
                receiver_type,
                method_name,
                Some(arg_count),
            )
        })
        .flatten()
}

fn builtin_unsupported_external_api_for_receiver(
    receiver_type: Option<&str>,
    method_name: &str,
    enabled: bool,
) -> Option<crate::facts::UnsupportedExternalApiFact> {
    enabled
        .then(|| {
            crate::external_packs::unsupported_external_api_for_receiver(receiver_type, method_name)
        })
        .flatten()
}

fn builtin_external_constructor_for(
    type_name: &str,
    arg_types: &[JavaType],
    enabled: bool,
) -> Option<ExternalConstructorFact> {
    enabled
        .then(|| crate::external_packs::external_constructor_for(type_name, arg_types))
        .flatten()
}

fn builtin_unsupported_external_constructor_for(
    type_name: &str,
    arg_count: usize,
    enabled: bool,
) -> Option<crate::facts::UnsupportedExternalConstructorFact> {
    enabled
        .then(|| crate::external_packs::unsupported_external_constructor_for(type_name, arg_count))
        .flatten()
}

fn builtin_external_field_for(
    type_name: &str,
    field_name: &str,
    enabled: bool,
) -> Option<ExternalFieldFact> {
    enabled
        .then(|| crate::external_packs::external_field_for(type_name, field_name))
        .flatten()
}

fn builtin_unsupported_external_field_for(
    type_name: &str,
    field_name: &str,
    enabled: bool,
) -> Option<crate::facts::UnsupportedExternalFieldFact> {
    enabled
        .then(|| crate::external_packs::unsupported_external_field_for(type_name, field_name))
        .flatten()
}

fn builtin_external_library_pack_for_type(
    type_name: &str,
    enabled: bool,
) -> Option<crate::facts::ExternalLibraryPackFact> {
    enabled
        .then(|| crate::external_packs::external_library_pack_for_type(type_name))
        .flatten()
}

fn external_api_receiver_expr_type_key(receiver: &Expr, env: &TypeEnv) -> Option<String> {
    let inferred = infer_expr_type(receiver, env);
    if weak_unknown_field_access_receiver(receiver, &inferred) {
        let external_field_ty = infer_external_field_access_type(receiver, env)?;
        return external_api_receiver_type_key_with_env(&external_field_ty, env);
    }
    external_api_receiver_type_key_with_env(&inferred, env)
}

fn infer_external_field_access_type(expr: &Expr, env: &TypeEnv) -> Option<JavaType> {
    let Expr::FieldAccess(receiver, field) = expr else {
        return None;
    };
    let receiver_ty = infer_external_field_access_type(receiver, env)
        .unwrap_or_else(|| infer_expr_type(receiver, env));
    external_pack_or_stdlib_field_type(&receiver_ty, field)
}

fn external_pack_or_stdlib_field_type(receiver_ty: &JavaType, field: &str) -> Option<JavaType> {
    if let Some(receiver_name) = external_field_receiver_name(receiver_ty)
        && let Some(field_fact) = crate::external_packs::external_field_for(&receiver_name, field)
        && let Some(field_type) = field_fact.field_type
    {
        return Some(field_type);
    }
    crate::stdlib::known_external_field_type_for_receiver(receiver_ty, field)
}

fn external_field_receiver_name(receiver_ty: &JavaType) -> Option<String> {
    match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
        }
        _ => None,
    }
}

fn weak_unknown_field_access_receiver(receiver: &Expr, inferred: &JavaType) -> bool {
    match (receiver, inferred) {
        (Expr::FieldAccess(_, field), JavaType::Unknown(name)) => {
            name == field
                || crate::parser::to_snake_case(name) == crate::parser::to_snake_case(field)
        }
        _ => false,
    }
}

fn collect_external_api_type_env(
    ty: &TypeDecl,
    member_type_params: &[String],
    member_type_param_bounds: &[Option<JavaType>],
    params: &[crate::java_ir::Param],
) -> TypeEnv {
    let mut env = TypeEnv::new();
    insert_external_api_type_param_bounds(&mut env, &ty.type_params, &ty.type_param_bounds);
    insert_external_api_type_param_bounds(&mut env, member_type_params, member_type_param_bounds);
    for member in &ty.members {
        if let Member::Field(field) = member {
            env.insert_field(field.name.clone(), field.ty.clone());
        }
    }
    for param in params {
        env.insert_local(param.name.clone(), param.ty.clone());
    }
    env
}

fn insert_external_api_type_param_bounds(
    env: &mut TypeEnv,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
) {
    for (name, bound) in type_params.iter().zip(type_param_bounds.iter()) {
        if let Some(bound) = bound {
            env.insert_type_param_bound(name.clone(), bound.clone());
        }
    }
}

fn external_api_receiver_type_key_with_env(java_ty: &JavaType, env: &TypeEnv) -> Option<String> {
    if let Some(element_ty) = crate::types::optional_element_type(java_ty) {
        return external_api_receiver_type_key_with_env(&element_ty, env);
    }
    if let JavaType::Named { name, args } = java_ty
        && args.is_empty()
        && let Some(bound) = env.type_param_bound(name)
    {
        return external_api_receiver_type_key(bound);
    }
    external_api_receiver_type_key(java_ty)
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

fn collect_recursion_facts(program: &Program, facts: &mut FactSet) {
    let call_graph = usage::build_call_graph(program);
    for ty in &program.types {
        collect_type_recursion_facts(ty, &call_graph, facts);
    }
}

fn collect_method_effect_facts(program: &Program, usage: &UsageFacts, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_method_effect_facts(ty, usage, facts);
    }
}

fn propagate_source_call_thrown_effects(program: &Program, facts: &mut FactSet) {
    let graph = usage::build_call_graph(program);
    let mut changed = true;
    while changed {
        changed = false;
        for edge in &graph.edges {
            if edge.kind != CallGraphEdgeKind::DirectMethodCall {
                continue;
            }
            let (
                CallableKey::Method {
                    type_name: caller_type,
                    method_name: caller_method,
                },
                CallableKey::Method {
                    type_name: callee_type,
                    method_name: callee_method,
                },
            ) = (&edge.caller, &edge.callee)
            else {
                continue;
            };
            let callee_thrown = facts
                .method_effect_for(callee_type, callee_method)
                .map(|effect| effect.thrown.clone())
                .unwrap_or_default();
            if callee_thrown.is_empty() {
                continue;
            }
            if push_method_thrown_effects(facts, caller_type, caller_method, &callee_thrown) {
                changed = true;
            }
        }
    }
}

fn push_method_thrown_effects(
    facts: &mut FactSet,
    type_name: &str,
    method_name: &str,
    thrown: &[JavaType],
) -> bool {
    let index = facts
        .method_effect_facts
        .iter()
        .position(|fact| source_method_effect_key_matches(&fact.method, type_name, method_name))
        .unwrap_or_else(|| {
            facts.method_effect_facts.push(MethodEffectFact {
                method: MethodKey {
                    type_name: type_name.to_string(),
                    method_name: method_name.to_string(),
                },
                receiver_mutability: ReceiverMutabilityFact::Unknown,
                return_nullability: NullabilityState::Unknown,
                thrown: Vec::new(),
                provenance: FactProvenance::ParsedSource,
            });
            facts.method_effect_facts.len() - 1
        });
    if facts.method_effect_facts[index].provenance == FactProvenance::UserOverride {
        return false;
    };
    let effect = &mut facts.method_effect_facts[index];
    let before = effect.thrown.len();
    for ty in thrown {
        push_unique_java_type(&mut effect.thrown, ty.clone());
    }
    effect.thrown.len() != before
}

fn source_method_effect_key_matches(key: &MethodKey, type_name: &str, method_name: &str) -> bool {
    key.type_name == type_name
        && (key.method_name == method_name
            || crate::parser::to_snake_case(&key.method_name) == method_name
            || key.method_name == crate::parser::to_snake_case(method_name))
}

fn collect_type_method_effect_facts(ty: &TypeDecl, usage: &UsageFacts, facts: &mut FactSet) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let mut thrown = method_thrown_exception_types(method, facts);
                for external_throw in external_api_thrown_exception_types(ty, method, facts) {
                    push_unique_java_type(&mut thrown, external_throw);
                }
                let receiver_mutability = method_receiver_mutability_effect(ty, method, usage);
                let mut return_nullability = method_return_nullability_effect(method);
                if return_nullability == NullabilityState::Unknown
                    && let Some(existing) = facts.method_effect_for(&ty.name, &method.name)
                    && existing.return_nullability != NullabilityState::Unknown
                {
                    return_nullability = existing.return_nullability;
                }
                if receiver_mutability != ReceiverMutabilityFact::Unknown
                    || return_nullability != NullabilityState::Unknown
                    || !thrown.is_empty()
                {
                    let effect = MethodEffectFact {
                        method: MethodKey {
                            type_name: ty.name.clone(),
                            method_name: method.name.clone(),
                        },
                        receiver_mutability,
                        return_nullability,
                        thrown: thrown.clone(),
                        provenance: FactProvenance::ParsedSource,
                    };
                    facts.method_effect_facts.push(effect.clone());
                    if let Some(rust_name) = method.rust_name.as_deref()
                        && rust_name != method.name
                        && rust_name != crate::parser::to_snake_case(&method.name)
                    {
                        let mut rust_name_effect = effect;
                        rust_name_effect.method.method_name = rust_name.to_string();
                        facts.method_effect_facts.push(rust_name_effect);
                    }
                }
            }
            Member::Type(nested) => collect_type_method_effect_facts(nested, usage, facts),
            _ => {}
        }
    }
}

fn external_api_thrown_exception_types(
    ty: &TypeDecl,
    method: &MethodDecl,
    facts: &FactSet,
) -> Vec<JavaType> {
    let Some(body) = &method.body else {
        return Vec::new();
    };
    let mut env = collect_external_api_type_env(
        ty,
        &method.type_params,
        &method.type_param_bounds,
        &method.params,
    );
    let mut thrown = Vec::new();
    collect_external_api_thrown_exception_types_from_stmts(
        &body.statements,
        &ty.name,
        facts,
        &mut env,
        &mut thrown,
    );
    thrown
}

fn collect_external_api_thrown_exception_types_from_stmts(
    statements: &[Stmt],
    current_type: &str,
    facts: &FactSet,
    env: &mut TypeEnv,
    thrown: &mut Vec<JavaType>,
) {
    for stmt in statements {
        collect_external_api_thrown_exception_types_from_stmt(
            stmt,
            current_type,
            facts,
            env,
            thrown,
        );
        env.learn_stmt(stmt);
    }
}

fn collect_external_api_thrown_exception_types_from_stmt(
    stmt: &Stmt,
    current_type: &str,
    facts: &FactSet,
    env: &mut TypeEnv,
    thrown: &mut Vec<JavaType>,
) {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut try_env = env.clone();
            let mut try_thrown = Vec::new();
            collect_external_api_thrown_exception_types_from_stmts(
                resources,
                current_type,
                facts,
                &mut try_env,
                &mut try_thrown,
            );
            collect_external_api_thrown_exception_types_from_stmts(
                &body.statements,
                current_type,
                facts,
                &mut try_env,
                &mut try_thrown,
            );
            push_uncaught_after_catches(&mut try_thrown, catches, facts, thrown);
            for catch in catches {
                let mut catch_env = env.clone();
                collect_external_api_thrown_exception_types_from_stmts(
                    &catch.body.statements,
                    current_type,
                    facts,
                    &mut catch_env,
                    thrown,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_external_api_thrown_exception_types_from_stmts(
                    &finally_block.statements,
                    current_type,
                    facts,
                    env,
                    thrown,
                );
            }
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            let mut then_env = env.clone();
            collect_external_api_thrown_exception_types_from_stmts(
                &then_block.statements,
                current_type,
                facts,
                &mut then_env,
                thrown,
            );
            if let Some(else_block) = else_block {
                let mut else_env = env.clone();
                collect_external_api_thrown_exception_types_from_stmts(
                    &else_block.statements,
                    current_type,
                    facts,
                    &mut else_env,
                    thrown,
                );
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            let mut block_env = env.clone();
            collect_external_api_thrown_exception_types_from_stmts(
                &body.statements,
                current_type,
                facts,
                &mut block_env,
                thrown,
            );
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                let mut case_env = env.clone();
                collect_external_api_thrown_exception_types_from_stmts(
                    &case.statements,
                    current_type,
                    facts,
                    &mut case_env,
                    thrown,
                );
            }
        }
        _ => {
            collect_external_api_thrown_exception_types_from_exprs(
                stmt,
                current_type,
                facts,
                env,
                thrown,
            );
        }
    }
}

fn collect_external_api_thrown_exception_types_from_exprs(
    stmt: &Stmt,
    current_type: &str,
    facts: &FactSet,
    env: &TypeEnv,
    thrown: &mut Vec<JavaType>,
) {
    walk_stmt_exprs(stmt, &mut |expr| {
        let Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } = expr
        else {
            return;
        };
        if resolved.is_some() {
            return;
        }
        let Some(receiver_type_key) = crate::call_plan::external_call_receiver_type_key(
            receiver.as_deref(),
            name,
            args.len(),
            facts,
            env,
            Some(current_type),
        ) else {
            return;
        };
        let Some(effect) =
            facts.external_api_effect_for_receiver(receiver_type_key.as_deref(), name)
        else {
            return;
        };
        for ty in &effect.thrown {
            push_unique_java_type(thrown, ty.clone());
        }
    });
}

fn method_receiver_mutability_effect(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
) -> ReceiverMutabilityFact {
    if method.modifiers.contains(&Modifier::Static) {
        return ReceiverMutabilityFact::Unknown;
    }
    usage
        .methods
        .get(&UsageMethodKey {
            type_name: ty.name.clone(),
            method_name: method.name.clone(),
        })
        .map(|usage| {
            if usage.mutates_self {
                ReceiverMutabilityFact::MutatesReceiver
            } else {
                ReceiverMutabilityFact::ReadOnly
            }
        })
        .unwrap_or(ReceiverMutabilityFact::Unknown)
}

pub(crate) fn method_return_nullability_effect(method: &MethodDecl) -> NullabilityState {
    if matches!(method.return_type, JavaType::Void | JavaType::Primitive(_)) {
        return NullabilityState::DefinitelyNonNull;
    }
    let Some(body) = &method.body else {
        return NullabilityState::Unknown;
    };
    let mut states = Vec::new();
    collect_return_nullability_states_from_block(body, &mut states);
    merge_return_nullability_states(&states)
}

fn collect_return_nullability_states_from_block(block: &Block, states: &mut Vec<NullabilityState>) {
    for stmt in &block.statements {
        collect_return_nullability_states_from_stmt(stmt, states);
    }
}

fn collect_return_nullability_states_from_stmt(stmt: &Stmt, states: &mut Vec<NullabilityState>) {
    match stmt {
        Stmt::Return(Some(expr)) => states.push(expr_return_nullability(expr)),
        Stmt::Return(None) => states.push(NullabilityState::DefinitelyNull),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_return_nullability_states_from_block(then_block, states);
            if let Some(else_block) = else_block {
                collect_return_nullability_states_from_block(else_block, states);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            collect_return_nullability_states_from_block(body, states)
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    collect_return_nullability_states_from_stmt(stmt, states);
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
                collect_return_nullability_states_from_stmt(resource, states);
            }
            collect_return_nullability_states_from_block(body, states);
            for catch in catches {
                collect_return_nullability_states_from_block(&catch.body, states);
            }
            if let Some(finally_block) = finally_block {
                collect_return_nullability_states_from_block(finally_block, states);
            }
        }
        Stmt::LocalType(ty) => {
            for member in &ty.members {
                if let Member::Method(method) = member {
                    let state = method_return_nullability_effect(method);
                    if state != NullabilityState::Unknown {
                        states.push(state);
                    }
                }
            }
        }
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Throw(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn expr_return_nullability(expr: &Expr) -> NullabilityState {
    match expr {
        Expr::Literal(Literal::Null) => NullabilityState::DefinitelyNull,
        Expr::Literal(_)
        | Expr::This
        | Expr::New { .. }
        | Expr::ArrayCreation { .. }
        | Expr::ArrayInitializer(_)
        | Expr::ClassLiteral(_) => NullabilityState::DefinitelyNonNull,
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => merge_return_nullability_states(&[
            expr_return_nullability(then_expr),
            expr_return_nullability(else_expr),
        ]),
        _ => NullabilityState::Unknown,
    }
}

fn merge_return_nullability_states(states: &[NullabilityState]) -> NullabilityState {
    if states.is_empty() {
        return NullabilityState::Unknown;
    }
    if states.contains(&NullabilityState::Unknown) {
        NullabilityState::Unknown
    } else if states.contains(&NullabilityState::MaybeNull)
        || (states.contains(&NullabilityState::DefinitelyNull)
            && states.contains(&NullabilityState::DefinitelyNonNull))
    {
        NullabilityState::MaybeNull
    } else if states
        .iter()
        .all(|state| *state == NullabilityState::DefinitelyNull)
    {
        NullabilityState::DefinitelyNull
    } else {
        NullabilityState::DefinitelyNonNull
    }
}

fn method_thrown_exception_types(method: &MethodDecl, facts: &FactSet) -> Vec<JavaType> {
    let mut thrown = Vec::new();
    for ty in &method.throws {
        push_unique_java_type(&mut thrown, ty.clone());
    }
    if let Some(body) = &method.body {
        let mut exception_env = HashMap::new();
        collect_thrown_exception_types_from_block(body, &mut exception_env, facts, &mut thrown);
    }
    thrown
}

fn collect_thrown_exception_types_from_block(
    block: &Block,
    exception_env: &mut HashMap<String, JavaType>,
    facts: &FactSet,
    thrown: &mut Vec<JavaType>,
) {
    for stmt in &block.statements {
        collect_thrown_exception_types_from_stmt(stmt, exception_env, facts, thrown);
    }
}

fn collect_thrown_exception_types_from_stmt(
    stmt: &Stmt,
    exception_env: &mut HashMap<String, JavaType>,
    facts: &FactSet,
    thrown: &mut Vec<JavaType>,
) {
    match stmt {
        Stmt::Throw(expr) => {
            if let Some(ty) = thrown_exception_type_from_expr(expr, exception_env) {
                push_unique_java_type(thrown, ty);
            }
            collect_thrown_exception_types_from_expr(expr, exception_env, facts, thrown);
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_thrown_exception_types_from_block(then_block, exception_env, facts, thrown);
            if let Some(else_block) = else_block {
                collect_thrown_exception_types_from_block(else_block, exception_env, facts, thrown);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            collect_thrown_exception_types_from_block(body, exception_env, facts, thrown)
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    collect_thrown_exception_types_from_stmt(stmt, exception_env, facts, thrown);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut try_thrown = Vec::new();
            for resource in resources {
                collect_thrown_exception_types_from_stmt(
                    resource,
                    exception_env,
                    facts,
                    &mut try_thrown,
                );
            }
            collect_thrown_exception_types_from_block(body, exception_env, facts, &mut try_thrown);
            push_uncaught_after_catches(&mut try_thrown, catches, facts, thrown);
            for catch in catches {
                let previous = catch
                    .types
                    .first()
                    .cloned()
                    .and_then(|ty| exception_env.insert(catch.name.clone(), ty));
                collect_thrown_exception_types_from_block(
                    &catch.body,
                    exception_env,
                    facts,
                    thrown,
                );
                if let Some(previous) = previous {
                    exception_env.insert(catch.name.clone(), previous);
                } else {
                    exception_env.remove(&catch.name);
                }
            }
            if let Some(finally_block) = finally_block {
                collect_thrown_exception_types_from_block(
                    finally_block,
                    exception_env,
                    facts,
                    thrown,
                );
            }
        }
        Stmt::LocalType(ty) => {
            for member in &ty.members {
                if let Member::Method(method) = member {
                    for ty in method_thrown_exception_types(method, facts) {
                        push_unique_java_type(thrown, ty);
                    }
                }
            }
        }
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_thrown_exception_types_from_expr(initializer, exception_env, facts, thrown);
            }
        }
        Stmt::Assign { target, value } => {
            collect_thrown_exception_types_from_expr(target, exception_env, facts, thrown);
            collect_thrown_exception_types_from_expr(value, exception_env, facts, thrown);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) => {
            collect_thrown_exception_types_from_expr(expr, exception_env, facts, thrown);
        }
        Stmt::Return(None)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn collect_thrown_exception_types_from_expr(
    expr: &Expr,
    exception_env: &mut HashMap<String, JavaType>,
    facts: &FactSet,
    thrown: &mut Vec<JavaType>,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            collect_thrown_exception_types_from_expr(receiver, exception_env, facts, thrown);
        }
        Expr::ArrayAccess(array, index) => {
            collect_thrown_exception_types_from_expr(array, exception_env, facts, thrown);
            collect_thrown_exception_types_from_expr(index, exception_env, facts, thrown);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_thrown_exception_types_from_expr(dimension, exception_env, facts, thrown);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_thrown_exception_types_from_expr(expr, exception_env, facts, thrown);
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for value in values {
                collect_thrown_exception_types_from_expr(value, exception_env, facts, thrown);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_thrown_exception_types_from_expr(left, exception_env, facts, thrown);
            collect_thrown_exception_types_from_expr(right, exception_env, facts, thrown);
        }
        Expr::Unary { expr, .. } => {
            collect_thrown_exception_types_from_expr(expr, exception_env, facts, thrown);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_thrown_exception_types_from_expr(condition, exception_env, facts, thrown);
            collect_thrown_exception_types_from_expr(then_expr, exception_env, facts, thrown);
            collect_thrown_exception_types_from_expr(else_expr, exception_env, facts, thrown);
        }
        Expr::Switch { selector, cases } => {
            collect_thrown_exception_types_from_expr(selector, exception_env, facts, thrown);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_thrown_exception_types_from_expr(
                            expr,
                            exception_env,
                            facts,
                            thrown,
                        );
                    }
                }
                collect_thrown_exception_types_from_expr(&case.value, exception_env, facts, thrown);
            }
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            if let Some(receiver) = receiver {
                collect_thrown_exception_types_from_expr(receiver, exception_env, facts, thrown);
            }
            for arg in args {
                if let Expr::Lambda { body, .. } = arg {
                    if !call_captures_lambda_for_deferred_throw_projection(name) {
                        collect_thrown_exception_types_from_lambda_body(
                            body,
                            exception_env,
                            facts,
                            thrown,
                        );
                    }
                } else {
                    collect_thrown_exception_types_from_expr(arg, exception_env, facts, thrown);
                }
            }
        }
        Expr::New { ty, args, .. } => {
            if matches!(
                java_type_name(ty).map(|name| name.rsplit('.').next().unwrap_or(name)),
                Some("URI")
            ) && args.len() == 1
            {
                push_unique_java_type(
                    thrown,
                    JavaType::Named {
                        name: "URISyntaxException".to_string(),
                        args: Vec::new(),
                    },
                );
            }
            for arg in args {
                collect_thrown_exception_types_from_expr(arg, exception_env, facts, thrown);
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_thrown_exception_types_from_expr(expr, exception_env, facts, thrown);
        }
        Expr::Lambda { .. } => {}
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::MethodReference { .. }
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn collect_thrown_exception_types_from_lambda_body(
    body: &LambdaBody,
    exception_env: &mut HashMap<String, JavaType>,
    facts: &FactSet,
    thrown: &mut Vec<JavaType>,
) {
    match body {
        LambdaBody::Expr(expr) => {
            collect_thrown_exception_types_from_expr(expr, exception_env, facts, thrown);
        }
        LambdaBody::Block(block) => {
            collect_thrown_exception_types_from_block(block, exception_env, facts, thrown);
        }
    }
}

fn call_captures_lambda_for_deferred_throw_projection(name: &str) -> bool {
    matches!(name, "submit" | "execute" | "supplyAsync" | "runAsync")
}

fn push_uncaught_after_catches(
    try_thrown: &mut Vec<JavaType>,
    catches: &[CatchClause],
    facts: &FactSet,
    out: &mut Vec<JavaType>,
) {
    let hierarchy_index = facts.hierarchy_lookup_index();
    for thrown_ty in try_thrown.drain(..) {
        let caught = java_type_name(&thrown_ty).is_some_and(|thrown_name| {
            catches.iter().any(|catch| {
                catch.types.iter().any(|catch_ty| {
                    exception_assignable_to_catch(thrown_name, catch_ty, facts, &hierarchy_index)
                })
            })
        });
        if !caught {
            push_unique_java_type(out, thrown_ty);
        }
    }
}

fn thrown_exception_type_from_expr(
    expr: &Expr,
    exception_env: &HashMap<String, JavaType>,
) -> Option<JavaType> {
    match expr {
        Expr::New { ty, .. } => Some(ty.clone()),
        Expr::Var(name) => exception_env.get(name).cloned(),
        _ => None,
    }
}

fn push_unique_java_type(types: &mut Vec<JavaType>, ty: JavaType) {
    if !types.contains(&ty) {
        types.push(ty);
    }
}

fn collect_type_recursion_facts(ty: &TypeDecl, call_graph: &usage::CallGraph, facts: &mut FactSet) {
    let method_names = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(method.name.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_recursion_facts(nested, call_graph, facts);
        }
    }
    collect_method_recursion_facts_for_type(ty, call_graph, facts, method_names);
    collect_constructor_recursion_facts_for_type(ty, call_graph, facts);
}

fn collect_method_recursion_facts_for_type(
    ty: &TypeDecl,
    call_graph: &usage::CallGraph,
    facts: &mut FactSet,
    method_names: HashSet<String>,
) {
    let edges = same_type_method_call_graph_edges(&ty.name, call_graph);
    let mut successors_by_method = HashMap::<String, Vec<String>>::new();
    for (caller, callee) in &edges {
        successors_by_method
            .entry(caller.clone())
            .or_default()
            .push(callee.clone());
    }
    let scc = strongly_connected_components(method_names.iter().cloned(), |method| {
        successors_by_method
            .get(method)
            .cloned()
            .unwrap_or_default()
    });

    let mut edges = edges.into_iter().collect::<Vec<_>>();
    edges.sort();
    for (caller, callee) in edges {
        let kind = if caller == callee {
            Some(RecursionKind::Direct)
        } else if scc.component_by_node.get(&caller) == scc.component_by_node.get(&callee) {
            Some(RecursionKind::Mutual)
        } else {
            None
        };
        if let Some(kind) = kind {
            facts.recursion_facts.push(RecursionFact {
                method: MethodKey {
                    type_name: ty.name.clone(),
                    method_name: caller,
                },
                calls: MethodKey {
                    type_name: ty.name.clone(),
                    method_name: callee,
                },
                kind,
                provenance: FactProvenance::ParsedSource,
            });
        }
    }
}

fn collect_constructor_recursion_facts_for_type(
    ty: &TypeDecl,
    call_graph: &usage::CallGraph,
    facts: &mut FactSet,
) {
    let constructor_names = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(constructor) => Some(
                constructor
                    .rust_name
                    .clone()
                    .unwrap_or_else(|| "new".to_string()),
            ),
            _ => None,
        })
        .collect::<HashSet<_>>();
    if constructor_names.is_empty() {
        return;
    }

    let edges = same_type_constructor_call_graph_edges(&ty.name, call_graph);
    let mut successors_by_constructor = HashMap::<String, Vec<String>>::new();
    for (caller, callee) in &edges {
        successors_by_constructor
            .entry(caller.clone())
            .or_default()
            .push(callee.clone());
    }
    let scc = strongly_connected_components(constructor_names.iter().cloned(), |constructor| {
        successors_by_constructor
            .get(constructor)
            .cloned()
            .unwrap_or_default()
    });

    let mut edges = edges.into_iter().collect::<Vec<_>>();
    edges.sort();
    for (caller, callee) in edges {
        let kind = if caller == callee {
            Some(RecursionKind::Direct)
        } else if scc.component_by_node.get(&caller) == scc.component_by_node.get(&callee) {
            Some(RecursionKind::Mutual)
        } else {
            None
        };
        if let Some(kind) = kind {
            facts
                .constructor_recursion_facts
                .push(ConstructorRecursionFact {
                    constructor: ConstructorKey {
                        type_name: ty.name.clone(),
                        rust_name: caller,
                    },
                    calls: ConstructorKey {
                        type_name: ty.name.clone(),
                        rust_name: callee,
                    },
                    kind,
                    provenance: FactProvenance::ParsedSource,
                });
        }
    }
}

fn same_type_method_call_graph_edges(
    type_name: &str,
    call_graph: &usage::CallGraph,
) -> HashSet<(String, String)> {
    call_graph
        .edges
        .iter()
        .filter(|edge| edge.kind == CallGraphEdgeKind::DirectMethodCall)
        .filter_map(|edge| {
            let CallableKey::Method {
                type_name: caller_type,
                method_name: caller_method,
            } = &edge.caller
            else {
                return None;
            };
            let CallableKey::Method {
                type_name: callee_type,
                method_name: callee_method,
            } = &edge.callee
            else {
                return None;
            };
            if caller_type == type_name && callee_type == type_name {
                Some((caller_method.clone(), callee_method.clone()))
            } else {
                None
            }
        })
        .collect()
}

fn same_type_constructor_call_graph_edges(
    type_name: &str,
    call_graph: &usage::CallGraph,
) -> HashSet<(String, String)> {
    call_graph
        .edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.kind,
                CallGraphEdgeKind::ConstructorCall | CallGraphEdgeKind::ConstructorDelegationThis
            )
        })
        .filter_map(|edge| {
            let CallableKey::Constructor {
                type_name: caller_type,
                rust_name: caller_constructor,
            } = &edge.caller
            else {
                return None;
            };
            let CallableKey::Constructor {
                type_name: callee_type,
                rust_name: callee_constructor,
            } = &edge.callee
            else {
                return None;
            };
            if caller_type == type_name && callee_type == type_name {
                Some((caller_constructor.clone(), callee_constructor.clone()))
            } else {
                None
            }
        })
        .collect()
}

fn collect_dispatch_requirement_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_dispatch_requirement_facts(ty, facts);
    }
}

fn collect_type_dispatch_requirement_facts(ty: &TypeDecl, facts: &mut FactSet) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_dispatch_requirement_facts_from_expr(initializer, facts);
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_dispatch_requirement_facts_from_block(body, facts);
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_dispatch_requirement_facts_from_block(body, facts);
                }
            }
            Member::Type(nested) => collect_type_dispatch_requirement_facts(nested, facts),
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_dispatch_requirement_facts_from_block(block: &Block, facts: &mut FactSet) {
    for stmt in &block.statements {
        collect_dispatch_requirement_facts_from_stmt(stmt, facts);
    }
}

fn collect_dispatch_requirement_facts_from_stmts(stmts: &[Stmt], facts: &mut FactSet) {
    for stmt in stmts {
        collect_dispatch_requirement_facts_from_stmt(stmt, facts);
    }
}

fn collect_dispatch_requirement_facts_from_stmt(stmt: &Stmt, facts: &mut FactSet) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_dispatch_requirement_facts_from_expr(initializer, facts);
            }
        }
        Stmt::Assign { target, value } => {
            collect_dispatch_requirement_facts_from_expr(target, facts);
            collect_dispatch_requirement_facts_from_expr(value, facts);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_dispatch_requirement_facts_from_expr(expr, facts);
        }
        Stmt::Return(None) => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_dispatch_requirement_facts_from_expr(condition, facts);
            collect_dispatch_requirement_facts_from_block(then_block, facts);
            if let Some(else_block) = else_block {
                collect_dispatch_requirement_facts_from_block(else_block, facts);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_dispatch_requirement_facts_from_expr(condition, facts);
            collect_dispatch_requirement_facts_from_block(body, facts);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            collect_dispatch_requirement_facts_from_stmts(init, facts);
            if let Some(condition) = condition {
                collect_dispatch_requirement_facts_from_expr(condition, facts);
            }
            for expr in update {
                collect_dispatch_requirement_facts_from_expr(expr, facts);
            }
            collect_dispatch_requirement_facts_from_block(body, facts);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_dispatch_requirement_facts_from_expr(iterable, facts);
            collect_dispatch_requirement_facts_from_block(body, facts);
        }
        Stmt::Switch { selector, cases } => {
            collect_dispatch_requirement_facts_from_expr(selector, facts);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_dispatch_requirement_facts_from_expr(expr, facts);
                    }
                }
                collect_dispatch_requirement_facts_from_stmts(&case.statements, facts);
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            collect_dispatch_requirement_facts_from_stmts(resources, facts);
            collect_dispatch_requirement_facts_from_block(body, facts);
            for catch in catches {
                collect_dispatch_requirement_facts_from_block(&catch.body, facts);
            }
            if let Some(finally_block) = finally_block {
                collect_dispatch_requirement_facts_from_block(finally_block, facts);
            }
        }
        Stmt::LocalType(ty) => collect_type_dispatch_requirement_facts(ty, facts),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_dispatch_requirement_facts_from_expr(arg, facts);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_dispatch_requirement_facts_from_block(block, facts)
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_dispatch_requirement_facts_from_expr(expr: &Expr, facts: &mut FactSet) {
    match expr {
        Expr::Cast { ty, expr } => {
            push_dispatch_requirement_fact(facts, ty, DispatchRequirementKind::NeedsDowncast);
            collect_dispatch_requirement_facts_from_expr(expr, facts);
        }
        Expr::InstanceOf { expr, ty } => {
            push_dispatch_requirement_fact(facts, ty, DispatchRequirementKind::NeedsInstanceof);
            collect_dispatch_requirement_facts_from_expr(expr, facts);
        }
        Expr::FieldAccess(receiver, _) => {
            collect_dispatch_requirement_facts_from_expr(receiver, facts);
        }
        Expr::ArrayAccess(array, index) => {
            collect_dispatch_requirement_facts_from_expr(array, facts);
            collect_dispatch_requirement_facts_from_expr(index, facts);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_dispatch_requirement_facts_from_expr(expr, facts);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_dispatch_requirement_facts_from_expr(expr, facts);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_dispatch_requirement_facts_from_expr(expr, facts);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_dispatch_requirement_facts_from_expr(left, facts);
            collect_dispatch_requirement_facts_from_expr(right, facts);
        }
        Expr::Unary { expr, .. } => collect_dispatch_requirement_facts_from_expr(expr, facts),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_dispatch_requirement_facts_from_expr(condition, facts);
            collect_dispatch_requirement_facts_from_expr(then_expr, facts);
            collect_dispatch_requirement_facts_from_expr(else_expr, facts);
        }
        Expr::Switch { selector, cases } => {
            collect_dispatch_requirement_facts_from_expr(selector, facts);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_dispatch_requirement_facts_from_expr(expr, facts);
                    }
                }
                collect_dispatch_requirement_facts_from_expr(&case.value, facts);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_dispatch_requirement_facts_from_expr(receiver, facts);
            }
            for arg in args {
                collect_dispatch_requirement_facts_from_expr(arg, facts);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_dispatch_requirement_facts_from_expr(arg, facts);
            }
            if let Some(body) = body {
                for member in body {
                    if let Member::Method(method) = member
                        && let Some(body) = &method.body
                    {
                        collect_dispatch_requirement_facts_from_block(body, facts);
                    }
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                collect_dispatch_requirement_facts_from_expr(expr, facts);
            }
            crate::java_ir::LambdaBody::Block(block) => {
                collect_dispatch_requirement_facts_from_block(block, facts);
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

fn push_dispatch_requirement_fact(
    facts: &mut FactSet,
    java_ty: &JavaType,
    requirement: DispatchRequirementKind,
) {
    let Some(type_name) = dispatch_requirement_type_name(java_ty) else {
        return;
    };
    if facts.dispatch_requirement_facts.iter().any(|fact| {
        fact.type_name == type_name
            && fact.requirement == requirement
            && fact.provenance == FactProvenance::ParsedSource
    }) {
        return;
    }
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: type_name.to_string(),
            requirement,
            provenance: FactProvenance::ParsedSource,
        });
}

fn dispatch_requirement_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::String => Some("String"),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name.as_str()),
        _ => None,
    }
}

fn collect_control_flow_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_control_flow_facts(ty, facts);
    }
}

fn collect_type_control_flow_facts(ty: &TypeDecl, facts: &mut FactSet) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    if block_has_assignment_in_condition(body) {
                        facts.control_flow_facts.push(ControlFlowFact {
                            method: MethodKey {
                                type_name: ty.name.clone(),
                                method_name: method.name.clone(),
                            },
                            kind: ControlFlowFactKind::AssignmentInCondition,
                            provenance: FactProvenance::ParsedSource,
                        });
                    }
                    if block_has_tail_recursive_self_call(body, &method.name) {
                        facts.control_flow_facts.push(ControlFlowFact {
                            method: MethodKey {
                                type_name: ty.name.clone(),
                                method_name: method.name.clone(),
                            },
                            kind: ControlFlowFactKind::TailRecursiveSelfCall,
                            provenance: FactProvenance::ParsedSource,
                        });
                    }
                }
            }
            Member::Type(nested) => collect_type_control_flow_facts(nested, facts),
            _ => {}
        }
    }
}

fn collect_control_flow_plans(program: &Program) -> Vec<MethodControlFlowPlan> {
    let mut plans = Vec::new();
    for ty in &program.types {
        collect_type_control_flow_plans(ty, &mut plans);
    }
    plans
}

fn collect_type_control_flow_plans(ty: &TypeDecl, plans: &mut Vec<MethodControlFlowPlan>) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let mut rewrites = Vec::new();
                    collect_assignment_in_condition_plans(body, &mut rewrites);
                    if let Some(tail_plan) = tail_recursive_loop_plan(body, &method.name) {
                        rewrites.push(ControlFlowRewritePlan::TailRecursiveLoop(tail_plan));
                    }
                    if !rewrites.is_empty() {
                        plans.push(MethodControlFlowPlan {
                            method: MethodKey {
                                type_name: ty.name.clone(),
                                method_name: method.name.clone(),
                            },
                            rewrites,
                        });
                    }
                }
            }
            Member::Type(nested) => collect_type_control_flow_plans(nested, plans),
            _ => {}
        }
    }
}

fn collect_recursion_fallback_diagnostics(
    program: &Program,
    facts: &FactSet,
    control_flow_plans: &[MethodControlFlowPlan],
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    for ty in &program.types {
        collect_type_recursion_fallback_diagnostics(ty, facts, control_flow_plans, diagnostics);
    }
}

fn collect_type_recursion_fallback_diagnostics(
    ty: &TypeDecl,
    facts: &FactSet,
    control_flow_plans: &[MethodControlFlowPlan],
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let env = collect_external_api_type_env(
                        ty,
                        &method.type_params,
                        &method.type_param_bounds,
                        &method.params,
                    );
                    let mut method_body_env = env.clone();
                    collect_block_declared_locals(body, &mut method_body_env);
                    collect_method_recursion_fallback_diagnostics(
                        ty,
                        method,
                        body,
                        facts,
                        control_flow_plans,
                        diagnostics,
                        &method_body_env,
                    );
                    collect_method_external_boundary_diagnostics(
                        ty,
                        method,
                        body,
                        facts,
                        diagnostics,
                        &method_body_env,
                    );
                }
            }
            Member::Type(nested) => collect_type_recursion_fallback_diagnostics(
                nested,
                facts,
                control_flow_plans,
                diagnostics,
            ),
            _ => {}
        }
    }
}

fn collect_method_recursion_fallback_diagnostics(
    ty: &TypeDecl,
    method: &MethodDecl,
    body: &Block,
    facts: &FactSet,
    control_flow_plans: &[MethodControlFlowPlan],
    diagnostics: &mut Vec<SemanticDiagnostic>,
    env: &TypeEnv,
) {
    let has_safe_tail_plan =
        method_has_safe_tail_loop_plan(control_flow_plans, &ty.name, &method.name);
    if has_safe_tail_plan && !method.throws.is_empty() {
        push_semantic_diagnostic_once(
            diagnostics,
            "recursion_tco_fallback",
            format!(
                "{}::{} has a safe tail-recursive shape, but it is kept as ordinary recursion because exception lowering wraps the method in Result",
                ty.name, method.name
            ),
        );
    }

    let recursion = facts.recursion_facts_for(&ty.name, &method.name);
    if recursion
        .iter()
        .any(|fact| fact.kind == RecursionKind::Direct)
        && !has_safe_tail_plan
    {
        push_semantic_diagnostic_once(
            diagnostics,
            "recursion_tco_fallback",
            format!(
                "{}::{} is directly recursive but does not match the safe accumulator loop shape; it remains ordinary Rust recursion",
                ty.name, method.name
            ),
        );
    }

    if recursion
        .iter()
        .any(|fact| fact.kind == RecursionKind::Mutual)
        && block_tail_returns_any_method(
            body,
            &recursion
                .iter()
                .filter(|fact| fact.kind == RecursionKind::Mutual)
                .map(|fact| fact.calls.method_name.as_str())
                .collect::<HashSet<_>>(),
        )
    {
        push_semantic_diagnostic_once(
            diagnostics,
            "recursion_tco_fallback",
            format!(
                "{}::{} participates in mutual recursion; tail-style calls remain ordinary Rust calls because no single-method loop rewrite is sound",
                ty.name, method.name
            ),
        );
    }

    if block_has_receiver_recursive_call(body, &method.name, facts, env, &ty.name) {
        push_semantic_diagnostic_once(
            diagnostics,
            "recursion_tco_fallback",
            format!(
                "{}::{} calls {} through a receiver that is not proven to be this; no self-tail loop rewrite is applied",
                ty.name, method.name, method.name
            ),
        );
    }
}

fn collect_block_declared_locals(body: &Block, env: &mut TypeEnv) {
    for stmt in &body.statements {
        collect_stmt_declared_locals(stmt, env);
    }
}

fn collect_stmt_declared_locals(stmt: &Stmt, env: &mut TypeEnv) {
    match stmt {
        Stmt::VarDecl { ty, name, .. } => {
            env.insert_local(name.clone(), ty.clone());
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_block_declared_locals(then_block, env);
            if let Some(else_block) = else_block {
                collect_block_declared_locals(else_block, env);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => collect_block_declared_locals(body, env),
        Stmt::For { init, body, .. } => {
            for init in init {
                collect_stmt_declared_locals(init, env);
            }
            collect_block_declared_locals(body, env);
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            body,
            ..
        } => {
            env.insert_local(var_name.clone(), var_type.clone());
            collect_block_declared_locals(body, env);
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
            ..
        } => {
            for resource in resources {
                collect_stmt_declared_locals(resource, env);
            }
            collect_block_declared_locals(body, env);
            for catch in catches {
                env.insert_local(
                    catch.name.clone(),
                    catch
                        .types
                        .first()
                        .cloned()
                        .unwrap_or_else(|| JavaType::Unknown("exception".to_string())),
                );
                collect_block_declared_locals(&catch.body, env);
            }
            if let Some(finally_block) = finally_block {
                collect_block_declared_locals(finally_block, env);
            }
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_declared_locals(stmt, env);
                }
            }
        }
        _ => {}
    }
}

fn collect_method_external_boundary_diagnostics(
    ty: &TypeDecl,
    method: &MethodDecl,
    body: &Block,
    facts: &FactSet,
    diagnostics: &mut Vec<SemanticDiagnostic>,
    env: &TypeEnv,
) {
    walk_block_exprs(body, &mut |expr| {
        let Expr::Call {
            receiver,
            name,
            args,
            ..
        } = expr
        else {
            return;
        };
        let Some(pack_name) = external_boundary_pack_for_call_receiver(
            receiver.as_deref(),
            name,
            args.len(),
            facts,
            env,
            &ty.name,
        ) else {
            return;
        };
        push_semantic_diagnostic_once(
            diagnostics,
            "external_pack_boundary_call",
            format!(
                "{}::{} crosses external {} boundary at {}({} args)",
                ty.name,
                method.name,
                pack_name,
                name,
                args.len()
            ),
        );
    });
}

fn external_boundary_pack_for_call_receiver(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    env: &TypeEnv,
    current_type: &str,
) -> Option<&'static str> {
    if let Some(pack_name) = receiver.and_then(unmodeled_static_external_boundary_pack) {
        return Some(pack_name);
    }
    let Some(Some(receiver_type)) = crate::call_plan::external_call_receiver_type_key(
        receiver,
        name,
        arg_count,
        facts,
        env,
        Some(current_type),
    ) else {
        return None;
    };
    facts
        .external_library_pack_for_type(&receiver_type)
        .cloned()
        .or_else(|| crate::external_packs::external_library_pack_for_type(&receiver_type))
        .and_then(|pack| match pack.pack_name.as_str() {
            "org.apache.spark" => Some("org.apache.spark"),
            "org.disq_bio.disq" => Some("org.disq_bio.disq"),
            _ => None,
        })
}

fn unmodeled_static_external_boundary_pack(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::Var(name) if is_unmodeled_disq_static_owner(name) => Some("org.disq_bio.disq"),
        Expr::FieldAccess(receiver, _) => unmodeled_static_external_boundary_pack(receiver),
        Expr::Call {
            receiver: Some(receiver),
            ..
        } => unmodeled_static_external_boundary_pack(receiver),
        _ => None,
    }
}

fn is_unmodeled_disq_static_owner(name: &str) -> bool {
    matches!(
        name,
        "htsjdk_variants_rdd_storage" | "tabix_index_write_option"
    )
}

fn method_has_safe_tail_loop_plan(
    control_flow_plans: &[MethodControlFlowPlan],
    type_name: &str,
    method_name: &str,
) -> bool {
    control_flow_plans.iter().any(|plan| {
        plan.method.type_name == type_name
            && plan.method.method_name == method_name
            && plan.rewrites.iter().any(|rewrite| {
                matches!(
                    rewrite,
                    ControlFlowRewritePlan::TailRecursiveLoop(TailRecursiveLoopPlan {
                        normalized_shape:
                            TailRecursiveNormalizedShape::LoopWithParameterReassignment,
                        ..
                    })
                )
            })
    })
}

fn block_tail_returns_any_method(body: &Block, method_names: &HashSet<&str>) -> bool {
    body.statements
        .iter()
        .any(|stmt| stmt_tail_returns_any_method(stmt, method_names))
}

fn stmt_tail_returns_any_method(stmt: &Stmt, method_names: &HashSet<&str>) -> bool {
    match stmt {
        Stmt::Return(Some(Expr::Call { name, .. })) => method_names.contains(name.as_str()),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_tail_returns_any_method(then_block, method_names)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_tail_returns_any_method(block, method_names))
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_tail_returns_any_method(body, method_names)
        }
        _ => false,
    }
}

fn block_has_receiver_recursive_call(
    body: &Block,
    method_name: &str,
    facts: &FactSet,
    env: &TypeEnv,
    current_type: &str,
) -> bool {
    body.statements
        .iter()
        .any(|stmt| stmt_has_receiver_recursive_call(stmt, method_name, facts, env, current_type))
}

fn stmt_has_receiver_recursive_call(
    stmt: &Stmt,
    method_name: &str,
    facts: &FactSet,
    env: &TypeEnv,
    current_type: &str,
) -> bool {
    let mut found = false;
    walk_stmt_exprs(stmt, &mut |expr| {
        if expr_has_receiver_recursive_call(expr, method_name, facts, env, current_type) {
            found = true;
        }
    });
    found
}

fn expr_has_receiver_recursive_call(
    expr: &Expr,
    method_name: &str,
    facts: &FactSet,
    env: &TypeEnv,
    current_type: &str,
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
    if name != method_name || matches!(receiver.as_ref(), Expr::This) {
        return false;
    }
    !receiver_call_is_external_pack_boundary(receiver, name, args.len(), facts, env, current_type)
}

fn receiver_call_is_external_pack_boundary(
    receiver: &Expr,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    env: &TypeEnv,
    current_type: &str,
) -> bool {
    external_boundary_pack_for_call_receiver(
        Some(receiver),
        name,
        arg_count,
        facts,
        env,
        current_type,
    )
    .is_some()
}

fn collect_assignment_in_condition_plans(body: &Block, rewrites: &mut Vec<ControlFlowRewritePlan>) {
    for stmt in &body.statements {
        match stmt {
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                if let Some(plan) = assignment_in_condition_plan(condition) {
                    rewrites.push(ControlFlowRewritePlan::AssignmentInCondition(plan));
                }
                collect_assignment_in_condition_plans(then_block, rewrites);
                if let Some(else_block) = else_block {
                    collect_assignment_in_condition_plans(else_block, rewrites);
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
                if let Some(plan) = assignment_in_condition_plan(condition) {
                    rewrites.push(ControlFlowRewritePlan::AssignmentInCondition(plan));
                }
                collect_assignment_in_condition_plans(body, rewrites);
            }
            Stmt::For {
                condition, body, ..
            } => {
                if let Some(condition) = condition
                    && let Some(plan) = assignment_in_condition_plan(condition)
                {
                    rewrites.push(ControlFlowRewritePlan::AssignmentInCondition(plan));
                }
                collect_assignment_in_condition_plans(body, rewrites);
            }
            Stmt::EnhancedFor { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_assignment_in_condition_plans(body, rewrites);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_assignment_in_condition_plans(
                        &Block {
                            statements: case.statements.clone(),
                        },
                        rewrites,
                    );
                }
            }
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                collect_assignment_in_condition_plans(body, rewrites);
                for catch in catches {
                    collect_assignment_in_condition_plans(&catch.body, rewrites);
                }
                if let Some(finally_block) = finally_block {
                    collect_assignment_in_condition_plans(finally_block, rewrites);
                }
            }
            _ => {}
        }
    }
}

fn assignment_in_condition_plan(condition: &Expr) -> Option<AssignmentInConditionPlan> {
    let Expr::Binary { op, left, right } = condition else {
        return None;
    };
    let null_test = match op {
        BinaryOp::Ne => NullTestKind::NotNull,
        BinaryOp::Eq => NullTestKind::IsNull,
        _ => return None,
    };
    assignment_fragment_plan(left, null_test).or_else(|| assignment_fragment_plan(right, null_test))
}

fn assignment_fragment_plan(
    expr: &Expr,
    null_test: NullTestKind,
) -> Option<AssignmentInConditionPlan> {
    let Expr::Binary { op, left, right } = expr else {
        return None;
    };
    if *op != BinaryOp::Assign {
        return None;
    }
    let Expr::Var(local_name) = left.as_ref() else {
        return None;
    };
    let Expr::Call { name, .. } = right.as_ref() else {
        return None;
    };
    Some(AssignmentInConditionPlan {
        local_name: local_name.clone(),
        source_call: name.clone(),
        null_test,
        normalized_shape: AssignmentConditionNormalizedShape::LoopAssignThenBreak,
    })
}

fn tail_recursive_loop_plan(body: &Block, method_name: &str) -> Option<TailRecursiveLoopPlan> {
    let [
        Stmt::If {
            then_block,
            else_block: None,
            ..
        },
        Stmt::Return(Some(Expr::Call { name, args, .. })),
    ] = body.statements.as_slice()
    else {
        return None;
    };
    if name != method_name || !block_is_single_value_return(then_block) {
        return None;
    }
    Some(TailRecursiveLoopPlan {
        method_name: name.clone(),
        arg_count: args.len(),
        normalized_shape: TailRecursiveNormalizedShape::LoopWithParameterReassignment,
    })
}

fn block_is_single_value_return(block: &Block) -> bool {
    matches!(block.statements.as_slice(), [Stmt::Return(Some(_))])
}

fn collect_exception_facts(
    program: &Program,
    facts: &mut FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let mut exception_names = HashSet::new();
    for ty in &program.types {
        collect_exception_candidate_names(
            ty,
            facts,
            hierarchy_index,
            &mut exception_names,
            diagnostics,
        );
    }
    for type_name in exception_names {
        facts.exception_facts.push(ExceptionFact {
            kind: classify_exception_type(&type_name, facts, hierarchy_index, diagnostics),
            type_name,
            provenance: FactProvenance::ParsedSource,
        });
    }
}

fn collect_exception_candidate_names(
    ty: &TypeDecl,
    facts: &FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
    exception_names: &mut HashSet<String>,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    if ty.extends.as_deref().is_some_and(is_exception_root_name)
        || type_reaches_exception(&ty.name, facts, hierarchy_index, diagnostics)
    {
        exception_names.insert(ty.name.clone());
    }
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                for throws in &method.throws {
                    if let Some(name) = java_type_name(throws) {
                        exception_names.insert(name.to_string());
                    }
                }
                if let Some(body) = &method.body {
                    collect_exception_names_from_block(body, exception_names);
                }
            }
            Member::Constructor(ctor) => {
                for throws in &ctor.throws {
                    if let Some(name) = java_type_name(throws) {
                        exception_names.insert(name.to_string());
                    }
                }
                if let Some(body) = &ctor.body {
                    collect_exception_names_from_block(body, exception_names);
                }
            }
            Member::Type(nested) => collect_exception_candidate_names(
                nested,
                facts,
                hierarchy_index,
                exception_names,
                diagnostics,
            ),
            _ => {}
        }
    }
}

fn collect_exception_names_from_block(block: &Block, exception_names: &mut HashSet<String>) {
    for stmt in &block.statements {
        match stmt {
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                collect_exception_names_from_block(body, exception_names);
                for catch in catches {
                    for catch_ty in &catch.types {
                        if let Some(name) = java_type_name(catch_ty) {
                            exception_names.insert(name.to_string());
                        }
                    }
                    collect_exception_names_from_block(&catch.body, exception_names);
                }
                if let Some(finally_block) = finally_block {
                    collect_exception_names_from_block(finally_block, exception_names);
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_exception_names_from_block(then_block, exception_names);
                if let Some(else_block) = else_block {
                    collect_exception_names_from_block(else_block, exception_names);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::For { body, .. }
            | Stmt::EnhancedFor { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_exception_names_from_block(body, exception_names)
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    for stmt in &case.statements {
                        collect_exception_names_from_stmt(stmt, exception_names);
                    }
                }
            }
            _ => collect_exception_names_from_stmt(stmt, exception_names),
        }
    }
}

fn collect_exception_names_from_stmt(stmt: &Stmt, exception_names: &mut HashSet<String>) {
    if let Stmt::Throw(Expr::New { ty, .. }) = stmt
        && let Some(name) = java_type_name(ty)
    {
        exception_names.insert(name.to_string());
    }
}

fn collect_catch_facts(
    program: &Program,
    facts: &mut FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
) {
    for ty in &program.types {
        collect_type_catch_facts(ty, facts, hierarchy_index);
    }
}

fn collect_type_catch_facts(
    ty: &TypeDecl,
    facts: &mut FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_method_catch_facts(ty, method, body, facts, hierarchy_index);
                }
            }
            Member::Type(nested) => collect_type_catch_facts(nested, facts, hierarchy_index),
            _ => {}
        }
    }
}

fn collect_method_catch_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    body: &Block,
    facts: &mut FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
) {
    let mut clause_index = 0usize;
    collect_catch_facts_from_block(
        body,
        &MethodKey {
            type_name: ty.name.clone(),
            method_name: method.name.clone(),
        },
        &mut clause_index,
        facts,
        hierarchy_index,
    );
}

fn collect_catch_facts_from_block(
    body: &Block,
    method: &MethodKey,
    clause_index: &mut usize,
    facts: &mut FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
) {
    for stmt in &body.statements {
        match stmt {
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                collect_catch_facts_from_block(body, method, clause_index, facts, hierarchy_index);
                for catch in catches {
                    facts.catch_facts.push(CatchFact {
                        method: method.clone(),
                        clause_index: *clause_index,
                        assignable_from: catch_assignable_from_types(
                            &catch.types,
                            facts,
                            hierarchy_index,
                        ),
                        types: catch.types.clone(),
                        provenance: FactProvenance::ParsedSource,
                    });
                    *clause_index += 1;
                    collect_catch_facts_from_block(
                        &catch.body,
                        method,
                        clause_index,
                        facts,
                        hierarchy_index,
                    );
                }
                if let Some(finally_block) = finally_block {
                    collect_catch_facts_from_block(
                        finally_block,
                        method,
                        clause_index,
                        facts,
                        hierarchy_index,
                    );
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_catch_facts_from_block(
                    then_block,
                    method,
                    clause_index,
                    facts,
                    hierarchy_index,
                );
                if let Some(else_block) = else_block {
                    collect_catch_facts_from_block(
                        else_block,
                        method,
                        clause_index,
                        facts,
                        hierarchy_index,
                    );
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::For { body, .. }
            | Stmt::EnhancedFor { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_catch_facts_from_block(body, method, clause_index, facts, hierarchy_index)
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_catch_facts_from_stmts(
                        &case.statements,
                        method,
                        clause_index,
                        facts,
                        hierarchy_index,
                    );
                }
            }
            _ => {}
        }
    }
}

fn collect_catch_facts_from_stmts(
    statements: &[Stmt],
    method: &MethodKey,
    clause_index: &mut usize,
    facts: &mut FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
) {
    for stmt in statements {
        if let Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } = stmt {
            collect_catch_facts_from_block(block, method, clause_index, facts, hierarchy_index);
        }
    }
}

fn catch_assignable_from_types(
    catch_types: &[JavaType],
    facts: &FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
) -> Vec<JavaType> {
    let mut assignable = facts
        .exception_facts
        .iter()
        .filter_map(|exception| {
            catch_types
                .iter()
                .any(|catch_ty| {
                    exception_assignable_to_catch(
                        &exception.type_name,
                        catch_ty,
                        facts,
                        hierarchy_index,
                    )
                })
                .then_some(JavaType::Named {
                    name: exception.type_name.clone(),
                    args: Vec::new(),
                })
        })
        .collect::<Vec<_>>();
    assignable.sort_by(|left, right| {
        exception_assignable_alias_key(left)
            .cmp(&exception_assignable_alias_key(right))
            .then_with(|| {
                java_type_sort_key(left)
                    .len()
                    .cmp(&java_type_sort_key(right).len())
            })
            .then_with(|| java_type_sort_key(left).cmp(&java_type_sort_key(right)))
    });
    assignable.dedup_by(|left, right| {
        exception_assignable_alias_key(left) == exception_assignable_alias_key(right)
    });
    assignable
}

fn exception_assignable_to_catch(
    exception_name: &str,
    catch_ty: &JavaType,
    facts: &FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
) -> bool {
    let Some(catch_name) = java_type_name(catch_ty) else {
        return false;
    };
    crate::facts::external_type_name_matches(exception_name, catch_name)
        || type_reaches_without_diagnostics(exception_name, catch_name, facts, hierarchy_index)
}

fn java_type_sort_key(java_ty: &JavaType) -> String {
    java_type_name(java_ty).unwrap_or("").to_string()
}

fn exception_assignable_alias_key(java_ty: &JavaType) -> String {
    java_type_name(java_ty)
        .and_then(|name| name.rsplit('.').next())
        .unwrap_or("")
        .to_string()
}

fn collect_type_field_nullability_facts(
    ty: &TypeDecl,
    facts: &mut FactSet,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) -> usize {
    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if is_reference_type(&field.ty) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    let constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(constructor) => Some(constructor),
            _ => None,
        })
        .collect::<Vec<_>>();
    let helper_assignments = collect_helper_field_assignments(ty, facts);
    let constructor_assignments = collect_constructor_field_assignments(
        ty,
        &constructors,
        &helper_assignments,
        facts,
        diagnostics,
    );
    let mut max_iterations = constructor_assignments.iterations;
    let method_bodies = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => method
                .body
                .as_ref()
                .map(|body| (body, local_shadows_from_params(&method.params))),
            _ => None,
        })
        .collect::<Vec<_>>();

    for field in fields {
        facts.field_nullability_facts.push(field_nullability_fact(
            ty,
            field,
            &constructors,
            &constructor_assignments.assignments,
            &method_bodies,
            &helper_assignments,
            facts,
        ));
    }

    for member in &ty.members {
        if let Member::Type(nested) = member {
            max_iterations = max_iterations.max(collect_type_field_nullability_facts(
                nested,
                facts,
                diagnostics,
            ));
        }
    }
    max_iterations
}

fn field_nullability_fact(
    ty: &TypeDecl,
    field: &FieldDecl,
    constructors: &[&ConstructorDecl],
    constructor_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    method_bodies: &[(&Block, HashSet<String>)],
    helper_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    facts: &FactSet,
) -> FieldNullabilityFact {
    let field_key = FieldKey {
        type_name: ty.name.clone(),
        field_name: field.name.clone(),
    };
    let initializer_non_null = field
        .initializer
        .as_ref()
        .is_some_and(expr_observed_non_null);
    let final_initialized_non_null =
        field.modifiers.contains(&Modifier::Final) && initializer_non_null;
    let initializer_null = matches!(field.initializer, Some(Expr::Literal(Literal::Null)));
    let all_constructors_assign_non_null = !constructors.is_empty()
        && constructors.iter().all(|constructor| {
            constructor_assignments
                .get(&constructor_rust_name(constructor))
                .and_then(|assignments| assignments.get(&field.name))
                .is_some_and(|assignment| *assignment == NullabilityState::DefinitelyNonNull)
        });
    let method_assignment_nullability = strongest_assignment(
        method_bodies
            .iter()
            .flat_map(|(body, local_shadows)| {
                block_field_assignments(
                    body,
                    ty,
                    &field.name,
                    local_shadows,
                    helper_assignments,
                    facts,
                )
            })
            .collect(),
    );

    let (state, provenance) = if initializer_null {
        (
            NullabilityState::MaybeNull,
            FieldNullabilityProvenance::FieldInitializer,
        )
    } else if let Some(method_assignment) = method_assignment_nullability
        && method_assignment != NullabilityState::DefinitelyNonNull
    {
        (
            field_state_from_assignment_nullability(method_assignment),
            FieldNullabilityProvenance::MethodAssignment,
        )
    } else if field.modifiers.contains(&Modifier::Static) && initializer_non_null {
        (
            NullabilityState::DefinitelyNonNull,
            FieldNullabilityProvenance::FieldInitializer,
        )
    } else if all_constructors_assign_non_null {
        (
            NullabilityState::DefinitelyNonNull,
            FieldNullabilityProvenance::ConstructorAssignments,
        )
    } else if final_initialized_non_null || (constructors.is_empty() && initializer_non_null) {
        (
            NullabilityState::DefinitelyNonNull,
            FieldNullabilityProvenance::FieldInitializer,
        )
    } else {
        (
            NullabilityState::MaybeNull,
            FieldNullabilityProvenance::ConservativeFallback,
        )
    };

    FieldNullabilityFact {
        field: field_key,
        state,
        provenance,
    }
}

fn field_state_from_assignment_nullability(assignment: AssignmentNullability) -> NullabilityState {
    match assignment {
        NullabilityState::DefinitelyNonNull => NullabilityState::DefinitelyNonNull,
        NullabilityState::DefinitelyNull | NullabilityState::MaybeNull => {
            NullabilityState::MaybeNull
        }
        NullabilityState::Unknown => NullabilityState::Unknown,
    }
}

type AssignmentNullability = NullabilityState;

fn block_field_assignments(
    body: &Block,
    ty: &TypeDecl,
    field_name: &str,
    local_shadows: &HashSet<String>,
    helper_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    facts: &FactSet,
) -> Vec<AssignmentNullability> {
    let mut assignments = Vec::new();
    let mut local_shadows = local_shadows.clone();
    collect_field_assignments_from_stmts(
        &body.statements,
        ty,
        field_name,
        &mut local_shadows,
        helper_assignments,
        facts,
        &mut assignments,
    );
    assignments
}

fn collect_field_assignments_from_stmts(
    statements: &[Stmt],
    ty: &TypeDecl,
    field_name: &str,
    local_shadows: &mut HashSet<String>,
    helper_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    facts: &FactSet,
    assignments: &mut Vec<AssignmentNullability>,
) {
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                name, initializer, ..
            } => {
                if let Some(initializer) = initializer {
                    collect_field_assignments_from_expr(
                        initializer,
                        ty,
                        field_name,
                        local_shadows,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
                local_shadows.insert(name.clone());
            }
            Stmt::Assign { target, value } if assignment_targets_field(target, field_name) => {
                assignments.push(expr_assignment_nullability(value));
            }
            Stmt::Assign { target, value }
                if bare_assignment_targets_unshadowed_field(target, field_name, local_shadows) =>
            {
                assignments.push(bare_field_assignment_nullability(value, local_shadows));
            }
            Stmt::Assign { target, value } => {
                collect_field_assignments_from_expr(
                    target,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
                collect_field_assignments_from_expr(
                    value,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            Stmt::Expr(expr) => {
                collect_field_assignments_from_expr(
                    expr,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                let mut then_shadows = local_shadows.clone();
                collect_field_assignments_from_stmts(
                    &then_block.statements,
                    ty,
                    field_name,
                    &mut then_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
                if let Some(else_block) = else_block {
                    let mut else_shadows = local_shadows.clone();
                    collect_field_assignments_from_stmts(
                        &else_block.statements,
                        ty,
                        field_name,
                        &mut else_shadows,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                let mut body_shadows = local_shadows.clone();
                collect_field_assignments_from_stmts(
                    &body.statements,
                    ty,
                    field_name,
                    &mut body_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            Stmt::EnhancedFor { var_name, body, .. } => {
                let mut body_shadows = local_shadows.clone();
                body_shadows.insert(var_name.clone());
                collect_field_assignments_from_stmts(
                    &body.statements,
                    ty,
                    field_name,
                    &mut body_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            Stmt::For {
                init, update, body, ..
            } => {
                let mut loop_shadows = local_shadows.clone();
                collect_field_assignments_from_stmts(
                    init,
                    ty,
                    field_name,
                    &mut loop_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
                for expr in update {
                    collect_field_assignments_from_expr(
                        expr,
                        ty,
                        field_name,
                        &mut loop_shadows,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
                collect_field_assignments_from_stmts(
                    &body.statements,
                    ty,
                    field_name,
                    &mut loop_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    let mut case_shadows = local_shadows.clone();
                    collect_field_assignments_from_stmts(
                        &case.statements,
                        ty,
                        field_name,
                        &mut case_shadows,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                let mut try_shadows = local_shadows.clone();
                collect_field_assignments_from_stmts(
                    resources,
                    ty,
                    field_name,
                    &mut try_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
                collect_field_assignments_from_stmts(
                    &body.statements,
                    ty,
                    field_name,
                    &mut try_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
                for catch in catches {
                    let mut catch_shadows = local_shadows.clone();
                    catch_shadows.insert(catch.name.clone());
                    collect_field_assignments_from_stmts(
                        &catch.body.statements,
                        ty,
                        field_name,
                        &mut catch_shadows,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
                if let Some(finally_block) = finally_block {
                    let mut finally_shadows = local_shadows.clone();
                    collect_field_assignments_from_stmts(
                        &finally_block.statements,
                        ty,
                        field_name,
                        &mut finally_shadows,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
            }
            _ => {}
        }
    }
}

fn collect_field_assignments_from_expr(
    expr: &Expr,
    ty: &TypeDecl,
    field_name: &str,
    local_shadows: &mut HashSet<String>,
    helper_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    facts: &FactSet,
    assignments: &mut Vec<AssignmentNullability>,
) {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } if call_receiver_is_this(receiver) => {
            if let Some(accessor) = facts.trivial_accessor_for(&ty.name, name)
                && let TrivialAccessorKind::Setter { field, .. } = &accessor.kind
                && field.field_name == field_name
                && let Some(arg) = args.first()
            {
                assignments.push(expr_assignment_nullability(arg));
            }
            if let Some(helper) = helper_assignments.get(name)
                && let Some(assignment) = helper.get(field_name)
            {
                assignments.push(*assignment);
            }
            for arg in args {
                collect_field_assignments_from_expr(
                    arg,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_field_assignments_from_expr(
                    receiver,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            for arg in args {
                collect_field_assignments_from_expr(
                    arg,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_field_assignments_from_expr(
                left,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
            collect_field_assignments_from_expr(
                right,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_field_assignments_from_expr(
                expr,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_field_assignments_from_expr(
                condition,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
            collect_field_assignments_from_expr(
                then_expr,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
            collect_field_assignments_from_expr(
                else_expr,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
        }
        Expr::FieldAccess(receiver, _) => {
            collect_field_assignments_from_expr(
                receiver,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
        }
        Expr::ArrayAccess(array, index) => {
            collect_field_assignments_from_expr(
                array,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
            collect_field_assignments_from_expr(
                index,
                ty,
                field_name,
                local_shadows,
                helper_assignments,
                facts,
                assignments,
            );
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_field_assignments_from_expr(
                    expr,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_field_assignments_from_expr(
                        expr,
                        ty,
                        field_name,
                        local_shadows,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_field_assignments_from_expr(
                    expr,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_field_assignments_from_expr(
                    arg,
                    ty,
                    field_name,
                    local_shadows,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
        }
        _ => {}
    }
}

fn collect_helper_field_assignments(
    ty: &TypeDecl,
    facts: &FactSet,
) -> HashMap<String, HashMap<String, AssignmentNullability>> {
    let mut helpers = HashMap::new();
    for member in &ty.members {
        let Member::Method(method) = member else {
            continue;
        };
        if !method
            .modifiers
            .iter()
            .any(|modifier| matches!(modifier, Modifier::Private))
        {
            continue;
        }
        let Some(body) = &method.body else {
            continue;
        };
        let mut field_assignments = HashMap::new();
        let local_shadows = local_shadows_from_params(&method.params);
        for field in field_types(ty).keys() {
            let assignments =
                block_field_assignments(body, ty, field, &local_shadows, &HashMap::new(), facts);
            if let Some(assignment) = strongest_assignment(assignments) {
                field_assignments.insert(field.clone(), assignment);
            }
        }
        if !field_assignments.is_empty() {
            helpers.insert(method.name.clone(), field_assignments);
        }
    }
    helpers
}

fn collect_constructor_field_assignments(
    ty: &TypeDecl,
    constructors: &[&ConstructorDecl],
    helper_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    facts: &FactSet,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) -> ConstructorAssignmentSaturation {
    let fields = field_types(ty);
    let cyclic_constructors = cyclic_delegating_constructor_names(constructors);
    let mut by_constructor = HashMap::new();
    for constructor in constructors {
        let mut field_assignments = HashMap::new();
        if let Some(body) = &constructor.body
            && !cyclic_constructors.contains(&constructor_rust_name(constructor))
        {
            let local_shadows = local_shadows_from_params(&constructor.params);
            for field in fields.keys() {
                let assignments = block_field_assignments(
                    body,
                    ty,
                    field,
                    &local_shadows,
                    helper_assignments,
                    facts,
                );
                if let Some(assignment) = strongest_assignment(assignments) {
                    field_assignments.insert(field.clone(), assignment);
                }
            }
        }
        by_constructor.insert(constructor_rust_name(constructor), field_assignments);
    }

    let policy = FiniteSaturationPolicy::new(
        constructors.len().saturating_mul(fields.len()),
        ASSIGNMENT_NULLABILITY_FACT_KIND_COUNT,
    );
    let result = policy.saturate(
        by_constructor,
        |by_constructor| {
            let mut changed = false;
            for constructor in constructors {
                let Some(target_name) = delegated_this_constructor_name(constructor) else {
                    continue;
                };
                let delegated_assignments = by_constructor
                    .get(&target_name)
                    .cloned()
                    .unwrap_or_default();
                let current = by_constructor
                    .entry(constructor_rust_name(constructor))
                    .or_default();
                for (field_name, delegated_assignment) in delegated_assignments {
                    let merged = merge_assignment_nullability(
                        current.get(&field_name).copied(),
                        delegated_assignment,
                    );
                    if current.get(&field_name).copied() != Some(merged) {
                        current.insert(field_name, merged);
                        changed = true;
                    }
                }
            }
            SaturationStep::from(changed)
        },
        |max_iterations| SemanticDiagnostic {
            code: "semantic_field_nullability_budget_exhausted",
            message: format!(
                "{} field-nullability constructor delegation saturation exceeded finite budget of {max_iterations} iterations",
                ty.name
            ),
        },
    );

    diagnostics.extend(result.diagnostics.clone());
    ConstructorAssignmentSaturation {
        assignments: result.state,
        iterations: result.iterations,
        diagnostics: result.diagnostics,
    }
}

fn cyclic_delegating_constructor_names(constructors: &[&ConstructorDecl]) -> HashSet<String> {
    let delegation = constructors
        .iter()
        .filter_map(|constructor| {
            delegated_this_constructor_name(constructor)
                .map(|target| (constructor_rust_name(constructor), target))
        })
        .collect::<HashMap<_, _>>();
    delegation
        .keys()
        .filter(|name| constructor_delegation_reaches(&delegation, name, name))
        .cloned()
        .collect()
}

fn constructor_delegation_reaches(
    delegation: &HashMap<String, String>,
    start: &str,
    target: &str,
) -> bool {
    let mut seen = HashSet::new();
    let mut current = start;
    while let Some(next) = delegation.get(current) {
        if next == target {
            return true;
        }
        if !seen.insert(current.to_string()) {
            return false;
        }
        current = next;
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConstructorAssignmentSaturation {
    assignments: HashMap<String, HashMap<String, AssignmentNullability>>,
    iterations: usize,
    diagnostics: Vec<SemanticDiagnostic>,
}

fn delegated_this_constructor_name(constructor: &ConstructorDecl) -> Option<String> {
    let Some(body) = &constructor.body else {
        return None;
    };
    let Some(Stmt::ConstructorInvocation {
        kind: ConstructorInvocationKind::This,
        resolved: Some(resolved),
        ..
    }) = body.statements.first()
    else {
        return None;
    };
    Some(resolved.rust_name.clone())
}

fn merge_assignment_nullability(
    current: Option<AssignmentNullability>,
    next: AssignmentNullability,
) -> AssignmentNullability {
    current
        .map(|current| field_nullability_join(current, next))
        .unwrap_or(next)
}

fn constructor_rust_name(constructor: &ConstructorDecl) -> String {
    constructor
        .rust_name
        .clone()
        .unwrap_or_else(|| "new".to_string())
}

fn strongest_assignment(assignments: Vec<AssignmentNullability>) -> Option<AssignmentNullability> {
    assignments.into_iter().reduce(field_nullability_join)
}

const ASSIGNMENT_NULLABILITY_FACT_KIND_COUNT: usize = 4;

fn expr_assignment_nullability(expr: &Expr) -> AssignmentNullability {
    if matches!(expr, Expr::Literal(Literal::Null)) {
        NullabilityState::DefinitelyNull
    } else if expr_observed_non_null(expr) {
        NullabilityState::DefinitelyNonNull
    } else {
        NullabilityState::Unknown
    }
}

fn bare_field_assignment_nullability(
    expr: &Expr,
    local_shadows: &HashSet<String>,
) -> AssignmentNullability {
    match expr {
        Expr::Literal(Literal::Null) => NullabilityState::DefinitelyNull,
        Expr::Var(name) if local_shadows.contains(name) => NullabilityState::MaybeNull,
        Expr::Call { .. } => NullabilityState::MaybeNull,
        _ => expr_assignment_nullability(expr),
    }
}

fn expr_observed_non_null(expr: &Expr) -> bool {
    !matches!(expr, Expr::Literal(Literal::Null))
}

fn assignment_targets_field(target: &Expr, field_name: &str) -> bool {
    assigned_this_field_name(target) == Some(field_name)
}

fn bare_assignment_targets_unshadowed_field(
    target: &Expr,
    field_name: &str,
    local_shadows: &HashSet<String>,
) -> bool {
    matches!(target, Expr::Var(name) if name == field_name && !local_shadows.contains(name))
}

fn assigned_this_field_name(target: &Expr) -> Option<&str> {
    match target {
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            Some(field)
        }
        _ => None,
    }
}

fn local_shadows_from_params(params: &[Param]) -> HashSet<String> {
    params.iter().map(|param| param.name.clone()).collect()
}

fn returned_field_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            Some(field)
        }
        _ => None,
    }
}

fn call_receiver_is_this(receiver: &Option<Box<Expr>>) -> bool {
    receiver
        .as_deref()
        .is_none_or(|receiver| matches!(receiver, Expr::This))
}

fn field_types(ty: &TypeDecl) -> HashMap<String, JavaType> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
            _ => None,
        })
        .collect()
}

fn accessor_inline_policy(modifiers: &[Modifier]) -> AccessorInlinePolicy {
    if modifiers
        .iter()
        .any(|modifier| matches!(modifier, Modifier::Public | Modifier::Protected))
    {
        AccessorInlinePolicy::PreserveApi
    } else {
        AccessorInlinePolicy::InlineCandidate
    }
}

fn is_reference_type(java_ty: &JavaType) -> bool {
    !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void)
}

fn types_compatible(left: &JavaType, right: &JavaType) -> bool {
    left == right
}

fn classify_exception_type(
    type_name: &str,
    facts: &FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) -> ExceptionKind {
    if is_unchecked_exception_name(type_name)
        || type_reaches(
            type_name,
            "RuntimeException",
            facts,
            hierarchy_index,
            diagnostics,
        )
    {
        ExceptionKind::Unchecked
    } else if is_checked_exception_root_name(type_name)
        || type_reaches(type_name, "Exception", facts, hierarchy_index, diagnostics)
        || type_reaches(type_name, "Throwable", facts, hierarchy_index, diagnostics)
    {
        ExceptionKind::Checked
    } else {
        ExceptionKind::Unknown
    }
}

fn type_reaches_exception(
    type_name: &str,
    facts: &FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) -> bool {
    is_exception_root_name(type_name)
        || type_reaches(type_name, "Exception", facts, hierarchy_index, diagnostics)
        || type_reaches(
            type_name,
            "RuntimeException",
            facts,
            hierarchy_index,
            diagnostics,
        )
        || type_reaches(type_name, "Throwable", facts, hierarchy_index, diagnostics)
}

fn type_reaches_without_diagnostics(
    type_name: &str,
    super_name: &str,
    facts: &FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
) -> bool {
    let mut diagnostics = Vec::new();
    type_reaches(
        type_name,
        super_name,
        facts,
        hierarchy_index,
        &mut diagnostics,
    )
}

fn type_reaches(
    type_name: &str,
    super_name: &str,
    facts: &FactSet,
    hierarchy_index: &HierarchyFactLookupIndex,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) -> bool {
    let java_ty = JavaType::Named {
        name: type_name.to_string(),
        args: Vec::new(),
    };
    let reachability =
        hierarchy_index.reachable_hierarchy_edges_for_type_with_stats(facts, &java_ty);
    diagnostics.extend(
        reachability
            .diagnostics
            .into_iter()
            .map(|diagnostic| SemanticDiagnostic {
                code: diagnostic.code,
                message: diagnostic.message,
            }),
    );
    reachability
        .edges
        .iter()
        .any(|edge| edge.sub_name == type_name && edge.super_name == super_name)
}

fn is_exception_root_name(name: &str) -> bool {
    matches!(name, "Exception" | "RuntimeException" | "Throwable")
        || is_unchecked_exception_name(name)
}

fn is_checked_exception_root_name(name: &str) -> bool {
    matches!(name, "Exception" | "Throwable")
}

fn is_unchecked_exception_name(name: &str) -> bool {
    matches!(
        name,
        "RuntimeException"
            | "NullPointerException"
            | "NumberFormatException"
            | "IllegalArgumentException"
            | "IllegalStateException"
            | "IndexOutOfBoundsException"
            | "UnsupportedOperationException"
    )
}

fn java_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::String => Some("String"),
        _ => None,
    }
}

fn block_has_assignment_in_condition(body: &Block) -> bool {
    body.statements.iter().any(stmt_has_assignment_in_condition)
}

fn stmt_has_assignment_in_condition(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_looks_like_assignment_condition(condition)
                || block_has_assignment_in_condition(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_assignment_in_condition)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_looks_like_assignment_condition(condition)
                || block_has_assignment_in_condition(body)
        }
        Stmt::For {
            condition, body, ..
        } => {
            condition
                .as_ref()
                .is_some_and(expr_looks_like_assignment_condition)
                || block_has_assignment_in_condition(body)
        }
        Stmt::EnhancedFor { body, .. } | Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_has_assignment_in_condition(body)
        }
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_has_assignment_in_condition)),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_has_assignment_in_condition)
                || block_has_assignment_in_condition(body)
                || catches
                    .iter()
                    .any(|catch| block_has_assignment_in_condition(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_assignment_in_condition)
        }
        _ => false,
    }
}

fn expr_looks_like_assignment_condition(expr: &Expr) -> bool {
    match expr {
        Expr::Binary { left, right, .. } => {
            expr_looks_like_assignment_fragment(left)
                || expr_looks_like_assignment_fragment(right)
                || expr_looks_like_assignment_condition(left)
                || expr_looks_like_assignment_condition(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_looks_like_assignment_condition(condition)
                || expr_looks_like_assignment_condition(then_expr)
                || expr_looks_like_assignment_condition(else_expr)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            expr_looks_like_assignment_condition(expr)
        }
        _ => false,
    }
}

fn expr_looks_like_assignment_fragment(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Binary {
            left,
            right,
            ..
        } if matches!(left.as_ref(), Expr::Var(_))
            && matches!(right.as_ref(), Expr::Call { .. })
    )
}

fn block_has_tail_recursive_self_call(body: &Block, method_name: &str) -> bool {
    body.statements
        .iter()
        .any(|stmt| stmt_has_tail_recursive_self_call(stmt, method_name))
}

fn stmt_has_tail_recursive_self_call(stmt: &Stmt, method_name: &str) -> bool {
    match stmt {
        Stmt::Return(Some(expr)) => expr_is_self_call(expr, method_name),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_has_tail_recursive_self_call(then_block, method_name)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_has_tail_recursive_self_call(block, method_name))
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_has_tail_recursive_self_call(body, method_name)
        }
        _ => false,
    }
}

fn expr_is_self_call(expr: &Expr, method_name: &str) -> bool {
    matches!(
        expr,
        Expr::Call { receiver, name, .. } if name == method_name && call_receiver_is_this(receiver)
    )
}

fn walk_stmt_exprs(stmt: &Stmt, visit: &mut impl FnMut(&Expr)) {
    match stmt {
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        }
        | Stmt::Expr(expr)
        | Stmt::Return(Some(expr))
        | Stmt::Throw(expr) => walk_expr(expr, visit),
        Stmt::Assign { target, value } => {
            walk_expr(target, visit);
            walk_expr(value, visit);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            walk_expr(condition, visit);
            walk_block_exprs(then_block, visit);
            if let Some(else_block) = else_block {
                walk_block_exprs(else_block, visit);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            walk_expr(condition, visit);
            walk_block_exprs(body, visit);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                walk_stmt_exprs(stmt, visit);
            }
            if let Some(condition) = condition {
                walk_expr(condition, visit);
            }
            for expr in update {
                walk_expr(expr, visit);
            }
            walk_block_exprs(body, visit);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            walk_expr(iterable, visit);
            walk_block_exprs(body, visit);
        }
        Stmt::Switch { selector, cases } => {
            walk_expr(selector, visit);
            for case in cases {
                for stmt in &case.statements {
                    walk_stmt_exprs(stmt, visit);
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
                walk_stmt_exprs(stmt, visit);
            }
            walk_block_exprs(body, visit);
            for catch in catches {
                walk_block_exprs(&catch.body, visit);
            }
            if let Some(finally_block) = finally_block {
                walk_block_exprs(finally_block, visit);
            }
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => walk_block_exprs(body, visit),
        Stmt::VarDecl {
            initializer: None, ..
        }
        | Stmt::Return(None)
        | Stmt::LocalType(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn walk_block_exprs(block: &Block, visit: &mut impl FnMut(&Expr)) {
    for stmt in &block.statements {
        walk_stmt_exprs(stmt, visit);
    }
}

fn walk_expr(expr: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(expr);
    match expr {
        Expr::FieldAccess(receiver, _) => walk_expr(receiver, visit),
        Expr::ArrayAccess(array, index) => {
            walk_expr(array, visit);
            walk_expr(index, visit);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                walk_expr(expr, visit);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    walk_expr(expr, visit);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                walk_expr(expr, visit);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, visit);
            walk_expr(right, visit);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            walk_expr(expr, visit);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            walk_expr(condition, visit);
            walk_expr(then_expr, visit);
            walk_expr(else_expr, visit);
        }
        Expr::Switch { selector, cases } => {
            walk_expr(selector, visit);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        walk_expr(expr, visit);
                    }
                }
                walk_expr(&case.value, visit);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                walk_expr(receiver, visit);
            }
            for arg in args {
                walk_expr(arg, visit);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                walk_expr(arg, visit);
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => walk_expr(expr, visit),
            crate::java_ir::LambdaBody::Block(block) => walk_block_exprs(block, visit),
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
