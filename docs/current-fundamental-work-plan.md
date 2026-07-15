# Current Fundamental Work Plan

## Purpose

Use GATK as the pressure corpus to improve the translator's fundamental
semantic structure without hard-coding GATK.

The next work should prioritize decisions that later stages depend on:
resolution, external API facts, semantic facts, object strategy inputs,
slot-shape constraints, and conversion planning. Lowering should only consume
facts and plans that earlier stages have already made explicit.

## Plan Summary

Use GATK as the main pressure corpus for now, but treat it as evidence rather
than as the specification. The next work should be a sequence of small
benchmark-guided design slices:

1. Re-run the current benchmark and identify the leading unresolved or
   over-conservative outcome.
2. Classify that outcome by the layer that owns it: parser, resolver, external
   facts, semantic facts, object strategy, slot shape, conversion planning, or
   lowering.
3. Reduce the corpus outcome to a general rule that would still make sense for
   another Java corpus.
4. Add tests based on the outcome before or alongside implementation. These
   tests must try not to overfit to the corpus being used as the benchmark.
5. Implement the smallest responsible rule in the owning layer.
6. Re-run focused tests, broader tests when shared behavior changed, and the
   benchmark.
7. Record the motivating corpus evidence, generalized rule, tests, owning
   layer, before/after counts, and remaining risks in
   `docs/gatk-resolver-benchmark-report.md`.

The next slices should favor design-critical fundamentals before more Rust
output polishing: inherited lookup, generic substitution, overload
applicability, coercion evidence, external API boundaries, finite semantic
facts, object strategy inputs, and slot-shape/conversion co-solving.

Tests are required output of every selected slice. They should usually be small
synthetic fixtures with neutral names and assertions at the responsible layer.
Use exact corpus snippets only when the exact syntax shape or named external
API is the behavior under test. Keep negative and conservative tests when they
document fallback behavior or missing proof.

## Plan Of Record

The next phase should be benchmark-guided foundation work, with GATK as the
primary pressure corpus. The purpose of using GATK is to expose realistic Java
semantic pressure, not to make the translator contain GATK-specific rules.

Work should proceed in this order:

1. Re-baseline the GATK resolver benchmark and classify the top remaining
   unresolved calls, constructors, fields, diagnostics, and external API
   guesses by owning layer.
2. Select one failure family by design leverage, not just frequency. Prefer
   failures that would otherwise force later stages to rediscover Java meaning:
   inherited lookup, overload applicability, generics, external API behavior,
   nullability, mutability, dispatch, exceptions, and container shape.
3. Reduce the corpus outcome to a general translator rule and add tests for
   that rule before or alongside implementation. These tests must be based on
   the observed outcome, but should try not to overfit to the corpus we are
   trying to get to work.
4. Implement the smallest responsible rule in the owning layer: parser,
   resolver, external facts, semantic facts, object strategy, slot-shape
   planning, conversion planning, or lowering only when all earlier decisions
   are already explicit.
5. Re-run focused tests, broader tests when shared behavior changed, and the
   GATK benchmark.
6. Update `docs/gatk-resolver-benchmark-report.md` with the motivating corpus
   outcome, generalized rule, responsible layer, tests added, before/after
   counts, and remaining risks.

The main benchmark utility target is still GATK because it stresses nested
types, generic inheritance, external libraries, fluent APIs, collections,
exceptions, and real-world resolver ambiguity. Other corpora should be used as
cross-checks once a rule looks GATK-shaped, so we can catch overfitting early.

## Operating Loop

Each implementation slice follows the same loop:

1. Re-baseline the corpus.
2. Select one failure class by architectural leverage.
3. Reduce the corpus outcome to a general Java or Rust-planning rule.
4. Add outcome-driven tests that do not overfit to the corpus.
5. Implement the smallest rule in the owning layer.
6. Run focused tests, broader tests when needed, and the corpus benchmark.
7. Record the result in `docs/gatk-resolver-benchmark-report.md`.

