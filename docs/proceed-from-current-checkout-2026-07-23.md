# Proceed From Current Checkout Plan

Date: 2026-07-23

This plan starts from the current dirty checkout and the latest summarized test
state. Its purpose is to finish the current corpus-fix slice without losing the
larger design goal: stable callable/member identities, fact-backed facade
behavior, and corpus fixes expressed as reusable translator idioms.

## Starting Point

Recent local work added fixes or fixtures for:

- lower-case Java receiver names that collide with Rust static-owner rendering,
  e.g. `in.read()` must not become `in::read()`;
- parenthesizing raw casts before ordered comparisons, e.g.
  `(value.len() as i32) < limit`;
- parsing Java numeric literal suffixes and radix forms such as `2.5f`,
  `10.0D`, and `0x...L`.

The last broad verification did not finish green:

- `cargo check --quiet` passed;
- `cargo test --test batch_translation --quiet` passed;
- `cargo test --test type_coercion --quiet` passed;
- `cargo fmt --check` failed only on formatting in `src/emit.rs` and
  `src/parser.rs`;
- `cargo test --test idiom_gaps --quiet` failed two tests:
  - `byte_and_char_buffer_put_are_modeled_statement_facade_calls`;
  - `corpus_disq_external_chain_stays_unmodeled_boundary_without_self_recursion_warning`.

The latest useful FastQC corpus evidence remains
`.tmp/corpus-batch-current-fastqc-after-nested-interface-names`:

- 156/156 translated;
- 0 translation failures;
- invalid-pattern files reduced to 30;
- `.put` invalid patterns are gone;
- remaining invalid-pattern counts are 58 `compile_error!` placeholders across
  24 files, 40 Java cast downcasts across 14 files, and 1 concrete `None`
  assignment;
- offline cargo reaches generated Rust diagnostics with 3236 errors.

## Work Rules

1. Keep each corpus issue tied to a minimized enabled fixture.
2. Prefer planner, fact, slot, signature, coercion, and lowering fixes over
   emitted-string patches.
3. Put package-specific behavior in `javalibs` manifests/shims, not in
   `src/stdlib.rs`, unless it is a stable JDK rule.
4. Preserve the global semantic-analysis direction. Performance improvements
   should remove clone churn, repeated scans, and missing indexes without
   fragmenting future whole-program analysis.
5. Treat GenomicsDB native behavior and DISQ/Spark distributed execution as
   explicit FFI/handoff boundaries, not mechanically translated local behavior.

## Phase 1: Restore Local Gates

Status: done.

Actions:

1. Update the `ByteBuffer`/`CharBuffer` fixture to accept the normalized numeric
   literal rendering produced by the Java literal parser. The new output is
   semantically acceptable if it still evaluates the facade argument and leaves
   no `.put(...)` call behind.
2. Fix lower-case external static facade owners without reopening arbitrary
   lower-case static-owner guessing. The intended rule is:
   - fact-proven external facade/static owners such as
     `htsjdk_variants_rdd_storage.makeDefault(...)` and
     `tabix_index_write_option.fromBoolean(...)` may recover their canonical
     Rust owner calls;
   - ordinary lower-case receivers such as `in.read()` remain instance
     receivers unless imports/source summaries/facts prove otherwise.
3. Run `cargo fmt`.
4. Verify:
   - `cargo fmt --check`;
   - `cargo check --quiet`;
   - `cargo test --test idiom_gaps --quiet`;
   - `cargo test --test batch_translation --quiet`;
   - `cargo test --test type_coercion --quiet`.

Acceptance:

- Local gates are green.
- `DisqExternalBoundaryChain` renders
  `HtsjdkVariantsRddStorage::make_default(...)` and
  `TabixIndexWriteOption::from_boolean(...)`.
- The `in.read()` fixture still renders as a raw-identifier instance call, not
  `in::read`.

Verified:

- `cargo fmt --check`;
- `cargo check --quiet`;
- `cargo test --test parser_gaps --quiet`;
- `cargo test --test idiom_gaps --quiet` with 311 passing tests;
- `cargo test --test batch_translation --quiet`;
- `cargo test --test type_coercion --quiet`;
- `cargo build --release --quiet --bin new-java2rust`.

## Phase 2: Rerun FastQC After The New Parse Fixes

Status: partial evidence collected.

Actions:

1. Build the release translator:
   - `cargo build --release --quiet --bin new-java2rust`.
2. Rerun FastQC with the 10-thread default:
   - `scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 --translate-timeout-sec 60 --offline-cargo --primary /home/mahogny/github/claude/java2rust/testdata/FastQC --out .tmp/corpus-batch-current-fastqc-after-keyword-cast-literals`
3. Check whether the following blockers disappeared or decreased:
   - `in::read`;
   - `as i32 <` / cast-comparison parse errors;
   - Java decimal float suffixes and hex integer long suffixes.
4. Record new invalid-pattern counts, top compile errors, and any remaining
   parse blockers.

Acceptance:

- FastQC still reaches emission and cargo diagnostics without translator panic.
- The new parse/literal fixes have measurable corpus impact.
- `docs/current-corpus-issue-audit.md` is updated with the exact output path
  and counts.

