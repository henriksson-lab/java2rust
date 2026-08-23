# 0052 — The example corpora are asserted on, never compiled, and almost never run

Recorded 2026-08-21.
Status: **partially fixed** — the corpus compile visibility gap is now covered by
`tests/corpus_compile.rs`; behavioural coverage and per-assertion compile
discipline remain open.
Severity: **high** — 14 of 189 checked-in Java fixtures in the current dirty tree
translate to Rust that does not compile, and most of those failures are invisible
to the old summary metric.
Kind: measurement / coverage.

Extends [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md)
(the compile-error metric cannot see silent failures) and
[0041](done/0041-slot-shape-design-had-no-generated-crate-checks.md) (a suite
that asserted on rendered Rust with nothing compiling it). 0041 fixed one suite.
The same hole is still open in three more, and it is open on the *fixture
corpora themselves* — the Java files the whole project is organised around.

---

## Symptom

`examples/java` (67 files) and `examples/idiom-gaps` (122 files) are the
project's curated inputs. 921 tests assert on the Rust rendered from them and
from inline sources. **Nothing compiles the corpora.** Doing so once, by hand,
originally found 58 files whose output `rustc` rejects — including two that
produce Rust that does not even *parse*. Follow-up fixes reduced that baseline
to 20 files; a later corpus guard run surfaced eight stale baseline deltas in
the dirty tree. Follow-up fixes brought the checked baseline to 24 files, and
return-slot structural-form fixes reduced it to 20 files. A follow-up optional
method-receiver equality fix reduced it to 19 files by making
`InstanceofGuardNarrowsCast` compile. External public-field facts for the
comparator fixture then reduced it to 18 files by making
`ComparatorInterface` compile. Callable-return inference for raw `submit(Callable)`
callbacks then reduced it to 17 files by making `ExecutorSubmitBlockLambda`
compile. Raw collection erasure for `List.add` / `List.get` then reduced it to
16 files by making `RawTypes` compile. Fallback static-final external field
surfaces then reduced it to 15 files by making
`ExternalFieldThrowingCallInTry` compile without injected facts. Java NIO `Files`
static support plus mutable-byte-buffer reborrowing then reduced it to 14 files
by making `TryWithResourcesCopy` compile.

### Reproduction

```sh
for f in $(find examples/java examples/idiom-gaps -name '*.java'); do
  b=$(basename $f .java)
  ./target/release/new-java2rust "$f" > /tmp/$b.rs &&
  rustc --edition 2021 --crate-type lib --crate-name gen_x \
        --emit=metadata -o /tmp/$b.rmeta /tmp/$b.rs
done
```

| corpus | files | compile | do **not** compile |
|---|---:|---:|---:|
| `examples/java` | 67 | 58 | **9** |
| `examples/idiom-gaps` | 122 | 117 | **5** |
| total | 189 | 175 | **14** |

Only one of the 15 is a harness artefact: `ArrayInitializers` needs the
`ndarray` crate, the limitation `tests/differential/ObjectArray2D.java` already
records. The other 14 are translator output that `rustc` rejects on its own
terms — no unlinked crates, checked by `grep -l 'unlinked crate' *.err`.

---

## Why no metric sees it

`count_invalid_patterns` (`src/output_markers.rs:236`) is the signal that is
supposed to mean *"the translator produced something it cannot stand behind"*.
Across the 15 failures it fires **four times**:

| corpus | non-compiling | of which emit `compile_error!` |
|---|---:|---:|
| `examples/java` | 9 | **2** (`LocalClassCapture`, `NullOverloadAmbiguity`) |
| `examples/idiom-gaps` | 6 | **2** (`SuperConstructorState`, `UnresolvedConstructorDelegation`) |

So 10 of 14 files are scored as clean translations. This is exactly 0018's
thesis, now with a corpus-sized number attached: a file can be *rejected by the
Rust compiler* and still contribute zero to every counter in `summary.tsv`.

