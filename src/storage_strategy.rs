//! How values of a source type are held — steps 1 and 2 of
//! [0002b](../claude_audits/0002b-shared-representation-design.md).
//!
//! Java is a reference-semantics language for every non-primitive type, and
//! `ObjectRepresentation` has no shared case, so lowering answers "I need a
//! second handle to this object" with `clone()`. Where that copy is observable
//! the output compiles and computes a different answer
//! ([0002](../claude_audits/0002-object-aliasing-has-no-representation.md)).
//!
//! This module is deliberately **not** the repair. It computes the decision and
//! reports where lowering disagrees with it, which is what turns "we do not know
//! how big 0002b is" into a number. Nothing here changes the emitted Rust; the
//! representation work is sequenced after the measurement, on purpose.
//!
//! ## Per type, not per allocation site
//!
//! A translator emits one struct definition and one set of method signatures per
//! Java type. If two allocation sites of `Node` had different Rust types then
//! `Node b = a;` between them would not typecheck, so precision per site cannot
//! be spent. The decision is one per source type, taken as the join over every
//! use — the same way `HierarchyStrategy` is already decided.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::DiagnosticBag;
use crate::java_ir::{Block, Expr, JavaType, Member, Program, Stmt, TypeDecl};
use crate::usage::UsageFacts;

/// How values of a source type are held.
///
/// Ordered by how much sharing must be supported. The plan takes the join over
/// all uses, so the order must be monotone: adding a use can only move a type
/// up, which is what makes the fixpoint terminate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StorageStrategy {
    /// No observable aliasing anywhere. Today's behaviour — and, once this is
    /// consulted, a justified choice rather than an assumed one.
    Value,
    /// Aliased, never mutated after sharing. `Rc<T>`: no borrow flag, no panic
    /// risk.
    SharedImmutable,
    /// Aliased and mutated, single-threaded. `Rc<RefCell<T>>`.
    SharedLocal,
    /// Aliased and mutated, reachable across a thread boundary.
    /// `Arc<Mutex<T>>`.
    SharedSync,
}

impl StorageStrategy {
    /// Join is max, because the order is "how much sharing must be supported".
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }

    pub fn is_shared(self) -> bool {
        self != StorageStrategy::Value
    }

    pub fn name(self) -> &'static str {
        match self {
            StorageStrategy::Value => "Value",
            StorageStrategy::SharedImmutable => "SharedImmutable",
            StorageStrategy::SharedLocal => "SharedLocal",
            StorageStrategy::SharedSync => "SharedSync",
        }
    }
}

/// How a value of a type can become reachable from somewhere else.
///
/// The domain mirrors `crate::lattice::EscapeLatticeValue`, which was defined
/// for exactly this and never had a consumer. Ordered: a later variant subsumes
/// an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Escape {
    #[default]
    LocalOnly,
    Returned,
    Stored,
    UnknownBoundary,
}

/// The evidence behind one type's decision, kept so the report can say *why*.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeStorageFacts {
    pub escape: Escape,
    /// `T b = a;` with `a` a bare handle — the shape that makes lowering clone.
    pub locally_aliased: bool,
    /// Any field write through a value of the type, or a method that writes
    /// through its receiver.
    pub mutated: bool,
    /// Reachable from a thread entry point.
    pub threaded: bool,
}

impl TypeStorageFacts {
    /// The decision rule from the 0002b design, verbatim:
    ///
    /// ```text
    /// escapes(T) ∈ {Stored, UnknownBoundary} ∨ locally_aliased(T)
    ///   ? (mutated(T) ? (threaded(T) ? SharedSync : SharedLocal) : SharedImmutable)
    ///   : Value
    /// ```
    ///
    /// `UnknownBoundary` lands on the shared side deliberately: an unmodelled
    /// external call could retain the reference, and getting that wrong is a
    /// silent wrong answer, whereas over-sharing costs a pointer indirection.
    pub fn strategy(&self) -> StorageStrategy {
        let shared =
            matches!(self.escape, Escape::Stored | Escape::UnknownBoundary) || self.locally_aliased;
        if !shared {
            return StorageStrategy::Value;
        }
        if !self.mutated {
            return StorageStrategy::SharedImmutable;
        }
        if self.threaded {
            StorageStrategy::SharedSync
        } else {
            StorageStrategy::SharedLocal
        }
    }
}

