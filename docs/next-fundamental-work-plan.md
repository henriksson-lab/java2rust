# Next Fundamental Work Plan

## Purpose

This plan captures the next design-critical work for the Java-to-Rust
transpiler. The priority is to improve the semantic backbone before broad
lowering work: resolution, external API facts, type and shape constraints,
object representation, and tests that prove these responsibilities stay in the
right layer.

GATK remains the primary pressure corpus for this phase because it exposes real
resolver and API-modeling failures. It is a benchmark and failure miner, not the
target of hard-coded translation rules.

## Plan Of Record

The next phase is benchmark-guided foundation work. We should keep using GATK
as the main pressure corpus, but every implementation slice must turn the
observed outcome into a general translator rule, an explicit fact, or a
deliberate conservative fallback.

The main priorities are:

1. Improve resolver completeness before Rust output tuning.
2. Normalize un-translated Java library behavior into external API facts.
3. Produce finite semantic facts for nullability, mutation, exceptions,
   recursion, accessors, and control-flow markers.
4. Feed object strategy, slot-shape solving, and conversion planning from
   those facts instead of from lowering-time Java heuristics.
5. Add tests based on the outcome of each slice, while avoiding tests that only
   pass because they copy the corpus currently being benchmarked.

For each slice, choose the owning layer first:

- parser for syntax shapes;
- resolver for Java symbols, overloads, constructors, inheritance, generics,
  static members, casts, coercion evidence, and assignability;
- external API facts for behavior of libraries outside the translated corpus;
- semantic facts for finite program facts such as nullability, effects,
  exceptions, recursion, and accessors;
- object strategy for concrete structs, closed enums, `dyn Trait`, erased
  external objects, and conservative fallback choices;
- slot-shape and conversion planning for Rust ownership, borrowing,
  optionality, primitive conversions, collection adapters, and clones;
- lowering only when all earlier Java and Rust-planning decisions are already
  explicit.

Do not accept a corpus-motivated implementation unless it leaves behind tests
at the responsible layer. The tests should be derived from the observed
outcome, but should try not to overfit to GATK or any other current pressure
corpus.

Outcome-driven test rules:

- Add at least one minimized regression test for every selected corpus outcome.
- Name tests after the general rule, not the corpus class, method, package, or
  file.
- Prefer neutral synthetic Java examples unless the exact syntax or named
  external API is the feature.
- Assert parser, resolver, fact, object-strategy, slot-shape, conversion-plan,
  or diagnostic behavior before asserting generated Rust.
- Add negative or conservative tests when the rule depends on proof that may be
  unavailable.
- Keep tests even when the benchmark count does not move, if they document a
  real Java rule, intentional fallback, or user-override precedence.
- Record corpus evidence in `docs/gatk-resolver-benchmark-report.md`, not in
  test assertions.

## Current Safe-Point Execution Plan

This is the plan from the current committed safe point. The focus is
fundamental design work that also produces measurable corpus movement.

### 1. Re-baseline GATK and classify the next failures

Run the GATK resolver benchmark before each new implementation slice.

Record:

- parser success count
- resolver success count
- unresolved method calls, constructors, and field accesses
- unexplained external API calls
- external API guesses
- top structured resolver diagnostics
- semantic diagnostics and fact counts

Classify each candidate failure by the layer that should own it:

- parser syntax support
- Java resolver semantics
- external API facts
- semantic facts
- object strategy
- slot-shape and conversion planning
- lowering

Do not start from a lowering change unless Java meaning and Rust shape are
already known.

### 2. Prefer foundational resolver fixes

Work first on resolver gaps that would otherwise force later stages to
rediscover Java semantics.

Priority examples:

- overload applicability and rejection reasons
- inherited source and external members
- constructor selection
- static members and nested types
- generic substitution, wildcard bounds, and erasure
- primitive widening, boxing, unboxing, varargs, and target typing
- class literals, casts, and array/container element types

