# Object Model Strategy Algorithm Plan

## Purpose

This document audits the object model plan from an algorithm perspective. It
answers which translation decisions are solved separately, which must be
co-solved, and whether the project needs a general constraint solver.

The short answer is: use small finite solvers with explicit boundaries, not a
single general-purpose constraint solver.

## Design Conclusion

The translator should be built as a pipeline of monotone fact production,
finite strategy selection, slot-shape solving, and local conversion planning.
These stages are connected by typed facts and plans. They must not rediscover
each other's responsibilities.

Global object-model strategy and Rust slot shapes are related, but they should
not be mutually recursive in the first implementation. Object strategy produces
a hierarchy-level representation. Signature and slot-shape solving project that
representation into concrete storage, parameter, return, field, and local
shapes. Coercion then adapts expressions into those solved slots.

This gives us enough power for subclassing, interfaces, closed enums, dyn trait
fallbacks, erased-object fallbacks, user overrides, and external facts without
turning the compiler into an opaque search engine.

## Solver Boundaries

| Problem | Algorithm | Co-solved with | Not solved with |
| --- | --- | --- | --- |
| Java type semantics | Resolver plus Java type inference | overload selection, casts, generics, primitive coercion facts | Rust ownership or Rust object representation |
| Semantic facts | finite saturation to fixed point | hierarchy reachability, method effects, field nullability, recursion edges | final Rust slot ownership |
| Effective facts | precedence resolver | user overrides, source facts, corpus facts, conservative fallbacks | expression lowering |
| Hierarchy object strategy | finite per-root strategy solver | dispatch requirements, closed-world proof, object safety, override validity | per-call coercion details |
| Slot shape | finite axis solver over slots | usage constraints, representation projection, mutability, nullability | hierarchy proof |
| Expression conversion | recursive conversion planner | ownership/view/dispatch/primitive/nullability adapters for one boundary | global representation selection |
| Lowering | plan consumption | solved signatures, representation plans, conversion plans | new semantic inference |

## Pipeline

The intended one-way pipeline is:

```text
parser
  -> resolve and Java type inference
  -> semantic_analysis fixed-point facts
  -> effective fact resolver
  -> object_model strategy solver
  -> representation plan
  -> signature and slot-shape solver
  -> expression conversion planner
  -> lowering and emission
```

The pipeline may report diagnostics that suggest overrides, but late coercion
failure should not silently feed back into hierarchy strategy selection. If a
strategy is wrong for the user's target style, the correction should be an
explicit fact or policy override. A later version can add controlled policy
iteration, but the default architecture should remain one-way and inspectable.

## Effective Fact Resolution

Use a deterministic precedence algorithm:

```text
user override
  > source-proven fact
  > corpus or project fact
  > external API guess
  > conservative fallback
```

Rules:

- Facts at the same precedence that disagree produce a conflict diagnostic.
- Scalar facts choose the highest-precedence value.
- Set facts union non-conflicting values and diagnose invalid combinations.
- Every effective fact records provenance, so a bad representation choice can
  be explained without tracing through lowering.

This is not a constraint solver. It is a query layer over finite facts.

## Semantic Fact Saturation

Semantic analysis should use monotone fixed-point saturation over finite
domains.

Examples:

- hierarchy reachability
- exception subtype reachability
- method call graph and recursion strongly connected components
- field nullability states
- trivial accessor field reads and writes
- external API effect guesses

Each domain needs a bounded key space. Facts must not be keyed by arbitrary
expression text or by recursively specialized type states.

For recursive Java code, the solver computes a finite strongly connected
component summary, not an unbounded inline expansion. Direct recursion, mutual
recursion, tail-position calls, and non-tail recursive calls are facts on method
keys and call edges.

## Object Strategy Solver

Object strategy is a finite per-hierarchy-root choice.

Inputs:

- direct and reachable `extends`/`implements` facts
- known implementations and subclass set
- object-safety facts for interface or abstract method surfaces
- explicit representation overrides
- dispatch requirements extracted from usage
- API boundary requirements
- closed-world and corpus coverage facts

