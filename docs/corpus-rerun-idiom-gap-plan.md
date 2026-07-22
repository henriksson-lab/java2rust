# Corpus Rerun Idiom Gap Plan

## Context

After the latest corpus reruns, the remaining failures are no longer dominated
by parser placeholders. The next work should stay in this checkout rather than
being split into separate worktrees, because several fixes need coordinated
changes across the translator and `javalibs` manifests/facades.

Original output directories from the first rerun that created this backlog:

| Corpus | Output | Result |
|---|---|---|
| Picard program groups | `.tmp/corpus-rerun-picard-programgroups` | 11/11 translated, 0 invalid idiom patterns, `cargo check` failed with 12 errors |
| GATK GenomicsDB full context | `.tmp/corpus-rerun-gatk-genomicsdb-skipbench` | timed out before crate assembly |
| GATK GenomicsDB primary-only | `.tmp/corpus-rerun-gatk-genomicsdb-primary-only` | 6/6 translated, 0 invalid idiom patterns, `cargo check` failed with 74 errors |
| GATK Spark/DISQ full context | `.tmp/corpus-rerun-gatk-disq-spark-datasources-skipbench` | timed out before crate assembly |
| GATK Spark/DISQ primary-only | `.tmp/corpus-rerun-gatk-spark-datasources-primary-only` | 10/10 translated, 4 invalid-pattern files, `cargo check` failed with 119 errors |

## Current Status

Verified after the latest in-checkout idiom fixes:

Current local fixture gate:

| Gate | Result |
|---|---|
| `cargo fmt --check` | passes |
| `cargo check --quiet` | passes |
| `cargo test --test idiom_gaps --quiet` | passes: 267/267 |

The main `idiom_gaps` fixture suite is currently clean. Corpus-derived gaps are
still part of the same backlog when focused corpus gates fail; do not treat the
green local fixture run as proof that the real-code corpuses are clear. Assume
any focused corpus failure, invalid-pattern result, or timeout is an active
idiom gap until a rerun proves otherwise, including corpuses that are not yet
represented in `tests/idiom_gaps.rs`. Add compact fixtures for the observed
failure class before changing the corpus-specific lowering path.

Current full-suite failed fixture names: none.

Latest focused corpus gates from this checkout:

| Corpus | Output | Result |
|---|---|---|
| Picard program groups current | `.tmp/corpus-rerun-picard-programgroups-current-inclusion-check` | 11/11 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| GATK GenomicsDB primary-only current | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-broadcast-generic` | 6/6 translated, 0 invalid-pattern files, generated crate `cargo check` passes |
| GATK Spark/DISQ primary-only current | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-broadcast-generic` | 10/10 translated, 0 invalid-pattern files, generated crate `cargo check` passes |
| HTSJDK FASTQ after source struct derives | `.tmp/corpus-rerun-htsjdk-fastq-after-source-struct-derives` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 28 reported errors; source value structs now derive `Clone`/`PartialEq` when all fields support it, removing the prior `FastqRecord` clone/partial-equality bucket |
| HTSJDK FASTQ after concrete self-cast handling | `.tmp/corpus-rerun-htsjdk-fastq-after-source-self-cast` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 27 reported errors; concrete `(CurrentType)this` casts now lower through the current source slot, but erased object identity/class/downcast and trait-object slots remained at that point |
| HTSJDK FASTQ after Java Object identity | `.tmp/corpus-rerun-htsjdk-fastq-after-object-identity` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 27 reported errors; `this == obj` against erased `Object` now lowers through `Any::downcast_ref` plus pointer identity, removing the incompatible `&FastqRecord == Option<Box<dyn Any>>` bucket |
| HTSJDK FASTQ after exact `getClass` predicate | `.tmp/corpus-rerun-htsjdk-fastq-after-getclass-predicate` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 26 reported errors; `getClass() != obj.getClass()` now lowers as an erased-object `Any::is::<Current>()` predicate |
| HTSJDK FASTQ after exact-class cast refinement | `.tmp/corpus-rerun-htsjdk-fastq-after-exact-class-cast-refinement` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 26 reported errors; guarded `(FastqRecord)obj` now lowers to a borrowed `downcast_ref` view, but trait-object upcasts, nullable string helper adaptation, support constants/getters, `Appendable`, constructor inheritance, uninitialized locals, non-`Result` propagation, and `Option<File>` display remain |
| HTSJDK FASTQ after optional field receiver adapter | `.tmp/corpus-rerun-htsjdk-fastq-after-optional-field-receiver-adapter` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 25 reported errors; current-instance optional field receivers now use the shared helper adapter, so the remaining `qualityHeader.hashCode()` site lowers as `java_string_hash_code(self.quality_header.as_ref().unwrap().as_str())` |
| HTSJDK FASTQ after SAMRecord FASTQ getters | `.tmp/corpus-rerun-htsjdk-fastq-after-samrecord-fastq-getters` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 23 reported errors; javalibs-modeled `SAMRecord.getReadString` and `getBaseQualityString` now suppress false missing-pack diagnostics and emit typed `String` facade support |
| HTSJDK FASTQ after static byte-array constants | `.tmp/corpus-rerun-htsjdk-fastq-after-static-byte-array-constants-narrowed` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 21 reported errors; builtin external static-final primitive-array constants now emit typed facade surfaces, removing `SAMRecord::NULL_SEQUENCE` and `SAMRecord::NULL_QUALS` while avoiding unresolved named-object constants |
| HTSJDK FASTQ after external inherited static finals | `.tmp/corpus-rerun-htsjdk-fastq-after-external-inherited-static-final` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 20 reported errors; source-summary and external inherited static-final constants now emit subclass-owner associated constants, removing `AsyncFastqWriter::DEFAULT_QUEUE_SIZE` |
| HTSJDK FASTQ after boxed-dyn clone parentheses | `.tmp/corpus-rerun-htsjdk-fastq-after-lower-clone-parens` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 19 reported errors; audited clone recovery now parenthesizes cast receivers, removing the `cast cannot be followed by a method call` bucket |
| HTSJDK FASTQ after null-guarded ternary display | `.tmp/corpus-rerun-htsjdk-fastq-after-null-guarded-ternary-display-field-shape` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 18 reported errors; `this.field == null ? "" : this.field` display branches now use the current field shape and render the non-null arm as `self.field.as_ref().unwrap().to_string()` |
| HTSJDK FASTQ after current instance/static alias precedence | `.tmp/corpus-rerun-htsjdk-fastq-after-instance-field-static-alias-precedence` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 17 reported errors; current instance fields now win over colliding sibling static aliases such as `FastqConstants.QUALITY_HEADER`, so `qualityHeader` nullable getter/hash-code sites keep the `Option<String>` field shape |
| HTSJDK FASTQ after do-while/BufferedReader | `.tmp/corpus-rerun-htsjdk-fastq-after-do-while-bufferedreader` | 8/8 translated, 3 invalid-pattern files, generated crate `cargo check` failed with 16 reported errors; Java `do { ... } while (...)` bodies now lower to Rust loops and `BufferedReader.readLine()` shares the modeled Reader external effect surface |
| HTSJDK FASTQ after interface return slots | `.tmp/corpus-rerun-htsjdk-fastq-after-interface-return-slots` | 8/8 translated, 2 invalid-pattern files, generated crate `cargo check` failed with 16 reported errors; interface return slots now render as boxed dyn traits instead of raw trait types, removing the `FastqWriter`/`Iterator` raw trait and `FastqReader` downcast buckets while exposing source trait impl/clone gaps |
| HTSJDK FASTQ after cross-file trait impl | `.tmp/corpus-rerun-htsjdk-fastq-after-crossfile-trait-impl` | 8/8 translated, 2 invalid-pattern files, generated crate `cargo check` failed with 18 reported errors; source trait methods now use fallible implementation signatures across emitted context and `BasicFastqWriter` implements `FastqWriter`, but async superclass/template-method inheritance still emits missing-interface-method placeholders |
| HTSJDK FASTQ after current self argument | `.tmp/corpus-rerun-htsjdk-fastq-after-current-self-arg` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 17 reported errors; bare `this` passed to an owned concrete static/helper argument now clones the current receiver instead of emitting an erased-object runtime downcast |
| HTSJDK FASTQ after null-guarded append argument | `.tmp/corpus-rerun-htsjdk-fastq-after-null-guarded-append-arg` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 14 reported errors; refined non-null optional locals flowing into concrete or generic argument slots now expose their element shape, removing the four `String.unwrap().unwrap()` failures in `FastqEncoder.write` |
| HTSJDK FASTQ after Appendable implementor conversion | `.tmp/corpus-rerun-htsjdk-fastq-after-appendable-into` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 13 reported errors; Java platform conversion adapter facts now make `StringBuilder`/`PrintStream` flow into `Appendable` slots via `.into()`, removing the plain concrete-to-interface mismatch for `StringBuilder` |
| HTSJDK FASTQ after borrowed field `.into()` recovery | `.tmp/corpus-rerun-htsjdk-fastq-after-field-into-clone` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 12 reported errors; shared `self` fields consumed by external/interface `.into()` conversion now clone before conversion, removing the `self.writer.into()` move-out error |
| HTSJDK FASTQ after unchecked exception precedence | `.tmp/corpus-rerun-htsjdk-fastq-after-unchecked-exception-precedence` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 9 reported errors; non-unknown exception facts now beat parsed-source `Unknown`, so known unchecked `SAMException` no longer leaks `Result`/`?` into `FastqEncoder.write` callers |
| HTSJDK FASTQ after `PrintStream.checkError` support | `.tmp/corpus-rerun-htsjdk-fastq-after-printstream-check-error` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 8 reported errors; Java platform API facts now emit `PrintStream.check_error() -> bool` support |
| HTSJDK FASTQ after `PrintStream(OutputStream)` constructor typing | `.tmp/corpus-rerun-htsjdk-fastq-after-printstream-constructor-arg` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 7 reported errors; typed Java platform constructor argument facts now emit `PrintStream::new(_arg0: OutputStream)` and remove the generic inference failure |
| HTSJDK FASTQ after nonescaping callback slots | `.tmp/corpus-rerun-htsjdk-fastq-after-biconsumer-callback-slot` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 7 reported errors; `BiConsumer<FastqRecord,SAMRecord>` source parameters now render as `impl FnMut(FastqRecord, SAMRecord)` and `.accept(...)` lowers to a direct callback call, but the callback initially moved `sam_record` before return |
| HTSJDK FASTQ after callback argument clone recovery | `.tmp/corpus-rerun-htsjdk-fastq-after-callback-arg-clone` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 6 reported errors; direct functional-interface calls now clone non-copy local arguments before invoking the Rust callback, removing the `sam_record` move-after-callback error |
| HTSJDK FASTQ after branch-owned move recovery | `.tmp/corpus-rerun-htsjdk-fastq-after-branch-owned-move` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 6 reported errors; path-sensitive ownership recovery now moves `writer` into `AsyncFastqWriter::new(...)` in the terminal async branch instead of cloning `Box<dyn FastqWriter>`, exposing a separate `.into().clone()` inference issue |
| HTSJDK FASTQ after `.into()` clone idempotence | `.tmp/corpus-rerun-htsjdk-fastq-after-into-clone-idempotence` | 8/8 translated, 1 invalid-pattern file, generated crate `cargo check` failed with 5 reported errors; audited clone-before-into recovery is now idempotent, removing the ambiguous `self.writer.clone().into().clone()` chain in `BasicFastqWriter.write` |
| HTSJDK FASTQ after async template superclass | `.tmp/corpus-rerun-htsjdk-fastq-after-async-template-super` | 8/8 translated, 0 invalid-pattern files, generated crate `cargo check` passes; modeled no-state external superclass constructors and template-method hooks now lower `AbstractAsyncWriter` subclasses without constructor or missing-interface placeholders |
| JAligner Tier 1 current | `.tmp/corpus-rerun-jaligner-tier1-current` | 29/29 translated, 20 invalid-pattern files, generated crate `cargo check` blocked on fetching `ndarray`; invalid-pattern output is active backlog |
| JAligner after typed map cast fixture | `.tmp/corpus-rerun-jaligner-after-typed-map-cast` | interrupted after repeated no-output polls; partial output only, so use local fixture evidence for the typed map cast improvement until a fresh timed rerun completes |
| JAligner after exception control-flow fixtures | `.tmp/corpus-rerun-jaligner-after-exception-control-flow` | timed out after 180s with only partial temp output and no populated `status.tsv`; direct translation of real `MatrixLoader.java` and `SequenceParser.java` shows the targeted try/catch and try/catch/finally placeholders are gone, but this is not a full corpus result |
| JAligner direct cast/super checks | `.tmp/direct-jaligner-check` | direct real-file checks now show `MatrixLoader` lowers the `JarURLConnection` factory cast without `downcast::<JarURLConnection>`; `CLUSTAL` and `FASTA` lower no-arg real-super plus inherited `setId(...)` when the `formats` superclass context is supplied |
| JAligner after fallthrough-catch/release rerun | `.tmp/corpus-rerun-jaligner-after-fallthrough-catch-release2` | 29/29 translated with the release translator, 0 invalid-pattern files; generated crate `cargo check` is still blocked only by fetching `ndarray` |
| VarScan Tier 1 timeout 180 | `.tmp/corpus-rerun-varscan-tier1-timeout180` | timed out after 180s before assembled crate summary; partial output only |

