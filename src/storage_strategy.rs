//! How values of a source type are held for
//! [0002b](../claude_audits/0002b-shared-representation-design.md).
//!
//! Java is a reference-semantics language for every non-primitive type, and
//! `ObjectRepresentation` has no shared case, so lowering answers "I need a
//! second handle to this object" with `clone()`. Where that copy is observable
//! the output compiles and computes a different answer
//! ([0002](../claude_audits/0002-object-aliasing-has-no-representation.md)).
//!
//! This module started as the measurement layer: compute the decision and report
//! where lowering disagrees with it, turning "we do not know how big 0002b is"
//! into a number. It now also supplies the storage placement consumed by slot
//! rendering and handle wrapping for the implemented `SharedImmutable` and
//! single-threaded `SharedLocal` and thread-reachable `SharedSync` tiers.
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
use crate::facts::FactSet;
use crate::java_ir::{
    Block, Expr, JavaType, LambdaBody, Member, Modifier, Program, Stmt, SwitchLabel, TypeDecl,
    TypeKind, WildcardBound,
};
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
/// Ordered: a later variant subsumes an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Escape {
    #[default]
    LocalOnly,
    Returned,
    Stored,
    UnknownBoundary,
}

/// The evidence behind one type's decision, kept so the report can say *why*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeStorageFacts {
    escape: Escape,
    /// `T b = a;` with `a` a bare handle — the shape that makes lowering clone.
    locally_aliased: bool,
    /// Any field write through a value of the type, or a method that writes
    /// through its receiver.
    mutated: bool,
    /// Reachable from a thread entry point.
    threaded: bool,
    /// Whether every trait this type is reached through can forward a handle.
    ///
    /// A generated trait with a `&mut self` method has no
    /// `impl<..> Trait for Rc<..>`, so a wrapped value cannot satisfy a Java
    /// bound like `Standards<T extends HeaderLine>`. That makes forwardability a
    /// **precondition of the tier**, not a consequence: without it the wrap
    /// produces `doesn't satisfy Rc<FormatLine>: HeaderLineBound`.
    /// See [0002b](../claude_audits/0002b-shared-representation-design.md).
    surface_forwardable: bool,
    /// Whether the absence of mutation could be *proved*.
    ///
    /// `mutated` says a mutation was seen; this says the walk was able to see
    /// them all. A field write through a receiver it cannot type clears this,
    /// and without it a type cannot be `SharedImmutable` however clean it looks.
    mutation_provably_absent: bool,
    /// Reachable from a Java `static` field.
    ///
    /// Independent of `threaded`, and it forces the same answer for a different
    /// reason. [0030](../claude_audits/done/0030-java-statics-are-thread-local.md)
    /// made every Java `static` a process-wide `std::sync::Mutex`, and a
    /// `static` in Rust must be `Sync` — so a type a Java `static` can reach
    /// must be `Send + Sync` **in a single-threaded program with no `Thread`
    /// anywhere**. `Rc` is neither; `Arc` is both.
    static_reachable: bool,
}

impl Default for TypeStorageFacts {
    fn default() -> Self {
        Self {
            escape: Escape::default(),
            locally_aliased: false,
            mutated: false,
            threaded: false,
            // Absence of mutation is assumed provable until the walk meets a
            // field write it cannot attribute to a type.
            mutation_provably_absent: true,
            static_reachable: false,
            // Assumed forwardable until the surface is examined; the plan sets
            // this for every declared type before any placement is read.
            surface_forwardable: true,
        }
    }
}

impl TypeStorageFacts {
    pub fn escape(&self) -> Escape {
        self.escape
    }

    pub fn locally_aliased(&self) -> bool {
        self.locally_aliased
    }

    pub fn mutated(&self) -> bool {
        self.mutated
    }

    pub fn threaded(&self) -> bool {
        self.threaded
    }

    pub fn surface_forwardable(&self) -> bool {
        self.surface_forwardable
    }

    pub fn mutation_provably_absent(&self) -> bool {
        self.mutation_provably_absent
    }

    pub fn static_reachable(&self) -> bool {
        self.static_reachable
    }

    pub fn with_escape(mut self, escape: Escape) -> Self {
        self.escape = escape;
        self
    }

    pub fn with_locally_aliased(mut self, locally_aliased: bool) -> Self {
        self.locally_aliased = locally_aliased;
        self
    }

    pub fn with_mutated(mut self, mutated: bool) -> Self {
        self.mutated = mutated;
        self
    }

    pub fn with_threaded(mut self, threaded: bool) -> Self {
        self.threaded = threaded;
        self
    }

    pub fn with_surface_forwardable(mut self, surface_forwardable: bool) -> Self {
        self.surface_forwardable = surface_forwardable;
        self
    }

    pub fn with_mutation_provably_absent(mut self, mutation_provably_absent: bool) -> Self {
        self.mutation_provably_absent = mutation_provably_absent;
        self
    }

    pub fn with_static_reachable(mut self, static_reachable: bool) -> Self {
        self.static_reachable = static_reachable;
        self
    }

    /// Conservative merge of two observations for the same source type.
    pub fn join(&mut self, other: TypeStorageFacts) {
        self.escape = self.escape.max(other.escape);
        self.locally_aliased |= other.locally_aliased;
        self.mutated |= other.mutated;
        self.threaded |= other.threaded;
        self.mutation_provably_absent &= other.mutation_provably_absent;
        self.static_reachable |= other.static_reachable;
        // Conservative in the same direction as the others: one non-forwardable
        // observation is enough.
        self.surface_forwardable &= other.surface_forwardable;
    }

    /// The decision rule from the 0002b design, with the later static-storage
    /// correction folded in:
    ///
    /// ```text
    /// escapes(T) ∈ {Stored, UnknownBoundary} ∨ locally_aliased(T)
    ///   ? (mutated(T) ? (sync(T) ? SharedSync : SharedLocal) : SharedImmutable)
    ///   : Value
    /// ```
    ///
    /// where `sync(T)` is `threaded(T) ∨ static_reachable(T)`. Java statics are
    /// process-wide Rust statics, so a type reachable from static storage must
    /// use the sync handle/cell even in a single-threaded Java source.
    ///
    /// `UnknownBoundary` lands on the shared side deliberately: an unmodelled
    /// external call could retain the reference, and getting that wrong is a
    /// silent wrong answer, whereas over-sharing costs a pointer indirection.
    /// The three axes, read off the evidence.
    ///
    /// Each is an inference step, not a case in a table:
    /// reachable-from-two-places implies `shared`; mutated-after-sharing implies
    /// `interior_mutable`; reachable-from-a-static-or-a-thread implies `sync`.
    pub fn placement(&self) -> StoragePlacement {
        let shared =
            matches!(self.escape, Escape::Stored | Escape::UnknownBoundary) || self.locally_aliased;
        StoragePlacement::new(
            shared,
            // `SharedImmutable` must be *proved*, not merely unobserved: a
            // missed mutation gives `Rc<T>` for a type that is written through,
            // which does not compile.
            // A non-forwardable surface falls back to holding the value inline,
            // through the same `is_implemented()` path an unproved mutation
            // takes: wrong in a known and reported way rather than not compiling.
            shared && (self.mutated || !self.mutation_provably_absent || !self.surface_forwardable),
            self.threaded || self.static_reachable,
        )
    }

    /// Which pointer this type's shared handle must be.
    pub fn handle(&self) -> SharedHandle {
        if self.threaded || self.static_reachable {
            SharedHandle::Sync
        } else {
            SharedHandle::Local
        }
    }

    pub fn strategy(&self) -> StorageStrategy {
        let shared =
            matches!(self.escape, Escape::Stored | Escape::UnknownBoundary) || self.locally_aliased;
        if !shared {
            return StorageStrategy::Value;
        }
        // `SharedImmutable` is `Rc<T>` with no `RefCell`, so it must be
        // *proved*, not merely unobserved: a missed mutation emits `cannot
        // assign to data in an Rc` and does not compile.
        if !self.mutated && self.mutation_provably_absent && self.surface_forwardable {
            return StorageStrategy::SharedImmutable;
        }
        if self.threaded || self.static_reachable {
            StorageStrategy::SharedSync
        } else {
            StorageStrategy::SharedLocal
        }
    }
}

/// Which pointer a shared type is held behind.
///
/// Orthogonal to [`StorageStrategy`], which says *whether* and *how mutably* a
/// type is shared. This says whether the handle has to be thread-safe, and the
/// two combine: `SharedImmutable` is `Rc<T>` or `Arc<T>`, `SharedLocal` is
/// `Rc<RefCell<T>>` or `Arc<Mutex<T>>`.
///
/// `Rc` is the default because it is cheaper — a non-atomic refcount — and
/// `Arc` is selected only where it is forced, which keeps the common case out
/// of atomics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SharedHandle {
    /// `Rc` — single-threaded, not reachable from a Java `static`.
    #[default]
    Local,
    /// `Arc` — reachable from a thread entry point or from a Java `static`.
    Sync,
}

impl SharedHandle {
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }
}

/// Where a value lives, as three independent yes/no questions.
///
/// This replaces an enumerated `StorageForm` — five named rungs and a `match`
/// over the pair `(strategy, handle)`. That match was a lookup table: it listed
/// the answers instead of deriving them, so a sixth combination would have had
/// to be *remembered* rather than falling out.
///
/// The wrapping is **compositional**. Reading inside out:
///
/// ```text
/// rust_type(T) = slot( handle( cell( T ) ) )
///
///   cell(T)   = T | RefCell<T> | Mutex<T>    interior_mutable, then sync
///   handle(X) = X | Rc<X>      | Arc<X>      shared,           then sync
///   slot(Y)   = Y | &Y                       decided per slot, not per type
/// ```
///
/// `RefCell`/`Mutex` and `Rc`/`Arc` are chosen by the *same* `sync` bit, which
/// is why they always agree — that agreement was an invariant the table had to
/// maintain by hand.
///
/// Each axis is a monotone boolean, so the join is `|` per axis: adding a use
/// can only turn a `false` into a `true`, and the fixpoint terminates because
/// there are three bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StoragePlacement {
    /// The value has an identity that outlives one owner: it needs a pointer.
    ///
    /// From `escape ∈ {Stored, UnknownBoundary} ∨ locally_aliased`.
    shared: bool,
    /// It is mutated *after* being shared, so the pointer needs a cell.
    ///
    /// From `mutated`, and only meaningful when `shared`.
    interior_mutable: bool,
    /// The handle must be `Send + Sync`.
    ///
    /// From `threaded ∨ static_reachable` — a Rust `static` must be `Sync`, and
    /// [0030](../claude_audits/done/0030-java-statics-are-thread-local.md) makes
    /// every Java `static` a real one, so this is reachable with no `Thread` in
    /// the program.
    sync: bool,
}

