# Rust Slot Shape And Object Representation

## Purpose

This document locks in the design direction for Rust signature, ownership, view, dispatch, and object-model decisions.

The translator must not decide whether a Java value becomes `String`, `&str`, `Vec<T>`, `&[T]`, `&dyn Trait`, `Box<dyn Trait>`, or a named enum at the point where Java types are parsed or where expressions are locally lowered.

Instead, Java semantic typing, Rust slot-shape solving, object representation, and coercion must stay separate.

This document supersedes narrow string-specific ownership planning. Strings remain an important case, but the design must scale to arrays, collections, primitives, user classes, interfaces, subclassing, casts, and user-supplied hierarchy facts.

## Core Principle

Lowering does not choose ownership or object representation.

Lowering receives a solved plan:

- what Rust type each parameter, return, field, local, and temporary slot should have
- whether a slot is owned, borrowed, mutable, copied, sliced, string-viewed, dynamically dispatched, or enum-dispatched
- what adapters are needed at expression boundaries

Lowering then emits the chosen representation and asks coercion to adapt actual expressions into target slots.

## Java Type Is Not Rust Shape

Java type inference owns Java semantics:

```text
Java parameter: String s
Java type: java.lang.String
Rust slot shape: unresolved
```

The Rust slot may later become any of:

```rust
s: String
s: &String
s: &str
s: Cow<'_, str>
```

That decision belongs to the Rust slot-shape solver, not the Java type pass.

Likewise:

```text
Java List<String>
```

may become:

```rust
Vec<String>
&[String]
&mut Vec<String>
Box<dyn JavaList<String>>
```

depending on usage, ownership requirements, object-model strategy, and configured policy.

## Slot Shape

The solver should produce a structured shape rather than a flat enum with one variant per type-specific case.

```rust
struct RustSlotShape {
    java_ty: JavaType,
    ownership: OwnershipShape,
    view: ViewShape,
    dispatch: DispatchShape,
    mutability: MutabilityShape,
    type_identity: TypeIdentity,
}
```

### Ownership Shape

```rust
enum OwnershipShape {
    Owned,
    SharedBorrow,
    MutableBorrow,
    CopyValue,
}
```

Examples:

```text
String
=> Owned

&str
=> SharedBorrow

&mut Vec<T>
=> MutableBorrow

i32
=> CopyValue
```

### View Shape

```rust
enum ViewShape {
    Native,
    Str,
    Slice,
    DynTraitView,
    EnumView,
    ErasedObjectView,
}
```

Examples:

```text
Java String as Rust String
=> view = Native

Java String as Rust &str
=> view = Str

Java List<T> or T[] as &[T]
=> view = Slice
```

`ViewShape` should not absorb object dispatch by itself. Interfaces, subclassing, trait objects, and named enum hierarchies need the separate `DispatchShape` axis below.

### Dispatch Shape

```rust
enum DispatchShape {
    Concrete,
    DynTrait { trait_name: String },
    BoxedDynTrait { trait_name: String },
    ClosedEnum { enum_name: String },
    ErasedObject,
}
```

Examples:

```text
Java Dog as Dog
=> Concrete

Java Animal as &dyn Animal
=> DynTrait("Animal")

Java Animal as Box<dyn Animal>
=> BoxedDynTrait("Animal")

Java Expr as ExprValue enum
=> ClosedEnum("ExprValue")

Unknown raw Java Object
=> ErasedObject
```

### Mutability Shape

```rust
enum MutabilityShape {
    Immutable,
    Mutable,
}
```

Mutability is separate from ownership because a binding can be reassigned without implying mutable access to the object contents, and Java reference mutability does not map cleanly to Rust binding mutability.

### Type Identity

```rust
struct TypeIdentity {
    declared_java_ty: JavaType,
    runtime_bounds: Vec<JavaType>,
    generic_args: Vec<JavaType>,
    erasure: Option<JavaType>,
}
```

