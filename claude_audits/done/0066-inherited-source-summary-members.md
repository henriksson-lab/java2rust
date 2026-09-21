# 0066 — Inherited source-summary methods were omitted from emitted subtypes

Status: **fixed** — an emitted primary type now receives the used instance
method surfaces declared by its non-emitted source-summary supertypes.

## Reproducer

`SummaryOverloadChild extends SummaryOverloadBase`, where the summary-only base
declares `convert(int)` and `convert(String)`, translated calls to
`self.convert_int(...)` and `self.convert_string(...)` but emitted neither
method on `SummaryOverloadChild`. The generated crate consequently failed with
two `E0599` missing-method errors.

## Root cause and fix

`ExternalApiFacadeSupport::add_emitted_source_summary_super_method_surfaces_for_type`
discarded every inherited summary method through
`source_summary_inherited_method_will_be_lowered`. That predicate searched the
same summary hierarchy currently being visited, so it was true for every
candidate. Its assumption was false: ordinary source lowering does not emit
members from the separate summary program.

The redundant skip and predicate are removed. Existing signature de-duplication
still prevents a primary/source-hierarchy method from being emitted twice.

## Verification

`audit_0066_inherited_source_summary_overloads_keep_distinct_surfaces` checks
both signature-qualified surfaces, both call sites, absence of
`compile_error!`, and compiles the generated Rust crate.

