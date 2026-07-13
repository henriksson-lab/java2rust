# Semantic Fact Layer Implementation Goal

## Goal

Make semantic facts the explicit contract between Java IR analysis and Rust
representation/lowering.

The immediate deliverable is not full Java support. It is a working analysis
pipeline where facts are produced, saturated to a finite fixed point, tested
directly, and consumed by representation/signature/lowering instead of being
rediscovered in late-stage code.

Current pipeline:

```text
parser -> resolve -> facts from resolve
       -> usage
       -> representation
       -> signature
       -> lower
       -> emit
```

Target pipeline for this goal:

```text
parser -> resolve
       -> semantic_analysis::analyze_program
          - starts from resolve facts plus external facts
          - runs usage and semantic fact producers
          - saturates finite fact domains
          - returns SemanticFacts
       -> representation from facts
       -> signature from facts and usage
       -> lower from planned facts/representation/signatures
       -> emit
```

## Non-Goals

- Do not make every idiom-gap fixture translate idiomatically in this goal.
- Do not lower `try`/`catch`, assignment-in-condition, or tail recursion yet.
- Do not replace `UsageFacts`, `RepresentationPlan`, or `SignaturePlan`.
- Do not add one-off lowering branches for nullability, exception hierarchy, or
  external API mutability.

## Existing Anchors

Use the existing modules rather than creating parallel systems:

- `src/facts.rs`: source, override, capability, capture, hierarchy, and object
  representation facts.
- `src/fact_saturation.rs`: finite saturation policy and budgeted fixed-point
  runner.
- `src/usage.rs`: body-use facts, call-site facts, binding uses, and field
  uses.
- `src/representation.rs`: object representation planning from `FactSet`.
- `src/signature.rs`: parameter, receiver, local, return, and field slot shape
  planning.
- `src/external_facts.rs`: user-provided facts and override parsing.
- `src/lib.rs`: top-level pipeline integration point.

## New Or Renamed Concepts

### `SemanticFacts`

Add a small orchestration-facing struct, likely in `src/semantic_analysis.rs`:

```rust
pub struct SemanticFacts {
    pub facts: FactSet,
    pub usage: UsageFacts,
    pub diagnostics: Vec<SemanticDiagnostic>,
    pub iterations: SemanticAnalysisIterations,
}
```

This is not a replacement for `FactSet`. It is the analysis product that keeps
`FactSet` together with body-usage facts and convergence diagnostics.

### `SemanticAnalysisConfig`

Add an explicit config for bounded analysis:

```rust
pub struct SemanticAnalysisConfig {
    pub max_external_guess_iterations: Option<usize>,
    pub enable_external_api_guesses: bool,
    pub enable_control_flow_facts: bool,
}
```

Default behavior should be conservative and deterministic.

### Fact Domains To Add

Add these to `src/facts.rs` in small, queryable structs:

- Fact identity:
  - `FactKind` for bounded-domain accounting
  - stable keys for types, fields, methods, constructors, symbols, call sites,
    and control-flow sites
- Field nullability:
  - field key: type name plus field name
  - state: definitely non-null, maybe-null, definitely-null, unknown
  - provenance: constructor assignment, field initializer, method assignment,
    external override, conservative fallback
- Method effect:
  - receiver mutability: read-only, mutates receiver, unknown
  - return nullability: definitely non-null, maybe-null, unknown
  - thrown exception names
  - recursion kind: none, direct recursion, mutual recursion
- Trivial accessor facts:
  - getter method key plus backing field key
  - setter method key plus backing field key and assigned parameter
  - fluent setter marker when the setter returns `this`
  - visibility/override safety marker for whether the method can be inlined,
    preserved as API, or both
- External API effect:
  - method name and optional receiver type
  - guessed role: getter, boolean getter, setter, mutator, fluent mutator,
    indexed accessor, unknown
  - receiver mutation guess
  - result ownership/nullability guess
- Control-flow markers:
  - assignment in condition
  - tail-recursive self call
  - mutual recursion edge
- Exception hierarchy:
  - checked vs unchecked classification where known
  - direct and reachable exception-supertype edges

Keep each domain finite. No path-specific facts, no recursive specialization,
and no facts whose keys include unbounded expression text.

## Phase 1: Pipeline Shell

Files:

- Add `src/semantic_analysis.rs`.
- Export it from `src/lib.rs`.
- Change `translate_source_with_facts` to call `semantic_analysis::analyze_program`.
- Keep downstream calls equivalent:
  - `representation::plan_representations(&program, &semantic.facts)`
  - `signature::solve_signatures_with_facts(..., &semantic.usage, &semantic.facts)`
  - `lower::lower_program_with_plans(..., &semantic.facts, &semantic.usage, ...)`

Implementation details:

- `analyze_program` starts with `resolved.facts.clone()`.
- It extends with external facts.
- It computes `UsageFacts` once.
- It initially returns the same effective facts as the current pipeline.

Acceptance tests:

- Existing `cargo test` passes.
- A new test proves `translate_source_with_facts` and the semantic-analysis
  shell preserve current generated Rust for one small fixture.
- No representation/signature/lowering behavior changes in this phase.

## Phase 2: Fact-Level Test Harness

Files:

- Add `tests/semantic_facts.rs`.
- Add helper functions that parse, resolve, and run semantic analysis without
  emitting Rust.

Test style:

- Fact tests must inspect `SemanticFacts`, `FactSet`, or `UsageFacts`.
- They must not assert generated Rust unless the test is explicitly an emission
  regression.

Initial tests:

- `records_non_null_field_from_all_constructors`
  - Fixture: `examples/idiom-gaps/NonNullFieldFromConstructors.java`.
  - Assert field `NonNullFieldFromConstructors.name` is definitely non-null.
- `records_trivial_getter_and_setter_facts`
  - Fixture: `examples/idiom-gaps/JavaBeanGetterSetterNames.java` or a smaller
    accessor fixture if needed.
  - Assert `getValue` is a trivial getter for `value` and `setValue` is a
    trivial setter for `value`.
- `records_external_setter_as_receiver_mutation_guess`
  - Fixture: `ExternalGetterSetterMutability.java`.
  - Assert `setValue` is guessed to mutate receiver.
- `records_external_getter_as_read_only_guess`
  - Same fixture.
  - Assert `getValue` is guessed read-only.
- `records_mutual_recursion_edges_without_unbounded_growth`
  - Fixture: `MutualRecursion.java`.
  - Assert `even -> odd`, `odd -> even`, and convergence within budget.
- `records_exception_subclass_edges`
  - Fixture: `ExceptionSubclassCatchOrder.java`.
  - Assert `SpecificProblem -> BaseProblem -> Exception`.
- `records_assignment_in_condition_marker`
  - Fixture: `WhileAssignmentNotNull.java`.
  - Assert the condition contains an assignment marker owned by analysis.

## Phase 3: Field Nullability Facts

Ownership:

- Fact production belongs in `semantic_analysis` or a small
  `field_nullability` helper called from it.
- Representation/signature can consume the facts.
- Lowering must not infer field nullability from constructor bodies.

Rules:

- A reference field is definitely non-null when:
  - every constructor assigns it before normal exit, and
  - every observed assignment writes a definitely non-null expression.
- A constructor may prove a field non-null through a private helper method once
  fixed-point analysis proves that helper always writes the field non-null.
- Field initializer counts as constructor coverage.
- Assignment from `null` makes the field maybe-null.
- Missing constructor assignment makes the field maybe-null unless a non-null
  initializer exists.
- Assignment from an unknown external call is unknown/maybe-null unless facts
  prove otherwise.
- A public/protected method assigning the field from a nullable or unknown
  parameter keeps it maybe-null.

Acceptance:

- `NonNullFieldFromConstructors.name` becomes definitely non-null at fact level.
- Existing nullable cache fixture remains maybe-null.
- A constructor calling a private initializer helper can prove a field non-null
  after saturation.
- Calls to proven trivial setters are treated as writes to the backing field for
  nullability analysis.
- Calls to proven trivial getters are treated as reads of the backing field for
  ownership and field-use analysis.
- `signature` field shape policy has a single consumption point for field
  nullability facts.

## Phase 4: Trivial Accessor Facts

Ownership:

- Detecting trivial getters/setters belongs in semantic analysis.
- The Java IR should keep the original methods; deletion or inlining is a later
  representation/lowering decision.
- Nullability, usage, signature, and lowering consume accessor facts instead of
  rediscovering JavaBean patterns.

Rules:

- A trivial getter has:
  - no parameters
  - a body equivalent to `return field;` or `return this.field;`
  - no other statements or side effects
  - a return type compatible with the field type
- A trivial setter has:
  - one parameter
  - a body equivalent to `this.field = param;`
  - no validation, coercion, logging, synchronization, exception, lazy
    initialization, or normalization
  - `void` return, or `return this;` for a fluent setter
- A public/protected accessor may need to be preserved as Rust API even when its
  body is analyzed as direct field access.
- Private/internal trivial accessors can be candidates for later inlining.
- Interface methods, overridden methods, and framework-sensitive JavaBean APIs
  must keep enough metadata to avoid unsound deletion.

Consumers:

- Field nullability treats `setX(non_null)` as an assignment to field `x`.
- Receiver planning treats trivial setters as receiver mutation.
- Ownership analysis treats `getX()` as a field read.
- Call lowering may rewrite `obj.getX()` to `obj.x` only when visibility,
  representation, and ownership rules allow it.
- API emission may still keep wrapper methods for public Java surfaces.

Acceptance:

- Trivial getter and setter facts are present for the JavaBean fixture.
- Non-trivial accessors are not marked trivial.
- No text-level or parser-level deletion of getter/setter methods occurs.
- Lowering does not add independent JavaBean getter/setter pattern matching.

## Phase 5: External API Effect Guesses