Output:

```rust
enum ObjectStrategy {
    ConcreteStruct,
    ClosedEnum { enum_name: String, variants: Vec<String> },
    DynTrait { trait_name: String },
    ErasedObject,
}
```

`Box<dyn Trait>` is normally a slot projection of `DynTrait`, not a separate
hierarchy strategy. A field or return slot may need owned polymorphic storage,
but that should become `OwnershipShape::Owned` plus
`DispatchShape::BoxedDynTrait`, while the hierarchy strategy remains
`DynTrait`. This avoids having one hierarchy choice per ownership case.

Selection algorithm:

1. Apply valid explicit override first.
2. If the root is not polymorphic, choose `ConcreteStruct`.
3. If a closed enum is requested or clearly useful, choose `ClosedEnum` when
   the implementation set is closed and every variant can be named.
4. If dynamic dispatch is required and the trait surface is object safe, choose
   `DynTrait`.
5. If runtime type tests, casts, raw objects, or object-safety failures cannot
   be represented safely, choose `ErasedObject` or emit an unsupported
   diagnostic depending on policy.

The solver should return both the chosen strategy and rejected alternatives
with reasons. That keeps fallback behavior debuggable.

## Dispatch Requirement Extraction

Dispatch requirements are facts. They are not emitted as ad hoc lowering
branches.

Useful requirements include:

- `NeedsSubtypeDispatch`
- `NeedsClosedRuntimeCaseSplit`
- `NeedsInstanceof`
- `NeedsDowncast`
- `NeedsOwnedPolymorphicStorage`
- `NeedsBorrowedTraitView`
- `NeedsPublicExtensibleApi`
- `RejectTraitObject`
- `NeedsUnknownRuntimeObject`

These facts are finite and keyed by hierarchy root or slot. The object strategy
solver consumes root-level requirements. The slot-shape solver consumes
slot-level requirements such as owned polymorphic storage.

## Slot Shape Solver

Slot shape solving stays axis-based:

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

The solver combines:

- Java type identity from resolve
- semantic facts such as nullability and mutation
- usage constraints such as escapes, reads, writes, and argument passing
- object strategy projection for the Java type
- boundary policies for public API, fields, locals, returns, and temporaries

This solver may need fixed-point iteration across calls and methods, because a
method signature affects callers and callers affect preferred signatures.
However, the iteration is still over finite shape axes. It does not choose
hierarchy strategy and it does not invent new semantic facts.

## Conversion Planner

Expression conversion is local and recursive. Given an actual shape and a
target slot shape, it plans a sequence of adapters across independent axes:

- primitive widening or narrowing diagnostic
- nullability wrapping/unwrapping
- ownership borrow/clone/move/copy recovery
- view conversion such as `String` to `&str` or `Vec<T>` to `&[T]`
- dispatch conversion such as concrete to enum, concrete to dyn trait, boxed
  dyn borrow, or supported downcast

The conversion planner may recurse into container element conversions, optional
element conversions, tuple-like helper values, stream stages, and enum variants.
That recursion is structural over the solved type shape, so it terminates. It
does not ask the object strategy solver to reconsider a hierarchy.

This is where specific examples such as `findFirst()` over `Vec<String>` become
general. The stream rule says the source produces a borrowed element shape. The
target slot says `Option<String>` or `Option<&str>`. The conversion planner
chooses `.cloned()`, `.copied()`, borrowing, or a diagnostic through ownership
and view adapters, not through a stream-specific special case.

## Why Not A General Constraint Solver

A monolithic solver would make these hard to control:

- fact provenance and override diagnostics
- preserving Java semantics before choosing Rust idioms
- explaining why closed enum, dyn trait, or erased object was chosen
- keeping lowering free of inference
- adding corpus-specific facts without changing core algorithms
- avoiding search explosion across ownership, dispatch, nullability, and casts

The problems are finite and mostly monotone. A set of typed lattice/fix-point
passes is easier to test and easier to debug than a general solver.

