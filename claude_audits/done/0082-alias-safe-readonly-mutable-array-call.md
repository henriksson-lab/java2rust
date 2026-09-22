# 0082 — One call could borrow an array as both readonly and mutable

Status: **fixed and regression-gated** — a readonly argument aliases through a
snapshot when the same call also passes its storage as mutable output.

## Finding

For calls equivalent to `transform(values, values)`, Rust cannot hold a shared
slice and mutable slice of the same vector for the call. The translator's
alias-aware argument lowering creates a cloned temporary for the readonly
argument, then passes `temp.as_slice()` alongside `values.as_mut_slice()`.

The audited test had partly drifted to an obsolete spelling and also inherited
the nullable-array storage contamination from 0080. Its assertions now require
the snapshot and its slice view, while the product fix keeps the caller's fresh
array as owned storage.

## Verification

`same_call_readonly_array_argument_snapshots_before_mutable_output_alias`
passes and compiles the generated crate. The full failure-name census removes
this case without adding a replacement failure.
