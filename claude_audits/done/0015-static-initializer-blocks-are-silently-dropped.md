# 0015 — `static { … }` initializer blocks are silently dropped

Status: **FIXED** — was: validated (runtime-confirmed)
Severity: high — silent wrong values, no diagnostic
Kind: missing member kind


## Resolution

**FIXED (verified 2026-08-01).** `static_initializer` is parsed into
`TypeDecl::static_init_blocks` (carrying its member position), and a
`__java_class_init()` runs field initializers and static blocks in textual order
behind a once-guard. Static getters, explicit/default constructors, and static
methods trigger it. Focused parser and lowering regressions cover the parsed
member position, initializer order, and active-use triggers.

## Symptom

A class's static initializer block is not emitted anywhere. Static fields keep
their declared-initializer values as if the block never ran.

```java
public class StaticInit {
    static int counter = 0;
    static { counter = 5; }
    static int derived = counter * 2;
    public int run() { return derived; }
}
```

| | `run()` |
|---|---|
| Java | `10` |
| emitted Rust | `0` |

No diagnostic: `wc -c StaticInit.err` → `0`, and the value `5` appears nowhere
in the output.

## Emitted Rust

```rust
std::thread_local! {
    static STATIC_INIT_DERIVED: std::cell::RefCell<Option<i32>> = const { std::cell::RefCell::new(None) };
}

pub fn __static_counter() -> i32 {
    STATIC_INIT_COUNTER.with(|cell| cell.borrow().clone().unwrap_or_else(|| 0))
}

pub fn __static_derived() -> i32 {
    STATIC_INIT_DERIVED.with(|cell| cell.borrow().clone()
        .unwrap_or_else(|| Self::__static_counter() * 2))     // 0 * 2
}
```

The `static { counter = 5; }` block has no representation.

## Two defects, one visible

### 1. The block is dropped (the confirmed bug)

Java class initialisation (JLS 12.4.2) runs *all* static field initialisers and
`static` blocks **in textual order** at class-init time. Only the field
initialisers are modelled.

### 2. The lazy scheme cannot express ordering (latent)

Each static is evaluated lazily and independently on first read
(`unwrap_or_else(|| …)`). That coincides with Java only when initialisers are
pure and order-independent. It diverges whenever:

- a `static {}` block assigns a field (this bug);
- one initialiser has a side effect another observes;
- initialisers read each other and Java's textual order matters — Java reads a
  not-yet-initialised static as its *default* value, whereas the lazy scheme
  computes it on demand, which can even recurse.

Fixing only defect 1 by running the block on first access would still not
reproduce ordering.

## Fix direction

Model class initialisation explicitly rather than per-field lazily:

1. Represent `static { … }` as a member kind in `java_ir` (it currently has no
   variant), preserving its textual position among field initialisers.
2. Emit one `fn __class_init()` per type containing field initialisers and
   static blocks **in source order**, and drive it from a
   `std::sync::Once`/`OnceLock` (or the existing thread-local) triggered by the
   first static access or first instance construction.
3. Keep per-field storage as it is; only the *initialisation* moves into
   `__class_init`.

This also removes the recursion hazard in the current lazy scheme.

If full class-init ordering is too large a step right now, the **minimum
acceptable interim behaviour is a diagnostic**: a dropped `static {}` block must
produce a `compile_error!`, not silence. Same principle as
[0014](0014-labelled-statements-are-silently-deleted.md) — refusing to translate
is acceptable, silently omitting is not.

## Regression tests worth adding

```java
static int a = 1; static { a = 5; } static int b = a * 2;   // b == 10
static { x = 1; } static int x;                             // JLS: legal, x == 1
static int p = q; static int q = 3;                         // p == 0 (illegal forward read → default)
```

The third pins that lazy evaluation is *not* substituted for textual order.
