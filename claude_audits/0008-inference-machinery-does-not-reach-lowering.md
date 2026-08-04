# 0008 — The inference machinery exists but does not reach lowering

Status: **validated by inspection** (structural)
Severity: medium-high — explains why lowering keeps growing ad-hoc predicates
Kind: architecture / wiring

## Observation

The project has real inference machinery:

- `src/lattice.rs` — `LatticeDescriptor<T>`, `RankedLatticeJoin<T>`, and value
  domains: `DispatchLatticeValue`, `EscapeLatticeValue`
  (`LocalOnly | Returned | Stored | UnknownBoundary`),
  `InitializationLatticeValue`, `NULLABILITY_VALUES`, `OWNERSHIP_VALUES`,
  `VIEW_VALUES`.
- `src/fact_saturation.rs` — fixpoint saturation over facts.
- `src/graph.rs` — graph utilities.
- `src/usage.rs` — a real call graph (`CallGraph`, `CallableKey`,
  `CallGraphEdge`) with usage facts.

None of `lattice`, `fact_saturation`, or `graph` is referenced from
`src/lower.rs`:

```
$ grep -l 'lattice' src/*.rs
fact_saturation.rs  lib.rs  lattice.rs  semantic_analysis.rs
resolver_benchmark.rs  signature.rs

$ grep -l 'fact_saturation' src/*.rs
lib.rs  facts.rs  semantic_analysis.rs  signature.rs

$ grep -l 'crate::graph' src/*.rs
semantic_analysis.rs  signature.rs
```

The machinery is confined to the upstream fact/signature layer. `src/lower.rs`
— 23.7 kloc, the largest consumer of semantic information — reaches none of it.

## What lowering does instead

It re-derives the same information locally, with 206 `-> bool` predicates and
six ad-hoc name-set fields on `LowerCtx`
(see [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md)).

Predicate counts by module:

| module | `-> bool` fns |
|---|---|
| `lower.rs` | 206 |
| `stdlib.rs` | 200 |
| `signature.rs` | 136 |
| `call_plan.rs` | 43 |
| `semantic_analysis.rs` | 38 |
| `coercion.rs` | 30 |
| `facts.rs` | 30 |
| `conversion_plan.rs` | 22 |
| `usage.rs` | 12 |

Reading the `lower.rs` names, they cluster into a small number of relations
that each *want* to be one analysis:

- **Liveness / move legality** (~17 predicates) — see 0006.
- **Shape specificity / subsumption** (~10): `shape_has_more_generic_information`,
  `lowered_type_slot_has_more_generic_information`,
  `initializer_shape_is_more_specific_for_local`,
  `local_shape_should_inherit_initializer_shape`,
  `cast_target_erases_slot_generic_detail`,
  `java_types_have_same_erased_identity`, `same_concrete_java_type`, …
  These are hand-rolled comparisons on a **partial order over shapes**. A
  partial order with an explicit `join`/`meet` and a subsumption test would
  replace all of them, and would make the "which shape wins" question answerable
  once instead of per site. `src/lattice.rs` already has the vocabulary
  (`OWNERSHIP_VALUES`, `VIEW_VALUES`, `RankedLatticeJoin`) — it is simply not
  applied to slot shapes at lowering time.
- **Nullability refinement** (~12): `expr_type_is_optional`,
  `non_null_refinements_make_optional_expr_an_element`,
  `refined_non_null_receiver`, `exact_erased_object_local_is_optional`, …
  This is flow-sensitive **refinement typing**. A partial version already exists
  (`ExprRefinements` with `non_null_locals`), threaded manually through block
  lowering. `NULLABILITY_VALUES` exists in the lattice module and is unused here.
- **Hardcoded library knowledge** (~15): `is_java_get_class_name`,
  `is_known_exception_supertype_name`, `java_type_is_string_type`,
  `current_type_extends_jdk_input_stream_facade`,
  `receiver_is_known_string_producing_call`, `java_type_name_is_tuple2`,
  `expr_is_tuple2_owner`, … Library-specific facts compiled into the translator
  rather than declared in `javalibs` manifests. The convergence plan already
  calls this out; the count is the argument for prioritising it.

## Why the wiring is the problem