`unimplemented!` is present in many of them (24 occurrences in
`TryWithResources`), but it is a *stub* marker counted separately and it is
never at the site of the error.

---

## Verified backlog A — `examples/java`, 9 files

Work queue. Error code is the first `rustc` diagnostic; `refs` is how many test
files mention the fixture path at all.

| fixture | first error | refs |
|---|---|---:|
| `03-methods/NullOverloadAmbiguity.java` | `compile_error!` ambiguous null overload | 1 |
| `04-objects/AnonymousInterface.java` | `E0782` expected a type, found a trait | 3 |
| `04-objects/DynTraitMethodCompatibility.java` | `E0038`/`E0782`, 27 errors | 1 |
| `04-objects/LocalClassCapture.java` | `compile_error!` local type declaration | 2 |
| `05-types/ArrayInitializers.java` | `E0433` unlinked crate `ndarray` *(harness)* | 0 |
| `05-types/WildcardBounds.java` | `E0599` `int_value` on `&Box<dyn Any>` | 1 |
| `06-stdlib/StreamFlatMapBlockPipeline.java` | `E0282`/`E0599` `trim` on `Vec<String>` | 1 |
| `06-stdlib/TryCatchFinally.java` | `E0308` | 1 |
| `06-stdlib/TryWithResources.java` | `E0433` undeclared type `JavaNioFileFiles` | 1 |

Two are worth reading in full because the emitted Rust is not subtly wrong,
it is nonsense:

```rust
// TryWithResources.rs:3260 — a facade type that is never emitted
let reader: Box<dyn std::any::Any> = (Box::new(JavaNioFileFiles::new_buffered_reader(&path)…
```

**Four of the 9 are referenced by no test whatsoever** (`refs=0`). They exist
only to be counted by `all_java_fixtures_are_loadable`, which asserts
`fixtures.len() == 65` and currently *fails* because there are 67 — the count
was never updated when two files were added
(`tests/fixtures.rs:16`, `left: 67  right: 65`).

More generally, **21 of the 67 `examples/java` files are referenced by no test**:
`MixedNumericArithmetic`, `NumericPromotion`, `StringConcat`, `ForOmittedParts`,
`LabeledBreak`, `LabeledContinueForUpdate`, `NamingPolicy`, `NullOverloadAmbiguity`,
`Varargs`, `VarargsCalls`, `AliasingSharedMutation`, `ConstructorExpressions`,
`FieldWriteReceiverMutation`, `StaticFinalFields`,
`ArrayInitializers`, `ArraysAndVarMutation`, `GenericsAndWildcards`,
`CheckedException`, `OptionalAndObjects`,
`StringFormatAndThrow`, `SynchronizedBlock`.

---

## Verified backlog B — `examples/idiom-gaps`, 5 files

Only **43 of the 122** idiom-gaps fixtures are reached by any test that compiles
its output. `tests/idiom_gaps.rs` has 351 compile-checked tests, but they check
mostly *inline* Java sources: of the 696 tests that assert on `translation.rust`,
571 use inline source (317 compile-checked) and **124 use a fixture file, of
which 34 compile-checked**.

So **4 of the 5 non-compiling fixtures are unreached by any compile check**:

| error | fixtures |
|---|---|
| `E0599` no method (2) | `SparkRddVariantPipeline`, `StreamCallbackChain` |
| `E0277`/`E0308` try-with-resources uncaught-result wrapping (1) | `TryWithResourcesReturn` |
| `compile_error!` diagnostics (2) | `SuperConstructorState`, `UnresolvedConstructorDelegation` |

Three fixtures are referenced by nothing at all: `NonNullFieldViaHelper`,
`PackagePrivateGetterSetterInlining`, `ToArrayEmptyStringArray`.

### Fixed sharp edge: optional array field read into owned setter parameter

