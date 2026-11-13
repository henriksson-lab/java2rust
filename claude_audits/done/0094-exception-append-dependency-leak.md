# 0094 — Exception support item must not depend on the append platform facade

## Symptom

`stdlib::stdlib_support_item_tests::checked_iterator_runtime_compiles_without_the_platform_facade`
fails: the harness compiles the exception support item plus the iterator
runtime alone, and rustc reports `JavaAppendValue`, `JavaUtf16` and
`JavaTextView` as not found.

## Evidence

`java_exception_support_item_with_facts` includes `src/java_exception.rs`
verbatim, and that template carries

```rust
impl JavaAppendValue for JavaException { ... }
impl JavaAppendValue for &JavaException { ... }
```

Those impls reference the string-append runtime (`JavaAppendValue`,
`JavaUtf16`, `JavaTextView`), which is emitted by a different support item
(`src/java_append_value.rs`). The gate's contract is exactly that the checked
iterator runtime plus the exception carrier compile without the platform
facade; the sibling harness
(`unknown_size_spliterator_and_linked_stream_lifecycle_keep_live_protocol`)
already had to stub the append traits locally, showing the dependency leak.

## Classification

Local emission fix (support-item modularization). The append impls for
`JavaException` belong with the append capability item, which already hosts
the analogous impls for every other runtime type (`JavaUtf16`, `JavaCodeUnit`,
`Vec<JavaCodeUnit>`, ...). Generated programs that use string append emit both
items, so moving the impls does not change any complete generated output; it
only makes the exception item self-contained.

## Fix

Move `impl JavaAppendValue for JavaException` and
`impl JavaAppendValue for &JavaException` from `src/java_exception.rs` into
`src/java_append_value.rs`.
