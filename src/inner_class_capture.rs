//! Non-static inner classes whose construction cannot be lowered.
//!
//! A non-static inner class captures its enclosing instance by reference. The
//! *access* side of that works: the generated struct carries
//! `__outer: Rc<RefCell<Outer>>` and reads through it, which gives Java's
//! semantics. The *construction* side cannot: a method of `Outer` holds
//! `&mut Outer`, and there is no way to obtain an `Rc<RefCell<Outer>>` from it
//! while `Outer` is represented by value.
//!
//! So unsupported construction is reported by the translator and lowered to an
//! explicit unsupported-feature handoff instead of being treated as ordinary
//! object construction. The historical `panic!`-only path was invisible to a
//! compile-error count, which is exactly the failure mode
//! [0018](../claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md)
//! is about, and what
//! [0023](../claude_audits/0023-inner-class-construction-always-panics.md) asks
//! be reported by the *translator* rather than left as a runtime surprise.
//!
//! This module does not repair anything: the repair is the shared
//! representation in [0002b](../claude_audits/0002b-shared-representation-design.md),
//! and 0023 is its clearest concrete consumer. This makes the cost visible in
//! the meantime.

use crate::diagnostics::DiagnosticBag;
use crate::facts::FactSet;
use crate::java_ir::{Block, Expr, JavaType, LambdaBody, Member, Program, Stmt, TypeDecl};
use std::collections::HashSet;

/// A non-static inner class whose constructor cannot produce its outer handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnconstructableInnerClass {
    /// The inner class, as written in Java.
    inner_type: String,
    /// The enclosing type it captures.
    outer_type: String,
}

impl UnconstructableInnerClass {
    pub(crate) fn new(inner_type: String, outer_type: String) -> Self {
        Self {
            inner_type,
            outer_type,
        }
    }

    pub(crate) fn inner_type(&self) -> &str {
        &self.inner_type
    }

    pub(crate) fn outer_type(&self) -> &str {
        &self.outer_type
    }
}

/// Every inner class in `program` whose construction cannot produce an outer handle.
pub(crate) fn unconstructable_inner_classes(
    program: &Program,
    facts: &FactSet,
) -> Vec<UnconstructableInnerClass> {
    let constructed = unsupported_outer_this_constructed_type_names(program);
    let mut found = Vec::new();
    for ty in &program.types {
        collect_in_type(ty, facts, &constructed, &mut found);
    }
    found
        .into_iter()
        .filter(|inner| !outer_this_constructor_handle_is_implemented(inner.outer_type(), facts))
        .collect()
}

#[cfg(test)]
pub(crate) fn constructed_outer_this_inner_classes(
    program: &Program,
    facts: &FactSet,
) -> Vec<UnconstructableInnerClass> {
    let constructed = constructed_type_names(program);
    let mut found = Vec::new();
    for ty in &program.types {
        collect_in_type(ty, facts, &constructed, &mut found);
    }
    found
}

/// Whether evaluating `block` constructs a non-static inner class whose
/// enclosing instance is `outer_type`.
///
/// This is an identity requirement, not merely a lowering detail: when a
/// constructor reaches such an allocation (possibly through a same-receiver
/// method), the outer object must already live behind its final shared handle.
/// Keep the predicate with capture analysis so storage planning and lowering
/// cannot invent different definitions of "needs the outer identity".
pub(crate) fn block_constructs_inner_for_outer(
    block: &Block,
    outer_type: &str,
    facts: &FactSet,
) -> bool {
    let mut found = false;
    crate::semantic_analysis::walk_block_exprs(block, &mut |expr| {
        if found {
            return;
        }
        let Expr::New { ty, resolved, .. } = expr else {
            return;
        };
        let constructed = resolved
            .as_ref()
            .map(|resolved| resolved.declaring_type.as_str())
            .or_else(|| java_type_name(ty));
        let Some(constructed) = constructed else {
            return;
        };
        found = constructed_inner_outer_matches(constructed, outer_type, facts)
            || (!constructed.contains('.')
                && constructed_inner_outer_matches(
                    &format!("{outer_type}.{constructed}"),
                    outer_type,
                    facts,
                ));
    });
    found
}