impl StoragePlacement {
    pub fn new(shared: bool, interior_mutable: bool, sync: bool) -> Self {
        Self {
            shared,
            interior_mutable,
            sync,
        }
    }

    pub fn shared(self) -> bool {
        self.shared
    }

    pub fn interior_mutable(self) -> bool {
        self.interior_mutable
    }

    pub fn sync(self) -> bool {
        self.sync
    }

    /// Join per axis. Monotone, so the plan's fixpoint terminates.
    pub fn join(self, other: Self) -> Self {
        Self::new(
            self.shared || other.shared,
            self.interior_mutable || other.interior_mutable,
            self.sync || other.sync,
        )
    }

    /// The cell a mutated shared value needs, if any.
    pub fn cell_path(self) -> Option<&'static str> {
        if !self.shared || !self.interior_mutable {
            return None;
        }
        Some(if self.sync {
            "std::sync::Mutex"
        } else {
            "std::cell::RefCell"
        })
    }

    /// The pointer a shared value is held behind, if any.
    pub fn handle_path(self) -> Option<&'static str> {
        if !self.shared {
            return None;
        }
        Some(if self.sync {
            "std::sync::Arc"
        } else {
            "std::rc::Rc"
        })
    }

    /// Applies `cell` then `handle` to a rendered inner type.
    ///
    /// The slot step (`&Y`) is *not* here: whether a position owns or borrows is
    /// a property of the slot, not of the type, and a borrowed slot stays `&T`
    /// because the handle derefs to `T`.
    pub fn wrap(self, inner: String) -> String {
        let celled = match self.cell_path() {
            Some(cell) => format!("{cell}<{inner}>"),
            None => inner,
        };
        match self.handle_path() {
            Some(handle) => format!("{handle}<{celled}>"),
            None => celled,
        }
    }

    /// Whether lowering can emit this placement today.
    ///
    /// Interior-mutable shared values are rendered as either
    /// `Rc<RefCell<T>>` or `Arc<Mutex<T>>`; lowering selects the corresponding
    /// short-lived receiver projection at each access.
    pub fn is_implemented(self) -> bool {
        true
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

    pub fn type_names(&self) -> impl Iterator<Item = &str> {
        self.types.keys().map(String::as_str)
    }

    pub fn insert(&mut self, type_name: String, facts: TypeStorageFacts) {
        self.types.insert(type_name, facts);
    }

    pub fn extend_from(&mut self, other: StoragePlan) {
        for (type_name, facts) in other.types {
            self.types
                .entry(type_name)
                .and_modify(|existing| existing.join(facts.clone()))
                .or_insert(facts);
        }
    }

    /// Which pointer `type_name`'s shared handle must be. `Local` (`Rc`) for
    /// anything unknown, which is only ever consulted for a type the plan
    /// already says is shared.
    pub fn handle(&self, type_name: &str) -> SharedHandle {
        self.types
            .get(type_name)
            .map(TypeStorageFacts::handle)
            .unwrap_or_default()
    }

    /// Where values of `type_name` live. `Value` (all axes false) for a type
    /// the plan has never heard of, so "unknown" can never read as "shared".
    pub fn placement(&self, type_name: &str) -> StoragePlacement {
        self.types
            .get(type_name)
            .map(TypeStorageFacts::placement)
            .unwrap_or_default()
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
    plan_storage_strategies_with_facts(program, usage, None)
}

/// Compute the plan with semantic/capture facts that are not present in usage.
pub fn plan_storage_strategies_from_facts(
    program: &Program,
    usage: &UsageFacts,
    facts: &FactSet,
) -> StoragePlan {
    plan_storage_strategies_with_facts(program, usage, Some(facts))
}

fn plan_storage_strategies_with_facts(
    program: &Program,
    usage: &UsageFacts,
    facts: Option<&FactSet>,
) -> StoragePlan {
    let mut types = BTreeMap::new();
    let mut declared = BTreeSet::new();
    for ty in &program.types {
        collect_declared_types(ty, &mut declared);
    }
    for name in &declared {
        types.insert(name.clone(), TypeStorageFacts::default());
    }

    // A Java type *parameter* and a source class can share a name, and
    // `JavaType::Named` cannot tell them apart -- there is no type-variable
    // variant, and slot rendering has no type-parameter scope to consult. A name
    // used as a type parameter anywhere is therefore removed from the plan:
    // wrapping it would put a handle on the parameter (`input: Rc<ELEM>`) rather
    // than on the class. Conservative in the safe direction -- the type keeps
    // today's by-value behaviour, which is wrong in the known and reported way
    // rather than wrong in a way that does not compile.
    // See [0002b](../claude_audits/0002b-shared-representation-design.md).
    let type_param_names = program_type_param_names(program);
    for name in &type_param_names {
        types.remove(name);
        declared.remove(name);
    }

    let threaded = threaded_type_names(program, &declared);
    let static_reachable = static_reachable_type_names(program, &declared);
    let all_static_reachable = statically_reachable_source_type_names(program);
    let mut interfaces = BTreeSet::new();
    for ty in &program.types {
        collect_interface_type_names(ty, &mut interfaces);
    }
    for ty in &program.types {
        collect_type_facts(ty, program, usage, &declared, &threaded, &mut types);
    }
    if let Some(facts) = facts {
        collect_inner_outer_capture_storage_requirements(program, facts, &declared, &mut types);
    }
    for (name, facts) in types.iter_mut() {
        facts.static_reachable = static_reachable.contains(name);
        if all_static_reachable.contains(name)
            && source_type_has_interface_field(program, name, &interfaces)
        {
            // An interface field may hide interior mutable anonymous state.
            // A static Arc<T> would require that opaque field to be Sync;
            // holding T behind a Mutex only requires its interface to be Send.
            facts.mutation_provably_absent = false;
        }
        facts.surface_forwardable =
            type_interface_surface_is_handle_forwardable(name, program, usage);
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
    facts: &FactSet,
    into: &mut DiagnosticBag,
) {
    let plan = plan_storage_strategies_from_facts(program, usage, facts);
    for (name, strategy) in plan.shared_types() {
        let facts = plan.facts(name).expect("shared type has facts");
        // "needs sharing" and "is still lowered by value" stopped being the same
        // statement when the handle landed: an implemented placement *is* behind
        // an `Rc`/`Arc` now, and reporting it as a copy misdescribes the thing
        // 0028's identity lowering is built on. Only the unimplemented tiers --
        // the `RefCell`/`Mutex` rungs -- are still held inline.
        let placement = facts.placement();
        if !placement.is_implemented() {
            into.warning(
                "storage_strategy_disagreement",
                format!(
                    "{name} needs {} (escape={:?}, locally_aliased={}, mutated={}, \
                     mutation_provably_absent={}, threaded={}, static_reachable={}) \
                     but is lowered by value, so an alias becomes a copy",
                    strategy.name(),
                    facts.escape,
                    facts.locally_aliased,
                    facts.mutated,
                    facts.mutation_provably_absent,
                    facts.threaded,
                    facts.static_reachable,
                ),
            );
        }
        if !type_interface_surface_is_handle_forwardable(name, program, usage) {
            into.warning(
                "storage_strategy_surface_not_forwardable",
                format!(
                    "{name} is shared, but an interface it implements has a mutating method: that \
trait lowers to `&mut self`, no `impl<T: Trait> Trait for Rc<T>` can exist, and a handle cannot \
satisfy a generic bound on it. It cannot take the shared tier until that surface is `&self`-only."
                ),
            );
        }
    }
}

/// [`interface_surface_is_handle_forwardable`] by type name.
pub fn type_interface_surface_is_handle_forwardable(
    name: &str,
    program: &Program,
    usage: &UsageFacts,
) -> bool {
    program
        .types
        .iter()
        .find(|ty| ty.name == name)
        .is_none_or(|ty| interface_surface_is_handle_forwardable(ty, program, usage))
}

/// Whether a shared handle can carry this type through the traits it implements.
///
/// A generated trait method whose Java body mutates the receiver lowers to
/// Mutable interface methods are forwarded through `Rc<RefCell<T>>` and
/// `Arc<Mutex<T>>`, so receiver mutability is no longer a precondition of the
/// sharing tier. The retained program/usage arguments document that this query
/// remains the policy boundary for future non-forwardable surfaces (notably a
/// borrowed return that would outlive a cell guard).
pub fn interface_surface_is_handle_forwardable(
    _ty: &TypeDecl,
    _program: &Program,
    _usage: &UsageFacts,
) -> bool {
    true
}

/// Types whose *values* this lattice describes.
///
/// Classes only. A Java `enum` constant is a singleton by construction — there
/// is nothing to alias and no identity question to answer — and it lowers to a
/// Rust `enum`, where a shared handle breaks `match`, derived equality and the
/// switch lowering. An `interface` has no values of its own. Including them
/// cost 27 `idiom_gaps` tests, almost all of them enum or equality tests
/// ([0002b](../claude_audits/0002b-shared-representation-design.md)).
/// Every name bound as a type parameter anywhere in the program.
fn program_type_param_names(program: &Program) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for ty in &program.types {
        collect_type_param_names(ty, &mut names);
    }
    names
}

fn collect_type_param_names(ty: &TypeDecl, into: &mut BTreeSet<String>) {
    into.extend(ty.type_params.iter().cloned());
    for member in &ty.members {
        match member {
            Member::Method(method) => into.extend(method.type_params.iter().cloned()),
            Member::Type(nested) => collect_type_param_names(nested, into),
            _ => {}
        }
    }
}

fn collect_declared_types(ty: &TypeDecl, into: &mut BTreeSet<String>) {
    if matches!(ty.kind, TypeKind::Class | TypeKind::Record) {
        into.insert(ty.name.clone());
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_declared_types(nested, into);
        }
    }
}

