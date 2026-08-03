# Next batch

> **Implementation status 2026-08-01.** Landed this round: **10a**, **10b**,
> **6b**, **6c**, 0018's coverage counters, and 0017's JDK bound projection.
> 0021 was found already closed. Remaining: 6a (see the note below — it turned
> out to be a no-op in behaviour, so it was replaced with a checked invariant),
> 0002, 0007, 0008, and the 0022 tree-based type parameters.
> No regressions at any step: fixtures held at 21 passed / 20 failed throughout.


Two parts. **Part 1** closes 0006 and 0010, the issues still live from the last
round. **Part 2** ("Extended plan") covers the ten issues requested for the
coming batch, with their state re-measured against the current tree.

Concrete plan for the two issues still live. Ordered by value per unit of risk.
Each item names the code, the shape of the change, the failure mode to watch,
and how to tell it worked.

Baseline to hold throughout: `cargo test --release --test fixtures` = **21
passed / 20 failed** (the 20 are pre-existing, from in-flight `Arrays` work),
build clean at 0 errors / 0 warnings.

---

## 0010 — two items, both small

### 10a. Catch-aware div/rem seeding (fixes the over-marking)

**Problem.** `guarded` below returns `Result` even though it handles the
exception, because the div/rem seed marks the *containing callable*
unconditionally:

```java
public int guarded(int a, int b) {
    try { return div(a, b); } catch (ArithmeticException e) { return -1; }
}
```

**Why the existing mechanism misses it.** `usage.rs` already downgrades a call
edge to `CallGraphEdgeKind::HandledExceptionBoundary`, and the fixpoint skips
those edges (`exception_plan.rs`). But the guard is
`callable_checked_exceptions_are_caught(...)` — **checked** exceptions only, and
`ArithmeticException` is unchecked. So the edge stays live and the seed is never
cleared.

**Change.** In `integer_div_rem_callables`, track enclosing catch types while
walking the body and skip a div/rem occurrence whose enclosing `try` handles
`ArithmeticException` (or `RuntimeException`/`Exception`/`Throwable`). The
walker already recurses through `Stmt::Try`; it needs to thread the handled set
the way `CallGraphBuildContext::handled_catch_types` does.

Consider generalising in the same pass: rename the predicate to cover unchecked
exceptions too, so any seeded throw — not just div/rem — is cleared by a local
handler. That is the more valuable version and is barely more work.

**Watch for.** A `catch` that rethrows, or that catches and then throws
something else, must stay fallible. That still works, because the rethrow seeds
fallibility on its own.

**Done when.** `guarded` is `-> i32`, `div` and `caller` stay `-> Result`.

### 10b. One store for fallibility

**Problem.** `CallableAnalysis::source_method_returns_result` ORs two
independently-computed answers:

```rust
self.callable_returns_result(&key)                        // the fixpoint
    || self.method_exception_plan(ty, method).wrap_returns_in_result   // a parallel computation
```

Two stores is the disagreement this issue is named for; centralising candidate
discovery did not remove it.

**Change.** Make the fixpoint the only writer. `plan_program_exceptions_with_fallibility`
already consults it — invert the remaining direction so
`MethodExceptionPlan::wrap_returns_in_result` is *derived from* the fallibility
plan rather than computed alongside it. Then `source_method_returns_result`
becomes a single lookup, and the declaration-based probe in
`source_method_call_returns_result_with_exception_plan` can be deleted.

**Watch for.** `plan_method_exceptions` reads `throws` clauses that the
call-graph keying does not carry. Fold those into the *seed*
(`seed_callable_fallibility`) before deleting the second store, or fallibility
declared only via `throws` will be lost.

**Done when.** `grep -c wrap_returns_in_result` shows one writer, and
`source_method_named_returns_result_with_exception_plan` is gone.

---

## 0006 — four items, the first is the one that matters

Current surface, for measuring progress:

| field on `LowerCtx` | refs in `lower.rs` |
|---|---|
| `later_read_names` | 31 |
| `later_mutated_names` | 13 |
| `preinitialized_try_capture_names` | 10 |
| `current_stmt_read_counts` | 9 |
| `catch_future_use_names` | 9 |
| `exclusive_terminal_branch_read_names` | 5 |

plus 12 `statement_suffix_*` fallback sites and 2 `live_out_for` (name-projecting)
call sites.

### 6a. Consumers on bindings, not names (the real remaining defect)

**Problem.** The analysis is binding-keyed internally, then
`live_out_for` projects back to `HashSet<String>` at the two block walkers, so
every consumer is still name-based. The projection is exact *today* — at most one
binding of a name is live at a point — but nothing enforces that, and the
precision is thrown away at the boundary.

**Change.** `MoveUseContext` is the choke point: nearly all ~17 predicates funnel
through it. Give it the statement's `HashSet<Binding>` plus the scope map already
computed by `LivenessAnalysis` (`scope_at`), so `has_later_read(name)` resolves
the name to a binding at that point and compares bindings. The predicate
signatures can stay `&str` — the resolution happens inside.

This is the highest-value item: it is concentrated in one struct, and it is what
makes the binding keying observable rather than internal.

**Watch for.** Field reads. `this.x` is already distinct, but a bare field read
resolves to `OUTER_DECL` — make sure that is treated as "not a local binding"
rather than colliding with a parameter of the same name.

### 6b. Retire the sets liveness already subsumes

Two of these look already-covered and should be *verified then deleted*, not
rewritten:

- `catch_future_use_names` — liveness's `Stmt::Try` arm unions catch bodies into
  the body's live-out, which is the same relation.
- `exclusive_terminal_branch_read_names` — liveness kills everything at
  `Stmt::Return`/`Throw`, so a terminating branch already contributes nothing.

Method: assert both the old set and the liveness answer, diff over a corpus run,
and delete once the divergence list is empty. Expect a short list; each entry is
either a bug being fixed or an approximation worth writing down.

`later_mutated_names` is a genuinely different relation (may-be-written-after).
Add it to `liveness.rs` as a second backward pass with the same structure — `def`
where the read pass uses `use` — rather than leaving a suffix scan.

`current_stmt_read_counts` is not a dataflow relation at all (within-statement
multiplicity). Keep the computation, but move it into the analysis as
`use_count_at(stmt, binding)` so there is one owner of "how is this binding
used here".

`preinitialized_try_capture_names` is about *initialization*, not the move
relation. Leave it, but rename it so it stops reading as a sixth approximation
of the same thing.

### 6c. Delete the suffix fallbacks