fn constructed_inner_outer_matches(
    constructed_type: &str,
    outer_type: &str,
    facts: &FactSet,
) -> bool {
    facts
        .captures
        .outer_this_type_for_type(constructed_type)
        .is_some_and(|constructed_outer| {
            crate::stdlib::canonical_external_receiver_type_key(constructed_outer)
                == crate::stdlib::canonical_external_receiver_type_key(outer_type)
        })
}

/// Reports each as `inner_class_construction_panics`.
///
/// A warning accompanies the generated unsupported-feature handoff so corpus
/// diagnostics can still count the feature gap even when callers do not execute
/// the unsupported path.
pub(crate) fn report_unconstructable_inner_classes(
    program: &Program,
    facts: &FactSet,
    into: &mut DiagnosticBag,
) {
    for inner in unconstructable_inner_classes(program, facts) {
        into.warning(
            "inner_class_construction_panics",
            format!(
                "{} captures an enclosing {} instance, but its constructor cannot obtain a \
                    shared handle to it, so `new {}(…)` lowers to an explicit unsupported \
                 feature. Needs the shared object representation (0002b).",
                inner.inner_type(),
                inner.outer_type(),
                inner.inner_type(),
            ),
        );
    }
}

fn collect_in_type(
    ty: &TypeDecl,
    facts: &FactSet,
    constructed: &HashSet<String>,
    found: &mut Vec<UnconstructableInnerClass>,
) {
    if let Some(outer_type) = facts.captures.outer_this_type_for_type(&ty.name) {
        if constructed_type_names_include(constructed, &ty.name) {
            found.push(UnconstructableInnerClass::new(
                ty.name.clone(),
                outer_type.to_string(),
            ));
        }
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_in_type(nested, facts, constructed, found);
        }
    }
}

#[cfg(test)]
fn constructed_type_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for ty in &program.types {
        collect_constructed_type_names_from_type(ty, &mut names);
    }
    names
}

fn unsupported_outer_this_constructed_type_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for ty in &program.types {
        collect_unsupported_outer_this_constructed_type_names_from_type(ty, &mut names);
    }
    names
}

fn collect_unsupported_outer_this_constructed_type_names_from_type(
    ty: &TypeDecl,
    names: &mut HashSet<String>,
) {
    for member in &ty.members {
        collect_unsupported_outer_this_constructed_type_names_from_member(member, names);
    }
    for block in &ty.static_init_blocks {
        collect_unsupported_outer_this_constructed_type_names_from_block(&block.block, names);
    }
}

fn collect_unsupported_outer_this_constructed_type_names_from_member(
    member: &Member,
    names: &mut HashSet<String>,
) {
    match member {
        Member::Field(field) => {
            if let Some(initializer) = &field.initializer {
                collect_unsupported_outer_this_constructed_type_names_from_expr(initializer, names);
            }
        }
        Member::Method(method) => {
            if let Some(body) = &method.body {
                collect_unsupported_outer_this_constructed_type_names_from_block(body, names);
            }
        }
        Member::Constructor(ctor) => {
            if let Some(body) = &ctor.body {
                collect_unsupported_outer_this_constructed_type_names_from_block(body, names);
            }
        }
        Member::Type(ty) => {
            collect_unsupported_outer_this_constructed_type_names_from_type(ty, names)
        }
        _ => {}
    }
}

fn collect_unsupported_outer_this_constructed_type_names_from_block(
    block: &Block,
    names: &mut HashSet<String>,
) {
    for stmt in &block.statements {
        collect_unsupported_outer_this_constructed_type_names_from_stmt(stmt, names);
    }
}

