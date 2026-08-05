# 0033 — Receiver mutability is not propagated through lambda bodies

Status: **validated** (rustc-confirmed)
Severity: medium — any `forEach`/stream lambda that calls a mutating method fails to compile
Kind: mutability inference gap

## Symptom

A method whose body only mutates `self` *inside a lambda* is inferred as
`&self`, so the lowered loop body cannot call the `&mut self` method.

```java
import java.util.List;
public class Live3 {
    private List<String> items = new ArrayList<>();
    void take(String s) { items.add(s); }
    public void inLambda(String v, List<String> xs) { xs.forEach(x -> take(v)); }
}
```

Emitted:

```rust
pub fn take(&mut self, s: String) { self.items.push(s); }

pub fn in_lambda(&self, v: String, xs: &[String]) {   // <-- should be &mut self
    for x in xs.iter() {
        self.take(v.clone());
    }
}
```

```
error[E0596]: cannot borrow `*self` as mutable, as it is behind a `&` reference
```

## Root cause

Receiver-mutability inference walks method bodies to find `self` mutations, but
a call made from inside a lambda body does not contribute to the enclosing
method's receiver requirement. `usage.rs` has
`propagate_self_mutability_through_self_calls`, so the mechanism exists — it
simply does not reach through the lambda boundary.

`forEach` is lowered to a real loop with the lambda body inlined
(`for x in xs.iter() { self.take(...) }`), so by the time the body is emitted the
call *is* a direct `self` call — but the signature was already decided.

## Fix direction

When collecting self-mutation evidence for a method, descend into lambda bodies
that are lowered inline (`forEach`, `stream().forEach`, and similar), treating a
`self` call inside them as a call from the enclosing method.

The narrower alternative — inferring mutability after lambda inlining — would
also work but changes the ordering between signature planning and lowering,
which is a bigger change.

## Discovered while fixing 0001

Found while verifying the loop-carried move analysis
([0001](0001-loop-back-edge-invisible-to-move-analysis.md)); the `E0382` in the
same probe is fixed, this `E0596` is not. The two are independent: one is
liveness, this is mutability.

## Regression test worth adding

```java
void take(String s) { items.add(s); }
public void inLambda(String v, List<String> xs) { xs.forEach(x -> take(v)); }
// in_lambda must be emitted as `&mut self`
```

---

## Update 2026-08-04 — fixed

Status: **fixed.** Regression test: `tests/differential/LambdaMutation.java`,
which runs both toolchains and compares stdout (`3,3,2`).

### The cause was narrower than this file assumed

This file expected the evidence walk to stop at the lambda boundary. It does
not: `analyze_lambda_body` (`src/usage.rs`) already walks a lambda body with the
**enclosing method's** `MethodUsageFacts`, so `take(v)` inside
`xs.forEach(x -> take(v))` was recorded as a call site on `inLambda` all along.

What was missing is one step earlier. `resolve_expr_calls` (`src/resolve.rs`)
has a bare

```rust
Expr::Lambda { .. } => {}
```

so calls inside a lambda are never resolved. The call site therefore arrived at
the self-mutability fixpoint with `resolved: None` **and** `receiver_type: None`
— and every arm of that fixpoint's condition required one or the other. The
evidence was present and unusable.

Worth recording because the fix direction this file proposed ("descend into
lambda bodies when collecting self-mutation evidence") would have been a no-op:
the descent was already there.

### The fix

An **unqualified** call needs no resolution to be identified as a self call —
the enclosing type is already known, because it is the key of the map the facts
are stored under. The fixpoint now iterates `iter_mut()` rather than
`values_mut()` and adds one arm: a receiverless call naming a mutating method of
the owning type marks the caller as mutating.

That is exactly the rule lowering already applies when it emits `self.take(...)`
for an unqualified `take(...)`, so the signature decision and the emitted body
now agree by construction instead of by coincidence.

Deliberately *not* done: making `resolve_expr_calls` descend into lambdas.
`should_defer_stream_lambda_arg_resolution` sits immediately below that arm, so
the non-descent is load-bearing for stream lambdas whose parameter types are not
known until the stream is analysed. Changing it would have meant reordering
resolution against stream analysis to fix a mutability bug.

### Coverage

The fixture pins both the direct shape and the transitive one:

- `collectAll` mutates only inside the lambda — the audit's case;
- `countAll` calls `bumpTwice()` inside the lambda, which calls `bump()`, which
  mutates. Two hops, so the fixpoint has to actually iterate rather than catch it
  in one pass.

The fixture deliberately avoids `items.get(i)` inside a `StringBuilder.append`:
that shape trips an unrelated optional-carrier defect
(`Option<&String>` reaching a `JavaAppendValue` bound), and a fixture that cannot
pass for a second reason cannot pin the first one.
