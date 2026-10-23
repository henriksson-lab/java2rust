use crate::effective_facts::EffectiveFacts;
use crate::facts::FactSet;
use crate::java_ir::{
    BinaryOp, ConstructorDecl, Expr, JavaType, LambdaBody, Member, MethodDecl, Modifier, Param,
    Stmt, TypeDecl, UnaryOp,
};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    apply_borrowed_sequence_content_projection, apply_borrowed_string_content_projection,
    value_shape_for_java_type,
};
use crate::slot_shape::RustSlotShape;
use crate::usage::{BindingUse, MethodUsageFacts, UsageFacts};

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
    string_content: bool,
    sequence_element: Option<JavaType>,
}

impl BorrowedContentCapabilityPolicy {
    pub fn string_content(&self) -> bool {
        self.string_content
    }

    pub fn sequence_element(&self) -> Option<&JavaType> {
        self.sequence_element.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedContentTypePolicy {
    java_ty: JavaType,
    policy: Option<BorrowedContentPolicy>,
}

impl BorrowedContentTypePolicy {
    pub fn java_ty(&self) -> &JavaType {
        &self.java_ty
    }

    pub fn policy(&self) -> Option<BorrowedContentPolicy> {
        self.policy
    }
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
    let content_policy =
        borrowed_content_type_policy_with_effective_facts(&param.ty, facts).policy();
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
            .policy()
            .is_none()
    {
        return None;
    }
    let ctor_usage = usage.source_constructor_usage(ty, ctor)?;
    if usage_allows_borrowed_content(ctor_usage, param) {
        borrowed_content_type_policy_with_effective_facts(&param.ty, facts).policy()
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
            borrowed_content_type_policy_with_effective_facts(&param.ty, facts).policy()
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
    let policy = if capability.string_content() {
        Some(BorrowedContentPolicy::string_like_view())
    } else if facts.native_functional_carrier(java_ty).is_some() {
        // A callable/iterable value owns a lazy protocol, not contiguous
        // collection storage that can be replaced by a borrowed Rust slice.
        None
    } else if crate::stdlib::java_collection_kind_for_type(java_ty)
        .is_some_and(|kind| kind != crate::stdlib::JavaCollectionKind::List)
    {
        // Sets and maps are containers but not contiguous Java sequences.
        // Rendering their borrowed content as a Rust slice invents
        // `.as_slice()` on HashSet/HashMap and, more importantly, loses their
        // membership/key semantics. Keep their native carrier ABI.
        None
    } else if capability.sequence_element().is_some() {
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
        .policy()
        .is_none()
    {
        return false;
    }
    let Some(method_usage) = usage.source_method_usage(ty, method) else {
        return false;
    };
    // A private source method with a body cannot retain a parameter it never
    // uses.  Its string/sequence slot can therefore be borrowed even when the
    // usage collector has no read evidence (for example, an intentionally
    // empty callback sink).  Do not extend this to bodyless context summaries:
    // those are declarations for code outside the translated program.
    if method.body.is_some()
        && method_usage
            .binding_facts_for_param(param)
            .is_some_and(|binding| binding.uses().is_empty())
    {
        return true;
    }
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
        .policy()
        .is_none()
        || borrowed_content_type_policy_with_effective_facts(&method.return_type, facts)
            .policy()
            .is_none()
        || method.modifiers.contains(&Modifier::Public)
        || !method.modifiers.contains(&Modifier::Static)
    {
        return false;
    }
    let Some(method_usage) = usage.source_method_usage(ty, method) else {
        return false;
    };
    let Some(binding) = method_usage.binding_facts_for_param(param) else {
        return false;
    };
    if param.binding.is_some() {
        if !binding
            .uses()
            .iter()
            .any(|usage| matches!(usage, BindingUse::Returned))
        {
            return false;
        }
    } else {
        if !idless_compatibility_exact_returned_param_is_assignment_safe(method, &param.source_name)
        {
            return false;
        }
    }
    !binding.uses().is_empty()
        && binding
            .uses()
            .iter()
            .all(|usage| matches!(usage, BindingUse::Returned | BindingUse::ContentRead))
}

pub(crate) fn idless_compatibility_exact_returned_param_is_assignment_safe(
    method: &MethodDecl,
    binding_name: &str,
) -> bool {
    method_exactly_returns_param(method, binding_name)
        && !method_assigns_to_binding(method, binding_name)
}

fn method_assigns_to_binding(method: &MethodDecl, binding_name: &str) -> bool {
    method
        .body
        .as_ref()
        .is_some_and(|body| stmts_assign_to_binding(&body.statements, binding_name, false))
}

fn stmts_assign_to_binding(
    statements: &[Stmt],
    binding_name: &str,
    mut binding_shadowed: bool,
) -> bool {
    for stmt in statements {
        if stmt_assigns_to_binding(stmt, binding_name, binding_shadowed) {
            return true;
        }
        if stmt_declares_binding_name(stmt, binding_name) {
            binding_shadowed = true;
        }
    }
    false
}

fn stmt_assigns_to_binding(stmt: &Stmt, binding_name: &str, binding_shadowed: bool) -> bool {
    match stmt {
        Stmt::Assign { target, value } => {
            (!binding_shadowed && expr_is_binding_assignment_target(target, binding_name))
                || expr_assigns_to_binding(target, binding_name, binding_shadowed)
                || expr_assigns_to_binding(value, binding_name, binding_shadowed)
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            expr_assigns_to_binding(expr, binding_name, binding_shadowed)
        }
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_assigns_to_binding(expr, binding_name, binding_shadowed)),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            let then_binding_shadowed = binding_shadowed
                || expr_declares_binding_name_for_true_branch(condition, binding_name);
            let else_binding_shadowed = binding_shadowed
                || expr_declares_binding_name_for_false_branch(condition, binding_name);
            expr_assigns_to_binding(condition, binding_name, binding_shadowed)
                || stmts_assign_to_binding(
                    &then_block.statements,
                    binding_name,
                    then_binding_shadowed,
                )
                || else_block.as_ref().is_some_and(|block| {
                    stmts_assign_to_binding(&block.statements, binding_name, else_binding_shadowed)
                })
        }
        Stmt::While {
            condition, body, ..
        } => {
            let body_binding_shadowed = binding_shadowed
                || expr_declares_binding_name_for_true_branch(condition, binding_name);
            expr_assigns_to_binding(condition, binding_name, binding_shadowed)
                || stmts_assign_to_binding(&body.statements, binding_name, body_binding_shadowed)
        }
        Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_assigns_to_binding(condition, binding_name, binding_shadowed)
                || stmts_assign_to_binding(&body.statements, binding_name, binding_shadowed)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let init_assigns = stmts_assign_to_binding(init, binding_name, binding_shadowed);
            let for_binding_shadowed = binding_shadowed
                || init
                    .iter()
                    .any(|stmt| stmt_declares_binding_name(stmt, binding_name));
            let for_body_binding_shadowed = for_binding_shadowed
                || condition.as_ref().is_some_and(|expr| {
                    expr_declares_binding_name_for_true_branch(expr, binding_name)
                });
            init_assigns
                || condition.as_ref().is_some_and(|expr| {
                    expr_assigns_to_binding(expr, binding_name, for_binding_shadowed)
                })
                || update
                    .iter()
                    .any(|expr| expr_assigns_to_binding(expr, binding_name, for_binding_shadowed))
                || stmts_assign_to_binding(
                    &body.statements,
                    binding_name,
                    for_body_binding_shadowed,
                )
        }
        Stmt::EnhancedFor {
            iterable,
            body,
            source_var_name,
            ..
        } => {
            expr_assigns_to_binding(iterable, binding_name, binding_shadowed)
                || stmts_assign_to_binding(
                    &body.statements,
                    binding_name,
                    binding_shadowed || source_var_name == binding_name,
                )
        }
        Stmt::Switch { selector, cases } => {
            expr_assigns_to_binding(selector, binding_name, binding_shadowed)
                || switch_cases_assign_to_binding(cases, binding_name, binding_shadowed)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let resource_binding_shadowed = binding_shadowed
                || resources
                    .iter()
                    .any(|stmt| stmt_declares_binding_name(stmt, binding_name));
            stmts_assign_to_binding(resources, binding_name, binding_shadowed)
                || stmts_assign_to_binding(
                    &body.statements,
                    binding_name,
                    resource_binding_shadowed,
                )
                || catches.iter().any(|catch| {
                    stmts_assign_to_binding(
                        &catch.body.statements,
                        binding_name,
                        binding_shadowed || catch.source_name == binding_name,
                    )
                })
                || finally_block.as_ref().is_some_and(|block| {
                    stmts_assign_to_binding(&block.statements, binding_name, binding_shadowed)
                })
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            stmts_assign_to_binding(&body.statements, binding_name, binding_shadowed)
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|arg| expr_assigns_to_binding(arg, binding_name, binding_shadowed)),
        Stmt::LocalType(_)
        | Stmt::Return(None)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    }
}

fn switch_cases_assign_to_binding(
    cases: &[crate::java_ir::SwitchCase],
    binding_name: &str,
    mut binding_shadowed: bool,
) -> bool {
    for case in cases {
        let case_binding_shadowed =
            binding_shadowed || switch_case_labels_declare_binding_name(case, binding_name);
        if case.labels.iter().any(|label| match label {
            crate::java_ir::SwitchLabel::Expr(expr) => {
                expr_assigns_to_binding(expr, binding_name, binding_shadowed)
            }
            crate::java_ir::SwitchLabel::Default => false,
        }) || stmts_assign_to_binding(&case.statements, binding_name, case_binding_shadowed)
        {
            return true;
        }
        if case
            .statements
            .iter()
            .any(|stmt| stmt_declares_binding_name(stmt, binding_name))
        {
            binding_shadowed = true;
        }
    }
    false
}

fn switch_case_labels_declare_binding_name(
    case: &crate::java_ir::SwitchCase,
    binding_name: &str,
) -> bool {
    case.labels.iter().any(|label| match label {
        crate::java_ir::SwitchLabel::Expr(expr) => {
            expr_declares_binding_name_for_true_branch(expr, binding_name)
        }
        crate::java_ir::SwitchLabel::Default => false,
    })
}

fn switch_expr_case_labels_declare_binding_name(
    case: &crate::java_ir::SwitchExprCase,
    binding_name: &str,
) -> bool {
    case.labels.iter().any(|label| match label {
        crate::java_ir::SwitchLabel::Expr(expr) => {
            expr_declares_binding_name_for_true_branch(expr, binding_name)
        }
        crate::java_ir::SwitchLabel::Default => false,
    })
}

fn stmt_declares_binding_name(stmt: &Stmt, binding_name: &str) -> bool {
    match stmt {
        Stmt::VarDecl { source_name, .. } => source_name == binding_name,
        _ => false,
    }
}

fn expr_assigns_to_binding(expr: &Expr, binding_name: &str, binding_shadowed: bool) -> bool {
    match expr {
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            (!binding_shadowed && expr_is_binding_assignment_target(left, binding_name))
                || expr_assigns_to_binding(left, binding_name, binding_shadowed)
                || expr_assigns_to_binding(right, binding_name, binding_shadowed)
        }
        Expr::Unary {
            op: UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec,
            expr,
        } => {
            (!binding_shadowed && expr_is_binding_assignment_target(expr, binding_name))
                || expr_assigns_to_binding(expr, binding_name, binding_shadowed)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_assigns_to_binding(expr, binding_name, binding_shadowed)
        }
        Expr::Binary {
            op: BinaryOp::And,
            left,
            right,
        } => {
            let right_binding_shadowed =
                binding_shadowed || expr_declares_binding_name_for_true_branch(left, binding_name);
            expr_assigns_to_binding(left, binding_name, binding_shadowed)
                || expr_assigns_to_binding(right, binding_name, right_binding_shadowed)
        }
        Expr::Binary {
            op: BinaryOp::Or,
            left,
            right,
        } => {
            let right_binding_shadowed =
                binding_shadowed || expr_declares_binding_name_for_false_branch(left, binding_name);
            expr_assigns_to_binding(left, binding_name, binding_shadowed)
                || expr_assigns_to_binding(right, binding_name, right_binding_shadowed)
        }
        Expr::Binary { left, right, .. } => {
            expr_assigns_to_binding(left, binding_name, binding_shadowed)
                || expr_assigns_to_binding(right, binding_name, binding_shadowed)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            let then_binding_shadowed = binding_shadowed
                || expr_declares_binding_name_for_true_branch(condition, binding_name);
            let else_binding_shadowed = binding_shadowed
                || expr_declares_binding_name_for_false_branch(condition, binding_name);
            expr_assigns_to_binding(condition, binding_name, binding_shadowed)
                || expr_assigns_to_binding(then_expr, binding_name, then_binding_shadowed)
                || expr_assigns_to_binding(else_expr, binding_name, else_binding_shadowed)
        }
        Expr::Switch { selector, cases } => {
            expr_assigns_to_binding(selector, binding_name, binding_shadowed)
                || cases.iter().any(|case| {
                    let case_binding_shadowed = binding_shadowed
                        || switch_expr_case_labels_declare_binding_name(case, binding_name);
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_assigns_to_binding(expr, binding_name, binding_shadowed)
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || expr_assigns_to_binding(&case.value, binding_name, case_binding_shadowed)
                })
        }
        Expr::Call { receiver, args, .. } => {
            receiver.as_ref().is_some_and(|receiver| {
                expr_assigns_to_binding(receiver, binding_name, binding_shadowed)
            }) || args
                .iter()
                .any(|arg| expr_assigns_to_binding(arg, binding_name, binding_shadowed))
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier.as_ref().is_some_and(|qualifier| {
                expr_assigns_to_binding(qualifier, binding_name, binding_shadowed)
            }) || args
                .iter()
                .any(|arg| expr_assigns_to_binding(arg, binding_name, binding_shadowed))
                || body.as_ref().is_some_and(|members| {
                    members_assign_to_binding(members, binding_name, binding_shadowed)
                })
        }
        Expr::ArrayInitializer(args) => args
            .iter()
            .any(|arg| expr_assigns_to_binding(arg, binding_name, binding_shadowed)),
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_assigns_to_binding(expr, binding_name, binding_shadowed))
                || initializer.as_ref().is_some_and(|items| {
                    items
                        .iter()
                        .any(|expr| expr_assigns_to_binding(expr, binding_name, binding_shadowed))
                })
        }
        Expr::FieldAccess(receiver, _) => {
            expr_assigns_to_binding(receiver, binding_name, binding_shadowed)
        }
        Expr::ArrayAccess(array, index) => {
            expr_assigns_to_binding(array, binding_name, binding_shadowed)
                || expr_assigns_to_binding(index, binding_name, binding_shadowed)
        }
        Expr::Lambda { params, body } => {
            let lambda_binding_shadowed = binding_shadowed
                || params
                    .iter()
                    .any(|param| param.source_name == binding_name || param.name == binding_name);
            match body {
                LambdaBody::Expr(expr) => {
                    expr_assigns_to_binding(expr, binding_name, lambda_binding_shadowed)
                }
                LambdaBody::Block(block) => stmts_assign_to_binding(
                    &block.statements,
                    binding_name,
                    lambda_binding_shadowed,
                ),
            }
        }
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => false,
    }
}

