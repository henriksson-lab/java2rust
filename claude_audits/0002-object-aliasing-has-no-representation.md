# 0002 — Java object aliasing has no representation; aliases become clones

Status: **open, materially advanced** (`SharedImmutable`/`Rc<T>`,
reference-identity lowering, and the first single-threaded
`SharedLocal`/`Rc<RefCell<T>>` slices have landed)
Severity: high — the original mutable-alias oracle is now promoted and passing,
including direct locals, field/array storage, named callback parameters, stored
actions, and the narrow zero-argument anonymous functional-interface action.
The remaining mutable-sharing work is broader anonymous object lowering,
shared receivers/ordinary inner construction, and the `SharedSync` tier.
Kind: missing representation in the object model

Codex follow-up 2026-09-17 synchronized-erasure refusal: Java
`synchronized (monitor) { ... }` statements and `synchronized` methods are no
longer lowered as ordinary blocks/methods. Both forms now emit an explicit
unsupported diagnostic and generated `compile_error!` until the reentrant
SharedSync monitor representation is implemented. Guards:
`idiom_gaps::{synchronized_statement_is_loud_until_monitor_semantics_exist,
synchronized_method_is_loud_until_monitor_semantics_exist}`. The same pass
also refuses `volatile` fields until Java atomic memory-order semantics are
implemented; guard:
`idiom_gaps::volatile_field_is_loud_until_atomic_memory_order_semantics_exist`.

Codex follow-up 2026-09-15 direct SharedLocal receiver companion pass:
implemented mutable receiver companions now get emitted even when the ordinary
lowered body is textually identical, because call sites with only an
`Rc<RefCell<T>>` handle still need a legal entry point. The identical-body
case delegates through one `borrow_mut().method(...)` call, avoiding the
previous `RefCell already borrowed` runtime panic from repeated
`__java_self.borrow_mut()` projections inside the generated companion body.
Guards:
`differential::aliasing_shared_local_fixtures_match_java_output`,
`differential::qualified_inner_capture_fixtures_match_java_output`, and the
adjacent `differential::super_call_fixtures_match_java_output` regression.

## Symptom

Java reference assignment that requires mutable sharing originally had no
representation as one object with two live mutable handles. The historical repro
below was first translated as a deep copy (`a.clone()`), so subsequent mutation
through one reference was not observed through the other. The direct mutable
`Node`/`Counter` shapes are now represented with `SharedLocal`
(`Rc<RefCell<T>>`) in the promoted `tests/differential/Aliasing.java` oracle.
The remaining defect is no longer this direct aliasing shape; it is the wider
surface where a shared receiver handle or general anonymous object lowering is
needed.

Follow-up cleanup 2026-09-12: the `aliasing` module is crate-internal now. The
translator still reports observable mutable-alias diagnostics through the batch
analysis path, but the detection structs/helpers and diagnostic entry point are
`pub(crate)` implementation details rather than public crate API. The no-facts
convenience collector is test-only/private; production reporting uses the
fact-aware collector.

Codex follow-up cleanup 2026-09-12: `ObservableAlias` is no longer a
crate-local field bag. The detector constructs it through `new(...)`, the
reporter reads the diagnostic fields through accessors, and same-module tests
assert observed alias facts through the same API. This is a layout cleanup only;
mutable aliasing still requires the `SharedLocal` representation work above.

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

## Historical emitted Rust (verified when filed)

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

Clone recovery remains load-bearing, but the marker rendering is no longer a
21-site `lower.rs` mechanism; the audited clone-recovery spelling is centralized
in `coercion.rs`, with lowering calling through structured recovery helpers.

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

General `Rc<RefCell<T>>` and `Arc<Mutex<T>>` storage/render helpers now exist,
but mutable shared receiver and field lowering are still not implemented.
`__outer` remains the concrete inner-capture use rather than a placement the
planner can safely enable for mutable aliases.

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

## Historical proposed machinery

The concrete names below predate the current `StorageStrategy` /
`StoragePlacement` split. The live version of this work is tracked in 0002b:
`SharedImmutable` has landed, while mutable shared placements remain disabled
until the receiver/field borrow policy for `SharedLocal`/`SharedSync` exists.

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

