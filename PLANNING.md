# Java-to-Rust Transpiler Planning

## Aim

Create a Java-to-Rust transpiler written in Rust. The emitted Rust should be as idiomatic as possible while preserving Java semantics for the supported subset.

The previous attempt at `/home/mahogny/github/claude/java2rust` is useful as design input, especially its corpuses, failure patterns, and history. This project should avoid growing around many small unresolved corner cases by defining examples and transformation rules before implementation.

## Ground Rule

Do not start implementing transpiler logic until the example suite and design documents have been drafted and evaluated against each other.

It is acceptable to sketch Rust crate/module layout, placeholder responsibilities, and public data structures if that helps clarify the design. Avoid substantive translation code until the design phase is complete.

## Shared Rust Refactoring Tooling

`/data/henriksson/github/claude/rust_refactor` is available as a shared Rust
cleanup/refactoring tool for this and future transpilers. In particular, it can
handle Rust inlining work that is common across generated outputs. Treat it as a
post-generation cleanup step: it assumes the Rust code is already in a working
state, so the Java translator must still emit correct, type-checkable Rust
before delegating cleanup there.

## Planning Phases

### 1. Inventory the Previous Attempt

Inspect `/home/mahogny/github/claude/java2rust` and extract:

- Existing corpuses and fixture organization.
- Tests and examples that exposed corner cases.
- Git history showing repeated fixes, reversions, or unstable areas.
- Architecture choices that helped or hurt.
- Classes of unresolved failures, grouped by root cause rather than by individual bug.

Expected artifact:

- `docs/old-attempt-inventory.md`

### 2. Build a Narrow Java Example Suite

Create many small Java examples that each challenge one translator responsibility. Prefer simple, focused examples over broad programs.

Initial example groups:

- Expressions and operator precedence.
- Local variables, assignment, mutability, and scopes.
- Control flow: `if`, loops, `break`, `continue`, `switch`, early returns.
- Methods, overloads, static methods, and return typing.
- Fields, constructors, initialization order, and `this`.
- Classes, inheritance, overriding, `super`, dynamic dispatch.
- Interfaces, default methods, and constants.
- Generics and type erasure pressure points.
- Arrays, varargs, strings, and common collections.
- Nullability and defensive translations.
- Exceptions, checked exceptions, `try`, `catch`, `finally`.
- Visibility, packages, imports, and name resolution.
- Inner, nested, anonymous, and local classes.
- Java standard library calls that need Rust shims or idiomatic replacements.

For each example, record:

- The Java source.
- The specific language feature being tested.
- The expected Rust shape or semantic behavior.
- Whether idiomatic Rust requires a direct translation, a helper shim, or a deliberate rewrite.

Expected artifacts:

- `examples/java/...`
- `docs/example-suite.md`

### 3. Draft Transformation Rule Documents

Write design documents before coding. The rules should be concrete enough that a reviewer can predict the Rust output for each example.

Every transformation rule in the broader plan must follow the repository-wide
rule contract in `docs/transformation-rule-contract.md`: write the **Ideal
Rust Target** section before the **Implementation Rule** section. The ideal
target is the Rust we would want to maintain by hand when the necessary facts
are available. Only after that target is explicit should the rule describe the
semantic facts, owning layer, fallback shape, and evidence used to implement
it. This applies per transformation rule, not just per document or milestone.
If a document lists several mappings, each mapping must either have its own
target-first rule entry or explicitly say that the mappings share one target
Rust shape and one implementation rule.

Suggested documents:

- `docs/source-subset.md`
  - Supported Java version/features.
  - Explicitly unsupported or deferred constructs.

- `docs/pipeline.md`
  - Parser input.
  - Java AST normalization.
  - Name/type resolution.
  - Java IR.
  - Lowering passes.
  - Rust IR.
  - Rust emission and formatting.

- `docs/type-mapping.md`
  - Primitive types.
  - Boxed primitives.
  - Strings.
  - Arrays.
  - Collections.
  - User classes.
  - Generics.
  - Nullability.

- `docs/ownership-and-mutability.md`
  - When to emit owned values, references, `Clone`, `Rc`, `Arc`, `RefCell`, or `Box`.
  - How to model Java aliasing.
  - How to preserve semantics without making every output maximally indirect.

- `docs/object-model.md`
  - Classes.
  - Struct layout.
  - Constructors.
  - Inheritance strategy.
  - Interfaces and trait generation.
  - Dynamic dispatch.
  - `super`, overriding, and visibility.

- `docs/control-flow.md`
  - Statement lowering.
  - Loop translation.
  - `switch`/`match`.
  - Labeled breaks and continues.
  - Definite assignment concerns.

- `docs/errors-and-exceptions.md`
  - Checked exceptions.
  - Runtime exceptions.
  - `try`/`catch`/`finally`.
  - Mapping to `Result`, panic, or helper runtime types.

- `docs/standard-library.md`
  - Java APIs translated to idiomatic Rust.
  - Java APIs requiring compatibility shims.
  - Deferred APIs.

- `docs/rust-emission-style.md`
  - Naming.
  - Formatting.
  - Module layout.
  - Trait/impl organization.
  - Idiomatic rewrites that are allowed.

Expected artifact:

- The design docs above, plus any additional focused docs required by the examples.

### 4. Evaluate Rules Against Examples

For every gathered Java example:

- Apply the transformation rules manually.
- Record the expected Rust output or a precise Rust shape.
- Identify missing rules.
- Mark each example as covered, partially covered, or blocked.

This phase should reveal design problems before they become implementation corner cases.

Expected artifacts:

- `docs/example-evaluation.md`
- Updated transformation docs with resolved gaps.

### 5. Sketch Crate and Module Responsibilities

Define where responsibilities live without implementing the logic.

Likely crate/module areas:

- `parser`: adapter around the chosen Java parser.
- `java_ast`: source syntax model or parser-facing wrappers.
- `resolve`: names, scopes, imports, packages, overload candidates.
- `types`: Java type model and Rust type mapping.
- `java_ir`: normalized Java representation.
- `lower`: transformation passes from Java IR toward Rust IR.
- `rust_ir`: Rust-oriented semantic output model.
- `emit`: Rust code generation.
- `runtime`: optional Java compatibility shims.
- `diagnostics`: unsupported constructs, warnings, and source spans.
- `fixtures`: example loading and expected-output handling.

Expected artifact:

- `docs/crate-layout.md`
- Optional placeholder `.rs` files only if they clarify boundaries.

### 6. Choose the First Implementation Slice

After the design docs and examples have been evaluated, choose a small first implementation slice.

The first slice should prove the whole pipeline shape, not maximize Java feature coverage. A good candidate is a tiny subset such as:

- One class.
- Primitive fields.
- One constructor.
- Simple methods.
- Local variables.
- `if` and basic loops.
- No inheritance, generics, exceptions, or complex standard library calls.

Expected artifact:

- `docs/first-implementation-slice.md`

## Completion Criteria for Planning

Planning is complete when:

- The previous attempt has been inventoried.
- A focused Java example suite exists.
- Transformation rules are documented.
- Every example has been evaluated against the rules.
- Known gaps are explicit.
- The crate/module layout is documented.
- A small first implementation slice has been selected.
