# 0002b — Shared object representation: the design, on paper

Status: **design, not implemented**
Blocks: [0002b](0002-object-aliasing-has-no-representation.md),
[0023](0023-inner-class-construction-always-panics.md),
[0028](0028-reference-equality-lowered-as-structural-equality.md),
and all of [CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md)

This exists because three separate documents all stop at the same sentence —
*"the object model has no shared representation"* — and
[CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md) step 2 says explicitly: **fix the
representation choice on paper before implementing it. Cheap now, expensive
later.** This is that paper.

Nothing here is committed to code. It is written to be argued with.

## What one decision buys

| issue | today | under this design |
|---|---|---|
| 0002 | `Node b = a` copies; writes through `a` are invisible through `b` | `Rc::clone` — one object, two handles |
| 0028 | `a == b` is structural equality, violating the stated project decision | `Rc::ptr_eq` — *exactly* Java's reference identity |
| 0023 | every non-static inner class panics on construction | `__outer` stops being a one-off; it becomes the general case |
| 0006 | clone/borrow recovery is the largest rule cluster in `lower.rs` | for shared types the answer is always `Rc::clone`, and the cluster does not apply |
| concurrency | `synchronized` erased; nothing to lock | `Arc<Mutex<T>>` gives `synchronized (o)` an object to lock |

0028 is worth pausing on: it is not "also fixed", it is fixed *because the
representation is finally the right one*. Reference identity is unimplementable
on values and free on handles. That is the strongest argument that the current
representation is the defect rather than any individual symptom.

## The choice: per **type**, not per allocation site

Per-allocation-site is more precise and is the wrong call here. A translator
must emit one struct definition and one set of method signatures per Java type;
if two allocation sites of `Node` had different Rust types, `Node b = a;`
between them would not typecheck. Precision that cannot be spent is not
precision.

So: one decision per source type, taken as the **join over every use** of that
type. This mirrors how `HierarchyStrategy` (`src/facts.rs:1415`) is already
decided.

## The lattice

```rust
/// How values of a source type are held.
///
/// Ordered by how much sharing must be supported; the plan takes the join over
/// all uses, so the order must be monotone — adding a use can only move a type
/// up.
pub enum StorageStrategy {
    /// No observable aliasing anywhere. Today's behaviour, and now justified
    /// rather than assumed.
    Value,
    /// Aliased, never mutated after sharing. `Rc<T>` / `Arc<T>` — no borrow
    /// flag, no panic risk.
    SharedImmutable,
    /// Aliased and mutated, single-threaded. `Rc<RefCell<T>>`.
    SharedLocal,
    /// Aliased and mutated, reachable across a thread boundary.
    /// `Arc<Mutex<T>>`.
    SharedSync,
}
```

`Value < SharedImmutable < SharedLocal < SharedSync`. Join is max. Monotone, so
the fixpoint terminates.

This is the `Local`/`Shared` policy axis CONCURRENCY-SCOPE asks for, with
`SharedImmutable` split out because it is common (value objects, keys, strings)
and strictly cheaper than `SharedLocal`.

## The inputs, and where each already exists

| input | question | source |
|---|---|---|
| **escapes** | can a value of `T` be reachable from two live places? | `EscapeLatticeValue` in `src/lattice.rs` (`LocalOnly \| Returned \| Stored \| UnknownBoundary`) — defined, never consumed |
| **locally aliased** | `T b = a;` with both live | `src/aliasing.rs`, built for 0002 detection |
| **mutated** | any field write or `mutates_self` method | `MethodUsageFacts.mutates_self` in `src/usage.rs` |
| **threaded** | reachable from `Runnable.run` / `Thread` / `ExecutorService.submit` | `CallGraph` in `src/usage.rs` |

**Every input already exists.** Not one of them needs to be built. That is the
main finding of this design pass, and it is why the work is large but not deep:
it is wiring plus a lowering rewrite, not new analysis.

Decision rule:

```
escapes(T) ∈ {Stored, UnknownBoundary} ∨ locally_aliased(T)
  ? (mutated(T) ? (threaded(T) ? SharedSync : SharedLocal) : SharedImmutable)
  : Value
```

`UnknownBoundary` must land on the shared side: an unmodelled external call
could retain the reference, and getting this wrong is a silent wrong answer,
whereas over-sharing costs a pointer indirection.

## What every use site becomes

This table is the real cost of the change; everything else is bookkeeping.

