# Focused Batch Translation Speed Plan

## Goal

Make real-code corpus gates fast enough to run routinely without giving up the
global semantic analysis the translator needs for correct Java lowering.

The key constraint is architectural: semantic/source-summary/fact analysis must
be global for the selected translation envelope. Parallelism should happen after
that global state is built, over immutable plans and facts, rather than by
repeating partial semantic analysis in isolated worker processes.

## Current Evidence

- `scripts/translate-focused-package.sh` previously defaulted to
  `target/debug/new-java2rust`. `Alignment.java` timed out after 60s in debug
  but completed in about 20s with `target/release/new-java2rust`.
- The focused script now defaults to release builds and uses batch translation
  by default with 10 emission jobs unless `JOBS` or `--jobs` overrides it.
- The raw `new-java2rust translate-batch` CLI now also defaults to 10 emission
  jobs when `--jobs` is omitted. Direct CLI users therefore get the same
  default parallelism as the focused corpus script.
- The legacy per-file worker path is still available behind
  `--no-batch-translate`, but it no longer defines the normal focused corpus
  workflow.
- JAligner now reaches 29/29 translated and 0 invalid patterns with release
  translation. After fetching `ndarray` once and using `--offline-cargo`, the
  generated crate reaches Rust typechecking; remaining failures are generated
  Rust errors, not dependency-cache failures.
- VarScan serial batch and default-10 batch output match, but the run shows that
  global analysis, not Rust emission, is now the dominant speed bottleneck.

## Plan

### 1. Batch Translation Mode

Add a `new-java2rust translate-batch` command that accepts a primary directory,
context roots, source-summary roots, facts, and external pack manifests, then
emits all selected Java files in one process.

Required shape:

- Load primary files, emitted context files, source-summary files, facts, and
  manifests once.
- Parse each source once.
- Build one global analysis context: resolver summaries, semantic facts,
  representation plans, signature plans, inherited-member surfaces, and
  external/facade fact indexes.
- Emit per-primary-file Rust modules from that shared context.
- Produce the same per-file status rows as `translate-focused-package.sh`:
  source path, Rust path, translation status, diagnostic count, invalid-pattern
  count.

Acceptance evidence:

- JAligner focused batch output matches the current release per-file output for
  invalid-pattern counts.
- A focused GATK/Picard/HTSJDK slice has the same or fewer diagnostics than the
  per-file script path.
- Batch mode records phase timings for parse, global analysis, emission, crate
  assembly, and cargo check.

Implementation status:

- Initial serial batch mode exists as `new-java2rust translate-batch`.
- The focused package script uses batch mode by default and keeps the legacy
  per-file path behind `--no-batch-translate`.
- Current batch mode deliberately preserves one global analysis context before
  emitting files. This is the baseline that future parallelism must match.
- Rust-side batch timings are now split into input collection, external fact
  loading, context loading, primary parse, context parse, summary parse, global
  analysis, emission, status write, and command total.
- Batch mode writes `batch-manifest.tsv` with deterministic generated-module
  hashes for serial/parallel output comparison.
- `scripts/run-focused-batch-stability-gate.sh` runs focused translation with
  `--jobs 1` and `--jobs N`, then compares normalized manifests, status rows,
  invalid-pattern tables, top compile errors, and key summary fields. Its
  parallel side defaults to 10 jobs unless `JOBS` or `--jobs` overrides it.
- Picard programgroups now has both a batch stability gate and a legacy per-file
  focused gate with zero diagnostics, zero invalid-pattern files, and
  `cargo_check=ok`.

### 2. Safe Parallel Emission

Investigate and implement parallelism only after the global semantic state is
complete.

Allowed parallelism:

- Parallel parse of independent Java files before global analysis, if parsed AST
  order is made deterministic before planning.
- Parallel Rust emission per primary file after global analysis, using immutable
  shared analysis state.
- Parallel per-file invalid-pattern scans after emission.
- Parallel generated-crate support extraction only when outputs are merged by
  deterministic keys.

Avoid:

- Per-file workers that independently infer source summaries, overloads,
  ownership, nullability, or inherited members.
- Parallel passes that mutate shared `FactSet`, signature maps, representation
  plans, or support surfaces without deterministic merge semantics.
- Any shortcut that makes global semantic analysis optional for corpus gates.

Acceptance evidence:

- Batch serial mode and batch parallel-emission mode produce byte-stable or
  semantically equivalent output for the same corpus.
- Race-sensitive data structures are either immutable during emission or merged
  with sorted deterministic keys.
- A `--jobs` flag controls emission parallelism without changing analysis
  results.

Implementation plan:

1. Make the serial batch path the oracle.
   Capture a stable manifest for each run: input file order, fact/manifest
   inputs, global analysis diagnostics, generated module paths, invalid-pattern
   counts, and content hashes.