fn collect_inner_outer_capture_storage_requirements(
    program: &Program,
    facts: &FactSet,
    declared: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for inner in crate::inner_class_capture::constructed_outer_this_inner_classes(program, facts) {
        if let Some(outer_type) = declared_type_name_from_name(inner.outer_type(), declared)
            && let Some(storage) = into.get_mut(&outer_type)
        {
            storage.escape = storage.escape.max(Escape::Stored);
            storage.mutated = true;
        }
    }
}

/// Types a Java `static` field can reach, transitively through fields.
///
/// A `static` in Rust must be `Sync`, and a Java `static` is now a process-wide
/// `Mutex` ([0030](../claude_audits/done/0030-java-statics-are-thread-local.md)),
/// so anything it holds must be `Send + Sync` — and so must anything *that*
/// holds, which is why this is a closure rather than a direct check. Those types
/// get `Arc`; everything else keeps the cheaper `Rc`.
fn static_reachable_type_names(program: &Program, declared: &BTreeSet<String>) -> BTreeSet<String> {
    let mut fields_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut reachable = BTreeSet::new();
    for ty in &program.types {
        collect_static_reachability_edges(ty, declared, &mut fields_of, &mut reachable);
    }
    // Transitive closure: a type held by a reachable type is reachable.
    loop {
        let mut grew = false;
        for (owner, fields) in &fields_of {
            if !reachable.contains(owner) {
                continue;
            }
            for field in fields {
                if reachable.insert(field.clone()) {
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    reachable
}

pub(crate) fn statically_reachable_source_type_names(program: &Program) -> BTreeSet<String> {
    let mut declared = BTreeSet::new();
    for ty in &program.types {
        collect_all_source_type_names(ty, &mut declared);
    }
    static_reachable_type_names(program, &declared)
}

fn collect_all_source_type_names(ty: &TypeDecl, into: &mut BTreeSet<String>) {
    into.insert(ty.name.clone());
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_all_source_type_names(nested, into);
        }
    }
}

fn collect_interface_type_names(ty: &TypeDecl, into: &mut BTreeSet<String>) {
    if ty.kind == TypeKind::Interface {
        into.insert(ty.name.clone());
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_interface_type_names(nested, into);
        }
    }
}

fn source_type_has_interface_field(
    program: &Program,
    name: &str,
    interfaces: &BTreeSet<String>,
) -> bool {
    fn find_type<'a>(types: &'a [TypeDecl], name: &str) -> Option<&'a TypeDecl> {
        for ty in types {
            if ty.name == name {
                return Some(ty);
            }
            for member in &ty.members {
                if let Member::Type(nested) = member
                    && let Some(found) = find_type(std::slice::from_ref(nested), name)
                {
                    return Some(found);
                }
            }
        }
        None
    }
    fn has_interface_field(
        ty: &TypeDecl,
        program: &Program,
        interfaces: &BTreeSet<String>,
        declared: &BTreeSet<String>,
        seen: &mut BTreeSet<String>,
    ) -> bool {
        if !seen.insert(ty.name.clone()) {
            return false;
        }
        if ty.members.iter().any(|member| {
            matches!(member, Member::Field(field)
                if !field.modifiers.contains(&Modifier::Static)
                    && !declared_storage_type_names(&field.ty, &ty.name, interfaces).is_empty())
        }) {
            return true;
        }
        ty.extends.iter().any(|parent| {
            let parent_ty = JavaType::Named {
                name: parent.clone(),
                args: Vec::new(),
            };
            declared_type_name(&parent_ty, &ty.name, declared)
                .and_then(|resolved| find_type(&program.types, &resolved))
                .is_some_and(|parent| {
                    has_interface_field(parent, program, interfaces, declared, seen)
                })
        })
    }
    let mut declared = BTreeSet::new();
    for ty in &program.types {
        collect_all_source_type_names(ty, &mut declared);
    }
    find_type(&program.types, name).is_some_and(|ty| {
        has_interface_field(ty, program, interfaces, &declared, &mut BTreeSet::new())
    })
}

fn collect_static_reachability_edges(
    ty: &TypeDecl,
    declared: &BTreeSet<String>,
    fields_of: &mut BTreeMap<String, Vec<String>>,
    reachable: &mut BTreeSet<String>,
) {
    if let Some(parent) = &ty.extends {
        let parent_ty = JavaType::Named {
            name: parent.clone(),
            args: Vec::new(),
        };
        fields_of
            .entry(ty.name.clone())
            .or_default()
            .extend(declared_storage_type_names(&parent_ty, &ty.name, declared));
    }
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                let field_types = declared_storage_type_names(&field.ty, &ty.name, declared);
                if field_types.is_empty() {
                    continue;
                }
                if field.modifiers.contains(&Modifier::Static) {
                    reachable.extend(field_types);
                } else {
                    fields_of
                        .entry(ty.name.clone())
                        .or_default()
                        .extend(field_types);
                }
            }
            Member::Type(nested) => {
                collect_static_reachability_edges(nested, declared, fields_of, reachable)
            }
            _ => {}
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
                // The same is true for values physically stored inside an
                // array or generic field: `Cell[]` and `List<Cell>` both retain
                // `Cell` values even though the field's top-level type is not a
                // declared source class.
                for name in declared_storage_type_names(&field.ty, &ty.name, declared) {
                    if let Some(facts) = into.get_mut(&name) {
                        facts.escape = facts.escape.max(Escape::Stored);
                    }
                }
            }
            Member::Method(method) => {
                if let Some(name) = declared_type_name(&method.return_type, &ty.name, declared)
                    && let Some(facts) = into.get_mut(&name)
                {
                    facts.escape = facts.escape.max(Escape::Returned);
                }
                // A method that writes through its receiver makes its *owner*
                // mutated. This is `mutates_self` from `usage`, already a
                // fixpoint over self-calls, so a method that mutates only by
                // delegating counts.
                if let Some(method_usage) = usage.source_method_usage(ty, method) {
                    if method_usage.mutates_self()
                        && let Some(facts) = into.get_mut(&ty.name)
                    {
                        facts.mutated = true;
                    }
                    collect_unknown_boundary_escapes(method_usage, ty, declared, into);
                    collect_erased_cast_identity_aliases(method_usage, ty, program, declared, into);
                }
                if let Some(body) = &method.body {
                    let mut locals = local_types_from_params(&method.params);
                    collect_block_facts(body, ty, program, declared, false, &mut locals, into);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let mut locals = local_types_from_params(&constructor.params);
                    collect_block_facts(body, ty, program, declared, true, &mut locals, into);
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
    owner: &TypeDecl,
    declared: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for binding in method_usage.all_binding_facts() {
        let crosses_unknown_boundary = binding.uses().iter().any(|use_site| {
            matches!(
                use_site,
                BindingUse::PassedToCall {
                    boundary: CallArgumentBoundary::Unknown,
                    ..
                }
            )
        });
        if crosses_unknown_boundary {
            for name in declared_storage_type_names(binding.java_ty(), &owner.name, declared) {
                if let Some(facts) = into.get_mut(&name) {
                    facts.escape = facts.escape.max(Escape::UnknownBoundary);
                }
            }
        }
    }
    for field_use in method_usage.field_uses() {
        let crate::usage::FieldUse::PassedToCall {
            field_name,
            boundary: CallArgumentBoundary::Unknown,
            ..
        } = field_use
        else {
            continue;
        };
        for name in field_storage_type_names(owner, field_name, declared) {
            if let Some(facts) = into.get_mut(&name) {
                facts.escape = facts.escape.max(Escape::UnknownBoundary);
            }
        }
    }
}

fn collect_erased_cast_identity_aliases(
    method_usage: &MethodUsageFacts,
    owner: &TypeDecl,
    program: &Program,
    declared: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for binding in method_usage.all_binding_facts() {
        if !java_type_is_erased_object(binding.java_ty()) {
            continue;
        }
        for use_site in binding.uses() {
            let BindingUse::CastTarget { target_ty } = use_site else {
                continue;
            };
            let Some(target_name) = declared_type_name(target_ty, &owner.name, declared) else {
                continue;
            };
            for name in source_hierarchy_family_names(program, &target_name, declared) {
                if let Some(facts) = into.get_mut(&name) {
                    facts.locally_aliased = true;
                }
            }
        }
    }
}

fn java_type_is_erased_object(ty: &JavaType) -> bool {
    match ty {
        JavaType::Unknown(name) => simple_name(name) == "Object",
        JavaType::Named { name, .. } => simple_name(name) == "Object",
        _ => false,
    }
}

fn source_hierarchy_family_names(
    program: &Program,
    target_name: &str,
    declared: &BTreeSet<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    for name in declared {
        if name == target_name || source_subtype_reaches(program, name, target_name) {
            out.push(name.clone());
        }
    }
    out
}

fn source_subtype_reaches(program: &Program, subtype: &str, target: &str) -> bool {
    let mut current_owner = subtype.to_string();
    let mut current = source_type_decl(program, subtype).and_then(|ty| ty.extends.clone());
    for _ in 0..64 {
        let Some(super_name) = current else {
            return false;
        };
        let Some(super_ty) =
            source_type_decl_for_owner_context(program, &current_owner, &super_name)
        else {
            return false;
        };
        if super_ty.name == target {
            return true;
        }
        current_owner = super_ty.name.clone();
        current = super_ty.extends.clone();
    }
    false
}

fn source_type_decl<'a>(program: &'a Program, name: &str) -> Option<&'a TypeDecl> {
    let types = program
        .types
        .iter()
        .flat_map(source_type_decl_and_nested)
        .collect::<Vec<_>>();
    unique_type_decl_match(
        &types
            .iter()
            .copied()
            .filter(|ty| ty.name == name)
            .collect::<Vec<_>>(),
    )
    .or_else(|| {
        unique_type_decl_match(
            &types
                .iter()
                .copied()
                .filter(|ty| simple_name(&ty.name) == simple_name(name))
                .collect::<Vec<_>>(),
        )
    })
}

