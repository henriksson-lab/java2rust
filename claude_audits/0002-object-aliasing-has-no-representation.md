# 0002 — Java object aliasing has no representation; aliases become clones

Status: **open, partially implemented** (`SharedImmutable`/`Rc<T>` plus
reference-identity lowering have landed; mutable aliasing still lacks
`SharedLocal`/`Rc<RefCell<T>>` lowering)
Severity: high — silently changes program meaning
Kind: missing representation in the object model

## Symptom

Java reference assignment that requires mutable sharing still cannot be
represented as one object with two live mutable handles. The historical repro
below was originally translated as a deep copy (`a.clone()`), so subsequent
mutation through one reference was not observed through the other. The
`SharedImmutable` tier now covers aliased source values that are not mutated
after sharing, but this mutable `Node` shape still requires the unimplemented
`SharedLocal` tier.

## Minimal repro

`claude_audits/repro/Shared.java`:

```java
public class Shared {
    static class Node { int v; Node next; }
    public int run() {
        Node a = new Node();
        Node b = a;
        a.v = 5;
        return b.v;
    }
}
```

Java result: `5`. (`a` and `b` are the same object.)

## Emitted Rust (verified)

```rust
pub struct SharedNode {          // note: no #[derive(Clone)]
    v: i32,
    next: Option<Box<SharedNode>>,
}

impl Shared {
    pub fn run(&self) -> i32 {
        let mut a: SharedNode = SharedNode::new();
        let b: SharedNode = a.clone()/* TODO(translation): validate added clone */;
        a.v = 5;
        return b.v;                  // would be 0, not 5
    }
}
```

rustc output:

```
error[E0599]: no method named `clone` found for struct `SharedNode` in the current scope
```

Two independent defects in one line:

1. **Semantic**: alias → copy. Even with `#[derive(Clone)]` added, `run()`
   returns `0` where Java returns `5`.
2. **Compile**: the clone-insertion path does not require or record that the
   cloned type is `Clone`, so no derive is added.

The `/* TODO(translation): validate added clone */` marker is emitted from 21
distinct sites in `src/lower.rs` — the fallback is load-bearing, not rare.

## Root cause

Historically, `ObjectRepresentation` (`src/object_model.rs:151`) could only
express value-like or trait-object representations:

```rust
pub enum ObjectRepresentation {
    ConcreteStruct,
    DynTrait { trait_name: String },
    JavaEnum { enum_name: String, variants: Vec<String> },
    ClosedEnum { enum_name: String, variants: Vec<String> },
    ErasedObject,
}
```

The current tree has a storage-strategy lattice and implements the immutable
handle tier (`SharedImmutable`, rendered as `Rc<T>` or `Arc<T>` depending on
reachability). It still does **not** implement the mutable shared tier for this
repro: no general `Rc<RefCell<T>>`/`Arc<Mutex<T>>` receiver and field lowering,
so Java's mutable reference semantics remain missing for aliased objects that
are written after sharing.

The only `Rc<RefCell<…>>` in the whole emitter is the `__outer` field for
inner-class captures (`src/item_assembly.rs:525`). It is a one-off, not a
representation the planner can choose.

For the mutable aliasing tier, every place that needs a second handle to a
mutable object must still either move it, copy it, or emit a loud unsupported
path. Clone insertion remains a pervasive compatibility mechanism for value
and recovery paths, which is why clone/borrow recovery is still one of the
largest rule clusters in `src/lower.rs` (see
[0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md)).

## Why this is a machinery gap

This cannot be fixed at a lowering site. Deciding between value, borrow, and
shared ownership is a whole-program property of each *allocation site and
type*: does any object of this type become reachable from two live places at
once, and is it mutated after that point?

That is an **escape / sharing analysis** producing a representation decision,
not a local rule. The old unused `EscapeLatticeValue` descriptor in
`src/lattice.rs` has since been deleted as part of 0062; the live domain is
`storage_strategy::Escape`, which is computed by the storage-strategy planner.

## Proposed machinery

1. Extend `ObjectRepresentation` with at least a shared case, e.g.
   `SharedMutable { cell: Rc<RefCell<_>> }` (and later `SharedImmutable(Rc<_>)`
   when the type is never mutated after sharing).
