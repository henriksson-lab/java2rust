# Fundamental Design Next Plan

## Purpose

This is the trackable plan for the next design-critical phase of the
Java-to-Rust transpiler.

The goal is to strengthen the semantic backbone before broad lowering work:
Java resolution, external API facts, semantic facts, object strategy inputs,
slot-shape solving, and conversion planning.

GATK is the main pressure corpus for this phase because it exposes real
inheritance, generics, external APIs, collections, exceptions, and builder-like
Java idioms. It is a benchmark and failure miner, not the target of hard-coded
rules.

## Operating Rule

Work in benchmark-backed design slices:

1. Start from a real corpus outcome.
2. Reduce it to a general Java semantic, external API, fact, solver, or
   representation rule.
3. Add tests for that generalized rule, based on the corpus outcome but
   deliberately written to avoid overfitting to that corpus.
4. Implement the rule in the responsible layer.
5. Re-run focused tests and the corpus benchmark.
6. Record the measured result.

Do not add a production rule whose only explanation is one GATK class, method,
or package path. If the exact external API name is itself the semantic fact,
model it as an external API fact with provenance rather than as lowering logic.

Every transformation rule must also state the desired Rust before the
implementation mechanics. Use `docs/transformation-rule-contract.md` for the
required order: **Ideal Rust Target** first, **Implementation Rule** second.
The target section should make the intended Rust API, expression, type,
ownership, borrowing, optionality, mutability, dispatch, conversion, error, and
external-boundary shape explicit before any module-specific lowering rule is
accepted.

Apply this at the level of each transformation rule, not only at the level of a
plan item. When one design slice covers several mappings, each mapping needs
its own **Ideal Rust Target** before its **Implementation Rule**, unless the
slice explicitly documents that the mappings share one target Rust shape, one
set of required facts, one owning layer, and one fallback shape.

## Corpus-Derived Test Gate

Every selected corpus outcome must produce tests before or alongside the
implementation. These tests are part of the design work, not just regression
coverage.

The test should be derived from the observed failure, but the assertion should
target the general rule that the failure exposed. Prefer a reduced synthetic
Java example with neutral names over a copied corpus class. The test should
still make sense if the same language or library pattern appeared in another
large Java corpus.

For each slice:

1. Record the motivating corpus diagnostic and file in the benchmark report.
2. Reduce the outcome to the smallest general rule that explains it.
3. Add a focused test against the responsible layer: parser, resolver,
   external fact, semantic fact, object strategy, slot shape, conversion plan,
   or diagnostic.
4. Add negative or conservative tests when missing proof changes the expected
   behavior.
5. Add Rust-output tests only after the owning facts or plans have direct
   tests.
6. Keep the test even if the benchmark count does not move, when it captures a
   real Java rule, a required fallback, or user override precedence.

Copy a real corpus snippet only when the exact syntax shape is the feature
under test, such as imports, nested types, anonymous classes, overload shape,
exception shape, class literals, method references, static imports, casts, or a
named external API boundary.

## Plan Of Record

The next work should stay focused on foundational structure. GATK is the
pressure corpus for ordering, but each accepted change must become a general
translator capability.

The main next objective is to turn corpus pressure into durable semantic
infrastructure:

1. Use GATK to expose the highest-value unresolved resolver and external API
   failures.
2. Reduce each selected failure to a general Java rule or Rust-planning
   responsibility.
3. Add tests from the observed outcome, but write them so they would still be
   meaningful for another large Java corpus.
4. Implement the smallest responsible rule in the owning layer.
5. Re-run focused tests and the GATK benchmark.
6. Record the result, including when the generalized rule is correct but does
   not move the current benchmark count.

The expected early slices are resolver and external-boundary work because those
facts shape every later decision:

- source and inherited member lookup
- overload and constructor applicability
- generic substitution and target typing
- external library return types, mutability, nullability, and collection shape
- structured diagnostics for unresolved calls, constructors, and fields

After those stabilize, move the same outcome-driven loop to semantic facts,
object strategy, slot-shape solving, and conversion planning.

## Immediate Execution Plan

Use this as the concrete plan for the next implementation cycle. The point is
to improve the semantic backbone in an order that pays off for later design,
while using GATK as the benchmark that exposes real pressure.

### Concrete Next Cycle

Treat the next implementation cycle as a benchmark-backed resolver and fact
slice, not as a broad lowering pass.

1. Re-run or inspect the latest GATK resolver benchmark.
   - Start from the top remaining structured diagnostics, unresolved method
     calls, unresolved constructors, and unresolved field accesses.
   - Prefer candidates that affect later type shape, ownership, nullability,
     mutability, dispatch, or exception decisions.
   - Use frequency as a tie-breaker after architectural responsibility.