/// One decision per source type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoragePlan {
    types: BTreeMap<String, TypeStorageFacts>,
}

impl StoragePlan {
    pub fn facts(&self, type_name: &str) -> Option<&TypeStorageFacts> {
        self.types.get(type_name)
    }

    pub fn strategy(&self, type_name: &str) -> StorageStrategy {
        self.types
            .get(type_name)
            .map(TypeStorageFacts::strategy)
            .unwrap_or(StorageStrategy::Value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &TypeStorageFacts)> {
        self.types.iter()
    }

    /// Types the plan says need sharing. Every one is a place where lowering
    /// currently copies where Java aliased.
    pub fn shared_types(&self) -> Vec<(&String, StorageStrategy)> {
        self.types
            .iter()
            .filter_map(|(name, facts)| {
                let strategy = facts.strategy();
                strategy.is_shared().then_some((name, strategy))
            })
            .collect()
    }
}

/// Compute the plan for a program.
pub fn plan_storage_strategies(program: &Program, usage: &UsageFacts) -> StoragePlan {
    let mut types = BTreeMap::new();
    let mut declared = BTreeSet::new();
    for ty in &program.types {
        collect_declared_types(ty, &mut declared);
    }
    for name in &declared {
        types.insert(name.clone(), TypeStorageFacts::default());
    }

    let threaded = threaded_type_names(program, &declared);
    for ty in &program.types {
        collect_type_facts(ty, program, usage, &declared, &threaded, &mut types);
    }
    StoragePlan { types }
}

/// Report every type the plan says should be shared while lowering holds it by
/// value.
///
/// This is the work queue for the representation change, and the size estimate
/// the design asks for before anything expensive starts. It is a warning, not an
/// error: the output is what it always was, and the diagnostic says what that
/// costs.
pub fn report_storage_disagreements(
    program: &Program,
    usage: &UsageFacts,
    into: &mut DiagnosticBag,
) {
    let plan = plan_storage_strategies(program, usage);
    for (name, strategy) in plan.shared_types() {
        let facts = plan.facts(name).expect("shared type has facts");
        into.warning(
            "storage_strategy_disagreement",
            format!(
                "{name} needs {} (escape={:?}, locally_aliased={}, mutated={}, threaded={}) \
                 but is lowered by value, so an alias becomes a copy",
                strategy.name(),
                facts.escape,
                facts.locally_aliased,
                facts.mutated,
                facts.threaded,
            ),
        );
    }
}

fn collect_declared_types(ty: &TypeDecl, into: &mut BTreeSet<String>) {
    into.insert(ty.name.clone());
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_declared_types(nested, into);
        }
    }
}

/// Types reachable from a thread entry point.
///
/// Conservative and shallow on purpose: `SharedSync` is sequenced last in the
/// design (after [0030a](../claude_audits/0030-java-statics-are-thread-local.md)),
/// so this only has to be right enough to keep the tier from being reported as
/// empty when it is not.
fn threaded_type_names(program: &Program, declared: &BTreeSet<String>) -> BTreeSet<String> {
    let mut threaded = BTreeSet::new();
    for ty in &program.types {
        collect_threaded(ty, declared, &mut threaded);
    }
    threaded
}

fn collect_threaded(ty: &TypeDecl, declared: &BTreeSet<String>, into: &mut BTreeSet<String>) {
    let implements_thread = ty
        .implements
        .iter()
        .any(|name| matches!(simple_name(name), "Runnable" | "Callable"))
        || ty
            .extends
            .as_deref()
            .is_some_and(|name| simple_name(name) == "Thread");
    if implements_thread && declared.contains(&ty.name) {
        into.insert(ty.name.clone());
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_threaded(nested, declared, into);
        }
    }
}

