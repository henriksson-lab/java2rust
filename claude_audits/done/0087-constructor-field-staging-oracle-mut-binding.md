# 0087 — Constructor field-staging oracle rejected the deliberate mutable binding

Status: **fixed as an oracle defect** — the staged carrier handle keeps its
documented `mut` capability.

## Finding

Gate: `lower::tests::optional_source_supertype_field_assignment_uses_carrier_handle`
(one of the 13 named failures in the 1841/12-failure baseline measured
2026-11-13; reduced to 12 by 0086 before this audit).

The translator stages constructor field initializers as
`let mut __java_initial_field_N = ...;` bindings. Both staging sites in the
constructor prefix pass emit `mutable: true` on purpose: later staged
statements may invoke mutable value-carrier methods on the binding before it
moves into the struct literal, and the assignment-path site carries an
explicit comment to that effect. The gate's assertion string still expected
the pre-mut spelling `let __java_initial_field_0 = ...`, so it failed while
the generated code was already correct — the same staleness family as 0085.

The assertion now matches the deliberate `let mut __java_initial_field_0`
spelling. No translator change.

## Verification

`optional_source_supertype_field_assignment_uses_carrier_handle` passes.
Full lib suite after 0086 + 0087: 1843 passed / 11 failed, no new failure
names.
