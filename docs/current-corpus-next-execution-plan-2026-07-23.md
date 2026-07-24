# Current Corpus Next Execution Plan

Date: 2026-07-23

This is the trackable next-action plan from the current checkout after the
nullable external argument policy fix and `Map.keySet()` loop key borrow fix.
The local translator gates were green after those changes, and the latest
focused FastQC exception subset now exposes a small generated-Rust error set
instead of broad invalid Java-pattern output.

## Current Baseline

Latest focused evidence:

| Corpus | Output | Result |
|---|---|---|
| FastQC exception subset | `.tmp/corpus-fastqc-exception-subset-after-keyset-mapget-borrow-fix` | 9/9 translated, 0 invalid-pattern files, generated crate fails with 5 underlying Rust errors |

The previous `String: Borrow<&String>` `Map.get(keySet loop key)` error is
gone. The remaining focused subset families are:

- non-cloneable erased object sequence storage, visible as
  `Vec<Box<dyn Any>>` clone recovery around `msgArguments.add(args)`;
- typed deserialization recovery from `ObjectInputStream.readObject()` into
  `Throwable`, `String`, and source interfaces such as `Localizable`;
- erased object/interface handoff where Java `Object` needs a precise Rust
  carrier or typed downcast boundary;
- one borrowed source-field lifetime row, currently represented as a Rust
  reference stored without an explicit lifetime model.

Local gates to preserve:

- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet`
- `cargo test --test type_coercion --quiet`
- `cargo test --test parser_gaps --quiet`
- `cargo test --test batch_translation --quiet`
- `cargo build --release --quiet --bin new-java2rust`

## Guardrails

- Add or update minimized `tests/idiom_gaps.rs` fixtures for each recurring
  corpus failure before relying on a corpus-only fix.
- Keep non-JDK package behavior out of `src/stdlib.rs`; use `javalibs`
  manifests/shims for package-specific surfaces.
- Prefer fact, callable/member, slot, and conversion plans over rendered Rust
  string matching.
- Keep GenomicsDB native behavior and DISQ/Spark distributed execution as
  facade/FFI handoff boundaries. Translate the Java wrapper code around those
  boundaries.
- Use `--jobs 10` for focused corpus reruns.
- Do not introduce algorithmic changes that make future global semantic
  analysis harder. Clone reduction, indices, and caches are fine when semantics
  are preserved.

## Phase 1: Fix Erased Object Sequence Moves

Goal: remove the `args.clone()` row where `args` is an `Object[]`/erased object
sequence stored into `List<Object[]>`.

Actions:

1. Add a fixture close to the corpus shape:
   `ObjectInputStream`, `Object[] args = new Object[len]`, indexed
   `readObject()` assignments, and `msgArguments.add(args)` inside a loop.
2. Inspect whether the call flows through native `List.add`, unshaped
   argument lowering, or a source-summary argument path.
3. Broaden erased-object sequence detection structurally:
   direct `Object[]`, `List<Object>`, nested `List<Object[]>`, and equivalent
   slot-derived element shapes.
4. Move non-reused erased object sequences into owned collection slots when
   later-read/current-statement use analysis proves the source value is not
   reused.
5. Keep clone recovery for true repeated owned uses.

Verification:

- targeted new fixture
- `cargo test --test idiom_gaps --quiet`
- focused FastQC exception subset rerun:
  `scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 --primary /tmp/new_java2rust_fastqc_exception_subset --out .tmp/corpus-fastqc-exception-subset-after-erased-sequence-move`

Acceptance:

- no generated `Vec<Box<dyn Any>>::clone()` requirement for terminal
  `List<Object[]>.add(args)`;
- no regression in ordinary repeated owned `String`/collection clone tests.

## Phase 2: Typed `readObject()` Recovery

Goal: make `ObjectInputStream.readObject()` produce an erased carrier that can
coerce into the Java target slot when the target type is known.

Actions:

1. Add fixtures for `readObject()` assigned or cast to:
   - `String`;
   - `Throwable` or Java exception carrier;
   - `Object`;
   - source interface such as `Localizable`;
   - indexed `Object[]` elements.
2. Define the desired ABI as an explicit handoff:
   `read_object() -> Result<Box<dyn Any>, JavaException>` at the facade
   boundary.
3. Plan target-slot recovery through explicit helpers, for example typed
   downcast/clone helpers for `String` and exception carriers, and boxed trait
   conversion for source interfaces when the object model can prove or defer
   the cast.
4. Avoid making a blanket Rust `Any` clone/display assumption.

Verification:

- targeted fixtures for each target slot;
- focused FastQC exception subset rerun.

Acceptance:

- deserialized `String`, exception, and source-interface slots no longer emit
  invalid downcasts or impossible `Box<dyn Any>` trait bounds;
- raw Java `Object` slots continue to use the erased carrier.

## Phase 3: Source Interface And Lifetime Cleanup

Goal: resolve the last focused exception rows that are not pure object-stream
handoffs.

Actions:

1. Minimize the `unit -> Localizable` or erased-interface coercion row and
   decide whether it is a missing enum/source trait impl, a bad return shape, or
   a target-slot coercion bug.
2. Minimize the borrowed source-field lifetime row.
3. Prefer owned fields for Java object state unless there is an explicit,
   scoped borrowed ABI fact. Java instance fields generally should not become
   unconstrained Rust references.
4. If a true borrowed representation is required, make the lifetime explicit in
   the emitted owner type and add a fixture that proves it.

Verification:

- targeted fixtures;
- full `idiom_gaps`;
- focused FastQC exception subset rerun.

Acceptance:

- FastQC exception subset either typechecks or the remaining rows are explicit
  unsupported/facade handoff diagnostics, not accidental Rust type errors.

## Phase 4: Refresh The Corpus Matrix

After the focused subset is materially improved, rerun the current tracked
corpuses with 10 jobs and update `docs/current-corpus-issue-audit.md`.

Run or refresh:

- `picard-programgroups`
- `htsjdk-fastq`
- `gatk-genomicsdb`
- `gatk-disq-spark-datasources`
- `htsjdk-vcf`
- `trimmomatic`
- `biojava-aaprop`
- `biojava-alignment`
- `jaligner`
- `varscan`
- `jahmm`
- `fastqc`
- `jhlabs`
- `jsoup`

For each run, record:

- translated count;
- failed translation count;
- invalid-pattern count and top buckets;
- generated Rust `cargo check` result and top error buckets;
- whether each bucket is mechanical Java, facade/FFI handoff, or intentionally
  out of scope runtime behavior.

Acceptance:

- the audit table has current output directories for all tracked corpuses;
- every failing corpus has at least one fixture candidate or a documented
  non-mechanical boundary.

## Phase 5: Next Shared Idiom Batches

Prioritize repeated failures after the refresh in this order:

1. generated-Rust correctness in the FastQC exception/object-model family;
2. VCF shared support enum/display assembly and the unclosed delimiter;
3. Spark/DISQ interface bridge and no-op facade handoff gaps;
4. GenomicsDB overload/name conflicts and facade handoff shape gaps;
5. ndarray and 2D array/matrix lowering gaps;
6. switch/state-machine scaffolding;
7. Java string/file/logging facade surfaces;
8. source-super and constructor delegation variants;
9. trait/interface assembly and implementation bridging;
10. collection/map spelling variants not already routed through native plans.

Each batch should follow the same loop:

1. minimize one corpus error family into a fixture;
2. implement the shared translator or `javalibs` fact/shim fix;
3. run targeted tests, local gates, and the smallest affected corpus;
4. update `docs/current-corpus-issue-audit.md` with exact evidence.

## Done Criteria

This slice is complete when:

- the FastQC exception subset has no accidental generated Rust type errors, or
  every remaining error is a documented facade/FFI/unsupported boundary;
- all local gates pass;
- the tracked corpus matrix has fresh results from this checkout;
- recurring idioms from failed corpuses have fixtures or explicit deferral
  notes;
- no new non-JDK package tables have been added to `src/stdlib.rs`.