The fallback only fires for synthesized statements. Give each synthesizing site
its own analysis at the point of synthesis — exactly what
`LivenessAnalysis::for_loop_body` already does for inlined lambda bodies — then
delete `statement_suffix_read_names` / `statement_suffix_mutated_names` and the
`match … { Some(..) => …, None => … }` at both walkers.

**Done when.** `grep -c statement_suffix` is 0 and `LowerCtx` has one
analysis handle instead of six optional sets.

---

## Recommended order

1. **10a** — smallest, fixes a visible wrong signature, self-contained.
2. **6b (the two verify-and-delete items)** — cheap, and shrinks the surface
   before the harder change.
3. **10b** — needs the `throws` seed folded in first; do not delete the second
   store before that.
4. **6a** — the substantive one. Do it after 6b so there is less to convert.
5. **6c** — last; it is mechanical once nothing else reads the sets.

## What I would not do

- Do not convert the ~17 predicate signatures to take `Binding`. Resolving
  inside `MoveUseContext` gets the same correctness for a fraction of the churn.
- Do not delete `preinitialized_try_capture_names` as part of 0006. It is a
  different relation that happens to live next door.
- Do not chase AWT further under 0011. It is hand-modelled by design; the
  remaining probe error is the intended unmodelled-constant diagnostic.

---

# Extended plan — the ten open issues

Measured against the tree on 2026-08-01, not the state the issues were filed in.
Several have moved; two are further along than their docs say.

## Where things actually stand

| issue | measured now |
|---|---|
| 0002 object aliasing | `ObjectRepresentation` still has no shared **or subtype** case |
| 0006 move analysis | binding-keyed inside, name-projected out; 6 optional sets, 12 suffix sites |
| 0007 opaque strings | `RustExpr::Raw` **591 / 1004** (59%); 49 `emit_expr` round-trips; 56 syntax literals matched |
| 0008 machinery reach | `liveness` reaches `lower.rs` (7 refs); `lattice`/`fact_saturation`/`graph` still **0**; 220 bool predicates |
| 0009 overload identity | **largely fixed** — `f_int`/`f_int_int` suffixes, child override survives, probe compiles |
| 0010 fallibility | candidate discovery centralized; two stores remain; div/rem over-marks |
| 0017 generic bounds | `type_parameter` / `type_bound` still **0 refs** in `parser.rs` |
| 0018 metric blind | no coverage counters in `summary.tsv` |
| 0021 instanceof pattern | `type_pattern` still **0 refs** in `parser.rs` |
| 0022 types from text | **half done** — `parse_type_node` now dispatches on `node.kind()` |

Two corrections worth carrying forward:

- **0009 is nearly closed.** The Base/Child probe compiles and the child's own
  body survives. What still fails is passing a `Child` where a `Base` is
  expected (`E0308`) — that is **0002**, not overload identity. Re-scope 0009 to
  the naming inconsistency below and close the rest.
- **0008 is partly demonstrated.** `liveness` is a worked example of an analysis
  reaching lowering. The issue is now specifically that `lattice`,
  `fact_saturation` and `graph` are still unreachable, and 220 predicates still
  re-derive locally.

## Dependency order

```
0018 (measurement)  ──► everything else, because it is how you know a fix worked
0022 (types from tree) ──► 0017 (bounds need type_parameter/type_bound)
0002 (representation)  ──► 0009 residual (upcast), 0023 (inner classes), 0028 (identity ==)
0009 (identity)        ──► 0010 residual (which overload is the callee)
0007 (shape tags)      ──► 0006 consumer conversion, coercion cleanup
```

Do **0018 first**. Every other item below is judged by corpus behaviour, and the
current metric cannot see silent failures — which is the issue itself.

---

## Phase 1 — make results measurable (0018)

Cheapest work here, and it gates the value of everything after.

1. **Silent-drop prohibition.** Any construct the translator does not handle must
   appear as `compile_error!` rather than vanishing. Add a test that a
   deliberately unsupported construct produces a diagnostic.
2. **Coverage counters in `summary.tsv`**: `noop_facade_methods`,
   `noop_facade_call_sites`, `unmodelled_types`, `stubbed_constants`,
   `dropped_statements`, `parse_failures`. `count_stubbed_constants` already
   exists in `main.rs` — extend that pattern.
3. **Differential testing harness.** A dozen deterministic entry points run
   under both Java and the translated Rust, output diffed. This is the only
   technique that finds the silent class at all, and it is what makes 0002 and
   0007 checkable rather than reviewable.

**Done when** a corpus run reports coverage alongside the error count, and a
dropped construct fails a test.

## Phase 2 — small, independent, high certainty

### 0021 — `instanceof` pattern binding
`type_pattern` is unreferenced. Add it to the `instanceof_expression` handler,
extend `Expr::InstanceOf` with `binding: Option<String>`, lower to
`if let Some(s) = o.downcast_ref::<T>()`. Start with the common case; diagnose
the negated-scope form (`if (!(o instanceof T s)) return; … s …`) rather than
mis-scoping it.

### 0022 — finish types from the parse tree
`parse_type_node` already dispatches on `node.kind()`; the remaining fallthrough
is `_ => parse_type_text(...)`. Add `type_parameter`, `type_bound`, and the
record-pattern nodes, then keep `parse_type_text` **only** for genuinely
text-sourced types (external manifests, `javalibs` signatures).

**Done when** the grammar-coverage diff in `AUDIT-BACKLOG.md` shows the
type-level node kinds handled.

### 0009 — the naming inconsistency
`Base` emits `f_int` while `Child` emits `f` for the same Java method. That is
fine while calls are statically bound and breaks the moment dispatch is virtual.
Make the Rust name a function of the **resolved signature set for the whole
override group**, not per class. Cheap now, load-bearing once 0002 lands.

## Phase 3 — the representation (0002)

The largest single item, and three other issues are waiting on it.

1. Extend `ObjectRepresentation` with the two missing cases: a **shared** one
   (`Rc`/`Arc` per `CONCURRENCY-SCOPE.md` — settle the `Local`/`Shared` policy
   axis *before* implementing, not after) and a **subtype** one, so a `Child`
   value can flow into a `Base` slot.
2. Drive the choice from an escape/sharing analysis over the existing call graph
   (`usage.rs`). `EscapeLatticeValue` already exists in `lattice.rs` and is
   unused — this is its first real consumer, which also chips at 0008.
3. Make clone insertion require a justification: legal only when no aliasing is
   observable. Today an unjustified clone is emitted with a `TODO` comment.

