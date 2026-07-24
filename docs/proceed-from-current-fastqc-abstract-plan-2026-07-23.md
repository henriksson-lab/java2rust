# Proceed From Current FastQC AbstractQCModule State

Date: 2026-07-23

This plan starts from the focused FastQC AbstractQCModule rerun:

- `.tmp/corpus-fastqc-abstract-qcmodule-after-file-facades-r3`
- Command shape:
  `scripts/translate-focused-package.sh --primary .tmp/fastqc-abstract-qcmodule-subset --out .tmp/corpus-fastqc-abstract-qcmodule-after-file-facades-r3 --skip-resolver-bench --jobs 10 --translate-timeout-sec 60 --offline-cargo`

Previous focused state:

- 8 files translated.
- 0 translation failures.
- 0 invalid-pattern files.
- Offline `cargo check` reaches generated Rust and fails with 198 reported
  errors.
- The previous missing `java.io.File` instance facade rows are gone after
  adding `getName`, `getParent`, `getAbsoluteFile`, `isDirectory`, and `mkdir`
  support to both `src/stdlib.rs` and the focused wrapper support block.

Previous focused state:

- `.tmp/corpus-fastqc-abstract-qcmodule-after-getclass-awt-facades-r7`
- 8 files translated.
- 0 translation failures.
- 0 invalid-pattern files.
- Offline `cargo check` reaches generated Rust and fails with 186 reported
  errors.
- Closed since the r3 checkpoint:
  - `Option<&Vec<_>>` indexing from `group.get(0)[n]`;
  - missing `String.get_bytes`;
  - missing `String.index_of`;
  - missing focused-wrapper support for `java_string_index_of`.
  - unresolved free `get_class()` for receiverless `getClass()`;
  - missing `BufferedImage::TYPE_INT_RGB`;
  - `BufferedImage.createGraphics()` erased to `Box<dyn Any>`;
  - invalid `Graphics` downcasts around `JPanel.print(graphics)`;
  - missing `Graphics.dispose()` on typed graphics locals.

Intermediate focused state:

- `.tmp/corpus-fastqc-abstract-qcmodule-after-inputstream-resource-facade-r3`
- 8 files translated.
- 0 translation failures.
- 0 invalid-pattern files.
- Offline `cargo check` reaches generated Rust and fails with 174 reported
  errors.
- Closed since the r7 checkpoint:
  - missing `InputStream.read(byte[])` and `InputStream.close()`;
  - invalid `FastQCApplication.VERSION` string-slot downcast;
  - unresolved focused support for conventional external `VERSION` static
    string constants.

Previous focused state:

- `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-local-graphics-r4`
- 8 files translated.
- 0 translation failures.
- 0 invalid-pattern files.
- Offline `cargo check` reaches generated Rust and fails with 55 reported
  errors.
- Closed since the input-stream checkpoint:
  - bare non-static inner-class outer-field rows for `sb`, `c_width`,
    `c_height`, and `g`;
  - most `StringBuilder`/`StringBuffer.append(...)` display-slot mismatches;
  - double-wrapped nullable constructor facade fields;
  - constructor field assignment compaction that used constructor-local
    variables before declaration;
  - `BufferedImage.getGraphics()` missing/erased facade support;
  - `SVGGenerator.g` erased storage for the same-typed AWT return.

Latest focused state:

- `.tmp/corpus-fastqc-abstract-qcmodule-after-gui-erased-display-r2`
- 8 files translated.
- 0 translation failures.
- 0 invalid-pattern files.
- Offline `cargo check` reaches generated Rust and fails with 39 reported
  errors.
- Closed since the constructor-local graphics checkpoint:
  - `Component.getWidth/getHeight`, `Color.getRed/getGreen/getBlue`,
    `Font.getSize/isBold`, and `TableModel` count/name/value calls no longer
    return `()`;
  - `String.valueOf(table.getValueAt(...))`, XML writer display slots, and
    `StringBuffer.append(table.getValueAt(...))` lower erased values through
    `java_erased_object_to_string(&table.get_value_at(...))`;
  - the invalid Java cast scanner reports 0 files for this subset.

Top current generated-Rust families:

Latest focused evidence:

- `.tmp/corpus-fastqc-abstract-qcmodule-after-zip-outputstream-adapter-r2`
- 8/8 translated
- 0 invalid-pattern files
- offline `cargo check` reaches generated Rust and reports 31 errors
- source borrowed getter locals recover owned locals through `.cloned()` or
  `.clone()`, and local-shadowed JDK `ZipFile` receivers no longer inherit the
  optional shape of same-named fields. `ZipFile.entries()`,
  `ZipFile.getInputStream(...)`, and `ZipFile.close()` now use typed fallible
  platform support. Constructor field assignments whose RHS reads another
  receiverless instance field now defer until `__java_self` exists, and
  UpperCamel static-owner field access such as `Color.BLACK`/`Font.PLAIN`
  no longer collapses to same-named instance fields. Deferred constructor RHS
  lowering now applies self-field slot recovery before substituting
  `__java_self`, so nested source-constructor args no longer clone the whole
  partially built struct. Actual-argument-aware JDK constructor slot planning
  now distinguishes `FileOutputStream(File)` from `FileOutputStream(String)`;
  the generated `HTMLReportArchive` constructor recovers
  `__java_self.zip_file.as_ref().expect(...).clone()` instead of passing the
  nullable field directly. JDK `ZipOutputStream` now converts to
  `OutputStream` via platform conversion adapter facts, and reused values clone
  before `.into()`. Apache Commons Codec `Base64OutputStream` and FastQC
  iHarder `Base64.OutputStream` now use external-pack conversion facts for the
  same `OutputStream` slot. Java public source fields now remain public in the
  generated Rust struct, closing the FastQC `svg_output` and `line_separator`
  visibility rows. Source `static final Pattern` fields now emit associated
  default facade values, closing the `SVGImageSaver` pattern constant rows.

| Count | Family | Likely Source |
|---:|---|---|
| 8 | mismatched types | Mostly remaining source/facade return slots and facade argument/return slot drift |
| 4 | type annotations needed | Remaining generated support/source inference gaps |
| 6 | `()` display/display-adjacent rows | Void expressions flowing into string/display slots |
| 1 | `HTMLReportArchive` clone from source argument passing | Remaining source method argument slot ownership row in `AbstractQCModule.write_xhtml_table(report, table)` |
| 0 | external stream facade conversion | JDK `ZipOutputStream`/`OutputStream`, Apache Commons Codec `Base64OutputStream`/`OutputStream`, and FastQC iHarder `Base64.OutputStream`/`OutputStream` compatibility are fixed in focused checks |
| 0 | private `FastQCConfig` field access | Source visibility now preserves Java public fields in focused checks |
| 0 | missing `SVGImageSaver` pattern constants | Static source Pattern constants are fixed in focused checks |

## Goal For The Next Slice

Reduce the focused AbstractQCModule subset to a smaller, more typed set of
generated-Rust failures without weakening the larger design constraints:

- preserve green local translator gates;
- add fixture-sized tests before each fix;
- prefer shared planner/fact behavior over corpus-specific string matching;
- keep non-JDK package behavior outside `src/stdlib.rs` unless it is only a
  generated-crate support shim for focused corpus checking.

## Work Order

### 1. Record The Current File Facade Fix

Status: implementation appears done; documentation and full gate proof remain.

Actions:

1. Add the r3 focused result to `docs/current-corpus-issue-audit.md`.
2. Add a short progress bullet to `docs/corpus-fix-next-plan.md`.
3. Run:
   - `cargo fmt --check`
   - `cargo check --quiet`
   - `cargo test --test idiom_gaps jdk_file_common_instance_facades_compile --quiet`
   - `cargo test --test idiom_gaps --quiet`
4. If these pass, treat the File facade row family as closed for this subset.

Acceptance:

- `top-compile-errors.tsv` contains no `File.get_name`, `File.get_parent`,
  `File.get_absolute_file`, `File.is_directory`, or `File.mkdir` rows.
- Generated focused support contains the real methods in
  `generated-crate/src/lib.rs`.

### 2. Fix Optional Vector Indexing

Status: done for `collection.get(...)[index]`.

Evidence before fix:

- r3 has 5 `Option<&Vec<i32>>` indexing errors.
- r3 has 2 `Option<&Vec<String>>` indexing errors.

Plan:

1. Find the generated lines in the r3 crate and map them back to the Java source.
2. Add a minimized fixture where a nullable/optional Java array or list is
   guarded and then indexed.
3. Route indexed optional sequence receivers through the same non-null
   refinement/slot conversion used by other optional carriers.
4. Keep unguarded optional indexing explicit and conservative; do not silently
   default missing values.

Acceptance:

- Guarded optional sequence indexing emits an unwrap/view before indexing.
- Unguarded optional sequence indexing still emits a typed diagnostic or an
  explicit optional-handling shape.
- The r3 optional-indexing rows disappear in a focused rerun.

Verified:

- `collection_get_result_unwraps_before_array_indexing`
- `erased_object_sequence_push_moves_when_no_later_read`
- `cargo test --test idiom_gaps --quiet` with 354 passing tests
- `.tmp/corpus-fastqc-abstract-qcmodule-after-collection-get-index`

### 3. Add Small JDK/GUI Facade Surfaces

These are low-risk if modeled as general Java platform support.

Candidate fixtures:

- `BufferedImage.TYPE_INT_RGB`.
- `Graphics.dispose()` when the receiver is known as a Java graphics facade.
- Java `getClass()` on source/JDK objects.

Done:

- `String.getBytes()` and `String.indexOf(...)`, verified with
  `java_string_get_bytes_and_index_of_use_native_helpers` and
  `.tmp/corpus-fastqc-abstract-qcmodule-after-string-facades-r2`.
- Receiverless `getClass().getResourceAsStream(...)`, verified with
  `receiverless_get_class_resource_stream_uses_current_receiver`.
- `BufferedImage.TYPE_INT_RGB`, `BufferedImage.createGraphics() -> Graphics`,
  `Graphics.dispose()`, and `JPanel.print(Graphics)` argument typing, verified
  with the buffered-image/graphics fixture group and focused rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-getclass-awt-facades-r7`.

Plan:

1. Inspect the Java call sites before adding support.
2. Add precise fixtures for each surface.
3. Prefer typed call/member facts where possible; use generated support shims
   only for Java platform types.
4. Mirror any focused-wrapper shared support additions in
   `scripts/translate-focused-package.sh`.

Acceptance:

- Each new facade has a fixture that compiles generated Rust.
- The focused subset loses the corresponding missing-method/constant rows.

Next small facade candidate:

- done: `InputStream.read(byte[])`/`close()` for streams returned by
  `JavaClass.getResourceAsStream(...)`, verified by
  `resource_input_stream_read_and_close_have_typed_facade_support` and focused
  rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-inputstream-resource-facade-r3`.

### 4. Design Non-Static Inner Class Outer Capture

This is the largest current family and should be handled deliberately.

Evidence:

- `SVGGenerator.SVGGraphics` is a non-static Java inner class.
- Generated `SVGGeneratorSVGGraphics` currently has only its own fields:
  `translate_x`, `translate_y`, `color`, and `font`.
- Its methods access outer fields as bare names: `sb`, `c_width`, `c_height`,
  and `g`.

Plan:

1. Add a minimized fixture for a non-static inner class reading and mutating
   outer fields.
2. Decide the Rust representation:
   - an explicit outer handle/reference field on the inner struct;
   - or closure-like lowering for locally constructed inner instances;
   - or a typed unsupported diagnostic for cases that require Java object
     identity/aliasing beyond Rust ownership.
3. Start with a read-only outer-field fixture, then add mutation only if the
   ownership model is clear.
4. Avoid a string rewrite from bare `sb` to an arbitrary owner; the fix should
   be based on source-summary owner/capture facts.

Acceptance:

- Inner methods can resolve proven outer fields through the chosen capture
  representation.
- Unsupported aliasing cases produce a precise diagnostic, not unresolved bare
  identifiers.
- The r3 `sb`/`c_width`/`c_height`/`g` rows are either eliminated or replaced
  by one explicit unsupported-inner-capture diagnostic.

### 5. Rerun Strategy

After each small fix:

1. Run the new focused fixture test.
2. Run `cargo test --test idiom_gaps --quiet`.
3. Run `cargo fmt --check` and `cargo check --quiet`.
4. Build the release translator:
   `cargo build --release --quiet --bin new-java2rust`.
5. Rerun the focused AbstractQCModule subset with `--jobs 10`.
6. Update:
   - `docs/current-corpus-issue-audit.md`
   - `docs/corpus-fix-next-plan.md`

Only after the focused subset improves should a larger FastQC rerun be started.

## Do Not Do

- Do not model true GUI rendering behavior in the translator. Keep GUI support
  to typed facade surfaces or explicit handoff states.
- Do not add FastQC-specific method tables to `src/stdlib.rs`.
- Do not let inner-class work block smaller shared idiom fixes.
- Do not make algorithm changes that interfere with the future global semantic
  analysis model.
