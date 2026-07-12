# Lowering Next Plan

## Goal

Move from a complete fixture reader to semantic lowering for the structured Java IR constructs that are currently represented but not translated.

Current state:

- The tree-sitter reader preserves every current Java fixture as `java_ir`.
- The reader guard rejects `Expr::Raw`, `Stmt::Unsupported`, and `Member::Unsupported` in fixtures.
- `lower` still intentionally emits `compile_error!` for several structured constructs.

This plan covers the next tasks:

1. Define lowering design rules for the new IR constructs.
2. Implement the smallest semantic lowering slice.
3. Add fixture tests for generated Rust shape and compilation where possible.
4. Tackle lambdas and streams separately after the core slice.

## Responsibility Rules

Keep these boundaries explicit:

- `parser`: CST to `java_ir` only.
- `java_ir`: Java source semantics, not Rust strategy.
- `resolve`: names, overloads, constructor targets, inherited/member facts.
- `types`: Java expression/type facts and Java assignment compatibility.
- `coercion`: Rust slot adaptation once Java type facts are known.
- `lower`: Java IR plus resolved/type facts to Rust IR.
- `emit`: Rust syntax only.
- `stdlib`: Java library surface mappings and compatibility shims.

Do not put Rust output decisions in the parser. Do not put Java parsing decisions in lowering.

## Stage 1: Lowering Design Rules

Write or update focused docs before coding each construct:

- `docs/control-flow.md`
  - `switch` fallthrough semantics.
  - Whether to lower each switch as `match`, `if` ladder, or labeled loop plus state.
  - Rules for Java labeled `break`/`continue`.

- `docs/object-model.md`
  - Constructor delegation: `this(...)` and `super(...)`.
  - Constructor initialization order.
  - What is allowed before delegation.
  - How to represent superclass construction once inheritance lowering starts.

- `docs/errors-and-exceptions.md`
  - `throw`.
  - checked exception propagation.
  - `try`/`catch`/`finally`.
  - When Rust output should use `Result`, `panic!`, or a compatibility enum.

- `docs/standard-library.md`
  - Stream pipeline scope.
  - Functional interface scope.
  - Iterator lowering versus compatibility shims.

- `docs/ownership-and-mutability.md`
  - How switch/try lowering interacts with mutable locals and early exits.
  - Captures for lambdas, local classes, and anonymous classes.

Deliverable:

- A short rule section for each construct before implementation.
- Each rule names the owning module and the data it requires.

## Stage 2: Smallest Semantic Lowering Slice

Implement in this order.

### 2.1 Constructor Delegation

Fixtures:

- `examples/java/03-methods/ConstructorOverloads.java`
- `examples/java/04-objects/InheritanceAndSuper.java` only for `super(...)` preservation; full inheritance can remain deferred.

Design target:

- `resolve` determines which same-class constructor a `this(...)` delegation targets.
- `lower` emits one Rust constructor body that reuses the target constructor result rather than duplicating initialization.
- `super(...)` remains a structured unsupported lowering until superclass layout is designed.

Expected Rust shape for `this(...)`:

```rust
pub fn new() -> Self {
    Self::new_string_int("empty".to_string(), 0)
}
```

Completion:

- No `compile_error!` for same-class `this(...)` constructor delegation in the fixture.
- Existing overload suffix rules still hold.

### 2.2 Switch Fallthrough

Fixture:

- `examples/java/02-control-flow/SwitchFallthrough.java`

Design target:

- Preserve Java fallthrough exactly.
- Prefer a simple, correct lowering before clever `match` generation.
- Candidate strategy:
  - Evaluate selector once.
  - Track whether execution has entered a case.
  - Execute subsequent case bodies until `break`.

Completion:

- Generated Rust for the switch fixture compiles.
- Behavior-preserving tests cover fallthrough from `case "B"` into `case "C"`.

### 2.3 Throw and Checked Exception Policy

Fixtures:

- `examples/java/06-stdlib/CheckedException.java`
- `examples/java/06-stdlib/CheckedPropagation.java`
- `examples/java/06-stdlib/MultiCatch.java`
- `examples/java/06-stdlib/TryCatchFinally.java`

Design target:

- Decide the first supported exception strategy before coding.
- Recommended first slice:
  - Methods with `throws` lower to `Result<T, JavaException>`.
  - `throw new E(...)` lowers to `return Err(JavaException::from(...))`.
  - `try/catch/finally` can remain structured unsupported until the `Result` surface compiles.

Completion:

- The chosen policy is documented.
- A narrow fixture compiles if it only throws/propagates without catch/finally.

## Stage 3: Fixture Tests and Gates

Add tests as the lowering slice grows:

- Shape tests:
  - Assert constructor delegation emits a call to the delegated constructor.
  - Assert switch lowering does not emit `compile_error!`.
  - Assert exception policy changes function return type where expected.

- Compile tests:
  - Add fixtures to the compile-positive set only when generated Rust is expected to compile.
  - Keep out-of-scope constructs in guardrail tests with structured diagnostics.

- Behavior tests:
  - Where practical, compile generated Rust and run small assertions for semantic cases.
  - Start with switch fallthrough because it has clear input/output behavior.

Do not weaken the reader guard:

- Current Java fixtures should continue parsing without raw/unsupported reader fallback nodes.

## Stage 4: Lambdas and Streams

Treat lambdas/streams as a separate phase after constructor/switch/exception basics.

Fixtures:

- `examples/java/06-stdlib/StreamsUse.java`
- `examples/java/06-stdlib/StreamsCollect.java`
- `examples/java/06-stdlib/StreamsFindMatch.java`
- `examples/java/06-stdlib/CollectorsToMap.java`
- `examples/java/04-objects/AnonymousInterface.java`
- `examples/java/04-objects/LocalClassCapture.java`

Design target:

- `types` determines functional interface shapes and closure parameter types.
- `resolve` maps stream method chains and collector calls to Java stdlib facts.
- `stdlib` owns Java API to Rust iterator/shim mappings.
- `lower` emits Rust closures/iterators only after those facts exist.

Recommended order:

1. Plain lambda expression as Rust closure when parameter types are known.
2. Simple stream `.filter(...).mapToInt(...).sum()`.
3. `.anyMatch(...)` and `.findFirst()`.
4. `.collect(...)` and `Collectors.toMap(...)`.
5. Anonymous/local class capture after closure capture rules are stable.

Completion:

- At least one stream fixture compiles to idiomatic iterator Rust.
- Capture and ownership decisions are documented before local/anonymous classes are lowered.

## Global Completion Criteria

This next lowering phase is complete when:

- The design docs above contain concrete rules for the implemented constructs.
- Constructor delegation, switch fallthrough, and the first exception policy have fixture-backed tests.
- Compile-positive fixtures compile without introducing parser-layer shortcuts.
- Lambdas/streams have a separate documented implementation path and at least one compiled fixture when that phase starts.
- `cargo fmt` and `cargo test` pass.
