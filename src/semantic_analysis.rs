use std::collections::{HashMap, HashSet};

use crate::fact_saturation::{FiniteSaturationPolicy, SaturationStep};
use crate::facts::{
    AccessorInlinePolicy, CatchFact, ControlFlowFact, ControlFlowFactKind, ExceptionFact,
    ExceptionKind, ExternalApiEffectFact, ExternalApiRole, FactProvenance, FactSet, FieldKey,
    FieldNullabilityFact, FieldNullabilityProvenance, MethodEffectFact, MethodKey,
    NullabilityState, ReceiverMutabilityFact, RecursionFact, RecursionKind, TrivialAccessorFact,
    TrivialAccessorKind,
};
use crate::java_ir::{
    BinaryOp, Block, ConstructorDecl, ConstructorInvocationKind, Expr, FieldDecl, JavaType,
    Literal, Member, MethodDecl, Modifier, Program, Stmt, TypeDecl,
};
use crate::rewrite::{
    AssignmentConditionNormalizedShape, AssignmentInConditionPlan, ControlFlowRewritePlan,
    MethodControlFlowPlan, NullTestKind, TailRecursiveLoopPlan, TailRecursiveNormalizedShape,
};
use crate::usage::{self, UsageFacts, UsageMethodKey};

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
}

impl Default for SemanticAnalysisConfig {
    fn default() -> Self {
        Self {
            max_external_guess_iterations: None,
            enable_external_api_guesses: true,
            enable_control_flow_facts: true,
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

    collect_trivial_accessor_facts(program, &mut facts);
    let usage = usage::analyze_usage_with_facts(program, &facts);
    let field_nullability_iterations =
        collect_field_nullability_facts(program, &mut facts, &mut diagnostics);
    if config.enable_external_api_guesses {
        collect_external_api_effect_facts(program, &mut facts, &config, &mut diagnostics);
    }
    collect_method_effect_facts(program, &usage, &mut facts);
    collect_recursion_facts(program, &mut facts);
    let control_flow_plans = if config.enable_control_flow_facts {
        collect_control_flow_facts(program, &mut facts);
        collect_control_flow_plans(program)
    } else {
        Vec::new()
    };
    collect_exception_facts(program, &mut facts);
    collect_catch_facts(program, &mut facts);

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
            &mut guessed_count,
            &mut budget_exhausted,
        );
    }
    if budget_exhausted {
        diagnostics.push(SemanticDiagnostic {
            code: "semantic_external_api_effect_budget_exhausted",
            message: "external API effect guessing stopped at configured finite budget".to_string(),
        });
    }
}

fn collect_type_external_api_effect_facts(
    ty: &TypeDecl,
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_external_api_effects_from_block(
                        body,
                        facts,
                        seen,
                        budget,
                        guessed_count,
                        budget_exhausted,
                    );
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_external_api_effects_from_block(
                        body,
                        facts,
                        seen,
                        budget,
                        guessed_count,
                        budget_exhausted,
                    );
                }
            }
            Member::Type(nested) => collect_type_external_api_effect_facts(
                nested,
                facts,
                seen,
                budget,
                guessed_count,
                budget_exhausted,
            ),
            _ => {}
        }
    }
}

fn collect_external_api_effects_from_block(
    body: &Block,
    facts: &mut FactSet,
    seen: &mut HashSet<String>,
    budget: Option<usize>,
    guessed_count: &mut usize,
    budget_exhausted: &mut bool,
) {
    for stmt in &body.statements {
        walk_stmt_exprs(stmt, &mut |expr| {
            if let Expr::Call {
                receiver: Some(receiver),
                name,
                args,
                resolved,
            } = expr
                && !matches!(receiver.as_ref(), Expr::This | Expr::Super)
                && resolved.is_none()
                && seen.insert(name.clone())
            {
                if budget.is_some_and(|budget| *guessed_count >= budget) {
                    *budget_exhausted = true;
                    return;
                }
                let role = external_api_role(name, args.len());
                facts.external_api_effect_facts.push(ExternalApiEffectFact {
                    method_name: name.clone(),
                    receiver_type: None,
                    role,
                    receiver_mutability: external_api_receiver_mutability(role),
                    provenance: FactProvenance::ConservativeFallback,
                });
                *guessed_count += 1;
            }
        });
    }
}

