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

## Transformation Rule Documentation Contract

Every transformation rule introduced or revised under this plan must document
the desired Rust before it documents the implementation mechanism. Use the
canonical rule template from `docs/transformation-rule-contract.md`: **Ideal
Rust Target**, **Implementation Rule**, **Required Facts**, **Owning Layer**,
**Fallback Shape**, and **Evidence**. The active GATK plan mirrors this
contract in `docs/gatk-running-algorithm-strengthening-plan.md`.

The **Ideal Rust Target** section is mandatory and must appear before any
implementation rule, status note, module assignment, or lowering detail. It
should show the Rust shape we want to emit when enough facts are known,
including ownership, borrowing, nullability, dispatch, collection, error, and
external-boundary shape when those dimensions matter. The implementation rule
then explains how semantic facts and plan nodes reach that target without
introducing fixture-specific or corpus-specific branches.

## Future Goal: Unify Facts And Reusable Ideas

After the current semantic fact layer is stable, add an explicit unification
goal so new work reuses existing concepts instead of creating parallel names,
parallel heuristics, or late lowering equivalents.

Ideal Rust target:

- analysis produces one queryable fact graph for semantic properties such as
  nullability, ownership recovery, mutability, object representation, hierarchy,
  external API effects, error shape, collection element shape, and crate/facade
  boundary shape;
- downstream planners consume that graph through named capability queries and
  plan nodes, not through ad hoc Java-name checks or duplicated local booleans;
- emitted Rust decisions for equivalent source situations are selected by the
  same fact/query path even when they originate from source analysis, external
  packs, user overrides, source summaries, or conservative guesses.

Implementation rule:

- maintain a fact/idea registry in this document or a successor
  `docs/fact-concept-registry.md`;
- every new fact-like concept must declare whether it is a new canonical fact,
  an alias/projection of an existing fact, or a consumer-specific plan derived
  from existing facts;
- every new lowering or planning rule must name the existing fact query it
  consumes before adding a new query;
- repeated corpus fixes must first be checked against the registry and either
  reuse the existing concept or explicitly extend it with migration notes;
- tests should include at least one cross-origin case when possible, for
  example source-derived fact plus external-pack fact selecting the same slot
  shape.

Acceptance criteria:

- no module-local list of special type, method, or package names may be added
  when the same behavior can be represented as a fact, capability, external
  pack entry, source summary, or override;
- duplicate concepts are removed or linked through one canonical query before
  broadening corpus coverage;
- audit tests or review checklist items catch newly introduced duplicate
  concepts, especially in `stdlib`, resolver shortcuts, coercion, signature,
  slot rendering, and late emission;
- benchmark-driven fixes update the registry entry for the generalized idea
  rather than only recording the corpus symptom.

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
  - bounded-domain accounting for fact categories, introduced with real
    producers/consumers rather than a standalone unused enum
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

### Accessor Elimination And Ownership

Removing trivial accessors from internal analysis helps Rust implementation, but
only if it is represented as facts and plans rather than text-level method
deletion.

The key reason is that trivial getters and setters otherwise hide the true slot
being read or written. If `getName()` remains an opaque call, ownership and
lifetime analysis must reason about a method return. If it is known to be a read
of field `name`, the same field-slot rules used for direct `this.name` access
can decide whether the Rust surface should be `String`, `&String`, `&str`,
`Option<T>`, `Rc<T>`, `Box<dyn Trait>`, or another shape. Likewise, a trivial
setter should be seen as a write to the backing field before field nullability,
receiver mutability, and struct layout are solved.

Responsibilities:

- `semantic_analysis` detects trivial accessor bodies and records
  `TrivialAccessorFact`.
- `usage` expands calls to proven trivial accessors into the same field-read and
  field-write uses produced by direct field access.
- `signature` and field-shape planning consume those field uses. They do not
  need separate JavaBean-specific rules.
- `call_plan` decides whether a call site may be emitted as direct field access,
  a wrapper method call, or left opaque.
- `lower` consumes the call plan. It must not match names like `getX` or `setX`
  directly.
- Public/protected Java accessors remain available as Rust wrapper methods
  unless representation planning proves they are not part of the translated API.

This means accessor elimination should be an internal normalization layer, not
an API-destroying source rewrite. A public Java method can still be emitted as:

```rust
pub fn get_name(&self) -> &str {
    self.name.as_str()
}
```

