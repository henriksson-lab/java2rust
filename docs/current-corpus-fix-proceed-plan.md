# Current Corpus Fix Proceed Plan

Date: 2026-07-23

This plan starts from the current dirty checkout and the latest focused FastQC
AbstractQCModule evidence in
`docs/proceed-from-current-fastqc-abstract-plan-2026-07-23.md`.

The immediate goal is to keep turning real corpus failures into enabled idiom
fixtures and shared translator fixes. Use `--jobs 10` for focused corpus reruns.

## Current Baseline

Latest useful focused evidence:

- `.tmp/corpus-fastqc-abstract-qcmodule-after-inner-outer-capture-r2`
- 8/8 files translated.
- 0 translation failures.
- 0 invalid-pattern files.
- Offline cargo reaches generated Rust and fails with 154 reported errors.

Closed in that focused slice:

- receiverless `getClass().getResourceAsStream(...)`;
- `BufferedImage::TYPE_INT_RGB`;
- typed `BufferedImage.createGraphics() -> Graphics`;
- `Graphics.dispose()`;
- typed `JPanel.print(Graphics)` argument handling;
- typed `InputStream.read(byte[])` and `InputStream.close()` for resource
  streams;
- conventional external `VERSION` static string constants in string slots;
- proven non-static inner class outer-field reads now lower through an explicit
  `__outer: Rc<RefCell<Outer>>` capture handle instead of bare unresolved Rust
  locals;
- earlier `File`, `String.getBytes`, `String.indexOf`, and optional collection
  indexing rows.

Current top focused blockers:

| Family | Status |
|---|---|
| `InputStream.read(byte[])` / `close()` | done; r3 has no missing read/close rows |
| non-static inner class outer capture | partial done; r2 has no bare `sb`/`c_width`/`c_height`/`g` unresolved rows, but constructor handoff still uses a typed outer-capture panic and optional outer `g` calls remain |
| string-slot static constant downcast | done for conventional external `VERSION`; r3 has 0 invalid-pattern files |
| `StringBuffer.append(...)` argument shaping | new dominant family after outer capture: many mismatched types under `self.__outer.borrow().sb.append(...)` |
| optional outer `g` facade calls | exposed as `Option<T>` missing `get_font_metrics`/`get_clip`/`draw_image` |

## Rules For This Slice

1. Add or keep an enabled minimized fixture before each translator behavior fix.
2. Prefer fact/planner/slot/coercion/lowering fixes over rendered-string edits.
3. Keep stable JDK platform support in `src/stdlib.rs`; keep non-JDK package
   behavior in `javalibs` manifests or explicit generated-crate shims.
4. If a raw support type or method is added for focused wrapper checks, mirror
   it in `scripts/translate-focused-package.sh`.
5. Do not make performance or algorithm changes that block future global
   semantic analysis. Indexes and clone reduction are acceptable; fragmented
   per-file semantics are not.

## Phase 1: Finish The InputStream Slice

Status: done.

Known intended changes:

- `tests/idiom_gaps.rs` has a resource-stream fixture that reads into a
  `byte[]` buffer and closes the stream.
- `src/exception_lowering.rs` and `scripts/translate-focused-package.sh` should
  expose concrete `InputStream::read` and `InputStream::close` support.
- `src/external_packs.rs` should model `InputStream.read(byte[])` as a borrowed
  mutable argument effect, so generated loops call `.read(&buffer)` rather than
  moving the buffer.

Next commands:

```sh
rg -n "enum ReceiverMutabilityFact|InputStream" src tests scripts
cargo test --test idiom_gaps resource_input_stream_read_and_close_have_typed_facade_support --quiet
```

If the fixture fails:

- fix the argument-effect enum variant if it does not compile;
- inspect generated Rust from the fixture;
- ensure `read(byte[])` is planned through the shared argument-effect path;
- keep the concrete support signature borrowed if the buffer is reused in a
  loop.

Acceptance:

- the fixture passes;
- generated Rust contains typed `.read(&buffer)` or equivalent non-moving
  borrowed argument behavior;
- generated Rust has concrete `pub fn read` and `pub fn close` support;
- no local `InputStream.read` or `InputStream.close` errors remain in the
  focused AbstractQCModule rerun.

Verified:

- `cargo test --test idiom_gaps resource_input_stream_read_and_close_have_typed_facade_support --quiet`
- `cargo test --test slot_shape_design nullable_receiver_external_argument_facts_shape_borrowed_stream_read_args --quiet`
- `cargo test --test slot_shape_design platform_mutable_external_argument_marks_local_binding_mutable --quiet`
- `cargo test --test slot_shape_design conventional_external_version_constant_append_does_not_downcast --quiet`
- `cargo test --test idiom_gaps --quiet` with 361 passing tests
- `cargo fmt --check`
- `cargo check --quiet`
- `cargo build --release --quiet --bin new-java2rust`
- focused rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-inputstream-resource-facade-r3`
  with 8/8 translated, 0 invalid-pattern files, and 174 generated Rust errors.

## Phase 2: Restore Local Gates

Run the local gates after the InputStream fixture passes:

```sh
cargo fmt --check
cargo check --quiet
cargo test --test idiom_gaps --quiet
cargo test --test batch_translation --quiet
cargo test --test type_coercion --quiet
bash -n scripts/translate-focused-package.sh
cargo build --release --quiet --bin new-java2rust
```

If any gate fails, fix only the regression introduced by the current slice.
Do not clean up unrelated dirty files.

## Phase 3: Rerun Focused AbstractQCModule

Rerun the focused subset:

```sh
scripts/translate-focused-package.sh \
  --primary .tmp/fastqc-abstract-qcmodule-subset \
  --out .tmp/corpus-fastqc-abstract-qcmodule-after-inputstream-resource-facade \
  --skip-resolver-bench \
  --jobs 10 \
  --translate-timeout-sec 60 \
  --offline-cargo
