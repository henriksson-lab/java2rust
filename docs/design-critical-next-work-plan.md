# Design-Critical Next Work Plan

## Tracking Status

This is the plan-of-record for the next big design work after the current safe
point.

Use it to choose implementation slices, decide which module owns each change,
and judge whether tests were added in the right form. Supporting details may
live in other planning documents, but this file should stay the short,
trackable plan for the next design phase.

## Purpose

Use the current GATK resolver benchmark as a pressure corpus to improve the
translator's foundation without turning GATK into the specification.

The next work should focus on decisions that later stages depend on:

- Java semantic resolution
- external API facts
- semantic facts
- object strategy inputs
- slot-shape and conversion constraints
- conservative fallback diagnostics

Broad Rust-output polishing should wait until these earlier layers expose the
needed facts and plans.

## Guiding Rule

Do not fix benchmark outcomes by adding late lowering special cases.

Each selected corpus outcome must become one of:

- a parser syntax rule
- a resolver rule or structured resolver diagnostic
- an external API fact with provenance
- a finite semantic fact
- an object-strategy rule
- a slot-shape or conversion-planning constraint
- an explicit conservative fallback

Lowering should consume these facts and plans. It should not infer Java meaning
from class names, method names, package names, or one corpus-specific pattern.

## Work Loop

For each implementation slice:

1. Re-run or inspect the GATK resolver benchmark.
2. Classify the leading outcome by owning layer.
3. Reduce the outcome to a general Java semantic or Rust-planning rule.
4. Add tests based on that outcome before or alongside implementation.
5. Implement the smallest responsible change in the owning layer.
6. Run focused tests, broader tests when shared behavior changed, and the GATK
   benchmark.
7. Update `docs/gatk-resolver-benchmark-report.md` with the corpus evidence,
   generalized rule, responsible layer, tests added, before/after counts, and
   remaining risks.

Frequency is only a tie-breaker. Prefer work that prevents later stages from
rediscovering Java meaning.

## Transformation Rule Authoring

Each generalized transformation rule must start from the Rust result we want,
not from the Java syntax symptom. Follow
`docs/transformation-rule-contract.md`: add an **Ideal Rust Target** section
before the **Implementation Rule** section. The ideal target names the Rust
type/API/control-flow shape and the relevant ownership, borrowing,
nullability, mutability, dispatch, conversion, error, collection, lifetime, and
external-boundary choices. The implementation rule then explains which facts
and plans select that target and which fallback is used when proof is missing.

## Milestones

Track the next phase through these milestones. Each milestone may take several
small implementation slices, but every slice must follow the work loop above.

### Milestone 1: Rebaseline And Classify

Make the benchmark signal actionable before choosing fixes.

Deliverables:

- a fresh GATK resolver benchmark entry;
- top unresolved methods, constructors, fields, constructor delegations, and
  external guesses grouped by owning layer;
- representative corpus locations for the selected outcome;
- a written decision about whether the selected outcome belongs to parser,
  resolver, external facts, semantic facts, object strategy, slot-shape
  planning, conversion planning, or an explicit conservative fallback.

Exit criteria:

- the next implementation target has one clear owner;
- the intended generalized rule is stated without depending on a GATK class,
  method, or package name unless the named external API is itself the fact.

### Milestone 2: Resolver Foundation Slice

Prefer source-language meaning before downstream Rust-shape work.

Candidate outcomes:

- inherited source or external member lookup;
- constructor and method overload applicability;
- static imports, static members, enum constants, and nested names;
- generic substitution through superclass, interface, wildcard, raw, erased,
  and target-typed contexts;
- primitive widening, boxing, unboxing, varargs, casts, and class literals.

Deliverables:

- focused resolver tests for the generalized rule;
- negative or conservative tests for ambiguity or missing proof;
- implementation in resolver/type logic only;
- benchmark report entry with before/after counts.

Exit criteria:

- lowering does not gain new Java name heuristics;
- slot-shape or conversion logic receives resolved types, symbols, or
  structured diagnostics instead of guessing.

### Milestone 3: External Boundary Slice