2. Introduce an explicit immutable emission context.
   The data passed into per-file lowering/emission must be read-only after
   global analysis: resolved support program, source program, summary program,
   semantic facts, representation plans, signature plans, control-flow plans,
   and external facts. Any lowerer that currently mutates shared support state
   should return per-file support requirements instead.

3. Parallelize only per-primary-file lowering and Rust emission.
   Use the same sorted primary file list as serial mode. Workers may emit into
   memory or per-file temp paths, but the final status rows and writes must be
   merged by deterministic input index.

4. Keep global analysis single-owner until there is evidence it is the
   bottleneck.
   Parsing can be parallelized earlier if AST ordering is normalized before
   resolver/source-summary planning. Resolver, semantic analysis, representation
   planning, signature solving, inherited-member planning, and facade fact
   indexing should remain global and serial for now.

5. Add a byte-stability gate.
   For representative corpuses, run `translate-batch --jobs 1` and
   `translate-batch --jobs N`, then compare generated Rust hashes, status rows,
   invalid-pattern tables, and cargo-check diagnostics. Parallel mode is not
   acceptable if it changes semantic analysis output.

Implementation status:

- `translate-batch --jobs N` now uses one global analysis context and
  parallelizes only per-primary-file lowering/emission after that context has
  been built.
- Worker outputs are merged by deterministic source index before status rows
  and generated module paths are written.
- `tests/batch_translation.rs` checks serial/parallel batch output equality for
  a two-file batch and verifies CLI phase/worker event output.
- `scripts/compare-focused-batch-artifacts.py` compares corpus-level
  `batch-manifest.tsv` output and related focused-gate artifacts while ignoring
  output-directory path prefixes.
- The HTSJDK FASTQ real-code stability gate passes for `--jobs 1` versus
  `--jobs 2`.
- The Picard programgroups real-code stability gate passes for `--jobs 1`
  versus the default 10-job parallel side.
- The JAligner stability gate has matching serial/parallel artifacts and zero
  invalid-pattern files. After dependency fetch, an offline focused gate reaches
  generated Rust typechecking and reports the remaining errors as
  `cargo_failure_kind=generated_rust`.
- The GATK GenomicsDB primary-only and Spark/DISQ primary-only stability gates
  have matching serial/parallel artifacts in this checkout, but their generated
  crates currently fail typechecking. Those are translation-quality regressions
  or corpus idiom gaps, not batch parallelism races.
- The VarScan stability gate has matching serial/default-10 artifacts and one
  invalid-pattern file in both modes. Both generated crates currently fail at
  dependency resolution for `ndarray`, so the run is a stability/timing result
  rather than a generated Rust quality result.

Parallelism boundaries:

- Safe now: invalid-pattern scans, generated file hashing, cargo-error grouping,
  and other post-emission reporting.
- Safe after context immutability audit: per-file lowering/emission.
- Possible later: parser fan-out with deterministic merge.
- Avoid for this slice: parallel resolver/semantic/signature mutation or
  process-level workers that repeat global analysis.

### 3. Workflow Instrumentation And Liveness

Improve corpus-run observability before deeper optimization.

Add:

- `worker-events.tsv` or `phase-events.tsv` with start/end timestamps, pid,
  source path, phase, elapsed time, and status.
- Per-phase elapsed timing in `summary.tsv`.
- Optional per-file translation timeout that records `timeout` status instead of
  leaving zero-byte temp/status fragments.
- Clear `translator_binary` recording, already present in focused summaries.

Use this to distinguish:

- translator CPU time;
- repeated context analysis;
- generated crate assembly time;
- dependency fetch or cargo typecheck time;
- script orchestration failures.

Acceptance evidence:

- A timed-out corpus run leaves enough status to identify the active file or
  phase.
- JAligner and VarScan runs report the slowest files/phases without manual `ps`
  or temp-file inspection.

Implementation status:

- The focused script now writes `phase-events.tsv` and `worker-events.tsv`.
- `summary.tsv` records translator binary, cargo target dir, translation time,
  crate assembly time, cargo-check time, invalid scan time, idiom-sweep time,
  and total elapsed time.
- Legacy per-file mode records worker start/end rows and supports
  `--translate-timeout-sec`.
- Batch mode records batch start/end rows, Rust-side phase rows, and live
  `emit_start`/`emit_end` per-file emission rows from the Rust process.
- The focused script writes `slowest-phases.tsv` and `slowest-workers.tsv` so
  slow files and phases are available without manual process inspection.

Next instrumentation work:

1. Preserve the legacy timeout path for debugging, but treat it as a fallback
   rather than the default corpus mode.
2. Extend liveness reporting to include parse/global-analysis subphase progress
   if a future corpus shows those phases can hang.

