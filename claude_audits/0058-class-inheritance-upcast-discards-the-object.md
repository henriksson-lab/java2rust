# 0058 — A class upcast discards the object; `super.m()` returns `Default`; `instanceof` is exact-type

Recorded 2026-08-23.
Status: **open, narrowed to class hierarchy carrier**.

Codex follow-up 2026-09-14 identity-preserving erased carrier recovery:
`Object` values cast back to a represented source superclass now keep the
variant object's shared handle instead of cloning the payload. Cast-target
usage feeds the storage lattice for the recovered source hierarchy family,
source hierarchy carrier variants render payload types from the
`RepresentationPlan` storage plan (including nested emitted-name lookup),
`From<Rc<Sub>> for SuperValue` keeps the handle, erased-object source-supertype
casts downcast `Rc<Variant>`/`Arc<Variant>` by borrowing the erased object, and
the carrier emits `java_identity_eq` to compare same-variant handles rather
than outer carrier allocations. `tests/differential/ErasedCarrierIdentity.java`
is promoted from known-gaps and matches Java (`true,false,true,false`).
Regressions:
`idiom_gaps::erased_object_source_hierarchy_carrier_identity_preserves_variant_handle_identity`,
`usage::tests::erased_object_cast_records_target_type_on_source_binding`, and
`storage_strategy::constructor_mutation_tests::erased_object_cast_to_source_supertype_shares_recovered_hierarchy_family`.

Codex follow-up 2026-09-14 carrier conversion cleanup: source hierarchy carrier
conversion no longer also emits a concrete `impl From<Sub> for Super` field-copy
helper. That helper was a latent slicing path beside the correct
`impl From<Sub> for SuperValue` carrier conversion. Same-file and emitted-context
constructor fixtures now assert the carrier conversion and reject the concrete
helper. Carrier field accessors also render from the signature field slot shape,
so nullable inherited fields return `Option<String>` instead of invalidly
returning `String` while cloning an `Option<String>`. Guards:
`idiom_gaps::source_subclass_constructor_to_superclass_slot_uses_hierarchy_carrier_not_flattened`,
`idiom_gaps::emitted_context_source_subclass_constructor_flows_to_superclass_slot`,
and `lower::tests::concrete_source_supertype_conversion_impl_is_loud_not_field_copy`.

Codex follow-up 2026-09-14 wildcard static adapter owner pass:
wildcard-imported `SAMRecordToGATKReadAdapter.headerlessReadAdapter(...)` now
applies the same preferred static owner alias in call planning that support
emission already uses. The call site and emitted support method both use
`SAMRecordToGATKReadAdapter`, so the generated crate no longer fails on
undeclared `OrgBroadinstituteHellbenderUtilsReadSAMRecordToGATKReadAdapter`.
Guard:
`idiom_gaps::wildcard_imported_headerless_read_adapter_has_gatkread_return_surface`.