`ArraySetterOwnedStorage.copy` passed `source.letters` into a setter whose
signature wants owned `Vec<char>`. The old lowering unwrapped the `Option` by
value (`source.letters.expect(...)`), which tried to move a field out of the
borrowed source object and failed with `E0507`. A 2026-08-23 follow-up now
treats optional field reads feeding owned array parameters as borrowed carrier
reads, emitting `as_ref().cloned()` before the Java-null check. The fixture is
compile-checked in `tests/idiom_gaps.rs`.

### Fixed sharp edge: imported static side-effect fallback returns generic `R`

`FallbackStaticMutBorrowArgument` calls an unresolved imported static
`ExternalApi.fill(config)` as a statement. The fallback facade used to render
`pub fn fill<R, T0>(_arg0: T0) -> R`, leaving `R` unconstrained at the call site
and failing with `E0282`. A 2026-08-23 follow-up classifies `fill` with the
other side-effect-style imported static names, so the no-facts corpus path now
renders a unit-returning stub and compiles. The existing argument-effect test
also asserts the more precise `&mut MutableConfig` unit-returning facade when an
explicit mutating argument fact is supplied.

### Fixed sharp edge: fallible nullable assignment into erased optional local

`JalignerTryCatchFinallyCleanup` assigned `resource.read()` into a nullable
`String` local inside a `try` block. The local currently lowers through an
erased optional carrier, and the conversion pipeline lost the fact that `?` had
already been applied while wrapping the value as `Some(Box<dyn Any>)`. That
emitted `Some(...) ?` inside a `Result` closure and failed with `E0277`. A
2026-08-23 follow-up preserves the `ResultPropagated` shape through structured
`Some` and `Box::new(... as dyn Any)` wrappers, so the generated assignment is
`sequence = Some(Box::new(resource.read()?) as Box<dyn std::any::Any>);`.
The same shape fix also made `JalignerFallthroughTryCatch` compile. Both
fixtures have generated-crate checks.

### Fixed sharp edge: captured collection mutation inside `forEach`

`MapForEachTryCatchLambda` lowered `Map.forEach` to a loop, but signature
planning did not look through lambda bodies when deciding whether a method
parameter binding needed `mut`. The generated body inserted into `dest`, while
the generated parameter was immutable:

```rust
pub fn copy(&self, futures: &HashMap<String, Future>, dest: HashMap<String, Rc<Item>>) -> Result<(), JavaException> {
    // ...
    dest.insert(name.clone(), item);
}
```

That failed with `E0596`. A 2026-08-23 follow-up makes the signature mutation
walk recurse into lambda bodies, excludes lambda parameters from the captured
mutation set, and reuses the native collection mutator table for calls such as
`put`, `add`, and `addAll`. The generated `copy` parameter is now `mut dest`,
and the fixture compiles directly.

### Fixed sharp edge: unresolved collection source calls in `addAll`

`UnresolvedSetReturnAddAll` used the argument slot from `Set.addAll`, but still
rendered an unresolved receiverless source call as a Rust free function:

```rust
self.names.as_mut().unwrap().extend(default_names());
```

Because the Java fixture has no `defaultNames` declaration, the generated Rust
failed with `E0425`. A 2026-08-23 follow-up keeps the target collection shape
but renders an unresolved collection-producing call as a typed empty fallback
with an audit marker:

```rust
self.names.as_mut().unwrap().extend(
    <std::collections::HashSet<String> as Default>::default()
    /* TODO(translation): unresolved Java collection source call defaultNames */
);
```

The fixture now compiles directly and has a fixture-backed generated-crate check.

### Fixed sharp edge: unresolved class literal token types

`ClassLiteralRegistration` compiled Java code that referenced
`RegisteredType.class` without declaring `RegisteredType` in the fixture. The
lowering kept the Java class name but used it as the Rust token type:

```rust
kryo.register_1(JavaClass::<RegisteredType>::literal("RegisteredType"));
```

That made the output fail with `E0412`. A 2026-08-23 follow-up keeps concrete
source class literals typed, but renders unresolved named literals as
`JavaClass<()>` while preserving the runtime Java name:

```rust
kryo.register_1(JavaClass::<()>::literal("RegisteredType"));
```

The fixture now compiles directly and has a fixture-backed generated-crate check.

### Fixed sharp edge: a facade method named `1`

`Tuple2::_1` / `Tuple2::_2` (Spark) lose their leading underscore when the
facade **definition** is synthesised, while the **call site** keeps it:

```rust
// SparkRddVariantPipeline.rs:1929
impl Tuple2 {
    pub fn 1(&self) -> Option<Box<dyn std::any::Any>> { unimplemented!("Tuple2.1 is not modelled") }
    pub fn 1<T0>(&self, _arg0: T0) -> Option<Box<dyn std::any::Any>> { unimplemented!("Tuple2.1 is not modelled") }
    pub fn _1(&self) -> Option<Box<dyn std::any::Any>> { unimplemented!("Tuple2._1 is not modelled") }
    …
}
…
// :1979
return sorted.map(|value: Tuple2| value._1());
```

`pub fn 1` is not valid Rust, so **the entire file failed to parse** — which per
0018 meant the file reported *zero type errors* in a corpus run, the strongest
form of the erasure-rewards-the-metric problem. A 2026-08-23 follow-up now
normalizes digit-leading facade aliases to `_1` / `_1_1` style Rust identifiers.
Both fixtures still fail later type checks, but they no longer poison the corpus
measurement as parse failures.

Two tests cover these fixtures:
`corpus_spark_rdd_pipeline_keeps_stream_lowering_at_noop_spark_boundary`
(`tests/idiom_gaps.rs:15300`) asserts `value._1()`, and
`documents_stream_callback_chain_gap` (`tests/idiom_gaps.rs:13414`) asserts
`value._2()`. They now also assert that the generated facade does not contain
digit-leading method definitions.

---

## Which suites assert on rendered Rust without compiling it

Counted by scanning each `#[test]` block for `.rust` and for
`cargo_check_generated_modules` / `cargo_test_generated_modules`
(`tests/common/mod.rs:21,25`):

| suite | tests | assert on `translation.rust` | compile it | **unchecked** |
|---|---:|---:|---:|---:|
| `tests/idiom_gaps.rs` | 767 | 696 | 349 | **347** |
| `tests/slot_shape_design.rs` | 604 | 160 | 104 | **56** |
| `tests/fixtures.rs` | 54 | 42 | **4** | **38** |
| `tests/semantic_facts.rs` | 170 | 11 | 0 | 11 |
| `tests/output_markers.rs` | 23 | 8 | 0 | 8 |
| `tests/batch_translation.rs` | 11 | 3 | 0 | 3 |
| `tests/external_packs.rs` | 167 | 2 | 0 | 2 |
| `tests/idiom_sweep_script.rs` | 48 | 1 | 0 | 1 |
| **total** | | **923** | **456** | **467** |

The remaining nine suites (`dependency_mode`, `resolve_calls`, `type_coercion`,
`object_model`, `parser_gaps`, `parser_tree_sitter`, `capture_facts`,
`effective_facts`, `differential`) assert on facts/IR, not on rendered Rust, and
are correctly out of scope here.

`tests/fixtures.rs` is 0041's finding repeated one suite over: **38
tests assert on rendered Rust and only four of them compile it**, and it is the
suite that owns `examples/java` — the corpus with 9 non-compiling files.

### The backlog 0041 opened is growing, silently

0041 listed 52 `slot_shape_design` tests left unchecked (21 genuine + 31
needing harness support). Today the unchecked set is **56**: three of the listed
ones were fixed (`collections_sort_comparator_*`) and **seven new unchecked
tests were added** —

- `a_cotranslated_source_type_gets_no_unmodelled_import_stub`
- `context_static_string_constant_owner_stays_sibling_source_type`
- `implemented_shared_identity_equality_has_no_gap_diagnostics`
- `item_assembly_owns_struct_impl_and_function_shells_from_signatures`
- `semantic_analysis_owns_exception_classification_and_catch_facts`
- `source_summary_static_string_constant_emits_external_facade`
- `translate_source_with_facts_ingests_external_closed_hierarchy_facts`

