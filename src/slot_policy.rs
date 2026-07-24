use std::collections::HashSet;

use crate::effective_facts::EffectiveFacts;
use crate::facts::FactSet;
use crate::java_ir::{
    ConstructorDecl, Expr, JavaType, MethodDecl, Modifier, Param, Stmt, TypeDecl,
};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    apply_borrowed_sequence_content_projection, apply_borrowed_string_content_projection,
    value_shape_for_java_type,
};
use crate::slot_shape::RustSlotShape;
use crate::usage::{
    BindingUse, CallArgumentBoundary, MethodUsageFacts, UsageConstructorKey, UsageFacts,
    UsageMethodKey,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedContentKind {
    StringLike,
    SequenceLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedContentPolicy {
    ContentView { kind: BorrowedContentKind },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedContentCapabilityPolicy {
    pub java_ty: JavaType,
    pub string_content: bool,
    pub sequence_element: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedContentTypePolicy {
    pub java_ty: JavaType,
    pub policy: Option<BorrowedContentPolicy>,
}

impl BorrowedContentPolicy {
    pub const fn string_like_view() -> Self {
        Self::ContentView {
            kind: BorrowedContentKind::StringLike,
        }
    }

    pub const fn sequence_like_view() -> Self {
        Self::ContentView {
            kind: BorrowedContentKind::SequenceLike,
        }
    }
}

pub fn borrowed_content_capability_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> BorrowedContentCapabilityPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    borrowed_content_capability_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn borrowed_content_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> BorrowedContentCapabilityPolicy {
    BorrowedContentCapabilityPolicy {
        java_ty: java_ty.clone(),
        string_content: facts.is_string_like_type(java_ty),
        sequence_element: facts.sequence_element_type_for(java_ty),
    }
}

pub fn method_param_borrowed_content_policy(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
) -> Option<BorrowedContentPolicy> {
    method_param_borrowed_content_policy_with_facts(ty, method, param, usage, &FactSet::default())
}

pub fn method_param_borrowed_content_policy_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Option<BorrowedContentPolicy> {
    let effective_facts = EffectiveFacts::new(facts);
    method_param_borrowed_content_policy_with_effective_facts(
        ty,
        method,
        param,
        usage,
        &effective_facts,
    )
}

pub fn method_param_borrowed_content_policy_with_effective_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &EffectiveFacts<'_>,
) -> Option<BorrowedContentPolicy> {
    let content_policy = borrowed_content_type_policy_with_effective_facts(&param.ty, facts).policy;
    if method.modifiers.contains(&Modifier::Public)
        && content_policy != Some(BorrowedContentPolicy::string_like_view())
    {
        return None;
    }
    let public_string_content_body_proof = method.modifiers.contains(&Modifier::Public)
        && content_policy == Some(BorrowedContentPolicy::string_like_view())
        && public_method_string_param_body_allows_borrowed_content(method, &param.name);
    if method_param_allows_borrowed_content(ty, method, param, usage, facts)
        || public_string_content_body_proof
        || method_return_allows_borrowed_param_content(ty, method, param, usage, facts)
    {
        content_policy
    } else {
        None
    }
}

pub fn constructor_param_borrowed_content_policy(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
) -> Option<BorrowedContentPolicy> {
    constructor_param_borrowed_content_policy_with_facts(
        ty,
        ctor,
        param,
        usage,
        &FactSet::default(),
    )
}

pub fn constructor_param_borrowed_content_policy_with_facts(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Option<BorrowedContentPolicy> {
    let effective_facts = EffectiveFacts::new(facts);
    constructor_param_borrowed_content_policy_with_effective_facts(
        ty,
        ctor,
        param,
        usage,
        &effective_facts,
    )
}

pub fn constructor_param_borrowed_content_policy_with_effective_facts(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &EffectiveFacts<'_>,
) -> Option<BorrowedContentPolicy> {
    if ctor.modifiers.contains(&Modifier::Public)
        || borrowed_content_type_policy_with_effective_facts(&param.ty, facts)
            .policy
            .is_none()
    {
        return None;
    }
    let rust_name = ctor.rust_name.clone().unwrap_or_else(|| "new".to_string());
    let ctor_usage = usage.constructors.get(&UsageConstructorKey {
        type_name: ty.name.clone(),
        rust_name,
    })?;
    if usage_allows_borrowed_content(ctor_usage, &param.name) {
        borrowed_content_type_policy_with_effective_facts(&param.ty, facts).policy
    } else {
        None
    }
}

pub fn apply_borrowed_content_policy(shape: &mut RustSlotShape, policy: BorrowedContentPolicy) {
    match policy {
        BorrowedContentPolicy::ContentView {
            kind: BorrowedContentKind::StringLike,
        } => {
            apply_borrowed_string_content_projection(shape);
        }
        BorrowedContentPolicy::ContentView {
            kind: BorrowedContentKind::SequenceLike,
        } => {
            apply_borrowed_sequence_content_projection(shape);
        }
    }
}

pub fn borrowed_return_shape_from_param(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
) -> Option<RustSlotShape> {
    borrowed_return_shape_from_param_with_facts(
        ty,
        method,
        representations,
        usage,
        &FactSet::default(),
    )
}

pub fn borrowed_return_content_policy_from_param(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
) -> Option<BorrowedContentPolicy> {
    borrowed_return_content_policy_from_param_with_facts(ty, method, usage, &FactSet::default())
}

pub fn borrowed_return_content_policy_from_param_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Option<BorrowedContentPolicy> {
    let effective_facts = EffectiveFacts::new(facts);
    borrowed_return_content_policy_from_param_with_effective_facts(
        ty,
        method,
        usage,
        &effective_facts,
    )
}

pub fn borrowed_return_content_policy_from_param_with_effective_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &EffectiveFacts<'_>,
) -> Option<BorrowedContentPolicy> {
    method.params.iter().find_map(|param| {
        if method_return_allows_borrowed_param_content(ty, method, param, usage, facts) {
            borrowed_content_type_policy_with_effective_facts(&param.ty, facts).policy
        } else {
            None
        }
    })
}

pub fn borrowed_return_shape_from_param_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    let effective_facts = EffectiveFacts::new(facts);
    borrowed_return_shape_from_param_with_effective_facts(
        ty,
        method,
        representations,
        usage,
        &effective_facts,
    )
}

pub fn borrowed_return_shape_from_param_with_effective_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
    facts: &EffectiveFacts<'_>,
) -> Option<RustSlotShape> {
    let param = method.params.iter().find(|param| {
        method_return_allows_borrowed_param_content(ty, method, param, usage, facts)
    })?;
    let mut shape = value_shape_for_java_type(representations, param.ty.clone());
    apply_borrowed_content_policy(
        &mut shape,
        borrowed_return_content_policy_from_param_with_effective_facts(ty, method, usage, facts)?,
    );
    Some(shape)
}

pub fn borrowed_content_type_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> BorrowedContentTypePolicy {
    let effective_facts = EffectiveFacts::new(facts);
    borrowed_content_type_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn borrowed_content_type_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> BorrowedContentTypePolicy {
    let capability = borrowed_content_capability_policy_with_effective_facts(java_ty, facts);
    let policy = if capability.string_content {
        Some(BorrowedContentPolicy::string_like_view())
    } else if capability.sequence_element.is_some() {
        Some(BorrowedContentPolicy::sequence_like_view())
    } else {
        None
    };
    BorrowedContentTypePolicy {
        java_ty: capability.java_ty,
        policy,
    }
}

fn method_param_allows_borrowed_content(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &EffectiveFacts<'_>,
) -> bool {
    if borrowed_content_type_policy_with_effective_facts(&param.ty, facts)
        .policy
        .is_none()
    {
        return false;
    }
    let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) else {
        return false;
    };
    usage_allows_borrowed_content(method_usage, &param.name)
}

fn method_return_allows_borrowed_param_content(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &EffectiveFacts<'_>,
) -> bool {
    if borrowed_content_type_policy_with_effective_facts(&param.ty, facts)
        .policy
        .is_none()
        || borrowed_content_type_policy_with_effective_facts(&method.return_type, facts)
            .policy
            .is_none()
        || method.modifiers.contains(&Modifier::Public)
        || !method.modifiers.contains(&Modifier::Static)
    {
        return false;
    }
    let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) else {
        return false;
    };
    let Some(binding) = method_usage.bindings.get(&param.name) else {
        return false;
    };
    if !method_exactly_returns_param(method, &param.name) {
        return false;
    }
    !binding.uses.is_empty()
        && binding
            .uses
            .iter()
            .all(|usage| matches!(usage, BindingUse::Returned | BindingUse::ContentRead))
}

