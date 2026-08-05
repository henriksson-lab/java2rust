# 0038 — A two-dimensional object array is declared, built, written and read as three different carriers

Status: **validated** (runtime-confirmed; the output does not compile)
Severity: high — the generated code is rejected by `rustc`, and the two
allocation forms of the same Java type disagree with each other
Kind: carrier selection — one Java type, four sites, three answers

Found while fixing the ragged residual of
[0026](0026-assignment-evaluation-order-differs-from-java.md). It is not an
evaluation-order defect and does not belong there, but it **blocks** a
differential fixture for that residual, which is why it is filed rather than
noted in passing.

## Symptom

```java
public class Ragged {
    public static String run() {
        String[][] g = new String[2][2];   // or: new String[2][]
        g[0][1] = "v";
        return g[0][1];
    }
}
```

Emitted, for `new String[2][2]`:

```rust
let mut g: Vec<Vec<String>> = ndarray::Array2::from_elem((2 as usize, 2 as usize), String::new());
//         ^^^^^^^^^^^^^^^ declared as nested Vec        ^^^^^^^^^^^^^^ built as ndarray
g[__row][__col] = …;          // written with nested indexing
return &g[[0 as usize, 1 as usize]];   // read with ndarray indexing
```

Four sites, three different carriers. `rustc` rejects it — and it would be
rejected whichever carrier were right, because they disagree.

The two Java allocation forms of the *same declared type* also disagree with
each other:

| allocation | emitted initialiser |
|---|---|
| `new String[2][2]` | `ndarray::Array2::from_elem(…)` |
| `new String[2][]` | `vec![Vec::new(); 2]` |

So `String[][] g` is a `Vec<Vec<String>>` or an `ndarray::Array2` depending on
how it happened to be allocated, while its *type* says one thing and its reads
say another.

## What is already right

`ndarray_array2_element_type` (`src/types.rs:359`) is explicit that the ndarray
carrier is for **primitive** elements:

```rust
let element_ty = two_dimensional_array_element_type(java_ty)?;
matches!(element_ty, JavaType::Primitive(_)).then_some(element_ty)
```

`String` is not a primitive, so this correctly answers `None`. The write path
consults it and correctly emits nested indexing. So the *policy* exists and is
right; two of the four sites do not consult it.

- **read**: `lower_array_access_expr` (~`src/lower.rs:22411`) does consult it,
  but reaches `array2_index_projection` anyway, so it is arriving through a
  second route — the guard beside it,
  `two_dimensional_array_expr_uses_nested_slice`, only recognises `static final`
  const arrays and answers `false` for a local.
- **construction**: `new String[2][2]` selects `ndarray::Array2::from_elem`
  without asking, while `new String[2][]` does not.

## Why it matters beyond this shape

`int[][]` is unaffected — ndarray is the right carrier there and all four sites
agree. The defect is confined to *object* two-dimensional arrays, which is a
common shape (`String[][]`, `Object[][]`, any `T[][]` of a source type), and it
is a **compile error**, not a silent wrong answer. That makes it cheap to detect
and is why it has presumably been masked: a corpus file containing it fails
loudly and gets counted as one more `E0308`/`E0277` among many.

## Fix direction

One predicate, consulted by all four sites. `ndarray_array2_element_type` is
already that predicate; the work is:

1. **Construction** asks it, so `new String[2][2]` builds
   `vec![vec![String::new(); 2]; 2]` — matching what `new String[2][]` already
   does, and matching the declared type.
2. **Read** asks the same predicate rather than
   `two_dimensional_array_expr_uses_nested_slice`, whose `static final`-only
   scope is the actual bug at that site. That helper should either be widened to
   the general question or deleted in favour of the predicate.
3. A test that the declared type, the initialiser, the write projection and the
   read projection agree — the same "one decision, one place" shape as
   `src/output_markers.rs`, and the reason this is worth doing once rather than
   at each site.

## Regression test worth adding

Cannot be a differential fixture until the fix lands, because the output does
not compile. Once it does:

```java
String[][] a = new String[2][2];
String[][] b = new String[2][];  b[0] = new String[2];  b[1] = new String[2];
a[0][1] = "x";  b[1][0] = "y";
return a[0][1] + "," + b[1][0];   // "x,y"
```

Both allocation forms, both written and read, so the four sites are forced to
agree.

## Relationship to other findings

- Blocks the differential fixture for
  [0026](0026-assignment-evaluation-order-differs-from-java.md)'s ragged
  multi-level residual. That residual's *ordering* is fixed and pinned by a text
  assertion; it cannot be pinned against the Java oracle until this compiles.
- Same shape as
  [0009](0009-overload-identity-collapses-onto-rust-name.md) and
  [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md):
  **one decision taken independently at several sites**, which disagree. The
  predicate here already exists, which makes this the cheapest instance of that
  pattern currently open.
