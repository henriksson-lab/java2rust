# Corpus Fix Proceed Plan

Date: 2026-07-22

This is the current execution plan for continuing the corpus-guided idiom work
from the dirty checkout. It is intentionally narrower than the long-term
callable-key/facade design plan: the job here is to keep the translator green
locally while reducing real corpus failures with minimized fixtures.

## Baseline

Primary status documents:

- `docs/current-corpus-issue-audit.md`
- `docs/current-corpus-fix-execution-plan.md`
- `docs/current-goal-proceed-plan.md`
- `docs/corpus-fix-next-plan.md`

Green local gates from the latest stable point:

- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet`
- `cargo test --test batch_translation --quiet`
- `cargo build --release --quiet --bin new-java2rust`

Clean focused corpuses:

- `picard-programgroups`
- `htsjdk-fastq`

Current best generated-Rust target:

- VarScan:
  `.tmp/corpus-batch-current-varscan-after-compound-index-cast`
  translates 18/18 files, has 0 invalid-pattern files, and fails offline
  `cargo check` with 84 generated-Rust errors.

The old VarScan blockers for `super(...)`, Rust keyword identifiers,
`Out::`/`Err::`, output/error printing, Java exception logging, Java string API
helpers, temporary string-reference returns, and `String`/`&String` comparison
are gone.

## Execution Rules

- Add a minimized fixture before each translator fix.
- Keep local gates green after each slice.
- Prefer shared planner, shape, coercion, facade, or lowering fixes over
  corpus-specific generated-code patches.
- Do not add non-JDK package behavior to `src/stdlib.rs`.
- Put package/facade facts in `javalibs` when the behavior is library-specific.
- Keep GenomicsDB as an FFI handoff boundary, not a semantic Java facade.
- Keep Spark/DISQ distributed execution as facade/handoff behavior, not
  mechanical distributed-runtime translation.
- Use parallel workers for independent audits and corpus reruns, but make
  shared core and `javalibs` edits in this checkout to avoid merge churn.

## Phase 1: VarScan JDK Facade Batch

Status: done.

Why:

- VarScan has no invalid-pattern files, so compiler errors are now clean signals.
- The largest remaining easy families are shared JDK facade gaps, not
  VarScan-specific behavior.

Current top signals from
`.tmp/corpus-batch-current-varscan-after-string-equality-all-strings/top-compile-errors.tsv`:

| Count | Signal | Likely cause |
|---:|---|---|
| 11 | ``()` doesn't implement Display` | formatter/facade calls planned as void |
| 9 | wrong arity | static Java methods emitted as support instance methods |
| 4 | `File.exists` missing | JDK file facade gap |
| 2 | `()` as `f64` | numeric facade calls planned as void |
| 2 | `!` on `()` | boolean facade call planned as void |
| 1 | `OutputStream` missing | JDK stream facade gap |

Actions:

1. Add a fixture for JDK numeric/text facade calls:
   - `Math.log`
   - `Math.exp`
   - `Double.isNaN`
   - `DecimalFormat.format`
2. Plan those through explicit native Java call targets instead of generated
   support methods with implicit receivers.
3. Add or verify return facts:
   - `Math.log(double) -> double`
   - `Math.exp(double) -> double`
   - `Double.isNaN(double) -> boolean`
   - `DecimalFormat.format(...) -> String`
4. Rerun:
   - focused fixture test
   - `cargo test --test idiom_gaps --quiet`
   - `cargo check --quiet`
   - focused VarScan into a new `.tmp/corpus-batch-current-varscan-after-*`
     directory
5. Update the audit docs with new error counts.

Acceptance:

- No `Math::log`, `Math::exp`, or `Double::is_na_n` support methods that require
  an accidental receiver.
- `DecimalFormat.format` produces a string-shaped Rust expression.
- VarScan error count drops or the remaining errors clearly move to the next
  families.

Verified:

- Added fixtures:
  - `jdk_numeric_and_decimal_format_facades_keep_precise_return_shapes`
  - `jdk_file_reader_and_system_facades_keep_control_flow_shapes`
  - `jdk_bitset_get_keeps_boolean_shape`
- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet` with 285 passing tests
- `cargo test --test batch_translation --quiet`
- `cargo build --release --quiet --bin new-java2rust`
- VarScan rerun:
  `.tmp/corpus-batch-current-varscan-after-jdk-facades-bitset`
  translates 18/18, reports 0 invalid-pattern files, and fails offline
  `cargo check` as `generated_rust` with 119 errors. The JDK facade rows for
  `Math.log`, `Math.exp`, `Double.isNaN`, `DecimalFormat.format`,
  `File.exists`, reader `ready`, `System.exit`, and `BitSet.get` are gone from
  the current top-error list.

## Phase 2: VarScan Container And Coercion Batch

Status: partially done.

Do this from the
`.tmp/corpus-batch-current-varscan-after-jdk-facades-bitset` baseline.

Actions:

1. Add fixtures for:
   - `List<String>.toArray(...)` and no-arg `toArray` style conversions;
   - `Map.get(...)`/optional numeric values used in arithmetic;
   - Java integer indices applied to Rust slices/vectors;
   - moved string values reused after insertion, concatenation, or map access.
2. Fix through shared plans:
   - collection conversion planning;
   - optional unwrapping/defaulting only when Java code proves presence;
   - index coercion to `usize`;
   - borrow/clone policy for reused Java `String` values.
3. Rerun VarScan and update the docs.

Acceptance:

- `to_array` is not emitted as a missing Rust method.
- generated slice/vector indices are `usize` where Rust requires it.
- optional arithmetic does not operate directly on `Option<&T>`.
- clone/borrow recovery is explicit rather than relying on moved values.

Progress:

- Collection `toArray` is done for direct list conversion and casted
  `map.keySet().toArray(new String[0])`.
- Compound Java integer array-index expressions are done; the renderer now
  emits `(a + b) as usize` rather than `a + b as usize`.
- Added/updated fixture:
  - `collection_to_array_empty_string_array_lowers_to_owned_vec`
  - `java_int_arithmetic_array_index_casts_whole_expression_to_usize`
  - `buffered_reader_read_line_assignment_to_initialized_optional_wraps_result_value`
- Verified:
  - focused fixture test
  - `cargo fmt --check`
  - `cargo check --quiet`
  - `cargo test --test idiom_gaps --quiet` with 287 passing tests
  - `cargo test --test type_coercion --quiet`
  - `cargo test --test batch_translation --quiet`
  - `cargo build --release --quiet --bin new-java2rust`
  - VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-compound-index-cast`
    translates 18/18, reports 0 invalid-pattern files, and fails offline
    `cargo check` as `generated_rust` with 84 errors. The `to_array`, `i32 +
    usize`, `i32 - usize`, and indexed-by-`i32` rows are gone from the current
    top-error list.
- Remaining in this phase:
  - optional numeric values used in arithmetic;
  - moved string values reused after insertion, concatenation, or map access.

## Phase 3: FastQC Invalid-Pattern Batch

FastQC now reaches normal post-processing, so use its invalid-pattern list as a
fixture source.

Current evidence:

- `.tmp/corpus-batch-current-fastqc-after-scope-kind-skip-long`
- 156/156 files translated.
- no duplicate-owner panic.
- invalid scan reports 45 affected files.

Order:

1. Remaining `super(...)` constructor placeholders beyond the simple cases
   already fixed.
2. Java array creation placeholders.
3. Java collection/map spelling such as `.put`.
4. Guarded casts and `instanceof` narrowing.
5. Concrete `None` assigned into non-optional storage.
6. Switch scaffold cleanup when it blocks parse/typecheck progress.

Acceptance:

- Each family has one package-neutral fixture.
- Fixed families reduce FastQC invalid-pattern counts in a focused rerun.
- Remaining failures are ordinary generated-Rust errors or explicit unsupported
  handoff diagnostics.

## Phase 4: VCF Shared Support Parse Failure

Why:

- VCF is blocked before normal typecheck by generated syntax around shared
  support enum/display assembly.

Actions:

1. Minimize the `VCFCompoundHeaderLineSupportedHeaderLineType` generated support
   failure.
2. Add a fixture for shared enum/display/support type assembly.
3. Fix item assembly so the generated crate parses.
4. Rerun focused `htsjdk-vcf`.

Acceptance:

- VCF gets past Rust parsing.
- Remaining VCF issues become normal typecheck diagnostics or invalid-pattern
  rows.

## Phase 5: Boundary Corpuses

Spark/DISQ:

- Keep Hadoop configuration, Spark, and Google Cloud Storage as noop facades for
  now.
- Add interface/delegation fixtures around `Reference*SparkSource.getBases`.
- Do not model distributed execution in this translator.

GenomicsDB:

- Keep GenomicsDB out of `javalibs` as a behavioral facade.
- Add FFI handoff fixtures for overloaded native calls and generated signature
  stability.
- Let the future C/C++ transpiler provide native signatures when it runs first.
- Fix duplicate generated item names such as overloaded `query` without
  reimplementing GenomicsDB behavior.

Acceptance:

- Spark/DISQ failures are classified as facade/handoff issues rather than Java
  lowering bugs.
- GenomicsDB emits stable FFI handoff shapes and avoids duplicate Rust item
  names.

## Phase 6: Broader Matrix And Math Corpuses

Use parallel workers to audit independent corpuses, but feed fixes back through
the same fixture-first flow in this checkout.

Corpuses to keep in the rotation:

- JAligner
- BioJava AAPROP
- BioJava alignment
- JAHMM
- Trimmomatic
- FastQC

Focus categories:

- 2D array and matrix lowering to `ndarray` when shape is statically clear.
- Java numeric/math APIs.
- switch lowering.
- trait/interface assembly.
- collection conversion and map APIs.
- source-super and inherited initialization variants.

Acceptance:

- Shared idioms are fixed once and verified against at least one real corpus.
- Mechanical 2D array-to-`ndarray` lowering is used only when dimensions and
  element type are clear enough to preserve semantics.

## Phase 7: Performance Guardrails

Current runtime is acceptable for correctness work. Keep profile/release builds
as the normal corpus mode and avoid algorithm changes that compromise global
semantic analysis.

Allowed:

- remove unnecessary cloning;
- add indices for repeated lookups;
- keep live phase reporting;
- use 10 jobs by default for focused corpus runs;
- profile specific long phases when they become a blocker.

Avoid:

- local-only shortcuts that prevent future global semantic analysis;
- solver rewrites without equivalence tests;
- accepting speedups that change nullable, borrowed, or owned propagation.

## Immediate Next Command Sequence

After implementing Phase 1:

```bash
cargo fmt --check
cargo check --quiet
cargo test --test idiom_gaps --quiet
cargo test --test batch_translation --quiet
cargo build --release --quiet --bin new-java2rust
scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 --translate-timeout-sec 60 --offline-cargo --primary /home/mahogny/github/claude/java2rust/testdata/varscan --out .tmp/corpus-batch-current-varscan-after-jdk-facades
```

Then inspect:

- `.tmp/corpus-batch-current-varscan-after-jdk-facades/top-compile-errors.tsv`
- `.tmp/corpus-batch-current-varscan-after-jdk-facades/compile.log`
- `.tmp/corpus-batch-current-varscan-after-jdk-facades/invalid-patterns.tsv`

Update:

- `docs/current-corpus-issue-audit.md`
- `docs/current-corpus-fix-execution-plan.md`
- this plan, if the next priority changes.