This preserves facts needed for Java generics, raw types, wildcard bounds, casts, and subtype checks.

## Capabilities

Rules should be driven by capabilities, not by a long list of special cases.

```rust
struct TypeCapabilities {
    copy: bool,
    clone: bool,
    string_like: bool,
    sequence_like: Option<JavaType>,
    implements: Vec<JavaType>,
    extends: Option<JavaType>,
    final_type: bool,
    object_safe_trait: bool,
}
```

Capabilities answer questions such as:

- can this value be copied?
- can a borrowed value be converted to an owned value?
- can it be viewed as `str`?
- can it be viewed as a slice?
- does it implement an interface?
- is its hierarchy closed enough for enum representation?
- can the interface become a Rust trait object?

## Object Representation Policy

A separate policy pass chooses how Java class/interface hierarchies are represented.

```rust
enum ObjectRepresentation {
    ConcreteStruct,
    DynTrait,
    ClosedEnum,
    ErasedObject,
}
```

The policy should prefer more idiomatic/static Rust representations when they are provably valid, and fall back to more general representations when proof is missing.

Preferred order for hierarchy-like values:

1. `ConcreteStruct` when no polymorphism is needed.
2. `ClosedEnum` when a closed set of subclasses/implementations is known.
3. `DynTrait` when interface/subtype dispatch is needed and object safety works.
4. `ErasedObject` or an unsupported diagnostic when the type is too dynamic or unsafe to represent faithfully.

`BoxedDynTrait` is intentionally not an `ObjectRepresentation`. It is a slot
projection of a `DynTrait` representation when an owned polymorphic value must
escape, be returned, be captured, or be stored. The hierarchy strategy remains
`DynTrait`; only the slot dispatch becomes `DispatchShape::BoxedDynTrait`.

### Object Representation Fallback Matrix

| Role | Concrete Struct | Closed Enum | Dyn Trait | Boxed Dyn Trait | Erased Object | Unsupported Hierarchy |
| --- | --- | --- | --- | --- | --- | --- |
| Public API param | Concrete param when the declared type has no polymorphic representation requirement. | Accepted only if public extensibility is not required by facts. | Borrowed trait view for object-safe public interfaces. | Not a representation; public params remain borrowed dyn unless an owned storage requirement is explicit. | Used for raw/wildcard/object-preserving public boundaries or rejected trait objects. | Diagnostic when an extends/implements edge cannot be represented and no erased fallback is selected. |
| Public API return | Concrete owned return for concrete declared types. | Accepted for closed hierarchy returns. | Projected to owned boxed dyn slot when returning an interface value. | Slot projection for owned escaping dyn-trait return. | Used for unknown/raw/wildcard object-preserving returns. | Diagnostic before lowering if required conversion back from erased/dyn cannot be proven. |
| Unknown API param | Concrete param for resolved helper calls. | Accepted when the unknown call target has a closed enum type. | Borrowed trait view for object-safe interface arguments. | Not a representation; only used if the slot itself requires owned polymorphic storage. | Used for unknown runtime object requirements. | Diagnostic when dispatch coercion lacks a runtime target. |
| Unknown API return | Concrete local/return shape for resolved concrete helpers. | Accepted when source-summary facts prove closed variants. | Borrowed dyn as a value shape, then projected by the consuming slot. | Slot projection for storing or returning the result. | Used for unknown call results when runtime type is not modeled. | Diagnostic when cast/instanceof asks for unsupported recovery. |
| Storage/field | Concrete struct field for non-polymorphic types. | Enum field for closed hierarchy storage. | Source representation remains dyn trait. | Owned field/storage projection for dyn trait. | Stored as erased object for raw/wildcard/rejected trait-object types. | Diagnostic if field hierarchy edges cannot be represented. |
| Local | Concrete local unless usage forces polymorphic behavior. | Closed enum local when case-split proof exists. | Borrowed dyn local when only borrowed trait view is needed. | Local slot may project to boxed dyn if it escapes or is stored. | Erased object local for unknown runtime object facts. | Diagnostic if local conversion would require unsupported ownership/dispatch recovery. |
| Captured value | Concrete captured value when capture does not cross a polymorphic boundary. | Closed enum capture for closed hierarchy values. | Captured borrowed dyn only for non-escaping closure use. | Escaping capture projects dyn trait to boxed dyn. | Capture falls back to erased object when runtime identity is unknown. | Diagnostic if capture requires unsupported clone-box or downcast recovery. |
| External pack fact | Concrete when pack says final/concrete with no subtype dispatch. | Closed enum when pack provides a closed hierarchy override. | Dyn trait when pack proves object safety. | Provided by slot projection from dyn trait plus owned storage requirement. | Erased object when pack rejects trait objects or names raw object behavior. | Pack diagnostics should name missing subtype, object-safety, or runtime-target proof. |
| Non-object-safe interface | Not applicable for the interface root. | Possible only with explicit closed enum override and valid variants. | Rejected. | Rejected as a representation; slot projection cannot rescue non-object-safe dyn. | Fallback representation. | Diagnostic records the object-safety violation reason. |
| Closed hierarchy | Concrete for leaves. | Preferred for root when closed-world proof and named variants exist. | Rejected if closed enum is selected. | Only a slot projection if closed enum is not selected and dyn trait wins. | Used when override or requirements reject static case split. | Diagnostic if proof is incomplete or variants cannot be named. |
| Raw object / wildcard object | Not used without concrete runtime identity. | Not used without named variants. | Not used without object-safe trait identity. | Not used as representation. | Default conservative representation. | Diagnostic if later casts require a missing runtime target. |

