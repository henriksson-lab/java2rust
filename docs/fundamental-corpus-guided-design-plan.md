# Fundamental Corpus-Guided Design Plan

## Purpose

Use a real corpus, currently GATK, to expose design pressure in the Java-to-Rust
transpiler while keeping the implementation general.

The aim is not to make GATK pass by accumulating corpus-shaped exceptions. The
aim is to discover Java semantic rules, Rust representation constraints, and
external-library boundaries that the translator must model explicitly.

## Core Principle

Every benchmark-derived improvement must land in the owning layer:

- parser: Java syntax shape
- resolver: names, overloads, inheritance, generics, coercion evidence, and
  structured unresolved diagnostics
- external facts: behavior of libraries outside the translated corpus
- semantic facts: finite facts about nullability, mutation, accessors,
  exceptions, recursion, and control flow
- object strategy: struct, enum, trait object, erased boundary, or fallback
- slot shape and conversion planning: ownership, borrowing, optionality,
  primitive conversions, collection adapters, and clones
- lowering: final syntax emission only after earlier layers made the decision
  explicit

Lowering should not learn Java semantics from method names, class names, or the
current benchmark corpus.

## Why GATK For Now

GATK is the current pressure corpus because it contains realistic stressors:

- large source surface with many partially known types
- inherited source and external members
- nested types, static members, enum constants, and class literals
- generics, wildcard-like API surfaces, and collections
- fluent builders and external library boundaries
- exceptions and exception class hierarchies
- recursive and mutually dependent control flow

GATK is evidence. It is not the specification. Other corpora should be used as
cross-checks when a proposed rule starts to look too corpus-specific.

## Workstreams

### 1. Resolver Foundations

Improve Java meaning before Rust planning:

- inherited method and field lookup
- overload applicability and rejection reasons
- constructor resolution, including diamond inference pressure
- static imports, static members, enum constants, and nested type names
- generic substitution through superclass and interface chains
- erasure, wildcard bounds, target typing, boxing, unboxing, widening, and
  varargs
- explicit casts, `instanceof`, class literals, and array/container element
  types

Output should be resolved symbols, inferred Java types, coercion evidence, or a
structured diagnostic.

### 2. External API Fact Boundaries

Model un-translated libraries as auditable facts:

- return type and element type
- return nullability
- receiver and argument mutation
- getter, setter, predicate, iterator, builder, and fluent roles
- exception summaries
- external object and lifetime boundaries
- provenance: user override, known model, source declaration, corpus summary,
  or guess

User overrides must win over known models, and known models must win over
guesses. Guesses must stay visible in reports.

### 3. Semantic Fact Layer

Keep Java-wide reasoning finite and queryable:

- field initialization and non-null proofs
- trivial getter and setter summaries
- receiver and argument effects
- assignment-in-condition markers
- exception hierarchy, throw, catch, rethrow, and wrapping facts
- direct and mutual recursion edges
- convergence diagnostics

Recursive source code should still produce a finite fact set. No fact key should
contain unbounded expression text or recursively specialized Rust shapes.

### 4. Object Strategy Inputs

Choose object representation from facts, not from lowering convenience:

- concrete structs for ordinary classes
- named enums for proven closed subclass or interface implementation families
- `dyn Trait` fallback when closure cannot be proven
- erased boundaries for external or unknown objects
- conservative fallback when casts, reflection, unknown implementors, or
  external construction make an idiomatic model unsafe

Injected facts may force or override closure, enum eligibility, or fallback
strategy, but provenance must remain visible.

### 5. Slot Shape And Conversion Co-Solving

Solve Rust-facing storage and API shapes from constraints:

- owned values
- shared and mutable borrows
- string and string-view shapes
- `Option<T>` only when nullability requires it or proof is missing
- primitive coercions and explicit casts
- collection and iterator adapters
- trait object, enum, and erased-object adapters
- conservative clones and conversions at explicit boundaries

