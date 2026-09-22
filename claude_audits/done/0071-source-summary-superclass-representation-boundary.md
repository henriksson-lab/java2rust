# 0071 — Summary superclasses polluted emitted representation diagnostics

Status: **fixed** — a primary class extending a summary-only superclass is
reported as an erased framework boundary, not unsupported emitted inheritance.

## Root cause

Resolver benchmarking correctly derived an erased-boundary override, but then
planned object diagnostics over a context containing the full parsed summary
superclass. The general safeguard against replacing parsed source types
suppressed the override, leaving a false `unsupported_inheritance` diagnostic
on the primary class.

Representation diagnostics now remove only the superclass declarations named
by `SourceSummarySuperclassBoundaryPolicy`. Other summary declarations remain
visible, preserving object-safety diagnostics for summary interfaces and
nested self-parameter contracts.

## Verification

`primary_extends_source_summary_base_is_erased_boundary_not_inheritance_pressure`
passes alongside
`source_summary_object_safety_distinguishes_direct_and_nested_self_parameters`
and the full dependency-mode source-summary family.

