# Next Big Things Plan

## Purpose

Define the next design-critical work for the Java-to-Rust transpiler.

The goal is to improve the semantic backbone of the translator before broad
Rust-output work. GATK is the current pressure corpus because it exposes real
resolver, external API, inheritance, generic, exception, and collection-shape
problems. It is a benchmark and failure miner, not the specification.

## Trackable Goal

Run the next phase as benchmark-backed foundation slices until the translator
has a clearer semantic contract between Java analysis and Rust planning.

The immediate target is not "make GATK translate." The target is to use GATK to
find high-pressure semantic gaps, extract general rules from those gaps, add
tests that do not overfit to GATK, and implement the rule in the responsible
layer.

A slice is done only when it has:

- a classified corpus outcome or fixture outcome;
- a generalized rule or explicit conservative fallback;
- tests at the responsible layer, derived from the outcome but not coupled to
  incidental corpus names;
- an implementation in the owning module boundary;
- focused verification and a refreshed benchmark/report entry.

## Guiding Rule

Do not fix corpus failures by adding late lowering special cases.

Each selected outcome should become one of:

- a parser syntax capability
- a resolver rule or structured diagnostic
- an external API fact
- a semantic fact
- an object strategy rule
- a slot-shape or conversion-planning rule
- an explicit conservative fallback

Lowering should consume these facts and plans. It should not rediscover Java
semantics from method names, class names, or corpus-specific patterns.

## Priority Order

### 1. Resolver Completeness

Work first on Java meaning that every later stage depends on:

- inherited source and external methods
- inherited source and external fields
- constructor and method overload resolution
- static imports, static members, enum constants, and nested types
- generic substitution through superclass and interface chains
- wildcard bounds, raw types, erasure, target typing, and diamond inference
- primitive widening, boxing, unboxing, and varargs
- casts, `instanceof`, class literals, and array/container element types

Resolver output should be resolved symbols, inferred Java types, coercion
evidence, or structured diagnostics.

### 2. External API Facts

Model libraries outside the translated corpus as auditable facts:

- return type and element type
- return nullability
- receiver and parameter mutation
- getter, setter, predicate, iterator, builder, and fluent roles
- exception summaries
- external object and lifetime boundaries
- provenance: user override, known model, source declaration, or guess

User overrides take precedence over known models and guesses. Guesses must stay
visible in facts and benchmark reports.

### 3. Semantic Facts

Strengthen finite facts that feed representation and signatures:

- field initialization and non-null proofs
- receiver and argument mutation effects
- trivial getter and setter summaries
- assignment-in-condition markers
- exception hierarchy, throw, catch, rethrow, and wrapping facts
- direct and mutual recursion edges
- convergence diagnostics

Recursive Java code should produce a finite fact set. It must not create
recursive specialization or unbounded expression-keyed facts.

### 4. Object Strategy Inputs

Use resolver and semantic facts to decide which Rust object representation is
available:

- concrete structs
- named enums for proven closed subclass or interface families
- `dyn Trait` fallback when closure cannot be proven
- erased external object boundaries
- conservative fallback when casts, reflection, unknown implementors, or
  external construction prevent a closed model

Injected facts may force or override hierarchy closure, enum eligibility, or
fallback strategy, but provenance must remain visible.

### 5. Slot Shape And Conversion Co-Solving

Choose Rust-facing API and storage shapes from constraints:

- owned values
- shared and mutable borrows
- string and string-view shapes
- `Option<T>` only when nullability requires it or proof is missing
- primitive coercions and explicit casts
- collection and iterator element adapters
- trait object, enum, and erased-object adapters
- conservative clones and conversions at explicit boundaries

Slot-shape and conversion planning should consume resolver and semantic facts.
It should not be driven by whichever example happened to be implemented first.

## Benchmark-Guided Operating Loop

Every implementation slice follows this loop:

1. Re-baseline the GATK resolver benchmark.
2. Classify the leading outcome by owning layer.
3. Reduce the outcome to a general Java or Rust-planning rule.
4. Design the test first: decide which parser, resolver, fact, object-strategy,
   slot-shape, conversion-plan, diagnostic, or output behavior should prove the
   rule.
5. Add tests based on that outcome before or alongside implementation.
6. Implement the smallest responsible rule in the owning layer.
7. Run focused tests, broader tests when shared behavior changed, and the
   corpus benchmark.
