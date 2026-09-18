# Claude audits

Validated defects and structural findings for the Java→Rust translator.
One file per issue, written so another agent can pick it up without re-deriving
the evidence.

Each file states: symptom, minimal repro, **verified** evidence (rustc output or
runtime output), root cause with `file:symbol` anchors, whether it needs a local
fix or new machinery, and a proposed direction.

Java sources for every repro are in `claude_audits/repro/`.

## Convention: completed issues move to `done/`

When an issue is fixed, its file is **moved to `claude_audits/done/`** — it is not
deleted. If a numbered file is missing from this directory, look in `done/`
before concluding anything is lost, and **do not recreate it**.

Issue index:

| # | issue |
|---|---|
| [0012](done/0012-unsupported-diagnostics-emit-invalid-rust.md) | `compile_error!` emitted invalid Rust, voiding its file's other diagnostics |
| [0013](done/0013-nested-type-names-unmangled-in-downcast.md) | `downcast::<GuideTree.Node>` parse error |
| [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md) | move analysis was five name-keyed approximations of one relation |
| [0021](done/0021-instanceof-pattern-variable-not-bound.md) | `instanceof` pattern bindings dropped outside `if` conditions |
| [0022](done/0022-types-are-reparsed-from-source-text.md) | source types were re-parsed from text, hiding a dropped array dimension. **Scoped:** closed for the source route; four defensive `parse_type_text` fallbacks remain in `parser.rs` by design, measured at 0 hits |
| [0034](done/0034-object-array-allocation-produces-an-empty-vec.md) | `new T[n]` allocated length 0, so every index panicked |
| [0035](done/0035-idiom-gaps-suite-never-terminates.md) | the largest behavioural suite never returned, so it had no baseline |
| [0025](done/0025-interface-default-methods-are-discarded.md) | interface `default` methods: five defects, four sharing one nested-name-lookup cause |
| [0033](done/0033-receiver-mutability-not-propagated-through-lambdas.md) | a method mutating self only inside a lambda was inferred `&self` |
| [0036](done/0036-string-concat-type-inference-is-exponential.md) | type inference was exponential in `+` chain length; a dozen segments took minutes |
| [0037](done/0037-nested-class-outer-static-read-is-capture.md) | nested class reading an enclosing `static` lowered as an outer-*instance* capture |
| [0038](done/0038-two-dimensional-object-array-carrier-disagrees.md) | `String[][]` was declared, built, written and read as three different carriers |
| [0039](done/0039-static-array-fields-lose-their-value-and-their-writes.md) | `static int[] a = new int[3]` had length 0, and every `a[i] = v` was silently discarded |
| [0040](done/0040-external-type-name-matching-recomputes-on-the-hot-path.md) | External type-name matching rebuilt its alias lists per pack member — **81 s to translate 7 methods** | performance | fixed, memo |
| [0041](done/0041-slot-shape-design-had-no-generated-crate-checks.md) | `slot_shape_design` asserted on rendered Rust with **no** check that it compiled — 596 tests, 0 generated-crate checks | test coverage | 99 checked; 21 genuine failures and 31 harness-only ones enumerated |
| [0042](done/0042-collection-mutation-is-five-lists-that-disagree.md) | Collection mutability/recognition now routes through shared typed native-collection policy; static argument mutators feed `mutates_self`; table-driven lowering dispatch is optional cleanup |
| [0043](done/0043-rust-keyword-escaping-is-four-lists-and-two-strategies.md) | Rust-keyword escaping now has one shared policy; fields, static fields, method/constructor params, locals, enhanced-for vars, ordinary callback lambdas, stream pipeline lambda params, and stream block locals allocate emitted names from source identity |
| [0044](done/0044-external-receiver-owner-key-is-computed-twelve-times.md) | External-fact **owner key** disagreement fixed across field-style keys, high-risk API receiver policy, parsed/lowered method-reference targets, usage modeled-receiver boundaries, known-builtin keys, aliasing mutator detection, external static-call owner facts, qualified static-owner Java walkers, and `lower.rs` source-context/static-retarget owner candidates |
| [0046](done/0046-slot-shape-type-identity-is-a-hand-refreshed-cache.md) | `RustSlotShape.type_identity` was a hand-refreshed cache of a pure function of `java_ty`; fixed by deleting the field and deriving `type_identity()` on demand |
| [0047](done/0047-facade-slot-algebra-is-nine-hand-copied-recursions.md) | `FacadeSlot` borrow predicates, slot→shape mapping, Rust-type→slot parsing, and ABI/effect precedence are centralized/fixed |
| [0048](done/0048-erased-static-trait-body-is-rewritten-by-substring.md) | Erased-static companion trait bodies are structurally rewritten; rendered-string substring replacement removed |
| [0049](done/0049-erased-object-owner-loses-the-static-call-target.md) | Erased static calls preserve the resolved owner; helper names preserve the owner/method boundary |
| [0050](done/0050-builtin-pack-facts-and-name-aliases-rebuilt-per-call.md) | Built-in pack aggregate/method/type aliases and manifest list are cached; production callers borrow built-in facts/manifests or use narrower iterators; remaining clone-returning APIs are compatibility wrappers |
| [0051](done/0051-derived-fields-refreshed-at-some-mutation-sites.md) | Documented stale derived fields now refresh at mutation sites; hierarchy lookup index is tied to its `FactSet` |
| [0052](done/0052-the-example-corpora-are-asserted-on-but-never-compiled.md) | Corpus compile visibility is covered by `tests/corpus_compile.rs`; the checked baseline is down to one loud invalid-Java failure, and the `idiom_gaps` rendered-Rust assertion surface is generated-crate-backed or expected-failure-backed |
| [0053](done/0053-the-gate-compares-counts-not-names.md) | Gate now compares failure names for every red target; missing metric signals fail loudly instead of becoming zero |
| [0054](done/0054-switch-arms-are-silently-deleted.md) | Switch parser now preserves arrow statement rules, comma labels, shared yield labels, and unsupported block/throw expression arms instead of silently deleting them |
| [0055](done/0055-no-java-syntax-error-is-ever-reported.md) | Java syntax errors now emit parser diagnostics, single-file translation exits non-zero on `Severity::Error`, and batch `translate_status` is derived from diagnostic severity |
| [0057](0057-exceptions-are-panics-and-finally-is-a-closure.md) | Focused exception/finally actual-error scope is covered by generated-crate and Java-oracle gates; general and nested `finally` completion plus remaining panic paths are still open |
| [0056](0056-null-is-planned-away-not-represented.md) | Partially mitigated: reference-typed null comparisons without nullable representation now emit `compile_error!`, concrete named concat no longer uses the erased-object string stub, source `toString()`, plain `Object.toString()`, and basic platform carriers including `java.io.File`, `java.nio.file.Path`, `java.net.URI`, `java.net.URL`, `java.util.BitSet`, `java.util.Locale`, `java.util.Random`, `java.util.StringTokenizer`, `java.util.jar.JarEntry`, `java.util.zip.ZipEntry`, `java.util.regex.Pattern`, `java.util.concurrent.atomic.AtomicInteger`, `java.util.concurrent.atomic.AtomicLong`, `java.util.concurrent.atomic.AtomicBoolean`, `StringBuilder`, `System.in`/`InputStream`, and class-literal `JavaClass<T>` tokens can be recovered through present erased-object display, basic null locals/default fields/source-object returns are differential-gated, nullable constructor call results propagate into stored fields, nullable `List`, map values written through `put(k, null)`, and reference-array elements are represented, null-bearing `String.join` literal collections and varargs use Java display policy, null `String.join` delimiters are catchable, source-class-bound trait forwarding preserves nullable implementation returns, nullable `String.valueOf(Object)` uses optional-erased display, `Objects.nonNull`/`Objects.isNull` predicate Rust `Option` carriers directly, and null optional/source-parameter/argument-slot plus several structured or parseable-raw non-argument coercions can raise catchable `NullPointerException`. Nullable list-element method receivers, nullable array `.length`, nullable array writes in propagating `try/catch`, primitive `Map.get` arithmetic operands, optional `OutputStream.write(byte[])` and fully-qualified `ByteArrayOutputStream` receivers, and specialized `entrySet().iterator().next().getKey()` map receivers now remain catchable after projection/unboxing. Still open for broader non-argument catchable null coercions and unknown external erased display | **silent -> loud/catchable/correct for several paths** | Java `true,x,null,2` now matches for reference arrays; `NullCtorArg`, `NullInList`, `NullStringArray`, `ListElementReceiverNullCatch`, `NullableArrayLengthCatch`, `MapGetArithmeticNullCatch`, `MapPutNullValue`, `StringValueOfNullObject`, `StringJoinNullDelimiter`, `StringJoinNullElement`, `StringJoinVarargsNullElement`, `ObjectToStringConcat`, `InheritedObjectToStringConcat`, `ObjectToStringFallback`, `NullArgCatch`, `NullCallArgCatch`, `ErasedBoxedPrimitiveDisplay`, `ErasedBitSetDisplay`, `ErasedClassDisplay`, `ErasedFileDisplay`, `ErasedHashMapDisplay`, `ErasedHashSetDisplay`, `ErasedLocaleDisplay`, `ErasedPathDisplay`, `ErasedInputStreamDisplay`, `ErasedJarEntryDisplay`, `ErasedPatternDisplay`, `ErasedPriorityQueueDisplay`, `ErasedRandomDisplay`, `ErasedStringBuilderDisplay`, `ErasedStringTokenizerDisplay`, `ErasedZipEntryDisplay`, `ErasedUriDisplay`, `ErasedUrlDisplay`, `ErasedAtomicIntegerDisplay`, `ErasedAtomicLongDisplay`, and `ErasedAtomicBooleanDisplay` are promoted; focused source-bound trait, display, output-stream, byte-array-output-stream, raw catchable-null expression, Objects null-predicate, nullable array write, map literal-null value, and map entry-set receiver tests pin nullable propagation |
| [0058](0058-class-inheritance-upcast-discards-the-object.md) | Top-level and nested source-supertype locals, explicit source-subtype casts, optional source-supertype present returns, erased-object casts to source supertypes, `ArrayList<Super>.add(new Sub(...))`/`get()` collection-element flows, used `Object instanceof Super s` pattern bindings, and erased-object source-supertype identity recovery now use the source hierarchy carrier; `ErasedCarrierIdentity` is promoted and matches Java via variant-handle identity; explicit exception self-casts and unsupported source-supertype views remain loud guardrails; plain boolean erased-object `instanceof` and unused pattern-bindings over a known source supertype expand across known subtype runtime targets; direct emitted-source `super.m()` calls lower to the superclass implementation, and the checked mutating `super.m()` fixture set uses superclass-temp copyback where safe; real class hierarchy representation is still incomplete | **partly fixed, design open** | remaining work is broader class-hierarchy representation |
| [0059](0059-concatenation-erases-reference-values-and-collections-lose-their-contract.md) | Concrete concat/display and collection contract repros are mitigated; declared/inherited source `toString()`, plain `Object.toString()`, basic platform erased carriers (`String`, `Boolean`, boxed primitive/`Character`, `StringBuilder`/`StringBuffer`, `JavaClass<T>` class-literal tokens, `java.io.File`, `java.io.InputStream`, `java.nio.file.Path`, `java.net.URI`, `java.net.URL`, `java.util.Random`, `java.util.StringTokenizer`, `java.util.jar.JarEntry`, `java.util.zip.ZipEntry`, `java.util.regex.Pattern`, `java.util.concurrent.atomic.AtomicInteger`, `java.util.concurrent.atomic.AtomicLong`, and `java.util.concurrent.atomic.AtomicBoolean` values), typed list/map display, modeled external `toString(): String` facts, decomposable `SimpleEntry`/`SimpleImmutableEntry` display, direct and aliased entry-set iterator display, typed `Map.Entry` field/accessor values, typed `Map.Entry` parameter and detached `entrySet().iterator().next()` locals in `entrySet().contains/remove`, expression-valued list/set `addAll` and `removeAll` return booleans, `Map.values().remove` mutable-borrowed backing maps, `Map.keySet().add/addAll` and `Map.entrySet().add/addAll` unsupported live-view mutation, `HashSet<>(map.entrySet())` constructor recovery, borrowed collection elements into primitive slots, source enum `values()[i]`/array storage, and private `Map`-interface parameters with unambiguous ordered-map call-site carriers are recovered. Unknown external erased-display residue remains loud for values without source/platform downcast evidence or an explicit display-capability fact; arbitrary opaque `Map.Entry` values without extractable key/value still need a real entry-value model | **mostly silent -> loud/correct; model residue open** | `Generics`, `ObjectToStringConcat`, `InheritedObjectToStringConcat`, `ObjectToStringFallback`, `StringValueOfObject`, `ErasedPlatformDisplay`, `ErasedBoxedPrimitiveDisplay`, `ErasedClassDisplay`, `ErasedMapEntryBooleanDisplay`, `ErasedMapEntryDisplay`, `ErasedNullableArrayListDisplay`, `ErasedNullableBooleanArrayListDisplay`, `ErasedNullableByteArrayListDisplay`, `ErasedNullableCharacterArrayListDisplay`, `ErasedNullableDoubleArrayListDisplay`, `ErasedNullableFloatArrayListDisplay`, `ErasedNullableIntegerArrayListDisplay`, `ErasedNullableLongArrayListDisplay`, `ErasedNullableShortArrayListDisplay`, `ErasedSimpleImmutableEntryDisplay`, `ErasedStringBuilderDisplay`, `ErasedStringTokenizerDisplay`, `ErasedJarEntryDisplay`, `ErasedZipEntryDisplay`, `ErasedFileDisplay`, `ErasedUriDisplay`, `ErasedRandomDisplay`, `ErasedUrlDisplay`, `ErasedPatternDisplay`, `ErasedAtomicIntegerDisplay`, `ErasedAtomicLongDisplay`, `ErasedAtomicBooleanDisplay`, `EnumValuesElementIntoObject`, `BorrowedElementIntoPrimitiveLocal`, and `ListDisplay`, `PriorityQueueHeadOrder`, `PriorityQueueOfferHeadOrder`, `PriorityQueuePollBooleanConcat`, `PriorityQueuePollIntegerConcat`, and `PriorityQueuePollDoubleConcat` are promoted; bounded `ArrayBlockingQueue.add`/`offer` are promoted with an exact-capacity `JavaArrayBlockingQueue<T>` carrier, named and inline anonymous/lambda priority-queue comparators, including simple subtraction and `Integer.compare` bodies in `PriorityQueueSubtractionComparatorConstructor`/`PriorityQueueStaticCompareComparatorConstructor`/`PriorityQueueLambdaComparatorConstructor`, plus nonescaping `Comparator<T>` method parameters consumed by `new PriorityQueue<>(comparator)`, are promoted with a `JavaPriorityQueue<T>` carrier; stored/returned functional-interface lambdas use callback-backed facades, while arbitrary opaque comparator objects still need concrete modeling evidence; ordered collection fixtures including `ArrayDequeOffer`, `QueuePollNull`, `QueuePollAliases`, `LinkedListQueueHeadCatch`, `LinkedHashMapReinsertOrder`, sorted-map endpoint/navigation keys, `MapKeySetRemove`, `MapKeySetClear`, `MapKeySetAddUnsupported`, `MapKeySetAddAllUnsupported`, `MapKeySetBulkContract`, `MapValuesRemove`, `MapValuesBulkContract`, `MapEntrySetClear`, `MapEntrySetRemove`, `MapEntrySetAddUnsupported`, `MapEntrySetAddAllUnsupported`, `MapEntrySetBulkContract`, `MapEntrySetContainsSimpleEntry`, `MapEntrySetRemoveSimpleEntry`, `MapEntrySetContainsEntryLocal`, `MapEntrySetRemoveEntryLocal`, `MapEntrySetContainsNextLocal`, `MapEntrySetRemoveNextLocal`, `MapEntryDetachedContract`, `MapEntrySetEnhancedForSetValue`, `MapEntrySetContainsNext`, `MapEntrySetContainsParam`, `MapEntrySetRemoveParam`, `MapEntryDisplay`, `MapEntryNextDisplay`, `MapEntryFieldDisplay`, `MapEntrySimpleImmutableEntry`, and `CollectionAddAllReturn` are promoted; focused map-view contract oracle, queue-poll oracle, TreeMap concat, map-entry view, aliased entry-set iteration, addAll/removeAll/containsAll/retainAll ABI/argument-boundary, removeAll self-alias, map-entry constructor, and internal Map carrier tests check native collection behavior |
| [0060](done/0060-catch-and-finally-bodies-are-invisible-to-the-analysis-phases.md) | Usage and signature local-plan walkers dropped `catch`/`finally` bodies; fixed for receiver mutability and catch/finally local plans | fixed | focused regressions plus nearby try/catch/finally checks pass |
| [0061](done/0061-a-local-binding-has-no-identity-so-the-plan-is-rejoined-by-name-four-times.md) | **Closed for ordinary source bindings.** Resolver-bound declarations carry `BindingId`; the remaining parser-only/synthetic adapters are explicitly named and fail closed on ambiguity. | fixed/fenced | preserve the adapter boundary rather than reintroducing name lookup |
| [0062](done/0062-dead-unreachable-and-never-connected-code.md) | Rechecked 2026-09-06 and moved back to `done/`; historical cleanup removed accidental zero-caller wrappers, production `#[allow(dead_code)]`, and `shape_projection` echo fields; the regrown wrapper/zero-caller census is clean after deleting `builtin_external_library_packs()`, default `receiver_constraints()`, ownership-only `project_optional_element_borrow_carrier()`, default `solve_signatures()`, no-facts hierarchy/object-model wrappers, unused storage accessors, unused shared-handle path helper, unused rendered projection test helpers, and the inlined exception-message helper | done | keep generated runtime-support template functions excluded from zero-caller scans |
| [0063](done/0063-test-facing-dead-weight.md) | Fixed for the rechecked source-layout scope; descriptor-only inventory is gone, the conservative `slot_shape_design.rs` source-only scanner reports `candidate_count 0`, and moved guards pass in `tests/layering.rs` | done | keep source-layout guards in `tests/layering.rs` |
| [0064](done/0064-the-java-to-rust-name-map-is-a-pure-non-injective-function.md) | **Closed for source-owner identity.** Source usage, signature, field, callable, call-graph, context, and method-reference plans retain opaque package-aware `JavaOwnerKey` values; external facts are intentionally spelling-keyed and ambiguity-refusing adapters. | fixed/fenced | preserve explicit source/external boundary |
| [0065](done/0065-the-overload-suffix-is-scoped-to-one-file-and-one-level-of-inheritance.md) | Reopened scopes rechecked 2026-09-06: cross-file, interface-default, transitive-superclass, and subclass-added-overload cases are focused-test covered; stale string-signature expectations were corrected | done | keep overload tests value/compile backed, not spelling-only |
| [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md) | Compile-error counts were blind to silent wrong output and could reward erasure; fixed with marker/counter ownership, differential tests, a terminating idiom baseline, shared-support sweep coverage, and no-op facade call-site accounting |
| [0008](done/0008-inference-machinery-does-not-reach-lowering.md) | Inference stayed upstream while lowering re-derived facts locally; fixed for the audited scope by routing solved relations through `CallableAnalysis`, consolidating slot-shape specificity, landing storage strategy as a whole-program relation, and replacing drifted hardcoded library/enumeration tables with rule-backed queries |
| [0009](done/0009-overload-identity-collapses-onto-rust-name.md) | Overload identity collapse fixed: source method identity now uses erased Java signature/staticness across inherited suppression, summary surfaces, exception override equalization, interface candidates, and usage/call planning |