Evidence:

- `.tmp/corpus-batch-current-fastqc-after-keyword-cast-literals` completed
  translation and crate assembly:
  - 156/156 translated;
  - 0 failed files;
  - no translator panic;
  - global analysis 614130 ms;
  - signature fixpoint 577545 ms;
  - emission 66962 ms;
  - crate assembly 207014 ms.
- Targeted scans show:
  - `in::read` static-owner renderings: 0;
  - raw `as i32 <` cast-comparison renderings: 0;
  - Java block-comment condition artifacts: 1;
  - Java hex floating literals: 2;
  - Java hex long suffix masks before the later parser patch: 24.
- The wrapper was interrupted after crate assembly during/after cargo reporting,
  so no final `summary.tsv` exists for this run. `compile.log` exists and
  contains 80 reported errors headed by `if /*!encode &&*/`, then duplicate
  re-export names.
- After this partial run, local/focused fixes landed for the block-comment
  condition, `0x8000000000000000L` as a Java two's-complement long literal, and
  Java hex floating literals. Focused Base64, FastMath, and
  BitsStreamGenerator translations prove those exact source shapes no longer
  emit the old raw spellings. A fresh FastQC rerun is still needed for full
  corpus proof.

## Phase 3: Fix The Next FastQC Families

Status: pending after the rerun.

Candidate families after the next FastQC rerun:

- remaining `super(...)` placeholders;
- guarded Java downcasts and `instanceof` forms;
- optional array indexing/length;
- interface stubs;
- duplicate support re-export names after crate assembly;
- facade/static constant gaps;

Execution loop:

1. Pick the largest remaining family from the newest FastQC evidence.
2. Add one minimized fixture for the idiom shape.
3. Fix the shared translator path.
4. Run the local gates from Phase 1.
5. Rerun FastQC only after a meaningful family is fixed.

Acceptance:

- FastQC invalid-pattern files and parse blockers continue to decrease.
- No new package-specific non-JDK table is added to `src/stdlib.rs`.

## Phase 4: Resume Other Corpus Families

Status: pending after FastQC has a clearer top-error profile.

Priority order:

1. `htsjdk-vcf`: minimize shared-support enum/display assembly around
   `VCFCompoundHeaderLineSupportedHeaderLineType` so the generated crate parses.
2. `gatk-disq-spark-datasources`: add interface bridge and borrowed/owned
   handoff fixtures while keeping Spark/DISQ distributed behavior as an explicit
   facade boundary.
3. `gatk-genomicsdb`: fix overload naming and FFI handoff signature shapes only;
   do not add GenomicsDB semantic behavior to `javalibs`.
4. Ndarray-heavy corpuses: continue lowering precise rectangular
   `Vec<Vec<T>>`/2D primitive arrays to `ndarray`, while preserving nested
   vectors for ragged or independently mutated rows.
5. BioJava/JAligner/JAHMM/Trimmomatic shared idioms: array creation,
   switch scaffolding, Java string/file/logging facades, keyword leaks outside
   normal emit paths, collection/map surfaces, and trait/interface assembly.

Acceptance:

- Each corpus-family fix has enabled fixtures.
- Focused reruns move failures from parse/invalid patterns toward ordinary
  generated-Rust type diagnostics, then reduce those diagnostics.

## Phase 5: Return To Fact-Unification Work

Status: pending after the current corpus blockers are less noisy.

The original design plan remains active:

- stable `CallableKey` and member identities;
- typed external field facts;
- non-JDK package knowledge moved into `javalibs`;
- stream lowering through shared call/member/coercion plans;
- explicit `FacadeAbiFact` behavior instead of Rust type-string heuristics.

Next design slices:

1. Finish migrating external API effects, argument effects, return facts,
   constructor facts, and field facts to stable callable/member keys.
2. Add negative tests proving same simple names in different packages do not
   match a `javalibs` fact accidentally.
3. Move remaining GATK/HTSJDK/HDF5/SQLite/Arrow-specific behavior out of
   core translator code where it still exists.
4. Make stream method references and lambda member/call lowering use the same
   planned call/member/slot conversions as ordinary expressions.
5. Replace facade ABI string-shape guesses with explicit receiver, argument,
   return, result, borrow, and exception facts.

Acceptance:

- Source analysis, external packs, user overrides, `javalibs` shims, and
  fallback guesses attach facts to the same callable/member identities.
- New non-JDK behavior normally requires editing `javalibs`, not
  `src/stdlib.rs`.

## Standard Verification Commands

Use after each translator slice:

```sh
cargo fmt --check
cargo check --quiet
cargo test --test idiom_gaps --quiet
cargo test --test batch_translation --quiet
cargo test --test type_coercion --quiet
```

Use before focused corpus reruns:

```sh
cargo build --release --quiet --bin new-java2rust
```

Use focused corpus reruns with 10 jobs by default:

```sh
scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 \
  --translate-timeout-sec 60 --offline-cargo \
  --primary <java-corpus-path> --out .tmp/<descriptive-output-name>
```