8. Record before/after counts, responsible layer, tests, and remaining risks in
   `docs/gatk-resolver-benchmark-report.md`.

Frequency is a tie-breaker. Architectural leverage and correct ownership come
first.

## Test Gate

Tests are required output of each selected slice, not optional follow-up work.
They should be based on the observed corpus outcome, but they should test the
general translator rule exposed by that outcome rather than the corpus itself.

Before accepting a production rule, the slice should include:

- at least one focused test for the owning layer;
- a reduced Java example with neutral names when the corpus name is incidental;
- a negative or conservative test when the rule depends on proof that may be
  missing;
- a provenance test when the rule adds or changes an external API fact;
- a benchmark report entry that links the corpus evidence to the generalized
  test and rule.

The benchmark can justify why a test exists, but it should not define the test
oracle unless the exact external API, syntax form, or library boundary is the
thing being modeled.

Tests should be added based on what actually happened in the benchmark or
fixture run. They should not be speculative coverage unrelated to the selected
slice, and they should not merely snapshot GATK behavior. The test should answer:
"what general translator rule did this outcome reveal?"

## Outcome-Driven Test Policy

Every selected corpus outcome must create tests. These tests should use the
corpus result as evidence, but should try not to overfit to the corpus we are
trying to get to work.

Required policy:

- Add at least one minimized regression test for each selected outcome.
- Name tests after the generalized rule, not the GATK class, method, or package.
- Prefer neutral synthetic Java names such as `Base`, `Derived`, `Holder`,
  `Box<T>`, `Builder`, `Reader`, `Source`, `Sink`, `Column`,
  `CollectionLike`, `ExternalRecord`, `Item`, or `Evidence`.
- Assert the responsible layer first: parser shape, resolved symbol, inferred
  Java type, coercion evidence, external fact, semantic fact, object strategy,
  slot shape, conversion plan, or structured diagnostic.
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

## Next Concrete Work

### Step 1: Re-baseline And Classify

Run the GATK resolver benchmark and update
`docs/gatk-resolver-benchmark-report.md` with:

- files seen, parsed, and resolved
- resolver diagnostics
- unresolved method calls, constructors, and field accesses
- explained and unexplained external API calls
- external API guesses
- semantic diagnostics
- fact counts
- top remaining diagnostics

Classify the top remaining outcomes by owner before coding.

Deliverable:

- a short benchmark-report section naming the leading outcomes;
- a candidate slice selected by design leverage;
- a note explaining why the candidate belongs to parser, resolver, external API
  facts, semantic facts, object strategy, slot-shape planning, conversion
  planning, or lowering.

### Step 2: Prefer General Source/Member Rules Before More Curated Constants

The recent benchmark work reduced many unresolved field guesses by modeling
external constant families. That was useful, but the next slice should check
whether the remaining failures expose a more general resolver rule before adding
more named constants.

Start with the highest recurring source/member-looking outcomes, such as:

- fields reached through source classes, inherited source classes, generic
  bounds, or external superclasses
- enum constants and static fields whose receiver is known but whose owner is
  represented through nested or package-qualified names
- unresolved field receivers whose static type is a source class, source
  generic parameter, or external class with a modeled superclass
- conservative external guesses that repeat because a reusable library boundary
  role is missing

For each candidate:

1. Inspect the motivating corpus occurrences.
2. Decide whether this is a resolver lookup rule, a missing source symbol
   relation, a missing external fact, or an intentional conservative fallback.
3. Add a minimized generalized test based on the observed outcome.
4. Add a negative or fallback test if unknown external state, open hierarchy,
   erased generics, or missing proof changes the correct behavior.
5. Implement only the owning rule.
6. Re-run the focused test, full tests when shared behavior changed, and the
   GATK benchmark.
7. Record the before/after result and remaining risk in
   `docs/gatk-resolver-benchmark-report.md`.

Do not add a new curated external constant until this classification says the
missing behavior really is a stable library fact rather than a resolver or
semantic rule.

### Step 3: Finish Resolver And External API Boundary Slice

The current high-value slice is resolver and external API boundary quality. It
has the best leverage because object strategy, slot shape, conversion planning,
and lowering all depend on resolved Java meaning.

Work only in the owning layer:

- resolver for Java lookup, overloads, generics, fields, constructors,
  coercions, and assignability
