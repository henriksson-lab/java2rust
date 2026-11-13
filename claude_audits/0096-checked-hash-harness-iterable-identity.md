# 0096 — checked-hash harness missed the Iterable carrier's identity/Eq impls

## Symptom

`stdlib::external_facade_generic_carrier_tests::checked_java_hash_runtime_reuses_leaf_kernels_and_live_iteration`
fails to compile its generated fixture:

```
error[E0277]: the trait bound `Iterable<T0>: JavaReferenceIdentity` is not satisfied
  --> ...: self.erased_reference_with_source(self.clone(), encode)
```

## Evidence

The harness composes `runtime.rust` +
`external_facade_functional_closure_carrier_struct("Iterable")` + the checked
hash support items. The carrier-rendering function emits the struct and its
inherent methods only; the `JavaReferenceIdentity`/`PartialEq`/`Eq` impls are
emitted separately by `external_facade_eq_impl`, which the real assembly line
always pairs with the carrier (src/stdlib.rs, facade assembly `lines.push(
external_facade_eq_impl(...))` next to the carrier). The harness omitted that
line, so `erased_reference`'s `S: JavaReferenceIdentity` bound had no impl.

## Classification

Stale/incomplete test harness, same category as 0091 and the stubbed sibling
harness in `unknown_size_spliterator_and_linked_stream_lifecycle_keep_live_protocol`.
No translator change: production output already emits the pair together.

## Fix

Include `external_facade_eq_impl("Iterable", &no_inferred_arities())` in the
harness module so it mirrors the real assembly.
