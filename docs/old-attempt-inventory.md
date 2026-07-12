# Old Attempt Inventory

Source inspected: `/home/mahogny/github/claude/java2rust`.

## Durable Structure

The old repository is a Rust crate with:

- `src/ast.rs`, `src/parse.rs`, `src/dump.rs`, and many `src/dump_*_ops.rs` modules.
- Semantic analysis modules for types, nullability, borrow facts, constructor null facts, symbol maps, stubs, naming, and runtime support.
- Runtime compatibility fragments under `src/runtime/`.
- Stdlib lowering tables and helpers under `src/stdlib.rs` and `src/stdlib_impl/`.
- Golden fixtures in `tests/corpus/*.java` with `*.rs.expected`.
- Larger corpus material under `testdata/`, including Trimmomatic, GATK, htsjdk, Picard, FastQC, BioFormats, jsoup, JTS, VarScan, jaligner, jahmm, and BioJava slices.
- Gate/report tooling under `tools/`.

The current old-repo status had untracked generated symbol/build files, so this inventory treats tracked docs and source organization as the durable evidence.

## Existing Corpus Signals

`docs/CORPORA.tsv` tracks Trimmomatic as a materialized corpus. The README also describes a broader SUM10 gate that includes Trimmomatic, jaligner, jahmm, VarScan, FastQ/VCF htsjdk slices, BioJava slices, jhlabs, and JTS, plus dedicated BioFormats and jsoup checks.

The tracked snippet corpus covers early syntax/codegen cases:

- Comments, packages, fields, locals, parameters, static methods.
- Integer expressions, arrays, for loops, switch, ternary, labels, synchronized.
- String concatenation.
- Simple object creation.
- Snake-case naming.

This new project should preserve the value of small snippets, but should group them by semantic responsibility and record expected Rust shape before implementation.

## History And Refactor Signals

Recent commits include many generic `stable`/`s` commits, followed by `start refactor`, `about to refactor`, `separate responsibilities`, and `docs`. The durable docs show that the old project was actively splitting a large dumper into many operation modules while trying to preserve behavior.

Important operating rules from the old docs:

- Exact facts beat names.
- `Unknown` is capable bottom, not positive evidence.
- Type/symbol/codegen failures must be separated from borrow/lifetime failures.
- A carrier representation change must update reads, writes, rendering, coercion, and operation dispatch together.
- Operation surfaces must be gated by real carrier/type evidence.
- Structural rewrites should be explicit, not hidden inside local expression printers.

## Architecture Lessons

Useful ideas:

- Keep provenance for Java origins.
- Build symbol facts before relying on cross-file or dependency calls.
- Treat nullability, ownership, routing, and resolved type as separate overlays.
- Use executable gates and report artifacts as part of the design.
- Keep stdlib/runtime compatibility behind explicit owners.

Problems to avoid:

- A central dumper becoming the place where every semantic decision happens.
- Name-based fallbacks silently driving collection, stream, static utility, or enum behavior.
- Making rustc error count the only progress measure.
- Adding local patches for every new corner case instead of strengthening the rule that owns the class.
- Letting stubs become semantic evidence.

## Failure Classes To Design For

- Type/name resolution: imports, same-package lookup, static owner shadowing, overloads, and linked dependencies.
- Carrier classification: list/map/set/stream/iterator/optional/object carriers must be facts, not name guesses.
- Nullability: `null`, fields initialized by constructors, optional returns, and reads after checks.
- Ownership and aliasing: Java reference semantics versus Rust moves, clones, borrows, and shared mutation.
- Object model: inheritance, interface dispatch, `super`, dynamic dispatch, and recursive hierarchies.
- Generics: owner parameter substitution, wildcards, raw types, bounds, and erased object positions.
- Arrays and numeric promotion: Java index and numeric conversion rules differ from Rust.
- Stdlib: collection factories, streams, strings, optionals, paths/files, boxed primitives, and exact owner gates.
- Structural rewrites: temporaries, borrow conflicts, constructor initialization, anonymous/local/inner class capture.
- External APIs: untranslated dependencies need exact symbol maps or explicit stubs, not simple-name guesses.

## Design Consequence For This Repo

This repo should begin with a documented subset and a narrow example suite. The first implementation slice should prove the pipeline and fact boundaries before attempting corpus breadth. GATK-scale and dependency-heavy corpuses should remain design input and later gates, not the first implementation target.