| Java | `Value` (today) | `SharedLocal` |
|---|---|---|
| `new T(…)` | `T::new(…)` | `Rc::new(RefCell::new(T::new(…)))` |
| `T b = a;` | `a.clone()` ⚠ | `Rc::clone(&a)` |
| `a.f` | `a.f` | `a.borrow().f` |
| `a.f = v` | `a.f = v` | `a.borrow_mut().f = v` |
| `a.m()` read-only | `a.m()` | `a.borrow().m()` |
| `a.m()` mutating | `a.m()` | `a.borrow_mut().m()` |
| `a == b` | structural ⚠ | `Rc::ptr_eq(&a, &b)` |
| `a == null` | `Option::is_none` | `Option::is_none` (unchanged) |
| field `T f;` | `T` / `Option<Box<T>>` | `Option<Rc<RefCell<T>>>` |
| param `T x` | `&T` or `T` | `&Rc<RefCell<T>>` |
| return `T` | `T` | `Rc<RefCell<T>>` |

`SharedSync` is the same with `Arc`/`.lock().unwrap()`. `SharedImmutable` is the
same with `Rc` and no `borrow()`.

Note the recursive-field case gets *simpler*: `Option<Box<Node>>` becomes
`Option<Rc<RefCell<Node>>>`, and the `Box` special-casing that
[0002](0002-object-aliasing-has-no-representation.md)'s derive fix had to teach
`rust_type_supports_struct_derive_clone` about stops mattering, because `Rc<T>`
is `Clone` regardless of `T`.

## The hazard: `RefCell` turns a Java-legal program into a panic

```java
void a() { helper.callBack(this); }   // `this` re-entered through another handle
```

Java allows re-entry through a second handle to the same object. `RefCell` does
not: a `borrow_mut()` live across that call panics with *already borrowed*. This
is the exact analogue of the reentrant-monitor problem in
[CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md) §1, and it is worse, because it
converts a silent wrong answer into a runtime abort.

**The design rule that avoids it: never hold a borrow across a call.** Borrow at
each access and drop the guard at the end of the statement.

```rust
a.borrow_mut().f = v;              // guard dropped here — safe
let g = a.borrow_mut(); g.m();     // guard live across the call — forbidden
```

This must be a property lowering *guarantees*, not a convention. It costs a
re-borrow per access, which is a branch on a flag — cheap, and the price of
matching Java's aliasing.

`Mutex` under `SharedSync` has the same problem and is not solved by short
guards, because Java monitors are genuinely reentrant. CONCURRENCY-SCOPE §1
already lists the options; the middle one (lock at public entry points, call an
unlocked inner method internally) reuses the call graph and is the best fit.

## Migration, so the corpus does not explode

The audit set's own discipline, applied:

1. **Land `StorageStrategy` computing `Value` for everything.** No output
   change. Assert the plan is stable and the corpus is byte-identical. This
   proves the wiring before it can break anything.
2. **Turn on the real decision rule but keep lowering unchanged**, and *report*
   the disagreement: every type the plan says should be shared but is lowered by
   value is an `observable_alias`-class diagnostic. This gives a work queue and
   a size estimate before a single lowering changes.
3. **Implement `SharedImmutable` first.** It is the largest population and the
   only tier with no `borrow()` and no panic risk, so it isolates "did the
   handle plumbing work" from "did the borrow discipline work".
4. **Then `SharedLocal`**, gated per type. `tests/differential/known-gaps/Aliasing.java`
   moves up a level as the regression test.
5. **`SharedSync` only after** [0030a](0030-java-statics-are-thread-local.md),
   per CONCURRENCY-SCOPE's sequencing — mutable statics are wrong today under
   both models and must not be baked in.

Steps 1 and 2 are cheap, independently useful, and answer the question everyone
actually wants answered — *how much of the corpus is affected?* — before the
expensive part starts.

## What this design does **not** settle

- **Cycles leak.** `Rc` cycles (a parent/child graph, a doubly-linked list) never
  free. Java's GC collects them. This is a real semantic difference that no
  choice here fixes; `Weak` requires knowing which edge is the back edge, which
  is a genuinely open analysis. For a transpiler whose output is run rather than
  audited for memory, leaking is the right first answer, but it should be
  *stated* rather than discovered.
- **Which `PartialEq`.** With `Rc::ptr_eq` for `==`, Java's `.equals()` needs a
  separate path. That is 0028's other half and is not designed here.
- **Whether `SharedImmutable` needs interior mutability for lazy fields**
  (`hashCode` caching is the classic case). Probably yes, probably `Cell`.