Clean focused gates such as Picard programgroups, GenomicsDB primary-only, and
Spark/DISQ primary-only are regression guards. If a later rerun fails, promote
the observed compile errors, invalid-pattern buckets, or timeout/scalability
failure back into the active backlog immediately and add fixture coverage before
the fix.

Current corpus-derived active gaps, assuming failed corpus gates are in scope:

| Corpus output | Active gap families |
|---|---|
| `.tmp/corpus-rerun-htsjdk-fastq-after-async-template-super` | no active FASTQ compile or idiom-sweep gap in this focused output; keep as a regression guard |
| `.tmp/corpus-rerun-jaligner-after-fallthrough-catch-release2` | no active invalid-pattern gap in this focused output; generated crate Rust typechecking remains blocked by `ndarray` dependency fetch, so compile-quality validation is still incomplete |
| `.tmp/corpus-rerun-picard-programgroups-current-inclusion-check` | no active compile or idiom-sweep gap in this focused output; keep as regression guard |
| `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-corpus-gap-inclusion` | older inclusion-check output still has active gaps if treated as failed: callback argument field typing, raw `List` symbol leakage, unresolved static owner/type tokens such as `smartMergeHeaders`/`newFixedThreadPool`, optional/raw downcast placeholders, `ProgressMeter` support identity, and several optional/borrowed shape mismatches |
| `.tmp/corpus-rerun-gatk-spark-datasources-primary-current-inclusion-check` | older inclusion-check output still has active gaps if treated as failed: borrowed varargs conversion into `WriteOption`, one switch-state-machine bucket, and one runtime downcast in `ReadsSparkSink` |

Latest GenomicsDB primary-only top categories:

- No current focused compile or invalid-pattern category. The latest focused
  GenomicsDB primary-only gate compiles cleanly.
- Treat this as a regression guard, not as permanently complete. Any future
  primary-only compile failure or invalid-pattern rerun immediately re-enters
  the active backlog with fixture coverage for the new failure class.
- Runtime downcast invalid patterns are clear in the focused GenomicsDB gate.
  `smartMergeHeaders`, `newFixedThreadPool`, `Files.readAllLines`, and the
  interval `List<SimpleInterval>` ternary no longer emit `invalid Java cast`
  placeholders.
- New fixture coverage includes static external calls into optional slots,
  static external calls into generic `Set` slots, raw `List` casts over
  optional ternaries, and plain optional ternary list assignments.
- Source-summary methods whose bodies can return `null` now emit optional
  support surfaces and call-return shapes. The previous double-wrapped
  `SampleNameMap.getVCF*AsPath` sites and the `Option<&str>` versus
  `Option<&String>` comparison site are gone from the focused GenomicsDB gate.
- Indexed array elements flowing into `Option<&T>` argument slots now borrow the
  element explicitly. The previous `sampleNameIsLegal(Some(split[0]))` mismatch
  now renders `Some(&split[0])`.
- The previous borrowed `HashMap<String, URI>` helper mismatch in
  `createSampleToReaderMap` is fixed by preserving cast target ownership and
  using the shared owned-recovery plan.
- The previous `&&Path`/`&Path`, `&VCFHeader`, borrowed
  `GenomicsDBImporter`, nullable static external `SAMSequenceDictionary`,
  unchecked direct throw, and concrete-to-`Box<dyn Any>` mismatches are no
  longer present in the focused GenomicsDB gate.
- `ProgressMeter.setRecordsBetweenTimeChecks` no longer appears as a compile
  error in the latest focused run. Parsed lowercase inherited field receivers
  such as `progress_meter` are rejected as external static-owner fallback
  receivers, while type-like lower-snake imported adapter/utility owners remain
  accepted.

Parallel worker audit, current GenomicsDB slice:

- `ProgressMeter.setRecordsBetweenTimeChecks` is a general facade-support
  identity bug, not a `ProgressMeter` special case. Shared support discovery and
  dedupe currently conflate same-owner/same-method/same-arity static and
  instance surfaces, so an imported static-owner candidate can suppress the
  required instance mutator support method.
- Throw-only methods that emit `Err(JavaException::new(...))` need exception
  planning to mark the Rust signature as `Result<..., JavaException>` when the
  method body contains a lowerable `throw`, even if Java omitted a checked
  `throws` clause for an unchecked/runtime exception.
- Erased `Object` return/local slots still need normal slot coercion from
  concrete values into `Box<dyn std::any::Any>`, including concrete external
  getter/factory returns and primitive values such as `boolean`.
- Optional source/result slots need to use the actual expression Rust shape
  before wrapping. Current examples include `Option<T> -> Option<T>` double
  wrapping, `Some(&T)` into `Option<T>`, `Some(T)` into `Option<&T>`, and
  `Option<&String>` versus `Option<&str>` comparisons after `trim()`.

Latest Spark/DISQ primary-only top categories:

- No current focused compile or invalid-pattern category. The latest focused
  Spark/DISQ primary-only gate compiles cleanly.
- Treat this as a regression guard, not as permanently complete. Any future
  primary-only compile failure or invalid-pattern rerun immediately re-enters
  the active backlog with fixture coverage for the new failure class.
- `ReadsSparkSource.checkCramReference` emits a borrowed nullable alias
  signature with explicit static lifetime, and call sites agree on
  `Option<&GATKPath>`.
- `VariantsSparkSource.getParallelVariantContexts` no longer leaks checked
  `IOException` through a method that catches it and throws unchecked
  `RuntimeException`; the prior invalid `?` in `getParallelVariants` is gone.
- Chained Spark `filter(...).map(...)` preserves the `JavaRDD` element shape and
  emits typed map closures such as `|vc: VariantContext| ...`.
- `ReferenceMultiSparkSource.getReferenceBases(SimpleInterval)` delegates
  through the boxed interface receiver as `get_reference_bases(&interval)`,
  matching the emitted `ReferenceSparkSource` trait slot.
- Borrowed DISQ write-option varargs now clone/recover owned values before
  applying explicit `into` adapter facts.
- Interface receiver delegation now uses the globally emitted trait signature,
  so owned slots such as `SAMSequenceDictionary` are not accidentally borrowed.
- `Broadcast<T>.value/getValue` now projects the receiver generic argument from
  the external pack fact, removing the final `ReadsSparkSink` invalid downcast.

Latest HTSJDK FASTQ Tier 1 top categories:

- Active output:
  `.tmp/corpus-rerun-htsjdk-fastq-after-printstream-constructor-arg`.
- `String.substring` and most nullable `String.hashCode` sites now lower
  through shared Java string runtime helpers; local fixtures cover both plain
  nullable locals and bare final instance fields.
- Nested Java enum constant access such as `LineType.SequenceLine` now lowers
  to the unit Rust enum variant, not to a constructor-like payload expression.
  This removed the stale `LineType(...)`/`line_type` failure bucket from the
  focused FASTQ rerun.
- Emitted Java enums are now suppressed from opaque external facade fallback
  support, so generated source no longer contains duplicate manual
  `PartialEq/Eq` impls for enums that already derive equality. Java enum
  representation now owns `Hash` and `Display`, so enum `toString()`/format
  call sites do not depend on opaque facade display shims.
- Source source-value structs now derive `Clone` and `PartialEq` when all
  emitted fields support those derives. The previous `FastqRecord` clone and
  direct source-value equality compile failures are gone from the focused
  FASTQ output.
- Concrete casts of the current receiver such as `(FastqRecord)this` now flow
  through the current source slot instead of producing a runtime downcast
  placeholder. Remaining `downcast` invalid patterns are erased-object casts,
  not concrete self-casts.
- Java identity checks of the current receiver against erased `Object` now use
  `Any::downcast_ref::<Current>()` plus `std::ptr::eq`, not Rust structural
  equality between incompatible types. The previous `self == obj` compile
  failure in `FastqRecord.equals(Object)` is gone from the focused FASTQ
  output.
- Java `Object.getClass` exact-type guards now use the same erased-object
  predicate path for Java spelling and normalized `get_class` spelling. The
  former `get_class()` unit-return comparison in `FastqRecord.equals(Object)` is
  gone from the focused FASTQ output.
- Early-return exact-class guards now refine the guarded erased `Object` local,
  so a later `(FastqRecord)obj` cast lowers to a borrowed `downcast_ref` view
  instead of consuming the `Option<Box<dyn Any>>`. This fixes the previous
  guarded `obj.unwrap().downcast::<FastqRecord>()` bucket.
- Local fixtures cover normalized `hash_code`/`hashCode` native string helper
  planning for current-instance nullable fields. The focused FASTQ output now
  lowers the former `quality_header.as_str()` mismatch as
  `java_string_hash_code(self.quality_header.as_ref().unwrap().as_str())`.
- `SAMRecord.getReadString` and `getBaseQualityString` are now represented as
  javalibs-modeled APIs with typed shared support surfaces. The prior two
  missing-method compile errors in `FastqEncoder` are gone.
- Primitive-array static-final external constants now emit typed facade
  surfaces for emitted opaque owners. The prior `SAMRecord::NULL_SEQUENCE` and
  `SAMRecord::NULL_QUALS` compile errors are gone; automatic builtin emission
  deliberately excludes named-object constants unless they are enum-like.
- Null-guarded ternary display branches now use the current instance field
  shape, including Java camelCase and emitted snake_case aliases. The previous
  `Option<File>` display failure in `FastqReader.toString()` now renders the
  non-null arm as `(self.fastq_file.as_ref().unwrap()).to_string()`.
- Current instance fields now take precedence over colliding static aliases
  from emitted sibling context. In FASTQ this fixes `qualityHeader` versus
  `FastqConstants.QUALITY_HEADER`, so `getBaseQualityHeader()` returns
  `self.quality_header.clone()` and the hash-code null guard lowers as
  `self.quality_header.is_none()` plus `java_string_hash_code(...)`.
- Java `do { ... } while (...)` statements now preserve their body and trailing
  condition instead of dropping the loop. In FASTQ this removes the old
  uninitialized `line` return in `readLineConditionallySkippingBlanks`.
- `BufferedReader.readLine()` now uses the same external-effect surface as
  `Reader.readLine()`, so generated support returns
  `Result<String, JavaException>` instead of unit.
- Interface return slots now keep their dyn-trait projection in source and
  emitted context. `FastqWriterFactory.newWriter` returns
  `Box<dyn FastqWriter>` instead of raw `FastqWriter`, and
  `FastqReader.iterator()` no longer emits a runtime downcast placeholder for
  returning `this`.
- Source trait emission now scans emitted support context for sibling
  implementors and promotes trait methods to `Result` when compatible
  implementations throw checked exceptions. This emits
  `impl FastqWriter for BasicFastqWriter`, but `AsyncFastqWriter` still needs
  inheritance/template-method lowering before it can satisfy `write` and
  `close`.
- Bare current receiver arguments such as `FastqEncoder.encode(this)` now lower
  through the current source slot as an audited clone instead of
  `self.downcast::<FastqRecord>()`.
- Null-guarded optional locals flowing into generic or concrete argument slots
  now use the refined element shape before slot conversion. In FASTQ this
  changes the `FastqEncoder.write` append chain from
  `read_name.unwrap().unwrap()` to `read_name.unwrap()`, removing the four
  `String` double-unwrap compile errors.
- Java platform conversion adapter facts now cover `StringBuilder`,
  `StringBuffer`, and `PrintStream` flowing into `Appendable` slots. This makes
  `FastqEncoder.encodeFastqRecord` render `StringBuilder::new(capacity).into()`
  for the call to `write(...)`.
