# Current Goal Proceed Plan

Date: 2026-07-23

This is the tracked proceed-from-here plan for the active goal: keep converting
current corpus failures into generalized translator fixes and fixtures until the
agreed corpus/idiom plan is fulfilled. It supersedes the older VarScan-centered
content in this file and should be updated after each focused rerun.

Primary evidence files:

- `docs/current-corpus-issue-audit.md`
- `docs/corpus-fix-next-plan.md`
- `docs/proceed-from-current-fastqc-abstract-plan-2026-07-23.md`

Audit closeout invariant: when a Claude audit issue is genuinely done, move its
issue file from `claude_audits/` to
`/home/mahogny/github/claude/new_java2rust/claude_audits/done` in the same
checkpoint as the implementation, focused verification, and plan update. Partial
or inspiration-only audits stay active until fully closed.

Top todo:

- Decision: Rust stdlib/core/alloc behavior should be inferred from Rust
  signature/ABI facts whenever that is enough to produce the same emitted code
  as a tailored rule. The translator should not grow a broad hand-written
  `std` model; it should grow small, corpus-driven signature fact clusters and
  use those facts as the preferred receiver/argument/return slot authority.
- Because Rust stdlib/core/alloc are huge, missing `&`, `&mut`, slice,
  `Option`, `Result`, iterator, and owned/borrowed-return rows should default to
  a signature-fact investigation first. Add a tailored rule only when the Rust
  signature cannot encode the Java semantic difference being preserved.
- Treat Rust stdlib/core/alloc as signature-fact territory, not as a
  handwritten modeling project. The stdlib surface is too large for tailored
  rules; add small ABI/signature fact clusters only when generated corpus code
  hits them, and prefer those facts whenever they produce the same receiver,
  argument, return, optional/result, borrow, or slice behavior as a specialized
  rule would have produced.
- Do not try to enumerate Rust stdlib behavior in translator logic. The rule is
  equivalence-driven: if a Rust signature fact can derive the same lowered call
  shape as a tailored branch, the fact is the preferred implementation and the
  branch should either move behind the fact path or be deleted after migration.
- Add the `&`/borrow inference work as a cleanup trigger, not only a feature:
  whenever the translator learns a more general way to infer Rust receiver or
  argument slots from source/facade signatures, audit older specialized
  inference rules that may now be too narrow or contradictory. In particular,
  look for Java-method-name rules, package-specific borrow insertion, rendered
  Rust type-string checks, and stream-only slot guesses that were compensating
  for missing signature facts.
- Put the general Rust facade ABI/slot inference fix ahead of more
  call-site-specific `&`, clone, optional unwrap, and facade argument patches.
  Each time that general path subsumes an older inference rule, audit the old
  rule for over-specialization before leaving it in place; rules that were only
  compensating for missing ABI facts can now create semantic drift.
- Keep an explicit cleanup audit with that work: once Rust source/signature
  inference can derive a borrow, clone, slice, optional, or facade argument slot,
  review existing specialized rules that infer the same thing from method names,
  package names, rendered Rust type strings, or stream-local guesses, then mark
  each rule as kept Java semantics, bridged migration shim, or deletion target.
- Add a specific semantic-drift audit for previous inference rules that are too
  specialized for the new general path. When a source/facade ABI fact can infer
  a slot, any older package-, method-, or string-shape-specific rule for the
  same behavior must be checked for precedence bugs and either moved behind the
  general fact path, rewritten as an explicit compatibility fact, or removed.
- Definition of done for each new Rust stdlib/core/alloc idiom: a test proves
  the selected signature/ABI fact gives the same receiver style, argument
  coercions, return slot, and `Option`/`Result` shape that a custom lowering rule
  would have produced. If that proof exists, do not add the tailored rule.
- Before adding any new Rust stdlib/core/alloc special case, write down the
  Rust signature that would justify the same lowering. If that signature can be
  represented as a `FacadeAbiFact` or slot fact, implement the fact-backed path
  instead and leave the special case only as a documented Java-semantic rule or
  temporary migration bridge.
