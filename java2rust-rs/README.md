# java2rust-rs

A Rust reimplementation of [java2rust](../java-to-rust) — a command-line tool that
ports Java source code to (somewhat "unrusty") Rust.

The goal is **behavioral equivalence**, not a source-level translation: given the
same Java input, this tool should emit the **same Rust text** as the original
`java2rust.jar`. Internals are idiomatic Rust and use tree-sitter for parsing.

## Architecture

The original Java tool runs a four-stage pipeline (see `JavaConverter.convert`):

1. **Parse** Java → AST (originally JavaParser 2.5.1).
2. **IdTrackerVisitor → IdTracker** — scope / identifier tracking.
3. **TypeTrackerVisitor** — type inference over tracked identifiers.
4. **RustDumpVisitor** — the code generator (81 visit methods, ~2200 LOC) + a
   `SourcePrinter`. Produces the Rust string.

We reproduce the same pipeline. The key decision is the **parse layer**:

> **tree-sitter parses → an adapter builds a JavaParser-shaped typed AST → the
> three passes are ported ~1:1 against that typed AST → the dumper emits a string.**

tree-sitter does the robust parsing; an adapter reconstructs a typed AST that
mirrors the JavaParser node types the original code is written against. This
isolates every tree-sitter quirk in one adapter layer and lets the intricate
codegen port mechanically — the lowest-risk route to matching output.

## Conformance strategy

The original jar is a deterministic `String -> String` oracle. We build a
**golden corpus**: every Java input from the original's JUnit tests (plus more),
run through `java2rust.jar` to capture exact `.rs` output, then asserted
byte-identical from this port. The original `containsString` JUnit assertions
port over as a second, looser check.

## Status: complete — 42/42 golden cases byte-identical to `java2rust.jar`

All phases are done. `cargo test` passes; `cargo run --example check` reports
`42/42 passing`. The corpus covers every input from the original JUnit tests plus
generalization cases (switch, ternary, string concat, labeled loops,
synchronized, casts, a multi-construct class, comments, ...).

Modules:
- `ast.rs` — arena AST, all ~90 JavaParser node kinds, `JClass` type model.
- `adapter.rs` — tree-sitter CST → arena, PartParser tiered wrapping with
  JavaParser-acceptance shape validation, and comment attribution.
- `id_tracker.rs` — `IdTracker` + `Block` + `IdTrackerVisitor` + `java.lang` resolution.
- `type_tracker.rs` — `TypeTrackerVisitor`.
- `dump.rs` — `RustDumpVisitor` (all 81 visit methods) + `SourcePrinter`.
- `modifiers.rs` — `ModifierSet`. `naming.rs` — `NamingHelper`.
- `main.rs` — CLI (`-d -o -i -v -cp`, directory recursion).

### Phases (all ✅)

0. Scaffold — crate, tree-sitter deps, golden harness.
1. Golden corpus — inputs + expected outputs from the jar (`tools/gen_golden.sh`).
2. Typed AST — JavaParser node types, `ModifierSet`, positions, comments.
3. Adapter — tree-sitter CST → typed AST.
4. IdTracker + IdTrackerVisitor.
5. TypeDescription + TypeTrackerVisitor.
6. RustDumpVisitor + SourcePrinter + NamingHelper + helpers.
7. CLI — `main`, directory recursion, options.
8. Close gaps — golden corpus passes 100%.

## Known risk areas (tree-sitter ≠ JavaParser)

- **Comment attribution** — the dumper prints comments; tree-sitter treats
  comments as "extras", so the adapter needs a JavaParser-like attach pass.
- **Position ordering** — `sortByBeginPosition`, mapped from tree-sitter ranges.
- **Special literals** — `IntegerLiteralMinValueExpr` / `LongLiteralMinValueExpr`
  (how JavaParser models `MIN_VALUE`) have no tree-sitter equivalent.
- **`ModifierSet`** bitmask semantics; identity-keyed `Map<Node,…>` → stable ids.

## Usage (target)

```
java2rust-rs -d <path_file.java | path_directory> [-o output] [-i] [-v 2] [-cp]
```
