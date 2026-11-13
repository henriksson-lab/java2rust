# 0091 — Outer-handle companion oracle asserted a call that does not exist

Status: **fixed as an oracle defect.**

## Finding

Gate: `lower::tests::outer_handle_companion_passes_mutable_parameter_by_binding_name`
(one of the 13 named baseline failures measured 2026-11-13).

The probe declares `adjust` (mutates a field) and `adjustAmount` (does not)
and never calls either. The gate's third assertion required a call spelled
`.adjust_amount(amount)` or `::adjust_amount(&mut *..., amount)` — no such
call exists in the program, so the gate could never pass regardless of
translator behavior. The intent (the companion receives the mutable parameter
by binding name) is already exercised by what the translator emits:
`adjust_with_outer(..., mut amount: i32)` with the shadowing
`let mut amount = amount;` in every variant. The assertion now checks that
directly — companion signature plus the shadowing bindings — instead of an
absent call site. No translator change.

## Caveat

If the gate was reduced from a larger fixture that *did* contain a caller,
the call-shape check should be restored together with the missing source
lines; none of the current fixtures contain one.

## Verification

The gate passes. Full lib suite after 0086–0091: 1846 passed / 8 failed, no
new failure names.