fn members_assign_to_binding(
    members: &[Member],
    binding_name: &str,
    binding_shadowed: bool,
) -> bool {
    members.iter().any(|member| match member {
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(|expr| expr_assigns_to_binding(expr, binding_name, binding_shadowed)),
        Member::Constructor(constructor) => constructor.body.as_ref().is_some_and(|body| {
            stmts_assign_to_binding(&body.statements, binding_name, binding_shadowed)
        }),
        Member::Method(_) | Member::Type(_) | Member::Unsupported { .. } => false,
    })
}

fn expr_is_binding_assignment_target(expr: &Expr, binding_name: &str) -> bool {
    matches!(expr, Expr::Var(name) if name == binding_name)
}

fn expr_declares_binding_name_for_true_branch(expr: &Expr, binding_name: &str) -> bool {
    match expr {
        Expr::InstanceOf {
            binding: Some(name),
            ..
        } => name == binding_name,
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => expr_declares_binding_name_for_false_branch(expr, binding_name),
        Expr::Binary {
            op: BinaryOp::And,
            left,
            right,
        } => {
            expr_declares_binding_name_for_true_branch(left, binding_name)
                || expr_declares_binding_name_for_true_branch(right, binding_name)
        }
        _ => false,
    }
}

fn expr_declares_binding_name_for_false_branch(expr: &Expr, binding_name: &str) -> bool {
    match expr {
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => expr_declares_binding_name_for_true_branch(expr, binding_name),
        Expr::Binary {
            op: BinaryOp::Or,
            left,
            right,
        } => {
            expr_declares_binding_name_for_false_branch(left, binding_name)
                || expr_declares_binding_name_for_false_branch(right, binding_name)
        }
        _ => false,
    }
}