**Sequencing.** Consider doing the *subtype* case first and shipping it: it
unblocks 0009's residual and is far smaller than the sharing analysis. Then the
`Local`/`Shared` split, which unblocks 0023 (inner classes) and 0028
(reference `==`).

**Watch for.** `Send`/`Sync` propagating through the whole object graph the first
time `Arc<Mutex<_>>` appears. Enable per-type behind the policy rather than
globally.

## Phase 4 — the IR (0007), then finish 0006

0007 is the enabler for 0006's consumer conversion, so do it in that order.

1. **Make the shape tag mandatory.** `RustExpr::Raw` cannot be constructed
   except through a helper that also supplies a `RustExprShape`
   (`Borrowed`, `MutBorrowed`, `OwnedRecovery`, `OptionalProjected`, `SliceView`,
   `StringView`, `ResultPropagated`). The escape hatch stays; it stops losing
   information. This is the single highest-leverage change in the file.
2. **Invert the round trips at the producer.** Each of the 56 `ends_with(…)`
   checks corresponds to a shape the producer already knew. Fix them there, not
   at the consumer. Prioritise the two worst: `contains(" + ")` parsing an
   operator, and the match against an emitted `/* TODO */` comment, which makes
   a comment load-bearing syntax.
3. **Add the missing structured variants.** Measure which constructs dominate
   the 591 `Raw` sites before designing — expect a small number of node kinds
   (closures, `?`, turbofish chains, `match`) to account for most of them.
4. **Ban regrowth with a test** that greps for `emit_expr` followed by a string
   predicate and fails on new occurrences.

Then finish 0006 as described in the first half of this document (6a–6c), which
gets easier once lowered expressions carry shapes.

## Phase 5 — 0008 as the closing argument

By this point `liveness` (0006), the escape lattice (0002) and shape tags (0007)
are all analyses reaching lowering. 0008 closes by:

1. Routing `lattice` into the **slot-shape subsumption** decision — the ~10
   "more generic information" predicates are a hand-rolled partial order; give
   `RustSlotShape` a real `subsumes`/`join` and delete them.
2. Moving the ~15 hardcoded library predicates (`is_java_get_class_name`,
   `java_type_is_string_type`, …) into `javalibs` manifests.
3. Tracking the predicate count as the metric: **220 today**. Expect the
   liveness, subsumption and library moves to take out 40–50 between them.

---

## Migration discipline (applies to every phase)

For each "replace N local rules with one solved relation" change: compute both
answers behind an assertion for one corpus run and diff. Every divergence is
either a bug being fixed or an undocumented approximation. With 220 predicates
in one file, an unattributed error-count change is indistinguishable from a
regression — and `docs/rule-generalization-convergence-plan.md` correctly warns
that the count will move.

## What I would not do

- Do not start 0017 before 0022. Bounds have no path into the IR until
  `type_parameter`/`type_bound` are parsed; anything else is a workaround.
- Do not attempt 0002's sharing analysis and the concurrency representation as
  one change. Settle the policy axis on paper, ship the subtype case, then the
  sharing case.
- Do not try to delete `RustExpr::Raw`. Making it carry a shape is achievable and
  gets the benefit; removing it is neither.
- Do not judge any of this by the compile-error count alone until Phase 1 lands.


---

# What landed (2026-08-01)

| item | result |
|---|---|
| **10a** catch-aware div/rem | `guarded`/`guardedRt` are `-> i32`; `div`/`caller`/`rethrows` stay `Result`. The existing `HandledExceptionBoundary` guard only considered *checked* exceptions, which is why it never fired for ArithmeticException. |
| **10b** one fallibility store | Declared `throws` facts are now seeded into the fixpoint under both the Rust and Java name, `MethodExceptionPlan` is derived from it, and `source_method_named_returns_result_with_exception_plan` is **deleted**. `source_method_returns_result` is a single lookup. |
| **6b** retire subsumed sets | `catch_future_use_names` and `exclusive_terminal_branch_read_names` deleted. **6 optional sets → 4.** |
| **6c** suffix fallbacks | The interface-default lowering path was the last site without an analysis; it now builds one. Instrumentation showed the fallback firing 0 times across fixtures and 40/41 repros before that fix, 0 after. |
| **0017** JDK bound projection | `T extends Comparable<T>` now emits `impl<T: Ord + Clone>`, and a gated `JavaComparable` support trait blanket-implements `compare_to` for every `T: Ord`. The `E0599 compare_to` is gone. |
| **0018** coverage counters | `noop_facade_methods`, `unimplemented_facade_methods`, `compile_error_markers` in `analysis-profile.tsv`. |
| **0021** | Already closed — basic, negated-scope and `&&`-chain forms all correct. |

## Two findings that changed the plan

**`catch_future_use_names` was not subsumed, and fixing that improved the
analysis.** Routing it to `has_later_read` regressed `TryMove`: a `return` inside
a `try` kills everything after it, but the exception edge can leave from *any*
statement in the guarded region and still reach the catch. Liveness modelled that
edge only at the Try entry. It now applies the catch's reads as a **live floor**
across the whole guarded region (`add_live_floor_to_block`), which genuinely
subsumes the old set. This is a correctness improvement that the verify-then-delete
discipline surfaced — deleting on the first hypothesis would have introduced a
silent move bug.

**6a delivers no behavioural change.** Only three sites query `later_read_names`
by name; the rest is plumbing. Because a declaration kills its name for
everything before it, at most one binding of a given name is live at any point —
so a binding-keyed query returns exactly what the name query returns. Converting
the consumers would be churn for nothing. Instead the invariant that makes the
projection lossless is now **checked**: `live_out_for` carries a `debug_assert`
that no two live bindings share a name. Verified at 0 violations across the probe
set in a debug build. If a future scoping change breaks it, that fires rather
than silently making name-keyed consumers wrong.

## 0017 residual, re-scoped

`GenericBox` is down to 2 errors, both type-parameter **erasure**, not bounds:
a *bounded* type parameter erases to `Box<dyn Any>` in argument position, while
an unbounded one stays `T`. Isolated cause: calling a method on a `T`-typed
receiver classifies the call as an unknown boundary, which erases the arguments.
That is the erasure policy from
[0011](0011-dyn-any-fallback-fails-at-every-use.md), not bound projection —
track it there.

## 0006 surface now

4 optional sets (was 6), 10 `statement_suffix` sites (was 12), fallback
unreachable in practice. Remaining per the plan: `later_mutated_names` as a
second backward pass in `liveness.rs`, and `current_stmt_read_counts` moved into
the analysis. `preinitialized_try_capture_names` stays — different relation.