0059 fixture note: the bounded `ArrayBlockingQueue` promotion includes
`ArrayBlockingQueueAdd`, `ArrayBlockingQueueOffer`,
`ArrayBlockingQueueBadCapacity`, and `ArrayBlockingQueueNull`.

Issues 0012 and 0013 were the parse-error pair in Tier 0 of
[SUGGESTIONS.md](SUGGESTIONS.md), so that step is complete: corpus error counts
are no longer being voided by files that fail to parse. Worth re-running the
corpora to see what those two files were hiding.

## How to reproduce any of these

```sh
cargo build --release
./target/release/new-java2rust claude_audits/repro/<Name>.java > /tmp/out.rs
rustc --edition 2021 --crate-type lib --crate-name probe --emit=metadata --out-dir /tmp /tmp/out.rs
```

For the runtime-divergence issues, append a `fn main()` to `/tmp/out.rs` and
build it as a binary instead.

**Concurrency scope: [../CONCURRENCY-SCOPE.md](../CONCURRENCY-SCOPE.md)** — what
"concurrency is in scope" costs, and the representation decision it forces
*before* 0002 is implemented.

**Coverage map: [../SEMANTIC-COVERAGE.md](../SEMANTIC-COVERAGE.md)** — what has
and has not been checked, indexed by JLS chapter. Short version: these findings
cover roughly a third of the semantic surface; concurrency and arrays are
near-zero.