The benchmark may show no movement. If the generalized rule is correct and
tested, keep the test and record the no-movement result.

## Per-Slice Required Outputs

Every slice must leave behind more than a lower benchmark count. The required
outputs are:

1. A short classification of the selected corpus outcome and its owning layer.
2. A generalized rule stated without depending on one corpus class, method, or
   package name.
3. Tests derived from the outcome.
4. An implementation in the owning layer only.
5. Benchmark evidence showing whether the rule changed the corpus result.
6. A report entry linking the corpus evidence to the generalized rule.

If the selected outcome cannot be generalized yet, the slice should improve
diagnostics or add an explicit conservative fallback test instead of adding a
speculative production rule.

## Priority Order

### 1. Resolver Completeness

Work first on Java semantics that later stages should not rediscover:

- inherited source and external methods
- inherited source and external fields
- constructor and method overload applicability
- structured overload rejection reasons
- generic substitution through superclass and interface chains
- wildcard bounds, erasure, target typing, and diamond inference
- primitive widening, boxing, unboxing, and varargs
- static imports, static members, nested types, enum constants, and class
  literals
- casts and `instanceof`

Resolver output should be resolved symbols, inferred Java types, coercion
evidence, or structured diagnostics.

### 2. External API Facts

Model libraries outside the translated corpus as facts, not lowering
heuristics:

- return type and container element type
- return nullability
- receiver mutation
- parameter mutation
- getter, setter, predicate, iterator, builder, and fluent roles
- thrown exception summaries
- conservative external boundary facts
- provenance: user override, known model, source declaration, or guess

User-provided overrides have highest precedence. Guesses must remain visible so
they can be audited or replaced.

### 3. Semantic Facts

Strengthen finite semantic facts before Rust representation choices:

- field initialization and non-null proofs
- method receiver and argument effects
- trivial getter and setter summaries
- assignment-in-condition markers
- exception hierarchy, throw, catch, rethrow, and wrapping facts
- direct and mutual recursion edges

Recursive Java code should produce a finite set of call/effect facts. It should
not trigger recursive specialization or unbounded inference.

### 4. Object Strategy Inputs

Use resolver and semantic facts to decide which object model is available:

- concrete structs for ordinary classes
- named enums for proven closed subclass or implementation families
- `dyn Trait` fallback when closure or dispatch cannot be proven
- erased external object boundaries when the library is not translated
- explicit fallback when casts, reflection, external construction, or unknown
  subclasses make a closed model unsafe

Injected facts may force or override hierarchy closure, enum eligibility, or
fallback strategy, but the provenance must be visible.

### 5. Slot Shape And Conversion Co-Solving

Choose Rust-facing shapes from constraints, not from the first example:

- owned values
- shared borrows
- mutable borrows
- `str`/`String`-like string views
- `Option<T>` only when nullability requires it or proof is missing
- primitive coercions and numeric conversions
- collection element adapters
- dynamic dispatch adapters
- conservative clones at explicit boundaries

Slot-shape and conversion planning should consume resolver and semantic facts.
Lowering should not contain Java semantic special cases for ownership,
nullability, coercion, or API roles.

## Test Policy

Every selected corpus outcome must produce tests before or alongside the
implementation.

Tests should be derived from the outcome, but they should try not to overfit to
GATK:

- Name tests after the general rule, not a GATK class, method, or package.
- Prefer minimized synthetic Java with neutral names such as `Base`, `Derived`,
  `Holder`, `Box<T>`, `Reader`, `Builder`, `Source`, `Sink`, `Column`,
  `CollectionLike`, or `ExternalRecord`.
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
  `docs/gatk-resolver-benchmark-report.md`, not in the assertion.

Copying a real GATK snippet is acceptable only when the exact syntax shape is
the feature: imports, static imports, nested types, anonymous classes, overload
shape, exception syntax, class literals, method references, casts, or
library-boundary usage.

## Anti-Overfit Test Gate

Before accepting a test derived from a GATK outcome, check:

1. Would this test still be meaningful against another large Java corpus?
2. Does the test name describe the Java semantic rule or Rust-planning rule,
   rather than a GATK type or package?
3. Is the assertion at the responsible layer, rather than at a later layer that
   would hide the missing fact or plan?
4. If the test uses a real API name, is that API identity itself the external
   fact being modeled?
5. Is there a negative or conservative case when the rule depends on unavailable
   proof?

If the answer to any question is no, reduce the fixture further or move the
corpus-specific detail into `docs/gatk-resolver-benchmark-report.md`.

## Current Slice Selection Rule

Pick the next slice by this ordering:

1. It prevents later stages from rediscovering Java semantics.
2. It improves object strategy, slot shape, conversion planning, nullability,
   mutability, dispatch, or exception decisions.
3. It affects multiple corpus failures or a high-frequency failure family.
4. It has a clean owning layer and can be tested there.

Frequency is a tie-breaker. It is not the definition of the rule.

## Immediate Goal: Benchmark-Guided Foundation Slice

The next concrete goal is to run one complete design slice against GATK:

1. Re-baseline the GATK resolver benchmark and identify the highest-leverage
   remaining failure family.
2. Classify the failure by responsible layer before coding.
3. Reduce the corpus outcome to one or more general Java or Rust-planning
   rules.
4. Add regression tests from the outcome before or alongside implementation.
5. Implement only the smallest rule in the owning layer.
6. Re-run focused tests, broader tests when shared behavior changed, and the
   GATK benchmark.
7. Update `docs/gatk-resolver-benchmark-report.md` with the motivating corpus
   evidence, generalized rule, tests added, responsible layer, before/after
   counts, and remaining risks.

This is the main track until a stronger design blocker appears. The goal is
not just lower unresolved counts. The goal is to improve the semantic contract
between stages so later representation, slot-shape, conversion, and lowering
work does not need corpus-specific Java heuristics.

## Near-Term Plan

The next design work should stay focused on fundamentals that affect later
translation choices. The order is:

1. Re-baseline the GATK resolver benchmark and classify the leading remaining
   outcomes by owning layer.
2. Prefer source/member resolution, inherited lookup, generic substitution,
   overload/coercion evidence, and external API boundary facts over late Rust
   output polishing.
3. For each selected outcome, reduce the corpus evidence to a general rule:
   Java syntax, Java resolution, external API fact, semantic fact, object
   strategy, slot-shape rule, conversion rule, or explicit conservative
   fallback.
4. Add tests based on the outcome before or alongside implementation. These
   tests must try not to overfit to GATK or any other pressure corpus.
5. Implement the smallest responsible rule in the owning module only.
6. Re-run focused tests, broader tests when shared behavior changed, and the
   GATK benchmark.
7. Update `docs/gatk-resolver-benchmark-report.md` with the corpus evidence,
   generalized rule, tests added, responsible layer, benchmark movement, and
   remaining risks.

For the next slice, the preferred target is a high-leverage resolver or
external-boundary outcome, not another narrow lowering rule. Good candidates
are inherited source/external members, generic substitution through inheritance,
constructor or method overload applicability, primitive/boxing/varargs
coercion evidence, static imports and enum constants, or repeated external API
guesses that can be promoted into auditable facts.

Tests are mandatory output for the slice. They should be based on the corpus
outcome, but they should try not to overfit to the corpus we are trying to get
to work:

- Prefer minimized synthetic Java that captures the rule independently of GATK
  package names, class names, or local idioms.
- Use the real corpus snippet only when the exact syntax or external API name
  is the feature being tested.
- Assert the owning layer first. For example, a resolver failure should add a
  resolver test for symbol/type facts before any Rust-output snapshot.
- Add conservative or negative tests when the correct general behavior is to
  keep an unresolved diagnostic, `dyn Trait` fallback, erased external object,
  `Option<T>`, clone, or conversion boundary.

## Concrete Execution Plan

Use this plan for the next implementation phase.

### 1. Establish The Current Corpus Signal