Promote repeated external guesses into auditable facts when they are real
library-boundary behavior.

Candidate outcomes:

- known JDK or common library constructors, methods, constants, and builders;
- getter, setter, predicate, iterator, collection, factory, or fluent roles;
- external return nullability and container element shapes;
- receiver and argument mutation;
- declared or inferred exception behavior;
- erased external object and aliasing boundaries.

Deliverables:

- external-fact tests with provenance expectations;
- user-override precedence tests when a known model or guess is involved;
- conservative fallback tests for unknown libraries;
- benchmark report entry showing which guesses became known facts and which
  remain intentionally conservative.

Exit criteria:

- external behavior is represented as facts with visible provenance;
- no external API behavior is encoded in lowering.

### Milestone 4: Semantic Fact Slice

Move Java behavior that affects Rust representation into finite facts.

Candidate outcomes:

- field initialization and non-null proofs;
- trivial getter and setter summaries;
- method receiver and argument effects;
- assignment-in-condition facts;
- exception hierarchy, throw, catch, rethrow, and wrapping facts;
- direct and mutual recursion edges.

Deliverables:

- semantic-fact tests using small Java examples;
- convergence or bounded-fallback tests for recursive examples;
- report entry connecting the corpus outcome to the generalized fact.

Exit criteria:

- representation, signatures, and lowering consume semantic facts instead of
  recomputing Java behavior from statement shapes or names.

### Milestone 5: Object Strategy, Slot Shape, And Conversion Slice

Only after Java meaning is explicit, improve Rust-facing shape decisions.

Candidate outcomes:

- concrete struct versus named enum versus `dyn Trait` versus erased external
  object;
- override facts for closed hierarchies or forced fallbacks;
- owned, borrowed, mutable, optional, and clone boundaries;
- primitive numeric conversions and explicit casts;
- collection, iterator, trait-object, enum, and erased-object adapters.

Deliverables:

- object-strategy, slot-shape, or conversion-plan tests before Rust-output
  snapshots;
- conservative tests when closure, mutability, lifetime, or nullability proof is
  unavailable;
- benchmark report entry describing which later-stage special case was avoided.

Exit criteria:

- Rust output improves because earlier plans are better, not because lowering
  learned corpus-specific Java behavior.

## Outcome-Driven Test Policy

Every selected corpus outcome should create tests.

The tests should be based on the observed outcome, but should try not to
overfit to the corpus we are trying to get to work.

Required rules:

- Name tests after the general rule, not a GATK class, method, or package.
- Prefer minimized synthetic Java with neutral names such as `Base`, `Derived`,
  `Holder`, `Box<T>`, `Reader`, `Builder`, `Source`, `Sink`, `Column`,
  `CollectionLike`, `ExternalRecord`, `Item`, or `Evidence`.
- Assert the responsible layer first: parser shape, resolved symbol, inferred
  Java type, coercion evidence, external fact, semantic fact, object strategy,
  slot shape, conversion plan, or structured diagnostic.
- Add Rust-output assertions only after the owning facts or plans have direct
  tests.
- Add negative or conservative tests when a rule depends on proof that may be
  unavailable.
- Keep tests even when the benchmark count does not move, if they cover a real
  Java rule, an intentional fallback, or user-override precedence.
- Record the motivating corpus file or diagnostic in
  `docs/gatk-resolver-benchmark-report.md`, not in the test assertion.

Copying real corpus snippets is acceptable only when the exact syntax shape is
the feature: imports, static imports, nested types, anonymous classes, overload
shape, exception syntax, class literals, method references, casts, or a named
external API boundary.

## Priority 1: Finish Resolver And External Boundary Quality

The resolver and external API boundary remain the highest-leverage area because
object strategy, slot shape, conversion planning, and lowering all depend on
resolved Java meaning.

Work first on:

- unresolved source constructors and constructor overload applicability
- unresolved source and inherited fields
- external constructors and field accesses that need structured conservative
  facts
- generic substitution through nested, inherited, wildcard, raw, and erased
  types
- primitive widening, boxing, unboxing, varargs, and target typing
- static imports, static members, nested types, enum constants, and class
  literals
