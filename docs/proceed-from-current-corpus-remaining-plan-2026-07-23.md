# Proceed From Current Corpus Remaining Plan

Date: 2026-07-23

This plan starts from the current checkout and the latest tracked audit in
`docs/current-corpus-issue-audit.md`. The active goal is to finish the current
corpus-fix slice by turning remaining generated-Rust blockers into minimized
idiom tests, translator fixes, and fresh corpus evidence.

## Current Baseline

The most recent stable focused FastQC exception-subset evidence is:

- output: `.tmp/corpus-fastqc-exception-subset-after-readint-facade`;
- translation: 9/9 files translated;
- invalid-pattern scan: 0 files;
- generated Rust: 19 reported cargo errors.

The latest in-progress local work has started the next object-model slice:

- `Object.getClass().getName()` is modeled as a typed JDK facade path in local
  changes;
- source interface collection elements now use explicit trait-object carriers
  such as `Vec<Box<dyn Localizable>>`;
- source trait ABI support for `as_any`/`clone_box` is implemented for the
  current focused paths, and `List<Localizable>.add/get` now lower through the
  shared slot conversion plan.

Treat the tree as dirty. Do not revert unrelated changes.

## Objective

Reach a fresh corpus state where:

- translator-local gates are green;
- the FastQC exception subset has fewer generated-Rust type/interface errors
  than the current 19-error baseline;
- every fixed idiom has a minimized regression test;
- `docs/current-corpus-issue-audit.md` records the fresh evidence;
- package-specific behavior stays in `javalibs` rather than `src/stdlib.rs`.

This does not require true GenomicsDB native behavior or real DISQ/Spark
distributed execution. Those should be modeled as explicit FFI/handoff or noop
facade boundaries, not mechanically reproduced by the Java translator.

## Phase 1: Finish Source Trait Object ABI

Status: done for the current FastQC exception subset slice.

Primary target:

- FastQC `Localizable`/`ExceptionContext` generated Rust failures.

Fixed failure shapes:

- collection fields rendered as trait objects need owned element carriers;
- native collection `add` tried trait downcast recovery for a borrowed
  dyn-trait argument;
- indexed `List<Localizable>.get(i)` boxed `&Box<dyn Localizable>` instead of
  cloning the boxed trait object;
- generated trait impl `clone_box` accidentally cloned `&T` when the
  implementor was not actually cloneable.

Plan:

1. Add or keep minimized idiom fixtures for:
   - source interface parameter stored in `List<Interface>`;
   - source interface collection element type;
   - enum or source type implementing the interface;
   - failed unsupported downcast cases.
2. Route native collection `add` and similar element-consuming methods through
   the same target slot conversion used by ordinary calls.
3. Derive the collection element target shape from the receiver type and source
   representation facts, so `List<Localizable>.add(pattern)` targets
   `Box<dyn Localizable>` and can use `pattern.clone_box()`.
4. Make generated trait-object ABI compile for non-clone implementors. If clone
   semantics cannot be proven, emit an explicit TODO/panic helper instead of
   accidentally requiring `T: Clone` through `self.clone()`.
5. Keep true runtime identity/downcast gaps explicit. Do not hide them behind
   string-based casts.

Acceptance:

- done: no generated `Vec<SomeTrait>` bare trait element types for the focused
  `Localizable` path;
- done: no invalid `downcast_ref::<Localizable>()` for trait-object storage in
  the focused subset;
- done: no `&Box<dyn Localizable>: Localizable` indexed-access rows in the
  latest focused subset;
- remaining: one unit-to-`Localizable` coercion and one lifetime row still need
  follow-up after erased-object carrier work exposes cleaner ownership shapes.

Verification:

- `cargo test source_interface_collection_elements_use_boxed_dyn_trait --test idiom_gaps --quiet`;
- `cargo test --test idiom_gaps --quiet`;
- focused FastQC exception subset with offline cargo:
  `.tmp/corpus-fastqc-exception-subset-after-indexed-localizable-box`, 9/9
  translated, 0 invalid-pattern files, 15 generated cargo errors.

## Phase 2: Erased Object Carrier Policy

Status: next.

Primary target:

- `Box<dyn Any>` and `Vec<Box<dyn Any>>` clone/display/serialization rows in
  FastQC exception handling.

Plan:

1. Add fixtures for erased Java `Object...` values used in:
   - varargs storage;
   - varargs forwarding;
   - `StringBuilder.append`;
   - `ObjectInputStream.readObject`;
   - `ObjectOutputStream.writeObject`.
2. Introduce or reuse a generated Java-erased-object carrier helper instead of
   calling `.clone()` or `.to_string()` directly on `dyn Any`.
3. Make display/debug/serialization behavior explicit:
   - compile-safe fallback display for unknown erased objects;
   - clone only when the source value is known cloneable or already owned in a
     cloneable carrier;
   - handoff helper for serialization when precise runtime behavior is outside
     the Java translator.
4. Revisit `ObjectInputStream.readObject()` only after `Object[]` assignment
   casts can target the erased carrier without invalid downcasts.

Acceptance:

- generated Rust no longer calls `.clone()` or `.to_string()` directly on
  `Box<dyn Any>` or `Vec<Box<dyn Any>>`;
- exception message formatting compiles through an explicit erased-object
  display helper;
- serialization gaps compile as explicit handoff helpers or remain precise
  unsupported diagnostics.