### 4. Shared Cargo Check Cache

Make generated-crate typechecking less noisy and less redundant.

Implement:

- `CARGO_TARGET_DIR` support in focused scripts, defaulting to a stable
  `.tmp/focused-cargo-target` unless the caller overrides it.
- Non-quiet cargo mode or a `--verbose-cargo` flag for dependency fetch failures.
- Optional `CARGO_NET_OFFLINE=true` mode once dependencies are known cached.

Acceptance evidence:

- JAligner no longer repeatedly rebuilds or refetches `ndarray` artifacts when
  the dependency is available.
- Dependency failures are reported as dependency/cache issues, not confused with
  generated Rust type errors.
- Typecheck failures after dependencies resolve are captured in
  `top-compile-errors.tsv` and promoted to minimized fixtures.

Implementation status:

- The focused script honors caller-provided `CARGO_TARGET_DIR`.
- When unset, it defaults to `.tmp/focused-cargo-target` so focused reruns share
  dependency and build artifacts.
- `--verbose-cargo` is available for dependency/cache failures.
- `--offline-cargo` runs generated-crate checking with Cargo `--offline`, and
  `summary.tsv` records `cargo_offline`.
- `summary.tsv` records `cargo_failure_kind=dependency_cache` when cargo output
  indicates dependency fetch/cache/registry/offline failure, and
  `cargo_failure_kind=generated_rust` for generated-crate typecheck failures.

Remaining work:

1. Promote dependency-enabled JAligner generated Rust failures into the broader
   idiom/translation-quality backlog. The speed-plan dependency/cache
   classification is complete for this corpus.

Current verification:

- `cargo check --quiet` passes.
- `cargo test --test batch_translation --quiet` passes.
- `scripts/translate-focused-package.sh --skip-resolver-bench --primary
  examples/java/01-expressions --out
  .tmp/focused-batch-jobs-expressions-smoke --jobs 2` passes with
  `cargo_check=ok`, zero invalid-pattern files, Rust-side batch phase rows, and
  per-file emission rows.
- `scripts/run-focused-batch-stability-gate.sh --skip-resolver-bench --primary
  examples/java/01-expressions --out
  .tmp/focused-batch-stability-expressions2 --jobs 2` passes with matching
  serial/parallel artifacts and zero run failures.
- `scripts/translate-focused-package.sh --skip-resolver-bench --primary
  examples/java/01-expressions --out
  .tmp/focused-batch-liveness-slowest-smoke` passes using the default 10 jobs
  and records live `emit_start`/`emit_end` rows plus `slowest-phases.tsv` and
  `slowest-workers.tsv`.
- `scripts/translate-focused-package.sh --skip-resolver-bench --primary
  examples/java/01-expressions --out .tmp/focused-batch-offline-cargo-smoke
  --offline-cargo` passes and records `cargo_offline=1`.
- `scripts/run-focused-batch-stability-gate.sh --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/htsjdk/src/main/java/htsjdk/samtools/fastq
  --out .tmp/focused-batch-stability-htsjdk-fastq-after-status-counter --jobs
  2` passes with matching serial/parallel artifacts, `cargo_check=ok`, and zero
  invalid-pattern files.
- `scripts/run-focused-batch-stability-gate.sh --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/picard/src/main/java/picard/cmdline/programgroups
  --out .tmp/focused-batch-stability-picard-programgroups-default10` passes
  with matching serial/parallel artifacts, `cargo_check=ok`, and zero
  invalid-pattern files.
- `scripts/translate-focused-package.sh --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/picard/src/main/java/picard/cmdline/programgroups
  --out .tmp/focused-legacy-picard-programgroups-default10
  --no-batch-translate` passes with `cargo_check=ok`, zero diagnostics, and
  zero invalid-pattern files, matching the batch gate quality.
- `scripts/run-focused-batch-stability-gate.sh --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/jaligner/src/jaligner --out
  .tmp/focused-batch-stability-jaligner-after-catch-sort --jobs 2
  --verbose-cargo` has `compare_exit=0`, zero invalid-pattern files, and
  matching serial/parallel batch artifacts. The focused runs still exit nonzero
  because `cargo check` fails to fetch `ndarray`.
- `scripts/translate-focused-package.sh --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/jaligner/src/jaligner --out
  .tmp/focused-batch-jaligner-dependency-kind-default10 --verbose-cargo`
  records `cargo_failure_kind=dependency_cache` for the `ndarray` fetch failure.
- After allowing Cargo to fetch `ndarray`, `scripts/translate-focused-package.sh
  --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/jaligner/src/jaligner --out
  .tmp/focused-batch-jaligner-offline-after-self-import-filter --verbose-cargo
  --offline-cargo` reaches generated Rust typechecking with
  `cargo_failure_kind=generated_rust`, zero invalid-pattern files, and 618
  reported cargo errors. The earlier self-import duplicate-definition bucket was
  removed by filtering current-module declarations out of generated crate import
  preludes.
