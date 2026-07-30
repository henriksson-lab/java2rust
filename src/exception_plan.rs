use std::collections::{BTreeMap, HashMap};

use crate::facts::FactSet;
use crate::java_ir::{
    Block, CatchClause, Expr, JavaType, Member, MethodDecl, Program, Stmt, TypeDecl,
};
use crate::usage::{CallGraph, CallableKey};

#[derive(Debug, Clone, PartialEq)]
pub enum ThrowExprPlan<'a> {
    NewException {
        class_name: String,
        args: &'a [Expr],
    },
    CauseOrSelfIfInstance {
        wrapper: &'a Expr,
        class_names: Vec<String>,
    },
    ExistingException(&'a Expr),
    FallbackValue(&'a Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaExceptionSupportPlan {
    pub emit_support_item: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodExceptionPlan {
    pub wrap_returns_in_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramExceptionPlan {
    pub support: JavaExceptionSupportPlan,
    methods: BTreeMap<(String, String), MethodExceptionPlan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramFallibilityPlan {
    callables: HashMap<CallableKey, CallableFallibility>,
    solve_iterations: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallableFallibility {
    pub returns_result: bool,
    pub reasons: Vec<FallibilityReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallibilityReason {
    MethodThrowFact,
    ExternalThrowFact,
    PropagatedFromCallee,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TryStmtPlan<'a> {
    CatchOnly {
        body: &'a Block,
        catches: &'a [CatchClause],
        completion: TryBlockCompletion,
    },
    WithResources {
        resources: &'a [Stmt],
        body: &'a Block,
        catches: &'a [CatchClause],
        completion: TryBlockCompletion,
    },
    FinallyOnly {
        body: &'a Block,
        finally_block: &'a Block,
        completion: TryBlockCompletion,
    },
    CatchFinally {
        body: &'a Block,
        catches: &'a [CatchClause],
        finally_block: &'a Block,
        completion: TryBlockCompletion,
    },
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryBlockCompletion {
    Terminates,
    FallsThroughUnit,
}

impl ProgramExceptionPlan {
    pub fn method_plan(&self, ty: &TypeDecl, method: &MethodDecl) -> MethodExceptionPlan {
        self.methods
            .get(&method_exception_key(ty, method))
            .or_else(|| self.methods.get(&(ty.name.clone(), method.name.clone())))
            .copied()
            .unwrap_or(MethodExceptionPlan {
                wrap_returns_in_result: false,
            })
    }
}

impl ProgramFallibilityPlan {
    pub fn callable(&self, key: &CallableKey) -> Option<&CallableFallibility> {
        self.callables.get(key)
    }

    pub fn returns_result(&self, key: &CallableKey) -> bool {
        self.callable(key)
            .is_some_and(|fallibility| fallibility.returns_result)
    }

    pub fn solve_iterations(&self) -> usize {
        self.solve_iterations
    }
}

impl Default for ProgramExceptionPlan {
    fn default() -> Self {
        Self {
            support: JavaExceptionSupportPlan {
                emit_support_item: false,
            },
            methods: BTreeMap::new(),
        }
    }
}

pub fn program_uses_java_exception_with_facts(program: &Program, facts: &FactSet) -> bool {
    facts.has_method_throw_effects()
        || !facts.exception_facts.is_empty()
        || !facts.catch_facts.is_empty()
        || program_has_lowerable_try_scope(program)
}

pub fn plan_java_exception_support(program: &Program, facts: &FactSet) -> JavaExceptionSupportPlan {
    JavaExceptionSupportPlan {
        emit_support_item: program_uses_java_exception_with_facts(program, facts),
    }
}

fn program_has_lowerable_try_scope(program: &Program) -> bool {
    program.types.iter().any(type_has_lowerable_try_scope)
}

fn type_has_lowerable_try_scope(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_has_lowerable_try_scope),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_has_lowerable_try_scope),
        Member::Type(nested) => type_has_lowerable_try_scope(nested),
        Member::Field(_) | Member::Unsupported { .. } => false,
    })
}

fn block_has_lowerable_try_scope(block: &Block) -> bool {
    block.statements.iter().any(stmt_has_lowerable_try_scope)
}

fn stmt_has_lowerable_try_scope(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            !matches!(
                plan_try_stmt(resources, body, catches, finally_block.as_ref()),
                TryStmtPlan::Unsupported
            ) || block_has_lowerable_try_scope(body)
                || catches
                    .iter()
                    .any(|catch| block_has_lowerable_try_scope(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_lowerable_try_scope)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_has_lowerable_try_scope(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_lowerable_try_scope)
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => block_has_lowerable_try_scope(body),
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_has_lowerable_try_scope)),
        Stmt::LocalType(nested) => type_has_lowerable_try_scope(nested),
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    }
}

pub fn plan_method_exceptions(
    ty: &TypeDecl,
    method: &MethodDecl,
    facts: &FactSet,
) -> MethodExceptionPlan {
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    MethodExceptionPlan {
        wrap_returns_in_result: facts.method_has_thrown_exceptions(&ty.name, &rust_name)
            || facts.method_has_thrown_exceptions(&ty.name, &method.name),
    }
}

pub fn plan_program_exceptions(program: &Program, facts: &FactSet) -> ProgramExceptionPlan {
    let mut methods = BTreeMap::new();
    for ty in &program.types {
        collect_method_exception_plans_for_type(ty, facts, None, &mut methods);
    }
    ProgramExceptionPlan {
        support: plan_java_exception_support(program, facts),
        methods,
    }
}

pub fn plan_program_exceptions_with_fallibility(
    program: &Program,
    facts: &FactSet,
    fallibility: &ProgramFallibilityPlan,
) -> ProgramExceptionPlan {
    let mut methods = BTreeMap::new();
    for ty in &program.types {
        collect_method_exception_plans_for_type(ty, facts, Some(fallibility), &mut methods);
    }
    ProgramExceptionPlan {
        support: plan_java_exception_support(program, facts),
        methods,
    }
}

pub fn plan_program_fallibility(call_graph: &CallGraph, facts: &FactSet) -> ProgramFallibilityPlan {
    let mut callables = call_graph
        .nodes
        .iter()
        .cloned()
        .map(|key| {
            let fallibility = seed_callable_fallibility(&key, facts);
            (key, fallibility)
        })
        .collect::<HashMap<_, _>>();
    let mut solve_iterations = 0usize;
    loop {
        solve_iterations += 1;
        let mut changed = false;
        for edge in &call_graph.edges {
            let callee_returns_result = callables
                .get(&edge.callee)
                .is_some_and(|fallibility| fallibility.returns_result);
            if !callee_returns_result {
                continue;
            }
            let caller = callables.entry(edge.caller.clone()).or_default();
            if !caller.returns_result {
                caller.returns_result = true;
                caller.reasons.push(FallibilityReason::PropagatedFromCallee);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    ProgramFallibilityPlan {
        callables,
        solve_iterations,
    }
}

fn seed_callable_fallibility(key: &CallableKey, facts: &FactSet) -> CallableFallibility {
    let reason = match key {
        CallableKey::Method {
            type_name,
            method_name,
        }
        | CallableKey::Constructor {
            type_name,
            rust_name: method_name,
        } => facts
            .method_effect_for(type_name, method_name)
            .is_some_and(|fact| !fact.thrown.is_empty())
            .then_some(FallibilityReason::MethodThrowFact),
        CallableKey::ExternalMethod {
            receiver_type,
            method_name,
            arg_count,
        } => facts
            .external_api_effect_facts
            .iter()
            .any(|fact| {
                !fact.thrown.is_empty()
                    && fact.method_name == *method_name
                    && fact.arg_count == Some(*arg_count)
                    && fact.receiver_type.as_ref() == receiver_type.as_ref()
            })
            .then_some(FallibilityReason::ExternalThrowFact),
        CallableKey::ExternalConstructor { .. } => None,
        CallableKey::Lambda { .. } | CallableKey::AnonymousMethod { .. } => None,
    };
    let mut fallibility = CallableFallibility::default();
    if let Some(reason) = reason {
        fallibility.returns_result = true;
        fallibility.reasons.push(reason);
    }
    fallibility
}

pub fn plan_try_stmt<'a>(
    resources: &'a [Stmt],
    body: &'a Block,
    catches: &'a [CatchClause],
    finally_block: Option<&'a Block>,
) -> TryStmtPlan<'a> {
    if let Some(finally_block) = finally_block {
        let completion = plan_block_exception_completion(body);
        if resources.is_empty()
            && plan_block_exception_completion(finally_block)
                == TryBlockCompletion::FallsThroughUnit
        {
            if catches.is_empty() {
                return TryStmtPlan::FinallyOnly {
                    body,
                    finally_block,
                    completion,
                };
            }
            if completion == TryBlockCompletion::Terminates
                && catches
                    .iter()
                    .any(|catch| !block_terminates_with_exception_boundary(&catch.body))
            {
                return TryStmtPlan::Unsupported;
            }
            return TryStmtPlan::CatchFinally {
                body,
                catches,
                finally_block,
                completion,
            };
        }
        return TryStmtPlan::Unsupported;
    }
    let completion = plan_block_exception_completion(body);
    if !resources.is_empty() {
        if !resources.iter().all(stmt_is_resource_declaration) {
            return TryStmtPlan::Unsupported;
        }
        if catches
            .iter()
            .any(|catch| !block_terminates_with_exception_boundary(&catch.body))
        {
            return TryStmtPlan::Unsupported;
        }
        return TryStmtPlan::WithResources {
            resources,
            body,
            catches,
            completion,
        };
    }
    if catches.is_empty() {
        return TryStmtPlan::Unsupported;
    }
    TryStmtPlan::CatchOnly {
        body,
        catches,
        completion,
    }
}

pub fn try_resource_names(resources: &[Stmt]) -> Vec<String> {
    resources
        .iter()
        .filter_map(|resource| match resource {
            Stmt::VarDecl { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn stmt_is_resource_declaration(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::VarDecl {
            initializer: Some(_),
            ..
        }
    )
}

pub fn catch_match_class_names(
    current_type: Option<&str>,
    current_method: Option<&str>,
    clause_index: usize,
    catch: &CatchClause,
    facts: &FactSet,
) -> Vec<String> {
    let mut names = catch_assignable_class_names(&catch.types, facts);
    let fact_names = current_type
        .zip(current_method)
        .and_then(|(type_name, method_name)| {
            facts
                .catch_facts_for(type_name, method_name)
                .into_iter()
                .find(|fact| fact.clause_index == clause_index && fact.types == catch.types)
                .map(|fact| fact.assignable_from.as_slice())
        });
    if let Some(fact_names) = fact_names {
        for ty in fact_names {
            let name = exception_class_name(ty);
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

fn catch_assignable_class_names(catch_types: &[JavaType], facts: &FactSet) -> Vec<String> {
    let mut out = catch_types
        .iter()
        .map(exception_class_name)
        .collect::<Vec<_>>();
    for exception in &facts.exception_facts {
        let exception_ty = JavaType::Named {
            name: exception.type_name.clone(),
            args: Vec::new(),
        };
        if catch_types.iter().any(|catch_ty| {
            exception_class_name(catch_ty) == exception.type_name
                || facts
                    .hierarchy_subtype_policy(&exception_ty, catch_ty)
                    .reaches
                || crate::stdlib::is_known_java_subtype(
                    &exception.type_name,
                    &exception_class_name(catch_ty),
                )
        }) && !out.contains(&exception.type_name)
        {
            out.push(exception.type_name.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn block_terminates_with_exception_boundary(block: &Block) -> bool {
    block
        .statements
        .last()
        .is_some_and(stmt_terminates_with_exception_boundary)
}

fn stmt_terminates_with_exception_boundary(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) | Stmt::Throw(_) => true,
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_terminates_with_exception_boundary(block)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_terminates_with_exception_boundary(then_block)
                && else_block
                    .as_ref()
                    .is_some_and(block_terminates_with_exception_boundary)
        }
        Stmt::Switch { .. } => true,
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => matches!(
            plan_try_stmt(resources, body, catches, finally_block.as_ref()),
            TryStmtPlan::CatchOnly {
                completion: TryBlockCompletion::Terminates,
                ..
            } | TryStmtPlan::WithResources {
                completion: TryBlockCompletion::Terminates,
                ..
            } | TryStmtPlan::FinallyOnly {
                completion: TryBlockCompletion::Terminates,
                ..
            } | TryStmtPlan::CatchFinally {
                completion: TryBlockCompletion::Terminates,
                ..
            }
        ),
        _ => false,
    }
}

pub fn plan_block_exception_completion(block: &Block) -> TryBlockCompletion {
    if block_terminates_with_exception_boundary(block) {
        return TryBlockCompletion::Terminates;
    }
    TryBlockCompletion::FallsThroughUnit
}

fn collect_method_exception_plans_for_type(
    ty: &TypeDecl,
    facts: &FactSet,
    fallibility: Option<&ProgramFallibilityPlan>,
    methods: &mut BTreeMap<(String, String), MethodExceptionPlan>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let mut plan = plan_method_exceptions(ty, method, facts);
                if let Some(fallibility) = fallibility {
                    let key = CallableKey::Method {
                        type_name: ty.name.clone(),
                        method_name: method
                            .rust_name
                            .clone()
                            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
                    };
                    plan.wrap_returns_in_result |= fallibility.returns_result(&key);
                }
                methods.insert(method_exception_key(ty, method), plan);
            }
            Member::Type(nested) => {
                collect_method_exception_plans_for_type(nested, facts, fallibility, methods)
            }
            _ => {}
        }
    }
}

fn method_exception_key(ty: &TypeDecl, method: &MethodDecl) -> (String, String) {
    (
        ty.name.clone(),
        method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
    )
}

pub fn plan_throw_expr(expr: &Expr) -> ThrowExprPlan<'_> {
    match expr {
        Expr::New { ty, args, .. } => ThrowExprPlan::NewException {
            class_name: exception_class_name(ty),
            args,
        },
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } if cause_or_self_rethrow_target(condition, then_expr, else_expr).is_some() => {
            let (wrapper, class_name) =
                cause_or_self_rethrow_target(condition, then_expr, else_expr)
                    .expect("checked cause/self rethrow pattern");
            ThrowExprPlan::CauseOrSelfIfInstance {
                wrapper,
                class_names: vec![class_name],
            }
        }
        Expr::Var(_) => ThrowExprPlan::ExistingException(expr),
        _ => ThrowExprPlan::FallbackValue(expr),
    }
}

fn cause_or_self_rethrow_target<'a>(
    condition: &'a Expr,
    then_expr: &'a Expr,
    else_expr: &'a Expr,
) -> Option<(&'a Expr, String)> {
    let Expr::InstanceOf { expr: cause, ty } = condition else {
        return None;
    };
    let Expr::Cast { ty: cast_ty, expr } = then_expr else {
        return None;
    };
    if exception_class_name(ty) != exception_class_name(cast_ty) || expr.as_ref() != cause.as_ref()
    {
        return None;
    }
    let wrapper = exception_cause_receiver(cause)?;
    (else_expr == wrapper).then(|| (wrapper, exception_class_name(ty)))
}

fn exception_cause_receiver(expr: &Expr) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    (args.is_empty() && name == "getCause").then_some(receiver.as_ref())
}

pub fn exception_class_name(class_ty: &JavaType) -> String {
    match class_ty {
        JavaType::Named { name, .. } => name.clone(),
        JavaType::Unknown(raw) => raw.clone(),
        _ => "JavaException".to_string(),
    }
}