- Shared current-instance fields that are consumed by an external/interface
  `.into()` conversion now clone before conversion. In FASTQ this changes
  `FastqEncoder::write(self.writer.into(), rec)?` to
  `FastqEncoder::write(self.writer.clone()/* TODO(translation): validate added clone */.into(), rec)?`,
- Nonescaping Java functional-interface parameters now have a callback ABI for
  core SAM interfaces such as `BiConsumer<T,U>`. In FASTQ this changes
  `custom: BiConsumer` plus `custom.accept(record, samRecord)` to
  `mut custom: impl FnMut(FastqRecord, SAMRecord)` plus `custom(record.clone(),
  sam_record.clone())`, removing the closure-to-nominal mismatch and the
  move-after-callback error.
- Path-sensitive ownership recovery now distinguishes same-statement reuse from
  mutually exclusive terminal branches. In FASTQ this changes the async branch
  in `FastqWriterFactory.newWriter` from
  `AsyncFastqWriter::new(writer.clone(), ...)` to
  `AsyncFastqWriter::new(writer, ...)`, while preserving clone recovery for
  same-statement cases such as `choose(value, value)`.
- Audited clone recovery is idempotent across explicit adapter conversions such
  as `PrintStream -> Appendable`. In FASTQ this keeps
  `self.writer.clone().into()` and avoids adding a second ambiguous
  `.clone()` after `.into()`.
- Modeled no-state external superclass constructors now let source subclasses
  lower `super(args)` when the external pack declares that superclass state is
  intentionally erased. In FASTQ this changes `AsyncFastqWriter::new(...)` from
  a `compile_error!` placeholder to `AsyncFastqWriter { writer: out }`.
- External template-method hooks now let inherited public methods satisfy
  source interfaces by delegating to subclass hooks. In FASTQ this changes
  missing `FastqWriter.write/close` placeholders into calls to
  `AsyncFastqWriter::synchronously_write(self, rec)` and
  `AsyncFastqWriter::synchronously_close(self)`.
  removing the move-out-of-`&self` failure.
- Known checked/unchecked external exception facts now take precedence over
  later parsed-source `Unknown` exception facts. In FASTQ this removes the
  illegal `Self::write(...)?` from `encode_fastq_record` and makes `write`
  return plain `Appendable`.
- Java platform API facts now include `PrintStream.checkError`, so emitted
  support returns `bool` instead of unit for boolean conditions.
- Java platform constructor argument facts now include
  `PrintStream(OutputStream)`, so emitted support uses
  `PrintStream::new(_arg0: OutputStream)` and constrains upstream generic
  stream helper return inference.
- Remaining compile categories include async superclass/interface method
  bridging, `Box<dyn FastqWriter>` clone recovery, `super(...)` constructor
  boundaries, and closure-to-`BiConsumer` coercion.
- Invalid-pattern categories are now one `compile_error_placeholder` file with
  three placeholders: the unsupported `super(...)` constructor and the two
  missing async interface method bodies. Runtime `invalid Java cast` patterns
  are clear in the latest focused FASTQ run.

Latest JAligner Tier 1 top categories:

- Active output: `.tmp/corpus-rerun-jaligner-after-fallthrough-catch-release2`.
- The focused JAligner rerun now translates all 29 files with 0 invalid-pattern
  files. The old `super(...)`, try/catch/finally, typed map downcast, and
  concrete-null invalid-pattern categories are cleared in this focused output.
- Generated crate assembly still reaches dependency resolution and then fails
  while fetching `ndarray` under restricted network. Do not count this as a
  Rust typechecking result until dependencies are available.
- `scripts/translate-focused-package.sh` now defaults to the release
  `new-java2rust` binary and records `translator_binary` in `summary.tsv`.
  Direct `Alignment.java` translation timed out after 60s in debug but
  completed in about 20s in release, so release builds are now the normal corpus
  gate path.
- Local fixtures now cover the general typed-map cast, explicit `Object`
  `super()` no-op, JAligner-style fallthrough try/catch in a value-returning
  `Result` method, JAligner-style try/catch/finally cleanup, target-typed
  external factory casts into concrete optional slots, no-arg real-super
  constructors followed by inherited setter calls, and the source-summary
  version of inherited `super(); setId(...)`.
- Local fixtures also cover the `Commons.getManifestBuildTimestamp` shape:
  `try { return value; } catch (Exception e) { side effect; } return null;`.
  Catch-only planning now allows a returning try body with a fallthrough catch,
  letting `Ok(value)` return immediately while the catch arm falls through to
  the following Java statement.
- Direct translation checks against the real JAligner files show
  `MatrixLoader.load*`, `SequenceParser.parse(File)`, and
  `Commons.getManifestBuildTimestamp` lower without
  `unsupported Java statement: try/catch/finally` placeholders.
- Direct translation of real `MatrixLoader.java` now emits
  `connection = Some(url.open_connection());` for the `JarURLConnection`
  factory cast, with no `downcast::<JarURLConnection>` placeholder. The
  regression fixture preserves typed optional external getter casts so
  `reader.get_header()` is not double-wrapped.
- Direct translation of real `CLUSTAL.java` and `FASTA.java` with the
  `jaligner/formats` context now lowers their `super(); setId(...)`
  constructors with `id: Option<String>`,
  `let mut __java_self = ... { id: None };`,
  `__java_self.set_id(...);`, and `return __java_self;`. Single-file direct
  translation without superclass context still cannot infer the inherited
  field/method surface, and the direct context run still reports the broad
  `unsupported_inheritance` diagnostic for the class. Treat this as remaining
  hierarchy-model work, not the old `super(...)` constructor-placeholder issue.
  The release focused rerun supplies enough context to clear the constructor
  placeholder corpus-wide.
- The one runtime downcast placeholder in the latest completed JAligner corpus
  output was a typed generic collection cast:
  `HashMap<String, Format>.get(id)` cast back to `Format` in
  `FormatFactory.getFormat`. Local fixture
  `typed_map_get_redundant_cast_uses_generic_value_shape` now covers the
  general rule: when the collection's generic value type already proves the
  cast target, lower through the collection get slot conversion instead of
  emitting `downcast::<T>()`. Direct real-file checks show
  `FormatFactory.getFormat` now returns the typed map value via `clone()`
  without a runtime downcast; the release focused rerun confirms the corpus-wide
  invalid-pattern count is now zero.
- Because this corpus is matrix-heavy, keep classifying its failures with the
  `Vec<Vec<T>>` to `ndarray` work, but do not treat the current invalid-pattern
  set as evidence against mechanical two-dimensional array lowering.
- The matrix/2D-array examples inspected so far are mostly not invalid-pattern
  failures. `Matrix`, `MatrixGenerator`, and the `byte[][] pointers` path in
  `NeedlemanWunsch` already lower toward `ndarray::Array2`; keep them as
  regression candidates rather than immediate blockers.

Latest VarScan Tier 1 status:

- Active outputs: `.tmp/corpus-rerun-varscan-tier1-current` and
  `.tmp/corpus-rerun-varscan-tier1-timeout180`.
- The first focused rerun did not finish after repeated polls and was
  interrupted to avoid leaving a live command session behind. The second rerun
  used `timeout 180` and exited before assembled crate summary. Both output
  directories had partial translated files and `status.tsv`, but no generated
  crate, `top-compile-errors.tsv`, or `invalid-patterns.tsv` yet.
- Keep it in active backlog until a fresh timed rerun finishes or records a
  timeout/scalability failure with concrete elapsed time and partial-file
  categories.
- Track the first finished VarScan evidence under three explicit buckets:
  timeout/scalability, partial-output/no assembled crate, and promoted
  compile-or-invalid-pattern fixture classes. At least one minimized
  `tests/idiom_gaps.rs` fixture should be added before implementing any fix for
  a concrete VarScan failure class.

