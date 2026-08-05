# 0026 — Rust evaluates an assignment's right side first; Java evaluates the target first

Status: **validated** (runtime-confirmed, compiles cleanly)
Severity: medium — silent wrong answer; narrow trigger but impossible to spot by reading
Kind: language-level evaluation-order mismatch — direct translation cannot preserve it

## Symptom

```java
public int arr(int[] a) { int k = 0; a[k++] = k; return a[0]; }
```

| | result |
|---|---|
| Java | `1` |
| emitted Rust | `0` |

Emitted, which looks like a faithful translation:

```rust
pub fn arr(&self, a: &mut [i32]) -> i32 {
    let mut k: i32 = 0;
    a[{ let __java_old_value = k; k += 1; __java_old_value } as usize] = k;
    return a[0 as usize];
}
```

## Root cause: the two languages specify opposite orders

- **Java** (JLS 15.26.1): for `arrayref[index] = rhs`, the array reference is
  evaluated, then the index, **then** the right-hand side. So `k++` runs first
  (index `0`, `k` becomes `1`), and the RHS `k` is then `1` → `a[0] = 1`.
- **Rust** (Reference, assignment expressions): "the assigned value operand is
  evaluated first, then the assignee expression". So the RHS `k` is read first
  (`0`), and only then the index expression increments `k` → `a[0] = 0`.

The generated code is a *syntactically* faithful translation and a
*semantically* wrong one. No amount of care at the expression level fixes this,
because the mismatch is in the surrounding assignment construct.

## Scope

Triggers whenever an assignment's **target subexpressions** have side effects, or
read state the RHS writes:

```java
a[i++] = i;
a[i] = i++;
m[f()] = g();          // if f and g interact
this.arr[idx()] = compute();
a[i] += f();           // compound: target read also participates
```

The plain cases (`a[i] = v`, `x = f()`) are unaffected, which is why this has not
shown up in corpus errors — it compiles, and most code does not write this.

Note it is *not* limited to `++`: any side-effecting index or qualifier is
exposed. Compound assignment (`+=`) has an additional subtlety because the target
is both read and written.

## Fix

Hoist the target's subexpressions into temporaries **before** evaluating the RHS,
whenever the target is not a simple local:

```rust
let __idx = { let old = k; k += 1; old } as usize;   // target first, Java order
let __val = k;                                       // then RHS
a[__idx] = __val;
```

Emit temporaries only when needed, to avoid noise:

- target is a plain local/field with no side-effecting subexpression → no change;
- otherwise, if the RHS reads any variable the target's subexpressions write (or
  either contains a call, `++`/`--`, or assignment) → hoist.

A conservative version — hoist whenever the target is an index or field access
whose subexpressions are not trivially pure — would be correct and only slightly
more verbose. Given [0016](done/0016-core-jdk-facades-are-no-op-stubs.md) and
[0024](done/0024-numeric-limit-constants-stubbed-to-zero.md) show how costly silent
wrongness is here, the conservative choice looks right, with the precise
side-effect test as a later refinement.

## Broader point

This is the first finding in this audit that is a genuine **language-semantics
mismatch** rather than a missing feature or a modelling gap. It is worth checking
for siblings systematically, because they share the property that the emitted
code reads correctly:

- argument evaluation order (Java: strictly left-to-right) — probed with
  `"" + bump() + bump() + bump()`, **correct** today;
- operand order in binary expressions — Java left-to-right;
- `&&`/`||` short-circuit with side-effecting operands;
- order of the target and value in compound assignment;
- array-store evaluation vs bounds check vs `ArrayStoreException` ordering.

## Regression tests worth adding

```java
int k=0; a[k++] = k;      return a[0];   // 1
int k=0; a[k] = k++;      return a[0];   // 0
int i=0; a[i++] += 5;                    // compound with side-effecting index
```

---

## Update 2026-08-05 — residual surfaces, probed

The one-dimensional array cases this file opens with are fixed and hoist their
index before the right-hand side. Probing the rest of the surface with a
seven-case fixture (`EvalOrder.java`, run through `javac` for the oracle) splits
the remainder into three groups, and only two of them are this issue's to fix.

