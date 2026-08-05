# 0030 — Java `static` fields are lowered to thread-local storage

Status: **validated by inspection**
Severity: high **given that concurrency is in scope**
Kind: storage-model mismatch

## Symptom

Every static field becomes a `thread_local!`:

```rust
std::thread_local! {
    static R3_SHARED: std::cell::RefCell<Option<i32>> = const { std::cell::RefCell::new(None) };
}

pub fn __static_shared() -> i32 {
    R3_SHARED.with(|cell| cell.borrow().clone().unwrap_or_else(|| 0))
}
```

A Java `static` field is **one variable shared by all threads**. A
`thread_local!` gives every thread its own independent copy, initialised
separately.

## Why this is now a defect

Under a single-threaded target this is harmless and arguably a reasonable choice
— it sidesteps `Sync` bounds and interior-mutability plumbing entirely.

Project decision (2026-07-30): **concurrency is in scope** ("yes, if possible").
That makes the divergence real, and it is the silent kind:

- a shared counter, cache, or registry silently becomes per-thread — no error,
  no warning, wrong results;
- lazily initialised singletons are constructed once **per thread**, so any
  identity or "initialise exactly once" assumption breaks;
- the `unwrap_or_else(|| …)` lazy initialisation runs per thread, which compounds
  [0015](done/0015-static-initializer-blocks-are-silently-dropped.md) — class
  initialisation is supposed to happen once per class, not once per class per
  thread.

Evidence that this matters for the current corpora: **15 FastQC source files**
use `synchronized`, `implements Runnable`, `extends Thread`, or
`ExecutorService`. Concurrency is already present in the inputs.

## Fix direction

Static storage should be process-wide with interior mutability:

| Java static | Rust |
|---|---|
| `static final` primitive/`String` | `const` / `static` (no lock needed) |
| `static final` immutable object | `static X: OnceLock<T>` |
| mutable `static` primitive | `static X: AtomicI32/I64/Bool` |
| mutable `static` object | `static X: OnceLock<Mutex<T>>` or `RwLock` |

Notes on getting this right:

1. **`static final` is the common case** and needs no synchronisation at all —
   handle it separately so the plumbing does not appear where it is unnecessary.
   Combined with [0024](done/0024-numeric-limit-constants-stubbed-to-zero.md), most
   statics in real code are constants.
2. **Class initialisation must be once per class**, which is what `OnceLock`
   gives — this is the natural home for the `__class_init` function proposed in
   [0015](done/0015-static-initializer-blocks-are-silently-dropped.md). The two issues
   should be fixed together; doing 0015 on the current thread-local storage would
   bake in the per-thread error.
3. **`Sync` propagates.** A `static Mutex<T>` requires `T: Send`, which pushes
   `Send`/`Sync` requirements through the object graph and interacts with the
   representation choice in [0002](0002-object-aliasing-has-no-representation.md)
   — see `CONCURRENCY-SCOPE.md`.
4. Writes must go through the setter path, which is separately broken —
   [0029](done/0029-static-field-mutation-emits-invalid-rust.md).

## Sequencing

This is a prerequisite for meaningful concurrency support and a blocker on
doing [0015](done/0015-static-initializer-blocks-are-silently-dropped.md) properly.
It is also cheap for the `static final` majority, so it can be split:

- **0030a**: `static final` → `const`/`static`/`OnceLock`. Small, no locking.
- **0030b**: mutable statics → atomics / `OnceLock<Mutex<_>>`, together with
  0015's `__class_init` and 0029's setter routing.

## Regression test worth adding

Requires a threaded test, which the harness may not have yet:

```java
static int counter = 0;
// two threads each increment 1000 times; total must be 2000, not 1000 per thread
```

Until threaded tests exist, a weaker but useful check: assert that no
`thread_local!` appears in generated output for a `static` field.

---

## Update 2026-08-05 — fixed

Static storage is now process-wide. `thread_local!` appears nowhere in the
emitter.

### What changed

| | before | after |
|---|---|---|
| mutable static | `thread_local!` + `RefCell<Option<T>>` | `static … : std::sync::Mutex<Option<T>>` |
| class-init guard | per-thread flag | `AtomicBool` |
| getter | `.borrow().clone()` | `.lock()…clone()` |

The getter recovers from a poisoned lock
(`unwrap_or_else(|poisoned| poisoned.into_inner())`) rather than propagating the
panic. A Java static initialiser that throws leaves the class in an erroneous
state, but it does not make the *field* permanently unreadable, and propagating
poisoning would turn one failure into every subsequent access failing for an
unrelated reason.

### Both halves are pinned, because a single-threaded run cannot see the defect

This is the part worth stating: the bug is **invisible to behaviour** on one
thread. A per-thread copy and a shared variable are indistinguishable until a
second thread exists. So there are two tests, deliberately:

- `tests/differential/StaticSharing.java` — against the Java oracle, pins the
  observable behaviour the shared storage has to keep giving: `static final`
  reads, a mutable counter across calls, a lazily initialised value, and
  once-per-class initialisation. Both toolchains print `7,abc,2,99,1`.
- `static_storage_is_shared_across_threads_not_per_thread`
  (`tests/slot_shape_design.rs`) — pins the storage *identity*: no
  `thread_local!`, a `std::sync::Mutex`, and an `AtomicBool` init guard rather
  than a `Cell<bool>`. This is exactly the weaker check this file proposed while
  the harness has no threaded fixture, and it is the one that would catch a
  revert.

The threaded fixture this file asks for is still not written, and cannot be
until the translator supports `Thread`. That is a *coverage* gap, not an
unfixed defect: the identity check above fails for any per-thread storage,
which is the property the threaded test would be measuring.

### What this does not close

`Send`/`Sync` propagation through the object graph — item 3 of the sequencing
section — stays with [0002](0002-object-aliasing-has-no-representation.md) and
`CONCURRENCY-SCOPE.md`. A `static Mutex<T>` requires `T: Send`, and today the
types that reach static storage satisfy it; a shared representation using
`Rc<RefCell<_>>` would not. That is a constraint on 0002's design, recorded
there, not a remaining defect here.

Found on the way and filed separately:
[0037](0037-nested-class-outer-static-read-is-capture.md) — a nested class
reading an enclosing static was lowered as an outer-*instance* capture, which is
why the fixture for this issue had to route around that shape. Since fixed.