- Precedence constraint: signature facts are a default authority, not permission
  to erase more precise semantic planners. If an existing Java-semantic rule
  intentionally lowers to a narrower Rust shape than the nominal return type
  would imply, for example treating `String.trim()` as a borrowed string view
  where possible, that rule must run before the broad stdlib return-signature
  projection.
- Landed baseline: JDK collection methods now have small stdlib signature-fact
  clusters for `List`/`Set`/`Map` receiver argument slots and zero-argument
  receiver ABI such as `size`, `isEmpty`, `toArray`, `keySet`, and `entrySet`.
  Use those facts for mechanical owned/borrowed/slice/index/no-arg decisions,
  but keep native collection target and return semantics higher precedence.
  Collection ABI argument planning must derive Java semantic argument types
  from receiver generics so `Map.put` optional recovery and trait-object
  `List.add` storage still work.
- Landed baseline: Commons Math `Localizable`/`LocalizedFormats` now comes from
  built-in external-pack representation facts. `Localizable` is an object-safe
  dyn-trait facade, `LocalizedFormats` implements it, and semantic analysis
  imports those representation facts generically. This fixes the ordinary
  external enum-constant-to-interface slot mismatch; the focused FastQC
  math-util rerun at
  `.tmp/corpus-fastqc-math-util-after-localizable-pack-trait-r1` drops
  generated-Rust errors from 7 to 6.
- Landed baseline: dyn-trait facade assembly and anonymous nested-interface
  handoff now compile through the focused package wrapper. Source trait
  dependencies are promoted narrowly, `impl dyn Trait` support items are kept
  by both the merger and focused wrapper, nested interface aliases render as
  `type Alias = dyn OwnerAlias`, and anonymous `new Interface() { ... }`
  expressions passed to boxed dyn slots lower to `<dyn Trait>::anonymous()`
  without invalid `&dyn Trait::anonymous()` or `.clone_box()` forms. FastQC
  math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-anonymous-box-no-clone-r1` has 10/10
  translated, 0 invalid-pattern files, and generated-Rust errors down to 3.
- Landed baseline: FastQC math-util now cargo-checks as a focused corpus after
  the remaining `ResizableDoubleArray` rows were moved onto shared
  signature/slot machinery. Bare Java field array element writes now mark
  source methods as mutating `self`; `System.arraycopy` is represented as a JDK
  ABI fact with shared source slice, mutable destination slice, and index slots;
  optional array arguments lower through `as_deref()`/`as_deref_mut()` when the
  target slot is optional and through explicit Java-null unwraps when the ABI
  requires a concrete slice; and same-call alias snapshots now handle slotted
  calls containing index arguments plus mutable field projections such as
  `dest.internal_array.as_mut()...`. Focused evidence:
  `.tmp/corpus-fastqc-math-util-after-same-call-index-r1` has 10/10 translated,
  0 invalid-pattern files, `cargo_check=ok`, and 0 generated-Rust errors.
- Landed baseline: Java `Optional<T>.isPresent/get/orElse` now lowers through
  Rust `Option` semantics and has keyed ABI facts for no-arg/owned-fallback
  slots. Optional method recognition must use the declared Java receiver type,
  not just nullable internal `Option<T>` shape, so nullable non-Optional
  receivers such as `Future<T>` are not retargeted.
- Landed baseline: `java.util.Objects.nonNull/isNull` now have keyed stdlib
  ABI facts modeled as Rust `Option::is_some/is_none` optional-carrier
  predicates. Ordinary lowering remains shape-aware: nullable Rust carriers
  emit `.is_some()`/`.is_none()`, while non-optional translated values emit
  constant `true`/`false` because their Rust representation cannot be null.
  Stream and callback method-reference lowering no longer emits
  `Objects::non_null(...)` helper calls for concrete element types.
- Landed baseline: `java.io.File.exists()` now has a keyed JDK platform ABI
  fact for Rust `Path::exists(&self) -> bool`. The fact proves the zero-arg,
  borrowed-receiver, owned-bool Rust shape, but target planning still gives
  native Java File lowering precedence so emitted Rust keeps the Java
  `File`-to-path projection (`Path::new(file.get_path().as_str()).exists()`)
  instead of retargeting to an unresolved facade `file.exists()` call.
- Landed baseline: `java.nio.file.Files.exists(Path)` now has a keyed JDK
  platform ABI fact for the Rust `Path::exists` borrowed-path argument and
  owned-bool return shape. That fact is used only for mechanical argument slot
  planning; receiver identifiers proven to be static type aliases are not
  treated as facade instance receivers, so emitted Rust keeps the Java static
  support target (`JavaNioFileFiles::exists(path)`) instead of retargeting the
  whole call to `Path::exists(path)`.
- Landed baseline: `java.nio.file.Files.newInputStream(Path)`,
  `newOutputStream(Path)`, `newBufferedReader(Path)`, and
  `readAllLines(Path)` now have keyed JDK support ABI facts. The facts encode
  borrowed path arguments, owned return slots, and `Result` wrapping while
  preserving Java static facade target selection. Focused evidence covers the
  selected facts and the `Files.readAllLines(path)` call path.
- Landed baseline: JDK byte-range IO now uses ABI slot facts for borrowed
  slices. `InputStream.read(byte[], int, int)`/`read_3` takes a mutable slice
  argument, `OutputStream.write(byte[], int, int)`/`write_3` takes a shared
  slice, and same-call later reads such as `buffer.length` are temporarily
  evaluated before the mutable borrow. This keeps the fix in shared
  argument-slot/alias lowering rather than in an IO-specific call rewrite.
  Source parameter and local mutability inference also consumes mutable facade
  argument slots, so `&mut` facade facts produce `&mut [T]` parameters and
  `let mut` locals from facts.
- Landed baseline: source method-effect propagation now matches Java and
  emitted Rust method spellings symmetrically, so thrown effects attach to the
  same overloaded Rust callable names used by lowering and exception planning.
  Focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-symmetric-method-effect-key-r2` drops
  generated-Rust errors from 14 to 8 and removes the remaining fallible-call
  `?` rows.
