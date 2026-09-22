# 0072 — Summary classes inherited a recursive/unimplemented `toString`

Status: **fixed** — a declaration-only summary class with no declared
`toString()` uses deterministic Java Object-style `Class@0` display.

## Root cause

Generic external-surface synthesis interpreted a used inherited `toString()`
as an opaque external method and installed an `unimplemented!` inherent
method. Its generated `Display` implementation then called that method; without
the method it would recursively call Rust's `ToString` blanket implementation.

Source-summary declared methods are now tracked separately. Synthetic
`to_string` methods are removed from summary-declared facade shadows unless the
summary actually declares that callable. Their `Display` implementation uses
`Class@0`; an explicitly declared summary `toString()` retains its boundary
method instead.

## Verification

`source_summary_object_to_string_in_print_uses_java_object_stringification`
asserts the non-recursive fallback, rejects the unmodelled stub, and compiles
the generated crate.