**What is left to audit: [../AUDIT-BACKLOG.md](../AUDIT-BACKLOG.md)** —
prioritised remaining territory, including an *exhaustive* grammar-coverage diff.

**Start with [SUGGESTIONS.md](SUGGESTIONS.md)** — prioritised suggestions for
handling the idioms we currently fail, grounded in corpus error counts.

## Confirmed defects

| # | Issue | Evidence | Fix size |
|---|---|---|---|
| [0001](done/0001-loop-back-edge-invisible-to-move-analysis.md) | Loop back edges invisible to move analysis → `E0382` | rustc | needs machinery (0006) |
| [0002](0002-object-aliasing-has-no-representation.md) | Mutable Java aliasing has implemented `SharedImmutable`/`Rc<T>`, `SharedLocal`/`Rc<RefCell<T>>`, and base `SharedSync`/`Arc<Mutex<T>>` paths, including captured inners, mutating shared-trait forwarding, and object-backed anonymous-interface slices. Broader anonymous objects and borrowed-return trait surfaces remain | mixed: aliasing and capture Java/Rust oracles pass | finish general anonymous-object and borrowed-return representation |
| [0002b](0002b-shared-representation-design.md) | **Design, partly implemented**: all three base storage tiers, 0028 identity lowering, and mutating shared-trait forwarding are landed; broad anonymous objects and borrowed-return trait surfaces remain | — | design-sized remaining tiers |
| [0003](done/0003-java-integer-arithmetic-semantics-not-preserved.md) | Integer overflow panics in debug, wraps in release; `/ 0` not a Java exception; shift distance unmasked | runtime | medium, self-contained |
| [0004](done/0004-string-concat-absorbs-nested-arithmetic.md) | `"t " + (a + b)` → `"t 12"` instead of `"t 3"` | runtime | **small — exact fix given** |
| [0005](done/0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md) | `>>>` parsed as `>>`; wrong for all negative operands | runtime | small, mechanical |
| [0009](done/0009-overload-identity-collapses-onto-rust-name.md) | Child's own method body **silently replaced** by an inherited overload; fixed by Java signature identity | corpus (FastQC) | done |
| [0010](done/0010-fallibility-is-decided-three-times-and-disagrees.md) | `?` emitted where neither callee nor caller returns `Result`; 214 errors | corpus (FastQC) | medium — fixpoint |
| [0011](done/0011-dyn-any-fallback-fails-at-every-use.md) | `Box<dyn Any>` fallback fails at every method call; 271 errors | corpus (FastQC) | medium — fallback shape |
| [0012](done/0012-unsupported-diagnostics-emit-invalid-rust.md) | `compile_error!` with unescaped quotes → file fails to **parse**, hiding all its other errors | corpus (biojava) | **tiny** |
| [0013](done/0013-nested-type-names-unmangled-in-downcast.md) | `downcast::<GuideTree.Node>` → parse error | corpus (biojava) | **tiny** |
| [0014](done/0014-labelled-statements-are-silently-deleted.md) | **Any labelled loop is deleted from the output**, no diagnostic anywhere | runtime | small — parser arm |
| [0015](done/0015-static-initializer-blocks-are-silently-dropped.md) | `static { … }` blocks dropped silently | runtime | medium |
| [0016](done/0016-core-jdk-facades-are-no-op-stubs.md) | `StringBuilder`/`FastMath`/`Pattern` are no-ops; 181 live call sites | runtime + corpus | medium |
| [0017](done/0017-generic-bounds-dropped-and-method-type-params-undeclared.md) | Generic bounds dropped; method type params never declared; ~36% of biojava | probe + corpus | large |
| [0019](done/0019-unhandled-expressions-emit-raw-java-source.md) | Unhandled expressions emitted as **raw Java source**; hits `switch` expressions | rustc | small |
| [0020](done/0020-text-block-content-not-processed.md) | Text block keeps incidental indentation → wrong string | runtime | small |
| [0023](done/0023-inner-class-construction-always-panics.md) | Non-static inner construction routes implemented shared outer handles, including static-reachable `Arc<Mutex<Outer>>`, through `_with_outer` companions | runtime | fixed |
| [0024](done/0024-numeric-limit-constants-stubbed-to-zero.md) | **`Integer.MAX_VALUE` is `0`** — breaks every min/max idiom; live in corpus | runtime + corpus | **tiny** |
| [0025](done/0025-interface-default-methods-are-discarded.md) | `default` method bodies dropped → runtime panic | runtime | small — Rust supports it natively |
| [0026](done/0026-assignment-evaluation-order-differs-from-java.md) | Rust evaluates assignment RHS before the target; Java is the reverse | runtime | small |
| [0027](done/0027-compound-assignment-implicit-narrowing-missing.md) | `byte b; b += 300;` → `E0308`; missing JLS 15.26.2 narrowing cast | rustc | **tiny** |
| [0028](done/0028-reference-equality-lowered-as-structural-equality.md) | `==` on classes was structural, not identity — **violated decision 2** | runtime | fixed: `Rc`/`Arc::ptr_eq`, two differential fixtures |
| [0029](done/0029-static-field-mutation-emits-invalid-rust.md) | `static n; n++;` → `E0067` invalid left-hand side | rustc | small |
| [0030](done/0030-java-statics-are-thread-local.md) | Java `static` fields are `thread_local!` — per-thread copies | inspection | split 0030a/b |
| [0031](done/0031-stubbed-constant-census.md) | **`Math.PI` is `0.0`**; full census of 27 stubbed constants across 3 corpora | corpus | **tiny for the fix** |
| [0032](done/0032-java-lang-math-is-mistranslated.md) | `Math.round(-2.5)` → `-3`, Java `-2`; plus 5 more `Math` defects. **Tested replacement code included** | rustc + runtime | medium, one focused pass |

