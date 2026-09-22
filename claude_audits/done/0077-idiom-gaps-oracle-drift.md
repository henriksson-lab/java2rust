# 0077 — The `idiom_gaps` failure oracle had drifted to empty

Status: **fixed** — the suite's current failures are again recorded by name,
and a packaged-source regression found during reconciliation is repaired.

## Findings

The checked failure-name file contained only its two comment lines even though
the suite had a large, stable failing population. That made the name oracle
incapable of distinguishing an existing gap from a new regression.

Re-running the suite also exposed a real analysis discontinuity. Usage facts
for a method in a packaged compilation unit were stored only under the
package-qualified Java owner, while declaration-side signature consumers query
with the `TypeDecl` owner (the package is held by `Program`). Those consumers
therefore fell back to conservative owned parameters. The usage store now
retains both identities for an ordinary packaged unit; co-translated
same-simple-name package peers remain qualified and do not acquire a collapsing
simple alias.

The failure-name oracle and all suite counts are regenerated from actual test
output. The reconciled suite is **787 passed / 163 failed**, with all 163 names
checked in. Those names are known translator gaps, not silently treated as
successes.

## Verification

- `signature::tests::external_receiver_constraints_keep_native_collection_mutable_borrow`
  now includes a package declaration and checks the solved mutable/shared
  borrow parameter shapes.
- `idiom_gaps::bulk_add_all_borrowed_generated_rust_preserves_behavior`
  compiles and runs the packaged generated Rust.
- `lib::tests::batch_analysis_keys_keep_same_simple_name_package_peers_distinct`
  preserves the ambiguity-refusing multi-package case.
- `scripts/gate.sh --record` rebuilds the count and failure-name baselines from
  the reconciled suites.