Follow-up cleanup: `representation_lowering::trait_impl_method_message_policy`
is crate-local. The direct generated-message policy remains available to
representation lowering and same-module tests, but it is no longer exported as a
public crate helper. This does not change the remaining class hierarchy carrier
work.
Follow-up cleanup: `RepresentationTypeEmission` is crate-local too. The
raw-item-vs-concrete-struct decision remains owned by representation lowering;
external tests guard the boundary instead of importing the enum.
Follow-up 2026-09-12: `representation_lowering` is now a crate-internal module.
The generated representation items still flow through the same lowering path,
but the hierarchy-carrier implementation details are no longer exported as
public crate API.
Follow-up 2026-09-10: direct emitted `super.m()` now has Java/Rust oracle
coverage, not only generated-crate coverage. `tests/differential/SuperMethodCall.java`
pins `super.base() + 5` dispatching to the superclass implementation while a
fresh `Base` call still returns `10` (`15,10`). The class-hierarchy carrier
remains open for supertype slots and general mutating super calls.
Codex follow-up 2026-09-13: a narrow mutating `super.m()` shape is now promoted
when the superclass method is field-local: its body can assign or
compound-assign superclass instance fields using literals, params, and
superclass fields, with no calls or `this` escape. Lowering builds a temporary
superclass value, calls the superclass implementation with `&mut`, then copies
the inherited fields back to `self`; non-void calls store the superclass result
until after copyback so the expression can keep Java's order. `tests/differential/SuperMutatingCall.java`
now matches Java (`11`), `SuperMutatingCallReturning.java` matches Java
(`8,4`), `SuperMutatingCallWithTemp.java` matches Java (`14`) for a safe
temporary local inside the superclass body, and more complex mutating super
calls still require the real hierarchy carrier.
Codex follow-up 2026-09-13 hidden-field guard: that narrow mutating-`super`
copyback path now refuses superclass fields hidden by direct subclass fields of
the same Java source name. Copying `Base.n` back through `Derived.n` collapses
two Java fields into one Rust field and is a silent wrong answer, so the case
falls back to the existing loud `unsupported Java mutating super call`
diagnostic until the real hierarchy carrier exists. Regression:
`tests/idiom_gaps.rs::mutating_super_copyback_refuses_hidden_superclass_field`.
Codex follow-up 2026-09-14: the same narrow mutating-`super` copyback path now
accepts field-local superclass bodies that use explicit pure static
`Math`/`StrictMath` calls in copied-back field expressions. The allowlist
requires either resolved static-call evidence or an unshadowed explicit
`Math`/`StrictMath` receiver and still rejects implicit receiver calls and
arbitrary helper calls. Regression:
`tests/idiom_gaps.rs::source_mutating_super_method_call_allows_safe_math_static_call`.
Codex follow-up 2026-09-14: plain boolean `Object instanceof Super` now expands
the erased runtime test across the known source superclass and its known source
subtypes, without binding or casting a supertype view. `InstanceofSupertype.java`
is promoted to `tests/differential/` and matches Java (`true,true,false`).
Unused pattern bindings in statement and ternary `instanceof` tests now take the
same boolean path, so `PatternInstanceofSupertype.java` and
`PatternInstanceofSupertypeTernary.java` are also promoted and match Java
(`A,A,other`). Pattern `instanceof` that reads the bound superclass variable and
other carrier-consuming source-supertype views remain known gaps because they
need a usable superclass carrier, not just a boolean test.
Codex follow-up 2026-09-14 later: explicit source-subtype casts to a represented
source superclass now use the source hierarchy carrier. `NeedsSubtypeDispatch`
can select `SourceHierarchyCarrier`, the carrier emits conservative no-argument
instance-method forwarders, and explicit cast lowering no longer reports the
old `unsupported_inheritance` diagnostic once the target superclass has that
carrier. `ExplicitInheritanceUpcast.java` is promoted and matches Java
(`savings`). Plain assignment/list/nested upcasts still need the upstream usage
fact and broader carrier dispatch surface.
Codex follow-up 2026-09-14 top-level supertype slot: locals initialized or
assigned from a known subtype now contribute `NeedsSubtypeDispatch`, so
`Account a = new Savings(...)` selects the same carrier. Trivial-accessor
inlining is disabled for carrier receivers and for inherited method bodies, so
`a.balance()` calls the carrier forwarder and inherited `describe()` keeps Java
virtual dispatch through `self.balance()`. `InheritanceUpcast.java` is promoted
and matches Java (`savings,105,savings:105`).
Codex follow-up 2026-09-14 collection supertype slot: collection mutator calls
that store a known subtype into a source-supertype element slot now contribute
`NeedsSubtypeDispatch` for the element type, native collection argument shapes
pick up the representation carrier, and carrier reads from `get()` clone into
owned locals before optional receiver calls. `PolymorphicListUpcast.java` is
promoted and matches Java (`savings,105`). Nested source-supertype upcasts
were then fixed by resolving simple nested hierarchy names into their qualified
source type and by using Rust-safe companion-carrier names for nested roots.
`NestedInheritanceUpcast.java` is promoted and matches Java (`woof,woofwoof`).
Erased-object casts to a source superclass now recover through the same carrier
by checking the root and known source subtype variants with `downcast_ref` and
cloning the matched value. `ErasedObjectSupertypeCast.java` is promoted and
matches Java (`B`).
Codex follow-up 2026-09-14 used pattern carrier: `Object instanceof Super s`
now recovers a source hierarchy carrier when the bound variable is actually
used, instead of stopping at the boolean runtime test. The pattern path checks
the root and known source subtype variants, clones the matched value into the
carrier enum, and then dispatches through that carrier. `PatternInstanceofSupertypeUsed.java`
is promoted and matches Java (`B,A,other`).
Superseded guardrail, 2026-09-14 carrier identity guard: source-hierarchy carrier
reference identity recovered from an erased object no longer leaks invalid
`AValue == AValue` Rust. Until the carrier itself preserves object identity, the
path emits an explicit
`unsupported Java source hierarchy carrier reference identity comparison`
`compile_error!` and keeps the existing identity diagnostics. Regression:
`idiom_gaps::erased_object_source_hierarchy_carrier_identity_fails_loudly`.
Superseded known-gap, 2026-09-14 executable identity known-gap: the same carrier
identity residue is now in the differential known-gap harness as
`tests/differential/known-gaps/ErasedCarrierIdentity.java`. Java must print
`true,false,true,false`, while the current translation must fail loudly with the
identity-preserving carrier compile error and `java_reference_identity_equality`
diagnostic. Promote it when erased-object source-supertype recovery preserves
object identity instead of cloning carrier payloads.
Codex follow-up 2026-09-14 call-argument supertype slot: source method
arguments that pass a known subtype into a formal source-supertype parameter
now contribute `NeedsSubtypeDispatch` for the formal target type. The source
hierarchy carrier also emits inherited field accessors, and carrier field reads
lower through those accessors instead of invalid direct enum-field projection.
Regression:
`idiom_gaps::shaped_source_subtype_argument_requires_hierarchy_lowering`.
Codex follow-up 2026-09-14 mutating-super same-file inherited fields: the
narrow copyback path now builds the temporary superclass receiver with inherited
fields from the current source program as well as support/summary context. A
same-file chain `Root.n <- Base.bump() <- Derived.super.bump()` no longer emits
a superclass temporary that omits `n`; cross-program same-simple ambiguity still
fails closed. Regressions:
`idiom_gaps::source_mutating_super_copyback_includes_same_file_inherited_fields`
and
`lower::tests::source_instance_field_names_for_conversion_refuses_cross_program_ambiguous_simple_super`.
Severity: **critical** — the original silent source-class upcast and
most source-supertype `instanceof` paths are now loud or correct, and direct
emitted `super.m()` calls can lower to the superclass implementation, but Java
class hierarchy lowering still has an incomplete carrier surface for
general mutating super calls and broader class-hierarchy representation.
Kind: behavioural (differential), object model / dispatch.