2. Select one failure class and write down its owner.
   - Resolver owns Java lookup, overload selection, assignability, inheritance,
     generic substitution, target typing, casts, primitive coercion,
     boxing/unboxing, varargs, static members, fields, and constructors.
   - External API facts own behavior for libraries outside the translated
     corpus: return type, nullability, mutability, fluent/builder behavior,
     collection shape, exception summaries, and boundary uncertainty.
   - Semantic facts own field initialization, method effects, accessor
     summaries, recursion, exceptions, assignment-in-condition markers, and
     provenance.
   - Object strategy owns concrete struct, closed enum, `dyn Trait`, erased
     external, and conservative fallback representation choices.
   - Slot-shape and conversion planning own Rust API/storage shape and
     expression adaptation once Java meaning is known.
   - Lowering should only consume these decisions.

3. Add tests based on the outcome before or alongside implementation.
   - Every selected corpus outcome must produce at least one regression test.
   - The test should try not to overfit to GATK. Prefer minimized synthetic Java
     with neutral names and only the syntax needed for the rule.
   - Name the test after the generalized semantic rule, not the corpus class,
     package, or method that exposed it.
   - Assert the responsible layer first: parser shape, resolved symbol,
     inferred Java type, external API fact, semantic fact, representation
     choice, slot shape, conversion plan, or structured diagnostic.
   - Add Rust-output assertions only when the owning facts or plans are already
     covered.
   - Add negative or conservative tests when proof may be unavailable, so the
     fallback behavior is intentional rather than accidental.

4. Implement the smallest responsible rule.
   - Keep the implementation in the owning module.
   - Do not add Java method-name or corpus-name heuristics to lowering.
   - When behavior is uncertain, emit a conservative fact, fallback plan, or
     structured diagnostic rather than guessing late.
   - Preserve user override precedence over known models and guessed facts.

5. Verify and record the result.
   - Run the focused tests for the changed layer.
   - Run broader tests if the slice touches shared resolver, fact, type, or
     slot-shape behavior.
   - Re-run the GATK benchmark.
   - Update `docs/gatk-resolver-benchmark-report.md` with the motivating
     corpus outcome, generalized rule, responsible layer, tests added,
     before/after counts, and remaining risks.
   - Keep the tests even when the GATK count does not move if they capture a
     real Java rule or a deliberate conservative fallback.

### 1. Re-Baseline And Classify The Pressure Corpus

Run the GATK resolver benchmark from the current committed state and update
`docs/gatk-resolver-benchmark-report.md`.

Record:

- files parsed and resolved
- top unresolved calls, constructors, and fields
- top structured resolver diagnostics
- unexplained external API calls
- fact counts and semantic diagnostics
- any new parser gaps

Then classify the top failures by owner before coding:

- parser
- resolver
- external API facts
- semantic facts
- object strategy
- slot-shape and conversion planning
- lowering

Lowering should only be the owner when the Java meaning and Rust shape are
already known.

### 2. Pick Slices By Architectural Leverage

Choose the next slice by asking which failure would otherwise force a later
stage to rediscover Java semantics.

Priority order:

1. Java resolution gaps that block type, inheritance, overload, field, or
   constructor meaning.
2. External API facts that affect mutability, nullability, return type,
   fluent/builder behavior, collection shape, or exception behavior.
3. Generic, target-type, primitive coercion, boxing/unboxing, varargs, and
   container element inference.
4. Semantic facts that drive Rust representation: field nullability, receiver
   mutation, accessor summaries, recursion, exceptions, and
   assignment-in-condition markers.
5. Object strategy and slot-shape constraints that choose between concrete
   structs, closed enums, `dyn Trait`, erased externals, owned values,
   borrowed values, mutable borrows, `Option`, and conservative fallbacks.

Use GATK frequency as a tie-breaker, not as the definition of the rule.

### 3. Add Outcome-Driven Tests Before Or Alongside The Rule

Every selected corpus outcome must produce tests. These tests should try not to
overfit to the corpus being improved.

Required test policy for each slice:

1. Add at least one minimized regression test based on the observed outcome.
2. Name the test after the generalized semantic rule, not the GATK class or
   method that exposed it.
3. Prefer small synthetic Java snippets with neutral names such as `Base`,
   `Derived`, `Box<T>`, `Builder`, `Reader`, `Column`, `ExternalRecord`,
   `CollectionLike`, `Source`, `Sink`, or `Holder`.