Each cluster above has a matching piece of machinery that already exists one
layer up. The translator is not missing the *idea* of a lattice or a fixpoint —
it is missing a path for lowering to consume one. The result is that every new
corpus failure gets fixed where the failure is observed (in `lower.rs`), because
that is the only place with the context, and the predicate count grows.

This is consistent with the repository's own history: `docs/` contains ~20
sequential `proceed-from-current-corpus-*` plans, which is the signature of
fix-at-the-symptom.

## Proposed direction

1. **Pick one relation and route it end to end** rather than refactoring
   broadly. Liveness (0006) is the best first candidate: it has a confirmed
   miscompilation ([0001](done/0001-loop-back-edge-invisible-to-move-analysis.md)),
   a clear specification, and an obvious query surface.
2. **Give lowering a single analysis handle.** Replace the six `Option<&HashSet>`
   fields with one non-optional `&CallableAnalysis` carrying the results of the
   liveness, nullability, and shape-subsumption queries.
3. **Make slot-shape subsumption a real partial order.** Define
   `subsumes(a, b) -> bool` plus `join(a, b)` on `RustSlotShape` and delete the
   ~10 "more generic information" predicates. This is self-contained and does
   not need a CFG, so it can proceed in parallel with the liveness work.
4. **Move the ~15 hardcoded library predicates into `javalibs` manifests.**
   Mechanical, low risk, and it shrinks the surface that the other work has to
   reason about.

## A caution

The convergence plan notes that "a temporary corpus error-count increase is
acceptable". That is right, but with 206 predicates in one file and no
binding-identity model, a broad simultaneous refactor will produce an error
increase that cannot be attributed. Sequence by relation, keep the old predicate
and the new query both computed behind an assertion during migration, and use
the divergence list as the work queue.

---

## Update 2026-08-03 — steps 1–3 largely done; step 4 is what is left

Status: **partly done**. The headline metric in this file is now misleading and
should not be used to judge it.

### Measurements

| | when filed | 2026-08-03 |
|---|---|---|
| `-> bool` fns in `lower.rs` | 206 | **159** |
| `Option<&HashSet>` name-set fields on `LowerCtx` | 6 | **2** |
| `grep -c lattice src/lower.rs` | 0 | **0** |

### Step 1 — done, twice

*"Pick one relation and route it end to end."* Liveness was the chosen
candidate and is now a real backward fixpoint over binding identity
(`src/liveness.rs`), consumed by lowering through statement identity rather
than through pre-projected name sets. See
[0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md) and
[0001](done/0001-loop-back-edge-invisible-to-move-analysis.md).

A second relation followed, and it is the more interesting one because it was
*composed* from two existing solved facts rather than written from scratch:
`src/aliasing.rs` joins the liveness fixpoint with
`MethodUsageFacts.mutates_self` (already a fixpoint over self-calls in
`src/usage.rs`) to decide whether an inserted clone is observably wrong. That is
the first consumer of `mutates_self` outside signature planning, and it produced
a diagnostic with 1 hit across 196 corpus files — an analysis, not a heuristic.
See [0002](0002-object-aliasing-has-no-representation.md).

The lesson worth carrying: the payoff was not "lowering can now call a lattice",
it was that two analyses that already existed could answer a question neither
could answer alone, once something owned the join.

### Step 2 — done

`CallableAnalysis` (`src/callable_analysis.rs`) is the single handle, carrying
`usage`, `call_graph`, `signatures`, `exceptions` and `fallibility`;
`LowerCtx.callable_analysis` is non-optional. Four of the six ad-hoc name-set
fields are gone. The two that remain are load-bearing and documented as such: a
lambda whose body is a bare expression lowers into a loop with no `Stmt` of its
own, so there is no statement for the analysis to key on, and the override is
unioned with — not preferred over — the solved answer.

### Step 3 — done as far as it is honest to take it

The ~10 "more generic information" predicates are one method,
`RustSlotShape::has_more_generic_detail_than` (`src/slot_shape.rs`), and the
last pass-through wrapper in `lower.rs` is deleted; call sites use the method.

