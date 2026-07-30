# 0029 — Mutating a static field emits invalid Rust

Status: **fixed locally**
Severity: medium-high — any `static` counter or accumulator fails to compile
Kind: lowering gap; small fix

## Symptom

```java
public class R3 {
    static int shared = 0;
    public void bump() { shared++; }
}
```

Emitted:

```rust
pub fn bump(&mut self) {
    { let __java_old_value = Self::__static_shared();
      Self::__static_shared() += 1;                     // assigning to a call result
      __java_old_value };
}
```

```
error[E0067]: invalid left-hand side of assignment
```

The static *accessor* pair is generated correctly —
`__static_shared()` / `__set_static_shared(value)` — but the increment lowering
treats the getter call as an assignable place instead of routing the write
through the setter.

## Fix

Route static writes through the setter. For `shared++` used as a statement:

```rust
Self::__set_static_shared(Self::__static_shared() + 1);
```

and where the pre-increment value is needed as an expression:

```rust
{ let __old = Self::__static_shared();
  Self::__set_static_shared(__old + 1);
  __old }
```

Applies to `++`/`--` (prefix and postfix) and to every compound assignment
(`+= -= *= …`) on a static field. The same substitution is needed for static
fields of *other* classes (`Other.count++`), not just `Self`.

Interacts with:

- [0027](0027-compound-assignment-implicit-narrowing-missing.md) — a narrow
  static (`static byte b; b += 300;`) needs both the setter routing **and** the
  narrowing cast;
- [0003](0003-java-integer-arithmetic-semantics-not-preserved.md) — the
  arithmetic should be wrapping;
- [0030](0030-java-statics-are-thread-local.md) — the storage this writes to is
  currently thread-local, which is a separate defect now that concurrency is in
  scope.

## Why it has not shown up in the corpus

`static` mutable counters are less common than instance state, and this fails
*loudly*, so it cannot hide behind a clean run — it would already be visible in
any corpus containing the pattern. Worth checking whether the corpora simply
lack mutable statics, since that would also mean [0030](0030-java-statics-are-thread-local.md)
is currently untested in practice.

## Regression tests worth adding

```java
static int n = 0;  n++;  ++n;  n += 5;  n--;      // 6 after all four
static byte b = 10; b += 300;                     // 54 (with 0027)
class Other { static int c; }  Other.c++;         // cross-class static write
```
