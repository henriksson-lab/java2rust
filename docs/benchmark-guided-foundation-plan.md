# Benchmark-Guided Foundation Plan

## Purpose

Use real corpus outcomes to drive the next design-critical work, without
turning one corpus into the specification.

GATK is the current pressure corpus because it exposes hard Java semantics:
generics, overloads, external APIs, inheritance, collections, exceptions,
builder patterns, nested types, and stream-heavy code. The implementation goal
is not to make GATK pass by name. The goal is to extract general translator
rules and place each rule in the owning layer.

## Operating Loop

Every slice follows this loop:

1. Re-baseline the pressure corpus.
2. Select one failure class by architectural leverage.
3. Reduce the outcome to a general Java semantic rule, external API fact,
   semantic fact, object strategy rule, slot-shape rule, conversion rule, or
   structured fallback.
4. Add tests based on the outcome, written to avoid overfitting to the corpus.
5. Implement the smallest responsible rule in the owning layer.
6. Run focused tests, broader tests when shared behavior changed, and the
   corpus benchmark.
7. Record the result in `docs/gatk-resolver-benchmark-report.md`.

Frequency is a tie-breaker. Responsibility and design leverage come first.

## Current Priority

The next implementation work should prioritize foundations that later stages
would otherwise have to rediscover:

1. Resolver completeness.
2. External API facts.
3. Semantic facts.
4. Object strategy inputs.
5. Slot-shape and conversion co-solving.

Broad Rust-output polishing should wait until the Java meaning and Rust-facing
shape are represented as explicit facts or plans.

## Plan Of Record

The next phase should be run as a sequence of small benchmark-backed design
slices. Each slice starts from a real corpus outcome, reduces it to a general
translator responsibility, adds tests for that responsibility, and only then
implements the owning rule.

Use GATK as the main benchmark for this phase because it exposes the highest
pressure on resolver, external API, generic, inheritance, exception, and
collection behavior. Do not treat GATK as the specification. The specification
is the Java semantic rule, external API fact, semantic fact, object strategy,
slot-shape rule, conversion rule, or conservative fallback extracted from the
corpus result.

Priority for the next slices:

1. Improve benchmark classification so remaining failures are easier to assign
   to parser, resolver, external facts, semantic facts, object strategy,
   slot-shape planning, conversion planning, or lowering.
2. Fix source and inherited member resolution before adding more curated
   library constants, unless inspection proves the failure is truly an external
   API fact.
3. Strengthen generic substitution, target typing, diamond inference, wildcard
   erasure, and primitive/boxing/varargs coercion evidence in the resolver.
4. Promote repeated external-boundary guesses into auditable facts with
   provenance and user-override precedence.
5. Add semantic facts needed by Rust shape decisions: field non-null proofs,
   accessor summaries, mutation summaries, assignment-in-condition facts,
   exception facts, and finite recursion facts.
6. Feed object strategy, slot-shape, and conversion planning from those facts
   before expecting broad idiomatic Rust output.

Each selected outcome must add tests based on the outcome. These tests must try
not to overfit to the corpus being improved:

- reduce the corpus failure to a small fixture with neutral names when the
  exact source name is incidental;
- assert the owning layer before final Rust output;
- add negative or conservative tests when proof can be missing;
- add provenance tests when an external API fact is introduced or changed;
- record the corpus motivation in `docs/gatk-resolver-benchmark-report.md`,
  not in the test oracle.

## Layer Responsibilities

### Parser

Owns syntax shape only:

- imports and static imports
- nested and inner type syntax
- anonymous classes
- lambdas and method references
- class literals
- casts and `instanceof`
- try/catch/finally and try-with-resources
- assignment expressions in conditions

Parser tests should assert IR shape, not semantic meaning.

### Resolver

Owns Java symbol and type meaning:

- method and constructor targets
- overload applicability and rejection reasons
- field binding
- inherited source and external members
- static members and enum constants
- generic substitution
- wildcard bounds and erasure
- target typing and diamond inference
- primitive widening, boxing, unboxing, and varargs
- assignability and casts