4. Assert the responsible layer first: parser shape, resolver target,
   semantic fact, external API fact, object strategy, slot shape, conversion
   plan, or structured diagnostic.
5. Add Rust-output assertions only after the owning facts or plans are covered.
6. Add negative or conservative tests when the rule depends on proof that may
   be unavailable.
7. Record the motivating GATK file or diagnostic in
   `docs/gatk-resolver-benchmark-report.md`, not in the assertion, unless the
   exact syntax shape is the bug.

Copying a real GATK snippet is acceptable only when the exact syntax shape is
the feature being tested: imports, nested types, anonymous classes, overload
shape, exception shape, class literals, method references, static imports, or
library-boundary behavior.

Keep tests even when the benchmark count does not move if they capture a real
Java rule, a useful conservative fallback, or a boundary where user overrides
must take precedence.

### 4. Implement Only In The Responsible Layer

For each slice:

1. Implement the smallest rule in the layer that owns the decision.
2. Keep Java semantic knowledge out of lowering.
3. Prefer facts, constraints, and structured diagnostics over ad hoc emission
   branches.
4. Make uncertainty explicit through conservative facts or fallback plans.
5. Preserve user override precedence over built-in guesses.

The rule is not done if a downstream stage still has to inspect Java method
names, field names, or corpus class names to recover the same meaning.

### 5. Verify, Benchmark, And Report

For each slice:

1. Run focused tests for the changed layer.
2. Run broader tests when the change affects shared resolution, type facts, or
   slot-shape planning.
3. Re-run the GATK benchmark.
4. Update `docs/gatk-resolver-benchmark-report.md` with:
   - before and after counts
   - the generalized rule added
   - the responsible layer
   - tests added
   - corpus outcome that motivated the slice
   - remaining risks or known conservative fallbacks

Do not hide a no-movement benchmark result. If the generalized rule is correct
but does not move the current top count, record that explicitly and continue.

### 1. Stabilize Java Resolution Before Lowering

Deliverables:

- inherited member lookup across source and known external hierarchies
- constructor and method overload applicability with structured rejection
  reasons
- static members, static imports, nested classes, class literals, and enum
  constants resolved before lowering
- generic substitutions, wildcard bounds, primitive coercion, boxing/unboxing,
  varargs, and target typing represented as resolver evidence

Test requirement:

- For every GATK resolver outcome selected, add a minimized resolver test that
  asserts the selected symbol, rejected candidates, inferred Java type, or
  structured diagnostic.
- The test should use neutral class names unless the exact JDK or library API
  name is the fact being modeled.

### 2. Strengthen External API Facts

Deliverables:

- fact entries for un-translated library calls that affect return type,
  nullability, receiver mutation, parameter mutation, exceptions, fluent
  returns, builders, collections, iterators, streams, and optionals
- provenance and confidence for known facts, guessed facts, and user overrides
- a single precedence path through effective facts so downstream stages do not
  inspect Java method names

Test requirement:

- Add fact-level tests for external API behavior exposed by the corpus.
- Add resolver tests only when overload selection or expression typing depends
  on the external fact.
- Avoid final Rust snapshots until the external fact and resolver behavior are
  already covered.

### 3. Build Finite Semantic Facts And Constraint Inputs

Deliverables:

- field initialization and non-null proofs
- receiver and argument mutation summaries
- trivial getter and setter summaries that can feed field and slot decisions
- assignment-in-condition facts such as `while ((x = next()) != null)`
- exception throw, catch, hierarchy, rethrow, and wrapping facts
- recursion and mutual-recursion edges that saturate to a finite set of facts

Test requirement:

- Add semantic-fact tests from corpus outcomes before expecting Rust shape
  changes.
- Include negative tests for cases where proof is unavailable, such as a field
  that can be nulled or an external call with unknown nullability.

### 4. Defer Rust Shape Until Java Facts Are Stable

Deliverables:

- object strategy inputs for concrete structs, closed enums, `dyn Trait`,
  erased external values, and conservative fallback representations
- slot-shape constraints for owned, borrowed, mutable, optional, dynamic, and
  external-boundary values
- conversion plans that adapt expressions to chosen slots without re-solving
  Java overloads or method meanings

Test requirement:

- Add representation and slot-shape tests that are independent of corpus names.
- When a corpus outcome motivates the test, record that link in
  `docs/gatk-resolver-benchmark-report.md`, not in the assertion itself.

### 5. Benchmark, Report, And Repeat

Each implementation slice must follow this loop:

1. Run or inspect the GATK benchmark outcome.
2. Classify the failure by responsible layer.
3. Reduce the outcome to a general Java or Rust-planning rule.
4. Add minimized tests for that rule, trying not to overfit to GATK.
5. Implement the smallest responsible change.
6. Run focused tests.
7. Re-run the GATK benchmark.
8. Update `docs/gatk-resolver-benchmark-report.md` with the count movement,
   tests added, rule added, responsible layer, and remaining risks.

If a test captures a real Java rule or a useful conservative fallback, keep it
even when the benchmark count does not improve.

## Test Policy

Tests must be added based on the outcome of each selected corpus slice, but the
tests should try not to overfit to the corpus being improved.

This is part of the definition of done for a slice. A benchmark-driven change
without a generalized regression test is not complete, even if the corpus count
improves.

For every implementation slice:

1. Add at least one minimized regression test before or alongside the
   implementation.
2. Prefer synthetic Java snippets with neutral names such as `Base`, `Derived`,
   `Box<T>`, `Builder`, `Reader`, `Column`, `ExternalRecord`, or
   `CollectionLike`.
3. Name tests after the semantic rule, not the corpus file or class.
4. Assert the responsible layer first: parser shape, resolver target, semantic
   fact, external fact, object strategy, slot shape, conversion plan, or
   diagnostic.
5. Add Rust-output assertions only after the owning facts or plans are covered.
6. Include conservative or negative tests when proof can be unavailable.
7. Keep the corpus reference in `docs/gatk-resolver-benchmark-report.md`, not
   in the assertion, unless exact syntax shape is the bug.

Copied real-corpus snippets are acceptable only when the syntax shape itself is
the feature: imports, nested types, anonymous classes, overload shape, exception
shape, class literals, method references, or a named library boundary.

### Outcome-To-Test Workflow

For each selected corpus outcome:

1. Record the original diagnostic, file, and minimal observed Java shape in the
   benchmark report.
2. Reduce the observed shape to the smallest general rule that explains it.
3. Write the primary regression test against that rule using neutral class and
   method names.
4. Add a second test only when needed to protect an important boundary, such as
   unknown proof, user override precedence, or conservative fallback behavior.
5. Keep the corpus example as benchmark evidence, not as the definition of the
   production behavior.

Anti-overfit checks before accepting a test:

- Would this test still make sense if GATK were replaced by another large Java
  corpus using the same Java feature?
- Does the assertion target the layer that owns the decision?
- Is the exact class, method, package, or library name necessary to the rule?
- If the rule is a library fact, is it recorded as an external API fact with
  provenance rather than as a lowering heuristic?
- If proof is incomplete, does the test cover the conservative fallback instead
  of forcing an optimistic Rust shape?

## Responsibility Boundaries

Parser:

- Owns Java syntax shape and parse diagnostics.
- Must not infer Java semantic meaning beyond source structure.

Resolver:

- Owns Java lookup, overload selection, constructors, fields, inheritance,
  generic substitution, static members, nested types, casts, primitive coercion,
  boxing/unboxing, varargs, and assignability.
- Emits structured unresolved diagnostics when facts are insufficient.

External API facts:

- Own un-translated Java library behavior.
- Cover receiver mutability, return type, nullability, fluent APIs, builder
  products, collection shapes, exception summaries, and external boundary
  uncertainty.
- User-provided overrides must take precedence over guessed or built-in facts.

Semantic facts:

- Own nullability, mutation/effects, recursion, exceptions, accessor summaries,
  control-flow markers, field initialization, escape facts, and provenance.
- Facts must remain finite even for recursive and mutually recursive code.

Object strategy:

- Chooses between concrete structs, closed enums, `dyn Trait`, erased external
  values, and conservative fallback representations.
- Consumes hierarchy closure, casts, `instanceof`, constructor, escape, and
  user override facts.

Slot-shape and conversion planning:

- Own Rust storage/API shapes and expression adaptation.
- Co-solves owned/borrowed/reference shapes, mutability, optionality, primitive
  coercion, collection element adapters, string views, dynamic dispatch
  adaptation, and fallback cloning.

Lowering:

- Consumes resolved symbols, facts, object plans, slot plans, and conversion
  plans.
- Must not rediscover Java semantics from method names, field names, or corpus
  examples.

## Phase 1: Re-Baseline GATK

1. Run the resolver benchmark from the current committed state.
2. Update `docs/gatk-resolver-benchmark-report.md` with:
   - files parsed and resolved
   - unresolved method calls
   - unresolved constructors
   - unresolved field accesses
   - external API explanations and guesses
   - top parser, resolver, and semantic diagnostics
   - fact counts
3. Classify the top failures by responsible layer before coding.