while internal translated call sites use `self.name` directly when the solved
slot shape supports that. The wrapper delegates to the solved field shape; the
field shape is not chosen to satisfy the wrapper.

### Accessor Normalization Policy

Trivial getter/setter removal is useful for Rust ownership and lifetime
planning, but only after it has been split into two separate decisions:

- Semantic normalization: proven trivial getters and setters are treated as
  reads and writes of the backing field for usage, nullability, receiver
  mutability, and slot-shape solving.
- API preservation: the original Java methods remain available to emission and
  can still become Rust wrapper methods when visibility, trait/interface
  contracts, framework conventions, or user policy require them.

The semantic layer must therefore not physically delete methods from Java IR.
It records `TrivialAccessorFact` plus an accessor inline/preserve policy. Later
planning stages decide whether each call site uses direct field access, a
wrapper method, or an opaque call. This keeps ownership decisions attached to
the field slot instead of to every accessor-shaped call.

This matters for struct layout. If all internal reads of `getFoo()` are known
to be reads of field `foo`, then `signature` can solve one shape for `foo`
using the same constraints as direct field access: borrowed view, owned value,
optional storage, shared pointer, dynamic trait object, enum projection, or
other representation choices. The solved field shape then drives any generated
wrapper signature. For example, a public getter may return `&str` because
`foo` is stored as `String`; the field is not forced into a getter-specific
shape.

Setter normalization has the same rule in the opposite direction. A trivial
`setFoo(value)` contributes a write constraint to field `foo`, so constructor
definite-assignment, non-null field proof, receiver mutability, and ownership
transfer are all solved at the field slot. Public setters can still be emitted
as wrappers, but their Rust receiver and parameter types are derived from the
field-write plan rather than from JavaBean naming.

External or unresolved accessor-shaped calls follow the same contract. Name
heuristics may create external API effect facts, and user overrides may replace
those guesses, but downstream stages consume only facts and plans. They do not
repeat getter/setter string matching.

Accessor-normalization acceptance:

- Trivial accessor facts are consumed by field-use analysis before signature
  solving.
- Internal calls to proven trivial accessors can be planned as field access
  without deleting the original methods.
- Public API wrappers are preserved unless a representation/API policy removes
  them explicitly.
- No module outside semantic analysis or call planning performs JavaBean name
  recognition.
- Struct field shapes are solved from field-slot constraints, not from the
  surface signatures of trivial accessor methods.
- Generated accessor wrappers adapt to the solved field shape instead of
  forcing the field shape.

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
- External reference and callback parameters are assumed non-null unless a
  nullable fact or observed `null` call site proves otherwise. This is a
  fact-layer boundary policy: injected or source-discovered facts enter first,
  the external-parameter policy normalizes remaining unknown slots to required
  values, and lowering consumes the normalized slot shape without adding a
  second heuristic.
- Keep this rule centralized. Adding a new external library, crate facade,
  callback adapter, ownership adapter, or coercion rule must not introduce a
  second nullability decision point. New evidence is represented as facts keyed
  by callable owner, kind, arity, and parameter index; the existing slot policy
  is the single place that decides `T`/`&T`/`F` versus `Option<T>`/`Option<F>`.
- Observed `null` at a resolved external owner/kind/arity/parameter slot makes
  that slot nullable. Non-null arguments into that optional slot are adapted at
  the conversion boundary with `Some(...)`; null arguments become `None`.
- External/user/source-summary facts may explicitly mark parameters nullable
  or non-null. If an explicit non-null fact conflicts with observed `null`, the
  semantic fact layer emits a conflict diagnostic and uses the safe optional
  shape unless a future override policy deliberately changes that priority.
- Callback parameter facts carry both nullability and the functional signature.
  For example, a nullable constructor callback taking
  `(Map<String, URI>, int, int)` and returning a reader map is one slot fact;
  method-reference lowering consumes that target signature instead of guessing
  arity from the Java syntax.

Acceptance:

- External getter/setter fixtures have fact-level method effect guesses.
- Receiver mutability planning can query the fact instead of matching method
  names.
- No new external API name heuristics are added to lowering.
- External parameter nullability tests prove the default non-null case, the
  observed-null optional case, `Some(...)` wrapping for non-null alternatives,
  and conflict diagnostics for contradictory explicit facts.

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
