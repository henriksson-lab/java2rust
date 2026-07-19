# Transformation Rule Contract

Transformation rules must be designed from the Rust we want to emit, not from
the Java syntax symptom currently failing. Every tracked transformation rule
must include an **Ideal Rust Target** section before its **Implementation
Rule**. New rules must be written this way from the start; older rules that do
not yet follow the order must be normalized before they are used for an
implementation slice.

This applies to lowering rules, standard-library mappings, dependency boundary
rules, coercion rules, object representation rules, control-flow rewrites,
exception rules, constructor rules, and fallback rules. Any plan that proposes
one of those changes must include the same target-first structure before the
rule is considered ready to implement.

This is the repository-wide contract for the broader transpiler plan. Planning
documents may add local checklist detail, but they must not weaken this order:
each transformation rule needs its own explicit **Ideal Rust Target** section
before its **Implementation Rule** section. The ideal target is where we decide
what idiomatic Rust should look like before implementation details, corpus
failures, or current IR limitations bias the rule.

This is also the review invariant for the broader plan: transformation rules
are target-first artifacts. A rule entry that describes an implementation
algorithm, AST pattern, fixture symptom, corpus failure, or current generated
output before stating the **Ideal Rust Target** is incomplete, even if the
implementation idea is otherwise plausible.

The requirement applies at the level of each individual transformation rule,
not just at the level of a plan section, milestone, implementation slice, or
benchmark bucket. If a section contains several mappings, each mapping needs
its own **Ideal Rust Target** before its own **Implementation Rule**. Grouping
several Java symptoms under one implementation note is not enough unless they
share the same documented target Rust shape and the same fact requirements.

For the broader transpiler plan, this is a readiness gate. A transformation
rule is not ready to implement, review, or mark complete until the tracked
documentation for that rule has an **Ideal Rust Target** section immediately
before its **Implementation Rule** section. Status notes, current generated
output, benchmark failures, parser observations, and module-level mechanics may
explain why the rule matters, but they must not replace or precede the target
Rust shape, and they must not sit between the target and implementation
sections.

This is also a normalization requirement for broad planning documents. A plan
may start with coarse milestones, but before any milestone becomes an
implementation slice, every transformation rule inside it must be split into a
trackable rule entry with its own **Ideal Rust Target** followed by its own
**Implementation Rule**. Section-level prose such as "make streams idiomatic"
or "improve external APIs" is not enough to start coding until the contained
mappings have been normalized into target-first rule entries.

For the broader plan, this is mandatory for every transformation rule: add an
explicit **Ideal Rust Target** section before the **Implementation Rule**
section. The ideal target is where we choose the Rust shape we are aiming for;
the implementation rule is only where we describe how facts and plans select
that already chosen shape.

The section names are part of the contract. A rule entry may use Markdown
headings, checklist fields, or bullets to match the surrounding plan, but it
must contain an explicitly named `Ideal Rust Target` entry followed by an
explicitly named `Implementation Rule` entry. Synonyms such as "desired output",
"approach", "lowering", or "algorithm" are not substitutes for these two rule
sections.

## Ideal-First Design Gate

Before implementing or materially revising a transformation rule, write the
**Ideal Rust Target** first. The target is not an example after the fact; it is
the design input that decides which semantic facts, ownership shapes,
nullability guarantees, conversions, fallback forms, and owning modules the
implementation must preserve.

A rule may be motivated by a failing fixture or corpus diagnostic, but the
tracked rule must be phrased as:

- the Rust shape we would want to read and maintain by hand
- the semantic facts that prove that shape is legal
- the implementation rule that selects that shape from those facts

If the ideal Rust target cannot be stated yet, the next task is analysis or
fact-model design, not lowering code.

This gate applies to every broader-plan rule that affects emitted Rust:
ownership, borrowing, lifetimes, nullability, mutability, coercion, dispatch,
object representation, external APIs, collections, streams, exceptions,
control flow, constructors, recursion, fallback runtime shims, and diagnostics.
Those topics may be co-solved by shared facts or solvers, but their documented
rules still need the target section before the implementation section.

## Required Section Order

Use this order for each transformation rule:

1. **Ideal Rust Target**
   - Show the Rust API, expression, type, or control-flow shape we would write
     by hand.
   - Include ownership, borrowing, nullability, mutability, dispatch,
     collection, error, lifetime, and external-boundary shape when those
     dimensions affect the rule.
   - The target should be source-independent. It may be inspired by a fixture
     or corpus issue, but it must describe the desired general Rust outcome.
2. **Implementation Rule**
   - State how semantic facts and plan nodes select that target.
   - Avoid fixture names, package-name branches, rendered-string matching, and
     Java spelling-specific shortcuts unless the Java language semantics
     require that exact spelling.
3. **Required Facts**
   - List the facts needed to emit the target shape, such as resolved owner,
     callable signature, representation, slot shape, nullability, mutability,
     effects, hierarchy, source/dependency mode, or external capability.
4. **Owning Layer**
   - Assign each decision to the module or plan that owns it.
   - Rendering must not rediscover semantic facts. Semantic analysis must not
     decide final Rust spelling.
5. **Fallback Shape**
   - State what happens when the required facts are unavailable.
   - Fallbacks must be finite and explicit: `Option<T>`, `Box<dyn Trait>`,
     `Box<dyn Any>`, an opaque facade, a runtime shim, or a structured
     unsupported diagnostic.