---

# Remaining work, specified

The Phase 3–5 items were previously described only as directions. Below they are
specified to the same level as the items that landed: exact files, the shape of
the change, what to watch, and a done-when.

Ordered by tractability. Each is independently shippable.

## R1 — `later_mutated_names` as a liveness pass (finishes 0006's set retirement)

**Where.** `src/liveness.rs`, `src/lower.rs` (`statement_suffix_mutated_names`,
13 refs to `later_mutated_names`).

**Change.** Add a second backward pass with the same structure as the read pass:
`may_be_written_after(binding, point)`. Where the read pass gens on `use`, this
gens on `def` — assignment targets, compound assignment, `++`/`--`, and a
by-`&mut` argument. Loops iterate to the same fixpoint; the `try` live-floor
applies identically.

Expose `mutated_after_for(stmt) -> Option<HashSet<String>>` alongside
`live_out_for`, and feed `LowerCtx::later_mutated_names` from it exactly as
`later_read_names` is fed.

**Watch.** This is a *may* analysis too — over-approximating a write costs a
defensive copy, under-approximating produces aliasing bugs. Widen on anything
uncertain, same as the read pass.

**Done when.** `statement_suffix_mutated_names` is deleted and
`later_mutated_names` is fed only from the analysis.

## R2 — `current_stmt_read_counts` into the analysis (0006)

**Where.** `src/lower.rs` (`statement_read_counts`, 9 refs).

**Change.** Not a dataflow relation — within-statement multiplicity. Move the
computation into `LivenessAnalysis` as `use_count_at(stmt, name) -> usize`, so
one type owns "how is this binding used here". The consumer
(`has_repeated_current_stmt_read`) becomes a call on the analysis.

**Done when.** `LowerCtx` holds one analysis handle plus
`preinitialized_try_capture_names` (a genuinely different, initialization-time
relation that should be renamed, not merged).

## R3 — tree-based type parameters (finishes 0022)

**Where.** `src/parser.rs` — `parse_type_parameter_names`,
`parse_type_parameter_bounds`, `declared_type_parameters_text`.

**Change.** Both currently locate the `type_parameters` node and then re-parse
its **text** with `find('<')` / `matching_angle_close` / `split_top_level_commas`.
Walk the children instead: `type_parameter` nodes each carry an identifier and an
optional `type_bound`, and `type_bound` carries one or more types separated by
`&`. Keep `parse_type_text` only for genuinely text-sourced types.

**Watch.** Intersection bounds (`T extends A & B`) currently keep only the first
type; the tree gives all of them, so decide deliberately whether to emit all
bounds or keep taking the first.

**Done when.** `type_parameter` and `type_bound` appear in `parser.rs`, and the
grammar-coverage diff in `AUDIT-BACKLOG.md` no longer lists them.

## R4 — bounded type parameters stop erasing (0011 residual, unblocks 0017)

**Where.** `src/object_model.rs` (`dispatch_requirement_kinds_for_binding_use`),
`src/shape_projection.rs` (`dispatch_requirement_projection_policy`).

**Problem, isolated.** An *unbounded* `T` stays `T` in argument position; a
*bounded* `T` erases to `Box<dyn Any>`. Calling a method on a `T`-typed receiver
classifies the call as `CallArgumentBoundary::Unknown`, which yields
`NeedsUnknownRuntimeObject` and erases the arguments.

**Change.** A receiver whose Java type is a **declared type parameter of the
enclosing type** is not an unknown runtime object — its capability is exactly its
bound. Thread that through so such a call does not raise
`NeedsUnknownRuntimeObject`, and the argument keeps the parameter type.

**Done when.** `claude_audits/repro/GenericBox.java` compiles (currently 2
errors: `E0308` on the erased argument, `E0310` on the downcast lifetime).

## R5 — mandatory shape tags on `RustExpr::Raw` (0007)

**Where.** `src/rust_ir.rs`, `src/coercion.rs`, `src/lower.rs`.

Current: `RustExpr::Raw` is **591 of 1004** constructions; 49 `emit_expr`
round-trips; 56 distinct Rust-syntax literals matched.

**Change, in this order.**
1. Add `RustExprShape` (`Borrowed`, `MutBorrowed`, `OwnedRecovery`,
   `OptionalProjected`, `SliceView`, `StringView`, `ResultPropagated`, `Opaque`).
2. Make `Raw` constructible **only** through a helper taking a shape. Start with
   `Opaque` everywhere so the change is mechanical and behaviour-preserving.
3. Convert the round trips one cluster at a time, cheapest first: the
   `.clone()` / `.cloned()` / `.clone_box()` family, then `.as_ref()` /
   `.as_deref()` / `.as_mut()`, then the borrow prefix `starts_with('&')`.
   Each conversion is: producer sets the shape, consumer reads it, string check
   deleted.
4. Two to prioritise regardless of cluster order, because they are outright
   wrong rather than merely fragile: `contains(" + ")` parsing an operator by
   substring, and the match against an emitted `/* TODO(translation) */`
   comment, which makes comment text load-bearing syntax.
5. Add a test that greps for `emit_expr` followed by a string predicate and
   fails on new occurrences.

**Watch.** Do not attempt to remove `Raw`. The goal is that it stops losing
information, not that it disappears.

**Done when.** The 56-literal count is falling and no *decision* reads rendered
text — measured by the counts in `0007`.

## R6 — subtype representation (0002, first half)

**Where.** `src/object_model.rs` (`ObjectRepresentation`), `src/representation.rs`,
`src/signature.rs`.

**Change.** Add the missing **subtype** case so a `Child` value can flow into a
`Base` slot. This is separable from — and much smaller than — the shared-ownership
work, and it unblocks 0009's residual today:

```java
static class Child extends Base { }
public double viaBase(Base b) { … }
public double run() { return viaBase(new Child()); }   // E0308 today
```

**Watch.** Do not fold in `Rc`/`Arc`. Settle the `Local`/`Shared` policy axis on
paper first (`CONCURRENCY-SCOPE.md`), then do the sharing case as R8.

**Done when.** The probe above compiles, and
`claude_audits/repro/Ovl2.java` goes from 2 errors to 0.

## R7 — lattice into slot-shape subsumption (0008)

**Where.** `src/lattice.rs`, `src/slot_shape.rs`, `src/lower.rs`.