- Landed baseline: exception constructor message arguments now use the normal
  display slot planner, with the second-argument cause path preserved. Erased
  Java `Object`/`Object[]` parameters trigger erased-object runtime support and
  display through `java_erased_object_to_string(...)`. Focused FastQC math-util
  evidence:
  `.tmp/corpus-fastqc-math-util-after-exception-display-slot-r1` drops
  generated-Rust errors from 8 to 7 by removing the erased-varargs `Display`
  error.

## Current Baseline

- Local gates were last recorded green in `docs/current-corpus-issue-audit.md`.
- Clean focused corpuses already recorded:
  - `picard-programgroups`
  - `htsjdk-fastq`
  - `varscan`
  - FastQC statistics subset
  - FastQC exception subset
- Full FastQC no longer panics and reaches generated Rust. The latest full
  recorded run still has remaining invalid-pattern/generated-Rust families.
- Focused FastQC Utilities evidence now includes
  `.tmp/corpus-fastqc-utilities-after-super-read-boundary-r1`:
  10/10 translated, 1 invalid-pattern file, 3 generated Rust errors. The
  previous
  `fileBases.get(baseName).add(file)`/`Option<T>.push` family is gone via the
  shared native map-held collection mutation lowering, FastQC `File[][]`
  grouping now stays as ragged `Vec<Vec<File>>` instead of `Array2<File>`, JDK
  stream wrapper protected state (`in`, `inf`, `buf`, `len`) now comes from
  typed stdlib/platform facts, and `InputStream.read`/`read_3` result wrapping
  comes from facade ABI facts. Unsupported constructor bodies also add a
  diverging tail after the precise `compile_error!`, removing non-unit
  constructor return-type cascades. Explicit
  `((PushbackInputStream)this.in).unread(...)` calls now select the
  `PushbackInputStream` facade ABI fact and lower through an explicit facade
  cast boundary instead of incorrectly calling `InputStream.unread`.
  Calls to private constructors already classified as unsupported object-graph
  mutation now lower to a panic boundary at the call site instead of requiring
  `self.clone()`, and `super.read(byte[], int, int)` in JDK stream wrappers
  lowers through the inherited `InputStream.read_3` facade ABI instead of
  `Default::default()?`. Remaining rows are only
  `MultiMemberGZIPInputStream` object-graph/native input-stream modeling:
  the two deliberate unsupported self-linking constructor diagnostics.
