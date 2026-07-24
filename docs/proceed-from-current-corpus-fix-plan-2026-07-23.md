# Proceed From Current Corpus Fix Plan

Date: 2026-07-23

This is the tracked next-action plan for the current checkout. It starts from
`docs/current-corpus-issue-audit.md`,
`docs/proceed-from-current-corpus-state-plan.md`, and the latest focused
FastQC exception-subset work.

## Goal

Finish the current corpus-fix slice by turning the remaining real corpus
failures into minimized idiom fixtures, shared translator fixes, or explicit
facade/handoff boundaries.

This plan is intentionally narrower than the original fact-unification plan:
keep moving corpus failures down, but do not add package-specific non-JDK tables
to `src/stdlib.rs` and do not block the future stable callable-key/facade-fact
design.

## Current Baseline

Known clean focused corpuses:

| Corpus | Latest useful output | Status |
|---|---|---|
| `picard-programgroups` | `.tmp/corpus-batch-current-picard-programgroups` | clean |
| `htsjdk-fastq` | `.tmp/corpus-batch-current-htsjdk-fastq-after-signature-field-shape` | clean |
| `varscan` | `.tmp/corpus-batch-current-varscan-after-loop-owned-elements` | clean |

Latest focused FastQC exception-subset evidence:

| Output | Status |
|---|---|
| `.tmp/corpus-fastqc-exception-subset-after-object-map-erasure` | 9/9 translated, 0 invalid-pattern files, generated Rust still fails with 43 errors |
| `.tmp/corpus-fastqc-exception-subset-after-source-type-stub-strip` | 9/9 translated, 0 invalid-pattern files, `ExceptionContext` visibility improved, but source traits were stripped |
| `.tmp/corpus-fastqc-exception-subset-after-source-trait-stub-strip` | 9/9 translated, 0 invalid-pattern files, source traits preserved, but focused-wrapper support assembly regressed to 44 generated Rust errors |
| `.tmp/corpus-fastqc-exception-subset-after-preserved-prelude-impls` | 9/9 translated, 0 invalid-pattern files, source items exported once and support impls preserved, generated Rust fails with 40 errors |
| `.tmp/corpus-fastqc-exception-subset-after-stringbuilder-append-fact` | 9/9 translated, 0 invalid-pattern files, direct `StringBuilder.append` fact removes the append surface rows, generated Rust fails with 38 errors |
| `.tmp/corpus-fastqc-exception-subset-after-enum-trait-impl-fix` | 9/9 translated, 0 invalid-pattern files, enum constants/interface slots and Java enum trait impl fallback fixed, generated Rust fails with 33 errors |

The wrapper cleanup is now split into normal shared-support stripping plus a
source-type facade-stub stripper, so source-defined types and traits win over
generated facade stubs without removing unrelated support methods required by
the same generated module.

## Working Rules

- Use 10 jobs by default for focused corpus reruns.
- Keep each fix tied to an enabled minimized fixture.
- Fix shared planning/lowering/coercion/signature/support assembly behavior,
  not rendered strings.
- Package-specific non-JDK behavior belongs in `javalibs` manifests/shims or
  explicit facade handoff facts.
- GenomicsDB native behavior and DISQ/Spark distributed execution remain
  handoff boundaries. Java wrapper code around those calls should still
  translate.
- Preserve global semantic analysis. Performance work may add indexes, caches,
  and clone reduction, but must not fragment whole-program fact propagation.

## Phase 1: Repair Focused Wrapper Source-Type Assembly

Status: done for the currently exposed duplicate-source/stub regression.

Problem:

- Focused package crates may emit both real source items and external facade
  stubs for the same Java type.
- The current source-type stripping experiment fixed duplicate
  `ExceptionContext` visibility, but preserving source traits exposed that the
  stripping path is too broad and can remove support methods such as
  `ObjectOutputStream::write_object`, `ObjectInputStream::read_object`,
  `write_int`, and `read_int`.

Actions:

1. Inspect the generated files in
   `.tmp/corpus-fastqc-exception-subset-after-source-trait-stub-strip`.
2. Split wrapper cleanup into two steps:
   - normal shared-support stripping;
   - a dedicated source-type facade-stub stripper that only removes simple
     generated stubs for source-defined types in other modules.
3. Preserve real source modules for `struct`, `enum`, `trait`, and `type`
   declarations.
4. Do not strip unrelated support helper impls from the module that needs them.
5. Add or extend `tests/idiom_sweep_script.rs` coverage for focused package
   assembly with:
   - one source class referenced by another source class;
   - one source trait referenced by another source class;
   - generated support facade methods that must survive stripping.

Acceptance:

- The focused exception subset still has 0 invalid-pattern files.
- `ExceptionContext`, `Localizable`, and `ExceptionContextProvider` are exported
  as source items exactly once.
- Support methods required by `ExceptionContext` remain available.
- Generated Rust errors do not regress above the previous useful 43-error
  baseline because of wrapper stripping.

Verified:

- `bash -n scripts/translate-focused-package.sh`
- `cargo test focused_package_source_type_stub_stripping_is_separate_from_shared_support --test idiom_sweep_script --quiet`
- `.tmp/corpus-fastqc-exception-subset-after-preserved-prelude-impls`: 9/9
  translated, 0 invalid-pattern files, generated cargo errors reduced to 40.

## Phase 2: Fix The Next FastQC Exception-Subset Type Families

Status: pending after Phase 1.

Likely families:

| Family | Expected direction |
|---|---|
| Java serialization streams | Add JDK facade methods/shims for `ObjectInputStream` and `ObjectOutputStream` surfaces used by source code |
| `ExceptionContext` `Clone`/`Default` | Decide whether source-generated structs can derive these from field facts or require explicit constructor/default lowering |
| trait object recovery | Fix `&dyn Localizable` and `as_any`/trait-object boundary behavior through shared object model support |
| erased `Object[]` and varargs | Preserve erased object carrier semantics without invalid downcasts |
| Java collection/string methods | Direct `StringBuilder.append`/`StringBuffer.append` is now fact-backed; remaining work includes `Vec::size`, `MessageFormat::new`, locale constants, and Java-style display |
| enum/interface constants | Keep closed-enum-to-interface coercion and missing static constants fact-backed |

Recent evidence:

- `cargo test direct_string_builder_append_is_modeled_as_fluent_jdk_call --test idiom_gaps --quiet`
- `cargo test parsed_java_enum_constant_names_preserve_rust_valid_uppercase_constants --test slot_shape_design --quiet`
- `.tmp/corpus-fastqc-exception-subset-after-stringbuilder-append-fact`: 9/9
  translated, 0 invalid-pattern files, generated cargo errors reduced to 38.
- `.tmp/corpus-fastqc-exception-subset-after-enum-varargs-dyntrait-fix`: 9/9
  translated, 0 invalid-pattern files, generated cargo errors reduced to 36.
  Enum constants now pass to `Localizable` slots as
  `&LocalizedFormats::... as &dyn Localizable` without optional
  `.as_ref().expect("Java null receiver")` recovery.
- `.tmp/corpus-fastqc-exception-subset-after-enum-trait-impl-fix`: 9/9
  translated, 0 invalid-pattern files, generated cargo errors reduced to 33.
  `LocalizedFormats` now emits `impl Localizable for LocalizedFormats`, so the
  prior enum trait-bound family is gone.

Actions:

1. Pick the largest generated-Rust family from the repaired subset.
2. Add the smallest Java fixture under `examples/idiom-gaps` and a focused
   assertion in `tests/idiom_gaps.rs`.
3. Fix the shared translator or JDK facade path.
4. Run:
   - focused new test;
   - `cargo test --test idiom_gaps --quiet`;
   - relevant narrower tests such as `type_coercion` or script tests.
5. Rerun the 9-file FastQC exception subset.

Acceptance:

- Each fixed family decreases generated-Rust error count or replaces it with a
  precise unsupported diagnostic.
- No broad invalid-pattern placeholder returns to the subset.

## Phase 3: Refresh Full FastQC

Status: pending after exception-subset progress.

Actions:

1. Build the release translator.
2. Rerun full FastQC with `--jobs 10` and offline cargo.
3. Capture:
   - translation count;
   - invalid-pattern table;
   - top cargo diagnostics;
   - phase timings;
   - exact output directory.
4. Update `docs/current-corpus-issue-audit.md`.

Acceptance:

- Full FastQC completes without translator panic.
- The audit uses fresh evidence from this checkout.
- Remaining FastQC work is ranked by current counts, not stale parser blockers.

## Phase 4: Move To Other Current Corpus Families

Status: pending after FastQC refresh.

Priority order:

| Corpus/family | Next action |
|---|---|
| `htsjdk-vcf` | Minimize and fix shared support enum/display/module assembly so the generated crate parses |
| `gatk-disq-spark-datasources` | Fix Java wrapper/interface bridge issues while leaving Spark/DISQ execution as facade handoff |
| `gatk-genomicsdb` | Fix overload naming and FFI handoff signature shapes only; do not model GenomicsDB native semantics in `javalibs` |
| BioJava/JAligner/JAHMM/Trimmomatic | Group repeated array creation, switch, string/file/logging, collection/map, and trait/interface idioms |
| Ndarray-heavy code | Lower rectangular 2D primitive/object matrices to `ndarray`; keep ragged or independently-mutated rows as nested vectors |

Acceptance:

- Each corpus-family fix has a minimized fixture.
- Each rerun writes exact output paths and remaining top families to
  `docs/current-corpus-issue-audit.md`.

## Phase 5: Resume Design-Stabilizing Fact Work

Status: pending after corpus noise is lower.

Return to the original plan:

1. stable callable/member keys for external facts and planner lookups;
2. typed external field facts;
3. non-JDK package behavior moved into `javalibs`;
4. stream lowering through shared call/member/coercion plans;
5. explicit facade ABI facts instead of Rust type-string heuristics.

Do this once the current generated-Rust blockers are small enough that the
fact-layer migration can be verified by focused corpus improvements rather than
hidden behind broad parse/type noise.
