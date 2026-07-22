# Single-Checkout Idiom Gap Plan

## Decision

Finish the next GATK/Picard/DISQ idiom-gap batch in this checkout instead of
splitting implementation across cloned repos or worktrees.

The main reason is that the remaining fixes cross the boundary between
translator lowering and `javalibs` facts/facades. Parallel workers are still
useful for audits, corpus reruns, and fixture proposals, but implementation
should land in one checkout so shared identities, facade ABI facts, support
crate assembly, and generated crate shape stay coherent.

## Scope

This plan supersedes `docs/parallel-idiom-gap-worktree-plan.md` for the current
implementation batch. Keep the old file as background, but do not launch
separate edit branches for these fixes unless this checkout becomes blocked.

Use this checkout for:

- `src/external_packs.rs` javalibs facade facts and typed fields.
- `src/stdlib.rs` shared support/facade emission.
- `src/call_plan.rs` callable/facade selection.
- `src/call_lowering.rs` slot/result/coercion lowering.
- crate assembly scripts under `scripts/`.
- regression fixtures and corpus-focused tests.

Use parallel workers only for:

- read-only inspection of corpus failures;
- comparing generated output before/after a change;
- proposing fixture reductions;
- auditing whether a failure belongs in `javalibs`, translator lowering, or a
  future C/C++ FFI handover.

## Current Baseline

Latest verified focused corpus state:

| Corpus | Output | Status |
|---|---|---|
| Picard program groups | `.tmp/corpus-rerun-picard-programgroups-after-locatable-conversion-support` | 11/11 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| GATK GenomicsDB primary-only | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-locatable-conversion-support` | 6/6 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| GATK Spark/DISQ primary-only | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-broad-alias-flatmap-support` | 10/10 translated, 2 invalid-pattern files, `cargo check` fails with 57 reported errors |

Recent fixes already landed in this checkout:

- generated crate assembly adds `ndarray` when emitted code/support uses it;
- shared support discovery catches `JavaClass<T>` uses outside direct struct
  declarations;
- direct self-recursive shared support fields are boxed;
- overloaded javalibs constructors are selected by arity;
- `URI::parse` and `URI::get_path` are emitted in generated path support;
- alias-owned facade impls/fields are redirected to their alias target;
- duplicate prelude support stubs are removed when a richer generated type
  exists;
- `Result` propagation is idempotent, removing generated `??`.
- source context files now feed semantic/signature planning while only the
  selected primary sources are emitted;
- resolved source static calls use context signature shapes and throwing source
  static calls propagate `Result`;
- optional sequence receivers are unwrapped before `.get(...)`;
- stream block lambda local initializers clone owned `String` array elements
  instead of moving out of `Vec<String>`.
- borrowed `SimpleInterval` elements can be adapted to `Locatable` slots through
  `javalibs` subtype knowledge and generated conversion support;
- DISQ static facade factories lower as associated facade functions instead of
  snake-case modules;
- DISQ variant read boundary facts cover `HtsjdkVariantsRddStorage.read` and
  `HtsjdkVariantsRdd.getVariants`;
- external enum switch labels backed by typed field facts lower as associated
  constants;
- optional boxed dyn-trait storage slots default through `Option`/`None`;
- focused-package assembly exports sibling traits and emits a no-op
  `FlatMapFunction` boundary type when Spark/DISQ callback adapters require it;
- wildcard imports from `org.broadinstitute.hellbender.utils.read.*` can seed
  static owner aliases for `ReadUtils` and `SAMRecordToGATKReadAdapter`.

Current immediate blocker:

- GenomicsDB and the Picard slice compile, so the next batch should focus on
  Spark/DISQ primary-only.
- The top errors are now type-shape and facade-surface issues, not parser
  failures: varargs packing, optional borrowing, same-class overload selection
  inside stream lambdas, native collection `parallelStream`, missing no-op
  methods/constants on Spark/HTSJDK/DISQ handles, and the remaining explicit FFI
  or distributed-boundary cast placeholders.
- Keep implementation in this checkout because the fixes cross
  `src/lower.rs`, `src/call_plan.rs`, `src/external_packs.rs`,
  `src/javalibs.rs`, shared support assembly, and focused corpus scripts.

## Next Batch

