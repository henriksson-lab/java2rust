# JGraphT core translator-development target

The Java source is pinned to release tag `jgrapht-1.5.3`, commit
`be2abc30b9188e4099d737179e11bd669cf1e041`. Local tag metadata points to that
commit, whose release-plugin message and Maven version both identify 1.5.3.
The runner requires that exact commit and an unchanged tracked checkout.

There are 463 production Java files in `jgrapht-core`. The initial 104-file
dependency closure is recorded in
`scripts/focus-sets/jgrapht-core-graph-shortestpath.txt`. It includes core graph
interfaces, implementations, traversal and shortest-path algorithms, plus their
dependencies. The source checkout and byte-for-byte staging copy are disposable;
the translator, runner, focus list and corpus-neutral regressions are the work
products. Generated Rust must not be manually repaired.

## Current checkpoint

The latest completed full slice measurement is v50: all 104 files parse and
translate, but the generated crate still has 2,505 Rust errors and 54 invalid
patterns. The pinned Java behavior oracle passes; translated graph behavior
has not been verified because the crate does not compile. The goal is not
complete, and production emission has not widened beyond the 104-file closure.

Nested collection views and their source-precedence follow-ups are newer than
v42. The v43/v44 frozen binaries were not corpus-measured because stronger
checks found issues requiring further changes. v249 passed units/batch/nested
but failed the stronger qualified-platform/source identity check; frozen v45 is
therefore also unmeasured. The combined source-identity/mutable-trait-borrow
batch passes its selected gates (v256 plus v257). Frozen v46 removes 87 Rust
errors versus v42 without changing the invalid-pattern count. Its run is
complete. The result-payload/reborrow batch is measured in v47: 65 fewer errors,
but one additional fail-closed unsupported conversion, addressed in v48.
The combined source/wildcard/callback batch is measured in v48: four fewer
compiler errors and two fewer invalid patterns than v47. The comparator factory
argument no longer emits an unsupported conversion. Translation is slower,
however. The collection-return/catalog-lookup batch passes its combined gates
and is measured in v49: two fewer compiler errors, with the invalid-pattern
count unchanged. The imported-interface return bridge now passes. The
interface-default capability/inherited-generic batch passes its documented
regression gates and is measured in v50: 25 fewer compiler errors, with the
invalid-pattern count unchanged. Generic method equality is the next batch;
Java `.equals` must not require Rust structural `PartialEq`, and Java reference
`==` must remain a distinct operation.
Detailed evidence below is chronological;
older "pending" notes describe the state when each entry was made.

## Reproduction

```sh
scripts/prepare-jgrapht-core-1.5.3.sh
JOBS=10 scripts/run-jgrapht-core-1.5.3.sh
scripts/run-jgrapht-java-oracle.sh
```

`JGRAPHT_TRANSLATION_ROOT` selects a fresh output directory.
`JGRAPHT_BROAD_CONTEXT=1` analyzes all 463 sources while emitting the focus set.
The default analyzes only the staged 104-file closure.

For focused emission, call `scripts/translate-focused-package.sh` with absolute
`--primary`, `--focus-list`, `--emit-list` and `--out` paths. This retains the
analysis context while emitting only the requested files, and deliberately skips
assembly and `cargo check`. Explicit focus mode currently also creates source
summaries for the full primary tree, so its analysis timing is not directly
comparable with the default JGraphT runner. Use the same runner mode for timing
comparisons.

## Measured baseline, 2026-10-16

These complete 104-file gates used the default runner and ten workers.
Times are wall-clock seconds. Build and resolver preflight are excluded from the
combined translation/assembly comparison.

| Phase or result | focused-v2 | focused-v5 | focused-v6 | focused-v7 | focused-v8 | focused-v9 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Translation | 206.201 | 218.638 | 206.924 | 203.830 | 201.416 | 206.550 |
| Crate assembly | 242.112 | 60.520 | 61.204 | 60.466 | 59.541 | 58.684 |
| Translation + assembly | 448.313 | 279.158 | 268.128 | 264.296 | 260.957 | 265.234 |
| Generated `cargo check` | failed | failed | failed | failed | failed | failed |
| Reported compile errors | 510 | 445 | 354 | 327 | 299 | 260 |
| Unsupported expressions | 8 | 0 | 0 | 0 | 0 | 0 |

Assembly improved by 75%; translation plus assembly improved by approximately
38%. A separate worst-file lowering investigation reduced BiconnectivityInspector
from 822.711 to 79.456 seconds with byte-identical output at that measurement.
That per-file measurement does not replace a complete corpus gate.

The assembly profile identified repeated character scans of identical support
lines. Caching pure brace/delimiter scans and reusing existing item catalogs
removed that repetition. Lowering improvements reuse declaration signature plans,
scope source-type lookup caching to a lowering operation, and reduce name-rendering
allocations. Signature saturation in focused-v5 took 5.182 seconds over thirteen
iterations; it was not the dominant remaining phase on this slice.

Full evidence is produced below each output directory: `phase-events.tsv`,
`worker-events.tsv`, `status.tsv`, `compile.log`, `parse-failures.tsv`,
`invalid-patterns.tsv` and `top-compile-errors.tsv`. The measured directories were
`.tmp/jgrapht-core-1.5.3-focused-v2` and
`.tmp/jgrapht-core-1.5.3-focused-v5` and
`.tmp/jgrapht-core-1.5.3-focused-v6` and
`.tmp/jgrapht-core-1.5.3-focused-v7` and
`.tmp/jgrapht-core-1.5.3-focused-v8` and
`.tmp/jgrapht-core-1.5.3-focused-v9`.

## Current correctness work

The focused-v6 batch addresses source definitions taking precedence over unavailable
external facades, native collection arguments obscured by conservative hierarchy
entries, and generic nested-interface implementation identities. It also shares
the Java implicit-static member-type rule between enclosing-instance capture and
generic-parameter capture. Focused regressions check generated Rust compilation,
including a generic member class implementing its enclosing interface's member
interface.

That gate reported four translator error-status files, three generated Rust
syntax errors and 354 reported compile errors (including Cargo's final summary
line). Source interface aliases no longer erase `Graph` to `Any`, and generic
nested interface aliases and implementations retain their type arguments. The
centralized native-collection representation refinement and Consumer-expression
write-place fixes were made after the focused-v6 release build and are not
covered by that corpus measurement.

The follow-up write-place batch routes expression-bodied Consumer assignments
through ordinary statement lowering and field prefix/postfix updates through the
existing primitive update planner, with one mutable-place evaluation. Assignment
expressions and field updates now contribute storage mutation effects as well;
an untyped lambda receiver cannot prove shared storage immutable. Both minimized
generated-crate regressions in `tests/shared_place_lowering.rs` pass without any
extra method needed to force mutable storage. The 56 storage-strategy, 62
representation-lowering and 24 slot-render unit tests, plus the two
nested-interface generated-crate tests, also pass on this batch. The combined
focused-v7 gate reports 327 compile errors and one remaining syntax error, down
from 354 and three. The two Consumer field writes now use mutable borrows. The
remaining invalid increment concerns an unmodelled inherited JDK `modCount`
field, whose numeric type and storage still need a general inherited-member
solution. Four translator error-status files remain.

The focused-v8 batch records receiver reads during the existing usage walk, so private
methods reading instance fields in arithmetic or iteration cannot lose `self`.
Parameter shadowing still permits receiver-independent private helpers. It also
shares a parsed-source-aware nominal-facade policy between slot rendering and
facade return rendering, preventing built-in final facades from erasing source
generic arguments. The three neutral generated-crate tests in
`tests/receiver_identity.rs` and `tests/source_facade_precedence.rs` pass; the
60 usage unit tests pass as well. The combined focused-v8 gate reports 299
compile errors, four translator error-status files and one syntax error.

The follow-up batch preserves raw source generic erasure inside collection and
optional arguments, and carries Java type arguments through facts-only trait
slot policies, reusing the existing parameterized trait renderer. JDK abstract
list hierarchy/field facts now expose the protected integer modification count
to existing inherited-member storage. Usage analysis and lowering share those
field identities so mutation affects receiver capability as well as the emitted
field type. `javap -protected java.util.AbstractList java.util.AbstractSequentialList`
was used to verify the local JDK field and inheritance declarations. A neutral
generated-code runtime test checks inherited prefix/postfix and overflow results.
The combined focused-v9 gate reports 260 compile errors, four translator
error-status files and zero generated Rust syntax errors. This does not prove
type correctness: ownership/dispatch, constructor inheritance, generic class
bounds and unsupported platform calls remain substantial work.

The next inference batch preserves diamond construction separately from raw
construction in the Java IR. It reuses selected-constructor type constraints and
common-type joins, then lets target typing refine nested diamond arguments from
the original declaration parameters rather than an already-specialized copy.
Type and shape inference now consume the resolved constructed type. The three
neutral tests in `tests/diamond_inference.rs` pass, including a generated-crate
check for an inline factory's generic interface result. The existing nested
target-typed diamond regression passes. These changes postdate focused-v9; the
sixteen same-named boxed-trait conversion errors were eliminated by the
focused-v10 gate. That gate reports 245 compile errors, four translator
error-status files and zero syntax errors; translation took 225.524 seconds
and assembly 63.061 seconds.
The ten resolver, 27 expression-shape, 26 slot-render and 62
representation-lowering unit tests pass on this batch. The Java behavior oracle
also still passes. The focused-v10 evidence is recorded under
`.tmp/jgrapht-core-1.5.3-focused-v10`.

A stronger fixture that stores an interface-typed constructor argument exposed
a distinct ownership/dispatch gap: the constructor parameter became `Any` while
its destination field retained a typed trait object. The storage-contract batch
replaces separate field/parameter workarounds with one declaration-aware policy
after inferred and fact-derived constraints have been combined. Retained source
interface parameters, fields and returns use the same typed escaping carrier;
borrowed parameters retain borrowed dispatch. Explicit user erasure overrides
and actual `Object` boundaries remain erased.

Accessor inlining additionally exposed unspecialized generic field shapes.
Field shape lookup now substitutes declared owner parameter names, preserving
solver-owned nullability through the shared specialization utility. Accessor
result classification uses the resolver-specialized return type with that same
utility. It does not accept unequal generic instantiations merely because their
trait names match.

All three `constructor_interface_storage` regressions pass, including generated
Rust compilation and an executable constructor/accessor probe returning seven.
The diamond, nested-interface and source/facade suites also pass (ten focused
integration tests total), as do all 120 signature, 27 expression-shape and 26
slot-rendering unit tests. The five object-model suite failures listed below
remain open and unchanged in this batch. A complete library-unit run passed
1,353 tests and failed one stream-lambda fixture that seeded only the old
normalized type alias. The fixture now seeds the preserved Java spelling too,
as production scope seeding does; its output assertions are unchanged and the
focused rerun passes. No full-suite rerun after that fixture-only correction is
claimed.

