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
use crate::usage::{BindingUse, CallArgumentBoundary, MethodUsageFacts, UsageFacts};

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
/// design (after [0030](../claude_audits/done/0030-java-statics-are-thread-local.md)),
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
                if let Some(method_usage) = usage.source_method_usage(ty, method) {
                    if method_usage.mutates_self
                        && let Some(facts) = into.get_mut(&ty.name)
                    {
                        facts.mutated = true;
                    }
                    collect_unknown_boundary_escapes(method_usage, declared, into);
                }
                if let Some(body) = &method.body {
                    collect_block_facts(body, ty, program, declared, false, into);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    collect_block_facts(body, ty, program, declared, true, into);
                }
            }
            Member::Type(nested) => {
                collect_type_facts(nested, program, usage, declared, threaded, into)
            }
            Member::Unsupported { .. } => {}
        }
    }
}

/// A handle handed to a call the translator cannot see through escapes.
///
/// This is the `UnknownBoundary` input the design names and the earlier pass
/// left unproduced, which biased the plan **low** — and under-sharing is a
/// silent wrong answer, while over-sharing costs a pointer indirection. An
/// unmodelled callee may retain the reference, so a value that crosses that
/// boundary must be assumed reachable from a second place.
///
/// The evidence already existed: `usage` classifies every call argument as
/// `InternalSameType`, `KnownContentRead` or `Unknown`, and a binding carries
/// its declared Java type. Nothing new had to be analysed — the same finding as
/// the rest of this design, one input later.
///
/// `ReceiverForCall` is deliberately **not** treated as an escape. Passing an
/// argument gives the callee a handle it can store; being the receiver does
/// not, and `KnownContentRead`/`InternalSameType` say the boundary is
/// understood.
fn collect_unknown_boundary_escapes(
    method_usage: &MethodUsageFacts,
    declared: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for binding in method_usage.bindings.values() {
        let Some(name) = declared_type_name(&binding.java_ty, declared) else {
            continue;
        };
        let crosses_unknown_boundary = binding.uses.iter().any(|use_site| {
            matches!(
                use_site,
                BindingUse::PassedToCall {
                    boundary: CallArgumentBoundary::Unknown,
                    ..
                }
            )
        });
        if crosses_unknown_boundary && let Some(facts) = into.get_mut(&name) {
            facts.escape = facts.escape.max(Escape::UnknownBoundary);
        }
    }
}

fn collect_block_facts(
    block: &Block,
    owner: &TypeDecl,
    program: &Program,
    declared: &BTreeSet<String>,
    in_constructor: bool,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for stmt in &block.statements {
        collect_stmt_facts(stmt, owner, program, declared, in_constructor, into);
    }
}