2. Add a sharing/aliasing analysis over the call graph (`src/usage.rs` already
   builds one) that classifies each type — or better, each allocation site —
   as:
   - *linear*: at most one live handle ⇒ `ConcreteStruct` by value (today's behaviour, then correct);
   - *shared read-only*: multiple handles, never mutated after sharing ⇒ `Rc<T>` or by-reference;
   - *shared mutable*: multiple handles with mutation ⇒ `Rc<RefCell<T>>`.
3. Make clone insertion require a *justification*: a clone is legal only when
   the analysis proves no aliasing is observable (value is dead afterwards, or
   the type is immutable). Otherwise the representation must change, or a
   diagnostic must be raised. Today an unjustified clone is emitted with a
   comment.
4. Independently and cheaply: whenever a `clone()` is emitted for a
   source-declared struct, record a `Clone` requirement so `#[derive(Clone)]`
   is added. This fixes the E0599 class on its own and is worth doing first as
   a separate, small change.

## Suggested split for implementers

- **0002a (small, mechanical)**: derive-`Clone` obligation tracking. Fixes the
  compile error, leaves the semantics wrong.
- **0002b (large, design)**: shared representation + aliasing analysis. Fixes
  the semantics.

Do not let 0002a close this issue — it makes the wrong output compile, which is
strictly worse for detection.

---

## Update 2026-08-03 — 0002a done, detection added, 0002b still open

Status: **still open**. The semantics are unchanged; what changed is that the
divergence is now reported instead of shipped silently.

### 0002a is done (and confirmed the warning above)

`SharedNode { v: i32, next: Option<Box<SharedNode>> }` was missing
`#[derive(Clone)]` because the derive-reachability walk handled
`Box<dyn Trait>` but not `Box<ConcreteType>` — and a recursive Java object graph
(`Node next`) always lowers to `Option<Box<Node>>`. Fixed by
`raw_rust_box_sized_inner` (`src/types.rs`), used from both raw-type arms of
`rust_type_supports_struct_derive_clone` (`src/item_assembly.rs`). The recursion
guard was already coinductive, so self-referential types work.

The repro now compiles cleanly **and prints `0` where Java prints `5`** — the
exact outcome this file predicted. That is why the next part was not optional.

### New: the divergence is detected

`src/aliasing.rs` decides, per alias point, whether the substituted copy is
observationally equal to Java's alias. It joins two facts that already existed
apart:

- *a second handle is created here* — `T b = a;` where `a` is still live, which
  is precisely the shape that makes lowering clone instead of move;
- *the other handle is still read after the write* — the backward liveness
  fixpoint from [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md),
  queried at the mutation site.

A write is only reported when both hold, so ordering falls out of the liveness
query rather than needing a separate CFG walk: a write that precedes the
declaration of `b` cannot have `b` live after it.

Mutation sites are field writes (`x.f = …`), element writes (`x[i] = …`), and
calls solved as writing through their receiver — `MethodUsageFacts.mutates_self`
in `src/usage.rs`, which is already a fixpoint over self-calls, so a method that
mutates only by delegating counts. This is a second consumer for the liveness
relation and the first consumer for `mutates_self` outside signature planning,
which is the wiring [0008](done/0008-inference-machinery-does-not-reach-lowering.md)
asks for.

Reported as an `observable_alias` warning from `translate_parsed_source_with_context`.

**Precision measured on the corpus**: exactly **1** warning across all 196 files
under `examples/`, on `examples/java/04-objects/AliasingSharedMutation.java` —
the one file written to exercise this. No other file produced a warning, so the
diagnostic is not noise.

Conservative in three places. The first is deliberate — a diagnostic that
over-fires gets ignored — the other two were scope limits worth lifting:

- an **unresolved call** answers "does not mutate" rather than "unknown", so an
  external mutator is a miss rather than a false alarm;
- originally, only **local declarations** counted as alias points, so
  `cell[0] = a` and `this.f = a` (an alias stored into an element or a field)
  were not detected;
- **mutations inside a block-bodied lambda** are not walked, since the lambda's
  statements are nested in an expression rather than reached by the statement
  walk. `LivenessAnalysis::for_loop_body` already exists for exactly this shape
  and is the way to lift it.

All three fail in the same direction: a missed report, never a false one.

### What is still missing

`ObjectRepresentation` still has no shared case, so nothing can be *repaired* —
the analysis can only name the defect. The two-part split above stands: 0002b is
the representation work, unchanged.

`tests/differential/known-gaps/Aliasing.java` reproduces four cases against the
Java oracle (`5|7|3|9` vs `0|0|0|0`). The
`known_gap_fixtures_are_valid_java_and_match_expected_failure_mode` test now
verifies that this fixture is still live: Java must print `5|7|3|9`, the current
translation must print `0|0|0|0`, and the translator must report
`observable_alias`. The original implementation detected three; the
array-element case is lifted below.


---

## Update 2026-08-03 — the size of the repair is now a number

Still open — the semantics are unchanged and `ObjectRepresentation` still has no
shared case. What is new is that the *scope* is measured rather than guessed.

`src/storage_strategy.rs` implements steps 1–2 of
[0002b](0002b-shared-representation-design.md): the decision rule runs, lowering
is untouched, and every type the plan says needs sharing while lowering holds it
by value is reported as `storage_strategy_disagreement`. Over 248 files:

| tier | distinct types |
|---|---|
| `SharedLocal` (`Rc<RefCell<T>>`) | 34 |
| `SharedImmutable` (`Rc<T>`) | 14 |
| `SharedSync` (`Arc<Mutex<T>>`) | 0 |

Against **5** `observable_alias` hits on the same corpus. The two numbers measure
different things and the gap is informative: `observable_alias` reports an alias
whose divergence it can prove at a specific write — a lower bound on the damage —
while the 47 count types whose representation is wrong, which is the size of the
change. Neither is the other.

The escape input behind that 47 is deliberately shallow and does not yet produce
`UnknownBoundary`, so 47 is a floor.

## Update 2026-08-11 (Codex follow-up) — block-lambda mutation detection lifted

One of the detection limits above is now narrower. `observable_alias` still
reports diagnostics rather than repairing representation, but block-bodied
lambdas are no longer invisible to the mutation half of the join. When a lambda
body is nested inside an expression statement such as
`values.forEach(x -> { a.v = 5; })`, aliasing now walks the lambda block and
attributes its writes to the enclosing statement for the liveness query.

This originally covered only the immediate-execution lambda shape where the
statement's live-out set is the right observation point. A later follow-up below
lifts the `Runnable r = () -> { a.v = 5; }; r.run();` case too.

## Update 2026-08-11 (Codex follow-up) — storage alias points are detected

Another detection limit above is lifted. `cell[0] = a` and `holder.f = a` now
record alias points when the assigned source local is an object type and is still
live after the assignment. The alias handle recorded for the diagnostic is the
storage receiver (`cell` or `holder`), which is the name liveness can observe
later at `cell[0].v` or `holder.f.v`.

Primitive storage assignments stay excluded: `int` copied into `int[]` or an
`int` field is copy semantics in Java too.

## Update 2026-08-11 (Codex follow-up) — this-field storage aliases are detected

The remaining storage alias shape above is lifted too. `this.f = a` now records
`this.f` as the alias handle, and later reads are detected structurally in
subsequent statements instead of asking local liveness for a `this` binding that
does not exist. Assignment targets such as a later `this.f = new Node()` are not
counted as reads of the previous value.

## Update 2026-08-11 (Codex follow-up) — external receiver mutators feed the diagnostic

The unresolved-call detection limit above is narrower too. `observable_alias`
previously recognised receiver-mutating calls only when the call resolved to a
source method and `UsageFacts` proved `mutates_self`. Fact-backed external APIs
already know the same thing through `ExternalApiEffectFact.receiver_mutability`,
but the alias detector did not receive `FactSet`, so `a.setFlag(...)` on an
external `Record` was treated as non-mutating even when the external pack marked
the method `mutates-receiver`.

Post-lowering alias diagnostics now pass `FactSet` into aliasing. The call
mutation scan still uses source `UsageFacts` for resolved source methods, and
falls back to external API effects using the receiver binding's Java type and
argument count. A read-only external fact remains non-mutating; the new
regression pins both sides.

This still does not repair representation.

## Update 2026-08-11 (Codex follow-up) — stored `Runnable.run()` callbacks are detected

The stored-callback caveat above is narrower. `observable_alias` now records the
captured mutation in a local lambda initializer without treating the declaration
as an execution site. When the stored lambda is later invoked through
`r.run()`, those captured writes are attributed to that call statement, so
`Runnable r = () -> { a.v = 5; }; r.run(); return b.v;` reports the same
observable alias as a direct field write.

The regression also pins the inverse: declaring the callback without invoking it
does not report a mutation.

This still does not repair representation.

## Update 2026-08-28 — current state after the SharedImmutable rollout

This audit no longer means "there is no shared representation at all."
`StorageStrategy::SharedImmutable` is implemented and the identity fixtures now
exercise `Rc::ptr_eq` for shared immutable source values. The still-open part is
the mutable Java aliasing case from this file: aliased values that are mutated
after sharing need `SharedLocal` (`Rc<RefCell<T>>`) or `SharedSync`
(`Arc<Mutex<T>>`) throughout slots, receivers, field access, assignment, and
method calls.

That distinction matters for handoff. A value-object alias may now be correct
through `Rc<T>`; the `Node b = a; a.v = 5; return b.v;` repro is not, because
the observable write requires a mutable shared carrier. The implementation
direction is therefore the remaining mutable-handle tiers described in
[0002b](0002b-shared-representation-design.md), not another clone-recovery
patch.