## Structural findings

These explain why the defects above recur, and are the argument for what
machinery to add.

| # | Finding |
|---|---|
| [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md) | **Closed.** One backward liveness fixpoint over binding identity; no name-set approximation left on `LowerCtx` |
| [0007](0007-rust-ir-is-mostly-opaque-strings.md) | The audited raw-boundary cleanup has fenced historical bulk `Raw(format!(...))` producers and centralized core construction through `raw_leaf`; native producer coverage and the anti-regression lint remain open. |
| [0008](done/0008-inference-machinery-does-not-reach-lowering.md) | **Closed for the audited scope.** Lowering consumes solved relations through `CallableAnalysis`; slot-shape specificity is centralized; storage strategy is a whole-program lattice consumer; and the recorded hardcoded library/enumeration drift has rule-backed tests |
| [0022](done/0022-types-are-reparsed-from-source-text.md) | `parse_type_node` re-parsed types from text. **Closed for the source route**, not for the file: the ~4.9M remaining `parse_type_text` calls are all `javalibs`/manifest input (the correct use), and four defensive fallbacks stay in `parser.rs`, documented and measured at 0 hits over the corpus |
| [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md) | **Closed.** The compile metric is no longer the only quality signal: marker/counter ownership, fixture-backed detector tests, the differential harness, the `idiom_gaps` failure-name baseline, shared-support sweep inputs, and no-op facade call-site accounting are all gate-covered |