fn collect_unsupported_outer_this_constructed_type_names_from_stmt(
    stmt: &Stmt,
    names: &mut HashSet<String>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            if let Some(expr) = initializer {
                collect_unsupported_outer_this_constructed_type_names_from_expr(expr, names);
            }
        }
        Stmt::Assign { target, value } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(target, names);
            collect_unsupported_outer_this_constructed_type_names_from_expr(value, names);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(expr, names)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(condition, names);
            collect_unsupported_outer_this_constructed_type_names_from_block(then_block, names);
            if let Some(else_block) = else_block {
                collect_unsupported_outer_this_constructed_type_names_from_block(else_block, names);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(condition, names);
            collect_unsupported_outer_this_constructed_type_names_from_block(body, names);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_unsupported_outer_this_constructed_type_names_from_stmt(stmt, names);
            }
            if let Some(condition) = condition {
                collect_unsupported_outer_this_constructed_type_names_from_expr(condition, names);
            }
            for expr in update {
                collect_unsupported_outer_this_constructed_type_names_from_expr(expr, names);
            }
            collect_unsupported_outer_this_constructed_type_names_from_block(body, names);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(iterable, names);
            collect_unsupported_outer_this_constructed_type_names_from_block(body, names);
        }
        Stmt::Switch { selector, cases } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(selector, names);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_unsupported_outer_this_constructed_type_names_from_expr(
                            expr, names,
                        );
                    }
                }
                for stmt in &case.statements {
                    collect_unsupported_outer_this_constructed_type_names_from_stmt(stmt, names);
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
                collect_unsupported_outer_this_constructed_type_names_from_stmt(resource, names);
            }
            collect_unsupported_outer_this_constructed_type_names_from_block(body, names);
            for catch in catches {
                collect_unsupported_outer_this_constructed_type_names_from_block(
                    &catch.body,
                    names,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_unsupported_outer_this_constructed_type_names_from_block(
                    finally_block,
                    names,
                );
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_unsupported_outer_this_constructed_type_names_from_expr(arg, names);
            }
        }
        Stmt::LocalType(ty) => {
            collect_unsupported_outer_this_constructed_type_names_from_type(ty, names)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_unsupported_outer_this_constructed_type_names_from_block(block, names);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_unsupported_outer_this_constructed_type_names_from_expr(
    expr: &Expr,
    names: &mut HashSet<String>,
) {
    match expr {
        Expr::New {
            ty,
            qualifier,
            args,
            body,
            resolved,
            ..
        } => {
            if let Some(qualifier) = qualifier {
                collect_unsupported_outer_this_constructed_type_names_from_expr(qualifier, names);
            } else {
                if let Some(name) = java_type_name(ty) {
                    insert_constructed_type_name(names, name);
                }
                if let Some(resolved) = resolved
                    && let Some(name) = java_type_name(&resolved.constructed_type)
                {
                    insert_constructed_type_name(names, name);
                }
            }
            for arg in args {
                collect_unsupported_outer_this_constructed_type_names_from_expr(arg, names);
            }
            if let Some(body) = body {
                for member in body {
                    collect_unsupported_outer_this_constructed_type_names_from_member(
                        member, names,
                    );
                }
            }
        }
        Expr::FieldAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(receiver, names);
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(array, names);
            collect_unsupported_outer_this_constructed_type_names_from_expr(index, names);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_unsupported_outer_this_constructed_type_names_from_expr(dimension, names);
            }
            if let Some(initializer) = initializer {
                for item in initializer {
                    collect_unsupported_outer_this_constructed_type_names_from_expr(item, names);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                collect_unsupported_outer_this_constructed_type_names_from_expr(item, names);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(condition, names);
            collect_unsupported_outer_this_constructed_type_names_from_expr(then_expr, names);
            collect_unsupported_outer_this_constructed_type_names_from_expr(else_expr, names);
        }
        Expr::Switch { selector, cases } => {
            collect_unsupported_outer_this_constructed_type_names_from_expr(selector, names);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_unsupported_outer_this_constructed_type_names_from_expr(
                            expr, names,
                        );
                    }
                }
                collect_unsupported_outer_this_constructed_type_names_from_expr(&case.value, names);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_unsupported_outer_this_constructed_type_names_from_expr(receiver, names);
            }
            for arg in args {
                collect_unsupported_outer_this_constructed_type_names_from_expr(arg, names);
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_unsupported_outer_this_constructed_type_names_from_expr(expr, names)
            }
            LambdaBody::Block(block) => {
                collect_unsupported_outer_this_constructed_type_names_from_block(block, names)
            }
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => {}
    }
}