| Corpus | Output | Result |
|---|---|---|
| Picard program groups | `.tmp/corpus-rerun-picard-programgroups-after-assembly-fixes` | 11/11 translated, 0 invalid idiom patterns, generated crate `cargo check` passes after dependency fetch |
| Picard program groups current | `.tmp/corpus-rerun-picard-programgroups-current` | 11/11 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| Picard program groups after inherited fields | `.tmp/corpus-rerun-picard-programgroups-after-inherited-fields` | 11/11 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| Picard program groups after inherited methods | `.tmp/corpus-rerun-picard-programgroups-after-inherited-methods` | 11/11 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| GATK GenomicsDB primary-only | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-inherited-methods` | 6/6 translated, 0 invalid idiom patterns, `cargo check` failed with 21 errors |
| GATK GenomicsDB primary-only after static field facts | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-static-field-facts` | 6/6 translated, 0 invalid idiom patterns, `cargo check` failed with 20 errors |
| GATK GenomicsDB primary-only after nested/support merge fixes | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-stream-getter-surfaces` | 6/6 translated, 0 invalid idiom patterns, `cargo check` failed with 12 errors |
| GATK GenomicsDB with broad engine summary | `.tmp/corpus-rerun-gatk-genomicsdb-primary-with-engine-summary` | 6/6 translated, 0 invalid idiom patterns, `cargo check` failed with 535 errors; broad engine summaries currently over-expand support and duplicate many source-summary field symbols |
| GATK Spark/DISQ primary-only | `.tmp/corpus-rerun-gatk-spark-datasources-primary-current` | 10/10 translated, 4 invalid-pattern files, `cargo check` failed with 71 errors |
| GATK Spark/DISQ primary-only after inherited fields | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-inherited-fields` | 10/10 translated, 4 invalid-pattern files, `cargo check` failed with 71 errors |
| GATK Spark/DISQ primary-only after inherited methods | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-inherited-methods` | 10/10 translated, 4 invalid-pattern files, `cargo check` failed with 71 errors |
| GATK GenomicsDB primary-only after Locatable conversion support | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-locatable-conversion-support` | 6/6 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| Picard program groups after Locatable conversion support | `.tmp/corpus-rerun-picard-programgroups-after-locatable-conversion-support` | 11/11 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| GATK GenomicsDB primary-only after raw erasure cast slot fixtures | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-raw-erasure-cast-slot` | 6/6 translated, 2 invalid-pattern files, `cargo check` failed with 29 reported errors; local fixtures now cover direct typed external raw `List` casts and optional-slot raw casts, but corpus count is unchanged |
| GATK GenomicsDB primary-only after static external slot shape | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-static-external-slot-shape` | 6/6 translated, 1 invalid-pattern file, `cargo check` failed with 26 reported errors; `Files.readAllLines` no longer emits `downcast::<List>()`, leaving three invalid casts in `GenomicsDBImport` |
| GATK GenomicsDB primary-only after lower-camel static-owner shadow guard | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-static-owner-shadow` | 6/6 translated, 1 invalid-pattern file, `cargo check` failed with 25 reported errors; inherited `progressMeter.setRecordsBetweenTimeChecks(...)` now emits an instance surface instead of a static fallback |
| GATK GenomicsDB primary-only after typed static returns and optional ternary wrapping | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-optional-ternary-wrap` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 20 reported errors; `smartMergeHeaders`, `newFixedThreadPool`, and optional interval ternaries no longer emit runtime downcast placeholders |
| GATK GenomicsDB primary-only after static-owner field guard | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-static-owner-field-guard-final` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 19 reported errors; inherited lowercase field receivers such as `progress_meter.setRecordsBetweenTimeChecks(...)` no longer synthesize static owner support, removing the `ProgressMeter` E0599 while preserving lower-snake imported adapter/static utility owner support |
| GATK GenomicsDB primary-only after erased-object and unchecked-throw fixes | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-erased-local-boxing` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 13 reported errors; unchecked direct throws now panic in non-`Result` methods, and concrete values flowing into `Box<dyn Any>` return/local slots are boxed through the shared slot conversion path |
| GATK GenomicsDB primary-only after source-summary nullability | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-source-summary-nullability` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 9 reported errors; source-summary method bodies that can return `null` now produce optional support surfaces and call-return shapes, removing the double-wrapped `SampleNameMap` path sites and the optional string-view comparison mismatch |
| GATK GenomicsDB primary-only after indexed optional borrow | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-indexed-optional-borrow` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 8 reported errors; indexed array elements flowing into `Option<&T>` slots now borrow explicitly, removing the `sampleNameIsLegal(Some(split[0]))` mismatch |
| GATK GenomicsDB primary-only after static nullable return shape | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-static-nullability-slot` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 7 reported errors; fact-declared nullable static external calls into optional fields now preserve the `Option<T>` result and avoid `Some(...downcast...)`, removing the `SAMSequenceDictionary` assignment mismatch |
| GATK GenomicsDB primary-only after source-static/optional-ternary attempt | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-source-static-owned-optional-ternary` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 7 reported errors; local gates stayed green, but the same six borrowed/owned corpus mismatches remained |
| GATK Spark/DISQ primary-only after Objects/Tabix support | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-objects-tabix-support` | 10/10 translated, 4 invalid-pattern files, `cargo check` failed with 15 reported errors |
| GATK Spark/DISQ primary-only after enum/default/Object/DISQ alias fixes | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-disq-alias-emission` | 10/10 translated, 3 invalid-pattern files, `cargo check` failed with 6 reported errors: 5 real unresolved symbols plus the final cargo summary |
| GATK Spark/DISQ primary-only after trait/wildcard/read facts | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-trait-wildcard-read-facts` | 10/10 translated, unresolved static read utility owners reduced, but type-shape and missing facade-surface errors increased to 58 reported errors |
| GATK Spark/DISQ primary-only after Broad alias/FlatMap support | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-broad-alias-flatmap-support` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 57 reported errors |
| GATK Spark/DISQ primary-only after facade surface batch | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-used-fact-surfaces` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 45 reported errors |
| GATK Spark/DISQ primary-only after source varargs packing | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-source-varargs-packing` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 45 reported errors; expanded `WriteOption...` calls are now packed as `vec![...]` |
| GATK Spark/DISQ primary-only after varargs conversion work | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-varargs-single-conversion` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 45 reported errors; standalone source varargs conversion fixture passes, but the DISQ source-summary case still emits concrete option elements in `Vec<WriteOption>` |
| GATK Spark/DISQ primary-only after builtin conversion adapters | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-builtin-conversion-adapters` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 43 reported errors; DISQ `WriteOption...` source-summary calls now emit `.into()` elements and shared support includes `impl From<...> for WriteOption` |
| GATK Spark/DISQ primary-only after try-with-resources close surfaces | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-twr-close-surfaces` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 39 reported errors; implicit try-with-resources cleanup now emits matching `close()` facade surfaces for `ReferenceSequenceFile` and `ReadsDataSource` |
| GATK Spark/DISQ primary-only after nullable DISQ argument facts | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-nullable-disq-args` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 36 reported errors; `HtsjdkReadsRddStorage.referenceSourcePath` now has nullable `String` argument facts and emits `Some(value)` instead of `Some(&value)` |
| GATK Spark/DISQ primary-only after nullable SAM referenceSequence facts | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-nullable-sam-reference-sequence` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 35 reported errors; `SamReaderFactory.referenceSequence` now accepts nullable `Path` and wraps the non-null branch in `Some(...)` |
| GATK Spark/DISQ primary-only after DISQ read overload facts | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-disq-read-overloads` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 34 reported errors; `HtsjdkReadsRddStorage.read` now declares `read/1` and `read/2`, producing `read_1`/`read_2` facade surfaces and call sites |
| GATK Spark/DISQ primary-only after owned fluent facade returns | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-owned-fluent-facades` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 33 reported errors; pack-owned self-returning fluent facades now return owned `Self` via clone, so chained factory/storage calls can initialize owned variables |
| GATK Spark/DISQ primary-only after optional borrowed source-argument wrap | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-optional-borrow-wrap` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 31 reported errors; reused owned values passed to `Option<&T>` source slots now lower as `Some(&value)` instead of `Some(value.clone())` |
| GATK Spark/DISQ primary-only after `if` null refinements | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-if-null-refinements` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 29 reported errors; ordinary `if (x == null) ... else ...` branches now carry non-null refinements into string-concat/display lowering, removing the two `Option<GATKPath>: Display` failures |
| GATK Spark/DISQ primary-only after stream reduce/signature lowering | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-stream-reduce-signatures` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 27 reported errors; `parallelStream().map(...).reduce(...)` now lowers through the shared stream planner and same-class lambda calls select overload names such as `get_parallel_variants_string_list` |
| GATK Spark/DISQ primary-only after `JavaRDD.union` and captured stream clones | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-union-captured-clone` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 26 reported errors; Spark `JavaRDD.union` now emits a no-op facade method and same-class stream lambda calls clone captured owned arguments such as `intervals` |
| GATK Spark/DISQ primary-only after assignment-condition loop shadow refinement | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-loop-shadow-refinement` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 23 reported errors; assignment-condition loops such as `(seq = next()) != null` now lower their body with the emitted non-null shadow local shape, removing the three `ReferenceSequence.as_ref().unwrap()` receiver failures |
| GATK Spark/DISQ primary-only after Rust-keyed method usage facts | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-overload-usage-rust-keys` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 20 reported errors; method usage facts are now also keyed by resolved Rust overload name, allowing null-call propagation to mark exact overloaded callee parameters optional |
| GATK Spark/DISQ primary-only after borrowed constructor slot conversion | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-borrowed-constructor-slot` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 19 reported errors; external constructor expressions now still pass through slot conversion, so `new GATKPath(...)` can lower as `&GATKPath::new(...)` for borrowed source-summary helper parameters |
| GATK Spark/DISQ primary-only after optional field/call return shaping | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-call-return-precedence` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 18 reported errors; `this.field` optional assignments now use the current-type field signature and planned method-call return shapes win before generic optional wrapping, removing the double-wrapped `Option` locals |
| GATK Spark/DISQ primary-only after static string-slice iteration | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-static-string-slice-iteration` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 17 reported errors; typed `FileExtensions.FASTA` facts now emit an iterable static string slice and enhanced-for lowers it with `.iter().copied()` |
| GATK Spark/DISQ primary-only after `SAMSequenceDictionary.getSequence` type fact | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-sam-dictionary-getsequence-type` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 16 reported errors; `SAMSequenceDictionary.getSequence` now has an explicit `SAMSequenceRecord` return fact, fixing the previous unit-return facade surface and the follow-on `std::cmp::min` operand shape |
| GATK Spark/DISQ primary-only after `SAMSequenceRecord.getSequenceLength` type fact | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-sam-record-sequence-length-type` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 15 reported errors; `SAMSequenceRecord.getSequenceLength` now has an explicit `int` return fact |
| GATK Spark/DISQ primary-only after `TwoBitReference.getReferenceBases` return correction | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-twobit-reference-sequence-return` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 14 reported errors; `TwoBitReference.getReferenceBases` now returns `ReferenceSequence` for the observed source path |
| GATK Spark/DISQ primary-only after `ReadsPathDataSource` conversion | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-readspath-datasource-conversion` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 13 reported errors; Broad `ReadsPathDataSource` now adapts into `ReadsDataSource` through explicit javalibs conversion facts |
| GATK Spark/DISQ primary-only after comparator conversion | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-comparator-conversion` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 12 reported errors; `HeaderlessSAMRecordCoordinateComparator` now adapts into `Comparator`, and generic external adapter lookup matches erased target types |
| GATK Spark/DISQ primary-only after switch/exception shape fixes | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-switch-exception-shape` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 11 reported errors; exception constructor message arguments are stringified and fallback switch rendering no longer emits the original bare labeled break shape |
| GATK Spark/DISQ primary-only after Spark/seekable facade surfaces | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-spark-seekable-surfaces` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 10 reported errors; `SparkUtils.hadoopPathExists` and `SeekableStreamFactory.getInstance/getStreamFor` now have typed facade surfaces |
| GATK Spark/DISQ primary-only after `Files.exists` and switch-tail fixes | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-files-exists-switch-tail` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 9 reported errors; JDK `Files.exists` now has a non-throwing `bool` surface and exhaustive terminal fallback switches append an explicit unreachable tail |
| GATK Spark/DISQ primary-only after refined optional receiver borrowing | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-refined-optional-receiver-borrow` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 8 reported errors; refined optional method receivers now borrow through `as_ref().unwrap()` and reused optional display segments render through `as_ref().map(...)`, removing the `referencePathSpecifier` move/reuse failure |
| GATK Spark/DISQ primary-only after fact-aware varargs overload retargeting | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-fact-aware-varargs-overload` | 10/10 translated, 2 invalid-pattern files, `cargo check` failed with 7 reported errors; same-class source overload fallback now recognizes expanded varargs using explicit `implements`/conversion facts, fixing the `writeReads(..., splittingIndexGranularity, WriteOption...)` fixed-overload misselection |
| GATK Spark/DISQ primary-only after adapter return facts and callback param shaping | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-spark-callback-param-shapes-v3` | 10/10 translated, 1 invalid-pattern file, `cargo check` failed with 14 reported errors; `SAMRecordToGATKReadAdapter.headerlessReadAdapter`, `VariantContextVariantAdapter.sparkVariantAdapter`, and direct `JavaRDD.map` lambda parameter typing now have focused regressions, but the corpus still projects several Spark/DISQ facade/source-summary values to `Vec<T>` and loses Spark facade methods such as `map`, `filter`, `mapToPair`, and `mapPartitions` |
| GATK GenomicsDB primary-only after uninitialized owned optionals | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-uninitialized-owned-optionals` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 2 reported errors; uninitialized optional locals assigned temporary optional values no longer emit `E0716`, leaving one borrowed `HashMap<String, URI>` argument mismatch |
| GATK GenomicsDB primary-only after shared-borrow owned clone attempt | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-shared-borrow-owned-clone` | 6/6 translated, 0 invalid-pattern files, `cargo check` failed with 2 reported errors; direct shared-borrow argument recovery is covered locally, but the lowered conditional return still bypasses the same-class call plan for `getFeatureReadersInParallel` |
| Picard program groups current inclusion check | `.tmp/corpus-rerun-picard-programgroups-current-inclusion-check` | 11/11 translated, 0 invalid idiom patterns, generated crate `cargo check` passes |
| GATK Spark/DISQ primary-only current inclusion check | `.tmp/corpus-rerun-gatk-spark-datasources-primary-current-inclusion-check` | 10/10 translated, 1 invalid-pattern file, `cargo check` failed with 7 reported errors; active gaps are borrowed DISQ write-option varargs conversion, borrowed dictionary trait argument recovery, and one cast placeholder |
| GATK Spark/DISQ primary-only after shared-borrow owned clone attempt | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-shared-borrow-owned-clone` | 10/10 translated, 1 invalid-pattern file, `cargo check` failed with 7 reported errors; ordinary shared-borrow call recovery did not reach the varargs packing path or interface delegation path |
| GATK GenomicsDB primary-only after cast/varargs/trait fixes | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-cast-varargs-trait-fixes` | 6/6 translated, 0 invalid-pattern files, generated crate `cargo check` passes |
| GATK Spark/DISQ primary-only after global trait signature | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-global-trait-signature` | 10/10 translated, 1 invalid-pattern file, generated crate `cargo check` passes; only the `Broadcast<SAMFileHeader>.getValue()` erased return placeholder remained |
| GATK Spark/DISQ primary-only after Broadcast generic return fact | `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-broadcast-generic` | 10/10 translated, 0 invalid-pattern files, generated crate `cargo check` passes |
| GATK GenomicsDB primary-only after Broadcast generic return fact | `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-broadcast-generic` | 6/6 translated, 0 invalid-pattern files, generated crate `cargo check` passes |
| HTSJDK FASTQ Tier 1 current | `.tmp/corpus-rerun-htsjdk-fastq-tier1-current` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 48 reported errors |
| HTSJDK FASTQ after string helper support | `.tmp/corpus-rerun-htsjdk-fastq-after-string-helper-support` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 43 reported errors; Java string helper calls were emitted and focused-crate support injection learned `substring`/`hash_code` |
| HTSJDK FASTQ after current instance field/null fixes | `.tmp/corpus-rerun-htsjdk-fastq-after-string-field-fixes` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 42 reported errors; bare final instance field reads no longer become static constants |
| HTSJDK FASTQ after string shape retarget | `.tmp/corpus-rerun-htsjdk-fastq-after-string-shape-retarget` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 42 reported errors; one fallback-shaped nullable field `hashCode` call still bypasses the string helper |
| HTSJDK FASTQ after enum constant access | `.tmp/corpus-rerun-htsjdk-fastq-after-enum-constant-access` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 35 Rust type errors; nested enum constants no longer emit `LineType(...)`/`line_type` payload-shaped access |
| HTSJDK FASTQ after Java enum display | `.tmp/corpus-rerun-htsjdk-fastq-after-java-enum-display` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 32 reported errors; emitted Java enums no longer get duplicate opaque facade equality impls, and enum display/hash support is emitted by the Java enum representation |
| HTSJDK FASTQ after source struct derives | `.tmp/corpus-rerun-htsjdk-fastq-after-source-struct-derives` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 28 reported errors; field-safe source structs now derive `Clone` and `PartialEq`, removing the prior `FastqRecord` owned-slot clone bucket |
| HTSJDK FASTQ after concrete self-cast handling | `.tmp/corpus-rerun-htsjdk-fastq-after-source-self-cast` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 27 reported errors; concrete current-receiver casts no longer emit runtime downcast placeholders |
| HTSJDK FASTQ after Java Object identity | `.tmp/corpus-rerun-htsjdk-fastq-after-object-identity` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 27 reported errors; `FastqRecord.equals(Object)` now lowers `this == obj` as erased-object identity rather than incompatible Rust equality |
| HTSJDK FASTQ after exact `getClass` predicate | `.tmp/corpus-rerun-htsjdk-fastq-after-getclass-predicate` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 26 reported errors; exact class comparisons now use the erased-object type predicate path |
| HTSJDK FASTQ after exact-class cast refinement | `.tmp/corpus-rerun-htsjdk-fastq-after-exact-class-cast-refinement` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 26 reported errors; guarded erased-object casts now lower to borrowed `downcast_ref` views |
| HTSJDK FASTQ after optional field receiver adapter | `.tmp/corpus-rerun-htsjdk-fastq-after-optional-field-receiver-adapter` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 25 reported errors; optional field string-helper receiver adaptation fixed the `quality_header.as_str()` corpus site |
| HTSJDK FASTQ after SAMRecord FASTQ getters | `.tmp/corpus-rerun-htsjdk-fastq-after-samrecord-fastq-getters` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 23 reported errors; typed javalibs SAMRecord getter support removed the two `get_read_string`/`get_base_quality_string` unresolved-method errors |
| HTSJDK FASTQ after static byte-array constants | `.tmp/corpus-rerun-htsjdk-fastq-after-static-byte-array-constants-narrowed` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 21 reported errors; typed primitive-array static constants removed the `SAMRecord::NULL_SEQUENCE` and `SAMRecord::NULL_QUALS` unresolved associated constants |
| HTSJDK FASTQ after external inherited static finals | `.tmp/corpus-rerun-htsjdk-fastq-after-external-inherited-static-final` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 20 reported errors; inherited external/source-summary static-final constants now emit subclass-owner associated constants, removing `AsyncFastqWriter::DEFAULT_QUEUE_SIZE` |
| HTSJDK FASTQ after boxed-dyn clone parentheses | `.tmp/corpus-rerun-htsjdk-fastq-after-lower-clone-parens` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 19 reported errors; boxed-dyn cast receivers are parenthesized before audited clone recovery, removing the Rust parse error bucket |
| HTSJDK FASTQ after null-guarded ternary display | `.tmp/corpus-rerun-htsjdk-fastq-after-null-guarded-ternary-display-field-shape` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 18 reported errors; nullable current-instance field display branches now unwrap the non-null arm before string conversion, removing the `Option<File>: Display` bucket |
| HTSJDK FASTQ after current instance/static alias precedence | `.tmp/corpus-rerun-htsjdk-fastq-after-instance-field-static-alias-precedence` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 17 reported errors; nullable source getter/hash-code sites now keep current instance field shape even when emitted sibling static constants normalize to the same Rust name |
| HTSJDK FASTQ after do-while/BufferedReader | `.tmp/corpus-rerun-htsjdk-fastq-after-do-while-bufferedreader` | 8/8 translated, 3 invalid-pattern files, `cargo check` failed with 16 reported errors; do-while lowering and `BufferedReader.readLine` external facts remove the uninitialized `line` bucket |
| JAligner Tier 1 current | `.tmp/corpus-rerun-jaligner-tier1-current` | 29/29 translated, 20 invalid-pattern files, generated crate `cargo check` blocked on fetching `ndarray`; active invalid-pattern backlog remains |
| JAligner after typed map cast fixture | `.tmp/corpus-rerun-jaligner-after-typed-map-cast` | interrupted after repeated no-output polls; partial per-file output only, but local fixture proves the generic map-get cast rule |
| JAligner after fallthrough-catch/release rerun | `.tmp/corpus-rerun-jaligner-after-fallthrough-catch-release2` | 29/29 translated, 0 invalid-pattern files, generated crate `cargo check` blocked on fetching `ndarray` |
| VarScan Tier 1 current | `.tmp/corpus-rerun-varscan-tier1-current` | interrupted after repeated no-output polls; partial per-file output only |
| VarScan Tier 1 timeout 180 | `.tmp/corpus-rerun-varscan-tier1-timeout180` | timed out after 180s before assembled crate summary; partial per-file output only |