**Change.** ~10 predicates hand-roll a partial order over shapes
(`shape_has_more_generic_information`, `lowered_type_slot_has_more_generic_information`,
`initializer_shape_is_more_specific_for_local`, `cast_target_erases_slot_generic_detail`,
`java_types_have_same_erased_identity`, …). Give `RustSlotShape` a real
`subsumes(&self, other) -> bool` and `join(a, b)`, implemented against the
existing `OWNERSHIP_VALUES` / `VIEW_VALUES` / `DispatchLatticeValue` domains, and
retire the predicates onto it.

Second, mechanical part: move the ~15 hardcoded library predicates
(`is_java_get_class_name`, `java_type_is_string_type`,
`current_type_extends_jdk_input_stream_facade`, …) into `javalibs` manifests.

**Done when.** `lattice` is referenced from `lower.rs` (currently **0**), and the
bool-predicate count in `lower.rs` falls from **220**.

## R8 — shared representation (0002, second half)

Only after R6 and the policy decision. Extend `ObjectRepresentation` with
`Local` (`Rc`) and `Shared` (`Arc`) cases driven by an escape/sharing analysis
over the existing call graph; `EscapeLatticeValue` in `lattice.rs` is its first
real consumer. Unblocks 0023 (inner-class construction panics) and 0028
(reference `==`).

**Watch.** `Send`/`Sync` propagates through the whole object graph the first time
`Arc<Mutex<_>>` appears — enable per type behind the policy, never globally.

---

## Discipline for all of the above

- Hold the fixtures baseline (**21 passed / 20 failed**) after every step, and
  diff the failure *names*, not just the count.
- For each "N rules → one relation" change, compute both answers behind an
  assertion for one corpus run and diff before deleting anything. R1 and R7 are
  both of this shape. The `catch_future_use_names` episode above is why: the
  first hypothesis was wrong and the diff is what caught it.
- Verify a hypothesis before acting on it. Instrumenting whether the suffix
  fallback was reachable took two minutes and turned 6c from a guess into a
  measurement.


---

# Round 2 results (2026-08-01)

Fixtures held at **21 passed / 20 failed** after every step; build clean at 0
errors / 0 warnings throughout. Nothing committed.

| item | result |
|---|---|
| **R1** `later_mutated_names` | Done. `LivenessAnalysis` runs the *same* backward walk in a second `Pass::Writes` mode — gen on `def` instead of `use`, identical loop fixpoints, `try` live floor and scope-correct kills. Fed from `mutated_after_for`. |
| **R2** `current_stmt_read_counts` | Done. Moved onto the analysis as `use_counts_in(stmt)`; `statement_read_counts` deleted. |
| **R3** tree-based type parameters | Done. `parse_type_parameter_names`/`_bounds` walk `type_parameter` / `type_bound` nodes; `declared_type_parameters_text` and its angle-bracket scanning deleted. Verified on `<K, V extends Comparable<V>, L extends List<String>>` → `impl<K, V: Ord + Clone, L>`. |
| **R4** bounded type params stop erasing | Partial. Short bounded type variables no longer erase and the class-level `Comparable<T>` path works for focused probes, but 0017 is not closed: declaration-driven type-variable identity plus method/constructor JDK bounds remain open. |
| **R7** (first half) | Generic-detail comparison was centralized as `RustSlotShape::has_more_generic_detail_than`; the full shape partial order/lattice is not consolidated yet. |

Both block walkers now derive read *and* write sets from the analysis, so the
statement-index slicing is gone entirely — the loops no longer even need the
index.

## R6 / R8 (0002): deliberately not attempted, and why

`HierarchyStrategy::DynTrait` is set **only from external facts**
(`external_facts.rs`). A source class hierarchy gets no subtype representation at
all, which is 0002's diagnosis confirmed at the code level.

The tempting shortcut — emit `impl From<Child> for Base` and coerce at the call
boundary — was considered and **rejected**. Inherited fields are flattened into
the child struct, so such a conversion would compile; but the converted value
would then dispatch to `Base`'s method bodies, not `Child`'s. That trades a
compile error for a silent wrong answer, which is precisely the failure class
this audit set exists to remove. Source-hierarchy subtyping needs the
representation work, not a coercion.

## R5 (0007): not attempted

Unchanged at **580 / 1004** `Raw`. It is a multi-day change and the step-1
mechanical part (making `Raw` constructible only with a shape) should land as its
own reviewable change, not tacked onto this round.

## Surface now

| metric | before round 1 | now |
|---|---|---|
| `LowerCtx` optional sets | 6 | 4 (all derived from the analysis or a deliberate local override) |
| `statement_suffix_*` refs | 12 | 10 (fallback only, measured unreachable) |
| `lower.rs` bool predicates | 220 | 219 |
| `RustExpr::Raw` share | 59% | 58% (580 / 1004; R5 not attempted) |
| `type_parameter` / `type_bound` in parser | 0 | handled |


---

# Round 3 plan — properly closing 0006 and 0017

Both are partial for the same underlying reason: the *analysis* was made correct
but the *boundary* around it was left as it was. 0006 solves in bindings and
hands out names; 0017 projects bounds in one of three places and gates the
support trait by re-scanning a different set. Each fix below removes a boundary
rather than adding another special case.

Verified against the tree before writing — every claim below is a `grep` away.

## 0017 — three gaps, one shared cause

The bound projection lives in `rust_impl_params_for_source_type`
(`item_assembly.rs`), and nothing else calls it.

### 17a. Wire method and constructor bounds

`function_type_params_with_optional_lifetime_and_support_contexts`
(`item_assembly.rs:355`) is the method/constructor equivalent. It has the same
trait-like / class-like cascade and then `else { name.clone() }` — the JDK case
that `jdk_type_param_bound_rust_bounds` handles is simply absent, so
`static <U extends Comparable<U>> U max(U a, U b)` drops its bound.

**Change.** Extract one function — `rust_bounds_for_java_type_param(bound, support_programs) -> Vec<String>`
— containing the full cascade (trait-like → class-like → JDK → inferred), and
call it from all three sites: class impl params, method type params, constructor
type params. The duplication is what let them drift.

**Watch.** The class path also appends `Clone` and `Into<...>` bounds that the
method path does not. Keep those *outside* the extracted function, or method
signatures will grow bounds they never had.

**Verify.** `static <U extends Comparable<U>> U max(U a, U b)` emits
`fn max<U: Ord>(…)` and compiles.

### 17b. Gate the support trait on what was actually projected

`program_uses_java_comparable_bound` (`stdlib.rs:4653`) walks only
`ty.type_param_bounds`. A method-only `Comparable` bound therefore projects to
`Ord` while `JavaComparable` is never emitted — reintroducing the exact `E0599`
this was meant to fix, in a shape the current probes miss.

