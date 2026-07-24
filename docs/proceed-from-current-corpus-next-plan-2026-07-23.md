# Proceed From Current Corpus Next Plan

Date: 2026-07-23

This plan starts from the current dirty checkout and the latest focused FastQC
exception-subset run:

- output: `.tmp/corpus-fastqc-exception-subset-after-object-stream-erased-identity`
- translation: 9/9 files translated
- invalid-pattern scan: 0 files
- generated Rust: 12 reported cargo errors

The immediate goal is to keep making corpus-backed idiom fixes in this checkout,
with minimized tests for every behavior change and fresh focused corpus evidence
after each slice.

## Guardrails

- Preserve the current local gates:
  - `cargo fmt --check`
  - `cargo check --quiet`
  - `cargo test --test idiom_gaps --quiet`
  - `cargo test --test parser_gaps --quiet`
  - `cargo test --test batch_translation --quiet`
  - `cargo test --test type_coercion --quiet`
  - `cargo build --release --quiet --bin new-java2rust`
- Keep non-JDK package behavior out of `src/stdlib.rs`.
- Prefer typed facts, slot plans, and callable/member plans over rendered Rust
  string heuristics.
- Do not let local performance shortcuts block later global semantic analysis.
- Treat GenomicsDB native execution, DISQ/Spark distributed execution, and other
  cross-runtime behavior as explicit handoff/facade boundaries, not mechanical
  Java-to-Rust rewrites.
- Use `/data/henriksson/github/claude/rust_refactor` later for cleanup/inlining
  only after generated Rust is already in a working state.

## Phase 1: Lock In The Current FastQC Exception Baseline

Status: done for the current focused slice.

Actions:

1. Inspect the latest focused compile log and generated files for the 14-error
   set.
2. Update `docs/current-corpus-issue-audit.md` with the
   `.tmp/corpus-fastqc-exception-subset-after-borrowed-context-getter` evidence.
3. Confirm whether all previous `ExceptionContext.clone()` rows are gone, or
   whether one ownership path still returns an owned non-cloneable context.

Acceptance:

- The audit records the exact current top-error families.
- Any remaining `ExceptionContext` clone/borrow mismatch is isolated before
  starting the erased-object carrier work.

Evidence:

- `.tmp/corpus-fastqc-exception-subset-after-borrowed-context-getter`: 9/9
  translated, 0 invalid-pattern files, 14 generated cargo errors.
- `.tmp/corpus-fastqc-exception-subset-after-object-stream-erased-identity`: 9/9
  translated, 0 invalid-pattern files, 12 generated cargo errors after typed
  JDK object stream facades and erased-object identity assignment.

## Phase 2: Add An Explicit Erased Java Object Carrier Policy

Status: in progress.

Current top errors include direct operations on erased carriers:

- `Vec<Box<dyn Any>>::to_string()`
- `Vec<Box<dyn Any>>::clone()`
- `Box<dyn Any>::clone()`
- value/lifetime issues caused by returning or boxing borrowed erased map
  contents

Actions:

1. Add minimized fixtures for:
   - `Object...` varargs stored in fields or lists;
   - forwarding erased varargs;
   - appending erased objects to `StringBuilder`/string concat;
   - `Map<String, Object>` value retrieval;
   - `ObjectInputStream.readObject()` and `ObjectOutputStream.writeObject(...)`
     as explicit serialization handoff points.
2. Introduce or reuse a generated Java-erased-object support surface so lowering
   calls helper functions instead of calling `.clone()` or `.to_string()`
   directly on `dyn Any`.
3. Make clone/display/serialization semantics explicit:
   - compile-safe display fallback for unknown erased objects;
   - clone only through a deliberate erased-carrier helper;
   - serialization as a handoff helper unless precise runtime behavior is known.
4. Rerun the focused FastQC exception subset.

Progress:

- `ObjectInputStream.readObject()` now returns the explicit erased-object
  carrier through a fallible JDK facade.
