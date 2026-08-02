# 0017 — Generic bounds are dropped and method-level type parameters are never declared

Status: **DONE** — closed 2026-08-02
Severity: high — the largest error cluster in the biojava corpus
Kind: missing type-parameter model

## Symptom

Two distinct defects, both visible in one six-line probe:

```java
public class GenericBox<T extends Comparable<T>> {
    private T value;
    public GenericBox(T value) { this.value = value; }
    public T get() { return value; }
    public boolean isBigger(T other) { return value.compareTo(other) > 0; }
    public static <U> U identity(U u) { return u; }
}
```

Generated:

```rust
pub struct GenericBox<T> {          // (1) bound `Comparable<T>` dropped
    value: T,
}

impl<T: Clone> GenericBox<T> {      //     replaced by an unrelated inferred `Clone`
    pub fn is_bigger(&self, other: T) -> bool {
        return self.value.compare_to(other) > 0;    // E0599: no method `compare_to` on T
    }

    pub fn identity(u: U) -> U {    // (2) `U` is never declared anywhere
        return u;
    }
}
```

```
error[E0599]: no method named `compare_to` found for type parameter `T` in the current scope
```

### 1. Declared bounds are discarded

`T extends Comparable<T>` becomes bare `T`. The `impl` block carries `T: Clone`,
which is an inferred storage bound, not the Java bound. Every method the bound
was supposed to provide is then missing from `T`.

### 2. Method-level type parameters are not emitted

`static <U> U identity(U u)` becomes `pub fn identity(u: U) -> U` with no
`<U>` on the function. `U` is simply undeclared.

## Corpus corroboration

This is the dominant cluster in biojava-alignment (188 errors total):

| count | error |
|---|---|
| 25 | `E0405: cannot find trait GapPenalty in this scope` |
| 23 | `E0404: expected trait, found struct GapPenalty` |
| 20 | `E0433: failed to resolve: use of undeclared type Out` |

`undeclared type Out` is defect 2 exactly — a method type parameter named `Out`.
The `GapPenalty` pair is defect 1's neighbour: a Java type used as a bound needs
a **trait** in Rust, but the type was emitted as a **struct**, so referencing it
in bound position fails both ways at once.

htsjdk's 26 `E0282: type annotations needed` are likely the same area.
Together this is ~36% of biojava's errors.

## Root cause

The type-parameter model is incomplete in three ways:

1. **Class type parameters carry no bounds.** The declared `extends`/`&`-bounds
   are parsed away or never stored, so nothing reaches the `impl` header.
2. **Method type parameters are not modelled at all** — there is no place to
   put `<U>`, so it is dropped from the signature while still being used in the
   parameter and return types.
3. **A Java type in bound position needs a trait projection.** `Comparable<T>`
   and `GapPenalty` must resolve to the trait that the corresponding Rust type
   implements, not to the struct. Today a Java interface used as a bound and the
   same interface used as a value type map to the same Rust name, which is why
   `E0405` (no such trait) and `E0404` (found struct) appear together.

## Fix direction

Roughly in dependency order:

1. Add `type_params: Vec<TypeParam { name, bounds }>` to **both** `TypeDecl` and
   `MethodDecl` in `java_ir`, populated by the parser. Without the method-level
   slot, defect 2 cannot be fixed at all.
2. Emit `<U>` on generic methods and thread the parameter through the signature
   plan so parameter/return types resolve against it.
3. Project Java bounds onto Rust bounds:
   - `T extends Comparable<T>` → `T: Ord` (or a `JavaComparable` trait) —
     mapping through the existing `javalibs` model rather than by name;
   - `T extends SomeSourceInterface` → the trait emitted for that interface.
     This requires the interface's **trait** name to be reachable separately
     from its struct/value name — the missing distinction behind `E0404`.
4. Keep inferred bounds (`Clone`, etc.) as an *addition* to declared bounds, not
   a replacement. Today they appear to be substituting.

## Sequencing note

This is a genuinely large area and it interacts with the object model
(interface → trait vs struct). It is worth a dedicated probe pass before design:
the four probes here took ten minutes and already separated three distinct
defects, and biojava will yield more (wildcards `? extends`, generic
inheritance, raw types, erasure at call boundaries) — none of which are covered
by the current examples.

Do not attempt this at the same time as the object-representation work in
[0002](0002-object-aliasing-has-no-representation.md); they both touch how a
Java type becomes a Rust type, and simultaneous change would make failures
unattributable.

## Update 2026-08-01 — parser and class-bound path improved, not closed

The parser now walks `type_parameter` and `type_bound` nodes instead of scanning
angle-bracket text, and class-level `Comparable<T>` can project to `Ord` with a
blanket `JavaComparable` helper. Short bounded type variables such as `T`/`U`
also avoid the previous `NeedsUnknownRuntimeObject` erasure.

This does **not** close the audit. Type-variable identity is still inferred by a
name-shape heuristic rather than by declared parameters, so legal names such as
`Out` are not covered. `Comparable -> Ord` is wired for class type parameters,
but not method/constructor type parameters, and `program_uses_java_comparable_bound`
does not emit support for method-only `Comparable` bounds. Constructor-level
bounds are also not seeded into expression shape projection. Keep this active.

## Regression tests worth adding

```java
class Box<T extends Comparable<T>> { boolean bigger(T o){ return v.compareTo(o) > 0; } }
static <U> U id(U u) { return u; }                 // <U> must be emitted
static <K, V extends Number> V pick(K k, V v)      // multiple, one bounded
class Pair<A, B extends List<A>> { }               // bound referring to another param
```

## Resolution 2026-08-02

Closed for the audited defects. Type-parameter identity is now declaration
driven and scoped through `TypeParamScope`, rather than inferred from name
shape or collected globally. Class, method, constructor, parameter, return,
field, local, and inherited source-method signature slots carry the relevant
scope into `SignatureShapeRequest`.

Declared bounds now share `rust_bounds_for_java_type_param`, so class,
method, and constructor type parameters use the same trait-like, class-like,
and JDK projection. `Comparable` projections gate `JavaComparable` support from
the projection path itself, including method-only and constructor-only bounds.
Method/constructor type parameters also receive `Clone` when clone recovery can
insert a clone for repeated reads.

Regression coverage includes a same-name collision between a real class `ELEM`,
a class type parameter `ELEM`, and a method type parameter `ELEM`, plus existing
generic class/method/constructor bound fixtures. Multi-bound/intersection
coverage remains a broader generic-type-modeling follow-up, not this audit's
original failure.