## Closed Enum Hierarchies

The translator should support representing selected Java subclass/interface sets as named Rust enums.

Example Java:

```java
interface Expr {}
class Add implements Expr { Expr left, right; }
class Num implements Expr { int value; }
```

Possible Rust:

```rust
pub enum ExprValue {
    Add(Add),
    Num(Num),
}
```

This can avoid many Java casts and `instanceof` patterns by lowering them into Rust `match`.

Example:

```java
if (expr instanceof Add) {
    Add add = (Add) expr;
    return eval(add.left) + eval(add.right);
}
```

can become enum matching when `Expr` is represented as `ExprValue`:

```rust
match expr {
    ExprValue::Add(add) => eval(add.left) + eval(add.right),
    _ => ...
}
```

Closed enum representation is valid when one of these is true:

- Java uses sealed classes/interfaces and all permitted implementations are available.
- The corpus proves all implementations and no external implementation is possible.
- The user injects facts declaring the hierarchy closed.
- A package/project policy treats a set as closed for this translation unit.

Closed enum selection is driven by an explicit proof policy, not by emission
special cases. Parsed sealed declarations produce two separate facts: a closed
hierarchy fact and a sealed-permits fact naming the Java alternatives. External
packs and user facts may provide closed-hierarchy evidence without a Java
permits list, but then the known subtype/finality facts must still prove named
terminal variants.

The proof states are:

- `Proven`: a closed hierarchy fact is effective, all required permitted
  subtypes are present when a permits list exists, at least one direct subtype
  is known, and every direct variant is final.
- `OpenWorld`: no effective closed hierarchy fact is available, so dyn trait or
  erased-object fallback remains responsible.
- `NoKnownDirectSubtypes`: the hierarchy is closed but has no named direct
  variants for a Rust enum.
- `MissingPermittedSubtypes`: source sealed permits name alternatives that are
  not present in the known direct subtype set.
- `NonFinalVariants`: direct variants are not final, so matching the direct
  subtype set would miss later subclasses.

User `closed-enum` overrides are validated separately against source hierarchy
facts. They can intentionally select a named enum, but diagnostics must report
unknown variants, variants that are not recorded as subtypes, and omitted
terminal source subtypes.

