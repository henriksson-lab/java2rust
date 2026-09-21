# 0068 — Package-qualified identity across source-summary programs

Status: **verified clean** — equal simple type names in different summary
packages remain distinct through facade emission and primary-source lowering.

## Probe

Separate `left` and `right` summary programs each declare `Constants` and
`Service`. Their constants have different values and their services expose
different method signatures. A primary source consumes all four qualified
types.

The result contains distinct `LeftConstants`/`RightConstants` and
`LeftService`/`RightService` surfaces, selects constant values `11` and `22`,
keeps the parameter types distinct, emits no `compile_error!`, and compiles as
a generated Rust crate. No product change was needed for this scope.

## Verification

`audit_0068_package_qualified_source_summary_identity_stays_distinct` is the
permanent generated-crate gate.

