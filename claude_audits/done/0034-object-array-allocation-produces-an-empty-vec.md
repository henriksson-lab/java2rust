# 0034 — `new T[n]` for a reference element type allocates length 0

Status: **validated** (Java oracle vs. compiled translation, differential harness)
Severity: high — the program compiles, then panics on the first element access
Kind: missing lowering

## Symptom

`new Node[1]` lowers to an *empty* `Vec`, so every index into the array is out
of bounds. Primitive arrays are unaffected — `new int[2]` lowers correctly to
`vec![0; 2]`.

## Minimal repro

```java
public class ArrNew {
    static class Node { int v; }
    public static String run() {
        Node[] cell = new Node[1];
        cell[0] = new Node();
        cell[0].v = 4;
        int[] nums = new int[2];
        nums[1] = 3;
        return cell[0].v + "," + nums[1] + "," + cell.length;
    }
}
```

Java: `4,3,1`.

Emitted Rust (verified):

```rust
let mut cell: Vec<ArrNewNode> =
    /* unsupported Java array creation: Named { name: "ArrNew.Node", args: [] }[1] */ Vec::new();
cell[0 as usize] = ArrNewNode::new();   // panics: len is 0 but the index is 0
let mut nums: Vec<i32> = vec![0; 2 as usize];
```

Runtime: `index out of bounds: the len is 0 but the index is 0`.

## Root cause

`default_array_element_expr` (`src/array_lowering.rs:229`) returns a default
only for primitives and `String`:

```rust
JavaType::String => Some("String::new()"),
_ => None,
```

Everything else falls through to `java_array_creation_fallback_projection`,
which emits `Vec::new()` — the right *type*, the wrong *length*. Java allocates
`n` slots initialized to `null`; the translation allocates none.

## Why it was not caught

Two independent detectors both missed it:

1. **The corpus does not cover it.** Zero of 196 files under `examples/` reach
   the fallback. The idiom is common in real Java and absent here.
2. **The invalid-pattern metric did not match its own marker.**
   `count_invalid_patterns` (`src/main.rs:1006`) looks for
   `"unsupported Java array creation"`, but the marker read `"Java array
   creation"`. The line was emitted, counted as clean, and the translation
   reported no invalid patterns. That mismatch is fixed as part of filing this
   issue, so the fallback is now visible to the metric; the lowering itself is
   still missing.

This is the failure mode audit
[0018](../0018-the-compile-error-metric-cannot-see-silent-failures.md) is about,
with an extra turn of the screw: the guard that should have caught it was
looking for a string nothing produced.

## Proposed fix

Java's element value is `null`, which the translation has no representation
for, so there are two honest choices:

1. **`Vec<Option<T>>`** — faithful. A read of an unassigned element becomes
   `.expect("Java null array element")`, matching Java's `NullPointerException`.
   Costs a slot-shape change that ripples into indexing, assignment and
   `length`.
2. **`vec![T::default(); n]`** — length-correct, and the only divergence left is
   that reading an *unassigned* element yields a zero value where Java throws.
   Requires the element type to be `Default + Clone`, which source structs do
   not currently derive (`struct_derives_for_type` adds `Clone`, never
   `Default`), so it needs a derive obligation like the one
   [0002](../0002-object-aliasing-has-no-representation.md) needs for `Clone`.

Option 2 is the cheaper step and strictly better than today. Option 1 is the
correct end state and should be chosen if the array slot shape is being
revisited anyway.

Whichever is chosen, keep the fallback loud for element types that support
neither: an allocation with the wrong length must not lower silently.

## Regression fixture

`tests/differential/known-gaps/ObjectArray.java` reproduces it against the Java
oracle. Move it up one level as part of the fix.

---

## Update 2026-08-04 — fixed (option 2), with the residual divergence stated

Status: **fixed.** `tests/differential/ObjectArray.java` moved up a level and
runs in the differential gate; Java and the translation both print `4,3,1,ab`.

### What landed

The audit's **option 2**: `new T[n]` lowers to `vec![Default::default(); n]`
when `T` derives `Default`, so the allocation has the right length.

Getting there needed the derive obligation this file predicted, built the same
way [0002a](../0002-object-aliasing-has-no-representation.md) built the `Clone`
one — but obligation-driven rather than capability-driven, which matters:

- `src/array_default_obligations.rs` walks the program for `new T[…]` and
  records the element types. Only those types get `#[derive(Default)]`. Adding
  it to every struct that *could* support it would have churned every derive
  line in the corpus to serve a handful of allocation sites.
- `rust_type_supports_struct_derive_default` (`item_assembly.rs`) is the
  capability check, mirroring the `Clone` pair including its coinductive
  recursion guard, so a self-referential `Node { Node next; }` terminates.
- Lowering asks the **same** capability function before emitting
  `Default::default()`, so the derive and the allocation cannot disagree — the
  failure mode that would otherwise turn a loud fallback into a broken build.

One deliberate difference from the `Clone` walk: an **unknown** named type
answers `false` here, where `Clone` answers `true`. `Clone` is asked at a site
already calling `.clone()`, so a wrong "yes" costs a compile error that was
coming anyway. `Default` is asked speculatively, and a wrong "yes" would replace
a loud diagnostic with a broken build. Generic types and non-class kinds are
excluded for the same reason: no single zero value to derive.

### The residual divergence, stated rather than discovered

Reading an element that was never assigned yields a **zero value** where Java
throws `NullPointerException`:

```java
Node[] cell = new Node[1];
int v = cell[0].v;          // Java: NPE.  Translation: 0.
```

That is option 2's known cost and the reason option 1 (`Vec<Option<T>>`) is the
correct end state. It is strictly better than what it replaces — an allocation
of length 0, where *every* index panicked including the assignments — but it is
not faithful, and it should be revisited when the array slot shape is touched,
most naturally alongside [0002b](../0002b-shared-representation-design.md), which
is already changing how object slots are held.

### Element types that still take the fallback

An element type that is generic, not a plain class, or not declared in the
program keeps the `unsupported Java array creation` comment and `Vec::new()`.
That is the "keep the fallback loud" requirement above, and it is now genuinely
loud: the marker and its detector read one constant
(`src/output_markers.rs`), and `tests/output_markers.rs` fails if the lowering
stops emitting it.