1. Recompile the three focused corpuses from this checkout.
   - Picard program groups should remain green.
   - GenomicsDB primary-only should remain green.
   - Spark/DISQ primary-only should produce the next prioritized batch of
     ordinary Java idioms versus explicit distributed-boundary blockers.

2. Close the highest-value Spark/DISQ no-op facade gaps in `javalibs`.
   - Add explicit no-op support for Hadoop configuration, Spark context/RDD
     handles, Google Cloud Storage boundaries, and DISQ storage options.
   - Do not mechanically translate distributed execution semantics.
   - Keep Spark/DISQ behavior at boundary handles unless source code is ordinary
     Java logic around those handles.
   - Current concrete surfaces include:
     `ReferenceSequenceFile.close`, `ReadsDataSource.close`,
     `TraversalParameters.traverseUnmappedReads`,
     `HtsjdkReadsRddStorage.sbiIndexGranularity`,
     `TwoBitReference.getReferenceBases`, `Path.getFileSystem`,
     `SortOrder.getComparatorInstance`, `SAMRecord.NO_ALIGNMENT_START`, and
     `ReadsWriteFormat.SINGLE/SHARDED` support emission.

3. Fix translator shape bugs exposed by Spark/DISQ.
   - Lower Java `stream()`/`parallelStream()` on native Rust collections through
     the shared stream/call plan instead of emitting `.parallel_stream()` on
     `Vec<T>`.
   - Select same-class overloaded helpers inside stream lambdas using lambda
     parameter element shapes, so `getParallelVariants(vcf, intervals)` lowers
     to the generated overload method rather than an unresolved free function.
   - Pack Java varargs into `Vec<WriteOption>` at call sites.
   - Adapt `Option<T>` to `Option<&T>` slots when the facade ABI expects a
     borrowed optional value.
   - Repair null/default handling where generated signatures still require a
     non-null `String` but Java passes `null`.

4. Replace remaining broad `Any` and downcast fallbacks where facts can provide
   precise slots.
   - Prefer typed facade return/argument facts over generated
     `Box<dyn Any>`.
   - Use `ndarray` for `Vec<Vec<T>>` only when the Java shape is rectangular or
     the source/facts prove matrix semantics.
   - FFI handover placeholders are acceptable only when the future C/C++
     transpiler must provide native signatures first; record those separately
     from ordinary Java idiom failures.

5. Fix generated module/type visibility issues.
   - Resolve sibling source-summary module paths through one naming plan.
   - De-duplicate local nested classes, aliases, and shared support by stable
     type key before crate emission.

6. Add focused regression fixtures before broadening corpus runs.
   - One fixture per repeated failure shape.
   - Keep fixtures disabled only when they document a known non-mechanical
     boundary; otherwise make them active regression tests.

## Corpus Loop

Use this loop for each implementation step:

1. Add or adjust the smallest fixture that captures the repeated corpus error.
2. Patch the translator and `javalibs` facts in this checkout.
3. Run the focused fixture tests plus `cargo check --quiet`.
4. Build the CLI and rerun the affected focused corpus.
5. Record the output directory, error count, and top failure category in
   `docs/corpus-rerun-idiom-gap-plan.md`.

Parallel workers may run read-only audits over generated crates or propose
fixture reductions, but they should not land competing edits in cloned repos for
this batch.

## Verification

After each focused patch:

```bash
cargo fmt
cargo check --quiet
cargo test --test external_packs gatk -- --nocapture
cargo test --test idiom_sweep_script shared_support_merger -- --nocapture
cargo test --test slot_shape_design -- --nocapture
```

Then rerun the affected corpus slice:

```bash
scripts/translate-focused-package.sh <fixture-list> <output-dir>
```

Record new compile-error counts and top remaining errors in
`docs/corpus-rerun-idiom-gap-plan.md`.

## Done Criteria

- Picard, GenomicsDB primary-only, and Spark/DISQ primary-only generated crates
  compile or have only documented non-mechanical FFI/distributed-execution
  handover blockers.
- New package-specific behavior lives in `javalibs` manifests/facades, not
  hardcoded non-JDK tables in `src/stdlib.rs`.
- Stream and ordinary call/member lowering use the same call/member/slot plans.
- Facade ABI behavior is selected from explicit facts instead of Rust type
  string inspection.
