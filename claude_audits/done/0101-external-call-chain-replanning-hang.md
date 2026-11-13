# 0101 — Exponential external-call replanning on receiver chains

## Symptom

`new-java2rust` exceeds a 30s per-file timeout on a 422-line class that
makes many calls on external receivers, single-file and in batch; its batch
chunk worker sat at 36+ minutes CPU. Second hang family after 0099.

## Evidence

SIGINT backtrace under gdb: a mutual-recursion cycle in call_plan —
`external_receiver_expr_type_key_with_index` (call_plan.rs:3245) ->
`external_call_lowered_type_with_external_index` (1657) /
`external_call_lowered_type_from_static_owner_plan_with_external_index`
(1925) -> `plan_call_target_with_external_index_impl` (3846) ->
`external_method_rust_name_for_receiver` (7226) -> back to
`external_receiver_expr_type_key_with_index` on the next receiver level,
with `expr_shape::infer_expr_shape` re-entering the same cycle
(`native_java_call_expr_shape`). Hot leaf:
`java_naming::rust_name_for_java_named_type` via
`external_packs::external_receiver_lookup_aliases`.

## Diagnosis

Same shape as 0099, different subsystem: for a chained call expression of
depth n, planning the outer call re-plans the inner calls through several
independent paths per level (the lowered-type query at 3282, the static
owner plan at 3292, and the plan-target path through 7226), so cost is
exponential in chain depth. Every cycle passes through
`external_receiver_expr_type_key_with_index` keyed by
(receiver node, method name) — the recursion hub.

## Fix

Memoize `external_receiver_expr_type_key_with_index` with a thread-local
map keyed by (`receiver` pointer, method name), guarded by a depth counter
so the map lives only for the duration of the outermost planning query.
Planning takes `&Expr` everywhere (no AST mutation mid-query), and dropping
the map at depth 0 prevents pointer-aliasing staleness across queries, so
results are exactly what the uncached repeated evaluations would return.

## Verification

- The 422-line class translates in 18s.
- Full suite: 1852 passed / 2 failed (only the pre-existing 0090/0098 design questions).
- Four other files that hit the same scan timeout now finish in 16s to 53s. A sixth, the largest, needed the resolved_external_instance_call_rust_name per-call ExternalFactLookupIndex construction hoisted (with_index variant), then finishes in 75s.