Run the GATK resolver benchmark from the current tree and record the baseline
in `docs/gatk-resolver-benchmark-report.md`.

Capture:

- unresolved methods, constructors, and fields;
- resolver, parser, and semantic diagnostics;
- external API guesses by category;
- top recurring unresolved rows;
- whether any new diagnostics were introduced by recent resolver changes.

Do not select work from raw frequency alone. Pick the next failure family by
how much later design depends on it: source lookup, inherited lookup, generic
substitution, external API boundary facts, nullability, mutability, object
strategy, slot shape, or conversion planning.

### 2. Classify Before Coding

For the selected outcome, decide which layer owns it:

- parser: Java syntax was not represented correctly;
- resolver: Java names, overloads, inheritance, generics, static members, or
  source symbols were not resolved correctly;
- external facts: an un-translated library boundary needs a modeled fact,
  conservative guess, or user override point;
- semantic facts: the program needs finite nullability, mutation, exception,
  recursion, accessor, or assignment-condition facts;
- object strategy: the evidence decides between concrete struct, named enum,
  `dyn Trait`, erased external object, or conservative fallback;
- slot shape and conversion: Rust-facing ownership, borrow, `Option`,
  primitive coercion, collection adapter, or clone boundary must be planned;
- lowering: only when all Java facts and Rust-planning decisions already
  exist and the remaining work is structural emission.

If ownership is unclear, add a diagnostic or fact first. Do not add a lowering
heuristic to make one corpus row disappear.

### 3. Add Outcome-Driven Tests

Every selected corpus outcome must produce tests before or alongside the
implementation. The tests should be based on the outcome, but must try not to
overfit to the corpus we are trying to get to work.

Required tests for each slice:

- one minimized positive test for the owning layer;
- one negative or conservative test when the rule depends on proof that can be
  missing;
- one provenance test when an external fact, known model, user override, or
  conservative guess is involved;
- a benchmark report entry connecting the corpus evidence to the generalized
  rule and tests.

Test-shaping rules:

- Name the test after the general rule, not after a GATK class, method, file,
  or package.
- Prefer neutral fixture names like `Base`, `Derived`, `Outer`, `Params`,
  `Holder`, `Box<T>`, `Builder`, `Reader`, `Source`, `Sink`, `Record`,
  `Item`, or `ExternalApi`.
- Assert the responsible layer first: parse tree shape, resolved symbol,
  inferred Java type, overload target, coercion evidence, semantic fact,
  external fact provenance, object strategy, slot shape, conversion plan, or
  structured diagnostic.
- Add Rust-output assertions only after the responsible facts or plans have
  direct tests.
- Keep tests that document conservative behavior even when the corpus metric
  does not improve.

Copy real corpus code only when the exact syntax or library boundary is the
feature under test, such as static imports, nested imports, anonymous classes,
method references, class literals, exception syntax, casts, overload shape, or
a named external API.

### 4. Implement The Smallest Responsible Rule

Keep changes in the owning layer:

- resolver changes should produce resolved Java symbols, types, applicability
  evidence, or diagnostics;
- external API changes should live in the fact/model path and carry
  provenance;
- semantic-analysis changes should produce finite facts with convergence
  behavior;
- object and slot-shape changes should consume facts rather than inspect Java
  names directly;
- lowering changes should consume resolved facts and plans only.

When a corpus row suggests a tempting specific rule, reduce it to the
underlying mechanism first. For example, a single field or method name should
not become a lowering special case if the actual missing capability is nested
source type import resolution, inherited generic lookup, or external getter
mutability.

### 5. Verify And Record

Run the focused tests for the slice, then broader tests when shared behavior
changed. Re-run the GATK benchmark and update
`docs/gatk-resolver-benchmark-report.md`.

The report entry should include:

- baseline and after counts;
- motivating corpus files or diagnostics;
- the generalized rule;
- tests added or strengthened;
- owning layer;
- remaining risks and any conservative fallbacks.

