# GATK Non-Facade Translation Audit

Date: 2026-07-21

This audit uses local GATK and Picard source from
`/home/mahogny/github/claude/java2rust/testdata` and the current javalibs
facade manifests. It focuses on libraries previously classified as
translate/source-summary/no-op rather than Rust facades.

## Corpus Probes

| Slice | Result | Signal |
|---|---|---|
| `gatk-genomicsdb` | Partial in the first focused run. `GenomicsDBArgumentCollection`, `GenomicsDBConstants`, and `GenomicsDBOptions` translated. `GATKGenomicsDBUtils`, `GenomicsDBImport`, and `SampleNameMap` did not finish in the initial worker window. | Simple option/constant containers are tractable. GenomicsDB orchestration combines native APIs, protobuf builders, HTSJDK readers, Barclay annotations, GATK tool inheritance, and filesystem/cloud utilities. |
| `picard-programgroups` | Translated 11/11 files. Cargo check was blocked by sandbox dependency fetch for `bstr` through `htsjdk-facade`, not by generated syntax. | Picard program group classes are good source-summary/co-translate candidates. Current weak spot is typed external/source constants if context is missing. |
| `gatk-disq-spark-datasources` | Separate focused corpus for DISQ/Spark datasource code. | DISQ should remain outside javalibs. Spark is a no-op boundary for now, so this corpus should expose unsupported distributed-runtime calls rather than create runnable facade promises. |

## 2026-07-21 Focused Runs

All runs used `scripts/translate-focused-package.sh --jobs 4` and local GATK
or Picard source-summary roots. GenomicsDB and DISQ/Spark were bounded with a
240 second timeout so slow files do not hide per-file progress.

| Corpus | Output | Files translated | Gate status | Main signal |
|---|---|---:|---|---|
| `picard-programgroups` | `.tmp/corpus-picard-programgroups` | 11/11 | Translation complete; cargo check failed on sandboxed dependency fetch for `bstr` through `htsjdk-facade`. | 20 `invalid_java_cast_downcast` occurrences across 10 files, mostly Picard `HelpConstants` string constants without typed source context in the focused crate. |
| `gatk-genomicsdb` | `.tmp/corpus-gatk-genomicsdb` | 3/6 before timeout | Timed out. | Finished `GenomicsDBArgumentCollection`, `GenomicsDBConstants`, and `GenomicsDBOptions`; pending files were `GATKGenomicsDBUtils`, `GenomicsDBImport`, and `SampleNameMap`. |
| `gatk-disq-spark-datasources` | `.tmp/corpus-gatk-disq-spark-datasources` | 8/10 before timeout | Timed out. | Finished read/reference datasource classes. Pending files were `VariantsSparkSink` and `VariantsSparkSource`. Diagnostics include unresolved DISQ APIs (`HtsjdkReadsRdd.getReads`, `HtsjdkReadsRdd.getHeader`) and HTSJDK option gaps. |

## Biggest Remaining Tasks

| Priority | Area | Why It Matters | Corpus |
|---:|---|---|---|
| 1 | Source-summary package closure for GATK/Picard-owned classes | GATK imports many Picard classes. These should be translated or summarized, not added to javalibs. Missing context causes opaque shells and `Any`/downcast-style weak typing. | `picard-programgroups` |
| 2 | Protobuf-style nested builders | GenomicsDB utilities mutate nested protobuf builders (`toBuilder`, `getFieldsBuilder`, `build`). Current lowering can produce duplicate nested `Builder` Rust names and erased fallback builder types. | `gatk-genomicsdb` |
| 3 | GenomicsDB native boundary policy | `org.genomicsdb` should not go into javalibs per current decision, but GATK code calls it directly. The translator needs an explicit unsupported/native-boundary policy or a generated local shim boundary, not accidental opaque API guesses. | `gatk-genomicsdb` |
| 4 | Java map/null/URI ownership in data loaders | `SampleNameMap`-style code is common and output-affecting. Current output is close but clone-heavy and relies on optional unwrap recovery. | `gatk-genomicsdb` |
| 5 | DISQ/Spark distributed boundary | DISQ should not go into javalibs. Spark is no-op facade only. DISQ calls should report clear missing boundary coverage rather than silently promising runnable distributed behavior. | `gatk-disq-spark-datasources` |

## Corpus-Derived Fixtures

The minimized fixtures live in `examples/idiom-gaps` and are asserted from
`tests/idiom_gaps.rs` with the `corpus_` prefix. They are not replacements for
the real corpus gates; they isolate the repeated translator problems exposed by
the corpus probes.

