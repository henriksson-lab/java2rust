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
(see [0008](../0008-inference-machinery-does-not-reach-lowering.md)).

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


## Update 2026-08-01 — call-site `?` now reads the solved plan directly

Override equalization alone left the call site guessing. `source_method_call_returns_result_with_exception_plan`
identified the callee through three *name-based probes* and re-found the
declaration with `find_context_type_decl_by_erased_name`, so a call could
disagree with the signature whenever that lookup missed.

For a resolved call it now builds the callee's `CallableKey` from
`resolved.declaring_type` + `resolved.rust_name` and queries
`callable_returns_result` directly — the same key signature planning uses, so
the two cannot disagree. The name probes remain only for calls resolution could
not pin down.

Still open: those fallback probes are still name-based, and they answer "not
fallible" when they fail to find a declaration, which is the unsound direction.
Making them conservative needs the overload-identity work in
[0009](../0009-overload-identity-collapses-onto-rust-name.md).

## Update — legacy probes centralized, but not retired

`source_method_call_candidate_callees` centralizes candidate discovery and asks
the solved plan first. That is progress, but this audit is **not closed**:
`source_method_call_returns_result_with_exception_plan` still falls back to
`source_method_named_returns_result_with_exception_plan`, external call paths
still have direct fact probes in `call_plan`, and integer `/`/`%` fallibility is
seeded for any callable containing the expression even when a local catch handles
it. The focused tests compile, but the implementation is still not one
authoritative fallibility relation.

Status: **partial**. Finish by making every emitted `?` and every Result
signature decision read one solved callable relation, or emit a divergence
diagnostic.

---

## Update 2026-08-03 — one store, and the invariant is finally checked

Status: **closed for the source path.** One producer of `MethodExceptionPlan`,
the override-set equalization actually works, and the invariant this file has
asked for since it was filed exists and is exercised.

### The second store was already dead, and deleting it exposed the real defect

`plan_program_exceptions` had **no non-test caller**; `plan_method_exceptions`
was reachable only through it. Both are deleted and the fallibility plan is no
longer `Option` on the way in, so `plan_program_exceptions_with_fallibility` is
the only producer.

That left one genuine second store: `trait_method_exception_plan`
(`representation_lowering.rs`) joined `wrap_returns_in_result` over every
implementor with `|=`, re-doing the override equalization the fixpoint already
performs. Removing it should have been a no-op. Working out **why it was
load-bearing** is the finding of this pass:

> `override_groups_for_program` built its groups from `method.name` — the **Java**
> name — while every other `CallableKey::Method` in the program is built from
> `source_method_rust_name`. The two spellings coincide only for a method that
> is neither camelCased nor overloaded, so for `cumulativeProbability` (→
> `cumulative_probability`) and for every overload the equalization looked up
> keys that were not in the plan and silently did nothing.

That is [0009](../0009-overload-identity-collapses-onto-rust-name.md)'s thesis one
layer down: identity spelled two ways. With the group keys agreeing, the
fixpoint's equalization takes effect, and `trait_method_exception_plan` is a
plain lookup. All thirteen suites hold their numbers with the `|=` gone —
verify-then-delete, and this time the first hypothesis was right *because* the
cause was found first.

### The `?` invariant

`src/fallibility_invariant.rs` checks the property directly:

> for every call edge that is not a handled boundary, if the callee returns
> `Result` then the caller must too.

Reported as `fallibility_invariant` from `lower_program_*`, so a violation is a
named analysis failure rather than two anonymous `rustc` errors per call site.

**Measured at 0 violations across 120 corpus files.** Three unit tests pin that
the check can actually fire (a fallible callee in an infallible caller *is*
reported), that a fallible caller is not, and that a locally handled boundary is
not — because a detector nobody has seen fire is the failure mode
[0018](../0018-the-compile-error-metric-cannot-see-silent-failures.md) is about,
and two of `count_invalid_patterns`'s clauses had exactly that shape.

### What is still open

`call_plan.rs` retains four direct `facts.method_has_thrown_exceptions` probes
(`:1549`, `:1555`, `:1561`, `:1567`) on the external-call path.
`plan_call_exception_propagation` does not receive the solved plan, so threading
it through is a signature change across that module's callers. Those probes can
still answer "propagate" independently of the plan; the invariant above does not
cover them, because it works on call-graph edges rather than on the call-site
decision. **That is the remaining work for this issue**, and it is now the only
one.

## Update 2026-08-03 (second pass) — closed

The residual noted above is gone. `plan_call_exception_propagation_with_external_index`
is split in two:

- `plan_external_call_exception_propagation` — external-call logic only, **no
  source-method probe**. This is what lowering calls.
- `plan_call_exception_propagation_with_external_index` — keeps the four
  `method_has_thrown_exceptions` probes, for callers that have no solved plan.
  Its only remaining callers are tests, which pin that contract.

So a probe can no longer contradict the solver, and it is structural rather than
incidental: `lower.rs` does not name the probing function at all, which a test
asserts.

**Measured first, as this file's own advice requires.** Before splitting, the
probe block was instrumented and run over 100 corpus files: it fired **0 times**
once the solved plan had been consulted. The split turns that measured
unreachability into a guarantee rather than relying on it.

Fallibility now has one writer (the call-graph fixpoint), one store
(`MethodExceptionPlan`, derived from it), one call-site decision (the solved
plan, via `CallableAnalysis`), and one check that they agree
(`src/fallibility_invariant.rs`, 0 violations across the corpus).
