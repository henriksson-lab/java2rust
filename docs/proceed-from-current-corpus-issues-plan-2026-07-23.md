# Proceed From Current Corpus Issues Plan

Date: 2026-07-23

This is the active execution plan from the current checkout. It assumes the
translator itself no longer crashes on the main focused corpuses, and that the
next useful work is to reduce generated-Rust blockers by converting exposed
corpus failures into focused idiom tests.

## Current Baseline

- Keep using `docs/current-corpus-issue-audit.md` as the canonical running
  audit.
- Preserve the local green gate baseline:
  - `cargo fmt --check`
  - `cargo check --quiet`
  - `cargo test --test idiom_gaps --quiet`
  - `cargo test --test parser_gaps --quiet`
  - `cargo test --test batch_translation --quiet`
  - `cargo test --test type_coercion --quiet`
  - `cargo test --test idiom_sweep_script --quiet`
  - `cargo build --release --quiet --bin new-java2rust`
- Use `--jobs 10` for focused corpus reruns unless a failure requires smaller
  isolation.
- Treat generated invalid-pattern rows as higher priority than ordinary Rust
  type errors, because they usually indicate missing lowering semantics.

## Immediate Slice

### 1. Finish Lowercase JDK Static Owner Recovery

Status: done in focused tests and FastQC statistics subset; pending full
FastQC proof.

Why:

- Latest FastQC/VCF generated Rust still exposes lowercase JDK static-owner
  calls such as `math.pow`, `math.sqrt`, `double.is_infinite`,
  `float.compare`, `system.arraycopy`, and `integer.value_of`.
- This should be fixed through canonical JDK facade facts, not by restoring
  broad simple-name static fallback.

Work:

1. Keep the negative rule that an unproven lowercase receiver like `in.read()`
   remains an instance receiver.
2. Allow lowercase aliases only when canonical JDK/static facade facts prove
   the owner and method, e.g. `math` -> `Math`, `double` -> `Double`,
   `system` -> `System`.
3. Add/keep minimized fixtures for:
   - `Math.pow`, `Math.sqrt`
   - `Double.isInfinite`
   - `System.arraycopy`
   - negative `foo.toString()` and `in.read()` receiver cases
4. Verify support generation contains usable Rust surfaces for the recovered
   static calls. If generated crates emit `System::arraycopy` or
   `Double::is_infinite`, the shared support must define those methods.

Proof:

- Focused tests for the new fixture and call-plan shape.
- `cargo check --quiet`.
- `cargo test --test idiom_gaps --quiet`.
- A focused FastQC subset or full rerun proving the lowercase rows are gone.

Verified:

- `lowercase_jdk_static_owner_aliases_recover_canonical_static_calls`
- `call_plan_recovers_normalized_static_utility_owner_receivers`
- `jdk_numeric_and_decimal_format_facades_keep_precise_return_shapes`
- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet` with 351 passing tests
- `cargo build --release --quiet --bin new-java2rust`
- `.tmp/corpus-fastqc-statistics-after-lowercase-jdk-static-support`: 2/2
  translated, 0 invalid-pattern files, generated `Math::sqrt`/`Math::pow` and
  emitted `Math` support, no lowercase `math.` rows.
- `.tmp/corpus-fastqc-statistics-after-array-length-cast`: 2/2 translated, 0
  invalid-pattern files, offline `cargo check` ok after compound primitive
  array allocation lengths started casting the whole rendered expression.

### 2. Re-run FastQC After The Static-Owner Slice

Status: pending.

Why:

- FastQC is the broadest active corpus and currently has the most exposed
  generated-Rust failures.
- Latest complete evidence is
  `.tmp/corpus-batch-current-fastqc-after-object-stream-field-fixes`:
  156/156 translated, 20 invalid-pattern files, 2733 generated Rust errors.
- Parser-only reruns after comment/hex literal fixes completed translation but
  did not produce a final summary.

Work:

1. Rebuild the release translator.
2. Run a focused FastQC rerun with `--jobs 10`.
3. If the full wrapper hangs in compile/reporting, preserve the translation
   artifacts and run targeted generated-source scans manually.
4. Update `docs/current-corpus-issue-audit.md` with:
   - output directory;
   - translated/failed file counts;
   - invalid-pattern counts;
   - top generated-Rust families;
   - whether lowercase JDK owner rows remain.

### 3. Convert The Next FastQC Families Into Fixtures

Status: pending.

Priority order after the static-owner and FastQC statistics allocation-length
slices:

1. Optional array indexing/length recovery beyond primitive allocation repeat
   counts.
2. Remaining precise `super(...)` placeholders.
3. Object-graph mutation placeholders, especially parent/child link shapes that
   need an explicit handle/state model.
4. GUI erased-object method calls.
5. Static constants and facade gaps that belong to JDK platform facts.

Acceptance:

- Each fixed family gets a minimized `tests/idiom_gaps.rs` fixture before or
  with the implementation.
- No package-specific FastQC table should be added to `src/stdlib.rs`.
- Remaining unsupported behavior should be explicit and narrow, not broad
  placeholder text.

## Parallel Investigation Batch

Use parallel workers for read-only investigation and fixture proposals. Merge
implementation in this checkout after each worker returns a minimal diagnosis.

| Worker | Corpus | Question | Expected Output |
|---|---|---|---|
| A | FastQC | What is the next largest invalid-pattern family after lowercase JDK owner recovery? | exact files, representative generated snippets, fixture proposal |
| B | HTSJDK VCF | Which generated-Rust families block cargo most: constants, enum/version facades, `HashSet<VCFHeaderLine>`, string indexing, or `super(...)`? | ranked family list and fixture proposals |
| C | Spark/DISQ | Can the remaining errors be reduced to interface bridge/noop facade shapes, excluding true distributed execution? | fixture proposal and facade/non-facade split |
| D | GenomicsDB | Which errors are pure Java handoff/overload naming versus native behavior that should wait for C/C++ FFI handover? | overload fixture proposal or explicit defer note |
| E | Ndarray corpuses | Which 2D-array/matrix failures remain in Trimmomatic, BioJava, JAligner, and JAHMM? | common ndarray/array fixture candidates |

Do not let parallel workers edit the same source files at once. Their job is to
return audits, snippets, and minimized test ideas.

## Broader Corpus Order

After FastQC static-owner and top invalid-pattern fixes:

1. `htsjdk-vcf`
   - prioritize enum/facade constants, `HashSet` trait bounds, concrete
     `instanceof`, string index coercions, and remaining `super(...)`.
2. `gatk-disq-spark-datasources`
   - use noop Spark/Hadoop/GCS facades only for local API handoff;
   - do not mechanically translate distributed Spark execution semantics.
3. `gatk-genomicsdb`
   - keep GenomicsDB out of `javalibs` as a normal Rust facade;
   - translate Java-side FFI handoff state and defer native behavior to the
     future C/C++ transpiler path.
4. Matrix-heavy corpuses
   - continue lowering mechanical `Vec<Vec<T>>` shapes to `ndarray` when the
     Java shape is rectangular or semantically matrix-like;
   - keep ragged Java arrays as nested vectors.
5. Remaining Tier 2 corpuses
   - group errors by idiom before implementation: switch scaffolding,
     collection spelling, file/string/logging facades, trait/interface
     assembly, raw keyword leaks, and array creation placeholders.

## Non-Mechanical Boundaries

These should not be treated as ordinary Java-to-Rust mechanical translation:

- true Spark/DISQ distributed execution planning;
- true GenomicsDB native storage/query behavior;
- Picard as a whole-library facade;
- DISQ as a whole-library facade;
- native C/C++ behavior behind Java APIs.

For those cases, translate only the Java-side state and calls needed to hand
off to explicit FFI/runtime boundaries. The C/C++ transpiler is expected to run
first in that path and provide signatures the Java translator can target.

## Performance Constraint

The current speed is good enough for corpus-driven idiom work. Do not make
algorithmic rewrites that interfere with future global semantic analysis. If
performance work resumes, prefer:

- indices over repeated global scans;
- removal of unnecessary cloning;
- profile builds as the default for corpus runs;
- preserving the existing semantic fixed-point behavior with equivalence tests.

## Verification Protocol

For each implementation slice:

1. Add or update the focused idiom fixture.
2. Run the new focused test.
3. Run `cargo fmt --check`.
4. Run `cargo check --quiet`.
5. Run `cargo test --test idiom_gaps --quiet`.
6. Run any directly relevant secondary test suite.
7. Rebuild release with `cargo build --release --quiet --bin new-java2rust`.
8. Rerun the affected focused corpus with `--jobs 10`.
9. Update `docs/current-corpus-issue-audit.md` with exact evidence.
