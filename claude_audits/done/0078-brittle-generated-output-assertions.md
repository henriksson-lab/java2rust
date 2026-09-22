# 0078 — Generated-output assertions encoded obsolete spellings

Status: **fixed for the audited set** — five assertions now test ABI,
structure, diagnostics, or runtime behavior instead of incidental rendering.

## Findings

Several `idiom_gaps` failures were not translator regressions. Their assertions
required an older textual spelling even though the current output was at least
as precise and its generated crate compiled:

- anonymous interface implementations now use a concrete generated adapter
  rather than the obsolete `dyn Trait::anonymous()` handoff;
- bounded generic impls may carry an additional `Any` capability bound;
- `BufferedReader.readLine()` return normalization may map the optional string
  to owned storage while preserving the public `Result<Option<String>, ...>`
  ABI;
- checked byte-array reads may insert `.copied()` between `get()` and the
  exception conversion;
- package-qualified summary service parameters are correctly borrowed after
  usage facts reach signature solving.

The assertions retain negative checks for invalid handoffs, missing exception
behavior, and collapsed package identities. Generated-crate or runtime checks
remain the semantic backstop.

## Verification

Focused tests for all five cases pass, including the generated-crate checks and
the runtime-backed bulk-collection case associated with 0077.