Resolver output should be resolved symbols, inferred Java types, coercion
evidence, or structured diagnostics.

### External API Facts

Own behavior for libraries outside the translated corpus:

- return type and element type
- return nullability
- receiver and parameter mutation
- getter, setter, predicate, iterator, builder, and fluent roles
- exception summaries
- external object and lifetime boundaries
- provenance: user override, known model, source declaration, or guess

User overrides take precedence over known models and guesses. Guesses must stay
visible and auditable.

### Semantic Facts

Own finite facts that affect Rust representation and signatures:

- field initialization and non-null proofs
- receiver and argument mutation effects
- trivial getter and setter summaries
- assignment-in-condition markers
- exception hierarchy, throw, catch, rethrow, and wrapping facts
- direct and mutual recursion edges
- convergence diagnostics

Recursive Java code should produce a finite fact set. It should not trigger
unbounded specialization.

### Object Strategy

Own Java object representation choices:

- concrete structs
- named enums for proven closed subclass or interface families
- `dyn Trait` fallback
- erased external object boundaries
- conservative fallback when casts, reflection, unknown implementors, or
  external construction prevent a closed model

Injected facts may force or override hierarchy closure, enum eligibility, or
fallback strategy, but provenance must remain visible.

### Slot Shape And Conversion Planning

Own Rust-facing API and storage shapes after Java meaning is known:

- owned values
- shared and mutable borrows
- `String`, `str`, and string-like views
- `Option<T>` only when nullability requires it or proof is missing
- primitive coercions and explicit casts
- collection and iterator element adapters
- trait object, enum, and erased-object adapters
- conservative clones and conversions at explicit boundaries

Lowering should apply slot and conversion plans. It should not rediscover Java
ownership, nullability, coercion, or library behavior.

## Outcome-Driven Test Policy

Every selected corpus outcome must produce tests before or alongside the
implementation.

The tests should be derived from the observed outcome, but they should try not
to overfit to the corpus being improved:

- Add at least one minimized regression test for each selected outcome.
- Name tests after the generalized semantic rule, not the GATK class, method,
  or package that exposed it.
- Prefer synthetic Java with neutral names such as `Base`, `Derived`,
  `Holder`, `Box<T>`, `Builder`, `Reader`, `Source`, `Sink`, `Column`,
  `CollectionLike`, `ExternalRecord`, `Item`, or `Evidence`.
- Assert the responsible layer first: parser shape, resolved symbol, inferred
  Java type, external fact, semantic fact, object strategy, slot shape,
  conversion plan, or structured diagnostic.
- Add Rust-output assertions only after the owning facts or plans have direct
  tests.
- Add negative or conservative tests when the rule depends on proof that may be
  unavailable.
- Keep tests even when the benchmark count does not move, if they cover a real
  Java rule, an intentional fallback, or user-override precedence.
- Record the motivating corpus file or diagnostic in
  `docs/gatk-resolver-benchmark-report.md`, not in the test assertion.

Copy a real corpus snippet only when the exact syntax shape is the feature:
imports, static imports, nested types, anonymous classes, overload shape,
exception syntax, class literals, method references, casts, or a named external
API boundary.

## Concrete Work Packages

This is the plan for turning the current high-level direction into executable
work. Each package should be run as one or more benchmark-backed slices, not as
a broad rewrite.

### 1. Improve Benchmark Classification

Goal: make remaining GATK outcomes easier to assign to the right owner before
coding.

Work:

- report top unresolved methods, constructors, and fields with receiver type,
  source/external provenance, and import/static-import context when available
- distinguish source lookup failures from external-boundary guesses
- distinguish missing syntax support from resolver ambiguity
- record conservative fallbacks as first-class outcomes, not silent success

Required tests:

- add tests for benchmark classification helpers using small synthetic Java
  inputs
- avoid GATK names unless the exact import or external API boundary is the
  feature being classified

### 2. Resolve Source And Inherited Members

