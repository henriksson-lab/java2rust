# 0058 — A class upcast discards the object; `super.m()` returns `Default`; `instanceof` is exact-type

Recorded 2026-08-23.
Status: **open**.
Severity: **critical** — the original silent source-class upcast and
source-supertype `instanceof` paths are now loud, and direct emitted
`super.m()` calls can lower to the superclass implementation, but Java class
hierarchy lowering still has no real carrier for dynamic class identity and
dispatch.
Kind: behavioural (differential), object model / dispatch.

Updated 2026-09-06 (guardrail): erased-object aliases for nested Java classes
now use the emitted Rust type name instead of the Java dotted owner path.
`AliasMin.A` therefore emits `pub type AliasMin_A = Box<dyn std::any::Any>;`
rather than invalid `pub type AliasMin.A = ...`. Regression:
`representation_lowering::type_emission_tests::erased_object_alias_uses_rust_name_for_nested_java_type`.
This only removes the parse-error wrapper around the intended loud
class-hierarchy refusal; it is not the real hierarchy carrier.

Updated 2026-09-07 (stale guardrail cleanup): the `ZipOutputStream ->
OutputStream` external facade upcast is now modelled by the external pack and
the generated crate compiles through `zip.into()`. The old idiom-gap row still
expected the unsupported-upcast `compile_error!`; it has been converted to a
positive generated-crate check and removed from the failure baseline. This does
not change the remaining source-class hierarchy carrier work.

Updated 2026-08-30 current-tree triage: the original silent object-slicing
behavior is no longer current for ordinary source-class upcasts; expression
lowering and the generated concrete source hierarchy conversion helper now fail
loud with the class-hierarchy carrier diagnostic instead of fabricating a
superclass value. The audit remains valid as a missing-feature complaint:
`Base b = new Sub(); return b.m();` still cannot preserve Java dynamic dispatch
through the supertype slot. A real fix needs a member-preserving source class
hierarchy carrier with subtype-aware runtime identity and dispatch, not another
local conversion adapter. Existing guardrails include
`lower::tests::concrete_source_supertype_conversion_impl_is_loud_not_field_copy`
and the known-gap `InheritanceUpcast.java`.

Updated 2026-08-26 (guardrail): the generated concrete source hierarchy
`impl From<Sub> for Super` helper no longer rebuilds the superclass by copying
the apparent superclass fields out of the subclass value. If an unsupported
source hierarchy conversion reaches that impl, it now panics with the same
`unsupported Java class upcast: Sub to Super requires class hierarchy lowering`
message used by the expression-level refusal. Closed-enum hierarchy-carrier and
erased-object carrier conversions remain unchanged. Regression:
`lower::tests::concrete_source_supertype_conversion_impl_is_loud_not_field_copy`.
This is still not the class hierarchy carrier; it only prevents an accidentally
reachable conversion helper from silently slicing object state.

Updated 2026-08-26: the narrow exception-facade variant is now loud too.
`new ExceptionContext(this)` where the constructor wants `Throwable` used to
fabricate `Default::default()/* TODO(translation): Java exception
self-as-supertype facade boundary */`, losing the source exception object while
still compiling. `lower_current_self_cast_for_slot` now emits the same
unsupported class-upcast `compile_error!` used by the ordinary source hierarchy
path. Regression:
`idiom_gaps::exception_self_as_throwable_context_argument_is_loud_upcast_gap`.
The real class-hierarchy carrier remains open.