Updated 2026-09-06 (guardrail): erased-object aliases for nested Java classes
now use the emitted Rust type name instead of the Java dotted owner path.
`AliasMin.A` therefore emits `pub type AliasMin_A = Box<dyn std::any::Any>;`
rather than invalid `pub type AliasMin.A = ...`. Regression:
`representation_lowering::type_emission_tests::erased_object_alias_uses_rust_name_for_nested_java_type`.
This only removes the parse-error wrapper around the intended loud
class-hierarchy refusal; it is not the real hierarchy carrier.

Updated 2026-09-07 (stale guardrail cleanup, rechecked 2026-09-10): the
`ZipOutputStream -> OutputStream` external facade upcast is modelled by the
external pack and the generated crate compiles through `zip.into()`. The old
unsupported-upcast expectation is now a positive generated-crate idiom fixture,
with `idiom_gaps` at `816/0` and the failure-name file header-only. This does
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
overriding, virtual dispatch through a supertype reference, shadowed fields,
`instanceof` over a chain, polymorphic collections.
**19 shapes probed, 7 divergent.** Interfaces are correct for the probed
top-level and nested dispatch shapes; class `extends` remains open where it
needs a real source-class hierarchy carrier.

Repros: `claude_audits/repro/InheritanceUpcast.java`,
`InstanceofSupertype.java`, `NestedInterfaceDispatch.java`. Historical
positive control: `SuperMethodCall.java` now has differential coverage for the
direct read-only `super.m()` shape and is not part of the remaining carrier
gap.

Relation to [0002b](0002b-shared-representation-design.md): that design is about
*aliasing* — two names for one object. **None of the divergences here are
aliasing.** Every repro below has exactly one reference to each object. They are
about *subtyping*: a value whose static type is a supertype must keep its
dynamic type. A shared representation is necessary for that (a `Base` slot has
to be able to hold a `Sub`) but the missing piece is the hierarchy lowering that
0002b assumes already exists. Filing separately for that reason.

---

## 1. Historical upcast-to-superclass slicing — now loud

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

### 1b. The same program with *nested* classes now uses the carrier too

