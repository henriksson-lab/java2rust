//! Which inserted clones stand in for a Java alias that is actually observed.
//!
//! Java assigns object *references*. Rust's `=` on a struct moves the value, so
//! wherever the source of a reference assignment is still read afterwards,
//! lowering cannot move it and inserts a `clone()` instead. That substitutes
//! copy semantics for alias semantics.
//!
//! The substitution is invisible as long as nobody writes through either handle
//! afterwards, and a silently wrong answer as soon as somebody does:
//!
//! ```java
//! Node b = a;   // Java: one object, two names
//! a.v = 5;      // Rust:  two objects, one write
//! return b.v;   // Java 5, translation 0
//! ```
//!
//! Deciding which case a given clone is takes two facts that already exist
//! separately: *is a second handle created here* (syntax), and *is the other
//! handle still read after the write* (the backward liveness fixpoint in
//! [`crate::liveness`]). This module joins them.
//!
//! It is the detection half of audit 0002. The repair half needs a shared
//! representation — `Rc<RefCell<_>>` or an arena — that `ObjectRepresentation`
//! does not have, so until then the honest output is a diagnostic rather than a
//! clone that compiles and lies.

use std::collections::HashSet;

use crate::diagnostics::{DiagnosticBag, Severity};
use crate::java_ir::{Block, Expr, JavaType, Member, Program, Stmt, TypeDecl, UnaryOp};
use crate::liveness::LivenessAnalysis;
use crate::usage::{UsageFacts, UsageMethodKey};

/// A clone that Java would not have made, where the difference is observable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservableAlias {
    /// `Type.member`, for the diagnostic message.
    pub callable: String,
    /// The binding whose value is copied because it is still needed.
    pub original: String,
    /// The second handle Java would have created.
    pub alias: String,
    /// The handle the observed write goes through.
    pub mutated_through: String,
    /// The handle that still reads the object after that write.
    pub observed_through: String,
    /// What the write was, for the message.
    pub mutation: MutationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationKind {
    /// `x.f = …`
    FieldWrite { field: String },
    /// `x[i] = …`
    ElementWrite,
    /// `x.m(…)` where `m` was solved to write through its receiver.
    MutatingCall { method: String },
}

impl MutationKind {
    fn describe(&self, handle: &str) -> String {
        match self {
            MutationKind::FieldWrite { field } => format!("`{handle}.{field} = …`"),
            MutationKind::ElementWrite => format!("`{handle}[…] = …`"),
            MutationKind::MutatingCall { method } => {
                format!("`{handle}.{method}(…)`, which writes through its receiver")
            }
        }
    }
}

/// Every observable alias in the program, in source order per callable.
pub fn observable_aliases(program: &Program, usage: &UsageFacts) -> Vec<ObservableAlias> {
    let mut found = Vec::new();
    for ty in &program.types {
        collect_in_type(ty, usage, &mut found);
    }
    found
}

/// Report observable aliases as warnings. The message names both handles and
/// the write, because the fix is a source-level or representation-level
/// decision that a human has to make.
pub fn report_observable_aliases(program: &Program, usage: &UsageFacts, into: &mut DiagnosticBag) {
    for alias in observable_aliases(program, usage) {
        let ObservableAlias {
            callable,
            original,
            alias: alias_name,
            mutated_through,
            observed_through,
            mutation,
        } = alias;
        into.push(
            Severity::Warning,
            "observable_alias",
            format!(
                "{callable}: `{alias_name} = {original}` aliases one object in Java but is \
                 translated as a copy, and {} is still read through `{observed_through}` \
                 afterwards, so the two sides diverge",
                mutation.describe(&mutated_through)
            ),
            None,
        );
    }
}

fn collect_in_type(ty: &TypeDecl, usage: &UsageFacts, found: &mut Vec<ObservableAlias>) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_in_body(body, &format!("{}.{}", ty.name, method.name), usage, found);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    collect_in_body(body, &format!("{}.<init>", ty.name), usage, found);
                }
            }
            Member::Type(nested) => collect_in_type(nested, usage, found),
            _ => {}
        }
    }
}