A slice is acceptable even if the benchmark count does not move, provided the
rule is general, tested at the owning layer, and improves the contract that
later object strategy, slot-shape, conversion, or lowering work will consume.
Keep tests even if the benchmark does not move, provided the test captures a
real Java rule, a deliberate fallback, or an important non-overfitting guard.

## Verification And Reporting

For each slice:

1. Run the focused test target for the changed layer.
2. Run full tests when shared resolver, fact, signature, or lowering behavior
   changes.
3. Re-run the GATK resolver benchmark.
4. Update `docs/gatk-resolver-benchmark-report.md` with:
   - motivating corpus outcome
   - generalized rule
   - responsible layer
   - tests added
   - before and after counts
   - remaining risks or conservative fallbacks

Do not report a benchmark run taken while tests are failing as a valid
before/after result.

## Phase Roadmap

This plan turns the priority order above into the next concrete work sequence.
It uses GATK to find pressure points, but every implementation slice must be
expressed as a reusable Java semantic rule, Rust-planning rule, external API
fact, or conservative diagnostic.

### Phase 1: Stabilize Resolver And External API Boundaries

Goal: reduce unresolved call, constructor, and field failures that would force
later stages to rediscover Java semantics.

Work items:

1. Re-run the GATK resolver benchmark and classify the top remaining
   diagnostics by owning layer.
2. Pick the highest-leverage class among:
   - inherited generic source methods
   - inherited generic source fields
   - external method return types
   - external constructor overloads
   - wildcard or raw-type substitution
   - primitive, boxing, or varargs applicability
3. Add one or more minimized tests for the generalized rule before or alongside
   implementation.
4. Implement the rule only in its owning layer:
   - parser only for syntax shape
   - resolver only for symbols, overloads, generic substitution, and Java type
     facts
   - `stdlib`/external facts only for library boundary behavior
   - semantic facts only for effects, nullability, recursion, and exceptions
5. Re-run focused tests, full tests when shared behavior changed, and the GATK
   benchmark.
6. Record before/after counts and the rule boundary in
   `docs/gatk-resolver-benchmark-report.md`.

Tests to add from outcomes:

- Prefer tests named after general behavior such as
  `resolver_uses_wildcard_bound_for_receiver_return`,
  `resolver_applies_external_varargs_subtype_fact`, or
  `resolver_reports_structured_external_constructor_ambiguity`.
- Do not assert a GATK class name unless the class name is itself part of a
  library API model.
- Include a negative or conservative test when the rule depends on unavailable
  proof, such as open-world hierarchy, erased generic arguments, or unknown
  external overloads.

Acceptance:

- New tests pass and fail for the intended reason before implementation when
  practical.
- Benchmark movement is recorded, including no-movement outcomes.
- No new lowering branch is added to compensate for missing resolver facts.

### Phase 2: Make Semantic Facts The Shared Contract

Goal: ensure nullability, mutability, exception, recursion, and accessor
knowledge exists before object representation, slot-shape solving, conversion,
or lowering.

Work items:

1. Keep `semantic_analysis::analyze_program` as the orchestration point for
   `FactSet`, `UsageFacts`, external facts, and convergence diagnostics.
2. Add or strengthen fact producers for:
   - constructor-proven field non-nullness
   - trivial getters and setters
   - receiver and argument mutation effects
   - assignment in conditions
   - exception hierarchy and thrown/caught facts
   - direct and mutual recursion edges
3. Ensure every producer has a finite key domain and cannot create unbounded
   expression-specific facts.
4. Feed facts into representation and signature planning; lowering should only
   consume plans and facts.

Tests to add from outcomes:

- Add semantic-fact tests for each corpus-derived pattern using neutral
  fixtures.
- Assert facts directly before adding generated-Rust assertions.
- Add convergence tests for direct and mutual recursion so recursive Java code
  produces a finite fact set.
- Add conservative tests proving fields remain `Option<T>` or methods remain
  fallback-shaped when non-null or non-mutating behavior cannot be proven.

Acceptance:

- Field, accessor, effect, exception, and recursion facts are inspectable in
  tests.
- Recursive inputs converge within the configured budget.
- Representation and signature planning consume facts instead of repeating
  source-body scans for the same decisions.

### Phase 3: Object Strategy Inputs Before Object Rewrites

Goal: decide which Java class and interface families can become Rust structs,
named enums, traits, trait objects, or erased external boundaries.

Work items:

1. Collect hierarchy facts for classes, abstract classes, interfaces, anonymous
   classes, and external implementations.
2. Distinguish open-world and closed-world hierarchy evidence.
3. Support user-injected facts for closure, enum eligibility, external
   implementations, and forced fallbacks.
4. Produce explicit object-strategy diagnostics when casts, reflection,
   external construction, or unknown implementors prevent an idiomatic mapping.
5. Keep named-enum conversion as an optimization chosen by object strategy, not
   as a resolver or lowering special case.

Tests to add from outcomes:

- Add small hierarchy fixtures for closed subclass families, open interface
  families, casts, `instanceof`, anonymous implementations, and user overrides.
- Assert the object strategy, not final Rust syntax, unless signature and
  lowering plans are already covered.
- Include fallback tests where the correct result is `dyn Trait`, erased
  external object, or structured diagnostic.

Acceptance:

- The planner can explain why a family is a struct, enum, trait object, or
  fallback.
- Overrides have provenance and conflicts are diagnosed.
- Lowering does not decide hierarchy closure from local syntax.

### Phase 4: Slot Shape And Conversion Co-Solving

Goal: avoid one-off ownership, lifetime, nullability, numeric coercion, and
container conversion rules by solving Rust-facing slot shapes from shared
constraints.

Work items:

1. Collect constraints from declarations, body use, call sites, returns, fields,
   external boundaries, object strategy, and semantic facts.
2. Solve related choices together where they interact:
   - owned versus borrowed values
   - `Option<T>` versus non-null `T`
   - `String`, `str`, Java object strings, and borrowed views
   - primitive widening, boxing, unboxing, and explicit casts
   - collection and iterator element adapters
   - trait object, enum, and erased-object adapters
3. Emit conversion plans at boundaries. Lowering applies those plans rather
   than rediscovering ownership, nullability, or coercion.
4. Add structured fallback diagnostics when constraints are underdetermined or
   conflicting.

Tests to add from outcomes:

- Add constraint-level tests that avoid asserting incidental Rust formatting.
- Include examples where the same Java operation maps differently because the
  destination slot differs.
- Add broad numeric coercion, string/view, collection, option, and dispatch
  cases as solver tests before final-output snapshots.
- Keep corpus-derived examples minimized and generalized unless the exact
  library boundary is the relevant behavior.

Acceptance:

- A mapping such as stream `findFirst()` is represented once as a Java terminal
  shape plus conversion obligations, not repeated for every ownership case.
- Numeric coercions and Java type inference are represented as resolver/type
  facts before Rust conversion planning.
- Final Rust output follows from solved slot and conversion plans.

### Phase 5: Corpus-Guided Regression Growth

Goal: grow tests from real outcomes without turning GATK into the
specification.

Work items:

1. For each benchmark run, select diagnostics by architectural leverage before
   frequency.
2. Reduce each chosen diagnostic to a small general fixture.
3. Add tests at the responsible layer:
   - parser tests for syntax gaps
   - resolver tests for target and type facts
   - external-fact tests for library models
   - semantic-fact tests for effects and proofs
   - representation/signature tests for object and slot plans
   - lowering/output tests only after upstream plans are covered
4. Record the real corpus source and benchmark movement in the report, not in
   the test's core assertion.
5. Keep failing or conservative tests when they document a future rule or an
   intentional fallback.

Acceptance:

- Each new corpus-derived implementation has at least one generalized test.
- The suite contains negative/fallback cases as well as successful mappings.
- The benchmark report remains the link to GATK; the tests remain reusable for
  other Java corpuses.
