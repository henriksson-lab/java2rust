# 0039 — A `static` array field drops its initializer, and every write to it is silently discarded

Status: **validated** (runtime-confirmed; one half panics, the other half is silent)
Severity: **high** — a `static` array is a common shape, and the write half
computes wrong answers without failing
Kind: static storage — initializer not lowered, and the accessor returns a copy

Found while writing the differential fixture for
[0038](0038-two-dimensional-object-array-carrier-disagrees.md), which had used a
`static int[]` as a side-effect counter and failed for this reason instead.

## Symptom

```java
public class StaticArr {
    static int[] counter = new int[3];
    static int[] named = {7, 8};

    public static String run() {
        counter[0] = counter[0] + 1;
        return counter[0] + "," + counter.length + "," + named[1];
    }
}
```

Java prints `1,3,8`. The translation panics on the first index.

Emitted:

```rust
pub fn __static_counter() -> Vec<i32> {
    return STATIC_ARR_COUNTER.lock().unwrap_or_else(|p| p.into_inner())
        .clone().unwrap_or_else(|| Vec::new());
    //                                ^^^^^^^^^^ length 0, and `new int[3]` is gone
}

pub fn run() -> String {
    Self::__static_counter()[0 as usize] =
        (Self::__static_counter()[0 as usize]).wrapping_add(1);
    …
}
```

## Two independent defects, and the second is the dangerous one

### 1. The declared initializer is dropped

`new int[3]` and `{7, 8}` never reach the storage. The `unwrap_or_else` fallback
is `Vec::new()` — the right *type* and the wrong *length*, which is exactly the
shape [0034](done/0034-object-array-allocation-produces-an-empty-vec.md) fixed
for local `new T[n]`. It was not fixed for the static-storage path, which
computes its initial value separately
(`static_storage_initial_value_expr` and friends).

This half is **loud**: the first index panics.

### 2. Writes go into a temporary and are discarded

`__static_counter()` returns `Vec<i32>` **by value** — it `.clone()`s out of the
mutex. So

```rust
Self::__static_counter()[0 as usize] = …;
```

assigns into a temporary that is dropped at the end of the statement. Rust
accepts this: the call result is a place expression, so indexing and assigning
to it is legal and does nothing.

This half is **silent**. It compiles, it runs, it produces no diagnostic, and
the array simply never changes. Give the storage a correct initial length and
the program stops panicking and starts quietly computing the wrong answer —
which is worse, and is precisely the failure mode
[0018](0018-the-compile-error-metric-cannot-see-silent-failures.md) is about.

Note the *scalar* static path is unaffected: `counter = counter + 1` on a
`static int` routes through `__set_static_counter`, which writes back. It is only
element and field writes *through* the getter that vanish, because those are not
whole-value assignments and nothing routes them to a setter.

## Relationship to 0030

Not a regression from [0030](done/0030-java-statics-are-thread-local.md) — the
previous `thread_local!` + `RefCell` storage had the same by-value getter, so
the write was discarded then too, just per-thread. 0030 changed *where* the
storage lives, not how it is projected. But 0030's `.lock().clone()` makes the
copy explicit and easy to see, which is how this surfaced.

## Fix direction

The two halves are separable and the second should come first, because fixing
only the first turns a panic into a silent wrong answer.

1. **Route element and field writes through the storage.** A write whose *place*
   is rooted in a static-storage getter must not go through the by-value
   accessor. Either project the write through the guard —

   ```rust
   STATIC_ARR_COUNTER.lock().unwrap_or_else(…).as_mut().unwrap()[0] = …;
   ```

   — or read-modify-write through the existing setter. The guard form is better:
   it is one lock rather than two, and it is the same shape the *scalar* path
   already produces for a whole-value assignment.

   The design rule from [0002b](0002b-shared-representation-design.md) applies
   verbatim here: **never hold the guard across a call**, so the guard must be
   taken and dropped inside the one statement.