Goal: remove pressure that would otherwise force lowering or slot-shape logic
to rediscover Java member lookup.

Work:

- inherited source fields and methods
- inherited external fields and methods where an external superclass is modeled
- static imports, static members, enum constants, and nested type qualifiers
- source constructor and overload applicability diagnostics

Required tests:

- add resolver tests with neutral names such as `Base`, `Derived`, `Holder`,
  `ExternalBase`, and `Item`
- assert resolved symbol, inferred Java type, or structured rejection reason
- add negative tests for unknown external superclasses and ambiguous inherited
  members

### 3. Strengthen Generic And Coercion Evidence

Goal: keep Java type inference, primitive coercion, boxing, unboxing, varargs,
diamond inference, and wildcard erasure in resolver-owned facts.

Work:

- generic substitution through superclass and interface chains
- target typing and diamond inference
- primitive widening and narrowing diagnostics
- boxing, unboxing, and varargs applicability
- container element inference for common Java collection shapes

Required tests:

- add resolver tests for the generalized inference or coercion rule exposed by
  the corpus outcome
- assert coercion evidence and inferred Java types before any Rust-output
  assertion
- include conservative tests for raw types, erased generic arguments, and
  unknown external bounds

### 4. Promote Repeated External Boundaries Into Facts

Goal: model un-translated library behavior as auditable facts with provenance
instead of adding name checks downstream.

Work:

- return type and element type facts
- return and parameter nullability
- getter, setter, predicate, iterator, builder, factory, and fluent roles
- receiver and argument mutation
- exception summaries
- external object and lifetime boundary facts

Required tests:

- add external-fact tests for known model, user override, source declaration,
  and conservative guess provenance
- assert override precedence when a user fact conflicts with a known model or
  guess
- use real external API names only when the API identity is itself the fact
  being modeled

### 5. Add Semantic Facts Needed By Rust Shape Decisions

Goal: make later object strategy, slot shape, conversion planning, and lowering
consume finite facts instead of recomputing Java meaning.

Work:

- field initialization and non-null proofs
- trivial getter and setter summaries
- receiver and argument mutation summaries
- assignment-in-condition markers
- exception hierarchy, throw, catch, rethrow, and wrapping facts
- direct and mutual recursion edges with convergence diagnostics

Required tests:

- add semantic-fact tests that assert the fact set directly
- add conservative tests for missing proof, nullable assignment, open-world
  hierarchy, unknown thrown exception, and recursive dependency cycles
- keep tests even if the current GATK benchmark count does not move, when they
  protect a general semantic rule

### 6. Feed Object Strategy, Slot Shape, And Conversion Planning

Goal: choose Rust-facing shapes from constraints after Java meaning is known.

Work:

- concrete struct versus closed enum versus `dyn Trait` versus erased external
  object
- user overrides for hierarchy closure and representation strategy
- owned, shared-borrowed, mutable-borrowed, optional, and cloned slot shapes
- string, collection, iterator, numeric, dispatch, and erased-object adapters

Required tests:

- add plan-level tests before Rust-output snapshots
- assert representation and slot-shape decisions with neutral source examples
- add conservative fallback tests for reflection, casts, external construction,
  unknown implementors, and unresolved nullability

## Next Concrete Cycle

The immediate cycle should start from the current GATK resolver benchmark
state. The remaining field-access pressure has shifted away from broad parser
failures and mostly into source-shape, generic/container, and external-boundary
classification work.

Current motivating outcomes include:

- source or source-like public fields behind generic receivers, such as
  config/data-holder fields;
- tuple-like fields, including `_1` and `_2`, when the receiver shape is not a
  plain local tuple type;
- method-local or nested source result fields reached through factory-like
  expressions;
- field accesses on results of collection or iterator operations, such as
  `get(...).field`, `next().field`, and `peek().field`;
- enum constants and static members reached through aliases, static imports,
  inferred collection element types, or nested qualifiers;
- library singleton and generated-type patterns that may deserve external API
  facts rather than resolver heuristics.