- structured diagnostics that explain why a target remains unresolved

Expected outputs:

- resolver tests for target selection and inferred Java types
- external fact tests for un-translated library boundaries
- benchmark report entries showing whether counts moved
- no new lowering branches that compensate for missing resolver facts

## Priority 2: Promote Repeated External Guesses Into Facts

Repeated external guesses should become auditable facts or explicit unknowns.

Model:

- return type and element type
- return nullability
- receiver and parameter mutation
- getter, setter, predicate, iterator, builder, factory, and fluent roles
- thrown exception summaries
- external object and aliasing boundaries
- provenance: user override, known model, source declaration, corpus-derived
  fact, or conservative guess

User-provided overrides must win over known models and guesses. Guesses must
remain visible so they can be audited or replaced.

## Priority 3: Strengthen Semantic Facts Before Rust Shape Choices

Once resolver progress depends on representation choices, move the evidence into
semantic facts instead of lowering.

Add finite facts for:

- field initialization and non-null proofs
- trivial getter and setter summaries
- receiver and argument mutation effects
- assignment in conditions, such as `while ((x = next()) != null)`
- exception hierarchy, throw, catch, rethrow, and wrapping behavior
- direct and mutual recursion edges
- convergence or fallback diagnostics

Recursive code should produce a finite set of facts. It must not trigger
recursive specialization or unbounded expression-keyed inference.

## Priority 4: Object Strategy Inputs

Use resolver and semantic facts to decide the Rust object representation:

- concrete structs for ordinary classes
- named enums for proven closed subclass or interface families
- `dyn Trait` fallback when closure cannot be proven
- erased external object boundaries for un-translated libraries
- conservative fallback when casts, reflection, unknown implementors, or
  external construction make a closed model unsafe

Injected facts may force or override hierarchy closure, enum eligibility, or
fallback strategy. The selected strategy must retain provenance.

## Priority 5: Slot Shape And Conversion Co-Solving

Choose Rust-facing API and storage shapes from constraints, not from the first
example that happened to compile.

The solver should consider:

- owned values
- shared borrows
- mutable borrows
- string and string-view shapes
- `Option<T>` when nullability requires it or proof is missing
- primitive coercions and explicit casts
- collection and iterator element adapters
- trait object, enum, and erased-object adapters
- conservative clones and conversions at explicit boundaries

Slot-shape and conversion planning should consume resolver and semantic facts.
It should not rediscover Java coercion, ownership, nullability, or API roles in
lowering.

## Immediate Next Slice

Use the latest GATK resolver benchmark to pick the next high-leverage unresolved
family. The current preference order is:

1. Improve benchmark diagnostics if the remaining failure is too opaque to
   classify by owner.
2. Fix one source resolver gap that affects fields, constructors, inherited
   lookup, generics, or array/container typing.
3. Promote one repeated external boundary guess into a known fact or structured
   conservative diagnostic.
4. Add the reduced non-overfit tests first, then implement the owning rule.
5. Re-run the focused tests and benchmark, then update the benchmark report.

Acceptance for the next slice:

- the responsible layer is explicit
- at least one minimized non-overfit regression test is added
- benchmark evidence is recorded even if the count does not move
- no GATK-specific rule is introduced into lowering

## Concrete Plan For The Next Design Cycle

The next cycle should focus on fundamental structure before broad Rust-output
polishing. Work should be selected by how much later design depends on it, with
GATK used as the benchmark that exposes pressure.

### 1. Establish The Current Signal

Run or inspect the latest GATK resolver benchmark and identify the strongest
remaining failure families.

Record in `docs/gatk-resolver-benchmark-report.md`:

- unresolved field, method, constructor, and constructor-delegation counts
- explained versus unexplained external API outcomes
- external guesses and top guessed targets
- parser and resolver diagnostics
- semantic diagnostics and fact counts
- representative corpus files for the selected outcome

Do not start coding until the selected outcome has an assigned owner.

### 2. Pick By Design Leverage

Select the next work item using this order:

1. Resolver gaps that block Java meaning: inherited lookup, overloads,
   constructors, generic substitution, wildcard/erasure behavior, static
   members, casts, class literals, primitive coercion, boxing, unboxing, and
   varargs.
