# 0099 — Exponential receiver re-inference on unresolved method chains (FnSystemProperties hang)

## Symptom

`new-java2rust` never finishes on BaseX
`org/basex/query/func/fn/FnSystemProperties.java` (50 lines), both in
single-file mode and inside `translate-batch` (the `query-func-fn` chunk
worker sat at 100% CPU for 22+ minutes; normal 250-file chunks finish in
1-3 minutes). The file's `value()` body is a 14-deep
`new MapBuilder().put(...).put(...)... .map()` chain on an external type.

## Evidence

SIGINT backtrace under gdb (release binary): an unbounded
`infer_resolver_expr_type_with_facts` <-> `source_receiver_type` /
`source_receiver_type_for_method` / `resolve_expr_calls` recursion cycle,
with `known_external_method_return_type_for_call` cloning `Vec<JavaType>` at
every level.

## Diagnosis

The chain stays unresolved (external `MapBuilder.put`), so every inference
of an outer call node re-infers the entire receiver subtree, and each
unresolved `Expr::Call` node triggers multiple full subtree inferences:

1. `infer_resolver_expr_type_with_facts` computes `receiver_type` once
   (~line 6392).
2. `external_enum_synthetic_call` (~6528) re-infers the receiver itself.
3. `source_receiver_type_for_method` (~6562) infers at its head, and every
   fallback re-infers again through `source_receiver_type`.

That is ~5x multiplication per chain level: O(5^n) for depth n. n=14 with
heavy stdlib lookups per level is effectively a hang. `resolve_expr_calls`
repeats the pattern per node (~5127 receiver inference, then
`source_receiver_type_for_method` at ~5342 and `external_enum_synthetic_call`
at ~5432 re-inferring).

## Fix

Thread the already-computed receiver actual type through instead of
recomputing it:

- New `source_receiver_type_with_actual(receiver, receiver_actual_ty, ...)`
  holds the post-inference body of `source_receiver_type` (Super handling,
  type-param-bound check, `source_receiver_type_from_actual`).
- `source_receiver_type` and `source_receiver_type_for_method` infer once
  and delegate; all their fallbacks reuse the same actual type.
- `external_enum_synthetic_call` now takes the receiver type as a parameter
  instead of calling `infer_resolver_expr_type_with_facts` itself.
- The unresolved-`Call` arm of `infer_resolver_expr_type_with_facts` passes
  its `receiver_type` (computed once) to both fallback sites; the `Expr::Call`
  arm of `resolve_expr_calls` likewise reuses its own `receiver_type`.

Semantics are unchanged: the receiver subtree is resolved bottom-up before
any of these inferences run within a node visit, and nothing between the
first inference and the fallbacks mutates the receiver, so the reused type
is exactly what the removed re-inferences would have returned.

## Verification

- `FnSystemProperties.java` translates in well under the previous 20s scan
  timeout, and the `query-func-fn` batch chunk completes.
- Full suite: 1852 passed / 2 failed (only the pre-existing 0090 and 0098 design-question failures; no regressions). FnSystemProperties.java translates in 28s single-file (was unbounded).