## Recommendation

Do steps 1 and 2 next. They are small, they cannot regress anything, and they
convert "we do not know how big 0002b is" into a number. Everything after that
should be decided with that number in hand.

---

## Update 2026-08-03 — steps 1 and 2 are implemented, and the number is 47

Status: **steps 1–2 done.** `src/storage_strategy.rs` computes the plan and
reports the disagreements. Lowering is unchanged and the corpus output is
byte-identical, which was step 1's whole point.

### The number

Across the 248 files under `examples/`, `claude_audits/repro/` and
`tests/differential/`:

| tier | distinct types |
|---|---|
| `SharedLocal` (`Rc<RefCell<T>>`) | **34** |
| `SharedImmutable` (`Rc<T>`) | **14** |
| `SharedSync` (`Arc<Mutex<T>>`) | **0** |
| **total needing sharing** | **47** |

For comparison, [0002](0002-object-aliasing-has-no-representation.md)'s
`observable_alias` reports **5** hits over the same corpus. The gap is the
point: `observable_alias` finds aliases whose divergence it can *prove* at a
specific write, while this counts every type whose representation is wrong. The
first is a lower bound on the damage; the second is the size of the change.

`SharedSync` being empty matters for sequencing: nothing in this corpus needs
`Arc<Mutex<_>>`, so the tier that CONCURRENCY-SCOPE puts last, after
[0030a](0030-java-statics-are-thread-local.md), is not on the critical path.
Steps 3 and 4 (`SharedImmutable`, then `SharedLocal`) cover the entire measured
population.

### One correction to this document

> "**Every input already exists.** Not one of them needs to be built."

That was too strong. `EscapeLatticeValue` is a *value domain* in `lattice.rs`
with no analysis behind it — nothing computes escape for a type. The escape
input had to be written (`storage_strategy::Escape` plus the walk that fills
it), and the version here is deliberately shallow: a field of type `T`, or an
alias stored into a field or array element, is `Stored`; a method returning `T`
is `Returned`. `UnknownBoundary` is in the domain and is not yet produced,
because an unresolved-call escape needs the call graph.

The other three inputs *were* already there and were used as-is:
`locally_aliased` (the `T b = a;` shape, the same one that makes lowering
clone), `mutated` (`MethodUsageFacts.mutates_self`, already a fixpoint over
self-calls), and `threaded`.

That correction matters for the estimate: the escape input is the one that will
move the 47 up or down as it gets more precise, and it is currently biased
*low* (no `UnknownBoundary`), so 47 is a floor.

### Design points confirmed in code

- The lattice is `Value < SharedImmutable < SharedLocal < SharedSync` with join
  = max, and the order is asserted monotone by a unit test — the property that
  makes the join terminate.
- `Returned` alone does **not** force sharing: the value moves out, it does not
  become reachable from two places. `Stored` does. That distinction is pinned by
  a test, because it is the one most likely to be got wrong later.
- The decision is per **type**, taken as the join over uses, mirroring
  `HierarchyStrategy`.

### Recommendation, unchanged in shape but now costed

Do step 3 (`SharedImmutable`) next: 14 types, no `borrow()`, no panic risk, so
it isolates "did the handle plumbing work" from "did the borrow discipline
work". Then step 4 (`SharedLocal`, 34 types) with the
*never hold a borrow across a call* rule as a guarantee rather than a
convention. Step 5 has no population to serve yet.

---

## Update 2026-08-05 — `UnknownBoundary` is now produced, and the "47" is reconciled

### The correction the previous update promised

> `UnknownBoundary` is in the domain and is not yet produced … the escape input
> is the one that will move the 47 up or down, and it is currently biased *low*.

It is now produced. A handle passed as an argument to a call the translator
cannot see through escapes: the callee may retain the reference, and
under-sharing is a silent wrong answer while over-sharing costs a pointer
indirection.

No new analysis was needed, which is this design's recurring finding one input
later. `usage` already classifies every call argument as `InternalSameType`,
`KnownContentRead` or `Unknown`, and a binding already carries its declared Java
type; `collect_unknown_boundary_escapes` reads both.

`ReceiverForCall` is deliberately **not** an escape. Passing an argument gives
the callee a handle it can store; being the receiver does not.

### First: the "47" was never a well-defined number

Three different counts were being conflated, and the document's own table shows
it — `14 + 34 = 48`, while the stated total is `47`. Both are right under
different rules, and the missing sentence is that **a type can appear at two
tiers**, because the plan is computed per program and each fixture is its own
program. The distinct-name count is therefore the sum of the per-tier counts
minus the types that appear at both.