fn collect_recursion_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_recursion_facts(ty, facts);
    }
}

fn collect_method_effect_facts(program: &Program, usage: &UsageFacts, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_method_effect_facts(ty, usage, facts);
    }
}

fn collect_type_method_effect_facts(ty: &TypeDecl, usage: &UsageFacts, facts: &mut FactSet) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let thrown = method_thrown_exception_types(method);
                let receiver_mutability = method_receiver_mutability_effect(ty, method, usage);
                let return_nullability = method_return_nullability_effect(method);
                facts.method_effect_facts.push(MethodEffectFact {
                    method: MethodKey {
                        type_name: ty.name.clone(),
                        method_name: method.name.clone(),
                    },
                    receiver_mutability,
                    return_nullability,
                    thrown,
                    provenance: FactProvenance::ParsedSource,
                });
            }
            Member::Type(nested) => collect_type_method_effect_facts(nested, usage, facts),
            _ => {}
        }
    }
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

fn method_return_nullability_effect(method: &MethodDecl) -> NullabilityState {
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
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body) => collect_return_nullability_states_from_block(body, states),
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

fn method_thrown_exception_types(method: &MethodDecl) -> Vec<JavaType> {
    let mut thrown = Vec::new();
    for ty in &method.throws {
        push_unique_java_type(&mut thrown, ty.clone());
    }
    if let Some(body) = &method.body {
        let mut exception_env = HashMap::new();
        collect_thrown_exception_types_from_block(body, &mut exception_env, &mut thrown);
    }
    thrown
}

fn collect_thrown_exception_types_from_block(
    block: &Block,
    exception_env: &mut HashMap<String, JavaType>,
    thrown: &mut Vec<JavaType>,
) {
    for stmt in &block.statements {
        collect_thrown_exception_types_from_stmt(stmt, exception_env, thrown);
    }
}