| case | Java | translation | status |
|---|---|---|---|
| `a[k++] = k` | `a[0] == 1` | `1` | fixed |
| `c[i++] += 5` | `15` | `15` | fixed |
| `g.grid[f()][f()] = v` | `grid[0][1] == 2` | `2` | fixed |
| `b[m] = m++` | writes `b[0]` | **writes `b[1]`** | open — below |
| `arr[j++].field = j` | `arr[0].field == 1` | **`arr[0].field == 0`** | open — below |
| `pick(e).field = calls` | `e.field == 1` | `0`, and to a *clone* | blocked on 0002 |

### The RHS can invalidate a target index that has no side effect of its own

```java
int[] b = new int[3]; int m = 0;
b[m] = m++;             // Java: index 0, then RHS 0  ->  b[0] = 0
```

The hoist is currently gated on the *index* having a side effect. Here it does
not — the index is a bare local — and the **value** is what mutates it. Rust
evaluates the value first, so `m` is already `1` when the index is read and the
write lands on `b[1]`.

This is worth naming precisely because the observable symptom is so weak: `b[0]`
holds `0` either way, so a test that only reads the written-to slot passes. The
defect is that a *different element* was written.

The gate has to be "the index is not stable across evaluating the value", not
"the index has a side effect".

### A field-assignment place has target subexpressions too

```java
arr[j++].field = j;     // Java: index 0, then RHS 1  ->  arr[0].field = 1
```

Nothing about this is array-*assignment*; the assignment target is a field, and
the array access is a subexpression of the place. The ordered-assignment path
only recognises `Expr::ArrayAccess` targets, so this falls through to the plain
path and Rust's order applies.

### What is not this issue's to fix

`pick(e).field = calls` is wrong in two independent ways: the order, and the
fact that `pick` receives `e.clone()` so the write cannot be observed at all.
The second is [0002](0002-object-aliasing-has-no-representation.md), and fixing
the order alone would produce code that is still wrong — so this case belongs in
the aliasing fixture, not here. Stated so the next pass does not spend time on
it.

### Both residual surfaces fixed

**1. The value can invalidate a stable-looking index.** The hoist gate moved
from "the index has a side effect" to "the order is observable":
`assignment_order_is_observable(index, value, env)`.

The rule needed one refinement that the first attempt got wrong, and the
regression it caused is worth recording because it is the interesting part.
Gating on "the value has *any* order side effect" makes every `a[i] = f()`
hoist, since a call counts. That cost three `idiom_gaps` tests and would have
wrapped a large fraction of all array writes in a temporary for nothing.

The precise rule is that **a Java method cannot reach a caller's local**. So if
the index is a local, only an explicit write to that same name inside the value
can disturb it; if it is a field or an array element, a call *can* reach it and
the conservative rule stands. Telling the two apart needs the shape environment,
because a bare field reference and a local are the same `Expr::Var` in the IR.

**2. A field-assignment place has target subexpressions.**
`lower_ordered_field_place_assignment_stmt` handles
`FieldAccess(ArrayAccess(..), field)`. It hoists the index and then lowers the
place with the index replaced by the temporary's name, so the field projection —
optional carriers, reference storage and all — is the one the ordinary path
would have produced, rather than a second hand-built spelling of it.

### Verification

- `tests/differential/EvalOrder.java` against the Java oracle, both printing
  `1,0:0:0,15,1,1`. It reads the *whole* array for the second case, because
  `b[0]` reads `0` under either order and only the element written differs.
- `a_side_effecting_value_hoists_a_stable_looking_index`,
  `a_literal_index_is_not_hoisted_for_a_side_effecting_value` (the negative —
  no hoist where none is needed) and `a_field_place_hoists_its_receivers_array_index`
  in `tests/slot_shape_design.rs`.

### Still open

- **Multi-level ragged receivers.** `a[i()][j()] = v` on a `T[][]` that is not
  ndarray-backed. The ndarray-backed two-dimensional numeric case is handled;
  the ragged one is not.
- **Array-store exception ordering.** Java checks the store type after
  evaluating the value and before the store; there is no `ArrayStoreException`
  at all today, so the ordering question does not yet arise.
- **`pick(e).field = v`**, blocked on
  [0002](0002-object-aliasing-has-no-representation.md) as described above.