fn collect_type_facts(
    ty: &TypeDecl,
    program: &Program,
    usage: &UsageFacts,
    declared: &BTreeSet<String>,
    threaded: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    if threaded.contains(&ty.name)
        && let Some(facts) = into.get_mut(&ty.name)
    {
        facts.threaded = true;
    }

    for member in &ty.members {
        match member {
            Member::Field(field) => {
                // A field of type T is a second place a T can be reached from.
                if let Some(name) = declared_type_name(&field.ty, declared)
                    && let Some(facts) = into.get_mut(&name)
                {
                    facts.escape = facts.escape.max(Escape::Stored);
                }
            }
            Member::Method(method) => {
                if let Some(name) = declared_type_name(&method.return_type, declared)
                    && let Some(facts) = into.get_mut(&name)
                {
                    facts.escape = facts.escape.max(Escape::Returned);
                }
                // A method that writes through its receiver makes its *owner*
                // mutated. This is `mutates_self` from `usage`, already a
                // fixpoint over self-calls, so a method that mutates only by
                // delegating counts.
                if usage
                    .source_method_usage(ty, method)
                    .is_some_and(|facts| facts.mutates_self)
                    && let Some(facts) = into.get_mut(&ty.name)
                {
                    facts.mutated = true;
                }
                if let Some(body) = &method.body {
                    collect_block_facts(body, ty, program, declared, into);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    collect_block_facts(body, ty, program, declared, into);
                }
            }
            Member::Type(nested) => {
                collect_type_facts(nested, program, usage, declared, threaded, into)
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_block_facts(
    block: &Block,
    owner: &TypeDecl,
    program: &Program,
    declared: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for stmt in &block.statements {
        collect_stmt_facts(stmt, owner, program, declared, into);
    }
}

fn collect_stmt_facts(
    stmt: &Stmt,
    owner: &TypeDecl,
    program: &Program,
    declared: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    match stmt {
        Stmt::VarDecl {
            ty, initializer, ..
        } => {
            // `T b = a;` where `a` is a bare handle: a second name for one
            // object. This is exactly the shape that makes lowering insert a
            // clone, which is why it is the alias signal rather than a general
            // "two references exist" query.
            if let Some(initializer) = initializer
                && expr_is_bare_handle(initializer)
                && let Some(name) = declared_type_name(ty, declared)
                && let Some(facts) = into.get_mut(&name)
            {
                facts.locally_aliased = true;
            }
        }
        Stmt::Assign { target, value } => {
            if expr_is_bare_handle(value)
                && matches!(target, Expr::FieldAccess(..) | Expr::ArrayAccess(..))
            {
                // An alias stored into a field or an element. The type is not
                // available here without a type environment, so charge the
                // owner: it is the type whose object graph now has two edges to
                // one value.
                if let Some(facts) = into.get_mut(&owner.name) {
                    facts.escape = facts.escape.max(Escape::Stored);
                }
            }
            if let Expr::FieldAccess(receiver, _) = target
                && let Some(name) = handle_type_name(receiver, owner, declared)
                && let Some(facts) = into.get_mut(&name)
            {
                facts.mutated = true;
            }
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_block_facts(then_block, owner, program, declared, into);
            if let Some(else_block) = else_block {
                collect_block_facts(else_block, owner, program, declared, into);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            collect_block_facts(body, owner, program, declared, into)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                collect_stmt_facts(resource, owner, program, declared, into);
            }
            collect_block_facts(body, owner, program, declared, into);
            for catch in catches {
                collect_block_facts(&catch.body, owner, program, declared, into);
            }
            if let Some(finally_block) = finally_block {
                collect_block_facts(finally_block, owner, program, declared, into);
            }
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_facts(stmt, owner, program, declared, into);
                }
            }
        }
        _ => {}
    }
}

/// Whether an expression is a bare reference to an existing object, as opposed
/// to something that produces a fresh one.
fn expr_is_bare_handle(expr: &Expr) -> bool {
    matches!(expr, Expr::Var(_) | Expr::FieldAccess(..) | Expr::This)
}

/// The declared type a handle refers to, when that is cheap to see.
fn handle_type_name(expr: &Expr, owner: &TypeDecl, declared: &BTreeSet<String>) -> Option<String> {
    match expr {
        Expr::This => declared.contains(&owner.name).then(|| owner.name.clone()),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            field_type_name(owner, field, declared)
        }
        _ => None,
    }
}

fn field_type_name(owner: &TypeDecl, field: &str, declared: &BTreeSet<String>) -> Option<String> {
    owner.members.iter().find_map(|member| match member {
        Member::Field(decl) if decl.name == field => declared_type_name(&decl.ty, declared),
        _ => None,
    })
}

/// The declared source type a Java type names, ignoring anything external.
fn declared_type_name(ty: &JavaType, declared: &BTreeSet<String>) -> Option<String> {
    let JavaType::Named { name, .. } = ty else {
        return None;
    };
    if declared.contains(name) {
        return Some(name.clone());
    }
    let simple = simple_name(name);
    declared
        .iter()
        .find(|declared| simple_name(declared) == simple)
        .cloned()
}

fn simple_name(name: &str) -> &str {
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_order_is_monotone_and_join_is_max() {
        assert!(StorageStrategy::Value < StorageStrategy::SharedImmutable);
        assert!(StorageStrategy::SharedImmutable < StorageStrategy::SharedLocal);
        assert!(StorageStrategy::SharedLocal < StorageStrategy::SharedSync);
        assert_eq!(
            StorageStrategy::Value.join(StorageStrategy::SharedLocal),
            StorageStrategy::SharedLocal
        );
        assert_eq!(
            StorageStrategy::SharedSync.join(StorageStrategy::Value),
            StorageStrategy::SharedSync
        );
    }

    #[test]
    fn the_decision_rule_matches_the_design_table() {
        let value = TypeStorageFacts::default();
        assert_eq!(value.strategy(), StorageStrategy::Value);

        let aliased_immutable = TypeStorageFacts {
            locally_aliased: true,
            ..Default::default()
        };
        assert_eq!(
            aliased_immutable.strategy(),
            StorageStrategy::SharedImmutable
        );

        let aliased_mutable = TypeStorageFacts {
            locally_aliased: true,
            mutated: true,
            ..Default::default()
        };
        assert_eq!(aliased_mutable.strategy(), StorageStrategy::SharedLocal);

        let threaded = TypeStorageFacts {
            locally_aliased: true,
            mutated: true,
            threaded: true,
            ..Default::default()
        };
        assert_eq!(threaded.strategy(), StorageStrategy::SharedSync);
    }

    /// `Returned` alone is not sharing: the value moves out, it does not become
    /// reachable from two places. `Stored` is.
    #[test]
    fn returned_alone_stays_value_but_stored_does_not() {
        let returned = TypeStorageFacts {
            escape: Escape::Returned,
            ..Default::default()
        };
        assert_eq!(returned.strategy(), StorageStrategy::Value);

        let stored = TypeStorageFacts {
            escape: Escape::Stored,
            ..Default::default()
        };
        assert_eq!(stored.strategy(), StorageStrategy::SharedImmutable);
    }

    /// An unmodelled boundary could retain the reference, so it must land on
    /// the shared side: a wrong answer there is silent, over-sharing is not.
    #[test]
    fn an_unknown_boundary_is_shared() {
        let unknown = TypeStorageFacts {
            escape: Escape::UnknownBoundary,
            mutated: true,
            ..Default::default()
        };
        assert_eq!(unknown.strategy(), StorageStrategy::SharedLocal);
    }
}
