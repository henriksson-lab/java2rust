# Benchmark-Backed Design Slices Plan

## Purpose

Use real corpus pressure to improve the Java-to-Rust transpiler's foundations
without turning one corpus into the specification.

GATK is the current primary benchmark because it exposes hard Java semantics:
inheritance, overloads, generics, nested types, external APIs, exceptions,
collections, and builder-like code. It should drive prioritization and reveal
failure classes. It should not define corpus-specific production rules.

The next phase should focus on work that later stages depend on:

1. Java resolution.
2. External API facts.
3. Semantic facts.
4. Object strategy inputs.
5. Slot-shape and conversion planning.
6. Lowering only after the earlier facts and plans are explicit.

## Operating Loop

Each implementation slice follows this loop:

1. Re-run or inspect the current GATK resolver benchmark.
2. Select one outcome by architectural leverage, using frequency only as a
   tie-breaker.
3. Classify the outcome by owning layer before coding.
4. Reduce the outcome to a general Java semantic rule, external API fact,
   semantic fact, object strategy rule, slot-shape rule, conversion rule, or
   structured fallback.
5. Add tests based on the outcome before or alongside implementation.
6. Implement the smallest responsible rule in the owning layer.
7. Run focused tests, broader tests when shared behavior changed, and the GATK
   benchmark.
8. Update `docs/gatk-resolver-benchmark-report.md` with the motivating corpus
   outcome, generalized rule, responsible layer, tests added, before/after
   counts, and remaining risks.

The benchmark is allowed to show no count movement. If the generalized rule is
correct, tested, and owned by the right layer, keep the test and record the
no-movement result.

## Transformation Rule Authoring

Every transformation rule produced by this loop must follow
`docs/transformation-rule-contract.md`. Before writing the implementation rule,
write an **Ideal Rust Target** section that shows the Rust API, expression,
type, ownership, borrowing, optionality, mutability, dispatch, error, and
external-boundary shape we would want by hand when the needed facts are known.

Only after the ideal target is explicit should the rule describe how resolver
facts, semantic facts, object strategy, slot-shape plans, conversion plans, or
lowering nodes select that Rust target. A rule that starts with "lower this
Java pattern as that Rust snippet" is not ready for implementation.

## Layer Ownership

### Parser

Owns Java syntax shape:

- imports and static imports
- nested and inner type syntax
- anonymous classes
- lambdas and method references
- class literals
- casts and `instanceof`
- try/catch/finally and try-with-resources
- assignment expressions in conditions

Parser tests should assert IR shape, not resolved Java meaning.

### Resolver

Owns Java symbol and type meaning:

- method and constructor target selection
- overload applicability and rejection reasons
- inherited source and external members
- field binding
- static members, enum constants, and nested type qualifiers
- generic substitution through classes, methods, superclasses, and interfaces
- wildcard bounds, raw types, erasure, target typing, and diamond inference
- primitive widening, boxing, unboxing, varargs, and assignability
- casts and `instanceof`

Resolver output should be resolved symbols, inferred Java types, coercion
evidence, or structured diagnostics.

### External API Facts

Own behavior for libraries outside the translated corpus:

- return type and element type
- return nullability
- receiver and parameter mutation
- getter, setter, predicate, iterator, factory, builder, and fluent roles
- exception summaries
- external object, aliasing, and lifetime boundaries
- provenance: user override, known model, source declaration, corpus-derived
  model, or conservative guess

User overrides take precedence over built-in models and guesses. Guesses must
remain visible and auditable.

### Semantic Facts

Own finite facts that affect Rust representation and signatures:

- field initialization and non-null proofs
- receiver and argument mutation effects
- trivial getter and setter summaries
- assignment-in-condition markers
- exception hierarchy, throw, catch, rethrow, and wrapping facts
- direct and mutual recursion edges
- convergence diagnostics

Recursive Java code should produce a finite fact set. It must not trigger
recursive specialization or unbounded expression-keyed inference.

### Object Strategy

Own Java object representation choices:

- concrete structs
- named enums for proven closed subclass or interface families
- `dyn Trait` fallback when closure cannot be proven
- erased external object boundaries for un-translated libraries
- conservative fallback when casts, reflection, unknown implementors, or
  external construction prevent a closed model

Injected facts may force or override hierarchy closure, enum eligibility, or
fallback strategy, but provenance must remain visible.

### Slot Shape And Conversion Planning

Own Rust-facing API and storage shapes after Java meaning is known:

- owned values
- shared borrows
- mutable borrows
- string and string-view shapes
- `Option<T>` only when nullability requires it or proof is missing
- primitive coercions and explicit casts
- collection and iterator element adapters
- trait object, enum, and erased-object adapters
- conservative clones and conversions at explicit boundaries

Lowering should apply slot and conversion plans. It should not rediscover Java
ownership, nullability, coercion, or API roles from names.

## Outcome-Driven Test Policy

