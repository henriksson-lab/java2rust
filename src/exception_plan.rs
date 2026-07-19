use std::collections::BTreeMap;

use crate::facts::FactSet;
use crate::java_ir::{
    Block, CatchClause, Expr, JavaType, Member, MethodDecl, Program, Stmt, TypeDecl,
};

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
            .get(&(ty.name.clone(), method.name.clone()))
            .copied()
            .unwrap_or(MethodExceptionPlan {
                wrap_returns_in_result: false,
            })
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

pub fn program_uses_java_exception_with_facts(_program: &Program, facts: &FactSet) -> bool {
    facts.has_method_throw_effects()
        || !facts.exception_facts.is_empty()
        || !facts.catch_facts.is_empty()
}

pub fn plan_java_exception_support(program: &Program, facts: &FactSet) -> JavaExceptionSupportPlan {
    JavaExceptionSupportPlan {
        emit_support_item: program_uses_java_exception_with_facts(program, facts),
    }
}

pub fn plan_method_exceptions(
    ty: &TypeDecl,
    method: &MethodDecl,
    facts: &FactSet,
) -> MethodExceptionPlan {
    MethodExceptionPlan {
        wrap_returns_in_result: facts.method_has_thrown_exceptions(&ty.name, &method.name),
    }
}

pub fn plan_program_exceptions(program: &Program, facts: &FactSet) -> ProgramExceptionPlan {
    let mut methods = BTreeMap::new();
    for ty in &program.types {
        collect_method_exception_plans_for_type(ty, facts, &mut methods);
    }
    ProgramExceptionPlan {
        support: plan_java_exception_support(program, facts),
        methods,
    }
}

pub fn plan_try_stmt<'a>(
    resources: &'a [Stmt],
    body: &'a Block,
    catches: &'a [CatchClause],
    finally_block: Option<&'a Block>,
) -> TryStmtPlan<'a> {
    if let Some(finally_block) = finally_block {
        if resources.is_empty()
            && catches.is_empty()
            && plan_block_exception_completion(body) == TryBlockCompletion::FallsThroughUnit
            && plan_block_exception_completion(finally_block)
                == TryBlockCompletion::FallsThroughUnit
        {
            return TryStmtPlan::FinallyOnly {
                body,
                finally_block,
            };
        }
        return TryStmtPlan::Unsupported;
    }
    if catches.is_empty() {
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
    if catches
        .iter()
        .any(|catch| !block_terminates_with_exception_boundary(&catch.body))
    {
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
    let names = current_type
        .zip(current_method)
        .and_then(|(type_name, method_name)| {
            facts
                .catch_facts_for(type_name, method_name)
                .get(clause_index)
                .map(|fact| fact.assignable_from.as_slice())
        })
        .unwrap_or(catch.types.as_slice());
    let mut out = Vec::new();
    for ty in names {
        let name = exception_class_name(ty);
        if !out.contains(&name) {
            out.push(name);
        }
    }
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
        Stmt::Block(block) => block_terminates_with_exception_boundary(block),
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
        } => !matches!(
            plan_try_stmt(resources, body, catches, finally_block.as_ref()),
            TryStmtPlan::Unsupported | TryStmtPlan::FinallyOnly { .. }
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
    methods: &mut BTreeMap<(String, String), MethodExceptionPlan>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                methods.insert(
                    (ty.name.clone(), method.name.clone()),
                    plan_method_exceptions(ty, method, facts),
                );
            }
            Member::Type(nested) => collect_method_exception_plans_for_type(nested, facts, methods),
            _ => {}
        }
    }
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
