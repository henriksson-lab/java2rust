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
([0002](0002-object-aliasing-has-no-representation.md)), so two variables holding
"the same object" are two copies with equal fields — indistinguishable from two
distinct objects that happen to be equal.

Note the `sameRef()` case passes only because the alias became a `clone()`. Both
answers are right by coincidence, from the same underlying error.

## Fix requires the shared representation

Pointer equality is not expressible until objects have identity. Once
[0002](0002-object-aliasing-has-no-representation.md)'s shared representation
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
