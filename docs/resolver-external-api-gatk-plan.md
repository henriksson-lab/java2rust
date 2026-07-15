# Resolver and External API Fact Plan

## Purpose

The next design-critical work is to improve Java semantic resolution and
external API facts, using GATK as the pressure corpus.

The goal is not to hard-code GATK. GATK should expose real failure classes:
inheritance pressure, generics, external library calls, fluent APIs, exception
usage, nested types, and collection-heavy code. Each failure class should then
be reduced into a small general fixture or regression test that would also
apply to non-GATK Java code.

## Design Priority

The current object model, slot shape, conversion, and lowering boundaries depend
on accurate Java facts. If calls, constructors, fields, inheritance edges,
generic bindings, or external APIs are unresolved, later stages either fall
back too often or risk rediscovering Java semantics in lowering.

This work should therefore happen before broad Rust idiom tuning.

## Main Workstreams

### 1. Java Semantic Resolution

Add or strengthen resolver-owned facts for:

- method overload targets
- constructor targets
- inherited methods and fields
- static members and static imports
- nested and inner class names
- generic type parameters and substitutions
- wildcard bounds and erasure
- field versus local versus parameter binding
- explicit unresolved targets with structured diagnostics

Resolution must stay before Rust shape solving. Lowering should consume resolved
facts and diagnostics, not guess based on Java names.

### 2. External API Fact Layer

Introduce a stronger model for Java libraries that are not translated as part
of the corpus.

External API facts should describe:

- receiver mutability
- return nullability
- parameter nullability
- fluent setters and builders
- getter/setter effects
- collection and iterator shapes
- ownership or borrowed-view expectations
- declared or inferred exception behavior
- whether a call result is a boundary with unknown lifetime or aliasing

Facts may come from user overrides, corpus summaries, known JDK models, or
conservative name-based guesses. The precedence must continue to flow through
`EffectiveFacts`, not lowering.

### 3. Control-Flow Normalization Inputs

Use resolver and semantic facts to support normalization of Java control-flow
patterns before lowering:

- assignment in conditions, such as `while ((x = next()) != null)`
- switch state machines and fallthrough
- try-with-resources
- enhanced loops
- early returns and rethrow paths
- tail-recursive and mutually recursive method facts

The immediate aim is to produce plans and diagnostics. Full idiomatic rewriting
can come later.

### 4. Generic and Container Shape Facts

Make Java container semantics visible to slot-shape and conversion planning:

- `List<T>`, `Set<T>`, `Map<K, V>`
- arrays and primitive arrays
- `Optional<T>`
- `Iterator<T>` and `Iterable<T>`
- stream sources and terminal values
- wildcard and diamond inference results

These facts should avoid special cases such as treating one stream method as an
ownership rule. The container or terminal operation should produce an actual
shape, and conversion planning should adapt it to the target slot.

### 5. Exception Model Integration

Strengthen exception facts so method signatures can be shaped before lowering:

- checked versus unchecked exception classes
- exception hierarchy assignability
- catch ordering and multi-catch facts
- rethrow and wrapping behavior
- external API exception summaries
- `Result` shaping for methods that can throw

Exception decisions belong in semantic facts and signature planning. Statement
lowering should not infer exception class relationships.

## Corpus Benchmark Strategy

Use GATK as the main benchmark for this phase because it stresses the resolver
and external API model more than the small fixtures.

Track at least these metrics:

- number of Java files parsed without fallback nodes
- number of files that finish resolution without panic
- count and kind of unresolved method calls
- count and kind of unresolved constructors
- count and kind of unresolved field accesses
- count and kind of external API guesses
- count of facts produced per file and whether saturation terminates within
  budget
- top recurring unsupported diagnostics

The benchmark should be report-oriented. Corpus progress should not be achieved
by adding GATK-specific lowering branches.

## Test Policy

Every implementation slice should add tests based on the outcome of the corpus
run.

Those tests should be minimized and generalized:

- Prefer small synthetic Java fixtures that isolate the semantic pattern.
- Name the underlying rule, not the corpus file, in the test.
- Avoid copying large GATK fragments unless the exact syntax shape is essential.
- Keep GATK examples as comments or report references when useful, not as the
  only assertion.
- Assert facts, plans, and diagnostics at the responsible layer before checking
  final Rust output.
- Add final-output tests only when the responsible facts and plans are already
  covered.
- When a GATK failure is caused by a known Java library pattern, add an external
  API model test that would also apply to another project using the same kind of
  API.

Examples of good regression tests:

- inherited generic method resolution through an abstract superclass
- overloaded constructor selection with diamond inference
- external fluent setter marks receiver mutable and returns `self`
- external getter result assigned to a field creates a boundary fact
- `while ((x = next()) != null)` produces assignment-condition normalization
  facts
- multi-catch over an exception superclass and subclass records assignability
- unresolved external call emits a structured conservative boundary diagnostic

Examples of tests to avoid:

- asserting behavior for one exact GATK class name when a smaller class would
  cover the same rule
- adding a lowering snapshot that passes only because a Java method name was
  special-cased
- encoding one library's package path when the relevant behavior is generic
  getter, setter, builder, iterator, or exception behavior

## Implementation Slices

1. Add a GATK resolver benchmark command or test harness that reports unresolved
   targets and fact counts without requiring final Rust output.
2. Run the benchmark and classify the highest-volume unresolved categories.
3. Add minimized parser, resolver, semantic-fact, or external-fact tests for the
   first category before implementation.
4. Implement the responsible fact or resolver improvement.
5. Re-run the benchmark and update the report with before/after counts.
6. Repeat for the next category, keeping each slice tied to a general test.

## Acceptance Criteria

- GATK benchmark reports are reproducible from the repo.
- New tests are derived from corpus outcomes but do not overfit to GATK names or
  package structure.
- Resolver emits structured facts or diagnostics for unresolved calls,
  constructors, and fields.
- External API behavior is represented as facts with provenance and precedence.
- Lowering does not gain new Java name heuristics.
- Fact saturation remains finite on recursive and dependency-heavy examples.
- At least one benchmark report shows reduced unresolved resolver categories
  after generalized tests and implementation.

## Deferred Work

- Full translation quality scoring for all GATK files.
- Broad idiomatic Rust rewrite passes for all control-flow normalization plans.
- Automatic mining of external API facts from bytecode or Maven metadata.
- Corpus-specific override packs for production translation runs.