Nothing requires a new translation test to compile its output, so the ratio
drifts back down after every cleanup.

---

## Behavioural coverage: 23 fixtures and six live tests

Compiling is a gate, not an oracle. The project's entire behavioural surface —
"run the Java, run the Rust, compare" or "run the Rust against a written
expectation" — is:

1. `tests/differential.rs` — **one** `#[test]`, **23 fixtures**, 1241 lines of
   Java total. Verified working: a full run takes 55s and all 23 pass.
2. `tests/fixtures.rs` — **10** tests that write the translation to a file, add
   a hand-written `mod generated_tests`, and `rustc --test` it.
   **Four are currently failing** and baselined as such, so six are live.

That is 7 of 2617 tests, over 23 + ~10 Java programs.

### The hand-written oracles are not oracles, and one has been dead for a while

`switch_fallthrough_generated_rust_preserves_behavior` (`tests/fixtures.rs:1178`)
asserts `score("A") == 4, "B" == 2, "C" == 2, "D" == -1`, which is correct Java.
It fails — but not on behaviour:

```
error[E0308]: mismatched types
1798 |  assert_eq!(scorer.score("A".to_string()), 4);
1798 |  assert_eq!(scorer.score(&"A".to_string()), 4);   <-- rustc's suggestion
```

The generated signature moved to `&String`; the hand-written harness was never
updated. So the only behavioural check on switch fallthrough has not executed a
single assertion since the signature changed, and the failure is indistinguishable
from a translation defect. That is 0041's "a stale expectation hid a genuine type
error", inverted: here a stale expectation hides the *absence* of any check.

The deeper point is that these ten tests encode *the translator author's belief*
about what the Java means. `tests/differential.rs` asks `javac`. Only the second
kind can find a defect the author did not anticipate — which is why, per 0018,
it found one on every batch of fixtures added to it.

### What the differential corpus cannot see

Scanned across all 23 fixtures (`grep -lE … tests/differential/*.java`):

| Java construct | fixtures using it |
|---|---:|
| `null` literal | **0** |
| `throw` statement | **0** |
| `throws` clause | **0** |
| `char` declaration | **0** (one `'y'` literal) |
| `Map` / `Set` / `HashMap` / `HashSet` | **0** |
| enhanced `for (X x : xs)` | **0** |
| `enum` | **0** |
| `abstract` class | **0** |
| `String.format` | **0** |
| `String` instance methods (`substring`, `split`, `charAt`, `indexOf`, `trim`) | **0** |
| `do { } while` | **0** |
| varargs | **0** |
| `List` / `ArrayList` | 1 (`LambdaMutation`) |
| `.stream()` | **0** |

`null` is the one that matters most: nullable slots, `Option` carriers and
null-check narrowing are among the largest subsystems in `src/`, they are the
subject of a large share of `slot_shape_design`, and **not one line of Java in
the behavioural corpus contains the token `null`**. The same is true of every
`E0599 expect`-on-a-non-`Option` failure in backlog A, which is a nullable-slot
defect.

Three structural limits keep the corpus small, and they are worth naming
because they are the actual cost of adding fixtures:

- `fixture_paths()` (`tests/differential.rs:167`) reads `*.java` in one flat
  directory and `javac`s **one file**. Multi-class programs, packages and
  cross-file inheritance cannot be expressed.
- `harness_main()` (`:156`) can only drive `public static String run()`. A
  program whose observable behaviour is `System.out` interleaving, or a
  side effect, cannot be compared.
- the subject is built with bare `rustc`, no `Cargo.toml`, so any fixture
  touching a crate-mapped facade (`ndarray`, `hdf5_facade`, …) cannot run —
  the same limitation as 0041's 31 harness-only failures.