fn source_type_decl_for_owner_context<'a>(
    program: &'a Program,
    owner_name: &str,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    let types = program
        .types
        .iter()
        .flat_map(source_type_decl_and_nested)
        .collect::<Vec<_>>();
    unique_type_decl_match(
        &types
            .iter()
            .copied()
            .filter(|ty| ty.name == type_name)
            .collect::<Vec<_>>(),
    )
    .or_else(|| {
        unique_type_decl_match(
            &types
                .iter()
                .copied()
                .filter(|ty| {
                    simple_name(&ty.name) == simple_name(type_name)
                        && package_name(&ty.name) == package_name(owner_name)
                })
                .collect::<Vec<_>>(),
        )
    })
    .or_else(|| {
        unique_type_decl_match(
            &types
                .iter()
                .copied()
                .filter(|ty| simple_name(&ty.name) == simple_name(type_name))
                .collect::<Vec<_>>(),
        )
    })
}

fn unique_type_decl_match<'a>(matches: &[&'a TypeDecl]) -> Option<&'a TypeDecl> {
    match matches {
        [ty] => Some(*ty),
        _ => None,
    }
}

fn source_type_decl_and_nested(ty: &TypeDecl) -> Vec<&TypeDecl> {
    let mut out = vec![ty];
    for member in &ty.members {
        if let Member::Type(nested) = member {
            out.extend(source_type_decl_and_nested(nested));
        }
    }
    out
}

fn collect_block_facts(
    block: &Block,
    owner: &TypeDecl,
    program: &Program,
    declared: &BTreeSet<String>,
    in_constructor: bool,
    locals: &mut BTreeMap<String, JavaType>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for stmt in &block.statements {
        collect_stmt_facts(stmt, owner, program, declared, in_constructor, locals, into);
    }
}

fn collect_stmt_facts(
    stmt: &Stmt,
    owner: &TypeDecl,
    program: &Program,
    declared: &BTreeSet<String>,
    in_constructor: bool,
    locals: &mut BTreeMap<String, JavaType>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
            ..
        } => {
            // `T b = a;` where `a` is a bare handle: a second name for one
            // object. This is exactly the shape that makes lowering insert a
            // clone, which is why it is the alias signal rather than a general
            // "two references exist" query.
            if let Some(initializer) = initializer
                && expr_is_bare_handle(initializer)
                && let Some(name) = declared_type_name(ty, &owner.name, declared)
                && let Some(facts) = into.get_mut(&name)
            {
                facts.locally_aliased = true;
            }
            locals.insert(name.clone(), ty.clone());
            if let Some(initializer) = initializer {
                collect_anonymous_body_facts_from_expr(
                    initializer,
                    owner,
                    program,
                    declared,
                    locals,
                    into,
                );
            }
        }
        Stmt::Assign { target, value } => {
            collect_anonymous_body_facts_from_expr(target, owner, program, declared, locals, into);
            collect_anonymous_body_facts_from_expr(value, owner, program, declared, locals, into);
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
                    // For `other.f = value`, the receiver's local type lets us
                    // charge the field's declared type.  In particular,
                    // `other.intField = value` cannot retain `value`; charging
                    // the owner here turned ordinary copies into shared
                    // storage merely because the source expression was a bare
                    // handle.
                    Expr::FieldAccess(receiver, field_name) => {
                        let receiver_ty = handle_type_name(receiver, owner, declared, locals)
                            .and_then(|name| source_type_decl(program, &name));
                        if let Some(receiver_ty) = receiver_ty {
                            for name in field_storage_type_names(receiver_ty, field_name, declared)
                            {
                                if let Some(facts) = into.get_mut(&name) {
                                    facts.escape = facts.escape.max(Escape::Stored);
                                }
                            }
                        } else if let Some(facts) = into.get_mut(&owner.name) {
                            facts.escape = facts.escape.max(Escape::Stored);
                        }
                    }
                    // An array element's type is not available in this local
                    // environment, so retain the conservative owner charge.
                    Expr::ArrayAccess(..) => {
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
            if let Expr::FieldAccess(receiver, _) = target
                && !(in_constructor && matches!(receiver.as_ref(), Expr::This))
            {
                match handle_type_name(receiver, owner, declared, locals) {
                    Some(name) => {
                        if let Some(facts) = into.get_mut(&name) {
                            facts.mutated = true;
                        }
                    }
                    // A field write through a receiver this walk cannot type —
                    // `arr[i].f = v`, `other.f = v` — is still a *mutation of
                    // something*. `mutated` has to over-approximate: it is what
                    // says a type may be held as `Rc<T>` with no `RefCell`, and
                    // being wrong that way does not cost a pointer indirection,
                    // it emits `cannot assign to data in an Rc` and does not
                    // compile. Under-approximating is the unsafe direction, and
                    // the design's "over-sharing is cheap" argument does not
                    // cover it
                    // ([0002b](../claude_audits/0002b-shared-representation-design.md)).
                    None => {
                        for facts in into.values_mut() {
                            facts.mutation_provably_absent = false;
                        }
                    }
                }
            }
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            let mut then_locals = locals.clone();
            collect_block_facts(
                then_block,
                owner,
                program,
                declared,
                in_constructor,
                &mut then_locals,
                into,
            );
            if let Some(else_block) = else_block {
                let mut else_locals = locals.clone();
                collect_block_facts(
                    else_block,
                    owner,
                    program,
                    declared,
                    in_constructor,
                    &mut else_locals,
                    into,
                );
            }
        }
        Stmt::For {
            init,
            condition: _,
            update: _,
            body,
            ..
        } => {
            let mut nested_locals = locals.clone();
            for init_stmt in init {
                collect_stmt_facts(
                    init_stmt,
                    owner,
                    program,
                    declared,
                    in_constructor,
                    &mut nested_locals,
                    into,
                );
            }
            collect_block_facts(
                body,
                owner,
                program,
                declared,
                in_constructor,
                &mut nested_locals,
                into,
            )
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            body,
            ..
        } => {
            let mut nested_locals = locals.clone();
            nested_locals.insert(var_name.clone(), var_type.clone());
            collect_block_facts(
                body,
                owner,
                program,
                declared,
                in_constructor,
                &mut nested_locals,
                into,
            )
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            let mut nested_locals = locals.clone();
            collect_block_facts(
                body,
                owner,
                program,
                declared,
                in_constructor,
                &mut nested_locals,
                into,
            )
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                collect_stmt_facts(
                    resource,
                    owner,
                    program,
                    declared,
                    in_constructor,
                    locals,
                    into,
                );
            }
            let mut body_locals = locals.clone();
            collect_block_facts(
                body,
                owner,
                program,
                declared,
                in_constructor,
                &mut body_locals,
                into,
            );
            for catch in catches {
                let mut catch_locals = locals.clone();
                collect_block_facts(
                    &catch.body,
                    owner,
                    program,
                    declared,
                    in_constructor,
                    &mut catch_locals,
                    into,
                );
            }
            if let Some(finally_block) = finally_block {
                let mut finally_locals = locals.clone();
                collect_block_facts(
                    finally_block,
                    owner,
                    program,
                    declared,
                    in_constructor,
                    &mut finally_locals,
                    into,
                );
            }
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                let mut case_locals = locals.clone();
                for stmt in &case.statements {
                    collect_stmt_facts(
                        stmt,
                        owner,
                        program,
                        declared,
                        in_constructor,
                        &mut case_locals,
                        into,
                    );
                }
            }
        }
        Stmt::Return(Some(expr)) | Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_anonymous_body_facts_from_expr(expr, owner, program, declared, locals, into);
        }
        _ => {}
    }
}