fn collect_in_body(
    body: &Block,
    callable: &str,
    usage: &UsageFacts,
    found: &mut Vec<ObservableAlias>,
) {
    // Statement identity in the analysis is the address of the `Stmt`, so the
    // walk below must visit the very same tree that was analyzed.
    let liveness = LivenessAnalysis::for_body(body);

    let mut statements = Vec::new();
    collect_statements(body, &mut statements);

    let aliases = alias_points(&statements, &liveness);
    if aliases.is_empty() {
        return;
    }
    let mutations = mutation_sites(&statements, usage);

    for point in &aliases {
        // One report per alias point. A loop body that writes through the same
        // handle ten times is one defect, and ten copies of the same sentence
        // is how a diagnostic gets ignored.
        let Some((site, other)) = mutations.iter().find_map(|site| {
            let other = if site.handle == point.original {
                &point.alias
            } else if site.handle == point.alias {
                &point.original
            } else {
                return None;
            };
            // The write is only observable if the *other* handle is still read
            // after it. Ordering falls out of this: a write that precedes the
            // declaration of `alias` cannot have `alias` live after it.
            (liveness.binding_is_live_after(site.stmt, other) == Some(true))
                .then_some((site, other))
        }) else {
            continue;
        };
        found.push(ObservableAlias {
            callable: callable.to_string(),
            original: point.original.clone(),
            alias: point.alias.clone(),
            mutated_through: site.handle.clone(),
            observed_through: other.clone(),
            mutation: site.kind.clone(),
        });
    }
}

struct AliasPoint {
    original: String,
    alias: String,
}

/// `T b = a;` where `a` is read again afterwards — exactly the shape that makes
/// lowering insert a clone rather than move.
///
/// Restricted to declarations: the new handle is introduced here, which is what
/// lets a liveness query stand in for "after this point".
fn alias_points(statements: &[&Stmt], liveness: &LivenessAnalysis) -> Vec<AliasPoint> {
    let mut points = Vec::new();
    for stmt in statements {
        let Stmt::VarDecl {
            ty,
            name,
            initializer: Some(Expr::Var(source)),
        } = stmt
        else {
            continue;
        };
        if java_type_is_value_like(ty) || source == name {
            continue;
        }
        if liveness.binding_is_live_after(stmt, source) != Some(true) {
            continue;
        }
        points.push(AliasPoint {
            original: source.clone(),
            alias: name.clone(),
        });
    }
    points
}

/// A primitive assignment copies in Java too, so it is never an alias.
///
/// Deliberately not a list of immutable library types: a type with no reachable
/// write cannot produce a mutation site, so it drops out of the join instead of
/// needing to be named here.
fn java_type_is_value_like(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Primitive(_) | JavaType::Void)
}

struct MutationSite<'a> {
    stmt: &'a Stmt,
    handle: String,
    kind: MutationKind,
}

/// Writes that go *through* a local handle, rather than reassigning it.
///
/// Reassignment (`a = other`) rebinds the name and is already modelled by
/// liveness; it is the writes that reach the pointed-to object that a copy
/// fails to share.
fn mutation_sites<'a>(statements: &[&'a Stmt], usage: &UsageFacts) -> Vec<MutationSite<'a>> {
    let mut sites = Vec::new();
    for stmt in statements {
        let mut here = Vec::new();
        match stmt {
            Stmt::Assign { target, value } => {
                push_write_target(target, &mut here);
                collect_expr_mutations(value, usage, &mut here);
            }
            _ => visit_stmt_exprs(stmt, &mut |expr| {
                collect_expr_mutations(expr, usage, &mut here)
            }),
        }
        for (handle, kind) in here {
            sites.push(MutationSite { stmt, handle, kind });
        }
    }
    sites
}

fn collect_expr_mutations(expr: &Expr, usage: &UsageFacts, out: &mut Vec<(String, MutationKind)>) {
    match expr {
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Assign,
            left,
            right,
        } => {
            push_write_target(left, out);
            collect_expr_mutations(right, usage, out);
        }
        Expr::Unary {
            op: UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec,
            expr,
        } => {
            push_write_target(expr, out);
            collect_expr_mutations(expr, usage, out);
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            resolved,
        } => {
            if let Expr::Var(handle) = receiver.as_ref()
                && call_writes_through_receiver(resolved.as_ref(), name, usage)
            {
                out.push((
                    handle.clone(),
                    MutationKind::MutatingCall {
                        method: name.clone(),
                    },
                ));
            }
            collect_expr_mutations(receiver, usage, out);
            for arg in args {
                collect_expr_mutations(arg, usage, out);
            }
        }
        _ => visit_subexpressions(expr, &mut |inner| collect_expr_mutations(inner, usage, out)),
    }
}