**Zero of the 67 `examples/java` files declare a `main`**, so the curated corpus
is structurally incapable of being run as Java at all. The two corpora have no
overlap in method: one is asserted on and never executed, the other is executed
and never grows.

---

## Root cause

Three separate decisions, each locally reasonable:

1. `cargo_check_generated_modules` is **opt-in per test**. A test author who
   forgets it gets a green test. Nothing in the suite requires a test that
   touches `translation.rust` to also compile it — so the checked fraction is
   whatever history left behind (47% overall, 0% in `fixtures.rs`).
2. The **fixture corpora are inputs, not subjects**. `examples/java` and
   `examples/idiom-gaps` are read by tests but are never themselves swept: no
   job translates all 189 and compiles the results. `translate-focused-package.sh`
   does exactly this for *external* corpora, and would find all 58 in one run.
3. **Compiling and running are conflated in cost estimation.** Compiling the
   corpora is ~2 minutes of `rustc` with no cargo involved (measured: the 189-file
   sweep above). It has never been done because it looks like it belongs to the
   expensive differential harness, which it does not.

---

## Codex update — 2026-08-23

Implemented the first direction item:

- Added `tests/corpus_compile.rs::checked_in_example_corpora_have_named_compile_failures`.
  It translates every checked-in Java fixture under `examples/java` and
  `examples/idiom-gaps`, runs `rustc --crate-type lib --emit=metadata` on each
  generated file, and compares the failing fixture names against
  `tests/baselines/corpus-compile-failures.txt`.
- The sweep is bounded by the existing test watchdog and parallelized with a
  conservative default of four workers. Override with
  `NEW_JAVA2RUST_CORPUS_COMPILE_JOBS`.
- HEAD then had **59** named corpus compile failures. Claude's measured 58 were
  still valid as a shape, but the tree also failed
  `idiom-gaps/MethodReferenceFlatMap.java`.
- Fixed the stale `all_java_fixtures_are_loadable` count from 65 to the current
  67 files.

Still open from this audit:

- The corpus compile failures are now visible but not all fixed. A 2026-08-23
  follow-up fixed borrowed stream terminal collection for flat-map pipelines,
  removing `idiom-gaps/MethodReferenceFlatMap.java` and
  `java/06-stdlib/StreamFlatMapLambda.java`. A second pass fixed owned
  flat-map projection shape, removing
  `java/06-stdlib/StreamFlatMapProjection.java`. A third pass fixed source
  static owners shadowing JDK facade owners, removing
  `java/06-stdlib/StaticOwnerShadowing.java`. A fourth pass fixed redundant
  Java-null receiver projection after optional collection `unwrap()` in native
  capacity arguments, removing
  `java/06-stdlib/CollectionCapacityConstructor.java`. A fifth pass applied the
  same normalization to required borrowed string views after fallible optional
  reads, removing `java/06-stdlib/CheckedPropagation.java`; the same projection
  fix also removed `idiom-gaps/IndexedStringArrayOwnership.java`,
  `idiom-gaps/NullableTrimEqualsOriginal.java`, and
  `idiom-gaps/OptionalArgumentWrapReuse.java`. A sixth pass fixed imported
  static-owner support where the fully qualified facade owns the associated
  function while the simple instantiated facade needs an instance-compatible
  wrapper, removing `java/06-stdlib/ImportedStaticUtilityOwner.java`. A seventh
  pass made string return signatures respect the merged owner/view constraint
  result instead of raw rejected borrowed constraints, removing
  `java/06-stdlib/ExternalStringConstantArgument.java`. An eighth pass skipped
  source enum methods that duplicate generated constructor-payload accessors,
  removing `java/04-objects/EnumConstantsWithFields.java`. A ninth pass made
  effect-backed external getter facades reuse known direct return types for
  audited primitive and primitive-array getters, removing
  `idiom-gaps/BooleanGetterDispatchCoercion.java`,
  `idiom-gaps/ByteArrayGetterDispatchCoercion.java`,
  `idiom-gaps/ExternalGetterReadOnly.java`,
  `idiom-gaps/ExternalGetterSetterMutability.java`, and
  `idiom-gaps/PrimitiveGetterDispatchCoercion.java`. A tenth pass recursively
  stripped redundant Java-null receiver `expect` calls after optional
  `unwrap()` inside map-entry iteration receivers, removing
  `idiom-gaps/MapEntrySetFirstKeyFromNullableField.java`. An eleventh pass
  seeded self-returning fluent facts for chained external calls whose expression
  shape or declared return type proves that the external receiver is returned,
  removing `idiom-gaps/ExternalFluentMutation.java`. A twelfth pass fixed
  digit-leading external facade aliases for Scala tuple accessors, changing
  `SparkRddVariantPipeline.java` and `StreamCallbackChain.java` from parse
  failures into ordinary type-check failures. Later focused fixes, including
  optional array field reads into owned setter parameters, imported static
  side-effect fallback stubs, fallible nullable assignment into erased
  optional locals, and duplicate static/instance `Objects.equals` facade
  pruning, reduced the known corpus compile baseline to **30** failures. A
  subsequent pass lowered class literals as erased `JavaClass<()>` tokens when
  returning into `Class<?>` slots or comparing against `getClass()`, removing
  `idiom-gaps/ClassLiteralToken.java` and reducing the baseline to **29**
  failures. Another pass made expected assignment/variable types seed guessed
  external call return surfaces and allowed known external `String` returns to
  replace opaque placeholders, removing `idiom-gaps/TypedGetterAvoidsDowncast.java`
  and `idiom-gaps/ExternalIndexedAccessors.java`; the baseline is now **27**
  failures. The same target-type seeding now also applies to return expressions
  for unknown external calls, removing
  `idiom-gaps/ExceptionBaseCatchCoversSubclass.java`,
  `idiom-gaps/ExceptionSubclassCatchOrder.java`, and
  `idiom-gaps/ExceptionRuntimeCheckedHierarchy.java`; the baseline is now
  **24** failures. A later narrow return-slot fix wraps `return this` values
  when the same-type return slot is shared, removing
  `idiom-gaps/MapForEachBlockLambda.java`; the baseline is now **23**
  failures. A subsequent pass let declared local/assignment target types seed
  named external call return surfaces, propagated expected primitive types
  through arithmetic, and marked null-compared assignment calls as nullable,
  removing `idiom-gaps/FfiNativeHandoverCall.java`,
  `idiom-gaps/WhileAssignmentNotNull.java`, and
  `idiom-gaps/WhileAssignmentNullVariants.java`; the baseline is now **20**
  failures. A later return-slot ABI alignment made source method bodies lower
  against the same erased external return boundary emitted in the signature,
  removing `idiom-gaps/TypedFeatureReaderHeaderCast.java`; the same corpus guard
  run exposed eight stale baseline deltas already present in the dirty tree, so
  the checked baseline moved back to **24** named failures after the source
  static-owner fix for `java/06-stdlib/StaticOwnerShadowing.java` and primitive
  stream-sum lowering for `java/06-stdlib/StreamsUse.java` and
  `java/06-stdlib/StreamCapturesLocal.java`. A subsequent pass made
  return-slot external-call adaptation decline recognized stream expressions,
  so `anyMatch` terminals are lowered by the stream planner instead of as raw
  receiver method calls, removing `java/06-stdlib/StreamsFindMatch.java` and
  `java/06-stdlib/StreamStringMethodReferences.java`; the checked baseline is
  now **22** named failures. The same return-slot preemption class also hid the
  existing map-entry chain lowering behind external-call adaptation; declining
  recognized map-entry accessors there and normalizing optional receiver
  projections removed `idiom-gaps/MapEntrySetFirstKeyFromNullableField.java`
  and `idiom-gaps/MapEntrySetFirstKey.java`. The checked baseline is now
  **20** named failures. A follow-up made raw `submit(Callable)` parameters lower
  as generic zero-argument closures and taught callback return inference to see
  block-local declarations before return expressions, removing
  `idiom-gaps/ExecutorSubmitBlockLambda.java`. A follow-up made raw `List`
  arguments use erased-object element slots for `add`, and made raw sequence
  `get` clone through the erased-object runtime helper before returning an owned
  erased value, removing `java/05-types/RawTypes.java`. A subsequent pass made
  inferred fallback static-final external fields emit opaque associated-const
  surfaces instead of undeclared owner paths or intentional compile errors,
  removing `idiom-gaps/ExternalFieldThrowingCallInTry.java`. A follow-up added
  `JavaNioFileFiles` support for static `Files` calls and made one-argument
  `InputStream.read(byte[])` lowering reborrow mutable byte buffers instead of
  consuming `&mut [i8]`, removing `idiom-gaps/TryWithResourcesCopy.java`. The
  checked baseline is now **14** named failures after the comparator,
  submit-callable, raw collection erasure, fallback static-field, and
  try-with-resources copy fixes.
