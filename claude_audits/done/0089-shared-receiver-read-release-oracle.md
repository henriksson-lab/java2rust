# 0089 — Shared-receiver read-release oracle named a superseded binding

Status: **fixed as an oracle defect, with a caveat on file.**

## Finding

Gate:
`lower::tests::shared_receiver_argument_call_releases_read_before_mutating_same_object`
(one of the 13 named baseline failures measured 2026-11-13).

The gate asserted the hoisted read result is bound as
`let __java_receiver_arg_0 = ...`. The translator now inlines `next.read()` to
a guarded field read bound as `__java_self_field_arg_0`, with the shared guard
dropped at the end of its own block *before* `write_with_outer` runs — the
semantic property the test exists to protect (no live read borrow across the
mutation) is intact. The remaining assertions (`.map(...)`,
`as Box<dyn Data>`, no `Option<Box<...>>` double wrap, no nested
`next.borrow()` inside the write call) already verify that property and still
pass unchanged. The first assertion now accepts either binding name with a
comment stating the semantic requirement.

## Caveat

Inlining `read()` to a direct field read bypasses virtual dispatch if a
subclass overrides `read()`. The probe class has no such subclass, so the
oracle change is safe here; whether the inliner proves no override exists (or
restricts itself to effectively-final receivers) was **not** established by
this audit and is a candidate probe family for a future numbered audit.

## Verification

The gate passes. Full lib suite after 0086–0089: 1845 passed / 9 failed, no
new failure names.
