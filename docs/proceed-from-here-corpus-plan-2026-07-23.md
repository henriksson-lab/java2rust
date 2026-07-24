# Proceed From Here Corpus Plan

Date: 2026-07-23

This plan starts from the active goal to implement
`docs/corpus-fix-next-plan.md`, the current audit in
`docs/current-corpus-issue-audit.md`, and the latest focused FastQC exception
subset work. It is intended as the single next-action checklist for this
checkout.

## Goal

Finish the current corpus-fix slice by converting remaining corpus failures
into minimized fixtures, shared translator fixes, explicit `javalibs` facts, or
deliberate FFI/facade handoff boundaries.

This is not the full callable-key migration. The immediate target is to reduce
real corpus noise enough that the stable callable-key, typed-field, stream-plan,
and facade-ABI work can be verified against meaningful corpus evidence.

## Current Baseline

Known clean focused corpuses:

| Corpus | Latest useful output | Status |
|---|---|---|
| `picard-programgroups` | `.tmp/corpus-batch-current-picard-programgroups` | clean |
| `htsjdk-fastq` | `.tmp/corpus-batch-current-htsjdk-fastq-after-signature-field-shape` | clean |
| `varscan` | `.tmp/corpus-batch-current-varscan-after-loop-owned-elements` | clean |

Focused FastQC exception subset:

| Output | Status |
|---|---|
| `.tmp/corpus-fastqc-exception-subset-after-stringbuilder-append-fact` | 9/9 translated, 0 invalid-pattern files, generated cargo errors: 38 |
| `.tmp/corpus-fastqc-exception-subset-after-enum-constant-parser-fix` | 9/9 translated, 0 invalid-pattern files, generated cargo errors: 38; missing collapsed enum variants are gone, but enum constants still flow through optional `.as_ref().expect("Java null receiver")` in constructor delegation |
| `.tmp/corpus-fastqc-exception-subset-after-enum-varargs-dyntrait-fix` | 9/9 translated, 0 invalid-pattern files, generated cargo errors: 36; enum constants now pass to `Localizable` slots without optional receiver recovery |
| `.tmp/corpus-fastqc-exception-subset-after-enum-trait-impl-fix` | 9/9 translated, 0 invalid-pattern files, generated cargo errors: 33; source Java enums now emit a source-interface trait impl fallback for zero-arg `String` methods |
| `.tmp/corpus-fastqc-exception-subset-after-self-temp-seed-fix` | 9/9 translated, 0 invalid-pattern files, generated cargo errors: 30; self-temporary constructor seeds reuse constructor initializers when they do not read `__java_self` |

Local gates were green before the current enum-constructor fixture was added.
After finishing the fixture, reverify them instead of assuming the baseline
still holds.

## Working Rules

- Use 10 jobs by default for focused corpus reruns.
- Keep each code fix tied to an enabled minimized fixture.
- Fix planner, signature, coercion, representation, facade fact, or support
  assembly behavior rather than emitted Rust strings.
- Do not add non-JDK package-specific behavior to `src/stdlib.rs`.
- Put package-specific library behavior in `javalibs` manifests/shims, or mark
  it as an explicit FFI/handoff boundary.
- Preserve future global semantic analysis. Performance improvements should be
  indexes, caches, clone reduction, and equivalent scoped propagation, not
  fragmented local analysis.
- Treat GenomicsDB native behavior, DISQ/Spark distributed execution, Hadoop
  runtime behavior, and Google Cloud Storage IO as handoff/noop facade
  boundaries unless a concrete Rust counterpart is selected.

## Phase 1: Finish Enum Constant To Interface Coercion

Status: done.

Problem:

The parser now preserves Java enum constant labels such as
`MAX_COUNT_EXCEEDED` and `ILLEGAL_STATE`, and representation can emit those
labels as Rust enum variants. The remaining focused fixture still emits:

```rust
LocalizedFormats::ILLEGAL_STATE.as_ref().expect("Java null receiver")
```

when a closed enum constant is passed to a source interface/dyn-trait
constructor slot. The intended Rust shape is:

```rust
&LocalizedFormats::ILLEGAL_STATE as &dyn Localizable
```