The completed focused-v12 corpus gate reports 234 compiler-error lines
(233 errors plus Cargo's summary), down from 240 in focused-v11. Five erased
`Graph` recoveries, one erased `GraphPath` recovery and one `Graph -> Any`
coercion disappeared. A `ListNodeIterator -> NodeIterator` coercion now appears
as a typed dispatch gap instead of being obscured by erasure. Remaining generic
instantiation mismatches are still rejected. Translator-failure files remain
four; parse failures remain zero; invalid-pattern files fall from 48 to 46.
Translation took 245.342 seconds, assembly 64.869 seconds and `cargo check`
6.516 seconds. This correctness gate overlapped library-unit verification, so
its timings are not an isolated performance comparison.

The following construction batch addresses allocation/initialization ordering
across flattened superclass constructors and self-dependent field initializers.
The focused-v12 traversal constructors demonstrate superclass statement prefixes
reading or writing fields before `__java_self` exists (30 reported errors).
The planner now composes explicit and implicit ancestor execution phases,
allocates default storage before field-dependent statements, and replays each
class's field initializers after superclass execution and before its own body.
Copied statements preserve lexical ownership for hidden fields/private calls.
Initializers use class scope, outside constructor parameter/local bindings.

Super-constructor arguments are evaluated eagerly when nontrivial, including
unused arguments, and nested argument temporaries use the shared collision-safe
name allocator. Pure local prefixes remain eligible for the compact path;
field-dependent locals do not. The resolver also visits ordinary field
initializers, sharing call resolution and diamond target typing with local
declarations instead of leaving their source calls as unresolved placeholders.

Both `constructor_initialization_order` integration tests pass. Their generated
crate runs five behavior checks: explicit three-level control flow followed by
child initialization, implicit three-level initialization, virtual dispatch
observing child default state, constructor-parameter shadowing of initializer
field names, argument side-effect order (including unused arguments), and nested
temporary isolation. All 294 resolver tests, the existing multilevel virtual
dispatch test, and all three reused/flattened-argument tests pass. Fixture
assertions that assumed inline enum operands or an older fallible ABI were
updated while retaining or adding executable Java-value checks; the loop
regression now requires its constructor argument to be evaluated exactly once.

The first established `source_super_constructor` subset check passed 12 of 19
tests. Some assertions require the former struct-literal layout. Those being
updated gain executable guard/store checks, not waived failures. Two generated
compilation failures are independently reproduced with the pre-batch focused-v12
binary (`.tmp/new-java2rust-before-constructor-order`, SHA-256
`3370ed43ff701a03c0827fe839a033366567ed5d3c92bf9599e859d776d1cb67`): missing
context enums `CountKind`/`ValueType`, and leaked superclass parameter `FUNC` in
static constant paths. Inputs and compiler evidence live under
`.tmp/constructor-baseline/{header,generic}`. They remain required correctness
work, not successful generated-crate checks. The final subset run passes 15 of
19 tests: the two baseline compilation failures above and two remaining
context/`Number` ABI/layout assertions remain open
(`implicit_source_super_constructor_initializes_inherited_fields` and
`source_super_constructor_preserves_trailing_method_result_call`). Their broader
attribution is not established. The entire library-unit suite passes all 1,354
tests on the construction/resolver batch. Upgraded guard and conditional-store
fixtures execute both success/throw and both branch cases, respectively.

The completed focused-v13 gate reports 205 compiler-error lines (204 actual
errors plus Cargo's summary), down from 234. `self`-without-receiver diagnostics
fall from 30 to two; the two remaining cases are captured-outer field reads in
inner-class initializers in `DoublyLinkedList`, not superclass body prefixes.
Unsupported-super diagnostics fall from seven to six. Four translator-failure
files and zero parse failures remain; invalid-pattern files fall from 46 to 45.
Translation takes 225.938 seconds, assembly 64.385 seconds and `cargo check`
7.104 seconds. This run overlaps unit/integration verification and is not an
isolated performance comparison. Generated Rust still does not compile.
General inherited self-escape/alias identity still requires verification; the
primitive-state phase tests alone do not establish that broader property.

The lexical-identity follow-up made all 294 `resolve_calls` tests pass, including
the earlier static-field/type collision and imported nested constructor/sibling
functional-interface cases. Constant/type-like declarations and expressions use
one parser naming policy. Exact Java field lookup is separate from compatibility
aliases. Constructor member types use lexical owner context; source-index
extraction retains the nested Java owner. A new scope regression additionally
requires constructor type parameters to shadow sibling classes during early
qualification. All three `source_name_identity` regressions, all three diamond
regressions, all 294 resolver integration tests and the source-summary shadowing
unit test pass on the final batch. Parameter mutation shadows reuse the same
source-binding name map as declarations and reads. These fixes postdate
focused-v10. The completed focused-v11 gate reports 240 compiler-error lines
(including Cargo's summary), four translator-failure files, zero parse failures,
and invalid patterns in 48 files. Its recorded timings are 238.371 seconds for
translation, 65.112 seconds for assembly, and 7.143 seconds for `cargo check`.
These results precede the constructor/interface storage-contract batch.

The parser suite reported 51 passes and one open failure:
`all_fixtures_parse_without_reader_fallback_nodes` encounters the explicitly
unsupported non-this synchronized monitor in
`examples/java/06-stdlib/SynchronizedBlock.java`. It is not an accepted baseline
exclusion; monitor support or clean-baseline attribution remains required.

## Outer initializer and marker-interface follow-up

The completed focused-v14 run reports 191 compiler-error lines (190 actual
errors plus Cargo's summary), down from 205 in focused-v13. Four translator
failure files, zero parse failures, and 45 invalid-pattern files remain.
Translation takes 212.337 seconds, assembly 59.256 seconds, and generated-crate
checking 5.693 seconds. These overlapping development runs are correctness
measurements, not isolated performance benchmarks. The generated crate still
does not compile, and placeholder counts remain a separate required gate.

Outer field initializer lowering now reuses inherited source/platform field
shapes and constructor initializer scopes. Implicit inner constructors use the
existing allocate-before-self-dependent-initialization path. A shared lexical
outer-field binding rule respects inner members, local shadowing, and Java's
case-sensitive field identity. Source interfaces extending platform marker
interfaces retain their source trait ABI instead of being treated as concrete
platform-facade subclasses. Neither fix depends on JGraphT names.

The focused constructor-order, outer-initializer, facade-precedence, and source
name suites pass all nine integration tests, including generated runtime checks
for inherited outer state, constructor-parameter shadowing, implicit constructors,
inner member hiding, case-distinct field names, and marker-interface dispatch.
The library suite passes all 1,354 tests on this batch. The established inner
subset then passes 17 of 19 tests: the strengthened outer-method identity probe
exposes accidental recursion, and the generic-inner construction test exposes
declaration parameters leaking into an instantiated constructor call. The latter
also fails with the saved pre-constructor-batch translator binary; the minimized
input and compiler evidence are under `.tmp/constructor-baseline/inner-generic`.
These failures are being addressed, not waived. The subsequent construction-site
generic inference fix postdates focused-v14. Its strengthened regression executes
the generated constructor and checks both the instantiated string payload and
the captured outer object's identity.

The outer-method probe identified another general receiver rule: making an
implicit receiver explicit must respect the resolved declaring owner before
consulting signature base-name fallbacks. Otherwise an enclosing `make()` can
be rewritten as inner `this.make()` because an inner `makeViaOuter()` shares
its Rust-name prefix, and fallback lowering turns that into recursion. The
follow-up consults the existing hierarchy dispatch rule before introducing
`this`, uses the preferred emitted declaration lookup for companion targets,
and does not fall back to an outer member when resolution selected an inner
member. The runtime probe now checks both live outer identity and inner-member
precedence. The final established inner subset passes all 19 tests, including
the strengthened generated-runtime probes. All nine constructor-order,
outer-initializer, facade-precedence, and source-name integration tests also
pass on the follow-up. Its full library-unit gate passes all 1,354 tests
(193.37 seconds). Formatting and whitespace checks pass, and the pinned Java
checkout remains unchanged. These results do not replace a subsequent full
corpus or translated-Rust behavior gate.

The subsequent representation batch addresses nested generic payloads:
focused-v14 emits `Vec<Graph<V, E>>` inside a map field although adjacent slots
already use `Box<dyn Graph<V, E>>`. The representation-aware generic-argument
renderer eventually falls back to the facts-only renderer for a nested native
collection, losing representation information inside that collection. The
shared recursive rendering boundary was the first fix location; no emitted
Rust was patched. Functional-facade generic payloads and summary return
slots also contribute to the 20 trait-as-type diagnostics and need separate
minimized checks before being grouped under the same root cause.

The neutral rendering regression first failed with
`List<List<Port<String>>>` rendered as `Vec<Vec<Port<String>>>`. Recursive
generic arguments now re-enter the existing representation-aware carrier
renderer instead of falling back to facts-only recursion. Shared generic
payloads retain that plan inside their existing storage wrapper. Nominal
generic facades preserve representation-aware arguments too; native mappings
and explicit dispatch still take precedence. Standard map entries reuse one
representation-aware payload projection for both tuple and `JavaMapEntryValue`
carriers, while a parsed source class named `Entry` stays a source class.
Explicit user overrides of the platform-entry representation also take
precedence over its default tuple/value carrier, with a dedicated policy check.

The slot suite caught an ordering regression in that broader nominal fallback:
direct `Optional<List<Port<String>>>` rendering lost its `Option` carrier.
The canonical renderer now reuses the existing optional-element projection
before nominal fallback. All 29 slot-rendering tests pass on the corrected
batch. The new generated-crate test also passes before that final optional
ordering correction, covering nested maps/lists, reference arrays, nominal
wrappers, source-versus-platform entries, a functional-facade signature, and
Java-default null container fields. Focused-v15 was launched before the final
optional correction was verified and is intermediate evidence, not an exact
identification of the final source snapshot.

The final explicit-entry policy extension passes all 29 rendering tests,
including the user-override case. Focused-v15 emission already confirms the
original field defect is corrected: `vertex2blocks` now contains
`Vec<Box<dyn Graph<V, E>>>`. The completed run reports 186 compiler-error lines
(185 actual errors plus Cargo's summary), down from 191 in focused-v14. The
trait-as-type diagnostics fall from 20 to 15. Four translator failure files and
zero parse failures remain. Invalid-pattern occurrences increase from 112 to
113, across 46 rather than 45 files: `DijkstraClosestFirstIterator` now emits an
unsupported erased-object clone helper. That fallback remains required work,
not an accepted exclusion. Translation takes 522.039 seconds, assembly 132.291
seconds and checking 13.182 seconds. This gate overlaps heavy test compilation
and execution, so it is not an isolated performance comparison.

The full gate queued before the final entry-override extension also does
not, by itself, prove that final source snapshot's complete unit suite. That
pre-extension library gate passes all 1,357 tests (254.07 seconds); its
constructor-interface, new generated-crate, and facade checks pass all seven
integration tests. The final entry-override extension is additionally covered
by the 29 passing rendering tests.

The four translator-status errors in focused-v14 are resolution failures:
an unknown `vertexSet()` result passed to an overloaded constructor, a static
generic builder overload receiving `Supplier<E>`, and ambiguous constructor
choices for a captured inner class receiving `Collection<V>` and for
`this(new ArrayList<>(...))` delegation. They are candidates for a subsequent
instantiated-signature/constructor-specificity batch, but must be minimized
separately before assuming they share one cause. In particular, a captured
outer type parameter must not be inferred afresh from an inner constructor's
argument as if it were an independent method type parameter.

## Instantiated constructor overloads and specificity

Neutral tests reproduce four of the constructor/method overload failures:
an explicit `Choice<String>` incorrectly re-infers its class parameter per
candidate; an inner constructor re-infers a captured outer parameter;
`this(new ArrayList<>(...))` cannot choose between `List<T>` and `Collection<T>`;
and the equivalent method overload also remains ambiguous. A control checks
that a constructor's own type parameter can shadow a class parameter.

The current batch seeds constructor matching from supplied owner arguments,
retains instantiated `this`/`super` owner types, fixes capture qualification
across multiple nested owners, and shares strict parameter-list specificity
between methods and constructors when conversion costs tie. Unknown argument
types are not evidence that permits selecting a narrower overload. Seven
resolution checks pass, including preserved ambiguities, but the generated
runtime probe exposed a second issue: a fixed scoped `T` was still treated as
an unconstrained wildcard during conversion scoring, selecting `Choice(T)`
for an `ArrayList<T>` argument. The batch now reuses scoped type-bound data to
validate fixed-variable constraints before ranking, including Java's implicit
`Object` upper bound. Additional checks cover upper-bound chains, wildcard
variance, and rejection of incompatible invariant arguments. All ten new tests
pass, including a generated-Rust runtime probe selecting the list constructor
through collection delegation. The same neutral Java program prints `1` with
`javac`/`java` (local oracle under `.tmp/constructor-overload-oracle`); the Rust
probe also returns `1`. All 294 resolver integration tests, all three diamond
tests and all three source-name identity tests pass: 310 focused integration
tests in total. The full library-unit gate on this constructor batch passes
all 1,357 tests (216.19 seconds). No generated Rust has been repaired.

## Generic method binding follow-up

Four further neutral failures isolate the related method-side gap: receiver
arguments overwrite a same-named method binder, including the inferred result;
mixed-case declared method parameters are not inferred before applicability;
and a generic factory overload rejects `Supplier<E>` for that reason. The
existing callable-binder normalization intentionally produces names such as
`EMethod` to avoid Rust generic-name collisions. Focused-v15's factory methods
have exactly those normalized binders; relying on an all-uppercase-name
heuristic cannot resolve their generic argument contracts.

Methods and constructors now share owner/member argument binding, excluding
shadowed class binders. Method inference is applied before candidate scoring
and reused for the selected signature. Inferred arguments must satisfy declared
bounds, using the same scoped-bound constraint check as constructors. Unknown
calls, null, and un-targeted lambdas/references are not concrete inference
evidence. Non-generic methods avoid the otherwise redundant inference walk.
The existing callable-binder renaming pass is reused unchanged.

All eleven method-binding tests pass, including a generated-Rust runtime check
of a method binder shadowing the receiver's integer binder: Java and Rust both
produce string length `4` (Java oracle under `.tmp/generic-method-oracle`).
Controls cover exact non-generic overload precedence, lambda target selection,
declared bounds, and caller variables with proven versus unproven numeric
bounds. The combined method, constructor, resolver, diamond, and source-name
gate passes all 321 integration tests. The subsequent full library gate passes
all 1,357 tests (181.32 seconds; `.tmp/generic-method-binding-lib-tests.log`).
Focused-v16 then runs sequentially, without that test contention. It has one
translator error (down from four), 184 actual Rust compiler errors (the runner's
185 includes Cargo's final summary), no parse failures, and invalid patterns in
46 files. The remaining translator error is the ambiguous
`VertexToIntegerMapping` constructor in `Graphs.java`, where
`Objects.requireNonNull(...).vertexSet()` loses the source receiver type.
Translation takes 198.321 seconds, assembly 58.303 seconds, and generated-crate
checking 5.376 seconds; total runner time is 387.777 seconds. Artifacts are under
`.tmp/jgrapht-core-1.5.3-focused-v16`, with runner log
`.tmp/jgrapht-v16-run.log`. This is not a completed compile or behavior gate.

The next neutral regression batch isolates checked generic identity calls.
Before fixes, four of five resolver controls fail; the source-defined `Objects`
precedence control passes. Reusing native call contracts fixes resolution of
source receivers and `Objects.nonNull`'s boolean result, while retaining a real
Java `Optional<T>` as the complete semantic type. A generated runtime regression
then exposes a separate carrier-projection gap for nullable method results.
The completed six-test batch passes (`.tmp/checked-identity-tests-v5.log`).
Its generated Rust and Java oracle both produce `71, 12, 1, 127, 0, 1, 0, 1`:
source-method dispatch and boolean overload selection; message evaluation before
a caught null failure; the one-argument null check; message evaluation even for
a present reference; empty, present inline, and empty-with-message semantic
Optionals; and mutation through the returned reference. Java oracle sources are
under `.tmp/checked-identity-oracle`; maintained neutral regressions are in
`tests/checked_identity_resolution.rs`.

Resolution records the generic identity's semantic return type without inventing
a non-null argument ABI. Lowering retains actual argument carriers and marks
extra nullable carriers explicitly, stages value/message evaluation, then uses
the existing catchable-null projection. Semantic `Optional` values remain whole.
Optional instance planning now recognizes inferred return types on chained
producers, reusing its existing native contract instead of falling into generic
nullable facade adaptation. Source definitions still outrank native utility
models. The broader related integration gate passes all 327 tests
(`.tmp/checked-identity-batch-tests.log`). One of the two established identity
idiom checks then detects an unused unmodelled `Objects.requireNonNull` facade
stub: actual calls already lower natively, but the resolved-static inventory
still requests support methods. The former Math-only support filter now reuses
the shared native utility planner, with source-summary owners explicitly
preserved. Focused checks, a runtime source-owner precedence control, the idiom
checks, and the full library gate are being verified sequentially on that
follow-up. That follow-up passes all seven checked-identity tests
(`.tmp/checked-identity-tests-v6.log`), both established idiom checks
(`.tmp/checked-identity-idiom-tests-v6.log`), and all 1,357 library tests
(170.65 seconds; `.tmp/checked-identity-lib-tests-v6.log`). The runtime
source-owner control verifies a custom source `Objects.requireNonNull` body is
executed instead of the native identity. A fresh focused-v17 corpus run is now
running after those gates, not concurrently with them; its results are pending.
The maintained Java graph oracle also passes again
(`.tmp/jgrapht-java-oracle-v17.log`); the Rust-versus-Java graph comparison is
still pending generated-crate compilation.

### Focused-v17 checked-identity corpus gate

Focused-v17 completes with zero translator failures and no `Error` diagnostics
in `translation-logs/batch.stderr`, down from the one constructor ambiguity in
focused-v16. All 104 production sources emit Rust, with no parse failures.
The assembled crate still fails: 181 actual compiler errors (182 runner lines
including Cargo's summary), versus 184 previously. Invalid patterns increase
from 114 occurrences in 46 files to 128 occurrences in 47 files. All fourteen
additional occurrences are erased-object clone fallback helpers in seven graph
implementations; `AbstractBaseGraph` is the newly affected file. These are open
failures, not waived support code. The diagnostics explicitly erase `GraphType`
because `asDirected()` returns that interface, motivating the next general
interface-value batch.

Artifacts: `.tmp/jgrapht-core-1.5.3-focused-v17`, runner log
`.tmp/jgrapht-v17-run.log`. Build 95.491 seconds, resolver preflight 37.538,
translation 210.301, assembly 62.352, generated-crate check 5.426, total 414.366.
The run uses the checked-identity batch's built binary; interface-value changes
were prepared only after that build completed and are not included in v17.

### Named interface values and trait-signature representation

Three minimized neutral semantic tests fail before the next batch: arbitrary
`Link next()` returns, nested interface returns (`List<Link<T>>`, `Link<T>[]`),
and nested interface parameters are incorrectly classified as Rust `Self`.
The old policy has method-name exceptions for copies and combinators. Named Java
interfaces instead denote reference values, rendered through their boxed/shared
trait carriers. The new rule removes those name exceptions while retaining
explicit external/user object-safety facts. Trait signature rendering now
requires the existing representation plan rather than optionally dropping it;
declarations, forwarding adapters, defaults, implementations, and support
surfaces reuse that same renderer. Tests whose old expectation was unconditional
erasure of `Factory next()` are updated to assert its dynamic reference contract,
with a new generated-runtime dispatch probe as independent behavior evidence.
All four new interface-value tests pass, including generated Rust matching the
Java oracle's dynamic-dispatch result `54` and null default for a nested-interface
container. Java oracle: `.tmp/interface-value-oracle/Probe.java`. The existing
nested-generic representation generated-crate regression also passes
(`.tmp/interface-value-batch-tests-v2.log`, five tests total). These tests verify
ordinary returns, nested generic/array returns and parameters, method-name
independence, dynamic dispatch, and trait/forwarder signature compilation. Broader
object-model and library gates are still pending; no claim is made that the
corpus clone fallbacks have disappeared until it is rerun on this batch.

A stronger `Ref same() { return this; }` regression initially fails generated
compilation with `Box<&MutableCell>`. The existing companion is present but
delegates to the invalid borrowed entry point because both lowerings produce
the same body. Owned interface slots now reuse the companion's canonical
receiver handle, as concrete and hierarchy slots already do, then use the
existing parameterized trait boxing projection. The existing body-difference
rule consequently removes the invalid borrowed entry point. Erased Object
slots also preserve that handle rather than cloning the borrowed payload;
nullable polymorphic slots retain their outer carrier. No separate method-name
or class-specific forwarding policy is introduced.

The extended runtime probe passes dynamic dispatch (`54`), mutation through the
returned interface alias (`5`), and pointer identity through an erased Object
return, matching Java (`54, 5, true, true`, including the null-default control).
The seven checked-identity controls remain green in the final focused gate
(`.tmp/interface-value-final-focused-tests.log`). The object-model suite remains
at 28 passing and the same five previously recorded failures
(`.tmp/interface-value-object-model-tests.log`); their baseline attribution
remains open, as recorded below. All twelve final focused tests pass (seven
checked-identity, four interface-value, one nested-generic). Full library
verification also passes all 1,357 tests (125.84 seconds;
`.tmp/interface-value-lib-tests.log`). Focused-v18 is running sequentially after
that gate to measure the interface batch.

### Focused-v18 interface-value corpus gate

Focused-v18 completes with zero translator failures, zero parse failures, and
168 actual Rust compiler errors (169 runner lines including Cargo's summary),
down from 181. Twelve of the thirteen bare-trait type errors disappear. The
remaining one is a source-summary `GraphTests.requireUndirected` facade in the
shared support, despite `GraphTests.java` being in the emitted source closure;
this is still an open source-owner/support-surface issue, not an accepted stub.
Factory contracts now contain `BiFunction<Box<dyn Graph<V,E>>, Box<dyn GraphType>,
Box<dyn Specifics<V,E>>>`, and `GraphType` is emitted as a trait with boxed
interface return values rather than an erased alias.

Invalid patterns remain unchanged at 128 occurrences in 47 files. In particular,
the erased-object clone fallbacks are **not** eliminated by this batch. No
behavior-comparison completion is claimed. Unmodelled type inventory drops from
562 to 452, while unmodelled call sites remain 260; no-op facade methods are 571
and marked no-op call sites remain zero. These inventory counts are not
independent bug counts or substitutes for the invalid-pattern gate.

Artifacts: `.tmp/jgrapht-core-1.5.3-focused-v18`, log
`.tmp/jgrapht-v18-run.log`. Build 85.740 seconds, resolver preflight 35.463,
translation 195.144, assembly 58.218, cargo check 5.064, total 382.769. The pinned
Java source checkout is still tracked-clean.

The next selected batch is formatted output: fourteen missing `Out` type errors
come from `System.out.printf`, while four unused-format-argument errors share a
literal-brace/conversion bug in the Java-to-Rust formatter (for example Java
`"R {%s}"` incorrectly becomes Rust `"R {{}}"`). Neutral regressions in
`tests/formatted_output.rs` exercise stdout/stderr stream values, zero through
six formatting arguments, returned-stream chaining and output-buffer identity,
literal braces independently of conversions, and a source-defined PrintStream
owner. This batch should reuse the existing print-stream carrier and Java string
formatting machinery, not add a corpus-specific printf path. All four new
regressions fail at baseline (`.tmp/formatted-output-baseline-tests.log`): the
stdout call becomes `Out::printf`; brace-adjacent conversions fail Rust's format
arity check; buffered output exposes arity-suffixed printf calls plus missing
ByteArrayOutputStream-to-PrintStream/display adapters; and a source-defined
PrintStream collides with the monolithic platform-support declaration. The
existing ByteArrayOutputStream already retains an OutputStream and has the
appropriate owned conversion, so its sink identity can be reused. Platform
support selection must respect source definitions before injecting native
declarations.

The formatted-output batch adds one native PrintStream format contract for
`printf`/`format`, including owned stream return typing. Lowering evaluates and
retains the stream carrier, reuses the existing Java string-format projection,
writes through the runtime stream, then returns that same carrier. Literal Java
braces are escaped independently from generated Rust conversions. Dynamic
templates reuse `java_string_format`; no separate printf formatter is added.
ByteArrayOutputStream gains the missing PrintStream conversion through its
existing OutputStream and a Display adapter delegating to its existing Java
text conversion. The existing source-platform-support suppression policy now
also recognizes its stream-type family; native format selection respects
parsed source owners.

After those changes, three tests pass but the source-owner case returns `3`
instead of Java's `4`: its own printf body executes, but returning `this` copies
the object. Storage analysis now distinguishes returning a bare existing handle
from returning a fresh constructor value, reusing its existing alias/type proof.
The canonical-handle companion machinery then preserves the receiver identity
without any PrintStream-specific storage rule. Two neutral storage tests verify
the shared mutable receiver and fresh-value control.

All four formatted-output tests pass, including dynamic `format` templates,
zero through six scalar formatting arguments, stdout/stderr calls, buffer
identity and source-owner precedence (`.tmp/formatted-output-tests-v3.log`). The
four interface-value tests also pass (`.tmp/formatted-output-tests-v2.log`), as do
all 58 storage strategy unit tests (`.tmp/formatted-output-storage-tests.log`).
Java oracle: `.tmp/formatted-output-oracle/Probe.java`. All four established
string-format idiom checks pass (`.tmp/formatted-output-idiom-tests.log`). The
first full library run passes 1,358 tests and finds one exact signature snapshot
whose returned source handle now correctly uses `Rc<Vec<ExternalId>>` around its
user-overridden content. The snapshot retains that precise content/dispatch
assertion with its new shared carrier. The runtime control is also strengthened
to return an existing parameter as well as `this`, and passes all four formatted
output tests again (`.tmp/formatted-output-tests-v4.log`). The final full library
gate passes all 1,359 tests (127.10 seconds;
`.tmp/formatted-output-lib-tests-v2.log`). Focused-v19 is now running sequentially
after those gates.

### Focused-v19 formatted-output corpus gate

Focused-v19 completes with zero translator failures, zero parse failures, and
151 actual Rust errors (152 runner lines including Cargo's summary), down from
168. All fourteen missing `Out` errors and all four unused-format-argument
errors disappear. Invalid patterns remain 128 occurrences in 47 files, so the
invalid-placeholder and behavior gates remain open. The pinned source checkout
is tracked-clean. Artifacts: `.tmp/jgrapht-core-1.5.3-focused-v19`; log:
`.tmp/jgrapht-v19-run.log`. Build 85.609 seconds, resolver preflight 36.103,
translation 199.228, assembly 58.315, cargo check 4.716, total 387.068.

The new alias evidence also exposes three return-lifetime failures in canonical
handle companions: a read-only generic getter's borrowed result is copied into
an owning-handle function signature (`Option<&G>`, `Option<&E>`, `Option<&V>`).
A neutral regression in `tests/shared_receiver_returns.rs` reproduces the
failure using read-only and mutable generic holders, without any graph names.
The existing return-materialization policy covered only shared interior-mutable
owners. It now covers every shared instance receiver, including Rc/Arc without
cells, and the borrowed-string shortcut cannot bypass that rule. Static methods
are not forced through the instance-receiver rule. The follow-up focused run
kept all four formatted-output and four interface-value tests green
(`.tmp/shared-receiver-returns-tests.log`). It also exposed pre-existing generic
setter clones without `T: Clone`, present in the original baseline too. A
distinctly named setter parameter reproduced the same failure, excluding a
parameter/field spelling collision as its cause.

The nullable-value reuse path was treating a callable-wide field escape as a
second use even when the owned argument was dead after that statement. It now
consults the existing statement-liveness analysis first and retains the old
fallback only without statement facts. Last-use generic parameters can then use
the existing present-value construction path without an unnecessary clone.
An inherited neutral getter independently reproduced the owning-handle lifetime
error: its constraints were solved against the declaring base's placement.
Inherited signatures now reuse the shared-return materialization policy for the
actual emitted subclass while retaining declaration-owned constraints.

All nine focused tests pass in `.tmp/shared-receiver-returns-tests-v2.log`,
including generated-Rust execution for read-only and mutable generic getters,
inherited getters, shared mutation, and a string argument read after assignment
(which must not be moved prematurely). A Java probe in
`.tmp/shared-receiver-java-oracle/Probe.java` independently prints the expected
`3, 7, 9, 8` (recorded in its `output.txt`). The strengthened fixture also
passes setters instantiated with a non-Clone Rust payload, proving that the
fix does not merely impose another Clone bound. The ownership follow-up passes
all five integration tests, including existing receiver and shared-place
regressions (`.tmp/shared-receiver-ownership-tests.log`). Full unit verification
passes all 1,359 tests in 135.10 seconds
(`.tmp/shared-receiver-lib-tests.log`); formatting checks pass.
Focused-v20 completes with zero translator failures, zero parse failures, and
148 actual Rust errors (149 runner lines including Cargo's summary), down from
151. All three owning-handle return-lifetime errors disappear: the inherited
builder and flyweight-event companions now return `Option<G>`, `Option<E>`, and
`Option<V>`, respectively. Invalid patterns remain 128 occurrences in 47 files.
The pinned Java checkout remains tracked-clean. Artifacts:
`.tmp/jgrapht-core-1.5.3-focused-v20`; log `.tmp/jgrapht-v20-run.log`.
Build 109.584 seconds, resolver 35.342, translation 197.303, assembly 62.903,
cargo check 4.723, total 413.034. Build overlapped other verification, so its
wall time is not an isolated performance comparison.

The next coherent batch addresses stream collector planning: three-argument
joining, native collection-constructor suppliers, eager collector-argument
evaluation versus lazy traversal, and physical item ownership at stream
boundaries. The new neutral `tests/stream_collectors.rs` fixtures initially all
fail (`.tmp/stream-collectors-baseline.log`). The independent Java oracle in
`.tmp/stream-collectors-java-oracle/Probe.java` records `ab`, `a,b`, `[a,b]`, `[]`,
`a,b,tail`, `3`, `[x,y]:12344`, and `x,y:122` in `output.txt`.

Collector classification is consolidated into one overload-aware planner.
Native constructor suppliers reuse existing list/set kinds and terminal
conversion; arbitrary suppliers and ordered-set collectors are not claimed as
implemented. Declaration identity and imported aliases take precedence over
spelling, using the same parsed-source owner guard as native formatted output.
Qualified native owners reuse the existing Java owner-path recognizer, with
local/field roots rejected. Two neutral owner tests verify that a source
`Collectors` class or source `ArrayList` subclass is not silently replaced by a
native collector (`.tmp/stream-collectors-owner-tests.log`).

Three-argument joining stages delimiter/prefix/suffix in Java evaluation order
before lazy mapper execution; nonliteral one-argument joining uses the same
path. A structured match retains temporary source lifetimes. Literal zero/one
argument joins retain the existing projection. Context-aware signature shapes
avoid adding `String::as_str` to an already borrowed source-method result.
Borrowed mapper results are materialized through ordinary owned-slot
conversions before escaping the closure. Initial item ownership is normalized
from the physical iterator using `effective_stream_item_shape`, preventing an
owned temporary set collector from acquiring an invalid `.cloned()`.

All seven focused formatting/collector tests pass on the final product edits
(`.tmp/stream-collectors-focused-final.log`). The collector regressions execute
all joining overloads, empty-stream affixes, fully qualified native calls,
mutable list and set results, and one/three-argument evaluation order against
the Java results above. Existing nullable-joining and nullable stream-element
regressions both pass (`.tmp/stream-collectors-idiom-joining.log` and
`.tmp/stream-collectors-idiom-nullable.log`). Full unit verification passes all
1,361 tests in 131.20 seconds (`.tmp/stream-collectors-lib-tests.log`). Formatting
and diff-whitespace checks pass.

Focused-v21 completes with zero translator failures and zero parse failures.
There are 145 actual Rust errors (146 runner lines including Cargo's summary),
down from 148. Missing `Collectors` errors fall from eight to six and the
remaining missing `ArrayList` error disappears. Invalid patterns remain 128
occurrences across 47 files. The pinned checkout remains tracked-clean.
Artifacts: `.tmp/jgrapht-core-1.5.3-focused-v21`; log `.tmp/jgrapht-v21-run.log`.
Build 89.954 seconds, resolver 36.020, translation 200.910, assembly 58.212,
cargo check 4.635, total 392.885.

The next batch targets the eight unresolved bound-local `g::method` paths and
the related primitive terminal types. `tests/stream_bound_references.rs` first
reproduces the same pattern with neutral source/native receiver names
(`.tmp/stream-bound-references-baseline.log`). The existing ordinary callback
binding classifier is moved to shared method-reference planning and reused by
stream callbacks, which expand bound references through normal instance-call
argument lowering instead of guessing a type owner from the spelling.
Numeric stream plans retain int/long/double targets, use ordinary slot
conversion for mapper-result widening, and integer sum uses wrapping addition.
Source callback fallibility is selected from the same source signature used for
its return ABI; sum can sequence fallible items through the existing Result
collection path. The initial four numeric/native cases pass in
`.tmp/stream-bound-references-tests-v1.log`.

The expanded neutral fixture now also checks a named package, overloaded source
methods (using the selected declaration's emitted name), generic-interface
dispatch, a captured local named `value`, long-result widening, int overflow,
and caught callback exceptions. Synthesized callback parameters use the existing
Rust identifier allocator so they cannot hide their bound receiver. Nullable
receivers are checked and bound before traversal, with callbacks lowered in a
refined environment and moved into the iterator. Filter and any/all terminals
share this capture projection; empty-stream null receivers are not deferred
until a callback invocation. Captured immutable strings use the existing
string-view conversion rather than mixing `&String` and owned-string ABIs.

A cast-wrapped count exposed two reference-result shortcuts which also accepted
primitive casts and bypassed complete operand lowering/narrowing. Those
shortcuts now retain ordinary primitive-cast semantics. When the operand is a
modeled stream, the cast planner uses the existing `LoweredExpr`'s actual type
and consumes its emitted expression once, rather than trusting a provisional
target-typed call shape. Native stream `count()` return metadata is explicitly
long. Numeric cast policy itself remains the shared cast-conversion planner.

All five bound-reference/collector integration tests pass on these final edits
(`.tmp/stream-bound-references-tests-v11.log`). The independent Java probe in
`.tmp/stream-bound-java-oracle/Probe.java` records `3`, `0.75`, `6000000003`, `2`,
`3`, `3`, `-2`, `-1.0`, `0.75`, `7`, `7` in `output.txt`. These tests do not
establish all mutable-field receiver snapshot or escaping-callback semantics.
The final revision passes all 1,361 unit tests in 150.46 seconds
(`.tmp/stream-bound-lib-tests-final.log`) and all three existing explicit/
boxed primitive-cast regressions (`.tmp/stream-bound-primitive-cast-gate.log`).
The final method-reference gate again passes eight tests and reports the same
Spark alias-cycle failure (`.tmp/stream-bound-method-reference-gate-final.log`).
Formatting and diff-whitespace checks pass. The full unit run overlapped other
verification, so its duration is not an isolated performance measurement.

The first broader method-reference gate passes eight tests and fails
`disq_reads_rdd_get_reads_shapes_spark_method_reference_filter_param` with a
two-alias cycle (`.tmp/stream-bound-method-reference-gate.log`). The unchanged
pre-batch focused-v21 executable (SHA-256
`87ca0ebc8293986b029a5413feb8a56a885c5a0fa8d94c0ba34529322f2c623a`) reproduces
the same aliases and E0391 on the identical Java input. Evidence lives in
`.tmp/disq-method-reference-baseline/Probe.java`, `before.rs`, and `check.log`.
This establishes that the failure predates this batch, not that it predates the
whole JGraphT goal; it remains open and is not a goal-wide accepted exclusion.

Focused-v22 completed (`.tmp/jgrapht-v22-run.log`): 133 actual Rust compiler
errors, down from 145 in focused-v21. The runner reports 134 because its count
also includes Cargo's final error-summary line. All 104 files translated with
zero parse failures. Registry-invalid placeholders remain at 128 occurrences
across 47 files. Unresolved-module errors for bound receivers `g`, `graph` and
`edgeSet` disappeared. Translation took 206.671 seconds and assembly 59.830
seconds; the 460.250-second end-to-end run includes a release build overlapping
other verification and is not an isolated performance measurement.

### Primitive stream extrema and optional carriers (next batch)

Primitive `min`/`max` now use the existing stream terminal pipeline and produce
the appropriate OptionalInt, OptionalLong or OptionalDouble carrier. Accessors
reuse optional-call planning and declared NoSuchElementException behavior.
Semantic empty optionals remain non-null Java objects, including through
Objects.requireNonNull. Floating-point reduction reuses the Math min/max
helpers for NaN and signed-zero behavior; a thin runtime emits those same
helpers without requiring the full platform facade.

The old slot-driven implicit-sum fallback is removed: mapToInt/mapToLong/
mapToDouble are intermediate operations and must not invent a sum when a numeric
slot is requested. Arrays.stream metadata distinguishes object streams from
the three supported primitive streams, and primitive map preserves its declared
width even when a mapper returns an int expression for a long stream.
Filter closure projection now distinguishes owned iterator items from already
borrowed items rather than always adding two dereferences. String argument
recovery clones the projected String value rather than the closure reference,
including bound equals callbacks on owned streams. Constructor method
references retain the type namespace even when a local has the same spelling.

The minimized runtime regressions are in `tests/stream_primitive_extrema.rs`
and `tests/external_enum_display.rs`; the latter checks that synthesized external
enum default toString uses the existing enum Display implementation instead of
an unmodelled inherent stub. Native/modelled method implementations are not
replaced by this fallback. A valid javac-checked independent primitive-extrema
probe is `.tmp/stream-extrema-java-oracle/Probe.java`; its 13 records cover
widths, empty access/default/presence, NaN and signed zero. All seven focused
integration tests pass (`.tmp/stream-primitive-extrema-tests-v7.log`), including
the existing bound-reference and collector gates. The eight Math regressions
pass (`.tmp/stream-primitive-extrema-math-gate-v2.log`; before the final string
clone correction), as does the strengthened erased-enum display runtime test
(`.tmp/stream-primitive-extrema-enum-existing-gate-v2.log`).
All 1,363 unit tests pass on the final extrema revision
(`.tmp/stream-primitive-extrema-lib-tests-v2.log`, 456.88 seconds while overlapping
other verification). The earlier overlapping unit run in
`stream-primitive-extrema-lib-tests-final.log` also passes all 1,363 tests but
predates the final clone correction. Neither duration is an isolated benchmark.

Focused-v23 completed (`.tmp/jgrapht-v23-run.log`): the diagnostic count remains
133 actual Rust compiler errors (134 including Cargo's summary), with zero
translator/parse failures and unchanged 128 registry-invalid occurrences across
47 files. The top-error histogram is byte-identical to focused-v22. Translation
took 222.099 seconds, assembly 68.585 seconds and the full run 432.796 seconds.
The generated primitive max now reduces to an optional maximum rather than
inventing a sum and leaving max/getAsInt untranslated. The source collection
feeding that stream is still erased in this revision. Broader facade synthesis
also still emits unused unmodelled call surfaces, so this is not a claim of
reduced unsupported-surface counts.

### Resolved stream producers and single-store constructor values (next batch)

`tests/stream_source_resolution.rs` first reproduced an external List<Object>
fallback replacing a resolved source List<String> return. Source stream-element
inference now reuses call planning's exact ParsedSource owner/import identity
test and the resolved, already-substituted return type. External owners retain
their external contracts, including when an unrelated source has the same leaf
name. A source method returning a non-sequence is not made into a sequence by
external metadata. Expression-shape inference uses that same owner test before
consulting external ABI facts, allowing the authoritative source signature to
retain nullable collection carriers.

The end-to-end regression also exposed an unnecessary constructor clone of
Vec<T> stored once into an Option<Vec<T>> field. The existing binding-identity
single-store proof is generalized from bare owner type parameters to physically
compatible owned values. It still requires exactly the one content-read/store
pair for that field; distinct carriers and reused arguments do not qualify.
Proven transfers are constructed directly using the shared optional-slot
construction plan, without adding an artificial T: Clone requirement or
stripping clones from emitted text.

The initial four source-resolution tests, ten constructor-overload tests and
shared-receiver return integration test pass in
`.tmp/stream-source-resolution-batch-v2.log`. The final fixture additionally
checks a Rust non-Clone element and a constructor storing a collection twice;
that strengthened fixture, all 1,363 unit tests, and all eleven stream/enum/source
integration tests pass in `.tmp/stream-source-resolution-final-gates.log`.
The independent
javac-checked `.tmp/stream-source-java-oracle/Probe.java` prints `4` and `4`.
Focused-v23 predates this entire source-resolution/constructor batch; no new
corpus error count is available for it yet.

### Batch source emission ownership and imported static calls (next batch)

A neutral two-file reproduction in `.tmp/static-import-source/java` passes
assembled cargo check on the unchanged focused-v23 executable (SHA-256
`330fe6c1fc2b10fa2b19a23d5597d66ae1bdc46437f8a989ebb86c3f1264cf65`) but its
separate consumer probe panics at an unimplemented source-summary static method.
Evidence is `.tmp/static-import-source-before.log`, the `before` generated crate,
and `.tmp/static-import-source-before-probe.log`. Compilation alone did not prove
that static imports reached translated source bodies. The independent Java
probe prints `5` and `ok`.

`SourceEmissionPlan` now records the exact selected primary compilation units
and explicitly emitted context units. It is independent of the larger analysis
and summary programs, respects emission filters, preserves exact package owner
identity, and reuses the existing Java-to-Rust naming function. The immutable
batch plan is shared by all file lowerings. Facade suppression uses that same
inventory instead of treating every analyzed source as emitted. Static-import
owner selection uses structured imported/resolved identities; source ownership
is established in the shared lowering call-plan entry point before destination
slot shortcuts can mistake a source callable for an unavailable external API.

All three new `tests/source_static_imports.rs` regressions pass initially in
`.tmp/source-static-imports-tests-v2.log`, including emission-filter protection
and actual assembled Rust execution from a separate consumer crate. They also
pass after strengthening the fixture with overloaded imports and a caught
IllegalArgumentException (`.tmp/source-static-imports-tests-v3.log`, 39.77 seconds).
The existing shared overload/exception mechanisms handle these added cases;
no additional resolution policy was added. The independent Java overload probe
in `.tmp/static-import-overloads/Probe.java` prints `3` and `9`. Generated Rust
is not edited by any probe. `cargo check --tests` passes in
`.tmp/source-emission-check.log`.

The emission-plan unit test passes in `.tmp/source-emission-unit-gate.log`.
The first full unit gate passes all 1,364 tests in 434.27 seconds
(`.tmp/source-emission-lib-gate.log`, overlapping other verification). This run
predates the final restriction of early owner canonicalization to catalog-proven
emitted definitions; unavailable summaries retain their existing later planning
path. The final restriction passes all 1,364 unit tests in 451.14 seconds
(`.tmp/source-emission-lib-gate-final.log`, overlapping other work).

The first broad batch gate reports 78 passes and seven failures
(`.tmp/source-emission-batch-gate.log`). One compile fixture checked a single
generated file while omitting its now correctly emitted peer. It now goes
through production batch assembly with all actual sources, using the shared
test helper in `tests/common/mod.rs`; its focused rerun passes in the running
`.tmp/source-emission-batch-gate-v2.log`. The original shape assertions remain.

Three assertions were stale before this batch. The preserved focused-v23 binary
`.tmp/new-java2rust-pre-source-emission-330fe6c1` reproduces Rc hierarchy payload
construction and `__static_INST` (evidence in `.tmp/emission-gate-baseline/before`)
and the borrowed-str enum valueOf argument
(`.tmp/emission-enum-baseline/before/generated/00001_Main.rs`). The accessor test
now compares uses with the actual emitted declaration instead of independently
guessing acronym normalization. The enum test additionally executes the assembled
crate with both valid constants, without a hand-written Rust support prelude;
that strengthened test passes (`.tmp/source-emission-enum-gate-v2.log`). These
baseline fixtures establish the stale spellings, not a claim that every baseline
fixture compiles successfully.

Other summary-boundary batch failures remain under investigation. In particular,
the previous binary reproduces `Step::COMPOUND` when the supplied summary declares
`Aligned_Step` (`.tmp/summary-enum-baseline/before`), establishing that this defect
predates the emission batch. It remains open, not a goal-wide accepted waiver.
The second broad batch gate is running in `.tmp/source-emission-batch-gate-v2.log`;
it was compiled before the final enum assertion/harness update, whose independent
focused gate above is authoritative for that update.

Focused-v24 completed (`.tmp/jgrapht-v24-run.log`) and includes the source-return,
single-store constructor and emission-ownership batches together. It still has
133 actual Rust errors (134 including Cargo's summary), but registry-invalid
occurrences decrease from 128 to 127 across the same 47 files. All 104 files
translate without translator or parse failures. Unmodelled types decrease from
452 to 444 and unmodelled call sites from 264 to 253. Translation takes 226.446
seconds, assembly 60.085 seconds, and the full run 423.423 seconds; these overlap
other verification and are not isolated performance comparisons. Wildcard
static imports and other unverified import cases are not claimed to be fully
supported by these probes.

Two important corpus findings remain open. The GraphTests primitive-stream
producer is still erased to Box<dyn Any> in v24, so the neutral source-return
precedence fix does not yet cover this cross-file/imported-owner configuration.
The parsed-source identity guard and declaration/import aliases need a minimized
batch regression; relaxing package identity by a global leaf-name match is not
an acceptable repair.

The source-emission batch also exposed an assembly regression: v23's crate root declares
`__JavaInterfaceUpcastListenableGraphToGraph`, but v24's assembled crate does not.
The v24 raw ListenableGraph output still contains its private declaration,
forwarding impl and boxed From impl; the latter survives in the stripped source
module while its carrier is discarded. Removing duplicate source facades means
this helper now occurs only in its authoritative source unit. Shared-support
promotion must preserve a single-occurrence generated carrier together with its
conversion/impl dependencies, without relying on duplicate facades or the
JGraphT helper's name.

The merger now seeds its existing dependency closure with private carriers
referenced by boxed-trait conversions, and promotes those conversions together
with their carrier. No helper-name policy is used. All nine support-closure
tests pass (`.tmp/single-upcast-assembly-after-v2.log`), including a neutral
generic adapter regression that compiles and executes the assembled crate while
checking constructor privacy. The original raw input and a previously stripped
module are both exercised.

An assembly-only rerun on a disposable copy of frozen v24 confirms that the
declaration, forwarding impl and conversion now survive together. Actual Rust
errors decrease from 133 to 132 (runner count 133 includes Cargo's final summary).
Evidence: `.tmp/jgrapht-v24-assembly-repair-run.log` and
`.tmp/jgrapht-v24-assembly-repair.jbbClU/compile.log`. The original v24 artifacts
are unchanged. This is v24 translation plus the latest assembly scripts, not a
measurement of newer translator changes; the 127 invalid occurrences remain.

The full Java-to-Rust upcast regression initially exposes another failure:
returning a borrowed child interface as an owned parent is rejected despite
the generated boxed adapter (`.tmp/java-upcast-assembly-gate-v1.log`). A source
ancestor-instantiation-backed conversion planner now passes the original
end-to-end regression (`.tmp/java-upcast-assembly-gate-v2.log`). It reuses the
same instantiated ancestor discovery that generates boxed conversion adapters,
and composes object-safe clone recovery with that adapter only when its source
and target generic ABIs match. A focused policy regression covers transitive
generics, owned moves, mismatched arguments, missing adapter evidence and foreign
same-leaf identities (`.tmp/source-upcast-policy-gate-v1.log`).

The extended runtime regression adds mutation/identity and transitive generic
returns. Its first run exposed an inherited-contract specialization defect
(`.tmp/upcast-assembly-batch-gate-v3.log`): a cloned `Child<String>` declaration
specialized local methods but left its `Parent<T>` edge unchanged, producing
an unbound `T` in trait implementations. Implementation arguments now seed the
existing recursive contract walk; original source declaration identities and
method-type-parameter shadowing are retained. The final focused runtime/support
gate passes: one end-to-end runtime test (30.10s) and nine support-closure tests
(1.51s), recorded in `.tmp/upcast-assembly-batch-gate-v5.log`. The runtime consumer
checks direct value 37, mutation-visible identity value 38, and transitive generic
return length 5. Generated Rust is not modified. Formatting and whitespace checks
also pass. The combined full-unit/batch/source gates in
`.tmp/source-upcast-summary-broad-gates-v1.log` are still running; they include a
minimized inherited-contract unit regression checking method-parameter shadowing
and original declaration identity. The corpus translation has not been rerun on
these newer translator changes.

### Summary declaration paths and aliased method surfaces (in progress)

The second broad batch gate reports 81 passes and four failures
(`.tmp/source-emission-batch-gate-v2.log`). Its enum-valueOf assertion predates the
already-passing focused replacement. The other three summary-boundary failures
are being handled as a related batch rather than rerunning the full corpus for
each change.

Nested summary enum constants now recover their lexical owner by declaration
identity. The shared source-declaration naming helper is also used by the batch
emission inventory; two same-leaf nested declarations cannot be conflated, and
already-qualified owner paths are not duplicated. The previously failing
`qualified_summary_nested_enum_reuses_canonical_facade_owner` now passes
(`.tmp/summary-nested-enum-gate-v1.log`).

Summary method surfaces now follow transitive facade aliases to their actual
carrier. The same cycle-refusing alias resolver is reused by method discovery,
signature lookup and final method/field consolidation. The second, existing
source-summary closure was moved after carrier/alias-producing discovery phases
and before generic-bound trait promotion; no additional whole-surface fixpoint
pass was added. This restores methods on carriers discovered only through late
import/field/return evidence. The original imported-summary static-owner test
passes (`.tmp/summary-alias-static-gate-v2.log`). A neutral unit test covers a
two-hop alias and real convergence (`.tmp/summary-alias-unit-gate-v1.log`).

The nullable-summary upcast test now extracts the argument list without assuming
one of two equivalent facade-alias spellings. Its original unsupported-upcast,
no-fabricated-conversion and constructor-ABI assertions are unchanged and pass
(`.tmp/summary-nullable-upcast-gate-v1.log`). Declaration-path ambiguity and
alias-cycle unit regressions are included in the broader focused summary gate:
39 unit tests and 12 batch tests pass in `.tmp/summary-identity-focused-gates.log`. Full unit/batch gates still
need to be rerun on this summary batch. Focused-v24 predates all these summary
changes and must not be used as evidence of their corpus effect.

### Imported source stream types and generic inference (in progress)

A neutral two-file regression now reproduces the frozen GraphTests callback
erasure: an imported `Bucket<T, U>` declares `Set<T> values()` and `score(T)`,
while conflicting external contracts describe `Set<Object>`. The actual client
method emits `|value: Box<dyn Any>|` before the repair
(`.tmp/imported-source-stream-before-v3.log`). The assertion is scoped to the
client implementation, not the generated support prelude.

Call planning's existing source-provenance check now also consults the exact
batch emission inventory already used for source facade suppression and static
owner retargeting. Import qualification no longer discards proof that the source
declaration owns the ABI. The inventory is shared through `Arc` across lexical
environment clones, rather than copied or rebuilt per expression. Both inventory
unit tests pass (`.tmp/source-ownership-inventory-gate-v1.log`), including foreign
same-leaf rejection and ambiguous bare-owner rejection.

The stronger three-file production-assembly runtime regression confirms that
the callback retains `T`, but initially exposes two downstream failures: a
gratuitous clone of the owned callback input and unresolved `T, U` when inferring
`run(new Words())` through `Words implements Bucket<String, String>`
(`.tmp/imported-source-stream-runtime-gate-v2.log`). These are not waived.
The existing single-argument source-supertype projection has now been generalized
to return all instantiated arguments over class/interface ancestry. Both method
inference and scoped argument compatibility reuse it; the one-argument iterable
consumers remain wrappers. A minimized resolver regression includes reordered
arguments through an intermediate interface. Bound method-reference callbacks
can consume an owned input they forward exactly once, while borrowed filter
inputs and general lambda bodies keep their preservation policy. The combined
generic, source-stream and bound-reference regressions all pass in
`.tmp/source-stream-generic-batch-gates-v3.log`: 12 generic-binding tests (19.43s),
two bound-reference tests (31.80s), and five source-stream tests (55.85s). The
actual imported client callback is checked for typed `T` without an input clone;
production assembly and a separate runtime consumer then verify result 4 for
the three-file Java program under conflicting qualified and short-owner external
metadata. Neither Java nor generated Rust is manually repaired.

The preceding summary/upcast batch's gate completed successfully:
1,369 unit tests, all 85 batch tests, the interface-upcast runtime test, three
static-import tests, five source-stream tests and nine support-closure tests
(`.tmp/source-upcast-summary-broad-gates-v1.log`). Its long-running invocation
overlapped later builds; the unit binary predates this imported-source batch,
while integration checks use the shared debug translator executable. This is
not the final stable-revision verification required for goal completion.

The stream/ownership snapshot's full unit gate passes all 1,370 tests (459.19s);
its 85-test batch gate also passes (404.99s) in
`.tmp/source-stream-ownership-broad-gates-v2.log`. The fresh 104-file corpus run
completed in `.tmp/jgrapht-core-1.5.3-focused-v25`, with runner output in
`.tmp/jgrapht-v25-run.log`. It reports 131 actual Rust errors (132 including
Cargo's final summary), 104 registry-invalid occurrences in 38 files, and zero
translation/parse failures. Original v24 had 133 actual errors and 127 invalid
occurrences in 47 files; the assembly-only repaired v24 had 132 actual errors.
Unmodelled types decrease from 444 to 369; unmodelled call sites remain 253.
The original GraphTests `is_overfull` callback now consumes typed `V`, rather
than `Box<dyn Any>`, directly confirming the imported-source regression repair
on the untouched corpus. The crate still fails and the Rust graph oracle is not
claimed to pass.

Build/resolver/translation/assembly/cargo times are 96.321/40.507/225.542/62.682/
4.316 seconds (432.615 seconds total). Other verification overlapped; these are
run timings, not an isolated performance comparison. The v25 executable is
preserved as `.tmp/new-java2rust-v25-stream-ownership` (SHA-256
`b2ef28614d237bdb8fa0498900a233d772d92f0e41e75d068cad6b9ddaf6a2a7`).
The preceding v24
release executable was preserved as `.tmp/new-java2rust-v24-df93576a` (SHA-256
`df93576af13dd73d74a9c7042e707e7eaeea6e02da51be524af739a6c2b8209a`). The pinned
checkout was revalidated at the required commit and has no tracked changes.

### Erased collection display uses native carriers (in progress)

The next related runtime-support batch addresses the eight missing bare `List`
and `Set` type references seen in the frozen corpus. A corpus-neutral regression
adds external `toString` metadata for collection interfaces, declares native
collection fields, and invokes the translated erased-object display entry point.
The original generated crate independently fails `cargo check --lib` with 12
missing nominal collection-type errors (List, Set and Map):
`.tmp/external-collection-display-before-lib-check.log`. This excludes the initial
test fixture's three incorrect `.unwrap()` calls on an infallible String result;
those test-only calls were corrected without changing the generated Rust.

External display metadata no longer produces nominal downcasts for native
collections. The native collection downcast generator now reuses the existing
collection-kind classifier over declared and constructed Java types, including
generic arguments and bounds. Interface spellings such as Set and SortedMap
therefore select the same native carrier support as their implementations; no
new facade-name list is introduced. Collection formatting stays in the existing
Java collection/map display helpers.

The focused gate passes 25 erased-object unit tests and one compilation/runtime
regression (`.tmp/external-collection-display-focused-gates-v2.log`). The latter
uses both short and qualified external metadata and checks list/set/sorted-set
output `[only]`, and map/sorted-map output `{key=value}` through native Rust
carriers. Additional existing display/enum regressions pass: 29 display-focused
unit tests and one external-enum integration test in
`.tmp/native-collection-display-related-gates-v3.log`. Focused-v25 predates this
display change and must not be used as a corpus measurement of it.

The next coherent failure group was emitted versus implemented inherited interface
contracts. In v25, `GraphListener<V, E>` is emitted with an unparameterized Rust
`VertexSetListener` supertrait and no inherited vertex methods, while its class
implementations place those methods inside `impl GraphListener`. This explains
eight immediate E0407 diagnostics and suggests a mismatch between the source
interface emission paths (including interfaces with default bodies) and the
shared specialized-contract machinery. Reproduce with neutral generic
parent/child interfaces, including a child default method, before changing the
policy. Related iterator-interface method omissions should be checked as part
of the same group rather than patched by method name.

### One source-interface surface for abstracts and defaults (in progress)

The minimized neutral generic parent/child regression fails before the repair
with E0107 (an unparameterized parent) and E0407 (an inherited method implemented
on the child but absent from its trait): `.tmp/interface-default-inheritance-before.log`.
Interfaces containing defaults now use the same specialized instance-contract
selection and trait finalizer as abstract-only interfaces. The previous
supertrait, arity-based duplicate and separate collision policies were removed.
The existing source-parent lookup unit keeps its ambiguity/package checks,
without asserting obsolete Rust-supertrait syntax.

A shared binder-aware source-member specialization handles return/parameter,
local-declaration, nested scope, resolved-call and constructor types without
renaming variables or changing binding IDs. It is reused for inherited class
members as well as interface defaults. Solved local plans are specialized with
the bodies, retaining their representation and nullability wrappers. This
avoids an original Parent<T> local plan reintroducing T into Child<X> or a
non-generic concrete implementor. A resolver-bound unit checks preserved IDs,
method-binder shadowing and nested-class binder shadowing.

Trait implementations now consider the existing inherited-default member
selection rather than silently using a less-specific parent's Rust default.
Boxed upcasts and generic-retarget adapters forward the full virtual surface,
including overridden defaults. A proven boxed-interface conversion is routed
through the shared conversion planner before concrete-implementor boxing;
the liveness mechanism selects clone-box recovery when its source alias remains
live. Conservative Concrete dispatch tags do not justify double-boxing a source
interface whose instantiated adapter is known.

Explicit Parent.super calls keep their separate non-virtual meaning. Flat child
traits retain owner-qualified copies of ancestor default helpers, including
overridden bodies, specialized through the same ancestry/type mechanisms.
Inside an interface default, the named helper is called on Self; concrete
classes keep their existing parent-trait helper route. Helper naming reuses
the shared Rust identifier renderer rather than introducing another keyword
mapping.

The stronger neutral runtime regression passes, covering renamed generic
binders, inherited local types, a child override, Parent.super, and later use of
an alias after a parent upcast (expected 13). The basic inherited-contract/default
regression returns 7. Both and the production interface-upcast assembly test
pass in `.tmp/interface-default-inheritance-super-gates-v6.log`. Earlier focused
stages pass 54 interface unit tests and the type-specialization unit
(`.tmp/interface-contract-unit-gates-v4.log`); updated gates pass 54 interface,
18 inherited-member and one type-specialization unit tests in
`.tmp/interface-inheritance-unit-gates-v8.log`.

Anonymous adapters now consume the same specialized contract inventory and keep
the original declaring owner for solved signature lookups. This removes a second
default/collision policy and avoids looking up inherited methods under the child
owner. Non-synchronized adapters also implement their source interface on their
exact Rc carrier, matching named source implementations without adding an
overlapping blanket Rc implementation. Clone-box preserves the shared adapter
identity. All three anonymous-default runtime regressions pass in
`.tmp/anonymous-interface-default-gates-v11.log`.

Existing default-body, overload and generic-default tests all pass in
`.tmp/interface-default-existing-gates-v12.log`; explicit-super (two tests) and
lexical-scope (one test) gates pass in
`.tmp/interface-default-existing-gates-v10.log`. The overload test now also runs
an independent Rust consumer (expected 1103). Its stale Result-return assertion
was checked against the frozen v25 translator, which already emitted i32.
The generic-default fixture additionally exposed a pre-existing boxed forwarding
error: a Self: Sized method was forwarded through an unsized payload. Direct
compilation of frozen v25 output reproduces E0277 in
`.tmp/default-generic-v25-check.log`. Boxed forwarding now shares the sized-only
constraint rule, with a focused unit regression.

The interface-batch snapshot passes all 1,372 unit tests (465.82 seconds) in
`.tmp/interface-contract-broad-gates-v13.log`; its 85-test batch gate and later
integration gates remain running. The subsequently added nullable-generic
regression below is not part of that unit snapshot. The broader anonymous snapshot
finishes with 72 passes and six failures in
`.tmp/anonymous-interface-broad-gates-v14.log`; the parent-argument consumer is
subsequently corrected and passes independently. The other five remain open:
reference-field replacement, captured mutable alias, writes through a named
reference field, multi-method named-field handle, and static-array anonymous
state. Do not treat this as a passing broad gate or assume baseline attribution
for failures not yet compared.

The fresh untouched 104-file run completed in
`.tmp/jgrapht-core-1.5.3-focused-v26`, with runner output in
`.tmp/jgrapht-v26-run.log`: 137 actual Rust errors (138 including Cargo's final
summary), 110 registry-invalid occurrences in 38 files, and zero translation
or parse failures. This is worse in aggregate than v25's 131 errors and 104
invalid occurrences; the batch is not complete. The ten E0407 missing inherited
members, two unparameterized parent-trait errors and twelve native collection
bare-type errors are gone, but the expanded interface surface exposes additional
return-signature disagreements. Graph's nullable E/V results are incorrectly
rendered as bare generic values by the default-aware trait emitter, while
implementors retain their nullable carriers. The generic-contract specialization
must preserve solved nullability/ownership wrappers and share one contract with
all forwarding implementations, rather than resetting a generic return to a
fresh storage shape. The nested cursor probe below also exercises this boundary.
The minimized maintained runtime regression is
`generic_interface_with_default_preserves_nullable_return_contract` in
`tests/interface_default_inheritance.rs`: an interface with an unrelated default
method and a generic `read()` implementation returning null. It currently fails
with E0053 and an invalid nullable projection in
`.tmp/interface-nullable-generic-return-before-v19.log`, reproducing the corpus
contract loss without any graph-library names or external dependencies.

V26 reports 373 unmodelled types, 253 unmodelled call sites and 62 unmodelled
upcast conversions. Rebuild/resolver/translation/assembly/cargo-check times are
92.369/40.744/227.653/62.799/4.414 seconds, total 431.443 seconds. These timings
overlap broad gates and are not isolated performance evidence. The selected
crate still fails; no Rust graph-oracle pass or expansion decision is claimed.

Broader verification has identified two follow-ups (not a green broad gate):

- `anonymous_interface_named_reference_field_replacement_preserves_aliases`
  still emits a bare constructor into an Rc-backed replacement field. Frozen
  v25 independently reproduces that mismatch, as well as the now-fixed missing
  exact-Rc trait implementation:
  `.tmp/anonymous-reference-replacement-v25-check.log`. The focused current
  failure is `.tmp/anonymous-reference-field-gate-v15.log`. Anonymous writes
  must reuse the ordinary assignment carrier-construction mechanism rather
  than only sharing field type rendering.
- The parent-argument anonymous runtime test reached its consumer but retained
  a stale `.unwrap()` on an infallible i32 result. Its corrected consumer passes
  in `.tmp/anonymous-parent-argument-gate-v17.log`. Frozen v25 instead panics
  looking up inherited `Combined::shared`, confirming that the new canonical
  declaration-owner lookup makes this previously unreachable check execute.

An additional neutral nested generic cursor probe is in
`.tmp/nested-default-cursor-neutral.java`. Both frozen v25 and current v26 fail
direct Rust compilation, with logs ending `-v25-check.log` and `-v26-check.log`.
The new canonical surface removes the missing inherited trait member and
unparameterized supertrait, but the child's own default body still leaves an
inherited call unqualified and nullable return contracts disagree. A resolved
IR inspection (`.tmp/inspect-nested-default-resolution.log`) shows that the
parent's `nextCell` call resolves while the child's inherited `nextCell` call
does not. The hierarchy substitution helper currently ignores its lexical
owner; the existing lexical source-type lookup is the mechanism to reuse when
repairing this rule. This probe is diagnostic evidence, not a passing runtime
regression or a manually repaired generated program.

The v26 translator executable is frozen as
`.tmp/new-java2rust-v26-interface-contract`, SHA-256
`cd704ad2230f748fbfa670f02076a609d5a3d605cd5b03eed4df17128f91b8d7`.

### Preserve instantiated virtual contracts through the solver (in progress)

The follow-up uses `shape_projection::specialize_slot_shape_java_type` for
instantiated interface return and parameter shapes. Declarations, default
bodies, concrete/anonymous implementations, boxed upcasts and generic retarget
adapters reuse the same projection; specialization retains solver-owned nested
optionality instead of replacing a slot with its bare Java type. Escaping type
parameter returns still select their owned representation, now retaining the
nullable wrapper. Original method lookup uses the existing Java callable slot
key rather than name plus arity.

The strengthened neutral `Maybe<T>` regression adds a default `copy()` with a
typed local. Focused stages exposed two additional consumers, recorded rather
than bypassed: generic-retarget conversion treated `Option<T>` as T
(`.tmp/interface-contract-specialization-runtime-v21.log`), then call-site
shapes still treated an emitter-widened result as non-null and constructed
`Some(self.read())` into `Option<T>`
(`.tmp/interface-contract-specialization-runtime-v25.log`). Retarget conversion
now maps present elements while preserving None. Generic interface
implementation edges now join the existing source-override constraint topology
before signature saturation, sharing the ordinary nullability/receiver joins
instead of keeping a separate receiver-only loop. Callers and locals therefore
participate in the same solved Java value domain as emitted traits.

Implementation discovery now instantiates both the interface contract and the
candidate through the existing ancestry mechanisms. It no longer accepts any
same-arity overload merely because the interface mentions a type parameter.
Parameter slots must match before covariance is considered. A neutral unit
includes an unrelated Integer overload before a String implementation, and a
second overload pair where the unrelated method has a covariant return.

Resolver hierarchy edges are canonicalized once after every declaration has
been collected, reusing lexical member-type resolution. Typed and erased
hierarchy indexes derive from the same canonical inventory, so inherited
method lookup, fields, assignability and generic argument projection cannot
choose different nested parents. The neutral unit uses two owners with
identically named nested parents, including a later-declared parent. It passes
in `.tmp/nested-parent-edge-unit-v24.log`. The nested cursor diagnostic is now
also a maintained runtime regression in `tests/interface_default_inheritance.rs`.

Anonymous field writes reuse `lower_assignment_value_for_slot`, including
shared-constructor wrapping and narrowing, for sequential, conditional and
statement-based mutations. The reference-replacement runtime regression passes
in `.tmp/anonymous-field-assignment-runtime-v22.log`; the other two named-reference
tests in that invocation fail for separate read-only-capture and class-carrier
Display gaps. Frozen v25 independently reproduces the captured-mutable-reader's
Default/Fn/exception-ABI failures in
`.tmp/anonymous-captured-mutable-v25-check.log`; this establishes attribution,
not a fix or a passing gate.

The focused interface unit snapshot passes 55 tests and the subsequent nested
unit in `.tmp/interface-contract-solver-units-v27.log`. The four runtime
regressions initially expose two remaining consumers in
`.tmp/interface-contract-solver-runtime-v26.log`: a call-shape shortcut still
constructed a fresh generic storage slot, and named ancestor-default helpers
still rendered bare return shapes. Both now use the same canonical projection.
All four runtime regressions and the production upcast assembly test pass in
`.tmp/interface-contract-consumers-runtime-v28.log` (21.76 and 29.48 seconds).
A direct solved-plan inspection in
`.tmp/inspect-nullable-interface-contract.log` confirms nullable Maybe.read,
Maybe.copy and its local before lowering; the caller no longer discards that
information through its former shortcut.

Canonical declaration return shapes are now stored once in the immutable
SignaturePlan after all primary/support method signatures have been collected.
Trait emission and call-shape consumers share these entries, including cached
void results. This prevents the expanded call-shape consumer from repeatedly
walking every implementation for every call. Primary declarations populate
entries first, with context only filling missing entries. Generic instantiation
still overlays each actual contract on the cached solved shape. No timing
improvement is claimed without a fresh measurement.

The cached revision's final focused checks pass in
`.tmp/interface-contract-final-focused-v29.log`: 12 generic-binding tests
(19.29 seconds), four default/inheritance runtime tests (23.60 seconds), and
the production upcast assembly runtime (30.87 seconds). Focused units pass in
`.tmp/interface-contract-cached-units-v30.log`: 55 interface tests (18.80 seconds)
and the nested-parent identity unit. The broad gate passes all 1,374 unit tests
(434.11 seconds) in `.tmp/interface-contract-nullability-broad-gates-v31.log`;
its 85-test batch phase also passes (413.92 seconds). The debug CLI remained
unchanged while the subsequent support-scope work ran library-only tests.
The untouched 104-file corpus completed
in `.tmp/jgrapht-core-1.5.3-focused-v27`, log `.tmp/jgrapht-v27-run.log`.
The v27 translator executable
is frozen at `.tmp/new-java2rust-v27-nullable-contract`, SHA-256
`44f8fdd6bcedee964fd6e138a9cac5e83575d6944ca72e6614a8cb9f91949141`.

The preceding broad invocation passes 1,372 units, 85 batch tests,
the display runtime and 12 generic-binding tests, then fails the subsequently
added nullable regression. Its shared integration binary/CLI paths overlap later
builds, so it is not a final stable-revision all-green gate.

V27 reports 99 actual Rust errors (100 including Cargo's final summary), down
from v26's 137. All 36 interface signature mismatch diagnostics and the three
unqualified nested-cursor call diagnostics are absent. One other error becomes
visible after the earlier blockers are removed. Registry-invalid occurrences
remain 110 in 38 files; translation and parse failures remain zero across 104
files. Unmodelled types/calls/upcasts remain 373/253/62. No Rust graph oracle
pass is claimed.

Rebuild/resolver/translation/assembly/cargo-check timings are
89.920/40.295/216.325/61.103/4.193 seconds, total 415.065 seconds. These overlap
the broad gate and are not an isolated speed comparison. Within translation,
signature plan construction is 5.168 seconds and fixpoint saturation 4.798
seconds; emission is 148.844 seconds. The latest failure inventory still includes
bare generic source extension impls and bare generic source/carrier returns in
erased support facades. These are the next coherent support-scope batch; fixing
their Rust type validity must not disguise the existing unmodelled behavior as
a functioning Java implementation.

### Generic support scopes retain declaration arity (in progress)

Two minimized neutral regressions fail before this batch in
`.tmp/generic-support-scope-before-v32.log`: an emitted two-parameter source
class receives a bare inherent extension `impl`, and an erased support return
retains an uninstantiated generic class/carrier payload. These are support
producer errors, not permission to repair generated Rust or to pretend the
external methods have implementations.

The extension-surface producer now retains the source declaration's kind and
arity. One shared declaration-header renderer handles class and interface
extension impls; these records do not authorize opaque source redeclarations.
Source metadata takes precedence even when a source class has a built-in facade
name with a different arity. Method generic collision handling reuses the same
existing identifier rewrite, with the actual source arity rather than the
built-in arity. The neutral regression includes such a three-parameter source
declaration and a three-argument inherited method.

The existing erased generic return normalization is generalized from a literal
boxed-trait replacement to recursive type-argument traversal using shared Rust
type-text helpers. Only a known bare source payload is erased. Explicit
instantiations, distinct qualified paths and identifier prefixes are retained;
surrounding Result/Option/Rc/container structure is preserved. Generic arity
inventory includes generated closed/hierarchy carriers through their existing
representation entries, declines conflicting arities, and excludes existential
interface carriers that are deliberately non-generic.

The initial two regressions pass in `.tmp/generic-support-scope-after-v33.log`;
the source-priority/parameter-shadowing version passes in
`.tmp/generic-support-scope-source-priority-v34.log`. Related checks pass 25
external-facade tests and 26 erased-object tests in
`.tmp/generic-support-scope-related-units-v35.log`. The final dedicated gate
passes three tests in `.tmp/generic-support-scope-compile-v36.log` (17.40 seconds),
including a complete neutral translation compiled unchanged by rustc. The
unsupported inherited external method remains explicitly unmodelled and is
not invoked by this compile-only test; no behavioral implementation is claimed.

A fresh untouched 104-file measurement completed in
`.tmp/jgrapht-core-1.5.3-focused-v28`, log `.tmp/jgrapht-v28-run.log`: **79 actual
Rust errors**, down from v27's 99, with zero translation or parse failures.
Invalid occurrences remain 110 across 38 files; unmodelled types/calls/upcasts
are 379/253/62. The runner's 80-error count includes Cargo's final summary.
The frozen producer is `.tmp/new-java2rust-v28-generic-support`, SHA-256
`ee06385ddfd7cf79f3b1096036f2ba9be804264d5abd84639b2b125908ce4a03`.
Build/resolver/translation/assembly/check timings are
85.501/36.813/204.386/59.486/4.006 seconds, total 393.284 seconds. These overlapped
other gates and are not an isolated speed measurement.
A newly exposed v27 diagnostic is a
checked interface downcast from a parent cursor interface to a child interface;
it remains separate open representation work.

### Native collection algorithms and ordered collectors (in progress)

This coherent batch implements native `Collections.reverse` through the shared
statement planner and mutable-place lowering, including qualified owners,
source-owner precedence and existing immutable/unmodifiable view facts. Empty
and singleton unmodifiable lists remain no-ops; a required swap throws the
existing Java unsupported-operation exception.

`Collectors.toCollection(LinkedHashSet::new)` now retains insertion-ordered
collection identity through stream planning, borrowed-element recovery,
fallible collection and slot rendering. Generic materialization capabilities
reuse the existing stream plan rather than a second collector-name policy.
The ordered runtime carrier reuses Java key equality, keeps first encounter
order, and compares sets without making equality depend on iteration order.
Explicit native collection/map `toString()` calls reuse existing Java formatters.

Baseline minimized regressions are recorded in
`.tmp/native-collection-algorithms-before-v37.log` and
`.tmp/linked-set-collector-before-v38.log`. The v40 focused gate passed all four
stream collector tests and two of three native algorithm tests. The remaining
failure exposed qualified static owners missing from argument mutation usage
analysis; that shared path is being corrected. The v41 collection unit gate
passed 28 tests with one stale representation assertion expecting an unordered
carrier for a declared `LinkedHashSet`; the expectation now preserves order.
The v42 focused gate passes all three native algorithm and four stream
collector tests. The extended v44 collector gate also passes a Rust consumer
that supplies Java key equality without structural `PartialEq`, including
borrowed keys, first-encounter order and order-independent set equality.
The v43 collection unit rerun exposed a second assertion in the same test
whose solved local shape is explicitly `LinkedHashSet`; it too now expects the
ordered carrier. The full unit gate in
`.tmp/native-collection-broad-units-v45.log` completed with 1,377 passes and one
failure: the new qualified-owner argument-effect fixture supplied empty facts
although its assertion requires Java platform argument effects. The fixture now
explicitly supplies the existing platform facts; its focused recheck is in the
v49 related gate below. The native runtime regressions already passed with
normal translation facts. This broad run predates the wildcard-contract edits
and does not prove the entire latest worktree green. The 104-file corpus measurement
completed in `.tmp/jgrapht-v29-run.log`: **76 actual Rust errors**, down from
v28's 79 (the runner counts 77 including Cargo's final summary). All three
native `Collections.reverse` diagnostics disappeared. Invalid occurrences
remain 110 in 38 files; translation and parse failures remain zero. Unmodelled
types/calls/upcasts remain 379/253/62. Five ordered collector pipelines still
fall back before stream lowering and remain separate open work; passing the
neutral collector tests does not establish that these corpus call shapes work.
The v29 producer is frozen as `.tmp/new-java2rust-v29-native-collections`,
SHA-256 `3b42463c9b5e904bf635a521486b0b851e5cc9a89048427a8df5aa1b96795802`.
Build/resolver/translation/assembly/check took
87.837/37.877/218.271/62.373/4.251 seconds, total 414.072 seconds. Other tests
overlapped this run; these timings do not establish a speed regression. The
pinned Java checkout remains unchanged. The translated Rust graph oracle has
not passed.

### Reified captures on source class-bound contracts (in progress)

The corpus still has six missing wildcard-capture parameter diagnostics on
source class-bound traits and their forwarding implementations. A neutral
`Base.inspect(Container<?>)` called through `H extends Base` reproduces the
same failure in three surfaces in
`.tmp/wildcard-bound-contracts-before-v46.log`. The common signature builder
now uses the existing wildcard-reified method and ordinary callable generic
renderer, retaining capture bounds and the `Self: Sized` obligation. The
unbounded fixture passes in the first focused rerun.

A second fixture, `Producer<? extends Item>`, exposed a related missing
`ItemBound` capability declaration. Bound inventory now consumes the same
wildcard synthesis result as method emission. Its shared extraction avoids
cloning method bodies, and declaration lookup is skipped for named types
without direct reifiable wildcard arguments. The combined focused gate passes
both unchanged-generated-Rust compile regressions in
`.tmp/wildcard-bound-contracts-inventory-v48.log` (19.45 seconds). Related
wildcard, source-class and qualified mutation-analysis unit selections pass
5/9/1 tests respectively in `.tmp/wildcard-contract-related-units-v49.log`.

This does not yet solve the separate corpus case `? extends V` where `V` is
itself a type parameter: emitting `V` as a Rust trait is invalid. That requires
an explicit subtype-conversion capability and corresponding value conversion,
not dropping the Java bound or pretending that every type parameter is a trait.

### Collection constructor references and generic set boundaries (in progress)

The wildcard-import probe passes in
`.tmp/wildcard-imported-collector-before-v50.log`: wildcard imports alone do not
explain the five remaining corpus collector fallbacks. Two new neutral fixtures
in `tests/stream_collection_returns.rs` both reach native stream planning but
fail generated-Rust compilation, recorded in
`.tmp/stream-collection-returns-before-v51.log`.

A constructor reference can be the only use of `LinkedHashSet` when a method's
declared return is `Set<T>`. Runtime-support discovery previously ignored method
reference targets, omitting `JavaLinkedHashSet`. It now uses the same platform
type classifier as ordinary construction. In
`.tmp/stream-collection-returns-support-v52.log`, that missing-helper diagnostic
is gone. The support-discovery selection, including the new direct regression,
passes all 10 tests in `.tmp/collection-constructor-support-units-v53.log`.

At v52 both new collection-return fixtures still failed: a generic ordered
collector returned through `Set<T>` was converted to `HashSet<T>` without
structural Eq/Hash capabilities. The source-interface-field fixture exposed the
same problem in its erased generic retarget adapter. These results did not
constitute passing corpus gates.

The coherent boundary batch generalizes the existing field policy rather than
inventing structural equality bounds. Solved slots now retain
`ViewShape::JavaSemanticHash`, selected from the actual lexical generic scope
and guarded against parsed source types shadowing native collection names.
Representation-aware and facts-only rendering reuse the existing semantic
collection carrier renderer. Borrowed-element recovery and owned carrier
conversion honor the destination's Java equality policy, including when the
Java collection kind itself is unchanged. The concrete-declared return fast
path and typed-local fast paths use the same projection. Generic retarget
adapters request JavaMapKey when their collection contracts need it.

Generic capability inference now reuses the recursive hash-capability analysis
for both native collection construction and source-generic containers; its old
source-only duplicate has been removed. Class-generic methods receive selective
JavaMapKey where-bounds. Stream capability analysis skips source-inventory
construction when the body has no possible collection materialization terminal;
the shared stream planner still owns source identity, ownership and capabilities.
This is an avoided-work change, not a measured speedup claim.

All three neutral regressions pass in
`.tmp/semantic-collection-runtime-v58.log` (20.54 seconds): the source-interface
field pipeline compiles unchanged, the generic Set return executes with Java-key
elements lacking Rust Eq/Hash and retains encounter order, a Java caller observes
the same ordered result, and empty class-/method-generic factories accept keys
without requiring Clone or structural equality. The v56 related gate passed 12
generic-binding and four interface-value contract tests; its collection test
failure was a Rust consumer passing an owned Vec to a correctly borrowed-slice
parameter, corrected in v58. The v55 gate separately passed five established
collector and two wildcard-contract tests. The v59 collection unit selection
passes all 30 tests in `.tmp/semantic-collection-related-units-v59.log`.

The full unit snapshot passes all **1,379 tests** in
`.tmp/semantic-collection-broad-units-v60.log` (476.11 seconds). The untouched
104-file corpus measurement completed in `.tmp/jgrapht-core-1.5.3-focused-v30`,
log `.tmp/jgrapht-v30-run.log`, with **76 actual Rust errors** (77 including
Cargo's summary), zero translation/parse failures and 110 invalid occurrences
in 38 files. Six missing capture-parameter errors disappeared, but the now-bound
captures expose six additional invalid type-parameter-as-trait bounds, leaving
the total unchanged. Unmodelled types/calls/upcasts remain 379/253/62. The five corpus
collector fallbacks are not proven fixed by these neutral fixtures and remain
open to further minimization. No Rust graph oracle pass is claimed.
The v30 translator build took 88.436 seconds and is frozen as
`.tmp/new-java2rust-v30-semantic-collections`, SHA-256
`8aca4cac00134815176185add62d14e732142f8d17ff2f112a8aec24772f089b`.
Build/resolver/translation/assembly/check timings are
88.436/37.799/216.844/60.445/3.987 seconds, total 410.953 seconds. Other gates
overlapped the run, so these are not isolated speed-comparison measurements.

### Generic collection borrow classification and dependent bounds

A further neutral concrete caller of `has(Set<T>, T)` fails in
`.tmp/generic-set-parameter-before-v61.log`: the solved parameter correctly
uses `JavaLinkedHashSet<T>`, but an old duplicated borrowing allowlist does not
recognize ordered sets and emits a phantom `&LinkedHashSet` facade value.
The classifier now reuses `java_collection_kind_for_name`, also covering sorted
and qualified native collection names. It borrows the original value rather
than materializing a copied temporary collection.

The extended runtime fixture reads and mutates the caller's set through generic
methods. All four collection-boundary fixtures pass in
`.tmp/generic-collection-borrows-v62.log` (22.30 seconds). The inventory regression
passes in `.tmp/native-collection-borrow-inventory-v63.log`, checking ten native
collection spellings. This borrowing edit follows the v30/v60 frozen snapshots;
their results are not evidence for this later edit.

The next dependent-bound batch has two failing neutral compile fixtures in
`tests/type_parameter_subtyping.rs`, recorded in
`.tmp/type-parameter-subtyping-before-v64.log`: a method parameter extending
another method parameter, and a method parameter extending an enclosing class
parameter. Both currently emit a type parameter as though it were a Rust trait.
The shared bound renderer needs explicit lexical owner/method scope, rather
than guessing type parameters from names or treating them as external traits.
Value widening must accompany the capability contract, reusing the existing
source subtype-conversion projection; dropping the bound alone is not a fix.

### Lexically scoped dependent bounds and value widening (in progress)

The shared generic renderer now receives the enclosing declaration as well as
the callable's own binders. A dependent `B extends A` bound projects to
`B: Into<A>`, retaining transitive declared upper capabilities, rather than
inventing a nominal trait named A. Method and constructor shells, source
class-bound contracts, hierarchy forwarders and ordinary trait method signatures
reuse that projector. Source-bound inventory ignores lexical type parameters
instead of requesting a class-bound trait for a shadowed source class.

Lowering proves the dependent relationship from the existing scoped bound
environment and reuses the typed source `Into` value projection. Nullable
carriers preserve None through a mapped payload conversion; non-null values
enter nullable destinations through the existing Some construction. The
existing successful-Result mapping now shares its value-map builder with
nullable widening, retaining propagation markers so a converted Option is not
mistaken for another Result needing a second `?`.

The interface-capability runtime fixture also exposed loss of resolved source
constructor type arguments: `new Owner<Value>()` became `Owner::new()` even when
dependent conversion bounds could not determine A uniquely. Source constructor
paths now retain those arguments using the existing generic-argument renderer,
including representation-aware argument storage, while keeping the constructor
on its nominal source owner rather than an Rc/enum storage wrapper. Native and
external facade construction retain their existing separate planners.

The v67 gate passes 12 generic-binding, five dependent-bound runtime and two
wildcard-contract tests. The v68 gate passes three constructor-interface-storage,
three diamond-inference, one nested-generic and six dependent-bound tests.
`.tmp/dependent-bounds-carriers-v69.log` passes seven runtime cases, including
nullable values and checked exceptions. The additional class-bound constructor
assignment fixture passes in `.tmp/dependent-class-bound-constructor-v70.log`.
The related unit selection passes 18 tests in
`.tmp/dependent-bounds-related-units-v71.log`.

A full unit/batch-translation snapshot completed in
`.tmp/dependent-bounds-broad-gates-v72.log`: 1,380 unit tests passed; batch
translation had 80 passes and five failures. The failures concern bulk collection
nullability, solved collection boundary carriers, and source hierarchy collection
carriers/equality. Their attribution remains open; they are not baseline exclusions.
A fresh untouched 104-file corpus measurement completed in
`.tmp/jgrapht-v31-run.log`. The v31 build took 88.693
seconds. Its frozen producer is `.tmp/new-java2rust-v31-dependent-bounds`, SHA-256
`398b0118ddad313aaebfe3cba8d0068e1d32709542b2a66f913aee208dca404b`.
The v31 crate reports 3,115 actual Rust errors (3,116 runner-counted lines including
the compiler summary), with zero translation or parse failures. There remain
110 invalid-marker occurrences in 38 files. Removing declaration errors exposed body type-checking that
the previous 76-error v30 baseline did not reach; the larger count is not evidence
that all newly visible failures are regressions or that none are regressions.
The resolver, translation, assembly and cargo-check phases took 37.158, 214.515,
60.587 and 12.081 seconds respectively, overlapping other test work. These are
not isolated speed measurements. The next root-cause batch concerns generic
capability propagation and unnecessary comparison bounds on collection reads.

After freezing those runs, a focused bound-renderer correction retains the
exact declared identifier for built-in-like binders such as Integer/Object,
instead of applying nominal type unboxing/erasure to them. The projector's
method- and class-scope checks pass in
`.tmp/dependent-bounds-binder-names-v73.log`. This is a bound-projection test,
not a claim that every unrelated native-name-shadowing slot case is solved.
The v31/v72 snapshots predate that small correction.

### Collection read capabilities (after v31)

The corpus includes repeated failures calling `iter` on generic semantic sets
without a key-comparison bound. A minimized neutral Java `Provider<T>` returning
`Set<T>` and a generic enhanced-for consumer reproduces this failure. The
pre-fix run `.tmp/collection-read-v74.log` failed with eight E0599 diagnostics:
the Java loop and empty-storage APIs demanded comparison capabilities they did
not use.

The runtime now separates constructors, Default, iteration, size, clearing and
predicate-based retention from actual comparison operations for
JavaSemanticHashMap, JavaSemanticHashSet, JavaSemanticTreeSet and
JavaLinkedHashSet. Insertion, searching, removal, extension and collection
equality retain their existing Java key/ordering requirements. The regression
`tests/collection_read_capabilities.rs` additionally exercises storage with a
type implementing no comparison or Clone traits, and comparison operations
with Java key/ordering contracts but no Rust Eq/Hash/Ord implementations.
This changes shared runtime support, not generated corpus output. No fresh
corpus error reduction is claimed before a new translator-produced run.
Verification: `.tmp/collection-capabilities-v75.log` passes the new regression,
three native collection algorithm tests and four stream collection return tests.
The strengthened regression, including comparison-contract behavior, passes
in `.tmp/collection-read-v76.log`. Formatting and whitespace checks pass.

### Generic method key-capability propagation (after v31)

`GraphTests.isWeaklyConnected` delegates to `isConnected`, but the former lacked
the JavaMapKey bound inferred for the latter. The neutral regression
`tests/generic_key_capabilities.rs` reproduces the underlying rule with two
generic wrapper layers around a helper inserting into a set. Its pre-fix
`.tmp/generic-key-v77.log` fails with E0277 at the wrapper's helper call.

Capability inference now traverses a visited worklist of selected source
methods and their generic binders. Binder correspondences come from the
resolver's substituted argument targets and return type, using its existing
recursive substitution collector rather than positional name guesses or a
second inheritance inference policy. Each reachable method/binder is visited
at most once per query, including cycles. Constructor capability checks now
use resolved constructed types, retaining inferred diamond arguments. Source
method lookup is shared with existing Clone capability propagation.

The focused unit `.tmp/generic-key-binders-v79.log` passes seven method cases:
key versus payload positions, reversed arguments through an inherited generic
interface, a method binder shadowing an owner binder, positive recursive
capability propagation, and a comparison-free recursive cycle. All 25 related
tests pass in `.tmp/generic-key-related-v80.log` (one collection-read regression,
three diamond inference, one key propagation, 12 generic method binding, eight
dependent-bound tests); all 30 assembly units
pass in `.tmp/generic-key-assembly-units-v81.log`. The first integration rerun
v78 collided with fixture-only Rust type names matching emitted names; those
driver types were renamed before v80. No generated output was repaired.

The v32 release producer is frozen at `.tmp/new-java2rust-v32-key-capabilities`,
SHA-256 `19b143809a49501f7c2487d6db140af277b4f0b151b12b9f5dbd91288ba6a06d`.
Its build passed in 1m 27s (`.tmp/generic-key-release-v82.log`). The pinned Java
checkout remains tracked-clean. The fresh 104-file run completed in
`.tmp/jgrapht-v32-run.log`, output root `.tmp/jgrapht-core-1.5.3-focused-v32`.
The crate reports 3,017 actual errors (3,018 runner-counted lines including the
compiler summary), 98 fewer than v31. Translation and parse failures remain
zero; invalid placeholders remain 110 occurrences in 38 files. The previous
69 generic semantic-set `iter` bound failures are absent. Weak connectivity's
wrapper now has the same JavaMapKey bound as its selected helper. Other error
counts also moved, so the net reduction is not a count of independent bugs fixed.
Resolver/translation/assembly/check phases took 35.986/208.896/59.728/11.613
seconds. Total runner time was 319.455 seconds, excluding the separately frozen
release build. These overlap focused tests and are not isolated benchmarks.

This batch propagates method-binder requirements. Receiver-only owner-binder
requirements and overly broad capability inference for read-only native calls
remain separate follow-up work; no claim is made that all generic requirements
or all 3,115 corpus diagnostics are solved.

### Concrete collection views and conversion agreement (after v32 freeze)

The two neutral bulk-collection regressions still fail on the current producer
in `.tmp/collection-shape-baseline-v83.log`. In the local/return case, signature
rendering selects `JavaSemanticHashSet<Option<BulkLocalEntryValue>>`, while
conversion materializes a structural Rust HashSet requiring Eq/Hash on the
hierarchy enum. This is a shape/rendering disagreement, not a Java source issue.

The signature view projector now records semantic hash-set views for concrete
source hierarchy elements using the existing slot-rendering carrier predicates.
Generic scoped selection and parsed-source collection precedence remain in the
same projector, including typed-local and concrete-return fast paths. Conversion
planning can therefore consume the semantic view before Rust type rendering.
Ordered and sorted sets are not silently changed to hash sets. The neutral bulk
copy test also checks at runtime that its null element survives copying.
The first shape-only gate `.tmp/collection-shapes-v84.log` passed three of six
related collection tests: the local-return failure was fixed, while declared
abstract Set refinements and bulk insertion still lost carrier decisions. The
separate semantic-equality gate v85 also failed its carrier assertion. These
were not accepted as pre-existing exclusions.

The related follow-up keeps the declaration's semantic hash-set ABI when a
concrete LinkedHashSet initializer/caller refines an abstract Set, retaining
learned element nullability. Explicit concrete LinkedHashSet declarations keep
their ordered carrier. The same normalization applies to parameters, fields and
locals. Bulk insertion planning now carries the receiver's equality view into
its owned-source argument slot, instead of reconstructing a structural HashSet
from only the nominal Java type. The v86 intermediate snapshot passes five of
six collection tests. Its remaining local assertion exposed body lowering
replacing a solved semantic view with the initializer's more-specific collection
kind; that refinement now preserves the semantic ABI. A missing ViewShape clone
in the bulk planner was caught at compile time in v87/v88 and corrected before
final verification. The complete batch is under verification in
`.tmp/collection-complete-shapes-v90.log`, with Java equality behavior in
`.tmp/collection-equality-final-v91.log` and related generic, collection-read and
stream regressions in `.tmp/collection-related-final-v92.log`.
The v90 gate passes five of six cases and v91 passes Java equality behavior.
The remaining bulk-copy case caught an invariant violation: its refined logical
Java type was LinkedHashSet while its solved view was semantic Set. The bulk
planner now takes both nominal type and view from the solved receiver shape,
rather than recombining that view with the earlier initializer type. This final
correction is under verification in `.tmp/collection-shape-invariants-v93.log`.
The v92 related gate caught a regression in generic mutating helpers: forcing
their caller-selected LinkedHashSet ABI to a semantic Set induced collection
copies/moves at calls. Declaration normalization is now limited to concrete
source-hierarchy carriers that already require semantic equality; generic
caller-selected carriers remain intact. The alias-preserving regression is
running in `.tmp/collection-carrier-aliasing-v94.log`, alongside read-only,
stream and transitive key-capability tests. Current-source collection and Java
equality gates are `.tmp/collection-current-v95.log` and
`.tmp/collection-current-equality-v96.log`.
The current six-case collection gate v95 passes, including the strengthened
runtime null-copy check. The full unit/batch snapshot completed in
`.tmp/collection-carrier-broad-v97.log`: all 85 batch tests pass, recovering
the five-failure v72 batch gate. Its one unit expectation was corrected and
verified separately, as recorded below. The Java source pin and tracked-clean
status were reverified after v32 completed.
Final focused verification is green: all six v95 collection tests, the v96
Java-equality runtime case, and all six v94 related tests (one read-capability,
one generic-key propagation, four stream collection returns). In particular,
the generic mutation regression again preserves `true:true:[b]`. Formatting and
whitespace checks pass. The newer carrier batch still needs a fresh corpus
measurement; v32's 3,017 actual errors remain the measured corpus baseline.
These source changes are after the frozen v32 producer and are not included in
that corpus measurement.

### Collection objects as Java-equality values (after v32)

The corpus calls `java_list_remove_by_map_key` with semantic sets as elements,
but the runtime sets lacked JavaMapKey themselves. The new neutral regression
`tests/collection_value_equality.rs` exercises sets/maps as values with keys and
payloads implementing Java equality but no Rust PartialEq/Eq/Hash. The initial
runtime-surface baseline `.tmp/collection-values-baseline-v98.log` fails with
12 missing-method/bound diagnostics.

A frozen-v32 translation of the Java generic `removeDistinct` fixture is kept
in `.tmp/collection-values-v32-baseline.rs` (unmodified producer output). Its
standalone rustc check `.tmp/collection-values-v32-compile.log` reproduces two
independent errors: the semantic set lacks JavaMapKey, and the generic method
body uses Rust `==` on T rather than Java equality. The current Java fixture
additionally routes removal through a generic wrapper without its own set.

Semantic hash sets, semantic tree sets and linked hash sets now implement
JavaMapKey using the existing shared order-independent set-entry comparison.
Semantic maps implement it with Java key lookup and Java value equality, sharing
entry traversal with the existing PartialEq implementation. Missing keys remain
distinct from present keys holding null. No global Vec JavaMapKey implementation
was added: Vec also represents Java arrays, whose equality is not List equality.
No claim is made that native nested collection carrier selection is complete.

List value removal now uses the existing lexical type-parameter lookup for
owner, method and constructor binders. Its capability inference reuses native
call planning, preserving the remove(index)/remove(value) overload distinction
and refusing source-defined List facade guesses. The dedicated unit in
`.tmp/collection-native-identity-v103.log` passes all four positive/negative
checks. Runtime/generic/stream verification is in
`.tmp/collection-value-scope-v102.log`; additional constructor/method-binding
gates are running in `.tmp/collection-scope-binding-v105.log`.

The older v97 broad snapshot completed 1,381 unit passes and one stale exact
carrier expectation: its abstract Set local was expected to be LinkedHashSet,
while the corrected shared ABI is semantic Set. That assertion now explicitly
checks the semantic view and semantic rendered type; the adjacent explicit
LinkedHashSet assertion remains unchanged. The focused rerun is
`.tmp/collection-shape-unit-v104.log`. All 85 batch cases in v97 pass in 434.92
seconds; v97 predates this compositional equality batch.
The current v102 gate passes six tests: the collection-value runtime fixture
(including nested set/map keys and null-valued map comparisons), transitive
generic capabilities, and all four stream return/mutation cases. The updated
shape unit passes in v104. Both preserve exact checks rather than adding skips.
The release/frozen-producer/corpus workflow is running: release log
`.tmp/collection-values-release-v106.log`, planned producer
`.tmp/new-java2rust-v33-collection-values`, corpus log `.tmp/jgrapht-v33-run.log`
and output root `.tmp/jgrapht-core-1.5.3-focused-v33`. This combines the preceding
carrier-shape fixes with compositional equality and callable-scoped list removal.
The v33 run completed with 2,989 actual compiler errors (2,990 runner-counted
lines including the compiler summary), 28 fewer than v32. Translation/parse
failures remain zero, with 110 invalid occurrences in 38 files. The V Eq/Hash
diagnostic categories each drop by 12; this net diagnostic reduction is not a
count of independent translator bugs. Resolver/translation/assembly/check times
are 38.571/212.777/60.138/12.128 seconds, with a 327.101-second total excluding
the frozen release build. These overlap other tests and are not isolated speed
measurements. The Java checkout remains tracked-clean.
The frozen v33 producer SHA-256 is
`12f443177039c4e216d9920a894893f387cedda72aff1164120e66a253a2fddb`;
its release build completed in 1m 31s. It does not include the receiver-owner
capability extension described next.
The additional v105 scope/binding gate passes all 10 constructor-overload cases
and 12 generic-method cases. Together with v102 and the two focused unit gates,
this batch has 30 passing focused parent tests, with no ignored cases.

### Receiver-owner capability binding (after v33 freeze)

The corpus's generic wrappers can call methods such as a partitioning object's
predicate that introduce owner-parameter key/Clone requirements absent from its
fields or constructor. A neutral two-parameter Table reproduces the rule:
`remember` inserts Key into a set, `indirect` forwards to it, and a generic static
wrapper calls `indirect`. Its payload class intentionally has no JavaMapKey
contract. `.tmp/receiver-key-baseline-v107.log` fails at the implicit forwarding
call because Key's method-level capability was not propagated.

The capability worklist now follows unshadowed declaring-owner binders as well
as method binders. Argument/return bindings reuse the resolver's substitution
collector; explicit receiver bindings reuse its owner-argument policy and the
shared source-ancestor substitution mapper, including reordered inheritance.
The same receiver-binding helper is reused by class-level Clone propagation.
Once a direct key requirement is proven, the visitor skips redundant callee
analysis for that expression. No new fixpoint iteration is introduced.

Runtime/generic/constructor verification passes all 24 tests in
`.tmp/receiver-key-related-v108.log`; all 32 assembly units, including selective
key-versus-payload, inherited argument reversal, method shadowing and pure
overload checks, pass in `.tmp/receiver-key-assembly-v109.log`. These are 56
focused passing parent tests. A new full unit/batch snapshot is running in
`.tmp/receiver-capabilities-broad-v110.log` because the shared receiver-binding
change also affects Clone propagation. No new corpus run is made after just
this batch; the completed v33 count remains the authoritative corpus baseline.
The v110 snapshot passes all 1,384 unit tests in 424.30 seconds and all 85 batch
translation cases in 414.33 seconds. This snapshot predates the inherited-call
resolution batch below.

### Inherited generic call frames and simultaneous substitution (after v33)

Three neutral resolver regressions fail in `.tmp/inherited-call-baseline-v111.log`:
reordered parent arguments retain the wrong receiver frame; repeated child and
parent binder names are substituted sequentially; and an inherited method's own
binder is overwritten by specialization of its declaring class. The initial
simple runtime fixture passed despite those incorrect resolved types, so its
final version additionally selects an overload using the inherited return type.

Hierarchy edges now evaluate all parent arguments in the old child scope before
replacing that scope with the parent's bindings. This also bounds substitution
state by the current owner's arity rather than retaining every ancestor's
aliases. Method symbols already specialized into the receiver frame are bound
against that receiver's actual parameters. Specialization reuses the source
member substitution module's binder filter, including method bounds rather
than only parameter/return types. The shared filter borrows its input map when
no shadowed entry exists, allocating only when filtering is necessary. No timing
speedup is claimed without a fresh measurement.

Focused checks cover inherited overload behavior, reordered arguments, two
consecutive reversals, declaring-owner/method shadowing, and both acceptance and
rejection under an inherited dependent method bound. The intermediate v112 gate
passes 29 tests, with the strengthened v113 inherited selection passing all five
cases. The final Cow-filter/source-scoping unit gate v114 passes 11 resolver and
two substitution tests. The complete current integration gate passes all 41
tests in `.tmp/inherited-current-related-v115.log` (generic methods, constructor
overloads, key capabilities, nested representations, dependent bounds and
wildcard bounds). The cross-layer inherited unit selection passes 18 tests in
`.tmp/inherited-cross-layer-units-v116.log`; it overlaps the resolver selection
and is not an additional 18 distinct tests. Formatting and whitespace checks pass.
These checks do not claim that every possible cross-scope generic capture or
inherited receiver representation is solved. The v33 producer excludes this batch.

### Reference casts retain the Java null value (after v33)

The corpus still contains `?` inside plain conversion closures. A minimized
Object-to-String cast exposed an earlier prerequisite: `(String) value` lost
the nullability of a nullable Object operand, returned plain String and made
valid caller null comparisons emit compile_error. The initial diagnostic
fixture fails in `.tmp/nullable-cast-baseline-v117.log` with two unsupported-null
comparison errors. It also revealed that failed casts were not yet propagated
as catchable ClassCastException by an otherwise infallible helper.

This batch fixes the null-preservation rule, not all failed-cast propagation.
Return/local nullability analysis follows reference casts to their operands;
expression shapes, cast conversion targets and ownership-preserving cast
targets share one shape-projection helper. It preserves an existing nullable
carrier or a literal null, while primitive casts/unboxing keep their required
value semantics. The shared null-type test reuses the resolver's existing
classification rather than another spelling policy.

The neutral runtime test `tests/nullable_cast_propagation.rs` checks a null cast,
the exact present String payload, and a cast-null local. Its name explicitly
describes null/present values; it does not claim a failed cast is catchable.
The first runtime case passes in v118. The shared shape projection selection
passes four unit tests in `.tmp/nullable-cast-shape-units-v120.log`. The complete
current gate, including the literal-null local, passes all 18 integration tests
in `.tmp/nullable-cast-final-v121.log`; the earlier v119 snapshot is intermediate
and caught `Some(None)` when the null literal was not yet recognized by the
shared cast projection. The final four shape units pass in
`.tmp/nullable-cast-final-units-v122.log`, for 22 passing parent tests.
ClassCastException fallibility and carrier-aware fallible Option mapping remain
follow-up work. No invalid marker was renamed or suppressed, and the v33 corpus
producer excludes these changes.
The completed full unit/batch snapshot in
`.tmp/cast-and-inheritance-broad-v123.log` passes all 1,386 unit tests (445.79s)
and 85 batch tests (406.63s).

### Completed v34 corpus baseline

Release log `.tmp/cast-and-inheritance-release-v124.log` records the 1m28s build
of frozen producer `.tmp/new-java2rust-v34-cast-inheritance`, SHA-256
`730ac7679f3d3468903a28294672693f01198f954458bcf931abc1eff089e459`.
The completed corpus log `.tmp/jgrapht-v34-run.log` and output root
`.tmp/jgrapht-core-1.5.3-focused-v34` record 2,892 actual compiler errors
(2,893 including the runner-counted summary), down from 2,989 in v33.
All 104 selected files translated, with zero translation or parse failures;
110 invalid patterns still affect 38 files. This is not a successful Rust build.
Phase timings are resolver 38,130ms, translation 216,559ms, crate assembly
60,919ms and cargo check 11,649ms, total 330,574ms excluding producer build.
This combines receiver-owner propagation, inherited-call resolution and
reference-cast null preservation. It predates the checked-cast exception-flow
and fallible optional-payload mapping batch; no reduction is attributed to
that newer work without another corpus measurement.

### Checked-cast effects and fallible nullable payload adapters (after v34)

Exception planning now queries the existing conversion plan for runtime-checked
reference narrowing, including optional element conversions. One shared dispatch
classification covers concrete, trait, boxed-trait, erased-object and erased-object
to closed-hierarchy checks. Identity, null-literal and numeric casts do not seed
ClassCastException effects. Named callables and anonymous overrides participate
in the existing call-graph/override fallibility solver; the factory does not
inherit an anonymous method body's deferred effects.

The typed statement-effect traversal is shared with arithmetic checks. It learns
scoped local, for-init, enhanced-for, resource and catch bindings, and switch
declarations in source order. Context-aware local learning reuses the common
value-shape projector instead of treating an unsolved Object local as concrete.
Existing callers without a shape context retain their old conservative fallback.

Generated Option payload maps carry an IR marker distinct from iterator/Result
maps. In an exception-propagating context they use a typed Result closure and
transpose before propagating, so failed checks inside the mapping closure remain
catchable. Generated cast-failure branches are likewise marked in the IR rather
than identified by arbitrary user panic strings. Null-test adapters preserve
evidence that the operand was already unwrapped, avoiding a second question-mark
operator on the resulting boolean. Rewriting is regression-tested for idempotence.

The pre-final neighboring gate `.tmp/checked-cast-neighbors-v132.log` passes all
20 parent tests: checked call identity (7), nullable/caught casts (3), dependent
type bounds (8) and wildcard contracts (2). The scoped effects and anonymous
override contract checks pass in `.tmp/checked-cast-plan-v134.log` (18 exception
planner tests), with 9 existing catchable-null rewrite tests also passing.
The final snapshot in `.tmp/checked-cast-final-v136.log` passes all 44 cast-focused
unit tests (17.74s), 18 exception planner tests (0.48s, overlapping the cast
filter), and the generated fallible optional adapter test. The final runtime
cast gate in `.tmp/checked-cast-final-values-v137.log` passes all 3 tests (19.71s).
`git diff --check` also passes.

The completed release/frozen-producer/corpus workflow uses release log
`.tmp/checked-cast-release-v138.log`, producer `.tmp/new-java2rust-v35-checked-casts`
(1m27s build, SHA-256
`dab1c0b85dc620d68c73d8bc9867bc96db157d2dd834b57b70e52dc194724c29`),
corpus log `.tmp/jgrapht-v35-run.log` and output root
`.tmp/jgrapht-core-1.5.3-focused-v35`. It reports 2,881 actual compiler errors
(2,882 runner-counted including the summary), down from 2,892 in v34. All 104
files translate with zero translation/parse failures. Invalid patterns decrease
from 110 in 38 files to 108 in 37 files. The crate still fails cargo check.
Resolver/translation/assembly/check timings are 36,723 / 211,620 / 59,908 /
11,844ms; total 323,340ms excluding producer build. The frozen producer predates
the following anonymous-object Display/storage batch.

An additional neighboring gate `.tmp/checked-cast-anonymous-v133.log` fails all
13 selected anonymous-source-class fixtures. The common failure is carrier
Display forwarding through an anonymous dispatch trait lacking Display. This
is independently reproduced with the frozen pre-batch v34 producer on the
minimized `.tmp/anonymous-display-baseline.java`, output
`.tmp/anonymous-display-v34-baseline.rs` and rustc failure log
`.tmp/anonymous-display-v34-check.log`. The larger captured-state fixture also
reports ownership/type mismatches; those additional failures need classification.
This gate is not green and is not attributed to the cast batch without evidence.
Callers catching a callee's unchecked exception can still conservatively retain
a Result ABI: the call graph's handled-edge metadata currently tracks declared
checked exception types. Precise unchecked effect subtraction is not claimed.

### Anonymous-object formatting and shared assignment construction (after v35)

The virtual anonymous-class dispatch surface now explicitly requires Display,
and the generated object adapter implements that contract. An anonymous
toString override is formatted through its own method (including overrides of
Object.toString absent from the named base's explicit surface). Otherwise the
adapter delegates to the base object's existing formatter. The named-class and
anonymous formatter bridges share the existing String/nullable/exception result
adaptation instead of introducing another formatting policy. No clone is added
merely to satisfy the new Display contract.

Fresh shared-object construction now runs at the common assignment-slot
boundary, reusing the structural/idempotent constructor adapter previously
called by selected field/local/argument renderers. This fixes anonymous field
initializers and local/field replacements without adding an anonymous-specific
handle rule. The selected storage placement is used for both plain and fallible
constructors; it is no longer discarded by a second facts-only lookup after
selection from the representation plan. Existing shared references are not
wrapped again, and the owned/copy target requirement preserves borrowed slots.
The final refinement preserves discharged Result metadata through construction,
both for propagated exceptions and the uncaught-exception panic boundary. A
later slot must not append another question-mark or unwrap adapter to the plain
Rc/Arc payload. The extended structural regression passes in
`.tmp/anonymous-object-result-unit-v145.log`.

`.tmp/anonymous-object-storage-v141.log` passes 7 parent runtime tests:
anonymous inherited/overridden formatting (1), reference retention and fresh
field/local replacement (1), constructor phase ordering (2), and interface/field
storage contracts (3). The two focused structural tests in
`.tmp/anonymous-object-unit-v143.log` pass, covering the virtual Display surface
and Rc/RefCell plus Arc/Mutex placement, fallible constructor ordering, existing
references, and repeated adapter application. `git diff --check` passes.

The first neighboring rerun `.tmp/anonymous-display-neighbors-v140.log` improves
the 13 selected anonymous-class fixtures from 0 passes to 8 passes after only
the Display fix. The other five failures expose additional missing array-bounds
exception propagation and captured-object storage mismatches; the driver
assertions were not weakened. The post-storage rerun
`.tmp/anonymous-object-neighbors-v142.log` still has 8 passes and 5 failures:
four are array-bounds effect gaps, while the large captured-state fixture now
has no product-code compile errors and only three stale driver Result
expectations. Those factory captures are solved non-null and have a plain ABI;
the driver now checks the same values (12, 17, 24) without requiring a spurious
Result wrapper. All reference-sharing and mutation checks remain. The rerun
`.tmp/anonymous-object-captures-v147.log` passes (94.66s).

The broad snapshot `.tmp/anonymous-object-broad-v144.log` completes 1,390 unit
passes and one stale exact IR assertion. That assertion expected generated
optional mapping receivers without the new OptionalMapReceiver marker from the
v35 batch. Its six exact-tree comparisons now include the marker, retaining
the string/slice/owned mapped-value and slot-shape assertions. The focused
correction passes in `.tmp/optional-map-ir-assertion-v148.log`.
The broad unit failure prevented the combined command's batch phase; the
separate batch gate `.tmp/anonymous-object-batch-v149.log` passes all 85 tests
(119.50s). The final constructor/capture neighbors in
`.tmp/anonymous-object-results-v146.log` pass all 12 tests (anonymous formatting
and storage plus 10 constructor overload cases). The frozen v35 corpus producer
excludes this batch, so its 2,881-error count remains the measured baseline
until the following combined corpus gate completes.

### Array-bounds effects independent of nullability (after v35)

An array's solved non-null shape cannot prove that an index is in range. The
fallibility plan now records ArrayBoundsThrow for immediate array access in
named method/constructor bodies and anonymous method bodies, independently of
NullReceiverThrow. It reuses the shared statement-effect traversal and existing
call-graph/override contract solver. Deferred lambda and anonymous method bodies
do not make their factory fallible; anonymous field initializers execute
immediately and are included. There is no JGraphT-specific recognition and no
safe-index proof optimization is claimed.

The arithmetic, null-receiver, checked-cast and array-bounds intrinsic handler
queries now share stdlib's JDK subtype closure, rather than maintaining four
lists of ancestor answers. Qualified IndexOutOfBoundsException catches array
bounds failures; unrelated handlers do not. The final shared-handler gate
`.tmp/array-bounds-shared-catches-v159.log` passes all 20 exception-planner tests.

`.tmp/array-bounds-final-values-v160.log` passes all 6 runtime tests: three array
effect/order cases and the three nullable/checked-cast cases. Coverage includes
negative and oversized reads through arithmetic and call propagation, failed
writes, local handling without an escaping Result ABI, and simple versus
compound assignment RHS order. The initial focused results are also recorded
in `.tmp/array-bounds-effects-v150.log` and `.tmp/array-bounds-order-v154.log`.
The previously failing anonymous-source-class subset now passes all 13 fixtures
in `.tmp/array-bounds-anonymous-v151.log` (94.84s), without removing the invalid
index/exception assertions.

The broader `.tmp/array-bounds-broad-v153.log` records 1,391 unit passes and
one stale unchecked-index emission expectation. That assertion now requires a
checked lookup while keeping the shared self-owner guard alive, with the
mutable-versus-shared guard assertions preserved; its focused correction
passes in `.tmp/array-bounds-owner-guard-final-v157.log`. The separate batch
gate `.tmp/array-bounds-batch-v158.log` has 84 passes and one stale expectation
of a panic-only null check inside a now-fallible array caller. It now requires
the catchable NullPointerException adapter and retains the original
getter/argument shape checks; the correction passes in
`.tmp/array-bounds-getter-assertion-v161.log`. The final full snapshot in
`.tmp/array-bounds-final-broad-v162.log` passes all 1,393 unit tests (135.96s);
its batch phase also passes all 85 tests (115.15s).

The combined frozen producer `.tmp/new-java2rust-v36-object-array-effects`
was built in 1m26s, release log `.tmp/array-bounds-release-v155.log`, SHA-256
`ab8996c35c0bd2405284677d678bb196b00b9aaded04eb8dd6ede6faf76f0ea6`.
The completed 104-file corpus gate is recorded in `.tmp/jgrapht-v36-run.log`
with output root `.tmp/jgrapht-core-1.5.3-focused-v36`. It includes the anonymous Display,
assignment storage/Result metadata, and array effect-discovery changes. The
equivalent shared intrinsic-handler consolidation happened after this freeze.
The compiler reports 2,881 actual errors (2,882 runner-counted including the
summary), unchanged from v35. All 104 files translate with zero translation or
parse failures; invalid patterns remain 108 in 37 files. This batch fixes
verified translator/runtime regressions but does not reduce the measured
corpus compile-error count. Resolver/translation/assembly/check timings are
37,855 / 282,910 / 109,586 / 16,107ms; total 450,003ms excluding producer build.
This run overlapped broad verification, so these timings are not an isolated
performance comparison. Coverage of other array carrier and initializer/deferred
execution paths must still be verified separately; this gate does not declare
all Java array semantics complete.

The next compile-focused candidate is generic Clone capability precision:
v36 still reports 124 E:Clone and 111 V:Clone failures. For example, an emitted
generic getter returning Option<Box<dyn Graph<V,E>>> requires V/E:Clone even
though the generated Clone implementation for Box<dyn Graph<V,E>> is unbounded
in those arguments. Generic wrapper derives and getter capability inference
need corpus-neutral validation together; adding blanket Clone bounds to all
callers would not establish the right requirement.

### Generic Clone capability batch (after v36)

The producer now uses actual planned field storage to distinguish copying an
interface/reference handle from copying a generic payload. Generic source
structs that were already selected as cloneable receive field-wise Clone
implementations with obligations on the stored field types, instead of a
derive that unnecessarily requires every mentioned type argument to implement
Clone. Getter and interface-implementation capability analysis share the same
storage context and retain genuine field and method-body copying requirements.
The analysis cache keeps immutable planning references outside its mutable memo
to preserve the lowering context's lifetime covariance. No generated output or
Java corpus source is patched.

`tests/generic_clone_capabilities.rs` is a corpus-neutral generated-crate runtime
regression: a non-Clone payload is accepted in interface handles and wrappers,
cloned wrappers preserve shared state, and methods that actually duplicate a
payload still work with their required Clone bound. It additionally checks a
type parameter named Integer, its use in collection storage, and the distinct
qualified platform type java.lang.Integer. Binder normalization reuses the
shared binder-aware Java substitution pass; the parser preserves explicit
platform-wrapper qualification rather than capturing it as the lexical binder.
This does not claim complete normalization for every local/anonymous binder or
all platform type spellings.

The runtime regression passes in `.tmp/generic-clone-qualified-values-v171.log`.
All 40 neighboring tests pass in `.tmp/generic-clone-neighbors-v172.log`
(constructor overload scope, generic key capabilities, generic method binding,
dependent type-parameter bounds, and wildcard contracts). The new storage
carrier unit regression initially exposed a conservative mismatch for rendered
generic trait objects (`Option<Box<dyn Source<T>>>`); the follow-up shares the
existing raw carrier and clone-box recognition with typed carrier analysis.
The carrier regression and all three binder-substitution unit tests pass in
`.tmp/generic-clone-carriers-v174.log`. The first broad run,
`.tmp/generic-clone-broad-v175.log`, has 1,394 unit passes and one real failure:
the metadata-free compatibility API had constructed an empty signature plan,
which cannot project hierarchy fields. That API now retains its conservative
analysis; Display lowering instead shares the actual solved-context cache.
The existing owned-carrier regression passes unchanged for this correction
in `.tmp/generic-clone-owned-carrier-v179.log` (17.41s). The repeated broad gate
`.tmp/generic-clone-final-broad-v180.log` passes all 1,395 unit tests (134.63s);
its batch phase also passes all 85 tests (123.28s).

Extending the non-Clone runtime regression through a generic superclass exposed
two related omissions. Interface collection and method-candidate lookup used
`Parent<T>` as a declaration name, losing inherited interfaces and forwarding
bodies. Lookup now erases the type arguments at the shared declaration boundary,
while the existing ancestor substitution retains and composes the arguments.
The original evidence is `.tmp/generic-clone-inheritance-v176.log` (missing impl)
and `.tmp/generic-clone-inherited-interfaces-v177.log` (missing runtime forwarding).
The expanded runtime regression, including the unchanged inherited behavior
assertions, passes in `.tmp/generic-clone-inherited-final-v181.log` (21.97s).
Its four neighboring default-interface tests and two nested-interface identity
tests also pass there (25.88s and 47.06s respectively).
The completed v37 selected-slice gate reports **2,896 actual Rust errors**
(2,897 runner-counted including the summary), up 15 from v36's 2,881. E:Clone
diagnostics decrease from 124 to 110 and V:Clone from 111 to 100, but plain type
mismatches increase from 479 to 496; V:JavaMapKey increases from 36 to 56,
and lifetime/other diagnostics also change. Thus the measured batch does not
improve the total compile-error count, despite its proven capability fixes.
The new diagnostics require root-cause inspection, not a success claim from
the reduced Clone subset. All 104 files still translate with zero parse or
translation failures; invalid patterns remain 108 in 37 files.
The untouched pinned Java oracle has been rerun successfully for this snapshot
in `.tmp/generic-clone-java-oracle-v183.log`, with its 17 matching records and
input hashes under `.tmp/jgrapht-java-oracle-v37`.
The frozen producer `.tmp/new-java2rust-v37-generic-clone` was built in 2m13s
(`.tmp/generic-clone-release-v182.log`), SHA-256
`3978931caa9d0495cfce6e78d5d673db444875d0938a35909aa1d87bbb44985e`.
It includes the completed generic Clone, binder normalization, and shared
declaration lookup fixes. The release build overlapped broad tests; its timing
is not an isolated performance comparison. The completed selected-slice gate is
`.tmp/jgrapht-v37-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v37`. Resolver/translation/assembly/check times
are 36,497 / 219,291 / 60,269 / 11,777ms; total 331,059ms excluding the producer
build. Translation overlapped focused follow-up tests, so comparison to v36's
450,003ms is not controlled evidence of an algorithmic speedup.

One concrete newly reported diagnostic cluster is a generic static caller
invoking a bound-requiring instance method on a freshly constructed receiver:
v37 compile.log lines around 14,740 and 15,077 show method-local EMethod lacks
Clone at chained get_path calls in BFSShortestPath and BellmanFordShortestPath.
The caller signature/call spelling already existed in v36, so this is not proof
that the Java rule regressed in this batch; changed earlier errors can expose
later obligations. The general next probe is capability propagation through
`new GenericOwner<CallerParameter>(...).method(...)`, using a neutral fixture,
not a blanket bound on JGraphT methods.

### Follow-up: inherited members in a differently named child frame

While v37 runs from its immutable producer, the runtime fixture was extended
with `ReboundHandle<Unused, Payload> extends HandleBox<Payload>`. Unlike the
earlier same-name `T` case, this exercises a changed argument position and
spelling. `.tmp/generic-clone-ancestor-rebinding-v184.log` exposes two invalid
dispatch conversions during superclass constructor replay; inspection also
finds a panic-only incompatible interface forwarding body. Both originate in
using the parent's unspecialized formal type after moving into the child frame.

Interface candidate comparison now uses the existing shared source-ancestor
substitution and binder-aware member specialization for class as well as
interface ancestors. Covariant-return decisions use that same effective
declaration. Constructor field plans gain a shared specialization operation
that preserves solved slot metadata and nullability; superclass replay applies
the ancestor map before lowering its argument expressions. No name-based
positional inference or corpus-specific coercion exception is added.

The follow-up `.tmp/generic-ancestor-slots-v185.log` passes the expanded runtime
regression (22.16s), all 10 constructor-overload tests (21.62s), and all 18
generic-method binding tests (20.33s). The unit regression in
`.tmp/generic-ancestor-plan-unit-v186.log` also passes simultaneous substitution
and slot metadata checks for assigned, initialized, and default constructor
fields. A further two-hop argument-permutation runtime probe fails in
`.tmp/generic-ancestor-multihop-v187.log`: recursive construction still converts
between an intermediate parent's binders and the final emitted child's binders.
The initial final-frame attempt fails in
`.tmp/generic-ancestor-final-frame-v188.log`: current_type deliberately follows
the lexical superclass during replay, so it is not the emitted owner. Lowering
now tracks a separate physical constructor-frame owner, initialized at each
explicit/implicit constructor entry and retained through recursive replay.
Slot specialization and final field-shape recovery share its ancestor map,
without changing the lexical context needed for private/super dispatch. The
follow-up `.tmp/generic-ancestor-physical-frame-v189.log` passes all 10
constructor-overload checks but still fails the runtime fixture: re-looking up
the physical owner's spelling can be ambiguous across primary/analysis copies.
The context now retains the actual emitted TypeDecl reference, and solved
field-owner recovery first uses exact source identity. This preserves the
declaration selected at constructor entry instead of inventing a second
name-resolution policy. The focused correction passes the expanded runtime
fixture in `.tmp/generic-ancestor-frame-identity-v191.log` (28.54s), including
the two-hop permutation and shared-state assertions.
The broad run `.tmp/generic-ancestor-broad-v190.log` was started before this
last declaration-identity correction; it passes all 1,396 unit tests (133.66s)
and all 85 batch tests (116.62s), but cannot alone verify the final source.
These source changes are **not** in frozen v37; its measured result
must not be attributed to them.

### Follow-up: constructed receiver capabilities and owned locals

The neutral static-caller probe in `tests/generic_clone_capabilities.rs` first
fails with missing U lifetime requirements in
`.tmp/generic-constructed-receiver-v192.log`: an inferred Clone bound alone
does not satisfy the existing Any-constrained inherent source-class impl.
The expanded source-handle case in
`.tmp/generic-constructed-receiver-source-v193.log` also exposes reuse of an
owned generic local without a Clone requirement or emitted copy. Its driver
initially supplied a Box where the generated ABI borrows the source trait;
the driver was corrected to the borrowed ABI, without changing Java behavior.

Callable capability analysis now follows generic receiver instantiations even
when none of the invoked method's arguments mentions the caller variable. It
uses shared source-call selection, receiver/ancestor substitution, and the
existing method Clone analysis with its memo/recursion guards. Each owner
parameter is mapped separately, rather than propagating every receiver bound
to every caller parameter. Source-class lifetime inference also considers
constructed and called receiver types, not only declared parameter types.
Owned direct-T locals now participate in the same reuse count as direct-T
parameters; a shared local-name visitor retains the existing nested-statement
coverage. This extension does not classify arbitrary interface/reference
carriers mentioning T as payload copies.

Both runtime fixtures pass in
`.tmp/generic-constructed-receiver-locals-v195.log` (22.33s). The strengthened
fixture adds an unused non-Clone receiver argument, repeated reads of a copied
interface local, and a runtime source payload check. Those plus 46 neighboring
constructor/generic/default/nested/wildcard tests all pass in
`.tmp/generic-capability-neighbors-v196.log` (48 tests total). The strengthened
two runtime tests pass there in 23.37s; all 46 neighboring tests also pass.
The combined-source broad gate `.tmp/generic-capability-broad-v197.log` passes
all 1,396 unit tests (130.64s) and 85 batch tests (126.61s).

The frozen producer `.tmp/new-java2rust-v38-generic-capability-frames` was built
in 2m23s (`.tmp/generic-capability-release-v198.log`), SHA-256
`0bf8190049bd1e2c650e0b5158912fbd6493624c7716d2f08d78ac5722df46a1`.
The completed corpus gate `.tmp/jgrapht-v38-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v38`, reports **2,842 actual Rust errors**
(2,843 including the summary), down 54 from v37. All 104 files still translate
with zero parse/translation failures; invalid patterns remain 108 in 37 files.
E:Clone is down to 86 and V:Clone to 94. The inspected BFS static wrapper now
correctly carries EMethod:Clone+Any; remaining method-binder Clone diagnostics
include different graph-builder generic-bound/factory paths, not proof that
the constructed-receiver example is still broken. Resolver/translation/
assembly/check timings are 36,433 / 209,062 / 59,426 / 11,870ms; total 320,033ms
excluding the producer build. This overlapped follow-up tests and is not a
controlled performance comparison. The Rust graph oracle is still unavailable
because the assembled crate does not compile.

### Follow-up: receiver-only key requirements and owned field arguments

The neutral `receiver_only_key_requirements_reach_callers_and_interface_forwarders`
test in `tests/generic_key_capabilities.rs` fails in
`.tmp/generic-key-receiver-only-v199.log`. A class method acquires JavaMapKey
through a constructed helper; a no-argument caller and the source-interface
forwarder omit that obligation. The fixture also exposes copying a generic
field into constructor/collection arguments without its required Clone bound.

The existing key-capability worklist now retains each callable's lexical owner.
Receiverless/explicit-this calls use that owner's formal receiver frame and the
shared receiver/ancestor substitution, retaining method-binder shadowing and
the visited-node cycle guard. Trait implementation requirements generalize the
existing Clone traversal to a common Clone/JavaMapKey capability inventory;
both kinds compose inherited bindings, rather than introducing another
interface or hierarchy walker. Field-copy inference now includes argument
positions, using the actual stored field carrier and lexical parameter/local
lookup so copying a handle or a shadowing parameter does not automatically
impose a payload Clone bound.

The initial `.tmp/generic-key-trait-batch-v200.log` passes two clone and three
key runtime tests. Ownership-aware field argument checks additionally use the
shared call/constructor argument planner, preserving borrowed argument slots
instead of assuming every field argument is copied. The stronger shadowing
probe in `.tmp/generic-key-owned-slots-v201.log` exposes another copy boundary:
a borrowed identity-call result is adapted into an owned direct type-parameter
return, but the caller omitted Clone. Source capability analysis now consults
the solved producer/consumer return slots for that direct generic case.

All five focused runtime tests pass in `.tmp/generic-key-return-slots-v202.log`
(24.40s clone group, 20.68s key group). The fixture verifies that unrelated
payloads need neither Clone nor JavaMapKey and a custom key needs no Rust
Eq/Hash. Its shadowing control distinguishes a non-Clone parameter passed to a
void borrowed-argument call from a Clone payload actually copied into an owned
return; the Rust driver follows the solved borrowed argument ABI. This is not
a claim that all nested return/lifetime propagation is solved.

The 49-test neighboring gate `.tmp/generic-key-neighbors-v203.log` passes in
full. The combined-source `.tmp/generic-key-broad-v204.log` also passes all
1,396 unit tests (136.80s) and all 85 batch tests (135.16s).

The frozen producer `.tmp/new-java2rust-v39-key-copy-capabilities` was built in
2m18s (`.tmp/generic-key-release-v205.log`), SHA-256
`6bb79412a5d0ce94908c19738a3b92b1d0a50638cf1a80698a8ea99b1904220a`.
Its selected-slice gate completed in `.tmp/jgrapht-v39-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v39`. This frozen producer includes the key
worklist owner context, shared trait capability inventory, argument-slot field
copy checks, and direct borrowed generic return-copy requirement. These changes
are not in v38. v39 reports 2,854 actual Rust errors (2,855 including the
compiler's summary line), 12 more than v38, and still 108 invalid patterns in
37 files. Parsing and translation have zero failures across all 104 files.
The focused tests establish the new rules, but the aggregate corpus result
does not establish an improvement. Resolver/translation/assembly/check times
are 37,092/212,421/60,044/11,846ms; total 324,777ms. These overlapped tests and
are not a controlled performance comparison.

The next neutral probe, `nested_diamond_under_wildcard_return_keeps_constructor_witnesses`
in `tests/diamond_inference.rs`, fails in
`.tmp/nested-diamond-wildcard-baseline-v206.log` (19.23s). It captures a source builder
whose third parameter is bounded by a generic source interface, and a factory
returning a wildcard-qualified concrete implementation via nested diamond new
expressions. The related v38 Multigraph factory output contains a nonexistent
`.downcast::<G>()` on a concrete constructed value: constructor witnesses and
source return context must be retained, not repaired with a special cast.
This new probe is not part of the earlier green gate snapshots.
Its retained generated crate is
`.tmp/test-tmp/new_java2rust_nested_diamond_wildcard_return_1792223111782418300`;
the error is the same nonexistent downcast on ConcreteStore, now to the
declaration's uninstantiated Actual parameter. Resolver inspection finds that
target typing is applied to field/local initializers and nested constructor
arguments, but not method returns. In addition, the existing target helper
can overwrite argument-inferred witnesses with an empty argument list when
given a wildcard/non-nominal target. The next batch should preserve partial
argument inference, apply the callable's return context without leaking it
into nested callable scopes, and materialize wildcard-compatible constructor
witnesses through the existing substitution/assignability mechanisms. Raw
constructor syntax must remain distinct from diamond syntax. These are
inference gaps, not justification for a synthetic downcast implementation.

### Contextual diamond construction batch

The resolver now threads the current callable's return target through control
flow, and uses a separate functional-interface return target inside typed
lambdas. Assignment targets and ternary/switch alternatives also supply
constructor context. Source constructor formals are projected through the
existing superclass mapping before target constraints are collected, rather
than copying target arguments positionally. Existing argument substitutions
provide witnesses for wildcard bounds; shared assignability checks select
compatible witnesses. Unusable targets, including Object, preserve existing
argument inference instead of erasing it. Raw new syntax remains distinct.
Nested constructor argument metadata is refreshed after targeting.

Assembly's existing source-construction capability inference reads the resolved
constructed type instead of the parsed diamond syntax, so obligations refer to
the same instantiated frame as constructor lowering. No corpus-specific names,
casts, Java edits or generated-Rust repairs are introduced.

All seven diamond tests pass in `.tmp/contextual-diamond-contracts-v209.log`.
Two fixtures check generated crates, including the formerly failing nested
builder case. Other checks cover upper/lower/unbounded wildcard witnesses,
Object context, reordered superclass parameters, assignments, conditional
returns and lambda return-scope isolation. This is not a claim of complete
Java inference: arbitrary recursive bounds and all higher-order capability
dependencies still require evidence. Neighboring and broad gates are recorded
in `.tmp/contextual-diamond-neighbors-v210.log` and
`.tmp/contextual-diamond-broad-v211.log`: all 49 neighboring tests, 1,396 unit
tests and 85 batch tests pass. The v40 producer was frozen from this source
batch; the later payload-capability probe in
`.tmp/diamond-payload-capabilities-v213.log` is not part of these green gates.

That stronger probe fails: a builder parameter has the source-interface bound
`Actual extends ValueStore<Key, Value>`. The concrete PayloadStore stores and
returns Value, so its generated interface implementation correctly needs
`Value: Clone`, but the factory constructing the builder omits this transitive
obligation. The nested construction is now correctly instantiated; this is a
separate generic-bound fulfillment dependency, not another inference cast.
The retained generated crate is
`.tmp/test-tmp/new_java2rust_nested_diamond_payload_capabilities_1792224471580676114`.
Next work should make callable obligations consume the same concrete trait
implementation capability analysis used by emitted forwarders, preserving
carrier-sensitive payload requirements and unrelated-parameter negative
controls. Blanket Clone constraints on every source construction are not a
correct solution.

The frozen v40 producer is `.tmp/new-java2rust-v40-contextual-diamond`, SHA-256
`3b9fee962e6890170d19fe1412679e9d0b0d68a1ea215433f783ef9c8a9088c0`.
Release build `.tmp/contextual-diamond-release-v212.log` took 1m40s.
The 104-file corpus gate completed in `.tmp/jgrapht-v40-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v40`. No payload-capability implementation is
included in this snapshot. Formatting check
`.tmp/contextual-diamond-format-v214.log` passes.

v40 reports **2,820 actual Rust compile errors** (2,821 including the summary),
34 fewer than v39, and **55 invalid patterns in 30 files**, down from 108 in
37 files. All 104 files still parse and translate without failures. Resolver,
translation, assembly and check took 39,671/221,421/61,884/13,389ms; total
340,329ms, overlapping development gates rather than a controlled speed run.
The leading mismatched-type count remains 494. V Clone diagnostics fall from
93 to 87; E Clone remains 86. These diagnostic counts are not independent
root-cause counts, and the crate still does not compile.

### Source-interface bound fulfillment capabilities

The next batch factors the existing trait implementation capability traversal
and method Clone query into search-local helpers. Constructor capability
analysis now checks concrete actuals of source-interface-bounded owner
parameters against that same implementation traversal; its recursion guard
does not cache provisional false results. The existing ancestor substitution
and carrier-sensitive field inventory remain the source of obligations.

The first combined probe in `.tmp/diamond-bound-capabilities-v215.log` passes
seven diamond tests but fails the strengthened payload fixture. The actual
payload requirement is propagated, but a negative control reveals that generic
method rendering still uses metadata-free analysis and incorrectly requires
Clone for a payload stored behind a cloneable interface handle. The driver
also expected an owned interface argument although the solved ABI borrows it;
the driver now matches that borrowed ABI. Method generic rendering now consumes
the translation's existing typed Clone cache instead of unioning a precise
answer with an independently conservative metadata-free answer. Compatibility
callers without a cache retain their existing fallback.

The strengthened fixture has compile-time generic controls proving that the
phantom Key needs no Clone and an interface-handle payload needs no Clone,
while a genuinely stored value still requires it. The combined diamond,
generic-clone and generic-key gate is in
`.tmp/diamond-bound-storage-v216.log` (pending). These changes are after the
frozen v40 producer and cannot affect that measurement.

The v216 diamond group passes all eight tests (21.51s), including the
payload and handle/phantom negative controls; its two clone tests also pass
(24.32s), as do all three key tests (24.39s). Neighboring gate v218 and unit/batch
gate v217 are running. The next release build is v219.

The v218 neighboring gate completes with all 44 tests passing. v217's 1,396
unit tests pass (146.25s); its 85 batch tests are still finishing. Release v219
completed in 2m27s and froze `.tmp/new-java2rust-v41-bound-capabilities`, SHA-256
`eae874018769c3e9e1971dea4f4d7bb5a9dd217c005fc262956db7b51002dea7`.
Formatting v220 passes. No inherited-member visibility changes are in v41.

v217's 85 batch tests subsequently pass (131.86s), completing that gate.
The frozen v41 corpus run is in `.tmp/jgrapht-v41-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v41` (pending).

Inspection of the completed v40 corpus confirms the targeted factory
change: Multigraph's two create_builder methods now use explicitly instantiated
`Multigraph<VMethod, EMethod>` / `Multigraph<VMethod2, EMethod2>` witnesses.
The nonexistent `.downcast::<G>()`, outer-scope Supplier<V> and extra `?` on
the cast result are gone. This localized evidence does not establish that the
crate compiles or that the new interface-bound capability batch is measured.

Follow-up triage of v40 also identifies a separate visibility boundary:
inherited source bodies materialized in child modules retain accesses to their
lexical owner's private fields and constants, but those members can remain
Rust module-private. Multigraph's materialized AbstractBaseGraph clone body is
one occurrence (18 diagnostics each for the type and strategy fields, plus
private constants in repeated inherited methods). Existing field policy
already handles carrier, upcast and nestmate access. A future neutral
multi-file inherited-body regression should extend that shared visibility
policy or preserve delegation to lexical owner bodies, not special-case these
members or make all source internals externally public. This is triage only;
no visibility change is in the current capability batch.

### Inherited lexical visibility batch

`tests/inherited_member_visibility.rs` adds a two-file neutral generic parent
and child. The inherited body reads another parent instance's private count
and a private constant declared by its lexical owner. Baseline v221 initially
fails to compile the test driver because explicit `.into()` calls leave
`BatchSource::new`'s generic arguments ambiguous; the corrected driver uses
string literals directly. v222 then reproduces exactly two generated Rust
errors: private Storage.count and private Storage.OFFSET. The retained crate is
`.tmp/test-tmp/new_java2rust_batch_inherited_lexical_visibility_1792225034961611611/out`.

Subtype visibility now reuses `source_class_ancestor_type_arguments`, including
its generic hierarchy parsing, lexical lookup and cycle guard, instead of a
separate raw-string extends walker. Nested source declarations are included.
The existing field `pub(crate)` policy then applies to generic parents as it
already did to nongeneric parents. RustAssociatedConst gains the same explicit
crate-visibility option as RustField; source-private constants use it when
inherited bodies can cross modules. Ordinary private members without such
context remain private. This does not expose members publicly outside the
generated crate or change Java access resolution.

The strengthened regression also runs a separate consumer expecting 7 from
the inherited read, and has a cheap isolated-class visibility negative control.
These visibility edits are after v41. All three visibility tests pass in
`.tmp/inherited-visibility-contracts-v223.log` (24.10s), including the
two-hop generic inheritance consumer (result 7), nested-subclass check, and
isolated-private-member control. The same gate is still checking inherited
platform fields and interface/nested-identity neighbors. Format check v224
found only wrapping changes in the newly strengthened test; cargo fmt has
applied them. Combined unit/batch/diamond/clone/key gate v225 and frozen
release build v226 are running.

All v223 neighbors also pass: one platform-field test (18.89s), four interface
default tests (28.31s) and two nested-identity tests (33.93s), in addition to the
three visibility tests. No visibility fixture requires repaired output.

Read-only next-batch triage finds Rust Eq/Hash obligations introduced by nested
set representation mismatches. In v40's Partitioning default methods, get_partition
returns JavaSemanticHashSet<V>, but List<Set<V>> locals/returns use
Vec<std::collections::HashSet<V>> and insert an unwanted collect conversion.
The existing collection-value-equality test covers concrete nested sets, not
this generic default-method boundary. A new neutral Buckets<Item> default
method probe in `tests/nested_generic_representation.rs` checks that nested set
values retain JavaSemanticHashSet without Rust Eq/Hash requirements; its
baseline is `.tmp/nested-set-default-baseline-v227.log` (pending). No nested-set
implementation changes are in the visibility producer.

v227 fails with nine generated Rust errors (65.19s), including Rust Eq/Hash
requirements in the default body and a Vec<HashSet<Item>> versus
Vec<JavaSemanticHashSet<Item>> boundary mismatch. Retained generated crate:
`.tmp/test-tmp/new_java2rust_nested_generic_default_set_values_1792225407131329508`.
The top-level signature projection already selects JavaSemanticHash using a
lexical type-parameter scope, but that decision is represented only in the
outer RustSlotShape.view. Recursive generic rendering has no equivalent scope
or planned nested-carrier evidence. Globally recognizing type-variable names
would confuse shadowing source declarations, and adding Rust Eq/Hash changes
the intended key contract. The next implementation must retain recursive
carrier decisions and carry them through the shared slot-specialization path;
it must not merely rewrite the final Rust string or replace all sets without
checking facade and source-definition boundaries.

v225 passes all 1,396 unit tests (172.56s), all 85 batch tests (120.05s), and
all eight diamond tests (21.57s); clone/key groups are still finishing.
Release v226 completed in 2m55s, freezing
`.tmp/new-java2rust-v42-inherited-visibility`, SHA-256
`080f898784b0d28f2ed8d8f7633e63dd03c9bfb427fb1b38767d5226674efa74`.
The v227 test was added after this snapshot; no nested-set fix is included.

The v225 clone (2 tests, 25.66s) and key (3 tests, 20.44s) groups also pass,
completing the full gate. The v42 corpus is running in
`.tmp/jgrapht-v42-run.log`, output root `.tmp/jgrapht-core-1.5.3-focused-v42`.

The completed v41 snapshot reports **2,796 actual Rust errors** (2,797 including
the summary), down 24 from v40, and unchanged **55 invalid patterns in 30 files**.
All 104 files parse and translate without failures. The factory-specific
VMethod/EMethod Clone diagnostic groups disappear. Resolver/translation/
assembly/check times are 42,335/386,918/62,382/11,834ms; total 506,812ms.
Emission rises from 151,180ms in v40 to 288,117ms in v41, whereas assembly and
global analysis are much closer. This run overlaps v42 release compilation and
tests, so the cause is not established by these timings alone. Keep this
potential lowering-cost regression visible rather than claiming a speed win.

The completed v42 visibility snapshot reports **2,688 actual Rust errors**
(2,689 including the summary), 108 fewer than v41, with unchanged **55 invalid
patterns in 30 files** and zero parse/translation failures. Mismatched-type
diagnostics increase from 494 to 512 as formerly inaccessible members become
checkable. Resolver/translation/assembly/check times are
39,253/223,884/61,480/13,299ms; total 341,780ms. Emission is 155,093ms, much
closer to v40's 151,180ms than v41's 288,117ms. This weakens the case for a
persistent twofold regression in capability analysis, without replacing a
controlled performance comparison.

### Planned nested native collection views

RustSlotShape now retains typed nested Java-subtree/view pairs selected in the
signature's lexical generic scope. The semantic hash policy is factored from
the existing outer-slot projector and reused recursively, including the
parsed-source override guard. Rendering consumes these planned choices through
its existing EffectiveFacts recursion, rather than recognizing global binder
spellings or rewriting emitted Rust. Ordinary context-free concrete collection
rendering remains unchanged.

The shared slot-specialization helper remaps nested Java keys alongside the
solved Java type while preserving optionality. Collection mutation planning
projects child views from the solved receiver shape; index and unrelated
argument slots do not inherit irrelevant decisions. The storage re-projection
also retains nested metadata. No generated Rust or Java corpus was edited.

Cheap library check v228 passes. The first v229 focused gate passes collection
equality and all three generic-key tests, but nested defaults still fail:
declarations use the planned semantic set while insertion planning discards the
receiver shape and collects into Rust HashSet. v230 verifies that boundary fix:
all collection-equality, three generic-key and two nested-representation tests
pass (24.75/22.14/20.24s). The strengthened default fixture includes nested maps
and List<List<Set<Item>>> in addition to List<Set<Item>>, and its generic Rust
consumer imposes no Eq/Hash constraints.

The first unit build v231 finds eleven test-only RustSlotShape literals needing
the new metadata field; their empty plans are now explicit. The v232 projection
gate additionally checks specialization/nullability and relevant-child view
inheritance (pending). This is not a claim that all nested container ABI
conversions or alias-preserving adaptations are solved; those must consume the
same planned evidence rather than inventing independent rendering policies.

All six v232 shape-projection unit tests pass, including the two new nested
carrier tests. Format v233 passes. The stronger v234 neighboring gate adds a
source-defined Set<Item> inside a generic default method; its Rust consumer
requires the nominal source Set rather than any native semantic-set carrier.
The combined unit/batch/visibility/source-precedence gate is v235 and the next
frozen release build is v236; both are pending.

The same nested-default fixture is additionally strengthened in v237 to run
the generated default method with a Rust provider whose key implements only
JavaMapKey (not Rust Eq/Hash or Clone). The expected result is one row with one
deduplicated key. This driver implements the generated source-interface ABI;
it does not replace or repair the default method being tested. v237 is pending
and is later than the initial v234 test snapshot; the producer source is the
same. The source-defined Set control and full v235 gates also remain pending.

The v237 runtime check passes (76.75s): the generated default method returns
one row with one Java-equal key despite the Rust key lacking Eq, Hash and Clone.
v235 passes 1,398 unit tests (170.14s), 85 batch tests (131.86s), three inherited
visibility tests (24.08s), and three source-precedence tests (19.69s).
Release v236 froze `.tmp/new-java2rust-v43-nested-collection-views` in 2m46s,
SHA-256 `9283a0195b7de3fb68011fdc28dbef19acafe3e8bc7a0817aacb67ba88bb3c55`.
It is not advanced to a corpus measurement because the additional v234
source-defined Set negative control fails; the other nested tests and all
collection/diamond/clone/key groups in that gate pass.

The failing negative control has five generated errors in retained crate
`.tmp/test-tmp/new_java2rust_source_named_nested_set_1792226457767159756`:
the nominal source Set is still rendered as Rust HashSet in a default-return
surface, and the generic interface retarget adapter incorrectly treats that
source class as an enumerable native collection. This is a source-precedence
gap exposed by the stronger control, not permission to impose Rust Eq/Hash.
Canonical and facts-only native rendering now check parsed-source provenance
before built-in name policies; representation-aware rendering retains the
existing dispatch selection. Retarget eligibility now uses shared lexical
source lookup to distinguish actual native collection nodes from nominal
source classes. The follow-up gate is v239 (pending); these changes are after
the frozen v43 snapshot.

Read-only triage of v42 Graphs notices many Box<dyn Any> clone errors around
lower-bounded interface arguments. The neutral v238 Sizer<? super Value> probe
in `tests/lower_bounded_wildcards.rs` actually passes (52.84s), including a
concrete String implementation and caller. Therefore simple lower-bound
capture alone is not an established next failure class. Preserve this passing
control and minimize the remaining getter/owned-argument/receiver interaction
before changing wildcard representation or erased-object carriers.

All v239 gates pass: the lower-bound control (19.77s), all three nested/source
collection tests including the Java-only-key runtime probe (21.19s), and all
three source-facade tests (20.31s). Formatting v240 finds only the subsequently
added non-generic nominal fallback's wrapping; cargo fmt has applied it.
That fallback retains RustType::Named for a source nominal with no arguments,
rather than constructing an empty RustType::Generic.

The post-precedence source state is now under full unit/batch gate v241 and
native-collection/equality/clone/key/nested/type-bound gates v242. Release v243
will freeze `.tmp/new-java2rust-v44-nested-source-precedence`. These gates and
the producer build are pending; v43 remains unmeasured because it predates the
necessary source-precedence follow-up.

v242 passes all collection/equality/clone/key/nested/type-bound neighbors.
v241's unit gate instead reports 1,395 passes and three failures, so its batch
suite does not run. Two failures verify explicit UserOverride sequence mappings
on source declarations; parsed-source precedence must suppress built-in models,
not those intentional mappings. EffectiveFacts now distinguishes parsed-source
control from an explicit user sequence/string carrier override. The third
failure uses a source Character class: its nominal rendering reveals that
charValue was incorrectly lowered as native primitive unboxing. Native receiver
planning now consults the existing lexical parsed-source-owner predicate before
selecting built-in receiver operations, including resolved native string calls.

The overload regression retains its original source Character declaration and
checks Option<Character>, not the formerly incorrect Option<char>; its other
fallibility/overload assertions are retained. A new source-facade runtime
regression expects 'x' from the source Character.charValue body and 'q' from a
fully qualified java.lang.Character operation in the same source program.
The v246 nested/source runtime groups pass; v245 explicitly checks the two
user-override unit contracts. The short-name --exact command in v247 selects
no tests; v248 uses the fully qualified overload test name instead. These
unit results are pending at this entry.

Release v243 froze `.tmp/new-java2rust-v44-nested-source-precedence` in 1m45s,
SHA-256 `378db3183cff564eb9cc999879c00dd011dc7973a6e25b1cabef67e924a7d7fc`.
It predates these necessary gate fixes and is not corpus-measured.

The stronger v244 lower-bounded owned-sink probe fails with one generated
E0596: ValueTransfer.forward takes &dyn ValueSink<Value> while put needs a
mutable receiver. Retained crate:
`.tmp/test-tmp/new_java2rust_lower_bounded_owned_sink_1792226858986937058`.
Inspection also finds the caller borrowing a cloned TextCell, which would
update a copy rather than the original even if only the &mut error were fixed.
The intended Rust needs a mutable borrow of the original cell and propagation
of the callee's receiver requirement to forward's sink parameter. These are
small generated-code differences, but must be implemented generally in shared
signature/argument/borrow planning; output is not patched. This more precise
failure supersedes the speculative broad lower-bound-capture diagnosis.

Both v245 user-override unit contracts pass (0.23s). v248 runs exactly one
fully qualified overload/fallibility test and passes (16.71s); v247 ran zero
tests and is not evidence for that contract. All v246 nested tests and four
source-facade tests pass, including the source/native Character runtime check.
The stronger qualified-native Character case was added after v246; that earlier
pass proves only the source-defined method, not their coexistence. v249 passed
1398 unit tests but failed that stronger source-facade runtime compile gate:
`java.lang.Character` became `JavaLangCharacter` and its static call bound to
the source `Character`. This supersedes the earlier claim about native coverage.
Release v250 froze `.tmp/new-java2rust-v45-source-call-precedence` in 2m16s,
SHA-256 `f041e2cfd91d08d3a3406cee21c630efccf0119d5c9d0f8ae9ab10843ae86f22`.
It is unmeasured and does not pass that strengthened gate.

The v251 sink probe establishes partial progress: propagating mutable receiver
requirements through the existing signature dependency pass makes the parameter
`&mut dyn ValueSink<Value>` and removes the clone of the caller's cell. The
caller still casts to `&dyn`, failing E0308. The v252 focused signature unit
passes and covers method/constructor parameter propagation plus an unrelated
same-named read-only method negative control. The next coherent batch carries
target mutability through concrete, boxed and closed-enum trait coercion plans,
and distinguishes exact source ownership from external model leaf aliases.
Do not build another release or measure the corpus until its combined gates pass.

The v253 mutable trait-view unit passes. v254 could not compile the older
type_coercion test target: two slot literals lacked nested-view metadata and
two set-carrier literals still used the old enum shape. v255, after fixture
updates, passes all 1401 units (132.41s), lower-bounded wildcard tests (2,
20.80s), nested-generic tests (3, 22.36s), and source-facade tests (4, 24.06s).
This includes actual mutation of the original sink cell and coexistence of a
source Character with qualified java.lang.Character. Coercion tests pass 117/119:
the two failures expect old method-style clone spelling and inspect a match
without unwrapping its recorded shape. Their exact emitted behavior assertions
are retained while updating those representation assertions. Review also adds
the shared exact-source guard to native static calls, with a source valueOf
negative control alongside the qualified platform call. These last changes
still require the combined/broad gate; no new corpus measurement is claimed.

The v256 broad gate passes all 1401 units (144.03s), 85 batch tests (127.89s),
collection-read (1), diamond inference (8), generic clone (2), generic key (3),
inherited visibility (3), lower-bounded wildcards (2), nested generic views (3),
and source precedence (4, including both source static valueOf and qualified
platform valueOf). Its only failure is the coercion test's nested panic arm
metadata assertion; the exact emitted Rust assertion passes. The fixture now
checks the CheckedCastFailure tag explicitly and inspects its unshaped macro.
v257 reruns the whole coercion target: all 119 pass (0.22s), with no product
code change after v256. These gates justify one new release/corpus measurement,
not a claim that the JGraphT crate already compiles or all repository gates pass.

Release v258 finishes in 1m26s. The resulting frozen producer is
`.tmp/new-java2rust-v46-typed-interface-borrows`, SHA-256
`be83de27a793023bcac54c09a1044872a8bf72d5e41f607b25dc49a6f191400d`
(equal to the release artifact). Its single 104-file run uses JOBS=10,
`NEW_JAVA2RUST_BIN` set to that producer and `JGRAPHT_TRANSLATION_ROOT` set to
`.tmp/jgrapht-core-1.5.3-focused-v46`; log `.tmp/jgrapht-v46-run.log`.
The runner is active, not a completed measurement. The pinned source checkout
was rechecked at the recorded commit with no tracked changes. No Java or
generated Rust was repaired. The prior completed corpus checkpoint remains v42.

### v46 completed measurement and next coherent batch

The v46 runner exits 1 because the generated crate fails, with zero parse or
translation failures across 104 files. Its compiler reports 2601 previous errors
(the runner summary's 2602 includes the aggregate compiler failure line), down
87 from v42's 2688. Invalid patterns remain 55 across 30 files. Resolver,
translation, assembly and check timings are 37491/226155/61782/11799ms;
total 340537ms versus v42's 341780ms. Emission is 151427ms, global analysis
58122ms and signature solving 28841ms (collect 16795, fixpoint 5801, plan build
5956). No performance regression is inferred from this near-identical total.

Nested generic collection projection lowers V:Hash/Eq errors from 62/57 to
38/33, and V/E:JavaMapKey from 54/52 to 48/46; V/E:Clone falls from 87/86
to 81/78. Mismatched types rise from 512 to 530 as stronger mutable interfaces
expose downstream coercion mismatches; 31 failures now name an invalid as_ref
on a mutable Graph trait reference. These are not accepted exclusions.

The next result-payload batch is justified directly by v46's emitted GraphTests
and Graphs: cloning a consumed source-call result emits `Clone::clone(&call()?)?`,
and a boolean operation emits `(flag | call()?)?`. Shared ownership-clone
projection now preserves consumed-result metadata, and the structural result
query recognizes unary/binary payload transforms without treating arbitrary
function/method calls as infallible. Corpus-neutral unit coverage includes the
fresh-Result-call negative control; a runtime fixture covers success, errors,
short-circuiting and generic return/ownership composition. Focused v259 is
running, not yet passed. No generated output or Java source has been patched.

v259 completes successfully: 27 payload-focused units (1.80s) and the new
result-payload runtime regression (18.68s), including generic ownership,
exception propagation and boolean short-circuit behavior. A second neutral
Port<Value> fixture in v260 reproduces v46's as_ref error exactly: a parameter
rendered as `&mut dyn Port<Value>` is forwarded to a helper accepting `&dyn`
and then mutated again. This fails only on the spurious storage accessor.

The existing representation-aware boxed-interface borrow helper now compares
the actual rendered reference payload with the target payload, accepting the
valid mutable-to-shared reborrow as well as exact reference ABIs. It does not
pretend a different generic trait instantiation is the same ABI, and it retains
the target's mutable/read-only metadata. Genuine Box payloads use as_mut for a
mutable view and as_ref for a shared view. Rendered types are computed once and
reused rather than re-derived. Combined v261 is running all library units and
the new/neighboring runtime and coercion suites against this coherent
result-consumption/borrow-boundary batch. It is not yet a verified producer or a
new corpus measurement.

v261 passes all selected checks: 1402 library units (129.71s), lower-bounded
wildcards (2, 19.76s), nested generics (3, 19.64s), result-payload runtime (1,
19.80s), source precedence (4, 19.50s), the previously failing mutable-to-shared
trait reborrow runtime (1, 19.45s), and all 119 coercion tests (0.22s). The
reborrow fixture is being strengthened with an owned source-interface factory
to cover the genuinely boxed mutable branch as well. That fixture and the
batch-translation suite still need their gate before the next release build.

v262 passes all 85 batch tests (116.85s). The strengthened boxed runtime fixture
fails only because its hand-written Rust driver calls unwrap on an infallible
i32 return. Inspection confirms a genuine `&mut Box<dyn Port<String>>` producer,
an as_mut projection into the mutable trait argument, and read-only forwarding
followed by another mutation. The driver is corrected to compare the i32
directly; no product/generated code is changed. v263 reruns the strengthened
fixture and passes (20.27s), checking both original concrete storage and the
boxed producer. Combined with v261, the coherent result-payload/reborrow batch
is now gated for one new frozen release and corpus measurement.

Release v264 finishes in 1m26s. Frozen producer
`.tmp/new-java2rust-v47-result-payload-reborrows` matches the release artifact,
SHA-256 `c9dad1bf64efcb6cf32171e88c6c2f31873c01af631a23e4545d3127173354ba`.
The single v47 104-file measurement is running with JOBS=10 and that binary;
output `.tmp/jgrapht-core-1.5.3-focused-v47`, log `.tmp/jgrapht-v47-run.log`.
This is not a completed result. v46 remains the authoritative completed
checkpoint until the runner finishes.

During the frozen v47 run, the emitted GraphTests source already confirms
that both getOppositeVertex clone-recovery sites no longer acquire a second
try operator. Graphs' boolean accumulation likewise retains only the inner
call's try operator. These are emitted-code observations, not compile success.
The next neutral fixture is `tests/fallible_stream_matching.rs`, covering
anyMatch/allMatch with throwing expression lambdas and static method references,
empty inputs, early success/failure before a later throw, and encountered
predicate errors. It is not yet run. Investigation points to the terminal's
infallible any/all closure contract, plus missing source-call result adaptation
in the stream expression renderer. Reuse the solved CallableAnalysis and
signature/method-reference selection; do not infer fallibility from rendered
text, eagerly collect a matching stream, or add another source/external name
policy. No stream product implementation has been changed yet.

### v47 completed: result payloads, reborrows, and a revealed binder gap

v47 exits 1 on generated Rust: zero translator/parse failures across 104 files,
2536 compiler errors (summary 2537 includes the aggregate line), versus v46's
2601. Invalid patterns increase to 56 across 31 files. Timings are
36630/211564/60342/11769ms for resolver/translation/assembly/check; total
323620ms. Global analysis is 49324ms, signature solving 23185ms and emission
143725ms. The unsupported-try count drops substantially and the 31 mutable
Graph as_ref errors disappear; these are compile evidence, not crate success.

The additional placeholder is in the degree-comparator factory call from
FloydWarshallShortestPaths. v46 already emitted invalid Rust there:
`graph.as_ref() as &dyn Graph<VMethod, E>`, including an out-of-scope callee
binder. v47 rejects that different trait instantiation instead of pretending
it is boxed storage. The shared argument specializer currently preserves an
entire solved type whenever it contains one synthesized wildcard binder. That
also prevents unrelated ordinary method binders from being instantiated. Fix
the shared instantiation boundary, preserving only the wildcard witnesses that
need protection, with a neutral shadowing-method/wildcard regression; do not
restore the invalid as_ref cast or erase generic trait arguments.

The new stream fixture baseline v265 fails with 8 generated-crate errors
(18.80s): lambda source calls have a bare static-owner name and a double
primitive dereference; matching closures receive a Result where bool is
required; static method references also fail to propagate callable fallibility
to their caller's ABI. This is a real related callback-contract batch, not
only a terminal spelling edit. The fixture is intentionally still red and no
stream implementation has changed. Prioritize the newly revealed binder gap
before broadening the stream changes.

The wildcard-binder follow-up is implemented but not yet corpus-measured.
`specialize_resolved_argument_preserving_source_wildcards` overlays resolver
instantiations through matching named/array/intersection/bounded-wildcard
structures while retaining synthesized witnesses and truly rewritten owner
carriers. It delegates final optionality and nested-view remapping to the
existing canonical slot specializer. New units protect mixed ordinary/witness
binders, array nesting, nullability/semantic-view metadata, mutable ownership,
and the old H-to-Model<W> negative control. A neutral Reader/Factory/Bridge
runtime case covers a shadowing method binder alongside `Reader<Key, ?>`.
v266 is running all library units plus generic-method, wildcard-bound,
trait-reborrow, lower-bounded-wildcard and coercion suites. Do not claim it
passed or build another release until its result is known. The stream fixture
remains a documented red baseline, not an accepted completion exclusion.

v266 completes: all 1404 library units pass (128.87s), including both new
projection tests. Lower-bounded wildcards (2, 20.74s), trait reborrowing (1,
23.02s), coercion (119, 0.34s), and wildcard-bound contracts (2, 22.17s) pass.
Generic-method tests pass 18/19; the new runtime fixture fails before it can
establish the intended argument-boundary behavior. Retained crate:
`.tmp/test-tmp/new_java2rust_shadowed_binder_next_to_wildcard_1792231068865425464`.

The new failures are concrete, not accepted exclusions: source Reader's
parameters emit bare `&Reader<...>` rather than a dyn trait and its read call
receives external Reader ABI effects; the cross-owner static call emits
`Factory::<Key, Unused>::count` even though Unused is not in the caller's scope.
These need general source-facade precedence and static-owner generic-frame
solutions using the existing identity/instantiation machinery. Keep the fixture
and its assertions intact while investigating; do not rename away source-name
collisions or invent caller bindings to make the emitted text compile. The
projection unit evidence is positive but the runtime batch is NOT verified.
No v48 release or new full corpus run has been started. The next batch should
resolve these connected invocation-boundary failures before returning to the
recorded stream callback work.

### Invocation-boundary follow-up (v267–v273)

The original neutral `Reader<Key, Unused>` fixture is retained, including its
built-in-name collision. v267 and v268 exposed additional bypasses rather than
establishing success. Source-owner fact eligibility now uses the existing fact
precedence ranking and exact parsed-source identity. Shared facade ABI and
argument-effect lookups reject weaker built-in facts for co-translated source
owners, while explicit user overrides and qualified native owners remain
eligible. Source-aware dispatch projection and rendering consult representation
provenance instead of classifying a source interface as a platform facade by
name alone.

Static source member owners use the existing raw-generic-argument plan and
canonical argument rendering, not the callee class's lexical binders in the
caller frame. The fixture consequently emits an erased `Factory` owner without
leaking `Unused`. Its source interface argument now converts directly to the
planned borrowed trait instead of an untyped `.into()`.

Wildcard `Any` obligations use the existing source-call capability reachability
worklist, generalized from key capabilities. Reified wildcard substitutions
identify the caller binder that actually needs the obligation; selected source
call edges carry it transitively. The fixture includes a second generic caller
with differently named binders to exercise that propagation.

v269 (`.tmp/source-owner-contracts-probe-v269.log`) passes the internal source
representation/fact-precedence test (1.71s). The generated runtime crate is down
from three compiler errors to one: a return-value query still applies external
Reader fallibility to the source method's plain `i32` result (19.60s). The trait
argument and transitive generic-bound errors are gone. This remaining failure
is not an exclusion. v270 addresses the return-value precedence boundary using
the emitted source method's exception plan, leaving explicit facade overrides
and non-emitted compatibility summaries eligible for their own ABI handling.
v270 (`.tmp/source-owner-related-v270.log`) completes with key capabilities
(3, 20.67s), clone capabilities (2, 26.51s), lower wildcards (2, 20.14s), source
facade precedence (4, 19.52s), trait reborrows (1, 20.58s), and wildcard bounds
(2, 19.01s) passing. Generic-method binding remains 18/19 (21.11s): the added
resolved-call-only check does not cover the reconstructed-target path. The
follow-up moves the exact source target ABI ahead of external fallbacks for
emitted declarations, while retaining facade selection and overload retargeting.
The runtime fixture is strengthened with an ordinary sibling type argument that
contains a non-static Rust borrow, so indiscriminately adding `Any` cannot make
the test pass. v271 runs library units and broader source-identity, I/O,
object-model, coercion and runtime tests together. No new release or corpus
measurement has been made, so v47 remains the authoritative corpus baseline.

v271 (`.tmp/source-owner-broad-v271.log`) completes with all 1405 library units
passing (133.95s), result payload transforms (1, 19.38s), source-name identity
(3, 19.35s), and coercion (119, 0.22s) passing. The five object-model failures
listed below recur (28/33 pass, 0.20s); their baseline attribution remains open.
Generic-method binding remains 18/19 (20.67s), with the same erroneous recovery
on `reader.read(value)`. Both the resolved and reconstructed-target precedence
changes are therefore insufficient; trace the actual wrapper producer before
changing another fallback.

I/O passes 1/2 (20.47s): the failing Rust driver calls `.unwrap()` on the plain
`Vec<i8>` returned by `snapshot`. A frozen-v47 comparison of the neutral
`ByteArrayOutputStream.toByteArray()` method
(`.tmp/io-snapshot-abi-baseline.java`, output `.tmp/io-snapshot-abi-v47.rs`)
also emits `snapshot(...) -> Vec<i8>`, not `Result`. The driver is corrected to
consume the vector directly; its byte-for-byte behavior assertions are retained.
This correction does not establish that the still-failing source Reader call
is fixed. v272 enables temporary structured-result-recovery tracing for that
fixture alone to locate the actual producer.

v272 (`.tmp/source-reader-recovery-trace-v272.log`, terminal failure, 21.31s)
locates the first recovery in `lower_source_receiver_call_expr_for_slot`, via
`slot_expr_needs_result_propagation`; return lowering subsequently queries that
same classification again. Ordinary call emission itself produces the plain
source call. The early classification was gated by a separate candidate-method
lookup before consulting the selected/retargeted call target, losing the exact
ABI that emission already knows. Remove that redundant candidate gate and use
the selected source target directly; retain the non-emitted-summary boundary
check and the explicit external-facade exception. The temporary backtrace hook
has been removed. v273 combines generic-method, I/O, source-facade and result
payload tests against this change and the corrected I/O driver.

v273 (`.tmp/source-target-abi-v273.log`) completes successfully: all 19
generic-method tests (22.60s), both I/O runtime tests (23.33s), result-payload
runtime (1, 19.30s), and all four source-facade tests (23.94s) pass. The original
Reader/Factory/Bridge fixture now compiles and executes, including the extra
non-static-borrow control through both generic callers. This gates the connected
source-dispatch/static-owner/wildcard-bound/selected-result-ABI batch; it does
not establish JGraphT crate success. The full library-unit result above belongs
to v271, before the final ABI-classifier adjustment, not to a fresh all-unit
run of v273. Keep the documented stream callback regression and the five
unattributed object-model failures open. Batch the next callback-contract
changes before repeating the broader unit/corpus gates; do not create a new
release snapshot for this individual follow-up. Java and generated corpus
sources remain untouched, and v47 is still the latest complete corpus result.

### Consumed stream callback batch (v274 onward)

The neutral `fallible_stream_matching` fixture retains its original four
lambda/static-reference any/all cases and now also exercises nested boolean
expressions, qualified static calls, and fallible maps feeding matching
terminals. Its runtime assertions require empty-stream identities, error
propagation, and short-circuiting before a later throwing element. No Java
corpus or generated output has been edited.

The implementation follows existing shared boundaries:

- Structured Rust IR reports unbound `?` effects through expressions and
  statements but stops at nested callable boundaries; string contents and raw
  leaves are not effect evidence. Callback calls use solved source signatures
  and the existing callable fallibility analysis, and maps lift fallible bodies
  into explicitly typed Java-exception Results.
- Matching terminals with fallible predicates or fallible upstream items use a
  typed, scoped Result-returning loop, propagating or recovering its result at
  the enclosing exception boundary. Traversal remains lazy; no eager collection
  or unconditional predicate panic replaces Java short-circuit behavior.
- A projected Copy stream argument records the owned value it produced. Slot
  conversion consumes that physical shape rather than dereferencing the
  pre-projection iterator loan again.
- Source method-reference output typing and invocation reuse one solved
  signature selection. Static receiverless calls use the selected declaration's
  owner instead of an unqualified free Rust function.
- The stream plan inventories callbacks consumed by a terminal. New static and
  bound source-reference graph edges are added only at that invocation boundary,
  using a shared source-symbol index and the resolver's existing generic and
  overload applicability. Creating a static method reference or returning an
  unconsumed intermediate stream does not execute it. Existing current-receiver
  callback handling is preserved; arbitrary callback escape/invocation modeling
  is not claimed solved by this batch.

v274 stops at compile-time API mistakes (a missing Option wrapper on shape
metadata and private graph fields in the new test), corrected together. v275
(`.tmp/stream-callback-batch-v275.log`) passes both new analysis units (0.01s),
including a consumed overloaded static reference and non-invocation controls.
The expanded generated-runtime fixture fails with one compiler error (20.92s),
down from the original baseline's eight: a qualified static call inside a
lambda leaves the enclosing method infallible even though emission recognizes
the callback's Result ABI. That remaining error is not an exclusion.

The follow-up carries the stream callback's input type into graph analysis of
its expression/block body. AST identity avoids visiting the same body again as
an untyped callback. Previously unresolved source calls consult the existing
resolver's selection rules and canonical declaration identities rather than
inventing a Java-name method-family edge. Expression-lambda output typing also
reuses the selected source return slot. v276 runs the two analysis units and
the expanded runtime fixture against the combined follow-up.

Fallible intermediate filtering and operators after a fallible intermediate
still need a general fallible-pipeline plan; they fail closed rather than
emitting a bool-only filter closure with `?`. Flat-map callback invocation
inventory also needs the nested element/cardinality contract; this batch does
not guess its input type. These remain open capabilities, not completion
exclusions. v47 remains the latest complete corpus measurement.

v276 (`.tmp/stream-callback-batch-v276.log`) passes both analysis units (0.01s)
and the expanded eight-variant runtime fixture (22.27s). v277 combines the full
library and established stream/generic/source suites. All 1407 library units
pass (155.55s); generic-method binding (19, 22.26s), source-facade precedence
(4, 23.20s), collection returns (4, 23.05s), and collectors (5, 42.58s) pass.
Remaining suite results are still running at this entry.

The bound-reference suite passes its generic-interface case but fails its Rust
driver's signature assumption for `Use::exceptional` (1/2 pass, 36.49s).
Previously absent consumed-bound-callback edges now make that method
conservatively return Result. The existing handled-edge policy discharges only
known checked exception facts, not the callback's body-thrown unchecked
exception. Generated Java catch handling still returns its `-1.0` value inside
Ok; the driver is changed to unwrap the Result and retain the exact `-1.0`
assertion. Neither a propagated Err nor a panic is accepted. This is a deliberate
Rust ABI adjustment from stronger callback analysis, not a claim that typed
escaping-exception sets have been fully modeled.

Review finds that static stream callbacks must reuse the preceding batch's raw
generic-owner rendering, not merely add their own nominal owner spelling. The
existing renderer is moved to shared `slot_render::rust_source_static_owner_type`;
ordinary lowering and representation-aware stream lowering both call it. It
keeps static members on the source nominal declaration and instantiates owner
parameters through the existing raw-erasure and generic-argument rendering
plans. A generic static-reference case and a generic qualified-lambda case
extend the runtime fixture to ten variants. v278 runs that fixture plus
generic-method and source-facade tests; the initial ten-variant runtime passes
(26.85s). The fixture is then placed in a package to exercise qualified source
identity too; its packaged rerun and the adjusted bound-reference driver remain
required. No release/corpus rerun has started for this follow-up.

v277 completes: primitive extrema (1, 77.88s) also pass, while source-stream
resolution passes 4/5 (73.71s). Its imported generic-interface fixture fails in
the assembled trait bridge: `Words::values()` returns native `HashSet<String>`
but the implemented contract requires `JavaSemanticHashSet<String>`. The exact
retained neutral Java/facts fixture is replayed with frozen v47 into
`.tmp/imported-source-stream-v47-comparison`; it reports the identical bridge
compiler error. This is a verified pre-existing collection-contract gap, not a
new stream-callback regression, and remains required follow-up work rather than
a passing gate. Use this existing neutral fixture when batching generic
collection-return bridge/view fixes.

v278 (`.tmp/shared-static-callbacks-v278.log`) completes with all ten matching
variants (1 runtime test, 26.85s), generic-method binding (19, 27.02s), and source
facade precedence (4, 27.62s) passing after the shared static-owner extraction.
v279 (`.tmp/packaged-static-callbacks-v279.log`) passes the packaged ten-variant
runtime (31.11s) and both adjusted bound-reference runtime tests (38.29s).
The full 1407-unit result above predates only the static-owner extraction; it
is not described as a new full-unit run of v279. The five separately documented
object-model failures still lack a clean-baseline attribution.

The connected source/wildcard and callback batches are now substantial enough
for a fresh full-slice measurement. v280 builds a new release producer
(`.tmp/jgrapht-release-v280.log`) for a frozen v48 run. No v48 corpus outcome
is claimed until that run terminates and its outputs are inspected. Keep Java
sources untouched and never repair a generated bridge to make the corpus pass.

The collection-return follow-up has a concrete shared-boundary lead:
`representation_lowering::adapt_trait_impl_forwarded_return` currently adapts
borrowing and optionality by hand and returns Result contracts unchanged. The
existing neutral failure needs a collection-view conversion between the solved
implementation and specialized trait slots. Investigate reusing the ordinary
slot-conversion planner for that bridge, including conversion inside a Result
payload when needed, rather than adding a set-specific `.into()` string patch.
Verify which solved view metadata reaches the bridge before changing it; an
erased Java type alone cannot establish the emitted collection carrier.

v280 builds successfully (1m52s). Frozen v48 is
`.tmp/new-java2rust-v48-source-wildcard-stream-contracts`, SHA-256
`04231f071762e0dab69c7432b18bfe63df8d6238ad12ec69a4d3d8ad0dd9573b`.
The 104-file run is active at `.tmp/jgrapht-core-1.5.3-focused-v48`, log
`.tmp/jgrapht-v48-run.log`, with the standard pinned untouched-source runner.
Resolver-bench diagnostics match v47, but the elapsed benchmark increases from
36.55s to 46.05s, and translation's global signature phase increases from 23.19s
to 41.17s. These are observed timings, not proof of one exclusive cause.

A concrete avoidable-complexity lead is
`shape_projection::java_type_is_platform_facade_in_plan`: the new source
provenance guard asks `RepresentationPlan::type_representation` for every named
type before testing whether it is a platform facade at all. Missing generic
binders can trigger the existing unique-simple-name catalog scan. Move the
cheap platform-facade predicate before that lookup in the next coherent batch;
non-facade names need no provenance lookup. Preserve the source Reader and
qualified-native controls and measure the result rather than adding a new cache.

v281 adds a smaller neutral regression in `tests/generic_collection_returns.rs`
while the frozen corpus run proceeds. Its baseline
(`.tmp/generic-collection-return-baseline-v281.log`) fails with three compiler
errors (23.81s): both plain and Result trait-return bridges lack the native-set
to semantic-set conversion, and a concrete owned set producer is adapted as an
iterator (`self.values().cloned().collect()` without iteration). Keep all three
related boundary failures and the key-deduplication/error behavior assertions;
do not drop the Result or producer-composition case to make the test green.

v48 completes (runner exit 1: generated Rust does not compile). All 104 files
parse and translate with zero translator failures. The checker reports 2533
error lines including its aggregate failure line: **2532 actual compiler
errors**, versus v47's 2536. Invalid patterns decrease from 56/31 files to
54/29 files. The comparator factory call now emits the erased nominal static
owner and passes `graph` directly, without the v47 unsupported dyn-to-dyn
conversion. This is evidence of that boundary fix, not a compiling graph crate.

Elapsed resolver/translation/assembly/check times are
46156/306527/69606/13345ms; total 439348ms, versus v47's 323620ms. Translation
global analysis is 79519ms, signature planning 41173ms
(constraint collection 24745, fixpoint 7760, plan build 8188), and emission
207713ms. The avoidable platform-provenance catalog lookup above is a concrete
next optimization; timing improvement is not yet verified. Preserve the new
source-precedence controls while changing its cost.

Leading compile categories remain mismatches (529), `V != V` (82), missing
V/E Clone bounds (81/80), incompatible `?` payloads (77), and `&V == &V` (72).
E/V static-lifetime errors each decrease by two. Removing an unsupported
standalone-super placeholder exposes a still-incorrect Object.clone lowering:
`IntrusiveEdge` attempts Rust Clone despite boxed Any fields and returns a
hierarchy carrier where its erased-object contract requires a boxed value.
Those clone/return errors are open work, not successful shallow-clone semantics;
do not add a corpus-specific Clone implementation or count placeholder removal
as completing the Java rule. The full translated-Java behavior oracle is still
blocked on the non-compiling crate, so the goal remains active and incomplete.

### Collection return contracts and lookup cost (v282 onward)

The next coherent batch keeps v281's three boundary failures intact. Source
interface and source-class-bound trait return forwarding now use the ordinary
slot-conversion planner with the solved implementation and specialized contract
slots. Successful Result payloads are converted through Result::map, preserving
callee error values and single evaluation. This replaces hand-maintained
borrow/nullability-only forwarding rules; it does not special-case a corpus
method or append an untyped set-specific `.into()` patch.

v282's temporary conversion trace
(`.tmp/generic-collection-bridge-trace-v282.log`) proves that both trait return
contracts retain JavaSemanticHash view metadata while their concrete source
implementations return native owned sets. Those two bridge compiler errors are
fixed by the shared planner. Its remaining compiler error (24.41s) is the owned
producer incorrectly treated as a map-values iterator.

Inspection locates that error in map-view fast paths: an unknown receiver was
accepted unless it could be proved to be a non-map. After implicit-this
canonicalization, a source method named `values` consequently became
`self.values().cloned().collect()` even though it returns an owned set. A common
proof helper now requires call planning to select native MapKeys/MapValues/
MapIter before the direct view or collection-copy-constructor fast path runs.
The ordinary source method path handles namesakes. Existing carrier conversion
also uses a sequence-element iterator for borrowed sets instead of a map-pair
destructuring iterator. Temporary traces are removed.

The neutral runtime fixture now covers owned and stored/borrowed producers,
plain and Result trait contracts, error propagation, duplicate-key semantics,
source `values`/`keySet`/`entrySet` namesakes and copying a source producer, plus
native map-values/key-set copy controls. v283
(`.tmp/collection-boundary-batch-v283.log`) passes all 33 selected collection
library units (39.13s) and the expanded runtime (54.56s).

The performance change puts the existing platform-facade predicate before its
source-provenance catalog lookup. Non-facade generic/source names cannot require
the platform exception, so they no longer trigger a unique-simple-name scan in
that predicate. A test-only scan counter verifies this complexity property and
retains a native Reader positive control; source Reader/qualified native tests
remain required. No new production cache or invalidation policy is added.
The counter test was renamed after v283 compilation had already captured its
old name, so it is not counted among that filtered run's 33 passing units.

v284 runs the full library-unit suite, the expanded collection fixture,
interface-value contracts, nested generic representation, source-facade
precedence, coercion, and the imported source-stream regression together.
It is still active at this entry. v48 remains the latest completed corpus
measurement; neither a speed improvement nor a successful JGraphT crate is
claimed from the focused checks alone.

v284 (`.tmp/collection-boundary-broad-v284.log`) completes successfully: all
1408 library units pass (156.88s), including the catalog-scan counter control;
the expanded return-contract runtime (1, 30.77s), interface-value contracts
(4, 24.35s), nested generic representation (3, 26.92s), source-facade precedence
(4, 21.82s), all five source-stream tests (67.44s), and coercion (119, 0.23s)
pass. In particular, the imported-interface fixture that failed in both v47
and v48 now assembles, compiles and returns its expected Java behavior result.
The full batch covers 1544 tests (1408 units plus 136 integration tests).

v285 builds a release producer (`.tmp/jgrapht-release-v285.log`) after those
gates. Freeze it as v49 before timing, and run the old/new resolver benchmark
against the identical untouched 104-file staged source set without overlapping
translator compilation or changing the mutable debug producer during CLI tests.
Compare analysis counters as well as timing. The next full-slice compile count
has not yet been measured; do not present the fixed neutral regressions as a
new JGraphT crate result. The separately documented object-model and shallow
Object.clone issues remain open.

v285 completes successfully (1m38s). Frozen v49 is
`.tmp/new-java2rust-v49-collection-return-contracts`, SHA-256
`543b9139bf9dee950b4879ffefd6dedd21e492df68d87f93dbf24ca510c0fdf9`.
v286 runs frozen v48 and v49 consecutively with the same command:
`resolver-bench .tmp/jgrapht-core-1.5.3-focus-src`, without overlapping Cargo
compilation. Outputs are `.tmp/jgrapht-lookup-old-v286.log` and
`.tmp/jgrapht-lookup-new-v286.log`. Both complete successfully and have identical
output after removing elapsed/throughput and phase-timing fields: 104 parsed
and resolved files, 167 semantic diagnostics, 189139 facts, and 234 signature
fixpoint iterations, among the unchanged analysis counters.

Old/new elapsed times are 102742/99714ms (about a 3% difference); their
representation phases take 81352/79971ms and resolution phases 21039/19340ms.
The old frozen binary itself is much slower than its original 46046ms v48
measurement, so this pair does not prove a substantial wall-time improvement.
The counter regression proves the redundant scans are removed, but do not claim
that the corpus's overall speed regression is solved. Further profiling should
start with the dominant representation preflight, including repeated
context/program planning, and repeated source-provenance/emitted-name lookup
work; avoid adding global caches without a correct immutable-input lifetime.

This batch fixes the existing neutral compile/runtime failures and passes the
1544-test gate. It does not yet replace v48's full JGraphT compile measurement:
the next full-slice run must establish v49's remaining compiler errors,
placeholders and whole-pipeline timing before widening production emission.
Keep the full goal active.

After the completed batch gates and benchmark comparison, the standard v49
104-file runner is started with the frozen producer, output
`.tmp/jgrapht-core-1.5.3-focused-v49`, log `.tmp/jgrapht-v49-run.log`, JOBS=10.
It is active at this entry. Poll its existing process; do not restart it merely
because an observation times out. No v49 compile outcome is claimed yet.

### v49 full-slice outcome and interface-capability follow-up

The existing v49 run (session 42590) completed with runner exit 1 for generated
Rust compilation, not a translator failure. All 104 files parse and translate;
54 invalid patterns remain in 29 files. The compile log contains 2,493 typed
errors and 37 explicit fail-closed errors, totaling **2,530 actual errors**.
The runner's 2,531 count includes the aggregate "could not compile" line. This
is two fewer actual errors than v48, not a completed translation.

Evidence is `.tmp/jgrapht-core-1.5.3-focused-v49/{summary.tsv,compile.log,
phase-events.tsv,invalid-patterns.tsv}`. Resolver/translation/assembly/check
times were 83,470 / 354,163 / 100,818 / 24,107 ms, total 567,599 ms. This run
overlapped focused development compilation; these timings do not establish a
speedup. The paired immutable-producer benchmark above remains the controlled
lookup comparison. Leading failures include 527 mismatches, 81 missing
`V: Clone` and 80 missing `E: Clone` bounds.

v287 adds a corpus-neutral interface-default regression to
`tests/generic_clone_capabilities.rs`. The baseline reproduces three missing
`T: Clone` errors in the virtual/default-helper bodies, plus an unnecessary
`Unused: Clone` requirement on a generic caller. v288 reuses the existing
source-trait method-bound query for interface signatures: the three body
errors disappear and only the unwanted unused-parameter requirement remains;
the two established clone-capability integration tests pass. Logs are
`.tmp/interface-default-baseline-v287.log` and
`.tmp/interface-default-fixed-v288.log`.

The related batch replaces the overlapping name-only receiver rule with the
existing instantiated-receiver capability path, keeps capability requirements
structured until inherited owner arguments are substituted, and reuses ordinary
callable generic-bound inference for default methods' own binders. The expanded
fixture covers renamed inherited binders, shadowing method binders, boxed and
shared forwarding, and non-Clone handle-only arguments. v289 caught two local
API wiring errors before generated-code checks; those are corrected for the
combined v290 gate. No success for that gate or the new batch's corpus outcome
is claimed before completion.

v290 compiles the translator but the expanded neutral fixture exposes 17
generated-code errors: generic default forwarding through an erased upcast,
ancestor parameter names surviving in concrete inherited signatures, native
set carriers selected under an out-of-scope binder, and missing clones in
inherited interface-default bodies. The two established clone fixtures pass.
This is evidence for further shared fixes, not a reason to weaken the fixture.

The follow-up uses general source ancestry (interfaces as well as classes) in
inherited signature planning, substitutes declaration-local constraint types
before carrier selection, and removes lowering's redundant second local-type
substitution. Emitted parameter types reuse the same rewritten-slot projection
as body environments. Default-body clone-availability checks use the effective
method, while signature/exception lookups keep declaration identity. An upcast
inherits a generic parent default only when the derived surface selects that
exact same declaration; an override must not silently fall back to the parent.
The shared bound projector now resolves nominal bounds in their lexical owner,
preserving the existing qualified-package regression when interface method
generics reuse ordinary callable inference.

v291 (`.tmp/interface-capability-broad-v291.log`) runs all library units and
the clone, key, interface-value and default-inheritance suites together with
`--no-fail-fast`. It includes transposed inherited arguments and a focused
method-binder/owner-capability negative control. Its outcome is pending.

v291 completes successfully (session 72582, exit 0): **1,423 tests pass**,
comprising all 1,409 library units, three clone-capability integrations, three
key-capability integrations, four default-inheritance integrations and four
interface-value integrations. The stronger default fixture now compiles and
executes its direct, erased, shared, specialized and transposed paths. No
payload `Clone`/key capability is imposed on its unused handle argument.
`cargo fmt` and `git diff --check` pass. v292
(`.tmp/interface-capability-cross-gates-v292.log`) checks collection-return,
production interface-upcast assembly, generic-method binding, nested generic
representation, source-facade precedence and stream-source resolution together
before freezing the next corpus producer. Its outcome is pending; v49 remains
the latest measured full corpus.

v292 completes with 31 passing integrations and two failures, both in nested
generic defaults. The collection-return, 19 generic-binding, production upcast,
source-precedence and five stream gates pass. The unchanged nested tests reject
an unnecessary `JavaMapKey` bound on methods that only put existing set/map
carriers into lists; they also expose a generic-retarget signature using the
unsubstituted declaration binder when its owner substitution map is empty.

The follow-up factors direct-key detection from recursive type inventory:
callable/implementation capability analysis checks the current carrier's own
key position and gives parsed source declarations precedence, rather than
treating any nested key-bearing value as an operation on its payload. The
existing recursive inventory remains available to consumers that actually
convert whole nested carriers. Retarget signature substitution now explicitly
includes identity owner arguments before composing the target frame. A unit
checks nested list elements/map values, actual map keys, source `Set<T>` and a
source wrapper with a key-bearing stored field. No existing integration
expectations are weakened.

v293 runs the focused `item_assembly::tests` unit group followed by all ten
related integration targets in one final follow-up gate. Logs are
`.tmp/interface-capability-unit-followup-v293.log` and
`.tmp/interface-capability-final-gates-v293.log`; session 19425 is live at this
entry. The full-library v291 pass predates this final key-analysis follow-up;
do not treat its 1,409-unit result as a fresh full-unit run of the follow-up.

v293 completes successfully (session 19425, exit 0): **35 focused assembly/
capability units and all 47 integrations across the ten targets pass**. This
includes both unchanged nested-default controls that failed in v292, plus
production assembled upcasts and imported-interface streams. There are no
accepted failures in these gates. The preceding full-library v291 pass remains
documented separately, since the final direct-key follow-up came afterward.
`git diff --check` passes. v294 builds the release producer in
`.tmp/interface-capability-release-v294.log` before freezing v50 and running a
new full slice. No corpus improvement for the new batch is claimed yet.

v294 completes successfully (release build 2m08s, session 94784). Frozen v50 is
`.tmp/new-java2rust-v50-interface-capabilities`, SHA-256
`63d933baa82b358547fb6e6bb868872acd18eeb5607f52bba17412060f9a3b53`.
The standard untouched 104-file runner is now live as session **50764**, with
JOBS=10, output `.tmp/jgrapht-core-1.5.3-focused-v50` and log
`.tmp/jgrapht-v50-run.log`. Development compilation gates have finished before
this run starts. Poll the existing session; an observation timeout is not a
terminal result or a reason to restart it. v49 remains the latest completed
corpus measurement until v50 actually finishes.

### v50 outcome and generic value-equality batch

v50 completes (session 50764, runner exit 1 for generated Rust): all 104 files
parse/translate, zero translator failures, **2,505 actual compiler errors**
(2,468 typed plus 37 explicit fail-closed errors), and 54 invalid patterns in
29 files. The summary's 2,506 count includes the aggregate compile-failure line.
Compared with v49, V-Clone errors fall from 81 to 61 and E-Clone errors from 80
to 74; the total error reduction is 25. No translated Java oracle is claimed.

Resolver / translation / assembly / check take 37,921 / 249,461 / 61,397 /
12,581 ms; total is 364,690 ms (about six minutes). Development compilation
was idle during this run, whereas v49 overlapped compilation; do not attribute
the entire timing difference to a translator speedup. Evidence is under
`.tmp/jgrapht-core-1.5.3-focused-v50` and `.tmp/jgrapht-v50-run.log`.

The next related family is generic Java value equality. Read-only inspection
finds native `.equals` lowered to Rust comparison, including negated and
nullable-receiver paths. The existing `JavaMapKey` capability already supplies
Java equality independently of `PartialEq`; reuse its contract rather than
adding blanket Rust bounds. Preserve source overrides, null-receiver errors,
argument evaluation order, and the distinction from reference `==`. The first
neutral fixture is `tests/fixtures/generic_value_equality.java`; frozen v50
baseline output is `.tmp/generic-value-equality-v295-baseline.rs`.

The draft v296 integration reproduces comparison/borrowing failures, but its
first generic-bound control had an invalid Java erasure clash. v297 corrects
that control to a legal overload, validates the whole Java fixture with
`javac`, and records frozen v50 output in
`.tmp/generic-value-equality-v297-baseline.rs`. Do not use the draft's error
count as the valid Java baseline. The validated fixture covers generic and
negated equality, default interfaces, an unrelated unused binder, a source
overload and source-bound overload, null arguments inferred from another
argument, and a nullable receiver whose right operand has observable effects.

The batch adds borrowed value-equality helpers backed by `JavaMapKey`, not
Rust comparison. The nullable helper returns an optional result so ordinary
null projection checks the receiver only after both operands are evaluated;
the helper itself does not force exception runtime support into every output.
The shared callable-capability worklist propagates the equality requirement.
Trait-to-concrete argument borrowing reuses ordinary slot conversion. Source
generic argument instantiation substitutes declaration-proven binders while
retaining solved carrier constructors and wildcard witnesses. Source-bound
equality delegates to the existing typed-bound call lowering, not the native
comparison shortcut. A reference-operator negative control remains separate.

v298 (`.tmp/generic-value-equality-batch-v298.log`) passes the three clone and
19 generic-binding regressions but exposes remaining default-body physical
slot mismatches and null specializations. v299's cheap resolver probe
(`.tmp/generic-null-frame-v299.log`) proves both null call sites already have
`String` targets. v300's temporary ABI trace
(`.tmp/generic-value-equality-abi-v300.log`) narrows the remaining three errors
to source declarations being rejected as duplicate context matches, leaving
callee `T` in the caller's typed None. The temporary trace is removed.

Default bodies now use the exact specialized trait-parameter plan used by
their printed signatures. The shared emitted-owner lookup gives an exact
primary declaration precedence over its cloned planning views, while retaining
cross-input ambiguity checks for simple names and emitted aliases. Generic
retarget-adapter eligibility also consults solved ownership in one shared
predicate: borrowed dependent inputs/outputs cannot be fabricated into owned
checked conversions, even when no unchecked cast actually uses that adapter.
The valid source-bound control must therefore compile without an unsolicited
invalid adapter; an unsupported explicit retarget remains fail-closed.

v301 (`.tmp/generic-value-equality-source-frame-v301.log`, session 64954)
checks the equality, generic-binding, clone and nested-representation targets
together. New units cover binder substitution on a null argument without
replacing a solved TreeMap carrier, owned generic trait-argument borrowing,
and exact-primary lookup versus ambiguous simple names. Full batch/corpus
verification remains pending; v50 is the latest completed corpus measurement.

v301 completes with all 28 integrations passing. v302 then runs the complete
library suite plus equality/collection, key, default-interface, interface-value,
production-upcast, source-precedence and stream gates. All 25 integrations pass,
including the expanded negated-nullable evaluation-order case and a source-bound
overload implemented without JavaMapKey. The library reports 1,412 passes and
one failure: `emitted_context_type_lookup_refuses_cross_program_duplicate_owner_name`.
That failure rejects the attempted *global* primary-owner precedence change;
it is not accepted as a baseline failure or removed from coverage.

The correction restores global owner ambiguity rejection. Resolved callable
ABI lookup now reuses the existing primary-scoped declaration lookup used by
emitted declaration rewrites, with emitted-context fallback, in one shared
helper consumed by argument planning, binder specialization and signature
lookup. This distinction matters: a resolved call to the primary definition
needs its own emitted ABI even when support views clone it, while an unscoped
name query must not arbitrarily pick between duplicate input declarations.
The new unit checks both properties together. v303
(`.tmp/generic-value-equality-final-v303.log`, session 41112) reruns all library
units and eleven related integration targets with no failures excluded. Its
outcome is pending. No v51 corpus outcome is claimed.

This batch addresses method `.equals` for compatible generic payloads using
the existing Java equality capability. It does not turn Java reference `==`
into value equality, supply every missing concrete source-key implementation,
or claim the `Objects.equals` identity-shortcut/hashCode families are solved.
Those remain subject to the next measured compiler/root-cause inventory.

v303 completes successfully (session 41112, exit 0): **all 1,413 library units
and all 50 integrations in the eleven targets pass**, totaling 1,463 tests.
The original duplicate-owner rejection is preserved. The equality fixture's
negated nullable path also checks argument effects before an exception, and a
source-bound overload executes without requiring JavaMapKey on its payload.
`cargo fmt` and `git diff --check` pass.

v304 (`.tmp/generic-value-equality-assembly-v304.log`) runs a four-file,
cross-package production assembly regression. It compiles/runs the exact Java
inputs with a separate Java probe when the JDK is available, then translates
those untouched fixture inputs through the focused production runner and
checks a separate Rust consumer against that result. It covers inferred null
arguments, ordinary/negated generic equality and a specialized default
interface implementation. Its outcome is pending; no generated Rust is edited.

v304 completes (session 43884, exit 0): the Java oracle executes and returns
**15**, and the production-translated/assembled Rust consumer agrees. All four
fixture Java inputs are the same bytes for the Java compiler and translator;
both probes are separate consumers. No generated Rust was repaired. This
adds one assembly/oracle integration to the 1,463-test v303 gate. The JDK is
available locally; no oracle-skip path was taken.

v305 builds the release producer in
`.tmp/generic-value-equality-release-v305.log` before freezing v51. The whole
104-file corpus must still measure the new equality/ABI changes; v50's 2,505
errors and 54 invalid patterns remain the latest completed measurement.

## Java behavior oracle

`scripts/run-jgrapht-java-oracle.sh` passes against the pinned release built by
the preparation script. The maintained probe is
`tests/corpus/jgrapht/GraphOracle.java`; its 17 expected TSV records live beside
it. It covers graph construction, duplicate-edge rejection, degree, BFS reachable
membership and hop path, weighted Dijkstra paths/distances, unreachable results,
edge removal and vertex removal. All JGraphT classes directly used by the probe
are in the selected production slice. Reachable membership is sorted rather than
requiring unspecified graph iteration order; path comparisons use unique optima.

The runner records actual output, input artifact/fixture SHA-256 hashes and Java
version under `.tmp/jgrapht-java-oracle`. It verifies the untouched source pin
before using the locally built core jar and pinned JHeaps 0.14 dependency. This
establishes the Java baseline only: the corresponding translated-Rust behavior
comparison remains required and has not passed yet.

The first object-model suite run during this batch reported these failures:

- `object_model_input_usage_contributes_dispatch_requirements`
- `object_model_keeps_source_interface_dyn_trait_for_resolved_receiver_call`
- `object_model_uses_erased_object_for_unknown_runtime_objects`
- `object_model_uses_erased_object_when_trait_objects_are_rejected`
- `resolved_constructor_interface_flow_keeps_dyn_trait_storage`

Their attribution has not been established by a clean-baseline comparison. They
remain open verification work, not accepted baseline exclusions.

## Completion and expansion

The selected slice is not yet translated successfully: emitted code still has
invalid placeholders and compile errors. Representative graph construction,
traversal, mutation and shortest-path results must still be matched against the
established Java oracle. Do not expand
the emitted production scope until those gates pass and the established
translator/corpus gates have been checked. Broader source context can be used
earlier when it is needed to diagnose a general resolution rule.