These fixes should produce explicit resolved symbols, inferred Java types, or
structured diagnostics. Later stages should consume those outputs rather than
look at Java method names again.

### 3. Normalize external API behavior as facts

When GATK exposes calls into libraries we are not translating, model the
behavior in the external API fact layer.

Facts should cover:

- return type and element type
- nullability
- receiver and parameter mutation
- getter, setter, predicate, builder, fluent, iterator, and collection roles
- exception behavior
- provenance: user override, known model, source declaration, or conservative
  guess

User overrides must remain the highest-precedence input. Guesses should be
explicitly marked so they can be replaced or audited.

### 4. Strengthen semantic facts before Rust shape choices

Add finite semantic facts for properties that affect representation and
signatures:

- field initialization and non-null proofs
- method receiver mutation
- argument mutation
- trivial getter and setter summaries
- assignment-in-condition markers
- exception hierarchy, throw, catch, rethrow, and wrapping facts
- direct and mutual recursion edges

These facts must saturate to a finite fixed point. Recursive code should add a
finite set of call and effect facts, not create recursive specialization.

### 5. Co-solve object strategy, slot shape, and conversions

After resolver and semantic facts are available, use them to choose Rust-facing
shapes:

- concrete structs
- closed enums for proven subclass/interface families
- `dyn Trait` fallback
- erased external boundaries
- owned values
- shared or mutable borrows
- optional fields and returns
- conservative clones or conversions when proof is unavailable

Avoid choosing a concrete Rust parameter, field, or return shape from the first
example alone. The shape should come from constraints over declarations,
bodies, call sites, overrides, and fallback policy.

### 6. Add tests from outcomes without overfitting

Every implementation slice must add tests based on what the benchmark or
fixture run exposed. The tests should be outcome-driven but generalized.

Required test policy:

- Add at least one minimized regression test for each selected corpus outcome.
- Name the test after the semantic rule, not the GATK class or method.
- Prefer synthetic Java with neutral names such as `Base`, `Derived`,
  `Holder`, `Box<T>`, `Builder`, `Reader`, `Source`, `Sink`, `Column`,
  `CollectionLike`, or `ExternalRecord`.
- Assert the owning layer first: parser node, resolved symbol, inferred Java
  type, external fact, semantic fact, object strategy, slot shape, conversion
  plan, or structured diagnostic.
- Add final Rust-output assertions only after the owning facts or plans are
  directly tested.
- Add negative or conservative tests when the rule depends on proof that may be
  unavailable.
- Keep tests even when a benchmark count does not move, if they cover a real
  Java rule or intentional fallback.
- Record the motivating corpus file or diagnostic in
  `docs/gatk-resolver-benchmark-report.md`, not in the assertion itself.

Copying a real corpus snippet is acceptable only when the exact syntax shape is
the bug: imports, static imports, nested types, anonymous classes, overload
shape, exception syntax, class literals, method references, casts, or
library-boundary usage.

### 7. Verify and report each slice

For each slice:

1. Run focused tests for the changed layer.
2. Run broader tests when shared resolution or facts changed.
3. Re-run the GATK benchmark.
4. Update `docs/gatk-resolver-benchmark-report.md` with before/after counts,
   generalized rule, responsible layer, tests added, corpus motivation, and
   remaining risks.

The benchmark is allowed to show no movement if the generalized rule is still
correct. Record that explicitly and continue with the next highest-leverage
failure.

## Current Trackable Plan

Work the next phase as benchmark-backed design slices. Each slice starts from a
real corpus outcome, but the implementation must land as a general translator
capability with tests at the responsible layer.

1. Re-baseline the GATK resolver benchmark.
   - Record unresolved method calls, constructors, field accesses, external API
     guesses, parser gaps, fact counts, and top structured diagnostics.
   - Update `docs/gatk-resolver-benchmark-report.md` with the current numbers.
   - Classify the leading failures by owner before coding: parser, resolver,
     semantic facts, external API facts, object strategy, slot-shape solving,
     conversion planning, or lowering.