Current corpus gap inventory to keep alongside `idiom_gaps`:

| Corpus | Include In Backlog? | Current Gap Categories |
|---|---:|---|
| Picard program groups | No current compile gap | Latest focused gate passes generated crate `cargo check`; rerun as a regression guard after broad support/facade changes. |
| GATK GenomicsDB primary-only | No current compile gap | Latest focused gate passes generated crate `cargo check` with 0 invalid-pattern files; keep as a regression guard after broad support/facade changes. |
| GATK GenomicsDB full context | Yes | Full-context runs still time out or over-expand support; needs source-summary indexing, minimal superclass-summary mode, and duplicate source-summary field/type suppression. |
| GATK Spark/DISQ primary-only | No current compile gap | Latest focused gate passes generated crate `cargo check` with 0 invalid-pattern files; keep as a regression guard after Spark/DISQ facade or stream changes. |
| GATK Spark/DISQ full context | Yes | Same Spark/DISQ shape gaps plus full-context timeout/scalability risk. |
| HTSJDK FASTQ | No current compile gap | Latest focused gate `.tmp/corpus-rerun-htsjdk-fastq-after-async-template-super` passes generated crate `cargo check` with 0 invalid-pattern files; keep as a regression guard. |
| JAligner | Partial | Latest release-backed focused gate has 0 invalid-pattern files; generated crate typecheck is blocked by dependency fetch for `ndarray`, so compile-quality validation still needs a dependency-enabled rerun. |
| VarScan | Yes | Current Tier 1 rerun was interrupted with partial output only; promote concrete compile/invalid-pattern categories as soon as a fresh timed rerun produces an assembled crate or timeout result. |

Treat any newly rerun corpus whose generated crate fails `cargo check`, emits
invalid idiom patterns, or times out before crate assembly as part of this same
inventory. Picard is excluded only because the latest focused gate is clean; if
a later Picard rerun fails, add its top compile/invalid-pattern categories here
before implementing another targeted fix.

Corpus inclusion policy:

- A green local `cargo test --test idiom_gaps` run only proves the minimized
  fixture suite. It does not clear the real-code backlog.
- Every external corpus listed in `docs/real-code-corpora.md` is either a
  regression guard or an active idiom-gap source. There are no ignored
  "other corpus" failures: if a gate fails, its top compile categories,
  invalid-pattern categories, and timeout/scalability failures belong in this
  file until a later rerun is clean.
- If a corpus rerun fails, assume it contributes active idiom gaps until the
  generated crate compiles and invalid-pattern counts are zero.
- This applies even when we expect the rerun to fail: record the failure class
  immediately and use it to seed fixture coverage, rather than waiting for a
  later all-corpus cleanup pass.
- For failed corpuses, add a compact fixture or manifest test for each new
  failure class before relying on a corpus-specific output improvement.
- Do not exclude a corpus because its failures are outside GATK/Picard. Tier 1
  corpuses such as Trimmomatic, JAligner, JAHMM, VarScan, HTSJDK FASTQ/VCF,
  BioJava, and JHLabs should be tracked the same way once rerun.
- If a corpus is expected to fail, include it anyway: mark the expected failure
  as an active gap once the rerun shows a compile error, invalid pattern, or
  timeout, and keep it in this file until a clean rerun removes it.
- When a corpus has not been rerun in the current batch, assume failure for
  planning purposes only: put it in the "Include if rerun fails" bucket below,
  then promote it to active backlog immediately after evidence appears. Do not
  wait for the local `idiom_gaps` fixture suite to fail before tracking it.

Real-code corpus backlog status:

| Corpus | Status Policy |
|---|---|
| `old-golden-snippets` | Tier 0 regression guard; failures become active fixture gaps. |
| `current-fixtures` | Tier 0 regression guard; failures block the local gate. |
| `trimmomatic` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `jaligner` | No current invalid-pattern gap; latest release-backed focused rerun has 0 invalid-pattern files, but generated crate checking remains blocked by fetching `ndarray`. |
| `jahmm` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `varscan` | Active backlog: current rerun was interrupted with partial output; record final categories after a fresh timed rerun completes or times out. |
| `htsjdk-fastq` | Active backlog: latest rerun fails generated crate `cargo check` and invalid-pattern scan. |
| `htsjdk-vcf` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `picard-programgroups` | Currently a regression guard; include again if a later rerun fails. |
| `biojava-aaprop` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `biojava-alignment` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `jhlabs` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `jsoup` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `fastqc` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `gatk-genomicsdb` | Focused primary-only slice is currently a regression guard; full-context timeout/support over-expansion remains active backlog. |
| `gatk-disq-spark-datasources` | Focused primary-only slice is currently a regression guard; full-context timeout/scalability risk remains active backlog. |
| `bioformats` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `picard` | Assumed backlog until rerun; do not infer full Picard status from the clean programgroups slice. |
| `htsjdk` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `jts` | Assumed backlog until rerun; promote observed compile/invalid-pattern failures into `idiom_gaps`. |
| `gatk` | Assumed backlog until rerun; promote compile failures, invalid patterns, and timeout/scalability failures into active gaps. |

Assumed-failing corpus promotion plan:

| Corpus group | First evidence to collect | Fixture rule |
|---|---|---|
| Tier 1 small real-code corpuses: `trimmomatic`, `jaligner`, `jahmm`, `varscan`, `htsjdk-fastq`, `htsjdk-vcf`, `biojava-aaprop`, `biojava-alignment`, `jhlabs` | Rerun each focused corpus, then record `summary.tsv`, `top-compile-errors.tsv`, and `invalid-patterns.tsv` next to the latest output directory. | Add one minimized `tests/idiom_gaps.rs` fixture per new failure class before implementing a translator or `javalibs` fix. |
| Tier 2 dependency/project corpuses: `jsoup`, `fastqc`, `gatk-genomicsdb`, `gatk-disq-spark-datasources` | Keep `gatk-genomicsdb` and `gatk-disq-spark-datasources` active now; rerun `jsoup` and `fastqc` once Tier 1 is not producing new parser/control-flow blockers. | Prefer general language/runtime fixtures first; add package facts only when the failure is a true external boundary. |
| Tier 3 large corpuses: `bioformats`, `picard`, `htsjdk`, `jts` | Rerun after the smaller corpus classes stop adding new broad rules, or earlier if a fix touches object model, generics, numeric math, or external facts. | Promote repeated failures into fixtures by idiom class, not by source file name. Do not use clean `picard-programgroups` as evidence that full `picard` is clean. |
| Tier 4 stress corpus: `gatk` | Treat timeout, support over-expansion, duplicate source-summary symbols, compile failures, and invalid patterns as active gaps. | Use focused slices for minimized fixtures; keep full-corpus scalability as its own tracked gap when it times out. |

Assumed-failing backlog index:

| Corpus | Tracking state | Promotion rule |
|---|---|---|
| `trimmomatic` | Assumed failing until a fresh Tier 1 rerun proves `cargo check` and invalid-pattern gates are clean. | Promote parser/control-flow, collection, IO, and external-boundary failures into minimized fixtures before fixing. |
| `jaligner` | Regression guard for invalid patterns; generated crate typecheck remains blocked by `ndarray` fetch. Matrix/2D-array lowering remains a regression pressure area. | Keep exception/super/typed-map fixtures active; once dependencies are available, promote any concrete Rust compile category into a minimized fixture. |
| `jahmm` | Assumed failing until rerun; not currently represented by a focused fixture. | First concrete failure should become a general math/generics/control-flow fixture rather than a corpus-specific rule. |
| `varscan` | Active backlog from timeout/partial-output evidence. | Treat timeout/no-crate assembly as its own scalability gap; promote the first concrete compile or invalid-pattern class once a rerun finishes. |
| `htsjdk-fastq` | Active backlog from focused compile failures and placeholder evidence. | Continue adding general fixtures for interface inheritance, `Appendable`, checked/unchecked exceptions, and facade slot shape. |
| `htsjdk-vcf` | Assumed failing until rerun; adjacent to HTSJDK external facts but separate from FASTQ. | Promote typed constants, variant/VCF domain facades, generics, and collection failures into fixtures when observed. |
| `picard-programgroups` | Current focused gate is clean, so it is a regression guard. | Any future compile, invalid-pattern, or timeout failure re-enters active backlog with a minimized source-summary/fact fixture. |
| `gatk-genomicsdb` | Current primary-only gate is clean, full-context scalability remains tracked. | Native GenomicsDB calls stay FFI handover/boundary facts; ordinary Java orchestration failures become fixtures. |
| `gatk-disq-spark-datasources` | Current primary-only gate is clean, distributed-boundary behavior remains explicitly modeled as no-op facades. | DISQ/Spark execution semantics are boundary facts; ordinary stream/slot/callback failures become fixtures. |
| `biojava-aaprop` | Assumed failing until rerun. | Promote numeric/math, table lookup, and resource-loading failures into fixtures. |
| `biojava-alignment` | Assumed failing until rerun. | Promote alignment/matrix, dynamic programming array, and substitution-matrix idioms into fixtures. |
| `jhlabs` | Assumed failing until rerun. | Promote image/math/array-loop idioms into fixtures; prioritize general numeric and array lowering. |
| `jsoup` | Assumed failing Tier 2 backlog. | Promote parser/tree/string/collection idioms into fixtures after Tier 1 blockers stop moving. |
| `fastqc` | Assumed failing Tier 2 backlog. | Promote IO/reporting/chart/table idioms and package boundaries into fixtures when rerun evidence exists. |
| `bioformats` | Assumed failing large-corpus backlog. | Promote repeated metadata/IO/numeric/external-format failures by idiom class. |
| `picard` | Assumed failing large-corpus backlog despite clean `picard-programgroups`. | Promote full Picard command-line, metrics, HTSJDK integration, and source-summary failures separately from the small slice. |
| `htsjdk` | Assumed failing large-corpus backlog despite focused FASTQ work. | Promote repeated generics, interfaces, records, constants, and IO/resource patterns into fixtures. |
| `jts` | Assumed failing large-corpus backlog. | Promote geometry/numeric/object-model idioms into fixtures; math stays high priority. |
| `gatk` | Assumed failing stress backlog. | Use focused slices for minimized fixtures; record full-corpus timeout/scalability separately from ordinary Java idiom gaps. |

Completed in this pass:

- External/platform return facts now feed legacy return-type queries and
  imported static facade support, so JDK/static external factories can emit
  concrete support surfaces such as
  `JavaUtilConcurrentExecutors::new_fixed_thread_pool(...) -> ExecutorService`.
- Slot conversion now refuses erased-object downcasts to lower-camel
  method-name placeholders, preventing uncompilable targets such as
  `downcast::<newFixedThreadPool>()`.
- Optional ternary lowering now distinguishes non-null value branches from
  already-optional branches. Non-null branches are lowered against the optional
  element shape and wrapped once at the outer ternary, fixing the GenomicsDB
  interval assignment while preserving nullable getter/path ternary behavior.
- Focused fixture coverage increased to 210 passing `idiom_gaps` tests,
  including static external optional slots, generic `Set` slots, raw-list
  optional ternary casts, plain optional list ternary assignment, inherited
  external field receivers that must not be mistaken for static owners,
  unchecked direct throws in plain methods, and concrete values flowing into
  erased `Box<dyn Any>` return/local slots.
- GenomicsDB primary-only moved from 1 invalid-pattern file and 25 reported
  compile errors to 0 invalid-pattern files and 20 reported compile errors in
  `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-optional-ternary-wrap`.
- The static-owner field guard then moved GenomicsDB primary-only to 0
  invalid-pattern files and 19 reported compile errors in
  `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-static-owner-field-guard-final`.
- Erased-object boundary and unchecked direct-throw fixes moved GenomicsDB
  primary-only to 0 invalid-pattern files and 13 reported compile errors in
  `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-erased-local-boxing`.
  Previously failing sites now lower as boxed erased values for
  `InitializedQueryWrapper.getHeader`, `onTraversalSuccess`, and local
  `FeatureReader` factory initializers, while unchecked
  `UnsupportedOperationException` methods no longer emit `return Err(...)` from
  non-`Result` signatures.
- Shared support merge now uses observed call-site receiver shape when choosing
  between duplicate method variants, so static factory calls such as protobuf
  `new_builder()` keep static support while instance call sites prefer
  `&self`/`&mut self`/owned-self variants. The focused GenomicsDB gate moved
  from 30 to 29 reported compile errors in
  `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-shared-support-receiver-preference`.
- A broader experiment that stripped generated modules again after shared
  support merge regressed GenomicsDB to 95 reported errors in
  `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-post-merge-shared-strip`.
  Do not retry that as a broad pass; the remaining `ProgressMeter` failure
  needs a narrower rule for local weak shell shadowing or root export selection.
- Optional slot lowering now preserves inferred expression ownership instead of
  inventing owned element shapes for non-null expressions flowing into
  `Option<T>`. This lets ternary branches, nullable facade arguments, optional
  callback locals, map gets, and borrowed source arguments share the slot
  conversion planner for `Some(...)` payload adaptation.
- String view ownership conversion is idempotent for already-owned
  `.to_string()` expressions, removing redundant `.to_string().to_string()` in
  optional string slots.
- Fixture expectations were refreshed for the current borrowed source-signature
  policy: read-only params may be `&T`/`Option<&T>`, while owned recovery still
  happens at the precise payload or call boundary.
