# 0052 — The example corpora are asserted on, never compiled, and almost never run

Recorded 2026-08-21.
Status: **measured** — every number below is reproduced by a command in this file.
Severity: **high** — 58 of 189 checked-in Java fixtures translate to Rust that does
not compile, and 57 of those 58 are invisible to every metric the project has.
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
project's curated inputs. 918 tests assert on the Rust rendered from them and
from inline sources. **Nothing compiles the corpora.** Doing so once, by hand,
finds 58 files whose output `rustc` rejects — including two that produce Rust
that does not even *parse*.

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
| `examples/java` | 67 | 49 | **18** |
| `examples/idiom-gaps` | 122 | 82 | **40** |
| total | 189 | 131 | **58** |

Only one of the 58 is a harness artefact: `ArrayInitializers` needs the
`ndarray` crate, the limitation `tests/differential/ObjectArray2D.java` already
records. The other 57 are translator output that `rustc` rejects on its own
terms — no unlinked crates, checked by `grep -l 'unlinked crate' *.err`.

---

## Why no metric sees it

`count_invalid_patterns` (`src/output_markers.rs:236`) is the signal that is
supposed to mean *"the translator produced something it cannot stand behind"*.
Across the 58 failures it fires **three times**:

| corpus | non-compiling | of which emit `compile_error!` |
|---|---:|---:|
| `examples/java` | 18 | **1** (`LocalClassCapture`) |
| `examples/idiom-gaps` | 40 | **2** (`SuperConstructorState`, `UnresolvedConstructorDelegation`) |

So 55 of 58 files are scored as clean translations. This is exactly 0018's
thesis, now with a corpus-sized number attached: a file can be *rejected by the
Rust compiler* and still contribute zero to every counter in `summary.tsv`.

`unimplemented!` is present in many of them (24 occurrences in
`TryWithResources`, 18 in `NullOverloadAmbiguity`), but it is a *stub* marker
counted separately and it is never at the site of the error — `NullOverloadAmbiguity`
fails on `cannot find function 'pick' in this scope`, which no stub explains.

---

## Verified backlog A — `examples/java`, 18 files

Work queue. Error code is the first `rustc` diagnostic; `refs` is how many test
files mention the fixture path at all.

| fixture | first error | refs |
|---|---|---:|
| `03-methods/NullOverloadAmbiguity.java` | `E0425` cannot find function `pick` | 0 |
| `04-objects/AnonymousInterface.java` | `E0782` expected a type, found a trait | 3 |
| `04-objects/DynTraitMethodCompatibility.java` | `E0038`/`E0782`, 27 errors | 1 |
| `04-objects/EnumConstantsWithFields.java` | `E0592` duplicate definitions `id` | 0 |
| `04-objects/LocalClassCapture.java` | `compile_error!` local type declaration | 2 |
| `05-types/ArrayInitializers.java` | `E0433` unlinked crate `ndarray` *(harness)* | 0 |
| `05-types/RawTypes.java` | `E0308` | 0 |
| `05-types/WildcardBounds.java` | `E0599` `int_value` on `&Box<dyn Any>` | 1 |
| `06-stdlib/CheckedPropagation.java` | `E0599` `expect` on `&String` | 0 |
| `06-stdlib/CollectionCapacityConstructor.java` | `E0599` `expect` on `&Vec<String>` | 1 |
| `06-stdlib/ExternalStringConstantArgument.java` | `E0515` returns value referencing param | 1 |
| `06-stdlib/ImportedStaticUtilityOwner.java` | `E0599` `read_value` on `ExternalUtility` | 1 |
| `06-stdlib/StaticOwnerShadowing.java` | `E0308` `return Vec::new()` from `-> i32` | 0 |
| `06-stdlib/StreamFlatMapBlockPipeline.java` | `E0282`/`E0599` `trim` on `Vec<String>` | 1 |
| `06-stdlib/StreamFlatMapLambda.java` | `E0308` | 1 |
| `06-stdlib/StreamFlatMapProjection.java` | `E0271`/`E0599` | 1 |
| `06-stdlib/TryCatchFinally.java` | `E0308` | 1 |
| `06-stdlib/TryWithResources.java` | `E0433` undeclared type `JavaNioFileFiles` | 1 |