- `stdlib` or external fact modules for un-translated library behavior
- semantic analysis for finite effect, nullability, exception, recursion, and
  accessor facts

Do not add new lowering name heuristics for this slice.

Tests required for this step:

- Add resolver tests for lookup, inherited members, generic substitution,
  overload applicability, static members, enum constants, or coercion evidence
  using neutral synthetic Java.
- Add external-fact tests only when the behavior belongs to an un-translated
  library boundary.
- Add conservative tests when proof is unavailable, such as open-world
  hierarchy, unknown external fields, or erased generic arguments.
- Avoid GATK class names unless the exact external API is the fact being
  modeled.

### Step 4: Promote Repeated Boundary Guesses Into Facts

When benchmark results show repeated external API guesses, decide whether they
are:

- a known Java/JDK/library model
- a general getter, setter, builder, iterator, or collection-shape rule
- a user-overridable project fact
- a conservative unknown-boundary diagnostic

Add tests for the generalized rule, then promote the behavior into the external
API fact layer with visible provenance.

Tests required for this step:

- Add tests that assert provenance: user override, known model, source
  declaration, or guess.
- Add tests that show guessed behavior remains visible and auditable.
- Add tests for override precedence when a user fact conflicts with a known
  model or conservative guess.

### Step 5: Build Toward Object And Slot Decisions

After the resolver/external boundary is stable enough, prioritize facts that
will affect Rust shape:

- field non-null proofs
- trivial accessor summaries
- mutation summaries
- exception and recursion summaries
- hierarchy closure and enum eligibility
- fallback selection between concrete struct, closed enum, `dyn Trait`, and
  erased external object
- slot-shape and conversion constraints for ownership, borrowing,
  optionality, coercion, and collection adapters

Each item should have fact-level or plan-level tests before output snapshots.

Tests required for this step:

- Add semantic-fact tests for field non-null proofs, accessor summaries,
  mutation summaries, exceptions, and recursion before expecting better Rust
  signatures.
- Add object-strategy tests for closed enum eligibility, `dyn Trait` fallback,
  erased external fallback, and user overrides before lowering snapshots.
- Add slot-shape and conversion-plan tests for ownership, borrowing,
  optionality, numeric coercion, collection adapters, and dispatch adapters
  before final Rust-output assertions.

## Immediate Slice

The next implementation slice should be chosen as follows:

1. Re-run or inspect the latest GATK resolver benchmark.
2. Pick one top remaining field/method/constructor outcome that looks like a
   general source/member resolution or external-boundary rule.
3. Add at least one generalized test from that outcome before or alongside the
   implementation.
4. Implement the smallest rule in the resolver, external fact layer, or
   semantic fact layer.
5. Verify with focused tests, broader tests if shared behavior changed, and the
   GATK benchmark.
6. Update `docs/gatk-resolver-benchmark-report.md`.

The preferred first target is source/member resolution, not another batch of
library constants, unless inspection proves that the selected outcome is only a
stable external API fact.

## Test Examples To Prefer

These are examples of the kind of tests to add from outcomes. They are not a
fixed checklist; the selected benchmark outcome decides which ones matter.

- A source subclass inherits a generic method and substitutes the receiver type
  parameter.
- A static field receiver resolves through a source class instead of becoming a
  weak external receiver.
- An external object method such as `equals`, `hashCode`, `toString`, or
  `getClass` produces a built-in object-method fact rather than a corpus-specific
  external guess.
- A repeated external fluent setter records receiver mutation, fluent return,
  and provenance without lowering inspecting the method name.
- A wildcard or diamond constructor call produces resolver-owned type evidence
  that slot-shape planning can consume later.
- A missing proof case remains conservative and records why the translator did
  not choose a more idiomatic Rust shape.

Avoid tests that only prove a specific GATK class name, package name, or method
count changed. Those details belong in the benchmark report.

## Acceptance Criteria

- Each corpus-driven slice has generalized tests.
- Tests are derived from outcomes but avoid GATK-specific assertions unless the
  exact syntax or external API is the feature.
- Responsibility stays in parser, resolver, external facts, semantic facts,
  object strategy, slot-shape planning, conversion planning, or lowering as
  appropriate.
- Lowering does not gain Java semantic special cases for benchmark movement.
- Benchmark reports connect corpus evidence to generalized rules and tests.
- Conservative fallbacks remain explicit when proof is unavailable.
