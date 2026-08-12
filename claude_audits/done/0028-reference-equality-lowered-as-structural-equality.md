# 0028 — `==` on object references is lowered to structural equality

Status: **validated** (runtime-confirmed, compiles cleanly)
Severity: high — silent wrong answer; **contradicts an explicit project decision**
Kind: missing identity semantics

## Decision this violates

Project decision (2026-07-30): *"`==` on classes should in general mean pointer
equality. On `String`, lowering to value comparison is acceptable, but a warning
must be written into the generated code for later audit."*

Reference equality on classes is currently **structural**, so the decision is not
met.

## Symptom

```java
public class R1 {
    static class Node { int v; }
    public boolean distinct() { Node a = new Node(); Node b = new Node(); return a == b; }
    public boolean neq()      { Node a = new Node(); Node b = new Node(); return a != b; }
}
```

Emitted:

```rust
let a: R1Node = R1Node::new();
let b: R1Node = R1Node::new();
return a == b;                 // derived PartialEq -> field comparison
```

| | Java | emitted Rust |
|---|---|---|
| `distinct()` | `false` | **`true`** |
| `neq()` | `true` | **`false`** |
| `sameRef()` (`Node c = a; a == c`) | `true` | `true` (accidentally — via the inserted `clone()`) |

Two distinct objects with equal fields compare equal, and `!=` inverts to the
wrong answer. Compiles cleanly, so invisible to corpus error counts
([0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)).

## Root cause

`==` on a reference type is lowered to Rust `==`, which resolves to the derived
`PartialEq`. There is no notion of object identity, because there is no object
identity to have: values are represented inline
([0002](../0002-object-aliasing-has-no-representation.md)), so two variables holding
"the same object" are two copies with equal fields — indistinguishable from two
distinct objects that happen to be equal.

Note the `sameRef()` case passes only because the alias became a `clone()`. Both
answers are right by coincidence, from the same underlying error.

## Fix requires the shared representation

Pointer equality is not expressible until objects have identity. Once
[0002](../0002-object-aliasing-has-no-representation.md)'s shared representation
exists (`Rc`/`Arc`), `==` on references lowers to a pointer comparison:

```rust
Rc::ptr_eq(&a, &b)        // or Arc::ptr_eq
```

Sequencing consequence: **this issue and
[0023](0023-inner-class-construction-always-panics.md) are both blocked on the
same representation work**, which now has three independent motivations —
aliasing correctness, inner classes, and reference identity. That is a stronger
case for 0002b than the corpus error count suggests (`E0382` is rare because
clone insertion hides the problem as wrong behaviour).

Interim measure, worth doing immediately and independently: **emit an audit
warning** at every `==`/`!=` on a reference type, exactly as decided for
`String`. Today an auditor reading generated code cannot tell that identity
semantics were silently dropped.

## `String` `==` — the decided case, and why the warning matters

Value comparison is accepted for `String`. But note what the output currently
looks like:

```java
public boolean made(String s) { return s == "x"; }        // reference equality
public boolean eq(String s)   { return s.equals("x"); }   // value equality
```
```rust
pub fn made(&self, s: &str) -> bool { return s == "x".to_string().as_str(); }
pub fn eq(&self, s: &str)   -> bool { return s == "x".to_string().as_str(); }
```

**Identical output.** The two Java operators are indistinguishable in the
generated Rust, so nothing downstream — and no human auditor — can recover which
one the source used. That is precisely the case the decided warning is for:

```rust
// AUDIT(java-string-identity): Java `==` on String compared references;
// lowered to value comparison. Verify this is intended.
return s == "x";
```

A marker that is greppable (`AUDIT(java-string-identity)`) lets the whole class
be reviewed later in one pass. Also worth counting in `summary.tsv`.

## Regression tests worth adding

```java
Node a = new Node(); Node b = new Node();  a == b   // false even with equal fields
Node c = a;                                a == c   // true
String s = new String("x");                s == "x" // Java false; documented divergence + warning
"x" == "x"                                          // Java true (interned)
```

---

## Update 2026-08-05 — the `.equals()` half has a reproducer, and this issue is now on the critical path

Two things found while probing
[0002b](../0002b-shared-representation-design.md)'s step-3 sequencing.

### 1. `.equals()` diverges for the same reason, and it was undesigned

This file says of `Rc::ptr_eq` for `==`:

> With `Rc::ptr_eq` for `==`, Java's `.equals()` needs a separate path. That is
> 0028's other half and is not designed here.

It now has a reproducer. `Object.equals` in Java **is** reference identity
unless a class overrides it:

```java
static final class Key { final int v; Key(int v) { this.v = v; } }
Key a = new Key(5), c = new Key(5);
a.equals(c)     // Java: false — Key does not override equals
```

The translation answers `true`, because `.equals()` is lowered to structural
comparison whether or not the source class overrides it. Same defect as `==`,
same cause, and the same fix once objects have identity: **an inherited
(non-overridden) `equals` is `Rc::ptr_eq`; an overridden one calls the source
body.** The discriminator already exists — the override graph — so this is a
lookup, not new analysis.

Note the direction of the error: it answers `true` where Java says `false`, so a
`HashMap`/`contains`/dedup built on `.equals()` **merges entries Java keeps
apart**. That is data loss, not just a wrong boolean.

