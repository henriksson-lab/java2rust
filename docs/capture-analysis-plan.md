# Capture Analysis Plan

## Goal

Add semantic capture facts for lambdas, local classes, anonymous classes, and inner classes before broadening lowering for streams and object captures.

This is a facts/resolution task first. Lowering should consume capture facts later; it should not rediscover captures from expression text or parser structure.

## Motivation

The current stream lowering supports only item-only expression lambdas in `StreamsUse.java`. The next fixtures need more semantic facts:

- `StreamsCollect.java`: item-only lambdas over `String`, with method calls on the item.
- `LocalClassCapture.java`: local class captures method parameters and local variables.
- `InnerCapture.java`: inner class method captures outer `this` state.
- `AnonymousInterface.java`: anonymous class needs generated type provenance and possible captures.

Without explicit capture facts, lowering will become ad hoc and will likely repeat the old transpiler's corner-case pattern.

## Responsibility Boundaries

- `parser`
  - Continues to produce `java_ir` only.
  - Does not decide whether a variable is captured, effectively final, borrowed, cloned, or wrapped.

- `java_ir`
  - Preserves lambdas, local classes, anonymous class bodies, inner classes, and source spans.
  - May gain stable IDs if fact attachment needs a durable key.

- `resolve`
  - Builds lexical scopes.
  - Determines which identifiers bind to locals, params, fields, outer `this`, or type names.
  - Records capture facts in `facts`.

- `types`
  - Supplies expression and lambda parameter types.
  - Records effectively-final facts where mutation/type knowledge is required.

- `facts`
  - Owns capture fact structs and IDs.
  - Stores facts independent of Rust output strategy.

- `lower`
  - Consumes capture facts to choose closure captures, generated fields, `Rc<RefCell<T>>`, explicit borrows, or diagnostics.

## Data Model

Extend `src/facts.rs` from a notes placeholder into structured facts.

Suggested IDs:

```rust
pub struct NodeId(pub usize);
```

The ID assignment can initially be traversal-order scoped to one `Program`, as long as tests do not depend on unstable printed IDs. If later passes need long-lived source identity, replace this with span-backed IDs.

Suggested facts:

```rust
pub struct CaptureFacts {
    pub owners: Vec<CaptureOwner>,
}

pub struct CaptureOwner {
    pub id: NodeId,
    pub kind: CaptureOwnerKind,
    pub origin: CaptureOrigin,
    pub captures: Vec<Capture>,
}

pub enum CaptureOwnerKind {
    Lambda,
    LocalClass,
    AnonymousClass,
    InnerClass,
}

pub enum CaptureOrigin {
    Method { type_name: String, method_name: String },
    Constructor { type_name: String },
    Type { type_name: String },
}

pub struct Capture {
    pub name: String,
    pub source: CaptureSource,
    pub ty: JavaType,
    pub effectively_final: bool,
    pub mutable_after_capture: bool,
}

pub enum CaptureSource {
    Local,
    Parameter,
    ThisField,
    OuterThis,
}
```

Keep Rust choices out of these facts. A later ownership decision may map the same fact to a copy, borrow, explicit field, `Rc`, `RefCell`, or diagnostic.

## Stage 1: Scope And Binding Collection

Build a resolver-side lexical scope stack for methods and constructors:

- Parameters.
- Local variable declarations.
- Lambda parameters.
- Local class member scopes.
- Inner class member scopes.
- Anonymous class member scopes.

For every `Expr::Var`, decide whether it refers to:

- Current local/parameter.
- Enclosing method local/parameter.
- Current class field.
- Enclosing class field/outer `this`.
- Unknown or type/static owner.

Do not lower based on these bindings yet. Tests should inspect facts.

## Stage 2: Lambda Capture Facts

Implement capture facts for `Expr::Lambda`.

Initial supported cases:

- Item-only lambda captures nothing:
  - `StreamsCollect.java`: `v -> !v.isEmpty()`, `v -> v.trim()`.
  - `StreamsUse.java`: `v -> v > 0`, `v -> v`.

- Lambda captures enclosing local or parameter:
  - Add a small fixture if none exists yet, for example:

```java
public int addBase(List<Integer> values, int base) {
    return values.stream().mapToInt(v -> v + base).sum();
}
```

Facts should record `base` as a parameter capture.

## Stage 3: Local Class Capture Facts

Fixture:

- `examples/java/04-objects/LocalClassCapture.java`

Expected facts:

- Owner kind: `LocalClass`.
- Generated/source name: `Adder`.
- Captures:
  - `base` from method parameter, effectively final.
  - `offset` from local variable, effectively final.
- Does not capture `value`, because `value` is passed to `apply`.

Resolution rule:

- Local class members resolve first against their own members/params/locals.
- Unresolved local references then search enclosing method scopes.
- Then search enclosing class/outer class fields.

## Stage 4: Inner Class And Outer `this` Facts

Fixture:

- `examples/java/04-objects/InnerCapture.java`

Expected facts:

- Owner kind: `InnerClass`.
- Inner type `Adder` records an `OuterThis` capture of `InnerCapture`.
- `apply` resolves bare `base` to the outer class field.
- `offset` resolves to the inner class field.

Do not choose `&Outer` versus `Rc<RefCell<Outer>>` in this stage. Record only that an outer receiver is required.

## Stage 5: Anonymous Class Facts

Fixture:

- `examples/java/04-objects/AnonymousInterface.java`

Expected facts:

- Owner kind: `AnonymousClass`.
- Origin: method `positive`.
- Implemented/constructed type: `IntPredicate` if available from `Expr::New`.
- Captures: empty for the current fixture.

This establishes provenance and generated-type inputs even before anonymous class lowering is implemented.

## Stage 6: Effective Final And Mutation Facts

Track simple mutation after capture:

- Assignment to local.
- Compound assignment.
- Prefix/postfix increment/decrement.

Facts:

- `effectively_final = false` when a captured local/param is reassigned after initialization.
- `mutable_after_capture = true` when mutation occurs after the capture owner appears.

First implementation may be conservative: if mutation order is hard to prove, mark `mutable_after_capture = true`.

## Tests

Add resolver/facts tests, not lowering tests, for this phase.

Suggested test file:

- `tests/capture_facts.rs`

Assertions:

- `StreamsCollect.java` lambdas have no captures.
- `LocalClassCapture.java` local class captures `base` and `offset`.
- `InnerCapture.java` inner class records an `OuterThis` capture.
- `AnonymousInterface.java` anonymous class has an owner fact with no captures.
- Added lambda-captures-local fixture records the captured parameter/local.

The existing reader guard must keep passing:

- No `Expr::Raw`, `Stmt::Unsupported`, or `Member::Unsupported` for current fixtures.

## Lowering Gate

Do not broaden lowering until these are true:

- Capture facts exist for the target fixture.
- Tests prove the fixture's captures are understood.
- The lowering rule states how each capture source maps to Rust.

For example:

- `StreamsCollect.java` may lower after tests prove its lambdas capture only their item parameter.
- `LocalClassCapture.java` may lower only after local-class generated fields and constructor injection rules are documented.
- `InnerCapture.java` may lower only after the outer receiver representation is chosen.

## Completion Criteria

This phase is complete when:

- `src/facts.rs` contains structured capture fact types.
- `resolve` produces capture facts for lambdas, local classes, anonymous classes, and inner classes in the target fixtures.
- `tests/capture_facts.rs` verifies the expected facts.
- No parser/lower shortcut duplicates capture analysis.
- `cargo fmt` and `cargo test` pass.