Updated 2026-08-26 again: the loud source-class upcast and source-supertype
`instanceof` guards no longer priority-chain context programs when deciding
whether a hierarchy carrier is required. They now use the combined
ambiguity-aware context lookup, so bare `Child`/`Base` owners fail closed when
same-simple package peers are visible, while exact `a.Child`/`a.Base` still
trigger the class-hierarchy `compile_error!`. Regressions:
`lower::tests::source_class_upcast_hierarchy_guard_refuses_ambiguous_context_owner`
and
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_context_target`.

Updated 2026-08-24: the running-lie paths in §1, §2, §3, and external facade
upcast adapters are now loud for source classes. A source class upcast such as
`Account a = new Savings(...)` lowers the value expression through
`compile_error!` instead of rebuilding an `Account` and discarding the dynamic
type; source `super.m()` calls no longer become typed zero/default values; and
`Object o; o instanceof A` now refuses when `A` has known source subtypes,
instead of using exact `Any::is::<A>()`. Pattern `instanceof`
uses the same refusal for statement and expression positions. External
`impl From<X> for Y` upcast adapters keep the `Unmodelled Java upcast` marker
but panic when used instead of returning `Default::default()`. Context-summary
`super` calls still keep their explicit summary boundary because those bodies
are intentionally not translated. The audit remains open for the real
class-hierarchy carrier.

Updated 2026-08-24 again: re-triage confirmed the proposed "make unsupported
inheritance/super/instanceof loud" mitigation has mostly landed. Treat the
historical silent-output examples below as the reason the guardrails exist, not
as current output. The remaining implementation work is the substantive class
hierarchy representation: a supertype slot must hold subtype values with
dynamic dispatch and subtype-aware `instanceof`.

Found by differential testing along the JLS Ch. 8/9 axis: abstract classes,
overriding, virtual dispatch through a supertype reference, `super.method()`,
shadowed fields, `instanceof` over a chain, polymorphic collections.
**19 shapes probed, 7 divergent.** Interfaces are correct for the probed
top-level and nested dispatch shapes; class `extends` is almost entirely broken.

Repros: `claude_audits/repro/InheritanceUpcast.java`,
`SuperMethodCall.java`, `InstanceofSupertype.java`,
`NestedInterfaceDispatch.java`.

Relation to [0002b](0002b-shared-representation-design.md): that design is about
*aliasing* — two names for one object. **None of the divergences here are
aliasing.** Every repro below has exactly one reference to each object. They are
about *subtyping*: a value whose static type is a supertype must keep its
dynamic type. A shared representation is necessary for that (a `Base` slot has
to be able to hold a `Sub`) but the missing piece is the hierarchy lowering that
0002b assumes already exists. Filing separately for that reason.

---

## 1. Upcast to a superclass discards the subclass — silent, and diagnosed

Current status: fixed to loud for source-class value upcasts. The historical
evidence below describes the bug that `source_class_upcast_requires_hierarchy_lowering`
now pins against.

`claude_audits/repro/InheritanceUpcast.java`

```java
public class InheritanceUpcast {
    public static String run() {
        Account a = new Savings(100, 5);
        return a.label() + "," + a.balance() + "," + a.describe();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
class Account {
    int amount;
    Account(int amount) { this.amount = amount; }
    String label() { return "account"; }
    int balance() { return amount; }
    String describe() { return label() + ":" + balance(); }
}
class Savings extends Account {
    int rate;
    Savings(int amount, int rate) { super(amount); this.rate = rate; }
    String label() { return "savings"; }
    int balance() { return amount + rate; }
}
```

```
java:        savings,105,savings:105
translation: account,100,account:100     (compiles cleanly, exit 0)
```

The lowering:

```rust
pub fn run() -> String {
    let a: Account = Savings::new(100, 5).into();
    return format!("{},{},{}", a.label(), (a.balance()).to_string(), a.describe());
}

impl From<Savings> for Account {
    fn from(value: Savings) -> Self {
        return Account { amount: value.amount };
    }
}
```

`Account` and `Savings` are two **independent flat structs**. There is no trait,
no vtable, no discriminant. The upcast is a struct rebuild that keeps the base
fields and throws away everything else — the subclass field *and* the identity
needed to pick the override. All three answers are wrong, including the one
inside `describe()`, which in Java is the whole point of the method.

The translator does print

```
Unsupported unsupported_inheritance: Savings participates in an extends
hierarchy, but hierarchy lowering is not implemented yet
```

on stderr — and then emits a program that builds and runs. This is the shape
[0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md) was
about: the diagnostic exists, but the artefact that ships is a wrong answer, not
a refusal. **A construct that is known-unsupported should not lower to running
code.** Compare `compile_error!`, which
[0012](done/0012-unsupported-diagnostics-emit-invalid-rust.md) already
established as the mechanism.

### 1b. The same program with *nested* classes now fails loudly too

Current status: fixed to the same loud source-class upcast refusal as the
top-level case. `tests/differential/known-gaps/NestedInheritanceUpcast.java`
pins the Java oracle `woof,woofwoof` and the current `compile_error!`:
`unsupported Java class upcast: NestedInheritanceUpcast.Dog to
NestedInheritanceUpcast.Animal requires class hierarchy lowering`.

```java
public class I1 {
    static class Animal { String speak() { return "generic"; } String twice() { return speak() + speak(); } }
    static class Dog extends Animal { String speak() { return "woof"; } }
    public static String run() { Animal a = new Dog(); return a.speak(); }
}
```

```
error[E0277]: the trait bound `I1Animal: From<I1Dog>` is not satisfied
   let a: I1Animal = I1Dog::new().into();
                                   ^^^^
```

That old `.into()`/missing-`From` evidence is stale. The current translation
emits a class-hierarchy `compile_error!` at the upcast site for nested classes
as well. Reproduced historical probes for abstract classes, field shadowing,
and `List<Base>` holding subclasses (`I2`, `I4`, `N3`) still belong to the same
open design family, but this exact nested upcast is no longer an accidental
Rust trait error.

Note that the shipped JDK facade prelude already contains the *other* no-op
shape:

```rust
impl From<ZipOutputStream> for OutputStream {
    /// Unmodelled Java upcast: ZipOutputStream -> OutputStream.
    fn from(_value: ZipOutputStream) -> Self { Default::default() }
}
```

so a JDK upcast silently yields a default-constructed base. Same defect class,
already present for external types.

Current status: fixed to runtime-loud. External upcast adapters now preserve
the `Unmodelled Java upcast` doc marker for metrics, but their `from` bodies
panic with `unsupported Java external upcast: X -> Y` instead of fabricating
`Default::default()`.

---

## 2. `super.method()` lowered to `Default::default()` — fixed for emitted direct super calls

Current status: direct `super.m()` calls whose direct superclass is emitted now
lower as `SuperType::m(&SuperType { copied fields }, ...)`.
`source_super_method_call_uses_direct_emitted_super_impl` pins this and compiles
the generated Rust. Context-summary `super` boundaries and the broader
supertype/dynamic-dispatch carrier remain open.

`claude_audits/repro/SuperMethodCall.java`

```java
static class Base { int base() { return 10; } }
static class Derived extends Base { int base() { return super.base() + 5; } }
// run(): new Derived().base() + "," + new Base().base()
```

```
java:        15,10
translation: 5,10                        (compiles cleanly, exit 0)
```

```rust
impl S1Derived {
    pub fn base(&self) -> i32 {
        return (0i32).wrapping_add(5);      // super.base() -> Default::default()
    }
}
```

The whole call is replaced by the zero value of the return type. It happens to
fail to compile when the super method returns `String` (`Default::default()` in
a `format!` argument is untyped — `E0790`), which is why the `String` version of
this probe surfaces as a compile error and the `int` version does not. The
`String` version also emits two *misleading* diagnostics:

```
Warning recursion_tco_fallback: I3.Derived::tag is directly recursive but does
not match the safe accumulator loop shape
```

`Derived::tag` is not recursive — `super.tag()` was being classified as a
self-call before it was discarded. That diagnostic path is now corrected for
`super` receivers.

---

## 3. `instanceof` is an exact-type test, not a subtype test — silent

Current status: fixed to loud for ordinary boolean and pattern `instanceof`
checks from an erased `Object` carrier to a source class with known source
subclasses. The real subtype-aware runtime carrier remains open.

`claude_audits/repro/InstanceofSupertype.java`

```java
static class A {}
static class B extends A {}
static boolean isA(Object o) { return o instanceof A; }
// run(): isA(new B()) + "," + isA(new A()) + "," + isA("x")
```

```
java:        true,true,false
translation: false,true,false            (compiles cleanly, exit 0)
```

```rust
pub fn is_a(o: Box<dyn std::any::Any>) -> bool {
    return o.is::<N5A>();
}
```

`Any::is::<T>` is `TypeId` equality — exact runtime type. Java's `instanceof` is
a subtype test. A `B` is an `A` in Java and is not in the translation.

This is easy to miss because the *idiomatic* Java shape hides it: a chain that
tests the most-derived class first

```java
if (o instanceof C) return "C";
if (o instanceof B) return "B";
if (o instanceof A) return "A";
```

produces `C,B,A,?` under both, because each object only ever matches its own
arm. Only a lone `x instanceof Supertype` diverges — which is the common shape
in guard clauses and `equals` implementations. A fixture that only tests the
chain will report green.

Interacts with [0021](done/0021-instanceof-pattern-variable-not-bound.md): the
binding is now correct, the *test* is not.

---

## 4. Interface dispatch is correct — nested coercion name fixed

This is the good news, and it is worth acting on because the fix is one line of
name mangling.

Top-level interfaces work end to end:

```java
interface Speaker { String speak(); }
class Dog2 implements Speaker { public String speak() { return "woof"; } }
class Cat2 implements Speaker { public String speak() { return "meow"; } }
// Speaker a = new Dog2(); Speaker b = new Cat2();
```

```rust
let a: Box<dyn Speaker> = Box::new(Dog2::new()) as Box<dyn Speaker>;
```

```
java:        woof,meow
translation: woof,meow                   MATCH
```

A **nested** interface generates the identical, correct trait — including the
`default` method, `as_any`, and per-impl overrides — but the coercion site
used to render the trait object with the Java dotted name
(`claude_audits/repro/NestedInterfaceDispatch.java`):

```rust
let a: Box<dyn N1Greeter> = Box::new(N1En::new()) as Box<dyn N1.Greeter>;
//         ^^^^^^^^^^^^ mangled correctly              ^^^^^^^^^^^^ not mangled
```

Fixed 2026-08-24. `boxed_dyn_trait_projection` now normalizes the trait name the
same way borrowed dyn casts already did, so both sides of the coercion render as
`Box<dyn NestedInterfaceDispatchGreeter>`. The generated file compiles and
prints `hi bob,salut luc` — the exact Java answer, default method included.
`tests/differential/NestedInterfaceDispatch.java` pins the case in the
57-fixture differential suite.

This is [0013](done/0013-nested-type-names-unmangled-in-downcast.md) at a second
site: the *upcast* coercion rather than the downcast. The left-hand side of the
same generated line already holds the mangled name, so the correct value is in
scope at the point of the bug.

---

## What is correct

Probed and matching — these are the shapes not to spend time on:

| shape | result |
|---|---|
| override called on the **concrete** type (no upcast), including a base method that calls the override | ✅ `2,20` |
| method **inherited** and not redeclared (`Derived` calls `Base.base()`) | ✅ `10,1` |
| `super(...)` **constructor** chaining, including a computed argument and an inherited field read | ✅ `6,6` |
| abstract class used only through the concrete subtype (`Sq q = new Sq(4); q.describe()`) | ✅ `a=16,16` |
| **interface** `dyn` dispatch, top-level: two impls behind one interface-typed slot | ✅ |
| interface **`default` method**, plus an impl that overrides it | ✅ |
| `instanceof` chain ordered most-derived-first | ✅ (for the wrong reason — see §3) |
| user `toString()`/`equals()`/`hashCode()` **explicitly called** | ✅ (implicit `+` is 0059 §1) |

Summary: **inheritance of implementation works; subtyping does not.** Everything
that keeps the static type equal to the dynamic type is right. Every construct
whose whole point is that they differ — upcast, `super`, `instanceof` against a
supertype — is wrong.

---

## Root cause

One missing relation, surfacing at four sites.

A Java class hierarchy is lowered as *n* unrelated Rust structs. There is no
carrier that can hold "a `Base`, possibly a `Savings`", so:

- **upcast** — `src/conversion_plan.rs`, the `From`/`.into()` coercion path, and
  the generated `impl From<Sub> for Base` in `src/item_assembly.rs`. Falls back
  to a field-copy struct rebuild (user types) or `Default::default()` (external
  types, `Unmodelled Java upcast` comment).
- **`super.m()`** — resolves to no target and takes the "unknown call" default,
  `Default::default()`. Also misclassified as recursion upstream
  (`recursion_tco_fallback` in the TCO analysis).
- **`instanceof`** — `Box<dyn Any>` + `Any::is::<T>` is the *only* runtime type
  test available, and it is exact by construction
  (`src/conversion_plan.rs:InstanceOfFallbackPolicy`,
  `DowncastErasedObject`).
- **diagnostic vs lowering** — `unsupported_inheritance` is raised
  (`src/lowering_diagnostics.rs`, which keeps an `extends` map) but does not stop
  code generation.

The interface path shows what the class path is missing: `src/item_assembly.rs`
already emits a trait with `as_any` + `clone_box`, per-impl forwarding, default
method bodies, and a `Box<dyn Trait>` carrier — and it is correct. Classes need
the same treatment, plus a subtype-aware `instanceof` (a `dyn Trait` +
`as_any().is::<T>()` for each concrete type in the chain, or a class-id chain
walk).

---

## Proposed direction, cheapest first

1. **Make `unsupported_inheritance` stop lowering.** Emit `compile_error!` where
   §1 currently emits a running lie, and emit it for the *nested* case too so the
   two agree. Costs nothing, converts the worst severity class (silent wrong) to
   the best (diagnosed). Do this before, not after, the representation work.
2. **Make `super.m()` loud.** `Default::default()` for an unresolved call is the
   0016 no-op-facade pattern; a `super` call whose target is known but not
   representable should refuse, not zero. Separately, check why the TCO analysis
   sees `super.m()` as a self-call.
3. **Extend the interface trait machinery to classes.** A class with subtypes
   gets a trait carrying every method; each class gets an impl; a `Base`-typed
   slot becomes `Box<dyn BaseTrait>`. `super.m()` becomes the base's inherent
   method called explicitly. This is the same shape 0002b needs, and doing it
   through the existing, *working* interface path is much cheaper than a new
   representation.
4. **`instanceof` against a supertype** falls out of 3 (`as_any` downcast per
   concrete subtype). Until then it should refuse rather than answer `false`.

---

## Suggested fixture placement

`InheritanceUpcast.java`, `NestedInheritanceUpcast.java`, and
`InstanceofSupertype.java` compile and run, and are now staged under
`tests/differential/known-gaps/` with
`known_gap_fixtures_are_valid_java_and_match_expected_failure_mode` in
`tests/differential.rs` proving both sides of the contract: the Java oracle
remains valid and the translation fails loudly. The two upcast fixtures also
assert the `unsupported_inheritance` diagnostic; `InstanceofSupertype.java`
asserts `unsupported_instanceof_source_hierarchy_runtime_carrier`, so the
source-supertype `instanceof` refusal is now visible in diagnostics as well as
the generated `compile_error!`. Move them up a level when class hierarchy
lowering lands.

`SuperMethodCall.java` is no longer a known gap: it is pinned by
`source_super_method_call_uses_direct_emitted_super_impl`, which compiles the
generated Rust and rejects the old `Default::default()` lowering.

`NestedInterfaceDispatch.java` is now in `tests/differential/`.

`tests/differential/` currently has **zero** fixtures using `extends`,
`abstract`, or a supertype-typed variable. Every divergence in this file is
invisible to the present gate.

## Codex follow-up 2026-08-25 — type-parameter and argument upcasts are loud

The concrete slot path already refused `Savings -> Account`, but bounded type
parameters and argument adaptation still reached `.into()` first. A `T extends
CompoundLine` value assigned to `HeaderLine`, pushed into `List<HeaderLine>`, or
passed where `CompoundLine` was expected could still lower through the same
field-copy `From` impl that discards dynamic class identity.

`lower_source_subtype_to_supertype_slot`,
`adapt_source_subtype_to_supertype_argument`, and the optional-unwrapped result
upcast path now ask the same source-class hierarchy guard before emitting
`.into()`. When the actual type is a type parameter, the guard checks the
source-class bound and reports the bound as the unsupported upcast source. The
old running outputs are now `compile_error!` sites.

Pinned by:

- `type_param_bound_source_supertype_assignment_requires_hierarchy_lowering`
- `reused_source_subtype_argument_upcasts_after_clone_recovery`
- `reused_source_subtype_owned_argument_upcasts_after_clone_recovery`
- `borrowed_downcast_source_subtype_argument_upcasts_after_clone_recovery`
- `borrowed_downcast_source_subtype_owned_argument_upcasts_after_clone_recovery`
- `source_receiver_generic_result_upcasts_to_supertype_assignment_slot`

## Codex follow-up 2026-08-26 — explicit exception self-casts share the loud guard

The first self-as-exception guard caught `new ExceptionContext(this)`, but the
equivalent explicit cast `new ExceptionContext((Throwable) this)` still flowed
through ordinary cast lowering and emitted a misleading runtime
`downcast::<Throwable>().expect("invalid Java cast")`.

Current-self casts to a known exception supertype now take the same
`compile_error!` path before clone/downcast adaptation, both for shaped slots
and unshaped constructor arguments. This is still only a guardrail: it does not
introduce a source-class hierarchy carrier.

Pinned by:

- `exception_self_as_throwable_context_argument_is_loud_upcast_gap`
- `explicit_cast_exception_self_as_throwable_context_argument_is_loud_upcast_gap`

## Codex follow-up 2026-08-26 — explicit source hierarchy casts are loud

The implicit assignment/argument upcast guard did not cover explicit casts.
`Account a = (Account) new Savings()` could plan the cast target as `Account`
before slot lowering saw the actual source subtype. The cast path now checks
the cast conversion plan directly and emits the same hierarchy `compile_error!`
before normal shape coercion can produce a Rust type mismatch or `.into()`.

Erased-object casts to a source supertype with known source subtypes are loud as
well. `A a = (A) o` where `o` is erased cannot use an exact Rust downcast while
Java accepts subtype instances, so the cast now refuses until the class
hierarchy carrier exists.

Pinned by:

- `explicit_source_class_upcast_requires_hierarchy_lowering`
- `erased_object_cast_to_source_supertype_requires_hierarchy_lowering`

## Codex follow-up 2026-08-26 — stale same-program upcast expectation fixed

The same-program `PairOutputFormat extends BaseOutputFormat` idiom test still
expected the old successful `.into()` flattening path even though the current
policy is a loud hierarchy gap. The test now asserts the expression-site
`compile_error!`, keeps the generated `impl From<PairOutputFormat> for
BaseOutputFormat` panic-only, and rejects the historical field-copy body. This
does not implement the hierarchy carrier; it prevents the previous silent
object-slicing expectation from re-entering the suite.

Regression:

- `source_subclass_constructor_flows_to_flattened_superclass_slot`

## Codex follow-up 2026-08-28 — optional source-supertype present values are loud

The ordinary assignment and argument guards were still bypassed when a fresh
source subtype was placed into an optional source-supertype slot. Nullable field
storage and constructor-argument storage could lower `Child -> Option<Base>` as
`Some(Child::new())`, letting the optional wrapper hide the missing class
hierarchy carrier.

`lower_present_expr_for_optional_target` now checks the actual present value
against the optional element type before the optional construction plan rewrites
owned elements to the target element shape. If the present value is a source
class subtype of the optional element supertype, it emits the same unsupported
class-upcast `compile_error!` used by the non-optional paths.

Pinned by:

- `optional_source_supertype_field_assignment_is_loud_not_subtype_some`
- `optional_source_supertype_constructor_arg_is_loud_not_subtype_some`

The audit remains open for the real subtype carrier and dynamic dispatch model.

## Codex follow-up 2026-08-29 — stale passing repair test removed from failure baseline

`source_hierarchy_instanceof_repair_keeps_supertype_slot_adapted` now passes in
the current tree and was removed from `tests/baselines/idiom-gaps-failures.txt`.
The sealed hierarchy carrier repair remains listed and still represents the
open runtime-carrier gap; this is only baseline cleanup, not closure of 0058.

## Codex follow-up 2026-08-29 — source alias facade test matches current representation policy

`erased_source_alias_owner_suppresses_external_facade_inherent_impl` now asserts
the behavior it was meant to guard: a parsed source type that aliases an
external facade name must not receive generated external facade inherent impls.
Its representation assertion was stale after the object model began keeping
parsed source structs concrete unless an erased fallback is actually required.

## Codex follow-up 2026-08-29 — peer-param fixture no longer depends on string identity

`parsed_source_declared_peer_param_keeps_concrete_signature_under_erased_pressure`
now avoids Java `String` reference identity in its fixture body. The test still
verifies that conflicting erased external pressure is filtered for parsed
source classes and that peer parameters stay concrete, but generated-crate
checking no longer fails on the separate string-identity representation gap.

## Codex follow-up 2026-08-29 — generic source-supertype fixture is explicitly loud

`generic_source_supertype_into_bound_gets_static_for_erased_boxing` still checks
that the generic bound includes the source-supertype conversion surface, but no
longer asks generated-crate checking to compile the intentional
`compile_error!`. The fixture now pins the current 0058 policy directly:
`Mid -> Base` remains a loud class-hierarchy lowering gap until the source
hierarchy carrier exists.

## Codex follow-up 2026-08-29 — external upcasts use explicit adapter surfaces

The previous local note claiming `ExternalInto` coercions should be compile-loud
at every use site was superseded by the adapter-backed conversion pass recorded
in 0052. When an explicit external conversion adapter or built-in JDK stream
adapter exists, the call site now lowers through the ordinary Rust `.into()`
projection and the generated support owns the corresponding `From<X> for Y`
surface. Unsupported source-class upcasts remain loud; adapter-backed external
facade conversions are no longer part of this audit's open class-hierarchy
carrier gap. The
`external_upcast_coercion_uses_adapter_into_projection_at_use_site` fixture now
expects the structured `zip.into()` projection.

Follow-up 2026-08-29: the optional resource/facade-upcast fixture now pins the
same current policy. It still verifies that a nullable resource local is cloned
before the null unwrap so later `finally` cleanup can read it, but the
`ByteArrayOutputStream -> JavaIoOutputStream` constructor argument upcast lowers
through the adapter-backed `.into()` projection rather than being counted as a
source-class hierarchy gap. The fixture was removed from the idiom-gaps
expected failure baseline.

## Codex follow-up 2026-08-29 — summary super equals no longer fabricates false

`negated_boolean_super_boundary_call_uses_loud_summary_method_not_default` now
pins the current source-summary boundary policy. A `super.equals(o)` call that
resolves only to a summary superclass lowers to a local runtime-loud expression
with
`unimplemented!("source-summary method SummaryEqualsBase.equals is not translated")`.
It no longer asks for the old fabricated `if !false` fallback or emits a call to
the non-emitted summary owner.

## Codex follow-up 2026-08-29 — sealed carrier upcasts use the carrier conversion

The sealed source hierarchy repair fixture now compiles. The assignment path
was still rejecting every source class upcast before it reached the
representation-aware conversion, so `FormatLine -> HeaderLine` and
`InfoLine -> HeaderLine` emitted the old hierarchy `compile_error!` even though
`HeaderLine` is represented by `HeaderLineValue`.

`lower_source_subtype_to_supertype_slot` and the reused-argument adapter now
let closed-enum carrier targets use the existing `From<Subtype> for
CarrierValue` conversion instead of the loud fallback. Erased-object display
support also skips source hierarchy carrier roots by dispatch policy, avoiding
invalid `downcast_ref::<HeaderLine>` arms for roots that only have a carrier
enum at runtime.

Pinned by:

- `sealed_source_hierarchy_instanceof_repair_uses_closed_enum_runtime_carrier`
- `sealed_source_hierarchy_instanceof_pattern_binds_closed_enum_payload`

The audit remains open for non-carrier inheritance and wider dynamic dispatch
lowering; this closes the sealed carrier repair path only.

## Codex follow-up 2026-08-30 — shaped subtype arguments use the loud adapter

`lower_source_subtype_argument_as_actual` no longer returns the bare lowered
argument before the source-supertype adapter can inspect it. Shaped argument
lowering now routes the subtype value through
`adapt_source_subtype_to_supertype_argument`, so unsupported source-class
upcasts at method-call argument boundaries emit the existing
`compile_error!("unsupported Java class upcast: ... requires class hierarchy lowering")`
instead of depending on a fabricated `From<Sub> for Base` conversion or a raw
call-site coercion.

Pinned by:

- `shaped_source_subtype_argument_requires_hierarchy_lowering`

The audit remains open for the actual non-carrier class hierarchy
representation and dynamic dispatch model.

Follow-up 2026-09-07:
`optional_resource_facade_upcast_clones_wrapper_before_consuming_unwrap` is now a
positive compile-backed check. The remaining output still uses the placeholder
external upcast, but it compiles and no longer belongs in the idiom-gaps failure
baseline.

Follow-up 2026-09-07: direct emitted `super.m()` lowering now refuses mutating
superclass methods loudly. The read-only direct call path still constructs a
borrowed super-literal and is covered by
`source_super_method_call_uses_direct_emitted_super_impl`, but a mutable
receiver would mutate a copied temporary or emit invalid Rust. That path now
emits
`compile_error!("unsupported Java mutating super call: ... requires class hierarchy lowering")`
until the real source-class hierarchy carrier exists. Regression:
`idiom_gaps::source_mutating_super_method_call_is_loud_hierarchy_gap`.
