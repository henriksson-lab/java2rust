# 0002b — Shared object representation design and rollout

Status: **partially implemented** (`SharedImmutable` landed; `SharedLocal`/`SharedSync` remain open)
Blocks: [0002b](0002-object-aliasing-has-no-representation.md),
[0023](0023-inner-class-construction-always-panics.md),
and all of [CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md)

This exists because three separate documents all stop at the same sentence —
*"the object model has no shared representation"* — and
[CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md) step 2 says explicitly: **fix the
representation choice on paper before implementing it. Cheap now, expensive
later.** This is that paper.

The early sections below are the original design. Later updates record the
current implementation state: `SharedImmutable` handles and 0028 identity
lowering are landed; the mutable handle tiers and shared receivers are not.

## What one decision buys

| issue | today | under this design |
|---|---|---|
| 0002 | `Node b = a` copies; writes through `a` are invisible through `b` | `Rc::clone` — one object, two handles |
| 0028 | `a == b` is structural equality, violating the stated project decision | `Rc::ptr_eq` — *exactly* Java's reference identity |
| 0023 | ordinary outer-method inner construction still calls the panic fallback until a shared outer receiver can be passed to the existing companions | `__outer` stops being a one-off; it becomes the general receiver case |
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
| **escapes** | can a value of `T` be reachable from two live places? | `storage_strategy::Escape` (`LocalOnly \| Returned \| Stored \| UnknownBoundary`) computed by the storage-strategy planner |
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

That was too strong. The old `EscapeLatticeValue` was only an unused descriptor
domain and has since been deleted as part of 0062. The escape input had to be
written (`storage_strategy::Escape` plus the walk that fills it), and the version
here is deliberately shallow: a field of type `T`, or an alias stored into a
field or array element, is `Stored`; a method returning `T` is `Returned`.
`UnknownBoundary` is in the live domain and is not yet produced,
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
are [0028](done/0028-reference-equality-lowered-as-structural-equality.md).

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

---

## Update 2026-08-05 (third) — two wrong inputs, and the estimate halves

Attempting step 3 turned up two defects in the *plan itself*. Both were
over-approximations, both fired on very common shapes, and together they had
inflated the population and put it in the wrong tier.

### 1. `mutated` counted constructor field writes

`mutated` is the bit that separates `SharedImmutable` (`Rc<T>` — no borrow, no
panic risk) from `SharedLocal` (`Rc<RefCell<T>>` — borrow discipline, panic
risk). The question it is supposed to ask is whether a value is mutated **after
being shared**; this document says so in the lattice comment.

