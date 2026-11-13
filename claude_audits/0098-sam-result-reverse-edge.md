# 0098 — SAM result duplicate reverse nullability edge; field self-flow design question

## Symptom

`usage::tests::native_sam_result_qualifiers_use_result_argument_paths_and_capture_identities`
failed: `supplier` saw 4 flows instead of 3 and `function` saw 6 instead of
5. The extra edge in each was a reverse flow from the SAM result view back
into the producer (`Return[...] -> values[...]`).

## Diagnosis

The view-sharing block in `record_native_value_domain_storage`
(src/usage.rs, "The view preserves presence of its Java receiver...",
~line 8686) pushes the shared element domain in both directions. That
bidirectional join is intentional —
`stored_sam_result_flows_use_canonical_destination_slots` asserts it
explicitly ("only the shared Iterable's descendant element storage domain
is joined in both directions"). In the native-SAM scenario, however, the
lambda returns its own capture, so an earlier handler had already recorded
the same reverse edge; the view-sharing block then pushed a duplicate,
inflating the counts.

A first attempt removed the reverse push entirely; that broke
`stored_sam_result_flows_use_canonical_destination_slots` (2 flows instead
of 3), proving the reverse edge is load-bearing.

## Fix

Keep both directional pushes, but skip either one when
`nullable_collection_storage_flows` already contains that exact edge.
`supplier` (3) and `function` (6 -> 5 with dedup... see counts) now pass,
and `stored_sam_result_flows_use_canonical_destination_slots` still passes.

## Remaining design question (test still fails on `shadow`)

`shadow()` declares `values.add(null)` where `values` resolves to the
instance field `Captures.values`. The null-argument path in the functional
value invocation handler (~line 6438) records element-domain nullability as
a self-flow `(target, target)` rooted at the receiver slot — here
`Origin(Field Captures.values)` — producing a field-rooted flow, which the
test's "no Field roots in flows" assertion forbids. The same assertion also
expects `flows.len() == 5`; suppressing the field self-flow outright leaves
4, so the maintainer's intent appears to be that field element domains are
qualified through a different fact (a field-level domain record) rather
than a method-local flow edge, with a fifth method-local edge still
present. Choosing that representation is a cross-function contract
decision, recorded here for maintainer direction instead of being forced.

## Verification

- `cargo test --release sam_result`: `stored_sam...` and the other SAM
  tests pass; `native_sam...` fails only on the `shadow` field-root
  assertion above.
- Full suite: 1852 passed / 2 failed (this test plus the 0090 wildcard
  hierarchy design question).