fn push_write_target(target: &Expr, out: &mut Vec<(String, MutationKind)>) {
    match target {
        Expr::FieldAccess(receiver, field) => {
            if let Expr::Var(handle) = receiver.as_ref() {
                out.push((
                    handle.clone(),
                    MutationKind::FieldWrite {
                        field: field.clone(),
                    },
                ));
            }
        }
        Expr::ArrayAccess(receiver, _) => {
            if let Expr::Var(handle) = receiver.as_ref() {
                out.push((handle.clone(), MutationKind::ElementWrite));
            }
        }
        _ => {}
    }
}

/// Whether a call writes through its receiver, from the solved usage facts.
///
/// `mutates_self` is already a fixpoint over self-calls in [`crate::usage`], so
/// a method that only mutates by delegating is included. An unresolved call
/// answers `false`: over-reporting a divergence that is not there would train
/// readers to ignore the diagnostic, which costs more than the miss.
fn call_writes_through_receiver(
    resolved: Option<&crate::java_ir::ResolvedCall>,
    name: &str,
    usage: &UsageFacts,
) -> bool {
    let Some(resolved) = resolved else {
        return false;
    };
    usage
        .methods
        .get(&UsageMethodKey {
            type_name: resolved.declaring_type.clone(),
            method_name: name.to_string(),
        })
        .is_some_and(|facts| facts.mutates_self)
}

fn collect_statements<'a>(block: &'a Block, out: &mut Vec<&'a Stmt>) {
    for stmt in &block.statements {
        out.push(stmt);
        collect_nested_statements(stmt, out);
    }
}

fn collect_nested_statements<'a>(stmt: &'a Stmt, out: &mut Vec<&'a Stmt>) {
    match stmt {
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_statements(then_block, out);
            if let Some(else_block) = else_block {
                collect_statements(else_block, out);
            }
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::EnhancedFor { body, .. } => {
            collect_statements(body, out)
        }
        Stmt::For { init, body, .. } => {
            for stmt in init {
                out.push(stmt);
                collect_nested_statements(stmt, out);
            }
            collect_statements(body, out);
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    out.push(stmt);
                    collect_nested_statements(stmt, out);
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
                out.push(stmt);
                collect_nested_statements(stmt, out);
            }
            collect_statements(body, out);
            for catch in catches {
                collect_statements(&catch.body, out);
            }
            if let Some(finally_block) = finally_block {
                collect_statements(finally_block, out);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_statements(block, out)
        }
        _ => {}
    }
}

/// The expressions a statement evaluates directly, not counting nested blocks —
/// those are reached through [`collect_statements`], which keeps each mutation
/// attributed to the statement liveness knows about.
fn visit_stmt_exprs(stmt: &Stmt, visit: &mut impl FnMut(&Expr)) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                visit(expr);
            }
        }
        Stmt::Assign { target, value } => {
            visit(target);
            visit(value);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => visit(expr),
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                visit(expr);
            }
        }
        Stmt::If { condition, .. }
        | Stmt::While { condition, .. }
        | Stmt::DoWhile { condition, .. } => visit(condition),
        Stmt::For {
            condition, update, ..
        } => {
            if let Some(condition) = condition {
                visit(condition);
            }
            for expr in update {
                visit(expr);
            }
        }
        Stmt::EnhancedFor { iterable, .. } => visit(iterable),
        Stmt::Switch { selector, .. } => visit(selector),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                visit(arg);
            }
        }
        _ => {}
    }
}