Status at this checkpoint: **still open**. The mutable-alias semantics were
unchanged; what changed is that the divergence was now reported instead of
shipped silently. Later 0002b work landed `SharedImmutable` handles and
reference identity for the immutable tier, but this mutable-alias repro still
requires `SharedLocal`.

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

Reported as an `observable_alias` diagnostic from
`translate_parsed_source_with_context`.

**Precision measured on the corpus**: exactly **1** diagnostic across all 196 files
under `examples/`, on `examples/java/04-objects/AliasingSharedMutation.java` —
the one file written to exercise this. No other file produced the diagnostic, so the
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

Historical: `tests/differential/known-gaps/Aliasing.java` reproduced eight
mutable-alias cases against the Java oracle (`5|7|3|9|11|13|17|19`). The
`known_gap_fixtures_are_valid_java_and_match_expected_failure_mode` test verified
that this fixture was still live: Java had to print
`5|7|3|9|11|13|17|19`, and the translator had to report `observable_alias`
instead of silently translating mutable sharing without a `SharedLocal`
representation. That fixture has since moved to `tests/differential/Aliasing.java`
after the bounded SharedLocal slices below landed.


---

## Update 2026-08-03 — the size of the repair is now a number

Status at this checkpoint: still open — the semantics were unchanged and
`ObjectRepresentation` still had no shared case. What was new is that the
*scope* was measured rather than guessed. Later 0002b work consumes the storage
placement for implemented `SharedImmutable`; mutable `SharedLocal`/`SharedSync`
placements remain open.

`src/storage_strategy.rs` implements steps 1–2 of
[0002b](0002b-shared-representation-design.md): the decision rule runs, lowering
is untouched, and every type the plan says needs sharing while lowering holds it
by value is reported as `storage_strategy_disagreement`. In the original
2026-08-03 measurement over 248 files:

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

## Update 2026-09-07 — observable mutable aliases are translation errors

Until `SharedLocal`/`SharedSync` exist, a proven mutable alias still means the
generated Rust computes a known-wrong answer. `report_observable_aliases` now
emits `observable_alias` as `Severity::Error` instead of a warning, so the CLI
fails the translation rather than shipping the copied-value program. The
known-gap harness classifies `Aliasing.java` as `LoudDiagnostic` and checks that
the diagnostic code is an error.

## Update 2026-09-10 — known-gap oracle covered field storage too

Historical: `tests/differential/known-gaps/Aliasing.java` was expanded to
include the field-storage alias shapes already covered by `src/aliasing.rs`
unit tests: storing an alias through another object's field and through a
`this` field, alongside the block-action and stored-runnable callback cases.
The known-gap harness expected Java output `5|7|3|9|11|13|17|19` and the same
`observable_alias` error. This did not implement `SharedLocal`; it made the
then-remaining mutable-alias representation gap executable at the Java-oracle
level.

## Update 2026-09-10 — aliasing known gap was tied to the `SharedLocal` queue

Historical: the `Aliasing.java` known-gap row required more than the
error-level `observable_alias` diagnostic. It also required
`storage_strategy_disagreement` warnings naming `Aliasing.Node needs SharedLocal`
and `Aliasing.Counter needs SharedLocal`, so the core mutable-alias fixture
proved that the same source program landed in the 0002b mutable
shared-representation work queue.

## Update 2026-09-14 — first `SharedLocal` slice: direct local/storage aliases

`tests/differential/AliasingLocalField.java` is now a passing Java-vs-Rust
oracle for the smallest mutable-alias shapes:

```java
Node a = new Node();
Node b = a;
a.v = 5;
return b.v;

Counter a = new Counter();
Counter b = a;
a.bump();
return b.get();

Node[] cell = new Node[1];
cell[0] = a;
a.v = 9;
return cell[0].v;

holder.stored = a;
a.v = 11;
return holder.stored.v;

this.stored = node;
node.v = 13;
return this.stored.v;

StoredRunnable action = new StoredRunnable(a);
action.run();
return b.v;
```

