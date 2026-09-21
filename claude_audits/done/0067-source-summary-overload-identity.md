# 0067 — Source-summary overload identity across callable kinds

Status: **verified clean** — source-summary constructors, instance methods and
static methods retain distinct overload identities and emitted Rust surfaces.

## Probe

A summary-only `SummaryFactory` declares `int` and `String` overloads for its
constructor, instance `pick`, and static `select`. The primary source invokes
all six callables.

The translator emits `new_int`/`new_string`, `pick_int`/`pick_string`, and
`select_int`/`select_string`, retargets the corresponding calls, emits no
`compile_error!`, and the generated crate compiles. No product change was
needed for this scope.

## Verification

`audit_0067_source_summary_overload_identity_covers_all_callable_kinds` is the
permanent generated-crate gate.

