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
    if method
        .modifiers
        .iter()
        .any(|modifier| matches!(modifier, Modifier::Public | Modifier::Protected))
    {
        return None;
    }
    if method_param_allows_borrowed_content(ty, method, param, usage, facts)
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