- Many tests still assert on `translation.rust` without local compile checks;
  the corpus sweep catches checked-in fixtures, not every inline assertion.
- Behavioural coverage is still thin; differential fixtures for `null`,
  collections and `throw` remain the highest-value next step.
- The stale hand-written behaviour harnesses in `tests/fixtures.rs` still need
  repair or replacement.

## Direction

Cheapest first; the first item has landed, so the remaining work is shrinking
the named baseline and closing the still-unchecked rendered-Rust assertions.

1. **Keep the corpus compile test strict.** A single `#[test]` translates every file
   under `examples/` and `rustc --crate-type lib --emit=metadata`s the output,
   with an allow-list checked in at `tests/baselines/corpus-compile-failures.txt`
   holding the current 14 names. Cost: no cargo, one `rustc` per file, ~2
   minutes. Value: the whole of backlogs A and B stays a named, shrinking list
   instead of an unknown, and every future fixture is compiled the day it is
   added. Note this is the *corpus* analogue of what 0041 did per test.
2. **Make the check non-optional where it is cheap.** A test that mentions
   `translation.rust` and does not compile it is the 0041 pattern. The tests in
   `fixtures.rs` are the obvious next batch — they already have the corpus, and
   the fixture-level compile test from (1) subsumes most of them.
3. **Continue the Spark tuple fixtures.** `Tuple2::_1` and `_2` now render as
   valid Rust facade method names. The remaining failures are ordinary Spark
   receiver/iterator type mismatches, so the corpus sweep can now report useful
   compiler diagnostics for both files.
4. **Differential fixtures for `null`, collections and `throw`,** in that order.
   0018 records that the harness found a defect in every batch added to it; the
   three largest untouched areas are the three above, and backlog A's five
   `E0599 expect`-on-a-non-`Option` failures say the null area is already broken
   in ways a string assertion approved.
5. **Fix the stale hand-written harnesses** in `tests/fixtures.rs` (4 dead) or
   delete them in favour of differential fixtures. A behavioural test that
   cannot compile its own assertions is worse than none, because it occupies the
   slot where a working one would go.

## Relation to other issues

- [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md) — this
  file is the corpus-sized measurement of its central claim: 55 of 58
  compiler-rejected files score as clean.
- [0041](done/0041-slot-shape-design-had-no-generated-crate-checks.md) — same
  hole, three suites over, plus evidence that its backlog grows without a rule.
- [0009](0009-overload-identity-collapses-onto-rust-name.md) — the former
  `E0592` failures in `EqualsAndObjects` and `NullableEqualsComparison` are now
  fixed; `EnumConstantsWithFields` remains covered by the original overload
  identity audit history.
- [0011](done/0011-dyn-any-fallback-fails-at-every-use.md) — `E0599` on
  `&Box<dyn Any>` in `WildcardBounds`, exactly as predicted.