- JAligner timing from that run: global analysis is roughly 11.5s in both
  modes, while emission drops from 56.9s at `--jobs 1` to 36.0s at `--jobs 2`.
- `scripts/run-focused-batch-stability-gate.sh --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/tools/genomicsdb
  --out .tmp/focused-batch-stability-gatk-genomicsdb-default10` has
  `compare_exit=0`, zero invalid-pattern files, and matching serial/parallel
  batch artifacts. The generated crate currently fails with two Rust errors
  (`duplicate definitions with name query` and a `HashMap::insert` key mismatch).
- `scripts/run-focused-batch-stability-gate.sh --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/engine/spark/datasources
  --out .tmp/focused-batch-stability-gatk-spark-datasources-default10` has
  `compare_exit=0`, but the generated crate currently has four
  invalid-pattern files and generated Rust typecheck failures.
- `scripts/run-focused-batch-stability-gate.sh --skip-resolver-bench --primary
  /home/mahogny/github/claude/java2rust/testdata/varscan/src --out
  .tmp/focused-batch-stability-varscan-default10` has `compare_exit=0`; the
  serial and default-10 runs both translate 18/18 files, both report one
  invalid-pattern file, and both fail cargo check at `ndarray` dependency
  resolution. Timing shows the parallel run spends about 93.2s in global
  analysis and 1.7s in 10-job emission, so further speed work should target
  resolver/semantic/representation/signature algorithms, not more emission
  threads.
- `examples/java/04-objects` exercises the same batch/event path but still has
  pre-existing generated-crate compile/invalid-pattern failures, so it is not a
  green speed-plan gate.

## Investigation Questions

- How much of current runtime is global representation/signature planning versus
  Rust emission once repeated context analysis is removed?
- Which global analysis data structures need to become explicitly immutable or
  clone-on-write before parallel emission is safe?
- Can source-summary generation be cached as a serialized artifact shared across
  batch runs, or is in-memory reuse inside batch mode enough for the next scale
  target?
- Does generated support/facade assembly need to move into Rust batch mode, or
  is the current shell/Python assembly acceptable once translation itself is
  batched?

## Order Of Work

1. Add instrumentation and liveness reporting to the existing focused script.
2. Add serial `translate-batch` using one global analysis context.
3. Switch focused corpus scripts to use batch mode by default.
4. Add parallel emission behind `--jobs` after serial batch output is stable.
   Initial implementation and a small corpus-level stability gate are present;
   real-corpus byte/hash gates remain.
5. Add shared cargo target/cache controls and rerun JAligner with dependency
   access. This is implemented; remaining JAligner work is generated Rust
   translation quality, tracked outside the speed/cache layer.
6. Re-profile VarScan, JAligner, HTSJDK FASTQ, GenomicsDB primary-only, and
   Spark/DISQ primary-only to decide whether deeper algorithmic optimization is
   still needed. This is done for the named focused corpuses. The result is
   clear: parallel emission is deterministic, but VarScan and JAligner still
   spend enough time in global analysis that algorithmic optimization is the
   next speed work.

## Completion Audit

| Requirement | Status | Evidence |
|---|---|---|
| Release/profile builds are normal for focused runs | Done | `translate-focused-package.sh` defaults to `target/release/new-java2rust`; release binary was rebuilt before the latest VarScan gate. |
| Use 10 threads by default | Done | Focused scripts default `JOBS` to 10; `translate-batch` now defaults `jobs` to 10 unless `--jobs` overrides it. |
| Preserve global semantic analysis | Done | Batch mode builds one global context before per-file emission; no parallel resolver/semantic/signature mutation is introduced. |
| Parallelize only safe work | Done | `--jobs` is limited to per-primary-file lowering/emission after global analysis, with deterministic merge by source index. |
| Byte/hash stability gate | Done | Examples, HTSJDK FASTQ, Picard programgroups, JAligner, GenomicsDB primary-only, Spark/DISQ primary-only, and VarScan have serial/parallel comparison evidence. |
| Liveness and slow-phase reporting | Done | `phase-events.tsv`, `worker-events.tsv`, `slowest-phases.tsv`, and `slowest-workers.tsv` are written by focused runs. |
| Shared cargo cache and failure classification | Done | Focused runs use `.tmp/focused-cargo-target` by default and classify dependency/cache failures separately from generated Rust failures. |
| Decide whether algorithms remain a bottleneck | Done | Yes. VarScan's default-10 run spends roughly 93.2s of 95.1s translation time in global analysis, so additional threads alone will not solve large-corpus latency. |