**What was deliberately not done, and why.** The obvious next move is to express
that order through `crate::lattice` and finally make the `grep -c lattice` count
non-zero. It was tried and rejected: the lattice ranks encode *storage
requirement strength* (`Owned` 40 > `SharedBorrow` 10), while the shape order
here is *generic detail for the same erased type*. They are different orders
over the same values. Routing one through the other would raise the metric this
file publishes while making the code less precise — for instance replacing
`matches!(shape.dispatch, DispatchShape::Concrete)` with a rank comparison turns
a variant test into a "top of the lattice" test, which stops being equivalent
the moment a second variant shares the top rank.

So `grep -c lattice src/lower.rs` is still 0, and that is the right answer.
The count was a proxy for "lowering re-derives what upstream already solved",
and that underlying problem *has* moved — 206 → 159 predicates, and two whole-
program relations now consumed rather than approximated. **Judge this issue by
the predicate count and the list of relations lowering consumes, not by the
lattice grep.**

The lattice reaching lowering remains correct as an *end state*, but it needs
the ownership/view/nullability axes folded into a single shape order first, with
`join` and `subsumes` defined on `RustSlotShape` — the doc comment on
`has_more_generic_detail_than` already marks the spot. That is a design task, not
a wiring task.

### Step 4 — not started, and now the largest remaining item

The ~15 hardcoded library predicates (`is_java_get_class_name`,
`java_type_is_string_type`, `current_type_extends_jdk_input_stream_facade`,
`receiver_is_known_string_producing_call`, …) are unchanged, and `stdlib.rs`
still holds 195 `-> bool` predicates — more than `lower.rs`. This is the
mechanical, low-risk work this file recommended, it is where the remaining
predicate mass actually is, and nothing in the current state blocks it.

---

## Update 2026-08-03 (second pass) — step 4 started, and the lattice finally has a reason to reach lowering

### Step 4, honestly scoped

Two of the hardcoded library predicates are moved: the JDK exception-supertype
set and the `java.io` input-stream facade set now live in `src/javalibs.rs` as
named constants with one query and one test each, and `lower.rs` asks the
library module instead of embedding the names.

The rest of the list in this file needs re-reading, because it conflates two
different things:

- `is_known_exception_supertype_name`, `current_type_extends_jdk_input_stream_facade`
  — genuinely *library* knowledge: facts about the JDK compiled into lowering
  rules. These are the movable ones.
- `is_java_get_class_name` (`matches!(name, "getClass" | "get_class")`),
  `java_type_is_string_type` — **core language** knowledge, not library
  knowledge. Moving `String` into a library manifest would be
  miscategorisation, not progress.

And the destination is not what the issue assumes. `src/javalibs.rs` is Rust,
not a manifest format; the real manifest system is `external-pack` lines parsed
by `src/external_packs.rs` into a `FactSet`. Expressing these predicates as
manifest *data* is therefore a design task — deciding what the manifest grammar
must gain — not the mechanical move this file describes. **That reframing is the
main output of this pass on step 4**, and it should be settled before anyone
budgets the rest.

### The lattice now has a consumer, for a reason that holds

`grep -c lattice src/lower.rs` is still 0, and this file's previous update was
right to say chasing that number was wrong. What changed is upstream:
`src/storage_strategy.rs` computes an ordered lattice
(`Value < SharedImmutable < SharedLocal < SharedSync`, join = max) whose domain
mirrors `EscapeLatticeValue` — the value domain that has sat in `lattice.rs`
since it was written with no analysis behind it.

That is a **third** whole-program relation routed end to end, after liveness and
aliasing, and the first one whose shape the `lattice` module actually anticipated.
Worth recording precisely because the previous update rejected a *forced* use of
the lattice: the difference is that this one needed the order, rather than
needing the metric to move.

### Counts

| | filed | first pass | now |
|---|---|---|---|
| `-> bool` fns in `lower.rs` | 206 | 159 | **158** |
| `-> bool` fns in `stdlib.rs` | 200 | 195 | 196 |
| relations solved once and consumed by lowering | 0 | 2 | **3** |

The predicate count barely moved, and that is the honest reading: this pass
spent its effort on correctness (a diverging fixpoint, a mis-keyed override
group, a dropped array dimension) rather than on the count. The count is a proxy;
the relations row is the thing it was a proxy for.
