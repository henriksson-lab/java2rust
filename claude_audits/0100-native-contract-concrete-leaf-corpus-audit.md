# 0100 — Corpus feasibility: native-contract planning failures (Santuario 100%, BaseX query-expr partial)

## Symptom

Batch drafts for `xml-security` (410/410 files) and part of
`xquery/query-expr` (166/165 files at the time of writing) emit as
`compile_error!` stubs: native-contract planning rejects required slots, so
no Rust body is generated. 575 stub files carry ~104k diagnostics:

- `native_contract_non_native_position` (~103.6k): "allocation evidence
  addresses a concrete nonnative leaf, not a supported native outer
  allocation or intrinsic reference environment" (plan_contracts.rs:296-303)
- `native_contract_invalid_path` (~410): "structural native allocation path
  does not exist" (plan_contracts.rs:271-282)

Each stub embeds the program-wide unresolved requirement set, not only its
own class's; the distinct root-cause count is much smaller than the
diagnostic count.

## Distribution by slot node kind

- MethodReturn 49665, MethodLocal 38625, ConstructorParam 8250,
  MethodParam 4615, Field 3050, ConstructorLocal 410.

## Diagnosis (entry gate per repo rules)

The check at plan_contracts.rs:283-295 accepts a required leaf only when
its resolved type is a native outer-allocation carrier
(`native_allocation_has_outer_carrier`), a native functional carrier, or a
native cursor element. The failing corpora accumulate allocation evidence
(sharing/mutation through DOM trees, canonicalization contexts, XQuery
expression graphs) on leaves whose resolved types are ordinary concrete
Java types — W3C DOM (`org.w3c.dom.*`), JDK, and source types that were
never assigned a native carrier. Representative owners in one stub:
`DOMXMLObject`, `CanonicalizerBase`, `AbstractOutputProcessor`,
`XMLUtils`, `AbstractSecurityContextImpl` — i.e. DOM-centric code whose
shared mutable nodes map to plain concrete types.

This is a cross-cutting representation/framework question (which external
and source types gain native carriers, and how evidence on concrete leaves
is canonicalized), not a local emission fix. Options to surface:

1. Extend the native-carrier inventory for the W3C DOM / JDK facade types
   these corpora share mutable state through.
2. Revisit whether "concrete nonnative leaf" requirements should degrade
   to per-file drafts with the offending slots opaque, instead of failing
   the whole file's emission.

## State

Evidence-gathering audit only; no code change. Needs a representative
minimal reproduction next (smallest stub: xml-security
`00409_ResolverLocalFilesystem.rs`, 15 KB), then a maintainer-direction
decision between options 1 and 2 before a fix campaign.
