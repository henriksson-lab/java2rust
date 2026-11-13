# 0102 — Batch emission recomputes program-wide structures per file (performance, non-blocking)

## Symptom

Single-file translation of every file in one 182-file package is fast (182 files in 71s
total), but the `translate-batch` chunk over the same files needs 15+
minutes. Whole-tree chunks of 150-400 files take 10-25 minutes each on an
otherwise idle 16-core host. Correctness is unaffected; this is throughput.

## Evidence

gdb `thread apply all bt` on that chunk run (SIGINT during emission):
both `java2rust-emit-*` workers sit in program-wide recomputation per
emitted file:

- Thread A: `runtime_class::support` ->
  `FactSet::reachable_hierarchy_edges_for_type_with_stats` ->
  `hierarchy_reachability_domain` — rebuilds a `HashMap<String, ()>`
  reachability domain per type (called from
  `stdlib::java_class_support_item_with_classloader_shim` during
  `lower_program_...`).
- Thread B: `lower::collect_context_type_key_program_matches` ->
  `stdlib::canonical_external_receiver_type_key` — program-wide scan per
  lowered call expression.

Both are O(program) recomputations inside per-file (or per-expression)
lowering loops, giving super-linear chunk cost even though per-file
translation is fast.

## Proposed direction (not implemented)

Cache the hierarchy reachability domain and the context-type-key program
match index once per batch (the existing lookup-session TLS pattern in
`representation_lowering`/`effective_facts` is the natural home), instead
of rebuilding them inside emission loops.

## State

Implemented: `runtime_class::support` per-call hierarchy saturations now
share one memoized `HierarchyFactLookupIndex`, and
`FactSet::hierarchy_fact_type_names` is cached per (facts identity,
overrides length) with `hierarchy_reachability_domain` computing the count
from the cached set. Suite still 1852 passed / 2 failed.
That chunk: 18+ min before, 6m17s after the support()-index hoist. The
facts-name-set cache was verified by the suite (still 1852 passed / 2
failed); its separate timing run was invalidated by a scratch-dir
cleanup mid-run, so no separate number is claimed. The remaining emission
cost is the second hotspot
(`lower::collect_context_type_key_program_matches` /
`canonical_external_receiver_type_key` per lowered call), left for a later
batch.