#[cfg(test)]
fn collect_constructed_type_names_from_type(ty: &TypeDecl, names: &mut HashSet<String>) {
    for member in &ty.members {
        collect_constructed_type_names_from_member(member, names);
    }
    for block in &ty.static_init_blocks {
        collect_constructed_type_names_from_block(&block.block, names);
    }
}

#[cfg(test)]
fn collect_constructed_type_names_from_member(member: &Member, names: &mut HashSet<String>) {
    match member {
        Member::Field(field) => {
            if let Some(initializer) = &field.initializer {
                collect_constructed_type_names_from_expr(initializer, names);
            }
        }
        Member::Method(method) => {
            if let Some(body) = &method.body {
                collect_constructed_type_names_from_block(body, names);
            }
        }
        Member::Constructor(constructor) => {
            if let Some(body) = &constructor.body {
                collect_constructed_type_names_from_block(body, names);
            }
        }
        Member::Type(ty) => collect_constructed_type_names_from_type(ty, names),
        Member::Unsupported { .. } => {}
    }
}

#[cfg(test)]
fn collect_constructed_type_names_from_block(block: &Block, names: &mut HashSet<String>) {
    for stmt in &block.statements {
        collect_constructed_type_names_from_stmt(stmt, names);
    }
}

#[cfg(test)]
fn collect_constructed_type_names_from_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            if let Some(expr) = initializer {
                collect_constructed_type_names_from_expr(expr, names);
            }
        }
        Stmt::Assign { target, value } => {
            collect_constructed_type_names_from_expr(target, names);
            collect_constructed_type_names_from_expr(value, names);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_constructed_type_names_from_expr(expr, names)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_constructed_type_names_from_expr(condition, names);
            collect_constructed_type_names_from_block(then_block, names);
            if let Some(else_block) = else_block {
                collect_constructed_type_names_from_block(else_block, names);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_constructed_type_names_from_expr(condition, names);
            collect_constructed_type_names_from_block(body, names);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_constructed_type_names_from_stmt(stmt, names);
            }
            if let Some(condition) = condition {
                collect_constructed_type_names_from_expr(condition, names);
            }
            for expr in update {
                collect_constructed_type_names_from_expr(expr, names);
            }
            collect_constructed_type_names_from_block(body, names);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_constructed_type_names_from_expr(iterable, names);
            collect_constructed_type_names_from_block(body, names);
        }
        Stmt::Switch { selector, cases } => {
            collect_constructed_type_names_from_expr(selector, names);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_constructed_type_names_from_expr(expr, names);
                    }
                }
                for stmt in &case.statements {
                    collect_constructed_type_names_from_stmt(stmt, names);
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
                collect_constructed_type_names_from_stmt(resource, names);
            }
            collect_constructed_type_names_from_block(body, names);
            for catch in catches {
                collect_constructed_type_names_from_block(&catch.body, names);
            }
            if let Some(finally_block) = finally_block {
                collect_constructed_type_names_from_block(finally_block, names);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_constructed_type_names_from_expr(arg, names);
            }
        }
        Stmt::LocalType(ty) => collect_constructed_type_names_from_type(ty, names),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_constructed_type_names_from_block(block, names);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