- The most active small corpus is the FastQC `AbstractQCModule` subset:
  - latest recorded run:
    `.tmp/corpus-fastqc-abstract-qcmodule-after-zip-outputstream-adapter-r2`
  - 8/8 translated
  - 0 invalid-pattern files
  - offline `cargo check` reaches generated Rust and reports 31 errors
  - fixed since the older getClass/AWT baseline: resource `InputStream`
    `read`/`close`, non-static inner outer-field capture, display-capable
    `StringBuilder`/`StringBuffer.append(...)`, nullable constructor facade
    slots, constructor field assignments that depend on constructor-local
    declarations, `BufferedImage.getGraphics() -> Graphics`, and same-typed
    external return facts preserving concrete field storage, AWT/Swing GUI
    getters with precise primitive/string/object return shapes, and erased
    `TableModel.getValueAt(...)` display conversion through
    `java_erased_object_to_string(...)` instead of invalid downcasts. Source
    receiver call-slot planning now prefers emitted source `SignaturePlan`
    return ABI for primary source methods while preserving source-summary
    nullability for summary-only owners, which removed the previous borrowed
    getter local mismatches. Bare local receiver lookup now respects Java
    local-over-field shadowing, and JDK `ZipFile` receiver methods have typed
    fallible platform facts/support, closing the `ZipFile.as_ref` family.
    Constructor field assignments whose RHS reads another receiverless instance
    field now defer until `__java_self` exists, and UpperCamel static-owner
    field access such as `Color.BLACK`/`Font.PLAIN` no longer collapses to
    same-named instance fields. Deferred constructor RHS expressions now use
    self-field slot recovery before substituting `__java_self`, avoiding
    whole-struct clones for nested source-constructor arguments.
    Actual-argument-aware JDK constructor slot planning now distinguishes
    `FileOutputStream(File)` from `FileOutputStream(String)`, so nullable
    `File` fields are recovered before the constructor call. JDK
    `ZipOutputStream` values now convert into `OutputStream` slots through
    platform conversion adapter facts, with reused values cloning before
    `.into()`.

## Phase 1: Verify The Current WIP Slice

Status: done for the getClass/AWT slice.

Start here before new implementation. The current checkout has uncommitted
translator/support/test edits, so first determine whether the latest WIP is
already correct.

Run:

1. `cargo test --test idiom_gaps buffered_image_graphics_facades_are_typed --quiet`
2. `cargo test --test idiom_gaps receiverless_get_class_resource_stream_uses_current_receiver --quiet`
3. `cargo test --test idiom_gaps source_nullable_getter_local_initializer_stays_optional --quiet`

If the AWT fixture fails, inspect whether weak external support is still
emitting duplicate `BufferedImage` or `Graphics` stubs. Keep the fix at the
platform-facade/fact layer; do not add FastQC-specific lowering.

Acceptance:

- The generated fixture for `createGraphics()` uses `Graphics`, not
  `Box<dyn std::any::Any>` or `()`.
- `BufferedImage::TYPE_INT_RGB` is emitted as a typed JDK constant.
- Receiverless `getClass()` uses the current source type and does not emit a
  free `get_class()` call.
- The optional getter regression test still proves that local initializers are
  not generally wrapped as `Some(Option<T>)`.