- `ObjectOutputStream.writeInt(int)` and `writeObject(Object)` now lower as
  fallible side-effect facade calls.
- Erased-object values flowing into Java `Object` slots use identity dispatch
  instead of invalid placeholder-object downcasts.

Acceptance:

- Generated Rust has no direct `.clone()` or `.to_string()` calls on
  `Box<dyn Any>` or `Vec<Box<dyn Any>>`.
- Erased-object formatting compiles through an explicit helper.
- Serialization gaps are represented as typed helpers or precise unsupported
  diagnostics, not accidental unit values or invalid downcasts.

## Phase 3: Finish The Remaining Stable JDK Object Stream Facades

Status: done for the current JDK object-stream surface.

Actions:

1. Add focused fixtures for:
   - `ObjectOutputStream.writeInt(int)`
   - `ObjectOutputStream.writeObject(Object)`
   - `ObjectInputStream.readObject()`
2. Keep these as JDK-level facts only.
3. Route result and argument shapes through shared conversion planning.
4. Rerun the focused FastQC exception subset.

Acceptance:

- `writeInt`/`writeObject` do not lower to unresolved calls or unit placeholders.
- `readObject` returns the explicit erased carrier or handoff shape expected by
  downstream assignments.

Evidence:

- Regression test:
  `cargo test object_stream_object_methods_have_typed_handoff_facades --test idiom_gaps --quiet`.
- Focused corpus:
  `.tmp/corpus-fastqc-exception-subset-after-object-stream-erased-identity`,
  with 0 invalid-pattern files and no `downcast::<Object>()` regression.

## Phase 4: Fix The Remaining Source Trait/Object Follow-Ons

Status: pending cleaner erased-object output.

Known remaining families:

- one erased-carrier-to-`Localizable` row;
- one missing lifetime row;
- one `String: Borrow<&String>` row;
- any remaining mismatched-type rows after the erased carrier and stream facade
  fixes.

Actions:

1. Reclassify these after Phase 2 and Phase 3, because several may disappear
   once erased-object values have stable ownership/display behavior.
2. Add one minimized fixture per surviving family.
3. Fix through signature/slot/coercion planning, not local rendered-string
   patches.

Acceptance:

- The focused FastQC exception subset typechecks, or the remaining failures are
  reduced to explicit unsupported runtime handoff behavior with fixtures.

## Phase 5: Broaden Corpus Reruns And Capture New Idioms

Status: after focused FastQC exception subset improves materially.

Rerun tracked corpuses with `--jobs 10`:

- `picard-programgroups`
- `htsjdk-fastq`
- `htsjdk-vcf`
- `fastqc`
- `trimmomatic`
- `biojava-aaprop`
- `biojava-alignment`
- `jaligner`
- `gatk-genomicsdb`
- `gatk-disq-spark-datasources`

Actions:

1. Record translation count, invalid-pattern count, generated-Rust error count,
   and top idiom families for each corpus.
2. Add fixtures for any recurring idiom that does not already have coverage.
3. Keep GenomicsDB/DISQ/Spark runtime behavior as facade or FFI handoff
   boundaries unless the code is ordinary Java utility code with no distributed
   or native runtime dependency.

Acceptance:

- Each failing corpus has a current audit row and at least one fixture candidate
  for its largest remaining mechanical idiom family.
- The next implementation batch can be selected from measured frequency, not
  guesswork.

## Phase 6: Reconcile With The Stable Callable-Key Goal

Status: continuous, but not allowed to block corpus-backed fixes.

Actions:

1. When touching external/JDK/package facts, attach behavior to stable
   callable/member identities.
2. Keep partial external matches explicit and provenance-bearing.
3. Move any package-specific method/function list into `javalibs` manifests or
   facade notes, not hardcoded transpiler tables.
4. Prefer shared call/member/coercion plans for both ordinary expression
   lowering and stream lowering.

Acceptance:

- New fixes do not add non-JDK package tables to `src/stdlib.rs`.
- New idiom tests assert planned behavior in a way compatible with later
  callable-key unification.