- Static methods that return borrowed aliases from borrowed parameters now emit an explicit synthetic lifetime, fixing nullable alias helpers such as `ReadsSparkSource.checkCramReference`.
- Method signature planning now treats nullable object receivers through their optional element type for external read-only facts, and allows conservative `is*`/`has*`/`can*` validator calls in internal borrow-safe alias helpers.
- Checked external throw collection is now catch-aware, and `Result` wrapping only depends on checked thrown effects, so checked calls caught and rethrown as unchecked `RuntimeException` do not force callers to use `?`.
- Stream-like `filter` preserves receiver element shape for subsequent callback lowering, fixing chained Spark `filter(...).map(...)` closure inference.
- Context fallback receiver candidates are no longer dropped merely because the current translation unit lacks a signature for the context type; source-summary receiver matching also accepts snake-case method names.
- Generated crate assembly now emits `ndarray` when support or modules use `ndarray::`.
- Shared support discovery now catches `JavaClass<T>` uses outside direct struct declarations.
- Shared support merger boxes direct self-recursive fields before promoting support structs.
- `SimpleInterval` constructor arity facts now include both arity 1 and arity 3.
- Constructor call planning now consults builtin javalibs constructor arities, including resolved external constructors.
- Generated path support now emits `URI::parse` and `URI::get_path`.
- Shared support alias-owned methods are redirected to the alias target before emission.
- Shared support merger removes module declarations for aliases promoted into shared support.
- Shared support merger removes earlier prelude stubs when a richer local declaration for the same generated type appears later.
- `Result` propagation is idempotent, removing generated `??` from the GenomicsDB and Spark/DISQ slices.
- Shared support merger folds promoted alias impl methods/constants into the concrete target, fixing the GenomicsDB `VidMappingPB.getContigsList` alias shape.
- External facade support now counts builtin `javalibs` constructor overloads when emitting support methods, aligning `Interval::new_1`/`VCFHeader::new_1` support with call lowering.
- Broad Hellbender `FeatureInput.toPath` is modeled as a typed `Path` facade fact.
- Used builtin side-effect API facts can backfill concrete support methods for already-emitted non-builder opaque types, fixing `IntervalList.add` without importing all builtin methods into support.
- `Collectors.toMap` stream lowering now respects owned iterator elements for identity lambdas, removing the `(*item).clone()` deref/clone mismatch after `into_iter()`.
- Stream lambda static-owner rendering now consumes imported static type aliases, so snake-cased parsed owners such as `genomics_d_b_utils` lower to the canonical imported owner.
- Resolved static source calls now win over heuristic static-utility fallback and normalize snake-case declaring types, fixing same-package helpers such as `GATKGenomicsDBUtils.getProtobufVidMappingFromJsonFile`.
- External inherited instance field facts are projected onto emitted concrete subclasses when source declares an external superclass. This fixes primary-only GenomicsDB lowering for inherited `GATKTool` fields including `intervalArgumentCollection`, `progressMeter`, `cloudPrefetchBuffer`, and `cloudIndexPrefetchBuffer` without loading the broad engine source-summary corpus.
- External inherited field types are now included in generated facade support, so inherited field types such as `IntervalArgumentCollection` get local placeholder structs and method surfaces.
- External inherited instance method facts are projected onto emitted concrete subclasses when source declares an external superclass. This fixes receiverless `GATKTool.getDefaultToolVCFHeaderLines` in primary-only GenomicsDB by emitting a concrete `GenomicsDBImport::get_default_tool_v_c_f_header_lines` facade method and lowering calls to `self.get_default_tool_v_c_f_header_lines()`.
- Intel GKL callback argument fields are modeled as external instance fields, and typed external instance-field facts can enrich already emitted opaque support shells. This fixes `BatchCompletionCallbackFunctionArgument.batchCount` and `totalBatchCount` in the GenomicsDB focused slice.
- GenomicsDB nested protobuf owner fields are now typed as `NestedType` external field facts instead of generated static-final values, removing the bogus `pub const ContigPosition: ContigPosition = ContigPosition` emission.
- `org.genomicsdb.Constants.CHROMOSOME_FOLDER_DELIMITER_SYMBOL_REGEX` is now a typed javalibs field fact; static constant support uses the selected fact owner and emits a simple-owner type alias when an import proves the package.
- Shared support merge now preserves source-owned nested classes declared before the primary type while removing earlier empty stubs for the same nested class. This fixes `GenomicsDBImportInitializedQueryWrapper` and `GenomicsDBImportUnableToCreateGenomicsDBWorkspace` declaration loss during focused-package assembly.
- Shared support merge now prefers concrete method variants over weak generic fallbacks, fixing `OrgGenomicsdbGenomicsDBUtils::read_entire_file` as `&str -> String` instead of `R: Default`.
- Shared support merge now de-duplicates alias-target methods by method name after folding promoted alias impls into the target.
- External support emission can backfill missing builtin `get_*` javalibs method surfaces for already-emitted facade receivers, fixing stream lambda calls such as `GenomicsDBVidMapProtoChromosome::get_tiledb_column_offset` without enabling broad builtin support emission.
- Source context files now feed semantic/signature planning while only selected primary files are emitted, so primary-only source static helper calls can use context signatures without pulling broad context into the generated crate.
- Optional sequence receivers are unwrapped before `.get(...)`, and borrowed `SimpleInterval` elements can be adapted to `Locatable` slots through `javalibs` subtype knowledge plus generated conversion support.
- Stream block lambda local initializers clone owned `String` array elements instead of moving out of `Vec<String>`.
- DISQ static facade factories such as `BaiWriteOption.fromBoolean`, `SbiWriteOption.fromBoolean`, and `HtsjdkReadsRddStorage.makeDefault` now render as associated facade functions rather than snake-case modules.
- DISQ variant read boundary facts now cover `HtsjdkVariantsRddStorage.read` and `HtsjdkVariantsRdd.getVariants`, keeping the boundary explicit without `external_pack_missing_api` diagnostics.
- External enum switch labels backed by typed field facts now lower as associated constants such as `SortOrder::coordinate` instead of unresolved bare labels.
- Optional boxed dyn-trait storage slots now default through `Option::default()`/`None`, fixing `ReferenceMultiSparkSource` constructor defaults for nullable interface fields.
- Boxed dyn-trait receiver fallback calls now borrow source-summary object parameters through the trait slot shape. This fixes `ReferenceMultiSparkSource.getReferenceBases(SimpleInterval)` delegation as `get_reference_bases(&interval)` instead of passing the owned interval into a borrowed trait method.
- JDK `Object` placeholders can now be emitted into shared focused-crate support instead of being filtered as an unavailable builtin.
- DISQ write-option qualified owner aliases are emitted in shared support, so generated long owners such as `OrgDisqBioDisqTabixIndexWriteOption` can resolve to `TabixIndexWriteOption`.
- Focused-package assembly now indexes sibling traits as exported support types, fixing missing `ReferenceSparkSource` imports.
- `javalibs` can seed wildcard-import static aliases for Broad read helpers, and the external packs now include typed facade facts for `ReadUtils.isReadNameGroupedBam` and `SAMRecordToGATKReadAdapter.headerlessReadAdapter`.
- The focused Spark/DISQ crate now emits a no-op `FlatMapFunction` boundary type when translated Spark callback adapters refer to it.
- Focused shared-support stripping now removes impl blocks for promoted shared types, preventing dangling platform impls when `Path`/`File`/`URI` support is centralized.
- Platform path support now includes a no-op `FileSystem` facade with `Path.getFileSystem(Configuration)` and `FileSystem.delete(...)`.
- Typed static-final facade support now emits both Java-spelled associated constants and snake-case aliases when the fact is selected, fixing owners such as `ReadsWriteFormat::SINGLE` and `SAMRecord::NO_ALIGNMENT_START`.
- Used external-pack facts are no longer restricted to `get_*` methods when backfilling support surfaces, and DISQ/Broad packs now cover `HtsjdkReadsRddStorage.write`, `HtsjdkReadsRddStorage.sbiIndexGranularity`, `TraversalParameters.traverseUnmappedReads`, `TwoBitReference.getReferenceBases`, `ReadsDataSource.getHeader`, `ReadsDataSource.close`, and `SortOrder.getComparatorInstance`.
- Focused shared support emits Broad read-helper owner aliases for long owners such as `OrgBroadinstituteHellbenderUtilsReadReadUtils` when generated call sites still reference them.
- Source-declared varargs calls now pack trailing arguments through the shared call target path instead of emitting each trailing argument as a separate Rust call argument.
- Varargs element lowering now has a focused regression for explicit external `into` adapters and emits exactly one conversion for ordinary source varargs, including borrowed slice-shaped source signatures.
- DISQ write-option facts now declare `WriteOption` implementation relationships and explicit `into` adapter facts for `ReadsFormatWriteOption`, `FileCardinalityWriteOption`, `TempPartsDirectoryWriteOption`, `BaiWriteOption`, `SbiWriteOption`, and `TabixIndexWriteOption`.
- External conversion adapter lookup now accepts unresolved external type names as explicit keys, so `Unknown("ReadsFormatWriteOption") -> Unknown("WriteOption")` can still select an adapter fact.
- External facade support can synthesize trivial `impl From<Concrete> for Boundary` surfaces for explicit pack-owned `into` adapter facts when both sides are emitted as opaque support types.
- Builtin javalibs conversion adapters are now visible to both call conversion planning and generated facade support emission, fixing real Spark/DISQ source-summary `WriteOption...` calls such as `ReadsSparkSink`.
- External call receiver effect lookup now gives typed receiver inference precedence for qualified field access while preserving simple imported static owners, so typed external fields can still drive throwing/return facts.
- Exact generic external calls flowing into matching generic slots now keep the external return shape rather than inserting a raw-erasure downcast; this fixes `Files.readAllLines(Path) -> Vec<String>` local assignment in the GenomicsDB `SampleNameMap` slice.
- Imported static-owner collection now rejects lower-camel receiver names for external-pack static-owner fallback, preventing inherited fields such as `progressMeter` from synthesizing static facade methods.
- Try-with-resources now contributes an implicit used `close/0` call for support discovery, so generated cleanup calls on external resources get matching unit-returning facade methods.
- DISQ `HtsjdkReadsRddStorage.referenceSourcePath` now has typed nullable `String` argument facts, and external nullable argument slots no longer apply borrowed aliasing to the optional container.
- HTSJDK `SamReaderFactory.referenceSequence` now has typed nullable `Path` argument facts, and the Spark/DISQ `ReadsSparkSource` branch emits `None`/`Some(reference_path_specifier.to_path())` instead of a bare `Path`.
- DISQ `HtsjdkReadsRddStorage.read` now has explicit arity-specific facts for `read/1` and `read/2`; support and call lowering agree on `read_1`/`read_2`.
- Pack-owned self-returning fluent facade methods now return owned `Self` through `self.clone()`, allowing Java-style chained builder/factory calls to be assigned to owned variables in generated Rust.
- Reused optional wrapping now respects borrowed optional source slots, so `T` passed to `Option<&T>` is rendered as `Some(&value)` instead of cloning into `Option<T>`.
- Ordinary statement `if` lowering now threads null refinements into branch blocks and string-concat/display segments, so non-null branches unwrap optional references before display instead of calling `to_string()` on `Option<T>`.
- Stream planning now recognizes `parallelStream()` and the common `reduce(identity, accumulator)` terminal, and stream lambda receiverless calls can use source signature overload names and source parameter slot shapes. The Spark/DISQ `VariantsSparkSource` stream path now emits `vcfs.iter().map(... self.get_parallel_variants_string_list(vcf.clone(), intervals.clone())).fold(... result.union(rdd))`; `JavaRDD.union` is covered by the Spark no-op facade.
- Assignment-condition loops now lower their body using the concrete borrowed shape of the emitted non-null shadow local. The Spark/DISQ `ReferenceFileSparkSource` loop now calls `seq.get_name()`, `seq.get_bases()`, and `seq.length()` directly instead of trying to unwrap `&ReferenceSequence` as if it were still optional.
- Method usage facts are now also keyed by resolved Rust overload name, so signature propagation can target exact overloaded callees. This fixes nullable wrapper calls such as `writeReads(..., null, ...)` and `writeVariants(..., null, ...)` where the generated callee parameter must be optional.
- External constructor expressions used in typed slots now apply the normal slot conversion plan before returning. This fixes borrowed helper calls such as `ReadsSparkSource.checkCramReference(ctx, new GATKPath(...), ...)` by rendering a borrowed temporary.
- Current-instance field assignment and null-comparison lowering now fall back to the current type's planned field shape. This fixes `this.referencePath`-style optional fields whose emitted Rust storage is `Option<T>` even when the declared Java field type is non-nullable.
- Planned source method-call return shapes now take precedence over generic optional wrapping when lowering into optional slots, so calls to source helpers whose bodies can return `null` are not wrapped as `Some(helper(...)?)`.
- HTSJDK `FileExtensions.FASTA` is modeled as a typed string-sequence constant in external packs, and enhanced-for over static string-sequence fields lowers through `.iter().copied()` so loop variables are borrowed `&str` values.
- JDK `Math.min` now lowers through the native static-call planner as `std::cmp::min`, matching existing `Math.max` behavior.
- HTSJDK `SAMSequenceDictionary.getSequence` now has an explicit `SAMSequenceRecord` return fact, removing the weak unit-returning facade surface in the Spark/DISQ slice.
- HTSJDK `SAMSequenceRecord.getSequenceLength` now has an explicit `int` return fact.
- Broad `TwoBitReference.getReferenceBases` now returns `ReferenceSequence` in the facade facts used by the Spark/DISQ slice.
- Broad `ReadsPathDataSource` now declares `ReadsDataSource` implementation/conversion facts and a two-argument constructor surface.
- Broad `HeaderlessSAMRecordCoordinateComparator` now declares `Comparator` implementation/conversion facts, and explicit external adapter matching erases generic arguments such as `Comparator<SAMRecord>`.
- Exception constructor lowering stringifies message arguments at the `JavaException::new/with_cause` boundary, fixing `new RuntimeException(e)` style wrappers.
- Fallback switch rendering emits a statement-shaped block and appends `unreachable!()` for exhaustive return/throw switches, removing the observed tail-expression type conflict.
- Broad `SparkUtils.hadoopPathExists` now has an explicit `bool` facade surface.
- HTSJDK `SeekableStreamFactory.getInstance/getStreamFor` now has concrete facade surfaces instead of an unconstrained generic static factory.
- JDK `Files.exists` now has a non-throwing `bool` platform surface.
- Reused optional values in string concatenation/display segments now borrow with `as_ref().map(...)` when later uses would make a consuming `unwrap()` invalid, and refined optional method receivers now borrow with `as_ref().unwrap()` instead of moving out of the `Option`.
- Same-class source overload fallback now uses fact-aware varargs compatibility, including explicit `implements` and conversion facts, so expanded DISQ `WriteOption...` calls select the varargs helper rather than a same-arity fixed overload.
- Imported static facade support now consults builtin external-pack API facts, so `SAMRecordToGATKReadAdapter.headerlessReadAdapter` emits a concrete `GATKRead` return surface instead of an unconstrained `R: Default`.
- `VariantContextVariantAdapter.sparkVariantAdapter` now has explicit Broad/javalibs type, alias, capability, and return facts, so the observed adapter surface emits `GATKVariant`.
- `Objects::nonNull` method references used as callbacks now lower as closures, avoiding unconstrained generic function item inference.
- Direct Spark/DISQ `JavaRDD.map` callback lowering can now use typed element facts from `HtsjdkReadsRdd.getReads`/`HtsjdkVariantsRdd.getVariants`; the focused fixture emits typed parameters such as `|read: SAMRecord|`.
- Casted Spark `FlatMapFunction` callbacks now lower through the same typed callback path and render Java iterator callback inputs as concrete Rust iterator parameters such as `std::vec::IntoIter<VariantContext>`, removing the focused corpus `map_partitions_1` closure inference failure.
- Unbound accessor method references used as single-input callbacks now lower through the typed callback path. Scala tuple accessors at Spark boundaries use the erased tuple facade parameter for now, for example `|value: Tuple2| value._1()`, and opaque `Tuple2` support emits placeholder `_1()`/`_2()` methods.
- Typed null casts now survive unshaped expression lowering, so `(Void) null` can render as `None::<Void>` inside generic constructor calls such as `Tuple2::new(...)`.

Next verified idiom batch:

- GenomicsDB primary-only compiles in the focused package gate as of
  `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-broadcast-generic`.
- Picard program groups still compile after the GenomicsDB/Locatable changes.
- Broadly adding `org.broadinstitute.hellbender.engine` as source-summary context is not yet viable; it expands the generated support set and exposes hundreds of duplicate source-summary field/type symbols. Prefer targeted inherited member facts or a minimal superclass-summary mode first.
- A broad experiment that let support emission consider all builtin `javalibs` API facts filtered only by used method name/arity increased GenomicsDB errors to 182 in `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-builtin-api-support`; keep the narrower support-selection path and fix stream getter support by receiver/key instead.
- A later broad-ish experiment that backfilled all builtin methods for already-surfaced receivers still increased GenomicsDB errors to 62 in `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-stream-builtin-surfaces`; keep the getter-only path unless support replacement is made method-key aware.
- A current-turn experiment that made builtin-used facade support replace same-name/arity methods with receiver-shaped bodies increased GenomicsDB errors from 30 to 38 in `.tmp/corpus-rerun-gatk-genomicsdb-primary-after-builtin-facade-receiver-replace`. It converted several protobuf `new_builder()` static surfaces into instance methods while still not fixing `ProgressMeter`; the real fix needs staticness/receiver kind in shared support method identity, not broad replacement in `add_builtin_used_external_api_facts`.
- Spark/DISQ primary-only compiles in the focused package gate as of
  `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-broadcast-generic`.
  Full-context Spark/DISQ remains a timeout/scalability risk until rerun.

Latest Spark/DISQ primary-only top categories after `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-broadcast-generic`:

- Translator shape bugs: mismatched types are no longer dominated by same-name varargs/fixed overload selection or optional display move/reuse. Direct nullable wrapper overload failures, borrowed constructor helper arguments, optional `this.field` assignment wrapping, refined optional receiver borrowing, static string-slice iteration, `Math.min`, typed `SAMSequenceDictionary`/`SAMSequenceRecord` access, Broad concrete-to-interface conversions, exception message conversion, `Files.exists`, and the observed exhaustive fallback switch tail are fixed.
- Stream/native collection lowering: `List.parallelStream()`, stream-local same-class overload naming, captured owned-argument cloning, and `JavaRDD.union` facade support are fixed for the observed reduce path.
- Assignment-condition loop receiver refinement is fixed for the observed `ReferenceSequence` path.
- Remaining facade-surface issues are no longer `close`- or simple typed-return-dominated in this slice; `ReferenceSequenceFile.close`, local `ReadsDataSource.close`, `SparkUtils.hadoopPathExists`, `SeekableStreamFactory.getInstance/getStreamFor`, and `Files.exists` are covered.
- Constants and several no-op facade surfaces now resolve; the old `SAMRecord.NO_ALIGNMENT_START`, `ReadsWriteFormat.SINGLE/SHARDED`, `Path.getFileSystem`, `TwoBitReference.getReferenceBases`, `TraversalParameters.traverseUnmappedReads`, `HtsjdkReadsRddStorage.sbiIndexGranularity`, Broad read-helper owner, and `SortOrder.getComparatorInstance` categories are no longer top failures in `.tmp/corpus-rerun-gatk-spark-datasources-primary-after-used-fact-surfaces`.
- The previous source exception propagation mismatch in
  `VariantsSparkSource.getParallelVariantContexts` is fixed.