## Phase 3: Finish Small Stable JDK Facades

Status: partially done.

Already done in the tracked audit:

- `ObjectInputStream.readInt() -> i32`;
- `Locale.US`;
- `keySet().size()`;
- same-package utility owner recovery such as `ArgUtils.flatten(...)`.

Remaining stable JDK surfaces exposed by the focused subset:

- `Object.getClass().getName()`;
- `ObjectOutputStream.writeInt(int)`;
- `ObjectOutputStream.writeObject(Object)`;
- deferred `ObjectInputStream.readObject()` through erased carrier policy.

Plan:

1. Keep these as core stable JDK facts only when they are package-independent.
2. Add fixtures before each facade change.
3. Confirm facade methods attach to typed callable facts rather than simple-name
   or rendered-Rust-type heuristics.
4. Rerun the focused subset after each small facade group.

Acceptance:

- no unit placeholders for these high-confidence JDK calls when source expects
  a value;
- no new non-JDK package tables in `src/stdlib.rs`;
- deferred object stream reads are represented by the erased carrier plan, not
  ad hoc downcasts.

## Phase 4: Refresh FastQC Evidence

Status: pending Phase 1 and Phase 2 progress.

Plan:

1. Rerun the focused exception subset after the trait-object and erased-carrier
   slices:

   ```sh
   scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 \
     --translate-timeout-sec 60 --offline-cargo \
     --primary /tmp/new_java2rust_fastqc_exception_subset \
     --out .tmp/corpus-fastqc-exception-subset-after-OBJECT-CARRIER
   ```

2. Update `docs/current-corpus-issue-audit.md` with:
   - output path;
   - translated file count;
   - invalid-pattern count;
   - generated cargo error count;
   - top remaining families.
3. Once the focused subset is materially below the 19-error baseline, rerun full
   FastQC.

Acceptance:

- focused subset is still 9/9 translated;
- invalid-pattern files remain 0;
- generated Rust errors drop below the 19-error baseline or the audit explains
  a deliberate tradeoff;
- full FastQC still reaches normal generated-Rust diagnostics with no translator
  panic.

## Phase 5: Broaden Corpus Rerun And Fixture Capture

Status: pending refreshed FastQC evidence.

Tracked corpuses that should remain in the rerun set:

- `picard-programgroups`;
- `htsjdk-fastq`;
- `gatk-genomicsdb`;
- `gatk-disq-spark-datasources`;
- `htsjdk-vcf`;
- `trimmomatic`;
- `biojava-aaprop`;
- `biojava-alignment`;
- `jaligner`;
- `varscan`;
- `jahmm`;
- `fastqc`;
- `jhlabs`;
- `jsoup`.

Plan:

1. Rerun all tracked corpuses with `--jobs 10` and offline cargo where
   appropriate.
2. Include corpuses assumed to fail. Their failures are useful input for the
   next idiom batch.
3. Group top failures by shared translator idiom, not by source project.
4. Add fixtures for repeated families before implementing fixes.

Acceptance:

- clean corpuses stay clean, especially `picard-programgroups`, `htsjdk-fastq`,
  and `varscan`;
- failing corpuses have current top-error tables and invalid-pattern counts;
- each high-frequency failure family is represented by a fixture or explicitly
  documented as a non-mechanical boundary.

## Phase 6: Next Shared Idiom Families

After the FastQC exception subset no longer dominates, attack these in order:

1. VCF shared support enum/module assembly:
   - current symptom: generated crate unclosed delimiter;
   - likely fix area: support enum/display assembly and duplicate re-exports.
2. Spark/DISQ facade bridge:
   - translate ordinary Java glue;
   - model Spark execution as noop/handoff facade for now;
   - do not mechanically translate distributed execution semantics.
3. GenomicsDB handoff:
   - keep GenomicsDB out of `javalibs`;
   - translate Java setup/control code;
   - represent native behavior as FFI handoff state for the future C/C++
     transpiler.
4. Array-heavy corpuses:
   - lower `Vec<Vec<T>>` to `ndarray` when rectangular shape is mechanically
     provable;
   - keep ragged or alias-sensitive arrays as nested vectors.
5. Switch and state-machine scaffolding:
   - minimize high-frequency switch scaffold failures from BioJava, JAligner,
     VCF, and JAHMM.

## Guardrails

- Add minimized tests before behavior changes.
- Prefer shared call/member/slot plans over stream- or corpus-specific lowering.
- Do not add non-JDK package/type/method tables to `src/stdlib.rs`.
- Package-specific shims, aliases, constants, exceptions, and ABI facts belong
  in `javalibs`.
- Do not make algorithmic changes that obstruct future global semantic
  analysis. Prefer indices, reduced cloning, profile builds, and skipped no-op
  scans.
- Use `/data/henriksson/github/claude/rust_refactor` only after generated Rust
  is in a working state. It is useful for cleanup and inlining common to all
  transpilers, not for making broken generated crates typecheck.

## Standard Verification

Run these before claiming a slice is complete:

```sh
cargo fmt --check
cargo check --quiet
cargo test --test idiom_gaps --quiet
cargo test --test parser_gaps --quiet
cargo test --test batch_translation --quiet
cargo test --test type_coercion --quiet
cargo build --release --quiet --bin new-java2rust
```

For focused generated crates, use offline cargo and record the output directory
in `docs/current-corpus-issue-audit.md`.
