# 0012 — `compile_error!` diagnostics emit invalid Rust and destroy the rest of the file

Status: **validated** (corpus: biojava-alignment)
Severity: medium by count, **high by leverage** — a parse error kills every
other error in the file
Kind: emitter bug — trivial fix, disproportionate payoff

## Symptom

Unsupported-construct diagnostics are emitted as `compile_error!` with a message
built by `{:?}`-style debug formatting of an internal enum. The debug output
contains `"` characters, which are not escaped, so the emitted Rust does not
parse:

```
error: suffixes on string literals are invalid
   --> src/generated/00002_DemoAlignProteins.rs:241:176
241 | ...compile_error!("unsupported Java dispatch coercion: actual=ErasedObject,
       target=ClosedEnum { enum_name: "AlignmentsPairwiseSequenceAlignerType" }")...
    |                                                        invalid suffix `AlignmentsPairwiseSequenceAlignerType`

error: compile_error! takes 1 argument
   --> src/generated/00002_DemoAlignProteins.rs:241:358
```

The inner `"AlignmentsPairwiseSequenceAlignerType"` terminates the string
literal early. Everything after it is reparsed as garbage: the trailing text
becomes a "literal suffix", the closing `"` starts a *new* string, and the comma
inside `ClosedEnum { … }` makes `compile_error!` appear to take two arguments.

## Why it matters more than 3 errors suggest

A parse error is fatal for the whole file. Every genuine type error in
`00002_DemoAlignProteins.rs` is suppressed, so:

- the file's real problems are invisible until this is fixed;
- `top-compile-errors.tsv` under-reports whatever those problems are;
- the biojava error total (188) is not comparable with corpora that have no
  parse errors.

This is a diagnostic mechanism *hiding the diagnostics it exists to surface*.

## Fix

In whichever emitter formats these (search for `compile_error!` in
`src/unsupported.rs` / `src/emit.rs`):

1. Escape the message before interpolation — at minimum `\` and `"`; or
2. Render the shape description with a plain `Display` impl that emits no
   quotes, instead of `{:?}` on the internal enum; or
3. Emit the message as a raw string with a hash count chosen to exceed any run
   of `"#` in the payload.

Option 2 is preferable: these messages are read by people, and
`ClosedEnum { enum_name: "X" }` is debug noise. `closed enum X` conveys the same
thing and cannot break the parse.

Add a regression check that every generated file **parses**, separately from
whether it type-checks. A cheap version: run `rustc --edition 2021 -Zparse-only`
(or just count `error: expected`/`invalid suffix` in the log) and fail the batch
if any file fails to parse. Parse failures should be reported as a distinct
category in `summary.tsv`, because they invalidate the error counts for that
file.

## Related emitter bug

Same corpus, same class of problem — a Java nested type name is emitted verbatim
into Rust type position:

```
error: expected one of `,` or `>`, found `.`
   --> src/generated/00011_GuideTree.rs:519:52
519 | *self.get_child1().downcast::<GuideTree.Node>().expect("invalid Java cast")...
```

`GuideTree.Node` must be mangled (`GuideTreeNode`) as it is elsewhere; the
`downcast::<…>` path misses the mangling. Also fatal to the file's parse. See
[0013](0013-nested-type-names-unmangled-in-downcast.md).