```

Record:

- `summary.tsv`;
- `top-compile-errors.tsv`;
- `invalid-patterns.tsv`;
- any generated lines for remaining top families.

Observed movement:

- still 8/8 translated;
- still 0 invalid-pattern files;
- `InputStream.read` and `InputStream.close` rows gone;
- cargo error count drops from the r7 count of 186 to 174.

Update:

- `docs/current-corpus-issue-audit.md`;
- `docs/proceed-from-current-fastqc-abstract-plan-2026-07-23.md`;
- `docs/corpus-fix-next-plan.md`.

## Phase 4: Decide The Next Real Fix

After InputStream is closed, rank the new focused top errors.

Preferred next target is non-static inner class outer capture if it remains the
dominant family.

Plan for inner capture:

1. Add a minimized fixture for a non-static inner class reading an outer field.
2. Add a second fixture for mutation only after the read-only shape is clear.
3. Represent proven outer captures with explicit source-summary capture facts or
   a typed outer handle on the generated inner struct.
4. If Java object identity/aliasing cannot be represented safely, emit one
   precise unsupported diagnostic rather than unresolved bare identifiers.

Acceptance:

- `SVGGenerator.SVGGraphics` no longer emits bare unresolved `sb`, `c_width`,
  `c_height`, or `g`;
- unsupported aliasing cases are explicit and localized;
- no FastQC-specific table is added to `src/stdlib.rs`.

Status:

- first slice done in
  `.tmp/corpus-fastqc-abstract-qcmodule-after-inner-outer-capture-r2`.
- Added `non_static_inner_class_outer_fields_lower_through_outer_capture_handle`.
- Added `CaptureFacts::outer_this_type_for_type` and lowered proven outer
  fields through `self.__outer.borrow().field`.
- Generated Rust errors drop from 174 to 154; the bare outer-field unresolved
  rows disappear.
- Display-capable `StringBuilder`/`StringBuffer.append(...)` support is now
  covered by a typechecking outer-capture fixture and reduces the focused subset
  to 63 generated Rust errors in
  `.tmp/corpus-fastqc-abstract-qcmodule-after-display-append-r1`.
- Nullable constructor field slots now preserve nullable static facade returns
  such as `System.getProperty(...)` instead of emitting `Some(Option<T>)`;
  focused evidence is
  `.tmp/corpus-fastqc-abstract-qcmodule-after-nullable-constructor-slot-r1`
  with 62 generated Rust errors.

Next actions:

1. Replace the typed constructor panic with real outer handle threading for
   `new Inner()` call sites created from an outer instance.
2. Add an optional outer-field method-call fixture for nullable fields such as
   `g`.
3. Fix source/facade method return shapes that currently surface as `()`, for
   example `TableModel.getColumnCount()`, `getColumnName(...)`, and
   `getValueAt(...)`.
4. Fix remaining private source-field access such as `FastQCConfig` fields.

## Phase 5: Broaden Only After Focused Movement

When the focused AbstractQCModule subset improves, refresh broader corpus
evidence:

1. Full FastQC.
2. `htsjdk-vcf`.
3. `gatk-disq-spark-datasources`, keeping Spark/DISQ distributed execution as
   facade handoff.
4. `gatk-genomicsdb`, keeping GenomicsDB native behavior out of `javalibs` and
   limiting work to FFI handoff/signature shapes.
5. ndarray-heavy corpuses, preserving `ndarray` for rectangular numeric 2D
   arrays and nested vectors for ragged or independently mutated rows.

For each family:

- gather the latest top compile/idiom rows;
- add fixture candidates for the repeated idioms;
- fix shared translator behavior in this checkout;
- rerun with `--jobs 10`;
- update `docs/current-corpus-issue-audit.md`.

## Parallel Work Policy

Use parallel workers for investigation, fixture mining, and independent audits.
Keep implementation in this checkout when a fix touches shared translator code
and `javalibs` together.

Good parallel batches:

- inspect generated errors and map them back to Java source;
- propose minimized fixtures for each error family;
- audit whether a family belongs in JDK support, `javalibs`, FFI handoff, or
  explicit unsupported diagnostics;
- compare focused rerun outputs before and after a fix.

Avoid parallel workers making overlapping edits to core planner/lowering files
unless each worker uses a separate clone and reports a final status file before
merge.

## Stop Criteria

Stop and reassess when one of these happens:

- local gates cannot be restored without undoing unrelated user work;
- a corpus family requires Java object identity, GUI runtime behavior, native
  GenomicsDB semantics, or Spark distributed execution rather than a typed
  facade/handoff;
- the next largest blocker is no longer visible in focused evidence and needs a
  broader rerun before coding.
