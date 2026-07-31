# 0010 — Fallibility is decided three times independently and the decisions disagree

Status: **PARTIAL** — was: validated (corpus: FastQC, 214 errors)
Severity: high
Kind: missing fixpoint — a call-graph property computed locally


## Resolution

**PARTIAL (audited 2026-08-01).** A least-fixpoint solver over the call graph
exists (`plan_program_fallibility`), and fallibility is now equalized across
override sets so a trait method and all its implementations can share one
signature. Focused tests cover call-graph propagation, external Rust-name
mapping, source-call Result propagation, and override-set equalization.

Still open: lowering has legacy call-site propagation probes alongside the
solved relation. Close this only after every emitted `?` is justified by the
same callable fallibility plan, or a divergence diagnostic names the mismatch.

## Symptom

`?` is inserted at a call site whose callee does not return `Result`, inside a
caller that does not return `Result`. Every occurrence is two errors:

```
error[E0277]: the `?` operator can only be applied to values that implement `Try`
  --> src/generated/00003_AbstractUnivariateSolver.rs:63:9
63 |         self.increment_evaluation_count()?;
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ the `?` operator cannot be applied to type `()`

error[E0277]: the `?` operator can only be used in a method that returns `Result` ...
62 |     pub fn compute_objective_value(&self, point: f64) -> f64 {
   |     -------------------------------------------------------- this function should return `Result`
63 |         self.increment_evaluation_count()?;
   |                                          ^ cannot use the `?` operator in a method that returns `f64`
```

Counts in FastQC: 126 + 88 = **214**, the second-largest error class. A further
8 `E0308 expected String, found Result<String, JavaException>` in htsjdk are the
mirror image — a `Result` produced where a plain value was expected.

## Root cause

Three decisions are made in different places and never reconciled:

| decision | where |
|---|---|
| is this *call* fallible → insert `?` | lowering, at the call site |
| does this *callee* return `Result` | signature planning |
| does this *caller* return `Result` | signature planning, per method |

In the instance above, `incrementEvaluationCount()` throws
`MaxCountExceededException`, which is **unchecked**. Java does not require it in
a `throws` clause, so the signature layer left the callee returning `()`, while
the call-site layer independently concluded the call could throw and emitted
`?`. Nothing forced the two to agree, and nothing propagated the consequence up
to `computeObjectiveValue`'s own return type.

## Why this is a fixpoint problem

Fallibility is not a local property. It is the least fixpoint of:

```
fallible(m)  ⟺  m's body throws an exception that m does not catch
              ∨  ∃ call m → n where fallible(n) and m does not catch it
```

over the call graph, including recursion and virtual dispatch (a call through
an interface is fallible if *any* possible implementation is). Once solved,
everything else is determined and cannot disagree:

- return type of `m` is `Result<T, JavaException>` **iff** `fallible(m)`;
- `?` is inserted at call `m → n` **iff** `fallible(n)`;
- a `catch` that handles everything a callee throws is the only thing that
  clears the flag.

Computing any of the three independently guarantees eventual disagreement,
which is what the 214 errors are.

## What already exists

- `src/usage.rs` builds a real call graph (`CallGraph`, `CallableKey`,
  `CallGraphEdge`) — the substrate is there.
- `src/fact_saturation.rs` performs fixpoint saturation over facts.
- `src/exception_plan.rs` holds the per-method exception model.

The pieces exist; they are not composed into one fallibility solve, and
`fact_saturation` is not reachable from `src/lower.rs`
(see [0008](0008-inference-machinery-does-not-reach-lowering.md)).

## Fix direction

1. Add an explicit `fallible: bool` (better: `ThrownSet`) per `CallableKey`,
   solved to a fixpoint over the existing call graph *before* signature planning.
   Seed from `throws` clauses **and** from `throw` statements of unchecked
   exceptions — the unchecked case is what broke here.
2. For virtual calls, join over all implementations reachable through the
   dispatch model; default to fallible when the target set is unknown, since
   the conservative direction is `Result` (a spurious `Result` costs idiom
   quality, a missing one costs compilation).
3. Make signature planning and `?` insertion both *read* that one result. Neither
   should compute fallibility.
4. Add an assertion during migration: at every emitted `?`, the callee signature
   must be `Result`, and the enclosing signature must be `Result`. Any violation
   is a solver bug, not a lowering bug — this converts 214 downstream compile
   errors into a handful of attributable analysis failures.

## Note on ordering

This is the same shape of work as
[0006](0006-move-analysis-is-five-approximations-of-one-relation.md) — replace
several local guesses with one solved relation — but it is **cheaper**, because
the call graph already exists and no CFG or binding identity is required. It is
the better first demonstration that routing an analysis end to end works.
