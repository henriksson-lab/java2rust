# Proceed From Current Corpus Fixes

Date: 2026-07-23

This plan starts from the current dirty checkout and the evidence in
`docs/current-corpus-issue-audit.md` and `docs/corpus-fix-next-plan.md`.

The immediate objective is to keep the translator stable while converting the
remaining corpus failures into minimized idiom fixtures and general fixes. The
goal is not to hand-tune generated Rust for one corpus file; every accepted fix
should improve a shared planner, lowering rule, facade fact, or explicit FFI
handoff shape.

## Current Baseline

- Local gates were green after the latest parser, receiver, and stream-super
  slices:
  - `cargo fmt --check`
  - `cargo check --quiet`
  - `cargo test --test parser_gaps --quiet`
  - `cargo test --test idiom_gaps --quiet`
  - `cargo test --test batch_translation --quiet`
  - `cargo test --test type_coercion --quiet`
- Clean focused corpuses:
  - `picard-programgroups`
  - `htsjdk-fastq`
  - `varscan`
- FastQC now completes translation without the old duplicate-owner signature
  panic. The fresh parser rerun translated 156/156 files and showed that the
  `in::read`, cast-comparison, block-comment condition, hex-float, and
  sign-bit long literal blockers are gone from generated sources.
- FastQC still has invalid patterns:
  - 58 `compile_error!` placeholders across 24 files;
  - 40 Java cast downcasts across 14 files;
  - 0 `.put` calls.
- The largest remaining FastQC placeholder family is constructor
  `super(...)`. The JDK stream-wrapper no-state slice fixed Base64 stream
  wrappers and the public `MultiMemberGZIPInputStream` constructors, but the
  private parent-linking `MultiMemberGZIPInputStream(parent)` constructors
  remain blocked by object graph mutation after `super(parent.in)`.

## Principles

- Add a minimized fixture before changing lowering behavior.
- Prefer shared planner facts over rendered-string rewrites.
- Do not add non-JDK package behavior to `src/stdlib.rs`; package-specific
  behavior belongs in `javalibs` manifests, shims, or facade facts.
- Do not fake native, distributed, or object-identity semantics. GenomicsDB
  native behavior and DISQ/Spark distributed execution should lower to
  explicit handoff or facade shapes.
- Keep algorithmic performance work compatible with future global semantic
  analysis. Indices and clone reduction are acceptable; local-only shortcuts
  are not.
- Treat `/data/henriksson/github/claude/rust_refactor` as a later cleanup and
  inlining tool for working Rust, not as a substitute for making generated code
  typecheck here.

## Phase 1: Reconfirm The Local Gate

Run this before the next implementation slice:

```sh
cargo fmt --check
cargo check --quiet
cargo test --test parser_gaps --quiet
cargo test --test idiom_gaps --quiet
cargo test --test batch_translation --quiet
cargo test --test type_coercion --quiet
```

If a gate fails, fix or document that regression before touching new corpus
families.

## Phase 2: Finish The Current FastQC `super(...)` Investigation

Target file:

- `/home/mahogny/github/claude/java2rust/testdata/FastQC/uk/ac/babraham/FastQC/Utilities/MultiMemberGZIPInputStream.java`

Current unresolved shape:

```java
private MultiMemberGZIPInputStream(MultiMemberGZIPInputStream parent)
    throws IOException {
    super(parent.in);
    this.size = -1;
    this.parent = parent.parent == null ? parent : parent.parent;
    this.parent.child = this;
}
```

Actions:

1. Add a minimized fixture for a no-state external superclass constructor
   followed by normal direct field assignments.
2. Add a second fixture for nested object-field mutation after `super(...)`,
   such as `this.parent.child = this`.
3. Inspect whether the existing constructor trailing-statement planner can
   lower representable trailing statements after a no-state external
   superclass call.
4. If direct trailing statements are safe, generalize that path.
5. If the nested self-linking assignment requires an object identity or handle
   model, emit a precise unsupported diagnostic for self-referential constructor
   graph mutation instead of the broad `super(...) requires inheritance
   lowering` message.

Acceptance:

- Representable constructor bodies after no-state JDK `super(...)` no longer
  fall back to the broad inheritance placeholder.
- The `MultiMemberGZIPInputStream(parent)` constructors either translate
  correctly or fail with a precise object-identity diagnostic.
- No self-referential Rust ownership is invented just to clear the placeholder.

Verification:

```sh
cargo test <new fixture name> --test idiom_gaps --quiet
cargo test --test idiom_gaps --quiet
target/release/new-java2rust /home/mahogny/github/claude/java2rust/testdata/FastQC/uk/ac/babraham/FastQC/Utilities/MultiMemberGZIPInputStream.java > /tmp/new_java2rust_multimember_gzip_check.rs
rg -N 'super\(\.\.\.\) requires inheritance lowering|self-referential|object graph' /tmp/new_java2rust_multimember_gzip_check.rs
```

## Phase 3: Reduce The Next FastQC Invalid-Pattern Families

Work these in order, one fixture and one shared fix at a time:

1. Remaining `super(...)` placeholders outside stream wrappers.
2. Guarded downcasts and `instanceof` narrowing.
3. Interface stubs that should become explicit traits, facades, or unsupported
   boundary diagnostics.
4. Optional array indexing and optional array length.
5. Duplicate support re-exports and facade/static constant gaps.
6. Concrete `None` assigned into non-optional storage.

After each family:

```sh
cargo fmt --check
cargo check --quiet
cargo test --test idiom_gaps --quiet
```

Then rerun the smallest affected FastQC subset or single file before rerunning
the full focused FastQC package.

## Phase 4: Boundary Corpuses And FFI Handoff Shapes

Keep the agreed split:

- Hadoop configuration, Spark, and Google Cloud Storage are noop facades for
  now.
- DISQ/Spark distributed execution is a facade or handoff boundary, not a
  mechanical Java-to-Rust translation target.
- GenomicsDB native behavior is not a `javalibs` facade. It should become an
  FFI handoff shape whose signatures can be supplied by the future C/C++
  transpiler or explicit ABI facts.
- Picard is a corpus to translate, not a `javalibs` facade.
- Barclay remains maybe, only if a high-quality mapping is available.

Next fixtures:

1. Spark/DISQ interface bridge shape around `Reference*SparkSource.getBases`.
2. GenomicsDB overloaded native call handoff, including stable generated names.
3. Noop configuration facade calls for Hadoop/Spark/GCS where generated code
   only needs a typed boundary object.

Acceptance:

- Native/distributed operations lower to explicit handoff calls or noop facade
  shapes.
- The Java translator does not encode native GenomicsDB behavior or Spark
  distributed semantics.
- Generated FFI handoff call sites have stable names and argument/return slots
  that a future C/C++ transpiler can fill.

## Phase 5: Shared Generated-Rust Error Families

Use the now-unblocked ndarray corpuses to find repeated idioms:

- `htsjdk-vcf`
- `trimmomatic`
- `biojava-aaprop`
- `biojava-alignment`
- `jaligner`
- `jahmm`

Group and fixture the highest-frequency errors:

1. Array creation and 2D array/`ndarray` construction.
2. Switch scaffold forms.
3. Java string, file, logger, and assertion facade gaps.
4. Collection/map spelling variants.
5. Trait/interface assembly and enum-to-trait coercion.
6. Remaining keyword identifier leaks outside normal emitted identifiers.

Acceptance:

- Each new idiom has a minimized test in `tests/idiom_gaps.rs`,
  `tests/parser_gaps.rs`, or a focused design test.
- Fixes are corpus-agnostic unless the behavior is explicitly declared through
  `javalibs`.
- Corpus reruns show either fewer invalid patterns or movement from invalid
  generated Rust to ordinary type errors.

## Phase 6: Performance Guardrail

The current release-mode corpus speed is acceptable for correctness work.
Continue performance improvements only when they are low risk:

- keep release/profile builds as the normal corpus mode;
- avoid unnecessary cloning;
- add indices for repeated fact, member, and usage lookups;
- preserve live phase reporting.

Do not revive the rejected worklist signature solver without equivalence tests
against the current saturation semantics.

## Documentation Updates

After each completed slice, update:

- `docs/current-corpus-issue-audit.md` with exact command output directories,
  invalid-pattern counts, and generated-Rust error counts;
- `docs/corpus-fix-next-plan.md` with completed status and the next chosen
  family;
- this file if the execution order changes.

## Stop And Replan Conditions

- A local gate regresses without a minimized fixture.
- A proposed fix requires adding GATK/HTSJDK/HDF5/SQLite/Arrow/Spark-specific
  behavior to `src/stdlib.rs`.
- A generated-Rust fix depends on guessing from rendered Rust type strings
  rather than explicit planner/facade facts.
- A corpus boundary would require translating true native behavior or
  distributed execution semantics in this Java translator.
- A solver or performance optimization changes ownership, nullability, or
  borrow propagation semantics.