fn collect_thrown_exception_types_from_stmt(
    stmt: &Stmt,
    exception_env: &mut HashMap<String, JavaType>,
    thrown: &mut Vec<JavaType>,
) {
    match stmt {
        Stmt::Throw(expr) => {
            if let Some(ty) = thrown_exception_type_from_expr(expr, exception_env) {
                push_unique_java_type(thrown, ty);
            }
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_thrown_exception_types_from_block(then_block, exception_env, thrown);
            if let Some(else_block) = else_block {
                collect_thrown_exception_types_from_block(else_block, exception_env, thrown);
            }
        }
        Stmt::While { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body) => {
            collect_thrown_exception_types_from_block(body, exception_env, thrown)
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    collect_thrown_exception_types_from_stmt(stmt, exception_env, thrown);
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
                collect_thrown_exception_types_from_stmt(resource, exception_env, thrown);
            }
            collect_thrown_exception_types_from_block(body, exception_env, thrown);
            for catch in catches {
                let previous = catch
                    .types
                    .first()
                    .cloned()
                    .and_then(|ty| exception_env.insert(catch.name.clone(), ty));
                collect_thrown_exception_types_from_block(&catch.body, exception_env, thrown);
                if let Some(previous) = previous {
                    exception_env.insert(catch.name.clone(), previous);
                } else {
                    exception_env.remove(&catch.name);
                }
            }
            if let Some(finally_block) = finally_block {
                collect_thrown_exception_types_from_block(finally_block, exception_env, thrown);
            }
        }
        Stmt::LocalType(ty) => {
            for member in &ty.members {
                if let Member::Method(method) = member {
                    for ty in method_thrown_exception_types(method) {
                        push_unique_java_type(thrown, ty);
                    }
                }
            }
        }
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
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

fn collect_type_recursion_facts(ty: &TypeDecl, facts: &mut FactSet) {
    let method_names = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(method.name.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut edges = Vec::new();
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let calls = called_same_type_methods(body, &method_names);
                    for called in calls {
                        edges.push((method.name.clone(), called));
                    }
                }
            }
            Member::Type(nested) => collect_type_recursion_facts(nested, facts),
            _ => {}
        }
    }
    for (caller, callee) in &edges {
        let kind = if caller == callee {
            Some(RecursionKind::Direct)
        } else if edges
            .iter()
            .any(|(other_caller, other_callee)| other_caller == callee && other_callee == caller)
        {
            Some(RecursionKind::Mutual)
        } else {
            None
        };
        if let Some(kind) = kind {
            facts.recursion_facts.push(RecursionFact {
                method: MethodKey {
                    type_name: ty.name.clone(),
                    method_name: caller.clone(),
                },
                calls: MethodKey {
                    type_name: ty.name.clone(),
                    method_name: callee.clone(),
                },
                kind,
                provenance: FactProvenance::ParsedSource,
            });
        }
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
            Stmt::While { condition, body } => {
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
            Stmt::EnhancedFor { body, .. } | Stmt::Block(body) => {
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
    let Expr::Binary { left, right, .. } = expr else {
        return None;
    };
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
    body.statements
        .iter()
        .find_map(|stmt| tail_recursive_loop_plan_from_stmt(stmt, method_name))
}

fn tail_recursive_loop_plan_from_stmt(
    stmt: &Stmt,
    method_name: &str,
) -> Option<TailRecursiveLoopPlan> {
    match stmt {
        Stmt::Return(Some(Expr::Call { name, args, .. })) if name == method_name => {
            Some(TailRecursiveLoopPlan {
                method_name: name.clone(),
                arg_count: args.len(),
                normalized_shape: TailRecursiveNormalizedShape::LoopWithParameterReassignment,
            })
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => tail_recursive_loop_plan(then_block, method_name).or_else(|| {
            else_block
                .as_ref()
                .and_then(|else_block| tail_recursive_loop_plan(else_block, method_name))
        }),
        Stmt::Block(body) => tail_recursive_loop_plan(body, method_name),
        _ => None,
    }
}

fn collect_exception_facts(program: &Program, facts: &mut FactSet) {
    let mut exception_names = HashSet::new();
    for ty in &program.types {
        collect_exception_candidate_names(ty, facts, &mut exception_names);
    }
    for type_name in exception_names {
        facts.exception_facts.push(ExceptionFact {
            kind: classify_exception_type(&type_name, facts),
            type_name,
            provenance: FactProvenance::ParsedSource,
        });
    }
}

fn collect_exception_candidate_names(
    ty: &TypeDecl,
    facts: &FactSet,
    exception_names: &mut HashSet<String>,
) {
    if ty.extends.as_deref().is_some_and(is_exception_root_name)
        || type_reaches_exception(&ty.name, facts)
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
            Member::Type(nested) => {
                collect_exception_candidate_names(nested, facts, exception_names)
            }
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
            | Stmt::For { body, .. }
            | Stmt::EnhancedFor { body, .. }
            | Stmt::Block(body) => collect_exception_names_from_block(body, exception_names),
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

fn collect_catch_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_catch_facts(ty, facts);
    }
}

fn collect_type_catch_facts(ty: &TypeDecl, facts: &mut FactSet) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_method_catch_facts(ty, method, body, facts);
                }
            }
            Member::Type(nested) => collect_type_catch_facts(nested, facts),
            _ => {}
        }
    }
}

fn collect_method_catch_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    body: &Block,
    facts: &mut FactSet,
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
    );
}