## Phase 2: Re-establish Local Gates

Status: done for the getClass/AWT slice.

After Phase 1 passes, run the normal translator gates:

1. `cargo fmt --check`
2. `cargo check --quiet`
3. `cargo test --test idiom_gaps --quiet`
4. `cargo test --test parser_gaps --quiet`
5. `cargo test --test batch_translation --quiet`
6. `cargo test --test type_coercion --quiet`
7. `cargo test --test idiom_sweep_script --quiet`
8. `bash -n scripts/translate-focused-package.sh`
9. `cargo build --release --quiet --bin new-java2rust`

If a broad gate fails, add or name the narrow failing fixture before changing
behavior. Preserve the known regression guard around optional source getter
local initializers.

## Phase 3: Prove The WIP On AbstractQCModule

Status: done for the getClass/AWT slice.

Rerun the focused subset with the release translator:

```sh
scripts/translate-focused-package.sh \
  --primary .tmp/fastqc-abstract-qcmodule-subset \
  --out .tmp/corpus-fastqc-abstract-qcmodule-after-getclass-awt-facades \
  --skip-resolver-bench \
  --jobs 10 \
  --translate-timeout-sec 60 \
  --offline-cargo
```

Inspect:

- `summary.tsv`
- `top-compile-errors.tsv`
- `invalid-patterns.tsv`
- `compile.log`

Observed reductions in
`.tmp/corpus-fastqc-abstract-qcmodule-after-getclass-awt-facades-r7`:

- no missing free `get_class` rows;
- no missing `BufferedImage::TYPE_INT_RGB` rows;
- no `dispose` on `Box<dyn Any>` rows;
- no invalid `Graphics` downcast rows;
- generated Rust errors drop from 190 to 186.

Update:

- `docs/current-corpus-issue-audit.md`
- `docs/corpus-fix-next-plan.md`
- `docs/proceed-from-current-fastqc-abstract-plan-2026-07-23.md`

## Phase 4: Fix Non-Static Inner-Class Outer Capture

Status: mostly done for read-side outer capture; remaining work is follow-on
generated Rust cleanup exposed by the focused subset.

This is the largest remaining `AbstractQCModule` family and should be handled
as object-model work, not a rendered-name patch.

Evidence from the latest focused run:

- about 90 missing `sb` rows;
- about 10 missing `c_width` rows;
- about 10 missing `c_height` rows;
- about 4 missing `g` rows;
- source shape: `SVGGenerator.SVGGraphics` is a non-static Java inner class
  whose methods read or mutate fields on the enclosing `SVGGenerator`.

Implementation plan:

1. Add a minimized fixture for a non-static inner class reading an outer field.
2. Add a second fixture only after that works for mutation of an outer field.
3. Represent proven captures explicitly through source-summary/object-model
   facts, for example by adding an outer handle/reference field to the inner
   struct or by producing a precise unsupported diagnostic when Rust ownership
   cannot represent the aliasing.
4. Lower inner-method field access through the chosen capture representation.
5. Avoid bare-name rewrites such as `sb -> self.outer.sb` unless they are driven
   by resolved capture facts.

Acceptance:

- Read-only capture compiles in the minimized fixture.
- Mutation either compiles with a deliberate handle model or emits one precise
  unsupported diagnostic.
- The focused `AbstractQCModule` rerun removes the unresolved `sb`,
  `c_width`, `c_height`, and `g` rows or replaces them with a much smaller,
  explicit unsupported-inner-capture family.

Latest evidence:

- `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-local-graphics-r4`
  translates 8/8 files with 0 invalid-pattern files and 55 generated Rust
  errors.
- `SVGGenerator.g` is now `Option<Graphics>`, and `SVGGenerator::new` keeps
  the constructor-local `BufferedImage b` before assigning
  `__java_self.g = Some(b.get_graphics())`.
- `.tmp/corpus-fastqc-abstract-qcmodule-after-gui-erased-display-r2`
  translates 8/8 files with 0 invalid-pattern files and 39 generated Rust
  errors.