So `47 = 14 + 34 − 1`: fourteen `SharedImmutable`, thirty-four `SharedLocal`,
one type counted in both.

Measured today over the same three roots (`examples/`,
`claude_audits/repro/`, `tests/differential/`), stating the rule each time:

| counting rule | without `UnknownBoundary` | with `UnknownBoundary` |
|---|---:|---:|
| per-file occurrences | 49 | **51** |
| distinct names, `SharedImmutable` | 12 | **13** |
| distinct names, `SharedLocal` | 32 | 32 |
| sum of the two tiers | 44 | **45** |
| **distinct names overall** | 42 | **43** |
| types appearing at both tiers | 2 | 2 |

### What is attributable to this change, and what is not

The two columns above are the **same tree and the same corpus**, with only the
new escape input toggled. That is the attributable measurement:

> `UnknownBoundary` adds **one** distinct type (`SharedImmutable` 12 → 13) and
> two per-file occurrences.

Small — smaller than "will move the 47 up or down" implied. Worth stating
plainly rather than dressing up: the corpus's unresolved-call argument passing
mostly involves types that were already shared for another reason.

The remaining difference from the documented figure (47 distinct then, 43 now)
is **not** attributable to this change and is not claimed to be. The corpus grew
from 248 to 255 files, and the translator itself moved a great deal in between —
several of this round's fixes touch `usage`, which feeds the `mutated` and
alias inputs. Attributing that delta would need the old tree re-measured, which
has not been done. It is recorded here as drift, not as an effect.

### Consequence for sequencing

Unchanged. `SharedSync` is still empty, so step 5 has no population. Step 3
(`SharedImmutable`) is now **13 types** — still the right next step for the
reason the design gives: no `borrow()`, no panic risk, so it isolates "did the
handle plumbing work" from "did the borrow discipline work".

Lowering still does not consult the plan; this remains step 2, a report.

---

## Update 2026-08-05 (second) — a correction to the sequencing: `SharedImmutable` cannot be tested on its own

This document recommends step 3 (`SharedImmutable`) next, because it has no
`borrow()` and no panic risk and so "isolates *did the handle plumbing work*
from *did the borrow discipline work*". That is sound as engineering tactics and
**wrong as a plan**, for a reason the document does not state.

### The tier has no observable behaviour

`SharedImmutable` is by definition a type that is aliased and **never mutated
after sharing**. If it is never mutated, a copy and a shared handle are
indistinguishable — that is what "immutable" means. So changing the
representation of those 13 types changes no program output at all.

Measured rather than argued. `tests/differential/known-gaps/Identity.java`:

```java
Key a = new Key(5);
Key b = a;            // alias, never mutated
Key c = new Key(5);   // distinct object, equal contents
return b.get() + "," + (a == b) + "," + (a == c) + "," + a.equals(c);
```

| | Java | translation |
|---|---|---|
| `b.get()` — read through the alias | `5` | `5` ✓ |
| `a == b` — true identity | `true` | `true` ✓ |
| `a == c` — distinct objects | `false` | **`true`** |
| `a.equals(c)` — distinct objects | `false` | **`true`** |

Everything that is not an identity comparison already agrees. Both divergences
are [0028](0028-reference-equality-lowered-as-structural-equality.md).

### What that means for the plan

A differential gate cannot tell step 3 from a no-op: both report "nothing
changed". The change would be validated only structurally — the emitted types
are `Rc<T>` and the corpus still compiles — which is exactly the kind of
evidence this audit set keeps finding to be insufficient.

So the revised sequencing is:

> **Step 3 lands together with 0028's identity lowering**, not before it.
> `Rc::ptr_eq` for `==` is what gives the tier a test, and
> `known-gaps/Identity.java` is that test: it fails today on two of four cases
> and passes when both land.

This is not extra work bolted on. It is the same work in the order that makes it
verifiable: 0028 is *unimplementable* without the representation, and the
representation is *untestable* without 0028. They are one change.

Step 4 (`SharedLocal`) keeps its own test —
`tests/differential/known-gaps/Aliasing.java` — because mutation through an
alias is observable without any identity operator.

### A third finding, recorded where it belongs

`a.equals(c)` is `true` in the translation and `false` in Java. Java's
`Object.equals` **is** reference identity unless overridden, and the class in
the fixture does not override it. 0028 names this as its "other half" and
explicitly does not design it; it now has a reproducer, noted there.