That slice renders the aliased source type as `Rc<RefCell<T>>`, clones the
handle for the second local, writes fields through `borrow_mut()`, reads fields
through `borrow()`, and dispatches direct source receiver calls through
`borrow()`/`borrow_mut()` according to the solved method signature. The
optional field-read path also borrows through optional `Rc<RefCell<T>>`
receivers before projecting fields, so array-element and holder-field aliases
read the shared object rather than the handle. Optional SharedLocal assignment
targets also unwrap with `as_ref()` before `borrow_mut()`, so a named stored
action can mutate through an optional shared field without requiring `&mut self`.
The `observable_alias` diagnostic is suppressed only for these direct
local/direct field-or-receiver-call shapes when the aliased type has
implemented single-threaded `SharedLocal` placement.

At this checkpoint the broader `known-gaps/Aliasing.java` fixture still stopped
at `unsupported_anonymous_class`: the direct field and direct receiver-call
alias cases were represented, so the fixture reached the captured action
boundary. It identified the next unsupported shapes: captured anonymous actions
and callback methods whose parameter/trait-object carrier had to preserve the
shared source-object handle across the call. Inner-class outer construction was
also still blocked: `Rc<RefCell>` field storage does not imply lowering can
synthesize an `Rc<RefCell<Self>>` receiver handle from `&mut self`.

Follow-up 2026-09-14: named callback method parameters now preserve the
SharedLocal handle across the call boundary. `AliasingCallbackParam.java`
exercises `owner.apply(new NamedAction(a), b)`, where `NamedAction.run()`
mutates `a` and `Owner.apply(Action action, Node node)` reads `node.v` after
the callback. The emitted callee parameter and caller argument slot both use
the storage shape (`Rc<RefCell<Node>>`) rather than rejoining to `&Node`, so the
Java/Rust oracle now matches `17`. At that checkpoint the broad known-gap
fixture still stopped at the anonymous action object itself; implementing
anonymous class object lowering was separate from preserving named callback
parameter handles.

Follow-up 2026-09-14: the broad `Aliasing.java` fixture is promoted. The
remaining anonymous action shape is intentionally narrow: a zero-argument,
void-returning functional-interface implementation now lowers to a boxed
closure that clones captured locals before `move`, so the outer
`Rc<RefCell<Node>>` handle is preserved across `Action.run()`. The generated
trait surface has a matching closure adapter only for that single-method shape;
general anonymous class object lowering, anonymous bodies that use `this`, and
interfaces with parameters/returns remain separate work. The Java/Rust oracle
now covers the full original output:
`5|7|3|9|11|13|17|19`.

Codex follow-up 2026-09-14: the diagnostic-side support predicate now matches
the lowering-side boundary for that narrow anonymous action slice. A
zero-argument void anonymous functional-interface body that mentions `this` or
`super` is still outside the closure shortcut, so
`anonymous_class_diagnostics_with_plan` no longer treats it as supported and
continues to report `unsupported_anonymous_class`. Regression:
`representation::tests::anonymous_zero_arg_interface_body_using_this_still_reports_unsupported`.
The shared `this`/`super` body scanner now lives in `java_ir`, so representation
diagnostics and lowering use the same predicate instead of two copied walkers.

Codex follow-up 2026-09-14 later: the same diagnostic predicate now receives the
support-local anonymous-interface context that lowering uses. A source file can
implement a zero-argument void `Action` supplied by co-translated context while
capturing a SharedLocal handle; that narrow closure path lowers correctly and no
longer receives a false `unsupported_anonymous_class` diagnostic. Regression:
`fixtures::context_anonymous_action_with_sharedlocal_capture_is_not_reported_unsupported`.

Codex follow-up 2026-09-17 anonymous mutable-capture storage pass: the storage
lattice now descends into anonymous bodies and records writes through captured
outer handles. The context `Action` fixture therefore plans `Node` as
`Rc<RefCell<Node>>`, rather than `Rc<Node>`, and its closure mutation is both
compilable and observable through the alias (`"23"`). The same fixture is now
an execution guard, not only a generated-crate compile check.
