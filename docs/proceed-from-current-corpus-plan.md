# Proceed From Current Corpus Plan

Date: 2026-07-22

## Goal

Finish the current corpus-stabilization slice by turning the remaining corpus
failures into minimized idiom fixtures and then fixing them in the shared
translator or in `javalibs` ownership boundaries.

This plan assumes the broader design direction remains unchanged:

- package-specific behavior belongs in `javalibs`, not `src/stdlib.rs`;
- GenomicsDB native behavior is an FFI handoff boundary, not a Java semantic
  facade;
- Spark/DISQ distributed execution is a facade or handoff boundary, not
  mechanically translated distributed semantics;
- performance improvements must not block future global semantic analysis;
- every fix should have a minimized regression fixture before or alongside the
  translator change.

## Current Baseline

Known local gates from the latest working state:

- `cargo fmt --check` passes.
- `cargo check --quiet` passes.
- `cargo test --test batch_translation --quiet` passes.
- `cargo test --test idiom_gaps --quiet` passes in the latest complete audit;
  rerun after the current raw-identifier edits before declaring the batch done.
- `cargo build --release --quiet --bin new-java2rust` passes.
- `bash -n scripts/translate-focused-package.sh` passes.

Important recent progress:

- Focused generated crates that use `ndarray::` now get a generated local
  `ndarray` shim for offline corpus checks.
- Source-backed `super(args)` constructors can initialize flattened inherited
  fields for simple source superclass constructor bodies.
- JDK `FileReader` superclass construction is modeled as no-state facade
  construction for generated corpus crates.
- Java-derived Rust keyword identifiers now render through raw identifiers for
  ordinary emitted identifier positions. The VarScan rerun no longer reports
  keyword parse errors for `in`.
- The focused invalid-pattern scanner now treats raw optional locals such as
  `let mut r#in: Option<T> = None;` as optional, not concrete `None`
  assignments.

## Corpus State To Start From

| Corpus | Current state | Next use |
|---|---|---|
| `picard-programgroups` | Clean focused run | Keep as regression guard |
| `htsjdk-fastq` | Clean focused run | Keep as regression guard |
| `fastqc` | Full run reaches normal wrapper summary; invalid idioms remain | Mine invalid-pattern families and add fixtures |
| `htsjdk-vcf` | Translates but generated Rust has an unclosed delimiter | Minimize support enum/display assembly |
| `gatk-disq-spark-datasources` | Translates with invalid patterns and type errors around interface/facade shape | Fix noop facade/interface bridge shape |
| `gatk-genomicsdb` | Translates with duplicate overload names and type mismatch | Fix FFI handoff naming/slot shape |
| `varscan` | 18/18 translated, 0 invalid-pattern files after raw identifiers; `cargo check` fails on generated-Rust type/facade errors | Use as type/facade error mining corpus |
| `trimmomatic` | 68/68 translated; generated-Rust errors remain | Mine shared string/file/logging/index idioms |
| `biojava-aaprop` | 33/33 translated; generated-Rust errors remain | Mine switch, map, amino-acid constant assembly idioms |
| `biojava-alignment` | 53/53 translated; generated-Rust errors remain | Mine trait/interface, array creation, cast/instanceof idioms |
| `jaligner` | 29/29 translated; generated-Rust errors remain | Mine integer/index, switch, string/file/logging idioms |
| `jahmm` | 102/102 translated; generated-Rust errors remain | Mine trait/interface, array creation, test/assertion facade idioms |

## Work Plan

### 1. Close The Current Raw-Identifier Batch

Status: in progress.

Actions:

1. Rerun `cargo test --test idiom_gaps --quiet` and record the result.
2. Update `docs/current-corpus-issue-audit.md` and
   `docs/current-corpus-fix-execution-plan.md` with:
   - raw Rust keyword identifier rendering;
   - raw optional-local scanner fix;
   - latest VarScan output path and counts;
   - confirmation that keyword parse errors are gone from VarScan.
3. Check that no translator, wrapper, or cargo processes are left running.

Acceptance:

- Local gates are green after the raw-identifier change.
- VarScan remains at 0 invalid-pattern files.
- Remaining VarScan failures are classified as generated-Rust type/facade
  issues, not Rust parser keyword failures.

### 2. Mine The Highest-Value Shared Generated-Rust Families

Status: next.

Actions:

1. Reuse existing focused outputs before rerunning expensive corpuses.
2. Group diagnostics by fixable translator idiom, not by corpus:
   - temporary references returned from generated expressions;
   - `Err`/`Result` namespace collisions;
   - missing `Out`, `System.err`, `OutputStream`, and print/logging facades;
   - Java string facade gaps such as `Pattern`, case conversion spelling,
     localized messages, and stack traces;
   - `&String` versus `String` comparison and argument slot coercions;
   - index/integer coercions;
   - remaining concrete `None` or optionality drift.
3. For each family, add a minimized test in `tests/idiom_gaps.rs` or
   `tests/batch_translation.rs`.
4. Fix the shared planner/lowering path, then rerun the focused fixture and at
   least one affected corpus.

Acceptance:

- Each new fix has a package-neutral fixture unless the behavior is explicitly
  a `javalibs` facade.
- The error count for at least one non-clean corpus decreases without
  regressing `picard-programgroups` or `htsjdk-fastq`.

### 3. Fix VCF Support Enum/Display Assembly

Status: queued.

Actions:

1. Minimize from `VCFCompoundHeaderLineSupportedHeaderLineType`.
2. Add a fixture for shared support enum generation with nested owner context,
   generated `Display`, and static references if required.
3. Fix support item assembly so braces and impl blocks are balanced.
4. Rerun focused `htsjdk-vcf`.

Acceptance:

- VCF generated crate parses.
- Follow-up VCF failures are ordinary `cargo check` diagnostics or explicit
  invalid-pattern rows.

### 4. Fix FastQC Invalid Idioms

Status: queued.

Actions:

1. Start from `.tmp/corpus-batch-current-fastqc-after-scope-kind-skip-long`.
2. Fixture and fix top invalid-pattern families:
   - `compile_error!` placeholders;
   - Java cast downcasts;
   - map/collection `.put` and related spelling;
   - concrete `None` assignment;
   - remaining `super(...)` and constructor delegation variants;
   - Java array creation placeholders.
3. Keep fixtures package-neutral unless the issue is a true facade decision.
4. Rerun FastQC only after several focused fixture fixes, because it remains
   expensive.

Acceptance:

- FastQC invalid-pattern file count decreases.
- No translator panic returns.

### 5. Fix Spark/DISQ As A Facade Boundary

Status: queued.

Actions:

1. Keep Hadoop configuration, Spark, and Google Cloud Storage as noop facades
   for now.
2. Add an interface/delegation fixture for `Reference*SparkSource.getBases`
   style methods.
3. Ensure generated trait impl signatures use the same planned method and slot
   shapes as the source/delegate methods.
4. Preserve distributed execution as handoff state rather than translating it.

Acceptance:

- The focused Spark/DISQ corpus no longer fails because required trait methods
  are missing or have incompatible borrowed/owned delegate shapes.
- Remaining distributed behavior is explicitly recorded as facade/handoff work.

### 6. Fix GenomicsDB As FFI Handoff

Status: queued.

Actions:

1. Keep GenomicsDB out of `javalibs` as a semantic facade.
2. Add fixtures for Java wrappers around native/FFI state where C/C++
   transpiler-generated signatures are supplied first.
3. Fix duplicate Rust item names for overloaded `query`-style APIs.
4. Route unresolved native behavior to explicit handoff facts or diagnostics.

Acceptance:

- Duplicate generated method names are gone in the minimized fixture.
- Remaining GenomicsDB failures are either Java-side overload/slot bugs or
  explicitly waiting for external FFI signatures.

### 7. Preserve Performance Without Sacrificing Global Analysis

Status: continuous.

Actions:

1. Use profile or release builds for corpus runs by default.
2. Keep the accepted scope-kind skip optimization.
3. Prefer low-risk speedups:
   - reduce unnecessary cloning;
   - add indices for repeated lookups;
   - cache projections that are pure and fact-stable;
   - improve progress reporting for long phases.
4. Avoid algorithm changes that partition away global semantic information or
   weaken cross-method propagation.
5. Use `/data/henriksson/github/claude/rust_refactor` for cleanup such as
   inlining only after generated Rust is in a working state.

Acceptance:

- Correctness work stays fast enough for iterative focused corpus reruns.
- Any solver/index optimization has equivalence tests for nullability and
  borrowed/owned propagation.

## Stop Conditions

Stop and write an audit before continuing if:

- a proposed optimization changes clean corpus output without an explained
  semantic reason;
- a fix requires real Spark distributed execution;
- a fix requires true GenomicsDB native behavior before FFI signatures exist;
- new package-specific behavior would need to be hardcoded in `src/stdlib.rs`;
- `/tmp` or dependency resolution prevents reliable local verification.
