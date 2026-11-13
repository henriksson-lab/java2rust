# 0088 — instanceof oracle rejected the arity-agnostic variant wildcard

Status: **fixed as an oracle defect** — `matches!` keeps the deliberate
`Variant(..)` pattern.

## Finding

Gate: `conversion_lowering::tests::cast_and_instanceof_application_stays_structured`
(one of the 13 named baseline failures measured 2026-11-13).

`closed_enum_variant_pattern_leaf` emits `RustPattern::TupleVariantWildcard`,
which renders `AnimalValue::Dog(..)` — arity-agnostic on purpose, so the same
pattern type serves single- and multi-field variants. The gate's expected
string still carried the single-field spelling `Dog(_)`. `Dog(..)` matches
single-field tuple variants, so behavior is unchanged; only the oracle was
stale. The assertion now expects the `..` spelling. No translator change.

## Verification

`cast_and_instanceof_application_stays_structured` passes. Full lib suite
after 0086–0088: 1844 passed / 10 failed, no new failure names.