/// Anonymous class bodies execute after their enclosing method has constructed
/// aliases. Their field writes must therefore contribute mutation facts for
/// captured handles; otherwise the outer type is incorrectly planned as
/// `Rc<T>` instead of `Rc<RefCell<T>>`.
fn collect_anonymous_body_facts_from_expr(
    expr: &Expr,
    owner: &TypeDecl,
    program: &Program,
    declared: &BTreeSet<String>,
    locals: &BTreeMap<String, JavaType>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    match expr {
        Expr::FieldAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            collect_anonymous_body_facts_from_expr(receiver, owner, program, declared, locals, into)
        }
        Expr::ArrayAccess(array, index) => {
            collect_anonymous_body_facts_from_expr(array, owner, program, declared, locals, into);
            collect_anonymous_body_facts_from_expr(index, owner, program, declared, locals, into);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_anonymous_body_facts_from_expr(
                    dimension, owner, program, declared, locals, into,
                );
            }
            if let Some(initializer) = initializer {
                for item in initializer {
                    collect_anonymous_body_facts_from_expr(
                        item, owner, program, declared, locals, into,
                    );
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                collect_anonymous_body_facts_from_expr(
                    item, owner, program, declared, locals, into,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_anonymous_body_facts_from_expr(left, owner, program, declared, locals, into);
            collect_anonymous_body_facts_from_expr(right, owner, program, declared, locals, into);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            for child in [condition, then_expr, else_expr] {
                collect_anonymous_body_facts_from_expr(
                    child, owner, program, declared, locals, into,
                );
            }
        }
        Expr::Switch { selector, cases } => {
            collect_anonymous_body_facts_from_expr(
                selector, owner, program, declared, locals, into,
            );
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(label) = label {
                        collect_anonymous_body_facts_from_expr(
                            label, owner, program, declared, locals, into,
                        );
                    }
                }
                collect_anonymous_body_facts_from_expr(
                    &case.value,
                    owner,
                    program,
                    declared,
                    locals,
                    into,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_anonymous_body_facts_from_expr(
                    receiver, owner, program, declared, locals, into,
                );
            }
            for arg in args {
                collect_anonymous_body_facts_from_expr(arg, owner, program, declared, locals, into);
            }
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            if let Some(qualifier) = qualifier {
                collect_anonymous_body_facts_from_expr(
                    qualifier, owner, program, declared, locals, into,
                );
            }
            for arg in args {
                collect_anonymous_body_facts_from_expr(arg, owner, program, declared, locals, into);
            }
            if let Some(body) = body {
                // Anonymous instance fields outlive construction just as
                // captured locals do.  In particular, `new I() { T f = x;
                // ... }` retains the *same* `T` object as `x`, rather than a
                // value copy.  The field initializer is not part of a method
                // body, so the capture walk below cannot see it.  Charge a
                // bare-handle initializer to the field's declared type here
                // before signature and slot planning choose whether `x` is a
                // shared handle.
                for member in body {
                    if let Member::Field(field) = member {
                        if let Some(initializer) = &field.initializer {
                            collect_anonymous_body_facts_from_expr(
                                initializer,
                                owner,
                                program,
                                declared,
                                locals,
                                into,
                            );
                            if expr_is_bare_handle(initializer)
                                && let Some(name) =
                                    declared_type_name(&field.ty, &owner.name, declared)
                                && let Some(facts) = into.get_mut(&name)
                            {
                                facts.escape = facts.escape.max(Escape::Stored);
                            }
                        }
                    }
                }
                mark_anonymous_field_member_mutations(body, owner, declared, into);
                // An anonymous object retains every enclosing local its
                // methods read.  This is an alias even when the object has no
                // Java field declaration for it: lowering materializes the
                // capture as a Rust adapter field.  Mark it before lowering
                // chooses that field's shape, otherwise `capture.clone()` is
                // a clone of the source struct rather than a second Java
                // handle (0002/0002b).
                mark_anonymous_capture_aliases(body, owner, declared, locals, into);
                for member in body {
                    if let Member::Method(method) = member
                        && let Some(body) = &method.body
                    {
                        let mut closure_locals = locals.clone();
                        closure_locals.extend(local_types_from_params(&method.params));
                        collect_block_facts(
                            body,
                            owner,
                            program,
                            declared,
                            false,
                            &mut closure_locals,
                            into,
                        );
                    }
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_anonymous_body_facts_from_expr(expr, owner, program, declared, locals, into)
            }
            LambdaBody::Block(block) => {
                let mut closure_locals = locals.clone();
                collect_block_facts(
                    block,
                    owner,
                    program,
                    declared,
                    false,
                    &mut closure_locals,
                    into,
                );
            }
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

/// Anonymous instance fields do not belong to the enclosing source class, so
/// the ordinary assignment walk cannot recover the type of
/// `this.retained.member`.  A direct write through one of those fields still
/// mutates the retained declared-class object after construction.
fn mark_anonymous_field_member_mutations(
    members: &[Member],
    owner: &TypeDecl,
    declared: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    let fields = members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => {
                declared_type_name(&field.ty, &owner.name, declared)
                    .map(|type_name| (crate::parser::to_snake_case(&field.name), type_name))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    fn visit_block(
        block: &Block,
        fields: &BTreeMap<String, String>,
        into: &mut BTreeMap<String, TypeStorageFacts>,
    ) {
        for statement in &block.statements {
            match statement {
                Stmt::If {
                    then_block,
                    else_block,
                    ..
                } => {
                    visit_block(then_block, fields, into);
                    if let Some(else_block) = else_block {
                        visit_block(else_block, fields, into);
                    }
                }
                Stmt::While { body, .. }
                | Stmt::DoWhile { body, .. }
                | Stmt::Block(body)
                | Stmt::LabeledBlock { body, .. } => visit_block(body, fields, into),
                Stmt::For { init, body, .. } => {
                    visit_block(
                        &Block {
                            statements: init.clone(),
                        },
                        fields,
                        into,
                    );
                    visit_block(body, fields, into);
                }
                Stmt::EnhancedFor { body, .. } => visit_block(body, fields, into),
                _ => {}
            }
            let Stmt::Assign {
                target: Expr::FieldAccess(receiver, _),
                ..
            } = statement
            else {
                continue;
            };
            let Expr::FieldAccess(this, field) = receiver.as_ref() else {
                continue;
            };
            if matches!(this.as_ref(), Expr::This)
                && let Some(type_name) = fields.get(&crate::parser::to_snake_case(field))
                && let Some(facts) = into.get_mut(type_name)
            {
                facts.mutated = true;
            }
        }
    }
    for method in members.iter().filter_map(|member| match member {
        Member::Method(method) => method.body.as_ref(),
        _ => None,
    }) {
        visit_block(method, &fields, into);
    }
}

/// Mark declared-class locals retained by an anonymous object's methods as
/// aliases.  The separate lexical scope is important: a method parameter or a
/// local declared in the anonymous method must not be mistaken for an outer
/// capture with the same source name.
fn mark_anonymous_capture_aliases(
    members: &[Member],
    owner: &TypeDecl,
    declared: &BTreeSet<String>,
    outer_locals: &BTreeMap<String, JavaType>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for member in members {
        if let Member::Method(method) = member
            && let Some(body) = &method.body
        {
            let mut bound = method
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect::<BTreeSet<_>>();
            mark_captures_in_block(body, owner, declared, outer_locals, &mut bound, into);
        }
    }
}

fn mark_captured_local(
    name: &str,
    owner: &TypeDecl,
    declared: &BTreeSet<String>,
    outer_locals: &BTreeMap<String, JavaType>,
    bound: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    if !bound.contains(name)
        && let Some(ty) = outer_locals.get(name)
        && let Some(type_name) = declared_type_name(ty, &owner.name, declared)
        && let Some(facts) = into.get_mut(&type_name)
    {
        facts.locally_aliased = true;
    }
}

fn mark_captures_in_block(
    block: &Block,
    owner: &TypeDecl,
    declared: &BTreeSet<String>,
    outer_locals: &BTreeMap<String, JavaType>,
    bound: &mut BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    for stmt in &block.statements {
        mark_captures_in_stmt(stmt, owner, declared, outer_locals, bound, into);
    }
}

fn mark_captures_in_stmt(
    stmt: &Stmt,
    owner: &TypeDecl,
    declared: &BTreeSet<String>,
    outer_locals: &BTreeMap<String, JavaType>,
    bound: &mut BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    match stmt {
        Stmt::VarDecl {
            name, initializer, ..
        } => {
            if let Some(initializer) = initializer {
                mark_captures_in_expr(initializer, owner, declared, outer_locals, bound, into);
            }
            bound.insert(name.clone());
        }
        Stmt::Assign { target, value } => {
            mark_captures_in_expr(target, owner, declared, outer_locals, bound, into);
            mark_captures_in_expr(value, owner, declared, outer_locals, bound, into);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            mark_captures_in_expr(expr, owner, declared, outer_locals, bound, into);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            mark_captures_in_expr(condition, owner, declared, outer_locals, bound, into);
            mark_captures_in_block(
                then_block,
                owner,
                declared,
                outer_locals,
                &mut bound.clone(),
                into,
            );
            if let Some(else_block) = else_block {
                mark_captures_in_block(
                    else_block,
                    owner,
                    declared,
                    outer_locals,
                    &mut bound.clone(),
                    into,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            mark_captures_in_expr(condition, owner, declared, outer_locals, bound, into);
            mark_captures_in_block(
                body,
                owner,
                declared,
                outer_locals,
                &mut bound.clone(),
                into,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut loop_bound = bound.clone();
            for init in init {
                mark_captures_in_stmt(init, owner, declared, outer_locals, &mut loop_bound, into);
            }
            if let Some(condition) = condition {
                mark_captures_in_expr(condition, owner, declared, outer_locals, &loop_bound, into);
            }
            for update in update {
                mark_captures_in_expr(update, owner, declared, outer_locals, &loop_bound, into);
            }
            mark_captures_in_block(body, owner, declared, outer_locals, &mut loop_bound, into);
        }
        Stmt::EnhancedFor {
            var_name,
            iterable,
            body,
            ..
        } => {
            mark_captures_in_expr(iterable, owner, declared, outer_locals, bound, into);
            let mut loop_bound = bound.clone();
            loop_bound.insert(var_name.clone());
            mark_captures_in_block(body, owner, declared, outer_locals, &mut loop_bound, into);
        }
        Stmt::Switch { selector, cases } => {
            mark_captures_in_expr(selector, owner, declared, outer_locals, bound, into);
            for case in cases {
                let mut case_bound = bound.clone();
                for label in &case.labels {
                    if let SwitchLabel::Expr(label) = label {
                        mark_captures_in_expr(
                            label,
                            owner,
                            declared,
                            outer_locals,
                            &case_bound,
                            into,
                        );
                    }
                }
                for stmt in &case.statements {
                    mark_captures_in_stmt(
                        stmt,
                        owner,
                        declared,
                        outer_locals,
                        &mut case_bound,
                        into,
                    );
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut try_bound = bound.clone();
            for resource in resources {
                mark_captures_in_stmt(
                    resource,
                    owner,
                    declared,
                    outer_locals,
                    &mut try_bound,
                    into,
                );
            }
            mark_captures_in_block(body, owner, declared, outer_locals, &mut try_bound, into);
            for catch in catches {
                let mut catch_bound = bound.clone();
                catch_bound.insert(catch.name.clone());
                mark_captures_in_block(
                    &catch.body,
                    owner,
                    declared,
                    outer_locals,
                    &mut catch_bound,
                    into,
                );
            }
            if let Some(finally_block) = finally_block {
                mark_captures_in_block(
                    finally_block,
                    owner,
                    declared,
                    outer_locals,
                    &mut bound.clone(),
                    into,
                );
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                mark_captures_in_expr(arg, owner, declared, outer_locals, bound, into);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            mark_captures_in_block(
                block,
                owner,
                declared,
                outer_locals,
                &mut bound.clone(),
                into,
            );
        }
        Stmt::Return(None)
        | Stmt::LocalType(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn mark_captures_in_expr(
    expr: &Expr,
    owner: &TypeDecl,
    declared: &BTreeSet<String>,
    outer_locals: &BTreeMap<String, JavaType>,
    bound: &BTreeSet<String>,
    into: &mut BTreeMap<String, TypeStorageFacts>,
) {
    match expr {
        Expr::Var(name) => mark_captured_local(name, owner, declared, outer_locals, bound, into),
        Expr::FieldAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            mark_captures_in_expr(receiver, owner, declared, outer_locals, bound, into);
        }
        Expr::ArrayAccess(array, index) => {
            mark_captures_in_expr(array, owner, declared, outer_locals, bound, into);
            mark_captures_in_expr(index, owner, declared, outer_locals, bound, into);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                mark_captures_in_expr(dimension, owner, declared, outer_locals, bound, into);
            }
            if let Some(initializer) = initializer {
                for item in initializer {
                    mark_captures_in_expr(item, owner, declared, outer_locals, bound, into);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                mark_captures_in_expr(item, owner, declared, outer_locals, bound, into);
            }
        }
        Expr::Binary { left, right, .. } => {
            mark_captures_in_expr(left, owner, declared, outer_locals, bound, into);
            mark_captures_in_expr(right, owner, declared, outer_locals, bound, into);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            for child in [condition, then_expr, else_expr] {
                mark_captures_in_expr(child, owner, declared, outer_locals, bound, into);
            }
        }
        Expr::Switch { selector, cases } => {
            mark_captures_in_expr(selector, owner, declared, outer_locals, bound, into);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(label) = label {
                        mark_captures_in_expr(label, owner, declared, outer_locals, bound, into);
                    }
                }
                mark_captures_in_expr(&case.value, owner, declared, outer_locals, bound, into);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                mark_captures_in_expr(receiver, owner, declared, outer_locals, bound, into);
            }
            for arg in args {
                mark_captures_in_expr(arg, owner, declared, outer_locals, bound, into);
            }
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            if let Some(qualifier) = qualifier {
                mark_captures_in_expr(qualifier, owner, declared, outer_locals, bound, into);
            }
            for arg in args {
                mark_captures_in_expr(arg, owner, declared, outer_locals, bound, into);
            }
            if let Some(body) = body {
                mark_anonymous_capture_aliases(body, owner, declared, outer_locals, into);
            }
        }
        Expr::Lambda { params, body } => {
            let mut lambda_bound = bound.clone();
            lambda_bound.extend(params.iter().map(|param| param.name.clone()));
            match body {
                LambdaBody::Expr(expr) => {
                    mark_captures_in_expr(expr, owner, declared, outer_locals, &lambda_bound, into)
                }
                LambdaBody::Block(block) => mark_captures_in_block(
                    block,
                    owner,
                    declared,
                    outer_locals,
                    &mut lambda_bound,
                    into,
                ),
            }
        }
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

/// Whether an expression is a bare reference to an existing object, as opposed
/// to something that produces a fresh one.
fn expr_is_bare_handle(expr: &Expr) -> bool {
    matches!(expr, Expr::Var(_) | Expr::FieldAccess(..) | Expr::This)
}

/// The declared type a handle refers to, when that is cheap to see.
fn handle_type_name(
    expr: &Expr,
    owner: &TypeDecl,
    declared: &BTreeSet<String>,
    locals: &BTreeMap<String, JavaType>,
) -> Option<String> {
    match expr {
        Expr::This => declared.contains(&owner.name).then(|| owner.name.clone()),
        Expr::Var(name) => locals
            .get(name)
            .and_then(|ty| declared_type_name(ty, &owner.name, declared)),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            field_type_name(owner, field, declared)
        }
        _ => None,
    }
}

fn local_types_from_params(params: &[crate::java_ir::Param]) -> BTreeMap<String, JavaType> {
    params
        .iter()
        .map(|param| (param.name.clone(), param.ty.clone()))
        .collect()
}

fn field_type_name(owner: &TypeDecl, field: &str, declared: &BTreeSet<String>) -> Option<String> {
    owner.members.iter().find_map(|member| match member {
        Member::Field(decl) if decl.name == field => {
            declared_type_name(&decl.ty, &owner.name, declared)
        }
        _ => None,
    })
}

fn field_storage_type_names(
    owner: &TypeDecl,
    field: &str,
    declared: &BTreeSet<String>,
) -> Vec<String> {
    owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(decl) if decl.name == field => {
                Some(declared_storage_type_names(&decl.ty, &owner.name, declared))
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// Declared source types that are physically reachable through a stored value of
/// `ty`.
///
/// Static reachability is a `Send + Sync` question for the whole Rust static's
/// object graph, so `Cell[]` and `List<Cell>` both carry `Cell` even though most
/// other storage facts deliberately ask for the single nominal type a slot
/// names.
fn declared_storage_type_names(
    ty: &JavaType,
    owner_name: &str,
    declared: &BTreeSet<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    collect_declared_storage_type_names(ty, owner_name, declared, &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect_declared_storage_type_names(
    ty: &JavaType,
    owner_name: &str,
    declared: &BTreeSet<String>,
    into: &mut Vec<String>,
) {
    match ty {
        JavaType::Array(inner) => {
            collect_declared_storage_type_names(inner, owner_name, declared, into)
        }
        JavaType::Named { args, .. } => {
            if let Some(declared_name) = declared_type_name(ty, owner_name, declared) {
                into.push(declared_name);
            }
            for arg in args {
                collect_declared_storage_type_names(arg, owner_name, declared, into);
            }
        }
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            collect_declared_storage_type_names(bound, owner_name, declared, into);
        }
        _ => {}
    }
}

/// The declared source type a Java type names, ignoring anything external.
fn declared_type_name(
    ty: &JavaType,
    owner_name: &str,
    declared: &BTreeSet<String>,
) -> Option<String> {
    let JavaType::Named { name, .. } = ty else {
        return None;
    };
    if declared.contains(name) {
        return Some(name.clone());
    }
    let simple = simple_name(name);
    let owner_package = package_name(owner_name);
    let same_package_matches = declared
        .iter()
        .filter(|declared_name| {
            simple_name(declared_name) == simple && package_name(declared_name) == owner_package
        })
        .collect::<Vec<_>>();
    if same_package_matches.len() == 1 {
        return Some((*same_package_matches[0]).clone());
    }
    let simple_matches = declared
        .iter()
        .filter(|declared_name| simple_name(declared_name) == simple)
        .collect::<Vec<_>>();
    (simple_matches.len() == 1).then(|| (*simple_matches[0]).clone())
}

fn declared_type_name_from_name(name: &str, declared: &BTreeSet<String>) -> Option<String> {
    if declared.contains(name) {
        return Some(name.to_string());
    }
    let simple = simple_name(name);
    let simple_matches = declared
        .iter()
        .filter(|declared_name| simple_name(declared_name) == simple)
        .collect::<Vec<_>>();
    (simple_matches.len() == 1).then(|| (*simple_matches[0]).clone())
}

fn simple_name(name: &str) -> &str {
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

fn package_name(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(package, _)| package)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Span;
    use crate::java_ir::FieldDecl;

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

    #[test]
    fn non_forwardable_shared_surface_is_not_shared_immutable() {
        let facts = TypeStorageFacts {
            escape: Escape::Stored,
            surface_forwardable: false,
            ..Default::default()
        };

        assert_eq!(facts.strategy(), StorageStrategy::SharedLocal);
        assert_eq!(facts.placement(), StoragePlacement::new(true, true, false));
        assert!(facts.placement().is_implemented());
    }

    #[test]
    fn mutated_static_reachable_shared_type_reports_sync_strategy() {
        let facts = TypeStorageFacts {
            escape: Escape::Stored,
            mutated: true,
            static_reachable: true,
            ..Default::default()
        };

        assert_eq!(facts.strategy(), StorageStrategy::SharedSync);
        assert_eq!(facts.handle(), SharedHandle::Sync);
        assert_eq!(facts.placement(), StoragePlacement::new(true, true, true));
    }

    #[test]
    fn storage_plan_merge_preserves_conservative_evidence() {
        let mut base = StoragePlan::default();
        base.insert(
            "Node".to_string(),
            TypeStorageFacts {
                escape: Escape::Stored,
                locally_aliased: true,
                ..Default::default()
            },
        );
        let mut extra = StoragePlan::default();
        extra.insert(
            "Node".to_string(),
            TypeStorageFacts {
                mutated: true,
                mutation_provably_absent: false,
                static_reachable: true,
                ..Default::default()
            },
        );
        extra.insert(
            "Leaf".to_string(),
            TypeStorageFacts {
                escape: Escape::UnknownBoundary,
                ..Default::default()
            },
        );

        base.extend_from(extra);

        let node = base.facts("Node").expect("merged Node facts");
        assert_eq!(node.escape, Escape::Stored);
        assert!(node.locally_aliased);
        assert!(node.mutated);
        assert!(!node.mutation_provably_absent);
        assert!(node.static_reachable);
        assert_eq!(node.strategy(), StorageStrategy::SharedSync);
        assert_eq!(node.handle(), SharedHandle::Sync);
        assert_eq!(base.strategy("Leaf"), StorageStrategy::SharedImmutable);
    }

    #[test]
    fn storage_strategy_simple_owner_uses_same_package_before_first_simple_match() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                empty_class("a.Item"),
                empty_class("b.Item"),
                TypeDecl {
                    kind: TypeKind::Class,
                    name: "b.Holder".to_string(),
                    type_params: Vec::new(),
                    type_param_bounds: Vec::new(),
                    modifiers: Vec::new(),
                    extends: None,
                    implements: Vec::new(),
                    permits: Vec::new(),
                    members: vec![Member::Field(FieldDecl {
                        name: "held".to_string(),
                        ty: JavaType::Named {
                            name: "Item".to_string(),
                            args: Vec::new(),
                        },
                        modifiers: vec![Modifier::Static],
                        initializer: None,
                        span: Span::default(),
                    })],
                    static_init_blocks: Vec::new(),
                    span: Span::default(),
                },
            ],
        };

        let plan = plan_storage_strategies(&program, &UsageFacts::default());

        assert!(
            !plan
                .facts("a.Item")
                .expect("a.Item storage facts")
                .static_reachable
        );
        assert!(
            plan.facts("b.Item")
                .expect("b.Item storage facts")
                .static_reachable
        );
    }

    #[test]
    fn source_subtype_reachability_prefers_same_package_super_edge() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                empty_class("a.Base"),
                empty_class("b.Base"),
                TypeDecl {
                    kind: TypeKind::Class,
                    name: "b.Child".to_string(),
                    type_params: Vec::new(),
                    type_param_bounds: Vec::new(),
                    modifiers: Vec::new(),
                    extends: Some("Base".to_string()),
                    implements: Vec::new(),
                    permits: Vec::new(),
                    members: Vec::new(),
                    static_init_blocks: Vec::new(),
                    span: Span::default(),
                },
            ],
        };

        assert!(source_subtype_reaches(&program, "b.Child", "b.Base"));
        assert!(
            !source_subtype_reaches(&program, "b.Child", "a.Base"),
            "bare super edge must not reach the first same-simple package peer"
        );
    }

    fn empty_class(name: &str) -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Class,
            name: name.to_string(),
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

    fn facts_of(source: &str, type_name: &str) -> TypeStorageFacts {
        let parsed = parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = crate::usage::analyze_usage(&resolved.program);
        plan_storage_strategies(&resolved.program, &usage)
            .facts(type_name)
            .unwrap_or_else(|| panic!("missing storage facts for {type_name}"))
            .clone()
    }

    #[test]
    fn source_null_assertion_helper_does_not_make_argument_escape() {
        let facts = facts_of(
            r#"
class NullCheckHelper {
    static Object checkNotNull(Object value) {
        if (value == null) throw new RuntimeException();
        return value;
    }
}
class CopyTarget {
    int count;
    static void copy(CopyTarget source, CopyTarget dest) {
        NullCheckHelper.checkNotNull(source);
        NullCheckHelper.checkNotNull(dest);
        dest.count = source.count;
    }
}
"#,
            "CopyTarget",
        );
        assert_eq!(facts.escape(), Escape::LocalOnly, "{facts:?}");
        assert!(!facts.locally_aliased(), "{facts:?}");
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

    #[test]
    fn constructor_write_through_parameter_still_makes_that_type_mutable() {
        let strategy = strategy_of(
            r#"
public class Outer {
    static final class Cell {
        int v;
    }
    static final class MutatesOther {
        MutatesOther(Cell cell) { cell.v = 2; }
    }
    static int run(Cell a) {
        Cell b = a;
        new MutatesOther(a);
        return b.v;
    }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedLocal);
    }

    #[test]
    fn records_participate_in_the_storage_lattice() {
        let strategy = strategy_of(
            r#"
record Cell(int v) {}

class Outer {
    static int run(Cell a) {
        Cell b = a;
        return b.v();
    }
}
"#,
            "Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedImmutable);
    }

    #[test]
    fn erased_object_cast_to_source_supertype_shares_recovered_hierarchy_family() {
        let source = r#"
class Use {
    static boolean run(Object value) {
        A first = (A) value;
        A second = (A) value;
        return first == second;
    }
}
class A {
    int v;
}
class B extends A {
}
"#;
        assert_eq!(strategy_of(source, "A"), StorageStrategy::SharedImmutable);
        assert_eq!(strategy_of(source, "B"), StorageStrategy::SharedImmutable);
    }

    #[test]
    fn for_initializer_locals_feed_alias_and_mutation_facts() {
        let strategy = strategy_of(
            r#"
public class Outer {
    static final class Cell {
        int v;
    }
    static int run(Cell a) {
        for (Cell b = a; b.v < 10; ) {
            b.v = 2;
            break;
        }
        return a.v;
    }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedLocal);
    }

    #[test]
    fn static_reachability_propagates_through_instance_fields() {
        let strategy = strategy_of(
            r#"
public class Outer {
    static Holder ROOT;
    static final class Holder {
        Cell cell;
    }
    static final class Cell {
        int v;
        void set(int x) { this.v = x; }
    }
    static int run(Cell a) { Cell b = a; a.set(1); return b.v; }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedSync);
    }

    #[test]
    fn statically_reachable_interface_container_needs_sync_cell() {
        let strategy = strategy_of(
            r#"
interface Counter { int read(); }
class Holder {
    Counter counter = new Counter() {
        int value = 1;
        public int read() { return value; }
    };
}
class Root { static Holder holder = new Holder(); }
"#,
            "Holder",
        );
        assert_eq!(strategy, StorageStrategy::SharedSync);
    }

    #[test]
    fn statically_reachable_subclass_inherits_interface_cell_requirement() {
        let strategy = strategy_of(
            r#"
interface Counter { int read(); }
class Base {
    Counter counter = new Counter() {
        int value = 1;
        public int read() { return value; }
    };
}
class Child extends Base {}
class Root { static Child holder = new Child(); }
"#,
            "Child",
        );
        assert_eq!(strategy, StorageStrategy::SharedSync);
    }

    #[test]
    fn static_reachability_sees_array_element_types() {
        let strategy = strategy_of(
            r#"
public class Outer {
    static Cell[] CELLS;
    static final class Cell {
        int v;
        void set(int x) { this.v = x; }
    }
    static int run(Cell a) { Cell b = a; a.set(1); return b.v; }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedSync);
    }

    #[test]
    fn static_reachability_propagates_through_array_fields() {
        let strategy = strategy_of(
            r#"
public class Outer {
    static Holder ROOT;
    static final class Holder {
        Cell[] cells;
    }
    static final class Cell {
        int v;
        void set(int x) { this.v = x; }
    }
    static int run(Cell a) { Cell b = a; a.set(1); return b.v; }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedSync);
    }

    #[test]
    fn field_storage_sees_array_element_types() {
        let strategy = strategy_of(
            r#"
public class Outer {
    Cell[] cells;
    static final class Cell {
        int v;
    }
    static int run(Cell a) { return a.v; }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedImmutable);
    }

    #[test]
    fn field_storage_sees_generic_argument_types() {
        let strategy = strategy_of(
            r#"
import java.util.List;

public class Outer {
    List<Cell> cells;
    static final class Cell {
        int v;
    }
    static int run(Cell a) { return a.v; }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedImmutable);
    }

    #[test]
    fn constructed_inner_class_outer_capture_forces_mutable_shared_outer_plan() {
        let parsed = parse_java_source(
            r#"
public class Outer {
    int base = 10;
    class Inner {
        int get() { return base; }
    }
    int run() {
        Inner inner = new Inner();
        base = 20;
        return inner.get();
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = crate::usage::analyze_usage(&resolved.program);

        assert_eq!(
            plan_storage_strategies(&resolved.program, &usage).strategy("Outer"),
            StorageStrategy::Value,
            "usage alone cannot see the outer-this handle required by constructed inner classes"
        );

        let plan = plan_storage_strategies_from_facts(&resolved.program, &usage, &resolved.facts);
        let facts = plan.facts("Outer").expect("Outer storage facts");
        assert_eq!(facts.escape, Escape::Stored);
        assert!(facts.mutated);
        assert_eq!(facts.strategy(), StorageStrategy::SharedLocal);
        assert!(facts.placement().is_implemented());
    }

    #[test]
    fn anonymous_object_capture_marks_mutated_factory_argument_shared_local() {
        let facts = facts_of(
            r#"
interface Reader { int read(); }
class Node { int value; Node(int value) { this.value = value; } }
class CapturedMutableAnonymousReader {
    static Reader make(Node node) {
        return new Reader() { public int read() { return node.value; } };
    }
    static int run() {
        Node node = new Node(3);
        Reader reader = make(node);
        node.value = 8;
        return reader.read();
    }
}
"#,
            "Node",
        );
        assert!(
            facts.locally_aliased(),
            "anonymous capture must retain the factory argument: {facts:?}"
        );
        assert!(
            facts.mutated(),
            "the caller mutates the captured node: {facts:?}"
        );
        assert_eq!(facts.strategy(), StorageStrategy::SharedLocal);
    }

    #[test]
    fn anonymous_object_field_initializer_marks_factory_argument_stored() {
        let facts = facts_of(
            r#"
interface Reader { int read(); }
class Node { int value; Node(int value) { this.value = value; } }
class FieldInitializedAnonymousReader {
    static Reader make(Node node) {
        return new Reader() {
            Node retained = node;
            public int read() { return this.retained.value; }
        };
    }
    static int run() {
        Node node = new Node(3);
        Reader reader = make(node);
        node.value = 8;
        return reader.read();
    }
}
"#,
            "Node",
        );
        assert_eq!(facts.escape(), Escape::Stored, "{facts:?}");
        assert!(facts.mutated(), "{facts:?}");
        assert_eq!(facts.strategy(), StorageStrategy::SharedLocal);
    }

    #[test]
    fn nested_anonymous_field_member_write_marks_retained_type_mutable() {
        let facts = facts_of(
            r#"
interface Reader { int write(boolean enabled, int value); }
class Node { int value; Node(int value) { this.value = value; } }
class NestedAnonymousFieldWrite {
    static Reader make(Node node) {
        return new Reader() {
            Node retained = node;
            public int write(boolean enabled, int value) {
                if (enabled) { this.retained.value = value; }
                return this.retained.value;
            }
        };
    }
}
"#,
            "Node",
        );
        assert_eq!(facts.escape(), Escape::Stored, "{facts:?}");
        assert!(facts.mutated(), "{facts:?}");
        assert_eq!(facts.strategy(), StorageStrategy::SharedLocal);
    }

    #[test]
    fn anonymous_method_parameter_shadowing_is_not_an_outer_capture() {
        let facts = facts_of(
            r#"
interface Reader { int read(Node node); }
class Node { int value; Node(int value) { this.value = value; } }
class ShadowedAnonymousReader {
    static Reader make(Node node) {
        return new Reader() {
            public int read(Node node) { return node.value; }
        };
    }
}
"#,
            "Node",
        );
        assert!(
            !facts.locally_aliased(),
            "the anonymous method parameter shadows the factory parameter: {facts:?}"
        );
        assert_eq!(facts.strategy(), StorageStrategy::Value);
    }

    #[test]
    fn all_storage_placements_are_lowerable() {
        assert!(StoragePlacement::new(false, false, false).is_implemented());
        assert!(StoragePlacement::new(true, false, false).is_implemented());
        assert!(StoragePlacement::new(true, false, true).is_implemented());
        assert!(
            StoragePlacement::new(true, true, false).is_implemented(),
            "SharedLocal Rc<RefCell<T>> lowering is enabled for direct field borrows"
        );
        assert!(
            StoragePlacement::new(true, true, true).is_implemented(),
            "SharedSync Arc<Mutex<T>> lowering uses short-lived lock projections"
        );
    }

    #[test]
    fn field_storage_sees_wildcard_generic_bound_types() {
        let strategy = strategy_of(
            r#"
import java.util.List;

public class Outer {
    List<? extends Cell> cells;
    static final class Cell {
        int v;
    }
    static int run(Cell a) { return a.v; }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedImmutable);
    }

    #[test]
    fn static_reachability_sees_wildcard_generic_bound_types() {
        let strategy = strategy_of(
            r#"
import java.util.List;

public class Outer {
    static List<? super Cell> CELLS;
    static final class Cell {
        int v;
        void set(int x) { this.v = x; }
    }
    static int run(Cell a) { Cell b = a; a.set(1); return b.v; }
}
"#,
            "Outer.Cell",
        );
        assert_eq!(strategy, StorageStrategy::SharedSync);
    }

    #[test]
    fn unknown_boundary_sees_field_argument_type() {
        let facts = facts_of(
            r#"
public class Outer {
    ExternalName cached;
    void send(Helper helper) { helper.consume(cached); }
}

class ExternalName {}
"#,
            "ExternalName",
        );
        assert_eq!(facts.escape, Escape::UnknownBoundary);
    }

    #[test]
    fn unknown_boundary_sees_generic_field_argument_types() {
        let facts = facts_of(
            r#"
import java.util.List;

public class Outer {
    List<Cell> cached;
    void send(Helper helper) { helper.consume(cached); }
}

class Cell {}
"#,
            "Cell",
        );
        assert_eq!(facts.escape, Escape::UnknownBoundary);
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

#[cfg(test)]
mod handle_forwardable_surface_tests {
    use super::*;
    use crate::parser::parse_java_source;

    fn forwardable(source: &str, type_name: &str) -> bool {
        let parsed = parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = crate::usage::analyze_usage(&resolved.program);
        type_interface_surface_is_handle_forwardable(type_name, &resolved.program, &usage)
    }

    /// A `&self`-only interface forwards, so the tier is available.
    #[test]
    fn read_only_interface_surface_forwards() {
        assert!(forwardable(
            r#"
interface Named { String id(); }
class Fixed implements Named {
    private final String n;
    Fixed(String n) { this.n = n; }
    public String id() { return n; }
}
"#,
            "Fixed",
        ));
    }

    /// A mutating sibling makes the trait method `&mut self`, which now forwards
    /// through the cell-backed shared handle rather than blocking sharing.
    #[test]
    fn a_mutating_sibling_implementor_allows_cell_handle_forwarding() {
        let source = r#"
interface Shape { int size(); void grow(); }
class Fixed implements Shape {
    private final int s;
    Fixed(int s) { this.s = s; }
    public int size() { return s; }
    public void grow() { }
}
class Growing implements Shape {
    private int s;
    public int size() { return s; }
    public void grow() { s = s + 1; }
}
"#;
        assert!(forwardable(source, "Fixed"));
        assert!(forwardable(source, "Growing"));
    }

    #[test]
    fn camel_case_mutating_interface_method_forwards() {
        let source = r#"
interface Shape { void resetSize(); }
class Fixed implements Shape {
    private int s;
    public void resetSize() { s = 0; }
}
"#;
        assert!(forwardable(source, "Fixed"));
    }

    /// A type implementing nothing of ours has no surface to forward through.
    #[test]
    fn a_type_with_no_source_interface_forwards() {
        assert!(forwardable(
            r#"
class Plain {
    private int v;
    int get() { return v; }
}
"#,
            "Plain",
        ));
    }
}

#[cfg(test)]
mod placement_law_tests {
    use super::*;

    fn all_placements() -> Vec<StoragePlacement> {
        let mut out = Vec::new();
        for shared in [false, true] {
            for interior_mutable in [false, true] {
                for sync in [false, true] {
                    out.push(StoragePlacement::new(shared, interior_mutable, sync));
                }
            }
        }
        out
    }

    /// `join` is the lattice's least upper bound, so it must be commutative,
    /// associative, idempotent and extensive.
    ///
    /// These four laws are what make the plan's fixpoint well defined: extensive
    /// plus a finite domain is why it terminates, and commutative plus
    /// associative is why the order uses are visited in cannot change the
    /// answer. Asserting them is cheaper and stronger than enumerating the
    /// results — the domain is 8 elements, so the laws are checked exhaustively.
    #[test]
    fn join_obeys_the_lattice_laws() {
        for a in all_placements() {
            assert_eq!(a.join(a), a, "idempotent");
            for b in all_placements() {
                assert_eq!(a.join(b), b.join(a), "commutative");
                // Extensive: joining can only add requirements.
                let j = a.join(b);
                assert!(j.shared() >= a.shared() && j.shared() >= b.shared());
                assert!(j.interior_mutable() >= a.interior_mutable());
                assert!(j.sync() >= a.sync() && j.sync() >= b.sync());
                for c in all_placements() {
                    assert_eq!(a.join(b).join(c), a.join(b.join(c)), "associative");
                }
            }
        }
    }

    /// The wrapping is derived from the axes, so each layer appears exactly when
    /// its axis says it should — checked as a property over the whole domain
    /// rather than as a table of eight expected strings.
    #[test]
    fn wrapping_follows_the_axes() {
        for placement in all_placements() {
            let wrapped = placement.wrap("T".to_string());

            assert_eq!(
                wrapped.contains("Rc<") || wrapped.contains("Arc<"),
                placement.shared(),
                "a handle appears exactly when the value is shared: {placement:?} -> {wrapped}"
            );
            assert_eq!(
                wrapped.contains("RefCell<") || wrapped.contains("Mutex<"),
                placement.shared() && placement.interior_mutable(),
                "a cell appears exactly when a shared value is mutated: {placement:?} -> {wrapped}"
            );
            // One `sync` bit picks both, so they can never disagree.
            if placement.shared() {
                assert_eq!(
                    wrapped.contains("std::sync::Arc<"),
                    placement.sync(),
                    "the atomic handle is used exactly when sync is required"
                );
            }
            if placement.shared() && placement.interior_mutable() {
                assert_eq!(
                    wrapped.contains("std::sync::Mutex<"),
                    placement.sync(),
                    "the lock and the handle agree on sync by construction"
                );
            }
            // The cell nests inside the handle, never the other way round.
            if let (Some(handle), Some(cell)) = (placement.handle_path(), placement.cell_path()) {
                assert!(
                    wrapped.find(handle).unwrap() < wrapped.find(cell).unwrap(),
                    "the handle is outermost: {wrapped}"
                );
            }
        }
    }

    /// The type form and the construction form must nest identically.
    ///
    /// `Rc<RefCell<T>>` has to be built by `Rc::new(RefCell::new(v))`, never the
    /// other way round. Deriving both from the same axes is what makes that
    /// true; asserting it is what keeps it true.
    #[test]
    fn the_type_and_construction_forms_nest_the_same_way() {
        for placement in all_placements() {
            let ty = placement.wrap("T".to_string());
            let value =
                crate::emit::emit_expr(&crate::slot_render::storage_placement_construction_expr(
                    placement,
                    crate::rust_ir::RustExpr::Var("v".to_string()),
                ));
            for path in [placement.handle_path(), placement.cell_path()]
                .into_iter()
                .flatten()
            {
                assert_eq!(
                    ty.contains(&format!("{path}<")),
                    value.contains(&format!("{path}::new(")),
                    "{placement:?}: {path} appears in one form and not the other"
                );
            }
            if let (Some(handle), Some(cell)) = (placement.handle_path(), placement.cell_path()) {
                assert!(
                    value.find(handle).unwrap() < value.find(cell).unwrap(),
                    "the handle is outermost in the construction too: {value}"
                );
            }
            // An unshared value is constructed exactly as written.
            if !placement.shared() {
                assert_eq!(value, "v");
            }
        }
    }

    /// An unshared value is never wrapped, whatever the other axes say.
    #[test]
    fn an_unshared_value_is_held_inline() {
        for placement in all_placements().into_iter().filter(|p| !p.shared()) {
            assert_eq!(placement.wrap("T".to_string()), "T");
            assert_eq!(placement.handle_path(), None);
            assert_eq!(placement.cell_path(), None);
        }
    }
}