## Dynamic Trait Fallback

If a closed enum cannot be proven, the translator should fall back to trait-object representations when possible:

```rust
&dyn Animal
Box<dyn Animal>
```

This is the general fallback for open Java interfaces and subclassing where external implementations may exist.

Trait fallback requires:

- a Rust trait can be generated for the Java interface/base API
- the trait is object-safe
- lifetimes and ownership can be represented
- generic methods and associated constraints do not make object safety impossible

If trait objects are not safe or practical, the translator should fall back to `ErasedObject` or emit a structured unsupported diagnostic.

## User-Injected Facts And Overrides

The translator should allow users to inject or override facts. This is important because corpus analysis may not prove closed-world assumptions that are true for a specific project.

Possible configuration shape:

```toml
[hierarchies.Expr]
strategy = "closed_enum"
enum_name = "ExprValue"
variants = ["Add", "Num"]

[hierarchies.Animal]
strategy = "dyn_trait"
trait_name = "Animal"

[types.ExternalId]
copy = true
clone = true

[api.public]
prefer_borrowed_string_params = true
```

Injected facts should be recorded as facts with provenance:

```rust
enum FactProvenance {
    ParsedSource,
    CorpusAnalysis,
    UserOverride,
    ConservativeFallback,
}
```

When user overrides conflict with source-proven facts, diagnostics should report the conflict rather than silently producing unsound Rust.

## Casts And Instanceof

Casts and `instanceof` should be represented as shape/type operations, not as ad hoc lowering rules.

### Upcast

Java:

```java
Animal a = dog;
```

Shape operation:

```text
Concrete subtype -> selected supertype/interface representation
```

Depending on policy:

```rust
AnimalValue::Dog(dog)
&dog as &dyn Animal
Box::new(dog) as Box<dyn Animal>
```

### Downcast

Java:

```java
Dog d = (Dog) animal;
```

Shape operation:

```text
supertype/interface representation -> concrete subtype
```

Depending on representation:

```rust
match animal {
    AnimalValue::Dog(dog) => dog,
    _ => panic!("invalid Java cast"),
}
```

or:

```rust
animal.as_any().downcast_ref::<Dog>()
```

The exact runtime behavior policy should be explicit: panic, `Result`, `Option`, or unsupported diagnostic, depending on the translation mode.

### Instanceof

Java:

```java
animal instanceof Dog
```

With closed enums:

```rust
matches!(animal, AnimalValue::Dog(_))
```

With trait objects:

```rust
animal.as_any().is::<Dog>()
```

## Constraint Solving

The slot-shape solver should collect constraints before lowering.

Examples:

```text
stored into field
=> ownership >= Owned

only read
=> ownership >= SharedBorrow

primitive value
=> ownership = CopyValue

string content API
=> view can be Str

indexed or iterated sequence
=> view can be Slice

assigned to interface/base variable
=> dispatch must represent declared supertype

passed to unknown external API
=> Native + Owned conservative fallback

returned from public method
=> public API policy applies
```

Constraints should be monotonic and solved to a fixpoint because call sites and callees influence each other.

Recursive Java code must not be analyzed by recursively expanding function
bodies during lowering. Recursion is represented as finite graph edges between
known slots, and each propagation step may only add constraints from a bounded
domain to those slots. Direct recursion, mutual recursion, recursive
constructors, and recursive hierarchy/capability facts should therefore
saturate to a fixpoint or produce a finite-budget diagnostic; they must not
create unbounded path-specific facts, fresh temporary slots, or recursively
specialized Rust shapes.

Reusable recursive analyses should use the finite saturation layer: declare the
bounded fact domain, compute the iteration budget from the number of known slots
and fact kinds, then call the shared saturation loop. A feature-specific module
may count its own slots and fact kinds, but it should not own a separate
recursive expansion strategy or local unbounded worklist policy.

## Coercion

Coercion adapts actual expression shape to target slot shape.

