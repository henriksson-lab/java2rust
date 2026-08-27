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
use crate::usage::{BindingUse, MethodUsageFacts, UsageConstructorKey, UsageFacts, UsageMethodKey};

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

pub fn borrowed_content_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> BorrowedContentCapabilityPolicy {
    BorrowedContentCapabilityPolicy {
        string_content: facts.is_string_like_type(java_ty),
        sequence_element: facts.sequence_element_type_for(java_ty),
    }
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
    if method_param_allows_borrowed_content(ty, method, param, usage, facts)
        || method_return_allows_borrowed_param_content(ty, method, param, usage, facts)
    {
        content_policy
    } else {
        None
    }
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
    if usage_allows_borrowed_content(ctor_usage, param) {
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
        java_ty: java_ty.clone(),
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
    usage_allows_borrowed_content(method_usage, param)
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
    let Some(binding) = method_usage.binding_for_declaration(param.binding.as_ref(), &param.name)
    else {
        return false;
    };
    if !method_exactly_returns_param(method, &param.name) {
        return false;
    }
    if param.binding.is_none() && method_assigns_to_binding(method, &param.name) {
        return false;
    }
    !binding.uses.is_empty()
        && binding
            .uses
            .iter()
            .all(|usage| matches!(usage, BindingUse::Returned | BindingUse::ContentRead))
}

pub fn method_assigns_to_binding(method: &MethodDecl, binding_name: &str) -> bool {
    method
        .body
        .as_ref()
        .is_some_and(|body| stmts_assign_to_binding(&body.statements, binding_name))
}

fn stmts_assign_to_binding(statements: &[Stmt], binding_name: &str) -> bool {
    statements
        .iter()
        .any(|stmt| stmt_assigns_to_binding(stmt, binding_name))
}

fn stmt_assigns_to_binding(stmt: &Stmt, binding_name: &str) -> bool {
    match stmt {
        Stmt::Assign { target, .. } => matches!(target, Expr::Var(name) if name == binding_name),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            stmts_assign_to_binding(&then_block.statements, binding_name)
                || else_block
                    .as_ref()
                    .is_some_and(|block| stmts_assign_to_binding(&block.statements, binding_name))
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            stmts_assign_to_binding(&body.statements, binding_name)
        }
        Stmt::For { init, body, .. } => {
            stmts_assign_to_binding(init, binding_name)
                || stmts_assign_to_binding(&body.statements, binding_name)
        }
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| stmts_assign_to_binding(&case.statements, binding_name)),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            stmts_assign_to_binding(resources, binding_name)
                || stmts_assign_to_binding(&body.statements, binding_name)
                || catches
                    .iter()
                    .any(|catch| stmts_assign_to_binding(&catch.body.statements, binding_name))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| stmts_assign_to_binding(&block.statements, binding_name))
        }
        Stmt::LocalType(_)
        | Stmt::VarDecl { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    }
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
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
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

fn usage_allows_borrowed_content(usage: &MethodUsageFacts, param: &Param) -> bool {
    let Some(binding) = usage.binding_for_declaration(param.binding.as_ref(), &param.name) else {
        return false;
    };
    !binding.uses.is_empty()
        && binding.uses.iter().all(|usage| {
            matches!(
                usage,
                BindingUse::ContentRead
                    | BindingUse::PassedToCall {
                        boundary: crate::usage::CallArgumentBoundary::KnownContentRead,
                        ..
                    }
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding_id::{BindingId, BindingKind};
    use crate::diagnostics::Span;
    use crate::java_ir::{Block, Expr, Literal, TypeKind};
    use crate::usage::{BindingUseFacts, UsageMethodKey};

    fn owner_type() -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Class,
            name: "Owner".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: Vec::new(),
            static_init_blocks: Vec::new(),
            span: Span::default(),
        }
    }

    fn string_param(binding: BindingId) -> Param {
        Param {
            name: "value".to_string(),
            source_name: "value".to_string(),
            ty: JavaType::String,
            is_varargs: false,
            binding: Some(binding),
        }
    }

    fn private_static_returning_param_method(param: Param) -> MethodDecl {
        MethodDecl {
            name: "borrow".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::String,
            throws: Vec::new(),
            params: vec![param],
            modifiers: vec![Modifier::Static],
            body: Some(Block {
                statements: vec![
                    Stmt::Block(Block {
                        statements: vec![
                            Stmt::VarDecl {
                                ty: JavaType::String,
                                name: "value".to_string(),
                                source_name: "value".to_string(),
                                initializer: None,
                                binding: Some(BindingId::new(BindingKind::Local, "value", 0)),
                            },
                            Stmt::Assign {
                                target: Expr::Var("value".to_string()),
                                value: Expr::Literal(Literal::String("shadow".to_string())),
                            },
                        ],
                    }),
                    Stmt::Return(Some(Expr::Var("value".to_string()))),
                ],
            }),
            span: Span::default(),
        }
    }

    fn usage_with_param_facts(binding: BindingId, uses: Vec<BindingUse>) -> UsageFacts {
        let mut usage = UsageFacts::default();
        let mut method_usage = MethodUsageFacts::default();
        method_usage.bindings_by_id.insert(
            binding.clone(),
            BindingUseFacts {
                name: "value".to_string(),
                binding: Some(binding),
                java_ty: JavaType::String,
                uses,
            },
        );
        usage.methods.insert(
            UsageMethodKey {
                type_name: "Owner".to_string(),
                method_name: "borrow".to_string(),
            },
            method_usage,
        );
        usage
    }

    #[test]
    fn borrowed_return_param_policy_uses_binding_facts_not_name_assignment_scan() {
        let binding = BindingId::new(BindingKind::Param, "value", 0);
        let param = string_param(binding.clone());
        let method = private_static_returning_param_method(param.clone());
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);

        let returned_only = usage_with_param_facts(binding.clone(), vec![BindingUse::Returned]);
        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &param,
                &returned_only,
                &effective_facts
            ),
            "a stale same-name assignment scan must not override the resolver-bound parameter facts"
        );

        let mutated_exact =
            usage_with_param_facts(binding, vec![BindingUse::Returned, BindingUse::Mutated]);
        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &param,
                &mutated_exact,
                &effective_facts
            ),
            "mutation of the exact parameter binding still rejects borrowed return policy"
        );
    }
}