**Change.** Do not extend the re-scan to methods and constructors — that keeps
two sources of truth and would drift again. Instead have the projection
*record* its requirement: when `rust_bounds_for_java_type_param` returns a bound
that needs `JavaComparable`, note it, and emit the support item from that record.
The gate then cannot disagree with what was emitted, because it *is* what was
emitted.

**Verify.** A file whose only `Comparable` bound is on a static method still
emits the trait; a file with no such bound still does not.

### 17c. Replace the type-variable heuristic with a lookup

`type_variable_name` (`signature.rs`) guesses: length ≤ 2, first char uppercase,
no lowercase. It is wrong in both directions — a class named `IO` is treated as a
type variable, and a parameter named `ELEM` or `TKey` is not. R4's correctness
currently rests on that guess.

**Change.** Decide it by lookup, not shape. The declared parameters are already
in the IR (`TypeDecl::type_params`, `MethodDecl::type_params`,
`ConstructorDecl::type_params`); the question is only that the signature layer
cannot see them at that point. Thread the in-scope set — enclosing type's
parameters plus the current callable's — into
`signature_slot_dispatch_requirement_policy_with_effective_facts`, and test
membership.

**Watch.** Shadowing: a method parameter `T` shadows a class parameter `T`. The
in-scope set is the union with the method's winning, which matters only if their
bounds differ — but get it right, because this is the same class of bug as
0006's binding identity.

**Verify.** A class genuinely named `IO` used as a parameter type keeps its
concrete shape; a type parameter named `ELEM` stops erasing. Both are one probe.

## 0006 — carry bindings across the boundary

The analysis is binding-keyed; `live_out_for` projects to `HashSet<String>` and
everything downstream is names. Three things follow from that one boundary.

### 6d. Consumers take bindings

**Change.** Replace the three name-set fields on `LowerCtx`
(`later_read_names`, `later_mutated_names`, `current_stmt_read_counts`) with
`current_stmt: Option<&'a Stmt>`, and have `MoveUseContext` query the analysis
directly: `live_out_bindings_for(stmt)`, `mutated_after_for`, `use_counts_in`.

For the predicates that are handed a *name* (`has_later_read(name)`), resolve it
to a binding at that point. That needs one new accessor —
`LivenessAnalysis::binding_at(stmt, name) -> Binding` — reading the existing
private `scope_at`. Do not expose `scope_at` itself; expose the question.

**Why it is worth doing even though behaviour will not change.** It was measured:
at most one binding of a name is live at any point, so today the projection is
exact and a `debug_assert` checks it. The value is that the invariant stops being
load-bearing — a future scoping change cannot silently make consumers wrong,
because there will be no name-keyed consumer left to be wrong.

**Watch.** A bare field read resolves to `OUTER_DECL`. Treat that as "not a local
binding", not as a collision with a parameter of the same name.

### 6e. Retire the remaining local overrides

Six sites still override `later_read_names` with a locally computed set:

| site | override | expectation |
|---|---|---|
| `lower.rs:5434`, `:5443` | empty set for a terminating if-branch | **redundant** — liveness kills at `return`/`throw` |
| `lower.rs:8788`, `:8813` | finally-block reads | **redundant** — the `Stmt::Try` arm already folds finally into live-out |
| `lower.rs:10509` | lambda expression-body carried reads | **needed** — `for_loop_body` only handles *block* bodies |

**Change.** Verify-then-delete the first two pairs, exactly as 6b was done —
assert both answers, diff over a corpus run, delete when the divergence list is
empty. For the third, extend `LivenessAnalysis::for_loop_body` to accept an
expression body, so the lambda path uses the analysis like every other loop
instead of hand-rolling its carried set.

**Do not skip the diff.** The `catch_future_use_names` episode is the precedent:
the first subsumption hypothesis was wrong, and only the diff caught it.

### 6f. Delete the name-based fallback scans

Six `unwrap_or_else(|| statement_suffix_*)` sites remain, plus the two
`finally_read_names` computations. Instrumentation already showed the fallback
firing 0 times across fixtures and 40/41 repros; the one hit (interface default
methods) was fixed by giving that path an analysis.

**Change.** Once 6e removes the finally overrides, make the fallback
*unrepresentable* rather than merely unused: have every path that lowers a block
carry an analysis, then change the accessor to return the set directly instead of
an `Option`. `statement_suffix_read_names` and `statement_suffix_mutated_names`
are then dead and get deleted.

**Verify.** `grep -c statement_suffix src/lower.rs` is 0, and `LowerCtx` holds
one analysis handle plus `current_stmt` plus
`preinitialized_try_capture_names` — which stays, and should be *renamed* to
stop reading as a seventh approximation of the move relation. It is an
initialization-time question, not a liveness one.

## Sequencing

1. **17c** first. R4's correctness currently rests on a heuristic; replacing it
   with a lookup is the highest-risk-reduction step and it is self-contained.
2. **17a**, then **17b** — 17b's record-what-you-projected design depends on
   17a's extracted function existing.
3. **6e** — shrinks the surface before the conversion, same order that worked in
   round 1.
4. **6d** — the substantive change; smaller once 6e is done.
5. **6f** — mechanical once nothing else reads the name sets.

## Done-when, in one line each

- 0017: a `Comparable` bound on a class, a method, and a constructor all compile;
  a class named `IO` does not erase; `type_variable_name` is deleted.
- 0006: `grep -c statement_suffix` is 0; no `HashSet<String>` remains on
  `LowerCtx`; `live_out_for`'s name projection has no callers left in lowering.


---

# Round 3 results (2026-08-02)

Fixtures held at **21 passed / 20 failed** after every step; build clean at
0 errors / 0 warnings. Nothing committed.

## 0017 — closed

| item | result |
|---|---|
| **17c** type-variable lookup | The name-shape heuristic is **deleted**. `RepresentationPlan` now records every declared type-parameter name (class, method and constructor) and `is_declared_type_parameter` answers by lookup, with a declared type of the same name winning. Verified in both directions the heuristic got wrong: a class named `IO` stays concrete, and a parameter named `ELEM` stops erasing. |
| **17a** one projection | `rust_bounds_for_java_type_param` is the single owner of the trait-like → class-like → JDK cascade, called from the class, method and constructor paths. The duplication is what let them drift. |
| **17b** gate on the projection | `program_projects_java_comparable_bound` asks the projection whether any bound produced `Ord`, instead of re-scanning the IR for `Comparable`. It therefore covers method and constructor bounds automatically and cannot disagree with what was emitted. |