## Suggested order

The historical prioritization in [SUGGESTIONS.md](SUGGESTIONS.md) is useful as
background but names many issues that have since moved to `done/`. Current work
should start from the active rows in this file: [0002](0002-object-aliasing-has-no-representation.md),
[0002b](0002b-shared-representation-design.md),
[0056](0056-null-is-planned-away-not-represented.md),
[0057](0057-exceptions-are-panics-and-finally-is-a-closure.md),
[0058](0058-class-inheritance-upcast-discards-the-object.md),
[0059](0059-concatenation-erases-reference-values-and-collections-lose-their-contract.md),
[0061](done/0061-a-local-binding-has-no-identity-so-the-plan-is-rejoined-by-name-four-times.md),
and [0064](done/0064-the-java-to-rust-name-map-is-a-pure-non-injective-function.md).

## Scope note

Not an exhaustive sweep, but the parser-coverage dimension **is** now exhaustive:
see [../AUDIT-BACKLOG.md](../AUDIT-BACKLOG.md) for the grammar diff and the
remaining territory, ranked by expected yield.

30 probes have produced 16 validated defects, **9 of them silent** — they
compile cleanly and return wrong values, delete code, or panic at runtime, so no
compile-error count would ever surface them. The method that found them costs
about ten minutes each: write a ten-line Java file, translate, compile, run,
diff against Java.

That method is now automated: `tests/differential.rs` runs `javac`+`java` as
the oracle against translate+`rustc`+run and compares stdout, over the fixtures
in `tests/differential/`. Fixtures reproducing an open defect live in
`tests/differential/known-gaps/` so the harness stays green as a gate; moving
one up a level is the regression test for the fix. 0034 was found this way,
along with six arithmetic and formatting defects before it.
