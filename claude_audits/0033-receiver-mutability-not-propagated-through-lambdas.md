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
([0001](done/0001-loop-back-edge-invisible-to-move-analysis.md)); the `E0382` in the
same probe is fixed, this `E0596` is not. The two are independent: one is
liveness, this is mutability.

## Regression test worth adding

```java
void take(String s) { items.add(s); }
public void inLambda(String v, List<String> xs) { xs.forEach(x -> take(v)); }
// in_lambda must be emitted as `&mut self`
```