pub(crate) fn method_exactly_returns_param(method: &MethodDecl, param_name: &str) -> bool {
    method.body.as_ref().is_some_and(|body| {
        stmts_return_param_scan(&body.statements, param_name, false).allows_exact_return_param()
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ReturnParamScan {
    saw_return: bool,
    saw_non_exact_return: bool,
}

impl ReturnParamScan {
    fn exact_return() -> Self {
        Self {
            saw_return: true,
            saw_non_exact_return: false,
        }
    }

    fn non_exact_return() -> Self {
        Self {
            saw_return: true,
            saw_non_exact_return: true,
        }
    }

    fn merge(&mut self, other: Self) {
        self.saw_return |= other.saw_return;
        self.saw_non_exact_return |= other.saw_non_exact_return;
    }

    fn allows_exact_return_param(self) -> bool {
        self.saw_return && !self.saw_non_exact_return
    }
}

fn stmts_return_param_scan(
    statements: &[Stmt],
    binding_name: &str,
    mut binding_shadowed: bool,
) -> ReturnParamScan {
    let mut scan = ReturnParamScan::default();
    for stmt in statements {
        scan.merge(stmt_return_param_scan(stmt, binding_name, binding_shadowed));
        if stmt_declares_binding_name(stmt, binding_name) {
            binding_shadowed = true;
        }
    }
    scan
}

fn stmt_return_param_scan(
    stmt: &Stmt,
    binding_name: &str,
    binding_shadowed: bool,
) -> ReturnParamScan {
    match stmt {
        Stmt::Return(Some(expr)) => expr_return_param_scan(expr, binding_name, binding_shadowed),
        Stmt::Return(None) => ReturnParamScan::non_exact_return(),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            let then_binding_shadowed = binding_shadowed
                || expr_declares_binding_name_for_true_branch(condition, binding_name);
            let else_binding_shadowed = binding_shadowed
                || expr_declares_binding_name_for_false_branch(condition, binding_name);
            let mut scan = stmts_return_param_scan(
                &then_block.statements,
                binding_name,
                then_binding_shadowed,
            );
            if let Some(else_block) = else_block {
                scan.merge(stmts_return_param_scan(
                    &else_block.statements,
                    binding_name,
                    else_binding_shadowed,
                ));
            }
            scan
        }
        Stmt::While {
            condition, body, ..
        } => {
            let body_binding_shadowed = binding_shadowed
                || expr_declares_binding_name_for_true_branch(condition, binding_name);
            stmts_return_param_scan(&body.statements, binding_name, body_binding_shadowed)
        }
        Stmt::DoWhile { body, .. } => {
            stmts_return_param_scan(&body.statements, binding_name, binding_shadowed)
        }
        Stmt::For {
            init,
            body,
            condition,
            update: _,
            label: _,
        } => {
            let for_binding_shadowed = binding_shadowed
                || init
                    .iter()
                    .any(|stmt| stmt_declares_binding_name(stmt, binding_name));
            let for_body_binding_shadowed = for_binding_shadowed
                || condition.as_ref().is_some_and(|expr| {
                    expr_declares_binding_name_for_true_branch(expr, binding_name)
                });
            let mut scan = stmts_return_param_scan(init, binding_name, binding_shadowed);
            scan.merge(stmts_return_param_scan(
                &body.statements,
                binding_name,
                for_body_binding_shadowed,
            ));
            scan
        }
        Stmt::EnhancedFor {
            source_var_name,
            body,
            ..
        } => stmts_return_param_scan(
            &body.statements,
            binding_name,
            binding_shadowed || source_var_name == binding_name,
        ),
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            stmts_return_param_scan(&body.statements, binding_name, binding_shadowed)
        }
        Stmt::Switch { cases, .. } => {
            switch_cases_return_param_scan(cases, binding_name, binding_shadowed)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let resource_binding_shadowed = binding_shadowed
                || resources
                    .iter()
                    .any(|stmt| stmt_declares_binding_name(stmt, binding_name));
            let mut scan = stmts_return_param_scan(resources, binding_name, binding_shadowed);
            scan.merge(stmts_return_param_scan(
                &body.statements,
                binding_name,
                resource_binding_shadowed,
            ));
            for catch in catches {
                scan.merge(stmts_return_param_scan(
                    &catch.body.statements,
                    binding_name,
                    binding_shadowed || catch.source_name == binding_name,
                ));
            }
            if let Some(finally_block) = finally_block {
                scan.merge(stmts_return_param_scan(
                    &finally_block.statements,
                    binding_name,
                    binding_shadowed,
                ));
            }
            scan
        }
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Throw(_)
        | Stmt::LocalType(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => ReturnParamScan::default(),
    }
}

fn expr_return_param_scan(
    expr: &Expr,
    binding_name: &str,
    binding_shadowed: bool,
) -> ReturnParamScan {
    match expr {
        Expr::Var(name) if !binding_shadowed && name == binding_name => {
            ReturnParamScan::exact_return()
        }
        Expr::Switch { cases, .. } => {
            let mut scan = ReturnParamScan::default();
            for case in cases {
                let case_binding_shadowed = binding_shadowed
                    || switch_expr_case_labels_declare_binding_name(case, binding_name);
                scan.merge(expr_return_param_scan(
                    &case.value,
                    binding_name,
                    case_binding_shadowed,
                ));
            }
            scan
        }
        _ => ReturnParamScan::non_exact_return(),
    }
}

fn switch_cases_return_param_scan(
    cases: &[crate::java_ir::SwitchCase],
    binding_name: &str,
    mut binding_shadowed: bool,
) -> ReturnParamScan {
    let mut scan = ReturnParamScan::default();
    for case in cases {
        let case_binding_shadowed =
            binding_shadowed || switch_case_labels_declare_binding_name(case, binding_name);
        scan.merge(stmts_return_param_scan(
            &case.statements,
            binding_name,
            case_binding_shadowed,
        ));
        if case
            .statements
            .iter()
            .any(|stmt| stmt_declares_binding_name(stmt, binding_name))
        {
            binding_shadowed = true;
        }
    }
    scan
}

fn usage_allows_borrowed_content(usage: &MethodUsageFacts, param: &Param) -> bool {
    let Some(binding) = usage.binding_facts_for_param(param) else {
        return false;
    };
    !binding.uses().is_empty()
        && binding.uses().iter().all(|usage| {
            matches!(
                usage,
                BindingUse::ContentRead
                    | BindingUse::PassedToCall {
                        boundary: crate::usage::CallArgumentBoundary::KnownContentRead,
                        ..
                    }
                    | BindingUse::ReceiverForCall {
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
            source_names: None,
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

    fn idless_string_param() -> Param {
        Param {
            name: "value".to_string(),
            source_name: "value".to_string(),
            ty: JavaType::String,
            is_varargs: false,
            binding: None,
        }
    }

    fn idless_keyword_string_param() -> Param {
        Param {
            name: "type__".to_string(),
            source_name: "type".to_string(),
            ty: JavaType::String,
            is_varargs: false,
            binding: None,
        }
    }

    fn private_static_idless_method_with_body(statements: Vec<Stmt>) -> MethodDecl {
        MethodDecl {
            name: "borrow".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::String,
            throws: Vec::new(),
            params: vec![idless_string_param()],
            modifiers: vec![Modifier::Static],
            body: Some(Block { statements }),
            span: Span::default(),
        }
    }

    fn usage_with_param_facts(binding: BindingId, uses: Vec<BindingUse>) -> UsageFacts {
        let mut usage = UsageFacts::default();
        let ty = owner_type();
        let method = private_static_returning_param_method(string_param(binding.clone()));
        let method_usage = MethodUsageFacts::from_exact_binding_facts([BindingUseFacts::new(
            "value",
            Some(binding),
            JavaType::String,
            uses,
        )]);
        let legacy_key = UsageMethodKey::current_rust_method_name("Owner", "borrow");
        usage.insert_method_usage(legacy_key, method_usage.clone());
        usage.insert_source_method_usage(&ty, &method, method_usage);
        usage
    }

    fn idless_keyword_usage_with_name_facts(
        method: &MethodDecl,
        uses: Vec<BindingUse>,
    ) -> UsageFacts {
        let mut usage = UsageFacts::default();
        let method_usage =
            MethodUsageFacts::from_idless_compatibility_facts([BindingUseFacts::new(
                "type",
                None,
                JavaType::String,
                uses,
            )]);
        let ty = owner_type();
        let legacy_key = UsageMethodKey::current_rust_method_name("Owner", "borrow");
        usage.insert_method_usage(legacy_key, method_usage.clone());
        usage.insert_source_method_usage(&ty, method, method_usage);
        usage
    }

    fn idless_usage_with_name_facts(method: &MethodDecl, uses: Vec<BindingUse>) -> UsageFacts {
        let mut usage = UsageFacts::default();
        let method_usage =
            MethodUsageFacts::from_idless_compatibility_facts([BindingUseFacts::new(
                "value",
                None,
                JavaType::String,
                uses,
            )]);
        let ty = owner_type();
        let legacy_key = UsageMethodKey::current_rust_method_name("Owner", "borrow");
        usage.insert_method_usage(legacy_key, method_usage.clone());
        usage.insert_source_method_usage(&ty, method, method_usage);
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

    #[test]
    fn borrowed_return_param_content_requires_exact_returned_binding_when_param_bound() {
        let binding = BindingId::new(BindingKind::Param, "value", 0);
        let param = string_param(binding.clone());
        let method = private_static_returning_param_method(param.clone());
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let content_read_only = usage_with_param_facts(binding, vec![BindingUse::ContentRead]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &param,
                &content_read_only,
                &effective_facts
            ),
            "resolver-bound params must prove the exact binding is returned instead of trusting the name-only return scan"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_accepts_all_exact_branch_returns() {
        let method = private_static_idless_method_with_body(vec![Stmt::If {
            condition: Expr::Var("flag".to_string()),
            then_block: Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            },
            else_block: Some(Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            }),
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility may borrow when every observed return is exactly the parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_mixed_non_param_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::If {
            condition: Expr::Var("copy".to_string()),
            then_block: Block {
                statements: vec![Stmt::Return(Some(Expr::Literal(Literal::String(
                    "copy".to_string(),
                ))))],
            },
            else_block: Some(Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            }),
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "one exact return is not enough when another branch returns a fresh value"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_update_expr_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Unary {
                op: UnaryOp::PostInc,
                expr: Box::new(Expr::Var("value".to_string())),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not ignore ++/-- mutations before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_param_policy_uses_source_name_not_escaped_name() {
        let param = idless_keyword_string_param();
        let method = MethodDecl {
            name: "borrow".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::String,
            throws: Vec::new(),
            params: vec![param.clone()],
            modifiers: vec![Modifier::Static],
            body: Some(Block {
                statements: vec![Stmt::Return(Some(Expr::Var("type".to_string())))],
            }),
            span: Span::default(),
        };
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_keyword_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &param,
                &usage,
                &effective_facts
            ),
            "id-less compatibility must query usage and source body text with the Java source name"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_for_update_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::For {
                init: Vec::new(),
                condition: None,
                update: vec![Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(Expr::Var("value".to_string())),
                    right: Box::new(Expr::Literal(Literal::String("next".to_string()))),
                }],
                body: Block {
                    statements: Vec::new(),
                },
                label: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect for-update assignment expressions before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_do_while_condition_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::DoWhile {
                body: Block {
                    statements: Vec::new(),
                },
                condition: Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(Expr::Var("value".to_string())),
                    right: Box::new(Expr::Literal(Literal::String("next".to_string()))),
                },
                label: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect do-while condition assignment expressions before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_constructor_invocation_arg_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::ConstructorInvocation {
                kind: crate::java_ir::ConstructorInvocationKind::This,
                args: vec![Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(Expr::Var("value".to_string())),
                    right: Box::new(Expr::Literal(Literal::String("next".to_string()))),
                }],
                resolved: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect this/super constructor invocation arguments before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_qualified_new_receiver_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::New {
                infer_type_arguments: false,
                ty: JavaType::Unknown("Outer.Inner".to_string()),
                qualifier: Some(Box::new(Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(Expr::Var("value".to_string())),
                    right: Box::new(Expr::Literal(Literal::String("next".to_string()))),
                })),
                args: Vec::new(),
                body: None,
                resolved: None,
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect qualified new receiver expressions before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_assignment_target_index_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Assign {
                target: Expr::ArrayAccess(
                    Box::new(Expr::Var("items".to_string())),
                    Box::new(Expr::Binary {
                        op: BinaryOp::Assign,
                        left: Box::new(Expr::Var("value".to_string())),
                        right: Box::new(Expr::Literal(Literal::String("next".to_string()))),
                    }),
                ),
                value: Expr::Literal(Literal::String("stored".to_string())),
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect assignment target expressions before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_nested_assignment_target_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Binary {
                op: BinaryOp::Assign,
                left: Box::new(Expr::ArrayAccess(
                    Box::new(Expr::Var("items".to_string())),
                    Box::new(Expr::Binary {
                        op: BinaryOp::Assign,
                        left: Box::new(Expr::Var("value".to_string())),
                        right: Box::new(Expr::Literal(Literal::String("next".to_string()))),
                    }),
                )),
                right: Box::new(Expr::Literal(Literal::String("stored".to_string()))),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect assignment expression targets before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_update_target_index_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Unary {
                op: UnaryOp::PostInc,
                expr: Box::new(Expr::ArrayAccess(
                    Box::new(Expr::Var("items".to_string())),
                    Box::new(Expr::Binary {
                        op: BinaryOp::Assign,
                        left: Box::new(Expr::Var("value".to_string())),
                        right: Box::new(Expr::Literal(Literal::String("next".to_string()))),
                    }),
                )),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect evaluated update targets before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_update_target_receiver_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Unary {
                op: UnaryOp::PostInc,
                expr: Box::new(Expr::FieldAccess(
                    Box::new(Expr::Binary {
                        op: BinaryOp::Assign,
                        left: Box::new(Expr::Var("value".to_string())),
                        right: Box::new(Expr::Literal(Literal::String("next".to_string()))),
                    }),
                    "length".to_string(),
                )),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect evaluated update receivers before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_anonymous_field_initializer_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::New {
                infer_type_arguments: false,
                ty: JavaType::Named {
                    name: "Runnable".to_string(),
                    args: Vec::new(),
                },
                qualifier: None,
                args: Vec::new(),
                body: Some(vec![Member::Field(crate::java_ir::FieldDecl {
                    name: "changed".to_string(),
                    ty: JavaType::String,
                    modifiers: Vec::new(),
                    initializer: Some(Expr::Binary {
                        op: BinaryOp::Assign,
                        left: Box::new(Expr::Var("value".to_string())),
                        right: Box::new(Expr::Literal(Literal::String("changed".to_string()))),
                    }),
                    span: Span::default(),
                })]),
                resolved: None,
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect anonymous construction field initializers before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_anonymous_constructor_body_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::New {
                infer_type_arguments: false,
                ty: JavaType::Named {
                    name: "Local".to_string(),
                    args: Vec::new(),
                },
                qualifier: None,
                args: Vec::new(),
                body: Some(vec![Member::Constructor(ConstructorDecl {
                    name: "Local".to_string(),
                    rust_name: None,
                    type_params: Vec::new(),
                    type_param_bounds: Vec::new(),
                    params: Vec::new(),
                    throws: Vec::new(),
                    modifiers: Vec::new(),
                    body: Some(Block {
                        statements: vec![Stmt::Assign {
                            target: Expr::Var("value".to_string()),
                            value: Expr::Literal(Literal::String("changed".to_string())),
                        }],
                    }),
                    span: Span::default(),
                })]),
                resolved: None,
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must inspect anonymous construction constructor bodies before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_shadowed_local_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Block(Block {
                statements: vec![
                    Stmt::VarDecl {
                        ty: JavaType::String,
                        name: "value".to_string(),
                        source_name: "value".to_string(),
                        initializer: Some(Expr::Literal(Literal::String("local".to_string()))),
                        binding: None,
                    },
                    Stmt::Assign {
                        target: Expr::Var("value".to_string()),
                        value: Expr::Literal(Literal::String("changed".to_string())),
                    },
                ],
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a shadowed local assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_shadowed_local_return() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::VarDecl {
                ty: JavaType::String,
                name: "value".to_string(),
                source_name: "value".to_string(),
                initializer: Some(Expr::Literal(Literal::String("local".to_string()))),
                binding: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a shadowed local return as returning the parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_shadowed_catch_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Try {
                resources: Vec::new(),
                body: Block {
                    statements: Vec::new(),
                },
                catches: vec![crate::java_ir::CatchClause {
                    types: vec![JavaType::Unknown("Exception".to_string())],
                    name: "value".to_string(),
                    source_name: "value".to_string(),
                    body: Block {
                        statements: vec![Stmt::Assign {
                            target: Expr::Var("value".to_string()),
                            value: Expr::Literal(Literal::String("changed".to_string())),
                        }],
                    },
                    binding: None,
                }],
                finally_block: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a catch-parameter assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_shadowed_catch_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::Try {
            resources: Vec::new(),
            body: Block {
                statements: Vec::new(),
            },
            catches: vec![crate::java_ir::CatchClause {
                types: vec![JavaType::Unknown("Exception".to_string())],
                name: "value".to_string(),
                source_name: "value".to_string(),
                body: Block {
                    statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
                },
                binding: None,
            }],
            finally_block: None,
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned catch parameter as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_shadowed_resource_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Try {
                resources: vec![Stmt::VarDecl {
                    ty: JavaType::String,
                    name: "value".to_string(),
                    source_name: "value".to_string(),
                    initializer: Some(Expr::Literal(Literal::String("resource".to_string()))),
                    binding: None,
                }],
                body: Block {
                    statements: vec![Stmt::Assign {
                        target: Expr::Var("value".to_string()),
                        value: Expr::Literal(Literal::String("changed".to_string())),
                    }],
                },
                catches: Vec::new(),
                finally_block: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a resource-variable assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_shadowed_resource_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::Try {
            resources: vec![Stmt::VarDecl {
                ty: JavaType::String,
                name: "value".to_string(),
                source_name: "value".to_string(),
                initializer: Some(Expr::Literal(Literal::String("resource".to_string()))),
                binding: None,
            }],
            body: Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            },
            catches: Vec::new(),
            finally_block: None,
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned resource variable as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_pattern_binding_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::If {
                condition: Expr::InstanceOf {
                    expr: Box::new(Expr::Var("input".to_string())),
                    ty: JavaType::String,
                    binding: Some("value".to_string()),
                },
                then_block: Block {
                    statements: vec![Stmt::Assign {
                        target: Expr::Var("value".to_string()),
                        value: Expr::Literal(Literal::String("changed".to_string())),
                    }],
                },
                else_block: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a pattern-binding assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_ternary_pattern_binding_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Ternary {
                condition: Box::new(Expr::InstanceOf {
                    expr: Box::new(Expr::Var("input".to_string())),
                    ty: JavaType::String,
                    binding: Some("value".to_string()),
                }),
                then_expr: Box::new(Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(Expr::Var("value".to_string())),
                    right: Box::new(Expr::Literal(Literal::String("changed".to_string()))),
                }),
                else_expr: Box::new(Expr::Literal(Literal::String("unchanged".to_string()))),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a ternary pattern-binding assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_and_pattern_binding_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(Expr::InstanceOf {
                    expr: Box::new(Expr::Var("input".to_string())),
                    ty: JavaType::String,
                    binding: Some("value".to_string()),
                }),
                right: Box::new(Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(Expr::Var("value".to_string())),
                    right: Box::new(Expr::Literal(Literal::String("changed".to_string()))),
                }),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a short-circuit pattern-binding assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_or_negated_pattern_binding_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(Expr::InstanceOf {
                        expr: Box::new(Expr::Var("input".to_string())),
                        ty: JavaType::String,
                        binding: Some("value".to_string()),
                    }),
                }),
                right: Box::new(Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(Expr::Var("value".to_string())),
                    right: Box::new(Expr::Literal(Literal::String("changed".to_string()))),
                }),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a negated short-circuit pattern-binding assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_pattern_binding_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::If {
            condition: Expr::InstanceOf {
                expr: Box::new(Expr::Var("input".to_string())),
                ty: JavaType::String,
                binding: Some("value".to_string()),
            },
            then_block: Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            },
            else_block: None,
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned pattern binding as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_double_negated_pattern_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::If {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(Expr::InstanceOf {
                        expr: Box::new(Expr::Var("input".to_string())),
                        ty: JavaType::String,
                        binding: Some("value".to_string()),
                    }),
                }),
            },
            then_block: Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            },
            else_block: None,
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned double-negated pattern binding as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_negated_pattern_else_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::If {
                condition: Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(Expr::InstanceOf {
                        expr: Box::new(Expr::Var("input".to_string())),
                        ty: JavaType::String,
                        binding: Some("value".to_string()),
                    }),
                },
                then_block: Block {
                    statements: Vec::new(),
                },
                else_block: Some(Block {
                    statements: vec![Stmt::Assign {
                        target: Expr::Var("value".to_string()),
                        value: Expr::Literal(Literal::String("changed".to_string())),
                    }],
                }),
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a negated-pattern else assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_negated_pattern_else_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::If {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::InstanceOf {
                    expr: Box::new(Expr::Var("input".to_string())),
                    ty: JavaType::String,
                    binding: Some("value".to_string()),
                }),
            },
            then_block: Block {
                statements: Vec::new(),
            },
            else_block: Some(Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            }),
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned negated-pattern else binding as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_while_pattern_binding_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::While {
                condition: Expr::InstanceOf {
                    expr: Box::new(Expr::Var("input".to_string())),
                    ty: JavaType::String,
                    binding: Some("value".to_string()),
                },
                body: Block {
                    statements: vec![Stmt::Assign {
                        target: Expr::Var("value".to_string()),
                        value: Expr::Literal(Literal::String("changed".to_string())),
                    }],
                },
                label: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a while-pattern assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_while_pattern_binding_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::While {
            condition: Expr::InstanceOf {
                expr: Box::new(Expr::Var("input".to_string())),
                ty: JavaType::String,
                binding: Some("value".to_string()),
            },
            body: Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            },
            label: None,
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned while-pattern binding as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_for_pattern_binding_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::For {
                init: Vec::new(),
                condition: Some(Expr::InstanceOf {
                    expr: Box::new(Expr::Var("input".to_string())),
                    ty: JavaType::String,
                    binding: Some("value".to_string()),
                }),
                update: Vec::new(),
                body: Block {
                    statements: vec![Stmt::Assign {
                        target: Expr::Var("value".to_string()),
                        value: Expr::Literal(Literal::String("changed".to_string())),
                    }],
                },
                label: None,
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a for-pattern assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_for_pattern_binding_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::For {
            init: Vec::new(),
            condition: Some(Expr::InstanceOf {
                expr: Box::new(Expr::Var("input".to_string())),
                ty: JavaType::String,
                binding: Some("value".to_string()),
            }),
            update: Vec::new(),
            body: Block {
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            },
            label: None,
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned for-pattern binding as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_switch_shadowed_case_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Switch {
                selector: Expr::Literal(Literal::Int(1)),
                cases: vec![
                    crate::java_ir::SwitchCase {
                        labels: vec![crate::java_ir::SwitchLabel::Expr(Expr::Literal(
                            Literal::Int(0),
                        ))],
                        statements: vec![Stmt::VarDecl {
                            ty: JavaType::String,
                            name: "value".to_string(),
                            source_name: "value".to_string(),
                            initializer: Some(Expr::Literal(Literal::String("local".to_string()))),
                            binding: None,
                        }],
                    },
                    crate::java_ir::SwitchCase {
                        labels: vec![crate::java_ir::SwitchLabel::Default],
                        statements: vec![Stmt::Assign {
                            target: Expr::Var("value".to_string()),
                            value: Expr::Literal(Literal::String("changed".to_string())),
                        }],
                    },
                ],
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a later switch case assignment to the parameter after an earlier case declares the same source name"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_switch_shadowed_case_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::Switch {
            selector: Expr::Literal(Literal::Int(1)),
            cases: vec![
                crate::java_ir::SwitchCase {
                    labels: vec![crate::java_ir::SwitchLabel::Expr(Expr::Literal(
                        Literal::Int(0),
                    ))],
                    statements: vec![Stmt::VarDecl {
                        ty: JavaType::String,
                        name: "value".to_string(),
                        source_name: "value".to_string(),
                        initializer: Some(Expr::Literal(Literal::String("local".to_string()))),
                        binding: None,
                    }],
                },
                crate::java_ir::SwitchCase {
                    labels: vec![crate::java_ir::SwitchLabel::Default],
                    statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
                },
            ],
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a later switch case return as returning the parameter after an earlier case declares the same source name"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_switch_pattern_case_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Switch {
                selector: Expr::Var("input".to_string()),
                cases: vec![crate::java_ir::SwitchCase {
                    labels: vec![crate::java_ir::SwitchLabel::Expr(Expr::InstanceOf {
                        expr: Box::new(Expr::Var("input".to_string())),
                        ty: JavaType::String,
                        binding: Some("value".to_string()),
                    })],
                    statements: vec![Stmt::Assign {
                        target: Expr::Var("value".to_string()),
                        value: Expr::Literal(Literal::String("changed".to_string())),
                    }],
                }],
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a switch pattern-case assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_switch_pattern_case_return() {
        let method = private_static_idless_method_with_body(vec![Stmt::Switch {
            selector: Expr::Var("input".to_string()),
            cases: vec![crate::java_ir::SwitchCase {
                labels: vec![crate::java_ir::SwitchLabel::Expr(Expr::InstanceOf {
                    expr: Box::new(Expr::Var("input".to_string())),
                    ty: JavaType::String,
                    binding: Some("value".to_string()),
                })],
                statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
            }],
        }]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned switch pattern-case binding as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_switch_expression_pattern_case_assignment() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Switch {
                selector: Box::new(Expr::Var("input".to_string())),
                cases: vec![crate::java_ir::SwitchExprCase {
                    labels: vec![crate::java_ir::SwitchLabel::Expr(Expr::InstanceOf {
                        expr: Box::new(Expr::Var("input".to_string())),
                        ty: JavaType::String,
                        binding: Some("value".to_string()),
                    })],
                    value: Expr::Binary {
                        op: BinaryOp::Assign,
                        left: Box::new(Expr::Var("value".to_string())),
                        right: Box::new(Expr::Literal(Literal::String("changed".to_string()))),
                    },
                }],
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a switch expression pattern-case assignment to the returned parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_accepts_switch_expression_returning_param() {
        let method =
            private_static_idless_method_with_body(vec![Stmt::Return(Some(Expr::Switch {
                selector: Box::new(Expr::Var("input".to_string())),
                cases: vec![crate::java_ir::SwitchExprCase {
                    labels: vec![crate::java_ir::SwitchLabel::Default],
                    value: Expr::Var("value".to_string()),
                }],
            }))]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility should treat a switch expression whose arms return the parameter as an exact return"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_switch_expression_pattern_case_return() {
        let method =
            private_static_idless_method_with_body(vec![Stmt::Return(Some(Expr::Switch {
                selector: Box::new(Expr::Var("input".to_string())),
                cases: vec![
                    crate::java_ir::SwitchExprCase {
                        labels: vec![crate::java_ir::SwitchLabel::Expr(Expr::InstanceOf {
                            expr: Box::new(Expr::Var("input".to_string())),
                            ty: JavaType::String,
                            binding: Some("value".to_string()),
                        })],
                        value: Expr::Var("value".to_string()),
                    },
                    crate::java_ir::SwitchExprCase {
                        labels: vec![crate::java_ir::SwitchLabel::Default],
                        value: Expr::Var("value".to_string()),
                    },
                ],
            }))]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat a returned switch-expression pattern binding as returning the method parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_switch_expression_mixed_return() {
        let method =
            private_static_idless_method_with_body(vec![Stmt::Return(Some(Expr::Switch {
                selector: Box::new(Expr::Var("input".to_string())),
                cases: vec![
                    crate::java_ir::SwitchExprCase {
                        labels: vec![crate::java_ir::SwitchLabel::Expr(Expr::Literal(
                            Literal::Int(0),
                        ))],
                        value: Expr::Var("value".to_string()),
                    },
                    crate::java_ir::SwitchExprCase {
                        labels: vec![crate::java_ir::SwitchLabel::Default],
                        value: Expr::Literal(Literal::String("fresh".to_string())),
                    },
                ],
            }))]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must reject switch-expression returns when any arm returns a fresh value"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_rejects_lambda_body_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Lambda {
                params: Vec::new(),
                body: LambdaBody::Block(Block {
                    statements: vec![Stmt::Assign {
                        target: Expr::Var("value".to_string()),
                        value: Expr::Literal(Literal::String("changed".to_string())),
                    }],
                }),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            !method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not treat lambda-body mutation as safe before returning a parameter"
        );
    }

    #[test]
    fn idless_borrowed_return_policy_ignores_lambda_param_shadow_mutation() {
        let method = private_static_idless_method_with_body(vec![
            Stmt::Expr(Expr::Lambda {
                params: vec![crate::java_ir::LambdaParam {
                    name: "value".to_string(),
                    source_name: "value".to_string(),
                    ty: Some(JavaType::String),
                }],
                body: LambdaBody::Expr(Box::new(Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(Expr::Var("value".to_string())),
                    right: Box::new(Expr::Literal(Literal::String("changed".to_string()))),
                })),
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let ty = owner_type();
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let usage = idless_usage_with_name_facts(&method, vec![BindingUse::Returned]);

        assert!(
            method_return_allows_borrowed_param_content(
                &ty,
                &method,
                &method.params[0],
                &usage,
                &effective_facts
            ),
            "id-less compatibility must not attribute a lambda parameter assignment to the outer returned parameter"
        );
    }

    #[test]
    fn borrowed_content_policy_keeps_sets_native_instead_of_slicing_them() {
        let facts = crate::stdlib::rust_stdlib_signature_facts();
        let enum_set = JavaType::Named {
            name: "java.util.EnumSet".to_string(),
            args: vec![JavaType::Named {
                name: "Flag".to_string(),
                args: Vec::new(),
            }],
        };
        let list = JavaType::Named {
            name: "java.util.List".to_string(),
            args: vec![JavaType::String],
        };

        assert!(
            borrowed_content_type_policy(&enum_set, &facts)
                .policy()
                .is_none()
        );
        assert!(
            borrowed_content_type_policy(&list, &facts)
                .policy()
                .is_some()
        );
    }
}