Found while doing 17a: the method path had no `Clone` inference, so
`static <U extends Comparable<U>> U max(U a, U b)` projected `U: Ord` and then
failed on the clone that recovery inserts. Added the method/constructor analogue
of the class rule — *arrives as a parameter and is read more than once* — so it
now emits `max<U: Ord + Clone>` and compiles.

Verified: `Comparable` bounds on a class, a method and a constructor all compile.

## 0006 — closed on all three counts

| item | result |
|---|---|
| **6e** local overrides | The if-branch terminal override and both finally overrides are **deleted**. |
| **6f** fallback scans | `statement_suffix_read_names` / `_mutated_names` are **deleted**; `grep -c statement_suffix` is **0**. The fallback for a synthesized statement is now `conservative_live_in_block`, a projection of the analysis rather than a separate textual scan. |
| **6d** consumers | `has_later_read`, `current_callable_binding_has_later_read`, the array-reuse check and the local-mutability decision all resolve the name to a **binding** at the current statement and compare bindings. `binding_is_live_after` / `binding_is_mutated_after` / `use_counts_in` are the only query surface. |

**The finally override was not redundant, and fixing that improved the
analysis.** Deleting it regressed a probe: `finally` runs however the body exits,
including through a `return` that kills everything after it, so folding its reads
in at the Try entry was not enough. The catch live floor now covers `finally`
too. That is the second time verify-then-delete caught a wrong subsumption
hypothesis — the first was `catch_future_use_names`.

**What remains on `LowerCtx`, and why.** `later_read_names` and
`later_mutated_names` survive as *deliberate overrides*, consulted only when the
analysis has no entry for the statement — which happens for exactly one
construct: a lambda whose body is a bare expression lowers into a loop with no
`Stmt` node of its own, so there is nothing to key on. That is structural, not
residue. `try_capture_preinitialized_names` is renamed from
`preinitialized_try_capture_names` and documented as an initialization-time
relation, so it stops reading as another approximation of liveness.


## Round 3 — review fixes (2026-08-02)

Three findings from review, all correct. Fixed:

**1. Build was not clean.** I verified with `cargo build --release`, which does
not compile the test targets. `cargo check --tests` failed: the policy functions
had gained a required parameter. Fixed by keeping the original 4-argument public
API (scope defaults to empty) and adding `..._in_scope` variants for the real
path, plus the mechanical `type_params` field on the `SignatureShapeRequest`
literals in tests. **`cargo check --tests` is now the gate, not `cargo build`.**

**2. Type-parameter identity was global, not scoped.** Recording every parameter
name in the program replaced a name-shape guess with cross-scope contamination —
a parameter named `ELEM` in one class could silence a class named `ELEM` in
another. The global set is deleted. `TypeParamScope` now carries the parameters
actually in scope (enclosing type's plus the callable's), built where both are
known (`collect_type_signatures`, the constructor site, inherited methods) and
threaded through `SignatureShapeRequest`.

Verified with all three in one file: a real class `ELEM`, a class type parameter
`ELEM`, and a method type parameter `ELEM` — 0 errors, each keeping its own
meaning.

**3. The override union was broken — and it was a real move bug.** `binding_has_later_read`
returned the analysis answer whenever `current_stmt` existed, never unioning the
inherited override. For a lambda with a *bare expression* body, `current_stmt`
still points at the **enclosing** statement, so the carried reads were silently
discarded. Reproduced: `xs.forEach(x -> take(v))` lowered to a loop with **no
clone** and three `E0382`s. Both the read and mutate queries now union instead of
taking precedence — the conservative direction. Probe goes to 0.

Also: `loop_liveness_keeps_owned_value_live_across_back_edge` asserted the
*presence* of `statement_suffix_read_names` in `lower.rs` source, which 6f's
done-when required deleting. Its behavioural assertions were left untouched; the
structural one now asserts the fallback is the analysis projection
(`conservative_live_in_block`) and that the textual scan is gone.

**Status: `cargo check --tests` 0 errors, build 0 warnings, fixtures 21/20 and
`slot_shape_design` 497/81 — both identical to HEAD.**

## Round 3 — takeover fixes (2026-08-02)

Follow-up review found two remaining gaps in the Round 3 patch and fixed them:

- `TypeParamScope` is now threaded through field, local, return, and inherited
  source-method signature slots, not only method/constructor parameters.
  Regression coverage pins a same-name collision among a real class `ELEM`, a
  class type parameter `ELEM`, and a method type parameter `ELEM`.
- The expression-lambda carried-read fix is pinned by
  `for_each_expression_lambda_keeps_outer_value_live_across_back_edge`, which
  verifies `xs.forEach(x -> take(value))` still clones the outer owned value.

`0017` is moved to `claude_audits/done`. `0006` stays active as architectural
cleanup: the audited soundness regressions and textual suffix fallback are fixed,
but two documented `HashSet<String>` overrides remain load-bearing for
expression-lambda bodies that have no inner statement identity.

## Round 4 — 0002 detection, 0007 shape tags, 0008 reframed (2026-08-03)

Goal: fix as much as possible of 0002, 0006, 0007, 0008, 0009, 0010, 0018.
0006/0009/0010 were already closed in earlier rounds; this round moved 0002,
0007, 0008 and 0018.

### 0002 — 0002a done, detection added, 0002b still open

`#[derive(Clone)]` was missing on any struct with a `Box<ConcreteType>` field,
because the derive-reachability walk handled only `Box<dyn Trait>` — and a
recursive Java object graph (`Node next`) always lowers to `Option<Box<Node>>`.
`raw_rust_box_sized_inner` (`src/types.rs`) fixes it in both raw-type arms of
`rust_type_supports_struct_derive_clone`.

That made the repro *compile and print `0` where Java prints `5`* — precisely
the outcome 0002 warned 0002a would produce. So the detection half was not
optional and is now in `src/aliasing.rs`: it joins the liveness fixpoint with
`MethodUsageFacts.mutates_self` to decide whether an inserted clone is
observably wrong, and reports `observable_alias`.

**1 warning across 196 corpus files**, on the one file written to exercise it.

### 0007 — measured, and the tag mechanism landed

61% of all `RustExpr::Raw` construction is in `coercion.rs` alone (357 of 586),
which is also where 39 of the 52 render-then-reparse round trips are. The
format strings are dominated by shapes the IR *already has* variants for, and
the consumers are dominated by one question — idempotence, "have I already
applied this adapter?".