Treat this list as evidence, not a whitelist. Each selected item must be
reduced to a general rule before implementation.

### 1. Re-Baseline

Run the GATK resolver benchmark and record:

- files parsed and resolved
- resolver diagnostics
- unresolved method calls
- unresolved constructors
- unresolved field accesses
- external API guesses
- semantic diagnostics
- fact counts
- top recurring parser or resolver gaps

Use `docs/gatk-resolver-benchmark-report.md` as the report of record.

### 2. Classify Remaining Failures

Classify the leading failures by owner before coding:

- parser
- resolver
- external API facts
- semantic facts
- object strategy
- slot-shape and conversion planning
- lowering

If a failure could be solved in lowering but its root cause is missing Java
meaning, it belongs upstream.

### 3. Pick The Slice

Choose the next slice by this ordering:

1. It prevents later stages from rediscovering Java semantics.
2. It improves object strategy, slot shape, conversion planning, nullability,
   mutability, dispatch, or exception decisions.
3. It affects multiple corpus failures or a high-frequency failure family.
4. It has a clean owning layer and can be tested there.

### 4. Add General Tests

Before or alongside implementation:

1. Reduce the corpus outcome to a small fixture.
2. Rename classes and methods to neutral names unless the exact library API is
   the fact being modeled.
3. Assert the owning layer directly.
4. Add conservative or negative tests where proof is intentionally missing.
5. Record the corpus motivation in the benchmark report.

Tests are mandatory even when the implementation target is selected from a real
corpus report. The test suite should grow from corpus outcomes, but it should
not become a mirror of the corpus.

Use this checklist for every selected outcome:

- Would the test still make sense if the motivating corpus were replaced by a
  different Java project?
- Does the test name describe a Java rule or translator responsibility instead
  of a GATK type or method?
- Is the assertion at the owning layer, such as parser shape, resolver type,
  external fact provenance, semantic fact, object strategy, slot shape, or
  conversion plan?
- Is there a conservative or negative test when the rule depends on proof that
  can be absent?
- Is the corpus-specific evidence recorded in
  `docs/gatk-resolver-benchmark-report.md` instead of baked into the test
  oracle?

Examples of acceptable outcome-derived tests:

- `Iterator<Item>.next().value` resolves `value` from the element type.
- `Queue<Item>.peek().value` resolves through the queue element type.
- `Map<Key, Item>.get(key).value` resolves through the value type while
  preserving nullability uncertainty for later facts.
- a method-local result class returned from a static factory exposes its fields
  to overload resolution.
- a raw or erased collection receiver produces a structured conservative
  diagnostic instead of a fabricated element field type.

### 5. Implement In The Owning Layer

Keep the rule local to the layer that owns it.

Do not add Java name heuristics to lowering. If a rule needs library knowledge,
model it as an external API fact. If a rule needs Java type knowledge, model it
in resolver output. If a rule needs Rust shape knowledge, model it in slot
shape or conversion planning.

### 6. Verify And Report

For each slice:

1. Run focused tests for the changed layer.
2. Run broader tests when resolver, fact, signature, conversion, or lowering
   behavior changed.
3. Re-run the GATK benchmark.
4. Update `docs/gatk-resolver-benchmark-report.md` with:
   - motivating corpus outcome
   - generalized rule
   - responsible layer
   - tests added
   - before and after counts
   - remaining risks or conservative fallbacks

The benchmark may show no movement. If the generalized rule is correct and
tested, keep the test and record the no-movement outcome.

## Acceptance Criteria

- Each implemented corpus-driven slice has a generalized regression test.
- Tests assert the owning layer before final Rust output.
- Benchmark reports link corpus evidence to generalized rules.
- No new lowering branch compensates for missing resolver, external API,
  semantic, object-strategy, slot-shape, or conversion facts.
- User overrides remain explicit and higher-precedence than guessed facts.
- Recursive and dependency-heavy inputs converge to finite fact sets.
- GATK progress is measurable, but the resulting rules remain useful for other
  Java corpuses.