2. Pick the next slice by architectural leverage.
   - Prefer failures that would otherwise force lowering to rediscover Java
     semantics.
   - Prefer work that improves later object representation, slot-shape solving,
     conversion planning, nullability, mutability, or dispatch decisions.
   - Use corpus frequency as a tie-breaker after responsibility and design
     leverage.
   - Do not define a rule from a GATK class name unless the exact external API
     name is the thing being modeled.

3. Add outcome-driven regression tests.
   - Every selected corpus outcome must create at least one regression test
     before or alongside implementation.
   - Tests should try not to overfit to GATK. Prefer minimized synthetic Java
     snippets with neutral names such as `Base`, `Derived`, `ExternalRecord`,
     `Builder`, `Reader`, `Column`, `Box<T>`, or `CollectionLike`.
   - Copy real corpus snippets only when the exact syntax shape is the feature:
     imports, nested types, anonymous classes, overload shape, exception shape,
     class literals, or library-boundary behavior.
   - Assert the responsible layer first: parser shape, resolver target,
     semantic fact, external fact, object strategy, slot shape, conversion plan,
     or structured diagnostic.
   - Add final Rust-output assertions only after the owning facts or plans are
     covered.
   - Add conservative or negative tests when the rule depends on proof that may
     be unavailable.

4. Implement the smallest responsible rule.
   - Resolver owns Java lookup, overloads, constructors, fields, inheritance,
     generics, static members, nested types, casts, primitive coercion,
     boxing/unboxing, varargs, and assignability.
   - Semantic facts own nullability, mutation/effects, recursion, exceptions,
     accessor summaries, control-flow markers, and provenance.
   - External API facts own un-translated library behavior and guessed boundary
     facts, with user overrides taking precedence.
   - Object strategy owns concrete struct, closed enum, dyn trait, erased
     object, and fallback representation decisions.
   - Slot-shape and conversion planning own Rust storage/API shapes and
     expression adaptation.
   - Lowering must consume facts and plans. It must not gain Java semantic
     heuristics for the corpus slice.

5. Verify and report.
   - Run focused tests for the changed layer.
   - Re-run the GATK benchmark.
   - Update `docs/gatk-resolver-benchmark-report.md` with the benchmark
     outcome, the generalized rule, responsible layer, tests added, movement in
     counts, and remaining risks.
   - Keep tests even when the benchmark count does not move if they capture a
     real unsupported Java rule or intentional conservative fallback.

6. Continue with this priority order unless the benchmark exposes a stronger
   foundational blocker.
   - Source resolution: inherited members, nested source types, source fields
     across files, overload candidates, static imports, and structured
     unresolved diagnostics.
   - External API fact normalization: receiver mutability, return type,
     nullability, fluent APIs, builder products, library boundaries, and user
     override precedence.
   - Generic and target-type resolution: diamond inference, wildcard bounds,
     stream/container terminal types, method references, class literals, and
     Java coercion as resolver-owned evidence.
   - Object strategy inputs: hierarchy closure, enum eligibility, dyn-trait
     fallback, casts, `instanceof`, constructors, escape facts, and injected
     hierarchy facts.
   - Slot-shape and conversion co-solving: owned/borrowed/reference shapes,
     mutability, optionality, primitive coercion, collection element adapters,
     string views, dynamic dispatch adaptation, and fallback cloning.

## Guiding Principles

- Keep Java semantic knowledge out of lowering.
- Resolve Java names, overloads, constructors, inheritance, generics, and
  external APIs before Rust representation planning.
- Represent uncertainty as facts and constraints, not as late string or method
  name heuristics.
- Prefer general rules that explain many corpus failures over local patches that
  fix one file.