#[cfg(test)]
fn collect_constructed_type_names_from_expr(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::New {
            ty,
            qualifier,
            args,
            body,
            resolved,
            ..
        } => {
            if let Some(qualifier) = qualifier {
                collect_constructed_type_names_from_expr(qualifier, names);
            }
            if let Some(name) = java_type_name(ty) {
                insert_constructed_type_name(names, name);
            }
            if let Some(resolved) = resolved
                && let Some(name) = java_type_name(&resolved.constructed_type)
            {
                insert_constructed_type_name(names, name);
            }
            for arg in args {
                collect_constructed_type_names_from_expr(arg, names);
            }
            if let Some(body) = body {
                for member in body {
                    collect_constructed_type_names_from_member(member, names);
                }
            }
        }
        Expr::FieldAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            collect_constructed_type_names_from_expr(receiver, names);
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            collect_constructed_type_names_from_expr(array, names);
            collect_constructed_type_names_from_expr(index, names);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_constructed_type_names_from_expr(dimension, names);
            }
            if let Some(initializer) = initializer {
                for item in initializer {
                    collect_constructed_type_names_from_expr(item, names);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                collect_constructed_type_names_from_expr(item, names);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_constructed_type_names_from_expr(condition, names);
            collect_constructed_type_names_from_expr(then_expr, names);
            collect_constructed_type_names_from_expr(else_expr, names);
        }
        Expr::Switch { selector, cases } => {
            collect_constructed_type_names_from_expr(selector, names);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_constructed_type_names_from_expr(expr, names);
                    }
                }
                collect_constructed_type_names_from_expr(&case.value, names);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_constructed_type_names_from_expr(receiver, names);
            }
            for arg in args {
                collect_constructed_type_names_from_expr(arg, names);
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_constructed_type_names_from_expr(expr, names),
            LambdaBody::Block(block) => collect_constructed_type_names_from_block(block, names),
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => {}
    }
}

fn insert_constructed_type_name(names: &mut HashSet<String>, name: &str) {
    names.insert(name.to_string());
}

fn constructed_type_names_include(names: &HashSet<String>, ty_name: &str) -> bool {
    names.contains(ty_name)
}

fn java_type_name(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::Intersection(bounds) => bounds.iter().find_map(java_type_name),
        JavaType::Void
        | JavaType::Primitive(_)
        | JavaType::String
        | JavaType::Array(_)
        | JavaType::Wildcard(_) => None,
    }
}

fn outer_this_constructor_handle_is_implemented(outer_type: &str, facts: &FactSet) -> bool {
    let placement = facts.storage_placement_for(outer_type);
    if !placement.shared() || !placement.interior_mutable() || !placement.is_implemented() {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        constructed_outer_this_inner_classes, outer_this_constructor_handle_is_implemented,
        unconstructable_inner_classes,
    };
    use crate::facts::FactSet;
    use crate::parser::parse_java_source;
    use crate::resolve::resolve_program;
    use crate::storage_strategy::{Escape, TypeStorageFacts};

    #[test]
    fn outer_this_construction_needs_an_implemented_mutable_shared_placement() {
        let mut facts = FactSet::default();
        assert!(!outer_this_constructor_handle_is_implemented(
            "Outer", &facts
        ));

        facts.storage_strategies.insert(
            "Outer".to_string(),
            TypeStorageFacts::default()
                .with_escape(Escape::Stored)
                .with_mutated(false)
                .with_mutation_provably_absent(true),
        );
        assert!(
            !outer_this_constructor_handle_is_implemented("Outer", &facts),
            "SharedImmutable gives Rc<Outer>, but captured inners need Rc<RefCell<Outer>>"
        );

        facts.storage_strategies.insert(
            "Outer".to_string(),
            TypeStorageFacts::default()
                .with_escape(Escape::Stored)
                .with_mutated(true)
                .with_mutation_provably_absent(false),
        );
        assert!(
            outer_this_constructor_handle_is_implemented("Outer", &facts),
            "SharedLocal Rc<RefCell<T>> can carry the outer handle"
        );
    }

    #[test]
    fn qualified_inner_construction_requires_storage_but_is_constructable_from_qualifier() {
        let parsed = parse_java_source(
            r#"
class Outer {
    int base = 1;
    class Inner {
        int value() { return base; }
    }
    Inner make(Outer other) {
        return other.new Inner();
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);

        assert!(
            resolved
                .facts
                .captures
                .outer_this_type_for_type("Outer.Inner")
                .is_some()
                || resolved
                    .facts
                    .captures
                    .outer_this_type_for_type("Inner")
                    .is_some(),
            "{:#?}",
            resolved.facts.captures
        );

        assert_eq!(
            constructed_outer_this_inner_classes(&resolved.program, &resolved.facts),
            vec![super::UnconstructableInnerClass::new(
                "Outer.Inner".to_string(),
                "Outer".to_string(),
            )]
        );
        assert_eq!(
            unconstructable_inner_classes(&resolved.program, &resolved.facts),
            Vec::<super::UnconstructableInnerClass>::new()
        );
    }
}