fn collect_catch_facts_from_block(
    body: &Block,
    method: &MethodKey,
    clause_index: &mut usize,
    facts: &mut FactSet,
) {
    for stmt in &body.statements {
        match stmt {
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                collect_catch_facts_from_block(body, method, clause_index, facts);
                for catch in catches {
                    facts.catch_facts.push(CatchFact {
                        method: method.clone(),
                        clause_index: *clause_index,
                        assignable_from: catch_assignable_from_types(&catch.types, facts),
                        types: catch.types.clone(),
                        provenance: FactProvenance::ParsedSource,
                    });
                    *clause_index += 1;
                    collect_catch_facts_from_block(&catch.body, method, clause_index, facts);
                }
                if let Some(finally_block) = finally_block {
                    collect_catch_facts_from_block(finally_block, method, clause_index, facts);
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_catch_facts_from_block(then_block, method, clause_index, facts);
                if let Some(else_block) = else_block {
                    collect_catch_facts_from_block(else_block, method, clause_index, facts);
                }
            }
            Stmt::While { body, .. }
            | Stmt::For { body, .. }
            | Stmt::EnhancedFor { body, .. }
            | Stmt::Block(body) => {
                collect_catch_facts_from_block(body, method, clause_index, facts)
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_catch_facts_from_stmts(&case.statements, method, clause_index, facts);
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
) {
    for stmt in statements {
        if let Stmt::Block(block) = stmt {
            collect_catch_facts_from_block(block, method, clause_index, facts);
        }
    }
}

fn catch_assignable_from_types(catch_types: &[JavaType], facts: &FactSet) -> Vec<JavaType> {
    let mut assignable = facts
        .exception_facts
        .iter()
        .filter_map(|exception| {
            catch_types
                .iter()
                .any(|catch_ty| {
                    exception_assignable_to_catch(&exception.type_name, catch_ty, facts)
                })
                .then_some(JavaType::Named {
                    name: exception.type_name.clone(),
                    args: Vec::new(),
                })
        })
        .collect::<Vec<_>>();
    assignable.sort_by(|left, right| java_type_sort_key(left).cmp(&java_type_sort_key(right)));
    assignable.dedup();
    assignable
}

fn exception_assignable_to_catch(
    exception_name: &str,
    catch_ty: &JavaType,
    facts: &FactSet,
) -> bool {
    let Some(catch_name) = java_type_name(catch_ty) else {
        return false;
    };
    exception_name == catch_name || type_reaches(exception_name, catch_name, facts)
}

fn java_type_sort_key(java_ty: &JavaType) -> String {
    java_type_name(java_ty).unwrap_or("").to_string()
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
            Member::Method(method) => method.body.as_ref(),
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
    method_bodies: &[&Block],
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
    let initializer_null = matches!(field.initializer, Some(Expr::Literal(Literal::Null)));
    let any_method_null_assignment = method_bodies
        .iter()
        .any(|body| block_assigns_field_null(body, ty, &field.name, helper_assignments, facts));
    let all_constructors_assign_non_null = !constructors.is_empty()
        && constructors.iter().all(|constructor| {
            constructor_assignments
                .get(&constructor_rust_name(constructor))
                .and_then(|assignments| assignments.get(&field.name))
                .is_some_and(|assignment| *assignment == AssignmentNullability::ObservedNonNull)
        });

    let (state, provenance) = if initializer_null || any_method_null_assignment {
        (
            NullabilityState::MaybeNull,
            FieldNullabilityProvenance::MethodAssignment,
        )
    } else if all_constructors_assign_non_null {
        (
            NullabilityState::DefinitelyNonNull,
            FieldNullabilityProvenance::ConstructorAssignments,
        )
    } else if constructors.is_empty() && initializer_non_null {
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

fn block_assigns_field_null(
    body: &Block,
    ty: &TypeDecl,
    field_name: &str,
    helper_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    facts: &FactSet,
) -> bool {
    block_field_assignments(body, ty, field_name, helper_assignments, facts)
        .into_iter()
        .any(|assignment| assignment == AssignmentNullability::Null)
}

fn block_field_assignments(
    body: &Block,
    ty: &TypeDecl,
    field_name: &str,
    helper_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    facts: &FactSet,
) -> Vec<AssignmentNullability> {
    let mut assignments = Vec::new();
    collect_field_assignments_from_stmts(
        &body.statements,
        ty,
        field_name,
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
    helper_assignments: &HashMap<String, HashMap<String, AssignmentNullability>>,
    facts: &FactSet,
    assignments: &mut Vec<AssignmentNullability>,
) {
    for stmt in statements {
        match stmt {
            Stmt::Assign { target, value } if assignment_targets_field(target, field_name) => {
                assignments.push(expr_assignment_nullability(value));
            }
            Stmt::Expr(expr) => {
                collect_field_assignments_from_expr(
                    expr,
                    ty,
                    field_name,
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
                collect_field_assignments_from_stmts(
                    &then_block.statements,
                    ty,
                    field_name,
                    helper_assignments,
                    facts,
                    assignments,
                );
                if let Some(else_block) = else_block {
                    collect_field_assignments_from_stmts(
                        &else_block.statements,
                        ty,
                        field_name,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
            }
            Stmt::While { body, .. } | Stmt::EnhancedFor { body, .. } | Stmt::Block(body) => {
                collect_field_assignments_from_stmts(
                    &body.statements,
                    ty,
                    field_name,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            Stmt::For {
                init, update, body, ..
            } => {
                collect_field_assignments_from_stmts(
                    init,
                    ty,
                    field_name,
                    helper_assignments,
                    facts,
                    assignments,
                );
                for expr in update {
                    collect_field_assignments_from_expr(
                        expr,
                        ty,
                        field_name,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
                collect_field_assignments_from_stmts(
                    &body.statements,
                    ty,
                    field_name,
                    helper_assignments,
                    facts,
                    assignments,
                );
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_field_assignments_from_stmts(
                        &case.statements,
                        ty,
                        field_name,
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
                collect_field_assignments_from_stmts(
                    resources,
                    ty,
                    field_name,
                    helper_assignments,
                    facts,
                    assignments,
                );
                collect_field_assignments_from_stmts(
                    &body.statements,
                    ty,
                    field_name,
                    helper_assignments,
                    facts,
                    assignments,
                );
                for catch in catches {
                    collect_field_assignments_from_stmts(
                        &catch.body.statements,
                        ty,
                        field_name,
                        helper_assignments,
                        facts,
                        assignments,
                    );
                }
                if let Some(finally_block) = finally_block {
                    collect_field_assignments_from_stmts(
                        &finally_block.statements,
                        ty,
                        field_name,
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
                helper_assignments,
                facts,
                assignments,
            );
            collect_field_assignments_from_expr(
                right,
                ty,
                field_name,
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
                helper_assignments,
                facts,
                assignments,
            );
            collect_field_assignments_from_expr(
                then_expr,
                ty,
                field_name,
                helper_assignments,
                facts,
                assignments,
            );
            collect_field_assignments_from_expr(
                else_expr,
                ty,
                field_name,
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
                helper_assignments,
                facts,
                assignments,
            );
            collect_field_assignments_from_expr(
                index,
                ty,
                field_name,
                helper_assignments,
                facts,
                assignments,
            );
        }
        Expr::ArrayCreation { dimensions, .. } | Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_field_assignments_from_expr(
                    expr,
                    ty,
                    field_name,
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
        for field in field_types(ty).keys() {
            let assignments = block_field_assignments(body, ty, field, &HashMap::new(), facts);
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
    let mut by_constructor = HashMap::new();
    for constructor in constructors {
        let mut field_assignments = HashMap::new();
        if let Some(body) = &constructor.body {
            for field in fields.keys() {
                let assignments =
                    block_field_assignments(body, ty, field, helper_assignments, facts);
                if let Some(assignment) = strongest_assignment(assignments) {
                    field_assignments.insert(field.clone(), assignment);
                }
            }
        }
        by_constructor.insert(constructor_rust_name(constructor), field_assignments);
    }

    let policy = FiniteSaturationPolicy::new(
        constructors.len().saturating_mul(fields.len()),
        AssignmentNullability::FACT_KIND_COUNT,
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
    strongest_assignment(current.into_iter().chain([next]).collect())
        .expect("merged assignment list is non-empty")
}

fn constructor_rust_name(constructor: &ConstructorDecl) -> String {
    constructor
        .rust_name
        .clone()
        .unwrap_or_else(|| "new".to_string())
}

fn strongest_assignment(assignments: Vec<AssignmentNullability>) -> Option<AssignmentNullability> {
    if assignments
        .iter()
        .any(|assignment| *assignment == AssignmentNullability::Null)
    {
        Some(AssignmentNullability::Null)
    } else if assignments
        .iter()
        .any(|assignment| *assignment == AssignmentNullability::ObservedNonNull)
    {
        Some(AssignmentNullability::ObservedNonNull)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssignmentNullability {
    Null,
    ObservedNonNull,
}

impl AssignmentNullability {
    const FACT_KIND_COUNT: usize = 2;
}

fn expr_assignment_nullability(expr: &Expr) -> AssignmentNullability {
    if matches!(expr, Expr::Literal(Literal::Null)) {
        AssignmentNullability::Null
    } else {
        AssignmentNullability::ObservedNonNull
    }
}

fn expr_observed_non_null(expr: &Expr) -> bool {
    !matches!(expr, Expr::Literal(Literal::Null))
}

fn assignment_targets_field(target: &Expr, field_name: &str) -> bool {
    assigned_this_field_name(target) == Some(field_name)
}

fn assigned_this_field_name(target: &Expr) -> Option<&str> {
    match target {
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            Some(field)
        }
        _ => None,
    }
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

fn external_api_role(name: &str, arg_count: usize) -> ExternalApiRole {
    if (name == "get" && arg_count > 1) || name.starts_with("get") {
        ExternalApiRole::Getter
    } else if name.starts_with("is") || name.starts_with("has") {
        ExternalApiRole::BooleanGetter
    } else if name == "size" || name == "length" {
        ExternalApiRole::Getter
    } else if name == "set" && arg_count > 1 {
        ExternalApiRole::IndexedSetter
    } else if name.starts_with("set") {
        ExternalApiRole::Setter
    } else if matches!(
        name,
        "add" | "remove" | "put" | "clear" | "advance" | "read" | "write" | "copy"
    ) {
        ExternalApiRole::Mutator
    } else {
        ExternalApiRole::Unknown
    }
}

fn external_api_receiver_mutability(role: ExternalApiRole) -> ReceiverMutabilityFact {
    match role {
        ExternalApiRole::Getter
        | ExternalApiRole::BooleanGetter
        | ExternalApiRole::IndexedGetter => ReceiverMutabilityFact::ReadOnly,
        ExternalApiRole::Setter
        | ExternalApiRole::Mutator
        | ExternalApiRole::FluentMutator
        | ExternalApiRole::IndexedSetter => ReceiverMutabilityFact::MutatesReceiver,
        ExternalApiRole::Unknown => ReceiverMutabilityFact::Unknown,
    }
}

fn classify_exception_type(type_name: &str, facts: &FactSet) -> ExceptionKind {
    if is_unchecked_exception_name(type_name) || type_reaches(type_name, "RuntimeException", facts)
    {
        ExceptionKind::Unchecked
    } else if is_checked_exception_root_name(type_name)
        || type_reaches(type_name, "Exception", facts)
        || type_reaches(type_name, "Throwable", facts)
    {
        ExceptionKind::Checked
    } else {
        ExceptionKind::Unknown
    }
}

fn type_reaches_exception(type_name: &str, facts: &FactSet) -> bool {
    is_exception_root_name(type_name)
        || type_reaches(type_name, "Exception", facts)
        || type_reaches(type_name, "RuntimeException", facts)
        || type_reaches(type_name, "Throwable", facts)
}

fn type_reaches(type_name: &str, super_name: &str, facts: &FactSet) -> bool {
    let java_ty = JavaType::Named {
        name: type_name.to_string(),
        args: Vec::new(),
    };
    facts
        .reachable_hierarchy_edges_for_type(&java_ty)
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

fn called_same_type_methods(body: &Block, method_names: &HashSet<String>) -> HashSet<String> {
    let mut calls = HashSet::new();
    for stmt in &body.statements {
        walk_stmt_exprs(stmt, &mut |expr| {
            if let Expr::Call { receiver, name, .. } = expr
                && call_receiver_is_this(receiver)
                && method_names.contains(name)
            {
                calls.insert(name.clone());
            }
        });
    }
    calls
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
        Stmt::While { condition, body } => {
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
        Stmt::EnhancedFor { body, .. } | Stmt::Block(body) => {
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
        Stmt::Block(body) => block_has_tail_recursive_self_call(body, method_name),
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
        Stmt::While { condition, body } => {
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
        Stmt::Block(body) => walk_block_exprs(body, visit),
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
        Expr::ArrayCreation { dimensions, .. } | Expr::ArrayInitializer(dimensions) => {
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
        | Expr::Raw(_) => {}
    }
}