The only place that looks like constraint solving is signature and slot-shape
selection. Even there, use explicit finite axes and priority rules, not an
open-ended SAT/SMT-style engine.

## Co-Solved Problems

These decisions should be co-solved:

- Closed enum versus dyn trait versus erased object must consider closed-world
  proof, object safety, `instanceof`, casts, downcasts, and override facts
  together.
- Slot ownership, view, nullability, mutability, and dispatch projection should
  be solved together for each slot because the axes interact.
- Conversion planning should solve ownership, view, dispatch, primitive, and
  nullability adapters together for a single expression boundary.
- Field nullability and trivial accessor normalization should be saturated
  together, because setters and helper methods can prove field writes.
- Recursion classification and call graph SCCs should be solved together,
  because mutual recursion is not visible from one method at a time.

## Separately Solved Problems

These decisions should stay separate:

- Java overload resolution and Java type inference happen before Rust slot
  shapes.
- Hierarchy object strategy happens before slot projection.
- Slot-shape solving consumes object strategy; it does not prove hierarchy
  closure.
- Coercion consumes solved shapes; it does not choose signatures.
- Lowering consumes plans; it does not perform name heuristics or semantic
  inference.

## Data Structure Sketch

The implementation should introduce a dedicated strategy layer:

```rust
pub struct ObjectModelInput<'a> {
    pub program: &'a Program,
    pub facts: &'a FactSet,
    pub effective_facts: &'a EffectiveFacts,
    pub usage: &'a UsageFacts,
}

pub struct ObjectModelPlan {
    pub strategies: BTreeMap<JavaTypeKey, ObjectStrategyPlan>,
    pub diagnostics: Vec<ObjectModelDiagnostic>,
}

pub struct ObjectStrategyPlan {
    pub root: JavaTypeKey,
    pub strategy: ObjectStrategy,
    pub requirements: Vec<DispatchRequirement>,
    pub rejected: Vec<RejectedObjectStrategy>,
    pub provenance: Vec<FactProvenance>,
}
```

`representation::plan_representations` should become a consumer of this object
model plan. Existing representation structs can remain, but the strategy
decision belongs in `object_model`, not in lowering or coercion.

## Implementation Slices

1. Add `src/object_model.rs` with pure policy structs and a per-root strategy
   solver. Initially preserve current behavior.
2. Add effective fact query helpers for representation-relevant facts.
3. Move hierarchy override validation, closed-enum inference, object-safety
   fallback, and erased-object fallback behind the object-model API.
4. Add dispatch requirement facts and tests for `instanceof`, casts, interface
   slots, owned polymorphic storage, public API boundaries, and unknown runtime
   objects.
5. Make `representation::plan_representations` consume `ObjectModelPlan`.
6. Make `signature` consume representation projection and slot-level dispatch
   requirements without redoing hierarchy strategy.
7. Refactor conversion into a recursive axis planner with tests proving stream,
   optional, sequence, enum, dyn trait, and primitive cases share the same
   ownership/view/dispatch adapters.

## Acceptance Criteria

- One module owns object strategy selection.
- One module owns effective fact precedence.
- One module owns recursive conversion planning.
- Lowering has no JavaBean, stream ownership, closed enum, dyn trait, or
  external API name heuristics.
- Tests can assert rejected object strategies and their reasons.
- A closed enum override, dyn trait fallback, erased-object fallback, and
  boxed-dyn slot projection are covered by tests without adding per-example
  lowering rules.
- Recursive and mutually recursive Java code produce finite facts and never
  trigger unbounded specialization.

## Deferred Work

- Controlled policy iteration if late signature solving discovers a dispatch
  requirement that was impossible to know earlier.
- Dyn clone-box adapter generation for borrowed dyn to owned boxed dyn recovery.
- More complete object-safety validation for Java interfaces with generics,
  default methods, associated constants, and self-referential APIs.
- User-facing diagnostics that recommend exact external fact overrides.