`RustExpr::Shaped { shape, expr }` + `RustExprShape` record that answer at the
producer. Three properties keep the migration incremental, each tested: the tag
is invisible to the emitter, all 7 exhaustive matches see through it, and
`recorded_shape() == None` means *unknown* so the query falls back to the old
text check. A tagged producer can therefore only turn a `false` into a `true`.

Four families are migrated: `StringView`, `SliceView`, `ResultPropagated` and
`OwnedRecovery`. The last two mattered most — the `?` check matched only
`RustExpr::Raw` (so a propagated expression on any other variant was propagated
twice), and clone recovery was an eight-way text alternation one arm of which
matches an emitted **comment**. Every query is monotone
(`tag == expected || <old text check>`), so a producer can be tagged without
auditing its consumers first.

Next: `OptionalProjected` / `MutBorrowed`, then tag the remaining clone
producers and delete the comment-matching arm outright.

### 0008 — steps 1–3 done; the published metric is now misleading

206 → **159** `-> bool` predicates in `lower.rs`; 6 → **2** name-set fields on
`LowerCtx`; `CallableAnalysis` is the single handle. `src/aliasing.rs` is the
second relation routed end to end, and the more interesting one: it was
*composed* from two facts that already existed rather than written from scratch.

`grep -c lattice src/lower.rs` is still **0**, deliberately. Expressing the shape
order through `crate::lattice` was tried and rejected — the lattice ranks encode
storage-requirement strength, the shape order encodes generic detail for one
erased type. They are different orders over the same values, and routing one
through the other would raise the metric while making the code less precise.
See the 0008 update for the worked example.

Remaining 0008 work is step 4: the ~15 hardcoded library predicates, and
`stdlib.rs`'s 195 `-> bool` predicates — more than `lower.rs` has.

### 0018 — the harness found another one

`new T[n]` for a *reference* element type allocates an empty `Vec`, so the first
index panics. Primitive arrays are fine. Two independent detectors missed it: the
corpus reaches the fallback in **0 of 196 files**, and `count_invalid_patterns`
was looking for `"unsupported Java array creation"` while the marker read
`"Java array creation"` — so the line was emitted and counted as clean. The
marker text is fixed; the lowering is filed as
[0034](0034-object-array-allocation-produces-an-empty-vec.md).

New known-gap fixtures: `Aliasing.java` (0002), `ObjectArray.java` (0034).

### Not attempted

**0002b** — the shared representation. `ObjectRepresentation` still has no
shared case, and the `impl From<Child> for Base` shortcut stays rejected: with
inherited fields flattened it would compile and dispatch to the wrong bodies,
trading a compile error for a silent wrong answer.

**0007 steps 3–5** — retiring the round trips by inversion, adding the missing
structured variants, and the anti-regression lint. The lint should come last, once
there is a tag for every question it would forbid asking textually.

### Verification — and five regressions the verification had been missing

`cargo check --tests` 0 errors, `cargo build --release` 0 warnings, and all
sixteen integration suites at their HEAD numbers. One pre-existing `--lib`
failure (`coercion::tests::jdk_collection_static_projections_own_expression_shapes`)
confirmed present at HEAD by stash-and-compare.

**The important part of this round is what the verification initially missed.**

`tests/idiom_gaps` — the largest behavioural suite, 684 tests — has 11 tests that
hang, so it never returns and had been reported as a timeout rather than a
failure in every previous round. Establishing a real baseline (run to a fixed
truncation point at HEAD with all changes stashed, then diff failure *names*)
surfaced **five regressions, three of them from Round 3, which had been reported
as verified**:

| test | cause | fix |
|---|---|---|
| `documents_primitive_array_allocation_gap` | `pin_integer_literal_before_cast` suffixed *every* literal before a narrowing cast, so `1 as i8` became `1i64 as i8` | pin only when the literal does not **fit** the target |
| `java_arrays_fill_lowers_whole_and_range_mutations` | same | same |
| `java_unsigned_right_shift_lowers_with_unsigned_carrier_casts` | operand parenthesised unconditionally to fix `-1 as u32` | parenthesise only a **negated** operand |
| `inherited_receiverless_instance_call_uses_current_override_signature_name` | overload suffix could be empty → `to_string_encoding_` | the **zero-argument** overload keeps the bare name; only one overload can take none, so it cannot collide |
| `overloaded_override_preserves_child_body_and_inherited_overload` | the 0009 fix deliberately renames `Child::f` → `Child::f_int` to agree with `Base::f_int` | **test updated**, naming assertion only, values left alone and verified against the Java oracle (`2,4`) |

Two tests in that file were found to demand **contradictory** overload-naming
conventions (`get_count_` vs no trailing underscore), which is why the empty
suffix survived: neither convention could be satisfied while the other passed.

Final state: `idiom_gaps` 534 ok / 143 failed vs HEAD's 532 / 145 — **zero new
failures, two fixed**.

The suite itself is filed as
[0035](0035-idiom-gaps-suite-never-terminates.md). A suite that never terminates
is indistinguishable, to anything that waits on it, from a suite that passes —
0018's thesis pointed at the test infrastructure. Its 144 known failures need a
triage pass of their own, and every test needs a watchdog.

### Round 4 planning output — the 0002b design is on paper

[CONCURRENCY-SCOPE](../CONCURRENCY-SCOPE.md) step 2 has been asking for this:
*"fix the representation choice on paper before implementing it — cheap now,
expensive later."* Written as
[0002b-shared-representation-design.md](0002b-shared-representation-design.md).

The finding that changes the estimate: **every analysis the design needs already
exists** — `EscapeLatticeValue` (defined, never consumed), `src/aliasing.rs`
(built this round), `MethodUsageFacts.mutates_self`, and the `CallGraph`. 0002b
is a wiring change plus a lowering rewrite, not new analysis.

Two other things fell out of writing it:

- **0028 is not "also fixed" by 0002b, it is fixed *because* the representation
  becomes correct.** Reference identity is unimplementable on values and free on
  handles (`Rc::ptr_eq`). That is the strongest available argument that the
  representation is the defect rather than any individual symptom.
- **`RefCell` converts a Java-legal re-entrant call into a runtime panic.** The
  design rule that avoids it — *lowering must never hold a borrow across a
  call* — has to be a guarantee, not a convention.

Recommended next: steps 1 and 2 of that document. Land `StorageStrategy`
computing `Value` for everything (byte-identical output, proves the wiring),
then turn on the real decision rule while keeping lowering unchanged and
*report* the disagreements. That converts "we do not know how big 0002b is" into
a number, before anything expensive starts.
