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