fn visit_subexpressions(expr: &Expr, visit: &mut impl FnMut(&Expr)) {
    match expr {
        Expr::FieldAccess(inner, _) | Expr::Cast { expr: inner, .. } => visit(inner),
        Expr::Unary { expr: inner, .. } | Expr::InstanceOf { expr: inner, .. } => visit(inner),
        Expr::ArrayAccess(receiver, index) => {
            visit(receiver);
            visit(index);
        }
        Expr::Binary { left, right, .. } => {
            visit(left);
            visit(right);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            visit(condition);
            visit(then_expr);
            visit(else_expr);
        }
        Expr::Switch { selector, cases } => {
            visit(selector);
            for case in cases {
                visit(&case.value);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                visit(receiver);
            }
            for arg in args {
                visit(arg);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                visit(arg);
            }
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                visit(dimension);
            }
            for expr in initializer.iter().flatten() {
                visit(expr);
            }
        }
        Expr::ArrayInitializer(exprs) => {
            for expr in exprs {
                visit(expr);
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => visit(expr),
            crate::java_ir::LambdaBody::Block(_) => {}
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

/// Names of the locals a body aliases observably — the projection lowering
/// would need if it ever gains a shared representation to spend the answer on.
pub fn observably_aliased_locals(program: &Program, usage: &UsageFacts) -> HashSet<String> {
    observable_aliases(program, usage)
        .into_iter()
        .flat_map(|alias| [alias.original, alias.alias])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_java_source;

    /// Resolve first: a mutating-call mutation site is only visible once calls
    /// carry their declaring type, which is what the real pipeline feeds in.
    fn aliases_in(source: &str) -> Vec<ObservableAlias> {
        let parsed = parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = crate::usage::analyze_usage(&resolved.program);
        observable_aliases(&resolved.program, &usage)
    }

    /// The archetype from audit 0002. Java prints 5; a copy prints 0.
    #[test]
    fn field_write_through_the_original_is_observed_through_the_alias() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    a.v = 5;
                    return b.v;
                }
            }
            "#,
        );
        assert_eq!(found.len(), 1, "expected one observable alias: {found:?}");
        assert_eq!(found[0].original, "a");
        assert_eq!(found[0].alias, "b");
        assert_eq!(found[0].mutated_through, "a");
        assert_eq!(found[0].observed_through, "b");
    }

    /// Symmetric: writing through the new handle is just as observable.
    #[test]
    fn field_write_through_the_alias_is_observed_through_the_original() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    b.v = 7;
                    return a.v;
                }
            }
            "#,
        );
        assert_eq!(found.len(), 1, "expected one observable alias: {found:?}");
        assert_eq!(found[0].mutated_through, "b");
        assert_eq!(found[0].observed_through, "a");
    }

    /// A copy and an alias are indistinguishable when nothing is written
    /// afterwards. Reporting these would bury the real ones.
    #[test]
    fn an_alias_nobody_writes_through_is_not_reported() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    return a.v + b.v;
                }
            }
            "#,
        );
        assert!(found.is_empty(), "no write, so no divergence: {found:?}");
    }

    /// The write happens after the last read of the other handle, so a copy
    /// still agrees with Java. This is the case the liveness join buys.
    #[test]
    fn a_write_after_the_other_handle_is_dead_is_not_reported() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Node { int v; }
                public int run() {
                    Node a = new Node();
                    Node b = a;
                    int seen = b.v;
                    a.v = 5;
                    return seen;
                }
            }
            "#,
        );
        assert!(found.is_empty(), "`b` is dead at the write: {found:?}");
    }

    /// Primitive assignment is a copy in Java too.
    #[test]
    fn primitive_locals_are_not_aliases() {
        let found = aliases_in(
            r#"
            public class Shared {
                public int run() {
                    int a = 1;
                    int b = a;
                    return a + b;
                }
            }
            "#,
        );
        assert!(found.is_empty(), "primitives copy in Java: {found:?}");
    }

    /// A method solved as writing through its receiver is a mutation site, so
    /// the same divergence is found without a syntactic field write.
    #[test]
    fn a_receiver_mutating_call_is_a_mutation_site() {
        let found = aliases_in(
            r#"
            public class Shared {
                static class Counter {
                    int v;
                    void bump() { this.v = this.v + 1; }
                    int get() { return this.v; }
                }
                public int run() {
                    Counter a = new Counter();
                    Counter b = a;
                    a.bump();
                    return b.get();
                }
            }
            "#,
        );
        assert_eq!(
            found.len(),
            1,
            "`bump` writes through its receiver: {found:?}"
        );
        assert!(matches!(
            found[0].mutation,
            MutationKind::MutatingCall { .. }
        ));
    }
}
