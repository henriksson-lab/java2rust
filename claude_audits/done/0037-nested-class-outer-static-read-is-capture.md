# 0037 — A nested class reading an outer `static` field is lowered as an outer-instance capture

Status: **validated** (rustc-confirmed)
Severity: medium — any nested class touching an outer static fails to compile
Kind: capture classification

## Symptom

A nested class that reads a `static` field of its enclosing class is treated as
capturing the enclosing **instance**, even when the nested class *and* the
method are `static`:

```java
public class StaticSharing {
    static int counter = 0;
    static class Reader {
        static int readCounter() { return counter; }   // reads a *static*
    }
}
```

Emitted:

```rust
pub fn read_counter() -> i32 {
    return self.__outer.borrow().counter;   // no `self`; and `counter` is not a field
}
```

```
error[E0424]: expected value, found module `self`
```

With an *instance* method on the nested class the shape is the same and fails
differently:

```
error[E0609]: no field `counter` on type `Ref<'_, StaticSharing>`
```

— because `counter` is static storage, not a struct field, so even a correctly
captured outer instance would not have it.

## Root cause

Outer-`this` capture is decided from *name resolution* — an unqualified
`counter` inside a nested class resolves to the enclosing class — without
checking whether the resolved member is `static`. A static member needs no
instance, and a `static` nested class cannot capture one at all.

Two independent conditions are being ignored:

1. the resolved member is `static`, so no enclosing instance is involved;
2. the nested class is `static`, so it has no enclosing instance to capture.

Either alone is sufficient to suppress the capture.

## Fix direction

When classifying an unqualified name in a nested class as an outer-`this`
capture, require that the resolved member is an **instance** member and that the
nested class is non-static. A static member should lower to the enclosing type's
static accessor (`StaticSharing::__static_counter()`), which already exists and
is what a same-class reference compiles to.

## Relationship to other findings

Distinct from [0023](0023-inner-class-construction-always-panics.md), which is
about *constructing* a genuine inner class and needs the shared representation.
This one needs no representation work: the capture should not be classified in
the first place. It was found while writing the regression fixture for
[0030](0030-java-statics-are-thread-local.md), which had to route around it.

## Regression test worth adding

```java
class Outer { static int n = 1;
              static class In { static int get(){ return n; } }
              static int run(){ n = 5; return In.get(); } }   // 5
```

---

## Update 2026-08-05 — fixed

Two defects were stacked here, and the second was only visible once the first
was gone.

### 1. The capture classification ignored `static`

`analyze_inner_type` was reached for **every** `Member::Type`, and the outer
field list it built came from all of the enclosing type's fields. So a `static`
nested class reading a `static` field became `self.__outer.borrow().n` inside a
method that has no `self` at all.

Both conditions the audit names are now required, which is what it asked for:

- `Member::Type(inner) if !inner.modifiers.contains(&Modifier::Static)` gates
  `analyze_inner_type`, so a `static` nested class is analysed as an ordinary
  type;
- a new `instance_field_types` filters `Modifier::Static` out of `outer_fields`,
  so even a genuine inner class does not treat an enclosing *static* as
  capturable state.

Either alone suppresses the wrong classification; both are applied because they
are independently true.

### 2. The read then named the wrong type

With the capture gone, the read resolved through the shared shape environment —
which carries every type's statics under their bare names, since Java has them
in scope inside a nested class — and `static_storage_accessor_projection`
hardcoded `Self::`. From inside `In` that emits `OuterIn::__static_n()` for a
field `Outer` owns:

```
error[E0599]: no associated item named `__static_n` found for struct `OuterIn`
```

The **write** side was already correct, which is why only the read failed:
assignment planning carries a `rust_owner` and calls
`qualified_static_storage_setter_path_projection`, while the bare-name read path
had no owner to carry. `ShapeEnv` now records the declaring type alongside each
static-storage name and knows which type it was built for, so
`static_storage_accessor_qualifier` answers "`Self`, or this owner" — mirroring
what the associated-const branch beside it already did.

Same-type reads still emit `Self::`, so this is not a rename of every static
access; only a cross-type read changes.

### Verification

- `tests/differential/NestedStatic.java`, against the Java oracle: a nested
  class reading *and* writing an enclosing static, a nested static with the same
  simple name as an enclosing one (so `Self` and the qualified owner cannot be
  confused), the enclosing class reading a nested static, and a second nested
  class so the owner cannot be guessed from arity. Both toolchains print
  `1,1,4,9,2`.
- `nested_class_reads_enclosing_static_through_its_owner` in
  `tests/slot_shape_design.rs` pins the emitted spelling, including the negative:
  no `__outer` for a `static` nested class.
- `scripts/gate.sh --all`: all 17 targets plus `--lib`, zero deltas.

### Note for whoever reads this next

The `StaticSharing.java` fixture for
[0030](0030-java-statics-are-thread-local.md) was written to route *around* this
defect and says so in its header. That comment is now updated: the two stay
separate on purpose, because a fixture about storage identity should not fail
for a name-resolution reason.