- Direct `JavaRDD.map` closure parameters can be typed from
  `HtsjdkReadsRdd.getReads`/`HtsjdkVariantsRdd.getVariants` facts, and
  `SAMRecordToGATKReadAdapter.headerlessReadAdapter` selects the known
  `GATKRead` return.
- The previous focused-slice boundary cast placeholders are clear. Future
  distributed/FFI handover shapes should still become explicit boundary handles
  or await native signatures from the future C/C++ transpiler instead of being
  mechanically translated as real Spark execution.
- Invalid patterns are down to 0 files in the focused Spark/DISQ slice.

Next Spark/DISQ implementation tasks:

1. Keep overload/signature selection cleanup under regression.
   - Direct nullable wrapper overload calls now mark exact callee parameters optional.
   - The observed same-name fixed/varargs `writeReads(..., splittingIndexGranularity, WriteOption...)` call now selects the varargs helper and packs trailing options with explicit conversion facts.
   - The observed borrowed write-option varargs and trait-delegation slot mismatches are fixed in the focused slice.

2. Keep optional borrowing adaptation under regression.
   - Calls to `Option<&GATKPath>` now emit `Some(&reference_path_specifier)` for the source varargs path.
   - Direct borrowed parameters now receive borrowed external constructor temporaries in the observed source-summary helper path, for example `ReadsSparkSource.checkCramReference(ctx, &GATKPath::new(...), ...)`.
   - `this.field` optional assignment wrapping is fixed for the observed `ReferenceFileSparkSource` path.
   - Reused optional display and refined optional method receiver borrowing are fixed for the observed `ReadsSparkSource.checkCramReference` path.

3. Continue stream/shared-plan work as new corpus cases expose it.
   - The observed `parallelStream().map(...).reduce(...)` path now lowers through shared stream planning, clones captured owned arguments, and has the needed `JavaRDD.union` no-op facade surface.
   - Direct Spark/DISQ `JavaRDD.map` callbacks now use shared receiver element-shape inference for typed lambda parameters in focused fixtures.
   - Next step: split "sequence-like for element inference" from "native sequence Rust representation" for Spark/DISQ facade handles, so `JavaRDD<T>` can inform callback slots without rendering method receivers or source-summary parameters as `Vec<T>`.

4. Keep distributed/FFI cast placeholders as explicit boundary-handle work.
   - The focused Spark/DISQ primary-only slice currently has 0 invalid-pattern files.
   - Spark/DISQ distributed execution should remain a handover/no-op facade until native or distributed runtime signatures are available; if a later corpus rerun exposes new downcast placeholders, model them as explicit boundary handles rather than translating distributed execution.

5. Continue typed facade return cleanup.
   - The latest observed simple typed-return gaps are fixed, including `TwoBitReference.getReferenceBases`, `SAMSequenceDictionary.getSequence`, `SAMSequenceRecord.getSequenceLength`, `SparkUtils.hadoopPathExists`, `SeekableStreamFactory.getInstance`, and `Files.exists`.
   - Remaining type-shape work is now more about source-summary return propagation, optional ownership reuse, and Spark callback boundary handles than one-off primitive/facade return facts.

## Why This Checkout

Fix this batch in the main checkout:

- Many gaps are split between `new_java2rust` planner/lowering code and
  `javalibs` facade or manifest facts.
- Full-context GATK runs expose source-summary scalability issues that are hard
  to isolate in per-category clones.
- Facade coverage, dependency emission, shared support assembly, and generated
  crate shape need to move together to avoid workers solving the same symptom in
  incompatible ways.

Use parallel workers for auditing and fixture design when useful, but land the
implementation in this checkout.

## Batch 1: Make Current Output Compile Farther

1. Add generated crate dependency emission for support crates.
   - Immediate failure: Picard emits `ndarray::Array2<_>` but generated
     `Cargo.toml` does not include `ndarray`.
   - Expected fix: assemble dependencies from emitted `RustType`/support facts,
     not from ad hoc corpus-specific rules.

2. Fix shared support discovery for source-summary-only needs.
   - Immediate failure: Picard needs `JavaClass<T>` for a source-summary stub
     field, but the assembled crate omitted the support item.
   - Expected fix: shared support assembly must scan generated stubs and module
     bodies for support-type uses, not only concrete translated expressions.

3. Handle recursive source-summary/support structs.
   - Immediate failures: `IntervalTreeNode` and `ContentDigestsKNOWNDIGESTS`
     are emitted as direct recursive value fields.
   - Expected fix: recursive fields should be boxed/optional where ownership is
     meaningful, or suppressed behind external/source-summary facade stubs when
     the body is not being translated.

## Batch 2: Expand `javalibs` Facade Coverage

Add or adjust facade facts for APIs repeatedly reported by GATK primary-only
compile checks.

GenomicsDB/protobuf-like builders:

- `VidMappingPB.getFieldsCount`
- `VidMappingPB.getFields`
- `VidMappingPB.toBuilder`
- `VidMappingPB.Builder.getFieldsBuilder`
- `GenomicsDBFieldInfo.Builder.setVCFFieldCombineOperation`
- `GenomicsDBFieldInfo.Builder.clearLength`
- `GenomicsDBFieldInfo.Builder.clearVcfDelimiter`
- `GenomicsDBFieldInfo.Builder.clearType`
- `GenomicsDBFieldInfo.Builder.addLength`
- `GenomicsDBFieldInfo.Builder.addVcfDelimiter`
- `GenomicsDBFieldInfo.Builder.addType`
- `GenomicsDBFieldInfo.Builder.setDisableRemapMissingWithNonRef`
- `FieldLengthDescriptorComponentPB.Builder.setVariableLengthDescriptor`
- `ExportConfiguration.Builder.setWorkspace`
- `ExportConfiguration.Builder.setArrayName`
- `ExportConfiguration.Builder.setGenerateArrayNameFromPartitionBounds`
- `JsonFormat.Parser.merge`

GATK/HTSJDK path and interval helpers:

- `GATKPath.toPath`
- `GATKPath.getRawInputString`
- `GATKPath.isCram`
- `GATKPath.isHadoopURL`
- `FeatureInput.toPath`
- `SimpleInterval.getContig`
- `SimpleInterval.getStart`
- `SimpleInterval.getEnd`
- `SAMRecord.getContig`
- `SAMRecord.getStart`
- `SAMRecord.getEnd`
- `SAMRecord.getReadUnmappedFlag`
- `ReferenceSequenceFile.close`
- `ReferenceSequenceFile.getSequenceDictionary`
- `ReferenceSequenceFile.getSubsequenceAt`
- `ReferenceSequence.getBases`
- `TwoBitReference.getSequenceDictionary`

Spark/DISQ no-op boundary helpers:

- `Broadcast.getValue`
- `JavaSparkContext.emptyRDD`
- `HtsjdkReadsRdd.getReads`
- `HtsjdkReadsRdd.getHeader`
- `HtsjdkReadsRddStorage.makeDefault`
- `HtsjdkReadsRddStorage.read`
- `HtsjdkReadsRddStorage.useNio`
- `HtsjdkReadsRddStorage.splitSize`
- `HtsjdkReadsRddStorage.validationStringency`
- `HtsjdkReadsRddStorage.referenceSourcePath`

Constants and options:

- `ReadConstants.DEFAULT_READ_VALIDATION_STRINGENCY`
- `ReadsWriteFormat.SINGLE`
- `ReadsWriteFormat.SHARDED`
- BAI write option types/constants
- SBI write option types/constants
- DISQ Tabix index write option types/constants
- missing `GATKVCFConstants` string-key fields used by GenomicsDB utilities

## Batch 3: Translator Shape Bugs

1. Fix `Option`/`Result` propagation.
   - Current failures include applying `?` to non-`Try` values and using `?` on
     `Option` inside `Result`-returning functions.
   - Expected fix: exception/nullability lowering needs explicit conversion
     plans, not opportunistic `?` insertion.

2. Fix generated module path resolution.
   - Current unresolved module examples: `g_a_t_k_genomics_d_b_utils`,
     `constants`, `progress_meter`, `interval_argument_collection`,
     `reference_two_bit_spark_source`, `read_utils`,
     `htsjdk_reads_rdd_storage`, `bai_write_option`, `sbi_write_option`.
   - Expected fix: source-summary and sibling module references should lower
     through one module naming plan shared by call/member lowering.

3. Fix duplicate support/nested type emission.
   - Current examples: `GenomicsDBImportInitializedQueryWrapper`,
     `GenomicsDBImportUnableToCreateGenomicsDBWorkspace`,
     `GenomicsDBVidMapProtoVidMappingPB`/`VidMappingPB`.
   - Expected fix: de-duplicate generated support aliases and nested classes by
     stable type key before emitting a crate.

4. Remove erased object default placeholders where a facade boundary is intended.
   - Current example: `ReferenceMultiSparkSource` emits
     `compile_error!("no default value is available for erased object slot")`.
   - Expected fix: facade ABI facts should provide boundary handle defaults or
     constructor plans when Java stores an abstract/distributed source.

5. Reduce runtime downcasts at Spark/DISQ boundaries.
   - Current examples include `Broadcast.getValue().downcast::<Object>()`,
     `DEFAULT_READ_VALIDATION_STRINGENCY.downcast::<ValidationStringency>()`,
     adapter `headerlessReadAdapter(...).downcast::<GATKRead>()`, and
     Spark function-object casts.
   - Expected fix: typed facade return/argument facts should select precise
     slots before representation fallback inserts `Any`.

6. Improve switch lowering for Spark sink code.
   - Current signal: `ReadsSparkSink.java` still produces synthetic switch
     state-machine scaffolding.
   - Expected fix: enum/string switch lowering should produce direct `match`
     expressions for the common return/assignment shapes.

## Batch 4: Full-Context Performance

Full-context GATK runs timed out even when individual files finish quickly with
primary-only context. Investigate and fix before treating corpus status as
representative.

Steps:

1. Add timing around context-source-summary loading, fact creation, and overload
   planning for `--context-dir-package` roots.
2. Cache or pre-index source summaries shared across parallel file translations.
3. Avoid scanning all GATK/HTSJDK/Picard summaries for every unresolved simple
   name when package/import facts already narrow the search.
4. Re-run:
   - GenomicsDB full context.
   - Spark/DISQ datasources full context.
   - Picard program groups full context.

## Verification Gates

Run after each batch:

```bash
cargo check --quiet
cargo test --no-run
cargo test --test idiom_gaps corpus_picard -- --nocapture
cargo test --test idiom_gaps spark -- --nocapture
cargo test --test idiom_gaps disq -- --nocapture
timeout 240 scripts/translate-focused-package.sh --skip-resolver-bench --primary /home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/tools/genomicsdb --out .tmp/corpus-rerun-gatk-genomicsdb-primary-only --jobs 4
timeout 240 scripts/translate-focused-package.sh --skip-resolver-bench --primary /home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/engine/spark/datasources --out .tmp/corpus-rerun-gatk-spark-datasources-primary-only --jobs 4
```

Full-context gates become mandatory after Batch 4:

```bash
timeout 240 scripts/translate-focused-package.sh --primary /home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/tools/genomicsdb --out .tmp/corpus-rerun-gatk-genomicsdb --jobs 4 --source-root-package source-summary:/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java:org.broadinstitute.hellbender --source-root-package source-summary:/home/mahogny/github/claude/java2rust/testdata/htsjdk/src/main/java:htsjdk --source-root-package source-summary:/home/mahogny/github/claude/java2rust/testdata/picard/src/main/java:picard
timeout 240 scripts/translate-focused-package.sh --primary /home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/engine/spark/datasources --out .tmp/corpus-rerun-gatk-spark-datasources --jobs 4 --source-root-package source-summary:/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java:org.broadinstitute.hellbender --source-root-package source-summary:/home/mahogny/github/claude/java2rust/testdata/htsjdk/src/main/java:htsjdk --source-root-package source-summary:/home/mahogny/github/claude/java2rust/testdata/picard/src/main/java:picard
```