fn method_exactly_returns_param(method: &MethodDecl, param_name: &str) -> bool {
    method.body.as_ref().is_some_and(|body| {
        exact_returned_var_names_from_stmts(&body.statements).contains(param_name)
    })
}

fn exact_returned_var_names_from_stmts(statements: &[Stmt]) -> HashSet<&str> {
    let mut names = HashSet::new();
    collect_exact_returned_var_names_from_stmts(statements, &mut names);
    names
}

fn collect_exact_returned_var_names_from_stmts<'a>(
    statements: &'a [Stmt],
    names: &mut HashSet<&'a str>,
) {
    for stmt in statements {
        match stmt {
            Stmt::Return(Some(Expr::Var(name))) => {
                names.insert(name.as_str());
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_exact_returned_var_names_from_stmts(&then_block.statements, names);
                if let Some(else_block) = else_block {
                    collect_exact_returned_var_names_from_stmts(&else_block.statements, names);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::For { body, .. }
            | Stmt::EnhancedFor { body, .. }
            | Stmt::Block(body) => {
                collect_exact_returned_var_names_from_stmts(&body.statements, names);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_exact_returned_var_names_from_stmts(&case.statements, names);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_exact_returned_var_names_from_stmts(resources, names);
                collect_exact_returned_var_names_from_stmts(&body.statements, names);
                for catch in catches {
                    collect_exact_returned_var_names_from_stmts(&catch.body.statements, names);
                }
                if let Some(finally_block) = finally_block {
                    collect_exact_returned_var_names_from_stmts(&finally_block.statements, names);
                }
            }
            Stmt::VarDecl { .. }
            | Stmt::Assign { .. }
            | Stmt::Expr(_)
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::LocalType(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn usage_allows_borrowed_content(usage: &MethodUsageFacts, param_name: &str) -> bool {
    let Some(binding) = usage.bindings.get(param_name) else {
        return false;
    };
    !binding.uses.is_empty()
        && binding.uses.iter().all(|usage| {
            matches!(
                usage,
                BindingUse::ContentRead
                    | BindingUse::PassedToCall {
                        boundary: CallArgumentBoundary::KnownContentRead,
                        ..
                    }
            )
        })
}

fn public_method_string_param_body_allows_borrowed_content(
    method: &MethodDecl,
    param_name: &str,
) -> bool {
    let Some(body) = &method.body else {
        return false;
    };
    let mut saw_param = false;
    block_allows_string_param_content_reads(body, param_name, &mut saw_param) && saw_param
}

fn block_allows_string_param_content_reads(
    block: &crate::java_ir::Block,
    param_name: &str,
    saw_param: &mut bool,
) -> bool {
    block
        .statements
        .iter()
        .all(|stmt| stmt_allows_string_param_content_reads(stmt, param_name, saw_param))
}

fn stmt_allows_string_param_content_reads(
    stmt: &Stmt,
    param_name: &str,
    saw_param: &mut bool,
) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_none_or(|expr| expr_allows_string_param_content_reads(expr, param_name, saw_param)),
        Stmt::Assign { target, value } => {
            !expr_contains_param_var(target, param_name)
                && expr_allows_string_param_content_reads(value, param_name, saw_param)
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            expr_allows_string_param_content_reads(expr, param_name, saw_param)
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => true,
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_allows_string_param_content_reads(condition, param_name, saw_param)
                && block_allows_string_param_content_reads(then_block, param_name, saw_param)
                && else_block.as_ref().is_none_or(|block| {
                    block_allows_string_param_content_reads(block, param_name, saw_param)
                })
        }
        Stmt::While { condition, body } | Stmt::DoWhile { condition, body } => {
            expr_allows_string_param_content_reads(condition, param_name, saw_param)
                && block_allows_string_param_content_reads(body, param_name, saw_param)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .all(|stmt| stmt_allows_string_param_content_reads(stmt, param_name, saw_param))
                && condition.as_ref().is_none_or(|expr| {
                    expr_allows_string_param_content_reads(expr, param_name, saw_param)
                })
                && update
                    .iter()
                    .all(|expr| expr_allows_string_param_content_reads(expr, param_name, saw_param))
                && block_allows_string_param_content_reads(body, param_name, saw_param)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_allows_string_param_content_reads(iterable, param_name, saw_param)
                && block_allows_string_param_content_reads(body, param_name, saw_param)
        }
        Stmt::Switch { selector, cases } => {
            expr_allows_string_param_content_reads(selector, param_name, saw_param)
                && cases.iter().all(|case| {
                    case.labels.iter().all(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_allows_string_param_content_reads(expr, param_name, saw_param)
                        }
                        crate::java_ir::SwitchLabel::Default => true,
                    }) && case.statements.iter().all(|stmt| {
                        stmt_allows_string_param_content_reads(stmt, param_name, saw_param)
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
                .all(|stmt| stmt_allows_string_param_content_reads(stmt, param_name, saw_param))
                && block_allows_string_param_content_reads(body, param_name, saw_param)
                && catches.iter().all(|catch| {
                    block_allows_string_param_content_reads(&catch.body, param_name, saw_param)
                })
                && finally_block.as_ref().is_none_or(|block| {
                    block_allows_string_param_content_reads(block, param_name, saw_param)
                })
        }
        Stmt::Block(block) => block_allows_string_param_content_reads(block, param_name, saw_param),
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .all(|arg| expr_allows_string_param_content_reads(arg, param_name, saw_param)),
        Stmt::LocalType(_) => !type_contains_param_var(stmt, param_name),
    }
}

fn expr_allows_string_param_content_reads(
    expr: &Expr,
    param_name: &str,
    saw_param: &mut bool,
) -> bool {
    match expr {
        Expr::Var(name) if name == param_name => false,
        Expr::Var(_) | Expr::Literal(_) | Expr::This | Expr::Super | Expr::ClassLiteral(_) => true,
        Expr::FieldAccess(receiver, field) => {
            if field == "length"
                && matches!(receiver.as_ref(), Expr::Var(name) if name == param_name)
            {
                *saw_param = true;
                true
            } else {
                expr_allows_string_param_content_reads(receiver, param_name, saw_param)
            }
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            if matches!(receiver.as_deref(), Some(Expr::Var(param)) if param == param_name) {
                if !crate::stdlib::is_content_read_method(name) {
                    return false;
                }
                *saw_param = true;
            } else if let Some(receiver) = receiver
                && !expr_allows_string_param_content_reads(receiver, param_name, saw_param)
            {
                return false;
            }
            args.iter().enumerate().all(|(index, arg)| {
                if expr_contains_param_var(arg, param_name)
                    && !crate::stdlib::is_content_read_argument_method(name, index)
                {
                    return false;
                }
                if matches!(arg, Expr::Var(name) if name == param_name) {
                    *saw_param = true;
                    true
                } else {
                    expr_allows_string_param_content_reads(arg, param_name, saw_param)
                }
            })
        }
        Expr::ArrayAccess(array, index) => {
            expr_allows_string_param_content_reads(array, param_name, saw_param)
                && expr_allows_string_param_content_reads(index, param_name, saw_param)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .all(|expr| expr_allows_string_param_content_reads(expr, param_name, saw_param))
                && initializer.as_ref().is_none_or(|items| {
                    items.iter().all(|expr| {
                        expr_allows_string_param_content_reads(expr, param_name, saw_param)
                    })
                })
        }
        Expr::ArrayInitializer(items) => items
            .iter()
            .all(|expr| expr_allows_string_param_content_reads(expr, param_name, saw_param)),
        Expr::Binary { left, right, .. } => {
            expr_allows_string_param_content_reads(left, param_name, saw_param)
                && expr_allows_string_param_content_reads(right, param_name, saw_param)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_allows_string_param_content_reads(expr, param_name, saw_param)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_allows_string_param_content_reads(condition, param_name, saw_param)
                && expr_allows_string_param_content_reads(then_expr, param_name, saw_param)
                && expr_allows_string_param_content_reads(else_expr, param_name, saw_param)
        }
        Expr::New { args, body, .. } => {
            body.is_none()
                && args
                    .iter()
                    .all(|arg| expr_allows_string_param_content_reads(arg, param_name, saw_param))
        }
        Expr::Lambda { body, .. } => !lambda_body_contains_param_var(body, param_name),
        Expr::MethodReference { .. } | Expr::Raw(_) => !expr_contains_param_var(expr, param_name),
    }
}

fn expr_contains_param_var(expr: &Expr, param_name: &str) -> bool {
    match expr {
        Expr::Var(name) => name == param_name,
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            expr_contains_param_var(receiver, param_name)
        }
        Expr::ArrayAccess(array, index) => {
            expr_contains_param_var(array, param_name) || expr_contains_param_var(index, param_name)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_contains_param_var(expr, param_name))
                || initializer.as_ref().is_some_and(|items| {
                    items
                        .iter()
                        .any(|expr| expr_contains_param_var(expr, param_name))
                })
        }
        Expr::ArrayInitializer(items) => items
            .iter()
            .any(|expr| expr_contains_param_var(expr, param_name)),
        Expr::Binary { left, right, .. } => {
            expr_contains_param_var(left, param_name) || expr_contains_param_var(right, param_name)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_contains_param_var(condition, param_name)
                || expr_contains_param_var(then_expr, param_name)
                || expr_contains_param_var(else_expr, param_name)
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| expr_contains_param_var(receiver, param_name))
                || args
                    .iter()
                    .any(|arg| expr_contains_param_var(arg, param_name))
        }
        Expr::New { args, body, .. } => {
            body.is_some()
                || args
                    .iter()
                    .any(|arg| expr_contains_param_var(arg, param_name))
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_contains_param_var(expr, param_name)
        }
        Expr::Lambda { body, .. } => lambda_body_contains_param_var(body, param_name),
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => false,
    }
}

fn lambda_body_contains_param_var(body: &crate::java_ir::LambdaBody, param_name: &str) -> bool {
    match body {
        crate::java_ir::LambdaBody::Expr(expr) => expr_contains_param_var(expr, param_name),
        crate::java_ir::LambdaBody::Block(block) => block.statements.iter().any(|stmt| {
            let mut saw_param = false;
            !stmt_allows_string_param_content_reads(stmt, param_name, &mut saw_param) || saw_param
        }),
    }
}

fn type_contains_param_var(_stmt: &Stmt, _param_name: &str) -> bool {
    true
}
