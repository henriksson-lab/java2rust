# 0025 — Interface default method bodies are discarded, then panic at runtime

Status: **validated** (runtime-confirmed)
Severity: high — default methods are ubiquitous in modern Java
Kind: missing member handling; the Rust target supports it directly

## Symptom

A `default` method's body is dropped. The trait declares the method with no
body, and each implementing class gets a `panic!` stub.

```java
interface Greeter {
    String name();
    default String greet() { return "hello " + name(); }
}
public class Q5 implements Greeter {
    public String name() { return "world"; }
    public String run() { return greet(); }
}
```

Generated:

```rust
fn greet(&self) -> String;                 // trait: declaration only, body lost

// impl for Q5:
fn greet(&self) -> String {
    panic!("missing Java interface method implementation: Q5.greet");
}
```

Running it:

```
thread 'main' panicked at q5m.rs:1697:33:
missing Java interface method implementation: Q5.greet
```

Java prints `"hello world"`. The output compiles cleanly, so this is invisible
to the corpus error count ([0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)).

## Why this one should be cheap

Rust traits support default method bodies natively, and the mapping is direct:

```rust
trait Greeter {
    fn name(&self) -> String;
    fn greet(&self) -> String { format!("hello {}", self.name()) }   // default body
}
```

The default body calls other trait methods through `&self`, exactly as the Java
version calls them through `this`. No new machinery is needed — the body simply
needs to be lowered into the trait declaration instead of being dropped, and the
`panic!` stub must not be generated when a default exists.

## Points to get right

1. **A class that overrides the default must win.** Emit the override in the
   `impl` and rely on Rust's normal precedence.
2. **`Iface.super.method()`** — Java's explicit qualified super-call to a
   specific interface default. In Rust this is `<dyn Iface>::method(self)` or
   `Iface::method(self)`. Needed for the diamond-resolution case.
3. **Conflicting defaults from two interfaces.** Java requires the class to
   override and disambiguate; Rust likewise will not pick one. If the source
   compiles, an override exists, so this should fall out — but it is worth a test.
4. **`static` and `private` interface methods** (Java 8/9) are separate cases;
   probe them rather than assuming they share the path.
5. Default methods on interfaces that are also used as **`dyn` trait objects**
   must stay object-safe. A default body that is generic or takes `self` by value
   would break that; diagnose rather than emit.

## Relationship to other findings

The `panic!`-as-placeholder pattern here is the same as
[0023](../0023-inner-class-construction-always-panics.md) and 0009's
`panic!("…parameter count does not match interface")`. In all three, a
*translator* gap is encoded as a *runtime* failure in generated code, which
means it compiles and the corpus metric scores it as success. Whatever emits
these should raise a translator diagnostic as well — see
[0018](0018-the-compile-error-metric-cannot-see-silent-failures.md).

## Regression tests worth adding

```java
interface G { String name(); default String greet(){ return "hi " + name(); } }
class A implements G { public String name(){ return "a"; } }              // greet() -> "hi a"
class B implements G { public String name(){ return "b"; }
                       public String greet(){ return "yo"; } }            // override wins -> "yo"
interface H { default String greet(){ return "h"; } }
class C implements G, H { public String name(){ return "c"; }
                          public String greet(){ return G.super.greet(); } }  // -> "hi c"
```

The third case pins `Iface.super`, which is the part most likely to be missed.


---

## Update 2026-08-05 — closed

Status: **fixed**, across every surface this file lists. Regression test:
`tests/differential/InterfaceDefaults.java`, seven cases against the Java
oracle (`hi a,yo,hi d,hi c!,hi e,hi a!,yo!`).

### The trait was never the problem

Default bodies were already emitted into the trait, and implementors already
omitted the method so Rust's default applied. Every remaining failure was
somewhere else, and the previous update's `panic!` symptom had already been
replaced by a worse one: a silent `""`.

Five distinct defects, all found by driving one fixture to green:

| # | defect | symptom |
|---|---|---|
| 1 | a default inherited through `implements` was not an inherited member, so signature planning registered no return type | `a.greet()` took the erased-display path → `""`, compiling cleanly |
| 2 | a locally declared type was treated as an *external* super, so facade shells were synthesised for a type already emitted | `E0592` duplicate definitions, `E0428` name collision |
| 3 | external inherited signatures were registered **before** source-inherited ones, and `or_insert` means first-wins | `fn greet(&self)` with no return type |
| 4 | a super-interface was not lowered as a supertrait, and was not counted as an implemented interface | `E0599` in the default body, `E0277` on the impl |
| 5 | `Iface.super.m()` could not resolve a nested interface, so it fell through to the static-owner path | `E0433 use of undeclared type Super` |