2. External API facts that describe un-translated libraries: return type,
   element type, nullability, receiver mutation, argument mutation, fluent or
   builder behavior, collection shape, exceptions, and boundary uncertainty.
3. Semantic facts needed by Rust representation: field non-null proofs,
   accessor summaries, method effects, assignment-in-condition markers,
   exception hierarchy, and finite recursion facts.
4. Object strategy facts: concrete struct, named enum for proven closed
   families, `dyn Trait` fallback, erased external object, and override or
   fallback provenance.
5. Slot-shape and conversion constraints: ownership, borrowing, mutability,
   optionality, string views, numeric conversions, collection adapters, dynamic
   dispatch adapters, and conservative clones.

Frequency in GATK is only a tie-breaker. A lower-frequency issue can be first
if fixing it prevents lowering, slot-shape solving, or object strategy from
rediscovering Java semantics.

### 3. Reduce The Outcome To A General Rule

Before implementation, write the rule in corpus-independent terms.

Examples:

- "A source class implementing `Iterable<T>` can bind a generic method
  parameter declared as `Iterable<T>` through the implemented supertype."
- "A field initialized by all constructors and never assigned `null` should
  produce a non-null field fact."
- "An unknown external setter-like method should produce an auditable receiver
  mutation guess, not a lowering rewrite."
- "A closed hierarchy may be represented as a named enum only when all
  implementors are known or supplied by an override fact."

Avoid rules whose only explanation is one GATK class, package, or local method
name. If the real rule is a named library boundary, place it in external API
facts with provenance.

### 4. Add Outcome-Driven Tests Without Overfitting

Tests are mandatory output for each selected outcome.

The test should be based on the benchmark outcome, but it should try not to
overfit to the corpus we are trying to get to work.

Required tests for each slice:

- at least one minimized regression test for the generalized rule;
- a test name that describes the rule, not the GATK file, class, method, or
  package;
- neutral synthetic Java names when corpus names are incidental;
- an assertion at the owning layer before final Rust output;
- a negative or conservative test when missing proof changes behavior;
- a report entry linking the corpus motivation to the generalized test.

Good assertion targets:

- parser node shape
- resolved symbol or inferred Java type
- coercion or assignability evidence
- external API fact with provenance
- semantic fact
- object strategy decision
- slot-shape or conversion plan
- structured diagnostic or conservative fallback

Use real corpus snippets only when the exact syntax shape or named external API
is the feature being tested. Otherwise reduce the example to synthetic Java
that would also make sense in another large corpus.

Keep the tests even if the benchmark count does not move, provided they capture
a real Java rule, a required fallback, or user-override precedence.

### 5. Implement In The Owning Layer

Keep responsibility boundaries strict:

- parser normalizes syntax only;
- resolver owns Java symbols, lookup, overloads, constructors, fields,
  inheritance, generic substitution, assignability, casts, coercion, target
  typing, and static/nested names;
- external API facts own behavior of libraries outside the translated corpus;
- semantic facts own finite effects, nullability, accessors, exceptions,
  recursion, and provenance;
- object strategy owns concrete struct, enum, `dyn Trait`, erased external, and
  fallback representation decisions;
- slot-shape and conversion planning own Rust-facing storage/API shapes and
  expression adaptation;
- lowering consumes facts and plans and should not gain Java or corpus
  heuristics.

If the right module cannot implement the rule cleanly, stop and refactor the
boundary rather than leaking the rule downstream.

### 6. Verify And Record

For each completed slice:

1. Run focused tests for the changed layer.
2. Run broader tests when shared resolver, fact, type, representation, or
   conversion behavior changed.
3. Re-run the GATK benchmark.
4. Update `docs/gatk-resolver-benchmark-report.md` with:
   - corpus motivation;
   - generalized rule;
   - responsible layer;
   - tests added;
   - before/after counts;
   - remaining risks or intentional conservative fallbacks.

The benchmark report is part of the deliverable. A slice that changes code but
does not say what corpus outcome it generalized from is incomplete.
