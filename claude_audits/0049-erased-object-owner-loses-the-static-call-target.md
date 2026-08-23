# 0049 — A static call to an erased-object class is re-attributed to the *calling* class

Status: **validated by execution** — reproduced with `target/release/new-java2rust`,
minimal Java, generated Rust quoted
Severity: **high** — the worst case is **silent wrong output**: it compiles and
calls a different method than the Java did
Live or latent: **live** — triggered by one `erased-object` fact on the callee,
which is the representation the corpus assigns widely
Kind: call-target identity lost at an erasure boundary

Found while verifying [0048](0048-erased-static-trait-body-is-rewritten-by-substring.md);
it is a different subsystem (call planning, not text rewriting) and is filed
separately for that reason.

## Symptom

`B.m()` calls `A.m2()`. If `A` lowers as an erased-object alias, the generated
Rust no longer calls `A`'s helper. It calls `B`'s.

The severity depends on whether `B` happens to have a method of the same Rust
name:

| `B` has a same-named static? | is `B` erased? | emitted | outcome |
|---|---|---|---|
| yes | no | `Self::run()` | **silently calls `B::run`** |
| no | no | `Self::run()` | `E0599` |
| no | yes | `beta_run()` | `E0425` — the name does not exist |

## Verified evidence

### 1. Silent wrong output

`facts5.txt`:

```
erased-object Alpha
```

`Alpha2.java`:

```java
class Alpha {
    static int run() { return 1; }
}
class Beta {
    static int run() { return 2; }
    static int go() { return Alpha.run(); }
}
```

`./target/release/new-java2rust --facts facts5.txt Alpha2.java`:

```rust
pub type Alpha = Box<dyn std::any::Any>;

pub trait AlphaErasedStatic {
    fn run() -> i32 { return 1; }
}
impl AlphaErasedStatic for Alpha {}

pub fn alpha_run() -> i32 { return 1; }

impl Beta {
    pub fn run() -> i32 { return 2; }

    pub fn go() -> i32 {
        return Self::run();          // <-- Java said Alpha.run()
    }
}
```

`Beta.go()` returns **1** in Java and **2** in the generated Rust. It compiles
without a warning, and `alpha_run()` — the correct target — sits unused two
items above.

Removing the fact file restores the correct output:

```rust
pub fn go() -> i32 {
    return Alpha::run();
}
```

So the erasure of the *callee* is what loses the target.

### 2. Same input without `Beta.run` — `E0599`

`Alpha3.java` (identical minus `Beta.run`) still emits `return Self::run();`
inside `impl Beta`, and `Beta` has no `run`.

### 3. Both classes erased — a call to a name that was never emitted

`facts4.txt`: `erased-object Alpha` + `erased-object Beta`; `Alpha.java` is
`Alpha.run` plus `Beta.go` calling it. Emitted:

```rust
pub fn alpha_run() -> i32 { return 1; }
…
pub fn beta_go() -> i32 {
    return beta_run();
}
```

`beta_run` is not defined anywhere in the output (`grep` finds exactly the three
lines quoted). `E0425`.

## Root cause

Two layers; the second is the one with a clean anchor.

**Layer 1 — the receiver is gone before planning finishes.** The emitted
`Self::…` spelling means the call reached
`CallTargetPlan::StaticSelf`/`FallbackSelf`. Both of those arms in
`src/call_plan.rs:plan_resolved_static_call` (line 2866) and the fallback at
line 2515 are reachable **only** with `receiver == None`; the `Some(receiver)`
arms produce `StaticOwner` / `FallbackReceiver`. So by the time the plan is
chosen, `Alpha.run()` is being planned as a receiverless call. I did not pin
which arm drops it — that needs instrumentation, and this file does not guess.

**Layer 2 — the erased-static retarget then *invents* an owner.**
`src/lower.rs:erased_source_static_call_owner_and_rust_name` (line 13183):

```rust
if receiver.is_none()
    && let Some(current_type) = ctx.current_type
    && let Some(rust_name) = static_call_plan_rust_name(plan)
{
    return Some((current_type.to_string(), rust_name.to_string()));
}
```

A receiverless static call is attributed to the *current* type, unconditionally.
`src/lower.rs:retarget_erased_source_static_call_to_free_function` (line 13088)
then checks only that this invented owner is an erased alias — not that it
declares the method — and mints
`erased_source_static_free_function_name(owner_key, rust_name)`. That is where
`beta_run` comes from in case 3: `Beta` is erased, so the guard passes, and a
helper name is synthesised for a method `Beta` does not have.

The guard is cheap and local: `retarget_…` already holds
`owner_ty: &TypeDecl` from `find_context_type_by_key`, so
"does this owner declare a static `rust_name`?" is a scan of `owner_ty.members`
that is already in scope. Today nothing checks it, and the failure is silent.

### A third defect visible in the same feature

The helper naming scheme `to_snake_case(owner) + "_" + rust_name`
(`src/lower.rs:erased_source_static_free_function_name`, line 1275) is not
injective. `Node2.java` (`erased-object Node`, `erased-object NodeX`, with
`NodeX.y()` and `Node.x_y()`) emits **two** `pub fn node_x_y()` with different
bodies:

```
1777:pub fn node_x_y() -> i32 {     // NodeX::y   -> 1
1797:pub fn node_x_y() -> i32 {     // Node::x_y  -> 99
```

`E0428`. This is [0009](0009-overload-identity-collapses-onto-rust-name.md)'s
shape at the owner level: two distinct `(owner, method)` identities flattened
into one Rust name by string concatenation.

## Why it went unnoticed

The feature's only test,
`tests/idiom_gaps.rs:erased_source_static_methods_lower_as_free_helpers`, has a
**single** erased class calling **its own** static. That is precisely the case
where "attribute a receiverless static call to `ctx.current_type`" is correct,
so the assumption is never exercised against a second owner. There is no fixture
with two erased classes, and none where an erased class is called from a
non-erased one — which is the ordinary corpus situation.

## Proposed direction

1. **Guard the retarget.** In
   `retarget_erased_source_static_call_to_free_function`, require that
   `owner_ty` actually declares a static with that Rust name before rewriting
   the target. This converts case 3 from a dangling name into "leave the plan
   alone", and costs nothing.
2. **Do not invent an owner.** The `receiver.is_none()` branch of
   `erased_source_static_call_owner_and_rust_name` should return `None` when the
   resolved call's declaring type is available and is not the current type,
   rather than defaulting to `ctx.current_type`.
3. **Find the receiver loss (layer 1).** The correct fix is upstream: a static
   call with an explicit type receiver should not lose that receiver just because
   the receiver type lowers to `Box<dyn Any>`. Case 1's silent miscompile is
   entirely a layer-1 defect; layers 2 and 3 only change *which* wrong thing is
   emitted.
4. **Make the helper name carry both halves of the identity**, or key the
   emitted free functions by `(owner, rust_name)` and disambiguate on collision,
   as 0009 proposes for overloads.
5. **Fixtures**: two erased classes calling each other; an erased class called
   from a plain class; and the `NodeX`/`Node.x_y` collision — all three with
   `cargo_check_generated_modules`. All three fail today, and case 1 needs a
   behavioural assertion, not just a compile check, because it compiles.

## Repro files

```
facts5.txt: erased-object Alpha
facts4.txt: erased-object Alpha
            erased-object Beta
facts2.txt: erased-object Node
            erased-object NodeX
```

with `Alpha2.java`, `Alpha3.java`, `Alpha.java`, `Node2.java` as quoted above.