- Preserve fallback paths such as dynamic dispatch, unknown external boundaries,
  and conservative `Option` or owned shapes when the solver cannot prove a more
  idiomatic result.
- Add tests from benchmark outcomes, but write them as general Java/Rust
  translation rules rather than corpus-specific snapshots.

## Workstream 1: Resolver Completeness

### Goal

Make the resolver the single source of truth for Java symbol meaning.

### Scope

- Constructor overload selection.
- Method overload selection.
- Inherited method and field lookup.
- Source and external class hierarchy assignability.
- Generic method and class substitutions.
- Nested and inner class naming.
- Static members and static imports.
- Structured unresolved diagnostics.

### Algorithm Direction

- Keep resolution as a finite constraint and lookup problem over the parsed
  corpus plus known external facts.
- Separate candidate collection from applicability scoring.
- Let Java assignability, primitive widening, boxing, varargs, generic
  substitution, and exception hierarchy contribute comparable evidence.
- Emit structured diagnostics when candidate selection fails instead of
  returning an untyped expression that later stages must reinterpret.

### Tests From Outcomes

For each high-frequency resolver failure in GATK, add a minimized test that
isolates the rule:

- Use synthetic class names unless the exact external API name is the rule.
- Assert the resolved symbol, selected overload, substituted type, or diagnostic.
- Avoid asserting generated Rust for resolver-only behavior.
- Keep one corpus reference in the benchmark report when it helps explain why
  the test exists.

Examples:

- `FileNotFoundException` is assignable to `IOException` in constructor
  overload selection.
- A subclass inherits a generic method and substitutes the receiver's type
  parameter.
- A static factory overload accepts a class literal with erased or bounded type.
- A nested source type resolves through both qualified and imported names.

## Workstream 2: External API Fact Model

### Goal

Model un-translated Java libraries with explicit facts so downstream ownership,
nullability, and mutability decisions do not depend on scattered special cases.

### Scope

- Known JDK and common library return types.
- External getter, setter, builder, predicate, mutator, and indexed-access roles.
- Receiver mutability.
- Return nullability.
- Parameter mutability and nullability where known.
- Fluent APIs.
- External constructors and exception behavior.
- User-provided overrides with precedence over guessed facts.

### Algorithm Direction

- Maintain a layered fact source order:
  1. user overrides
  2. source declarations
  3. known external API models
  4. conservative structural guesses
  5. unresolved boundary diagnostics
- Keep argument-sensitive external return typing in the external API layer.
- Represent guessed behavior with provenance and confidence.
- Do not let lowering inspect Java method names to infer effects.

### Tests From Outcomes

When the corpus reveals an external API gap:

- Add one fact-level test for the API role or return type.
- Add one resolver test only if overload selection depends on that fact.
- Add final Rust emission tests only after fact and resolver behavior are
  covered.
- Generalize the test to a method family when possible, such as JavaBean
  predicates or fluent setters.

Examples:

- `isX()`, `hasX()`, and `containsX(arg)` are non-null boolean predicate facts.
- External `getX()` returns an unknown-nullability boundary unless a known model
  says otherwise.
- External `setX(value)` mutates the receiver, and a fluent setter returns the
  receiver shape.
- External quality or codec methods can have argument-sensitive return types
  without adding lowering branches.

## Workstream 3: Semantic Fact and Constraint Layer

### Goal

Make semantic facts the contract between Java analysis and Rust representation.

### Scope

- Field nullability.
- Method effects.
- Receiver mutation.
- Trivial getter and setter summaries.
- Recursion edges.
- Assignment-in-condition markers.
- Exception hierarchy facts.
- Object representation alternatives.
- Slot-shape constraints for owned, borrowed, optional, dynamic, enum, and
  external-boundary values.

### Algorithm Direction

- Use finite fact domains and bounded fixed-point saturation.
- Use constraints to co-solve related decisions when local choices are
  interdependent, especially parameter ownership, field shapes, return shapes,
  and object representation.