Inputs:

```rust
actual: RustSlotShape
target: RustSlotShape
expr: RustExpr
```

Examples:

```text
Owned String -> SharedBorrow Str
=> expr.as_str() or &expr depending context

SharedBorrow Str -> Owned String
=> expr.to_string()

Owned Vec<T> -> SharedBorrow Slice
=> expr.as_slice() or &expr

Concrete Dog -> ClosedEnum AnimalValue
=> AnimalValue::Dog(expr)

Concrete Dog -> DynTrait Animal
=> &expr as &dyn Animal

Concrete Dog -> ErasedObject
=> Box::new(expr) as Box<dyn std::any::Any>

ErasedObject -> Concrete Dog
=> *expr.downcast::<Dog>().expect("invalid Java cast")

ErasedObject instanceof Dog
=> expr.is::<Dog>()

ErasedObject -> DynTrait Animal
=> unsupported diagnostic; a concrete runtime downcast target is required

ErasedObject -> BoxedDynTrait Animal
=> unsupported diagnostic; a concrete runtime downcast target and ownership proof are required

ClosedEnum AnimalValue -> Concrete Dog
=> match/downcast adapter
```

Coercion owns adapters such as `.clone()`, `.cloned()`, `.to_string()`, enum wrapping, trait-object upcasting, and downcasting support.

Stream lowering, method-call lowering, and ordinary expression lowering must not embed those ownership/dispatch decisions directly.

## Erased Object Runtime Contract

`ErasedObject` lowers to `Box<dyn std::any::Any>`. It is an owned runtime value
used when source facts cannot preserve a more precise concrete, enum, or
object-safe trait representation. The contract is:

- storing, returning, and passing erased values preserves the
  `Box<dyn std::any::Any>` carrier;
- concrete values enter the carrier with `Box::new(value) as Box<dyn std::any::Any>`;
- concrete casts use `downcast::<Target>()` and panic on invalid Java casts;
- concrete `instanceof` checks use `is::<Target>()`;
- erased-to-borrowed-dyn and erased-to-boxed-dyn recovery are unsupported until
  a concrete runtime target and ownership proof are available;
- anonymous erased boundaries and external-pack erased handles use this same
  carrier instead of generating anonymous concrete implementations.

Source-summary superclass boundaries use the same representation contract.
When an emitted primary/co-translated type extends a class that is present only
as a source-summary declaration, `representation` emits a corpus-analysis
`HierarchyOverride` for the summary superclass with `ErasedObject`. That keeps
non-emitted framework bases out of inheritance-lowering diagnostics while
preserving ordinary diagnostics for co-translated superclass bodies, which
still require real hierarchy lowering.

## Boxed Dyn Clone-Box Requirements

Borrowed dyn to owned boxed dyn recovery is not an ordinary `.clone()` on the
trait object. The live conversion planner represents this boundary directly:
when the actual slot is `DynTrait` and the target slot is the same
`BoxedDynTrait`, it emits `DispatchCoercionPolicy::CloneBoxDynTrait`, lowers that
to `Coercion::CloneBoxDynTrait`, and renders the receiver through `.clone_box()`.

The old deferred adapter-generation descriptors were removed because production
never entered them; tests now assert the reachable `clone_box()` coercion path
instead of constructing unavailable policy states.

## Owned Recovery Capability Facts

Owned recovery is a fact-layer decision, not a lowering special case. A Java
type that crosses a borrowed-to-owned Rust boundary is classified by
`OwnedRecoveryCapabilityFact` before coercion, signature solving, or external
library lowering chooses an adapter.

The capability vocabulary is deliberately broader than Rust `Copy`/`Clone`:

- `Copy` covers primitive and proven copyable value types.
- `ExplicitClone` covers ordinary Java values with a trustworthy Rust clone.
- `CloneBoxAdapter` covers trait-object values that can be recovered through a
  known clone-box adapter.
