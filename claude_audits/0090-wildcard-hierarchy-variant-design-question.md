# 0090 — Wildcard-parameterized hierarchy variant: skip or abort is a design question

Status: **audit** — root cause traced, candidate fix rejected on evidence,
design decision surfaced.

## Finding

Gate:
`hierarchy_products::tests::fixed_owned_products_are_planned_without_retagging_stored_values`
(one of the 13 named baseline failures measured 2026-11-13).

The fixture's `ProductReader<P>` hierarchy has three concrete variants, one of
them `StatelessProductReader extends ProductReader<ReaderProduct<?>>`. The
gate expects the product plan to keep the two concrete-product variants. The
observed plan has all three: the variant filter checks
`java_type_contains_synthesized_source_wildcard`, but the wildcard in the
`read` output shape is a raw `JavaType::Wildcard(None)` (instrumented:
`Named { name: "ReaderProduct", args: [Wildcard(None)] }`), never reified to
a `__JavaWildcardN` parameter, so the filter does not fire.

## Why the obvious fix is wrong

Rejecting the raw wildcard inside the per-variant shape loop hits the loop's
`return None`, which aborts the *entire root* (the planner's deliberate
all-or-nothing contract), leaving zero roots — worse than the defect. For the
gate to see two variants, `StatelessProductReader` must be excluded from the
`SourceHierarchyCarrier` variant list itself, or the shape loop needs a
skip-this-variant path distinct from abort-root. Both change the planner's
contract:

- Excluding a concrete subclass from the hierarchy carrier affects every
  other consumer of that variant list, not just product planning.
- Skipping one variant silently produces a non-uniform product adapter.

## Recommendation

Decide which contract is intended before fixing. A differential probe
(comparing dispatch through the carrier with the variant excluded) would show
whether exclusion is observable. Recorded here rather than guessed, per the
entry-gate rule.

## Verification

The attempted filter change was reverted; the gate remains in the known-failure
queue with this analysis attached.
