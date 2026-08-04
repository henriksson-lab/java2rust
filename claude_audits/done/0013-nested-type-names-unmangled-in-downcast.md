# 0013 — Nested Java type names are emitted unmangled in `downcast::<…>`

Status: **validated** (corpus: biojava-alignment)
Severity: low count, **fatal per file** — it is a parse error
Kind: emitter bug, mechanical fix

## Symptom

```
error: expected one of `,` or `>`, found `.`
   --> src/generated/00011_GuideTree.rs:519:52
    |
519 |  *self.get_child1().downcast::<GuideTree.Node>().expect("invalid Java cast").clear_visited();
    |                                          ^ expected one of `,` or `>`
```

The Java nested type `GuideTree.Node` is written straight into Rust type
position. Rust has no `.` in type paths, so the file fails to parse — and with
it, every other diagnostic in that file is lost (same leverage argument as
[0012](0012-unsupported-diagnostics-emit-invalid-rust.md)).

Multiple occurrences in `00011_GuideTree.rs` (lines 519, 520, …).

## Root cause

Nested types *are* mangled elsewhere — the corpus contains
`SharedNode`, `pub type Node = SharedNode;`, and similar, so the naming function
exists. The `downcast::<…>` emission path builds its type argument from the raw
Java type name and does not route through it.

## Fix

Find the `downcast::<` emission site (`src/coercion.rs` — it also appears in the
string check `rendered.contains(".downcast::<")`) and pass the type through the
same Java→Rust type-name mapping used for struct declarations, rather than
formatting the Java name directly.

Then look for sibling cases: any place that interpolates a Java type name into
Rust type position with `format!`. `instanceof`, `.expect("invalid Java cast")`
paths, and turbofish arguments are the likely neighbours.

## Prevention

The general defect is that Java type names reach the emitter as `String` and can
be interpolated anywhere without conversion. A `RustTypeName` newtype that can
only be constructed by the mapping function would make this class unrepresentable.
That is a small instance of the argument in
[0007](../0007-rust-ir-is-mostly-opaque-strings.md): the bug exists because the
type is `String`.

Cheap interim guard: assert in the emitter that no rendered type path contains
`.`, and add the parse check described in 0012.
