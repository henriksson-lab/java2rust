# Object Model

## Classes

Simple Java classes lower to Rust structs plus impl blocks. Field visibility follows Java visibility when possible, but private fields should stay private unless tests or generated sibling modules need an accessor.

Constructors lower to associated functions such as `new`. Field initializers, instance initializer blocks, constructor delegation, and `super(...)` must be represented explicitly in Java IR before Rust emission.

## Inheritance

Rust has no direct class inheritance. Use one of these strategies per hierarchy:

- Composition for concrete superclass state: child struct contains `base: Base`.
- Trait for virtual method surface: superclass/interface method contracts become traits.
- Route enum for closed hierarchies where Java code uses `instanceof` or downcast-like behavior.

The chosen strategy is a hierarchy-level decision, not a per-use patch.

## Interfaces

Interfaces lower to traits. Default methods become provided trait methods when object safe; otherwise they become helper functions or require an explicit diagnostic.

Interface constants become associated constants when type-compatible, or module-level constants when Java access patterns require a concrete path.

## Dynamic Dispatch

Use static dispatch for monomorphic concrete calls. Use `Box<dyn Trait>`, `Rc<dyn Trait>`, or references to `dyn Trait` only when Java dynamic dispatch is required at a heterogeneous site.

## `super`

`super.method(...)` calls the composed base implementation directly when composition is used. If the method is virtual through a trait, the lowering must avoid accidental recursive dispatch.

## Inner And Anonymous Classes

Captured outer `this` and captured locals must become explicit fields. Local classes cannot be emitted inside a Rust function if their type is referenced from module-level signatures; hoist them with generated names and source provenance.

## Generated Class Names

Generated names are deterministic:

- Anonymous class inside method `ownerMethod` becomes `OwnerMethodAnonN`, numbered by source order within the declaring top-level type.
- Local class `Adder` inside method `addLater` becomes `AddLaterAdder`.
- Inner class `Outer.Inner` becomes `OuterInner` as the Rust type name, with the module path preserving package ownership.

Every generated type records the Java origin and capture list in provenance.

## Capture Representation

Capture choice follows ownership facts:

- Captured immutable primitive or immutable value: copy or clone at construction.
- Captured effectively-final non-primitive local: owned field when no later mutation is observed, otherwise `Rc<RefCell<T>>`.
- Captured outer `this`: borrowed `&Outer` only when the generated value cannot escape the outer borrow scope; otherwise `Rc<RefCell<Outer>>` or an explicit rewrite-needed diagnostic.
- Captured mutable local: `Rc<RefCell<T>>` unless the local class never escapes and a lexical `&mut` borrow is provably sufficient.

Anonymous interface implementations lower to generated structs implementing the target trait. Closure-backed trait objects are allowed only when no Java identity, fields, or multiple methods are required.