Every selected corpus outcome must produce tests. These tests should be based
on the benchmark or fixture outcome, but they should try not to overfit to the
corpus we are trying to get to work.

Required rules:

1. Add at least one minimized regression test for each selected outcome.
2. Name tests after the generalized semantic rule, not the GATK class, method,
   or package that exposed it.
3. Prefer synthetic Java with neutral names such as `Base`, `Derived`,
   `Holder`, `Box<T>`, `Builder`, `Reader`, `Source`, `Sink`, `Column`,
   `CollectionLike`, `ExternalRecord`, `Item`, or `Evidence`.
4. Assert the responsible layer first: parser shape, resolved symbol,
   inferred Java type, coercion evidence, external fact, semantic fact, object
   strategy, slot shape, conversion plan, or structured diagnostic.
5. Add Rust-output assertions only after the owning facts or plans have direct
   tests.
6. Add negative or conservative tests when the rule depends on proof that may
   be unavailable.
7. Keep tests even when the benchmark count does not move, if they cover a
   real Java rule, an intentional fallback, or user-override precedence.
8. Record the motivating corpus file or diagnostic in
   `docs/gatk-resolver-benchmark-report.md`, not in the test assertion.

Copy a real corpus snippet only when the exact syntax shape is the feature:
imports, static imports, nested types, anonymous classes, overload shape,
exception syntax, class literals, method references, casts, or a named external
API boundary.

## Priority Order

### 1. Re-Baseline And Classify

Run the GATK resolver benchmark from the current committed state.

Record:

- files seen, parsed, and resolved
- resolver diagnostics
- unresolved method calls, constructors, and field accesses
- explained and unexplained external API calls
- external API guesses and provenance
- semantic diagnostics
- fact counts
- top remaining diagnostics or target rows

Classify the leading outcomes by owner before coding. If ownership is unclear,
improve diagnostics or add a conservative fact first.

### 2. Fix Resolver Gaps That Block Later Design

Prefer resolver slices when missing Java meaning would otherwise leak into
representation, slot shape, conversion planning, or lowering.

High-leverage candidates:

- inherited source or external member lookup
- constructor and overload applicability
- static imports, static members, nested types, enum constants, and class
  literals
- generic substitution through superclass and interface chains
- wildcard bounds, raw types, erasure, target typing, and diamond inference
- primitive widening, boxing, unboxing, varargs, and assignability
- structured diagnostics for unresolved or ambiguous targets

Required tests:

- minimized resolver tests with neutral names
- assertions on selected symbol, inferred Java type, coercion evidence, or
  structured rejection reason
- conservative tests for unknown external types, erased generics, or ambiguous
  inherited members

### 3. Promote Repeated External Boundaries Into Facts

When a repeated unresolved or guessed row is genuinely an un-translated library
boundary, model it as an external API fact.

Model:

- return type and element type
- nullability
- receiver and parameter mutation
- getter, setter, predicate, iterator, builder, factory, and fluent behavior
- exception summaries
- conservative boundary uncertainty
- user override precedence

Required tests:

- known-model fact tests
- conservative guess tests
- override-precedence tests
- no lowering test unless the owning external facts are already covered

### 4. Strengthen Semantic Facts Before Rust Shape Choices

Once resolver progress depends on program effects, add finite semantic facts.

Focus:

- field initialization and non-null proofs
- trivial getter and setter summaries
- receiver and argument mutation
- assignment in conditions, such as `while ((x = next()) != null)`
- checked and unchecked exception hierarchy facts
- throw, catch, rethrow, and wrapping summaries
- direct and mutual recursion edges

Required tests:

- fact-level tests over `SemanticFacts`, `FactSet`, or `UsageFacts`
- convergence tests for recursive examples
- negative tests where missing proof must keep a conservative fallback

### 5. Feed Object Strategy And Slot Shape From Facts

After Java meaning and semantic facts are explicit, use them to choose Rust
representation and API shape.

Focus:

- concrete struct versus closed enum versus `dyn Trait`
- injected facts for hierarchy closure or enum eligibility
- erased external object boundaries
- owned, borrowed, mutable, optional, and cloned slot shapes
- primitive conversion plans
- collection and iterator adapters
- string ownership and view choices

Required tests:

- object strategy tests for closed and open hierarchies
- slot-shape tests that assert constraints rather than emitted syntax first
- conversion-plan tests for primitive, string, collection, and dynamic-dispatch
  boundaries

## Acceptance Criteria For This Phase

- Each implementation slice has an explicit owning layer.
- Every selected corpus outcome creates at least one non-overfit test.
- GATK benchmark results are recorded before and after each slice.
- External behavior is represented as facts with provenance and override
  precedence.
- Recursive and mutually recursive code produce finite facts.
- Lowering does not gain new Java-name or GATK-name heuristics.
- Conservative fallbacks are explicit and tested when proof is unavailable.