6. **Evidence**
   - Add design-level tests for the owning layer.
   - Add neutral fixtures or output assertions when emitted Rust changes.
   - Corpus-derived tests should minimize the general pattern and avoid
     encoding the corpus class or method as the rule.

The first two sections are mandatory, named, adjacent, and ordered. A
transformation rule entry must not introduce an implementation rule, lowering
algorithm, matcher, renderer behavior, current output sample, or corpus failure
before the **Ideal Rust Target** section. The **Implementation Rule** section
must follow the target section so reviewers can compare the proposed algorithm
against the Rust shape it is supposed to preserve before reading supporting
facts, ownership, fallback, evidence, or status notes.

This ordering is required even when the implementation work seems obvious or
small. The implementation rule is not where the target Rust shape is chosen;
it is only where the pipeline rule for selecting an already documented target
is written. If the ideal target and the implementation rule cannot be separated
cleanly, the work item is still mixing design responsibilities and should be
split before coding.

## Rule Template

Use this template when adding a new rule or materially changing an existing
rule. Do not replace the first two sections with implementation notes,
diagnostics, corpus observations, or examples of generated output.

```md
### RULE-ID: Short Rule Name

#### Ideal Rust Target

Describe the Rust we would write by hand when the relevant facts are known.
Include the target API, expression, type, ownership, borrowing, nullability,
mutability, dispatch, error, and external-boundary shape when they affect the
rule.

#### Implementation Rule

Describe how semantic facts and plan nodes select the ideal target. This must
be phrased as a general transformation, not as a fixture-specific or
corpus-specific branch.

#### Required Facts

List the facts needed to make the rule legal.

#### Owning Layer

Name the module or plan that owns each decision.

#### Fallback Shape

State the structured fallback or diagnostic used when facts are missing.

#### Evidence

List the design tests, fixtures, corpus reductions, or compile gates that prove
the rule.
```

## Review Check

A rule is not ready for implementation if its first concrete statement is
"lower Java pattern X as Rust expression Y". First write the **Ideal Rust
Target**, then explain which facts prove that target is legal and which
fallback is used when those facts are missing.

Do not mark a transformation-rule task complete unless the tracked rule
documentation has the **Ideal Rust Target** section immediately before the
**Implementation Rule** section. This applies even when the code change is
small, benchmark-driven, or implemented in a narrow module: the target Rust
shape is part of the deliverable, not optional design commentary.

When reviewing a plan or implementation slice, audit the changed rule entries
one by one. A slice may be complete only if every changed mapping has an
explicit ideal target, a fact-driven implementation rule, required facts,
owning layer, fallback shape, and evidence.

This contract is intentionally stricter than a TODO list. Its purpose is to
prevent narrow mappings from becoming hidden special cases in later pipeline
stages.

## Broader Plan Rule

For any broader-plan document, treat each transformation rule as a target-first
design record. The rule may live inside a milestone, benchmark slice, corpus
report, dependency plan, or feature checklist, but the tracked rule entry must
still put **Ideal Rust Target** before **Implementation Rule**.

Each transformation rule must therefore answer the design question before the
mechanical one: first document the ideal Rust target, then document the
implementation rule that selects that target. The implementation rule is not
allowed to be the place where the desired Rust shape is discovered, selected,
or justified after the fact.

This order is required for every transformation rule, not just for examples or
large design sections. Before a rule describes how to match Java syntax, which
semantic facts to read, which plan node to build, which Rust expression to
render, or which corpus failure it fixes, it must first have an explicitly
named **Ideal Rust Target** section. The **Implementation Rule** section comes
after that target and explains how the pipeline selects the already-stated
Rust shape.

This is a per-rule documentation requirement, not a stylistic preference for
long-form design docs. Each transformation rule in the broader plan must have
an explicitly named **Ideal Rust Target** section immediately before its
**Implementation Rule** section. The ideal target is where the desired idiomatic
Rust output is chosen; the implementation rule is only allowed to explain how
facts, constraints, plan nodes, and fallbacks select that already-stated target.

When a broad item contains several mappings, split it before implementation:
one target, one implementation rule, one owning layer, and one fallback shape
per mapping unless the document explicitly states that the mappings share the
same ideal Rust shape and fact requirements. A plan item that only says which
Java pattern to match, which module to edit, or which fixture currently fails
is still an analysis item, not an implementable transformation rule.

Use this as the clearing condition for planning tasks: a transformation-rule
task can move from design to implementation only after the ideal Rust target is
documented first and the implementation rule is written as the fact-driven
method for selecting that target.

When expanding a broad plan item into work, use this audit before coding:

- Identify every distinct emitted Rust shape affected by the item.
- Create or update one transformation-rule entry per distinct shape.
- Put **Ideal Rust Target** before **Implementation Rule** in each entry.
- Check that the target section can be reviewed without knowing the proposed
  matcher, module edit, or current generated output.
- Check that the implementation section only explains how facts, constraints,
  plan nodes, and fallbacks select the documented target.
- Reject or leave open any rule whose first concrete section is an
  implementation algorithm, Java matcher, current output sample, or benchmark
  symptom.
- Keep corpus symptoms, current generated output, module sketches, and status
  notes after those two sections.
- Do not implement the item until every affected rule has passed that order
  check.