fn collect_stmt_facts(
    stmt: &Stmt,
    owner: &TypeDecl,
    program: &Program,
    declared: &BTreeSet<String>,
    in_constructor: bool,
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
            if expr_is_bare_handle(value) {
                match target {
                    // `this.f = x` stores an alias *of `f`'s declared type*.
                    // Charging the **owner** instead — which is what this did,
                    // for want of a type environment — marked `Stored` on every
                    // class whose constructor assigns a parameter to a field,
                    // including `this.v = v` where `v` is an `int`. That is
                    // almost every class, so the escape input said "shared" for
                    // types that never leave a local
                    // ([0002b](../claude_audits/0002b-shared-representation-design.md)).
                    //
                    // The field's declared type is available right here, and it
                    // is the correct answer rather than a cheaper one.
                    Expr::FieldAccess(receiver, field_name)
                        if matches!(receiver.as_ref(), Expr::This) =>
                    {
                        if let Some(name) = field_type_name(owner, field_name, declared)
                            && let Some(facts) = into.get_mut(&name)
                        {
                            facts.escape = facts.escape.max(Escape::Stored);
                        }
                    }
                    // A qualified field or an array element: the element type is
                    // genuinely not available here, so the conservative charge
                    // on the owner stands.
                    Expr::FieldAccess(..) | Expr::ArrayAccess(..) => {
                        if let Some(facts) = into.get_mut(&owner.name) {
                            facts.escape = facts.escape.max(Escape::Stored);
                        }
                    }
                    _ => {}
                }
            }
            // A field write inside a *constructor* is not mutation in this
            // lattice's sense. `mutated` distinguishes `SharedImmutable`
            // (`Rc<T>`) from `SharedLocal` (`Rc<RefCell<T>>`), and the question
            // it asks is whether a value is mutated **after being shared** —
            // construction necessarily precedes any alias to the object.
            //
            // Counting it moved exactly the population the design expects to
            // find in `SharedImmutable` ("value objects, keys, strings") into
            // the harder tier: a `final`-field class whose only writes are
            // `this.v = v` was reported `SharedLocal`
            // ([0002b](../claude_audits/0002b-shared-representation-design.md)).
            if !in_constructor
                && let Expr::FieldAccess(receiver, _) = target
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
            collect_block_facts(then_block, owner, program, declared, in_constructor, into);
            if let Some(else_block) = else_block {
                collect_block_facts(else_block, owner, program, declared, in_constructor, into);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            collect_block_facts(body, owner, program, declared, in_constructor, into)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                collect_stmt_facts(resource, owner, program, declared, in_constructor, into);
            }
            collect_block_facts(body, owner, program, declared, in_constructor, into);
            for catch in catches {
                collect_block_facts(&catch.body, owner, program, declared, in_constructor, into);
            }
            if let Some(finally_block) = finally_block {
                collect_block_facts(
                    finally_block,
                    owner,
                    program,
                    declared,
                    in_constructor,
                    into,
                );
            }
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_facts(stmt, owner, program, declared, in_constructor, into);
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

#[cfg(test)]
mod constructor_mutation_tests {
    use super::*;
    use crate::parser::parse_java_source;

    /// Resolves first, because nested type names are qualified during
    /// resolution and the plan is keyed by the qualified name — the same
    /// program the lowering pipeline hands it.
    fn strategy_of(source: &str, type_name: &str) -> StorageStrategy {
        let parsed = parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = crate::usage::analyze_usage(&resolved.program);
        plan_storage_strategies(&resolved.program, &usage).strategy(type_name)
    }

    /// A constructor writing its own fields is **not** mutation in this
    /// lattice.
    ///
    /// `mutated` decides `SharedImmutable` (`Rc<T>`) against `SharedLocal`
    /// (`Rc<RefCell<T>>`, borrow discipline, panic risk), and the question it
    /// asks is whether a value is mutated *after being shared*. Construction
    /// necessarily precedes any alias, so counting it put the exact population
    /// the design expects in `SharedImmutable` — value objects and keys — into
    /// the harder tier.
    #[test]
    fn constructor_field_writes_do_not_make_a_type_mutable() {
        let strategy = strategy_of(
            r#"
public class Outer {
    static final class Key {
        final int v;
        Key(int v) { this.v = v; }
        int get() { return v; }
    }
    static int run() { Key a = new Key(1); Key b = a; return b.get(); }
}
"#,
            "Outer.Key",
        );
        assert_eq!(strategy, StorageStrategy::SharedImmutable);
    }

    /// The negative: a write from an ordinary method still counts, or the
    /// distinction would be meaningless.
    #[test]
    fn a_method_field_write_still_makes_a_type_mutable() {
        let strategy = strategy_of(
            r#"
public class Outer {
    static final class Cell {
        int v;
        Cell(int v) { this.v = v; }
        void set(int x) { this.v = x; }
        int get() { return v; }
    }
    static int run() { Cell a = new Cell(1); Cell b = a; a.set(2); return b.get(); }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedLocal);
    }

    /// A type that is never aliased stays `Value` whichever way it is written —
    /// the escape/alias inputs still gate everything.
    #[test]
    fn an_unaliased_type_stays_held_by_value() {
        let strategy = strategy_of(
            r#"
public class Outer {
    static final class Local {
        int v;
        Local(int v) { this.v = v; }
        int get() { return v; }
    }
    static int run() { Local a = new Local(1); return a.get(); }
}
"#,
            "Outer.Local",
        );
        assert_eq!(strategy, StorageStrategy::Value);
    }
}