## Phase 2: Resolver And External Boundary Stabilization

Use GATK benchmark frequency as a signal, but choose slices by architectural
leverage first.

Prioritize:

1. Source resolution across files.
2. Inherited members and nested source types.
3. Constructor and method overload applicability.
4. Static members and static imports.
5. External API return typing and nullability.
6. External receiver mutability and fluent methods.
7. Structured unresolved diagnostics.

For each slice:

1. Pick one top unresolved method, constructor, field, or diagnostic category.
2. Write a minimized test for the general rule.
3. Implement the resolver or external fact improvement in the owning module.
4. Run focused tests.
5. Re-run the GATK benchmark.
6. Update the benchmark report with before/after counts and the tests added.

## Phase 3: Generic, Target-Type, And Coercion Resolution

Move Java type evidence into resolver/type inference before Rust representation
planning.

Prioritize:

1. Diamond inference with one or more unknowns.
2. Wildcard bounds and erasure.
3. Generic method return substitution.
4. Class literals and `T.class`.
5. Java primitive widening, narrowing diagnostics, boxing, and unboxing.
6. Container terminal types: arrays, lists, sets, maps, iterators, streams, and
   optionals.
7. Method references and lambdas as target-typed expressions.

Tests should assert inferred Java types, selected overloads, or structured
diagnostics before any Rust output assertion.

## Phase 4: Semantic Fact Layer And Finite Co-Solving

When resolver progress starts depending on nullability, mutation, recursion,
exceptions, or object representation, shift to finite semantic facts rather
than adding more name heuristics.

Implement or strengthen facts for:

1. Field initialization and proven non-null fields.
2. Accessor summaries, including trivial getter/setter removal inputs.
3. Receiver and argument mutation.
4. Assignment in conditions such as `while ((x = next()) != null)`.
5. Recursive and mutually recursive call edges.
6. Exception throw, catch, hierarchy, and rethrow summaries.
7. External boundary facts with provenance.

The solver must allow conservative fallback facts: `Option`, owned values,
cloned values, `Box<dyn Trait>`, erased external values, or explicit
diagnostics.

## Phase 5: Object Strategy Inputs

Use solved Java facts and optional user overrides to choose Rust object
representations.

Support:

1. Concrete structs for direct concrete classes.
2. Named enums when a closed subclass or interface implementation set is proven
   or injected.
3. `dyn Trait` or boxed trait objects for open behavior-driven hierarchies.
4. Erased external boundaries for unknown object shapes.
5. Conservative dynamic fallback when casts or runtime type tests prevent a
   closed representation.

Tests should include both closed and open hierarchy variants so enum choices do
not prevent fallback paths.

## Phase 6: Slot-Shape And Conversion Co-Solving

Only after enough Java facts are stable, co-solve Rust-facing shapes.

Plan:

1. Build finite constraints for parameter, return, local, field, and receiver
   slots.
2. Track ownership, borrowing, mutability, optionality, dispatch, and external
   boundary shape separately.
3. Let conversion planning adapt expressions to target slots.
4. Keep Java coercion evidence in resolver/type facts; Rust conversion planning
   consumes that evidence.
5. Prefer idiomatic Rust where proven, with conservative fallback when facts
   are incomplete.

## Immediate Next Steps

1. Continue the GATK resolver benchmark loop from the current top diagnostics.
2. Select one high-leverage failure class by responsibility boundary.
3. Add minimized tests derived from that outcome, using the test policy above.
4. Implement the responsible rule in the owning module.
5. Run focused tests and the benchmark.
6. Update `docs/gatk-resolver-benchmark-report.md`.
7. Repeat until the top failures shift from resolver/external API gaps to
   semantic facts or representation planning.

## Acceptance Criteria

- Every implementation slice has at least one generalized test derived from a
  corpus outcome.
- Tests avoid overfitting to GATK unless the exact syntax or external API name
  is the rule.
- Benchmark reports show before/after counts and the generalized rule added.
- Resolver, external API facts, semantic facts, object strategy, slot-shape,
  conversion planning, and lowering responsibilities remain separate.
- No new Java semantic heuristics are added to lowering.
- Recursive and mutually dependent facts converge within a finite budget.
- User overrides can refine or replace conservative inferred facts.

## Related Documents

- `docs/resolver-external-api-gatk-plan.md`
- `docs/gatk-resolver-benchmark-report.md`
- `docs/semantic-fact-layer-next-plan.md`
- `docs/object-model-strategy-algorithm-plan.md`
- `docs/rust-slot-shape-and-object-representation.md`
- `docs/new-feature-responsibility-checklist.md`