Actions:

1. Inspect `src/lower.rs` around
   `lower_java_enum_static_field_access_for_slot` and confirm whether the
   source-interface enum-owner guard is present and actually reached.
2. If the slot shape lacks the expected dyn-trait flags, add a direct source
   interface check: when the enum owner is a source enum and directly
   implements the source interface named by the target slot, lower the enum
   constant as a shared dyn-trait borrow.
3. Keep the check proof-based. Do not allow arbitrary closed enum values to
   become arbitrary trait objects.
4. Re-run:

```sh
cargo test closed_enum_variant_constructor_delegation_supplies_empty_varargs_without_optional_as_ref --test idiom_gaps --quiet
cargo test parses_enum_constants_preserving_java_constant_labels --test parser_gaps --quiet
cargo test closed_enum_variant_borrows_as_dyn_trait_parameter --test idiom_gaps --quiet
cargo test enum_constant_with_constructor_args_lowers_as_unit_variant_access --test idiom_gaps --quiet
cargo test parsed_java_enum_constant_names_preserve_rust_valid_uppercase_constants --test slot_shape_design --quiet
```

Acceptance:

- The constructor-delegation fixture emits a dyn-trait borrow for the enum
  constant.
- Empty Java varargs are supplied as `Vec::new()`.
- No `.as_ref().expect("Java null receiver")` remains for this enum-constant
  path.

Verified:

- `cargo test closed_enum_variant_constructor_delegation_supplies_empty_varargs_without_optional_as_ref --test idiom_gaps --quiet`
- `cargo test closed_enum_variant_borrows_as_dyn_trait_parameter --test idiom_gaps --quiet`
- `cargo test enum_constant_with_constructor_args_lowers_as_unit_variant_access --test idiom_gaps --quiet`
- `cargo test parses_enum_constants_preserving_java_constant_labels --test parser_gaps --quiet`
- `cargo test parsed_java_enum_constant_names_preserve_rust_valid_uppercase_constants --test slot_shape_design --quiet`

## Phase 2: Rerun The FastQC Exception Subset

Status: done for the enum/interface slice.

Actions:

1. Run the core local gates:

```sh
cargo fmt --check
cargo check --quiet
cargo test --test idiom_gaps --quiet
cargo test --test parser_gaps --quiet
cargo test --test batch_translation --quiet
cargo test --test type_coercion --quiet
cargo build --release --quiet --bin new-java2rust
```

2. Rerun the 9-file exception subset:

```sh
scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 \
  --translate-timeout-sec 60 --offline-cargo \
  --primary /tmp/new_java2rust_fastqc_exception_subset \
  --out .tmp/corpus-fastqc-exception-subset-after-enum-varargs-dyntrait-fix
```

3. Record generated cargo error count, invalid-pattern count, and top error
   families in `docs/current-corpus-issue-audit.md`.

Acceptance:

- The subset stays 9/9 translated.
- Invalid-pattern files remain at 0.
- The enum-constant `.as_ref()` family is gone or reduced.
- Any new remaining rows are ranked by count and tied to the next fixture.

Verified:

- `.tmp/corpus-fastqc-exception-subset-after-enum-varargs-dyntrait-fix`: 9/9
  translated, 0 invalid-pattern files, generated cargo errors: 36.
- `.tmp/corpus-fastqc-exception-subset-after-enum-trait-impl-fix`: 9/9
  translated, 0 invalid-pattern files, generated cargo errors: 33.
- Generated calls include
  `&LocalizedFormats::MAX_COUNT_EXCEEDED as &dyn Localizable`,
  `&LocalizedFormats::ILLEGAL_STATE as &dyn Localizable`, and
  `&LocalizedFormats::EVALUATIONS as &dyn Localizable`.
- Generated `LocalizedFormats` includes
  `impl Localizable for LocalizedFormats`.

## Phase 3: Fix The Remaining Exception-Subset Type Families

Status: pending.

Likely families, based on the latest subset:

| Family | Direction |
|---|---|
| `ExceptionContext` `Default`/`Clone` | Decide whether source structs should derive these from field/constructor facts or whether constructor/default lowering should avoid requiring them |
| `Localizable` trait-object recovery | Fix shared trait-object support, `as_any` expectations, and enum/interface object boundaries |
| Erased `Object[]` and varargs | Keep erased-object carriers and Java varargs aligned without invalid downcasts |
| Locale/static constants | Add typed JDK/static field facts or conservative facade constants with provenance |
| Collection/string surface gaps | Add shared JDK facts/support for remaining ordinary Java collection/string methods |

Loop for each family:

1. Minimize the generated Rust error to a Java fixture in
   `tests/idiom_gaps.rs` or a narrower module.
2. Fix the shared translator or JDK facade path.
3. Run the focused test and local gates affected by the change.
4. Rerun the 9-file exception subset.
5. Update `docs/current-corpus-issue-audit.md` with exact output path and
   counts.

Acceptance:

- Each fix either reduces generated cargo errors, removes an invalid-pattern
  row, or replaces an unsafe fallback with a precise unsupported diagnostic.
- No broad constructor, cast, or `compile_error!` placeholder returns.

Latest evidence:

- `.tmp/corpus-fastqc-exception-subset-after-self-temp-seed-fix`: 9/9
  translated, 0 invalid-pattern files, generated cargo errors: 30.
- `ExceptionContext: Default` rows dropped from 5 to 2 after self-temporary
  constructor seeds began reusing lowerable constructor initializers.
- The remaining `ExceptionContext: Default` rows come from inherited `context`
  defaults in constructors that rely on implicit Java source-superclass
  construction. The next fixture should model a child constructor with no
  explicit `super(...)`, an inherited field initialized by the superclass
  zero-arg constructor, and trailing child field assignments/calls.

## Phase 4: Refresh Full FastQC

Status: pending after the exception subset stops exposing high-count shared
type/interface blockers.

Actions:

1. Build the release translator.
2. Rerun full FastQC with `--jobs 10` and offline cargo.
3. Capture translation count, invalid-pattern table, top cargo diagnostics,
   phase timings, and exact output directory.
4. Update `docs/current-corpus-issue-audit.md`.

Acceptance:

- Full FastQC completes without translator panic.
- Current parser blockers remain gone.
- Remaining work is ranked from fresh evidence rather than stale scans.

## Phase 5: Expand Back To Other Corpuses

Status: pending after the FastQC refresh.

Priority order:

| Corpus/family | Next action |
|---|---|
| `htsjdk-vcf` | Fix shared support enum/display/module assembly so the generated crate parses |
| `gatk-disq-spark-datasources` | Fix Java wrapper/interface bridge issues while keeping Spark/DISQ as handoff facades |
| `gatk-genomicsdb` | Fix overload naming and FFI handoff signature shapes only; do not put GenomicsDB native semantics in `javalibs` |
| BioJava/JAligner/JAHMM/Trimmomatic | Group repeated matrix, switch, string/file/logging, collection/map, and trait/interface idioms |
| Ndarray-heavy code | Lower rectangular 2D arrays/matrices to `ndarray` when type and rectangularity are precise; preserve ragged or independently mutated rows as nested vectors |

Acceptance:

- Each corpus-family fix has a minimized fixture.
- Corpus reruns use exact output paths and refresh
  `docs/current-corpus-issue-audit.md`.

## Phase 6: Resume Fact-Unification Work

Status: pending after corpus noise is lower.

Return to the original design-stabilizing plan:

1. Stable `CallableKey`/member identities for external API effects, argument
   effects, return facts, constructor facts, and field facts.
2. Typed external field facts with `type=` and `const-kind=` manifest support.
3. Non-JDK package behavior moved into `javalibs` manifests/shims.
4. Stream lowering consuming shared call/member/coercion/slot plans.
5. Explicit facade ABI facts for receiver style, argument slots, return slots,
   result wrapping, borrow source, and exception conversion.

Acceptance:

- Source summaries, external packs, user overrides, `javalibs` manifests, and
  fallback facts attach to the same callable/member identities.
- Negative tests prove same simple names in different packages do not match the
  wrong library facts.
- Adding a new GATK/HTSJDK/HDF5/SQLite/Arrow behavior normally edits
  `javalibs`, not `src/stdlib.rs`.
