# Tree-Sitter Parser Plan

## Goal

Replace the current bootstrap parser internals with a tree-sitter Java adapter while preserving the existing architecture:

```text
tree-sitter Java CST -> parser adapter -> java_ir -> resolve/types/coercion/lower/emit
```

Tree-sitter nodes must not leak past `src/parser.rs` or a small parser-facing helper module. The rest of the transpiler continues to consume `java_ir`.

## Rationale

The handwritten parser was useful to prove the first slice offline, but it is not a real Java frontend. The previous java2rust attempt used `tree-sitter` and `tree-sitter-java`, and those crates are already available in the local Cargo cache:

- `tree-sitter 0.26.9`
- `tree-sitter-java 0.23.5`

Using tree-sitter now reduces parser-specific corner cases and lets later work focus on resolution, typing, coercion, lowering, and diagnostics.

## Constraints

- Preserve `java_ir` as the parser-neutral boundary.
- Preserve current passing behavior for the first-slice fixtures.
- Preserve structured unsupported diagnostics for out-of-slice fixtures.
- Do not copy the old repo parser wholesale unless a piece cleanly fits the new smaller architecture.
- Do not let tree-sitter CST node kinds become semantic decisions outside the parser adapter.

## Implementation Steps

### 1. Add Dependencies

Add to `Cargo.toml`:

```toml
tree-sitter = "0.26.9"
tree-sitter-java = "0.23.5"
```

Run `cargo check` to confirm the cached crates build without network.

### 2. Introduce Parser Adapter Internals

Keep the public parser API:

```rust
pub fn parse_java_source(source: &str) -> ParsedProgram
```

Internally split `src/parser.rs` into responsibility sections or helper functions:

- tree-sitter parse setup
- CST node navigation helpers
- type conversion
- expression conversion
- statement conversion
- declaration/member conversion
- unsupported syntax diagnostics

If the file grows too much, create a parser-private module such as `src/parser/ts.rs` later.

### 3. Convert Top-Level Structure

Map tree-sitter nodes into `java_ir`:

- `program`
- `package_declaration`
- `import_declaration`
- `class_declaration`
- `interface_declaration` as unsupported-but-representable
- class body members

Initial target: one top-level class per file, matching the first implementation slice.

### 4. Convert Declarations

Support first-slice declaration nodes:

- fields
- constructors
- methods
- modifiers
- parameters
- primitive types
- `String`
- array types
- simple generic type shapes, enough to preserve unsupported fixtures as IR/diagnostics

Out-of-slice declarations should produce diagnostics but still preserve as much IR shape as reasonable.

### 5. Convert Statements

Support:

- local variable declarations
- assignments and compound assignments
- expression statements
- `return`
- `if`
- `while`
- classic `for`
- enhanced `for`
- `break`
- `continue`
- block statements

Diagnose but represent:

- `switch`
- `try`/`catch`/`finally`
- local class declarations
- synchronized blocks
- throw statements

### 6. Convert Expressions

Support:

- literals
- identifiers
- `this`
- field access
- array access
- binary expressions
- unary/postfix increment/decrement
- ternary expressions
- method invocation
- object creation where shape is simple enough

Diagnose but preserve raw/unsupported nodes for:

- lambdas
- method references
- casts if not yet lowered
- anonymous class bodies
- complex generic method invocations

### 7. Keep Guardrail Diagnostics Syntax-Based

Replace current string-search guardrails with tree-sitter node based diagnostics where possible:

- inheritance: `superclass`
- interfaces: `super_interfaces`, `interface_declaration`
- checked exceptions: `throws`
- `catch_clause`
- `finally_clause`
- lambda expression
- wildcard type
- stream calls can remain semantic/string-ish until stdlib resolution is stronger, but prefer expression-shape detection later.

### 8. Preserve Resolver/Type/Coercion Boundaries

After parser replacement:

- `resolve` still owns overload/call target facts.
- `types` still owns expression typing.
- `coercion` still owns slot adaptation.
- `lower` still consumes facts and emits Rust IR.

No parser code should decide Rust names, overload targets, Rust coercions, or final operation surfaces.

## Tests

### Existing Tests Must Continue Passing

Run:

```text
cargo fmt
cargo test
```

Existing tests cover:

- fixture loading
- first-slice translation shape
- generated Rust compilation
- unsupported diagnostics
- type inference/coercion responsibility
- same-class overload call resolution

### Add Parser-Specific Tests

Add tests that assert tree-sitter parsing reaches the expected IR shape:

- package and imports are parsed from a fixture
- `SimpleClass` has two fields, constructor, and two methods
- `ExpressionPrecedence` contains a ternary return expression
- `LoopsAndBranches` contains a classic `for`
- `WildcardBounds` produces a wildcard diagnostic from syntax
- `CheckedException` produces a throws/exception diagnostic from syntax

## Migration Strategy

Do the replacement incrementally:

1. Add dependencies and a tree-sitter parse smoke test.
2. Parse package/import/class shells.
3. Parse fields/method signatures.
4. Parse first-slice statements and expressions.
5. Switch `parse_java_source` to tree-sitter output.
6. Delete or quarantine handwritten parser helpers that are no longer used.
7. Run full test suite after each step.

If a tree-sitter conversion gap blocks first-slice tests, keep the handwritten fallback only behind an explicit temporary function and mark it for deletion in this document.

## Completion Criteria

This migration is complete when:

- `Cargo.toml` depends on tree-sitter Java.
- `parse_java_source` uses tree-sitter for normal parsing.
- Current first-slice generated Rust still compiles in tests.
- Guardrail diagnostics are generated from tree-sitter syntax nodes where practical.
- No tree-sitter node types leak into `java_ir`, `resolve`, `types`, `coercion`, `lower`, or `emit`.
- Handwritten parser logic is deleted or explicitly quarantined as temporary fallback.