Three are worth reading in full because the emitted Rust is not subtly wrong,
it is nonsense:

```rust
// StaticOwnerShadowing.rs:1814
pub fn count(&self) -> i32 {
    return Vec::new();
}

// CollectionCapacityConstructor.rs:1789 — .expect() applied after .unwrap()
let headers: Vec<String> =
    Vec::with_capacity((self.names.as_ref().unwrap().expect("Java null receiver").len() as i32) as usize);

// TryWithResources.rs:3260 — a facade type that is never emitted
let reader: Box<dyn std::any::Any> = (Box::new(JavaNioFileFiles::new_buffered_reader(&path)…
```

**Six of the 18 are referenced by no test whatsoever** (`refs=0`). They exist
only to be counted by `all_java_fixtures_are_loadable`, which asserts
`fixtures.len() == 65` and currently *fails* because there are 67 — the count
was never updated when two files were added
(`tests/fixtures.rs:16`, `left: 67  right: 65`).

More generally, **25 of the 67 `examples/java` files are referenced by no test**:
`MixedNumericArithmetic`, `NumericPromotion`, `StringConcat`, `ForOmittedParts`,
`LabeledBreak`, `LabeledContinueForUpdate`, `NamingPolicy`, `NullOverloadAmbiguity`,
`Varargs`, `VarargsCalls`, `AliasingSharedMutation`, `ConstructorExpressions`,
`EnumConstantsWithFields`, `FieldWriteReceiverMutation`, `StaticFinalFields`,
`ArrayInitializers`, `ArraysAndVarMutation`, `GenericsAndWildcards`, `RawTypes`,
`CheckedException`, `CheckedPropagation`, `OptionalAndObjects`,
`StaticOwnerShadowing`, `StringFormatAndThrow`, `SynchronizedBlock`.

---

## Verified backlog B — `examples/idiom-gaps`, 40 files

Only **18 of the 122** idiom-gaps fixtures are reached by any test that compiles
its output. `tests/idiom_gaps.rs` has 328 compile-checked tests, but they check
*inline* Java sources: of the 694 tests that assert on `translation.rust`,
571 use inline source (317 compile-checked) and **123 use a fixture file, of
which 11 compile-checked**.

So **38 of the 40 non-compiling fixtures are unreached by any compile check**:

| error | fixtures |
|---|---|
| unparseable Rust | `SparkRddVariantPipeline`, `StreamCallbackChain` |
| `E0308` mismatched types (18) | `BooleanGetterDispatchCoercion`, `ByteArrayGetterDispatchCoercion`, `ClassLiteralToken`, `ExceptionBaseCatchCoversSubclass`, `ExceptionRuntimeCheckedHierarchy`, `ExceptionSubclassCatchOrder`, `ExecutorSubmitBlockLambda`, `ExternalGetterReadOnly`, `ExternalGetterSetterMutability`, `ExternalIndexedAccessors`, `FfiNativeHandoverCall`, `JalignerFallthroughTryCatch`, `MapForEachBlockLambda`, `PrimitiveGetterDispatchCoercion`, `TypedFeatureReaderHeaderCast`, `TypedGetterAvoidsDowncast`, `WhileAssignmentNotNull`, `WhileAssignmentNullVariants` |
| `E0599` no method (5) | `ExternalFluentMutation`, `IndexedStringArrayOwnership`, `MapEntrySetFirstKeyFromNullableField`, `NullableTrimEqualsOriginal`, `OptionalArgumentWrapReuse` |
| `E0433` undeclared type (3) | `ExternalFieldThrowingCallInTry`, `TryWithResourcesCopy`, `TryWithResourcesReturn` |
| `E0592` duplicate definitions (2) | `EqualsAndObjects`, `NullableEqualsComparison` — this is [0009](0009-overload-identity-collapses-onto-rust-name.md) |
| other singles (8) | `ArraySetterOwnedStorage` `E0507`, `ClassLiteralRegistration` `E0412`, `ComparatorInterface` `E0609`, `FallbackStaticMutBorrowArgument` `E0282`, `JalignerTryCatchFinallyCleanup` `E0277`, `MapForEachTryCatchLambda` `E0596`, `UnresolvedSetReturnAddAll` `E0425`, `SuperConstructorState` (`compile_error!`) |