The walk counted *any* field write, including `this.v = v;` in a constructor. But
construction necessarily precedes any alias to the object — there is nothing to
observe a constructor's writes through. So a `final`-field value class, the exact
population this document names for `SharedImmutable` ("value objects, keys,
strings"), was landing in the harder tier:

```
Identity.Key needs SharedLocal (… locally_aliased=true, mutated=true …)
```

Constructor bodies are still walked for *escape*; only the mutation signal is
suppressed. A setter that happens to be called only from a constructor still
counts, which is right: it is public and can be called later.

### 2. `escape` charged the owner for `this.v = v` on an `int`

The `Stmt::Assign` arm charged `Escape::Stored` to the **owner** whenever a bare
handle was assigned into a field, with the comment "the type is not available
here without a type environment".

The field's declared type *is* available — the walk holds the owning `TypeDecl`,
and `field_type_name` was already being used two lines away for the mutation
signal. Charging the owner instead meant that **every class whose constructor
assigns a parameter to a field** was marked `Stored`, including `this.v = v`
where `v` is an `int`. That is almost every class, so the escape input reported
"shared" for types that never leave a local:

```
Unaliased.Local needs SharedImmutable (escape=Stored, locally_aliased=false …)
```

Now `this.f = x` charges `f`'s declared type. A *qualified* field or an array
element keeps the conservative owner charge, because there the element type
genuinely is not available.

### The revised measurement

Same corpus, same three roots, distinct names:

| tier | as documented | after both fixes |
|---|---:|---:|
| `SharedImmutable` (`Rc<T>`) | 14 | **19** |
| `SharedLocal` (`Rc<RefCell<T>>`) | 34 | **4** |
| `SharedSync` | 0 | 0 |
| **distinct types needing sharing** | **47** | **22** |

The population is **less than half** what this document costed, and the split
has inverted. That matters more than the total:

- `SharedLocal` is the tier with the `RefCell` hazard — the *"never hold a
  borrow across a call"* rule, and the risk of turning a Java-legal program into
  a runtime abort. It is **4 types**, not 34.
- `SharedImmutable` is 19 of 23 tier assignments, and it needs no `borrow()` at
  all.

So the "large but not deep" characterisation holds and the depth is concentrated
in four types. Whether the remaining 4 justify `RefCell` at all, or should be
handled some other way, is now a question worth asking — it was not, at 34.

### What this does *not* change

The sequencing correction from the previous update stands: `SharedImmutable`
still has no observable behaviour of its own, so step 3 still has to land with
0028's identity lowering to be testable. Fewer types, same acceptance test.

### Pinned

Three tests in `src/storage_strategy.rs`: a constructor-only writer is
`SharedImmutable`, a method writer is still `SharedLocal` (or the distinction
would be meaningless), and an unaliased type stays `Value`. They resolve the
program first, because the plan is keyed by qualified nested type names.

`scripts/gate.sh --all`: zero deltas — the plan is still report-only.

---

## Update 2026-08-05 (fourth) — step 3 attempted; the blocker is not where this document says

Status: **step 3 not landed.** Attempted, measured, and reverted. What follows
replaces "it is wiring plus a lowering rewrite, not new analysis" with a number
and a specific obstacle, because that sentence is true and materially
understates the wiring.

### What was tried

`shared_handle_rust_type` in `src/slot_render.rs`: wrap a `SharedImmutable`
type's rendered `RustType` in `std::rc::Rc<…>`, hooked into
`rust_type_for_java_type_with_facts_and_representations` — the rendering entry
point that has the plan.

`SharedImmutable` was chosen exactly as this document recommends: `Rc<T>` with no
`RefCell`, so reads and `&self` calls work through auto-deref and there is no
borrow discipline to get wrong.

### The result, and what it proves

`scripts/gate.sh --all`: **zero deltas.** Not "no regressions" — the hook had no
effect at all. `known-gaps/Identity.java` still emitted
`let a: IdentityKey = IdentityKey::new(5);`, unwrapped.

That is the finding: **the Rust type of a source class is never rendered through
a representation-aware path.** The plan-carrying entry point exists and is
consulted 11 times, and none of those 11 are where a source class's type is
decided.

### The actual wiring cost, counted

Type rendering has two entry points, and the split is not even:

| variant | call sites |
|---|---:|
| `rust_type_for_slot_with_facts` / `…_java_type_with_facts` (**no plan**) | **49** |
| `…_with_facts_and_representations` (has the plan) | 11 |

By file, for the facts-only variant: `lower.rs` 12, **`item_assembly.rs` 12**,
`slot_render.rs` 7, `stdlib.rs` 7, `representation_lowering.rs` 4, and five
others with 1–2 each.

`item_assembly.rs` is the one that matters and the worst placed: it renders
**struct fields, parameters and return types**, and it mentions
`representations` exactly **zero** times. Its whole public surface takes
`(shape, facts)`.

### Why it cannot be done incrementally

A type's Rust spelling has to change at *every* site at once. Render a field as
`Rc<Key>` while a parameter is still `Key` and the output does not compile — so
unlike every other migration in this audit set, there is no monotone
"tag-first, text-fallback" path where a half-done state is merely less precise.
The switch is atomic.

That is what makes this expensive, and it is not "a lowering rewrite" — it is a
signature change across 49 call sites in 10 files, which must land before any
behaviour changes.

### The order that makes it verifiable

This document's own step 1 discipline applies, one level down:

1. **Thread the plan through all 49 sites without consulting it.** A pure
   signature change; the gate must show zero deltas. Large, mechanical, and
   provably inert.
2. **Then flip the switch for `SharedImmutable` only** — 19 types — together
   with 0028's identity lowering, since (per the previous update) the tier has
   no observable behaviour without it and `known-gaps/Identity.java` is the
   acceptance test.
3. `SharedLocal` is **4 types** and can wait, or be reconsidered entirely.

Step 1 is the honest next unit of work. It is not small, but it is safe, and
until it exists nothing about step 3 can be tested.

### Reverted

The hook was removed rather than left in place. An inert hook that looks like an
implementation is worse than no hook: it would read as "shared representation:
started" when nothing reaches it.

---

## Update 2026-08-05 (fifth) — step 3 implemented end to end; it works, and it costs 17 tests

The previous update said the blocker was threading `RepresentationPlan` through
49 sites. **That was the wrong diagnosis.** `FactSet` already reaches all 49,
and it already carries per-type decisions (`HierarchyStrategy`,
`TypeCapabilityOverride`). Putting `StoragePlan` there made the storage decision
visible everywhere for one field and one population line.

So step 3 was implemented, and it **works**:

```rust
let a: std::rc::Rc<IdentityKey> = std::rc::Rc::new(IdentityKey::new(5));
let b: std::rc::Rc<IdentityKey> = a.clone();          // Rc::clone — one object
let c: std::rc::Rc<IdentityKey> = std::rc::Rc::new(IdentityKey::new(5));
std::rc::Rc::ptr_eq(&a, &b)   // true
std::rc::Rc::ptr_eq(&a, &c)   // false
```

`tests/differential/known-gaps/Identity.java` — the acceptance test this design
defined two updates ago — **passes against the Java oracle**: both toolchains
print `5,true,false,false`. That is 0002's aliasing, 0028's `==`, and 0028's
`.equals()` half, all satisfied for the `SharedImmutable` tier.

Four pieces, each small: the plan on `FactSet`; a wrap in
`rust_type_for_java_type_with_effective_facts`; `Rc::new` at the one source
constructor site; and `Rc::ptr_eq` for `==` and for an inherited `equals`.

### Two soundness defects it exposed, both now fixed

**`mutated` must over-approximate, and did the opposite.** It saw field writes
through `this` and `this.f` only, so `arr[i].f = v` and `other.f = v` were
invisible. A type mutated that way was classified `SharedImmutable`, became
`Rc<T>` with no `RefCell`, and produced

```
error[E0594]: cannot assign to data in an `Rc`
```

This document's "over-sharing costs a pointer indirection" argument does not
cover it: **under-detecting mutation does not cost an indirection, it does not
compile.** `mutation_provably_absent` now records whether the walk could see all
writes, and `SharedImmutable` requires it. A field write the walk cannot
attribute clears the flag for every type.

**Enums and interfaces were in the lattice at all.** A Java `enum` constant is a
singleton by construction — nothing to alias, no identity question — and it
lowers to a Rust `enum`, where a shared handle breaks `match`, derived equality
and the switch lowering. That alone was 10 of the initial 32 failures. The plan
is now classes only.

### What it still costs, and why it was reverted

After both fixes: `differential` green, `slot_shape_design` −2, **`idiom_gaps`
538 → 521**. Seventeen regressions, and they do not cluster:

```
borrowed_downcast_source_subtype_argument_upcasts_after_clone_recovery
reused_source_subtype_argument_upcasts_after_clone_recovery
constructor_nested_arg_reads_assigned_field_without_cloning_self
implicit_source_super_constructor_initializes_inherited_fields
source_self_referential_instance_fields_use_boxed_optional_storage
source_borrowed_getter_initializers_recover_owned_locals
source_interface_object_getter_returns_borrowed_field
generic_source_map_storage_return_adds_clone_bound
emitted_source_call_uses_authoritative_owned_param_signature
context_translation_does_not_emit_opaque_facade_for_sibling_source_type
inherited_source_summary_field_types_emit_opaque_support_shells
source_summary_direct_field_type_emits_opaque_support_shell
private_static_singleton_getter_lowers_to_owned_source_value
erased_object_set_uses_hash_free_vec_carrier
exception_self_as_throwable_context_argument_uses_facade_boundary
foreach_lambda_throw_wraps_enclosing_method_in_result
nullable_custom_equals_method_uses_null_safe_custom_equals
```

Each is a separate place that assumes a source class's Rust type is the bare
struct — subtype upcasts, clone recovery, super-constructor field
initialisation, self-referential `Box` storage, borrowed getters, facade shells.
That is the real cost of the change, and it is **the list, not a number**: it is
a work queue with seventeen entries, each individually small.

The switch was reverted so the gate stays green. What is kept is everything that
is inert or independently correct:

- `FactSet::storage_strategies` and its population — the plumbing, verified
  inert;
- the constructor-mutation and escape-precision corrections (previous update);
- `mutation_provably_absent`;
- classes-only.

### Revised recommendation

Do **not** treat step 3 as a design problem any more. It is implemented, it is
four small pieces, and the acceptance test passes. What remains is working the
seventeen-item list above, which can be done one at a time with the gate as the
check — unlike the switch itself, those *are* individually attributable.

The realistic sequence: reapply the four pieces on a branch, then fix the list
top-down. `SharedLocal` (4 types) still needs `Rc<RefCell<T>>` and the
never-hold-a-borrow-across-a-call rule, and is still last.

---

## Update 2026-08-05 (sixth) — the 17 triaged, and one of them is a design conflict with 0030

The seventeen-item list from the previous update was worked. Triaging it changes
what it is: **five** are genuinely broken output, **eleven** are tests asserting
the old spelling, and **one** of the five is not a wiring bug at all.

### `Rc` and process-wide statics are incompatible

```
error[E0277]: `Rc<Standards<NamedLine>>` cannot be sent between threads safely
  --> static STANDARDS_REGISTRY_LINES:
       std::sync::Mutex<Option<std::rc::Rc<Standards<NamedLine>>>> = …
  = note: required for `Mutex<Option<Rc<…>>>` to implement `Sync`
  = note: shared static variables must have a type that implements `Sync`
```

This document anticipates `Sync` propagation, but attributes it to *threading*:

> **`Sync` propagates.** A `static Mutex<T>` requires `T: Send`, which pushes
> `Send`/`Sync` requirements through the object graph.

The trigger is not threading. [0030](done/0030-java-statics-are-thread-local.md)
made every Java `static` a process-wide `std::sync::Mutex`, which is required
for correctness and is not negotiable — so **any type reachable from a Java
`static` field must be `Send + Sync`**, in a single-threaded program, with no
`Thread` anywhere in the source.

`Rc` is neither. So `SharedImmutable` cannot be `Rc<T>` for such a type; it must
be `Arc<T>`.

That is a change to the lattice, not to the wiring. The tier decision needs a
fourth input — *reachable from static storage* — and it selects `Arc` for the
same reason `threaded` does, independently of any thread existing. Concretely:

```
SharedImmutable + (threaded ∨ static-reachable)  ->  Arc<T>
SharedLocal     + (threaded ∨ static-reachable)  ->  Arc<Mutex<T>>
```

`SharedSync` in this document is defined by threading alone and is measured at
**0** types. Adding static-reachability will make it non-empty, and the
sequencing note that "`SharedSync` is not on the critical path" is therefore
wrong — it is on the critical path for any corpus with a static registry, which
is a common Java shape.

### The other four are the conversion machinery

```
CompoundLine: From<Rc<FormatLine>> is not satisfied      (subtype upcast)
Box<dyn Any>: From<Rc<FormatLine>> is not satisfied      (erasure)
mismatched types                                          (custom equals, ×2)
```

All one thing: a conversion that used to see `T` now sees `Rc<T>` and does not
know to reach through it. The handle is transparent for *reads* (auto-deref) and
for *cloning* (`Rc::clone`), and opaque for every **conversion** —
`From`/`Into`, erasure to `Box<dyn Any>`, and supertype upcast. Those are the
sites the change actually has to teach, and there are four of them, not
seventeen.

### And eleven stale expectations

The remaining eleven assert the old spelling and would simply be updated —
including `source_self_referential_instance_fields_use_boxed_optional_storage`,
which asserts `Option<Box<Node>>`. That one *should* change: with a shared
handle the `Box` is redundant, giving `Option<Box<Rc<T>>>`, an indirection
through an indirection. Suppressing it is a two-line change (verified), and this
document already predicts it.

### Revised cost, and the revised order

| | count | kind |
|---|---:|---|
| conversion sites to teach | **4** | real work |
| stale test expectations | 11 | update |
| `Rc` vs static storage | 1 | **design change to this document** |

So the honest sequence is now:

1. **Settle the tier scheme first.** Add static-reachability to the lattice and
   accept that `Arc` — not `Rc` — is the common case wherever Java statics hold
   objects. Doing the wiring before this would produce code that has to be
   redone.
2. Teach the four conversion sites.
3. Update the eleven expectations.

Step 1 was not visible before this pass, and it is why step 3 has been reverted
a second time rather than pushed through: landing `Rc` now would bake in a
handle that 0030 has already made wrong for a whole class of types.

---

## Update 2026-08-06 (seventh) — the handle hierarchy, and the residual is now 14

Step 3 was reapplied with the corrections from the previous passes, verified,
and reverted again. What changed is the design, and the residual count.

### The `Rc`/`Arc` conflict is resolved by a hierarchy, not by picking one

The sixth update established that `Rc` cannot be used for anything a Java
`static` reaches, because a Rust `static` must be `Sync` and
[0030](done/0030-java-statics-are-thread-local.md) makes every Java `static` a
real one. Demonstrated, rather than argued — `Mutex<Option<Rc<T>>>`,
`OnceLock<Rc<T>>` and a bare `static Rc<T>` all fail, and the `const` escape does
not apply because `Rc::new` is not `const`.

The answer is **not** to use `Arc` everywhere. `StoragePlacement` now carries
three independent axes, and the handle follows from one of them:

```text
rust_type(T) = handle( cell( T ) )

  cell(T)   = T | RefCell<T> | Mutex<T>     interior_mutable, then sync
  handle(X) = X | Rc<X>      | Arc<X>       shared,           then sync
```

`RefCell`/`Mutex` and `Rc`/`Arc` are chosen by the **same** `sync` bit, so they
cannot disagree — an invariant the previous `StorageForm` match maintained by
hand. `sync` is `threaded ∨ static_reachable`, the second being a transitive
closure over fields from any `static`.

Measured over the corpus: **zero** types need `Arc`. All 15 shared types are
`Rc`, so the cheap rung covers everything today and the atomic rung exists for
correctness rather than for use. That is the argument for the hierarchy over
"just use `Arc`".

The type form and the value form are derived together — `wrap` gives
`Rc<RefCell<T>>`, `wrap_construction` gives `Rc::new(RefCell::new(v))` — after a
hand-written construction string got the nesting wrong. A law test asserts they
nest identically, alongside the lattice laws (commutative, associative,
idempotent, extensive) checked exhaustively over the 8-element domain.

### The acceptance test passes

`tests/differential/known-gaps/Identity.java`, against the Java oracle: both
toolchains print `5,true,false,false`. Aliasing, `==` and the inherited
`.equals()` half, all satisfied for the shared tier.

### The residual, and where it now sits

Reapplied with the narrower rules — the wrap at **owned slots only**, since a
borrowed slot sees through the handle — the cost is **14** `idiom_gaps` tests,
down from the 17 the fifth update measured. Every one is a *conversion*:
`E0308` where a downcast yields `T` into an `Rc<T>` slot, `E0277` where an
upcast wants `T` from an `Rc<T>`.

First, a `HandleAdaptation` sibling field was added to `SlotExprConversionPlan`
to answer that in one place — the outer plan holds both shapes and the facts, so
the decision seemed to belong there and applying it seemed like execution. It
did not reduce the count, and **the reason is now known**:

`apply_slot_conversion_plan` has **four** callers, and two of them
(`coercion.rs:711`, `coercion.rs:3670`) take a bare `&SlotConversionPlan` with
no access to a sibling field on the outer plan. Putting the crossing beside the
conversion meant only the applier that was taught about it could see it — the
same "one decision, several places" shape this audit set keeps finding, arrived
at by writing it. That sibling field has since been deleted; the current inert
plumbing is only the in-plan variant below.

Fixed by moving the crossing **inside** the plan:

```rust
SlotConversionPlan::Handled { handle: HandleAdaptation, inner: Box<SlotConversionPlan> }
```

`apply_slot_conversion_plan` unwraps it, so all four callers get the crossing
whether or not they know it exists. This is landed and **inert** — with values
held inline `plan_handle_adaptation` returns `None`, so no `Handled` is ever
constructed and the gate shows zero deltas.

The re-measurement was then run, and it gives a **negative result worth keeping**:
with all five pieces on, the count is 15 — the crossing inside the plan did not
reduce it. Tracing one failure shows why:

```rust
let that: std::rc::Rc<FormatLine> = *o.downcast::<FormatLine>().expect("invalid Java cast");
```

The downcast is built by the instanceof/cast binding path, which constructs its
`RustExpr` **directly** rather than routing through `plan_slot_conversion`. So
`apply_slot_conversion_plan` was never the single choke point; it is one of
several places that build a converted expression, and the others do not consult
a plan at all.

That is the corrected shape of the residual. It is not "teach the conversion
machinery" and not "one enum variant": it is **teach each expression-building
path that produces a value for a slot**, of which the cast/instanceof binding is
one and the four plan appliers are others. The `Handled` variant is still the
right home for the ones that *do* plan, and is kept and inert.

### Kept, and independently useful

- `StoragePlacement`, its three axes, `wrap`/`wrap_construction`, and the law
  tests.
- The `Rc`/`Arc` hierarchy and static-reachability.
- `FactSet::storage_placement_for` — the plan visible to every renderer.
- `mutation_provably_absent`, the classes-only restriction, and the
  constructor-mutation and escape-precision corrections.

### Honest status

Four audits — 0002, 0002b, 0023, 0028 — are one change. It is implemented,
tested against the oracle, and not landed, because landing it means 14
regressions in a suite whose value is that it sits at baseline.

It is no longer a design question. The five pieces to re-enable are named in
this file and the crossing is in place for the planned paths.

One further correction, from following that query. The erased-object downcast
*does* go through a plan — `apply_erased_object_downcast` is inside
`apply_shape_coercion_plan`. The gap was one level up again: the crossing was
decided in `plan_slot_conversion`, while plans are also built directly by
`slot_conversion_plan_with_facts`, which several callers use. Moving the
decision into that function reaches every producer.

With that done and step 3 on, the residual got **worse** — 15 → 16. So the
adaptation *rule* is not yet right: it wraps or unwraps at sites where neither
is wanted. That is the actual remaining work, and it is not a plumbing question.

### What five attempts have established

Each attempt corrected the diagnosis, and the corrections are the durable part:

| attempt | believed blocker | what it actually was |
|---|---|---|
| 1 | thread the plan through 49 render sites | `FactSet` already reaches all of them |
| 2 | `Rc` everywhere | `Rc` is illegal under a Java `static`; needs the `Rc`/`Arc` hierarchy |
| 3 | 17 diffuse regressions | 4 conversions + 11 stale expectations + 1 design conflict |
| 4 | teach `apply_slot_conversion_plan` | it has four callers; the crossing belongs *inside* the plan |
| 5 | the downcast bypasses the planner | it does not; the *plan builder* was the wrong level, and fixing that exposed that the rule itself is wrong |

The rule is where it stops. `Wrap` when the target is handled and the source is
not, `Unwrap` in the reverse — that is too coarse, and the sites it damages have
not been characterised. Characterising them is the next unit of work, and it
needs the failure list read one by one rather than another structural move.

## What the sixth attempt measured

The five earlier attempts each ended in a revert with a *guess* at the blocker.
This one ends with numbers. All figures are `idiom_gaps`, baseline **538/146**.

| configuration | result | vs baseline |
|---|---:|---:|
| crossing only, `Wrap` + `Unwrap`, no type wrap | 534/150 | −4 |
| crossing only, `Wrap` alone, no type wrap | 538/146 | **0** |
| type wrap + `Wrap` alone | 525/159 | −13 |
| type wrap + `Wrap` + `Unwrap` | 522/162 | −16 |

### `Unwrap` is pure loss — delete it from the lattice

Diffing the failure *sets* between the last two rows, rather than the counts:

- tests `Unwrap` moves fail → pass: **none**
- tests `Unwrap` moves pass → fail: **three**
  (`source_borrowed_getter_initializers_recover_owned_locals`,
  `source_interface_object_getter_returns_borrowed_field`,
  `source_receiver_generic_result_upcasts_to_supertype_assignment_slot`)

So `HandleAdaptation::Unwrap` is not a rule that needs refining; it is the wrong
mechanism, and the asymmetry has a structural reason:

> A conversion plan can **wrap** — it constructs the value, so it owns the point
> where the handle is created. It cannot **unwrap**, because the sites that need
> the unwrap are not conversion plans at all.

The sites are `Coercion`s, applied by `coercion::apply_one(expr, coercion)` —
which takes no `FactSet` and therefore *cannot* ask whether a type is shared.
That is why five attempts at moving the decision to a better level all failed:
every level tried was still inside the plan, and the plan never reaches these.

### The 13 regressions, classified

Split by whether the generated Rust still compiles:

**8 are stale expectations, and the new output is the better one.** Two checked
in full:

- `source_self_referential_instance_fields_use_boxed_optional_storage` asserts
  `parent: Option<Box<SelfLinkedNode>>`; the wrap emits
  `Option<Rc<SelfLinkedNode>>`. `Box` gives each node *exclusive* ownership of
  its parent, which no Java object graph has. The test encodes 0002.
- `private_static_singleton_getter_lowers_to_owned_source_value` — the name is
  the defect. The wrap emits
  `static CONFIG_INSTANCE: Mutex<Option<Arc<Config>>>` with
  `get_instance() -> Arc<Config>`; the asserted form returns `Config` *by value*,
  so every caller gets a copy and the singleton is not one. Note `Arc`, not
  `Rc` — the static-reachability closure picked the tier correctly with no
  special-casing.

**5 are real compile breaks, and three are one defect** —
`CompoundLine: From<Rc<FormatLine>>`, `Box<dyn Any>: From<Rc<FormatLine>>`:
`lines.push(line.into())` where `line` is now a handle. The conversion is
applied *to the handle* instead of to its contents.

### What lands this

1. Delete `HandleAdaptation::Unwrap` (measured pure loss above). **Done in the
   follow-up audit pass**; only `Wrap` remains in the inert `HandleAdaptation`
   enum.
2. Teach the five upcast sites. Two candidate designs, neither yet built:
   - resolve the placement where the `Coercion` is *constructed*
     (`external_conversion_adapter_policy` has a `&FactSet`) and carry it in the
     variant, so `apply_one` unwraps without needing facts; or
   - generate `impl From<Rc<Sub>> for Super` alongside the existing `From<Sub>`,
     so the conversion surface follows the storage form and no unwrap is needed.
     This also lets `Box<dyn Any>` hold the `Rc` and *keep* identity through the
     erasure, which the unwrap would destroy.

   The second design is the better one for the three `From` cases, and the
   impls are generated in one place — `source_supertype_conversion_impl_for_pair`
   in `src/lower.rs`. But it does **not** stretch to cover the erasure case, and
   the reason is a hard language rule rather than a design choice:

   ```rust
   impl From<FormatLine> for Box<dyn Any> { .. }      // generated today; compiles
   impl From<Rc<FormatLine>> for Box<dyn Any> { .. }  // error[E0117]
   ```

   `Rc` is not `#[fundamental]` (only `Box` and `&` are), so `Rc<Local>` does
   not count as a local type and the orphan rule rejects the companion. Checked
   against real rustc, not assumed.

   So the erasure site needs the *other* design: emit `Box::new(handle)` in place
   of `handle.into()`. That is the identity-preserving lowering anyway — the
   erased carrier holds the `Rc`, and a downcast recovers the same object rather
   than a copy — so the two designs are not competitors. Locally-emitted
   supertypes take the companion impl; `Box<dyn Any>` takes the changed
   coercion.
3. Update the 8 expectations, each of which asserts a 0002 symptom.

Steps 2 and 3 are ordinary work. Step 1 is the finding: the previous five
attempts were all trying to place a rule that should not exist.

## The seventh attempt: three of five fixed, and the actual blocker found

Working the five compile breaks individually — which is what the previous
section said to do — closed three of them with changes that are principled
rather than local patches, and turned the remaining two into a single sentence.

Progression, `idiom_gaps`, baseline **538/146**:

| step | result |
|---|---:|
| type wrap on (+ `Box` suppression) | 525/159 |
| + `From<Rc<Sub>>` companion impls | 526/158 |
| + `Box::new(handle)` at the erasure | 527/157 |
| + downcast recovers the handle | **528/156** |
| + wrap fresh `new` at slot boundaries | 527/157 (worse — dropped) |

### What fixed the three

- **`From<Rc<Sub>>` companions**, generated next to the existing `From<Sub>` in
  `source_supertype_conversion_impl_for_pair`, gated on the subtype being
  shared. Not emitted for `Box<dyn Any>` — E0117, above.
- **`Box::new(handle)` instead of `handle.into()`** at the two upcast sites that
  target an erased carrier. This is the identity-preserving lowering: the
  carrier holds the shared handle, so a downcast recovers *the same object*
  rather than a copy.
- **The downcast asking for the handle** (`downcast::<Rc<T>>`) — the paired half
  of the above. On its own it is silently wrong: `plausible_rust_nominal_type_name`
  judged a target by its *first character*, so `std::rc::Rc<FormatLine>` was
  rejected and the downcast was **dropped without a diagnostic**, letting the
  value flow on at the wrong type. That check now judges the innermost
  identifier. **Kept — it is an independent latent bug** (any qualified or
  generic downcast target hit it) and is gate-clean on its own.

### The blocker, stated exactly

The last two breaks are both a producer handing back a bare `T` where the slot
now holds `Rc<T>`:

```rust
let mut reader: std::rc::Rc<Reader> = future.get();          // external facade
pub fn lines() -> std::sync::Arc<Standards<NamedLine>> {
    return Standards::new();                                  // constructor
}
```

Fixing these needs one question answered at the use site: *is this expression
already behind a handle?* It cannot be answered.

> `RustSlotShape` records `java_ty` and `ownership`. It does **not** record
> whether the value sits behind a handle, because the handle is added by
> `slot_render` — *after* the shape, at rendering time. Two values with
> identical shapes render as `Rc<FormatLine>` and `FormatLine`.

That is why all six earlier attempts failed to place the crossing, and why no
amount of moving it helps: `Wrap` and `Unwrap` are both functions of a
distinction the IR does not carry. Restricting the wrap to `new` — the one
producer that is *structurally* known to be bare — was tried and measured
**worse** (528 → 527), because a `new` feeding a slot that is already handled
then double-wraps.

### So the next change is not in the crossing at all

Move the handle **into the shape**: a field on `RustSlotShape` set where the
storage decision is made, with `slot_render` reading it instead of re-deriving
it from `FactSet`. Then "is this a handle" is answerable at every site, the
crossing becomes a total function of two shapes rather than a guess, and the two
remaining breaks — plus the ~11 the corpus will have — are decided rather than
patched. This is the same defect [0007](0007-rust-ir-is-mostly-opaque-strings.md)
describes: a property that exists only in rendered text cannot be reasoned about.

Everything except the `plausible_rust_nominal_type_name` fix was reverted; the
gate is at baseline on all 18 targets.


## Update 2026-08-06 — four of the five breaks closed; the fifth names the change

Re-applied the stack and worked the remaining breaks one at a time. Where the
previous round left two unexplained, this round closed one more and turned the
last into a specific, non-obvious requirement.

| step | idiom_gaps |
|---|---:|
| baseline | 538/146 |
| type wrap + `Box` suppression | 525/159 |
| + `From<Rc<Sub>>` companions | 526/158 |
| + `Box::new(handle)` at the erasure | 527/157 |
| + downcast recovers the handle | 528/156 |
| + construction wrap at the static initializer and the return slot | **529/155** |

Nothing regressed at any step after the first; each addition was checked by
diffing failure *sets*, not counts.

### Two dead ends, both measured

- **Wrapping every `new` at `lower_expr_for_slot`** fixed nothing and broke
  `exception_self_as_throwable_context_argument_uses_facade_boundary`. The
  reason is worth keeping: the return path calls
  `lower_expr_for_slot_with_refinements` *directly*, so the wrapper is not on
  the path at all. Moving the wrap to the inner function did not help either --
  the value it needed to reach is not lowered from Java source.
- **Wrapping the slot default** (`default_expr_for_slot_with_effective_facts`)
  changed nothing either way. Correct in principle, unreachable in practice for
  these cases.

What did work was narrower and better justified: a **static field initializer**
and a **folded static read in return position** are both fresh constructions, so
they can be wrapped without answering "is this already a handle?". That is the
same argument the design gives for `wrap_construction`, applied at the two
places where a bare construction actually meets a handle-typed slot.

### The last break states the requirement precisely

```rust
let mut reader: std::rc::Rc<Reader> = future.get();   // expected Rc<Reader>, found Reader
readers.insert(name.clone(), reader);                 // expected Reader,     found Rc<Reader>
```

`Reader` is a source class and genuinely shared -- it is aliased into a map. The
two errors point in **opposite directions**, which is the whole finding: the
*local* slot is handle-wrapped and the *map element* is not. The wrap is applied
by `shared_handle_slot_rust_type`, which needs `OwnershipShape` to know that a
borrow must see through the handle. Generic arguments are rendered from a
`JavaType` by `rust_type_for_java_type_with_effective_facts`, where no ownership
is available -- so element positions cannot be wrapped at the level the wrap
currently lives.

Making a shared type shared *everywhere* -- which is the only consistent rule --
therefore requires the handle decision to travel with something that carries
ownership. That is the `RustSlotShape` migration this file already names, and
this is now a second, independent argument for it: not just "the crossing cannot
be decided", but "the wrap cannot be applied uniformly".

Reverted to baseline; the re-apply script survives at
`<scratchpad>/apply_0002b.py` and reproduces 528/156 in one run.


### Tested and rejected: wrapping at the Java-type level

The obvious way to reach generic positions is to move the wrap down into
`rust_type_for_java_type_with_effective_facts`, on the reasoning that a generic
argument is always *owned* by its container -- a borrow only ever appears at the
top level of a slot -- so no `OwnershipShape` is needed there.

The reasoning about elements is correct. The change is still wrong, and measured
**522/162**, worse than the slot-level wrap's 525/159. That function renders the
top-level type of *borrowed* slots too, so it produced `&Rc<T>` where `&T` was
wanted, breaking exactly the two tests `Unwrap` broke earlier
(`source_borrowed_getter_initializers_recover_owned_locals`,
`source_interface_object_getter_returns_borrowed_field`) -- the same two, from a
different direction, which is a good sign the borrow boundary is the real edge.

So the wrap needs **both** levels, distinguished:

- top level, ownership-aware — `shared_handle_slot_rust_type`, already written;
- nested element/argument positions, unconditional — not yet placed, because the
  generic arguments of a collection are not rendered through a single
  identifiable call. `class_token_arg_rust_type` handles class tokens,
  `SliceView`/`Optional` handle their own elements, and the collection carriers
  render their arguments elsewhere again.

Finding that one place — or introducing it — is the next concrete step, and it is
smaller than the full `RustSlotShape` migration. The migration is still needed
for the *crossing*; it is not needed for the *wrap*.

## Update 2026-08-06 (second pass) — all five compile breaks closed

The generic-argument position turned out to have a single, already-named home:
`rust_type_for_java_generic_arg_with_effective_facts` (9 call sites, all of them
argument positions). Wrapping there — unconditionally, since a generic argument
is always owned by its container — fixed the last break:

```rust
let mut readers: HashMap<String, std::rc::Rc<Reader>> = HashMap::new();
let mut reader: std::rc::Rc<Reader> = future.get();
readers.insert(name.clone(), reader);          // now type-checks
```

With that, **529/155, and the nine remaining failures appeared assert-only**.
That reading held until the sixth expectation exposed invalid generated Rust
below. The wrap therefore needs exactly two placements, and the earlier "cannot
be applied uniformly" conclusion was wrong:

| position | wrap | why |
|---|---|---|
| top level of a slot | ownership-aware (`shared_handle_slot_rust_type`) | a borrow must see through the handle |
| generic argument | unconditional (`..._generic_arg_...`) | always owned by its container |

Five of the nine expectations were then updated and pass. Two were renamed,
because the names asserted the defect: `..._use_boxed_optional_storage` →
`..._use_shared_handle_storage`, and `..._lowers_to_owned_source_value` →
`..._lowers_to_shared_handle`.

### What stopped it: a latent bug the wrap made visible

Updating the sixth expectation surfaced **invalid generated Rust**:

```rust
let mut __java_self = ChildProblem {
    max: max.clone(),
    context: std::rc::Rc<ExceptionContext>::new(),   // not valid Rust
};
```

Something in constructor lowering renders the field's Rust **type** and appends
`::new()`. With a bare `ExceptionContext` that happens to read correctly, so the
bug has always been there and never showed; the moment the type is `Rc<T>` it
emits a syntax error. (`Rc::<T>::new()` would be the valid spelling — the
generated form is not.)

`implicit_source_super_constructor_initializes_inherited_fields` does **not**
call `cargo_check_generated_modules`, which is why an assert-only failure was
hiding a non-compiling program. That is [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md)'s
blind spot inside the test suite itself.

Not located: it is not `static_storage_initial_value_expr`,
`lower_static_initialized_singleton_field_for_slot`, `render_slot_default_policy`,
or `plan_constructor_field_values_*` — all of those build the path from
`rust_name_for_java_named_type`, which is a name mapper and cannot produce
`Rc<…>`. Finding the real site is the next step, and it is worth doing on its own
merits: **a construction path that is a rendered type is a bug with or without
0002b.**

### Landing checklist, all of it now specific

1. Find and fix the `{rendered_type}::new()` construction path above.
2. Add `cargo_check_generated_modules` to
   `implicit_source_super_constructor_initializes_inherited_fields` so it cannot
   regress silently again.
3. Update the last four expectations (`constructor_nested_arg_reads_assigned_field_without_cloning_self`,
   `emitted_source_call_uses_authoritative_owned_param_signature`,
   `foreach_lambda_throw_wraps_enclosing_method_in_result`,
   `implicit_source_super_constructor_initializes_inherited_fields`).
4. Record the new baseline; 0002, 0023 and 0028 then unblock in that order.

Reverted to baseline. `<scratchpad>/apply_0002b.py` re-applies steps 1-5 and,
with the generic-arg wrap above, reproduces 529/155 in one run.

## Update 2026-08-06 (third pass) — 535/149, three left, and the last defect named

The constructor-path bug from the previous pass was found:
**`src/call_lowering.rs:978`** builds a constructor's owner path by rendering the
owner's *slot type*:

```rust
let owner_type = crate::emit::emit_type(&rust_type_for_slot_with_facts(&target.owner_shape, facts));
```

A path position is not a value position. With a bare `T` that reads correctly by
accident; the moment the slot type carries a handle it emits
`std::rc::Rc<ExceptionContext>::new()`, which is not valid Rust. Fixed by
rendering the owner without storage decoration
(`path_type_for_slot_with_facts_and_representations`), which closed
`implicit_source_super_constructor_initializes_inherited_fields` and took the run
to **530/154**. That fix is worth landing on its own: a constructor path built
from a rendered slot type is wrong regardless of 0002b.

Updating six expectations then reached **535/149 — three failures left**:

| test | why |
|---|---|
| `constructor_nested_arg_reads_assigned_field_without_cloning_self` | `Some(Wrapper::new(..))` into an `Option<Rc<Wrapper>>` field |
| `emitted_source_call_uses_authoritative_owned_param_signature` | not yet read |
| `foreach_lambda_throw_wraps_enclosing_method_in_result` | not yet read |

### The one remaining shape of defect

All the value-side wraps that landed share a property: they act where a *fresh
construction* meets a handle-typed slot, and each was placed at a different
point — the slot default, the static field initializer, the folded static read.
The constructor-field case does not yield to the same treatment:

```rust
// wrapping before the optional wrapper:
Some(std::rc::Rc::new(Some(Wrapper::new(..))))   // the Some is applied twice
// wrapping after it:
std::rc::Rc::new(Some(Wrapper::new(..)))         // handle outside the Option
```

The wrap has to go **between** the element value and its `Some`, and that point
is inside `lower_expr_for_slot`, which does not distinguish "the element of an
optional slot" from "the slot". Wrapping every `Expr::New` there was tried twice
and is a net loss (it breaks
`exception_self_as_throwable_context_argument_uses_facade_boundary` and fixes
nothing else).

So the value side needs one thing the type side already has: a way to name the
**element position** of an optional slot as a place where a value can be
produced. The type side got this for free because `rust_type_for_slot_*` recurses
through `Option` before rendering. The value side has no such recursion — and
that, not the crossing, is the last structural gap.

Reverted to baseline. The stack is reproducible: `<scratchpad>/apply_0002b.py`
plus the generic-argument wrap and the `call_lowering.rs` path fix gives 530/154
before any expectation edits.

## Update 2026-08-06 (Codex follow-up) — unknown-boundary escapes also include field arguments

The `UnknownBoundary` producer consumed local bindings passed to unresolved
calls, but ignored the parallel `FieldUse::PassedToCall` facts that usage
already records. A field value crossing an unmodelled call can be retained by
that callee just like a local value, so the escape input was still biased low
for `helper.consume(this.cached)` / `helper.consume(cached)`.

`collect_unknown_boundary_escapes` now walks unknown-boundary field arguments
and attributes the escape to the field's declared storage type, using the same
array/generic-aware helper as field storage. This means `Cell[] cached` and
`List<Cell> cached` passed to an unknown call mark `Cell` as
`UnknownBoundary`, not merely the owner type.

Pinned by storage-strategy tests for a direct field argument and a generic field
argument.

## Update 2026-08-06 (Codex follow-up) — constructor mutation exemption was too broad

The storage plan correctly stopped counting direct constructor writes to
`this.field` as post-sharing mutation, but the exemption was too wide: it also
skipped constructor writes through local variables and parameters. A constructor
body can mutate a pre-existing object passed as a parameter:

```java
Owner(Cell cell) {
    cell.v = 2;
}
```

That is not initialization of the newly constructed owner. The object may
already be shared, so `Cell` must still be considered mutable after sharing.

The fact walk now carries a local type environment from parameters and variable
declarations. Constructor field writes are exempt only when the receiver is
`this`; field writes through a typed local or parameter attribute mutation to
that receiver type. Pinned by
`constructor_write_through_parameter_still_makes_that_type_mutable`.

## Update 2026-08-06 (Codex follow-up) — records, loop locals, and wildcard fields are in the storage input

Three more under-approximations were still in the report-only storage plan.

Records are Java reference objects, but `collect_declared_types` admitted only
`TypeKind::Class`. Records now participate in the same lattice as classes; enums
and interfaces remain excluded for the reasons above.

The statement walk treated `for` like a plain block and skipped initializer
declarations, so `for (Cell b = a; ...) { b.v = 2; }` missed both the local alias
and the receiver type for the mutation. `for` initializers and enhanced-for
locals now seed a scoped local environment before walking the body.

Finally, stored generic contents now recurse through wildcard bounds:
`List<? extends Cell>` and `List<? super Cell>` can retain `Cell` values just as
`List<Cell>` can. Static reachability and field-storage tests cover both bound
directions.

## Update 2026-08-06 (fourth pass) — re-run against the current tree

Re-applied the stack on top of the committed tree. Two things had changed: the
constructor-owner-path bug is **already fixed** (`constructor_owner_path_type` in
`src/call_lowering.rs` now builds the path from the type *name*, which is
stronger than stripping decoration from a rendered slot type), and the shifted
tree moves the numbers.

Progression this pass, against the committed 538/146:

| step | result | regressions |
|---|---:|---:|
| stack re-applied | 523/161 | 15 |
| user-generic arguments left unwrapped | 526/158 | 12 |
| optional element handle on the construction plan | 526/158 | 12, but only **1** still fails to compile |

### Correction: the generic-argument wrap was too broad

The previous pass recommended wrapping *every* generic argument, on the grounds
that an argument is always owned by its container. Owned, yes — but that is not
sufficient:

```
error[E0599]: the method `repair` exists for reference `&Rc<Standards<Rc<FormatLine>>>`,
              but its trait bounds were not satisfied
```

`Standards<T extends Named>` instantiated with `Rc<FormatLine>` requires
`Rc<FormatLine>: Named`, which does not hold — the impl is on `FormatLine`. So
the wrap is safe for **collection elements** (containers we generate, no bounds)
and unsound for **user generic type parameters** (bounds we do not control).
Scoping it to the collection arms fixed three tests.

### The optional element position, solved

The element position did not need a new concept: `OptionalSlotConstructionPlan`
already models it. Adding `element_handle: Option<StoragePlacement>`, populated
where the plan is *built* (which has a `FactSet`) and applied where it is
rendered (which does not), produces the shape the handoff asked for:

```rust
__java_self.wrapper = Some(std::rc::Rc::new(Wrapper::new(..)));
```

The wrap is gated on an explicit `value_is_fresh_construction` flag threaded from
the `Expr::New` test, not inferred — inferring it is the unanswerable question.
Keeping the handle decision *on the plan* is also what stops
`Some(Rc::new(Some(..)))`: the `Some` and the handle are now one decision instead
of two that can disagree.

#### Follow-up 2026-08-26 — optional element fixture actually closed

Codex re-ran
`constructor_nested_arg_reads_assigned_field_without_cloning_self` and found the
fixture still failing. The generated constructor assignment had two residual
problems:

```rust
Some(std::rc::Rc::new(std::rc::Rc::<Wrapper>::new(Holder::new(..))))
```

The outer duplicate wrap came from treating `Rc::<T>::new(..)` as different from
`Rc::new(..)` in `storage_placement_construction_expr_is_wrapped`. The
constructor call itself came from rendering a constructor owner path through the
storage-decorated slot type, so a path position used `Rc::<Wrapper>::new` rather
than `Wrapper::new`.

Both are now fixed:

- storage wrapper detection recognizes generic path calls such as
  `std::rc::Rc::<Wrapper>::new(..)`;
- constructor owner paths are rendered from the source Java owner name, not the
  storage-decorated slot type.

The pinned idiom fixture now emits:

```rust
__java_self.wrapper = Some(std::rc::Rc::new(Wrapper::new(Holder::new(..))));
```

### The one remaining break is a design fork, not a missing fix

```rust
repair(*line.downcast::<std::rc::Rc<FormatLine>>().expect("invalid Java cast"))
//     expected `FormatLine`, found `Rc<FormatLine>`
```

Two rules that are each individually right now conflict:

- the erased carrier holds `Rc<T>`, so a downcast recovers *the same object*
  rather than a copy (this is what makes 0028 identity observable through
  erasure);
- user generic parameters stay bare, so `T extends Named` bounds still hold.

A value crossing from the first into the second needs an unwrap, and nothing can
currently decide that: the downcast result's shape does not record that it is a
handle. Reverting the erasure pair instead costs two other tests (13 vs 12), so
it is not the cheaper side.

**The fork, to be decided before the next attempt:**

1. *Wrap user generics too*, and generate blanket forwarding impls
   (`impl<T: Named> Named for Rc<T>`) so bounds are satisfied. Uniform, but the
   impls have to be generated for every source interface a type parameter is
   bounded by.
2. *Store bare values in the erased carrier*, accepting that identity is lost
   through erasure. Uniform and much smaller, but it gives up part of 0028 —
   the part `known-gaps/Identity.java` does not cover, since that fixture never
   erases.

Option 2 is the smaller move and keeps `SharedImmutable` landable; option 1 is
the one that finishes 0028 properly. This is a decision about scope, not a
technical unknown.

Reverted to the committed baseline; the gate is green on all 18 targets.

## Update 2026-08-06 (fifth pass) — the fork is decided, by evidence

The previous section left a fork: wrap user generics too (option 1), or store
bare values in the erased carrier (option 2). I recommended option 2 as the
smaller move and then implemented it. **It does not work, and the reason
generalises.**

Option 2 was built as one rule replacing the earlier erasure pair: unwrap the
handle at the erasure boundary, where both sides are visible. That much is fine
and fixed `erased_object_set_uses_hash_free_vec_carrier`. Two breaks remained,
and they are the same defect from two directions:

```rust
// E0614: the erasure unwrap fired on a value that was not a handle
line = (*self.formats.as_ref().unwrap().repair(..)).clone().into();
//     `FormatLine` cannot be dereferenced

// E0308: the downcast produced a bare value into a handle-typed slot
let that: std::rc::Rc<FormatLine> = *o.downcast::<FormatLine>().expect(..);
//        expected `Rc<FormatLine>`, found `FormatLine`
```

`repair` is declared on `Standards<T extends Named>`, so it returns a **bare**
`T` — because option 2 leaves user generic arguments unwrapped. So:

> Under option 2, a value whose Java type is shared may or may not be behind a
> handle, depending on **where it came from**. Under option 1 it always is.

That is the whole difference. Every wrap/unwrap decision in this design is
currently made from a `RustSlotShape`, which carries a type and an ownership and
no provenance. Option 1 keeps "shared type ⇒ behind a handle" a global
invariant, so those decisions stay derivable from the type. Option 2 makes them
depend on the producer, which is exactly the information the IR does not carry —
the same gap this file has hit from four other directions.

So the earlier framing was wrong. It is not "thorough versus small". It is:

- **option 1** — wrap user generic arguments too, and generate forwarding impls
  (`impl<T: Named> Named for Rc<T>`) for each interface a type parameter is
  bounded by. Cost: those impls. Benefit: the invariant holds, and every wrap
  site stays a pure function of the type.
- **option 2** — cheaper per site, but it removes the invariant, and then no
  amount of local cleverness makes the boundary cases decidable. Both failures
  above are unfixable without provenance.

**Recommendation: option 1.** The forwarding impls are mechanical and generated
from information already collected (the bounds are in the source), and they are
much less work than adding provenance to the IR.

Measured state at the end of this pass, against the committed 538/146: 525/159,
13 regressions, 11 of them assert-only stale expectations and 2 the breaks above.
Reverted; the gate is green on all 18 targets.

### Option 1's sharp edge, found while scoping it

The forwarding impls go where `dyn_trait_anonymous_impl` and
`boxed_dyn_trait_clone_impl` already go -- appended to each generated trait in
`src/representation_lowering.rs` (`trait_item_for_type`,
`trait_item_for_type_with_program`). That part is mechanical.

The edge is the receiver:

```rust
impl<T: Named> Named for std::rc::Rc<T> {
    fn id(&self) -> String { (**self).id() }        // fine
    fn set(&mut self, v: i32) { (**self).set(v) }   // impossible: Rc has no DerefMut
}
```

A trait with **any** `&mut self` method cannot have a forwarding impl for
`Rc<T>`. So option 1 is not purely additive: it feeds back into the tier
predicate. A type whose interface has a mutating method cannot be a wrapped
generic argument, which means `is_implemented()` -- currently
`!interior_mutable` -- has a second condition:

> a shared type must also have a forwardable interface surface, i.e. every trait
> it is used through must be `&self`-only.

That is checkable from the same information the placement analysis already reads,
and it is better found now than after the impls are generated. It also explains
why option 1 is not merely "more work": it narrows which types can take the
`SharedImmutable` tier, and the narrowing must be computed *before* the wrap is
applied, not discovered as a compile error afterwards.

Concrete order for whoever picks this up:

1. Extend the placement analysis with the forwardable-surface condition and
   report the new `SharedImmutable` census (it will be smaller than 19).
2. Generate the forwarding impls for the traits that qualify.
3. Re-wrap user generic arguments and restore the handle-carrying erasure pair.
4. Update the 11 stale expectations and record the baseline.

### Step 1 of option 1 is done and landed

`interface_surface_is_handle_forwardable` (`src/storage_strategy.rs`) computes
the new precondition, and `report_storage_disagreements` emits
`storage_strategy_surface_not_forwardable` for any shared type that fails it.
Output-inert: it adds a warning and changes no generated Rust. `--lib` 166 -> 169,
every other suite unchanged, `idiom_gaps` still 538/146 with identical failure
names.

The check earns its place by catching a case that cannot be read off
`TypeStorageFacts`:

```java
interface Shape { int size(); void grow(); }
class Fixed   implements Shape { /* never mutates */ }
class Growing implements Shape { public void grow() { s = s + 1; } }
```

`Fixed` is `SharedImmutable` -- `mutated=false`, `mutation_provably_absent=true`
-- and still cannot take the tier, because `Growing` forces `Shape::grow` to
`&mut self` and `Rc<T>` has no `DerefMut`. **The blocker is the trait's surface
across all implementors, not the candidate's own mutability.** Pinned by
`a_mutating_sibling_implementor_blocks_the_whole_trait`.

This also means the `SharedImmutable` census is a *join over implementors*, not a
per-type property, so the earlier "19 SharedImmutable / 4 SharedLocal" split is
an upper bound. Anyone continuing should re-run the census on a real corpus
before generating impls -- the diagnostic now prints exactly which types drop
out and why.

Remaining, unchanged: steps 2-4 (generate the forwarding impls for traits that
qualify, re-wrap user generic arguments and restore the handle-carrying erasure
pair, update the 11 stale expectations and record the baseline).

### Step 2 landed: the forwarding impls exist

`shared_handle_forwarding_impl` (`src/representation_lowering.rs`) generates
`impl<__JavaShared: Tr + 'static> Tr for std::rc::Rc<__JavaShared>` beside every
generated trait, forwarding each method as `(**self).m(args)`. Wired into all
**three** trait emission paths -- `trait_item_for_type`,
`trait_item_for_type_with_program`, and `source_class_bound_trait_item_for_type`.
Gate-clean: `idiom_gaps` unchanged at 538/146, one exact-output expectation
updated, `--lib` 169 -> 174.

Three things this got wrong first, each now pinned by a test:

- **`T` collides.** A Java interface method may declare its own type parameter
  (`<T> T id(T value)`), and an inner generic shadowing the impl's is E0403.
  Hence `__JavaShared`.
- **An empty trait still needs the impl.** A class used as a bare bound
  (`<T extends HeaderLine>` where `HeaderLine` declares no methods) generates an
  empty trait; returning `None` for "no methods to forward" left
  `Rc<FormatLine>: HeaderLineBound` unsatisfied. An empty trait is *trivially*
  forwardable.
- **Class bounds are a third emission path.** Wiring only the two interface
  paths missed `HeaderLineBound` entirely, which is the trait a
  `<T extends SomeClass>` bound actually lowers to.

The forwardability precondition from step 1 was also extended to walk the
`extends` chain, since a class-bound trait is built from the whole hierarchy's
methods, and `TypeStorageFacts` gained `surface_forwardable` so a non-forwardable
type falls back through the existing `is_implemented()` path rather than
producing code that does not compile.

### Where step 3 stands

Re-applying the wrap on top of steps 1-2 gives **523/161, 15 regressions**, down
from 16 before the empty-trait fix. Reverted, because it is not green. The
remaining set is the familiar one: ~11 stale expectations plus a handful of
conversion sites, and it is now unblocked by the trait machinery rather than by
the fork, which is closed.

### Step 2 completed: both handles

The forwarding impl is emitted for `Rc` **and** `Arc`. The tier picks `Arc` for
anything reachable from a Java `static` (which must be `Sync`) and `Rc`
otherwise, so a bound like `Standards<T extends Named>` has to be satisfiable by
either; emitting only the `Rc` form left `Arc<NamedLine>: Named` unsatisfied.

### Step 3: 525/159, two conversion sites left

With steps 1-2 in place the wrap now costs **13** rather than the 16 it cost
before the trait machinery existed: **11 stale expectations and 2 compile
breaks**. The progression, each step fixing a distinct site:

| after | regressions |
|---|---:|
| wrap re-applied on steps 1-2 | 15 |
| empty-trait forwarding impl | 15 -> 14 |
| optional element handle on the plan | 14 |
| `Arc` forwarding impl | 14 -> 13 |
| rewrap on clone-recovery from a borrow | 13 |

The two survivors are the same shape, and it is the one this file has been
circling from the start:

```rust
let repaired: HeaderLine = self.formats.as_ref().unwrap()
    .repair(line.clone()/* TODO */)   // expected `Rc<FormatLine>`, found `FormatLine`
```

`line` is a **borrowed parameter** of a shared type. A borrowed slot deliberately
sees *through* the handle, so cloning out of one produces a bare value while the
callee's slot still wants the handle. `rewrap_clone_recovered_shared_value`
handles this for the argument path it was added to; these two reach the call
through a different one (a generic *receiver*), so the rewrap never runs.

That is the whole remaining shape of the work: **every place a freshly produced
value meets a handle-typed slot needs the same rewrap, and they must be found
one at a time** because `RustSlotShape` cannot say whether a value is already
behind a handle. Four such sites are now handled (slot default, static field
initializer, folded static read, optional element, clone-recovery-from-borrow);
these two are not.

Reverted step 3 again; steps 1 and 2 are in the tree and green.

### Why step 3 keeps not landing, stated once and for all

Every attempt this session ended the same way, and the count kept falling
without reaching zero: 16 -> 15 -> 14 -> 13. Each fix was correct and each
revealed another site. The sites handled so far all share one description --
*a freshly produced value meets a handle-typed slot* -- and they are:

| site | where the fix lives |
|---|---|
| slot default | `default_expr_for_slot_with_effective_facts` |
| static field initializer | `static_storage_accessors_for_field` |
| folded static read in return position | `lower_return_stmt_with_refinements` |
| optional element | `OptionalSlotConstructionPlan::element_handle` |
| clone recovery from a borrow (argument path) | `rewrap_clone_recovered_shared_value` |

A central attempt was made and is worth recording as a negative: putting
`shared_handle_recovery` on `ShapeCoercionPlan`, so `coercions()` re-wraps after
*any* borrow-to-owned recovery, does **not** catch the two survivors. They reach
the callee through a generic *receiver* path that does not build a
`ShapeCoercionPlan` for the argument at all. So there is no single choke point;
the recovery paths are genuinely plural.

The two survivors:

```rust
let repaired: HeaderLine = self.formats.as_ref().unwrap().repair(line.clone());
//                                                               ^ bare FormatLine, slot wants Rc<FormatLine>
```

`line` is a **borrowed parameter**. A borrowed slot sees through the handle by
design -- that rule is what keeps `&T` from becoming `&Rc<T>` everywhere -- so
cloning out of one necessarily yields a bare value. The wrap must then be
re-applied wherever that clone is consumed, and "wherever" is the open set.

This is the same root the whole file keeps returning to: `RustSlotShape` records
a type and an ownership, not whether a value is behind a handle, so the question
"does this expression need the handle put back?" cannot be answered once. It has
to be answered at each consumer, and the consumers must be enumerated by running
the suite.

That is tractable -- the list is finite and shrinking -- but it is site-by-site
work, not another structural move. Anyone continuing should re-apply the stack
(`<scratchpad>/apply_0002b.py` plus the slot_render additions), read the failing
list, and add one rewrap per site until it is empty, then update the ~10 stale
expectations.

### Step 3, final measured state: one compile break left

Continuing site-by-site got the wrap from 16 regressions to **13 -- twelve stale
expectations and a single compile break**. The two sites added after the earlier
write-up:

- **Shaped-argument recovery** (`lower_args_with_plan`). The rewrap must be keyed
  on the **actual** type, not the target's. A generic method parameter is still
  `T` at the call site -- it is *not* substituted with the receiver's type
  argument -- so `storage_placement_for` on the target finds nothing. Keying on
  the actual is also correct rather than merely expedient: a concrete target of a
  shared type is wrapped, and a type-parameter target is instantiated with the
  wrapped form because generic arguments are wrapped. This closed
  `source_receiver_generic_result_upcasts_to_supertype_assignment_slot`.
- The earlier argument-path rewrap in `lower_source_subtype_argument*` remains,
  for the paths that do not reach `lower_args_with_plan`.

### The one that is left, and what it needs

```rust
line = self.formats.as_ref().unwrap()
    .repair(match line { HeaderLineValue::FormatLine(value) => value, _ => panic!(..) });
//                                                            ^ bare FormatLine, slot wants Rc<FormatLine>
```

`carrier_enum_item` (`src/representation_lowering.rs`) builds every closed-enum
and hierarchy-carrier variant as `Variant(Variant)` -- a **bare** payload. So
extracting from the carrier yields a bare value no matter what the consumer
wants, and no amount of wrapping at consumers fixes it in general.

The principled fix is to wrap the *payload type*, so the carrier holds
`FormatLine(Rc<FormatLine>)` and extraction yields the handle directly. That is
symmetric with what was already done for collection elements and generic
arguments, and it removes the site rather than patching it. It needs a `FactSet`
threaded into `carrier_enum_item`, which today takes only names, and matching
changes at the variant construction site (`coercion.rs`, `{enum}::{variant}(..)`)
so construction and extraction stay in agreement.

That is the last structural item. After it, what remains is updating twelve
expectations that assert 0002 symptoms, and recording the baseline.

### Step 3, best measured state: 12 assert-only, zero compile breaks in `idiom_gaps`

Continuing site-by-site reached **526/158 with every generated program in
`idiom_gaps` compiling**. Two more sites closed after the previous write-up:

- **Carrier payload type** (`carrier_enum_item`). A variant payload is a slot of
  its type like any other, so a shared type is stored behind its handle there
  too. Storing it bare made `match line { HeaderLineValue::FormatLine(v) => v }`
  hand back a bare value regardless of the consumer. Needs a `FactSet` threaded
  into `carrier_enum_item` and `source_hierarchy_carrier_item_for_type`.
- **Carrier construction** (`{enum_name}::{sub_name}(value)` in
  `source_supertype_conversion_impl_for_pair`, and
  `Coercion::WrapClosedEnum`). Construction and extraction must agree, so the
  `From<Sub> for Carrier` impl wraps the payload, and the coercion carries a
  `payload_handle` set only when the value arrives from a **borrowed** slot --
  an owned slot of a shared type already holds the handle.

### Why it still is not landable, and the honest limit

`slot_shape_design` then went 516 -> 514, and one of those is not an expectation
drift:

```rust
query: Option<std::rc::Rc<Cursor>>,                      // field: wrapped
let tmp: Option<Cursor> = self.query.as_ref().cloned();  // local: not
```

`slot_shape_design` does **not** run `cargo_check_generated_modules`, so this
reads as an assertion failure while being a type error. That is 0018's blind
spot again, and it means "zero compile breaks" was only ever true of the suite
that checks.

The count across this session went 16 -> 15 -> 14 -> 13 -> 12(+2 elsewhere).
Every step was a real fix; every step revealed another slot-rendering path that
does not go through the wrap. That is the honest characterisation: the wrap has
to be applied at *every* path that renders a slot type or produces a value into
one, and those paths are not enumerable by reading -- only by running the suites
and reading what breaks.

**Before the next attempt, do this first:** add `cargo_check_generated_modules`
to `slot_shape_design`'s translation tests, or at least to the ones that assert
on rendered types. Without it the suite cannot tell a stale expectation from a
type error, and the remaining work is exactly the job of telling those apart.

### The prerequisite is now landed

`cargo_check_generated_modules` lived only inside `tests/idiom_gaps.rs`, which is
why `slot_shape_design` -- 596 tests, many asserting on rendered types -- had
**zero** generated-crate checks (296 in `idiom_gaps`, 0 here). That is why the
`Option<Rc<Cursor>>` field against an `Option<Cursor>` local read as an assertion
failure rather than the type error it is.

The helper now lives in `tests/common/mod.rs` and is used by both suites, and
`optional_self_field_read_into_owned_local_borrows_then_clones_element` -- the
test that hid it -- checks its generated crate. Landed while green, so the guard
is in place *before* the next attempt at the wrap rather than after.

Extending the check to the rest of `slot_shape_design`'s translation tests is the
obvious follow-up and should be done incrementally: each one that goes red is a
pre-existing non-compiling output that nothing was measuring.

### A bug in the wrap itself, found by the new guard

The `cargo_check` added to
`optional_self_field_read_into_owned_local_borrows_then_clones_element` paid for
itself immediately. With the wrap on:

```rust
query: Option<std::rc::Rc<Cursor>>,   // field
let tmp: Option<Cursor> = ...;        // local -- disagrees
```

The cause was in the wrap, not the lowering. There are two slot-type renderers.
The `EffectiveFacts` one recurses through `Option` by re-entering *itself*, so
the element gets the handle. The representations-aware one had been changed to
recurse into the **unwrapped** inner function, so its optional elements never
did. Fixed by recursing through the wrapping entry point in both, and by having
`shared_handle_slot_rust_type` decline optional shapes -- the element is where
the handle belongs, and wrapping the outer as well gives `Rc<Option<T>>`.

Worth stating plainly: this defect was invisible to 596 `slot_shape_design`
tests and would have stayed invisible, because none of them compiled their
output. It is the first thing the new guard caught, and it argues for extending
`cargo_check` to the rest of that suite before the next attempt.

### A note for whoever extends the `cargo_check` coverage

Do not bulk-edit `tests/slot_shape_design.rs` by brace-matching to find test
bodies. The fixtures are Java in `r#"..."#` literals and are full of `{` and `}`,
so a depth counter walks straight past the end of a function. An attempt here
"found" one test out of ~155 and silently mangled its own scan. Match on the
literal `);\n}` that closes each test's final assertion, or do it by hand in
batches.

## Update 2026-08-06 — step 3 is landed

The shared handle is on. `scripts/gate.sh` is at baseline on all 18 targets and
`idiom_gaps` is 538/146 with **identical failure names** -- zero new failures,
zero silently-fixed ones.

```rust
query: Option<std::rc::Rc<Cursor>>,                              // field
let tmp: Option<std::rc::Rc<Cursor>> = self.query.as_ref().cloned();  // local
static CONFIG_INSTANCE: std::sync::Mutex<Option<std::sync::Arc<Config>>>;
pub fn get_instance() -> std::sync::Arc<Config>;                 // Arc: static-reachable
```

What made it land, after eight reverts, was the generated-crate checking added in
[0041](done/0041-slot-shape-design-had-no-generated-crate-checks.md). Every
earlier pass converged against assertion text and kept finding new unwrapped
paths; with 101 `slot_shape_design` tests and 323 `idiom_gaps` tests compiling
their output, each pass named a *type error* instead, and the list ran dry.

### The wrap, in one place

**Type side** -- two positions, and only two:

| position | rule |
|---|---|
| top level of a slot | ownership-aware: owned only, since a borrow sees through the handle |
| generic argument | unconditional: an argument is always owned by its container |

Both recurse through `Option` at the **element**, giving `Option<Rc<T>>`; the
outer wrap declines optional shapes so it cannot also produce `Rc<Option<T>>`.

**Value side** -- the handle goes on wherever a *fresh* value meets a handled
slot. "Fresh" is never guessed; each site has a structural reason:

| site | why the value is known to be bare |
|---|---|
| slot default | nothing has run yet |
| static field initializer | a field initializer is a construction |
| return of a folded static read | restricted to a bare `T::new(..)` |
| local declaration initializer | same restriction |
| optional element (`OptionalSlotConstructionPlan::element_handle`) | decided *with* the `Some`, so the two cannot disagree |
| argument recovered out of a borrow | a borrow saw through the handle |
| subtype upcast (`.into()`) | an upcast returns a fresh supertype value |
| carrier variant payload | construction and extraction share one placement |

### Four defects the landing exposed, all fixed

- **`Rc<T>` nested in a collection cost its struct the `Clone` derive.** The
  top-level arm of `rust_type_supports_struct_derive_clone` knew a handle is
  `Clone`; the element arm did not.
- **A type parameter sharing a name with a source class got wrapped**
  (`input: Rc<ELEM>`). `JavaType` has no type-variable variant and slot rendering
  has no type-parameter scope, so the plan now excludes any name bound as a type
  parameter anywhere -- conservative in the safe direction.
- **The two slot-type renderers disagreed on `Option` recursion**, so the
  representations-aware one never wrapped optional elements.
- **The return-slot wrap fired on values that were already handles**, giving
  `Arc<Arc<T>>`; restricted to a bare constructor call.

### Codex follow-up 2026-08-12 — constructor freshness guard asks structured calls first

The value-side guard that decides whether a value is fresh enough to wrap for a
handled slot still rendered the expression and parsed `Owner::new(..)` from the
result. That kept the guard dependent on value-position text even when lowering
had a structured `RustExpr::Call`.

`rust_expr_is_bare_constructor_call` now recognises structured call function
paths directly and keeps the old rendered parser only for raw expressions. This
does not broaden the fresh-value rule; it just makes the existing bare
constructor precondition structural when the IR already carries the call shape.

The shared path parser also handles generated turbofish constructor calls and
spacing before the call parenthesis, so `T::new_1::<U>(...)` is still recognised
as the same fresh bare constructor as `T::new_1(...)`.

### Codex follow-up 2026-08-12 — shared handle construction preserves call structure

`slot_render::shared_handle_construction_expr` still wrapped fresh defaults and
fresh local initializers by rendering the inner value and returning one raw
`Rc::new(...)` or `Arc::new(...)` string.

For the implemented `SharedImmutable` tier, the helper now returns a structured
`RustExpr::Call` with the original expression as its argument. The raw
`wrap_construction` fallback remains for future cell placements, where
`RefCell::new`/`Mutex::new` nesting is not implemented yet.

### Codex follow-up 2026-08-12 — optional element handle construction reuses the structured wrapper

The optional-construction lowering path still had its own
`placement.wrap_construction(emit_expr(value))` calls for fresh elements meeting
handled optional slots. Those are the same value-side handle wrap as defaults and
fresh local initializers, but they flattened `Some(Rc::new(value))` through a raw
string before the `Some` plan saw it.

Both optional-construction entry points now use the shared structured placement
helper, so implemented `SharedImmutable` elements keep the nested
`Some(Rc::new(value))` call shape. The raw fallback remains centralized for the
unimplemented cell placements.

### Codex follow-up 2026-08-12 — subtype/erasure fresh-constructor wraps use the shared helper

The source-subtype upcast and erased-object slot paths still duplicated the
fresh-constructor handle wrap with direct
`placement.wrap_construction(emit_expr(...))` calls. Those branches are the same
value-side SharedImmutable construction rule as defaults, locals, and optional
elements.

The fresh-constructor arms now delegate to the shared placement helper too. The
outer erased-object boxing remains raw because it still composes a cast
expression, but the handle construction inside it uses the centralized
implemented-placement path.

### Codex follow-up 2026-08-12 — recovered shared arguments use the shared wrapper

`rewrap_recovered_shared_argument` was the remaining argument-path rewrap named
above: after lowering recovered a bare value from a shared borrow, the callee's
owned generic slot still required the handle. That helper still rendered the
recovered value, checked the rendered wrapper prefix, and returned a raw
`placement.wrap_construction(...)` string.

It now delegates to the same placement helper as defaults, optional elements,
and subtype/erasure fresh-constructor wraps. The existing idiom-gaps fixture
`source_receiver_generic_result_upcasts_to_supertype_assignment_slot` continues
to pin the emitted `Rc::new(line.clone(...))` shape.

### Codex follow-up 2026-08-12 — borrowed subtype erasure uses the shared wrapper

The borrowed/mutable-borrow erased-object subtype branch still cloned the value,
rendered it, and passed the string through `placement.wrap_construction(...)`
before the outer `Box<dyn Any>` erasure. That is the same implemented-placement
construction rule as the owned fresh-constructor branch.

It now delegates to the centralized placement helper after building the cloned
payload expression. The outer erasure remains raw because it still composes the
`Box::new(...) as ...` cast, but the handle construction inside it stays on the
same structured path as other SharedImmutable value wraps.

### Codex follow-up 2026-08-12 — enum carrier conversions use the shared wrapper

Source-hierarchy enum conversions still had one generated-code duplicate of the
implemented-placement rule: `impl From<Sub> for SuperEnum` built the enum payload
with `placement.wrap_construction("value")` when the subtype was stored behind a
shared handle.

That conversion now renders the centralized placement helper applied to the
`value` expression. The surrounding `impl From` body is still raw generated Rust,
but the SharedImmutable construction spelling now has one owner.

### Codex follow-up 2026-08-12 — same-outer constructor storage uses the shared wrapper

The captured-inner constructor-body rewrite had the last non-central value-side
call to `placement.wrap_construction(...)`: after rewriting a same-outer sibling
constructor to pass `__outer.clone()`, it rendered the full constructor
expression and wrapped the string for storage.

It now passes the rewritten expression to the centralized placement helper. The
helper keeps implemented SharedImmutable handle construction structured and
retains the raw fallback for future cell placements.

### What this does *not* do

`SharedImmutable` only. `is_implemented()` still excludes any type whose mutation
cannot be proved absent, and now also any type whose trait surface is not
`&self`-forwardable. `SharedLocal`/`SharedSync` -- the `RefCell`/`Mutex` rungs --
remain off, and with them the reentrancy discipline this file describes.

## Update 2026-08-11 (Codex follow-up) — stale gap diagnostics removed for implemented identity

Two diagnostics were still describing landed `SharedImmutable` cases as if they
were open gaps.

`storage_strategy_disagreement` now fires only for shared placements that are not
implemented. An `Rc`/`Arc` placement is no longer "lowered by value"; the
surface-forwardability warning remains separate and still reports the cases that
block handle forwarding.

The reference-identity diagnostic pass now receives the same `FactSet` as
lowering. For source types whose storage placement is shared and implemented,
`==`, `!=`, and inherited `Object.equals` no longer report the old structural
equality warning; those are the cases 0028 lowers with `Rc::ptr_eq`/`Arc::ptr_eq`.
String identity and unimplemented reference-identity cases still warn. The
regression is `implemented_shared_identity_equality_has_no_gap_diagnostics`,
using `tests/differential/Identity.java`.