- `FactoryCopy` covers APIs whose owned recovery is represented by a copy
  factory rather than direct clone semantics.
- `ImmutableSharedHandle` covers shared external handles whose owned Rust value
  can be copied because the handle is immutable and identity-stable.
- `FacadeOwnedExternalHandle` covers facade values that need an explicit
  ownership-transfer adapter before Rust can own them.
- `FfiOwnedHandle` covers FFI/resource handles that need an explicit FFI
  transfer, retain, or dup operation before Rust can own them.
- `ConservativeClone` is the fallback audit state when no explicit fact exists
  but the Java type is not known to be non-cloneable.
- `NonCloneable` is the explicit unsupported state.

`EffectiveFacts::owned_recovery_capability_for` is the single query point.
Coercion maps clone-box and factory-copy capabilities through the same
`CloneRecoveryPolicy::ProvenCloneCapability` path for now, while preserving the
more specific capability in fact and signature policy APIs. Immutable shared
handles map to copy recovery. Facade-owned and FFI-owned handles remain
unsupported conversion/signature diagnostics until explicit transfer adapters
exist.

External packs must add capability facts instead of matching Java package names
inside lowering. This keeps GATK/HTSJDK, HDF5, SQLite, or future dependency
rules declarative and lets ownership recovery compose with signature inference,
borrowed element adapters, and dyn-trait fallback planning.

Serialized external pack manifests use:

```text
external-pack-owned-recovery <pack> <type> <kind> [java-type=<type>]
```

where `<kind>` is one of `copy`, `explicit-clone`, `clone-box-adapter`,
`factory-copy`, `immutable-shared-handle`,
`facade-owned-external-handle`, `ffi-owned-handle`,
`conservative-clone`, or `non-cloneable`.

## Java Cast Semantics

Java casts are classified before Rust rendering by
`JavaCastSemanticsPolicy`. The classifier consumes Java type predicates from
`types`, representation-shaped slots, and the existing `ShapeCoercionPlan`.

Named outcomes:

- `PrimitiveNumericCast` for Java numeric primitive casts, including narrowing
  and widening;
- `ReferenceUpcast` for concrete-to-enum, concrete-to-dyn, boxed dyn, and
  erased-object upcasts;
- `CheckedReferenceDowncast` for runtime-checked casts out of closed enum, dyn
  trait, boxed dyn trait, or erased-object representations, using
  `InvalidCastPolicy`;
- `UncheckedGenericCast` for same-raw-type generic casts with different type
  arguments;
- `RawOrWildcardCast` for raw generic counterparts and wildcard-containing
  types;
- `OptionalResultCast` for casts crossing `Optional` or planned `Result`
  carriers;
- `ImpossibleCast` for primitive/reference and invalid primitive casts;
- `DiagnosticOnlyFallback` when the representation-shaped coercion already
  produced unsupported diagnostics.

Lowering may render through `ShapeCoercionPlan`, but it must not rediscover
these Java cast categories from syntax.

## Java Instanceof Semantics

`instanceof` is classified before rendering by `InstanceOfTargetPolicy` and
`plan_instanceof_check_with_facts`.

Target classification names:

- `ConcreteRuntimeTarget` when the target has a concrete nominal runtime type;
- `ClosedEnumTarget` when the target is a closed hierarchy representation;
- `DynTraitTarget` and `BoxedDynTraitTarget` when the target is an object-safe
  interface representation or owned slot projection;
- `ErasedObjectTarget` when the target is represented as `Box<dyn Any>`;
- `PrimitiveTarget` for invalid Java primitive targets;
- `RawOrWildcardTarget` for raw generic targets and wildcard-containing
  targets;
- `MissingRuntimeTarget` when no Rust runtime target can be named.

Hierarchy facts are consumed through `HierarchySubtypePolicy`, which uses the
finite reachability worklist in `facts`. A known concrete subtype checked
against an interface or closed hierarchy target may become `AlwaysTrue`; dynamic
checks over closed enum, borrowed dyn, boxed dyn, and erased-object actuals still
lower through the representation-shaped `InstanceOfCheck` variants.

