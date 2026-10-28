# Concurrency: what "yes, if possible" costs

Project decision (2026-07-30): **multi-threaded Java is in scope, if possible.**

This note is the feasibility assessment that decision invites. Short version:
it is possible, but it is not an independent feature — it changes the *object
representation* decision that several other findings depend on, so it should be
settled before that representation is designed rather than after.

## It is already needed

15 FastQC source files use `synchronized`, `implements Runnable`,
`extends Thread`, or `ExecutorService`. This is not hypothetical for the current
corpora.

## Current state

| Java | current lowering | status |
|---|---|---|
| `synchronized` method | explicit unsupported diagnostic and generated `compile_error!` | monitor semantics not implemented |
| `synchronized (o) { … }` | explicit unsupported diagnostic and generated `compile_error!` | monitor semantics not implemented |
| mutable `static` field | process-wide `Mutex<Option<T>>` behind accessors | implemented; [0030](claude_audits/done/0030-java-statics-are-thread-local.md) |
| `volatile` | explicit unsupported diagnostic and generated `compile_error!` | atomic memory-order semantics not implemented |
| `AtomicInteger` | named in `stdlib.rs` | partially modelled |
| `wait`/`notify`, `ExecutorService` | not modelled | — |

Interestingly, `&mut self` for a `synchronized` method is not as wrong as it
looks: Rust's exclusivity rules already prevent concurrent `&mut` access. What
is missing is *cross-thread* mutual exclusion and the shared handle that would
make it necessary.

## The decision this forces

Nothing in Java's concurrency model can be expressed while objects are
represented by value. `synchronized (o)` locks *an object*, so `o` must be a
shared handle. That is [0002](claude_audits/0002-object-aliasing-has-no-representation.md)'s
missing shared representation — but concurrency changes **which** shared
representation:

| | single-threaded | concurrent |
|---|---|---|
| shared handle | `Rc<T>` | `Arc<T>` |
| shared mutable | `Rc<RefCell<T>>` | `Arc<Mutex<T>>` |
| static storage | process-wide `Mutex`/`AtomicBool` accessors | process-wide `OnceLock`/atomics |

**This matters now**: [0002](claude_audits/0002-object-aliasing-has-no-representation.md)
and [0023](claude_audits/0023-inner-class-construction-always-panics.md) both
currently propose `Rc<RefCell<T>>`, and the one existing `Rc<RefCell<…>>` in the
emitter (the inner-class `__outer` field) is already the single-threaded choice.
If concurrency is in scope, that should be revisited **before** the
representation work starts, not retrofitted.

Recommended approach: make the choice a **policy**, not a constant. Plan each
type as `Local` (`Rc`) or `Shared` (`Arc`), defaulting to `Arc` only where a
thread boundary is reachable. `Arc<Mutex<T>>` everywhere would cost performance
and readability across the whole corpus; the escape analysis already sketched in
0002 is what decides it, with "does this object cross a thread boundary" as one
more input.

## The hard parts, in order

### 1. Java monitors are reentrant; Rust `Mutex` is not

```java
synchronized void a() { b(); }      // same monitor, re-entered
synchronized void b() { }
```

A direct `Mutex` lowering **deadlocks**. Options:

- a reentrant mutex (`parking_lot::ReentrantMutex`, or hand-rolled with a
  thread-id + depth counter);
- lock only at public entry points and call an unlocked inner method internally —
  requires knowing that the callee is only reached with the lock held, which is a
  call-graph property (the graph already exists in `src/usage.rs`);
- accept non-reentrancy and diagnose self-calls from a `synchronized` method.

The middle option produces the best Rust and reuses existing machinery. It is
also the one that can fail silently if the analysis is wrong, so it wants the
same "assert both, diff over the corpus" migration discipline recommended in
[0006](claude_audits/0006-move-analysis-is-five-approximations-of-one-relation.md).

### 2. `Send`/`Sync` propagate through the whole object graph

`Arc<Mutex<T>>` requires `T: Send`. That obligation spreads to every field type,
and one non-`Send` member (a raw pointer, an `Rc`, a facade shell) poisons the
whole graph. Expect this to surface as a large, correlated wave of trait-bound
errors the first time it is enabled — which argues for enabling it per-type
behind the representation policy rather than globally.

### 3. `wait`/`notify` → `Condvar`

Mechanical but fiddly: Java's `wait()` releases the monitor and re-acquires it;
`Condvar::wait` takes the `MutexGuard` and returns it. `notifyAll` →
`notify_all`. Spurious wakeups exist in both, so idiomatic Java loops
(`while (!cond) wait();`) translate directly.

### 4. `volatile` → atomics with ordering

`volatile` in Java implies acquire/release plus total order on that variable.
`AtomicX` with `Ordering::SeqCst` is the safe default; `Acquire`/`Release` is the
performance answer and needs care. Default to `SeqCst`.

### 5. Threads need `'static` + `Send`

`std::thread::spawn` requires `'static`. Java's `new Thread(runnable)` captures
arbitrary object references, which map to `Arc` clones — workable, but it means
anything reachable from a thread body must be `Arc`, feeding back into the
representation policy.

## Recommended sequencing

1. **[0030a](claude_audits/0030-java-statics-are-thread-local.md)** — `static final`
   → `const`/`static`/`OnceLock`. Small, no locking, and correct under both
   models. Do it regardless.
2. **Fix the representation choice on paper** — extend
   [0002](claude_audits/0002-object-aliasing-has-no-representation.md)'s proposal
   with a `Local`/`Shared` policy axis before implementing it. Cheap now,
   expensive later.
3. **[0030b](claude_audits/0030-java-statics-are-thread-local.md) + [0015](claude_audits/0015-static-initializer-blocks-are-silently-dropped.md)**
   — mutable statics and `__class_init` via `OnceLock`. These must land together;
   doing 0015 on thread-local storage would bake in the per-thread error.
4. **Diagnose, don't erase.** **Landed 2026-09-17.** Until locking is real,
   synchronized statements and methods emit an unsupported diagnostic plus a
   generated `compile_error!`; this is intentionally stronger than the proposed
   audit marker. Guards:
   `idiom_gaps::{synchronized_statement_is_loud_until_monitor_semantics_exist,
   synchronized_method_is_loud_until_monitor_semantics_exist,
   volatile_field_is_loud_until_atomic_memory_order_semantics_exist}`. The same
   loud refusal now applies to `volatile` fields. Silently dropping mutual
   exclusion or memory-order semantics is no longer an accepted lowering path.
5. **Then** monitors, `volatile`, `wait`/`notify`, thread spawning.

## Honest caveat

Steps 1–4 are tractable and worth doing now. Step 5 is a substantial project, and
concurrency bugs are the hardest class to detect with the differential testing
proposed in `AUDIT-BACKLOG.md` — non-deterministic failures do not show up in a
single-run output diff. If concurrent Java turns out to be rare in the corpora
beyond those 15 files, the honest cost/benefit may favour **diagnosing**
`synchronized` and thread creation as unsupported rather than implementing them,
and revisiting when a corpus genuinely needs it. That is a judgement call worth
making explicitly rather than by drift — but either way, steps 1–3 are required,
because static storage and object representation are wrong today independent of
how much concurrency you support.
