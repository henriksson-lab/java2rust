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
    BoxedDynTrait,
    ClosedEnum,
    ErasedObject,
}
```

The policy should prefer more idiomatic/static Rust representations when they are provably valid, and fall back to more general representations when proof is missing.

Preferred order for hierarchy-like values:

1. `ConcreteStruct` when no polymorphism is needed.
2. `ClosedEnum` when a closed set of subclasses/implementations is known.
3. `DynTrait` when interface/subtype dispatch is needed and object safety works.
4. `BoxedDynTrait` when owned polymorphic values escape or must be stored heterogeneously.
5. `ErasedObject` or an unsupported diagnostic when the type is too dynamic or unsafe to represent faithfully.

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

ClosedEnum AnimalValue -> Concrete Dog
=> match/downcast adapter
```

Coercion owns adapters such as `.clone()`, `.cloned()`, `.to_string()`, enum wrapping, trait-object upcasting, and downcasting support.

Stream lowering, method-call lowering, and ordinary expression lowering must not embed those ownership/dispatch decisions directly.

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
