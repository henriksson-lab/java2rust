# 0097 — iterable-factories harness missed identity impls and erased-type imports

## Symptom

`stdlib::external_facade_generic_carrier_tests::native_iterable_factories_share_allocations_and_checked_protocol`
failed to compile its generated fixture: first
`Iterable<T0>: JavaReferenceIdentity` unsatisfied, then
`JavaErasedIterator`/`JavaErasedIterableView`/`JavaErasedValue`/
`JavaErasedIterableProjection`/`JavaReferenceArrayProjection`/
`JavaReferenceArrayView` not in scope.

## Evidence

Same harness-composition gap as 0096: the module paired only the carrier
struct text (`external_facade_functional_closure_carrier_struct("Iterable")`)
with a hand-maintained `use super::{...}` list. The real assembly always emits
`external_facade_eq_impl` beside the carrier, and the erased iterable/view
types live at the crate root of the translated runtime, which the selective
import list did not name.

## Classification

Stale/incomplete test harness. No translator change.

## Fix

Pair the carrier with `external_facade_eq_impl("Iterable",
&no_inferred_arities())` and add the erased iterable/reference-array types to
the module's import list, mirroring the production assembly.