Current status: fixed. `tests/differential/NestedInheritanceUpcast.java` pins
the Java oracle `woof,woofwoof`; the generated Rust uses
`NestedInheritanceUpcast_AnimalValue` and forwards `speak()`/`twice()` through
the carrier.

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

## 3. Historical exact-type `instanceof` lowering — now loud

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
  the generated `impl From<Sub> for Base` in `src/item_assembly.rs`. Historical
  output fell back to a field-copy struct rebuild for source types and
  `Default::default()` for external types; current source-class paths are loud,
  and external adapters either model the conversion or panic if the adapter is
  still unsupported.
- **`super.m()`** — historically resolved to no target and took the "unknown
  call" default, `Default::default()`, and was also misclassified as recursion
  upstream (`recursion_tco_fallback` in the TCO analysis). Direct emitted
  non-mutating `super.m()` calls now target the superclass implementation, while
  mutating super calls remain loud until a real hierarchy carrier exists.
- **`instanceof`** — `Box<dyn Any>` + `Any::is::<T>` is the *only* runtime type
  test available, and it is exact by construction
  (`src/conversion_plan.rs:InstanceOfFallbackPolicy`,
  `DowncastErasedObject`).
- **diagnostic vs lowering** — current source-class guardrails align the two
  for pinned upcast/cast/`instanceof` cases: `unsupported_inheritance` or the
  source-hierarchy runtime-carrier diagnostic is emitted, and the generated Rust
  fails loudly through `compile_error!`. This still documents the absence of a
  real carrier rather than solving it.

The interface path shows what the class path is missing: `src/item_assembly.rs`
already emits a trait with `as_any` + `clone_box`, per-impl forwarding, default
method bodies, and a `Box<dyn Trait>` carrier — and it is correct. Classes need
the same treatment, plus a subtype-aware `instanceof` (a `dyn Trait` +
`as_any().is::<T>()` for each concrete type in the chain, or a class-id chain
walk).

---

## Proposed direction

The cheap loudness guardrails described in the original direction have landed:
source-class upcasts, nested upcasts, source-supertype `instanceof`, and
mutating `super.m()` calls now refuse rather than silently fabricating values.
The remaining implementation work is the substantive class hierarchy carrier:
extend the interface trait machinery to classes so a supertype slot can hold a
subtype value with dynamic dispatch, and make subtype-aware `instanceof` fall out
of that runtime carrier.

---

## Suggested fixture placement

`InheritanceUpcast.java`, `NestedInheritanceUpcast.java`,
`ErasedObjectSupertypeCast.java`, `PatternInstanceofSupertypeUsed.java`, and
`InstanceofSupertype.java` compile and run, and are now promoted under
`tests/differential/`. The remaining hierarchy fixtures under
`tests/differential/known-gaps/` keep proving both sides of the contract: the
Java oracle remains valid and the translation fails loudly. Move them up a
level when class hierarchy lowering lands.

`SuperMethodCall.java` is no longer a known gap: it is pinned by
`source_super_method_call_uses_direct_emitted_super_impl`, which compiles the
generated Rust and rejects the old `Default::default()` lowering.

`NestedInterfaceDispatch.java` is now in `tests/differential/`.

The main `tests/differential/` suite now contains positive concrete inheritance,
direct `super`, and interface coverage. The source-class subtype-carrier cases
remain explicit known gaps under `tests/differential/known-gaps/` until the
class hierarchy carrier lands.

Follow-up 2026-09-09: `PolymorphicListUpcast.java` was added beside the direct
upcast fixtures so the collection-supertype variant had a known-gap contract.
Follow-up 2026-09-14: the fixture is promoted; `ArrayList<Account>.add(new
Savings(...))` now stores the `AccountValue` carrier and Java/Rust both produce
`savings,105`.

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

- `source_subclass_constructor_to_superclass_slot_is_loud_not_flattened`

## Codex follow-up 2026-08-28, revised 2026-09-14 — optional source-supertype present values use the carrier

The ordinary assignment and argument guards were originally bypassed when a
fresh source subtype was placed into an optional source-supertype slot. Nullable
field storage and constructor-argument storage could lower `Child ->
Option<Base>` as `Some(Child::new())`, letting the optional wrapper hide the
missing class hierarchy carrier.

The current tree has the narrow carrier-backed repair. Assigning a known source
subtype into a field whose declared type is a source supertype now records a
`NeedsSubtypeDispatch` requirement for the field type; direct constructor
arguments do the same for the resolved formal parameter type even when the
argument is `new Child()` rather than a local binding. The slot conversion wraps
the resulting `BaseValue` carrier in the target shared handle, so the generated
field/argument values are `Rc<BaseValue>` rather than bare `BaseValue` or
`Some(Child)`.

