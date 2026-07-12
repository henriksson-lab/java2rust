# Rust Emission Style

## Names

- Java classes/interfaces -> `UpperCamelCase`.
- Java methods/fields/locals -> `snake_case`.
- Java constants -> `SCREAMING_SNAKE_CASE` when they become Rust constants.
- Avoid collisions with Rust keywords by adding a trailing underscore and recording the original Java name in provenance.

## Modules

Mirror Java packages into Rust modules unless crate layout docs choose a flatter test-fixture structure. Keep one source Java top-level type per generated Rust module where practical.

## Formatting

Emit Rust IR, then format with `rustfmt`. String concatenation should become `format!` only when that is the idiomatic shape; otherwise use `String` mutation for repeated appends.

## Provenance

Generated items should include Java origin comments or attributes sufficient for later symbol maps:

- Type FQN.
- Field FQN/member.
- Method FQN/signature.
- Constructor origin.
- Diagnostics for fallbacks and audit-needed clones.

## Idiomatic Rewrites

Allowed when semantics are preserved:

- Use iterators for enhanced `for` over concrete Rust collections.
- Use `Option` combinators for simple optional logic.
- Use `Result` and `?` for checked exception propagation.
- Use traits for interface contracts.

Not allowed without a rule:

- Dropping null checks.
- Replacing Java aliasing with deep clones.
- Treating broad `Collection` as indexable.
- Rewriting shadowed JDK-looking names.

