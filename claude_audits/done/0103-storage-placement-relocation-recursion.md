# 0103 — Unbounded recursion in storage placement relocation aborts batch emission (stack overflow)

## Symptom

`translate-batch` aborts with `fatal runtime error: stack overflow` on the
emit workers (`java2rust-emit-*`) for a 44-file chunk (aborts
after 2 emitted). Raising thread stacks (`RUST_MIN_STACK=64MiB`) does
not help, which rules out merely-deep recursion and indicates non-termination.

Single-file translation of every file in that chunk succeeds, so the cycle
depends on cross-file slot planning evidence assembled in batch mode.

## Evidence

Bisection of the chunk to a minimal crashing set identifies a single
574-line configuration class as the culprit:
`translate-batch --primary <chunk> --primary-file <that file>` reproduces the abort in seconds.

gdb backtrace at the SIGSEGV (emit worker thread) shows pure self-recursion
in slot rendering — 49 of 60 sampled frames are the same address inside:

    slot_render::storage_placement_construction_expr
      (relocate_result_projection_inside_storage inlined)

with `RustExpr::clone` / `Vec<String>::clone` between levels. The mutual
descent `storage_placement_construction_expr` ->
`relocate_result_projection_inside_storage` ->
`storage_placement_construction_expr` (src/slot_render.rs:1594-1668) has no
progress guarantee: each relocation round strips the handle/cell wrappers
from a `Try(...)` payload and re-enters full construction, but when the
payload expression produced by whole-batch slot planning already satisfies
the wrapper shape in a form `storage_wrapper_new_call_inner` re-matches
after re-wrapping, construction and relocation undo each other's work
forever.

## Fix

Give the relocation descent explicit fuel:
`storage_placement_construction_expr` delegates to a fuelled helper
(initial fuel 8, far beyond the one handle+cell relocation round the
doc-comment describes as intended) and
`relocate_result_projection_inside_storage` takes the remaining fuel; at
fuel 0 the expression is returned unrelocated instead of recursing.
Behaviour for all terminating inputs is unchanged; cyclic inputs keep the
outer `?` projection (still compilable-shaped draft output) instead of
aborting the whole batch.

## State

Implemented. Regression coverage: the minimal repro above now
completes; full `cargo test --release` rerun recorded in the batch
verification note.