The same pass fixed a traversal bug in the implicit-default-constructor
emission check: `new Holder(new Child())` now counts the nested `new Child()`
use, so the generated `Child::new()` call has a matching implicit constructor.

Pinned by:

- `optional_source_supertype_field_assignment_uses_carrier_handle`
- `optional_source_supertype_constructor_arg_uses_carrier_handle`
- `source_constructor_direct_known_subtype_argument_records_target_subtype_dispatch_use`

Manual generated-library checks also pass for the two minimal fixtures in
`/tmp/new_java2rust_0058_optional_check/`:

- `OptionalSourceSupertypeField.java`
- `OptionalSourceSupertypeCtorArg.java`

The audit remains open for the broader class hierarchy surface, especially
general mutating super calls.

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
positive compile-backed check. The remaining output uses the adapter-backed
external upcast and compiles, so it no longer belongs in the idiom-gaps failure
baseline.

Follow-up 2026-09-07, narrowed 2026-09-13: direct emitted `super.m()` lowering
still refuses general mutating superclass methods loudly. The read-only direct
call path constructs a borrowed super-literal and is covered by
`source_super_method_call_uses_direct_emitted_super_impl`; the simple
field-local mutating shape now uses a superclass temporary plus inherited-field
copyback and is covered by
`idiom_gaps::source_mutating_super_method_call_uses_copyback_for_field_local_body`.
Follow-up 2026-09-14: the same field-local shape now also supports ordinary
non-void returns by storing the superclass result, copying fields back, then
returning the stored value. Regression:
`idiom_gaps::source_mutating_super_method_call_returns_value_after_copyback`,
with Java-oracle coverage in `SuperMutatingCallReturning.java`. Calls, `this`
escaping, result/exception methods, and other complex mutating super bodies
still require the real source-class hierarchy carrier.
Follow-up 2026-09-14 later: that conservative copyback safety check now also
allows forward-only temporary locals initialized from superclass fields, params,
other safe locals, and literals. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_temporary_local`;
Java-oracle coverage: `SuperMutatingCallWithTemp.java`.
Codex follow-up 2026-09-14: the same narrow copyback path now admits safe
conditional and nested-block bodies. Branch conditions must read only inherited
fields, parameters, literals, or already-known safe locals, and branch-local
declarations are scoped to the branch for the safety check. This keeps the
repair limited to methods that mutate inherited fields through a superclass
temporary and copy those fields back immediately after the call. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_conditional_body`;
Java-oracle coverage: `SuperMutatingCallConditional.java`.
Codex follow-up 2026-09-14 later: the safety check now distinguishes declared
locals from safe-to-read locals, so `int next; next = n + value; this.n = next;`
is admitted without treating declaration alone as proof that a local is
initialized. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_declared_then_assigned_local`;
Java-oracle coverage: `SuperMutatingCallAssignedLocal.java`.
Codex follow-up 2026-09-14 later still: safe ternary expressions are admitted
inside the same copyback check when the condition and both branches are
individually safe. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_ternary_local`;
Java-oracle coverage: `SuperMutatingCallTernaryLocal.java`.
Safe array reads are admitted too when both the array expression and index are
already safe, allowing superclass bodies such as `this.n += values[i]` without
opening calls or `this` escape. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_array_access`;
Java-oracle coverage: `SuperMutatingCallArrayAccess.java`.
Safe local array initializers are admitted under the same rule: dimensions and
initializer elements must be safe expressions, and the resulting local only
becomes readable through the existing safe-local path. This covers local
scratch arrays in field-local superclass methods without allowing arrays to
escape. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_local_array_initializer`;
Java-oracle coverage: `SuperMutatingCallLocalArray.java`.
Local scratch-array element writes are admitted for arrays declared and proven
safe inside the superclass method, again with safe index/value expressions.
At the time of that pass, parameter-array writes stayed outside this repair
because they have caller-visible aliasing semantics beyond inherited-field
copyback. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_local_array_writes`;
Java-oracle coverage: `SuperMutatingCallLocalArrayWrite.java`.
Codex follow-up 2026-09-14 later: parameter-array element writes are now
admitted for the narrow mutating-`super` copyback path when the superclass
method signature itself requires a mutable array argument. Usage recording
normalizes explicit `super.m(...)` call sites to the direct superclass owner so
callee parameter constraints propagate to the derived method, and
super-boundary argument lowering now takes its slots from the superclass
signature. `super.add(values, value)` can therefore pass `values.as_mut_slice()`
through to `Base::add(&mut __java_super, ...)`, preserving both caller-visible
array mutation and inherited-field copyback. The read-only array access fixture
still stays read-only. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_parameter_array_write`.
Unary updates of those same local scratch-array elements are admitted under the
same target check. This is deliberately narrower than general array mutation:
only array locals declared and proven safe inside the superclass body qualify.
Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_local_array_unary_updates`;
Java-oracle coverage: `SuperMutatingCallLocalArrayUnary.java`.
The same local scratch-array target check now applies to classic `for` update
clauses, including unary updates and assignment-form updates such as
`remaining[0] -= 1`. This keeps loop scratch state inside the superclass
temporary/copyback model while still refusing parameter-array mutation.
Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_local_array_for_updates`;
Java-oracle coverage: `SuperMutatingCallLocalArrayForUpdate.java`.
Codex follow-up 2026-09-14 final: branch definite-assignment is now tracked for
locals declared before an `if` and assigned in both branches. Branch-local
declarations still do not leak out, but `int delta; if (...) { delta = value; }
else { delta = 0; } this.n = n + delta;` is admitted. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_if_else_assigned_local`;
Java-oracle coverage: `SuperMutatingCallIfElseAssignedLocal.java`.
Codex follow-up 2026-09-14 final-plus: expression-statement unary updates of
inherited fields (`this.n++`, `--n`, etc.) are admitted for the same superclass
temporary/copyback path. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_unary_inherited_field_update`;
Java-oracle coverage: `SuperMutatingCallUnaryField.java`.
Codex follow-up 2026-09-14 final-plus two: expression-statement unary updates
of already-safe temporary locals are admitted as well. The local must already be
safe-to-read; a declaration without a safe initializer still does not qualify.
Regression:
`idiom_gaps::source_mutating_super_method_call_allows_unary_safe_local_update`;
Java-oracle coverage: `SuperMutatingCallUnaryLocal.java`.
Codex follow-up 2026-09-14 final-plus three: expression-statement unary updates
of method parameters are admitted too. Parameters are local to the superclass
method call on the temporary receiver, so `value++; this.n += value;` stays
inside the bounded copyback repair. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_unary_parameter_update`;
Java-oracle coverage: `SuperMutatingCallUnaryParam.java`.
Codex follow-up 2026-09-14 final-plus four: `while`/`do while` bodies and
simple classic `for` loops are admitted when their initializer, condition,
update, and body stay safe under the same inherited field/parameter/safe-local
rules. Loop updates may be unary increments/decrements or assignment-form
updates (`i += 1`, `value -= 1`) when the target is an inherited field,
parameter, or already-safe local and the right side is safe. `break`/`continue`
statements inside those safe bodies are admitted as ordinary loop control,
including labeled forms when the labeled block itself is otherwise safe. The
loop analysis deliberately does not propagate new definite-assignment facts out
of the loop. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_while_loop` and
`idiom_gaps::source_mutating_super_method_call_allows_safe_do_while_loop`, and
`idiom_gaps::source_mutating_super_method_call_allows_safe_for_loop`,
`idiom_gaps::source_mutating_super_method_call_allows_safe_for_assignment_update`, plus
`idiom_gaps::source_mutating_super_method_call_allows_safe_loop_control` and
`idiom_gaps::source_mutating_super_method_call_allows_safe_labeled_loop_control`;
Java-oracle coverage: `SuperMutatingCallWhileLoop.java`,
`SuperMutatingCallDoWhileLoop.java`, `SuperMutatingCallForLoop.java`,
`SuperMutatingCallForAssignmentUpdate.java`, and
`SuperMutatingCallLoopControl.java`/`SuperMutatingCallLabeledLoopControl.java`.
Safe `switch` statement bodies are admitted under the same rule: the selector,
case labels, and case statements must all be safe. A local declared before the
switch is treated as safe after the switch only when there is a `default` label
and every case body leaves that local safe, matching the same conservative
all-branches rule used for `if`/`else`. Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_switch_body`;
Java-oracle coverage: `SuperMutatingCallSwitch.java`. Follow-up 2026-09-14:
`idiom_gaps::source_mutating_super_method_call_allows_safe_switch_assigned_local`
pins the default-covered assigned-local variant.
Enhanced-for loops are admitted when the iterable is safe; the loop variable is
treated as safe only inside the loop body, and no loop-local facts propagate out.
Regression:
`idiom_gaps::source_mutating_super_method_call_allows_safe_enhanced_for_loop`;
Java-oracle coverage: `SuperMutatingCallEnhancedFor.java`.
Safe static `Math`/`StrictMath` calls inside mutating superclass bodies are now
Java-oracle covered too, not only asserted through generated Rust text. This
keeps `this.n = Math.max(this.n, value)` on the same bounded temporary/copyback
path while still refusing arbitrary calls that could observe or mutate `this`.
Java-oracle coverage: `SuperMutatingCallMathStaticCall.java`.

Follow-up 2026-09-10: the remaining non-carrier hierarchy gap is now pinned by
additional differential known-gap Java oracles, not only idiom assertion text.
Added `ExplicitInheritanceUpcast.java`, `ErasedObjectSupertypeCast.java`,
`PatternInstanceofSupertype.java`, `PatternInstanceofSupertypeTernary.java`,
and initially `SuperMutatingCall.java` under `tests/differential/known-gaps/`;
the known gap harness validates their Java outputs and the current loud failure
mode. `SuperMutatingCall.java` was promoted on 2026-09-13 after the narrow
field-local copyback lowering landed; `ExplicitInheritanceUpcast.java`,
`InheritanceUpcast.java`, `PolymorphicListUpcast.java`,
`NestedInheritanceUpcast.java`, and `ErasedObjectSupertypeCast.java` were
promoted on 2026-09-14 as the source hierarchy carrier path grew from explicit
casts to plain top-level supertype locals, collection element slots, nested
source-class locals, and erased-object source-supertype casts. Used pattern
`instanceof` over a source supertype was promoted later the same day.
The lowering diagnostics pass now also reports `unsupported_inheritance` for the
remaining explicit source-subtype cast refusals, so the known-gap harness
checks both the `compile_error!` surface and the diagnostic code where a
hierarchy view is still unsupported.

Follow-up 2026-09-10 cleanup: representation-lowering's dyn-trait
implementation compatibility policy carriers
(`TraitImplMethodFallbackPolicy`, `TraitImplMethodCompatibilityError`, and
`TraitImplMethodMessagePolicy`) are crate-local. This only narrows the helper
surface around current trait-impl diagnostics; it does not implement the source
class hierarchy carrier.
Follow-up cleanup: `TraitImplMethodMessagePolicy::compile_error_body` is private
to `representation_lowering`, so the crate-local policy no longer exposes a
public message-rendering method on the compatibility carrier.

Follow-up 2026-09-10 known-gap harness hardening: `LoudCompileError`
expectations can now require or forbid generated Rust fragments in addition to
checking the `compile_error!` message and diagnostic code. The source hierarchy
known gaps use that to reject stale slicing/exact-type shapes such as
`Savings::new(...).into()`, copied mutating `Base::bump(&...)`, exact
`.is::<A>()`, and exact `downcast_ref::<A>()` paths. This keeps the executable
known-gap fixtures self-contained while the real class hierarchy carrier
remains open.

Codex follow-up 2026-09-12: `object_model::ObjectModelInput` is no longer a
public field bag. Representation planning and object-model tests construct the
solver entrypoint through `ObjectModelInput::new(...)`, and the solver reads
program/facts/effective-facts/usage through accessors. This only fences the
object-model boundary; the source-class hierarchy carrier remains open. Guard:
`layering::object_representation_policy_source_boundaries_stay_in_object_model`.

Codex follow-up 2026-09-12: hierarchy reachability evidence is fenced the same
way. `facts::HierarchyReachabilityResult` now exposes edges, worklist stats,
and diagnostics through accessors/`into_parts()` instead of public mutable
fields. This does not implement the class hierarchy carrier, but it keeps the
current reachability saturation boundary from becoming another raw fact bag.
Guard: `layering::coercion_plan_owns_instanceof_fallback_policy`; focused
guards:
`slot_shape_design::facts_saturate_reachable_hierarchy_edges_through_finite_domain`
and
`slot_shape_design::facts_saturate_recursive_hierarchy_edges_without_unbounded_expansion`.
