# 0070 — Nullable source-summary returns lost their local `Option` shape

Status: **fixed** — nullable return facts from summary methods now reach local
shape planning through field receivers, without double wrapping and with one
required unwrap for a subsequent concrete receiver use.

## Root cause

The same duplicate summary/support owner lookup as 0069 prevented
`method_call_return_shape_from_receiver` from finding the summary declaration.
The facade correctly returned `Option<T>`, but the local plan remained `T`,
producing invalid Rust.

The authoritative-summary lookup restores the nullable shape. Summary surface
demand also recognizes the original Java method spelling in addition to snake
case and resolved Rust names, so camel-case follow-up calls such as `toFile()`
materialize their declared facade method.

## Verification

`source_summary_nullable_return_into_optional_local_does_not_double_wrap` and
`source_summary_nullable_return_into_concrete_local_unwraps_once` assert the
local shapes and compile generated crates.