2. **Lower the declared initializer into the storage.** `new int[3]` should give
   `vec![0; 3]` and `{7, 8}` should give `vec![7, 8]`, using the same array
   creation path lowering already has, rather than a separate fallback. The
   `unwrap_or_else(|| Vec::new())` should then be unreachable for a field with
   an initializer, and should arguably be a `compile_error!` rather than an
   empty `Vec` where it is reachable — the general rule 0031 argues for.

3. Once both land, the class-initialization guard already added by 0015/0030
   makes the ordering right: the value is installed on first access.

## Regression test worth adding

A differential fixture, since both halves are observable:

```java
static int[] counter = new int[3];
static int[] named = {7, 8};
static void bump() { counter[0] = counter[0] + 1; }
// bump(); bump();  ->  counter[0] == 2, counter.length == 3, named[1] == 8
```

Two `bump()` calls matter: one write could be masked by a coincidence, two
cannot.

## Why it has not shown up in corpus error counts

It compiles. The panic needs the array to be *indexed*, and the silent half
needs the value to be *read back* — a corpus that translates and type-checks
without running sees neither, which is
[0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)'s argument
for differential testing.

---

## Update 2026-08-05 — fixed, write half first

Both halves are fixed, in the order this file argued for: the silent one first,
so that fixing the loud one could not turn a panic into a wrong answer.

### 1. Writes reach the storage

`lower_static_storage_element_assignment_stmt` handles `staticArray[i] = v`:

```rust
{
    let __java_static_index = 0 as usize;
    let __java_static_value = (Self::__static_counter()[0 as usize]).wrapping_add(1);
    STATIC_ARRAY_COUNTER.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_or_insert_with(|| vec![0; 3 as usize])[__java_static_index] = __java_static_value;
}
```

Three details that are load-bearing rather than stylistic:

- **The operands are bound before the guard is taken.** `counter[0] = counter[0] + 1`
  re-reads the same static, and a `Mutex` is not reentrant — taking the guard
  first would deadlock, which is a worse failure than the one being fixed. It
  also gives Java's evaluation order (target subexpressions, then value) for
  free, which is [0026](0026-assignment-evaluation-order-differs-from-java.md)'s
  rule.
- **The guard is dropped at the end of the statement**, never held across a call.
  That is the discipline [0002b](0002b-shared-representation-design.md) states
  for the shared representation, and it applies here for the same reason.
- **`get_or_insert_with`, not `as_mut().unwrap()`**, so a write that happens
  before any read still installs the declared initial value.

### 2. The declared initializer reaches the storage

`static_storage_initial_value_expr` had no arm for `ArrayCreation` or
`ArrayInitializer` at all. It now folds a constant length or constant element
list — reusing `source_constant_values`, the JLS 15.28 folder built for
[0031](done/0031-stubbed-constant-census.md) — and emits `vec![0; 3]` or
`vec![7, 8]`.

Deliberately narrow: this runs during item assembly, with no shape environment
to lower a general expression against, so anything that does not fold keeps the
old fallback. That leaves the defect in place for a non-constant static array
initializer, which is stated here rather than left to be discovered.

One wrinkle worth recording: `ConstantValue::rust_value_for` renders a Java
`String` constant as a `&'static str` literal, which is right for `pub const`
and wrong for a `Vec<String>` storage. The array path adds `.to_string()`
explicitly rather than changing the shared renderer, because the `pub const` use
is the more common one and is correct as it stands.

### Verification

`tests/differential/StaticArray.java`, against the Java oracle, both printing
`2,3,8,6,2,150`. It covers:

- `new int[3]` (constant length) and `{7, 8}` / `{"a", "b"}` (constant elements);
- **two** `bump()` calls, because a single write can be masked by a coincidence;
- `counter[counter[0]] = counter[0] + 4`, where index *and* value both read the
  static being written — the case that deadlocks if the guard is taken first;
- a `String[]` element write and a read-back through `.length()`.

`scripts/gate.sh --all`: zero deltas.

### Still open

Non-constant static array initializers, as above. `static int[] a = compute();`
keeps the empty-`Vec` fallback and will still panic on first index — loud, and
the same class as the rest of this file, but out of reach of a fold.