Originally pinned by `tests/differential/known-gaps/Identity.java`; after this
fix it is promoted to `tests/differential/Identity.java`. The fixture covers
`==` and `.equals()`, both the true-identity and distinct-object cases.

**The interim measure is now applied to this half too.** This file argues that
an audit warning at every reference-identity comparison is "worth doing
immediately and independently"; `==` already had one, `.equals()` did not.
`java_inherited_equals_identity` now reports it, naming the class:

```
Immut.Key does not override equals, so Java compares references here;
lowering compares fields, which reports equal for distinct objects
```

The discriminator follows `extends`, so a class inheriting an override from a
source superclass is **not** reported — and an unknown supertype is treated as
possibly overriding, which is the conservative direction for a warning. Three
tests pin it: the positive, the overridden negative, and the
inherited-from-superclass negative.

### 2. This issue is no longer merely "blocked on 0002b" — it is *half of* it

0002b's `SharedImmutable` tier turns out to have **no observable behaviour of its
own**: a type that is never mutated after aliasing behaves identically whether it
is copied or shared. So the representation change cannot be validated by a
differential test without an identity operator to observe it through.

Which makes the dependency mutual:

- 0028 is **unimplementable** without the shared representation;
- 0002b step 3 is **untestable** without 0028.

They were one change, and the now-promoted `tests/differential/Identity.java`
is the acceptance test. Before the handle representation and identity lowering
landed together, it failed on two of four cases; it now passes. Recorded in
0002b as a correction to its sequencing.

---

## Update 2026-08-07 — implemented

Reference identity is lowered. `scripts/gate.sh --all` is at baseline on all 18
targets, and both acceptance fixtures pass against the Java oracle:

- `tests/differential/Identity.java` — **promoted out of `known-gaps/`**, prints
  `5,true,false,false` on both sides.
- `tests/differential/IdentityCustomEquals.java` — new, prints
  `true,false,true,false,true` on both sides.

```rust
std::rc::Rc::ptr_eq(&a, &b)                                        // a == b
holder.kept.as_ref().is_some_and(|value| Rc::ptr_eq(value, &c))    // nullable == live
match (a.as_ref(), b.as_ref()) {                                   // both nullable
    (Some(left), Some(right)) => Rc::ptr_eq(left, right),
    (None, None) => true,                                          // Java: null == null
    _ => false,
}
```

### The mutual dependency above is discharged

The section before this called 0028 and 0002b "one change": 0028 unimplementable
without the representation, 0002b step 3 untestable without 0028. 0002b landed
first, so this pass was purely the operator — and `Identity.java` then validated
*both*, exactly as predicted.

### Where it lives, and why not where the plan said

Not `src/lower.rs`. The decision is `plan_equality_comparison_for_source` in
`src/operator_lowering.rs`, which already canonicalises operand order and mirrors
the result so an asymmetric arm has nowhere to be written. The identity arms go
through that same path; `identity_planning_is_symmetric_under_operand_swap`
asserts the involution over the new arms specifically.

`handle_path()` supplies `std::rc::Rc` or `std::sync::Arc` from the placement, so
a type reachable from a Java `static` compares with `Arc::ptr_eq` without the
lowering knowing which.

### The two operators had to be told apart after all

Java's `==` is reference identity *whatever* `equals` says; `.equals()` is the
override when there is one. Mostly this needs no attention — the call plan routes
an overridden `.equals()` to the source method, so it never reaches this planner.
**Optional receivers are the exception**: they arrive here for the null-safe
wrapper, which is what `BothOptionalCustomEquals` already existed for. Planning
identity unconditionally broke
`nullable_custom_equals_method_uses_null_safe_custom_equals`, correctly.

Hence `EqualityComparisonSource::{ReferenceOperator, EqualsCall}`. The custom-
`equals` question is asked only for the call, never for `==`.

Follow-up correction: the first implementation gated the non-optional
custom-`equals` arm, but the both-optional arm still asked the custom-`equals`
question before checking the source. Shared optional operands were masked by the
earlier `ptr_eq` arm, but non-shared nullable custom-equals types could still
route `==` through `BothOptionalCustomEquals`. That branch is now gated to
`EqualityComparisonSource::EqualsCall`; `==` falls back to the existing optional
by-reference plan when no shared handle exists.

### Two pre-existing defects the new fixture exposed

Neither is about identity; both were invisible until a fixture overrode `equals`,
which is what forces an erased `Object` parameter through a cast.

- **`error[E0614]: type i32 cannot be dereferenced`.** The erased-object downcast
  rendered `*x.downcast::<T>().expect(..)`, and `.` binds tighter than unary `*`,
  so an appended field access made it a deref of the *field*. Now parenthesised.
- **The runtime type test named a type the carrier never stores.** With a shared
  element the carrier holds `Rc<T>`, and the downcast already asked for `Rc<T>`,
  but `instanceof` still asked `is::<T>()` — always false, so the guard silently
  rejected every value and `equals` always returned `false`. Both now go through
  `erased_runtime_target_type`.

### Still structural, deliberately

Primitives, `String`, enums, an overridden `.equals()`, and any type that is not
shared. A type with no aliases has no pointer to compare; `an_unshared_type_keeps_structural_equality`
pins that boundary. `SharedLocal`/`SharedSync` remain off, so nothing here
touches the `RefCell`/`Mutex` rungs.