Ownership:

- Guessing belongs in `semantic_analysis` or a dedicated helper such as
  `external_api_effects`.
- The guess result is stored as facts.
- Signature/lowering can consume the facts but must not reapply name heuristics.

Rules:

- `getX`, `isX`, `hasX`, `size`, `length` default to read-only.
- `setX`, `add`, `remove`, `put`, `clear`, `advance`, `read`, `write`, `copy`
  default to receiver-mutating when the receiver is an object.
- Fluent chains like `request.setName(...).setLimit(...)` are marked as
  receiver mutation or builder-style mutation, not plain immutable calls.
- `get(row, column)` is read-only indexed access.
- `set(row, column, value)` is indexed mutation.
- User-provided external facts override guesses and produce conflict
  diagnostics if they contradict source-proven facts.

Acceptance:

- External getter/setter fixtures have fact-level method effect guesses.
- Receiver mutability planning can query the fact instead of matching method
  names.
- No new external API name heuristics are added to lowering.

## Phase 6: Hierarchy And Exception Facts

Ownership:

- Parser and resolver provide declared `extends`, `implements`, `throws`, and
  catch types.
- Semantic analysis records hierarchy edges and exception-specific views.
- Exception lowering consumes exception facts later; it does not build the
  hierarchy itself.

Rules:

- Every `extends` and `implements` relation becomes a direct hierarchy edge.
- Reachable hierarchy edges are saturated with `FiniteSaturationPolicy`.
- Classes extending `Exception` are checked exceptions unless they extend
  `RuntimeException` or a known unchecked subtype.
- Catch clauses preserve source order.
- Catch assignability is answered through reachable hierarchy facts.
- Unknown external exception types remain generic but queryable.

Acceptance:

- `SpecificProblem -> BaseProblem -> Exception` is reachable.
- `UncheckedProblem -> RuntimeException` is classified as unchecked.
- `CheckedProblem -> Exception` is classified as checked.
- Multi-catch facts preserve each listed catch type.

## Phase 7: Recursion And Control-Flow Facts

Ownership:

- Detection belongs in semantic analysis.
- Rewriting belongs in `rewrite.rs` or a future normalization module.
- Lowering consumes normalized control flow or explicit unsupported diagnostics.

Rules:

- Direct recursion and mutual recursion are graph facts over method keys.
- Tail-recursive self calls are marked when the recursive call is in tail
  position.
- Assignment-in-condition is marked before lowering.
- The analysis records finite graph edges only.

Acceptance:

- `MutualRecursion` records a two-node cycle and converges.
- `TailRecursiveAccumulator.factorialLoop` records tail-recursive self call.
- `WhileAssignmentNotNull` records assignment-in-condition.
- No lowering change is required in this phase.

## Phase 8: Consume Facts In Representation And Signature

Start with one narrow consumer per fact family:

- Field nullability affects field storage shape.
- External mutability affects receiver planning.
- Hierarchy facts continue feeding `representation::plan_representations`.
- Exception facts are available to exception planning but may still lower to
  compatibility `JavaException`.

Acceptance:

- Existing shape/signature tests still pass.
- New responsibility guard asserts field nullability is not inferred in
  lowering.
- New responsibility guard asserts external getter/setter name heuristics are
  not in lowering.

## Done Criteria For This Goal

The goal is complete when all are true:

- `semantic_analysis::analyze_program` is the only top-level path that combines
  resolved facts, external facts, usage facts, and saturated semantic facts.
- There are fact-level tests for:
  - non-null field from constructors
  - nullable field fallback
  - helper-method field assignment propagation
  - trivial getter/setter facts
  - external getter read-only guess
  - external setter mutation guess
  - mutual recursion convergence
  - assignment-in-condition marker
  - exception subclass reachability
- Fixed-point/saturation diagnostics are surfaced through the translation
  diagnostics or a clearly documented semantic diagnostic path.
- `representation`, `signature`, and `lower` receive facts from the semantic
  analysis product rather than rebuilding equivalent facts locally.
- No new feature-specific special cases are added to lowering for this work.
- `cargo fmt --check` and `cargo test --quiet` pass.

## First Implementation Slice

Start here:

1. Add `src/semantic_analysis.rs` with `SemanticFacts` and
   `analyze_program`.
2. Wire `src/lib.rs` through it without changing output.
3. Add `tests/semantic_facts.rs` with the harness and one shell test.
4. Add field-nullability fact structs and queries to `src/facts.rs`.
5. Implement only the non-null field and nullable fallback analysis.
6. Add the field-nullability fact tests, including helper-method propagation if
   the initial saturated edge is small enough to implement cleanly.
7. Add trivial getter/setter fact detection for the JavaBean fixture and feed
   those facts into field-use/nullability analysis where simple.
8. Consume the field-nullability fact in the single signature field-shape entry
   point.

This creates the permanent path. Later slices can add external API effects,
exception hierarchy, and control-flow facts without changing the pipeline again.