These decisions need a shared constraint model. They should not be solved as
independent local rewrites in lowering.

## Benchmark-Guided Slice Loop

Each implementation slice should follow this loop:

1. Re-run the benchmark and capture current counts.
2. Pick one outcome by design leverage, not only by frequency.
3. Inspect representative corpus occurrences.
4. Classify the owning layer before implementation.
5. Reduce the outcome to a general Java or Rust-planning rule.
6. Add tests based on the outcome.
7. Implement the smallest responsible rule in the owning layer.
8. Run focused tests, broader tests when shared behavior changed, and the
   benchmark.
9. Update `docs/gatk-resolver-benchmark-report.md` with corpus evidence,
   generalized rule, tests added, before/after counts, and remaining risk.

If the outcome cannot yet be generalized, improve diagnostics or add an
explicit conservative fallback test instead of adding speculative production
logic.

## Test Policy

Tests are required output of every selected slice. They should be based on the
benchmark outcome, but they should try not to overfit to the corpus we are
trying to get to work.

Required rules:

- Add at least one focused regression test for each selected outcome.
- Name the test after the generalized rule, not a GATK class, method, or
  package.
- Prefer small synthetic Java examples with neutral names such as `Base`,
  `Derived`, `Holder`, `Box<T>`, `Builder`, `Reader`, `Writer`, `Source`,
  `Sink`, `Item`, `ExternalRecord`, or `CollectionLike`.
- Assert the responsible layer first: parser shape, resolved symbol, inferred
  Java type, coercion evidence, external fact, semantic fact, object strategy,
  slot shape, conversion plan, or structured diagnostic.
- Add generated-Rust assertions only after the owning facts or plans have
  direct tests.
- Add negative or conservative tests when the rule depends on proof that may be
  unavailable.
- Keep tests even when the benchmark count does not move, if they document a
  real Java rule, an intentional fallback, user-override precedence, or a
  safety boundary.
- Record the motivating corpus file, diagnostic, or benchmark category in
  `docs/gatk-resolver-benchmark-report.md`, not in the test oracle.

Copy a real corpus snippet only when the exact syntax shape is the behavior
under test: imports, static imports, nested types, anonymous classes, overload
shape, exception syntax, class literals, method references, casts, or a named
external API boundary.

## Anti-Overfit Gate

Before accepting a corpus-derived test or implementation, answer these
questions:

1. Would this rule still make sense for another Java corpus?
2. Is the assertion at the layer that owns the behavior?
3. Does the test avoid incidental corpus names and package paths?
4. If a real API name is used, is that API identity itself the behavior being
   modeled?
5. Is there a conservative or negative case when the rule depends on unavailable
   proof?
6. Does the implementation avoid adding a late lowering heuristic?

If not, reduce the fixture, move corpus details into the benchmark report, or
split the slice into a diagnostic/fallback task first.

## Near-Term Priority

The next design-critical work should stay near the resolver and fact boundary:

1. Re-baseline GATK and classify the highest-leverage remaining unresolved or
   over-conservative categories.
2. Prefer source/member resolver rules before adding more curated external
   constants.
3. Promote repeated external boundary behavior into explicit facts only when it
   is a stable library or API role.
4. Add semantic facts where later object strategy, slot shape, conversion, or
   exception handling would otherwise need to rediscover Java behavior.
5. Use benchmark movement as feedback, but judge success by whether the rule is
   generalized, owned by the right layer, tested, and recorded.

## Acceptance Criteria

- The benchmark command and report remain reproducible.
- Every selected corpus outcome produces a generalized test or an explicit
  conservative fallback test.
- Tests avoid corpus overfitting unless an exact syntax form or external API
  identity is the feature.
- New production behavior is implemented in the owning module.
- External guesses, overrides, and known facts preserve visible provenance.
- Recursive or mutually dependent code produces finite facts.
- Lowering remains a consumer of facts and plans, not a hidden resolver or
  semantic-analysis layer.
