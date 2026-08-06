# 0040 — External type-name matching recomputes its alias lists on the hottest path

Status: **validated and fixed** (profiled, before/after measured)
Severity: medium — no wrong output, but it made the corpus loop and the test
suite several times slower than they needed to be
Kind: performance — a pure function recomputed inside a linear scan

Found by asking why three-line test inputs took 70–100 seconds.

## Symptom

A seven-method Java file with one context-summary source took **81 s** to
translate. The same file with no summary source took **3.3 s**.

Bisecting showed the cost was per *method* that referenced the summary type, not
per reference and not per member:

| input | time |
|---|---|
| 7 methods, 7 different constants | 81 s |
| 7 methods, all referencing the *same* constant | 70 s |
| 1 method, 7 references to one constant | 3.2 s |
| 1 method, 1 reference | 12.7 s |
| no summary source at all | 3.3 s |

Roughly 10 s per method, on inputs of a few lines.

## Root cause

`perf` put 38% of samples under one call chain:

```
lower_static_primitive_constant_for_slot
  → conversion_plan::actual_shape_for_slot_expr
    → expr_shape::infer_field_access_shape
      → member_access_plan::plan_member_access_with_facts
        → FactSet::external_library_pack_for_type
          → external_pack_has_member_for_type
            → facts::external_type_name_matches      ← 38%
```

Self-time leaders: `parser::to_snake_case` **14.6%**,
`stdlib::canonical_external_receiver_type_key` **13.8%**, and ~25% across
`malloc` / `_int_free` / `realloc`.

`external_type_name_matches(fact, query)` calls `external_type_name_aliases` on
**both** names. That function allocates a `Vec<String>`, calls
`canonical_external_receiver_type_key` and `to_snake_case` up to six times,
sorts and dedups — and it is invoked **once per external-pack member** during a
linear scan of the built-in packs.

So resolving a single field access rebuilt the same handful of alias lists
thousands of times. The names repeat constantly across the scan; almost all of
that work was recomputation of an answer already produced microseconds earlier.

## Fix

`external_type_name_aliases` is memoized. It is a **pure function of its
argument** — no facts, no program, no configuration — so a per-thread cache is
sound; a thread-local `HashMap<String, Vec<String>>` keeps it free of
synchronisation.

Deliberately not fixed here: the linear scan itself. Indexing the packs by type
name would be the structural answer and is a larger change; the memo removes the
recomputation without touching how the scan is organised, and the measurement
below says that was where the cost actually was.

## Measured, before and after

| workload | before | after |
|---|---:|---:|
| 7 methods + summary source | 78.7 s | **12.4 s** |
| 7 methods, same constant | 69.7 s | **11.1 s** |
| 1 method + summary source | 12.9 s | **2.4 s** |
| `static_final_constants_from_summary_sources…` (one test) | 85.6 s | **12.8 s** |
| `inherited_equals_on_a_source_class…` (one test) | 3.5 s | **0.89 s** |
| whole `slot_shape_design` suite (592 tests) | 58–83 s | **24.5 s** |

`scripts/gate.sh --all`: **zero deltas** on all 18 targets. A pure memo cannot
change output, and the gate confirms it did not.

## A change that was tried and reverted

The first hypothesis was the *other* expensive thing on that path:
`non_emitted_static_context_fallback_signature` runs `analyze_usage` plus a full
signature solve over an isolated program, inside a per-call-site loop, and the
result depends only on the type. A `ContextSignatureCache` was built for it.

It made **no difference** — 11.15 s with, 10.52 s without, i.e. noise, and if
anything slightly worse for the `Rc`/`RefCell` overhead. So it was reverted
rather than kept: a struct, a `LowerCtx` field and three call-site changes for
no measured gain is exactly the unjustified complexity this audit set keeps
objecting to elsewhere.

Recorded because the negative is useful: that solve is *not* hot, so anyone
optimising this path again can skip it.

## Why it was not noticed

Nothing was wrong with the output, so no gate, corpus counter or differential
fixture could see it. It showed up only as "why is this slow", which is the same
blind spot [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)
describes for correctness: a signal nothing measures is a signal nobody acts on.
The corpus runs in this session took 10+ minutes each largely because of it.