## Cast and Instanceof Planning Boundary

`lower.rs` may pass signature, representation, and fact context into
`plan_cast_conversion` and `plan_instanceof`, but it must not inspect hierarchy
facts, object representations, dispatch strategies, package names, or Java type
capabilities for casts or `instanceof`.

The boundary is enforced by `lower_consumes_cast_and_instanceof_plans_without_rechecking_representation_facts`:

- casts consume representation-shaped source and target slots through
  `CastConversionPlan`;
- `instanceof` consumes representation-shaped slots and semantic facts through
  `InstanceOfPlan`;
- hierarchy reasoning happens in `facts` and `conversion_plan` before lowering;
- rendering uses `conversion_lowering` and `coercion` plan consumers only.

## Capability-Owned Type Classification

Nonprimitive Java type categories used by cast and `instanceof` planning are
owned by effective fact policies:

- `CastTypeCapabilityPolicy` identifies optional/result carriers,
  raw-or-wildcard casts, and unchecked same-raw-type generic casts;
- `InstanceOfTargetCapabilityPolicy` identifies primitive and
  raw-or-wildcard `instanceof` targets;
- `FactSet` exposes facade methods for both policies so conversion planning
  does not import raw structural helpers or branch on package/type spellings.

Conversion planning may still branch directly on Java primitive kinds where the
primitive value is required for Rust numeric casts or primitive-target
diagnostics.

## Fallback Strategy

The translator should attempt idiomatic mappings first, then fall back conservatively.

General order:

1. Preserve concrete values where no polymorphism or ownership conversion is needed.
2. Use borrowed views when body and call graph prove they are valid.
3. Use slices/`str` views for sequence/string reads.
4. Use closed named enums for proven or configured closed hierarchies.
5. Use `dyn Trait` for open polymorphism when object-safe.
6. Use `Box<dyn Trait>` for owned escaping polymorphism.
7. Use owned/native Java-like Rust values for public or unknown boundaries.
8. Emit structured diagnostics for cases that remain ambiguous or unsafe.

## Module Responsibilities

### `types`

Owns Java semantic type inference, Java subtyping facts, generic shapes, erasure, primitive promotion, and Java overload resolution.

Does not choose Rust ownership or object representation.

### `facts`

Stores analysis facts with provenance, including capture facts, hierarchy facts, type capabilities, and user overrides.

### `usage` or `analysis`

Collects body-use facts for parameters, locals, fields, returns, calls, casts, `instanceof`, storage, captures, mutation, and escape.

Does not emit Rust.

### `representation`

Chooses object representation policy for classes, interfaces, and hierarchy sets:

- concrete struct
- closed enum
- dyn trait
- boxed dyn trait
- erased object

Consumes source facts, corpus facts, and user overrides.

### `signature`

Solves `RustSlotShape` for method parameters, returns, fields, locals, and important temporaries.

Uses body-use facts, call graph constraints, object representation decisions, and public API policy.

### `coercion`

Adapts actual expression shape to target slot shape.

Owns ownership adapters, borrowed-to-owned adapters, view adapters, enum wrapping, trait-object conversion, and downcast support.

### `lower`

Consumes solved plans and emits Rust.

Does not choose parameter ownership, return ownership, hierarchy representation, or cast strategy.

## Refactoring Rule

When implementing a new Java feature, do not add a direct lowering special case if the behavior is really one of these:

- ownership adaptation
- borrowed/owned choice
- string/slice view conversion
- interface/subclass dispatch
- enum hierarchy wrapping
- downcast/upcast
- public API boundary policy

Instead, add the fact, capability, representation rule, signature constraint, or coercion adapter in the dedicated layer.

This is the rule intended to prevent a growing pile of local Java-to-Rust corner cases.