Four fixtures are referenced by nothing at all: `ExternalGetterReadOnly`,
`NonNullFieldViaHelper`, `PackagePrivateGetterSetterInlining`,
`ToArrayEmptyStringArray`.

### The sharpest instance: a facade method named `1`

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

`pub fn 1` is not valid Rust, so **the entire file fails to parse** — which per
0018 means the file reports *zero type errors* in a corpus run, the strongest
form of the erasure-rewards-the-metric problem.

Two tests cover these fixtures and both pass:
`corpus_spark_rdd_pipeline_keeps_stream_lowering_at_noop_spark_boundary`
(`tests/idiom_gaps.rs:15300`) asserts `value._1()`, and
`documents_stream_callback_chain_gap` (`tests/idiom_gaps.rs:13414`) asserts
`value._2()`. Both assert the **call site**, which is correct. Neither can see
the **definition**, which is not. A string assertion checks one side of a
two-sided contract; only the compiler checks that the two agree.

---

## Which suites assert on rendered Rust without compiling it

Counted by scanning each `#[test]` block for `.rust` and for
`cargo_check_generated_modules` / `cargo_test_generated_modules`
(`tests/common/mod.rs:21,25`):

| suite | tests | assert on `translation.rust` | compile it | **unchecked** |
|---|---:|---:|---:|---:|
| `tests/idiom_gaps.rs` | 765 | 694 | 328 | **366** |
| `tests/slot_shape_design.rs` | 604 | 160 | 104 | **56** |
| `tests/fixtures.rs` | 51 | 39 | **0** | **39** |
| `tests/semantic_facts.rs` | 170 | 11 | 0 | 11 |
| `tests/output_markers.rs` | 23 | 8 | 0 | 8 |
| `tests/batch_translation.rs` | 11 | 3 | 0 | 3 |
| `tests/external_packs.rs` | 167 | 2 | 0 | 2 |
| `tests/idiom_sweep_script.rs` | 48 | 1 | 0 | 1 |
| **total** | | **918** | **432** | **486** |

The remaining nine suites (`dependency_mode`, `resolve_calls`, `type_coercion`,
`object_model`, `parser_gaps`, `parser_tree_sitter`, `capture_facts`,
`effective_facts`, `differential`) assert on facts/IR, not on rendered Rust, and
are correctly out of scope here.

`tests/fixtures.rs` is 0041's finding repeated verbatim one suite over: **39
tests assert on rendered Rust and zero of them compile it**, and it is the suite
that owns `examples/java` — the corpus with 18 non-compiling files.

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

## Direction

Cheapest first; the first item is minutes of work and closes 55 of the 58
invisible failures.

1. **One corpus compile test.** A single `#[test]` that translates every file
   under `examples/` and `rustc --crate-type lib --emit=metadata`s the output,
   with an allow-list checked in at `tests/baselines/corpus-compile-failures.txt`
   holding today's 58 names. Cost: no cargo, one `rustc` per file, ~2 minutes.
   Value: the whole of backlogs A and B becomes a named, shrinking list instead
   of an unknown, and every future fixture is compiled the day it is added.
   Note this is the *corpus* analogue of what 0041 did per test.
2. **Make the check non-optional where it is cheap.** A test that mentions
   `translation.rust` and does not compile it is the 0041 pattern. The 39 in
   `fixtures.rs` are the obvious next batch — they already have the corpus, and
   the fixture-level compile test from (1) subsumes most of them.
3. **`pub fn 1`.** Fix the facade-name mangling that strips a leading
   underscore. It is one defect, it makes two files unparseable, and an
   unparseable file poisons the corpus error count for every other file in the
   run.
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
- [0009](0009-overload-identity-collapses-onto-rust-name.md) — `E0592` in
  `EqualsAndObjects`, `NullableEqualsComparison`, `EnumConstantsWithFields`.
- [0011](done/0011-dyn-any-fallback-fails-at-every-use.md) — `E0599` on
  `&Box<dyn Any>` in `WildcardBounds`, exactly as predicted.