- The GUI return-shape rows for `Component.getWidth/getHeight`,
  `Color.getRed/getGreen/getBlue`, `Font.getSize/isBold`, and
  `TableModel.getRowCount/getColumnCount/getColumnName/getValueAt` are typed.
  `String.valueOf(table.getValueAt(...))`, XML writer display slots, and
  `StringBuffer.append(table.getValueAt(...))` now lower through
  `java_erased_object_to_string(&table.get_value_at(...))`; the invalid
  `downcast::<String>()` row is gone.
- The next visible families are mismatched source/facade return slots,
  remaining `()` display expressions, singleton/static field lowering,
  class-loader resource access, and remaining source argument ownership rows.
  JDK `ZipOutputStream`, Apache Commons Codec `Base64OutputStream`, and FastQC
  iHarder `Base64.OutputStream` now convert to `OutputStream` through facts in
  focused checks; Java public source fields now remain public in the generated
  Rust struct, and `SVGImageSaver` static Pattern constants now emit as
  associated values.

## Phase 5: Sweep All Corpuses Again

Once the focused FastQC slice improves and gates are green, rerun the currently
tracked corpuses with `--jobs 10` and release translator builds.

Include at least:

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
- any separate Picard, GenomicsDB, and DISQ corpuses created for idiom-gap
  exploration

For each corpus, record:

- translated/failed counts;
- invalid-pattern counts and top families;
- generated Rust error count;
- top compile-error families;
- whether failures are translator crashes, invalid placeholders, generated Rust,
  or dependency/setup issues.

## Phase 6: Batch The Next Idioms

After the sweep, choose the next batch by cross-corpus frequency and by whether
the fix strengthens shared Java semantics.

Expected high-value families:

- non-static inner-class capture and Java object identity boundaries;
- remaining `super(...)` constructor placeholders;
- concrete downcast and `instanceof` narrowing;
- interface/trait bridge generation;
- array creation and optional array length/indexing;
- switch scaffold/state-machine lowering;
- Java collection/map spelling variants;
- remaining JDK facade constants and simple methods;
- source/facade owner assembly gaps;
- generated support type deduplication and visibility.

Workflow for each family:

1. Add one minimized fixture under `tests/idiom_gaps.rs` or the most specific
   existing test file.
2. Implement in shared planner/fact/lowering code.
3. Mirror focused-wrapper support blocks when raw platform support changes.
4. Run the narrow fixture, the relevant broader gate, and the smallest affected
   focused corpus.
5. Update the audit and this plan with counts.

## Boundary Rules

- Do not add non-JDK package/type/method tables to `src/stdlib.rs`.
- Hadoop configuration, Spark, and Google Cloud Storage remain noop facade
  boundaries for now.
- Spark/DISQ distributed execution is not mechanically translated by this Java
  translator; translate Java-side FFI/handoff setup only.
- GenomicsDB native behavior is not a `javalibs` semantic facade. It should be
  a handoff/FFI boundary whose signatures can come from the future C/C++
  transpiler or explicit ABI facts.
- Picard is a corpus to translate, not a `javalibs` facade.
- Barclay remains `maybe`, only where there is a high-quality mapping.
- Serialization may need explicit handoff/typed carrier facts; do not assume a
  mechanical Serde mapping without corpus proof.
- Math/numeric semantics are high priority when they show up in corpus errors.
- Performance work should favor release/profile builds, reduced cloning, and
  indices. Do not make algorithm changes that block future global semantic
  analysis.

## Stop And Replan Conditions

- A local gate regresses and the failing behavior is not covered by a fixture.
- A proposed fix depends on package-specific knowledge in `src/stdlib.rs`.
- A corpus fix starts modeling native/distributed runtime semantics instead of
  facade or FFI handoff shapes.
- A solver/performance optimization changes slot-shape or nullability
  propagation semantics.
- Inner-class capture requires Java object identity that cannot be represented
  mechanically without a handle model.