- Avoid making early concrete Rust type choices where a later global constraint
  can improve them.
- Allow fallback facts when proof fails: owned values, `Option`, `Box<dyn Trait>`,
  conservative cloning, or explicit external boundaries.

### Tests From Outcomes

Corpus-derived tests should target facts and constraints:

- Assert field nullability facts before checking struct field types.
- Assert receiver mutation facts before checking `&self` versus `&mut self`.
- Assert accessor facts before expecting field access rewrites.
- Assert recursion edges and convergence budgets before expecting tail-call
  optimization.
- Assert exception hierarchy and throw facts before expecting `Result`.

Examples:

- A field assigned non-null in every constructor is not represented as
  `Option`.
- A field nulled by any reachable assignment remains optional.
- A trivial setter feeds field nullability and receiver mutability facts.
- Mutual recursion produces finite call-edge facts.
- `while ((x = next()) != null)` records an assignment-condition fact.

## Workstream 4: Object Representation Strategy

### Goal

Choose Rust object representations from facts rather than from syntax alone.

### Scope

- Plain structs.
- Trait objects for interfaces and abstract classes.
- Named enums for closed subclass or implementation sets.
- Dynamic fallback when closed-world proof fails.
- Casts, `instanceof`, and downcast-heavy Java designs.
- User-injected facts that close or override a hierarchy.

### Algorithm Direction

- Treat representation as a solver decision over hierarchy facts, construction
  sites, escape/use facts, casts, and override facts.
- Prefer named enums when the implementation set is known and behavior can be
  represented without unsafe downcasts.
- Prefer traits or `dyn Trait` when hierarchy is open or behavior-driven.
- Preserve an escape hatch for user-provided hierarchy closure facts.
- Keep casts and `instanceof` as semantic constraints, not lowering hacks.

### Tests From Outcomes

Add tests that force representation choices without depending on corpus names:

- Closed interface implementation set maps to an enum when enabled.
- Open interface falls back to trait object.
- A cast-heavy hierarchy remains dynamic unless closure is proven.
- User override can close a hierarchy and enable enum representation.

## Workstream 5: Benchmark Loop

### Goal

Use real code to choose useful slices while keeping the implementation general.

### Loop

1. Run the GATK resolver benchmark.
2. Capture top unresolved methods, constructors, fields, and diagnostics.
3. Classify the top item by responsible layer.
4. Write minimized tests for the rule exposed by the corpus failure.
5. Implement the smallest general resolver, fact, or constraint improvement.
6. Re-run focused tests.
7. Re-run the benchmark.
8. Update the benchmark report with before/after counts and the generalized
   rule that changed.

### Test Policy

Tests must be added based on benchmark outcomes, but they should try not to
overfit to the corpus being improved.

Good tests:

- Small source snippets with generic class names.
- Assertions at the responsible layer.
- Names that describe the Java semantic rule.
- One test per rule plus a benchmark report reference.
- External API tests that model a reusable API behavior, not one project path.

Tests to avoid:

- Full copied corpus files unless the syntax shape cannot be minimized.
- Assertions that mention GATK class names when the rule is general.
- Final Rust snapshots for a resolver or fact-layer bug.
- Lowering tests that pass because one Java method name was special-cased.
- Large fixtures that make it unclear which rule failed.

## Current Execution Plan

### Objective

Drive the next implementation work from GATK resolver failures while improving
the general semantic backbone: resolver facts, external API facts, semantic
constraints, and representation planning. The useful benchmark is GATK progress,
but the deliverable is reusable Java-to-Rust machinery.

### Phase 1: Resolver and External Boundary Stabilization

Use the current GATK resolver benchmark as the ordering signal.

For each slice:

1. Pick one top unresolved method, constructor, field, or overload diagnostic.
2. Identify the responsible layer before editing code:
   - resolver lookup or overload scoring
   - external API fact model
   - Java type inference or generic substitution
   - semantic fact generation
   - representation or slot-shape planning
3. Add a minimized regression test that proves the general rule.
4. Implement the rule only in the owning layer.
5. Re-run the focused test and the GATK benchmark.
6. Update `docs/gatk-resolver-benchmark-report.md` with:
   - before/after counts
   - the generalized rule
   - why the test is not a GATK-specific fixture

Do not proceed with a slice when the only available fix is a corpus-specific
lowering branch. In that case, record the missing abstraction first.

### Phase 2: Semantic Fact Layer and Co-Solved Constraints

Once the remaining GATK resolver failures increasingly depend on ownership,
nullability, mutability, or object representation, shift from one-off resolver
facts to constraint-backed semantic facts.

Implement these as co-solved finite fact domains:

- field nullability and constructor initialization
- receiver and argument mutation
- trivial accessor summaries
- parameter, return, and field slot-shape constraints
- recursion and mutual-recursion call edges
- assignment-in-condition facts
- exception throw, catch, and hierarchy facts
- external boundary facts with provenance

The solver must allow conservative fallback facts instead of forcing an
idiomatic Rust choice too early. Examples of fallbacks are `Option`, owned
values, cloned values, `Box<dyn Trait>`, erased external values, and explicit
diagnostics.

### Phase 3: Object Representation Decisions

Use hierarchy, construction, casts, `instanceof`, escape facts, and user
overrides to choose Rust representations.

Representation choices should be expressed as plans, not emitted directly by
lowering:

- concrete struct when the Java type is concrete and direct
- named enum when a closed subclass or implementation set is proven or
  injected by user facts
- `dyn Trait` or boxed trait object for open behavior-driven hierarchies
- erased external boundary for unknown object shapes
- conservative dynamic fallback when casts or runtime type tests prevent a
  closed representation

Add tests that prove the representation decision independent of GATK names.

### Phase 4: Lowering Only After Facts Are Stable

Lowering should consume resolved symbols, semantic facts, signature plans, slot
shapes, and representation plans. It should not rediscover Java semantics from
method names, field names, or corpus examples.

Only add final Rust output tests after the lower-level facts and plans already
have direct tests. This prevents idiomatic Rust improvements from masking a
missing resolver or semantic fact.

## Test Derivation Gate

Every benchmark-derived implementation slice must produce tests from the
outcome, but the tests must avoid overfitting to the corpus being improved.

A slice is ready to implement only when the test can be described as one of:

- a Java language rule
- a JDK or common-library API fact
- a structural external API pattern
- a finite semantic fact or constraint
- a Rust representation planning rule
- a diagnostic contract for a still-unknown boundary

If the test requires a copied GATK class name, package path, or large method
body, first try to reduce it. Keep the original corpus reference in the
benchmark report, not in the assertion, unless the exact syntax shape is the
bug.

## Immediate Sequence

1. Continue the GATK resolver benchmark loop from the current top diagnostics.
2. For the next constructor or overload failure, add a minimized resolver test
   for the Java assignability or overload rule.
3. Implement the resolver or external API fact improvement in the owning module.
4. Update `docs/gatk-resolver-benchmark-report.md` with the measured effect.
5. Repeat for the next high-volume failure until the remaining failures point
   to a different fundamental layer.
6. When resolver progress stalls, switch to semantic fact constraints for field
   nullability, accessors, receiver mutability, and object representation.

## Acceptance Criteria

- Each implementation slice has at least one generalized test derived from a
  corpus outcome.
- Benchmark reports show before/after counts for changed failure categories.
- No new Java-name heuristics are added to lowering.
- Resolver, external API facts, semantic facts, representation, signature, and
  lowering responsibilities remain separate.
- Recursive and mutually dependent facts still converge within a finite budget.
- User overrides can refine or replace conservative inferred facts.
