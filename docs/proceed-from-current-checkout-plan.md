# Proceed From Current Checkout Plan

Date: 2026-07-22

This plan starts from the current dirty checkout. It is intentionally narrower
than `docs/current-corpus-fix-execution-plan.md`: first stabilize the in-flight
string API slice, then rerun the focused corpuses and select the next idiom
batch from real generated-Rust failures.

## Current State

- Local gates were previously green after the output/exception logging slice:
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test idiom_gaps --quiet`,
  `cargo test --test batch_translation --quiet`, and
  `cargo build --release --quiet --bin new-java2rust`.
- VarScan after output/exception fixes:
  `.tmp/corpus-batch-current-varscan-after-exception-runtime-wrapper`,
  18/18 translated, 0 invalid-pattern files, offline `cargo check` failed as
  `generated_rust` with 267 errors.
- A Java string API normalization slice is currently in progress:
  `String.toUpperCase()`, `String.replace(...)`, and `String.toCharArray()`.
- The focused test
  `java_string_uppercase_char_array_and_replace_use_rust_string_apis` currently
  fails only because `replace` arguments still render as
  `"X".to_string().as_str()` instead of borrowed literals. The generated
  `to_uppercase()` and `chars().collect::<Vec<char>>()` shapes are present.

## Phase 1: Finish The String API Slice

Actions:

1. Fix string literal argument coercion for native `String.replace(...)`.
   Prefer correcting the slot/coercion plan so borrowed string arguments stay
   borrowed literals, not a rendered-string post-pass.
2. Keep the rule scoped to Java string/native string-like calls and shared
   string pattern argument lowering. Do not add VarScan-specific logic.
3. Verify:
   - `cargo fmt --check`
   - `cargo test java_string_uppercase_char_array_and_replace_use_rust_string_apis --test idiom_gaps --quiet`
   - `cargo check --quiet`
   - `cargo test --test idiom_gaps --quiet`
   - `cargo test --test batch_translation --quiet`

Acceptance:

- The new fixture passes and cargo-checks the generated module.
- Generated `replace` calls do not require owned temporary `String` values for
  literal or already-borrowable pattern arguments.

## Phase 2: Rerun VarScan And Classify The Delta

Actions:

1. Rebuild the release translator:
   `cargo build --release --quiet --bin new-java2rust`.
2. Rerun VarScan with the local wrapper:
   `scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 --translate-timeout-sec 60 --offline-cargo --primary /home/mahogny/github/claude/java2rust/testdata/varscan --out .tmp/corpus-batch-current-varscan-after-string-api-normalization`.
3. Compare against the 267-error baseline:
   - confirm `to_upper_case` and `to_char_array` are gone;
   - confirm `String: Pattern` errors from `replace` are gone or reduced;
   - record the new top generated-Rust categories.

Acceptance:

- VarScan still translates 18/18 with 0 invalid-pattern files.
- The corpus issue audit points to the new run directory and error count.

## Phase 3: Pick The Next Shared Idiom Batch

Use the post-string VarScan data plus existing JAligner, BioJava, JAHMM,
Trimmomatic, and FastQC data. Favor fixes that appear across corpuses and have
small, general fixtures.

Expected next candidates:

| Family | Why It Matters | Likely Layer |
|---|---|---|
| Java `File`/stream facade methods such as `exists` and `OutputStream` | Still appears in VarScan and other file-heavy code | JDK facade facts/support |
| Java collection conversion methods such as `toArray` | Common generated-Rust blocker and easy to fixture | collection call planning |
| String comparison and borrowed/owned equality | Many `String` vs `&String` diagnostics | slot conversion/coercion |
| Option arithmetic and indexing after map/get flows | Shows up as `Option<&i32>` arithmetic and index use | nullable refinement and numeric/index coercion |
| Temporary-reference returns | High VarScan count and likely shared ownership issue | return slot planning |
| `()` flowing into display/numeric contexts | Indicates unresolved facade calls selected as void | call target/facade return facts |

Acceptance:

- Each chosen family gets at least one minimized `tests/idiom_gaps.rs` fixture
  before or alongside implementation.
- The fixture name describes the Java idiom, not the corpus class.

## Phase 4: Parallel Corpus Audits

Run parallel workers for audit-only work while keeping semantic edits in this
checkout. The workers should not make production changes unless explicitly
assigned a narrow branch/worktree.

Worker split:

| Worker | Input | Output |
|---|---|---|
| VarScan/JAHMM | latest generated-Rust logs | top shared scalar/string/option idioms |
| JAligner/BioJava | ndarray and numeric-heavy logs | matrix, index, and numeric coercion idioms |
| Trimmomatic/FastQC | invalid-pattern and generated-Rust logs | array creation, switch, collection, and trait/interface idioms |
| GATK boundary corpuses | existing Picard/GenomicsDB/Spark/DISQ fixtures | facade vs FFI vs no-translate classification |

Rules:

- Parallel workers may inspect logs, minimize examples, and propose fixtures.
- Core translator edits should remain centralized in this checkout when they
  touch shared lowering, semantic analysis, or `javalibs`.
- Do not make algorithm changes that interfere with future global semantic
  analysis. Unneeded clone removal and lookup indices are acceptable if covered
  by equivalence tests.

## Phase 5: Refresh Tracking Docs

After each accepted idiom slice:

1. Update `docs/current-corpus-issue-audit.md` with exact run directories,
   translated counts, invalid-pattern counts, and top generated-Rust categories.
2. Update `docs/current-corpus-fix-execution-plan.md` only when priorities
   change.
3. Add a short progress note to `docs/corpus-fix-next-plan.md` with tests and
   corpus evidence.
4. Keep boundary decisions aligned:
   - Hadoop configuration, Spark, and Google Cloud Storage stay noop facades
     for now.
   - GenomicsDB native behavior stays out of `javalibs` and becomes FFI
     handoff shape.
   - DISQ/Spark distributed execution is not mechanically translated by this
     Java translator.
   - Barclay remains maybe, only if a high-quality mapping exists.
   - Picard remains a corpus to translate, not a `javalibs` facade.

## Stop Conditions

Stop and re-plan if:

- a local green gate regresses without a clear fixture-level explanation;
- a proposed fix needs non-JDK package tables in `src/stdlib.rs`;
- a worker proposes modeling native/distributed runtime behavior instead of a
  facade or FFI handoff;
- a performance change alters nullable/borrowed/owned propagation semantics.