| Fixture | Source pressure | Current test signal |
|---|---|---|
| `SourceSummaryConstantAcrossPackage.java` | Picard program groups reading `picard.util.help.HelpConstants` string constants. | Without context, constants still downcast from `Any`; with source-summary context, typed constants lower to `&'static str` plus `.to_string()`. |
| `FileLoadConstructorDelegation.java` | `SampleNameMap` constructors, file loading, split-with-limit, nested try/catch, `TreeMap.put`, nullable duplicate checks. | Constructor delegation lowers, but catch selection records the current exception-type gap where an outer `IOException` catch is emitted as `URISyntaxException`. |
| `QualifiedNestedBuilderChain.java` | GenomicsDB/protobuf `toBuilder`, nested `Builder`, `getFieldsBuilder`, fluent field mutation, `build`. | Builder facts are inferred, but nested `Builder` declarations still collide on simple Rust names. |
| `SparkRddVariantPipeline.java` | `VariantsSparkSink.sortVariants`: `mapToPair`, `Tuple2`, `sortByKey`, method reference `Tuple2::_1`, nullable comparator. | Lambda/method-reference shapes lower, but comparator nullability collapses to `if false` and RDD APIs remain opaque shells. |
| `DisqExternalBoundaryChain.java` | DISQ `HtsjdkVariantsRddStorage.makeDefault(...).read(...).getVariants()` and write option chain. | Chained external calls render as guessed APIs and trigger `recursion_tco_fallback`; this should become explicit boundary diagnostics/facts. |
| `FfiNativeHandoverCall.java` | Native runtime call chain with `openArray`, `query`, `count`, and cleanup. | `ffi-boundary` manifest facts shape the external call surface, but `try/finally` cleanup still blocks a clean handover wrapper. |
| `FfiOwnedHandleTransfer.java` | Passing an FFI-owned handle across a consumer boundary and using it afterward. | FFI-owned handle capability emits `signature_owned_recovery_requires_ffi_transfer`; ownership transfer must be explicit. |

## Translator Plan

1. Source-summary closure for source-owned dependencies:
   include package-qualified static-final field facts from summary roots before
   unresolved external fallback facts are synthesized. Re-run
   `picard-programgroups`; success target is zero `invalid_java_cast_downcast`
   for `HelpConstants` constants.

2. Constructor and exception planning for `SampleNameMap`:
   fix nested try/catch exception target selection, keep `this(...)`
   constructor delegation as an initialized `__java_self`, and retain
   split-with-limit plus `TreeMap`/`SortedMap` map shape. Re-run the individual
   `SampleNameMap.java` translation before the full `gatk-genomicsdb` slice.

3. Nested type identity and builder facts:
   preserve qualified nested owner identity in Rust support names
   (`VidMappingBuilder` vs `FieldInfoBuilder`) and avoid emitting duplicate
   top-level `Builder` structs. Use callable keys for `toBuilder`,
   `getFieldsBuilder`, mutating builder setters, and `build`.

4. Spark RDD syntax planning:
   keep JavaRDD/JavaPairRDD as boundary types, but lower lambda and method
   reference call shapes consistently enough to compile against the no-op Spark
   facade. Fix nullable external return modeling for `getVCFRecordComparator`
   so `comparator == null` does not collapse to `if false`.

5. FFI handover boundary policy:
   use `ffi-boundary` manifests for native packages whose implementation will
   be handled by the future C/C++ transpiler or handwritten bindings. The Java
   translator should emit stable handover calls, handle types, ownership facts,
   and cleanup/error-shape metadata rather than model the native implementation.

6. DISQ/Spark boundary policy:
   split native/file-format handover from distributed execution. DISQ calls
   that ultimately cross Hadoop-BAM/native file-format code can become explicit
   handover boundaries. Spark RDD scheduling, partitioning, shuffle, executor
   serialization, and cluster IO remain runtime semantics and need a Spark
   facade/runtime contract, not C/C++ FFI handover alone.

7. Re-run corpus gates in increasing difficulty:
   `picard-programgroups`, single-file `SampleNameMap`, `gatk-genomicsdb`,
   `gatk-disq-spark-datasources`. Keep the 240 second timeout for package gates
   until the currently pending files finish.

## Mechanical Translation Boundaries

The future C/C++ transpiler changes the GenomicsDB evaluation. The Java
translator does not need to implement GenomicsDB native storage/query behavior;
it needs to lower Java calls into a stable FFI handover state with explicit
handle ownership, argument/result slots, cleanup calls, and exception/error
metadata.

DISQ/Spark changes only partially. DISQ's Hadoop-BAM/file-format edges can be
represented as handover calls when they point at native or external runtime
code. Spark distributed execution itself is still not mechanically translatable
as FFI: the semantics include scheduling, partitioning, shuffles, closure
serialization, executor lifetime, retries, and remote filesystem behavior. That
needs an explicit Spark runtime/no-op facade contract.
