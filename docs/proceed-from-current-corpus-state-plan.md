# Proceed From Current Corpus State Plan

Date: 2026-07-23

This is the tracked execution plan for continuing the current corpus-driven
translator work from this checkout. It starts from
`docs/current-corpus-issue-audit.md` and the latest focused FastQC exception
subset run.

The operating goal is to keep reducing real generated-Rust blockers as
generalized Java idioms, without drifting away from the larger stable
callable-key/fact-unification design.

## Current Baseline

Local gates were last verified green:

- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet`
- `cargo test --test batch_translation --quiet`
- `cargo test --test type_coercion --quiet`
- `cargo build --release --quiet --bin new-java2rust`

Clean focused corpuses:

| Corpus | Latest useful output | Status |
|---|---|---|
| `picard-programgroups` | `.tmp/corpus-batch-current-picard-programgroups` | clean: 11/11 translated, 0 invalid patterns, `cargo check` ok |
| `htsjdk-fastq` | `.tmp/corpus-batch-current-htsjdk-fastq-after-signature-field-shape` | clean: 8/8 translated, 0 invalid patterns, `cargo check` ok |
| `varscan` | `.tmp/corpus-batch-current-varscan-after-loop-owned-elements` | clean: 18/18 translated, 0 invalid patterns, offline `cargo check` ok |

Important current FastQC evidence:

- Full FastQC now reaches normal translation output with no old
  duplicate-owner signature panic.
- Parser cleanup slices removed the known keyword receiver, cast/comparison,
  block-comment condition, hex-float, and long-mask blockers in generated
  FastQC sources.
- Stream-wrapper no-state superclass construction, `String.join` varargs,
  source-super trailing expression statements, external exception superclass
  trailing bodies, bare constructor field assignments, and source-super varargs
  matching are covered by enabled idiom fixtures.
- Focused exception subset:
  `.tmp/corpus-fastqc-exception-subset-after-object-map-erasure`
  translates 9/9 files and has 0 broad
  `super(...) requires inheritance lowering` placeholders, 0 `compile_error!`
  placeholders, 0 invalid Java cast downcast rows, 0 `.put` calls, and 0
  unsupported array-creation placeholders.
- The `Serializable` marker `instanceof` rows in `ExceptionContext` now lower
  through `java_is_serializable(...)`. Current self passed to known Java
  exception supertypes lowers as an explicit facade boundary, and
  `HashMap<String,Object>` inserts keep erased-object carrier values. Remaining
  subset failures are generated Rust type/interface errors.

## Working Rules

Every implementation slice should follow this loop:

1. Start from corpus evidence: invalid-pattern rows, generated Rust errors, or
   translator diagnostics.
2. Reduce the failure to the smallest Java idiom fixture in
   `tests/idiom_gaps.rs` or a narrower test module.
3. Fix shared planning, lowering, signature, coercion, facade fact, or runtime
   support logic.
4. Run local gates.
5. Rerun the affected focused corpus with `--jobs 10`.
6. Update `docs/current-corpus-issue-audit.md` and this plan with the exact
   output directory and remaining counts.

Package-specific non-JDK behavior belongs in `javalibs` manifests or shims.
Do not add new non-JDK package tables to `src/stdlib.rs`.

## Phase 1: FastQC Object-Model And Cast Slice

Status: invalid-pattern slice complete; generated-Rust type/interface cleanup
remains.

The constructor work exposed object-model failures in the focused FastQC
exception subset. The `Serializable` marker `instanceof`, current-self
exception-supertype, and erased `Object` map insert subfamilies are fixed for
the invalid-pattern scanner. The remaining rows are generated Rust
type/interface issues, including trait-object recovery and cross-module support
type visibility.

Actions:

1. Inspect the exact generated rows in:
   - `.tmp/corpus-fastqc-exception-subset-after-varargs-super-matching/generated-crate/src/generated/00006_ExceptionContext.rs`
   - generated exception constructors that contain
     `downcast::<T>().expect("invalid Java cast")`.
2. Compare those rows with the Java source in
   `ExceptionContext.java`, especially `Object[]` handling and guarded
   formatting paths.
3. Add minimized fixtures for:
   - `Object value; if (value instanceof Serializable) ...` (done:
     `serializable_instanceof_erased_object_uses_runtime_marker_helper`);
   - `new ExceptionContext(this)` where the slot expects `Throwable` (done:
     `exception_self_as_throwable_context_argument_uses_facade_boundary`);
   - `HashMap<String,Object>.put(key, value)` with an erased `Object` value
     (done: `object_map_put_keeps_erased_object_value_without_downcast`);
   - `Object value; if (value instanceof Object[]) ...`;
   - `Object[]` varargs flowing through `Box<dyn Any>` or erased object slots;
   - guarded `instanceof` followed by cast/use;
   - `this` passed through an exception-superclass object/context boundary.
4. Decide per fixture whether the idiom is mechanically translatable with the
   current runtime carrier model:
   - if yes, implement it in coercion/slot planning and runtime support;
   - if no, emit a precise unsupported diagnostic that names the missing
     object identity or runtime tag requirement.
5. Do not fake object identity. In particular, avoid lowering `this`-to-parent
   or parent-to-child paths as arbitrary `Any` downcasts unless a real carrier
   proof exists.
6. Rerun:
   - focused new fixture tests;
   - `cargo test --test idiom_gaps --quiet`;
   - the 9-file FastQC exception subset.

Latest evidence:

- `.tmp/corpus-fastqc-exception-subset-after-object-map-erasure`
- 9/9 translated, 0 translation failures.
- Invalid patterns: all zero, 0 invalid-pattern files.
- `cargo_check_errors=43`, `cargo_failure_kind=generated_rust`.
- `.tmp/corpus-fastqc-exception-subset-after-preserved-prelude-impls`
  preserves source `ExceptionContext`, `Localizable`, and
  `ExceptionContextProvider` as unique exports while keeping support impls such
  as `ObjectInputStream::read_object` and `ObjectOutputStream::write_object`;
  generated cargo errors drop to 40 with invalid patterns still all zero.
- `.tmp/corpus-fastqc-exception-subset-after-stringbuilder-append-fact`
  models direct `StringBuilder.append`/`StringBuffer.append` through shared JDK
  platform facts; generated cargo errors drop to 38 with invalid patterns still
  all zero.
- `.tmp/corpus-fastqc-exception-subset-after-enum-varargs-dyntrait-fix`
  preserves enum constant labels and lowers enum constants passed to source
  interface slots as `&LocalizedFormats::... as &dyn Localizable`; generated
  cargo errors drop to 36 with invalid patterns still all zero.
- `.tmp/corpus-fastqc-exception-subset-after-enum-trait-impl-fix` emits a
  source Java enum trait impl fallback for zero-arg `String` interface methods,
  removing the `LocalizedFormats: Localizable` trait-bound family; generated
  cargo errors drop to 33 with invalid patterns still all zero.
- `.tmp/corpus-fastqc-exception-subset-after-self-temp-seed-fix` reuses
  self-dependent constructor field initializers as temporary seeds when the
  lowered initializer does not read `__java_self`; generated cargo errors drop
  to 30 with invalid patterns still all zero. `ExceptionContext: Default` rows
  drop from 5 to 2.
- Remaining top generated-Rust families include `ExceptionContext`
  implicit source-super constructor initialization and getter/clone ownership,
  `Localizable` trait-object `as_any` recovery, erased-object/varargs
  clone/display mismatches, missing locale/static constants, and
  collection/string method gaps such as `Vec::size`.

Acceptance:

- The exception subset has either fewer invalid downcasts/placeholders or
  replaces them with more precise unsupported diagnostics.
- No broad `super(...) requires inheritance lowering` placeholder returns in
  the exception subset.
- The fix is represented as shared object-model/coercion behavior, not as a
  FastQC-specific rule.

## Phase 2: Refresh Full FastQC After The Object-Model Slice

Status: pending.

Actions:

1. Rebuild the release translator.
2. Rerun full FastQC with 10 jobs and offline cargo.
3. Capture:
   - translation count;
   - invalid-pattern table;
   - top cargo diagnostics;
   - exact output directory.
4. Update `docs/current-corpus-issue-audit.md`.
5. Pick the next largest family from fresh evidence.

Acceptance:

- Full FastQC continues to complete normally.
- The audit no longer relies on stale parser-blocker evidence.
- Remaining FastQC work is ranked by current counts.

## Phase 3: VCF Shared Support Enum Assembly

Status: pending.

Evidence:

- `htsjdk-vcf` translates 32/32 files but the generated crate has an unclosed
  delimiter.
- The likely family is shared support enum/display assembly around
  `VCFCompoundHeaderLineSupportedHeaderLineType`.

Actions:

1. Inspect the generated file around the unclosed delimiter.
2. Add a fixture for nested support enum/display/helper assembly.
3. Fix support-member assembly so braces, impl blocks, helper functions, and
   re-exports are balanced.
4. Rerun `htsjdk-vcf`.

Acceptance:

- The VCF generated crate reaches Rust typechecking instead of parser failure.
- The fixture prevents regressions in shared support enum assembly.

## Phase 4: Spark/DISQ Facade Boundary And Interface Bridge

Status: pending.

Mechanical boundary:

- True Spark distributed execution is not mechanically translated by this Java
  translator.
- Hadoop configuration, Spark, and Google Cloud Storage stay noop or handoff
  facades for now.
- Java wrapper code around those boundaries should still translate.

Actions:

1. Add a fixture for a Java class implementing an interface through a
   Spark/DISQ-facing delegate method such as `getBases`.
2. Fix required trait impl emission and borrowed/owned delegate slots for that
   shape.
3. Keep distributed execution as an explicit facade/handoff call.
4. Rerun `gatk-disq-spark-datasources`.

Acceptance:

- Missing interface impls caused by translator shape drift are fixed.
- Remaining Spark/DISQ failures are classified as facade/handoff requirements,
  not ordinary Java lowering bugs.

## Phase 5: GenomicsDB FFI Handoff

Status: pending.

Mechanical boundary:

- GenomicsDB native behavior should not become a semantic `javalibs` facade.
- The Java translator should emit stable calls into a handoff state whose
  signatures can be provided by the future C/C++ transpiler or hand-authored
  ABI facts.

Actions:

1. Add FFI handoff fixtures for Java calls into native-backed methods with
   externally provided signatures.
2. Fix duplicate generated item names for overloaded `query`-style APIs.
3. Keep native behavior out of `javalibs`; represent it as explicit FFI
   handoff.
4. Rerun `gatk-genomicsdb`.

Acceptance:

- Overload/name conflicts are resolved mechanically.
- GenomicsDB behavior remains an explicit FFI boundary.

## Phase 6: Ndarray And Matrix Corpus Cleanup

Status: pending.

Evidence:

- The local `ndarray` shim removed the dependency-cache blocker for focused
  generated crates.
- BioJava AAPROP, BioJava alignment, JAligner, JAHMM, and Trimmomatic now
  reach real generated-Rust diagnostics.

Actions:

1. Lower `Vec<Vec<T>>` and rectangular primitive 2D arrays to `ndarray` when
   element type and rectangularity are precise.
2. Preserve nested `Vec<Vec<T>>` when rows are ragged, independently mutated,
   or semantically list-like rather than matrix-like.
3. Add fixtures for:
   - rectangular primitive 2D literals;
   - row replacement;
   - indexed matrix reads/writes;
   - ragged arrays that must remain nested vectors;
   - precise method returns for matrix values instead of `Any`.
4. Rerun one ndarray-heavy corpus after each fixture family.

Acceptance:

- Matrix-like Java 2D arrays lower to stable `ndarray` shapes when safe.
- Non-matrix nested arrays retain Java-compatible nested-vector behavior.

## Phase 7: Resume Callable-Key And Fact-Unification Slices

Status: pending after noisy corpus blockers are reduced.

The original design goal remains:

- stable `CallableKey`/member identities;
- typed external field facts;
- package-specific behavior in `javalibs`;
- stream lowering through shared call/member/coercion plans;
- explicit facade ABI facts instead of Rust type-string heuristics.

Actions:

1. Finish stable keys for external API effects, argument effects, return facts,
   constructor facts, and field facts.
2. Add negative tests proving same simple names in different packages do not
   match the wrong `javalibs` fact.
3. Move any remaining non-JDK package tables out of core translator code.
4. Make stream lowering consume the same call/member/slot plans used by normal
   expressions.
5. Replace facade ABI string-shape guessing with explicit receiver, argument,
   return, result, and borrow facts.

Acceptance:

- Source summaries, external packs, user overrides, `javalibs` manifests, and
  fallback facts attach to the same callable/member identities.
- Adding a new GATK/HTSJDK/HDF5/SQLite/Arrow behavior normally edits
  `javalibs`, not `src/stdlib.rs`.

## Verification Commands

Use these after each code slice:

```sh
cargo fmt --check
cargo check --quiet
cargo test --test idiom_gaps --quiet
cargo test --test batch_translation --quiet
cargo test --test type_coercion --quiet
cargo build --release --quiet --bin new-java2rust
```

Use focused corpus reruns with 10 jobs by default:

```sh
scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 \
  --translate-timeout-sec 60 --offline-cargo \
  --primary <java-corpus-path> --out .tmp/<descriptive-output-name>
```

## Guardrails

- Keep fixtures enabled unless they document a deliberate unsupported boundary.
- Prefer semantic facts, slot conversion, member plans, signature inference, and
  runtime support contracts over rendered-string patches.
- Do not let performance changes block future global semantic analysis.
  Reducing clone churn and adding indexes are preferred; worklist changes must
  prove equivalence with current saturation semantics.
- Treat true native behavior and distributed execution as FFI/handoff
  boundaries unless there is a concrete local Rust counterpart with matching
  semantics.