### The one cause underneath four of them

> **A nested type is stored qualified (`Outer.Inner`) but named simply (`Inner`)
> everywhere it is referenced, and several lookups compared only the qualified
> form.**

`find_unique_type_decl_by_simple_name` compared a *qualified stored name*
against a simple query — the function's own name says what it should have done.
`find_program_type_by_key` (`stdlib.rs`) had the same gap, while the equivalent
in `representation_lowering.rs` had always matched both. That single
inconsistency is why "is this type declared here?" answered *no* for every
nested type, which is what let facade shells be synthesised for emitted types.

Worth stating plainly because it presented as five unrelated `rustc` errors
across three files, and because the earlier two attempts each fixed a symptom
and were reverted. What made this pass converge was minimising to
`S1d` — *two defaults **and** an overriding sibling **and** a subclass*, where
removing any one made it pass — and then not stopping at the first green case.

### Changes

- `collect_inherited_interface_default_methods` (`inherited_members.rs`) —
  defaults from a type's own `implements`, its super-interfaces, and its
  superclasses' `implements`.
- `signature.rs` — source-inherited signatures registered **before** external
  ones, since `or_insert` gives the first writer the key and an external
  signature is a guess while a source one is solved.
- `stdlib.rs` — a supertype the program declares is not an external super; a
  type the program emits never needs a facade shell; simple names are recorded
  as local type keys and matched by `find_program_type_by_key`;
  `source_method_hierarchy_contains` gets the real program (it walks `extends`,
  so the synthetic one-type program it used to receive could never find a
  superclass) and consults implemented interfaces.
- `lower.rs` — `pub trait P: G` for interface inheritance; erased-name lookup
  for `Iface.super`; `find_unique_type_decl_by_simple_name` compares simple to
  simple.
- `representation_lowering.rs` — an interface's `extends` names an implemented
  *interface*, not a superclass, so it is collected as one;
  `Clone for Box<dyn T>` calls `T::clone_box(&**self)`, because a subtrait and
  its supertrait both declare `clone_box` and the unqualified call became
  ambiguous (`E0034`).

### Points from this file, checked

1. **Override wins** — `B` overrides `greet`; verified.
2. **`Iface.super`** — `E` delegates explicitly; verified.
3. **Conflicting defaults from two interfaces** — Java requires an override, so
   it reduces to case 1. Not separately fixtured.
4. **`static`/`private` interface methods** — still unprobed, and genuinely
   separate paths.
5. **Object safety** — the traits remain object-safe: a generic default is
   emitted `where Self: Sized`, which `slot_shape_design` pins.

---

## Update 2026-08-04 — the non-overriding case is fixed; facade-shell residual recorded

Status: **partial**, and the partition is now precise.

### What was still wrong, and it was silent

The trait already carried the default body correctly, and the `impl` correctly
omitted the method so Rust's default applied. The defect had moved to the **call
site**: `inherited_instance_method_refs_for_type` walked only `extends`, so a
default inherited through `implements` was not an inherited member at all, and
signature planning registered no return type for it. `a.greet()` therefore had
an unknown result type, took the erased-display path, and returned `""`.

```
java:        hi a,yo
translation: ,yo          (compiles cleanly)
```

That is worse than the original `panic!` this file reported: a panic is loud, an
empty string is not.

### Fix

`collect_inherited_interface_default_methods` (`src/inherited_members.rs`) adds
the default methods of the interfaces a type directly implements — and of those
interfaces' super-interfaces — to its inherited members, taking only methods
**with a body** (an abstract interface method is the implementor's own obligation
and already has its own path). Both consumers then do the right thing without
further change: signature planning registers the return type, and lowering emits
the body.

Verified by `tests/differential/InterfaceDefaults.java` (`hi a,yo,hi a!,yo!`),
which covers the inherited default, an override winning over it, and a default
calling another default.

### Residual: one underlying defect, three symptoms

Widening this walk further is blocked on something else, and it is worth naming
precisely because it looks like three separate bugs:

> **A locally declared type can be treated as an external one, so a facade shell
> is synthesised for a type the program already emits.**

It shows up three ways, all reproduced:

1. `class D extends A` where `A implements Greeter` — `D` gets
   `impl D { fn greet() { unimplemented!("A.greet is not modelled") } }`. Adding
   superclass-implemented defaults to the walk therefore turns a silent wrong
   answer into `E0592 duplicate definitions`, so this walk is deliberately
   limited to the type's **own** `implements` until the shell is fixed.
2. `interface P extends Greeter` — a shell for `P`, same duplicate. This is
   distinct from a class directly implementing `P`; the shipped walk follows a
   direct implement's super-interfaces, but does not make interface `P` itself
   inherit `Greeter`'s default while the shell issue is still present.
3. `Greeter.super.greet()` — a shell `pub struct Greeter;` colliding with the
   emitted `pub type Greeter = dyn OuterGreeter;` (`E0428`). The *lowering* of
   `Iface.super` is correct; only the shell is spurious.

2026-08-29 follow-up: the lowering note above was too narrow. The regular
expression path handled `Iface.super`, but slot-aware return lowering could
still bypass it and fall through to the unresolved static-call placeholder. The
parser also parsed nested `Greeter.super` without preserving the interface owner
in expression position. `lower_interface_super_default_call` now resolves both
the preserved `Greeter.super` receiver and resolved interface `super` receivers,
and the slot-aware call path uses the same helper before considering
external/static fallbacks. The nested `InterfaceDefaults.E.greet()` fixture now
emits `InterfaceDefaults_Greeter::__java_default_greet(self)` and the generated
`InterfaceDefaults.java` Rust compiles as a library. Regressions:
`parses_interface_super_method_receiver_with_interface_owner`,
`parses_nested_interface_super_method_receiver_with_interface_owner`, and
`nested_interface_super_default_call_in_slot_aware_return_lowers_to_default_body`.

Diagnosed to `add_used_external_api_facts` (`src/stdlib.rs`), which skips a
receiver only when `local_types` contains it. `local_types` comes from the
**support-context** program, which is empty for a single-file translation, while
`emitted_local_types` — the program actually being emitted — is not consulted.
Nested types compound it: they are keyed qualified (`Outer.Inner`) while source
refers to them simply (`Inner`), so even the qualified key does not match.

### The prototype, and the minimal case that defeated it

Two rounds of prototyping happened and both were **reverted**, because each
fixed some cases and broke another. The state reached is worth writing down so
the next pass does not rediscover it:

| change | effect |
|---|---|
| `add_used_external_api_facts` also skips `emitted_local_types` | removes the `E0428` shell |
| `collect_local_type_keys` also records the **simple** name of a nested type | needed for the above to match, since nested types are keyed `Outer.Inner` while source says `Inner` |
| `source_method_hierarchy_contains` gets the **real** program, not a synthetic one-type program | it walks `extends`, so a one-type program could never find the superclass and always answered "not present" |
| the same check also consults implemented interfaces | a default *is* provided by the hierarchy |

With all four, these pass end to end against the Java oracle:

```java
// D inherits the default through its superclass's `implements`
interface G { String name(); default String greet(){ return "hi " + name(); } }
class A implements G { public String name(){ return "a"; } }
class D extends A     { public String name(){ return "d"; } }   // -> "hi d"   ✅
```

and so do the variants that add an overriding sibling, or a second default.
**This one still fails**, with `E0592 duplicate definitions` for both defaults:

```java
interface G { String name();
              default String greet(){ return "hi " + name(); }
              default String loud(){ return greet() + "!"; } }
class A implements G { public String name(){ return "a"; } }
class D extends A     { public String name(){ return "d"; } }
class B implements G  { public String name(){ return "b"; }
                        public String greet(){ return "yo"; } }   // overrides one default
// A.greet, B.greet, D.greet, A.loud, B.loud   ->   E0592 on greet and loud
```

Two defaults **and** a class overriding one of them **and** a subclass — remove
any one and it passes. The stub is emitted on the subclass naming the superclass
as owner (`unimplemented!("A.greet is not modelled")`), so something still
classifies `A` as an external super for `D` under that combination.

Everything was reverted rather than shipped: the prototype turns a silent wrong
answer into a *compile error* for that case, which is a regression against the
gate even though it is a louder failure. The walk above therefore stays limited
to a type's own `implements`.

### Also observed

`a.greet().length()` on an inherited default's result does not map `length` to
`len()`, even though the return type is now known. Chained calls on an inherited
default's result look like a further gap in the same area; not chased here, and
the fixture avoids the shape rather than pretending it works.
