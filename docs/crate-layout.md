# Crate Layout

No transpiler implementation code should be written until the design phase is accepted. This file records intended ownership boundaries.

## Proposed Modules

- `parser`: adapter around the chosen Java parser, with source spans.
- `java_ast`: parser-facing source model if the parser type is too external or noisy.
- `java_ir`: normalized Java representation used by all later stages.
- `resolve`: packages, imports, scopes, members, overloads, and FQN resolution.
- `types`: Java type model, Rust type mapping, generic substitution, and primitive promotion.
- `facts`: nullability, constructor initialization, mutability, ownership, hierarchy, carrier, and external symbol facts.
- `lower`: Java IR to Rust IR transformation passes.
- `rewrite`: explicit structural rewrites such as temporary hoisting and loop reshaping.
- `rust_ir`: typed Rust output model.
- `emit`: Rust code generation from Rust IR.
- `stdlib`: exact JDK mapping tables and operation-surface rules.
- `runtime`: compatibility shims used by generated Rust.
- `diagnostics`: unsupported constructs, fallbacks, warnings, and source spans.
- `fixtures`: loading examples and expected shapes.

## Responsibility Rule

No module should both discover a semantic fact and independently decide a separate operation surface from names. Fact producers produce evidence; consumers either use that evidence or emit a diagnostic.

## First Crate Skeleton Later

When implementation begins, create a normal Rust crate with placeholder modules only as needed for the first slice. Avoid pre-creating every module if that encourages empty architecture.

