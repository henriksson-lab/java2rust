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

The latest accepted incremental full-slice checkpoint is **v101, release**: all 104 files
parse/translate without translator or generated parse failures, but the crate
still has 1,867 actual Rust diagnostics (1,784 typed + 83 untyped), versus
v99's 1,869 and
v97/v96's 1,870 and
v95's 1,908,
v93's 1,910, v91's 1,908 and v89's 1,902. Invalid markers are 82 across 41 files,
versus v99's 83 across 42 files and
v89's 87 across 44 files.
This is not a clean corpus gate. V96 removes 38 capability diagnostics, including
all six targeted upcast caller failures, with no added header/file diagnostics.
V95 closes two inherited cursor-factory capability errors. The v90
candidate had regressed to 2,420 errors; the directional, transfer-only
nullability correction in v91 removes 512 of those. Its positive and negative
element-domain regressions pass, including generated execution.

Retained-handle `.iter()` errors fall from 55 in
v85 to eight in v86, six in v87, one bulk operand in v89, and zero direct
retained-handle `.iter()` failures in v99. All 16 exposed
invalid `.as_ref()` calls on
owned generic cursor elements are gone, as are both array-source `.iter()`
errors. All 16 generic V expect() errors are also gone after reconciling generic
parameter and source-result contracts. Remaining generic equality and other
return projections still need work. Three exposed cursor capability errors are
gone in v92 and two more in v95. V96 separates adapter-wide and method-local
requirements and clones shared reference carriers without cloning payloads;
the six inherited upcast caller errors are gone. V97 closes neutral stored
callback result-domain cases but changes no corpus diagnostic header/file pairs.
V99 closes the retained bulk operand without adding header/file diagnostics.
V101 closes CollectionUtil's native Iterable List type-test/cast/get failures,
with two diagnostic removals and zero added header/file diagnostics. The broader
v100 candidate is rejected after exposing unrelated qualifier propagation.
The newer v102 materialization candidate has 79 markers but 1,868 diagnostics;
it removes three allocation guards and exposes four callback errors. Its v103
follow-up has 1,860 diagnostics and the same 79 markers, but exposes missing
checked source-call effects and a generic downcast in newly emitted callbacks.
The v104 follow-up reaches 1,829 diagnostics and 76 markers. V105 reaches
**1,777 diagnostics (1,699 typed + 78 untyped)** with the same 76 markers:
it removes 52 diagnostic header/file occurrences with none added relative to
v104, but eight additions relative to accepted v101 still remain. It is not
promoted yet; see the materialization section for the audited delta.
V106's checked-joining/native-entry batch reaches **1,775 diagnostics
(1,697 typed + 78 untyped)** and 76 markers across 41 files, with no translation
or generated parse failures. Against v105 it removes three Planarity joining
diagnostics and adds one NeighborCache shared-object receiver diagnostic.
Against accepted v101 it removes 98 header/file occurrences and adds six;
v101 remains accepted until those additions are resolved or explicitly justified.
V107 includes the native stored-object receiver and virtual object-string
batches. It reaches **1,766 diagnostics (1,688 typed + 78 untyped)**, with all
104 files translating, zero generated parse failures and the same 76 markers
across 41 files. Against v106, 76 header/file occurrences disappear and 67
appear; against accepted v101, 172 disappear and 71 appear. These are diagnostic
multisets, not independent bug counts. The lower total is not an acceptance
gate: v107 remains an unaccepted measured candidate, and v101 remains accepted.
The final related gate passes 1,521 library tests and 62 integrations, with
additional identity/Java gates and independently reproduced older failures
documented below.
The newer **v108** native/open string-contract batch reaches **1,740 diagnostics
(1,662 typed + 78 untyped)**, with all 104 files translating, no generated parse
failures and the same 76 markers across 41 files. It removes 27 header/file
occurrences and adds one versus v107; versus accepted v101, it removes 183 and
adds 56. The final related gate passes 1,523 library tests and 64 integrations,
plus fifteen historical identity and eleven selected Java differential cases.
V108 is not an accepted or clean checkpoint.
V109 has **1,737 diagnostics
(1,659 typed + 78 untyped)**. All 104 files translate, generated parse failures
remain zero, and the 76 markers across 41 files are unchanged. It removes three
header/file occurrences versus v108 and adds none: Planarity's nested checked
virtual-string result and the two extra try operators on BoyerMyrvold display
views. The final related gate passes 1,523 library tests and 32 integrations,
plus 15 historical identity and 11 selected Java differential cases. V109 is
still not a clean or promoted checkpoint; v101 remains accepted.
The latest measured candidate is **v110**, with **1,730 diagnostics
(1,652 typed + 78 untyped)**, zero translator/generated parse failures and the
same 76 markers across 41 files. It removes eight header/file occurrences
versus v109 and exposes one additional shared-field array borrowing error.
All four targeted nested-source double-Option joins and the owned nullable
`.cloned()` error are gone. The new error is independently reproduced as an
older latent shared-field array-write problem with frozen v109. V110's related
gate passes 1,526 library tests and 61 integrations, plus the selected historical
gates and controlled pre-existing failures documented below. V110 is not
promoted. Prioritize shared-field array mutation/guard lifetimes and the
remaining nullable formatting consumers; the separate escaping mapped-stream
gap remains unfinished.
Also pending are bounded source-interface interoperability, the remaining
42 native allocation guards (39 shared-to-plain, three plain-to-shared),
nullable/view boundaries and pending source-List projections. Do not remove the guards by
snapshotting aliased collections or manually repairing generated Rust.

The renewed bounded optimization pass finished at v79 with byte-identical
control/candidate output. Accepted v101 runtime is 391.961 seconds; v99 took
437.813 seconds, versus v97's
289.000 seconds and v96's
347.637 seconds and v95's
295.748 seconds and v93's
273.700 seconds and v92's
277.423 seconds and v91's
276.817 seconds and v90's
276.895 seconds, v89's 299.172 seconds and v88's
288.655 seconds and v87's
273.361 seconds, v86's
276.560 seconds, v85's
276.576 seconds and v84's
272.559 seconds, v83's 279.300 seconds and v79's 266.595 seconds. The new
correctness checkpoint alone does not establish a new complexity blocker.
V99 translation takes 246.306 seconds, preflight 55.322, assembly 106.570 and
Rust checking 22.623. Signature fixpoint is 6.167 seconds. The slowdown spans
unchanged semantic/representation phases too. V97 had returned close to v95's
earlier range after v96's increase; timing variation is not yet isolated.
A paired frozen resolver control/candidate check takes 82.784/80.592 seconds
(v97/v99) on identical inputs with no competing agent-owned workload. The
unchanged v97 control is itself much slower than its historical 46.636-second
preflight. This does not isolate emission/assembly cost or prove full-pipeline
performance neutrality; keep timing as a watch item rather than reopening
open-ended optimization from this run.
v80/v81 debug
diagnostics are not comparable speed measurements, and v80 aborted before
assembly. Producer hashes, sizes and build-profile labels are now recorded.
The v752 batched-source gate passes all 1,503 library tests and 109
integration tests. Its only failure is the established retained-Iterable List
cast/get acceptance, with the same two errors. Two earlier reporting/provenance
tests also pass. The List operation
view runtime exists, but its source/cast/slot wiring is not complete.

The pinned Java behavior oracle has passed; translated graph behavior remains
unverified because the crate does not compile. The combined goal stays active
in correctness work. Production emission has not widened beyond 104 sources.

### Historical summary

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
invalid-pattern count unchanged. The generic method equality/physical source ABI
batch is measured in v51: 151 fewer compiler errors, with invalid patterns
unchanged. Java `.equals` now uses the shared value capability for compatible
generic payloads; Java reference `==` remains a separate open operation. The
callable-completion/assembly/override-identity batch is measured in v52: 40
fewer compiler errors, with invalid patterns unchanged. Nullable stream callback
work continued in subsequent checkpoints.
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
Default runs build release. When selecting a frozen `NEW_JAVA2RUST_BIN`, also
set `NEW_JAVA2RUST_BUILD_PROFILE=release` (or the actual profile) and inspect
`translation-producer.tsv`; the prebuilt label is caller-declared, not inferred.
Never compare debug and release timing or assume a filename proves a profile.

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

v305 completed successfully (release build 1m34s). Frozen v51 producer:
`.tmp/new-java2rust-v51-generic-value-equality`, SHA-256
`cec3607023a804b057c25852f56d5d64a80bf0dba2fc78b6012e6bdd492ffb0d`.
The unchanged 104-file corpus measurement is running in session 96006, with
output `.tmp/jgrapht-core-1.5.3-focused-v51` and log
`.tmp/jgrapht-v51-run.log`. Development compilation is idle during this run.
This is a pending measurement, not evidence that the generated crate compiles.

While v51 runs, v306 isolates a separate default-interface completion defect.
The valid Java fixture `tests/fixtures/default_void_fallibility.java` passes
`javac`; the frozen v51 producer emits 12 Rust E0308 errors because its virtual,
default-helper and owner-qualified-helper void bodies omit successful Result
completion. Baseline output/logs are `.tmp/default-void-fallibility-v306-*`.
The next batch shares the ordinary method-boundary completion helper with
default methods, including early returns, branch fallthrough and caught throws.

v307 independently reproduces assembly retaining methods from an obsolete
seed representation after selecting a richer declaration. The new neutral
`Cursor<T>` regression fails before the fix: an old `impl<T: Clone>` containing
obsolete field accesses survives beside the new `impl<T>`. See
`.tmp/support-closure-v307-baseline.log`. The merger edit is deliberately deferred
until v51 assembly completes so that measurement does not mix script versions.
No development Cargo build/test ran during the measurement; small standalone
fixture emissions, javac/rustc probes and formatting did run, so timing is not
a strictly isolated benchmark.

v51 completed (session 96006, exit 1 for generated Rust errors). All 104 files
parse and translate, with zero translator failures. Actual compiler errors are
2,354 = 2,317 typed errors plus 37 explicit untyped fail-closed errors; the
summary's 2,355 includes the final aggregate diagnostic. This is 151 fewer than
v50's 2,505. Invalid patterns remain 54 across 29 files. Timing (ms): resolver
68,041; translation 333,419; assembly 138,019; cargo check 13,686; total 556,373.
Do not claim a speedup: these timings are slower than v50 and not an isolated
benchmark. Translated graph behavior is still unverified and emission remains
limited to the 104-file closure.

The next batch now shares unit-Result completion between ordinary/default
methods and discards seed inherent methods when their physical declaration is
replaced. Same-representation seed surface reconciliation is retained. All 10
standalone assembly regressions pass after the merger fix, including the
non-Clone Cursor compile/runtime probe
(`.tmp/support-closure-v307-fixed.log`). v308 runs the combined interface-default
and support-closure gates in `.tmp/default-completion-assembly-v308.log` (session
90724); its outcome is pending.

v308 completed successfully: all 5 interface-default integrations and all 10
support-closure tests pass. The new default fixture's generated crate compiles
and exercises success/error propagation, early return, branching, catch
recovery and both helper entrypoints. v309's attempted standalone rustc build
of the larger script suite was not a product test: that suite links the
translator crate, so it requires Cargo. It is included in the next combined
gate instead; no checks are excluded.

v310 adds a production-runner integration combining iterator support with the
same fallible defaults and a separate consumer. Its iterator Java fixture also
passes javac. Frozen v51 emission confirms `CursorUse.run` returns Result, so
the consumer unwraps that established ABI rather than assuming infallibility.
v311 runs the full library units and selected assembly/equality/default/null/
exception integrations together in `.tmp/completion-representation-gate-v311.log`.
Its live session is 61144 and its terminal outcome is pending. All 1,413 library
units have passed (192.30s), followed by the 3 array-exception integrations and
3 generic-equality integrations. Remaining targets, including the new
production-assembly probe, must still finish. No further full-corpus rerun has
been made for these individual fixes.

v311 is terminal (exit 101): all 1,413 library units and all 31 selected
non-script integrations pass, including the new production iterator/default
probe. The script suite has 81 passing and 4 failing tests. v312 checks the
four failures against copies of the committed HEAD merger, item utilities and
focused runner under `.tmp/assembly-seed-baseline.QPLq3f`; all four reproduce
there (`.tmp/idiom-sweep-checkpoint-v312.log`). They are not silently accepted
or excluded from verification.

The assembly follow-up fixes the three underlying cleanup defects: cached
catalog extraction now receives the same source-ownership inventory as final
module extraction; stale prelude shadows of concrete post-primary declarations
are removable without deleting source items; and a stale alias manifest does
not count as an unrelated shared nominal declaration. This preserves the
single-catalog performance design instead of adding another parsing policy.
The fourth failure was an outdated structural assertion: the runner already
shares one restoration helper between both ordered boundary passes. The test
now follows those calls and still asserts pre-index and post-import ordering.

v313 reruns the four checks through a standalone test executable linked to the
current translator library: all four pass
(`.tmp/idiom-sweep-current-v313.log`). v314 reruns all 85 script tests together
with support closure and three production-assembly integrations in
`.tmp/assembly-ownership-gate-v314.log`; its outcome is pending. No product Rust
code changed after v311's successful full-library gate.

v314 has passed all 85 script tests (74.03s); its production-assembly targets
are still running in session 71236. The source Rust version for that gate is
the same completion batch that passed v311's library units.

v315 establishes the next failure with three valid neutral Java files under
`tests/fixtures/nested_override_fallibility`: all pass javac. Frozen v51 batch
translation succeeds in `.tmp/nested-override-fallibility-v315`, but the nested
interface emits `read -> Option<T>` while the implementation emits
`read -> Result<Option<T>, JavaException>`; its trait forwarder therefore has
an inconsistent ABI. This is the same diagnostic shape seen in the corpus's
path-result implementations. The existing override-group walk only used an
exact-name map and missed an imported nested interface. The next source batch
uses the inheritance module's owner-context lookup, with a per-program cache
of hierarchy-edge lookups shared across method slots. A focused analysis unit
and production-assembly runtime regression have been added; verification is
pending and no new corpus measurement is claimed.

v314 completed successfully (session 71236, exit 0): 85 script tests, 10
support-closure tests, and all three production assembly/consumer probes pass
(generic equality, interface upcast, iterator/default completion). Thus the
four checkpoint failures identified by v312 are fixed, not baseline exclusions.

For the next override-identity batch, v316's formatting check caught a misplaced
test block inside an existing raw Java string before Cargo compilation began.
The test block was relocated; formatting now passes. v317 runs the complete
exception-plan unit subset in `.tmp/nested-override-analysis-v317.log` (session
25100). It includes imported nested generic override equalization and an exact
nested-owner negative control with two unrelated `Reader` interfaces. Its
terminal outcome and the new production-assembly regression are still pending.

v317 completed successfully (exit 0): all 23 exception-plan units pass,
including both new owner-identity controls and the existing non-throwing
override-group control. v318 now runs the full 1,415 library units with the new
nested-override production probe and related generic/default/upcast/iterator
integrations in `.tmp/nested-override-combined-v318.log` (session 93618). Its
terminal outcome is pending; no additional corpus run has started.

v318 has passed all 1,415 library units (149.01s); production integrations are
still finishing. While waiting, v319 records a separate next-batch probe in
`tests/fixtures/nullable_stream_capture.java`. It passes javac, but frozen v51
output calls `.contains` directly on a captured `Option<HashSet<i32>>` inside
both anyMatch/allMatch closures. Rustc reports two E0599 errors in
`.tmp/nullable-stream-capture-v319-compile.log`. This is a concrete nullable
native-capture lowering gap, not yet a verified cause of all corpus closure
errors. No stream product edits have been made in this batch.

v318 completed with one integration failure: all 1,415 library units and all
33 other selected integrations pass, but the new imported-nested override
production crate reports one E0277 error (plus Cargo's aggregate diagnostic).
Its interface, implementation and caller now agree on Result, proving the
override lookup correction is active. Assembly instead drops
`Clone for Box<dyn Scope_Reader<T>>`, making an emitted generic-retarget
adapter's Clone fail. The retained test output is
`.tmp/test-tmp/new_java2rust_batch_nested_override_fallibility_1792245928791736788/out`.

The source-restoration helper only inspected trait impls after the filename's
primary declaration. A nested interface and its boxed Clone legitimately occur
before that boundary. The follow-up uses the existing source-owned-trait-impl
predicate for trait companions anywhere in the raw module; weak pre-primary
inherent methods remain excluded. A new cheap script regression checks nested
boxed Clone recovery, repeatable restoration, and exclusion of an unrelated
prelude Clone. v320 runs all 86 script tests and the nested-override/upcast/
iterator production integrations in `.tmp/nested-companion-assembly-v320.log`
(session 8663). Its outcome is pending. No product Rust changed after v318's
successful full-library gate.

v320 completed successfully (session 8663, exit 0): all 86 script tests and all
three production assembly/consumer probes pass, including the previously
failing imported-nested-interface case (29.92s). The interface and concrete
method now share their Result ABI, error propagation executes through the
interface, and assembly retains the nested trait's required boxed Clone.
All 1,415 library units remain verified by v318; no product Rust changed since
that run. v321 builds the release producer in
`.tmp/callable-assembly-contracts-release-v321.log` (session 18642) before
freezing v52.
v51's 2,354 errors and 54 invalid patterns remain the latest completed corpus
measurement until that run finishes.

v321 completed successfully (release build 1m36s). Frozen v52 producer:
`.tmp/new-java2rust-v52-callable-assembly-contracts`, SHA-256
`b5753c933c3c0487e55b5bb305ec0fd66edb612870c063821c9f15bbf3ef4a23`.
The unchanged 104-file corpus measurement is running in session 53587 with output
`.tmp/jgrapht-core-1.5.3-focused-v52` and log `.tmp/jgrapht-v52-run.log`.
Development compilation will remain idle during the run. Runner/helper hashes:
`translate-focused-package.sh` =
`6fce35f9b6eb09d7ae7421980777b5a8fa12fbf4004e3767052842179331e313`;
`merge-shared-support-facades.py` =
`c6af180a412d7890003b05c0b48c36e58eeccd9d67937e8c7ae0ffbfce340816`;
`rust_item_utils.py` =
`76bc6e818ba38cdd8fbb8b1299adbc6982de5dcdbabe759656fa25abaa24f373`.
This is pending evidence, not a successful generated-crate translation.

The next-batch nullable native stream-capture probe now has a generated-crate
regression in `tests/fallible_stream_matching.rs`, covering empty-stream
laziness, catchable null dereference on consumption, and present-set any/all
results. It is not included in v320's passing checks and is expected to fail
until the next product fix; the frozen v51 rustc baseline already establishes
its two invalid Option membership calls. No stream product change is included
in v52.

v52 completed (session 53587, exit 1 for generated Rust errors): all 104 files
parse and translate, with zero translator failures. Actual errors are 2,314 =
2,277 typed errors plus 37 explicit untyped errors; the summary's 2,315 also
counts Cargo's aggregate diagnostic. This is 40 fewer than v51. The generated
crate still does not compile and translated graph behavior remains unverified.
Timing (ms): resolver 38,193; translation 248,736; assembly 62,429; cargo check
12,515; total 365,263. Compilation was idle during this run. These timings are
better than v51 but similar to v50; do not attribute all variation to a code
speedup. The 31 closure-local `?` errors remain in the new diagnostic baseline,
and the minimized nullable-native-capture regression is the next batch target.

The stream follow-up shares the ordinary fallible optional-receiver primitive
and slot projection with stream calls, including staged operand evaluation.
All operands are initialized as one tuple before receiver validation, so
argument side effects/errors precede a null-receiver error and generated
temporary names cannot shadow source operands. Nullable unbound receiver
projection also builds its Try before terminal callback-carrier selection.
No generated Rust was patched.

v322 runs the expanded stream regressions plus collection equality and source
stream resolution (`.tmp/nullable-stream-receivers-v322.log`, terminal exit
101). Collection equality, all 5 source-resolution tests and the pre-existing
fallible-stream integration pass. The new regression still fails to compile:
basic captured-null membership now lowers through fallible lazy matching, but
two source-call key arguments lack a borrow, and nullable array-element method
references lose their physical input shape (two invalid member calls plus six
consumer ABI errors). The Java oracle probe passes with assertions enabled:
`.tmp/NullableStreamCaptureProbe.java`, compiled with the exact fixture under
`.tmp/nullable-stream-capture-java-v322`, prints `nullable-stream-capture: ok`.

The next edits must carry solved source element shapes into the shared stream
plan and its consumed-callback effect analysis, rather than reconstructing
nullability in the renderer. The existing catch-aware ExpressionEffect walker
is available for that effect analysis. A separate general conversion follow-up
now permits CopyValue to satisfy a shared-borrow slot, with a focused unit;
this edit and lazy default-facts allocation cleanup are newer than v322 and
still unverified. The current stream batch is incomplete; do not freeze it or
claim its regressions pass yet. v52 remains the latest verified/frozen producer.

The subsequent v324 focused check still failed on reference-array callbacks.
A standalone solved-signature probe (`.tmp/stream-shape-probe-v325.log`)
established the missing contract: a Java `String[]` parameter keeps the logical
array type while its emitted element storage is nullable. The stream planner
now reuses the existing indexed-read projection for that physical element
contract, instead of guessing nullability from the declared Java element type.
The callback visitor separates deferred input contracts from immediate
invocation effects. Nullable source callback arguments enter the ordinary
source-call constraints; contextual lambdas use the existing overload resolver.
Catch-aware null effects and non-null filter refinement use those same plans.

v326 (`.tmp/nullable-stream-contracts-v326.log`, terminal exit 101) reduced the
new regression to one error: the static method-reference parameter accepted
null, but the equivalent lambda's source parameter did not. v327 adds the
contextual source-call resolution; its generated-crate nullable-capture test,
the existing fallible-stream test, collection equality, and all five
source-resolution tests pass (session 34555, exit 0). A subsequent source-only
provenance guard and a cheap overload/deferred/filter usage regression are newer
than that build and require the broader gate. The exact fixture's Java probe
again prints `nullable-stream-capture: ok` with assertions enabled. No generated
Rust or corpus Java was edited; v52 remains the latest frozen corpus baseline.

The broader v328 gate (`.tmp/nullable-stream-combined-v328.log`, session
77307) reports 1,417 of 1,418 library tests passing. The nullable input-flow,
deferred execution and CopyValue borrow controls pass; the constructor-reference
namespace control fails because its rendered pipeline returns None. Directly
rerunning that compiled test reproduces the failure. Inspection identifies a
general receiver/argument confusion in the shared method-reference helper:
constructors receive the null-checked receiver operand instead of the original
value operand. The staged follow-up fixes that operand selection and adds a
structured regression proving no receiver Try is introduced. It is newer than
v328 and unverified. v328's integration targets continue running; do not count
the whole batch as green or freeze a new corpus producer yet.

v328 finished with all 35 integration tests across its 12 targets passing;
session 77307 exits 101 solely for the library constructor-reference failure
above. v329 (`.tmp/nullable-stream-reference-repair-v329.log`, session 29672)
rebuilds once with the constructor operand fix and runs the library's
reference-related tests, including the original failure and the new structured
no-spurious-dereference regression. Its result is pending at this note.

v329 finished successfully: all 33 selected reference-related library tests
pass, including both constructor controls (session 29672, exit 0). Together
with v328's 1,417 other passing library tests and 35 passing integration tests,
this closes the current batch's observed regression. `cargo fmt -- --check`
and `git diff --check` pass. A new frozen release build and corpus measurement
are the next step; the last actual JGraphT error count is still v52's 2,314,
not a measurement of these newer stream changes. No verification process from
v327–v329 remains live.

v330 builds the release producer successfully (1m26s). Frozen v53:
`.tmp/new-java2rust-v53-nullable-stream-contracts`, SHA-256
`6215157b9eb66554cc52ce96b2aeb3e74fc5c7e84c7d0932e27d625e578193dd`.
Runner/assembly/helper hashes remain the same as v52. The 104-file measurement
under `.tmp/jgrapht-core-1.5.3-focused-v53` finishes with session 31041 exit 1;
`.tmp/jgrapht-v53-run.log` records zero translator failures and zero parse
failures, but 2,318 actual Rust errors (2,281 typed plus 37 explicit untyped;
summary 2,319 includes Cargo's aggregate). All 54 invalid patterns across 29
files remain. This is four more errors than v52, not an error-count improvement.
The diagnostic change is localized to Graph and GraphMetrics: optional
receivers now project, exposing unsupported fallible intermediate stream
filters and their ordinary-call fallback. Closure-local `?` errors rise from
31 to 34. Times (ms): resolver 37,222; translation 233,477; assembly 62,042;
cargo check 11,831; total 347,825. No development Cargo compile overlapped this
measurement. The generated crate still fails, and Rust graph behavior remains
unverified. The unchanged Java source scope is still 104 files.

The next coherent batch lifts intermediate filters and maps over Result items,
adds consuming forEach/forEachOrdered to the shared stream plan, and carries
errors through count/findFirst and the existing matching/collection/primitive
terminals. Callback bodies remain outside generated helper binding scopes.
The new neutral fixture is `tests/fixtures/fallible_stream_pipeline.java`; its
maintained Java oracle `tests/corpus/stream/FallibleStreamPipelineOracle.java`
passes with `javac` and `java -ea` (v331). Frozen v53 fails to compile that exact
fixture with 12 errors: unsupported pipeline fallbacks plus a closure-local Try
and a lost String method-reference receiver. No generated source was patched.
The staged product edits are newer than v53; v332 focused integration is
running in `.tmp/fallible-stream-pipeline-v332.log` (session 33219). This new
batch is not verified or frozen yet. Fallible flat-map, deferred iterator
traversal, reducer callbacks and joining/map-collector callbacks still require
their own correct traversal contracts; never consume Result as the Java item.

v332 completes with the two established stream integrations passing and the
new pipeline regression failing on three errors: primitive findFirst's
unrecognized Optional return contract, and statement-form forEach bypassing
the shared pipeline. v333 catches four compile-time arity mistakes while
threading callback input modes through block statement helpers; those are
corrected before v334. v334 keeps the two established stream checks and the
primitive-extrema integration green, but both new standalone/production
pipeline checks fail on the same one remaining fallback: the shared lambda
planner rejects void block callbacks before the consumer renderer is reached.
The staged follow-up admits void blocks only for consumers, preserving value
callback validation, with a cheap plan-level regression. Receiver capture now
moves callbacks supplied to runtime adapter function calls as well as iterator
methods; a neutral bound source predicate and a structured unit cover it.
The Java oracle is extended for that bound case and still passes with -ea.

The complete v335 batch is running (`.tmp/fallible-stream-pipeline-combined-v335.log`,
session 46776): full library plus fallible-stream, source-resolution,
collectors, bound references, primitive extrema, collection returns, collection
equality and support-closure integrations. It includes both generated-crate
runtime assertions and production-assembly runtime assertions for the new
pipeline fixture. This is pending verification, not a passing batch or a new
corpus measurement. The v53 run and v330–v334 checks are terminal; v335 is the
only live verification process at this note. Do not start a conflicting CLI
build while its assembly integration uses the debug producer.

v335's full library passes all 1,421 tests. Both new pipeline integrations
now compile, including the assembled crate, and reach their runtime probes.
They expose the same semantic failure: `nonNull([0,1,0,2])` incorrectly throws
NullPointerException. Inspection of the generated source shows the non-null
filter omitted altogether. Its renderer consulted the provisional plan input
instead of the actual nullable callback return slot. The follow-up reuses
`non_null_filter_output_shape` on the actual item shape, for ordinary and Result
streams alike; no generated code is edited. The neutral fixture and Java
oracle now also cover an infallible nullable-returning callback, to prevent
fixing only the Result branch. The extended Java oracle passes with -ea.
These follow-up edits are newer than v335 and still require verification.
The remaining v335 integration targets are still running at this note.

For disk headroom, only the obsolete `.tmp/stream-shape-probe` executable
(144 MB, created for v325) is removed. Its source `.tmp/stream_shape_probe.rs`
and `.tmp/stream-shape-probe-v325.log` are preserved; the binary is rebuildable.

v335 finishes (session 46776, exit 101): all 1,421 library tests and 30
integration tests pass; only the two new runtime probes fail as described
above. The v336 follow-up builds library/integration executables once with
`cargo test --lib --test fallible_stream_matching --no-run`, then runs the
reported executables without a second compile. All 29 selected stream-related
library tests pass (`.tmp/fallible-stream-refinement-units-v336.log`, session
42068 exit 0). All four fallible-stream integrations pass, including standalone
and production-assembly runtime probes and both nullable producer controls
(`.tmp/fallible-stream-refinement-runtime-v336.log`, session 42199 exit 0).
`git diff --check` passes, and the pinned JGraphT tracked checkout remains
untouched. This closes the observed pipeline regression without weakening its
Java comparisons or repairing generated Rust.

v337 builds the next release producer in `.tmp/fallible-stream-release-v337.log`;
its result is pending at this note. The intended next frozen producer/output
are `.tmp/new-java2rust-v54-fallible-stream-pipelines` and
`.tmp/jgrapht-core-1.5.3-focused-v54`, both checked absent before the build.
The current actual corpus baseline remains v53's 2,318 errors and 54 invalid
patterns until that new run completes. All v335/v336 test sessions are terminal.

v337 finishes successfully (release build 1m27s). Frozen v54 producer:
`.tmp/new-java2rust-v54-fallible-stream-pipelines`, SHA-256
`f7a1f77c6714ca04a341379d98153d9d90bf0d5fbcae2bf4ea9adf03b88bd9a9`.
Runner/assembly/helper hashes are unchanged from v53. Its fresh 104-file run
is live in session 41940, with log `.tmp/jgrapht-v54-run.log` and output
`.tmp/jgrapht-core-1.5.3-focused-v54`. No development compile is running beside
it. Poll this session before starting or replacing any measurement; do not
infer termination from an unchanged summary log while phases are still active.

v54 finishes (session 41940, exit 1 for generated Rust): all 104 files parse
and translate, with zero translator failures. There are 2,314 actual errors =
2,277 typed plus 37 explicit untyped; the summary's 2,315 includes Cargo's
aggregate. This is four fewer than v53 and the same total as v52. All 54 invalid
patterns across 29 files remain. The three GraphMetrics closure-local Try
errors exposed by v53 disappear; 31 such errors remain elsewhere, including
source functional-parameter contracts. GraphMetrics now exposes a malformed
mutable-borrow token (`&mutgraph`) and a further carrier mismatch. The former
comes from reconstructing an operator in stream receiver staging instead of
using the existing canonical mutable-borrow projection; the follow-up reuses
that shared builder, with a neutral mutating Set callback regression.

v54 times (ms): resolver 37,954; translation 229,076; assembly 62,498; cargo check
12,080; total 345,218. No development Cargo build overlapped the measurement;
one small frozen-producer/standalone-rustc callback probe ran alongside it.
These similar timings are not evidence of a new performance win. The generated
crate still does not compile; translated graph behavior is unverified, and
production scope remains the unchanged 104-file closure.

The next source-callback batch has a minimized fixture,
`tests/fixtures/source_callback_contracts.java`. Frozen v54 produces seven
compiler errors (`.tmp/source-callback-contracts-v338-{baseline.rs,compile.log}`):
two mismatched physical Cell/Rc callback input contracts, incompatible plain
versus Result callback returns, and forwarded functional parameters kept as
opaque facade objects. Rendering callback argument/result types now reuses
canonical Java **value-slot** projection and slot rendering, including source
storage placement; raw nominal-type rendering intentionally does not wrap a
source declaration in Rc. The first v339 unit exposes that distinction and
an incomplete test setup (storage planning must run as in the normal pipeline).
Its 35 existing assembly units pass and the new unit fails; the staged follow-up
uses the normal storage planner and value-slot renderer instead of a bespoke
carrier rule. Callback effect/forwarding contracts remain work in progress;
do not count the whole new fixture as translated or freeze this partial batch.

v340 (`.tmp/callback-carrier-mutable-stream-build-v340.log`, session 67658)
builds library and fallible-stream integration executables once for focused
verification of the physical callback slots and mutable receiver follow-up.
The maintained stream Java oracle passes with its added mutable/null receiver
cases. v339, v338 and the v54 corpus session are terminal; v340 is the only
live build at this note.

v340 completes its physical-slot checks: all 36 item-assembly units and 29
stream-related units pass (`.tmp/callback-carrier-mutable-stream-units-v340.log`,
session 74445 exit 0). A diagnostic translation of the complete source-callback
fixture now has five errors, down from seven: the Cell/Rc mismatches are gone,
while callback effect and forwarding failures remain
(`.tmp/source-callback-contracts-v340-{shaped.rs,compile.log}`). Its maintained
Java oracle `tests/corpus/callback/SourceCallbackContractsOracle.java` passes
with assertions enabled. Do not mistake those two corrected carrier errors
for completion of the source-callback batch.

The mutating-stream runtime checks in v340 (session 17177, exit 101) expose
one further shared routing omission: Set.add still goes through a Java-named
fallback instead of the existing native SetInsert renderer. The mutable
borrow itself now compiles. The follow-up admits canonical native collection
targets to shared stream call lowering, reusing existing argument planning and
`apply_call_target` mappings rather than duplicating collection method names.
v341 (`.tmp/native-callback-carriers-combined-v341.log`, session 67017) runs
full library plus fallible-stream, collectors, collection returns, source
resolution, bound references and collection equality integrations together.
This latest routing edit is not verified yet; no newer corpus producer is frozen.
All earlier v338–v340 probes are terminal.

Source-callback follow-up requirements: use one per-source-callable parameter
contract for declaration, actual lambda/method-reference arguments, forwarded
parameters and SAM invocation. Reuse the existing functional-interface shape,
source overload resolution and call graph/effect machinery. A callback's error
carrier must not be inferred merely from its enclosing method's fallibility,
nor should every Predicate be globally made fallible. Passing a functional
parameter into another proven nonescaping source callback slot must retain
that callable representation. Mutable callbacks also need the correct owned
or mutable callable argument, not an unconditional shared borrow. These remain
open implementation work; v341 does not claim to cover them.

v341 finishes successfully (session 67017, exit 0): all 1,422 library tests and
21 selected integration tests pass, including the mutating Set callback in
standalone and production assembly. This verifies the canonical native
collection routing and physical callback carrier fixes, not the pending effect
contract of the full source-callback fixture.

The next shared plan is `src/source_callbacks.rs`, retained by SignaturePlan.
It classifies declaration-scoped callback slots, uses the existing escape
scanner with a source-forwarding proof hook, and propagates escape evidence
backwards with a finite dependency worklist. Resolved source declaration
lookup is extracted into `java_signature::resolved_source_method_decl` and
reused by both the call graph and callback plan. Header emission, parameter
invocation and source argument adaptation consult the same plan. Actual
lambdas/function references are not borrowed as opaque facade objects;
forwarded inline parameters are mutably reborrowed for repeated invocation.
The established scanner and retained-comparator checks are moved out of
item assembly into the semantic module, rather than copied. A source class
named IntPredicate is kept on its source-declared path, not the JDK SAM path.

v342 focused checks pass: 2 callback-plan tests, 37 item-assembly tests and
62 usage tests (`.tmp/source-callback-forwarding-units-v342.log`). v343 passes
all four established fallible-stream tests and the new infallible callback
forwarding runtime test (`.tmp/source-callback-forwarding-runtime-v343.log`,
session 70061 exit 0). The Java forwarding oracle passes with assertions enabled.
v344 detects one missing relocated-test import and then runs out of disk while
archiving the library; this is not a product pass. Only the explicit disposable
`.tmp/focused-cargo-target` Cargo cache is cleaned (461 files, Cargo reports
2.0 GiB); source, generated evidence, logs and frozen producers are preserved.
The cache is rebuildable, and the rerun recreates useful entries normally.

v345 (`.tmp/source-callback-common-plan-v345.log`, session 48859 exit 101)
passes all 1,425 library tests, four fallible-stream integrations and both source
callback integrations, including repeated mutable invocation and source
functional lookalikes. Its layering suite reports 84 pass / 15 fail. A fresh
`git archive HEAD` of committed checkpoint `ecf3ceb`, exported under
`.tmp/layering-baseline.MbDNCq`, is compiled directly with rustc and run from its
own root. It reproduces exactly the same 15 failed test names (84 pass / 15 fail,
`.tmp/layering-baseline.MbDNCq/layering-baseline.log`; sorted-name diff is empty).
No layering test is weakened or disabled; these are attributed pre-existing
failures, still recorded as open architecture/gate maintenance.

A follow-up shares the declaring callback contract with inherited shells using
the existing inherited-member resolver; its new focused unit is running as
v346 (`.tmp/source-callback-inheritance-v346.log`, session 60618). It is newer
than v345. A diagnostic emission of the full original source-callback fixture
with v345's producer is running in session 96825, writing
`.tmp/source-callback-contracts-v345-{forwarded.rs,compile.log}`. Callback error
carriers remain incomplete; no new corpus producer is frozen. Recursive
forwarding needs a stable callable ABI rather than unbounded nested generic
reborrows, and override/object-safe callback contracts and retained callable
lifetimes also need coverage before treating this as a general completed ABI.

v346 passes all 3 callback-plan units, including inherited contract reuse
(session 60618 exit 0). The full source-callback diagnostic now has exactly
3 errors, down from 7 at v338 and 5 at v340: two lambda-local Try errors and
one method-reference Result/bool mismatch. Forwarded parameters no longer
remain opaque facade values. Session 96825 is terminal. All verification
processes through v346 are terminal at this note. The next implementation
step is callback parameter effects integrated with the existing call graph
and fallibility solver, so invocation, formal ABI and actual callbacks share
one error-carrier decision; do not freeze the partial callback batch yet.

### Callback effect contracts, staged after v346

The next batch adds declaration-scoped `CallableKey::CallbackParameter` nodes
and forwarding-contract equality edges to the existing call graph and
fallibility solver. Typed lambda bodies retain their own intrinsic-effect
analysis; passing a callback does not execute it. Source method-reference
resolution now handles complete SAM input lists (including zero inputs and
the receiver plus remaining inputs of an unbound reference), reusing ordinary
Java overload selection. Unary stream resolution delegates to that same
implementation. Functional Java input substitution is shared with lowering.

Focused failures identified and fixed two additional general issues: unused
non-escaping parameters also need a callable representation, and normalizing
`Worker::same` must not bind a known type owner to a local named `worker`.
Exact Java lexical bindings still take precedence over type owners.

This is staged analysis work, **not yet connected to production callback
emission**. The new graph tests cover forwarding, recursion, overloads,
lambda-local division/throw, unused callbacks, independent contracts, and
zero-argument/bound/unbound method references. The v351 full library run passes
all 1,429 tests (`.tmp/callback-effects-v351-lib.log`, session 5646, exit 0).
The v352 generated-Rust gates also pass all 4 fallible-stream tests and both
source-callback tests (`.tmp/callback-effects-v352-integration.log`, session
5886, exit 0), including production stream crate assembly. Formatting and
`git diff --check` pass. These gates cover existing emitted behavior, not the
still-staged callback Result ABI.
Earlier focused diagnostic runs v347--v350 are
failures or, for the first v347 command, zero selected tests; do not count
them as successful gates.

One additional pre-existing semantic mismatch is now localized:
`collect_thrown_exception_types_from_expr` eagerly scans lambda arguments to
most calls by method name. It records a `MethodThrowFact` for a source call
whose callback is never invoked. The new graph regression deliberately uses
resolved declaration facts plus solved signatures to isolate the new graph
contract; it does **not** claim that this semantic fact projection is fixed.
Execution evidence must be shared with that scan before production integration.
Then connect formal FnMut result types, invocation propagation and actual
lambda/method-reference adapters to the same fallibility nodes. Recursive
forwarding still needs a stable callable ABI. No new corpus producer or
measurement supersedes v54 yet; the three v345 callback-fixture emission
errors remain the latest measured baseline for that fixture.

### Source callback error ABI, v353 onward

v353 shares execution and retention evidence with the existing source callback
plan. Escape, possible invocation and retention each propagate backwards over
the forwarding graph using the same linear worklist; forwarding alone and a
forwarding-only recursive SCC do not prove invocation. The semantic throw scan
now consumes this evidence instead of eagerly scanning every source lambda
argument. A source method named `submit` that actually invokes its parameter
also takes precedence over the old external asynchronous-call name heuristic.
All 23 callback-focused library tests pass (`.tmp/callback-execution-v353.log`,
session 25241, exit 0). The earlier graph regression now uses complete semantic
facts; its temporary declaration-facts isolation is removed.

Production lowering now uses the augmented call graph and its existing
fallibility solver. Formal SAM return types, SAM invocation propagation,
expression/block lambdas and pure-reference Result adapters consume the same
declaration-scoped callback nodes. A structured `RustType::DynFnTrait` renders
non-escaping source slots as borrowed dynamic callables, avoiding unbounded
generic reborrow instantiation in recursive forwarding. Java input/result
slots keep the canonical representation projection. The callable context
retains one source-symbol registry shared by graph construction and typed
reference overload resolution; it is not rebuilt per reference.

v354 passes all four source-callback generated-Rust runtime tests
(`.tmp/callback-result-abi-v354.log`, session 98495, exit 0). The original neutral
fixture, previously seven errors at v338 and three at v345, now compiles and
runs: alias-preserving source handles, throwing lambda and method reference,
source forwarding, plain callbacks sharing a fallible contract, and Java catch
behavior all pass. Additional coverage verifies recursive forwarding, throwing
void expression/block lambdas, a plain method reference lifted to a fallible
slot, and throwing callbacks that are passed but never executed.

The retained comparator path remains owned with a `'static` bound rather than
borrowing a temporary. Retention now propagates through source forwarders and
the header consumes that shared evidence. Native `Collections.sort` invocation
edges use the existing call statement plan, and its established fallible-sort
adapter consumes the same callback fallibility node. v356 is running the full
library suite plus four stream and six source-callback generated-Rust tests
(`.tmp/callback-result-abi-v356-gates.log`, session 83752). v355 stopped at two
test-only LowerCtx initializers missing the newly shared symbol registry; they
are updated in v356. The maintained Java callback oracle passes again under
`.tmp/source-callback-java-v356`.

This does not prove every Java callable ABI: source interface override-family
callback contracts, retained callback alias/capture ownership, and general
multi-input bound/unbound reference emission still require broader coverage.
The full JGraphT slice measurement remains v54 until a new producer is measured.

v356 finishes with 1,431 library and all four stream tests passing. Five of six
source-callback runtime tests pass, including owned retained comparator
forwarding. The native-consumer regression compiles but fails behaviorally:
the caller still sees the unsorted list (2 rather than 1). Generated evidence
shows the wrapper taking `Vec<i32>` and receiving a clone of the caller's list;
the assertion is retained, not weakened. Signature planning's argument-effect
lookup missed cached built-in evidence visible to usage analysis, and its
"fully modeled unknown boundary" check did not include KnownContentRead calls
even though the boundary classifier puts those in the same UnknownApi category.

The follow-up shares cached built-in argument-effect evidence with signature
planning via a borrowed/owned fact view, preserving explicit facts and parsed
source owner precedence. It handles both classified boundary variants. Native
sort also reborrows source callback parameters so repeated calls do not move
the dynamic callable. The runtime regression now sorts twice; a focused unit
checks that unmaterialized built-in facts yield MutableBorrow and that a parsed
source `Collections` definition still overrides the platform owner. Combined
verification is running as v357 (`.tmp/callback-result-abi-v357-gates.log`,
session 50415); this result is not yet established at this note.

v357 finishes with 1,431 passing library tests and the new unmaterialized-fact
unit failing: the central built-in index itself omitted the standalone
Java-platform argument table. The follow-up indexes that table through the
same insertion helper as manifest argument facts. v358 passes all 24 selected
callback units and four selected runtime tests, but the native sort runtime
still fails (the fifth selected runtime test; the lookalike test was filtered).
A direct signature probe (`.tmp/native-argument-probe-v359.{rs,log}`) localizes
the remaining issue: the parameter constraint list contains both MutableBorrow
and Owned. A late rule interpreting binding mutability as owned sequence
storage adds the conflicting Owned requirement after the correct in-place
mutation loan is already known. Preserving that existing loan fixes the
behavior without generated-output changes.

v359 passes all six source-callback generated-Rust runtime tests
(`.tmp/callback-result-abi-v359-runtime.log`, session 87646, exit 0), including
caller-visible list mutation across the source wrapper, repeated sorting with
one callback, and comparator error propagation. A new unit checks the full
solved mutable parameter shape rather than only the external-effect lookup.
Usage analysis also now rejects built-in mutation evidence, including its
legacy unscoped name fallback, for a parsed source owner; an existing precedence
unit covers a source `Collections` alongside materialized platform facts.
Final combined verification is v360 (`.tmp/callback-result-abi-v360-gates.log`,
session 53009; running when this note was written).

A fresh standalone layering build at v358 reports 84 pass / 15 fail
(`.tmp/layering-v358.log`). Its sorted failed test names exactly match the
attributed committed checkpoint baseline; no layering assertion was weakened.

v360 finishes successfully (session 53009, exit 0): all 1,433 library tests,
all four fallible-stream integration tests, and all six source-callback runtime
tests pass. Formatting and `git diff --check` also pass. The mutable argument
loan survives signature solving, in-place list changes are visible to the
caller, repeated native sort calls preserve the callback, source owners retain
precedence, and the original effectful callback fixture remains green.
The remaining callable ABI gaps above are still open; these results do not
establish successful JGraphT crate compilation or Java graph equivalence.

The next release producer is building in session 98900, with log
`.tmp/callback-contracts-v55-release.log`. Its intended frozen path is
`.tmp/new-java2rust-v55-source-callback-contracts` (verified absent before
starting). It is not frozen or corpus-measured yet: poll the same build handle,
then copy and hash the completed binary before starting a fresh v55 output run.

The release build completes in 1m27s (session 98900, exit 0). Frozen v55 is
`.tmp/new-java2rust-v55-source-callback-contracts`, SHA-256
`bcc661081bdaade43101c8516060a35de2d66fef9eac0d8239a4f5141c990e88`.
All three runner/assembly/helper hashes match v54. Its fresh 104-file run is
live in session 44380, with log `.tmp/jgrapht-v55-run.log` and output
`.tmp/jgrapht-core-1.5.3-focused-v55`. No development build overlaps this
measurement. Poll that handle; do not treat an unchanged log as termination.

v55 stops before crate assembly/checking (session 44380, exit 1). A native
collection call inside AsSubgraph's stream callbacks panics at
`call_lowering.rs` with "native collection target must provide receiver";
the batch reports a worker panic. There are 103 completed file emissions.
This is a translator regression, not a new Rust-error baseline; v54 remains
the latest completed full-slice measurement. The tracked Java remains untouched.

The shared stream-native route had inferred a static call from failure to
render an instance receiver. The follow-up uses the existing target plan's
receiver requirement: an unrenderable instance operand returns None rather
than reaching the instance projection with no receiver. The stream expression
renderer also handles explicit `this` when the current receiver is available,
which bound instance-field references need. Neutral tests cover explicit-this
collection access, an instance-field method reference, a cast receiver that
this renderer does not support, and absence of a current receiver. v361 focused
verification is running (`.tmp/stream-receiver-v361-focused.log`); no new
producer is frozen for this follow-up yet.

v361 passes all 10 stream-lowering units; the integration target selected zero
tests because the unit filter applied to both executables, so it is not a
runtime gate. The reported binaries are run directly next. v362 passes all
30 stream-related library tests (`.tmp/stream-receiver-v362-units.log`, session
54520, exit 0). Four existing runtime tests pass, but the new fixture's Rust
assertion incorrectly compares a Result-returning probe to a bare integer;
its generated library compiles. The assertion is corrected to unwrap.

The maintained neutral fixture is `tests/fixtures/stream_instance_fields.java`,
with Java oracle `tests/corpus/stream/InstanceFieldCallbacksOracle.java`.
Frozen v55 reproduces the exact receiver panic on this one-file fixture
(`.tmp/stream-instance-fields-v55-baseline.log`, session 32488, exit 101),
without needing the full corpus. The Java oracle passes under
`.tmp/stream-instance-fields-java-v363`. v363 passes all five generated-Rust
stream integrations (`.tmp/stream-receiver-v363-runtime.log`, session 91471,
exit 0), including instance-field references, explicit-this predicates and
production stream crate assembly. No corpus Java or generated Rust is repaired.

The corrected release build is live in session 78819, with log
`.tmp/stream-receiver-v56-release.log`. Intended frozen producer:
`.tmp/new-java2rust-v56-stream-instance-receivers` (checked absent before
starting). Poll that build before freezing/hashing it and measuring a fresh
v56 output directory. The failed v55 run and frozen producer remain preserved.

The release completes in 1m34s (session 78819, exit 0). Frozen v56:
`.tmp/new-java2rust-v56-stream-instance-receivers`, SHA-256
`38382d3bd460e76450e185d1662d0b406157261f0a610ed0510dfb987f05d3d5`.
All runner/assembly/helper hashes remain unchanged. The frozen binary
successfully translates the one-file panic reproducer (session 55509, exit 0;
`.tmp/stream-instance-fields-v56-smoke.{rs,log}`). Its fresh 104-file run is live
in session 67863, writing `.tmp/jgrapht-v56-run.log` and
`.tmp/jgrapht-core-1.5.3-focused-v56`. The smoke translation is terminal before
measurement begins; no development compile overlaps the corpus run.

v56 completes (session 67863, exit 1 for generated Rust): all 104 files parse
and translate with zero translator failures. Rust reports 2,276 actual errors
(2,240 typed + 36 explicit untyped); the summary's 2,277 includes Cargo's
aggregate. This is 38 fewer than v54. Invalid patterns fall from 54 to 53,
still across 29 files. Closure-local Try errors fall from 31 to 18. The crate
does not compile, so translated graph behavior remains unverified. The pinned
tracked Java checkout is still clean, and production scope remains 104 files.

v56 timings (ms): resolver 38,633; translation 226,365; assembly 63,145; Rust
check 11,483; total 342,902. These are similar to v54's 345,218 total, not
evidence of a meaningful performance improvement. No development compilation
or other translator probe overlaps the timed run.

Inspection groups the remaining callback errors into different contracts:
generated bulk-collection predicates capture fallible receiver expressions
inside plain boolean closures (for example containsAll), nested stream
predicates with nullable captures, and stored Supplier callbacks. The next
coherent investigation is operand staging for native collection bulk
operations: Java evaluates receiver/arguments once before the operation, not
once per item or only when an iterator is nonempty. Existing shared operand
staging and exception propagation should be reused; do not patch the emitted
closures or change Java input. Generic Java object/equality/Clone capabilities
and source iterable view representations remain larger independent clusters.

The next family's neutral baseline is now maintained in
`tests/fixtures/bulk_collection_operands.java`, with Java oracle
`tests/corpus/collection/BulkCollectionOperandsOracle.java`. The oracle passes
under `.tmp/bulk-collection-java-v364`. It requires receiver-then-argument
evaluation exactly once, including empty arguments; receiver-expression
exceptions before argument evaluation; argument exceptions before checking a
null invocation receiver; and no generated-binding/name collisions. It covers
containsAll/addAll/removeAll/retainAll and their boolean results.

Frozen v56 translates the fixture (session 20788 exit 0), but standalone Rust
checking fails with 11 typed errors (`.tmp/bulk-collection-v56-compile.log`).
The emitted baseline `.tmp/bulk-collection-v56-baseline.rs` demonstrates:
containsAll captures the fallible receiver inside a bool predicate; addAll
repeats its receiver expression; removeAll/retainAll evaluate the argument
before the receiver; nullable source collections remain Option at iterator
boundaries; and an internal `value` parameter shadows a source receiver also
named `value`. Borrowed slice arguments also acquire invalid Clone recovery.
No output is edited. This baseline is not registered as a passing Rust gate;
the next implementation batch must fix the shared operand/argument contract,
then register runtime comparisons against these Java requirements.

### Bulk collection operand staging follow-up

The bulk operation batch reuses `staged_instance_call_projection` at the
native-collection call boundary. Intact receiver/source carriers are evaluated
once in Java order; required nullable payloads are then bound outside internal
iterator predicates. Source collections are borrowed rather than coerced to the
receiver's owned collection kind. AddAll reuses the existing borrowed-element
recovery plan and nullable-array conversion. A proven same-place operand uses
one receiver borrow, with an element snapshot for self-addition. Equal rendered
calls are no longer accepted as proof that two operands alias.

The maintained Rust integration is `tests/bulk_collection_operands.rs`; it
exercises untouched Java fixture translation, not repaired output. Its Java
oracle now also checks identical-looking factory calls, all self-alias bulk
operations, void add/retain statements, and List/Set cross-kind sources. The
latest Java run, including nullable field self-aliases, passes under
`.tmp/bulk-collection-java-v370`.

v365 first passes the original operand-order runtime regression. v366 extends
the passing runtime with self-alias and repeated-factory cases, but its broader
42-test collection sweep reports 29 passes and 13 failures. Five assertions
expect the superseded emitted bulk-call layout. The relevant tests have been
updated to assert the staged layout and strengthened from generated-crate
checking to executing their membership/mutation results. The other eight
failures all produce nonempty, byte-identical Rust with frozen v56 and the
current CLI (`.tmp/bulk-unrelated-{v56,current}-<test-name>.{rs,log}`):

- `collection_to_array_empty_string_array_lowers_to_owned_vec`
- `constructor_owned_argument_clones_native_collection_field`
- `generic_collection_field_copy_and_iterator_add_clone_bounds`
- `htsjdk_parsing_utils_collection_adapter_is_executable_and_mutates_split_buffer`
- `loop_carried_nullable_collection_constructor_argument_is_cloned`
- `map_values_returning_collection_collects_vec`
- `shared_self_concrete_collection_field_clones_for_interface_return`
- `source_interface_collection_elements_use_boxed_dyn_trait`

That comparison attributes their emitted behavior to before this batch, not to
an accepted exemption from the overall goal. They remain open suite work.

v367 passes the alias-proof unit, the extended operand runtime and 11 related
collection tests. An additional seven-test extension sweep catches a real
nullable-array element conversion regression and three emitted-layout
assertions; restoring the existing array conversion fixes the regression.
v368 passes the alias-proof unit, the operand runtime, and all 17 selected
collection regressions (`.tmp/bulk-collection-v368-test.log`, session 93084).
This includes the nested nullable-list and wildcard-list runtime checks, source
collection mutation, semantic-key/cross-nullability remove statements, and map
view bulk contracts. No corpus source or generated Rust has been repaired.

v369 passes all 1,435 library tests, the cross-kind-extended operand integration,
all five stream integrations (including production assembly), and all six
source-callback integrations (`.tmp/bulk-collection-v369-regression.log`,
session 99783, exit 0). The final field-alias follow-up reuses the existing
stable-place classifier formerly scoped to map-entry projection, so fields and
array places need no conflicting shared/mutable operand borrow. Its alias-proof
unit rejects fields whose owners are effectful calls. The Java fixture checks
both successful self-bulk field mutation and a null self-field exception.
v370 passes the final alias-proof unit, the complete operand runtime (including
nullable field self-aliases and mixed List/Set source kinds), and all 17 selected
collection regressions (`.tmp/bulk-collection-v370-test.log`, session 47686,
exit 0). The corresponding Java oracle also passes.

The standalone v370 layering suite still reports 84 passes and 15 failures;
its sorted failure names are identical to v358, which matched the documented
checkpoint comparison. No layering assertions have been changed.

The statement-only removeAll path still owns specialized semantic-key and
cross-nullability membership lowering and is deliberately retained. Static
storage mutation and unmodifiable-view paths also retain their existing
contracts. Sharing their richer policies with the staged bulk boundary is
follow-up work; do not claim all collection-call boundaries are solved. The
latest full 104-file measurement remains v56; no new corpus error count or
performance claim follows from these focused regressions.

### v57 measurement and retained-callback follow-up

The v57 release build completes in 1m28s (`.tmp/bulk-collection-v57-release.log`,
session 60501). Its frozen executable is
`.tmp/new-java2rust-v57-bulk-operands`, SHA-256
`0c017ed5d989ab7bcc9fabe465d338bcc85a65be6b73080605f1e2a6eacfe659`.
The focused runner, assembly script and item-helper hashes are unchanged from
v56. No other build or translator probe overlaps the timed run.

The fresh 104-file measurement completes in session 49990, exit 1 for generated
Rust, under `.tmp/jgrapht-core-1.5.3-focused-v57` with log
`.tmp/jgrapht-v57-run.log`. All 104 files parse and translate; there are zero
translator failures. The Rust checker reports 2,258 actual errors (2,222 typed
and 36 explicit untyped), 18 fewer than v56. Invalid patterns remain 53 across
29 files. Closure-local Try errors fall from 18 to 9: three in a nested stream
predicate, two in a captured-field predicate, and four in retained Supplier
closures. Collection element-carrier conversions and broader generic/object
capabilities remain open; do not infer complete bulk collection support from
the error reduction. The generated crate still fails, and translated graph
behavior remains unverified. Production scope is unchanged at 104 files; the
pinned tracked Java checkout remains clean.

v57 timings (ms): resolver 38,666; translation 236,746; assembly 61,704; Rust
check 11,336; total 351,750. This is not a performance improvement over v56's
342,902 total. A follow-up cheap statement-candidate filter now uses the same
bulk-method name metadata as native call planning, avoiding another full plan
for unrelated one-argument statements. Source/native precedence remains with
the normal planner; the filter is not a dispatch decision. This change is newer
than the frozen v57 executable and has not been corpus-timed.

The next retained-callback batch also replaces derived Clone on functional
carriers with unconditional shared-handle cloning. A callback copy must share
its captured state and must not require cloning its input or output payloads.
The generated-support regression covers Supplier, Consumer, Function,
BiFunction and Runnable with deliberately non-Clone payloads, shared counter
state, and a source-style name colliding with Rust's Clone trait. The impl uses
the qualified Rust trait path. These runtime changes are newer than v57.
v371 accidentally selects zero unit tests because its unqualified filter is
combined with `--exact`; it is not counted as a regression pass. The corrected
v372 command passes both new units, the complete bulk operand integration, and
all 17 selected collection regressions (`.tmp/callback-clone-v372-test.log`,
session 6674, exit 0). v373 passes all 22 functional-shape/generic-carrier units
(`.tmp/callback-clone-v373-test.log`, session 20626, exit 0), including compiling
and executing generated callback support with non-Clone payloads.

The maintained next-family fixture is `tests/fixtures/retained_callback_state.java`,
with Java oracle `tests/corpus/callback/RetainedCallbackStateOracle.java`.
The Java oracle passes under `.tmp/retained-callback-java-v372`. It covers
returned mutable suppliers, reference-copy aliasing, pure primitive suppliers,
lazy array access, and catching an exception at callback invocation.
Frozen v57 translates it (session 35687, exit 0), but the unchanged emitted Rust
fails standalone checking with three E0618 errors
(`.tmp/retained-callback-v57-compile.log`): stored Supplier/IntSupplier objects
are invoked as bare FnMut values. The emitted baseline is
`.tmp/retained-callback-v57-baseline.rs`; no output is repaired. This exposes
the physical stored-carrier versus borrowed-inline callback distinction in
addition to the corpus's retained callback exception ABI. It is not yet a
passing translated-Rust integration gate.

The next coherent batch is retained callback representation and invocation ABI,
not a SupplierUtil-specific rewrite. `lower_functional_interface_local_call`
currently treats every non-parameter local as a bare closure, although
`lower_functional_slot_lambda` wraps non-argument storage in a facade carrier.
Use the same authoritative representation decision at construction, storage,
argument adaptation and invocation; reuse the existing source callback plan
for source formal slots. Also align deferred body fallibility and the stored
SAM call's runtime ABI, preserving lazy exceptions and shared captured state.
The new oracle must become a passing untouched-output Rust runtime gate before
that family can be claimed fixed. The remaining nested-stream callback errors
are related exception-boundary work but are not evidence that this stored
callback contract is already implemented.

### Stored callback implementation follow-up (v374 onward)

The initial implementation reuses the local callable marker instead of assuming
every local binding is a bare FnMut value. Stored JDK functional lambdas and
method references are now built through a shared callback-contract lowerer,
also used for source callback arguments. Their canonical runtime storage holds
FnMut returning Result, with `from_fallible_fn` construction and a SAM-specific
result-returning invocation method. Existing infallible `from_fn` construction
lifts its callback into Ok, and its plain SAM method uses the established
uncaught-exception adapter for runtime compatibility. Callback copies still
share one Rc/RefCell and impose no payload Clone requirement.

v374 compiles the translator but its generated-runtime unit fixture lacks the
JavaClass dependency of real exception support. The test now includes actual
class-token support rather than a stub. v375 passes both new generated-Rust
integrations (`.tmp/retained-callback-v375-runtime.log`, session 10886): returned
and local mutable suppliers, reference aliases, zero-input method references,
pure primitive suppliers, lazy index failure and a catch at invocation.

The next related edits generalize invocation from local variables to typed
receiver expressions, including callback fields and callbacks returned by a
call. Operand staging reuses the common receiver/argument ordering helper.
Native stored SAM external call keys now seed the shared fallibility solver,
so errors can propagate through source callers. Source formal callback keys
retain their existing precise source contract, and parsed source lookalikes
are excluded from native SAM effects. The fixture and Java oracle now cover
field invocation and a source caller catching a returned callback's index
exception. The extended Java oracle passes under
`.tmp/retained-callback-java-v376`.

v376 fails to compile a new unit test because its resolver helper call omits
the diagnostics argument; that test call is corrected without changing product
policy. The v377 runtime unit additionally exposes ClassLoader/URL/JavaEnum
dependencies of the class-token prelude. Instead of continuing to hand-assemble
runtime pieces, the unit now obtains the complete dependency closure through
production translation and places its callback probes in a nested scope.
v378 passes both units; its runtime checks expose the next effect-identity
mismatch rather than a facade type error.

Resolved platform calls use method keys, whereas typed unresolved calls use
external keys. Native stored SAM effects now seed both identities; typed
unresolved functional receiver calls preserve their declared owner in the call
graph. v379's positive caller checks pass. Its negative factory check used the
signature-free fallback, whose nullable-parameter conservatism differs from
production; the test now uses semantic facts, representation planning and solved
signatures before exception planning. Three runtime assertions also need to
unwrap the now-explicit Result ABI of narrow catches, which conservatively
leave other runtime callback exceptions unhandled.

v380 passes all three focused units, both retained callback integrations, and
all six existing source-callback integrations
(`.tmp/retained-callback-v380-test.log`, session 88931, exit 0). v381 adds the
explicit parsed-source guard to stored facade selection and passes all 1,439
library units, the bulk integration, all five stream integrations (including
production assembly), both retained integrations and all six source-callback
integrations (`.tmp/retained-callback-v381-regression.log`, session 54084).
Its layering check remains 84 passes / the same 15 recorded failures; sorted
names match v370 exactly (`.tmp/layering-v381.log`).

The callable-initializer producer marker is also used by priority-queue
constructor planning, so it is retained for those consumers. Invocation now
distinguishes a stored JDK facade from a bare closure even when that producer
marker is present. Stored locals keep their solved type annotation; non-facade
callable locals retain the old inference path. The existing returned-supplier
test is strengthened from compilation to execution, and the local-comparator
assertion is updated to the fallible constructor layout. The combined queue,
collection and retained-runtime check v382 completes with 21 passes and six
failures (`.tmp/retained-callback-v382-test.log`, session 82893). Five involve
ordinary source comparators/plain adapters whose comparisons return an i32,
but their callers try to unwrap it. Initial suspicion of broad native effect
seeding is not sufficient attribution; the frozen comparison below establishes
that these failures predate this batch. Independently, the stored Result runtime
entry now has its own external call key, derived from shared SAM result-method
metadata; both resolved and typed unresolved stored calls target that entry.
This keeps the stored runtime ABI distinct from source/override contracts for
plain Java method identities. The effect unit checks the distinction explicitly.
The remaining v382 failure expects ordinary subtraction;
the shared typed callback lowering now correctly emits Java wrapping integer
subtraction, and the layout assertion is updated accordingly.

v383 passes four selected units and 22 of the 27 queue/collection tests; the same
five source-comparator tests fail (`.tmp/retained-callback-v383-test.log`, session
90409). Direct execution of its already-built retained integration binary passes
both runtime tests (`.tmp/retained-callback-v383-runtime-direct.log`, session
46402, exit 0). No generated Rust is repaired.

Frozen v57 translates the exact Java programs from all five remaining tests,
and standalone Rust checking fails with the same categories: unconditional
unwrap_or_else on infallible source comparisons, plus the object-field case's
argument mismatch and invalid boxed-source-to-Comparator cast. Evidence is
`.tmp/queue-v57-<test-name>.{rs,log}` and
`.tmp/queue-v57-<test-name>-compile.log` (session 45460). The test names are:

- `priority_queue_comparator_factory_constructor_uses_comparator_carrier`
- `priority_queue_comparator_factory_local_constructor_uses_comparator_carrier`
- `priority_queue_comparator_object_field_constructor_uses_comparator_carrier`
- `priority_queue_comparator_object_local_constructor_uses_comparator_carrier`
- `priority_queue_named_comparator_constructor_requires_comparator_carrier`

These are open pre-existing bridge failures, not newly passing gates or accepted
exemptions from the goal. In particular, do not remove an unconditional unwrap
without planning the actual comparison ABI: fallible source implementations
still require proper adaptation.

v384 passes all 1,439 library tests, all five stream integrations (including
production crate assembly), both retained-callback integrations, and all six
existing source-callback integrations
(`.tmp/retained-callback-v384-regression.log`, session 28630, exit 0). This is
after the stored-runtime/source-method call-key separation. The 17 collection
regressions and five additional applicable queue/returned-supplier checks also
pass in v383; the five frozen-v57 queue failures remain open. The latest complete
104-file measurement is still v57; remeasure the full slice before claiming
any corpus-count or performance improvement for this callback batch.

Stored values passed to borrowed-inline callback parameters still need their
own value-to-callable adapter coverage; native bound callback method references
and stored callbacks consumed inside streams also remain follow-up boundaries.
These are not accepted exemptions from the goal. No generated Rust is edited,
and none of the v374-onward changes are included in the frozen v57 measurement.

### Frozen v58 stored-callback measurement and next capability probe

The frozen executable `.tmp/new-java2rust-v58-retained-callbacks` has SHA-256
`1bb376b10eb57078bad24f6f4536e2f02b3233ae8a7239be496fcea7d7c3cc1f`.
The full run used the unchanged pinned 104-file slice, `JOBS=10`, and output
`.tmp/jgrapht-core-1.5.3-focused-v58`; session 62117 exited 1 because the generated
crate does not compile. No other translator/compiler probes overlapped this run.
All 104 files translated without translator failures. Rust reports 2,183 typed
errors plus 36 explicit untyped errors (2,219 actual errors, excluding Cargo's
final summary), 39 fewer than v57. There are still 53 invalid patterns across
29 files. Generated graph behavior remains unverified.

Recorded milliseconds: resolver 37,522; translation 220,728; crate assembly
61,245; Rust check 11,114; total 333,833. The previous total was 351,750 ms;
one run is not sufficient evidence for a stable speedup. The full preceding
focused regression results above apply to this frozen executable's source.

After the corpus run completed, the untouched neutral fixture
`tests/fixtures/generic_interface_construction.java` was translated with v58.
Its unchanged generated Rust fails with two E0277 errors: `snapshot<T>` and
`membership<T>` return a newly constructed concrete class through a source
interface, but omit the `Clone` obligation required by that class's generated
trait implementation. The membership factory already propagates `JavaMapKey`;
the snapshot factory correctly does not require that unrelated capability.
Logs: `.tmp/generic-interface-construction-v58-translate.log` and
`.tmp/generic-interface-construction-v58-compile.log`. The maintained Java oracle
is `tests/corpus/collection/GenericInterfaceConstructionOracle.java`; `javac`
and `java -ea` pass in `.tmp/generic-interface-construction-java-v58`, printing
`generic-interface-construction: ok`.

The next implementation should reuse the existing trait-implementation
capability analysis at actual conversion boundaries. Merely constructing a
generic class must not indiscriminately inherit every trait-implementation
bound, and cloning an interface handle must not impose payload `Clone`.
This is an isolated, confirmed next failure class, not yet a passing regression
or a claim that all generic interface conversions are fixed.

### Interface conversion capability batch (after frozen v58)

The neutral reproduction now drives shared conversion-boundary analysis in
`item_assembly.rs`. Returns (including nested and conditional returns), typed
local initialization, assignments, explicit casts, and selected method and
constructor argument targets query the existing concrete trait-implementation
capability planner. Generic source-call forwarding also projects selected method
binders, including factories with no value arguments. Method return types and
owner context are passed explicitly rather than recovered by name matching.
Interface-handle copies and concrete construction do not acquire payload-copy
requirements merely because an interface or generic class mentions that type.

The stronger cast probe exposed a second real defect: a guaranteed widening
cast of an owned concrete temporary produced `&dyn Trait`, inconsistent with the
owned return slot. Shared shape projection now preserves an owned, boxed
interface upcast when object-safety and the hierarchy relation are proven.
Expression inference, cast conversion planning and slot conversion use this
same rule. Borrowed operands are not unconditionally converted to owned ones.

The maintained integration `tests/generic_interface_construction.rs` compiles
and executes untouched translator output, with non-Clone Rust payload probes
for construction, empty interfaces and unrelated generic parameters. The Java
oracle passes with the expanded fixture, including nested/conditional returns,
assignment, constructor arguments and generic forwarding. v385/v386 are
intermediate failing runs: v385 exposed the cast ownership error and an incorrect
borrow in the Rust probe; v386's only remaining error was the probe asking for a
copying collection getter on a non-Clone payload. The latter probe now tests
construction alone, which is the intended negative requirement. The combined
v387 verification passes all 1,439 library tests and all five stream integrations,
then fails the expanded fixture on a reassigned borrowed interface parameter.
The method signature planner now gives that reference binding owned boxed
storage, preserving the preceding in-place-mutation loan cases. v388 passes
the new fixture, both retained-callback tests and all six source-callback tests
(`.tmp/interface-capabilities-v388-regression.log`, session 89201, exit 0).

The same conversion query now propagates `JavaMapKey` requirements from method
bodies, not just stored fields. `tests/fixtures/interface_method_capability.java`
isolates this: a fieldless implementor uses generic Java `.equals`, so its
generated trait implementation requires a key capability that simple field
inspection cannot find. Frozen v58 fails this probe with missing `Clone` and
`JavaMapKey` at the interface conversion
(`.tmp/interface-method-capability-v58-compile.log`). Its maintained Java oracle
`InterfaceMethodCapabilityOracle.java` passes. The selected-call reachability
planner now carries return context and shares the capability-qualified recursion
guard with implementation queries; it does not cache provisional negative
answers across cycles.

The final combined v390 verification passes all 1,439 library tests (128.65 s),
all five stream integrations including production assembly (46.24 s), both new
interface conversion runtime probes (22.74 s), both retained-callback tests
(20.51 s), and all six source-callback tests (20.70 s).
Evidence: `.tmp/interface-capabilities-v390-regression.log`, session 94972,
exit 0. The two expanded Java oracles also pass; formatting and `git diff --check`
pass. None of these changes are included in the frozen v58 corpus counts.
The next measurement must use a newly frozen executable; the production slice
is still unverified on this batch, and the overall goal remains incomplete.

### Frozen v59 measurement and storage-context follow-up

Frozen `.tmp/new-java2rust-v59-interface-capabilities` has SHA-256
`6e41dbe5b80b579f6652708bfc4fe881186636d79f65467ab8d4dd8bbcdbf8b9`.
It was copied without overwriting an existing destination, compared against the
completed release build, and run with `JOBS=10` against the unchanged pinned
checkout. Full output is `.tmp/jgrapht-core-1.5.3-focused-v59`; session 35129
exited 1 at the generated-crate gate. No other compiler/translator probes ran
during the measurement.

All 104 files translate without translator failures. Rust reports 2,090 typed
plus 36 explicit untyped errors: 2,126 actual errors, 93 fewer than v58.
Invalid patterns remain 53 across 29 files. Graph behavior is still unverified.
Recorded milliseconds: translation 220,475 (v58: 220,728), assembly 60,704,
Rust check 11,137, total 332,997 (v58: 333,833). There is no material measured
runtime regression from the capability batch and no claimed stable speedup.

The full slice confirms that conversions now propagate requirements, but also
reveals an overconstraint in default-interface declarations. Their where-clause
planner used a context-free Clone cache, unlike concrete implementation emission:
an interface handle field could consequently impose payload Clone even though
the stored Box itself is unconditionally cloneable. The neutral
`default_interface_handle_storage.java` fixture reproduces this extra bound
and six E0310 lifetime errors on v59
(`.tmp/default-interface-handle-storage-v59-compile.log`). The follow-up passes
the existing solved storage/signature context to the same requirement query
and adds default methods' actual reified generic Any obligations using existing
selected-call capability reachability. Its new regression invokes concrete and
dynamic default dispatch with a non-Clone payload. v391 verification passes
all 1,439 library tests (129.17 s), all three interface-conversion runtime probes
(22.76 s) and all five existing default-inheritance integrations (22.30 s).
Evidence: `.tmp/interface-capabilities-v391-regression.log`, session 68149,
exit 0. The maintained default-interface Java oracle also passes. None of that
follow-up is included in the frozen v59 count.

After the timed run completed, two further neutral probes were translated
without modifying their generated Rust. `stream_set_carriers.java` confirms
unnecessary Rust Eq/Hash requirements when generic set collectors materialize
HashSet before converting to the solved JavaSemanticHashSet. Its inline-throw
block callback also exposes an existing stream-plan fallback gap; that is not
evidence that the fallible stream case is fixed. `map_set_carriers.java` confirms
missing Clone requirements in map-key/value snapshots and a hard-coded HashSet
collector on key-set return/construction. Logs are
`.tmp/stream-set-carriers-v59-compile.log` and
`.tmp/map-set-carriers-v59-compile.log`. These are pending next-batch probes,
not passing gates. Backend repair must reuse solved collection carrier and
element-copy plans; it must not add Rust Eq/Hash to Java generic payloads.
Both new Java probe sources compile, and `StreamSetCarriersOracle` passes
(`.tmp/collection-carriers-java-v391`). Direct map-view aliasing remains a
separate correctness obligation and cannot
be claimed fixed merely by changing its collection backend.

### Collection materialization carrier/copy batch (after frozen v59)

Stream slot lowering now passes the solved materialization destination to the
outer stream renderer. Set terminal emission and its produced-shape metadata
select the same existing Java-equality carrier when the solved set view requires
it. The hint is an explicit renderer argument, not callback context state that
could leak into a nested pipeline. Borrowed element recovery continues through
the existing slot adapter. Both ordinary and fallible collectors share the
existing set-carrier selector rather than imposing Rust Eq/Hash on Java generic
payloads before a later conversion.

Native map key/value set materialization now shares a typed target projection.
It renders the owned, non-optional payload carrier and then uses normal slot
conversion for nullable returns or borrowed arguments. Explicit map-view copy
constructors use that same projection when their kind matches the destination;
differently ordered constructors remain on the existing general conversion
path, not a newly added shortcut. This is not a claim that all native collection
hierarchy/upcast ordering behavior is fixed. Copy-capability inference uses the native map operation
selected by the shared planner and the copied key or value type. It does not
impose value Clone just to copy keys, nor key Clone merely to count a view.
Parsed source methods are excluded from native view recognition.

`tests/collection_materialization.rs` executes both neutral fixtures with a
payload implementing only Clone and JavaMapKey, deliberately lacking Rust
Eq/Hash. It checks plain/filtered/mapped/fallible collectors, failure propagation
and empty-input laziness, duplicate elimination, independent copy constructors,
non-Clone map values, non-Clone keys in a count-only path and source `keySet`
name precedence. The expanded map fixture also covers optional returns,
materialization into a borrowed argument and generic forwarding. The Java
oracles pass under `.tmp/collection-carriers-java-v394` (rerun after fixture
extensions before treating that directory as final evidence).

The inline-throw callback from the initial v59 probe is preserved separately in
`tests/fixtures/stream_inline_throw.java`; its unsupported statement-prefix path
is not fixed by this batch. The fallible collector regression instead uses a
source helper call, isolating the supported Result-callback collection rule.
Direct map-view aliasing is also still open: the copy-constructor checks prove
snapshot behavior, not that a returned keySet stays attached to its map.

v393 passes both initial runtime tests (21.14 s). v394 passes all 1,439 library
tests (129.23 s), the bulk operand runtime test, both collection runtime tests,
all five stream integrations, all three interface-conversion integrations and
all six source-callback integrations (`.tmp/collection-carriers-v394-regression.log`,
session 96944, exit 0). A stronger forwarding probe then showed an older
argument-name approximation redundantly inferring V: Clone when forwarding a
Map<K,V> to a key-copying generic method. The immutable debug producer
`.tmp/new-java2rust-debug-v394-carriers` emits that unnecessary bound in
`.tmp/map-set-carriers-v394-forwarded.rs`. Callable-owned bounds now rely on the
existing resolver-selected binder projection instead of that second policy.
v395 passes all 1,439 library tests (130.89 s) and both expanded collection runtime
tests (21.01 s), including optional returns, borrowed arguments and forwarding
with a non-Clone value. The targeted 23-test legacy run passes the 17 established
collection checks but fails six map-view checks. Five still assert a Result ABI
that frozen v59 already does not emit; the captured baseline programs are
`.tmp/legacy-v59-<test-name>.rs`. Two legacy fixtures also returned Collection
directly as Set/HashSet, which is not valid Java. Those snippets now explicitly
construct a set. The original valid LinkedHashSet-to-HashSet return case remains,
and a separate explicitly ordered return case was added. All six map checks were
strengthened from compile-only validation to execution; pure methods retain
their pure ABI rather than adding exceptions just to match old assertions.

v396 passes 22 of those 23 legacy tests. Reaching the remaining entry-set
runtime check exposes a real older bug: `Clone::clone(&key)` on a map iterator's
`key: &K` clones the reference rather than K. Set/vector entry snapshots, erased
entry arrays and native map-carrier copying now reuse a single qualified payload
clone projection. A new unit covers all three entry snapshot surfaces. The new
`map_entry_arrays.java` runtime probe clears the map before reading an erased
owned pair, with a passing Java oracle. This does not solve general map-entry
backing-node alias semantics.

The v397 full library run passes 1,439 tests and fails only the old entry-array
rendering assertion, which expected the incorrect reference clone. That
assertion now requires the corrected qualified payload clone. The v398 rerun
passes all 1,440 library tests (156.32 s) and all three materialization runtime
tests (21.42 s), including the new erased entry-array lifetime probe.
All five stream integrations also pass (47.08 s), including production assembly;
`.tmp/collection-carriers-v398-regression.log`, session 11918, exits 0. The separate
v398 legacy run passes all 23 selected tests (78.08 s;
`.tmp/collection-carriers-v398-legacy.log`, session 14477, exit 0). The original
valid LinkedHashSet-to-HashSet case remains alongside the new ordered case;
only the invalid Collection-to-Set snippets were changed to explicit copies.
The legacy run overlapped the library checks, so those durations are not clean
translator performance measurements. All three Java oracles pass against the
final fixtures under `.tmp/collection-carriers-java-v394`. These edits were
subsequently measured in frozen v60 below.

### Frozen v60 collection-materialization measurement

Frozen `.tmp/new-java2rust-v60-collection-materialization` has SHA-256
`cbb526950c0ee5c4381565fa6baa21f06125c1c031bf0b2033aa409534f335df`.
The default ten-worker runner used the unchanged pinned checkout and emitted
the same 104-file closure into `.tmp/jgrapht-core-1.5.3-focused-v60`.
Session 92070 exits 1 because the emitted Rust does not compile, not because
translation failed. Evidence is `.tmp/jgrapht-v60-run.log`.

There are 2,106 actual Rust errors (2,070 typed and 36 explicit untyped,
excluding Cargo's final failure summary), 20 fewer than v59. The 53 invalid
patterns across 29 files are unchanged. All 104 files parse and translate;
translated graph behavior remains unverified. Total time is 346.994 seconds,
versus 332.997 seconds for v59: this batch does not establish a speedup.
Translation takes 232.003 seconds, emission 160.847 seconds within that phase,
assembly 62.018 seconds, and Rust checking 11.384 seconds. No competing compiler
or translator probes ran during this timed measurement.

### Functional carrier ownership follow-up (after frozen v60)

The neutral `functional_carrier_arity.java` probe found unnecessary Clone
requirements on generic, single-use BiFunction arguments. Native SAM invocation
had its own unconditional local-argument cloning helper, bypassing ordinary
slot ownership/liveness decisions. That helper is removed in favor of the
existing shared slot conversion. The repeated-input variant then exposed lost
borrowed-source evidence: qualified Clone received `&first` when `first` was
already `&A`, cloning a reference rather than the payload. The common shape
conversion now preserves that evidence for plain bindings with unchanged
view/dispatch clone recovery; already-produced conversions remain authoritative.
In particular, a stream-produced `*value` must still be borrowed by qualified
Clone rather than being mislabeled as the original borrowed binding.

Functional facade generic metadata also now uses the same arbitrary-arity
opaque-carrier builder as its emitted declaration. Earlier metadata only
recognized arities one and two, omitting BiFunction's three binders. Inferred
arity rescued the declarations in this particular probe; its demonstrated
compiler failure was ownership, not an erased field declaration.

`functional_carrier_arguments.rs` executes unchanged emitted Rust with
non-Clone single-use inputs/results and repeated Rc-backed mutable inputs.
The matching `FunctionalCarrierArityOracle.java` passes with assertions enabled
under `.tmp/functional-carrier-java-v400`. The focused Rust integration passes
in `.tmp/functional-carrier-v400-test.log` (session 26120, exit 0; 21.38 seconds
after building). The v401 broad run passes all 1,442 library tests but catches
the stream dereference regression in the materialization integration (two other
materialization checks pass). The binding-only correction adds a negative
dereference assertion to the shared recovery test; follow-up verification is in
progress. Evidence is `.tmp/functional-carrier-v401-regression.log`, session
73576, exit 101. Later integrations did not run after that failure.

The corrected v402 run passes all 20 selected integrations: materialization 3,
streams 5 (including production assembly), functional arguments 1, generic
interfaces 3, retained callbacks 2, and source callbacks 6. Evidence:
`.tmp/functional-carrier-v402-integration.log`, session 70881, exit 0. The focused
shared ownership unit also passes, including its dereferenced-expression
negative assertion (`.tmp/functional-carrier-v402-unit.log`, session 14849,
exit 0). The full library's 1,442-test pass preceded that final binding-only
narrowing; it is not claimed as a second complete library run after the change.

## Optimization phase restart after functional ownership

The combined goal first targets practical feedback-loop performance, then
returns directly to the unchanged JGraphT correctness gates. Frozen baseline
`.tmp/new-java2rust-v61-perf-baseline` has SHA-256
`817df2bfdb5fc5f0a31e044846d9db35c18e0c271b8dfc286a46984afdbeca43`.
Its initial release build failed with ENOSPC; a fresh disk check showed free
space, and the terminally failed build was retried successfully (91 seconds;
`.tmp/jgrapht-perf-v61-baseline-build-retry.log`, session 48607, exit 0).
No source or evidence cleanup was performed.

The baseline sampling run retains all 104 primary sources but emits only
`BoyerMyrvoldPlanarityInspector.java`, with one worker and
`NEW_JAVA2RUST_PROFILE_LOWERING=1`. It completes successfully: shared analysis
52.768 seconds and file emission 79.311 seconds. These instrumented timings
are profiling evidence, not an uninstrumented before/after comparison.
Artifacts: `.tmp/jgrapht-boyer-v61-profile`,
`.tmp/jgrapht-boyer-v61-{profile.log,phases.tsv,workers.tsv,perf.data}`;
session 69828 exits 0. Perf reports no lost samples; restricted kernel symbols
do not prevent inspecting the user-space samples.

The combined analysis/emission profile is dominated by name matching and
allocation: SipHash writing 10.14% self time, memcmp 9.12%, the type-name-pair
cache lookup 8.24%, snake-case conversion 7.70%, and named-type rendering 6.23%.
Repeated capability/hierarchy scans still perform a name comparison for every
fact on each query, despite memoizing individual comparisons. This supports
replacing the repeated scan with a per-immutable-lowering alias-to-row index,
while retaining original row order and the existing precedence predicates.
Direct (non-leaf) alias computation is also pure but was not memoized like its
broader alias counterpart; that is part of the same pending optimization batch.

The frozen v61 full-slice baseline completes in 337.965 seconds, without
competing compiler/translator probes: translation 222.707 seconds (shared
analysis 53.015 and emission 154.039), assembly 62.625, resolver preflight
37.393, and generated Rust checking 11.642. All 104 files parse/translate;
there are 2,098 actual errors (2,062 typed + 36 explicit untyped, excluding the
Cargo summary), and still 53 invalid patterns. Evidence:
`.tmp/jgrapht-core-1.5.3-focused-v61-perf-baseline`,
`.tmp/jgrapht-perf-v61-baseline-run.log`, session 47854, exit 1 because generated
Rust does not compile. The eight-error reduction against v60 precedes the
optimization and belongs to the functional ownership follow-up, not speed work.

The candidate indexes capability/hierarchy alias-to-row membership for an
immutable lowering session, using the same alias expansion as existing pairwise
matching. Candidate rows are deduplicated by row index and sorted back into
original order before applying unchanged name/precedence predicates. Nested
sessions restore on both success and unwind; unrelated FactSets fall back to
scans, and mutable planning outside a session remains unindexed. Direct-name
alias memoization preserves its narrower owner matching, excluding broad
leaf aliases. Source-identity/object-safety matchers retain their existing
distinct rules rather than being silently routed through a different matcher.

All ten focused fact tests pass, including four new index/scope/direct-alias
regressions (`.tmp/type-fact-index-v403-focused.log`, session 61928, exit 0).
The v404 regression run passes all 1,446 library tests and all 20 selected
integrations (materialization 3, streams 5 including production assembly,
functional arguments 1, generic interfaces 3, retained callbacks 2, source
callbacks 6). Evidence: `.tmp/type-fact-index-v404-regression.log`, session
82970, exit 0. The library phase overlaps a release build, so its 78.94 seconds
is not a clean translator performance measurement. Formatting and diff checks
also pass.

Frozen candidate `.tmp/new-java2rust-v62-type-fact-index` has SHA-256
`2e7f771d537c88df0f9a58b91563071cd8ee173c321a852b2374d306f3ac83ea`;
the release build completes in 119 seconds (`.tmp/type-fact-index-v62-release.log`,
session 76009, exit 0). The full-slice candidate run starts only after those
checks terminate, with the same ten-worker mode and no competing compiler or
translator probes. No optimization speedup or output-equivalence claim is made
until that run is verified against frozen v61.

Assembly profiling is prepared separately at `.tmp/jgrapht-assembly-v61-profile`,
a copy of v61 whose status Rust paths were mechanically redirected into the copy.
The scratch Python wrapper `.tmp/perf-assembly-python-v61/python3` profiles only
the merger's main `--generated-dir` invocation, leaving other Python invocations
unchanged. The original measured v61 artifacts must remain untouched.

### Frozen v62 type-fact index measurement

The same full-slice ten-worker gate completes in 303.689 seconds versus v61's
337.965 seconds (10.1% less total time). Translation falls from 222.707 to
190.515 seconds; shared analysis from 53.015 to 45.433; emission from 154.039
to 128.486 (16.6% less). Assembly is 61.177 versus 62.625 seconds and was not
changed by this batch. Resolver preflight is 37.664 and Rust checking 11.095
seconds. These runs had no competing compiler/translator probes. Evidence:
`.tmp/jgrapht-core-1.5.3-focused-v62-type-fact-index`,
`.tmp/jgrapht-v62-type-fact-index-run.log`, session 36448, exit 1 solely because
the emitted Rust still fails to compile.

Both the 104 raw generated modules and the entire assembled `generated-crate/src`
tree are byte-identical to frozen v61. Recursive comparisons have empty output:
`.tmp/jgrapht-v61-v62-{raw,assembled}-diff.log`. All 104 files translate, the same
2,098 actual Rust errors remain, and there are still 53 invalid patterns across
29 files. This is a behavior-preserving performance improvement, not a reduction
in translator correctness debt. Graph behavior remains unverified.

This first optimization is useful but not yet the phase-transition decision:
assembly still costs about a minute. The isolated baseline assembly replay
completes without competing compiler/translator work and reproduces byte-identical
assembled source (`.tmp/jgrapht-assembly-v61-replay-diff.log`, empty). Session
33906 exits 1 for the same generated-Rust failures. The instrumented replay takes
104.629 seconds total, with 90.147 seconds assembly; this is profiler overhead,
not a regression in the uninstrumented assembler.

The merger's profile records 77.8 million calls over 57.430 instrumented seconds.
Largest exclusive costs include regex findall (8.294 seconds over 465,033 calls),
regex match (5.333 seconds over 13.7 million calls), and splitlines (2.554 seconds
over 97,514 calls). `parse_items` runs 312 times (9.656 seconds cumulative), while
nominal-reference extraction runs 10,633 times (5.161 seconds cumulative).
Artifacts: `.tmp/jgrapht-assembly-v61-merger.prof`,
`.tmp/jgrapht-assembly-v61-merger-report.txt`, and
`.tmp/jgrapht-assembly-v61-profile-run.log`.

The next bounded batch should reuse pure lexical summaries for repeated support
item text, then evaluate whether immutable input catalogs can eliminate repeated
item parsing. Any cache must be keyed by immutable content, not a mutable path;
candidate-name intersection and source-owned item boundaries must retain their
current semantics. No assembly optimization has been implemented or measured
yet, and the correctness goal remains unchanged.

### Immutable lexical assembly batch (after frozen v62)

The merger now shares immutable identifier sets and method/trait-method/constant
summaries for identical text. Candidate-name intersection remains outside the
cache; no matching policy changes, mutable sets, path identities, or mutable
file snapshots are cached by these new helpers. Returned member inventories are
tuples of immutable tuples. Identifier extraction deliberately retains the old
regex's conservative handling of comments and string contents.

The new neutral boundary test checks repeated text, changed text, changing
candidate sets, mutable-result isolation, const methods versus constants, and
trait method order. All 47 selected shared-support tests pass
(`.tmp/assembly-lexical-v405-regression.log`, session 62632, exit 0). The broader
v406 run passes all 87 script tests and all five stream integrations, including
production crate assembly (`.tmp/assembly-lexical-v406-regression.log`, session
1631, exit 0). Rust translator code is unchanged from frozen v62.

Frozen assembly scripts live in `.tmp/assembly-scripts-v62-baseline` and
`.tmp/assembly-scripts-v63-candidate`. Their merger SHA-256 hashes are respectively
`c6af180a412d7890003b05c0b48c36e58eeccd9d67937e8c7ae0ffbfce340816`
and `1b6fc7cf676b54760f3fac3dad35ff1a22b0eb3c7b4445023d136e79b9cf3b0a`.
The Bash driver and rust_item_utils are unchanged and copied alongside them.
An uninstrumented A/B assembly replay is being measured on identical raw
v62 inputs, in separate `.tmp/jgrapht-assembly-v62-control` and
`.tmp/jgrapht-assembly-v63-lexical` directories. Every status Rust path has been
validated to point inside its own copy. Python wrappers select the frozen old
or new merger for every invocation, while preserving the same Python interpreter.
The uninstrumented sequential replay completes: frozen old-merger assembly takes
63.930 seconds; the immutable-summary candidate takes 55.556 seconds (13.1% less).
Replay totals, including Rust checking and scans, are 78.522 and 70.178 seconds.
Evidence: `.tmp/jgrapht-assembly-v62-control-run.log` (session 79323) and
`.tmp/jgrapht-assembly-v63-lexical-run.log` (session 96357). Both exit 1 solely for
the unchanged generated-Rust failures. `/usr/bin/time -v` reports whole-command
maximum RSS of 685,912 and 685,276 KiB respectively; these figures are not a
measurement of the merger alone. No other compiler/translator probes overlapped
the paired runs.

The control replay matches the original v62 assembled source, and candidate
versus control is also byte-identical. Empty recursive comparisons are recorded
in `.tmp/jgrapht-assembly-v62-control-diff.log` and
`.tmp/jgrapht-assembly-v62-v63-diff.log`. The candidate still has 2,098 actual
Rust errors and 53 invalid patterns; the optimization has not repaired or hidden
correctness debt.

The full combined v63 gate now runs with the frozen v62 CLI and the frozen v63
assembly wrapper, after all replay processes terminate. Its output is
`.tmp/jgrapht-core-1.5.3-focused-v63-lexical`; log
`.tmp/jgrapht-v63-lexical-run.log`. This will establish an actual end-to-end
measurement; component savings must not be added and presented as a measured
combined result.

Pending that final gate, the bounded optimization phase has its intended two
improvements: dominant emission is 16.6% faster in the v61/v62 full comparison,
and assembly is 13.1% faster in the controlled replay. Remaining parser work is
spread across item catalog construction, mutable-file cleanup, source ownership,
and call-shape reconciliation. Sharing those catalogs across rewrites requires
more semantic/invalidation redesign than the remaining few-second opportunities
justify while emitted Rust still does not compile. After verifying the combined
gate, return directly to correctness work rather than extending this into an
open-ended assembler rewrite. Named source objects implementing native SAM
interfaces remain a promising related next correctness batch.

### Combined v63 verification and return to correctness

The final combined gate completes in 306.573 seconds, versus the pre-optimization
v61 baseline's 337.965 seconds (9.3% less). The emitted raw and assembled source
trees are byte-identical to v61; recursive diffs are empty in
`.tmp/jgrapht-v61-v63-{raw,assembled}-diff.log`. All 104 files translate with zero
translator failures. The same 2,098 actual Rust errors (2,062 typed + 36 explicit
untyped) and 53 invalid patterns across 29 files remain. The run exits 1 for
generated-Rust failure (session 15353), not a translator failure.

| Wall-clock phase | v61 baseline | v62 indexed lowering | v63 combined |
| --- | ---: | ---: | ---: |
| Translation | 222.707 s | 190.515 s | 195.910 s |
| Emission, within translation | 154.039 s | 128.486 s | 132.997 s |
| Assembly | 62.625 s | 61.177 s | 54.780 s |
| Total | 337.965 s | 303.689 s | 306.573 s |

The combined total is slightly slower than the intermediate v62 measurement;
translation and compiler timing variation offset the isolated assembly saving.
Do not claim a combined 12–13% end-to-end improvement by adding component gains.
The paired assembly replay independently established its 13.1% phase reduction.
No competing compiler/translator probes ran during the final combined gate.

The bounded optimization phase is now finished. Both measured bottlenecks have
general improvements, neutral regression coverage, and full-slice unchanged-output
evidence. Further large gains would require broader incremental-analysis or
mutable item-catalog redesign; small residual scans are not worth delaying the
still-failing correctness gate. The combined goal remains active and work returns
directly to the original JGraphT translation objective, without expanding scope.

### Named native-functional object probe (after optimization)

The new neutral `native_functional_objects.java` covers named suppliers and their
shared mutable state, inherited SAM implementations, comparators, BiFunction
string arguments/results, null conversions, and catchable callback exceptions.
The Java oracle passes under `.tmp/native-functional-objects-java-v407`.
Frozen v62 translation is captured unchanged in
`.tmp/native-functional-objects-v62.rs`; Rust checking fails with nine errors
(`.tmp/native-functional-objects-v62-compile.log`, session 14348, exit 1).
No emitted output was manually repaired.

The root cause is broader than adapter construction. IntSupplier values are
planned as `Box<dyn Any>`; comparator factory returns can be specialized to a
concrete source class while invocation still expects the native `compare_result`
ABI; BiFunction retains its nominal facade but receives an invalid boxed `as`
cast. The inherited counter also produces an unsupported-inheritance diagnostic.
The null-source case discharges an Option and turns its null test into false.
Fix the shared native-functional carrier/ABI contract before adding conversion
adapters; use typed selected source calls to preserve borrowing and exception
contracts, not guessed method names or blanket Result unwraps.

The maintained executable gate is `tests/native_functional_objects.rs`. Its
initial v407 run fails as expected when compiling the generated crate
(`.tmp/native-functional-objects-v407-test.log`, session 89976, exit 101;
12.17 seconds). Unsupported diagnostics do not trigger the test's has_errors
guard; the independent frozen-output Rust check above reproduces the compiler
failures too. These are reproduced existing
failures, not passing coverage or new accepted baseline exclusions. JGraphT's
named comparator-to-facade conversion exercises the same missing capability.

The next implementation batch should establish the modeled native-functional
carrier across representation and signature planning, replace comparator-only
concrete-return/storage specialization with that common contract, and adapt a
source object through a typed selected SAM call. Reuse ordinary callback,
argument, return, capture/storage, and exception lowering. Evaluate the source
object once, preserve nullable carrier conversions without eager method-reference
null checks, and preserve source aliases through shared handles. Do not fix only
the final cast or add blanket cloning/Result unwraps. Source-defined SAM
lookalikes and explicit user representation facts retain precedence.

### Native-functional source-object implementation and regression evidence

Native functional values now use the shared modeled SAM carrier contract in
representation, signature, and slot lowering. Parsed source lookalikes and
explicit user representation overrides keep precedence. Comparator-only concrete
field/return specialization was removed: an interface value has the same facade
ABI whether it comes from a lambda, a source constructor, a field, or a factory.
Source-object adapters select the source SAM method through existing typed method
reference resolution and reuse ordinary fallible callback lowering. They evaluate
the source once, preserve nullable values without eager null checks, and reuse
existing assignment/storage and liveness adaptation rather than inventing a
separate capture policy.

Two minimized runtime failures established additional required general fixes.
An isolated mutable supplier converted into two interface values returned 333
instead of Java's 345: local interface conversions had not contributed source
aliasability. Native SAM value uses now participate in the existing shared-storage
analysis, including generic source-interface ancestry. Correctly sharing generic
immutable implementations exposed a missing `From<Rc<Source<T>>>` (and Arc)
trait-object conversion; those conversions now reuse the direct forwarding
implementation's capability bounds, without blanket payload cloning.

A callback invoked through a source-typed holder field also bypassed the Java
catch in its caller. Call-graph receiver inference now reuses the resolver's
source-field/hierarchy/generic lookup before recording the canonical checked
native SAM entry. The oracle's cross-method exception test now returns 9 instead
of panicking. Constructor-only, concrete source leaf inheritance supported by
existing flattening is recorded as represented rather than reported unsupported.

The maintained fixtures are `tests/fixtures/native_functional_objects.java` and
`native_functional_alias.java`, with Java assertions in
`tests/corpus/callback/NativeFunctionalObjectsOracle.java`. The current Java
oracle passes under `.tmp/native-functional-objects-java-v414`. Focused v415
passes all 13 integrations: native functional objects (2), generic interface
construction (3), retained callback state (2), and source callback contracts (6),
in `.tmp/native-functional-objects-v415-test.log`. This includes the isolated
345 alias test, inherited suppliers, nullable source conversion, fields and
replacement, borrowed string arguments/results, and cross-method exceptions.
The current v416 gate passes all 1,449 library tests and all 22 selected
integrations: collection materialization (3), fallible stream matching (5,
including production crate assembly), functional carrier arguments (1), generic
interface construction (3), native functional objects (2), retained callbacks
(2), and source callback contracts (6). The unfiltered command exits 0, with
results in `.tmp/native-functional-objects-v416-regression.log`. The new
field-receiver call-graph unit passes in that run; the current Java oracle also
passes in `.tmp/native-functional-objects-java-v416`. Formatting and diff
whitespace checks pass. No fresh full JGraphT count is claimed by these focused
checks.

Remaining boundaries are explicit: source-object arguments to nonretaining
inline callback formals and catchable exceptions from PriorityQueue comparators
still need work. Independent facade adapters have verified shared mutable state,
not a complete Java object-identity contract. The five previously failing
PriorityQueue integration cases remain open; several also encode obsolete
concrete comparator layouts and need behavior-preserving test modernization.
No emitted Rust or pinned Java was patched.

The next full 104-file gate uses frozen release
`.tmp/new-java2rust-v64-native-functional-objects`, SHA-256
`bf15c3fb819ea3b49d900ca1bfcd7b3bd684433a00774270d563ba476cfdca1f`.
Its output root is
`.tmp/jgrapht-core-1.5.3-focused-v64-native-functional-objects`, with runner log
`.tmp/jgrapht-v64-native-functional-objects-run.log`. The gate exits 1 for
generated-Rust failure (session 80555). All 104 files translate, with zero parse
failures; actual Rust errors increase from v63's 2,098 to 2,148 (2,112 typed +
36 explicit untyped, excluding Cargo's summary). Invalid patterns increase from
53 to 54 across the same 29 files. The new carrier model is therefore not a
corpus-wide compilation improvement yet, despite the neutral behavior gates.
Do not attribute all changed errors to newly exposed debt without checking them.

Total time is 309.557 seconds: resolver preflight 37.277, translation 201.251,
shared analysis within translation 43.518, emission 141.001, assembly 55.277,
and Rust checking 12.354 seconds. This correctness revision changes output, so
it is not another unchanged-output optimization comparison. No competing
compiler or translator probes ran during the full measurement.

The additional invalid-pattern occurrence is localized to `DirectedSpecifics`:
the newly generated shared-receiver companion duplicates its existing
untranslated `Collection.addAll` path. This is not a newly identified Java idiom,
but the companion must still be fixed; it is not a count waiver. The remaining
typed-error changes require further attribution rather than a blanket debt claim.

One confirmed next shared-carrier issue already present in both v63 and v64 is
13 errors asking for `From<Rc<RefCell<Rc<RefCell<AsSubgraph<V,E>>>>>>` at source
interface conversions. The source constructor actually returns its canonical
shared allocation (needed by an escaping inner capture), but conversion treats
every constructor call as a bare payload and adds another handle. The neutral
`shared_constructor_abi.java` fixture and Java oracle isolate this distinction;
its Rust baseline gate fails as expected in
`.tmp/shared-constructor-abi-v417-baseline.log` (session 36980, exit 101).
The initial fixture also exposed a source-SAM closure adapter using `&self`
despite a solved mutable interface receiver, and a separate constructor seed
requiring Default for a not-yet-initialized inner-reference field. The maintained
minimized fixture allocates the inner object without storing it, isolating the
canonical-constructor handle contract from that still-open initialization issue.

The next batch consults the existing constructor self-escape fact at source
conversion boundaries before adding storage, including the dedicated fresh-new
boxed-interface path as well as ordinary, wildcard and covariant adapters. It
also renders source zero-argument closure adapters with the solved interface
receiver (`FnMut`/`&mut self` versus `Fn`/`&self`) instead of hard-coding a
shared receiver. The strengthened regression checks factory returns, nullable
assignment, state mutation, and an actual mutable Rust closure through the
generated source-interface impl. The Java oracle passes in
`.tmp/shared-constructor-abi-java-v417`.

Intermediate v418 passes generic interface construction (3), but still fails
the constructor probe: the dedicated fresh-new conversion path was adding a
handle unconditionally, bypassing the other boundary checks. That path now uses
the same payload predicate. Focused v419 passes all 12 integrations: generic
interface construction (3), native functional objects (2), shared constructor
ABI (1, executing both Java behavior cases and the mutable closure test), and
source callback contracts (6). Results are in
`.tmp/shared-constructor-abi-v419-test.log` (session 74027, exit 0). No generated
output was edited. The broader gate passes all 1,449 library tests and all
23 selected integrations in `.tmp/shared-constructor-abi-v420-regression.log`
(session 82662, exit 0). Formatting and whitespace checks also pass. No new
full-slice improvement is claimed at this checkpoint.

### Stored native values at invoking-only source callback boundaries

The neutral `inline_native_values.java` probe demonstrates the next missing
boundary: a stored native facade or named source implementor is passed directly
as `&mut dyn FnMut`, although a facade is not a Rust closure. The current Java
oracle passes under `.tmp/inline-native-values-java-v421`; the untouched generated
Rust fails in `.tmp/inline-native-values-v421-baseline.log` (session 26530,
exit 101). Both zero-input suppliers and two-input functions reproduce it.
The probe also checks shared mutable source state, catchable callback errors,
ignored null values, caught null invocation, and exactly-once argument evaluation.

The implementation reuses slot lowering to materialize the native facade once,
then an ordinary typed lambda and native SAM invocation to meet the callee's
invoking-only callable ABI. Nullability belongs to the captured value; it is not
eagerly discharged during argument adaptation. Existing inline forwarding stays
on its original borrowed-closure path, and retained source formals continue to
receive owned facades. Source-defined lookalikes and explicit user representation
facts still control whether the native facade contract applies.

Call-graph construction also connects a nonliteral native callback value's
checked SAM entry to the formal callback contract. The edge is attached to
invocation, not argument evaluation, so unused callbacks do not acquire eager
effects. This preserves Result propagation through callers instead of making the
adapter panic at a Java catch boundary. Intermediate v422 fails on one remaining
compiler error: a nullable formal's invocation returns `Result`, but the effect
lookup was checking the Option shell instead of its functional payload and
omitted `?`. That lookup now normalizes the nullable shell using the same
payload rule as ordinary native SAM invocation. Focused v423 passes all 12
selected integrations in `.tmp/inline-native-values-v423-test.log`
(session 76836, exit 0): inline native values (1), native functional objects (2),
retained callbacks (2), shared constructor ABI (1), and source callback contracts
(6). A new call-graph unit additionally checks that the checked SAM effect is
linked to the invoking formal, while an unused callback has no invocation edge.
The broader v424 gate passes all 1,450 library tests and all 24 selected
integrations in `.tmp/inline-native-values-v424-regression.log` (session 91433,
exit 0), including production stream assembly, materialization, generic source
interfaces, retained callbacks, native objects and both new boundary probes.
The new call-graph unit passes. The release build for the combined revision
finishes successfully in `.tmp/callback-boundaries-v65-release.log`
(session 47372, exit 0). Formatting and diff whitespace checks pass.

The full 104-file v65 gate completes (session 32558, exit 1 for generated Rust), using frozen binary
`.tmp/new-java2rust-v65-callback-boundaries`, SHA-256
`5be167e87938e31237ae7511ea47717cbeb22e9d3a5c8136173c98c85229b5d6`.
Its output root is `.tmp/jgrapht-core-1.5.3-focused-v65-callback-boundaries`,
with runner log `.tmp/jgrapht-v65-callback-boundaries-run.log`. The assembler
script still has the verified v63 SHA-256
`1b6fc7cf676b54760f3fac3dad35ff1a22b0eb3c7b4445023d136e79b9cf3b0a`.
No competing compiler/translator probes ran during the measurement. All 104
files translate with zero parse failures. Actual Rust errors fall from v64's
2,148 to 2,133 (2,097 typed + 36 explicit untyped, excluding Cargo's summary).
All 13 nested-handle `AsSubgraph`-to-owned-Graph errors disappear, as do two
attempts to call a stored Predicate facade as a Rust function. Three borrowed
Graph conversions now have the correct single handle but still lack a legal
borrowed interface adapter. Other large error categories remain unchanged.
Invalid patterns remain 54 across 29 files. This is still worse than v63's
2,098 errors/53 invalid patterns; the v64 increase needs continued attribution,
not a blanket claim that all remaining movement is harmless debt.

Total time is 305.987 seconds: resolver 37.876, translation 197.227, shared
analysis within translation 43.294, emission 137.250, assembly 55.121, and Rust
checking 12.358 seconds. The changed output is a correctness measurement, not
another unchanged-output optimization comparison. The generated crate still
does not compile, and translated graph behavior remains unverified.

### Next callback-boundary batch selected from v65

Eight compiler errors pass a `Supplier<E>` value from generated source-interface
forwarders to implementation methods expecting `&mut dyn FnMut() -> E`.
The declaring interface and source implementation have independently selected
different callback representations. Reuse the existing source override groups
and callback forwarding graph to give virtual slots a stable value ABI rather
than inserting comparator/supplier-specific forwarding patches. Distinguish
that ABI requirement from evidence that a method actually retains its argument.
The neutral `virtual_callback_contracts.java` probe includes both invoking and
retaining implementations of one interface, direct and dispatched calls, and
catchable callback errors. Its Java oracle passes in
`.tmp/virtual-callback-contracts-java-v426`; the Rust baseline gate is in
`.tmp/virtual-callback-contracts-v426-baseline.log` (session 74783, exit 101).
The current implementation records virtual callback value-carrier requirements
separately from retention, seeds them from the shared source override groups
and abstract/interface declarations, and propagates them backwards along the
existing callback forwarding graph. Body invocation and call-site conversion
then use the same native facade contract while nonvirtual invoking-only methods
retain the established borrowed callable ABI. Intermediate v427 passes inline
native values (1), native objects (2), retained callbacks (2), and source callback
contracts (6), but the new virtual probe still fails: an immutable implementor
has no Rc forwarding impl when the interface's receiver is mutable because a
different implementation writes state. The callback argument mismatches are
gone; the remaining two compiler errors expose that separate adapter limitation.

Immutable shared forwarding now falls back to the existing typed source-method
implementation emitter, using a shared body receiver only when the selected
implementation's solved receiver permits it. The virtual trait signature stays
unchanged; no mutable payload reference is manufactured through Rc/Arc, and
genuinely mutable implementations still require their normal cell storage.
Intermediate v428 passes generic source interfaces (3), inline native values
(1), and source callback contracts (6). Its virtual probe has only one remaining
compiler error: the new direct Rc forwarding path selected the nominal cell
adapter's `as_any` support, referring to nonexistent `self.0`. It now reuses
direct trait-object support with handle cloneability (Rc/Arc is cloneable without
a payload Clone bound), preserving the shared handle as the runtime identity.
The fixture also now exercises a nonvirtual forwarder into the virtual slot,
checking backwards propagation of the value-ABI requirement. Its Java oracle
passes in `.tmp/virtual-callback-contracts-java-v429`.

Focused v429 passes all 11 selected integrations in
`.tmp/virtual-callback-contracts-v429-test.log` (session 20621, exit 0), including
the strengthened virtual-forwarding behavior probe. Frozen v65 output for the
callback-expression arithmetic probe
is `.tmp/qualified-functional-values-v65.rs`: nested fallible arithmetic is
wrapped in `Ok`, but an outer binary expression whose numeric type was not
recovered consumes that Result as a scalar. Inspect the shared numeric operand
type/promotion and Result-boundary helpers (`numeric_operand_java_type`,
`lower_numeric_binary_expr_with_fallible_operands`, and `lower_binary_expr`)
instead of patching this expression's text or changing the Java fixture.

The arithmetic follow-up recovers native SAM numeric outputs from the shared
metadata after substituting the receiver's actual type arguments. Source/user
representation precedence guards that recovery. This lets normal promotion,
checked-value staging and Java wrapping arithmetic handle the entire expression,
without an additional recursive whole-expression inference pass or Result-text
rewrites. `FunctionalInterfaceShape::return_type` now owns the primitive/generic/
void return substitution and is also reused by ordinary callback lowering.
Focused v430 passes all 12 selected integrations in
`.tmp/callback-boundaries-v430-test.log` (session 29238, exit 0), including the
original arithmetic value 457 and the virtual behavior cases. The maintained
arithmetic probe has since been strengthened with overflowing boxed-callback
arithmetic (expected -2147483647); its Java oracle passes in
`.tmp/qualified-functional-values-java-v430`.

An additional SourceCallbackPlan unit distinguishes the virtual value ABI from
actual retention and checks propagation into a nonvirtual forwarder. The broad
v431 gate passes all 1,451 library tests and all 26 selected integrations in
`.tmp/callback-boundaries-v431-regression.log` (session 78619, exit 0).
It includes the strengthened overflow probe, all related callback, constructor,
generic-interface and collection/stream gates. Formatting and diff whitespace
checks pass. The release build completes in 1m28s in
`.tmp/virtual-callback-abi-v66-release.log` (session 10089, exit 0).

The full 104-file v66 gate completes (session 3371, exit 1 for generated Rust), using frozen binary
`.tmp/new-java2rust-v66-virtual-callback-abi`, SHA-256
`6104900c92fbc366c13546f7bd16142e12baf2994579344f289e1f548072dec1`.
Its output root is `.tmp/jgrapht-core-1.5.3-focused-v66-virtual-callback-abi`,
with runner log `.tmp/jgrapht-v66-virtual-callback-abi-run.log`. The assembler
still has the verified v63 SHA-256
`1b6fc7cf676b54760f3fac3dad35ff1a22b0eb3c7b4445023d136e79b9cf3b0a`.
No competing compiler or translator probes ran during this measurement. All 104
files translate with zero parse failures. Actual errors fall from 2,133 to 2,125
(2,089 typed + 36 explicit untyped, excluding Cargo's summary). The eight
Supplier-value-to-FnMut forwarding mismatches disappear; no other error-category
count changes beyond Cargo's summary. Invalid patterns remain 54 across 29 files.
Total time is 307.859 seconds: resolver 37.839, translation 199.397, shared
analysis within translation 43.919, emission 139.149, assembly 54.930, and Rust
checking 12.323 seconds. This is not an unchanged-output optimization comparison.
The selected slice still fails compilation and has no translated graph behavior
verification. The remaining error increase relative to v63 remains open.

A related next-batch candidate is comparator factory/composition semantics,
subject to v66's regression attribution. The selected Java closure contains
`Comparator.comparingInt` and sorting, but no Java PriorityQueue use. v65 shared
support still emits `Comparator.comparing_int` as `unimplemented!` with an erased
default return type, and `then_comparing_int` is missing. A correct implementation
must cover typed key-extractor results, retained captures and error boundaries,
not just remove those compiler messages. The previously failing PriorityQueue
tests remain open broader verification work, not accepted exclusions.

The next neutral probe is `tests/fixtures/comparator_factories.java`, covering
typed comparingInt factories, non-eager construction, reversed and chained
ordering, source-state capture and catchable key-extractor errors. Its Java
oracle passes in `.tmp/comparator-factories-java-v432`. The Rust baseline fails
in `.tmp/comparator-factories-v432-baseline.log` (session 97666, exit 101).
Generated Rust and the pinned corpus remain untouched.

The comparator runtime foundation now supplies checked `comparing_int`,
`reversed`, and `then_comparing_int` operations on the shared native facade.
Key extraction is left-to-right and lazy until comparison; chaining skips its
secondary key extractor unless the primary ties, and propagates primary/key
errors without panics. Reversal swaps operands rather than negating an arbitrary
comparison result. Only chaining needs a payload Clone bound; a key comparator
and its reverse work on non-Clone owned inputs. Facade discovery no longer adds
opaque compatibility stubs on top of those executable native methods. The new
`NativeComparatorOperation` description is the shared operation surface for
subsequent typed call planning, including the retained key-extractor SAM type.

Runtime-stage v433 passes all four selected integrations in
`.tmp/comparator-factories-v433-runtime.log` (session 71107, exit 0): the native
comparator runtime probe, native functional objects (2), and virtual callback
contracts. `tests/comparator_runtime.rs` obtains its complete runtime from
ordinary translator output and exercises the newly emitted methods directly;
it does not repair generated Rust. This proves the runtime component, not
end-to-end Java factory translation. The v432 translated-Java gate remains open.

Next connect typed factory/composition lowering to that runtime using the
existing callback contract machinery and actual destination/receiver type
arguments. In particular, preserve the Java semantics of retained bound source
method references, checked key-extractor errors, required extractor/null checks,
and method-chain return shapes. The v432 baseline shows unimplemented source
factory calls, missing method-chain receiver type recovery (a reversed compare
is incorrectly infallible), and an untyped key lambda with `?` outside a Result
closure. Reuse/generalize existing argument retention and capture/signature
analysis rather than cloning a borrowed source payload or adding corpus-name
exceptions. Add source-lookalike precedence coverage when wiring the new API.
No full-corpus improvement from this runtime-only stage is claimed.

The separate qualified-functional probe's Java oracle passes in
`.tmp/qualified-functional-values-java-v425`, but its Rust baseline fails on an
arithmetic expression containing a prematurely `Ok`-wrapped intermediate value,
not a missing qualified facade name. Parser normalization means the suspected
qualified-name mismatch is not established by that probe; do not change naming
policy based on the hypothesis alone. The maintained fixture also serves as
coverage for composing checked callback results in larger expressions. Results
are in `.tmp/qualified-functional-values-v425-baseline.log` (session 73134,
exit 101). That was the initial baseline, not the current result: the qualified
numeric callback probe subsequently passes in the v431 combined regression gate.
The comparator runtime and translated-factory follow-ups are recorded below.
The latest completed full JGraphT evidence remains v66.

### Typed comparator factories and bound receiver capture

The typed factory/composition path now uses `NativeComparatorOperation` metadata
and the actual destination or receiver element type. Comparing-int callbacks
reuse the common functional invocation contract and checked result lowering;
instance operation return types preserve their comparator element type through
chains. External argument effects record key-extractor retention without
guessing a generic source formal type. Parsed source owners and lexical bindings
take precedence over native static factory recognition.

Bound method references now have distinct capture-owner facts. Capture analysis
uses the lexical expression walker; static/unbound type owners do not become
captured locals, and a bound reference captures the current receiver value even
when its variable is later reassigned. Captured source references contribute to
shared storage planning so retaining a receiver preserves Java object identity,
not a cloned payload. Typed functional bound-reference lowering captures that
receiver once and lowers invocation through ordinary typed call/lambda machinery,
reusing storage projection, overload selection and exception propagation.

The original translated Java factory probe and all ten selected integrations
pass in `.tmp/comparator-factories-v436-test.log` (session 75211, exit 0):
factory behavior, comparator runtime, retained callback state (2), and source
callback contracts (6). In particular, mutating the source object after factory
creation affects later comparisons, and arithmetic exceptions from its key
method remain catchable. Generated Rust was not repaired.

The expanded maintained fixture additionally exercises overloaded key methods
and receiver-variable reassignment; its Java oracle passes under
`.tmp/comparator-factories-java-v437`. A new source-defined `Comparator`
lookalike gate and capture-analysis unit test accompany that expansion. Their
combined library/selected integration gate is recorded in
`.tmp/comparator-factories-v437-regression.log` (session 93309, exit 101).
All 1,452 library tests and the three collection-materialization integrations
pass, as does source-comparator precedence. The expanded factory probe exposed
a genuine overload bug: the synthetic invocation retained receiver storage
projection but lost the source resolver's selected call, emitting `key` rather
than its disambiguated Rust name. Later suites were not reached.

The bound-reference adapter now carries the existing typed source-reference
resolver's `ResolvedCall` into ordinary invocation lowering. The same follow-up
also propagates a nested bound-reference receiver into its enclosing lambda's
capture facts, covered by a dedicated unit regression. The revised combined
library/selected integration gate passes in
`.tmp/comparator-factories-v438-regression.log` (session 72989, exit 0): all
1,453 library tests and 29 integrations across comparator factories/runtime,
collection materialization, fallible streams (including production assembly),
functional-carrier arguments, generic interface construction, inline native
values, native functional objects, qualified functional values, retained
callback state, shared constructor ABI, source callback contracts and virtual
callback contracts. The expanded overload/rebinding and source-precedence
probes execute successfully in generated Rust. Formatting and diff checks pass.

This does not complete comparator support: nullable key extractors still require
creation-time validation, generic captured receiver lifetimes/capabilities need
verification, and factory typing in collection-consumer argument contexts may
need more evidence. No fresh full-corpus error reduction is claimed; v66 remains
the authoritative 104-file baseline until the next substantial-batch run.

The v67 release build passes in `.tmp/comparator-factories-v67-release.log`
(session 58119, exit 0). The byte-compared frozen binary is
`.tmp/new-java2rust-v67-comparator-factories`, SHA-256
`bd21d092da2b23c8e98da8808c7170c89384e473b3260ab71853f89e00c8363b`.
The full pinned 104-file gate completes (session 35067, exit 1 for generated
Rust), with output root
`.tmp/jgrapht-core-1.5.3-focused-v67-comparator-factories` and runner log
`.tmp/jgrapht-v67-comparator-factories-run.log`. The assembler hash remains
`1b6fc7cf676b54760f3fac3dad35ff1a22b0eb3c7b4445023d136e79b9cf3b0a`.
No competing compiler or translator probes are run during this measurement;
this is a correctness-batch comparison, not unchanged-output optimization.
All 104 files parse and translate, with no translator failures. The assembled
crate still fails: 2,148 actual errors (2,112 typed and 36 explicit untyped),
excluding the final Cargo summary. This is 23 more errors than v66; the 54
invalid patterns in 29 files are unchanged. Total time is 305.876 seconds:
resolver 37.224, translation 197.775, assembly 55.266 and Rust checking 12.248.
The emission phase is 137.841 seconds. This is not accepted as a corpus
improvement merely because the focused tests pass.

Initial attribution identifies two distinct follow-ups. The generic comparator
factory in `VertexDegreeComparator.of` now captures its interface receiver as an
owned value and uses typed checked invocation, but exposes two missing
`VMethod: 'static` requirements. Separately, existing borrowed-interface
conversions now sometimes emit box-style `.as_ref()` against `&mut dyn Graph`:
that error category increases from 1 to 16, with two more against `&dyn Graph`.
The full error-category diff also includes eleven more ordinary E0308 errors
and removals/changes in other categories; complete attribution remains open.
Do not waive these as harmless ownership debt.

The chained factory in `GraphMetrics` still bypasses typed lowering: its base
bound reference has no independently inferred element type in the current
translator, while the outer assignment supplies `Comparator<V>`. Carrying that contextual type through a
proven native factory chain is a separate general typing fix, not a reason to
change qualified-name aliases. A simple generic factory already reaches the
native path in this same run. Next minimize the interface capture/reborrow,
generic lifetime, and contextual-chain cases into neutral regression probes
before the next coherent product batch. The full goal remains incomplete.

Neutral follow-up probes now reproduce the missing contracts. The initial
`tests/fixtures/comparator_capture_boundaries.java` gate fails on exactly two
generic retained-interface lifetime errors in
`.tmp/comparator-capture-v439-baseline.log` (session 6450, exit 101). Its expanded
nullable/mutable borrowed forwarding case fails on five errors in
`.tmp/comparator-capture-v441-baseline.log` (session 73764, exit 101): the same
two lifetime errors, an `Option<&mut dyn KeyLookup>` to `Option<&dyn KeyLookup>`
mismatch, an invalid clone of a mutable borrowed option, and a missing mutable
binding for an optional mutable receiver. This is related ownership-boundary
coverage, not yet a minimized reproduction of the corpus's exact `.as_ref()`
regression. Do not conflate those failures.

The maintained comparator factory fixture now also has a bound-reference chain
with an unambiguous source method. Its translated Rust fails on four errors in
`.tmp/comparator-chain-v440-baseline.log` (session 48614, exit 101), including a
raw shared-handle method call and erased `Box<dyn Any>` key/input arithmetic.
Source-comparator precedence still passes in that gate. Both expanded Java
oracles pass under `.tmp/comparator-capture-java-v439`. An initial proposed
chain using an overloaded method reference was rejected by javac; it was
replaced with the valid unambiguous reference before taking the v440 baseline.
No product changes have yet been made for this follow-up batch and these
failing Rust gates are open work, not accepted exclusions.

### Contextual comparator and interface-boundary follow-up

The next coherent product batch carries destination comparator element types
through proven native factory/composition chains, using a shared static-owner
guard so parsed source definitions and lexical bindings still win. Generic
callback lifetime requirements reuse capture-owner facts and the existing
selected-source capability reachability walk; callable identity travels with
each worklist entry, and reified `Any` obligations propagate through forwarding
calls. This is not a blanket lifetime bound on all generic methods.

Source parameter queries now reuse the emitted ABI normalization instead of
returning an unnormalized solver shape. Borrowed optional interface payload
projection agrees with signature rendering, optional mutable loans use
`as_deref`/`as_deref_mut` at the payload boundary, and nullable mutable parameter
bindings permit that reborrow. The scalar mutable-to-shared shortcut no longer
pretends Rust weakens mutability through an outer `Option`. Mutable optional
reborrow runs before the exact-carrier early return, and borrowed interface
recovery delegates to the existing object-safe clone adapter rather than a
payload `Clone` shortcut.

Capture storage planning now distinguishes interfaces from concrete objects:
captured interface references mark source implementors aliasable through the
existing interface-value mechanism. Inspection confirms abstract interfaces
were already excluded from concrete storage facts, so assigning an interface
its own shared payload is not an established explanation for v67's regression.
The new path consults the separate interface inventory before looking for
concrete storage. Capture owner context is retained for
same-package type resolution. Dedicated storage and optional-payload unit
regressions accompany this change.

Focused development results are
`.tmp/comparator-boundaries-v442-test.log` (session 30228, exit 101: generic
lifetime and mutable-binding errors removed, two optional-boundary errors
remain), and `.tmp/comparator-boundaries-v443-test.log` (session 94050, exit 101:
readonly optional reborrow fixed; expanded generic forwarding, mutable optional
reuse and a cloned borrowed optional remain). The fixture now additionally
checks a generic forwarder and a null borrowed receiver. Both expanded Java
oracles pass; Rust follow-ups remain unproven pending the next gate.

The combined library/selected integration gate in
`.tmp/comparator-boundaries-v445-regression.log` (session 47317, exit 101) passes
1,455 library tests, but the new storage unit incorrectly unwraps an interface's
nonexistent concrete storage entry. That assertion now checks the actual design
invariant (no concrete interface entry, shared implementor), rather than
expecting the interface to have a value entry. Later integrations were not
reached. A focused revised gate runs in
`.tmp/comparator-boundaries-v446-test.log` (session 21022, exit 101), additionally preserving
already-converted borrowed subtype arguments before owned recovery can clone
their optional loans. Generic forwarding now compiles, but two optional-loan
errors remain. The immediately
preceding v444 attempt failed to parse a newly inserted unit test inside its
neighbor's raw Java fixture; that test layout is corrected before v445, with no
product result claimed from v444. No new full-corpus outcome is claimed by this
batch yet; v67's 2,148 errors and 54 invalid patterns remain authoritative.

The generic argument-shape trace in
`.tmp/comparator-boundaries-v447-trace.log` (session 72011, exit 101) proves that
the nullable mutable target already has the borrowed ABI and the emitted value
is `Some(&mut ... as &mut dyn Trait)` before a later subtype pass clones it.
Constructed borrowed optional values now bypass that redundant owned recovery.
Removing the statement-only future-read prerequisite also exposes the real
mutable trait-object reborrow requirement: plain `as_deref_mut()` retains an
invariant trait-object lifetime, so a typed coercion inside the payload mapping
must establish a shorter call loan. The argument adapter now supplies it.
The temporary generic trace instrumentation is removed.

Focused v448 passes all seven integrations in
`.tmp/comparator-boundaries-v448-test.log` (session 62433, exit 0): generic
interface capture/forwarding/borrow/null behavior, both comparator-factory
gates (including the contextual bound chain), generic interface construction
(3), and shared constructor ABI. The generated code runs the maintained Java
contracts without edits to Java or emitted Rust. Both expanded Java oracles,
formatting and diff checks pass.

The complete selected library/callback/stream regression gate passes in
`.tmp/comparator-boundaries-v449-regression.log` (session 72094, exit 0): all
1,456 library tests and 30 integrations. This includes production stream crate
assembly, all comparator/capture probes and the established callback, collection,
generic interface, and constructor ABI gates. Formatting and diff checks pass.

Grouping the v66-to-v67 compiler errors by their first diagnostic source
location accounts for the net +23 in six files: BergeGraphInspector +19,
GraphTests +2, FloydWarshallShortestPaths +1, ChordalityInspector +1,
VertexDegreeComparator +1, and GraphMetrics -1. Other files' counts are
unchanged. Error counts are not independent root-cause counts; this localizes
the regression for the next full-batch comparison rather than waiving it.

The v68 release build followed by frozen full-corpus comparison is started
(session 3312).
The planned immutable binary is `.tmp/new-java2rust-v68-comparator-boundaries`,
release log `.tmp/comparator-boundaries-v68-release.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v68-comparator-boundaries`, and runner log
`.tmp/jgrapht-v68-comparator-boundaries-run.log`. The command checks release
success, copies without overwriting, byte-compares, and records its binary hash
before starting the unchanged pinned 104-file runner. No competing compiler or
translator probes are run during measurement. No v68 outcome is claimed yet.

The v68 release build completes successfully in 1m26s. Its frozen binary is
byte-compared with that build and has SHA-256
`a80fb74af730d38fed6c0ad5723fe980929464abac67e7cb05be8d3f77e0b765`.
Session 3312 has advanced into the full corpus runner; no second process is
started on an observation timeout. The assembler hash remains
`1b6fc7cf676b54760f3fac3dad35ff1a22b0eb3c7b4445023d136e79b9cf3b0a`.

The v68 full gate completes (session 3312, exit 1 for generated Rust). All 104
files parse and translate, with no translator failures. The assembled crate
has 2,099 actual errors (2,063 typed and 36 explicit untyped), excluding Cargo's
final summary. This removes 49 errors versus v67 and 26 versus v66; the invalid
pattern count remains 54 in 29 files. Total time is 305.552 seconds: resolver
37.414, translation 197.038, assembly 55.437 and Rust checking 12.222. This is a
correctness-batch measurement, not an unchanged-output speed comparison.

First-location grouping shows decreases in fourteen files and no file-count
increase: GraphTests -8; four traversal implementations and WeakChordalityInspector
-5 each; VertexDegreeComparator, Graphs and AbstractGraphIterator -3 each;
GraphMetrics and BergeGraphInspector -2 each; FloydWarshallShortestPaths,
ChordalityInspector and GraphMeasurer -1 each. VertexDegreeComparator now has
zero compiler errors, not a claim of independent graph behavior coverage.
The v67 borrowed-interface regression is only partially repaired: the inspector
still has 17 more errors than v66. These remain open attribution/fix work.

The previously untyped comparator chain in GraphMetrics now uses typed native
callback lowering, revealing narrower nullable-bound-receiver recovery and
static callback argument-ABI issues. Inspection also finds that method body
environment alignment skips ordinary interface parameter normalization unless
there is an unrelated storage or nominal rewrite. Constructor argument lowering
can therefore still see a stale boxed storage tag for an emitted borrowed
parameter. Minimize that case before changing the shared alignment boundary.
No production sources are expanded and translated graph oracles remain blocked
by the non-compiling crate, so the full goal is not complete.

### Nullable and borrowed method-reference ABI follow-up

Expanded neutral fixtures now cover constructor borrowing, local nullable
interface capture, eager null-bound-reference checking, and a static source
method whose String argument is borrowed in Rust. Both Java oracles pass under
`.tmp/comparator-references-java-v450`. Rust baseline v450 fails in
`.tmp/comparator-references-v450-baseline.log` (session 96607, exit 101): nullable
interface capture tries to clone `&mut dyn KeyLookup`, and a fresh constructor
argument is routed through `Into<&mut dyn KeyLookup>`. Its null-reference test
initially also assumed the wrong Rust Result ABI for a fully caught exception;
the assertion now checks the same value without the nonexistent unwrap.
Separate static-reference baseline v451 fails with a single String-versus-&str
argument mismatch in `.tmp/comparator-static-reference-v451-baseline.log`
(session 40882, exit 101); source-comparator precedence still passes.

The next batch routes resolved static source references through ordinary typed
call/lambda lowering, aligns all method parameter body environments with the
shared emitted ABI projector, and borrows fresh source implementors directly
for borrowed interface argument slots. A required owned nullable interface is
recovered through its object-safe clone adapter at the payload boundary, without
cloning the loan first; this path uses the physical boxed-interface ABI rather
than requiring a non-stale dispatch tag. The pinned Java and generated Rust
remain untouched.

Focused v452 in `.tmp/comparator-references-v452-test.log` (session 57595,
exit 101) passes comparator factories (including the static borrowed String
reference), native functional objects (2), and shared constructor ABI. The
capture probe improves to one nullable-loan cloning error; the fresh constructor
argument compiles. The new ordinary-interface body-environment unit passes in
`.tmp/comparator-references-v453-test.log` (session 83093, exit 0). That command's
name filter excludes integration cases, so it proves only the named unit, not
the listed zero-test integration targets.

The revised focused integration gate runs without a name filter in
`.tmp/comparator-references-v454-test.log` (session 77796, exit 101). The same
five integrations pass, but nullable interface capture still reaches the old
loan-cloning fallback. Its physical-ABI guard change alone is not a verified
fix. A temporary generic optional-recovery shape trace runs in
`.tmp/comparator-references-v455-trace.log` (session 79130, exit 101) to locate the rejecting
guard before further product changes. The first ordinary-interface constructor
probe passes already; it does not independently reproduce the full inspector's
stale boxed-projection case.

The trace confirms an owned `Box<dyn KeyLookup<i32>>` destination and nullable
mutable source shape, but declaration lookup returns None. The conservative
cross-program lookup rejects separate primary/summary copies; the new recovery
path now reuses the existing preferred-emitted-source lookup for its interface
proof. Direct source parameter queries use that same authoritative lookup with
an optional selected-signature check, rather than requiring a possibly absent
signature for a synthetic/inherited context. Temporary tracing is removed.
The revised focused gate passes all six integrations in
`.tmp/comparator-references-v456-test.log` (session 66718, exit 0): expanded
capture/borrow/null behavior, both factory tests, native functional objects (2),
and shared constructor ABI. Eager null-bound-reference checking is executed
before any comparator invocation and caught by the Java handler.

Nullable recovery additionally verifies identical physical generic trait-object
instantiations, leaving variance/retargeting to the existing typed adapters. A
cheap dispatch/representation rejection keeps ordinary owned slots out of this
interface-specific path. The full library/selected integration gate passes in
`.tmp/comparator-references-v457-regression.log` (session 96604, exit 0), without
a test name filter and with no-fail-fast: all 1,457 library tests and 30
integrations pass. This covers the expanded nullable receiver, eager bound
reference null checking, borrowed static reference and fresh constructor
interface argument cases, together with the established collection, stream,
callback, interface-construction and constructor ABI gates. Formatting and diff
checks pass.

The next release build/full-corpus chain is started (session 30799). Artifacts
are `.tmp/reference-recovery-v69-release.log`, immutable binary
`.tmp/new-java2rust-v69-reference-recovery`, output root
`.tmp/jgrapht-core-1.5.3-focused-v69-reference-recovery`, and runner log
`.tmp/jgrapht-v69-reference-recovery-run.log`. Build success, a non-overwriting
copy, byte comparison and SHA-256 precede the unchanged pinned 104-file runner.
No competing compiler/translator probes are run during measurement. v68 remains
the authoritative full count until this process completes; no v69 outcome is
claimed while live.
This follow-up has not received a new full-corpus measurement. Native static
callback ABI and System identity semantics, remaining borrowed-interface
regressions and two newly exposed collection downcast categories remain open.

The v69 release build completes in 1m27s. Its non-overwritten, byte-compared
frozen binary has SHA-256
`928074fd5c83ddb99be68c73b1d1ed6e97b3ced7b2a37a37201ef3eccaecdf6a`.
Session 30799 advances into the full runner. The assembler remains unchanged
from the hash recorded for v68. No full outcome is inferred from build success.

The v69 full gate completes (session 30799, exit 1 for generated Rust). All 104
files parse and translate without translator failures. The crate has 2,090
actual errors (2,054 typed and 36 explicit untyped), excluding Cargo's summary:
nine fewer than v68. Invalid patterns remain 54 in 29 files. Total time is
304.615 seconds: resolver 37.470, translation 196.374, assembly 54.907 and Rust
checking 12.470. This correctness batch has no measured timing regression
against v68's 305.552 seconds, not an unchanged-output optimization claim.

First-location grouping decreases errors in four files, with no file-count
increase: BergeGraphInspector -4, GraphTests -2, AbstractBaseGraph -2, and
GraphMetrics -1. The mutable nullable graph receiver in the comparator chain
now uses a checked null projection followed by `clone_box`, not `.cloned()` on
the mutable loan. The inspector still has thirteen more errors than v66, so
the old borrowed-interface regression remains only partially repaired. Do not
mark that family closed from the passing neutral probes or net error decrease.

Ordering consumers are a related next batch: `List.sort` is still emitted as a
Rust no-argument sort with a comparator argument, and the external static method
reference in its comparator chain still forwards an owned value to a borrowed
helper. Existing checked comparator/callback sort projections and
`java_collection_sort_by_result` already provide a shared runtime foundation;
reuse them rather than add a separate sorting framework. The runtime helper is
currently stable insertion sort (quadratic); algorithm changes are not yet
justified by a measured runtime blocker. Native System identity semantics and
the remaining borrowed-interface/downcast errors remain separate open work.

The v68-to-v69 category diff removes all sixteen mutable-Graph `.as_ref()`
errors, both readonly-Graph `.as_ref()` errors, and the remaining mutable-Graph
`.as_mut()` error. Other diagnostic categories still regress or change: ten
additional ordinary E0308 errors, six `clone_box` calls on nullable GraphPath
options, one on a numeric result, and one additional incorrect-arguments error.
Do not equate absence of boxed-accessor errors with completion of interface
return/ownership correctness. The numeric case already selected
`get_path_weight` where the source calls `getPath` in v68; its prior diagnostic
was `.expect()` on f64, not a valid translation. The option-return cases need
producer/callee/storage nullability evidence rather than blindly unwrapping or
cloning the carrier. These are open failures, not accepted baseline exclusions.

The next maintained ordering probe is `tests/fixtures/ordering_consumers.java`
with integration `tests/ordering_consumers.rs` and Java oracle
`tests/corpus/callback/OrderingConsumersOracle.java`. It covers `List.sort`,
`Collections.sort`, comparator min/max tie behavior, stable equal-key ordering,
and no further callback effects after the first comparator exception. Its Java
oracle passes under `.tmp/ordering-consumers-java-v458`. The initial Rust gate
fails in `.tmp/ordering-consumers-v458-baseline.log` (session 36191, exit 101):
two invalid comparator arguments to plain Rust sort and two borrowed scalar
results left inside nested get/unboxing arithmetic. Four additional errors are
test-only assumptions that each generated function returns Result. The behavior
test now accepts precisely i32 or Result<i32, JavaException>, panicking on any
uncaught exception in the latter; it does not relax expected Java values or
repair product output. The revised baseline runs in
`.tmp/ordering-consumers-v459-baseline.log` (session 74307, exit 101), with exactly
the four product compiler errors and no test assertion ABI errors. No passing
Rust result is claimed. Emitted min/max calls still reach explicit unimplemented
facade methods; successful compilation alone will not satisfy this behavior
gate. The existing Collections.sort path invokes the infallible facade entry
point and must also adopt the checked callback contract.
The scalar unboxing failure remains maintained coverage rather than being
removed to isolate an easier sort-only fixture. Reuse the existing fallible sort bridge and shared
callback contract machinery for implementation; preserve the unresolved
return-conversion and native identity work in the full goal.

### Ordering-consumer implementation and focused verification

The ordering batch shares a typed native-consumer plan across lowering and
callable-effect analysis. List.sort and Collections.sort reuse the existing
checked sort bridge and functional callback adapter rather than invoking the
Comparator facade's panic entry point. Collections.min/max use a checked,
single-pass extremum helper, retaining the first element on ties and stopping
at the first callback error. Primitive sequence-get operands reuse the existing
slot-directed getter lowering before generic optional unboxing; nested numeric
expressions no longer retain an extra scalar reference.

The first focused gate, `.tmp/ordering-consumers-v460-test.log`, finishes with
both comparator-factory tests and the capture-boundary test passing. The new
ordering test fails with one immutable List.sort receiver, down from four
product compiler errors. The shared native-list mutation policy now includes
sort without treating Set.sort as a supported method. The v461 attempt fails
in a newly added test harness because generated-module tests require static
text; that harness is corrected, not counted as a product failure. Extended
coverage includes isolated runtime-support selection, source Collections/List
lookalikes, empty extrema, and checked errors in both sort entry points and
max. The extended Java oracle passes under
`.tmp/ordering-consumers-java-v461`.

The v462 gate finishes (session 16818, exit 101). The main ordering behavior
test passes, as do all three existing factory/capture tests. Two new isolation
tests fail: inline-only comparingInt creates an opaque imported Comparator
alias, and an implicit source List constructor selects Vec::new. Carrier
discovery now records the executable SAM type for native factory expressions,
and source constructor selection clears native-collection plans before even
implicit constructor handling. The empty-collection slot shortcut also defers
to parsed source constructors. The extremum helper now consumes borrowed
IntoIterator items, supporting sets without materializing a temporary Vec.
Additional coverage exercises HashSet extrema and a borrowed List with a
forwarded comparator. The v463 focused gate is pending. No new full-corpus
result is claimed; v69 remains authoritative.

The v463 gate finishes (session 70052, exit 101): four existing comparator
tests, the main ordering oracle, and source-name precedence pass. Inline-only
factory carrier discovery is fixed. Isolated extrema exposes a missing caller
effect: a target-typed factory expression does not already infer as a facade
value, so the old expression-type test omits the checked callback edge and
emits a plain-return caller. Invocation analysis now uses the native consumer's
expected checked Comparator contract (the same one lowering adapts to), also
covering source implementors. Formal inline callbacks retain their specific
callback-contract edges. This avoids attributing invocation to factory
creation. Added source-implementor coverage forwards through a separate source
method before catching its comparison error; inline throwing-lambda coverage
also remains in the isolation gate. The v464 gate runs library tests and six
related integration suites; results are pending.

The v464 library portion passes all 1,457 tests in 67.49 seconds. All three
ordering integrations pass, including all six isolated consumer programs;
factory/capture/runtime and functional-carrier argument suites also pass.
The gate finishes with one source-callback regression (session 68317, exit
101): repeated consumption of an inline checked comparator is wrapped in Ok
twice. Shared functional callback adaptation now reborrows such a formal and
uses its solved callback-key ABI rather than treating it as a plain callable.
The v465 multi-file source-comparator test (session 8224, exit 101) reaches its
old direct-comparison spelling assertion; the selected source method still has
its expected borrowed String ABI. Its assertion now checks the checked adapter
and borrowed source arguments, supplemented by a generated multi-module
compile-and-run ordering test. The v466 related integration rerun and v467
focused batch test are pending; no broad pass is claimed. Remaining
ordering scope includes Java's null-comparator/natural-order behavior and
nullable factory-extractor validation; the current checked-callback tests do
not establish those semantics or native System.identityHashCode correctness.

The strengthened v467 batch test passes (session 70137, exit 0), compiling
the separately translated comparator and sorter modules and executing the
source String comparator through the checked native adapter. Module imports
are supplied by the test harness; neither generated module body is repaired.
The v466 focused callback/ordering rerun passes (session 24243, exit 0):
13 tests across the capture, factory, functional-argument, ordering, and source
callback suites. In particular, the repeated inline checked-comparator
regression passes without nested Result values or moving the mutable callback
reference. The final-state library rerun v468 passes all 1,457 tests in
67.31 seconds (session 3196, exit 0). The v70 release build is running
(session 43187), with log `.tmp/ordering-consumers-v70-release.log`. Its planned
non-overwriting snapshot is `.tmp/new-java2rust-v70-ordering-consumers`;
build success, byte comparison, and SHA-256 remain pending. No full-slice v70
run has started, and v69 remains authoritative.

The v70 release build finishes in 1m27s (session 43187, exit 0). Its
non-overwritten snapshot byte-compares equal to the release artifact and has
SHA-256 `977b66d128bf152223a83d99fab57864c144fb049e0d05da2f67ad11e33ed163`.
The full 104-file run now uses that frozen binary and JOBS=10, output
`.tmp/jgrapht-core-1.5.3-focused-v70-ordering-consumers`, and log
`.tmp/jgrapht-v70-ordering-consumers-run.log`. No other translator/compiler
probes are running during this measurement. Counts and timing are pending;
v69 is still the authoritative full checkpoint. This is a correctness batch,
not an unchanged-output optimization comparison.

While the frozen v70 run is live, the next source-only batch addresses a
separate callable-family identity gap. SignaturePlan already stores Java
callable identities, but its six family queries treat an empty exact Java
family as permission to match an unrelated longer Rust name. The new neutral
library regression `source_callable_family_lookups_do_not_guess_longer_java_names`
requires that read not select readWeight, while preserving selected overload
names, exact emitted names, and compatibility for plans without Java metadata.
The shared family lookup now distinguishes absent metadata from an
authoritatively absent family, and retains exact synthetic Rust identities.
These edits postdate the frozen v70 binary and are not included in its
measurement. They have not been compiled or tested yet; no competing probe
will run until the v70 process is terminal.
The companion behavior probe is `tests/fixtures/callable_family_identity.java`
with `tests/callable_family_identity.rs` and Java oracle
`tests/corpus/callback/CallableFamilyIdentityOracle.java`. An abstract source
class invokes its interface's fetch method while declaring a distinct
fetchWeight method. Both inherited call results are exercised through a
concrete implementation. This probe is also unverified pending the full run;
it must expose any remaining inheritance/return-ABI gaps rather than be
reduced to signature-query assertions alone.

The v70 full gate finishes (session 76145, exit 1 for generated Rust). All
104 files parse and translate, with zero translator failures. There are
2,080 actual compiler errors (2,044 typed + 36 explicit untyped), ten fewer
than v69; the runner's 2,081 includes Cargo's final failure summary. Invalid
patterns remain 54 across 29 files. Total time is 306.321 seconds: resolver
37.326, translation 197.833 (emission 138.613 within it), assembly 55.427, and
Rust checking 12.387. This correctness batch does not establish a speedup.

First-location diagnostic counts decrease by two in GraphMetrics (15 to 13),
three in GraphTests (33 to 30), three in ChordalityInspector (20 to 17), and
two in WeakChordalityInspector (39 to 37). No file count increases, but the
category diff exposes one new `&JavaSemanticHashSet<V>` IntoIterator error:
the new generic extremum consumer supports borrowed standard containers,
while that semantic carrier only implemented consuming iteration. This is
not an accepted exclusion. Shared borrowed IntoIterator implementations are
now added to both semantic set carriers, following JavaLinkedHashSet's
existing contract, without requiring Clone or key capabilities for iteration.
The maintained comparator-runtime probe checks generic bound-free borrowed
iteration, actual extrema, and preserving collection contents. These runtime
edits postdate the v469 test launch and require a subsequent focused gate.

The callable-family Java oracle passes under
`.tmp/callable-family-identity-java-v469`. The v469 regression process
(session 1291) runs library, callable-identity, ordering, and source-callback
tests. It does not establish full JGraphT compilation or graph behavior;
v70 remains incomplete and no production-source expansion is authorized by
these partial results.
The v469 library portion passes all 1,458 tests in 67.97 seconds, including
the new callable-family lookup regression. Its generated-code integrations
are still running. The semantic-set borrowed-iteration additions are newer
than this library build and remain separately unverified.
The inherited-abstract-call generated-code probe also passes in v469
(10.73 seconds), matching the passing Java oracle's value of eight. The
remaining v469 ordering/callback suites are live. A final combined gate,
`.tmp/callable-family-sets-v470-regression.log` (session 52842), now verifies
the current library plus seven related integration suites, including actual
runtime borrowed iteration over both semantic set carriers. Do not freeze or
measure v71 until this gate is terminal and its results are inspected.
Both pending gates finish successfully: v469 (session 1291, exit 0) and
v470 (session 52842, exit 0). The final v470 gate passes 1,458 library tests
and 15 tests across seven related integration suites, including both borrowed
semantic-set runtime contracts and the inherited callable-identity behavior.
Formatting and diff checks pass. The v71 release build now runs with log
`.tmp/callable-families-v71-release.log`; its planned non-overwriting snapshot
is `.tmp/new-java2rust-v71-callable-families`. Build success, byte comparison,
SHA-256 and the full-slice result remain pending. v70 remains authoritative.

The v71 release build finishes in 1m26s (session 48641, exit 0). The
non-overwritten snapshot byte-compares equal to the release artifact. The full
104-file gate now runs with JOBS=10 using that frozen binary, output
`.tmp/jgrapht-core-1.5.3-focused-v71-callable-families`, and runner log
`.tmp/jgrapht-v71-callable-families-run.log`. No competing translator/compiler
probe will run during the measurement. Full counts and timing are pending;
do not substitute the green regression suites for corpus completion.
The v71 snapshot SHA-256 is
`9257315463bed9700f271212392964366394642d9478479935b725c8ccceab72`;
its full-run session is 57707.

The v71 full run finishes (session 57707, exit 1 for generated Rust). All 104
files parse and translate, with zero translator failures. There are 2,079
actual compiler errors (2,043 typed + 36 explicit untyped), one fewer than
v70, and the same 54 invalid patterns in 29 files. Total time is 301.689s:
translation 193.757s, assembly 54.980s, and checking 12.355s. The sole file
count decrease is BaseShortestPathAlgorithm (four to three). Its wrong numeric
path call is removed; another nullable path-map storage error is now exposed.
No translated graph behavior or full compilation is established.

The borrowed semantic-set iterator still disappears during assembly: raw v71
contains `impl<'a, T> IntoIterator for &'a JavaSemanticHashSet<T>`, but the
assembled crate does not. The shared nominal-owner utility stripped `&` but
retained the lifetime, so it did not associate the impl with its carrier.
`scripts/rust_item_utils.py` now handles lifetime-qualified, mutable, and nested
references while leaving raw trait-impl identities distinct. A maintained
merger regression compiles and executes owned/shared/mutable iteration after
actual two-module support promotion. Cheap projection checks pass, but a full
assembly replay remains required; this category is not waived.

The newer nullable-map batch reuses NullableSequenceElement (which already
represents nullable map values) for put/putIfAbsent value arguments. It adds
one per-analysis owner-indexed return-nullability cache, reusing the resolver's
canonical owner index, so selected calls on other source receivers do not need
repeated whole-program searches or same-name guesses. Abstract reference
returns preserve their nullable value domain. The maintained fixture
`tests/fixtures/nullable_map_values.java`, integration `tests/nullable_map_values.rs`,
and Java oracle `tests/corpus/collection/NullableMapValuesOracle.java` cover
concrete/interface producers, locals, constructor fields, and interface-valued
payloads, distinguishing an absent key from a present null value. Its Java
oracle passes under `.tmp/nullable-map-values-java-v471`.

The v471 Rust gate fails before tests because the canonical-owner method was
exposed on the internal symbol table rather than the SourceSymbols wrapper.
That wrapper now delegates to the existing index. The corrected v472 library,
nullable-map, support-closure, and materialization gate is running (session
67252), log `.tmp/nullable-map-assembly-v472-regression.log`. These edits are
newer than the frozen v71 binary; no passing Rust result or corpus improvement
is claimed for them yet.

The v472 gate finishes (session 67252, exit 101): all 1,458 library tests,
three collection-materialization tests, and all eleven support-closure tests
pass, including compiled/executed owned/shared/mutable lifetime-qualified
iterator promotion. The nullable-map probe fails with four compiler errors.
Producer storage is now correctly nullable, but direct source-call-result and
non-self field arguments do not propagate the collection value-domain
constraint into the inspect helper's map parameter (three errors). The
interface-valued map lookup additionally preserves an extra Option/reference
layer when recovering its owned nullable trait value. Its failed source is
kept under `.tmp/test-tmp/new_java2rust_nullable_map_values_1792289382214307829`.
Do not remove forwarding or interface-valued cases to obtain a smaller pass.

Next general work: retain selected call-result/declared-field argument origins
in usage facts, feed their collection-storage constraints through the existing
signature dependency nodes in both directions (including SCC/dependency edges,
not just an untracked sweep), and reuse canonical map-get nullable payload and
owned-trait recovery rather than adding another ad hoc clone. A hypothetical
output correction is a nullable map parameter plus payload-aware map-get
recovery; this is evidence for those general mechanisms, not permission to
patch generated code.

A separate copied-artifact v71 assembly replay is now running with the shared
owner-parser fix, log `.tmp/jgrapht-v71-borrowed-assembly-replay.log`. It reuses
untouched frozen-v71 raw output and does not include the newer translator
nullable-map edits. Original v71 artifacts remain unchanged. This replay is a
script correctness check, not a new translator timing comparison or a claim of
full goal completion; its result is pending.

The first copied replay finishes (session 82644, exit 1), but its 12,548
errors are **invalid replay evidence**, not a translator regression: copied
status/manifest paths still point to the original output, so import
materialization follows those stale absolute paths instead of populating the
copied modules. It also rewrites imports in the old generated modules. The
earlier claim that all original artifacts were unchanged was therefore too
strong: frozen binary, raw Rust and original diagnostic logs remain intact,
but the original assembled module tree is no longer a pristine v71 snapshot.
A recursive diff verifies that the copied and original raw trees remain
byte-identical, so module bodies are recoverable by normal assembly.

Resume now runs `scripts/rebase-focused-resume-artifacts.py` before clearing
or rewriting modules. It validates all status/manifest rows against local raw
artifacts and output-root containment, then atomically rebases the rewrite
targets. The maintained isolation test checks copied absolute paths, an
external original-file canary, and failure-before-any-write for invalid later
metadata. The v473 support-closure gate passes all twelve tests (session
55943, exit 0), including this isolation test and compiled lifetime-qualified
iterator promotion. Shell syntax and diff checks pass.

A fresh, correctly rebased assembly replay is running with log
`.tmp/jgrapht-v71-borrowed-rebased-replay.log`. It retains the first failed
replay for diagnosis and never edits the frozen raw artifacts manually. Only
the rebased replay may be used to assess the owner-parser fix. The nullable-map
four-error regression remains open independently of this runner correction.
The corrected replay output is
`.tmp/jgrapht-v71-rebased-replay.Uz8N2B` (session 42390). Its status paths now
point into that copy's generated directory, verified before further assembly
progress. No second compiler/translator probe is active.

The corrected assembly replay finishes (session 42390, exit 1 for generated
Rust). It has 2,078 actual errors (2,042 typed + 36 explicit untyped), with
exactly one diagnostic removed from v71: the borrowed semantic-set iterator
error. No other diagnostic category changes; invalid patterns remain 54 across
29 files. This validates the shared owner-parser fix on real assembled output,
without conflating it with the newer nullable-map translator changes. The
incorrect first replay remains excluded from translation/diagnostic comparisons.

The nullable-map follow-up records collection-valued source-call-return and
declared-field argument origins for methods and constructors. Field identity
and ownership reuse the resolver's existing hierarchy/alias index, not a new
name-prefix heuristic. The signature graph and nullable-storage propagation
consume the same bidirectional endpoint edges (returns, fields, method params,
constructor params), retaining existing mutation scopes and SCC scheduling.
Slot-directed map gets normalize the physical Option-of-borrowed-Option loan
before existing representation-aware element recovery; they do not unwrap a
stored null or clone its Option carrier as an interface object.

The v474 gate finishes (session 88692, exit 101): all 1,458 library tests,
three materialization tests, and twelve assembly tests pass. All four earlier
nullable-map compiler errors are removed, including owned interface payload
recovery. One new diagnostic exposes the same missing normalization on an
ordinary nested receiver: String.length is applied to the stored Option after
only the outer lookup's missing-key layer is discharged. Plain native map-get
lowering and slot-directed lowering now share one payload-loan normalization.

The extended Java oracle passes under `.tmp/nullable-map-values-java-v475`;
it additionally covers a direct producer passed to a storing constructor and
both owned-local and direct-receiver interface payload reads. The v475 gate
(session 7205), `.tmp/nullable-map-boundaries-v475-regression.log`, runs library,
nullable-map, materialization and support-closure tests. Its new library test
`collection_expression_arguments_share_storage_sccs_and_late_constraints`
requires all four storage endpoints to share a dependency SCC, then seeds
nullability after graph construction and verifies scheduled propagation.
Results are pending; no full-slice nullable-map improvement is claimed.
The v475 gate passes (session 7205, exit 0): all 1,459 library tests,
the strengthened nullable-map behavior integration, three materialization
tests and twelve support-closure tests. In particular, the late-seeded SCC
regression passes, as do concrete/interface producers, constructor-field
storage, a direct collection result passed to a storing constructor, and owned
plus direct-receiver interface payload reads. The shared plain/slot map-get
normalization removes the final nested String receiver error without dropping
any fixture case. No generated module is manually repaired.

The next compatibility gate is
`.tmp/nullable-map-boundaries-v476-integration.log`: eight related collection,
constructor, generic-interface, virtual-callback and comparator suites. It
must finish before the next release snapshot/full-slice measurement. The
verified full-slice counts remain frozen-v71's 2,079 and its correctly rebased
assembly replay's 2,078, not a measurement of this newer translator batch.
The v476 compatibility gate finishes successfully (session 46257, exit 0):
18 tests pass across all eight selected suites. Formatting and diff checks
also pass. The v72 release build is running with log
`.tmp/nullable-map-v72-release.log`, planned non-overwriting snapshot
`.tmp/new-java2rust-v72-nullable-map-boundaries`. Build success, byte comparison,
hash and full-slice results are pending. The full goal remains active and no
source inventory expansion or graph-behavior success is claimed.
The v72 release build finishes in 1m26s (session 67073, exit 0). Its
non-overwritten snapshot byte-compares with the release artifact and has
SHA-256 `318b390a58687aed1b1a43c526ba93d5b8ab9d816c39abeaaf54cda62a755dc2`.
The full pinned 104-file run is active (session 63102), JOBS=10, output
`.tmp/jgrapht-core-1.5.3-focused-v72-nullable-map-boundaries`, log
`.tmp/jgrapht-v72-nullable-map-boundaries-run.log`. No competing compiler or
translator probe runs during this measurement. Counts and timing are pending;
the green neutral tests do not establish full-slice compilation or behavior.

### v72 terminal result and bounded marker-scan optimization

Session 63102 is terminal with exit 1 for generated Rust. All 104 files parse
and translate, with zero translator failures. Actual compiler diagnostics are
2,072 (2,036 typed + 36 untyped), versus 2,078 in the corrected v71 assembly
replay. Invalid patterns remain 54 across 29 files. The Cargo summary reports
2,073 because it includes the final compilation failure; that is not an
additional source diagnostic.

Wall time is 306.564 seconds: translation 197.983, emission within translation
138.500, assembly 55.307, separate resolver preflight 37.346, and Rust check
12.552. Signature fixpoint is 4.615 seconds. These are correctness-batch
measurements, not an unchanged-output optimization comparison.

The six-error net reduction conceals new failures: mismatched types rise
493 to 530, and 13 optional-slice `to_string` failures appear. NeighborCache
rises 24 to 35 errors, while BergeGraphInspector falls 224 to 213. Nullable
map/storage propagation therefore still needs consumer-shape follow-ups; its
focused green tests do not justify declaring the full batch regression-free.

Fresh inspection identified unnecessary character-by-character lexical scans
for marker patterns absent from a line. `line_count_outside_literals` now
rejects absent patterns with a byte search, retaining its existing lexer for
candidate lines. This changes no emitted Rust, marker definition, literal
handling, or counting rule. The standalone before/after harness
`.tmp/benchmark-output-markers-prefilter.rs` uses an untouched snapshot of the
old module and the 104 frozen v72 raw outputs (47 MiB), asserting identical
invalid-pattern and stubbed-constant counts per file before timing. Four
alternating-order rounds are recorded in
`.tmp/output-markers-prefilter-benchmark.log`: old 3.701–3.756 seconds, new
0.990–0.998 seconds, all returning 54 invalid patterns and zero constants.
This is approximately a 73% reduction for those counters, not for translation
or the end-to-end runner. No competing compiler/translator workload ran during
the benchmark. The separate resolver preflight already has an explicit skip
option; it remains enabled in comparative full gates so diagnostics and timing
coverage are not silently weakened.

Verification: all 14 standalone marker-module tests pass, including a new
overlap/Unicode/escaped-string/raw-string/comment regression. The combined
gate `.tmp/marker-prefilter-comparator-regression.log` passes all 1,460 library
tests and the generated comparator runtime test, but initially finds one stale
integration assertion requiring `.clone()` rather than the current explicit
`Clone::clone` spelling. The test now checks the companion-call boundary
without prescribing clone syntax, and additionally compiles and executes the
unmodified translation of `tests/differential/InnerCapture.java`. Both Java and
emitted Rust return `21`, proving observation of the outer mutation without a
reentrant borrow. The complete 28-test marker integration rerun passes in
`.tmp/marker-prefilter-integration-rerun.log` (session 61540, exit 0).
Formatting and diff checks pass. No emitted Rust was repaired. This bounded
follow-up is finished; correctness work resumes, with the full JGraphT
compilation/behavior objective still active and unachieved.

### Natural-order comparator batch (after v72; verification in progress)

`NativeComparatorOperation` now describes natural/reverse-order factories and
their exact utility owners in addition to key-extractor composition. Lowering
uses the existing native-utility owner proof (lexical bindings, imports,
resolved/source precedence); facade suppression and result-carrier discovery
reuse that metadata. In particular, an expression-only
`Collections.reverseOrder()` requires the executable Comparator carrier, not
an opaque Collections factory stub.

Natural comparisons reuse `JavaTreeKey` rather than inventing Rust `Ord`
requirements for Java objects. A checked integer-returning comparison method
preserves Java `compareTo` magnitudes and exceptions, while the existing tree
ordering entry point projects that integer's sign. Source hierarchy carriers
dispatch through their translated method and solved checked ABI. String and
identity-preserving JavaString ordering reuse the existing UTF-16 comparison
helper, moved to shared always-emitted support; JavaString's Ord uses that
same ordering. Reversal swaps operands, rather than negating a possibly minimum
integer. Native sort/min/max null comparator operands select natural ordering;
nullable stored comparators are evaluated once and retain shared callback state
when non-null. Empty extrema still use their existing checked runtime.

The maintained Java/Rust fixtures are `natural_order_consumers.java` and
`natural_order_comparable.java`, with Java oracle
`tests/corpus/callback/NaturalOrderConsumersOracle.java`. The Java oracle passes
under `.tmp/natural-order-java-oracle`. It checks integer and string comparator
factories, utility factories used inline, UTF-16 supplementary/BMP ordering,
literal/stored/conditionally-null comparators, sorting, extrema, empty minimum,
source hierarchy comparison magnitudes, lazy construction, and catchable source
comparison errors. The direct runtime gate additionally checks non-Clone keys,
nullable comparison keys, and checked failures without hand-editing output.
Source lookalike factory owners have a separate executable regression.

The first focused v477 gate passes both existing comparator factory tests and
the extended comparator runtime test, but fails the new translated fixtures:
callback parameters lose their hierarchy-carrier shape, a source static counter
update incorrectly demands a mutable instance receiver, and the Unicode-escaped
BMP string literal is misparsed as literal `uE000`. The shape follow-up uses the
existing representation-aware storage projection for native SAM input shapes,
typed lambda parameters, and sort adapter parameters. Static field names are
obtained once per source owner through the resolver's existing hierarchy-field
catalog and shared across callable usage facts; field constraints remain
recorded, but static storage updates do not mark `this` mutable. A focused unit
checks direct/inherited static counters, assignments, local shadowing, genuine
instance mutation, and an instance field hiding an inherited static field.

Unicode-escape preprocessing is a separate **unfixed** parser gap. The UTF-16
ordering fixture now uses the equivalent literal BMP character so it tests
ordering independently of escape decoding; Java and Rust must still compare
the same value. No Unicode parser support is claimed, and the v477 failure log
preserves evidence of the defect. Unproven generic natural-order capabilities,
lazy construction for non-Comparable `Collections.reverseOrder` element types,
and genuinely receiver-mutating Comparable implementations also remain open;
the current static-bound runtime is not a complete Java dynamic Comparable
implementation. The v479 focused compatibility gate is running in
`.tmp/natural-order-v479-regression.log`; no v73 binary or full-corpus result is
claimed yet.

The v479 gate finishes successfully (session 14592, exit 0): 15 tests across
comparator-capture boundaries (1), native functional objects (2), natural-order
consumers (3), existing ordering consumers (3), and source callback contracts
(6). The expanded v480 gate then catches a static assignment spelling mismatch
in the new unit regression: qualified `this.totalCount` remains Java-spelled
while catalog fields are normalized. A standalone parser/usage probe linked
against the built translator confirms that AST distinction in
`.tmp/probe-static-receiver.log`; the lookup now reuses the existing name
normalizer. This is not waived as a pre-existing failure.

The final v481 combined gate succeeds (session 84308, exit 0), recorded in
`.tmp/natural-order-v481-combined.log`: 1,462 library tests, two comparator
factory tests, one extended comparator runtime test, four natural-order
integration tests, and one nullable-map integration test (1,470 total). The
fourth natural-order test covers identity-preserving JavaString and Ord using
the same UTF-16 comparison. The primitive fixture now also checks empty null
sorts, direct invocation of a nullable comparator (which must still throw NPE,
not select natural order), repeated nullable comparator use, and forwarding a
null comparator through a source method to List.sort. Repeated values use the
existing owned-argument reuse adapter; comparator clones share callback state.
The expanded Java oracle passes as well. Formatting and diff checks pass.
These gates do not prove unsupported generic/dynamic Comparable capabilities,
or every nullable inline-source-callback ABI; those remain evidence-driven
follow-up work rather than completion claims.

A release build is now running with log
`.tmp/natural-order-v73-release.log`, planned non-overwriting frozen snapshot
`.tmp/new-java2rust-v73-natural-order-boundaries`. A fresh full 104-file gate
will follow successful build/copy/hash verification, with no competing compiler
or translator workload during measurement. No v73 timing or error-count claim
is made before that run is terminal.

The v73 release build finishes successfully in 1m26s (session 59978, exit 0).
The non-overwriting binary copy matches the release executable byte-for-byte,
SHA-256 `56d535cfbaf0bf71f19e688a30721ae6db413e9be0be3b1007e408a397a3bc77`.
The unchanged pinned 104-file runner is now active (session 18399), JOBS=10,
output `.tmp/jgrapht-core-1.5.3-focused-v73-natural-order-boundaries`, log
`.tmp/jgrapht-v73-natural-order-boundaries-run.log`. No competing compiler or
translator probe runs during this measurement. Counts, elapsed phases, and
source-error deltas remain pending; v72 is still the latest completed full
measurement.

### v73 terminal result and nullable/wildcard projection follow-up

Session 18399 is terminal with exit 1 for generated Rust. The pinned 104-file
run has zero parse or translation failures, 2,068 actual Rust errors (2,032
typed + 36 untyped), and 54 invalid patterns across 29 files. Cargo's aggregate
reports 2,069 including the final compilation-failure diagnostic. Relative to
v72, AbstractBaseGraph falls 37 to 35 errors, GraphTests 31 to 30, and
FastLookupGraphSpecificsStrategy 10 to 9. Category changes are not uniformly
positive: mismatched types fall 530 to 524, trait-used-as-type errors 13 to 9,
and a missing Collections owner 3 to 2, while missing Graph generic arguments
rise 3 to 9 and one additional E: JavaMapKey requirement appears. None of the
new diagnostics is waived as harmless.

Total time is 298.694 seconds: translation 190.479 (emission 135.563 within
that), assembly 54.821, separate resolver preflight 37.767, and Rust checking
12.247. Status/output writing is 3.635 seconds after the previously isolated
marker-counter optimization. No competing compiler or translator ran during
measurement. This is a correctness-batch measurement, not an unchanged-output
optimization comparison or proof that its total-time difference is causal.

The next coherent batch addresses value projection after source nullability and
wildcard dispatch have been selected. Nullable native collection display now
has a shared conversion/display policy: borrow the option, map present list,
set, or map through its existing Java formatter, and spell absence `null`.
It preserves evaluation once and does not consume retained collection storage.
Source-defined collection lookalikes are rejected by the existing effective
source-carrier proof. The existing checked source toString adapter is extended
to nullable infallible source overrides, including nullable Java String
results, rather than treating their source objects as native containers.
Literal null receiver invocation must still throw when Java calls toString
directly; concatenation/formatting have different null semantics.

The v73 missing-generic diagnostics identify a bounded-wildcard rendering loss:
dispatch selects Graph from `? super Graph<V,E>`, but the nominal renderer
previously inspected only top-level Named types and emitted raw `dyn Graph`.
The central dyn-trait/closed-carrier renderer now recovers the already-selected
bound's full nominal arguments without changing wildcard variance or admitting
new reads/writes. Callback parameter rendering also uses the existing
representation-aware slot renderer, retaining nested carrier identities.

The maintained fixtures/oracles are `nullable_collection_display.java` and
`wildcard_callback_carriers.java`, with Rust integration suites and corresponding
Java oracles in collection/ and callback/. Both Java oracles pass under
`.tmp/{nullable-display,wildcard-callback}-java-oracle`. The first v482 focused
gate passes collection materialization but fails the new display integration:
five harness assertions incorrectly unwrap infallible returns, and the source
lookalike exposes a genuine optional source-display omission. The harness ABI
is corrected and the source adapter generalized; no emitted Rust is repaired.
The v483 batch is running in
`.tmp/nullable-display-wildcard-v483-regression.log`. Neither its success nor
full-slice improvement is claimed before verification.

The v483 gate terminates (session 73616, exit 101). Collection materialization
(3), natural ordering (4), nullable collection display (2), and nullable map
values (1) pass. The wildcard callback integration fails with three diagnostics:
the declaration erases `? super Measure<String>` to Any, which disagrees with
the typed invocation and lambda. SAM metadata now projects an accepted input's
lower bound through a shared `generic_input_type`, dual to the existing
`generic_read_type` for producer results. Callback declaration assembly now
uses those same descriptor input/result types rather than its own raw argument
table. Directional unit coverage explicitly leaves extends inputs and super
results unprojected; it does not authorize arbitrary wildcard writes.

The v484 combined gate terminates (session 9622, exit 101): all 1,464 library
tests, three generic-interface construction tests, both nullable-display tests,
and six source-callback tests pass. The wildcard fixture then exposes a second
ABI disagreement: declaration assembly uses a borrowed interface value view
while invocation/lambda lowering uses the owned storage carrier. SAM inputs
now consistently use the existing storage projection, with a focused unit
checking the owned generic trait object. No new ownership framework is added.

The expanded source-lookalike display regression also allows Java toString to
return null. Its v485 run exposes Rust method lookup selecting ToString on an
extra reference, making a planned Option<Option<String>> flatten instead see
Option<String>. The adapter now discharges the physical borrowed payload layer
before source dispatch, retaining the declared Java nullable/checked result.
This uses the existing structured dereference projection, not a text rewrite.

The v486 combined gate is running in
`.tmp/nullable-display-wildcard-v486-combined.log` (session 8617); existing
`idiom_gaps` tests filtered by `to_string` are queued in
`.tmp/nullable-display-v486-existing-to-string.log` (session 28447). Neither
gate nor a new full-corpus measurement is claimed successful while pending.

The v486 combined gate terminates successfully (session 8617, exit 0): all
1,466 library tests, three generic-interface construction tests, two nullable
collection/source-display tests, six source-callback tests, and the wildcard
callback integration pass (1,478 total). The final source-lookalike test also
checks a non-null object's Java toString returning null. Both Java oracles
continue to pass. Formatting and diff checks pass.

The broader existing `idiom_gaps` filter terminates separately (session 28447,
exit 101): 21 pass and 11 fail. Ten failures are shape/string assertions; the
deep-array test fails its generated Rust harness because a cube cell is
supplied as Option<String> while the emitted signature expects String. These
are **verified pre-existing relative to v73**, not silently ignored new
regressions: `.tmp/probe-to-string-baselines-v2.sh` extracts each exact Java
input, uses the original files for the two include-file fixtures, and preserves
the Constants context source for the context-sensitive test. Frozen v73 CLI
and the current library API emit byte-identical Rust for every one of the 11
cases, with all v73 translation statuses zero. Evidence is
`.tmp/to-string-v73-v486-valid-baseline.tsv` and the corresponding
`.tmp/to-string-v73-v486-valid-baseline/` artifacts. The first exploratory
extractor accidentally retained Rust raw-string delimiters; its differently
named earlier artifacts are invalid and must not be used for attribution.
No legacy failure is claimed fixed or semantically harmless by this comparison;
it proves only that this batch did not introduce the observed output/test
failure. The original assertions and harness remain unchanged.

A v74 release build is running in `.tmp/value-projection-v74-release.log`
(session 2031). Baseline attribution is now terminal, so a non-overwriting
frozen copy and full 104-file gate can follow build verification. The unresolved
set/collection-to-slice argument carrier mismatch and generic collection-display
capability requirements remain follow-up work; the selected focused passes do
not establish full JGraphT compilation or graph behavior.

The v74 release build finishes successfully in 1m27s (session 2031, exit 0).
Its non-overwriting frozen copy matches byte-for-byte, SHA-256
`c13202d30d22cd2519f134dead366907748ddb9c87f37e1f896eafcb5fa6a1c5`.
The unchanged pinned 104-file run is active (session 27734), JOBS=10, output
`.tmp/jgrapht-core-1.5.3-focused-v74-value-projection`, log
`.tmp/jgrapht-v74-value-projection-run.log`. All other compiler/translator
probes are terminal; none competes with this measurement. Counts and timings
remain pending, with v73 still the latest completed full-slice result.

### v74 terminal result and renewed optimization readiness

The previously running v74 gate is terminal (session 27734, exit 1 for generated
Rust compilation). Direct counting of `compile.log`, excluding Cargo's summary,
gives 2,029 typed plus 36 untyped errors. The runner's summary count of 2,066
includes an extra summary diagnostic and is not the actual-error baseline.
All 104 files parse/translate with zero translator failures; 54 invalid patterns
remain across 29 files. Total time is 300.503 seconds, including translation
192.196, assembly 54.989, resolver preflight 37.546, and Rust checking 12.357.
These are correctness-batch timings, not an unchanged-output speedup comparison.

The subsequent v488 regression batch passes all 25 tests: generic collection
display (1), generic method binding (22), and nullable collection display (2).
Evidence: `.tmp/generic-display-explicit-witness-v488-regression.log`. Display
requirements follow selected generic source calls without imposing unnecessary
Clone/key/order bounds on collection iteration. Explicit source method witnesses
are represented in the IR, qualified and used in overload applicability, bound
checking, argument targets, return substitution, and capability propagation.
This does not establish support for every native factory witness or a Rust
turbofish when no argument/result can infer the witness. Source-object display
semantics and interface-wide display capability propagation remain open.

The readiness library gate also passes all 1,466 tests (session 27142, exit 0;
`.tmp/optimization-readiness-v489-lib.log`). This checks the schema migration
and existing library behavior before any renewed performance edits. It does
not substitute for compiling every integration target or the full corpus.

On the renewed optimization request, first verify the current regression batch,
then profile the current worktree rather than attributing old v61 hotspots to
new code. Preserve unfinished correctness changes and compare identical current
inputs/output for performance changes. The last full profile still identifies
emission/analysis and assembly as the dominant costs; fixpoint saturation alone
is only 4.861 seconds, versus 23.231 seconds for total signature planning.

### Fresh v75 performance baseline (no optimization claim yet)

The current worktree release builds successfully in 1m35s (session 38614,
exit 0; `.tmp/optimization-current-v75-baseline-build.log`). A non-overwriting
frozen copy, verified with `cmp`, is
`.tmp/new-java2rust-v75-optimization-baseline`, SHA-256
`2e8f36e152c8393b45e1adc4b4651b0f119488ff6c31276e7dc1e93e9b787704`.
This includes the newer correctness batch and must not be compared to v74 as
an unchanged-output optimization.

The fresh sampled run retains all 104 primary sources and emits only
`BoyerMyrvoldPlanarityInspector.java`, with one worker, context-dir-emitted,
and `NEW_JAVA2RUST_PROFILE_LOWERING=1`. It is terminal (session 83732, exit 0),
94.933 seconds total, emission 51.106 seconds, signature fixpoint 4.592 seconds,
and signature plan building 5.715 seconds. All other compiler/translator jobs
were terminal before sampling. Perf records 9,350 samples and zero lost samples.
Kernel symbols are restricted, but user-space symbols/call stacks are available.
Artifacts: `.tmp/jgrapht-boyer-v75-{perf.data,perf-flat.txt,perf-tree.txt,phases.tsv,workers.tsv,profile.log}`
and `.tmp/jgrapht-boyer-v75-profile/`. These instrumented single-file timings
are not a full-slice baseline or a demonstrated new speedup.

Fresh self-time evidence: hashing 8.72%, memcmp 7.80%, Rust type-name rendering
7.56%, snake-case conversion 5.65%, and the external type-name comparison cache
lookup 5.34%. Call stacks attribute 3.33% of total sampled CPU directly from
snake-case conversion to signature type-name alias generation, including 1.78%
through `SignaturePlan::java_method_family_signatures`. That method still scans
all Java method keys for every family query and regenerates owner aliases for
every method of the same owner. Representation declaration matching accounts
for another 2.26% of total samples under Rust type-name rendering. The suggested
next coherent batch is owner/family candidate indexing and reuse of immutable
declaration aliases, not a speculative fixpoint rewrite. Preserve exact source
family authority, synthetic exact-name fallback, symmetric owner-alias matching,
ambiguous package-owner refusal, and support-plan merge/update invalidation.
Prefer extending existing lookup inventories over a second naming policy.
Measure output equivalence and runtime against this frozen current baseline
before accepting a speedup or returning to correctness work.

### Owner-index and parsed-source identity batch under verification

The signature Java-key inventory now owns an owner-grouped map and leaf/snake
alias-to-owner index. All insertions, inherited additions, and primary-first
support merges pass through that inventory, so there is no separately stale
memo to invalidate. Exact Java keys remain authoritative. Family lookup selects
only matching owner buckets, using the existing symmetric name predicate's
candidate set (not transitive alias equivalence), then applies the unchanged
Java-family/emitted-name test. Synthetic fallback and ambiguous-owner guards
remain unchanged. An exhaustive neutral test compares indexed candidates with
the old pairwise predicate across simple, qualified, nested, dollar, snake,
keyword, missing, and colliding owners; it also covers replacement, primary-first
merge, clone independence, and clearing.

The existing immutable FactSet lookup session also indexes parsed-source
identity names once. Only exact source names and their existing flattened Rust
spellings enter this set: external leaf aliases and non-ParsedSource facts must
not claim source ownership. Other FactSets and calls outside a session retain
the original scan. New source-identity tests compare indexed/scan results,
including lookalike platform owners and mutations between sessions; the existing
nested/unwind/mutation gates apply to the same session lifetime.

The first signature-only gate passes 126 tests (session 36105, exit 0;
`.tmp/signature-owner-index-v490-units.log`). This precedes the parsed-source
identity addition. A complete library gate and current candidate release build
are running in `.tmp/signature-source-index-v491-lib.log` (session 57366) and
`.tmp/optimization-v76-index-build.log` (session 29494). Performance benefit and
full emitted-output equivalence are not yet established. Do not run timed probes
until both compiler/test processes are confirmed terminal.

The complete library gate is now terminal and passes all 1,468 tests (session
57366, exit 0). The six-suite integration batch also passes all 37 tests
(session 38061, exit 0; `.tmp/signature-source-index-v491-integration.log`):
callable-family identity 1, generic collection display 1, generic method binding
22, nullable collection display 2, source callback contracts 6, and source stream
resolution 5. The release build finishes in 1m59s (session 29494, exit 0).
Its non-overwriting frozen copy matches the build byte-for-byte:
`.tmp/new-java2rust-v76-owner-source-index`, SHA-256
`89416ac09f27ae5ce039d1cff9fcc9318706f4a430fafe34cd82a54936ac5690`.
All 104 staged source files compare equal to the clean pinned checkout at the
expected commit. An uninstrumented v75 single-file run is active (session 81423),
retaining all primary analysis, with no other compiler/translator/test workload.
The v76 comparison must follow its terminal result, not run concurrently.

### First controlled v75/v76 timing and output comparison

Both uninstrumented single-file runs are terminal: baseline session 81423 and
candidate session 44189 each exit 0. With identical 104-file analysis context,
one emission worker, and no competing workloads:

| Phase | v75 baseline | v76 indexed |
| --- | ---: | ---: |
| Signature planning | 23.015 s | 22.452 s |
| Emission | 50.192 s | 42.643 s |
| Total | 93.577 s | 85.611 s |

This pair measures 15.0% less emission time and 8.5% less total time; it is one
representative-file pair, not a full-corpus or repeatability claim. Recursive
generated-Rust comparison is empty in
`.tmp/jgrapht-boyer-v75-v76-output-diff.log`. The profiled and uninstrumented v75
outputs also match exactly (`.tmp/jgrapht-boyer-v75-profile-output-diff.log`).
Timings live in `.tmp/jgrapht-boyer-v{75,76}-uninstrumented-{phases,workers}.tsv`,
logs alongside, and output roots `.tmp/jgrapht-boyer-v{75,76}-uninstrumented/`.
The full 104-file frozen-v75 baseline is now running separately in
`.tmp/jgrapht-core-1.5.3-focused-v75-optimization-baseline`, log
`.tmp/jgrapht-v75-optimization-baseline-run.log`. Only after its terminal result
may the frozen-v76 comparison start. Those gates must establish whole-output
equivalence, unchanged failure/placeholder counts, and full-run timing before
accepting the performance phase transition.

The v75 full baseline is terminal (session 1184, exit 1 for generated Rust).
All 104 files parse/translate; direct `compile.log` counting gives 2,027 typed
plus 36 untyped errors, 2,063 total. The runner's 2,064 includes its summary
diagnostic. Invalid patterns remain 54 across 29 files. Total 301.661 seconds:
resolver preflight 37.299, translation 193.388 (emission 137.583), assembly
55.080, Rust checking 12.479, and idiom sweep 3.057. The v76 full comparison
is active (session 62721), output
`.tmp/jgrapht-core-1.5.3-focused-v76-owner-source-index`, log
`.tmp/jgrapht-v76-owner-source-index-run.log`. No other compiler/translator/test
workloads overlap either timed gate. No new source edits occur during this pair.

### v76 full gate: optimization accepted, return to correctness

The optimized full gate is terminal (session 62721, exit 1 for generated Rust,
not a translator failure). All 104 files parse/translate. Actual diagnostics
remain 2,027 typed + 36 untyped = 2,063, with 54 invalid patterns across 29 files.
Both category/count TSVs compare exactly with v75. Recursive comparisons of
all raw-before-support-strip Rust and assembled crate source are empty:
`.tmp/jgrapht-v75-v76-{raw,assembled}-output-diff.log`.

| Phase | v75 baseline | v76 indexed |
| --- | ---: | ---: |
| Resolver preflight | 37.299 s | 37.104 s |
| Translation | 193.388 s | 177.972 s |
| Emission, within translation | 137.583 s | 123.595 s |
| Assembly | 55.080 s | 55.416 s |
| Generated Rust checking | 12.479 s | 12.281 s |
| Total | 301.661 s | 286.199 s |

This pair measures 10.2% less emission time, 8.0% less translation time, and
5.1% less total time, with unchanged whole-slice output. Assembly is unchanged
within variation; no new assembly speedup is claimed. The isolated single-file
pair independently shows the same direction, but these are not repeated-run
confidence intervals. The library and integration gates above remain green.

Stop this bounded pass here: the measured repeated-work improvement is general
and verified, while a further dominant assembly improvement would require the
mutable-catalog/invalidation redesign identified in the previous profiling pass.
Fixpoint alone remains a small fraction of the gate. Full-corpus timing is now
about 4m46s and remains reserved for coherent batches, not individual fixes;
focused regressions provide the inner development loop. Do not extend this into
open-ended micro-optimization while emitted Rust still has 2,063 errors.

The combined goal stays active and returns to correctness immediately. A useful
next cluster is source `Iterable` implementations with retained supplier
callbacks: the baseline emits `Box<LiveIterableWrapper<E>> as Vec<E>`, expects a
native Supplier carrier but supplies a bare closure, and converts source-set
results through native HashSet snapshots. Those expressions share a missing
carrier/callback contract, not 25 independent cast bugs. Minimize them in neutral
fixtures and preserve live-view behavior; adding Rust Hash/Eq bounds or replacing
views with snapshots is not an acceptable repair.

### Correctness restart: minimized live supplier/Iterable contract

Maintained `tests/fixtures/live_iterable_supplier.java` and
`tests/live_iterable_supplier.rs` isolate this cluster without graph names or
corpus facts. A source generic Iterable retains a Supplier, then callers mutate
a captured array between traversals. Concrete and interface-typed traversals
must both observe 7 then 9, totaling 16. The Java oracle
`tests/corpus/collection/LiveIterableSupplierOracle.java` passes under
`.tmp/live-iterable-supplier-java`, printing `direct=16;interface=16`.

The first generated-Rust gate fails as expected (session 86011, exit 101;
`.tmp/live-iterable-supplier-v492-regression.log`), with six actual diagnostics:
one missing `.iterator()` on the Supplier result Vec, two bare-closure/native
Supplier carrier mismatches, two callback `?`-effect mismatches, and one source
object-to-Vec cast. This new maintained test is failing and is not included in
the earlier green 37-test batch. It must not be ignored or counted as an accepted
pre-existing exclusion. Generated code has not been repaired.

Inspection shows `SourceCallbackPlan::build` registers only Method members;
constructor callback slots do not share its selected-call retention/forwarding
contract. The captured state also remains an ordinary Vec in the reproducer,
so merely wrapping the bare closure would not establish shared Java aliasing.
Existing concrete-source enhanced-for lowering already calls the selected
source iterator. Reuse/generalize constructor callable identity, source callback
retention/effect planning, native SAM wrapping, and the iterator protocol;
do not add a second syntax-based callback policy or materialize the source view
into a Vec merely to silence the cast. This is the next coherent correctness
batch, with the full goal still active and no new performance run in progress.

### Constructor SAM values, invocation result typing, and owned iterator batch

Constructor argument lowering now reuses assignment's native SAM value adapter
for owned functional slots. Constructor declarations already use that value ABI;
passing a bare closure was inconsistent with the emitted declaration. Typed
facade construction, method references, callback input/result shapes and deferred
exception handling come from the existing functional-slot machinery. Same-class
constructor delegation and delegation with trailing work use the same constructor
argument path. Nullable SAM slots build a non-null typed payload and preserve
the outer Some carrier. No corpus names or output text matching are involved.

Generic external SAM invocation results also use FunctionalInterfaceShape,
including exact invocation arity, Java/snake method aliases, primitive results
and producer wildcard projection. This restores the Java collection type in
`supplier.get().iterator()`. It does not reinterpret a real Java Optional's get
as invocation of its contained Supplier. An initial new unit used the invalid
IR spelling `Option` for that distinction; the corrected test uses a real
`java.util.Optional` and the attempted generic Optional-unwrapping shortcut was
removed. Source-lookalike regression coverage remains part of the batch.

Iterator lowering now consumes a proven owned temporary Vec directly through
the existing JavaIterator::new IntoIterator contract, avoiding `.iter().cloned()`
and arbitrary T: Clone bounds. Selection requires a native iterator target,
temporary expression, solved owned receiver shape, and a rendered Vec carrier.
It does not consume local/field bindings, borrowed returns, queues with different
physical carriers, or source live views. Lower-context target application now
receives ShapeEnv so this decision uses planned physical carriers rather than
guessing ownership from Java type names or call syntax alone.

The stronger source-result test also exposed owned List slots retaining an
unsized Slice view. The existing array slot normalizer is generalized to sequence
slots: owned Slice recovers its sized Native carrier; the old borrowed-array
Native-to-Slice preference stays array-specific. This repairs an impossible ABI,
not Java collection aliasing: any pre-existing source-return snapshot behavior
still requires the shared-reference representation work below.

Maintained `constructor_callback_values.java` / `constructor_callback_values.rs`
and `ConstructorCallbackValuesOracle.java` cover primitive/object SAM constructor
arguments, method references, nullable slots, delegation, deferred parse failure,
SAM collection-result chaining, source sequence returns, and continued use of a
stored collection after iterator creation. The Java oracle passes with results
23, 7, -1, 9, 4, 9, 6, 6 respectively. An additional Rust consumer supplies a
non-Clone element through a generic Supplier/iterator, testing the absence of
unnecessary bounds. Its initial harness accidentally selected JavaIterator's
inherent next (which returns T); the corrected harness explicitly calls the
standard Iterator::next and checks its Option. No generated source is edited.

Verification history: v494 passes the expanded constructor test plus native
functional objects (2), retained callbacks (2), and source callback contracts (6),
while the live-view case fails with three diagnostics. V495 passes all existing
1,468 units but fails the new Optional distinction unit; it exposes clone bounds
after restoring result typing. V496 passes all 1,469 units and the production
iterator assembly test, and leaves the original live-view case with exactly two
diagnostics: source-to-Vec cast and use of moved captured state. Its expanded
constructor test exposes the unsized source-return ABI and the harness issue
described above. Those are addressed in the next batch; no failed gate is waived.
Current combined verification is running (session 5335) in
`.tmp/constructor-sam-sequence-v497-regression.log`, including all library tests,
both new regressions, native/retained/source callback suites, and production
iterator assembly. V76 remains the latest completed full-corpus measurement.

The remaining live case is not safely repaired by a local cast or added clone.
Native arrays are still value-owned Vecs, while source objects already have
canonical shared handles. General shared native-collection storage must preserve
the captured allocation and its external mutation sites; a lazy Iterable carrier
must dispatch each traversal to the retained source/supplier instead of converting
it to a snapshot Vec. Constructor callback slots still do not participate in the
method-only SourceCallbackPlan inventory; the value-ABI fix above does not claim
that graph generalization or native-array aliasing is complete. Keep the original
Java/Rust liveness regression failing visibly until those protocols are supported.

The final v497 combined gate is terminal (session 5335, exit 101 only for
`--test live_iterable_supplier`). All 1,470 library tests pass, as do constructor
callback values (1, including the generic non-Clone Rust consumer), production
iterator assembly (1), native functional objects (2), retained callback state
(2), and source callback contracts (6): 12 passing integration tests. The
unchanged live-view regression fails with exactly two generated-Rust errors,
E0605 for source-to-Vec cast and E0382 for externally reused captured state.
Formatting and diff checks pass. This is neither a fully green test suite nor
proof that all live iterable/capture semantics are implemented.

A v77 release build is active (session 25510), log
`.tmp/constructor-sam-sequence-v77-release.log`. Once terminal and successful,
freeze it non-destructively, record cmp/SHA evidence, and run the pinned full
104-file gate without competing compiler/translator workloads. This is a
correctness-batch comparison against v76, not another unchanged-output speedup
claim. The next general representation work remains shared native captured
allocations plus an actual lazy Iterable interface carrier; no generated output
or pinned Java has been repaired to bypass those requirements.

### v77 terminal corpus gate and delegation follow-up

The release build finishes in 1m27s (session 25510, exit 0). Its non-overwriting
frozen copy matches the build with cmp, SHA-256
`e2e156fefe86ed9d28aa47390064a13269e586da15a3994201704720fcf959f1`:
`.tmp/new-java2rust-v77-constructor-sam-sequence`. The full gate is terminal
(session 86997, exit 1 for generated Rust), output
`.tmp/jgrapht-core-1.5.3-focused-v77-constructor-sam-sequence`, log
`.tmp/jgrapht-v77-constructor-sam-sequence-run.log`. All 104 files parse/translate
with no translator failures. Direct compile.log counting gives 1,957 typed plus
36 untyped errors, 1,993 actual; invalid patterns are 40 across 22 files.

Time: total 293.000 seconds; resolver preflight 37.335, translation 185.092
(emission 128.220), assembly 55.067, Rust checking 12.132, idiom sweep 3.031.
No other compiler/translator ran during the gate. New metadata preparation
edits were made while the frozen binary ran; their compilation waited until
the gate was confirmed terminal. These working-tree edits are not part of v77.

Against v76: 70 fewer actual errors and 14 fewer invalid patterns. E: Hash and
E: Eq each decrease by 25; V: Hash and V: Eq each decrease by 5. Seven invalid
downcasts on each of the source Specifics and IntrusiveEdgesSpecifics trait
carriers disappear. This is not a clean regression gate: four newly reported
zero-argument factory errors expose PairingHeap::new still modeled as an opaque
instance method. The identical unmodeled `new(&self) -> ()` facade exists in v76;
v77's typed SAM wrapper exposes that underlying dependency-model gap. No heap
semantics or constructor-reference support is claimed fixed.

One additional Try error on an interface Box is a real conversion-path problem
in GraphMeasurer constructor delegation: using the simpler ordinary-constructor
argument path duplicated subtype conversion/propagation. The follow-up restores
the complete source argument pipeline for delegation, with an explicit native
SAM value ABI mode rather than discarding its ordinary conversion rules. Both
plain delegation and delegation with trailing work share it; ordinary source
calls retain their existing callback-contract mode. The owned SAM adapter itself
is shared with normal constructors. A neutral source-interface implementor with
a fallible array-reading constructor is now delegated alongside a Supplier,
covering both successful paths and a caught failure. Java oracle results include
69 and -3. Rust verification is active (session 52764), log
`.tmp/constructor-delegation-values-v499-regression.log`, alongside generic
interface construction and source callback contracts. The exact JGraphT result
must still be inspected after this follow-up; no full-corpus fix is asserted yet.

### Reusing SAM metadata for applied return types and lazy factory values

Before changing global Iterable representation, FunctionalTypeSource now supports
an applied unary result recipe such as Iterator<T>. Input/output projection,
generic arity discovery, source callback validation, and Rust result rendering
share that recipe; the existing SAM value emitter is factored to accept a shape.
No Iterable registration or collection-to-Iterable conversion has changed yet.
This is a necessary foundation, not completion of the live-view regression.

Both new focused units pass (session 86390, exit 0;
`.tmp/applied-sam-factory-v498-focused.log`). One checks generic/wildcard result
projection and canonical JavaIterator rendering. The other compiles and executes
the actual existing value runtime with a factory returning JavaIterator of a
non-Clone payload: construction and clone are lazy, clones share factory state,
each iterator request invokes the factory anew, and deferred errors propagate
through the checked result method. Production lowering supplies the runtime
dependency closure; no handwritten stand-in exception/iterator implementation is
used. This verifies reusable runtime machinery, not yet source Iterable adoption.

The v499 delegation reproducer is terminal (session 52764, exit 101). Restoring
the source argument pipeline removes its extra conversion propagation, exposing
one remaining E0308: an outer constructor with fallible argument evaluation
delegates to an infallible constructor but returns its bare value from a Result
function. The follow-up keeps caller and delegated-callee result ABIs separate.
The return-body helper wraps the delegated value in Ok only when the caller is
fallible and the callee is not; a callee Result is forwarded without nesting it.
The neutral fixture covers both branches plus trailing work and checked failure.

The combined v500 gate passes completely (session 80950, exit 0;
`.tmp/constructor-delegation-applied-sam-v500-regression.log`): all 1,472 library
tests, the expanded constructor callback integration (1), generic interface
construction (3), and source callback contracts (6). Java oracle output also
passes, including nested delegation 69, caught nested failure -3, and delegation
to an already-Result constructor 9. Formatting and diff checks pass. No full
slice run includes these newer fixes yet. The live iterable regression's latest
run remains v497 with the two representation errors; it has not been waived or
claimed fixed by the factory-runtime proof.

Next representation batch: adopt the applied Iterator<T> recipe for the actual
platform Iterable value ABI while retaining source-name precedence, reuse native
SAM source-object adaptation and canonical shared handles, and define native
collection-to-Iterable conversion without losing live allocation identity.
Enhanced-for must preserve checked iterator effects rather than hiding them in
Rust Iterator panics. Captured arrays that remain observable outside a callback
need binding/alias-aware shared storage and handle cloning, not Vec content
cloning. The existing type-wide nominal source placement alone cannot express
that native-array allocation requirement. These are general capability changes;
do not implement a DeferredSequence/JGraphT-specific cast or factory shortcut.

### Native Iterable ABI and checked enhanced-for follow-up

The applied-result recipe now defines the platform `Iterable<T>` SAM as a
lazy `iterator() -> JavaIterator<T>` factory, rather than contiguous `Vec<T>`
storage. Source-name/user-fact precedence is retained. Enhanced-for call-graph
construction and lowering share the selected iterator call, and lowering uses
the checked has-next/next protocol so Java catches can observe its failures.
Fresh native lists can transfer their allocation into the factory; existing
locals, fields, and array-backed views are not silently snapshotted into it.

The focused v501 gate passes all 12 carrier tests
(`.tmp/iterable-factory-focused-v501.log`, session 43360, exit 0). The new runtime
test checks independent cursors, shared removal, concurrent-modification errors,
repeat traversal, exhaustion, and unsupported removal for fixed-size sources.
The existing non-Clone iterator-result proof remains green.

The combined v502 gate is terminal with failures
(`.tmp/iterable-protocol-v502-regression.log`, session 46834, exit 101): 1,472
library tests pass and one stale assertion still expects Iterable to have no
functional shape. Constructor callbacks, production iterator assembly, and all
six source-callback integrations pass. The original live-iterable fixture now
has only two moved-array-capture errors; the invalid interface-to-Vec cast is
gone. Copying the array contents would hide the Java alias and is not a fix.

The new checked-iteration/source-view integration also finds an alias diagnostic
that examines the native facade's placement instead of the captured source
implementor's placement. The follow-up requires native-SAM identity, a parsed
source implementor, a proven subtype edge, and an implemented shared mutable
source handle before accepting that alias. Behavioral verification is required;
this is not a blanket interface-alias exemption. The metadata expectation and
two remaining external-facade Iterable-as-Vec renderers have been corrected.
The v503 gate is terminal (session 82274, exit 101;
`.tmp/iterable-protocol-v503-regression.log`). All 1,473 library tests pass,
as do constructor callbacks, production iterator assembly, and all six source
callback integrations. The checked-iteration fixture gets through diagnostics
and exposes three generated-consumer compile errors: two Rust-harness calls
need the emitted borrowed parameter ABI, and fresh-list adaptation declines a
contextually borrowed producer. The follow-up fixes the harness and uses the
selected fresh-allocation provenance to transfer its owned native carrier,
without extending conversion to aliased locals or array-backed views. The
focused v504 integration batch is terminal (session 13869, exit 101): generic
interface construction (3), native functional objects (2), and retained
callback state (2) pass; only the fresh-list boundary remains. Its follow-up
selects allocation adaptation before contiguous-sequence shortcuts and uses
the selected generic factory's applied native list result with contextual T.

The checked-iteration behavioral fixture passes in v505 (session 35764, exit 0;
`.tmp/iterable-protocol-v505-focused.log`). It proves repeated traversal (14),
live source-field mutations through an Iterable view (7 + 9 = 16), and Java
catch handling for exceptions from iterator creation, hasNext, and next.
The Java oracle independently prints `factory=-7;repeat=14;source=16`.
The v506 combined gate is terminal (session 52694, exit 101;
`.tmp/iterable-protocol-v506-regression.log`). All 1,473 library tests and the
existing constructor/iterator/source-callback gates pass. The original new
behavioral integration also passes. The added standalone factory test exposes
missing JavaIterator support when the Java source never explicitly declares an
Iterator, plus failure to plan fully qualified Arrays.asList.

Both new integrations pass in v507 (session 6677, exit 0;
`.tmp/iterable-support-v507-focused.log`). Runtime selection now follows applied
SAM result metadata, and native collection factories reuse qualified-owner
extraction and native owner/alias/shadowing checks. The native factory's List
result is fully qualified, so the test's co-translated source class named List
does not change its ABI. Additional unit tests cover qualified/aliased/shadowed
factory owners and platform dependencies through nested SAM results. The final
v508 batched gate is terminal (session 38828, exit 101;
`.tmp/iterable-support-v508-regression.log`): both new integrations, constructor
callbacks, production iterator assembly, and all six source-callback tests pass.
Of 1,475 library tests, 1,474 pass. The sole failure is the new negative
namespace test: leaf-name SAM grammar makes `other.Iterable` unnecessarily
request platform support. The follow-up gates that dependency through native
owner membership before projecting its SAM result; it does not change the
source-precedence or factory conversion plans. The v509 focused runtime-support
unit gate passes all 11 tests (session 13717, exit 0;
`.tmp/iterable-support-v509-focused-lib.log`), including the positive and
negative namespace cases. The older source-iterable generated-code regression
passes as v510 (session 48863, exit 0;
`.tmp/iterable-source-loop-v510-focused.log`) with its assertion updated to the
checked protocol; no behavioral check was removed. Formatting and whitespace
checks pass (session 84823, exit 0). All these gates are terminal. No full-corpus
result includes these changes yet; v77 remains the latest full measurement.
The original captured-array alias regression remains open; the active goal is
not complete.

For that next representation step, inspect and generalize existing native
allocation handling before adding another mechanism: ShapeEnv already tracks
shared native-map locals by BindingId and fields by their canonical accesses;
`promote_shared_native_map_local` preserves their allocation, while map receiver
and parameter lowering consume that decision. The array solution must also
propagate its physical carrier through aliases/callback captures and any
cross-call boundaries, use the same allocation for reads and writes, and clone
handles rather than contents. A parallel name-only captured-array special case
would bypass the existing identity and binding machinery.

### Native-allocation shape foundation for captured arrays

The next batch adds `RustSlotShape::native_allocation`, reusing the existing
compositional StoragePlacement (shared/interior-mutable/synchronized) rather
than treating ownership or a slice view as allocation identity. The matching
RequiredNativeAllocation constraint joins placement requirements monotonically
and travels through the existing shape-constraint graph and SCC solver. A late
constraint test proves exact local aliases, source call parameters, and source
returns receive it; unrelated methods and shadowed lexical bindings do not.
No separate array-name scan or array-only fixpoint was added.

Both canonical slot renderers retain a sized native payload behind Rc/RefCell
or Arc/Mutex, with outer nullability and handle borrowing kept independent.
Conversion plans preserve and check the allocation qualifier, including through
already-shaped expressions. A placement change without an identity-preserving
adapter is an explicit diagnostic/compile error, not an implicit Vec snapshot.
Borrowed-handle recovery uses the existing qualified Clone path with a proof
about the handle, not a Clone requirement on array elements. Legacy slice
preferences normalize before conversion selection so they cannot choose to_vec
for a shared handle. Parsed source definitions retain precedence.

v511 and v513 library checks pass. v512 passes three initial focused tests.
v514 was deliberately interrupted after spotting a runtime-test assertion that
could hold two locks on the same allocation. v515 passes five focused tests but
finds a harness error: canonical Java object-array storage correctly uses
Option<Opaque>, while the new Rust consumer constructed/read plain Opaque.
The corrected v516 gate passes all six focused unit tests and the generated
Rust runtime test (session 54690, exit 0;
`.tmp/native-allocation-v516-focused.log`). That runtime test covers all eight
local/synchronized, borrowed/mutably-borrowed, and nullable/non-nullable recovery
combinations, proves pointer identity and shared writes, and deliberately uses
a payload without Clone. It generates types and conversions through production
planners/renderers; it does not manually repair translated Rust.

The v517 batched gate passes completely (session 45671, exit 0;
`.tmp/native-allocation-v517-regression.log`): 1,482 library tests and 12
integrations, including the generated non-Clone allocation proof, checked
Iterable behavior, constructor callbacks, retained callback state, and source
callback contracts. A final source/native ownership-proof guard is newer than
that build; v518 passes against it (session 51168, exit 0;
`.tmp/native-allocation-v518-api-regression.log`): all 119 public coercion tests,
the generated allocation test, and both iterable integrations. The allocation
consumer now also exercises legacy borrowed-slice preferences through canonical
handle conversion. This is a foundation, not a claim
that live_iterable_supplier now passes: automatic capture promotion remains
unwired until allocation construction, read/write projections, and closure
capture-handle materialization agree on this solved property. Do not seed the
property globally and compensate for partial wiring with content copies.
Continue with those lowering paths and their neutral Java behavior regressions;
keep the original live-array oracle unchanged. v77 remains the latest full
corpus measurement, and the combined goal is still active.

Concrete wiring points found during this batch: local initializer conversion
and plan refinement in lower.rs; `lower_read_sequence_receiver` and
`lower_write_sequence_receiver` plus the checked indexed-access plans; and
escaping closure construction around `lower_callback_lambda_expr_with_shapes`
and `rust_closure_with_move`. Reuse storage-placement construction/receiver
projections and the canonical qualified handle-clone conversion. Ensure receiver
guards end before Java callbacks/RHS evaluation can mutate the same allocation.
Capture selection must use lexical binding identity (including shadowing), not
an indiscriminate clone of every read name. Source-call parameters/returns must
consume the solved allocation property, and unsupported native external-call
views need explicit scoped adapters rather than silently losing it.

### Current captured-array gate and renewed bounded optimization

The previously pending v526 run is terminal: it stopped on a moved-value
compile error in the new reference-element projection. The current source
already saves the ownership before consuming that policy. The fresh v527
batched gate passes all 1,482 library tests and 11 integration tests:
captured-array identity (1), unchanged live Iterable supplier (1), native
allocation shapes (1), retained callback state (2), and source callback
contracts (6). Evidence: `.tmp/native-array-v527-regression.log`, session
82574, exit 0. `cargo fmt --check` and `git diff --check` also pass.

Automatic captured-array allocation is now wired through resolved lexical
captures, signature propagation, construction, indexed reads/writes, and
closure handle materialization. Assigned source-call results propagate the
allocation requirement through resolved callable and binding identities.
Observable-alias diagnostics accept the solved matching allocation proof,
not a blanket exemption for arrays. Guarded reference-element reads recover
the stored value before the array guard expires. The unmodified live Iterable
fixture passes; no generated Rust was repaired.

The current Java captured-array oracle agrees with all 12 translated cases:
`[8, 18, 7, 16, 3, -6, 5, 7, 8, 9, -4, -5]`. These cover parameter, returned,
direct and rebound aliases; reference/null elements; writes; lexical shadowing;
nested and renamed captures; and catchable null/negative-size exceptions.
The Java oracle was rerun from the maintained fixture and consumer into
`.tmp/captured-array-identity-java`. This does not establish general callback
re-entry safety, nested-array identity, or support for every native collection
alias; those remain correctness work.

Following the renewed optimization-first instruction, preserve this verified
checkpoint and take a fresh frozen-current profile before performance edits.
v77 remains the latest full-corpus result, not an unchanged-output baseline
for these newer correctness changes. Compare optimization candidates against
the same current code/input, without competing compiler or translator jobs.
Keep the optimization bounded and resume the full JGraphT correctness goal
after recording the measurement and transition decision.

The current v78 release builds in 1m30s (session 23188, exit 0;
`.tmp/optimization-v78-current-build.log`). Frozen binary
`.tmp/new-java2rust-v78-current-optimization-baseline` compares byte-for-byte
with the build and has SHA-256
`93f83fa45afd216cd937fd2427ba44a8fe865a4a40687451f2a6a00e34ac3636`.
Its isolated one-worker representative-file profile analyzes all 104 primary
files and emits only BoyerMyrvoldPlanarityInspector. Session 46542 exits 0:
88.906 seconds total, 44.248 emission, 23.676 signature planning (13.015
constraint collection, 4.671 fixpoint, 5.757 plan building), 11.985 semantic
analysis, and 5.175 representation. This is an instrumented current-code
profile, not an A/B speedup claim or a fresh full-slice correctness result.

Artifacts are `.tmp/jgrapht-boyer-v78-{profile.log,perf.data,perf-flat.txt,phases.tsv,workers.tsv}`
and `.tmp/jgrapht-boyer-v78-profile/`. Perf records 8,762 samples with zero
lost samples. Flat self-time again shows hashing (8.98%), memcmp (7.71%),
Rust type-name rendering (7.59%), cached external name comparisons (5.52%),
and snake-case conversion (4.87%). This run uses frame-pointer call stacks;
many callers are unresolved, so it cannot attribute those costs to particular
call sites with the fidelity of the earlier DWARF profile. Do not invent such
attribution. A bounded candidate rejects unrelated callable headers before
owner matching and reorders external arity/fallback filters without changing
their predicates or candidate order. A differential header-matching matrix
covers direct/alias owners, member names, callable kinds, unknown staticness,
arity and generic arity. Acceptance still requires current A/B measurements
and unchanged output, not merely plausible savings.

The candidate's initial facts gate passes eight tests, including the new
differential matrix (session 42184, exit 0;
`.tmp/query-prefilter-v528-facts.log`). The full library gate then passes all
1,483 tests (session 52906, exit 0; `.tmp/query-prefilter-v529-lib.log`), including
the existing indexed-versus-full-scan overload-naming test. Formatting and
diff checks pass. Candidate release build finishes in 1m54s (session 5887,
exit 0; `.tmp/optimization-v79-prefilter-build.log`). Frozen binary
`.tmp/new-java2rust-v79-query-prefilter` matches the build, SHA-256
`d79755b7cfd3ed975b348fdbcaf5811594d7de37b24b4ec2b1a57683509e9fdd`.

The uninstrumented comparison is now running sequentially in session 15860,
after both compilation and regression jobs terminated. Baseline output is
`.tmp/jgrapht-boyer-v78-control`, candidate output
`.tmp/jgrapht-boyer-v79-candidate`; logs and phase/worker TSVs use the matching
prefixes. Both analyze the same complete 104-file source tree and emit only
the same representative file, with one worker. Do not start competing compiler
or translator workloads while this pair is live, restart it merely because a
poll times out, or accept the optimization before checking its terminal
results, timings and emitted-output equivalence.

The representative-file comparison is terminal (session 15860, exit 0).

| Phase | v78 control | v79 prefilter |
| --- | ---: | ---: |
| Emission | 43.769 s | 35.836 s |
| Total | 87.392 s | 78.588 s |

This one pair measures 18.1% less emission time and 10.1% less total time;
it is not yet a whole-slice speedup or a repeated-run confidence interval.
All generated Rust is byte-identical, with an empty recursive comparison in
`.tmp/jgrapht-boyer-v78-v79-output-diff.log`. The earlier instrumented v78
output also matches the uninstrumented control exactly
(`.tmp/jgrapht-boyer-v78-profile-control-diff.log`).

The full 104-file frozen-v78 baseline is now running separately in
`.tmp/jgrapht-core-1.5.3-focused-v78-allocation-baseline`, log
`.tmp/jgrapht-v78-allocation-baseline-run.log`. It includes the new captured
array/Iterable correctness batch, unlike v77. Only after its terminal result
may the frozen-v79 full comparison start. Preserve script and source inputs
throughout the pair and avoid competing compiler/translator/test workloads.
The new allocation ABI is not accepted for the whole corpus merely because
the neutral callback tests pass; inspect the fresh diagnostics as well as
the complete raw and assembled output comparison.

The full v78 baseline is terminal (session 4582, exit 1 for generated Rust,
not translator failure). All 104 files parse and translate; there are zero
translator failures, 1,928 typed + 36 untyped = 1,964 actual Rust errors, and
40 invalid patterns across 22 files. Compared with v77's 1,993 actual errors,
this newer correctness batch removes 29 net diagnostics, including the 30
invalid LiveIterableWrapper-to-Vec casts. Counts alone do not prove all new
Iterable boundaries work: CollectionUtil now exposes an invalid indexed
List operation on an Iterable carrier, and Supplier<Set<T>> still does not
satisfy Supplier<Iterable<T>> at retained graph-view boundaries.

Baseline timing: resolver preflight 37.331 s, translation 182.154 s (emission
126.087 s), assembly 54.704 s, generated Rust checking 12.112 s, and total
289.704 s. The isolated frozen-v79 full comparison is now running in session
7657, output `.tmp/jgrapht-core-1.5.3-focused-v79-query-prefilter`, log
`.tmp/jgrapht-v79-query-prefilter-run.log`. Do not overlap compiler/translator
jobs or change comparison inputs before its terminal result.

### v79 full comparison accepted; return to correctness

The full candidate is terminal (session 7657, exit 1 for generated Rust).
All 104 files parse/translate, with zero translator failures. Actual Rust
diagnostics remain 1,928 typed + 36 untyped = 1,964; invalid patterns remain
40 across 22 files. Category/count TSVs match the control exactly. Recursive
comparisons of every raw generated module and the complete assembled source
tree are empty in `.tmp/jgrapht-v78-v79-{raw,assembled}-output-diff.log`.

| Phase | v78 control | v79 prefilter |
| --- | ---: | ---: |
| Resolver preflight | 37.331 s | 37.412 s |
| Translation | 182.154 s | 158.709 s |
| Emission, within translation | 126.087 s | 103.796 s |
| Assembly | 54.704 s | 54.827 s |
| Generated Rust checking | 12.112 s | 12.248 s |
| Total | 289.704 s | 266.595 s |

The controlled full pair measures 17.7% less emission time, 12.9% less
translation time, and 8.0% less total time. There is no claimed assembly
speedup. The independent representative-file pair shows the same direction;
neither pair is a repeated-run confidence interval. Candidate library and
differential gates are green as recorded above.

End the renewed bounded optimization pass here. The general predicate-ordering
change provides a verified reduction without adding another cache or mutable
index. Whole-slice iteration is now about 4m27s and remains a batched outer
gate, not an inner-loop test. Further substantial assembly gains still require
a broader mutable-catalog redesign; fixpoint remains a small share of runtime.
Resume the unchanged JGraphT correctness objective immediately. The next
related regression batch should cover collection lvalues retained as Iterable,
supplier-returned collection views, alias-visible iterator removal/fail-fast
behavior, and native collection instanceof/downcast boundaries. Preserve the
existing allocation and lexical identity machinery rather than snapshotting
collections to satisfy the new interface carrier.

### Retained native collection/Iterable regression, after optimization

Added maintained neutral fixture `tests/fixtures/retained_collection_iterable.java`,
Rust integration `tests/retained_collection_iterable.rs`, and Java oracle
`tests/corpus/collection/RetainedCollectionIterableOracle.java`. The seven
cases cover List and LinkedHashSet aliases exposed as Iterable, a retained
supplier returning a collection lvalue, iterator removal visible through the
original List, fail-fast iteration after structural mutation, List downcast,
and the non-List fallback branch. Java passes with
`[20, 7, 23, 14, -7, 4, 6]` under
`.tmp/retained-collection-iterable-java`.

The first translator probe is deliberately still red (session 17812, exit
101; `.tmp/retained-collection-v530-probe.log`). It stops at the diagnostic
assertion: two observable-alias errors correctly reject copying the List and
LinkedHashSet into the Iterable view. Generated Rust has not been manually
patched, and that assertion has not been removed to hide the missing
allocation proof. The earlier green 1,483-library gate predates this new
integration test; it is not a claim that the entire current suite is green.

The implementation must preserve the actual native allocation through the
interface and supplier boundary, not clone its contents or waive alias
diagnostics. Reuse RequiredNativeAllocation/StoragePlacement, declaration IDs,
resolved call-result flow, qualified handle-clone conversion, and existing
live JavaIterator protocol. Structural collection mutation additionally needs
a shared modification counter (array element replacement alone did not).
Iterator.remove must update the original collection and its own expected
counter while invalidating other cursors. Guards must end before callback
evaluation; do not return references into expired guards. Native collection
runtime-type/downcast support must retain an identity-preserving projection,
not reinterpret an Iterable factory as Vec. Wire construction, operations,
iteration and retained projection coherently before enabling new allocation
seeds globally. These are general native-collection capabilities, not named
JGraphT exceptions. The complete pinned 104-file objective remains unchanged.

### Tracked collection runtime foundation (not yet lvalue storage wiring)

The existing native Iterable runtime now uses one `JavaNativeCollection<T>`
allocation for both ordinary collection operations and live iterator factories.
Its private items and shared modification counter distinguish nonstructural
List replacement from insertion/removal. Set operations carry the selected
Java equality witness rather than imposing Rust PartialEq. Fixed-size lists
allow replacement but reject structural mutation. Iterator reads return the
stored reference value before the guard expires; a non-Clone payload behind
Rc proves that neither the collection nor the payload is copied. Existing
`Iterable::from_owned_vec` delegates to this runtime, so this is exercised by
production lowering, not an unused alternate iterator implementation.

Native Iterable carriers retain an optional projection to that same allocation.
Clone and native List recovery preserve Rc pointer identity. An arbitrary
source factory has no native projection; that unknown state must not be
treated as proof of a false instanceof result. This is not general source
class downcast support, sorted-set support, synchronized collection storage,
or complete lvalue-to-Iterable conversion.

The first runtime gate passes (session 34509, exit 0;
`.tmp/retained-collection-v531-runtime.log`). The subsequent foundation batch
passes all 1,483 library tests and 15 integrations, including checked Iterable,
the unchanged live-supplier fixture, captured arrays, non-Clone allocation
shapes, constructor callbacks, retained callback state, source callback
contracts, and production iterator assembly (session 97535, exit 0;
`.tmp/retained-collection-v532-foundation-gate.log`). The seven-case retained
collection integration remains red: no automatic storage seeds or diagnostic
waivers have been enabled for it yet.

Extended the Java oracle with direct ArrayList/LinkedHashSet protocol probes.
Java confirms that hasNext does not perform the next() modCount check: a
second list cursor can report false after the first removes an element, and
an originally empty set cursor remains exhausted after insertion. next still
throws ConcurrentModificationException. remove before successful next throws
IllegalStateException first. The runtime now tracks list cursor/current size
versus set discovered-next state, preserves remove eligibility on failed
operations, and matches those cases. This intentionally corrects the older
unit assertion expecting eager hasNext failure; the maintained Java oracle
provides the independent behavior evidence. The focused v533 runtime test
passes (session 89385, exit 0; `.tmp/retained-collection-v533-protocol.log`).
The related production gate also passes all four integrations: checked
Iterable (2), iterator assembly (1), and unchanged live supplier (1)
(`.tmp/retained-collection-v534-protocol-integration.log`, session 67502,
exit 0). Formatting and diff checks pass after the protocol correction.

Next wire solved native collection payload types, allocation construction,
ordered guarded operations, interface projection/conversion, and protocol
iteration as a coherent batch. New runtime storage must be available whenever
a solved collection slot needs it, not only when the source spells Iterable.
Preserve the array representation and source/native precedence. Then enable
exact binding/call-result allocation seeds and extend alias-diagnostic proof
only for implemented matching carriers. The original seven-case integration,
including supplier returns and List downcast, remains the acceptance gate;
do not replace it with the passing runtime-only test.

### Retained collection wiring checkpoint (v541–v544)

The production path now uses the same native-allocation qualifier for captured
List/Set storage, exact binding-identity alias seeds, sized payload rendering,
fresh construction, ordered guarded operations and native Iterable conversion.
Constructor-anchored local planning must join allocation requirements rather
than discarding them. Native SAM carriers already own their callback handle:
propagated allocation requirements do not introduce a second outer handle.
Alias diagnostics are discharged only for implemented matching storage on the
identified bindings. Parsed source definitions retain precedence.

The v541 focused batch passed captured-array identity, live supplier and native
allocation-shape integrations. The original seven-case collection integration
now gets past the alias diagnostic assertion, but generated Rust still fails
on two errors: Iterable-to-List `instanceof` and the associated cast/get.
Evidence: `.tmp/retained-collection-v541-anchored-storage.log`. This is not a
passing behavioral gate; none of its seven results is claimed verified yet.

Do not implement that cast as a snapshot, substitute iterator traversal for
List.get, or interpret an absent native-allocation projection as proof that an
arbitrary source factory is not a List. Source-defined Lists need a genuine
identity-preserving operation projection using the source dispatch machinery.
That projection must also avoid holding an outer storage guard while invoking
source methods; otherwise reentrant Java calls can fail on Rust cell borrows.
The current native-only `native_list_result` helper is not that general solution.

The next source-preserving projection has a maintained Java oracle too:
`tests/fixtures/source_list_projection.java` and
`tests/corpus/collection/SourceListProjectionOracle.java` produce `47\t28`
under `.tmp/source-list-projection-java`. The source class extends AbstractList,
overrides get/set and deliberately distinguishes get from iterator traversal.
It tests mutation through both the original source and the List cast. The
current local CLI run exits 1 with an observable-alias diagnostic at the source
object-to-Iterable boundary, as well as an emitted unsupported-instanceof marker
(`.tmp/source-list-projection-current.{rs,log}`). Thus the next batch must also
preserve source allocation identity across inherited JDK interface boundaries;
native-only allocation projection cannot close this class of failure. This
oracle is Java evidence and a maintained reproducer, not a passing translated
Rust gate. Do not waive the alias diagnostic.

The v542 allocation-focused gate passes all 11 tests, including materialized
constructor-anchor shapes and the runtime protocol/equality checks
(`.tmp/retained-collection-v542-allocation-units.log`). The retained List search
path had incorrectly selected Set-only equality storage. It now supplies the
existing Java equality witness explicitly for contains, indexOf and object
removal, without a Rust PartialEq bound. Runtime probes check query-to-element
equality direction, first-match removal, absent-value nonmutation and fixed-size
list removal behavior. The maintained independent Java search oracle is
`tests/corpus/collection/RetainedCollectionSearchOracle.java`, paired with
`tests/fixtures/retained_collection_search.java`; it produces `132`.
The first translated probe (v543) exposed two additional general bugs: the
slot-specific get path bypassed retained storage, and Integer.valueOf was
described as Java primitive int, selecting remove(int) instead of remove(Object).
The get path now reuses retained-operation lowering with signed Java indices
and owned result shapes; wrapper factories keep reference return types until
storage lowering. The v545 generated integration compiles and executes with
Java's expected result (`.tmp/retained-collection-v545-search-integration.log`).

The v544 broad library gate finished with 1,485 passing tests and one failure in
the unaliased LinkedHashSet clear regression. Investigation found initialization
was being treated as a retained alias; exact seeds now exclude self-initialization
rather than changing that assertion. A negative signature regression and a
wrapper-return-type matrix were added. The v546 library gate passes all 1,488
tests. The v547 captured-array, live supplier and native-allocation-shape
integrations pass, as does the new search integration. Its original seven-case
probe exposed a missing seed for native collections passed directly to Iterable
parameters once the accidental initialization seed was removed.

That seed now reuses resolved call/constructor parameter contracts and exact
argument binding identities, independent of incomplete external subtype-use
edges. The allocation unit now checks the direct Set argument as well as List
and Set local aliases, and passes. The v549 search integration passes; the
seven-case integration is back to exactly its original two runtime type/cast
errors, not the extra argument-carrier mismatch (see
`.tmp/retained-collection-v549-integrations.log`). The final v548 library gate
including this seed refinement passes all 1,488 tests (232.90 s;
`.tmp/retained-collection-v548-lib-gate.log`). Reentrant user-defined equality, direct enhanced-for on
shared native storage, nullable/synchronized interface boundaries and source
List projections remain unverified; do not generalize the runtime-only evidence
to those cases. The v79 full-corpus totals remain the latest measured totals.

### Inherited native interface identity checkpoint (v550–v553)

The source List oracle exposed two independent prerequisites for a real List
projection. First, concrete source storage planning discovered native SAMs only
on direct inheritance edges. It now also discovers SAM value contracts in
declared fields, signatures and the existing usage binding index, recursively
through applied arguments. The existing interface-value analysis still gates
sharing; an unused AbstractList subclass remains a value. Its implementor walk
now follows both extends and implements, resolves source edges first, and uses
the existing platform subtype model only at external endpoints. It does not
invent a second list of functional-interface names.

Second, direct and indexed hierarchy queries now share a source-aware platform
boundary bridge. Recorded source edges can reach the modeled JDK supertypes of
an external endpoint. Parsed source definitions and explicit user overrides
take precedence; unrelated qualified names are not reduced to JDK leaf aliases.
The override checks reuse the existing type-fact candidate index, and ordinary
source-only queries reject before those boundary checks. AbstractSequentialList
and AbstractCollection are modeled as their actual collection families.
Iterator, Spliterator and Enumeration are no longer incorrectly classified as
Iterable. These are subtype semantics, not a declaration that all their runtime
operations have been implemented. The independent reflection oracle
`tests/corpus/collection/PlatformCollectionHierarchyOracle.java` verifies nine
positive/negative JDK relationships, including those three negative cases.

The first new behavior probe reached generated compilation and exposed a third
gap: slot-specific `iterable.iterator().next()` fusion attempted `.iter()` on
the retained Iterable factory. That path now retains the factory and checked
Java next protocol, reusing the existing slot-result/unboxing machinery rather
than snapshotting or bypassing exceptions.

The maintained fixture `tests/fixtures/inherited_iterable_identity.java` and
Java oracle `tests/corpus/collection/InheritedIterableIdentityOracle.java`
produce `9`; translated Rust now also executes to `9`, observing mutation of the
original source object through its inherited Iterable view. The focused v550
storage gate passes all six identity tests. The v552 gate passes both
`platform_collection_hierarchy` tests (direct/indexed queries, repeated lookup,
namespace/source/user precedence and generated behavior), both checked-Iterable
tests and the live supplier test. Evidence:
`.tmp/retained-collection-v550-source-identity.log` and
`.tmp/retained-collection-v552-source-identity-integration.log`.

Re-emitting the original source-List oracle now exits 0 without observable-alias
errors (`.tmp/source-list-projection-v552.{rs,log}`). A real rustc check still
fails with three errors: the deliberately unsupported member-preserving
instanceof check, get on the Iterable carrier and assigning that carrier to a
Vec at the List cast (`.tmp/source-list-projection-v552-check.log`). None of
these generated artifacts has been manually patched. The v553 broader library
gate passes all 1,489 tests (227.48 s). The final v554 combined gate also passes
all 1,489 library tests (224.30 s) and seven integrations across captured arrays,
checked Iterable, live supplier, native allocation shapes and the new platform
hierarchy suite. It includes the refinement that avoids redundant source
program lookup during native value-contract discovery. Re-emitting the source
List reproducer after that refinement is byte-identical (Rust and diagnostic
log) to v552. The v555 maintained retained-collection search probe still passes;
its original seven-case companion still fails on exactly the two known member-
preserving List type-test/cast errors. Logs:
`.tmp/retained-collection-v554-source-identity-final-gate.log` and
`.tmp/retained-collection-v555-retained-probe.log`. Formatting and diff checks
pass. The full corpus has not been remeasured since v79.

Next representation step: introduce a genuine List operation view capable of
retaining either native allocation storage or source-dispatched operations on
the canonical source handle. Keep native allocation placement separate from
interface view/dispatch. The runtime type witness and cast must travel with the
Iterable facade and preserve the same object in both directions. Source get/set
must call their selected virtual methods, not iterator traversal or a Vec copy.
Do not hold an outer cell guard or mutable callback borrow while invoking Java
source methods; reentrant callbacks need to remain valid. Extend the original
seven-case probe and the source oracle as translated gates before treating
native-only instanceof/downcast as a completed solution.

### List operation-view runtime foundation (v556–v558)

Added a distinct `JavaListView<T>` runtime operation view. It owns the canonical
receiver Rc erased to `Rc<dyn Any>` plus an independently shared operation table;
cloning the view never clones the receiver payload or elements. Allocation
identity is tested with Rc pointer identity, not operation-table identity or
the pre-existing SAM facade PartialEq implementation. Independent projections
of one receiver compare as the same allocation. The initial operation surface
is get, set, size and iterator, all checked; this is not a claim that the full
List interface is implemented.

Source operations are immutable Fn callbacks. They can reenter the same getter
or other operations without a mutable callback-cell borrow; generated source
callbacks must likewise drop source storage guards before calling Java code.
Native views reuse JavaNativeCollection checked operations and its canonical
allocation. The existing iterator protocol implementation was extracted into
`java_native_collection_iterator_result` so both Iterable factories and List
views use the same live cursor implementation instead of reconstructing one
through another interface adapter.

`JavaInterfaceProjection<V>` distinguishes Unknown, Absent and Present. Native
Iterable conversions from collection allocations now carry the corresponding
List metadata. Arbitrary/source factories default to Unknown, not Absent; a
type-test query returns None for that state, and an attempted projection returns
None distinctly from a known negative ClassCastException. Source lowering must
provide the genuine projection before runtime type tests/casts are enabled.
Nullability is still an outer Java reference contract, not the Unknown state.

The focused v556 emitted-runtime test passes. It covers native and source
operation views, independent-view identity, source mutation, get versus iterator
dispatch, checked errors, same-getter reentrancy, reference-only element reads,
and construction/cloning without T: Clone or source payload Clone bounds.
The v557 broad foundation gate adds source-lifetime/no-cycle checks and passes
all 1,489 library tests (233.82 s) plus all eight relevant integrations. Its log
is `.tmp/retained-collection-v557-list-view-foundation-gate.log`. The v558 retained
search integration still passes, while its original seven-case companion still
fails with the same two member-preserving List type-test/cast errors. No
generated Rust or pinned Java was manually patched. Runtime-only source adapter
tests are not a translated source-List behavior gate; production cast planning,
slot view rendering/propagation and source projection generation remain to wire.

A frozen correctness baseline for the next full 104-file measurement is
`.tmp/new-java2rust-v80-retained-collection-views`, SHA-256
`068c4e24a961dc1fba36cbef57940a352cbe2c71e858ea5664d605358a2d7502`.
It was copied without overwriting an existing artifact and compared bytewise
with the built CLI. This is a correctness rebaseline, not another optimization
pass or an output-preserving comparison with v79. After the foundation gate and
retained probe were terminal, the isolated full run started in session 76807,
output `.tmp/jgrapht-core-1.5.3-focused-v80-retained-collection-views`, log
`.tmp/jgrapht-v80-retained-collection-views-run.log`, ten emission workers, with
the normal resolver preflight and offline Rust check. The runner reverified the
untouched pinned commit and inventory. This attempt is now terminal: see the
build-profile correction below. Production List slot/cast/projection wiring
remains next, not another open-ended optimization pass.

### Build-profile correction and producer provenance (v559–v562)

The v80 attempt aborted during parallel emission with a worker stack overflow.
It never reached assembly/checking and has no valid complete diagnostic or
invalid-marker totals. The v79 totals remain the latest complete baseline.
Its preflight took 236.629 s and global analysis 317.808 s, but those are NOT a
valid performance-regression comparison: v80 was accidentally frozen from
`target/debug` (182 MiB, debug info), whereas v79 was a 20 MiB release executable.
Do not interpret that profile mismatch as a sixfold translator regression.

A scoped type-index experiment was tested in frozen debug v81 (five scope tests
and both platform-hierarchy integrations passed), then removed from the working
tree after identifying the measurement confounder. It is not an accepted
optimization. A five-second debug CPU sample mostly observed the existing
source-call thrown-effect/method-effect lookup path, not proof of a new hierarchy
regression. The two slow inline-symbol report processes were stopped; recorded
samples and a successful no-inline report remain under
`.tmp/jgrapht-v81-semantic-perf*`.

The owned gdb diagnostic run used one emission job, which code inspection showed
takes the main-thread path rather than the smaller parallel-worker stacks. It
completed GraphTests, then was deliberately interrupted during the next file;
the captured live stack is not an overflow backtrace and does not prove the
parallel failure fixed. Evidence: `.tmp/jgrapht-v81-crash-probe-gdb-escalated.log`.
The debug parallel-stack failure remains unattributed; check the matching release
configuration before changing recursion or stack policy.

The actual release build is terminal and successful
(`.tmp/retained-collection-v561-release-build.log`, Cargo optimized profile).
Frozen v82 is `.tmp/new-java2rust-v82-list-view-release-baseline`, 20,957,976 bytes,
SHA-256 `e1bdad4bd42d12a80f73fcfad2a656d8dda61ff4f9d5ab0fb75091f3ccf2441e`.
Its untouched 104-file run is active in session 31430, output
`.tmp/jgrapht-core-1.5.3-focused-v82-list-view-release-baseline`, log
`.tmp/jgrapht-v82-list-view-release-baseline-run.log`. Preflight is terminal at
38.396 s, consistent with v79's 37.412 s. The run is now terminal; its completed
baseline is recorded below. No competing compiler/test/translator workloads
were present during this release measurement.

To make producer identity visible even on an aborted run, the focused runner now
records `translation-producer.tsv` before preflight: binary path, content SHA-256,
byte size, build-profile label and the label's source. Default Cargo builds are
labeled runner-release-build; a prebuilt label uses
`NEW_JAVA2RUST_BUILD_PROFILE` and is explicitly caller-declared, not inferred.
Absent prebuilt labels remain unknown. Fresh runs verify the binary did not
change during translation. Resuming assembly loads the original snapshot rather
than relabeling old output with a different marker-counter CLI; legacy outputs
without a snapshot remain unknown. Corrupt snapshots are rejected.

The new `tests/focused_provenance.rs` regressions pass as standalone Rust tests
and will also be picked up by Cargo. It covers snapshot preservation across
binary replacement, replacement detection, a symlink/backslash filename, unknown
legacy metadata and rejected malformed labels/snapshots. The second regression
protects one shared diagnostic-header classifier: summaries/top-error tables
exclude Cargo/rustc aggregate footers and quoted source text, and accept colored
headers. The helper counts 1,987 errors in v82 and 1,964 in v79. v82's immutable
older summary reports 1,988 because it predates this footer-counting correction;
that field is not the actual diagnostic total. Evidence:
`.tmp/retained-collection-v562-provenance-reporting-tests.log`. Shell syntax,
formatting and diff checks pass. No generated Rust or pinned Java was repaired.
The goal and original List-view acceptance gates remain unchanged.

### Completed v82 release correctness baseline

Session 31430 is terminal with exit 1 for generated Rust, not translator failure.
All 104 inputs parse/translate, with zero translator failures and zero generated
parse failures. Actual diagnostics: 1,921 typed + 66 untyped = 1,987. Registry
markers: 65 across 26 files. Against v79's 1,964 diagnostics / 40 markers, typed
errors decrease by seven while explicit unsupported diagnostics increase by 30;
counts are not root-cause group counts or evidence of behavioral correctness.
The largest newly exposed groups are 23 missing native allocation adapters and
seven unsupported retained operations (notably bulk insertion). Examples include
an aliased Set inserted into a map/list whose nested slot still expects a plain
value, and a fresh source factory result assigned to shared collection storage.

| Phase | v82 release |
| --- | ---: |
| Resolver preflight | 38.396 s |
| Translation, including shared analysis | 159.243 s |
| Emission, within translation | 104.592 s |
| Assembly | 55.825 s |
| Rust check | 12.423 s |
| Total | 269.632 s |

This restores a valid practical corpus measurement, not an optimization claim.
The release ten-worker run completed without the debug worker stack overflow;
the latter's cause is still unproven. Next general storage work should reuse or
generalize the existing nested collection-view/child-slot and dependency-flow
machinery so allocation requirements survive container elements, field storage,
factory returns and call boundaries. Add minimized Map<String,Set<Integer>> and
List<List<Integer>> alias probes plus bulk/checked-iteration tests. Keep allocation
identity separate from the List operation view and retain the original seven-case
and source-List cast oracles. No expansion toward 463 production emissions is
justified while the selected slice fails these gates.

### Retained collection factory boundary (v563–v568 focused checks)

The minimized nested-collection probe now isolates three unsupported allocation
transfers, rather than mixing them with a generic factory failure. No generated
Rust or pinned JGraphT Java was repaired. The maintained
`tests/retained_nested_collections.rs` acceptance remains red: Map value, List
element, and instance-field Map value slots still discard the stored collection's
shared allocation requirement. The Java oracle for its four cases produces
63 / 63 / 4 / 63.

Two general boundary fixes are implemented:

- Collection payload adapters must not run `into_iter().collect()` on a retained
  allocation handle. Dispatch planning likewise preserves the shared native
  carrier across supported platform collection upcasts, rather than manufacturing
  `Into<HashSet<_>>`. Semantic-hash/slice preferences describe value payloads, not
  a different physical shared handle. Borrow recovery clones the handle, not its
  elements. Source precedence, matching element types, platform assignability,
  and allocation placement remain required; placement mismatches still fail
  explicitly. Ordinary value-backed carrier conversion remains covered.
- An unresolved platform collection constructor's diamond now retains its
  contextual element/key/value types. Existing selected-call capability inference
  can therefore discover the genuine `JavaMapKey` equality-witness requirement,
  including through generic forwarding. This does not add arbitrary Clone/Eq
  bounds or replace source constructors' hierarchy-aware binder inference. Raw
  constructors remain raw.

Evidence: `.tmp/retained-nested-v567-current-gate.log` passes all 1,490 library
tests, three generic-construction tests, 22 generic-method-binding tests and two
allocation-shape tests. The shape checks cover owned/shared-borrow/mutable-borrow
handles and both placement families. The preceding v566 foundation gate also
passes captured-array identity, checked Iterable, iterator assembly, live Iterable
supplier, and platform/source hierarchy checks. In
`.tmp/retained-nested-v568-acceptance-gate.log`, the independent retained generic
factory regression passes and executes results 4 and 7, including duplicate
suppression and source-method forwarding. The original nested acceptance still
fails on exactly three explicit allocation-adapter markers; the missing generic
equality bound and erroneous handle materialization are gone.

Next: propagate position-qualified child allocation requirements through container
arguments, read results, fields, aliases and source call boundaries. Reuse the
existing child-slot projection and signature dependency machinery, but do not key
allocation identity only by Java type: a Map's equally typed key and value slots
can require different physical placements. Retained bulk operations and List
type-test/cast projection remain subsequent open work. No full corpus measurement
was rerun for this small boundary batch; v82 remains the authoritative 104-file
baseline, not an updated count inferred from these focused tests.

### Position-qualified nested native storage (v569–v579 batch)

The nested-container acceptance now passes through the translator and generated
Rust execution. Its maintained Java oracle has ten results:
63 / 63 / 4 / 63 / 33 / 22 / 33 / 22 / -1 / 22. The cases cover Map values,
List elements, factories, instance fields, aliasing without an Iterable view,
extracted read aliases, a different field-owning source class, deeper nesting,
nullable stored elements, and source method parameter/return boundaries.
`.tmp/retained-nested-v578-call-storage-gate.log` passes both execution tests
(including the separate generic factory's 4 / 7 results) and all four native
allocation-shape/solver tests. No generated Rust or pinned Java was edited.

The physical layout axis now includes position-qualified child allocation
requirements. Optional wrappers are transparent to positions, and equally typed
Map key/value subtrees remain separate. Existing monotone signature requirements,
alias/call propagation, SCC dependencies and child-slot view projection are reused.
Usage records exact declaration IDs, canonical source field owners and source
return boundaries. Stored Java native references retain identity even without an
Iterable conversion. Path transfer is bounded by the actual target Java type tree,
not arbitrary type-variable names or an ever-growing recursive path. The solver
test covers disjoint same-named locals, an unaffected sibling binding, cross-owner
fields, constructors, returned elements, raw recursive storage and parsed source
lookalikes.

Rendering overlays child allocation layouts on canonical Rust payloads, preserving
existing nullability, source dispatch and equality carriers. Checked List reads
materialize the stored handle in both propagating and non-propagating contexts;
the Rust lookup's absent-index Option is not confused with Java null stored in a
valid element slot. Slot lowering and materialized-result metadata reuse that
same child projection. A mutable native Java List contract retains its Vec behind
the borrow because a mutable slice cannot support structural add/remove/clear;
read-only views and fixed-size arrays retain slice support.

The new usage queries reject scalar/non-storage operations before enriching
expression types. Field lookup reuses existing source symbols instead of rebuilding
external-member lookup indexes inside nested queries. Broader regression results
and a fresh frozen release corpus measurement are pending below; the unchanged
v82 measurement remains the authoritative full-slice diagnostic count for now.
The v579 established gate passes all 1,491 library tests and 39 integration tests
across native allocation shapes, nested storage/factories, captured arrays,
checked Iterable, generic construction/binding, iterator assembly, live suppliers,
source/platform hierarchy and retained collection search. Its only failing test
is the previously red seven-case retained-Iterable acceptance, still reporting
the same two List type-test/get projection errors. No new gate failure is being
treated as a baseline exclusion. The mutable List parameter regression also
passes generated execution through the source store/read helper methods.
Retained bulk operations and the original List type-test/cast acceptance remain
open. This focused acceptance does not establish completion of the JGraphT slice.

### v83 full-slice measurement: nested storage and follow-up field projection

Frozen release producer: `.tmp/new-java2rust-v83-nested-native-storage`,
20,770,056 bytes, SHA-256
`aeca87222e48924c13d0129f7b679f35376a48fbc0e064ddc04606cbd6001e1e`.
The default pinned runner used ten workers with no competing compiler/translator
workload. Output is
`.tmp/jgrapht-core-1.5.3-focused-v83-nested-native-storage`, with run log
`.tmp/jgrapht-v83-nested-native-storage-run.log`. The producer manifest records
release/caller-declared provenance; source pin, inventory and bytewise staging
checks pass. All 104 files translate, with zero translator failures and zero
generated Rust parse failures. Assembled cargo check fails: 2,079 actual diagnostic
headers (1,986 typed + 93 untyped), 86 invalid markers across 30 files.

| Phase | v82 release | v83 release |
| --- | ---: | ---: |
| Resolver preflight | 38.396 s | 39.413 s |
| Translation including shared analysis | 159.243 s | 166.436 s |
| Emission within translation | 104.592 s | 107.636 s |
| Shared analysis within translation | 42.069 s | 45.471 s |
| Signature fixpoint within analysis | 4.784 s | 4.965 s |
| Assembly | 55.825 s | 56.736 s |
| Rust check | 12.423 s | 12.831 s |
| Total | 269.632 s | 279.300 s |

The total is 3.6% slower, still practical for a substantial batch gate. No renewed
optimization round is justified by these costs. Signature solving reaches its
fixpoint in 13 iterations with zero saturation diagnostics.

The diagnostic increase is real, not a successful corpus gate: +65 typed and
+27 untyped diagnostics. The untyped delta consists of additional allocation
boundaries (43 versus 23) and retained operations (14 versus seven). A concrete
new typed-error group points to source generic fields: the existing
`java_semantic_hash_collection_field_rust_type` rewrites any outer generic type
as a semantic map/set. Once fields correctly acquire shared native allocation
storage, it rewrites `Rc<RefCell<JavaNativeCollection<T>>>` into a set of cell
payloads, discarding the physical handle. There are 38 Clone diagnostics for
`RefCell<JavaNativeCollection<Option<E>>>`, accompanied by invalid borrow calls.
The follow-up must preserve wrappers and the runtime's existing equality witness;
adding Clone to RefCell/payloads or accepting these errors is not a solution.
The frozen v83 baseline predates that follow-up; its results remain immutable.

The working-tree follow-up preserves Rc/Arc and cell/lock wrappers when adapting
semantic collection field types. `JavaNativeCollection` already owns its Java
equality witness and is not rewritten into another set. Field initialization and
assignment now retain the solved shared allocation rather than applying a
value-level `From` adapter to the handle. Reading such a field into an owned slot
uses the handle's unconditional Clone capability, independently of the generic
element's Clone capability; parsed source carriers and unrelated interface
adapters are not intercepted.

The minimized generic field fixture and Java oracle are maintained as
`tests/fixtures/retained_generic_field.java` and
`tests/corpus/collection/RetainedGenericFieldOracle.java`. Both Java and generated
Rust produce 22: a stored map alias continues to reference the old set after
field replacement, while the holder and replacement local share the new set.
The generic implementation compiles without adding a T: Clone bound. Focused
gates v583/v584 pass all allocation-shape checks and three execution tests,
including the ten-case nested-container oracle and source generic factories.
The completed broader gate `.tmp/retained-nested-v585-final-batch-gate.log` passes
all 1,492 library tests and 40 integration tests. The only failing test is still
the existing retained-Iterable List type-test/get projection acceptance, with
the same two errors and no newly excluded regression. Formatting and diff checks
pass. No new full-slice count has been inferred from this follow-up. Next batch:
remaining physical boundaries for collection factories/views and retained bulk
operations, before the next broad measurement; List cast/view wiring remains
open as well.

### Retained native bulk insertion follow-up

The native bulk planner now snapshots native collection sources before taking
a retained destination's mutable cell/lock guard. This supports distinct aliases
of the same allocation for `addAll`, ordinary-to-retained insertion, and
retained-to-ordinary insertion without converting the collection identity.
Receiver and argument expressions are staged before null checks. Set insertion
reuses the allocation's existing Java equality witness; no blanket payload Clone
or PartialEq bound is added to the runtime.

Empty insertion has a construction-time list policy: ArrayList invalidates its
cursor even when `addAll` returns false, whereas LinkedList does not. The policy
survives an erased List alias. Fixed-size native lists accept empty insertion but
reject nonempty insertion before mutation. This does not implement arbitrary
source Collection callbacks, retained bulk removal/membership operations,
unmodifiable views, or List casts.

The maintained neutral fixture is `tests/fixtures/retained_bulk_collections.java`;
its Java oracle `tests/corpus/collection/RetainedBulkOracle.java` produces
19, 1, 11, 101, and 101 for alias/equality behavior, null-source behavior,
ArrayList-versus-LinkedList cursor invalidation, and receiver-field replacement
during argument evaluation through different-object and same-object field
reads respectively. Earlier focused
gates v586/v587 pass aliasing, both storage directions, null-source behavior,
all three preceding nested/generic execution tests, and the existing bulk
operand-order suite. The expanded v588 gate passes the first three oracle
cases. The direct runtime policy check v591 passes, including non-Clone
payloads, fixed-size empty/nonempty insertions, and both list cursor policies.
The final focused gate v594 passes all ten tests: the existing bulk operand-order
suite, five allocation-shape/solver checks, and four generated execution tests.
The full library gate v595 passes all 1,492 tests (zero failures or ignored
tests). Formatting and diff checks pass. No new full-corpus count has been claimed.

The initial receiver-replacement probe (v590) also read the field through a
different object: `Iterable<Integer> old = holder.stored`. That exposed a
retention-inference gap: the field remained a Vec and the Iterable assignment
failed. The same-object variant (v592) exposed the same gap. The follow-up now
records native allocation retention demands for Iterable local initializers
using the existing canonical binding, declaring-field and resolved method-return
origins. Unary demands become self-edges in the existing monotone storage graph,
reusing its dependency and propagation rules rather than adding a field-only
solver. The v594 execution probe covers both field forms, and its solver
regression separately checks method results and an unrelated unretained field.

### Frozen v84 full-slice follow-up

Frozen release `.tmp/new-java2rust-v84-retained-bulk-origins` has SHA-256
`87110f04ca56dd512567020b7a46d8c9572846dc9b9ca177fa5435ecf2525114`
and size 20,748,376 bytes. The release build log is
`.tmp/retained-bulk-v596-release-build.log`; it finished in 1m28s.
The frozen copy was byte-compared with the build output before use.

The unchanged pinned 104-file runner wrote
`.tmp/jgrapht-core-1.5.3-focused-v84-retained-bulk-origins`; its run log is
`.tmp/jgrapht-v84-retained-bulk-origins-run.log`. It terminated with the expected
generated-Rust failure (exit 1), not a translator failure. The runner checked
the source pin, clean tracked checkout, 463-file inventory and byte-for-byte
104-file staging. Producer provenance records the binary hash, size and release
label. No competing compiler/translator workload ran during this measurement.

| Metric | v83 | v84 |
| --- | ---: | ---: |
| Translator failures / generated parse failures | 0 / 0 | 0 / 0 |
| Actual Rust diagnostics | 2,079 | 1,987 |
| Typed / untyped diagnostics | 1,986 / 93 | 1,909 / 78 |
| Invalid markers / files | 86 / 30 | 71 / 29 |
| Preflight seconds | 39.413 | 39.932 |
| Translation seconds | 166.436 | 159.595 |
| Emission seconds (nested) | 107.636 | 103.834 |
| Shared analysis seconds (nested) | 45.471 | 43.160 |
| Signature fixpoint seconds (nested) | 4.965 | 4.786 |
| Assembly seconds | 56.736 | 56.401 |
| Rust check seconds | 12.831 | 12.767 |
| Total seconds | 279.300 | 272.559 |

Canonical error-header counting excludes Cargo/rustc aggregate footers. Signature
solving reports zero diagnostics. The 43 allocation-adapter errors remain
(27 shared-to-plain, 16 plain-to-shared). One concrete source is override ABI
inconsistency: the override join propagates nullability but not native storage
qualifiers, so a trait can return a value-backed set while an implementation
returns an Rc-backed allocation. That is the next general correction; immutable
factory and unmodifiable-view policy are separate remaining boundaries. v84
predates all edits in the override batch and must not be used to claim their
corpus impact.

### Native allocation ABI across source overrides

The neutral fixture `tests/fixtures/retained_override_storage.java` reproduces
the missing override contract: one implementation returns a collection read
from a nested collection and stores a collection parameter, while another uses
an ordinary field and only mutates its parameter. The pre-fix metadata gate
v597 fails at the interface's return allocation. This is not a baseline
exclusion or an adapter allowed to snapshot the Java allocation.

The working-tree correction reuses the existing source override topology and
monotone slot join for root and nested native allocation qualifiers. Eligible
paths are precomputed once from declared native collection/array slots with
parsed-source precedence; arbitrary erased type parameters and unrelated
implementations do not acquire wrappers from another implementation's storage.
Both contributors and destinations are checked against those paths. Ordinary
ownership/view preferences remain local, and context-only imports still merge
only the established nullability information.

The execution probe exposed two adjacent boundary failures (v599), now corrected:
nullable borrowed native references are captured by cloning the handle inside
the Option, not by trying to clone the Option as a borrowed handle; empty and
capacity constructors for nullable native fields use the canonical payload slot
and preserve nested allocation qualifiers. Shared roots still go through the
existing retained-allocation construction path. Bulk and ordinary native calls
reuse the same capture helper; reborrowing nullable mutable references prevents
capture from consuming a reusable parameter.

The Java oracle `tests/corpus/collection/RetainedOverrideStorageOracle.java`
produces 21. Generated Rust passes the same behavior check through interface
dispatch. v598 passes all six then-current allocation checks; v600 passes 13
focused tests, and the expanded v601 gate passes 41 integration tests. New
checks cover root return/parameter unification, nested value-only paths without
retaining the outer map, and unrelated generic implementations. The final
reborrow gate `.tmp/retained-override-v603-reborrow-batch.log` passes all 41
integration tests; `.tmp/retained-override-v604-lib.log` passes all 1,492 library
tests with zero failures or ignored tests. Formatting and diff checks pass.
The preceding v602 library gate also passed before the final reborrow adjustment.
All these edits postdate
the frozen v84 measurement; their full-slice impact remains unmeasured.

### Owned immutable factory and retained-element boundary batch

The working-tree follow-up implements retained `Collections.emptyList`,
`emptySet`, `singletonList` and `singleton` through the selected native static
call plan. It constructs the canonical allocation directly; it does not copy a
backing collection or apply immutability to another Java reference. The runtime
now has explicit Mutable, FixedSize, ImmutableEmpty and ImmutableSingleton
mutation policies. Empty no-ops, absent removal, singleton replacement/addition,
and empty-versus-singleton iterator remove ordering remain distinct.

Factory construction does not introduce an equality or payload Clone bound.
Set queries supply the existing Java equality capability at the query boundary;
an allocation's selected equality witness still takes precedence. Generic
stored arguments are classified by the same selected static-factory operand
metadata used for element storage. `RetainedByCall` feeds the existing escaping
storage and object-model policies rather than pretending a retained argument is
only borrowed content. Factory elements reuse ordinary argument move recovery;
the native handle's unconditional Clone proof is independent of its element's
Clone capability.

Factory results used in local initialization, return, and assignment receive a
runtime allocation requirement so their immutability is not erased even when
the reference is unique. Nested collection arguments reuse the existing exact
binding/field/return storage graph. Native Iterable generic arguments now retain
position-qualified child allocation layouts. Enhanced-for records child-to-loop
binding flows before the loop declaration enters scope, and lowering preserves
the exact solved binding's physical qualifiers while choosing owned/borrowed
iteration access. This repairs the solver-to-lowering boundary rather than
changing generated Rust.

Retained native mutations can throw UnsupportedOperationException, and checked
list indices can throw IndexOutOfBoundsException. Those effects now reuse the
typed immediate-effect walker used by checked casts, its catch hierarchy and
the ordinary call/override fallibility closure. This prevents a source helper
from panicking instead of propagating a catchable Java exception. A cheap native
method-name prefilter avoids constructing typed environments for unrelated
callables; actual selection and solved native storage remain authoritative.

The neutral fixture is `tests/fixtures/retained_immutable_factories.java`, with
Java oracle `tests/corpus/collection/RetainedImmutableFactoriesOracle.java`.
Current Java output is 111111, 1111111, 44, 1, 1, 11, 11: immutable set/list
behavior, nested aliases through factory/Iterable/enhanced-for boundaries,
unique-reference policy, null elements, catch boundaries and generic handle
reuse. v608 passes the original two behavior cases; v612 separately proves the
loop binding was solved as shared before fixing its lowering. v615 passes
16 focused integration tests, including all seven Java/Rust oracle cases.
The strengthened runtime probe also uses a payload without Clone or PartialEq.

The broad v616 gate completed: all 1,492 library tests and 49 integration tests
pass. Its sole failure is the established retained-Iterable List type-test/get
acceptance, with the same two errors. A subsequent provenance probe v618 found
that usage analysis could classify a parsed source class named Collections as
the platform factory. The shared native static-call planner now receives the
existing FactSet source-owner proof; lowering, usage and capability analysis
use the same planner. The capability-only display walk already rejects source
owners from its support programs before calling it.

The v621 gate passes 19 selected tests, including ten allocation/provenance
checks and seven generated execution tests. Source-defined and fully qualified
JDK factories coexist in one program and retain distinct mutation policies;
a local named Collections does not acquire a platform retention contract.
The final broad source-state gate
`.tmp/retained-immutable-v622-final-source-gate.log` passes all 1,492 library
tests and 51 integration tests. Its only failure remains
`retained_collection_iterables_keep_aliases_protocol_and_runtime_type`, with
the same unsupported List type test and missing Iterable.get errors. Formatting
and diff checks pass. The accumulated override/factory batch is measured in v85
below; no isolated attribution between those changes is claimed. Backed
unmodifiable views, immutable map/nCopies/sorted-set factory
families, factory identity behavior, declaration initializers and direct
temporary-receiver paths still require their own implementation/verification;
the present probes do not prove those forms.

### Frozen v85 full-slice measurement

The frozen release is `.tmp/new-java2rust-v85-owned-immutable-factories`,
SHA-256 `c106cb8cc4f2afd7b2ddaad654bbe979ff61590053de739307692c5c93438c4a`,
20,864,432 bytes. Release build log
`.tmp/retained-immutable-v623-release-build.log` records 1m28s. The frozen copy
was byte-compared with the build output; its hash was checked again after the run.

The output is `.tmp/jgrapht-core-1.5.3-focused-v85-owned-immutable-factories`,
with run log `.tmp/jgrapht-v85-owned-immutable-factories-run.log`. The unchanged
104-file runner verified the clean tracked pin, 463-file source inventory and
byte-for-byte focused staging. No competing compiler or translator workload ran
during timing. Exit 1 is the generated-crate check failure, not a translator
failure. Producer provenance records the same hash, size and release label.

| Metric | v84 | v85 |
| --- | ---: | ---: |
| Translator failures / generated parse failures | 0 / 0 | 0 / 0 |
| Actual Rust diagnostics | 1,987 | 1,964 |
| Typed / untyped diagnostics | 1,909 / 78 | 1,877 / 87 |
| Invalid markers / files | 71 / 29 | 85 / 44 |
| Preflight seconds | 39.932 | 39.146 |
| Translation seconds | 159.595 | 162.933 |
| Emission seconds (nested) | 103.834 | 106.332 |
| Shared analysis seconds (nested) | 43.160 | 43.702 |
| Signature fixpoint seconds (nested) | 4.786 | 4.861 |
| Assembly seconds | 56.401 | 57.627 |
| Rust check seconds | 12.767 | 12.923 |
| Total seconds | 272.559 | 276.576 |

Canonical diagnostic counting excludes aggregate compiler/Cargo footers.
Signature saturation completes in 13 iterations with zero diagnostics. The
small timing increase does not reopen the bounded optimization phase.

The result removes 32 typed diagnostics but adds nine untyped ones. Allocation
adapters change from 27 shared-to-plain plus 16 plain-to-shared to 43 plus six.
The stronger return/override/factory contracts therefore expose additional
boundaries rather than just reducing the old marker count. There is also one
unsupported retained operation in GraphWalk's subList/addAll path, which needs
view-preserving semantics rather than a copied Vec.

The next directly evidenced group is retained native enhanced-for iteration:
27 diagnostics still call `.iter()` on `&Rc<RefCell<JavaNativeCollection<Option<E>>>>`.
The Iterable child-layout and loop-binding fixes do not by themselves choose
the checked cursor for an ordinary Set/List receiver. Reuse the explicit
retained iterator's checked protocol and keep generic capability inference,
exception planning and lowering consistent. Then address nullable retained
Iterable adapters and backed unmodifiable/subList views without freezing the
source reference or snapshotting its data. The selected slice still fails its
compile/behavior/completion gates; emission scope remains 104 sources.

### Retained native enhanced-for batch (after v85)

Direct enhanced-for over retained native List/Set storage now selects the same
checked live cursor as explicit native iteration, before the old value-backed
mutation heuristic. Iterator creation captures the allocation handle once;
replacing a field reference inside the loop does not replace its cursor, and
mutating that allocation through a helper still checks the change counter.
Arrays retain their separate indexed-loop semantics. Source owners remain
guarded by the common effective-facts classifier.

Lowering, implicit exception effects and generic capabilities share the solved
retained-iteration classification. Exception inference reuses the existing
typed, catch-aware expression-effect walker. Generic bounds inspect the actual
stored element carrier: copying a nested Rc handle does not require its payload
type parameter to implement Clone. Source forwarders reuse the existing
capability propagation rather than inventing a second call-graph pass.

The neutral execution probe exposed two other general boundaries: early returns
inside checked while-let/for loops nested in try/catch were missed by the
optional-return rewrite, and known non-null factory initializers could discard
a binding's later-null storage requirement. Both paths now preserve their
independently solved contracts. The nullability metadata regression runs with
semantic analysis and built-in library facts, not only resolved source facts.

Preserving explicit nullable storage also changes the modeled Map.Entry setter's
old-value local from String to Option<String>. The old string-only assertion
assumed the narrower carrier. An execution regression, rather than relaxing the
assertion alone, exposed an existing owned-tuple bug: the setter dereferenced an
owned field as if it came from a borrowed iterator entry. The common setter now
selects the actual tuple place, stages the replacement before borrowing it, and
uses mem::replace to return the old payload without a Clone requirement. The
separate Java oracle and translated Rust both print `1,a=2`.

The untouched Java iteration oracle prints `3, 5, 7, 5, 31`; translated Rust
matches it and verifies generic direct/forwarded iteration, including a nested
collection with a non-Clone payload. Focused log
`.tmp/retained-iteration-v630-focused.log` records 11 allocation-shape tests and
eight collection execution tests passing. The final focused follow-up
`.tmp/retained-iteration-v634-focused.log` passes all 20 tests, including the
setter execution regression. The focused map-entry library gate
`.tmp/retained-iteration-v635-entry-unit.log` passes 49 tests. Final source gate
`.tmp/retained-iteration-v636-source-gate.log` passes all 1,492 library tests and
54 integration tests. Its sole failure is the established retained-Iterable
List type-test/get case, with the same two generated-crate errors; no new gate
failure is excluded. Formatting and diff checks pass. The earlier v631/v632
attempts exposed the old setter assertion and are superseded by the execution-
verified correction and this final gate.

The full-slice follow-up is measured in v86 below.

### Frozen v86 full-slice measurement

The frozen release `.tmp/new-java2rust-v86-retained-native-iteration` has SHA-256
`5e91c48c6ad6bdb65b44dc5596d20204e0d6d1b8d3af1f8f0ab10d0860c0a74c`
and 20,867,872 bytes. Release build log
`.tmp/retained-iteration-v637-release-build.log` records 2m02s. The frozen copy
was byte-compared with the build output and its hash verified after the run.
Output is `.tmp/jgrapht-core-1.5.3-focused-v86-retained-native-iteration`; run log
is `.tmp/jgrapht-v86-retained-native-iteration-run.log`. Producer provenance
records the same hash, size and release label. The runner verified the clean
tracked Java pin, 463-file inventory and byte-compared 104-file staging. No
competing compiler or translator ran during timing. Exit 1 is the expected
generated-crate check failure, not a translator failure.

| Metric | v85 | v86 |
| --- | ---: | ---: |
| Translator failures / generated parse failures | 0 / 0 | 0 / 0 |
| Actual Rust diagnostics | 1,964 | 1,947 |
| Typed / untyped diagnostics | 1,877 / 87 | 1,860 / 87 |
| Invalid markers / files | 85 / 44 | 85 / 44 |
| Retained native collection missing `.iter()` diagnostics | 55 | 8 |
| Preflight seconds | 39.146 | 39.159 |
| Translation seconds | 162.933 | 164.681 |
| Emission seconds (nested) | 106.332 | 107.572 |
| Shared analysis seconds (nested) | 43.702 | 43.974 |
| Signature fixpoint seconds (nested) | 4.861 | 4.820 |
| Assembly seconds | 57.627 | 56.066 |
| Rust check seconds | 12.923 | 12.754 |
| Total seconds | 276.576 | 276.560 |

Counts use canonical diagnostic headers, excluding aggregate footers.
Signature saturation remains 13 iterations with zero signature diagnostics.
Runtime is effectively unchanged; the bounded optimization phase stays closed.

The 27 formerly prominent missing-iter errors were one carrier-specific subset,
not the total: all retained native collection types together had 55. The eight
remaining sites are three stream match terminals in BergeGraphInspector, one
removeAll operand, two array-element loop sources in BellmanFordShortestPath,
and two stream consumers in AsSubgraph. Inspection of the pinned Java confirms
that the boolean match cases are actual stream calls, not rewritten loop idioms.
They need the same checked iteration and child-layout authority,
not collection snapshots or emitted-code patches.

Removing opaque iterator failures exposes further binding problems: 16 new
`.as_ref()` errors on generic V, more Option<&V> comparisons and some missing
Clone/lifetime bounds. Cursor-owned values and the loop binding's solved
nullable carrier must be adapted consistently; do not treat the net reduction
as proof that this boundary is finished. These and the eight bypasses form the
next coherent iteration batch. The 49 allocation adapters and the backed
subList operation remain unchanged. The selected slice still fails compilation,
so its Rust behavior oracle and expansion gates remain unmet.

### Checked cursor binding and indexed storage follow-up

The cursor lowering boundary now carries a RustSlotShape rather than just a
Java element spelling, including nested native-allocation qualifiers. It uses
the ordinary slot conversion planner to materialize a declaration's nullable
binding carrier when the cursor yields a non-null owned value. Equality and
other consumers therefore see the same carrier that is actually emitted,
rather than recovering Option<T> from the callable plan around a bare T.
Primitive loop variables instead select their primitive slot and use the
existing required-Java-value check for nullable unboxing; its NPE participates
in the typed, catch-aware iterator effect analysis. The shared required-value
helper was renamed to reflect its already-general uses.

Indexed read and assignment projections now retain child allocation paths.
Array assignment lowering reuses that solved projection instead of rebuilding
an unqualified slot from the Java element type. Borrowed nullable handle capture
uses explicit Rc/Arc cloning after as_ref: this accepts both Option<&Handle>
parameter views and &Option<Handle> indexed reads without cloning RefCell or
its payload. No copied-collection identity adapter or payload Clone impl was
introduced.

The maintained Java oracle now prints `3, 5, 7, 5, 31, 4, 6, 7, 8, 1`, including
array-selected collections, catchable null unboxing, nullable reference
elements and generic non-null iteration. Rust matches it. Metadata explicitly
proves the generic collection stores T while its loop declaration requires
Option<T>, and checks indexed child qualifiers through nullable and non-null
arrays with both Rc and Arc placements. Focused
`.tmp/retained-iteration-v642-null-elements.log` passes 21 tests. Broad
`.tmp/retained-iteration-v643-source-gate.log` passes 1,492 library tests and
55 integration tests, with only the unchanged retained-Iterable List cast/get
failure and its same two compiler errors.
The final test-only extension `.tmp/retained-iteration-v648-unboxing-effect.log`
again passes all 21 focused tests. It additionally catches mutation/CME/NSE
effects in a callee while leaving only null unboxing uncaught, then verifies
that the source caller catches that NPE. No product source changed after the
broad gate; this proves the new unboxing effect independently of unrelated
fallibility seeds.

An additional array/coercion subset initially could not compile because an
existing test matched Vec::new() as a pattern. Replacing that with a bound
vector plus is_empty guard preserves its assertion. Follow-up
`.tmp/retained-iteration-v645-array-gate.log` passes 22 tests but finds six
slot-shape failures. These are verified pre-existing on v86, not silently
attributed: `.tmp/retained-iteration-v646-array-control.log` runs the same test
source against the retained v86 release library and reproduces the exact same
six failures (17 passes). The control executable is
`.tmp/slot-shape-v86-array-control`, SHA-256
`34a654a37600c2a05ff04d5c3698853e5035c70aeed451a40d5b24b5ac30b09c`.
Its library is archived as `.tmp/libnew_java2rust-v86-array-control.rlib`, SHA-256
`6ec8c2b48015f3489ff2c488a6228fe70d6dcbed3b7731f2b35b66f58dca8ecc`.
Before candidate rebuilding, the release executable paired with that library
was byte-compared with frozen v86. The control was built with rustc --test,
edition 2024, that rlib as --extern new_java2rust, and target/release/deps as
the dependency search path; Cargo environment macros point to this repository
and the frozen v86 executable.

The six open failures are:

- `slot_shape_projects_arrays_and_primitives_without_type_specific_ownership_variants`
- `stream_plan_accepts_arrays_stream_as_source`
- `stream_plan_accepts_arrays_stream_range_as_source`
- `stream_plan_lowers_arrays_stream_to_array_and_all_match_terminals`
- `rectangular_object_array_initializer_keeps_nested_vec_carrier`
- `external_conversion_adapter_wraps_array_initializer_elements`

They remain visible open failures, not a claim that the expanded gate is green.
The remaining retained stream sources also need a genuine fallible traversal
integration: adapting JavaIterator through its panicking Rust Iterator API
would lose catchable Java exceptions and is not an acceptable substitute.

### Frozen v87 full-slice measurement

The frozen release `.tmp/new-java2rust-v87-cursor-binding-array-layout` has
SHA-256 `a379db657ca85a097809f4f48905d37e3d4bf2e3bdacee6ed6fed0737c23644e`
and 20,866,904 bytes. Release build log
`.tmp/retained-iteration-v647-release-build.log` records 1m28s. Its frozen copy
was byte-compared with the release output and rehashed after the run. Output is
`.tmp/jgrapht-core-1.5.3-focused-v87-cursor-binding-array-layout`, with log
`.tmp/jgrapht-v87-cursor-binding-array-layout-run.log`. The clean tracked pin,
463-source inventory and 104 byte-compared staging files are unchanged.
Producer metadata agrees with the frozen hash, size and release label. No
other compiler or translator ran during timing. Exit 1 is generated Rust
checking, not translator or generated-parser failure.

| Metric | v86 | v87 |
| --- | ---: | ---: |
| Translator failures / generated parse failures | 0 / 0 | 0 / 0 |
| Actual Rust diagnostics | 1,947 | 1,937 |
| Typed / untyped diagnostics | 1,860 / 87 | 1,850 / 87 |
| Invalid markers / files | 85 / 44 | 85 / 44 |
| Retained collection missing `.iter()` diagnostics | 8 | 6 |
| Invalid generic V `.as_ref()` diagnostics | 16 | 0 |
| Preflight seconds | 39.159 | 38.754 |
| Translation seconds | 164.681 | 161.550 |
| Emission seconds (nested) | 107.572 | 105.127 |
| Shared analysis seconds (nested) | 43.974 | 43.398 |
| Signature fixpoint seconds (nested) | 4.820 | 4.993 |
| Assembly seconds | 56.066 | 56.358 |
| Rust check seconds | 12.754 | 12.834 |
| Total seconds | 276.560 | 273.361 |

Canonical diagnostic counts exclude aggregate footers. Signature saturation is
still 13 iterations with zero signature diagnostics. Timing remains practical;
the small variation does not reopen optimization.

The remaining six missing-iter sites are the five stream terminals and one
removeAll operand. The array-element loops now select the checked cursor and
no longer call insert on a borrowed Rc. Mismatched-type diagnostics fall by
four. This is still a mixed boundary checkpoint: nine more Option<&V> equality
obligations and four additional existing-style `.expect()` calls on V are
visible. The latter are source-call return-carrier disagreements, notably
non-optional getOppositeVertex results being treated as optional at uses.
Reconcile the general source signature/call result authority rather than
patching that method name or the emitted Rust. The 49 allocation adapters
(43 shared-to-plain, six plain-to-shared) are unchanged, as is the backed
subList operation. The selected slice still fails compilation; Rust graph
behavior and expansion gates remain unmet.

### Source-call result contract batch (after v87)

The generic argument-result shortcut previously used the argument's outer
nullable ABI as the result type. A source method accepting Option<T> and
returning T therefore acquired an expect() on its already non-optional result.
The minimized neutral control `.tmp/source-result-v651-reproduced.log` fails
with exactly that error on generic T and String. Its always-throwing generic
callee isolates the signature disagreement without relying on unsupported
non-null guard inference or on any corpus name.

The shortcut now infers the substitution for T separately from the selected
parameter and return wrappers. It strips a nullable parameter ABI when present,
recovers bounded raw collection arguments with the existing whole-shape
specialization helper, then applies the return ABI. It also preserves the
return ownership rather than claiming that an owned result is an argument
borrow. Physical native-allocation and nested-view metadata survive enrichment.

The shared planned/external result selector now gives emitted source contracts
priority in both nullability directions, using the existing emission inventory.
Receiver-only fallback additionally requires an actual declared source method,
so merely inheriting an external method does not manufacture a source ABI.
Declaration-only summaries do not activate the new source shortcut. Generic
specialization and ordinary argument, local, return and receiver consumers
reuse the same authority; no corpus-specific return-name rule was added.

The neutral fixture and Java oracle
`tests/corpus/construction/SourceResultContractOracle.java` print
`ok:null:chosen:caught:null:0`. Tests check both Option<T> parameter -> T return
and Option<T> -> Option<T>, forwarding, null results, caught exceptions, raw
bounded maps, and external summaries contradicting emitted source in both
directions. `.tmp/source-result-v658-final-generics.log` passes all 24 generic
binding tests. The summary-focused batch gate
`.tmp/source-result-v655-summary-gate.log` passes all 12 selected tests.
Final `.tmp/source-result-v659-final-source-gate.log` passes all 1,492 library
tests and 57 integration tests. Its sole failure remains the established
retained-Iterable List cast/get case, with the same two compiler errors.
Formatting and diff checks pass. The batch is measured in v88 below.

### Frozen v88 full-slice measurement

The frozen release `.tmp/new-java2rust-v88-source-result-contracts` has SHA-256
`f3cc76279c386678d5c0102599eda85f8e91b67fba7fbaf01f31fd741d9d974d`
and 20,867,648 bytes. Release build log
`.tmp/source-result-v660-final-release-build.log` records 2m02s. The frozen
copy was byte-compared with the build output and rehashed after the run. Output
is `.tmp/jgrapht-core-1.5.3-focused-v88-source-result-contracts`; run log is
`.tmp/jgrapht-v88-source-result-contracts-run.log`. The runner verified the
untouched tracked pin, 463-source inventory and byte-compared 104-file staging.
Producer provenance agrees with the frozen hash, size and release label. No
competing compiler or translator ran during timing. Exit 1 is the generated
Rust check failure, not a translation or generated-parser failure.

| Metric | v87 | v88 |
| --- | ---: | ---: |
| Translator failures / generated parse failures | 0 / 0 | 0 / 0 |
| Actual Rust diagnostics | 1,937 | 1,916 |
| Typed / untyped diagnostics | 1,850 / 87 | 1,829 / 87 |
| Invalid markers / files | 85 / 44 | 85 / 44 |
| Generic V missing expect() diagnostics | 16 | 0 |
| Incompatible `?` result diagnostics | 44 | 39 |
| Retained collection missing `.iter()` diagnostics | 6 | 6 |
| Preflight seconds | 38.754 | 39.295 |
| Translation seconds | 161.550 | 170.938 |
| Emission seconds (nested) | 105.127 | 111.823 |
| Shared analysis seconds (nested) | 43.398 | 44.014 |
| Signature fixpoint seconds (nested) | 4.993 | 4.885 |
| Assembly seconds | 56.358 | 61.854 |
| Rust check seconds | 12.834 | 12.660 |
| Total seconds | 273.361 | 288.655 |

Canonical diagnostic headers exclude aggregate footers. The complete
per-header histogram changes only in the two improving rows above: 16 fewer
invalid generic expect() projections and five fewer incompatible `?` results.
There are no added diagnostic categories or increased header counts. Signature
saturation remains 13 iterations with zero signature diagnostics. Both the 49
allocation adapters (43 shared-to-plain, six plain-to-shared) and the backed
subList marker are unchanged.

The observed total is about 5.6% slower than v87, with increases in emission
and assembly rather than fixpoint solving. This is a correctness improvement
with a modest timing cost, not a claimed optimization; the single comparison
does not establish a causal attribution for every phase. Runtime remains
practical and does not by itself reopen the bounded optimization phase.

The next related group is the five retained stream sources plus the removeAll
operand. Reuse the existing checked iterator and fallible callback machinery;
do not hide source errors behind the panicking Iterator adapter. Escaping
streams also have a separate carrier, so retain laziness, single-use behavior,
and alias/exception semantics across both inline and stored pipelines. The
selected slice still fails compilation; Rust graph behavior and expansion
gates remain unmet.

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

## Retained stream traversal batch

Retained collection streams now feed the existing fallible stream operations
with owned, checked iterator results. The runtime adapter fuses on exhaustion
or its first Java exception; it never routes checked source traversal through
the legacy panicking Iterator compatibility path. Stream creation captures the
collection reference, while the cursor binds on first pull. Collection guards
are released before callbacks run, including callbacks mutating their source.

The stream carrier supports deferred factories, linked/single-use state shared
by aliases, checked terminal operations and lazy fallible map/filter callbacks.
Cloning the carrier does not require cloning its payload. Factory and callback
errors remain Java exceptions, and reentrant callbacks do not retain a borrow
of stream state. The legacy ordinary from_iter compatibility constructor still
collects its input eagerly: this does not establish complete lazy translation
of arbitrary escaping pipelines. Stored stream roots, iterator terminals and
nested fallible flat-map/reduction boundaries remain follow-up work; unsupported
checked boundaries must not consume Result as the Java element type.

Physical iteration elements come from the same solved child-slot projection as
enhanced-for. Stream reference transfers seed the existing native-storage flow
graph so nested collections retain their allocation identity. The established
capability solver then reasons about actual handles, rather than demanding
Clone on a nested generic payload. Stream callbacks reuse the ordinary
retained-collection operation renderer, including argument evaluation before
borrowing and nullable receiver checks. Typed stream-source effects reuse the
catch-aware exception analysis; merely creating a non-null stream does not
invent traversal exceptions. Runtime demand follows typed usage facts for both
methods and constructors. Conditional integer method receivers also use the
existing Java promotion plan to avoid ambiguous Rust integer types.

At frozen v89, the initial neutral Java fixture and
`tests/corpus/collection/RetainedStreamsOracle.java` produce `11, 25, 3, 7, 2`:
all/any matching, filter/map/collect, forEach, caught concurrent modification,
and mutation observed through an original nested-list alias. Generated Rust
checks the same results and a nested generic collection with a non-Clone
payload. Separate runtime checks cover late binding, alias reuse, intermediate
linking, short-circuiting, reentrancy, callback errors and fused traversal.
Constructor-only use and a two-source production assembly probe also exercise
support dependency emission and shared runtime reconciliation.

Broader verification found three failures also present in the frozen v88
control: two test harnesses constructed plain sets instead of the current
retained-handle ABI, and single-source assembly lost a private collection state
enum. The v88 control library was copied before the new release build, with
SHA-256 `1c8af14b4386edbe2cc7cb3b6919a1014065830f88d89baccbe08997d896ae69`;
`.tmp/retained-stream-v667-matching-control.log` records the same three failing
tests. Harness construction now follows the emitted ABI without weakening its
behavior assertions. The prelude stripper preserves private state declarations
and distinguishes an empty impl body from a header awaiting its body. Final
coherence cleanup also recognizes crate-visible promoted static storage, so
source restoration cannot recreate a conflicting local copy. Minimized
assembly regressions cover these cases alongside the existing support closure.
No generated Rust or pinned Java was repaired.

The final source gate `.tmp/retained-stream-v671-final-source-gate.log` passes
all 1,493 library tests and 57 integration tests. Its sole failure remains the
established retained-Iterable List cast/get case, with the same two errors.
The earlier broadened stream gate also passes bound references, collectors,
collection returns, primitive extrema and the retained-allocation suites.
`.tmp/retained-stream-v673-final-assembly-gate.log` additionally passes all 27
selected tests: five fallible-stream checks, three retained-stream checks,
five source-precedence checks and 14 support-closure checks. This includes the
previously failing single-source production stream assembly and the new
two-source checked-runtime assembly probe. The final checked-runtime additions
pass in `.tmp/retained-stream-v669-followup-gate.log`. Formatting, shell syntax
and diff checks pass. A fresh frozen full-slice measurement is recorded below.
Follow-up source changes are identified separately, not credited to its
diagnostic count.

### Frozen v89 full-slice measurement

The release `.tmp/new-java2rust-v89-retained-streams` has SHA-256
`13a54f1db26ec4ac0d69aae64bbba78ae2e9768e3645af9adbdf9e134f7a548c`
and 20,890,872 bytes. Build log `.tmp/retained-stream-v674-release-build.log`
records 1m37s. The frozen copy was byte-compared with the build output and
rehashed after the run. Output is
`.tmp/jgrapht-core-1.5.3-focused-v89-retained-streams`, with run log
`.tmp/jgrapht-v89-retained-streams-run.log`. Producer provenance matches the
frozen binary and release label. No competing compiler or translator ran during
the full-slice measurement. The pinned checkout and 104-file staging were
verified by the runner; exit 1 denotes generated Rust failure, not translation
or generated parsing failure.

| Metric | v88 | v89 |
| --- | ---: | ---: |
| Translator failures / generated parse failures | 0 / 0 | 0 / 0 |
| Actual Rust diagnostics | 1,916 | 1,902 |
| Typed / untyped diagnostics | 1,829 / 87 | 1,815 / 87 |
| Invalid markers / files | 85 / 44 | 87 / 44 |
| Retained collection missing `.iter()` diagnostics | 6 | 1 |
| Incompatible `?` result diagnostics | 39 | 45 |
| V / E missing Clone diagnostics | 59 / 35 | 53 / 32 |
| Preflight seconds | 39.295 | 47.604 |
| Translation seconds | 170.938 | 168.118 |
| Emission seconds (nested) | 111.823 | 107.691 |
| Shared analysis seconds (nested) | 44.014 | 47.084 |
| Signature fixpoint seconds (nested) | 4.885 | 5.630 |
| Assembly seconds | 61.854 | 63.198 |
| Rust check seconds | 12.660 | 16.158 |
| Total seconds | 288.655 | 299.172 |

Canonical error counts exclude aggregate footers. The five retained-stream
`.iter()` diagnostics disappear, leaving only the removeAll operand. Additional
improvements include two retained collections previously consumed as Iterator,
two callbacks targeting plain contains methods, and one unresolved Collectors
use. Signature saturation remains 13 iterations with zero diagnostics. The 49
allocation adapters and backed subList marker are unchanged. End-to-end time
increases about 3.6%, while translation decreases slightly; the phase breakdown
does not justify restarting broad optimization.

Unlike v88, this histogram also exposes additional downstream failures. Six
new incompatible-`?` diagnostics involve nullable stream reference capture.
Nested collection layout additionally exposes an unprojected size method
reference. The two additional registry markers are erased-downcast guards in
stream terminal callbacks in AsSubgraph (six markers -> eight); they are not
new compile_error invocations. Other newly visible generic null/comparison/cast consumers remain
open; the corpus still does not compile. These increases are not reported as a
clean gate or hidden in the net reduction.

### Post-v89 boundary follow-up (not corpus-measured)

The extended neutral fixture reproduces nullable retained source capture and
nested `List::size`; `.tmp/retained-stream-v675-boundary-repro.log` reports
both errors plus a nullable Iterable conversion falling through to an invalid
allocation adapter. The final Java oracle prints
`11, 25, 3, 7, 2, 1, 9, 2, 2, 9`, additionally checking nullable parameter reuse.

Stream capture now reuses the ordinary allocation-reference cloning projection,
moved into shared coercion code without duplicating its handle/nullability
rules. Cloning the optional handle before unwrapping avoids an extra borrowed
layer around a fallible receiver and preserves reusable source parameters.
Unbound collection method references use the existing typed call planner and
retained operation renderer rather than emitting plain container methods.
The existing retained-list-to-Iterable adapter also maps nullable handles while
preserving null and allocation identity; it does not snapshot or unwrap null
at assignment. These source changes are newer than the frozen v89 binary.

The final `.tmp/retained-stream-v677-final-boundary-gate.log` passes all 1,493
library tests and 38 selected integration tests, including nullable sources,
collection method references, runtime behavior and production assembly. Its
only failure is the same established retained-Iterable List cast/get case.
The final nullable-parameter reuse extension passes all three retained-stream
tests in `.tmp/retained-stream-v678-parameter-gate.log`, including production
assembly and execution of the extended oracle values. The full corpus was not
rerun after these focused boundary fixes; the authoritative measured count
remains v89's 1,902, not an estimated lower figure. A later coherent bulk/cast
or identity-boundary batch should carry the next full-slice run.

## Generic stream value contracts and checked-runtime dependency closure

The next batch reconciles stream callbacks with the ordinary typed cast,
null-comparison, storage and callable-capability machinery. Identity reference
casts use the normal cast planner with the cursor's physical slot, including
nullable generic T. They do not reinterpret an already typed T as erased Any.
Checked narrowing and primitive unboxing remain on their exception-aware
fallback; the structured branch only admits nominal identity. Null tests use
the shared is_some/is_none projection instead of requiring generic payload
PartialEq. Lambda input slots take precedence over same-named outer locals.

`tests/fixtures/stream_generic_values.java` and the Java oracle
`tests/corpus/stream/GenericStreamValuesOracle.java` produce `1112`. They cover
filtered non-null copying, unfiltered copying including null, explicit generic
identity casts, null on either side of a comparison, bound add method
references, and a generic zero-argument factory. The generated generic method
bodies compile without a structural equality requirement. The production
assembly probe now combines three Java sources, preserving source ownership
and the shared runtime across module reconciliation.

The stronger unfiltered-copy probe found a missing signature transfer rather
than merely a rendering problem: a nullable source element could be unwrapped
before insertion into a destination whose slot had lost that qualifier.
Usage analysis now records identity-preserving stream element transfers using
the existing canonical binding/field/return storage endpoints. It enriches
field-backed stream sources and destinations through the established storage
type helper, not a second resolver. The ordinary dependency graph and monotone
signature fixpoint propagate bounded sequence-element qualifiers across these
edges. Recognized non-null filters do not transfer a nullable input domain;
arbitrary maps/flat maps are not assumed to preserve source references.
Map key/value paths remain with their existing dedicated solver. Outer
container nullability is not propagated as an element qualifier. A minimized
usage/signature regression checks both the retained nullable destination and
the absence of a spurious transfer past the non-null filter.

Creating an Iterable cursor factory also needs its payload Clone/lifetime
obligations even when creation never traverses it. Typed local factory sites
now reuse the existing retained-iteration capability projection. Constructor
declarations and lowering queries use the same storage-aware Clone cache as
methods; the old constructor-only syntax query is no longer an authority.
Class binders supplied through the constructed type also propagate constructor
requirements when there are no value parameters. The frozen control in
`.tmp/stream-values-v690-generic-factory-control.log` demonstrates the missing
T: Clone on this exact generic-factory boundary before that correction.

Checked iterator storage/method support and the native collection cursor are
factored into shared runtime generators. The platform facade and Iterable
factories reuse those definitions; a retained stream need not depend on a
source declaration spelling Iterable to receive its cursor helper. Before/after
platform text, excluding the unchanged appended math constant, is byte-equal:
207,202 bytes, SHA-256
`647bbf00b6bd8baf2fbf4b363bf645d28c9865056eabb6ea694f1e5db19f3d0f`.
The control source is `.tmp/stdlib-v89-stream-values-control.rs`. A unit test
compiles the small iterator runtime with the real JavaException support and a
narrow reflection-token stub, independently of the platform facade, and checks
non-Clone payload traversal and fused error propagation. Reflection itself is
not claimed covered by that stub. A separate translated program uses no
explicit Iterable/Iterator declaration and executes its checked count.

Verification: `.tmp/stream-values-v688-focused-gate.log` passes 31 selected
library tests and five retained-stream tests. The final
`.tmp/stream-values-v691-final-source-gate.log` passes all 1,495 library tests
and 80 integration tests. Its sole failure is the established retained-Iterable
List cast/get case with the same two errors. The preceding broader stream gate
also covers bound references, collectors, collection returns and primitive
extrema. Formatting, shell syntax and diff checks pass. Release build
`.tmp/stream-values-v692-release-build.log` completes in 1m33s. A new full-slice
measurement is tracked separately below; focused success is not a claim that
the corpus compiles.

Additional diagnostic controls remain open, not accepted as clean baseline
exclusions: `.tmp/stream-values-v679-full-repro.java` preserves the original
nested collection pin/nullable-container mismatch, and
`.tmp/native-stream-shadow-repro.java` exposes colliding source/facade
InputStream declarations. Frozen v89 already emits both InputStream
declarations in `.tmp/native-stream-shadow-v89-control.rs`, confirming that
this latter namespace problem predates the runtime factoring. Neither control
was silently repaired in generated Rust.

### Frozen v90 candidate and rejected propagation model

Frozen `.tmp/new-java2rust-v90-stream-value-contracts` is 20,967,304 bytes,
SHA-256 `1c676cd5e90c58480b34d94454f5e62b1c769e19a034e427a639d67fbade2733`
(rechecked after the run). Output is
`.tmp/jgrapht-core-1.5.3-focused-v90-stream-value-contracts`; run log is
`.tmp/jgrapht-v90-stream-value-contracts-run.log`.

| Metric | v89 | v90 |
| --- | ---: | ---: |
| Actual Rust diagnostics | 1,902 | 2,420 |
| Typed / untyped | 1,815 / 87 | 2,337 / 83 |
| Invalid markers / affected files | 87 / 44 | 83 / 42 |
| Total elapsed seconds | 299.172 | 276.895 |
| Preflight | 47.604 | 39.202 |
| Translation | 168.118 | 160.682 |
| Analysis (nested) | 47.084 | 43.375 |
| Emission (nested) | 107.691 | 104.061 |
| Signature fixpoint (nested) | 5.630 | 4.826 |
| Assembly | 63.198 | 60.142 |
| Rust check | 16.158 | 12.895 |

Both measurements translate all 104 sources without translator or generated
parse failures. The faster elapsed time does not justify accepting the type
regression. Mismatched types increase from 437 to 961; new `&Option<V>` equality
errors number 62. Shared-to-plain allocation markers decline from 43 to 39,
but nullable source/destination layouts become much less consistent.

The new pass had traversed every native allocation-retention edge in both
directions, even without a stream transfer. Allocation compatibility and copied
element value domains are different relations: `target.add(null)` must not
widen `source` in `source.stream().forEach(target::add)`, nor in
`target.addAll(source)`. The correction keeps canonical endpoint resolution but
selects only explicit nullable stream transfers and propagates source to target.
Existing native allocation and alias solvers retain their own responsibilities.
The neutral control `.tmp/directed-stream-transfer.java` covers both examples;
the signature regression also retains the positive nullable-source copy case.
This correction is newer than v90 and needs its own gates and measurement.

The corrected source passes all 1,496 library tests and 81 integration tests
across `.tmp/stream-values-v696-corrected-source-gate.log` and
`.tmp/stream-values-v697-corrected-tail-gate.log`. The sole selected failure is
the established retained-Iterable List cast/get acceptance, with the same
unsupported type-test marker and missing get method. Six retained-stream tests
pass, including a new translated execution check: a source containing `"x"`
is copied into a destination that first receives null, without widening the
source's element domain. Java control `.tmp/DirectedTransfersOracle.java`
and translated Rust both return `12`. The positive nullable-source fixture
still returns `1112`, so directionality does not discard genuine nullable
payloads. The first new unit assertion used Named String instead of the IR's
dedicated String variant; v696 includes the corrected assertion and actually
runs all 1,496 tests (the earlier exact-filter command ran zero).

Release build `.tmp/stream-values-v698-corrected-release-build.log` succeeds.
Frozen v91 `.tmp/new-java2rust-v91-directed-stream-values` is 20,965,880 bytes,
SHA-256 `a12fcf49f4bf2536b2cf74640e8dfca67252d3612b2eda48674e5a4b4acd086d`,
byte-compared with the just-built release executable. Its full-slice run is
`.tmp/jgrapht-v91-directed-stream-values-run.log`, with output in
`.tmp/jgrapht-core-1.5.3-focused-v91-directed-stream-values`.

The completed v91 measurement has 1,908 diagnostics: 1,824 typed and 84
untyped. Invalid markers remain 83 across 42 files; diagnostic counts and marker
counts are not interchangeable. All 104 sources translate with zero translator
failures and zero generated parse failures. Producer provenance matches the
frozen executable, whose SHA-256 is unchanged after the run. Elapsed times are
276.817 seconds total, 38.520 preflight, 160.542 translation, 61.178 assembly,
and 12.634 Rust checking. Nested analysis/emission take 43.290/103.822 seconds;
signature fixpoint takes 4.773 seconds. No competing test/build workload ran
during this measurement.

This removes 512 diagnostics versus v90 but is six above v89. Header and
generated-file attribution is recorded in `.tmp/stream-values-v91-headers.tsv`
and the compile logs. Versus v89:

- Six incompatible `?` capture errors disappear (four BergeGraphInspector,
  two AsSubgraph), as does the retained child size method-reference error.
- Four allocation markers disappear in GraphIterables/DefaultGraphIterables.
  Those now expose five missing `Option<E>: Clone` cursor-factory capability
  errors and five additional layout mismatches in the same two owners. These
  are still failures, not closed adapter cases.
- AsSubgraph exposes three fallible-in-infallible-closure errors, an unlowered
  stream call, an unlowered add call and one extra layout mismatch. Its
  unsupported reference null comparison replaces a generic equality error;
  it is not a successful lowering. BergeGraphInspector exposes two generic
  optional inequality errors after the capture correction.

The new callback/cursor boundaries need minimized neutral regressions and
general capability/physical-slot fixes before moving on to the broader native
bulk/view batch. The large unintended widening is corrected; the corpus as a
whole is not accepted, graph behavior is still unverified, and scope remains
104 sources.

## Iterable cursor conversion boundary capabilities

The v91 cursor errors motivated a neutral return/argument reproduction rather
than adding bounds to graph-specific owners. The first version of
`tests/fixtures/iterable_return_contracts.java` produces three missing `T: Clone`
errors with frozen v91 (`.tmp/iterable-return-v91-control.rs` and
`.tmp/iterable-return-v91-rustc.log`). These are direct and conditional returns
of an aliased native collection as Iterable.

Capability discovery now records typed conversion boundaries for local
declarations, returns, resolved method/constructor arguments and casts. It
walks expression children without flattening lambda scopes and projects the
actual retained payload through the existing Clone/lifetime demand function.
Conditional conversion uses the selected branch's physical slot. Native SAM
boundaries use the shared functional-interface input/result metadata, a fresh
lambda environment, and solved local plans for block bodies. These are creation
obligations of the lazy cursor, not eager callback execution or snapshotting.
Both Clone and lifetime reachability reuse this same query.

Explicit constructor arguments also bypassed the existing Iterable conversion
in their fast sequence path. They now invoke the same retained/fresh allocation
adapters as assignment and return lowering, before those sequence shortcuts.
There is no second cursor implementation or generated-code repair.

The expanded fixture checks direct and conditional returns, source forwarding,
ordinary argument conversion, constructor argument conversion, and expression
and block-bodied deferred captures of effectively-final collection references.
The returned Iterable factories observe a mutation made after factory
creation. Java oracle `tests/corpus/collection/IterableReturnContractsOracle.java`
returns `14`; translated execution also returns `14` in
`.tmp/iterable-return-v702-captured-values-gate.log`. The preceding v700 gate
checks the direct/argument subset. A broad source gate and frozen corpus
measurement follow separately; this positive probe is not corpus acceptance.

A stronger owner-capture probe remains open:
`.tmp/iterable-return-owner-capture.java` preserves a lambda reading an implicit
instance field and a method that replaces that field. The original combined
probe in `.tmp/iterable-return-v701-deferred-gate.log` exposes a borrowed-self
escape plus the block-alias cursor demand (the latter was addressed by using
the solved local plans). Capturing a clone of the field at callback creation
would incorrectly hide later field replacement, so no such workaround was
introduced. The owner needs canonical Java reference retention. This ownership
gap is separate from the positive effectively-final-reference capture checks;
it is not an accepted baseline exclusion or a closed callback case. Nullable
element qualifiers crossing native SAM result boundaries also remain open.

The complete batched source gate
`.tmp/iterable-return-v703-batched-source-gate.log` passes all 1,496 library
tests and 82 integration tests. Its only failure is the documented retained
Iterable-to-List cast/get check, with the same two errors. All seven retained
stream/boundary execution tests pass. No failing new acceptance test was
disabled or ignored. The separate captured-owner reproduction remains an
explicit unresolved diagnostic control, not a passing case.

Release build `.tmp/iterable-return-v704-release-build.log` succeeds. Frozen
`.tmp/new-java2rust-v92-iterable-boundaries` is 20,876,504 bytes, SHA-256
`25e498b4d0234601a51a66a21a85047d8fc8b98ed03bcd7fd143dffc7558f4b9`;
it was byte-compared with the just-built executable. The full-slice run is
`.tmp/jgrapht-v92-iterable-boundaries-run.log` and its output directory is
`.tmp/jgrapht-core-1.5.3-focused-v92-iterable-boundaries`.

The completed v92 run translates all 104 sources with zero translator and
generated parse failures. Rust checking still fails with 1,910 diagnostics
(1,826 typed + 84 untyped); invalid markers stay at 83 across 42 files. Versus
v91, three `Option<E>: Clone` cursor-factory errors disappear from GraphIterables
and one shared source-wrapper Clone error disappears from DefaultGraphIterables.
Six `E: Clone` errors appear at inherited callers' conversions from retained
DefaultGraphIterables into GraphIterables. They occur once each in Multigraph,
Pseudograph, SimpleDirectedGraph, SimpleDirectedWeightedGraph, SimpleGraph and
SimpleWeightedGraph. These are not closed cases: source-interface conversion
capability demand still needs to reach the inherited callers. Two cursor
factory Clone errors remain in DefaultGraphIterables; a third Option<E> Clone
error in GraphWalk is an older unrelated collection-copy case. Actual header
counts are in `.tmp/iterable-boundaries-v92-headers.tsv`.

Elapsed times are 277.423 seconds total, 40.167 preflight, 159.917 translation,
60.437 assembly and 12.908 Rust checking. Nested analysis/emission take
42.588/103.896 seconds, and signature fixpoint takes 4.845 seconds. This is
essentially unchanged from v91's 276.817-second total. The source was frozen
and no competing tests/builds ran during measurement. Producer provenance and
the executable's post-run hash match the frozen binary above.

Two post-measurement neutral controls keep the unresolved boundaries concrete:

- Frozen v92 translates `.tmp/iterable-return-owner-capture.java`, but Rust
  checking reports exactly one E0521 borrowed-owner escape in
  `.tmp/iterable-return-owner-v92-rustc.log`. Its cursor Clone demand is now
  present; receiver retention is not. The next ownership design should record
  current-instance capture distinctly from capturing a field value, propagate
  canonical handle demand, and reuse handle-aware method/callback lowering.
  Include reassignment and lexical-shadowing checks.
- `.tmp/nullable-iterable-supplier.java` captures an effectively-final retained
  list after inserting null and returns it through Supplier<Iterable<T>>.
  Frozen v92 reports one E0308 in
  `.tmp/nullable-iterable-supplier-v92-rustc.log`. The nested callback result
  loses the element qualifier; owner identity is not involved. General
  qualifier transfer through SAM result slots is still required.

These controls were run after timing finished, not concurrently with the
measurement. Neither is an accepted exclusion or a generated repair. The
full-slice count has not improved overall, so keep the goal active and close
these general boundary contracts before the native bulk/view batch or corpus
expansion.

## Structural nullable qualifiers across native SAM results

The next batch addresses the confirmed nullable Supplier reproduction from
v92. Frozen v92's `.tmp/nullable-iterable-supplier-v92-rustc.log` shows
Supplier<Iterable<T>> versus Supplier<Iterable<Option<T>>>. The captured list's
nullable element domain was correct; the enclosing callback result signature
had no structural qualifier path capable of expressing it.

`NullableValueAtPath` now records bounded Java generic/array child paths instead
of treating every parent as a sequence. Typed native SAM metadata supplies the
result argument index, so Supplier's result uses child zero and Function's
result uses child one. Usage records directional flows from captured canonical
storage into the returned callback's result slot. The existing dependency
graph, mutation scopes and monotone transfer pass consume these edges. Shape
construction applies qualifiers structurally, transparently traversing
Optional and producer wildcard bounds. Invalid paths cannot grow types,
primitive elements remain unboxed, and repeated application is idempotent.
Nested storage insertion projects structural qualifiers into list elements or
map values at their actual generic child indexes. Existing override,
return/local and argument-storage constraint forwarding preserves the new
qualifiers too; ordinary allocation retention remains a separate relation.

The neutral `tests/fixtures/nullable_iterable_supplier.java` executes both a
null element and post-capture mutation. Java oracle
`tests/corpus/collection/NullableIterableSupplierOracle.java` and translated
Rust return `102`; the first execution evidence is
`.tmp/sam-qualifiers-v708-execution-gate.log`. New unit coverage checks Supplier
and Function result indexes, a lambda parameter shadowing an enclosing field,
bounded/idempotent wildcard paths, primitive exclusion and nested list/map
storage coordinates.

The current recorded edges cover returned native SAMs that capture enclosing
canonical collection storage. Callback-local roots are deliberately not guessed
as same-named outer captures. Their lexical result-flow identities, stored SAM
and constructor-argument boundaries, nullable captured container references,
and captured-owner retention still need work. Map qualifier interpretation
continues to use its dedicated solver where sequence depth and structural
child coordinates differ. This is an element-domain fix, not a claim that all
callback value domains or ownership boundaries are implemented.

The inherited E: Clone caller investigation also tested mutually recursive
source methods in both query orders (v705/v706 controls). Both pass; no
cache-order bug was reproduced and no cache invalidation or optimization
policy was changed speculatively. The new cycle regression retains that
coverage. The six inherited upcast caller failures remain open.

Earlier v709/v710 broad attempts overlap debug binary rebuilds and therefore
hit the runner's "translation producer changed during the run" safeguard in
assembly probes. Those runs are not final-source evidence or new accepted
baseline exclusions. The final source is frozen for v711's complete broad
gate; its result and a separate frozen corpus measurement are recorded below.

The final `.tmp/sam-qualifiers-v711-final-source-gate.log` passes all 1,499
library tests and 84 integration tests. Its sole selected failure remains the
established retained Iterable-to-List cast/get check, with the same unsupported
type-test marker and missing get method. The final gate has no producer-change
failure. All eight retained-stream/boundary tests pass. Formatting and diff
checks pass. Release build `.tmp/sam-qualifiers-v712-release-build.log` succeeds.

Frozen `.tmp/new-java2rust-v93-sam-result-qualifiers` is 20,901,008 bytes,
SHA-256 `12a8e2579fab1dc7ff2a8535519778030ef6cc3faeba395a90fdf1bb93c014bb`,
byte-compared with the release executable. The original minimized v92 failure
now passes rustc with frozen v93, independently of the expanded integration
probe (`.tmp/nullable-iterable-supplier-v93-rustc.log`). The full-slice run is
`.tmp/jgrapht-v93-sam-result-qualifiers-run.log`, output
`.tmp/jgrapht-core-1.5.3-focused-v93-sam-result-qualifiers`.

The completed v93 run again translates all 104 inputs with zero translator or
generated parse failures. Rust checking has 1,910 diagnostics (1,826 typed +
84 untyped), and 83 invalid markers across 42 files. The sorted diagnostic
histogram is byte-identical to v92's; all 104 generated source module hashes
are also unchanged. Therefore the returned-SAM regression is fixed, but no
selected-corpus failure is counted closed by this batch. The outstanding SAM
creation and inherited callable boundaries must be connected to the new
mechanism rather than claiming success from the independent probe.

Elapsed times are 273.700 seconds total, 38.093 preflight, 158.563 translation,
60.631 assembly and 12.486 Rust checking. Nested analysis/emission take
42.260/103.165 seconds; signature fixpoint takes 4.650 seconds. This is
comparable to v92's 277.423-second total, not evidence to reopen optimization.
Producer metadata and the post-run SHA-256 match the frozen binary above.
No competing test/build/translator workload ran during timing.

A post-measurement control now reproduces an inherited-bound gap directly:
`.tmp/inherited-iterable-demand.java` defines RowProvider<Key, Value> with an
abstract collection getter and a default Iterable-returning method, then
SwappedRows<Left, Right> implements RowProvider<Right, Left>. Frozen v93 reports
exactly one missing Left: Clone error in
`.tmp/inherited-iterable-demand-v93b-rustc.log`; the inherited implementation
lacks the bound that its source interface already requires. An earlier
parameter-forwarding version passes, so it is not enough to test an inherited
method that simply returns a collection parameter.

The capability helper currently rediscovers the declared return target and
shape environment by scanning the emitted owner's directly declared members.
An inherited/specialized body is not one of those members. The next batch
should carry the callable's existing return target and applied source signature
into the shared demand query, using canonical ancestor binder substitutions.
Require Left rather than Right in the swapped-binder regression, and reuse
the same evidence for inherited emitted bounds, upcast caller demands and
callback constructor boundaries. Do not substitute a guessed empty environment
or add graph-specific bounds. Stored/constructor SAM result qualifier flow and
captured-owner retention remain unfinished as recorded above.

## Applied inherited callable context for cursor capabilities

The v93 inherited-getter control is now a durable neutral fixture,
`tests/fixtures/inherited_iterable_contracts.java`. Its default method returns
an Iterable from an abstract collection getter, rather than simply forwarding
a collection parameter. Frozen v93 lacks Left: Clone in the inherited
SwappedRows implementation after RowProvider<Key, Value> is applied as
RowProvider<Right, Left>.

Capability queries now carry the callable's return target and identity instead
of rediscovering the return type only among directly declared members. A
shared context lookup selects declared methods/constructors or reconstructs
the same specialized inherited method and wildcard binders used by signature
planning and lowering. It consumes the emitted owner's applied parameter/local
plans and shape environment. The same lookup supplies both cursor Clone and
lifetime demand and the physical-carrier proof that shields payload binders
when only a shared reference is cloned. It does not invent an empty context
for an inherited body and then treat the missing demand as proof of absence.

Callable identity also disambiguates distinct methods with identical parameter
and body syntax. A new unit regression compares the selected parameter plan
with each method's actual signature, so declaration order cannot substitute
another method's slots. Resolved method calls, new expressions and constructor
delegation now discover Iterable/SAM conversion demand from the ordinary
argument lowering plan, including its shaped/slotted target contracts, rather
than keeping a second projection from raw parameter declarations.

The inherited fixture's Java oracle,
`tests/corpus/collection/InheritedIterableContractsOracle.java`, returns `2`.
Translated execution does too, observing collection mutations after obtaining
the Iterable. A Rust instantiation with an unused NotClone binder compiles,
requiring Left's payload capability without imposing Clone on Right. Initial
evidence is `.tmp/inherited-callable-v714-execution-gate.log`; the following
v715 context-boundary gate passes retained streams, constructor callback values,
generic interface construction and nested override fallibility. Final source
verification and a new frozen corpus measurement follow separately.

This batch addresses capability context, not the remaining captured-owner
identity or stored/constructor SAM nullable-result qualifier flows. Those
diagnostic controls remain open. The 104-source scope and completion gates
are unchanged.

Final `.tmp/inherited-callable-v717-final-source-gate.log` passes all 1,500
library tests and 98 integration tests. Its only selected failure is the
established retained Iterable-to-List cast/get case, with the same unsupported
type-test marker and missing get method. There are no producer-change failures.
Release build `.tmp/inherited-callable-v718-release-build.log` succeeds.
Frozen `.tmp/new-java2rust-v94-inherited-callable-context` is 20,904,896 bytes,
SHA-256 `ef00c1221db38c047102774307c0f4cacce584df1f7581beaf1bb90378503c0d`,
byte-compared with the just-built release executable. The original minimized
inherited-getter source now passes rustc with that frozen binary
(`.tmp/inherited-iterable-demand-v94-rustc.log`). The full-slice run is
`.tmp/jgrapht-v94-inherited-callable-context-run.log`, output
`.tmp/jgrapht-core-1.5.3-focused-v94-inherited-callable-context`.

The v94 full-slice run completes with 1,908 diagnostics (1,824 typed + 84
untyped), 83 invalid markers across 42 files, and zero translator/generated
parse failures for all 104 sources. Total time is 275.699 seconds: preflight
38.491, translation 159.731, assembly 60.715 and Rust checking 12.745. Its
histogram is `.tmp/inherited-callable-v94-headers.tsv`. The net decrease is not
accepted as closure: six E: Clone caller errors disappear, but three
Option<E>: Clone cursor-factory errors and one shared wrapper Clone error
reappear. This reveals missing demand, rather than proving six valid fixes.

The ordinary argument plan correctly carries nullable SAM ABI slots. Discovery
had passed Optional<Supplier<...>> directly to a classifier for the non-null
functional payload. It consequently missed the lambda's cursor obligations.
The follow-up peels the nullable ABI wrapper for capability classification,
but retains the original target for conversion. This matches the ordinary
lambda adapter's handling; null-accepting call contracts are not removed.

`tests/fixtures/nullable_sam_constructor.java` is the minimized control: a
default constructor delegates with null, making the Supplier parameter
nullable, while a generic factory supplies a lambda capturing a retained
collection. Frozen v94 reports exactly one missing T: Clone error in
`.tmp/nullable-sam-constructor-v94b-rustc.log`. The control has no artificial
local Iterable alias in the factory: adding one would independently demand
Clone and mask the argument-boundary bug. Its Java oracle returns `1`,
observing mutation after deferred-wrapper creation. The nullable-SAM follow-up
is newer than v94 and requires its own final gates and corpus measurement.

The focused `.tmp/inherited-callable-v719-nullable-sam-gate.log` passes the
constructor control, matching its Java oracle `1`. The final
`.tmp/inherited-callable-v720-final-source-gate.log` passes all 1,500 library
tests and 99 integration tests. Its only failure remains the established List
cast/get check with the same two errors. All ten retained stream/boundary
execution checks pass. Release build
`.tmp/inherited-callable-v721-release-build.log` succeeds.

Frozen `.tmp/new-java2rust-v95-inherited-nullable-sam` is 20,905,288 bytes,
SHA-256 `efb731781f394b43dcc3aed328c0ba6c52938433c6cd20cb00a408d316f71cfd`;
it was byte-compared with the release executable. The original no-alias
constructor reproduction now passes rustc with frozen v95
(`.tmp/nullable-sam-constructor-v95-rustc.log`). The full run is
`.tmp/jgrapht-v95-inherited-nullable-sam-run.log`, output
`.tmp/jgrapht-core-1.5.3-focused-v95-inherited-nullable-sam`.

A small independent Rust check (`.tmp/bounded-cursor-object.rs`, compiled and
executed before the timed corpus run) confirms that constructing a generic
cursor trait object need not require Clone on its payload when traversal
methods carry the bound. The shared source-interface adapter still uses a
derived Clone on an Rc/Arc wrapper and aggregates method capabilities into
global implementation/From bounds. The next capability investigation should
distinguish actual adapter-wide obligations from method-local obligations,
and reuse physical handle-clone machinery, rather than reflexively widening
every caller. No such adapter rewrite has been made or claimed verified here.

The completed v95 measurement translates all 104 files with zero translator or
generated parse failures. Rust checking reports 1,908 diagnostics: 1,824 typed
and 84 untyped. Invalid markers remain 83 across 42 files. Against the accepted
v93 reference, the only header/file change is removal of the two
Option<E>: Clone cursor-factory errors in DefaultGraphIterables. The older
GraphWalk copy error is the remaining Option<E>: Clone diagnostic. The three
GraphIterables demands and shared-wrapper requirement lost by v94 are restored,
so v95's equal total to v94 must not be interpreted as equivalent failures.
Histograms are `.tmp/sam-qualifiers-v93-headers.tsv`,
`.tmp/inherited-callable-v94-headers.tsv` and
`.tmp/inherited-callable-v95-headers.tsv`.

Six E: Clone upcast caller errors remain. Shared adapter global Clone bounds,
its derived Clone implementation on a reference wrapper, and the distinction
between interface method-local and adapter-wide capabilities are the next
related investigation. Preserve the method-level bounds that real cursor
creation needs; don't fix the caller errors by hiding those demands again.
The captured-owner and stored/constructor nullable-result qualifier controls
are still open. Graph behavior is unverified and production emission remains
104 sources; this batch is not overall goal completion.

V95 takes 295.748 seconds total: preflight 39.302, translation 159.322, assembly
64.331 and Rust checking 28.412. Nested analysis/emission are 43.015/103.166,
and signature fixpoint 4.886 seconds. Translation is effectively unchanged;
the total increase versus v93 mainly comes from Rust checking and assembly.
There is no competing test/build/translator workload in this timed run. The
post-run executable hash and producer provenance match the frozen binary above.
The executable and Java inputs were not modified during measurement.

## Scoped interface adapter capabilities (v96)

Mutable shared source-interface adapters now distinguish the selected interface's
method-local Clone predicates from adapter-wide obligations. The capability
query reuses inherited contract specialization and the same compatible method
selection as adapter emission. Its recursion key includes the selected
interface. Exact binder substitutions can discharge a method-local predicate;
composite substitutions are not inverted to invent Rust bounds. Self ancestry
uses the ancestry API's identity convention. Other storage shapes retain the
established conservative path.

The shared adapter no longer derives Clone with an unnecessary payload bound.
It reuses the explicit reference-carrier clone generator already used for
generic Rc/Arc carriers. Bounds needed by implementation methods remain on the
interface implementation and conversion, not on cloning the reference handle.
Non-Clone well-formedness obligations are preserved.

The corpus-neutral scoped_adapter_capabilities fixture exercises an inherited
two-binder permutation, a traversal method with a local Clone predicate, a
second interface whose copying implementation genuinely needs Clone, and a
factory with an unused binder. Generated Rust constructs and clones the
traversal object with a non-Clone payload. Java and translated execution return
3, including observing collection mutation after view creation.

That stronger fixture also exposed lost explicit generic witnesses in chained
source static calls. Ordinary plain-path static calls now retain their explicit
type arguments in structured Rust IR, sharing signature emission's erased
exception-only binder policy. Known native throwable carriers use the existing
carrier classifier rather than recognizing only Throwable. A separate neutral
control preserves an unused binder and erases a RuntimeException-only binder;
Java and generated execution return 5. Generic-owner paths and instance-method
witnesses are not claimed covered by this change.

The focused scoped-adapter gate v727 and witness/erasure gate v729 pass.
Earlier v723 and v728 failures were intermediate controls, not accepted source
checkpoints. The final batched source gate passes 1,500 library and 101
integration tests. Its sole failure is the established retained-Iterable List
cast/get acceptance, with the same two generated compile errors. It is recorded in
`.tmp/adapter-capabilities-v730-final-source-gate.log`; release build provenance
is `.tmp/adapter-capabilities-v731-release-build.log`.
The existing Throwable-only ABI-erasure regression also passes separately in
`.tmp/adapter-capabilities-v732-existing-erasure-gate.log`. Independent Java
oracles are `tests/corpus/collection/ScopedAdapterCapabilitiesOracle.java` and
`tests/corpus/collection/ExplicitStaticWitnessOracle.java`.

Frozen release binary `.tmp/new-java2rust-v96-scoped-interface-adapters` is
20,911,920 bytes, SHA-256
`278f033806ccfd44d203befb39935612b8553b2504f409e625c5fd037dd921c0`.
The completed run is `.tmp/jgrapht-v96-scoped-interface-adapters-run.log`, with
artifacts in `.tmp/jgrapht-core-1.5.3-focused-v96-scoped-interface-adapters`.
All 104 sources translate with zero translator or generated parse failures.
The crate has 1,870 diagnostics (1,786 typed, 84 untyped), versus v95's 1,908.
Invalid markers remain 83 across 42 files. Comparing diagnostic headers and
origin files removes 38 and adds none: six E: Clone upcast callers, sixteen
V/E Clone demands in BoyerMyrvoldPlanarityInspector and FloydWarshallShortestPaths,
and sixteen shared-wrapper Clone errors. The histogram is
`.tmp/adapter-capabilities-v96-headers.tsv`. Genuine copying requirements remain
covered by the separate Copying interface in the neutral fixture.

V96 takes 347.637 seconds total: preflight 51.137, translation 205.024,
assembly 74.141 and Rust checking 13.236. Nested analysis/emission are
69.345/121.174, with signature fixpoint 8.600 seconds. No competing agent-owned
compiler/test/translator workload runs during measurement, and the executable
hash is unchanged afterward. This run is slower than v95; changes in several
unchanged analysis subphases mean the cause is not isolated. No speedup or
performance-neutrality claim is made. A paired timing check is warranted if
subsequent measurements retain the increase.

The next related correctness work is stored/constructor SAM nullable-result
qualifier propagation and captured-owner identity. The latter must retain the
owner, not snapshot its current collection field: replacement after callback
creation must remain observable. Native allocation adapters and List view
cast/get wiring remain open. Graph behavior is unverified; this is not overall
goal completion or permission to expand beyond the selected 104 sources.

## Stored callback result domains (v97)

The previous release reproduces five failures in the initial four-path neutral
control (`.tmp/stored-nullable-callback-v96-control.rs` and
`.tmp/stored-nullable-callback-v96-rustc.log`): missing nested nullable domains,
a missing cursor-copy bound at assignment, and a bare forwarded callback with
an invalid allocation conversion. The generated control was not repaired.

The structural nullable-flow recorder now handles lambdas in canonical local,
field, method-argument and constructor-argument destinations, not only direct
returns. Argument destinations resolve to existing callable parameter nodes;
locals retain declaration IDs. SAM input/result metadata selects the actual
generic child, including Function's child one. Existing lexical capture guards
remain: callback-local roots are not guessed to be same-named outer captures.

Reading a native SAM result projects its structural slot through the receiver's
canonical storage identity. Projected value domains flow directionally into
return, local, field and argument slots; allocation aliasing remains separate.
These edges reuse the dependency graph and nullable transfer pass. Non-lambda
projection edges are limited to native collection/Iterable destinations. Cheap
expression guards avoid enriching type environments for unrelated assignments.

Concrete source callbacks that cannot use the inline FnMut ABI now explicitly
record their emitted value-carrier ABI in SourceCallbackPlan, alongside virtual
contracts. Call lowering consequently reuses the existing typed facade adapter.
Proven inline callbacks and the retained inline priority-queue comparator ABI
remain distinct. Borrowing native functional values also preserves their actual
runtime carrier instead of evaluating it and substituting a unit facade; this
classification reuses shared functional metadata.

The expanded `tests/fixtures/stored_nullable_callback_results.java` exercises
local initialization, reassignment, field assignment, direct constructor and
source-forwarded arguments, and Supplier/Function result coordinates. Java
oracle `tests/corpus/collection/StoredNullableCallbackResultsOracle.java`, compiled
alongside that fixture, and generated execution both return 612. Every callback
observes a null element and mutation after callback creation. Execution passes
in `.tmp/stored-sam-v738-execution-gate.log`. Canonical destination and concrete
callback ABI unit regressions pass in v739/v740. V737 used an incorrect exact
test filter and ran zero tests; it is build evidence only, not regression proof.
The earlier v733-v736 failures are intermediate diagnosis, not accepted gates.

The broad source gate `.tmp/stored-sam-v741-batched-source-gate.log` passes
1,502 library and 103 integration tests. Its sole failing target is the
established retained-Iterable List cast/get acceptance, with the same two errors.
Release build provenance is `.tmp/stored-sam-v742-release-build.log`. Frozen
`.tmp/new-java2rust-v97-stored-callback-results` is 21,003,984 bytes, SHA-256
`b0e871fb67bc22741f77eb689f0de48acb0b7bc89eee602123431aa3b76f141d`.
It was byte-compared with the completed release build.
Captured-owner retention, callback-local result identities, nullable captured
container references, and dedicated map qualifier interpretation remain open.
The frozen candidate still produces exactly one E0521 borrowed-owner escape in
`.tmp/iterable-owner-capture-v97-rustc.log`, from the untouched owner-capture
control. Resolver `capture_var` explicitly skips current-type fields, and its
lambda capture walk has no current-instance identity capture. Existing storage
analysis already retains captured declared objects, and handle-aware method
companions already exist. The next owner fix should supply an explicit lexical
current-instance capture to those mechanisms, preserving static exclusions,
local shadowing and later field replacement, rather than introducing a separate
field snapshot adapter.

The completed full-slice run is `.tmp/jgrapht-v97-stored-callback-results-run.log`,
with artifacts in `.tmp/jgrapht-core-1.5.3-focused-v97-stored-callback-results`.
All 104 sources translate, with zero translator and generated parse failures.
Rust checking still reports 1,870 diagnostics (1,786 typed, 84 untyped), and
invalid markers remain 83 across 42 files. V96/v97 diagnostic header and origin
file counters are identical: zero removed and zero added. The histogram is
`.tmp/stored-sam-v97-headers.tsv`. This batch closes the minimized neutral
boundary controls, not any currently reported corpus error; do not claim a
corpus diagnostic improvement from the passing probes alone.

Total elapsed time is 289.000 seconds: preflight 46.636, translation 165.521,
assembly 60.221 and Rust checking 12.574. Nested analysis/emission are
44.372/107.008, and signature fixpoint 4.794 seconds. No competing agent-owned
compiler/test/translator workload runs during this measurement. Producer
provenance and the post-run binary hash match the frozen executable. Translation
returns close to v95's 159.322 seconds, so v96's 205.024-second translation
slowdown has not persisted. This does not isolate timing variance or prove a
new performance optimization.

Replan the next correctness batch around actual corpus blockers rather than
continuing to expand neutral callback controls. List cast/get wiring, the last
retained bulk operand, and identity-preserving allocation/view boundaries are
related confirmed failures. The owner-capture control remains important and
unresolved, but the corpus's five E0521 errors currently come from erased-object
cloning/display, not this captured-owner pattern. Keep that distinction when
choosing the next batch. Graph behavior remains unverified and production
emission remains 104 files.

## Checked retained bulk operands (v99)

V97's remaining direct `.iter()` on a retained collection occurs in
BergeGraphInspector's removeAll operand. The statement-only path assumes a
plain source even when the receiver is a plain semantic set and the operand is
an Rc native collection. The original three-path neutral fixture produces five
errors with frozen v97: three unsupported retained operations, the invalid
operand `.iter()`, and a generic Set carrier mismatch. Evidence is
`.tmp/retained-remove-all-v97-control.rs` and
`.tmp/retained-remove-all-v97-rustc.log`; generated Rust was not repaired.

Retained operands now use the existing checked native cursor protocol. Equality
runs after each reference read has released the allocation guard. Both retained
receivers and plain receiver storage share checked membership, while a plain
receiver's retain adapter preserves processed removals and the unprocessed
suffix on failure. Two retained operands use cursor removal rather than a
snapshot or a guard held across callbacks. The same path handles statement and
boolean-result calls, cross-nullability domains, self-aliasing and changed
results. Receiver and argument evaluation precede null projection. Existing
source precedence, unmodifiable/static boundaries and unsupported synchronized
storage exclusions remain; this is not a complete List operation projection.
The physical cursor-copy/lifetime demand reuses retained iteration's solved
element query for both operands rather than imposing payload Clone blindly.

Membership compares the query to the stored value using Java equality. The
plain cross-nullability helpers also used the reverse direction; they now use
the same query-to-element direction, with an asymmetric equality regression.

The positive generic Set probe exposed a second, independent ABI issue: generic
declarations already render a semantic hash-set carrier, but that choice was
implicit and absent from caller constraints. Method and constructor parameter
collection now records that existing choice as RequiredView(JavaSemanticHash),
using the shared generic-carrier classifier. Ordinary shape propagation can
therefore choose compatible concrete hash-set storage before construction and
borrowing. Ordered/source carriers are not reclassified to satisfy that ABI;
their broader interface-polymorphism boundary is not claimed solved here.

`tests/fixtures/retained_remove_all.java` covers a generic plain Set receiver,
a plain List receiver, retained receivers, both nullable element directions,
changed results and self-removal. Java oracle
`tests/corpus/collection/RetainedRemoveAllOracle.java`, compiled alongside the
fixture, and generated execution agree on `21 111 111 10`. The v745 focused gate
passes. V746's emitted-runtime gate covers reentrant source access,
nonstructural updates, fail-fast structural updates, fixed-size rejection,
partial failure, same-allocation removal and equality direction. The v747 ABI
policy unit preserves ordered-collection and parsed-source exclusions. V743
passes cargo check; v744 is the intermediate generic-carrier failure.

The first broad gate v748 passes 1,503 library tests but exposes a regression in
the existing retained_generic_field alias execution test: a propagated semantic
hash requirement survives refinement to an ordered carrier and panics during
rendering. This candidate is not accepted. The completed v749 release is frozen
as `.tmp/new-java2rust-v98-checked-bulk-operands` (21,064,896 bytes, SHA-256
`56be061847be061519a17d54d3cc1dec5e7d5698b7050128eded689c9c7beb35`), but is not
corpus-measured because of that source regression.

The shared facts-aware slot-view planner now clears an incompatible propagated
hash view before rendering an ordered or parsed-source carrier. Intrinsic
generic hash selection still uses the existing classifier. This compatibility
policy has a unit regression; both the old ordered-field alias execution (v750)
and new bulk execution (v751) pass together after the correction. The final
broad gate is `.tmp/retained-bulk-v752-final-source-gate.log`, and the corrected
release build is `.tmp/retained-bulk-v753-release-build.log`. Keep v97 as the
reference for the corrected full-slice comparison below.
The final gate passes 1,503 library and 109 integration tests; only the existing
retained-Iterable List cast/get acceptance fails, with the same two errors. The
ordered-field alias regression is closed, not an accepted exclusion. Corrected
frozen `.tmp/new-java2rust-v99-compatible-bulk-operands` is 21,065,064 bytes,
SHA-256 `dce5b7085f2e67d7f43e75a0370434bb94113bfcd0dd51c913eb6fa2f565cac1`;
it was byte-compared with the completed build.

List cast/get remains open. Its existing JavaListView runtime must be wired into
physical slot/view planning and propagation and native/source projections.
Source List get/set/size/iterator operations must dispatch through the canonical
source handle without an outer guard across callbacks. Unknown projections
cannot become negative instanceof answers. Both the original seven-case
acceptance and SourceListProjection's 47/28 oracle remain required; a native-only
allocation cast is not their general solution.
The frozen v98 source-List control refresh is
`.tmp/source-list-projection-v98-control.rs` with compiler output
`.tmp/source-list-projection-v98-rustc.log`: it has three errors, the unsupported
type test, cast/get, and local cast carrier mismatch. The source projection
oracle is still not a passing translation gate.
Its maintained Java oracle still returns `47\t28` in
`.tmp/source-list-projection-v754-java-oracle.log`.

The completed corrected full-slice run is
`.tmp/jgrapht-v99-compatible-bulk-operands-run.log`, with artifacts in
`.tmp/jgrapht-core-1.5.3-focused-v99-compatible-bulk-operands`.
All 104 sources translate with zero translator and generated parse failures.
There are 1,869 Rust diagnostics (1,785 typed, 84 untyped); invalid markers remain
83 across 42 files. Against v97, exactly one header/file diagnostic disappears:
the retained removeAll operand's invalid `.iter()` in BergeGraphInspector.
Zero header/file diagnostics are added. The histogram is
`.tmp/retained-bulk-v99-headers.tsv`. This closes the targeted operand family,
not the remaining generic `.into_iter()`/retargeting or collection view cases.

Elapsed time is 437.813 seconds: preflight 55.322, translation 246.306, assembly
106.570 and Rust checking 22.623. Nested analysis/emission are 63.786/161.872;
signature fixpoint is 6.167 seconds. Unchanged semantic and representation phases
also rise, to 15.583 and 8.201 seconds. No competing agent-owned compiler,
test or translator workload runs during this measurement. Producer provenance
matches the frozen corrected binary; no generated Rust or Java input was repaired.
The run is materially slower than v97, so a paired frozen resolver-phase control
is recorded separately before deciding whether a new optimization is warranted.

The bounded follow-up runs v97 and v99 sequentially on the same unchanged
104-file root with matching resolver-bench arguments and no competing
agent-owned build/test/translator workload. Logs are
`.tmp/retained-bulk-v755-control-resolver-bench.log` and
`.tmp/retained-bulk-v756-candidate-resolver-bench.log`. The control takes 82.784
seconds and the candidate 80.592. Representation/resolution phases are
60.941/21.568 and 52.029/28.281 seconds respectively, so even the subphase mix
varies. The unchanged control's earlier full-run preflight was 46.636 seconds;
substantial run-to-run timing variation exists independently of new code.

Reports are not byte-identical: the explicit view requirements change unique
signature constraints from 786 to 793 and reported join conflicts from 999 to
1,044, including 45 additional View joins. The report's signature iteration
count remains 238 and dependency nodes/edges remain 18,521/19,580. These are
resolver-report aggregate counters, not the full-run global fixpoint's loop
count. The pair shows no resolver slowdown, not an optimization or a proof that
emission/assembly cannot have changed. Both executable hashes are rechecked
after the pair. No new optimization is justified by this evidence alone;
continue correctness work and revisit with paired dominant-phase measurements
if a persistent progress blocker is reproduced.

## List-operation view wiring (v101; v100 rejected)

The List-view implementation is now wired through cast planning, slot rendering,
signature constraints and typed expression lowering. `ListInterface` denotes a
`JavaListView<T>` operation table over the original Java allocation, not a Vec
snapshot or an extra outer allocation handle. Checked get/set/size/isEmpty and
iterator operations delegate through that table. Native collection projections
and compatible parsed-source virtual get/set/size/iterator implementations use
the same runtime interface. Reference clones do not require cloning `T`.

View constraints are restricted to compatible platform List slots; an Iterable
or parsed-source carrier must not become a List view merely because a cast or
generic callee returns one. Nullable native SAM slots also discard propagated
outer allocation requirements, like their non-null counterparts. Generic call
result planning reuses resolver-owned type substitutions and the existing
qualified-shape specializer. Existing canonical storage edges carry view and
element-domain requirements across call results and aliases. Reference-cast
return provenance preserves Java null, and signed checked getters are reused
for numeric unboxing and null comparisons. Fresh Set producers transfer into
the retained Iterable protocol rather than being assigned directly to its ABI.

Focused gate `.tmp/list-view-v765-boundary-gate.log` passes all five integration
tests, including the previously failing retained Iterable List type-test/cast
case and source-defined virtual List operations. New neutral fixture
`tests/fixtures/list_projection_boundaries.java` exercises generic returns,
mutation through a List parameter, aliases, null casts, rejected Set casts and
nullable element writes observed through the original Iterable and List.
`tests/corpus/collection/ListProjectionBoundariesOracle.java` independently
confirms `34 7 7 110`; generated Rust agrees. A generated-crate probe also calls
the generic cast with a non-Clone element type. Source-defined List probes
continue to produce `47 28`.

Broader gate `.tmp/list-view-v766-broad-source-gate.log` passes 1,504 library
tests and 90 integration tests across generic methods, default inheritance,
allocation shapes, platform collection hierarchy, retained collection views,
nested collections, streams, source callbacks and support closure. A final
focused rerun also checks the subsequently strengthened generic type-test/get
fixture; `.tmp/list-view-v767-generic-element-gate.log` passes all five tests.
That generic branch probe returns `10` across a List and a non-List Set,
matching Java, and the non-Clone generic cast probe remains green.

The initial frozen candidate v100 is not an accepted full-slice checkpoint.
Candidate
`.tmp/new-java2rust-v100-list-operation-views` is 21,161,664 bytes with SHA-256
`4de472e9e4ce8161685cb7c0dee23b2d82a313276a965c002fd385c1d7fc23be`.
Release build log is `.tmp/list-view-v768-release-build.log`; the single
full-slice run is `.tmp/jgrapht-v100-list-operation-views-run.log`, with artifacts
under `.tmp/jgrapht-core-1.5.3-focused-v100-list-operation-views`.
V100 completes with no translator or generated parse failures and 1,856 actual
Rust diagnostics (1,779 typed, 77 untyped), versus v99's 1,869. Invalid markers
fall from 83/42 files to 76/39 files: CollectionUtil loses one, and the directed
and undirected edge containers lose four and two respectively. This net
improvement is **not** a regression-free gate: header/source-file comparison
finds 90 additions and 103 removals, including widespread changed nullable
domains and reference projections. The candidate is retained as diagnostic
evidence, not promoted to the current checkpoint.

The follow-up narrows two overly broad propagation changes: cast nullability
uses a directional qualifier edge instead of treating every reference cast as
an ordinary returned binding with shared ownership/dispatch constraints;
new generic-return qualifier feedback requires an actual List operation-view
contract, and additional whole-reference alias edges require an Iterable
endpoint. Existing collection transfer and native-allocation mechanisms remain
in place. Focused verification is
`.tmp/list-view-v769-qualified-boundaries-gate.log`: all 51 integration tests
pass, including the generic/null/non-Clone List-view probes, nested collection
aliases and retained stream qualifier boundaries. Library and complementary
source gate `.tmp/list-view-v770-qualified-source-gate.log` passes all 1,505
library tests and 26 complementary integration tests. The new usage
regression explicitly distinguishes a cast's directional outer-null edge from
an ordinary returned binding's whole-carrier contract.

The revised frozen candidate is `.tmp/new-java2rust-v101-qualified-list-views`,
21,144,328 bytes, SHA-256
`c823cb454c6025451af2bd82403f99778afbf92039171dfe25fc6e22c8ca470b`.
Release build is `.tmp/list-view-v771-release-build.log`; full-slice run is
`.tmp/jgrapht-v101-qualified-list-views-run.log` and artifacts are
`.tmp/jgrapht-core-1.5.3-focused-v101-qualified-list-views`.
V101 translates all 104 sources with zero translator/generated parse failures.
It has 1,867 actual Rust diagnostics (1,784 typed + 83 untyped) and 82 invalid
markers in 41 files. Against v99, header/source-file comparison has **zero
additions and two removals**, both in CollectionUtil: unsupported Iterable-to-List
instanceof and the nonexistent Iterable.get call. The corresponding invalid
marker is gone; all other marker counts are unchanged. V101 is the new accepted
incremental measurement, not a clean corpus compile or goal completion.

V101 takes 391.961 seconds: preflight 62.673, translation 227.463, assembly
79.244 and Rust checking 17.081. Nested emission is 147.245 seconds and signature
fixpoint 6.654 seconds. Compared with v99, the resolver report retains 793
unique constraints, 238 reported iterations, 1,044 joins and 18,521 nodes;
dependency edges increase only from 19,580 to 19,584 (v100 had 19,853).
The unpaired elapsed-time differences do not establish an optimization result.

V100 total time is 466.854 seconds: preflight 88.690, translation 267.625,
assembly 88.484 and Rust checking 16.813. Nested emission is 157.588 seconds
and signature fixpoint 8.279 seconds. This does not establish a new optimization
result or isolate a persistent complexity regression.
Unknown List projections remain distinct from proven Absent.
Incompatible source result domains, inherited unmodeled JDK operations and
unsupported source storage/dispatch forms still need explicit treatment before
claiming general source-List completeness. Additional List operations beyond
the table above remain guarded rather than silently using Vec behavior.

## Retained stream materialization (candidate after v101)

The six remaining plain-to-shared adapter errors include proven fresh stream
collector results (BergeGraphInspector.n and AsSubgraph.getAllEdges). These
require allocation construction at the materialization boundary, not copying an
existing collection handle. Unmodifiable/live views remain a separate problem.

The neutral fixture `tests/fixtures/retained_stream_collectors.java` reproduces
missing retained List/Set collector materialization. Its Java oracle confirms
`3134 21`, checked-callback handling, equality/callback interleaving and distinct
ArrayList/LinkedList empty-addAll cursor behavior. Generic probes use JavaMapKey
without Rust Eq/Hash. A copied destination can accept null without widening the
input's element domain.

The implementation consumes each stream element directly into a fresh native
collection, then installs its shared allocation. It reuses element conversion,
checked traversal and Java equality. An intermediate Vec/dedup pass was rejected
because it would reorder equality relative to stream callbacks. Materialized
reference domains use directional child-value edges; nested collection elements
may retain their own allocations, but source/result outer containers never alias.
The existing non-null-filter/identity-map policy is shared with foreach transfers.

Additional general boundaries exposed by the probe are addressed together:
declaration anchoring reuses qualifier-preserving specialization; collector
initializer planning uses the typed stream result; native functional arguments
use instantiated input slots in the shared call planner; stream functional
invocations use checked facade methods; inline empty generic collection
arguments receive the retained receiver's element context. Native List empty
addAll policy is shared by constructors and collector suppliers.

The initial control is `.tmp/retained-collect-v772-control-gate.log`. Current
focused gate `.tmp/retained-collect-v777-factory-context-gate.log` passes all 31
integration tests: four materialization, nine nested collection, twelve retained
stream and six source-callback tests. The new collector test also passes the
per-element equality effect log and checked callback oracle. Broader gate
`.tmp/retained-collect-v778-broad-source-gate.log` passes 1,506 library tests and
51 complementary integration tests (generic ABI, functional carriers,
inheritance, nullable maps, List views and support closure). Final supplier-kind
gate `.tmp/retained-collect-v780-supplier-kind-gate.log` passes five collector
parser tests, including a new queue exclusion: ArrayDeque suppliers must not
acquire List operations/type-test semantics. Source constructor/Collectors
precedence controls remain green. Queue/sorted/custom suppliers need their own
construction policy and are not silently treated as Lists. The release build
including this final guard is `.tmp/retained-collect-v781-release-build.log`;
the earlier v779 build predates that guard and is not a frozen checkpoint.
The initial full-slice candidate is not yet an accepted checkpoint.
Frozen candidate `.tmp/new-java2rust-v102-retained-stream-collectors` is
21,092,008 bytes, SHA-256
`7dc084438d53e689bba943295bf2b4b400dc3e35a59a70da1e87baf38c837df7`.
The full-slice run is `.tmp/jgrapht-v102-retained-stream-collectors-run.log`,
with artifacts in `.tmp/jgrapht-core-1.5.3-focused-v102-retained-stream-collectors`.
V102 translates all 104 files with zero translator/generated parse failures.
Invalid markers fall from 82 to 79 (same 41 affected files), removing the
three targeted plain-to-shared collector guards: one in BergeGraphInspector.n
and two in AsSubgraph.getAllEdges/outer replay. Rust diagnostics are 1,868
(1,788 typed + 80 untyped), versus v101's 1,867. Header/source-file comparison
has three removed guards and four added diagnostics, all within the newly
emitted callbacks: two overloaded source-call name failures and two nullable
Set.contains argument mismatches. V102 is diagnostic evidence, not a clean
compile or an accepted replacement for v101.

V102 takes 310.486 seconds: preflight 53.801, translation 178.842, assembly
60.932 and Rust checking 12.709. Nested emission is 113.863 seconds and
signature fixpoint 5.182 seconds. Reported solver iterations remain 238;
dependency edges rise only from 19,584 to 19,586. These unpaired times are not
an optimization claim.

The follow-up shares a solved source-call contract across method naming,
argument slots and effects, including an existing uniqueness-checked arity
fallback. Native argument planning receives solved captured-field qualifiers
instead of declaration-only types. The neutral fixture
`retained_stream_callback_contracts.java` and its Java oracle expect `22`;
it exercises overloaded generic interface calls and nullable field membership
references. Its first gate v782 proved source-name/qualifier selection but
exposed a projected-place/borrow-shape mismatch. The follow-up normalizes that
argument projection and recovers reusable owned references before wrapping
them in an optional argument. Verification is currently
`.tmp/retained-collect-v783-projected-arguments-gate.log`.
That probe confirms the callback bodies now compile through a generic source
interface, but additionally exposes a separate constructor-inference gap in
`filter(new OverloadedStreamReceiver<>(), words, "x")`: the emitted caller leaks
the unresolved method binder into its trait/Vec arguments. This is not claimed
fixed. The neutral callback probe uses explicit String constructor/method
arguments to isolate the callback contract, not as a corpus-source workaround.
Current verification is `.tmp/retained-collect-v784-explicit-frame-gate.log`;
the pinned JGraphT source is unchanged.
V784 then isolated a second general cause: caller-local return constraints were
merged by call name/boundary across unrelated owners (`filter` in this probe),
even though canonical binding/call assignments were already recorded. Method
and constructor binding-result propagation now consumes that existing identity;
legacy idless facts keep their compatibility fallback. The test asserts the two
different return element domains before compiling generated Rust.

`.tmp/retained-collect-v787-canonical-call-gate.log` passes all 48 integration
tests: five materialization/callback, 25 generic binding, twelve retained stream
and six source-callback tests. The generic interface and nullable field probes
execute and agree with Java's `22`. The separate diamond-constructor inference
gap remains documented above. The latest changes are not yet corpus-measured;
broader library and complementary gates are
`.tmp/retained-collect-v788-callback-source-gate.log`.
That gate passes all 1,507 library and 36 complementary integration tests;
combined with v787, 84 related integration tests pass. The final callback
regression runs through a generic interface and proves separate return domains
for same-named methods on different owners. The post-fix release build is
`.tmp/retained-collect-v789-release-build.log`; no v103 corpus result is claimed
before its frozen run completes.

The follow-up binary is frozen as
`.tmp/new-java2rust-v103-canonical-stream-contracts`, 21,105,024 bytes,
SHA-256 `2c0d3629f601bd044c87c64e5527cd96973dd79a26fc7a7a948678243b3ca912`.
The unchanged 104-file gate runs in
`.tmp/jgrapht-core-1.5.3-focused-v103-canonical-stream-contracts`, with log
`.tmp/jgrapht-v103-canonical-stream-contracts-run.log`. No competing build or
test workload runs during this measurement. Acceptance requires comparison
against both v101 and the unaccepted v102 candidate, not just a net error count.

V103 finishes with all 104 files translated, zero translator/generated parse
failures, 79 markers across 41 files, and **1,860 actual Rust diagnostics**
(1,780 typed + 80 untyped). Compared with v102, header/source-file counters
remove 22 diagnostics and add 14: fourteen `contains_edge` name/trait failures
are replaced by thirteen checked-result errors in BergeGraphInspector (eleven
bool/Result mismatches and two unary-not/Result failures); seven AsSubgraph
argument mismatches and one GraphMetrics mismatch disappear; one additional
generic `V.downcast` error is exposed in AsSubgraph. Compared with accepted
v101, the three allocation guards remain removed, along with twelve old
source-name failures and six argument mismatches, while the same fourteen
new diagnostics appear. This is a net improvement, not proof of a regression-
free checkpoint. Keep v101 accepted until those follow-up boundaries are
covered and the candidate is remeasured.

The next coherent callback batch should make solved source-call identity,
argument slots, checked effects and result element shape agree at the same
boundary. In particular, source-name fallback now reaches the correct checked
method, but predicate emission still treats its Result as bool. The additional
AsSubgraph cast is applied to an already typed generic element, not an erased
Any carrier. Add corpus-neutral checked overloaded-predicate and typed generic
filter regressions; do not special-case Graph methods or repair generated Rust.
The separate explicit-argument/diamond inference gap remains open too.

V103 takes 392.278 seconds: preflight 54.245, translation 229.390, assembly
86.338, checking 17.427. Nested emission is 146.558 seconds; the solver still
reports 238 iterations and 19,586 dependency edges. These are unpaired corpus
measurements, not evidence for another optimization phase. Relevant focused
gates remain the 1,507 library and 84 integration passes above. The combined
goal remains active: the 104-file crate does not compile or satisfy graph
behavior gates yet, and scope expansion is not warranted.

### Checked predicate identities and covariant reads (after v103)

The v103 predicate failures reproduce without JGraphT. A minimized unit
probe gives signature lookup a qualified alias for a simple source declaration
and two generic `accepts` overloads, only one checked. Before the fix, emitted
Rust invokes the correct overload name but omits `?`: the effect query uses
the alias rather than the selected declaration identity. Signature lookup now
exposes that same declaration owner/name to stream effect lookup, reusing its
existing uniqueness and package-peer guards. The stream renderer uses its
existing checked-or-uncaught boundary, rather than unconditionally adding `?`.
The unit also checks the infallible facade and overload separation. Identity
lookup rejects ambiguous simple owners and distinct qualified package peers.

Retained cursor reads now reuse `types::generic_read_type` for covariant
elements, preserving solver-added Optional qualifiers and nested carrier
metadata. A read through `? extends T` produces T, not an erased Any value;
contravariant/unbounded wildcards are not projected to T. The new generated
probe additionally exposes an existing call-site refinement problem: bounded
wildcards were specialized to one observed caller's concrete argument, changing
a still-generic declaration's `? extends T` to String. The monomorphic
`Model<?>` refinement is now limited to unbounded wildcards; bounded read/write
contracts remain declared. A mixed-wildcard unit verifies the distinction.

Permanent fixtures are `checked_stream_predicate_effects.java` and
`retained_covariant_stream_values.java`. Their independent Java oracle,
`tests/corpus/stream/CheckedPredicateAndCovariantReadsOracle.java`, returns
`1 7 1` (ordinary predicate success, caught callback failure, covariant result).
The initial covariant method-level probe also exposes broader generic ABI
refinement; the permanent owner-generic probe follows the corpus's declaration
shape and retains the runtime check rather than deleting its caller.
Earlier probe variants also exposed separate, still-open gaps: directly
passing `fail ? null : "x"` acquired an extra Some around its conditional,
and explicitly spelling a same-compilation-unit source parameter's package
name emitted an unmapped flattened type. The permanent runtime fixture uses
ordinary Java assignment and its source import spelling to isolate predicate
effects; the focused alias unit supplies the qualified lookup independently.
Neither unrelated gap is claimed fixed by this batch.
The alias control is `.tmp/callback-effects-v792-alias-control.log`;
the initial covariant controls are v793/v795. The first post-fix library gate
`.tmp/callback-effects-v796-library-gate.log` passes all 1,509 tests, including
the alias and read-domain units, before the bounded-wildcard refinement fix.
That final refinement and the combined generated probes are under verification
in `.tmp/callback-effects-v797-bounded-domain-gate.log`; no newer full-slice
checkpoint is claimed yet.

V797 removes the incorrect concrete String specialization and its downstream
insert/Iterable errors, leaving one independent membership ABI mismatch:
`contains_by` expects `&Option<T>` but gets `Option<&T>`. Filter callback planning
had retained the cursor's owned element shape even though Rust's filter helper
loans that element to the predicate. It now plans the callback with a borrowed
element shape while leaving the forwarded element unchanged. This reuses the
existing stream projection/slot conversion machinery rather than rewriting
nullable collection calls. Combined gate v798 covers materialization, source
callbacks and retained streams; v799 covers the library, generic binding,
platform collection hierarchy, nested retained collections and generic source
interface construction. Both are pending at this writing.

V798 passes all 25 integration tests (six materialization, twelve retained
stream, seven source-callback). V799 passes all 1,510 library tests and 39
complementary integrations (three generic source interface construction, 25
generic method binding, two platform collection hierarchy, nine retained
nested collections). All three new library regressions pass. Together the
latest gates cover 64 related integrations. Release build
`.tmp/callback-effects-v800-release-build.log` passes. The frozen binary is
`.tmp/new-java2rust-v104-callback-read-contracts`, 21,105,872 bytes, SHA-256
`4a596b6fcf23eb2c61736e0e90bd024c2e5afcf37736d364a72f5596a19e1426`.
Its unchanged full-slice gate runs in
`.tmp/jgrapht-core-1.5.3-focused-v104-callback-read-contracts`, log
`.tmp/jgrapht-v104-callback-read-contracts-run.log`. No competing build/test
workload runs during this measurement. The accepted checkpoint remains v101
until this candidate's full diagnostic delta is audited.

V104 completes all 104 translations with zero translator/generated parse
failures. There are **1,829 actual Rust diagnostics (1,751 typed + 78 untyped)**
and **76 markers across 41 files**. Compared with v103, header/source-file
counters remove 40 diagnostics and add nine (net -31); compared with accepted
v101, they remove 47 and add nine (net -38). All thirteen newly exposed
BergeGraphInspector bool/Result errors are gone, as are both AsSubgraph generic
downcasts. Eight Result-valued sum failures and the GraphTests Result-valued
sort/collect failures also disappear through the canonical effect lookup.
Two Graphs unsupported same-trait dispatch guards disappear. These gains do
not establish a regression-free checkpoint: **v101 remains accepted**.

The nine additions are localized and audited:

- AsSubgraph.initialize: one `Option<V>.cloned()` failure. Predicate loans now
  expose a separate owned-optional recovery mismatch: the projected Rust place
  is `*v` (Option<V>), but conversion treats the borrowed slot as Option<&V>.
  Reconcile the physical outer loan with the existing ownership-recovery plan;
  add a nullable filter predicate calling a source method with an owned argument.
- Graphs: two argument mismatches (mutability and erased generic input) and one
  `dyn Any: Clone` failure after bounded-wildcard refinement stops replacing
  declaration contracts with caller-specific concrete types. Bounded source
  generic read/write projections still need a consistent ABI; do not restore
  declaration specialization merely to silence these diagnostics.
- ComplementGraphGenerator: one changed argument error at the same Graphs
  bulk-vertex call boundary (nested mutable loan/erased generic input).
- PlanarityTestingAlgorithm.EmbeddingImpl: four errors in its nested stream /
  joining fallback (retained-handle `.stream`, nullable graph argument shape,
  generic toString and missing Collectors.joining). The old tuple-getKey and
  Result-toString/into_iter failures disappear, but this path still is not a
  supported composed collector contract.

Resolve the predicate owned-optional recovery and bounded source generic
boundaries before promoting the combined materialization candidate. The next
larger allocation batch remains 39 shared-to-plain guards spread across graph
and container boundaries; preserve alias identity rather than snapshotting.
All these failures remain translator work, with pinned Java and generated Rust
unchanged by hand.

V104 takes 307.408 seconds: preflight 44.109, translation 178.480, assembly
67.899 and checking 12.717. Nested emission is 118.144 seconds; the solver
retains 238 reported iterations and 19,586 edges. These unpaired elapsed times
are not an optimization claim. The combined goal stays active: compilation,
placeholder elimination and the full graph behavior gate remain incomplete.

### Owned nullable cursor arguments and exact bounded captures (after v104)

The nullable `Option<V>.cloned()` failure reproduces with a checked source
predicate over a nullable List, both inside filter and in the following
anyMatch. The fixture now places null first, so runtime verification traverses
the null-filtering branch instead of terminating before it. V801 reproduces
both invalid `.cloned()` calls. Stream slot adaptation now recognizes the
projected Rust place behind a borrowed cursor parameter, recovers an owned
reference before optional conversion, and records that recovery to prevent a
second clone. It reuses the existing owned-argument recovery rather than
pretending an outer `&Option<T>` loan is an `Option<&T>` facade. V802 passes
all 25 materialization, retained-stream and source-callback integrations.

Bounded source call refinement now has a separate, evidence-gated path. For
each canonical argument binding, it compares the actual type to the resolved
call's instantiated bound. Only exact equality permits capture elimination;
the resulting signature retains the callee's declaration binders, not the
caller's String/other concrete witness. A wider input, an unmodeled observed
argument, or a different variance direction rejects the declaration-wide
elimination. Tests cover wider contravariant inputs, proper subtype captures,
two independent binders, package peers and a mixed exact/wider call set.
This does not claim general source variance adaptation. Argument association
uses the existing binding IDs on the selected call, replacing name-only
cross-call matching in this carrier-propagation pass.

The minimized two-way source-interface probe (put plus first) also exposes an
unnecessary lifetime obligation. Its generated `Box` trait forwarding impl
imposes `T: 'static` although it only delegates to the underlying object. It
now follows the already-established boxed clone impl policy: no additional
static bound on trait arguments. The runtime test independently exercises an
owned Box interface instantiated with a non-static borrowed string, while
anonymous/Any-retarget adapters keep their real reification requirements.
No blanket Any bound is added to callers or their unrelated generic siblings.

Permanent fixture `exact_bounded_source_calls.java` and its Java oracle
`tests/corpus/construction/ExactBoundedSourceCallsOracle.java` produce `2`.
The updated predicate/covariant Java oracle still produces `1 7 1`.
V803/v804 isolate the boxed-forwarder lifetime failure; v805's negative owner
test rejects using the broader external leaf-name matcher, so final proof
matching reuses signature lookup's qualified/simple owner-alias rules instead.
The combined v806 gate and broader v807 gate are under verification; release
build v808 is pending. V104 is still the latest diagnostic candidate and v101
the accepted incremental checkpoint, not a clean corpus compile.

V806 finishes with all 51 related integrations passing (six materialization,
26 generic binding, twelve retained streams and seven source callbacks).
V807 passes all 1,512 library tests and 31 complementary integrations (three
generic interface construction, five default inheritance, nine retained nested
collections, fourteen support-closure tests). V809 separately repeats the
updated null-first predicate runtime probe and passes. The latest batch thus
covers 82 unique related integrations, plus the independent Java oracles.
The release build `.tmp/callback-ownership-v808-release-build.log` passes.
Frozen `.tmp/new-java2rust-v105-nullable-owned-captures` is 21,094,024 bytes,
SHA-256 `39e277fbe6846556edbf4ce702c9844ea9bc99f8a7c7544c43525cf25b4de6b6`.
The unchanged 104-file run is
`.tmp/jgrapht-v105-nullable-owned-captures-run.log`, with artifacts in
`.tmp/jgrapht-core-1.5.3-focused-v105-nullable-owned-captures`. No competing
build or test workload runs during this measurement. No v105 corpus result
is claimed until the completed run and its diagnostic delta are audited.

V105 completes all 104 translations with zero translator/generated parse
failures and **1,777 actual Rust diagnostics (1,699 typed + 78 untyped)**.
Markers remain **76 across 41 files**. Against v104, header/source-file
counters remove 52 diagnostic occurrences and add zero: all nine optional
`.cloned()` errors (eight in BergeGraphInspector and one in AsSubgraph) and
43 unnecessary lifetime errors (30 in GraphIterables, ten in GraphTests,
three in GraphPath). This validates the general owned-place recovery and
boxed-forwarding lifetime fixes beyond their neutral probes.

Against accepted v101, 98 header/source-file occurrences disappear and eight
are added (net -90). The same eight additions left from v104 remain: three
in Graphs, one at its ComplementGraphGenerator caller, and four in the nested
PlanarityTestingAlgorithm stream/joining fallback. Exact-bound capture
elimination does not close those broader graph boundaries, whose observed
contracts do not all prove exact instantiations. General mixed-variance source
interface interoperability remains required; do not replace its unknown/wider
captures with bound types or caller-specific concrete types. V101 stays the
accepted incremental checkpoint; v105 is the latest measured candidate, not
a clean compile or goal completion.

After the quiet corpus run, the neutral bounded fixture is strengthened with
both String and Integer callers of the same generic method. Its Java oracle
now returns `23` instead of the initial single-witness `2`, and focused gate
`.tmp/callback-ownership-v810-multiple-witness-gate.log` compiles/executes the
updated generated Rust with the same result. This checks preservation of the
declaration frame across different concrete witnesses, in addition to the
negative wider-input veto and non-static boxed-argument test. Product source
and the frozen binary do not change during or after this corpus measurement.

V105 takes 280.832 seconds: preflight 39.928, translation 163.230, assembly
61.222, checking 12.488. Nested emission is 105.272 seconds. Solver reporting
remains 238 iterations and 19,586 dependency edges. These unpaired timings
do not establish another optimization result. The next batch should address
the remaining source wildcard/receiver ABI boundaries and the nested checked
collector/display contract; 42 allocation guards still remain (39 shared-to-
plain, three plain-to-shared). The combined goal remains active and incomplete.

### Checked joining and native map-entry read contracts (after v105)

The next batch isolates the nested stream/joining fallback without graph names.
`checked_stream_joining.java` exercises a checked mapper, a retained source,
map-entry list values, eager delimiter/prefix/suffix effects, empty traversal,
callback and argument failures, and collector-argument mutation before late
cursor binding. Its Java oracle is
`tests/corpus/stream/CheckedStreamJoiningOracle.java`. Additional probes check
fixed-size varargs-list set/add policy, null map children, ordinary entry-value
aliases and plain borrowed map children. The expected cases end in `z:2`,
`null:0` and `z`, including mutation visible through the original map.

V811 reproduces thirteen generated errors. The existing planner already admits
joining, but the terminal renderer explicitly refuses Result-bearing traversal
or mapping and falls back to non-existent retained-handle stream / Collectors
facades. Joining now has a checked form of the existing typed collect/join
projection. It maps successful items to their display strings, short-circuits
the first Java error through Result<Vec<_>>, and reuses the established
checked-or-uncaught boundary. The existing staged collector helper still owns
argument order and temporary lifetimes. The existing lazy stream factory is
reused, not replaced: the mutation oracle confirms binding after eager collector
arguments. Ordinary infallible joining keeps its prior projection.

V812 closes that guard and exposes three adjacent native boundaries. A fresh
Arrays.asList varargs allocation can now construct a fixed-size retained list
using the native factory call plan and runtime's existing list policy. A single
existing array, including a nullable array carrier, is excluded from that
construction path: it still needs its shared backing/view contract, never a
snapshot masquerading as the original view. Source factory precedence remains
owned by the common call planner. Existing single-array/nullable/enum-array
regressions are included in the focused gate.

Native map iteration now records a declaration-ID-scoped physical tuple proof
and projects the map's solved nested storage into the entry binding. Entry
accessor index selection is shared across ordinary lowering, expression shapes,
stream callbacks and storage analysis. The expression-shape projection retains
the borrowed component and its nested allocation/view metadata. Ordinary slot
lowering consumes that same shape rather than reconstructing it from Java type
alone. A method-call spelling therefore does not turn a borrowed native child
into an owned iterator source. That borrowing refinement is restricted to native
storage, not arbitrary borrowed iterator/factory protocol objects or owned-item
captured-array cursors. Live entry carriers and shadowed bindings do not inherit
the tuple proof.

V813 passes all fifteen collector/nested-collection integrations; v814 passes
1,513 library and 37 complementary integrations before the last storage-flow
work. The stronger ordinary entry-alias probe in v816 then isolates one remaining
allocation guard: retention demand on entry.getValue does not reach the backing
map's value slot. Usage analysis now records native entry provenance for direct
native-map entrySet loops and routes key/value reads into the existing storage
graph at the corresponding child path. It does not alias the outer map with
the child collection. Unknown/source entry implementations and reassigned loop
variables receive no such provenance. The reassignment check reuses lexical
write analysis and is run only after native-map and backing-slot admission,
avoiding a new body scan for unrelated enhanced-for loops.

V817 passes all 22 captured-array, materialization, nested-collection and collector
integrations, including the map-visible alias mutation. The existing nullable
generic-read helper has been moved to the shared type utilities and reused by
ordinary iteration, retained cursors and entry projections rather than retaining
parallel nullable/wildcard policies. New unit coverage checks storage provenance,
unknown/rebound entries, physical tuple proof, shadowing, native child placement
and the borrowed stream source contract. This is read/provenance support, not a
claim that all arbitrary Map.Entry implementations, aliases, setters or general
mixed-variance source interfaces are implemented.

The v818 gate passes 1,514 library tests and 58 related integrations. V819 passes
three existing single-array/nullable/enum-array idiom regressions. V820 repeats
all 21 collector/materialization/nested integrations after the nullable-array
factory admission guard. A final review moves the provenance write check behind
cheap native-map admission without changing its rule. Final verification is
v822 (1,514 library tests and 22 integrations), v825 (three existing array-view
regressions), and v826 (two joining design regressions), all passing. The release
build v824 passes and is the final frozen producer; v821 predates the last cheap
admission-order correction. Across complementary gates, 63 distinct related
integration/design tests pass; repeated runs are not additional coverage.

V823 exposed a stale joining design assertion expecting an unstaged field
delimiter. Frozen v105 already emits the eager staged form, independently
confirmed with `.tmp/join-field-staging-control.java`. The updated assertion
checks that existing match/block/let shape and nullable slot conversion; v826
passes. Separately, v827's broad
`rendered_string_heuristics_have_centralization_guardrails` test fails on old
field-naming helper spellings (`field_decl_name` and
`source_static_field_rust_name(ty, &field.name)`) in naming code not changed by
this batch. That unrelated source-text guard remains failing and untouched;
the focused gates do not imply the entire design suite passes.

The frozen v106 binary is
`.tmp/new-java2rust-v106-checked-joining-entry-views` (21,207,696 bytes,
SHA-256 `12afe20d89909ce3a9d80a9bdb7a7c4c875c10e48de7d573e698a40eaccfddc1`).
Its completed quiet full-slice run is in
`.tmp/jgrapht-core-1.5.3-focused-v106-checked-joining-entry-views`, with outer log
`.tmp/jgrapht-v106-checked-joining-entry-views-run.log`.

All 104 files translate; translator failures and generated parse failures are
zero. Cargo check still fails with **1,775 actual diagnostics: 1,697 typed and
78 untyped**. Invalid markers remain **76 across 41 files**. Against v105,
three header/file occurrences disappear in PlanarityTestingAlgorithm:
the retained child `.stream()` call, the Collectors.joining fallback, and one
argument mismatch. One occurrence is added in NeighborCache: a proven native
entry value of `Rc<RefCell<ModifiableInteger>>` now reaches `.int_value()` without
the required object-receiver adaptation. Frozen v105 used its outer-receiver
adapter there. This is a real newly exposed representation/call-boundary gap,
not an error to hide by weakening tuple provenance or copying the value.

Against accepted v101, 98 header/file occurrences disappear and six are added:
Graphs has two E0308 mismatches plus `dyn Any: Clone`; Planarity retains one
generic V `.to_string()` failure; NeighborCache has the receiver failure above;
ComplementGraphGenerator has one incorrect-arguments diagnostic. These counts
compare diagnostic header/source-file multisets, not independently classified
root causes. The 42 allocation guards remain (39 shared-to-plain and three
plain-to-shared). V106 is a measured candidate, not a clean corpus or accepted
replacement for v101.

The run takes 276.608 seconds: preflight 39.116, translation 159.995 (emission
104.479), assembly 61.243, and Rust checking 12.304. Signature saturation remains
238 iterations over 19,586 dependency edges. V105 took 280.832 seconds; these
are unpaired correctness runs, not evidence for a performance improvement.
No competing build/test was launched during this corpus measurement.

The next coherent batch should first minimize and repair source-object
receiver adaptation through proven native child shapes, reusing ordinary
object/call planning rather than adding an entry-specific method hack. Generic
Java virtual-object display and the remaining source-interface variance
boundaries need shared protocols/adapters, not blanket Rust Display bounds or
global wildcard substitution. Keep runtime alias/evaluation-order oracles and
focused negative admission tests while batching those changes; rerun the full
slice after a substantial related batch. The combined goal remains active.

### Native stored-object receiver dispatch (after v106)

The next batch minimizes v106's NeighborCache error into
`tests/fixtures/native_entry_object_receiver.java`, independent of JGraphT.
Frozen v106 produces two Rust errors for the initial fixture: a direct `.bump()`
on `&Rc<RefCell<EntryCounter>>` and `.read()` on a borrowed nullable carrier.
Control translation and rustc evidence are in
`.tmp/native-entry-object-v106-control.rs` and
`.tmp/native-entry-object-v106-control-rustc.log`. No generated output was edited.

Companion eligibility previously admitted borrowed loop elements, but not a
proven native entry getter borrowing a stored object reference. A shared
`receiver_borrows_native_stored_handle` predicate now recognizes both physical
origins, using the existing declaration-ID-scoped native tuple proof. Ordinary
borrowed parameters and unproven/custom/live entry reads are not admitted. The
predicate is reused by companion dispatch and ordinary shared receiver
adaptation; the latter is also used by the direct trivial-getter path. Existing
nominal storage placement and closed-hierarchy exclusions still apply.

Companion method selection peels the internal optional storage qualifier before
declaration lookup, while preserving the original shape for null projection.
Nullable borrowed handles go through the existing optional receiver projection
before cloning a handle. This does not copy the object payload or weaken null
checks. Infallible getters still use the ordinary borrow-based dispatch where
appropriate; mutation uses the established handle companion.

V828 confirms the mutation path is repaired but leaves the nullable getter
error. V830 (the corrected test command after v829's nonexistent test-target
typo) confirms that declaration selection alone does not fix that direct-getter
path. V831, with the shared physical proof in both paths, passes all 1,514
then-built library tests and 35 related integrations. Final coverage adds the
proof-admission unit and key/value nullable/mutation cases. The final v832 gate
passes **1,515 library tests and 44 related integrations**, with terminal exit
zero, in `.tmp/native-entry-object-v832-final-related-gate.log`. The integrations
cover captured-array identity (one), collection materialization (six), platform
collection hierarchy (two), nested collections (ten), retained streams (twelve),
source callback contracts (seven), and stream collectors (six). The new receiver
integration executes all five Java-matched cases. `git diff --check` passes.

The independent Java oracle is
`tests/corpus/construction/NativeEntryObjectReceiverOracle.java`, compiled with
the fixture. It passes and prints `10:7:9:14:11`: value alias mutation, nullable
getter, nullable mutator, key alias mutation and nullable key getter. The Rust
integration checks those same five results. No new full-slice count is inferred
from this minimized gate: v106 remains the latest measured corpus candidate.
Batch further related representation/call-contract work before paying for a
new full corpus measurement. Generic virtual-object display and source-interface
variance remain distinct general capabilities to address, not reasons to add
blanket Rust Display bounds or snapshot aliased objects/collections.

### Virtual object-string result protocol (after the native receiver batch)

The next batch starts from Planarity's generic V `.to_string()` failure, but
first checks the existing dynamic display protocol rather than imposing Rust
Display on Java type parameters. The minimized
`tests/fixtures/virtual_object_strings.java` separates an explicit Java method
result from display: `toString()` may return null, while concatenation of that
result displays `"null"`. An overriding body may also throw a catchable Java
exception. V833 reproduces both lost nullability and Rust Display panicking
through a Java catch boundary. The independent Java oracle is
`tests/corpus/construction/VirtualObjectStringsOracle.java`.

The worktree adds a nullable object-string result protocol accepting String,
Option<String>, Result<String, JavaException>, and
Result<Option<String>, JavaException>. The established display-result API
delegates to it and normalizes null only at the display boundary. Dynamic
source arms select the source method's centralized emitted name (including
inherited names) and actual storage shape, using existing handle companions for
shared objects. They do not clone object payloads or call Rust Display to execute
an overriding Java method. The non-null display API remains available as a
wrapper over the nullable protocol.

Explicit object calls, including discarded expression statements, reuse one
nullable/effect-aware lowering boundary. The broad stdlib nullability fallback
no longer treats a `toString` spelling as proof of a non-null virtual result;
known native implementations keep their stronger contract. Open object dispatch
has an intrinsic fallibility reason, propagated through the existing source
call graph. Its scoped immediate-expression scan reuses the existing walker
that excludes deferred lambda bodies. A catch of one particular exception does
not prove the entire open virtual contract handled.

TypeEnv now records declared unbounded binders as well as explicit bounds.
Method binder shadowing clears an unrelated outer bound. Unbounded/object-bound
parameters can use the shared runtime protocol without a Rust Display bound;
bounded source-trait parameters retain their existing dispatch path. The
existing source capability reachability carries the Any requirement through
selected generic forwarders, with the same scoped binder registration rather
than a second name-based type-parameter rule.

The generic key-read fixture exposes a separate overconstraint in the ordered
map runtime: read-only iteration was inside an impl requiring K: PartialEq.
Following the already partitioned semantic-map runtime, ordered-map operations
now have only their actual method-local requirements. Insert/extend retain key
equality, while construction, iteration and entry-ID operations do not invent
it. Runtime coverage also instantiates a key without PartialEq.

V834/v835 confirm that changing the runtime protocol alone does not repair the
old local non-null inference or discarded-call path. V836 catches a misplaced
editing hunk before runtime testing; it is corrected. V837 isolates two source
runtime rows rendered from nominal types instead of storage slots; those rows
now use the common slot renderer. V838 passes ten nested and six collector
integrations before isolating the ordered-map bound. V839 passes 1,515 library
tests but exposes an overly broad Any rule for a bounded source parameter and
a new unit accidentally using the text parser to construct a qualified name.
The bound rule is narrowed; the unit now constructs the qualified IR directly.
V840 catches a missing module qualification for the existing immediate walker;
it is corrected. V841 catches a misplaced raw-string delimiter in the new test
harness, corrected before v842. V842 passes 1,517 library tests and 35 existing
related integrations, plus the generic-only support closure. The larger new
fixture still exposes three real boundaries: mutable erased-object access,
shared-reference consumption when boxing, and a lost Any requirement on a
generic forwarder. These are not bypassed by changing expected Java results.

The built-in Object.toString contract now supplies explicit nullable, mutating
and unchecked-throw metadata, rather than depending on a name-only conservative
fallback. Capability reachability now carries the selected callable's generic
declarations along its worklist edges; it no longer attempts to reconstruct
method binder scopes from body pointer equality. External-facade Display
generation consumes the existing emitted-signature accessor and handles plain,
nullable and JavaException-result string contracts rather than assuming String.

TypeStorageFacts gains a monotone `borrowed_object_dispatch` requirement.
Mutating source object-string methods need the canonical interior-mutable
reference for invocation through `&dyn Any`, even without a second local alias.
The existing inherited-method selection supplies this requirement for effective
inherited methods too. This is an explicit carrier requirement, not a fabricated
local alias. Local alias collection also preserves the concrete source identity
when the assignment's declared target is an upcast or Object. The observable
alias guard admits erased boxing only with a positively proven, implemented
shared mutable source carrier; ordinary boxed payloads remain rejected.

V843 exposes a plain-String assumption in external-facade formatting; v844 closes
that but reports the mutable Object alias. V845/v846 refine canonical storage and
the alias proof, then expose the remaining move of an Rc into Object boxing.
V847/v848 show that changing the portable ownership plan alone is insufficient.
V849 catches a mistake in temporary tracing; v850 verifies the ownership policy
but initially overasserts dispatch for an unprojected nominal Object shape.
The corrected unit distinguishes nominal type information from an explicit
erased dispatch shape. V851's trace proves that the common plan requests a
reference clone while an older subtype-erasure branch boxes the local directly.
That separate boxing path is removed: it normalizes the physical erased target
and delegates to the same slot converter as other boundaries. Temporary tracing
is removed. No generated source is repaired.

V852 passes the focused shared-erasure ownership/dispatch unit. V853 passes both
virtual-string integrations, including the expanded source fixture's runtime
cases and generic-only support closure. Shared source display also reuses the
handle companion without consuming a local displayed twice in one statement.
V854 passes 1,519 library tests and exposes one outdated unit assertion expecting
a panicking null receiver projection. The assertion now checks the catchable
null projection while preserving its negative clone-recovery check. V855 passes
all 1,520 library tests and 61 related integrations. V856 also passes the existing
`erased_string_builder_display_matches_java_output` differential Java oracle.

A further minimized Java string-identity oracle exposes a physical-carrier gap:
the nullable runtime protocol must preserve JavaString references when identity
mode is selected, not always convert results to native String. V857 reproduces
six errors. V858 selects the string carrier centrally for all four result
contracts and retains JavaString identity through direct and erased calls. It
removes four errors, leaving two comparison-lowering issues. V859 generalizes
the existing nullable string comparison to a tuple match. Both operands are
evaluated left to right even when the first is null; two null references compare
equal, but distinct fresh strings with equal contents do not. The completed
source equality expression is a boolean, not a Result requiring another `?`.
V860 subsequently demonstrates that the Java operator alone is not sufficient
proof: the existing fallible numeric-comparison adapter deliberately returns a
Result block. That overbroad propagation change is not retained.

V859 passes all three virtual-string integrations. Its new independent oracle
`tests/corpus/construction/ObjectStringIdentityOracle.java` prints
`true:true:12:21:true`, covering repeated source results, String.toString returning
the original reference, both nullable-operand orders and nullable identity.
The larger fixture also passes direct/generic null results, catchable overrides,
generic forwarding, borrowed map keys, mutable object alias identity, combined
nullable/throwing results, and generic-only support closure. The final post-identity
broad gate is recorded in `.tmp/virtual-object-strings-v860-final-broad-gate.log`:
all 1,520 library tests and the first ten related integrations pass, but one
nested-collection test exposes the numeric Result distinction above. V861 adds
an operand-reassignment oracle and exposes a second pre-existing boundary:
assignment inference drops the nullable carrier, and local assignment reads its
non-Copy destination back instead of returning the converted RHS reference.
The local path now reuses the existing reference-assignment staging helper;
shape inference preserves the converted RHS carrier. String comparison
snapshots the JavaString handles, not payloads or loans of mutable locals.
The result boundary is explicitly recorded as `RustExprShape::PlainValue` and
honored by the common propagation and uncaught-recovery policies. New unit
coverage ensures the proof does not leak into a fresh fallible call using that
value as an argument. V862 checks the string and nested-collection regressions
together in `.tmp/virtual-object-strings-v862-value-boundary-gate.log`.
V862 passes all ten nested integrations and two virtual-string integrations,
but the reassignment case shows that the staging-helper edit landed in an
already-staged array branch rather than the intended local branch. The redundant
array edit is removed and the local path is corrected. V863 passes all ten
nested and all three virtual-string integrations, including reassignment;
its log is `.tmp/virtual-object-strings-v863-local-assignment-gate.log`.
V864's final related gate is
`.tmp/virtual-object-strings-v864-final-broad-gate.log`.
It completes successfully: **1,521 library tests and 62 integrations pass**.
The integrations cover captured-array identity (one), checked stream runtime
(one), materialization (six), platform hierarchy (two), nested collections
(ten), retained streams (twelve), callback contracts (seven), collectors (six),
support closure (fourteen) and virtual strings (three).
The additional historical identity filter v865 exposes four failures. The
collection identity assertion names the old one-optional closure's exact RHS;
it now checks the staged reference comparison and still executes its existing
fresh-versus-recovered identity oracle. Two regex/replace guards already reject
the nullable `Option<&JavaString>` projection emitted by frozen v106. The
control `.tmp/identity-existing-v106-control.java` enables identity mode through
an otherwise independent equality method; its unchanged run bodies reproduce
`pattern.as_ref().ok_or_else(...)` with frozen v106. The assertions now require
that physical null projection and forbid only an unprojected Option argument;
both runtime harnesses also check a null argument throws NullPointerException.
V867 passes eleven of twelve tests before catching the old negative regex
guard's prefix match against a correctly null-checked argument. That guard is
made exact at the argument separator. The final filters are logged in
`.tmp/virtual-object-strings-v869-identity-existing-final-gate.log`.

The fourth failure, `identity_string_reader_and_transformations_preserve_the_carrier`,
is a genuine **pre-existing** source FileReader-subclass adapter gap, not a
string-protocol regression. Frozen v106 reproduces the same
`Reader: From<IdentityFileReader>` error using the unchanged Java method/class
bodies in `.tmp/identity-reader-v106-control.java`; evidence is in
`.tmp/identity-reader-v106-control-rustc.log`. It remains unfixed and is explicitly
excluded from the focused identity rerun, not counted as passing. This is in
addition to the older unrelated source-text naming guard failure documented
above. No generated Rust is repaired.
V869 passes all twelve focused `identity_string` cases (excluding that reader),
then passes two of three `string_identity` cases. The remaining test still
expects three unsupported guards from before automatic identity selection.
Frozen v106 already emits only one, for two nullable array elements; control
is `.tmp/identity-slots-v106-control.rs`. The now-implemented three comparisons
are covered by a positive generated runtime test instead: shared and fresh
references, literal identity, null versus present and two null array slots.
Its final filter is `.tmp/virtual-object-strings-v871-array-identity-final-gate.log`.
All three cases in that final filter pass, including the new array-slot runtime
coverage. Together with v869, fifteen historical identity integrations pass;
the reader failure is excluded as noted above.

V870's historical Java differential filter passes eleven of thirteen cases,
including UTF-16 comparison, locale formatting, generic/default Objects string
calls, null receiver exceptions, catchable virtual errors, erased aliases,
StringBuilder display, reference identity and null joining. Two additional
**pre-existing** failures are independently reproduced with frozen v106:
`NullableMapEntryStringSwitchCatch` has the same five nested Entry-carrier/type/
nullable-switch errors (`.tmp/nullable-entry-switch-v106-control-rustc.log`),
and `SourceObjectsToStringShadow` has the same stack overflow caused by a static
toString overload suppressing the default instance method
(`.tmp/source-objects-shadow-v106-control-run.log`). The latter control uses a
separate entrypoint harness including untouched generated source; it does not
repair it. These failures remain visible and unfixed. The source naming/dispatch
collision and source-entry interoperability belong to subsequent general batches.

The post-batch release build completes in v872. Frozen candidate v107 is
`.tmp/new-java2rust-v107-virtual-object-strings` (21,177,792 bytes, SHA-256
`0567612cd9c3e01f5c67883b39349a6ef6ec8f83d8ac724dccf00c950baf2498`).
The binary is byte-compared with the release build before use. Its quiet full
104-file measurement is in
`.tmp/jgrapht-core-1.5.3-focused-v107-virtual-object-strings`, with outer log
`.tmp/jgrapht-v107-virtual-object-strings-run.log`. No competing build or test is
launched during that measurement.
The run completes with all 104 files translated, zero translator failures and
zero generated parse failures. Cargo check still fails with **1,766 actual
diagnostics: 1,688 typed and 78 untyped**. Invalid markers remain **76 across
41 files**. All 42 native allocation guards remain: 39 shared-to-plain and three
plain-to-shared. V107 is the latest measured candidate, not an accepted or clean
checkpoint; v101 remains the accepted incremental baseline.

The diagnostic-header/source-file multiset audit removes **76** occurrences
and adds **67** versus v106. It removes **172** and adds **71** versus accepted
v101. The targeted NeighborCache stored-handle receiver error and Planarity
generic V method error disappear. Many other generic V/E Rust Display failures
also disappear, along with CollectionUtil's read-only key equality requirement
and six GraphPath lifetime errors. Two older GraphWalk exception-message
projections still call raw V `.to_string()` rather than the generic protocol;
those remain unsupported and must reuse the same protocol.

The added diagnostics are explicitly not treated as harmless merely because
the net total falls. Related string boundaries include thirteen attempts to
call `ok_or_else` on a native non-null String, two facade formatters applying
Display to Result<String, JavaException>, four nullable string formatter errors,
four double-Option branch joins, two spurious try projections, one owned nullable
handle `.cloned()` and one collector inference error. For example,
AbstractBaseGraph renders a non-null builder display helper but infers a nullable
object-method result; BoyerMyrvold joins Option<String> with Option<Option<String>>.
Those are producer/consumer contract disagreements, not reasons to restore an
incorrect blanket non-null Object.toString assumption.

Other newly reached native-collection conversion sites replace invalid boxed
casts with conversions that still expose Eq/Hash, erased-iterator or carrier
mismatches. They need their own minimized identity-preserving conversion proofs;
removing the old cast diagnostic is not proof of a correct replacement.

The next coherent batch is to make native versus open virtual string contracts
and emitted callable result contracts authoritative at every display, ternary,
collector and facade boundary. Reuse the result protocol and common slot/shape
planning; add minimized native builder, nullable override/branch, facade-result
and checked-collector regressions, retaining the Java string-identity oracles.
Audit the changed collection conversions separately. Do not manually patch
generated code or expand the production slice.

V107 takes **279.667 seconds**: preflight 39.486, translation 161.818 (emission
105.479), assembly 62.088 and Rust checking 12.279. V106 took 276.608 seconds;
these are unpaired correctness measurements, not a speed improvement claim.
The signature fixpoint itself is 4.215 seconds, so this run does not justify
restarting speculative fixpoint optimization.

### Native/open string contracts and nullable branch joining (after v107)

The next batch starts from the thirteen native String `ok_or_else` additions,
the two incompatible facade formatters and nullable branch errors. The
corpus-neutral `tests/fixtures/object_string_contracts.java` tests native
StringBuilder/StringBuffer/Random calls, a nullable source overload and field,
null versus present branch results, and a generic method result returned into
Object storage. `tests/object_string_contracts.rs` translates it in both native
String and identity-preserving JavaString modes. The independent Java oracle
`tests/corpus/construction/ObjectStringContractsOracle.java` prints
`value:absent:BUFFER:true:true:present:missing:true`.

The initial frozen-v107 control exposes eight compile errors, including an
additional default-instance/overload naming collision. The fixture then
declares both overloads explicitly, matching the nullable overload seam rather
than depending on that separate naming gap; the current frozen control is
`.tmp/object-string-contracts-v107-current-control-rustc.log`. Java expectations
are unchanged. No pinned Java or generated Rust is repaired.

Call planning now distinguishes `VirtualObjectToString` on a proven unbounded
binder from the existing known-native `ObjectToString` implementation. Only
the former has the open nullable return and intrinsic virtual-dispatch effect.
Native methods retain their non-null result contract. The same typed virtual
boundary is now used by the erased-return fast path; a generic nullable string
result is converted into Object storage only after dispatch and exception
projection, rather than boxing a raw Result or a display-normalized string.

Facade Display generation now consumes the common object-string result protocol
for all four return contracts. Every formatter remains valid if assembly picks
a stronger nullable/fallible method from a peer unit; it no longer depends on
which unit's separately generated Display variant wins. The new unit
`facade_display_protocol_is_independent_of_selected_method_result_abi` compiles
and executes plain, nullable, checked, checked-nullable and throwing results.
Known native types inheriting Object's class-name display use the existing
native display-name model instead of an inferred facade stub. Source shadows
retain precedence. JavaString also joins the already shared JavaAppendValue
protocol, for owned and borrowed strings, without requiring platform-only
support in otherwise headless programs.

V873 fixes native result-shape compilation but exposes the erased-return bypass
and the missing JavaString append bridge. V874 compiles all fixture cases, then
fails real runtime oracles: native Random display uses an inferred empty stub,
and nullable branch lowering throws on a null result. Branch joining had used
the Java String declaration as proof of a non-null Rust carrier. It now consults
the solved source-call/current-field shape, lowers nullable arms in the complete
destination carrier, and reuses the existing optional slot conversion. This
applies both to nullable fields beside a literal null arm and to a nullable
source result beside a present literal; it is not a toString-name exception.

V875 passes the new dual-representation integration, all six collector
integrations and all three virtual-string integrations, including their
generated runtime tests. Its log is
`.tmp/object-string-contracts-v875-branch-gate.log`. The broader related gate is
`.tmp/object-string-contracts-v876-broad-gate.log`; its result is not inferred
from the narrower fixture. It completes successfully: **1,523 library tests and
63 related integrations pass**, including both new units. The fixture is then
expanded to check a non-null generic boxed result as well as null, and to verify
that nullable source-field return adaptation preserves JavaString reference
identity rather than allocating a fresh string. Those added runtime assertions
are verified separately in
`.tmp/object-string-contracts-v877-positive-identity-gate.log`.
That gate passes, including the nullable-field reference-identity assertion.
V878 passes all fifteen historical identity cases (with the independently
reproduced reader gap still explicitly excluded); v879 passes all eleven
selected historical Java string differential cases, excluding the two older
entry/naming failures already proved with frozen controls. Logs are
`.tmp/object-string-contracts-v878-existing-identity-gate.log` and
`.tmp/object-string-contracts-v879-existing-java-gate.log`.
The broader existing ternary filter is
`.tmp/object-string-contracts-v880-ternary-gate.log`, to check the general
branch-joining change beyond the new string fixture.
It passes fourteen of twenty-two cases and exposes eight **pre-existing**
failures. Seven are exact-output guards: nullable Boolean assignment, optional
borrowed argument, boxed-Integer branch refinement, nullable Path receiver,
primitive map getter, StringBuffer append and optional List assignment. Every
failed condition is also false in frozen v107. The eighth is the receiverless
Enum/String overload's pre-existing `Some(if ... { None } ...)` compile error.
Controls are in `.tmp/ternary-v107-controls/`; the inline Java bodies are copied
unchanged from the tests, and the map case uses the existing
`examples/idiom-gaps/MapGetTernaryPrimitiveIndex.java`.
To avoid letting a stale assertion hide a new compile error, all eight generated
fixtures are also compiled independently with both frozen v107 and the current
debug translator. Seven compile in both versions; the overload fixture has the
same E0308 in both. These tests remain unchanged and failing, not silently
excluded or counted as passing. This expands the documented older-failure
inventory; it does not establish a fully green idiom suite.

A further producer-dependency probe catches a new formatter closure gap before
freezing the candidate: a primary class with its own toString and an external
display fact can need the string-result protocol without ever selecting the
full erased-object runtime. The initial probe is
`.tmp/facade-display-support-dependency.java` with its adjacent facts/translation/
compiler logs; it also contains older implicit-print limitations and is not
mistaken for a clean baseline. The newly missing scalar helper is addressed
without pulling unrelated Any downcasts into the program.

Facade emission now returns `ExternalFacadeSupportItems`: emitted items plus an
explicit `requires_string_result_protocol` dependency, accumulated from the
selected formatter producer. Planning happens before runtime-family selection;
the facade profiling boundary moves with that work. The string-result protocol
has one shared text factory, used both inside the existing erased runtime and
as a small standalone family. The lowerer closes its JavaException/JavaClass/
enum dependencies through the normal runtime flags. There is no second protocol
implementation and no scan of generated formatter spelling to infer dependencies.

V882 passes the library check. V883 passes the native/virtual runtime matrix but
the initial isolated interface probe exposes the older static-interface dyn
compatibility bug. Frozen v107 reproduces it in
`.tmp/facade-support-interface-static-v107.rustc.log`. The dependency fixture now
uses an ordinary object-safe default instance method; the separate static-method
gap is neither hidden in the translator nor claimed fixed. V884 passes both
class/interface scalar-protocol dependency checks, including an assertion that
the complete Any runtime is absent, plus the native/virtual and collector
integrations. The final post-dependency broad gate is
`.tmp/object-string-contracts-v885-final-broad-gate.log`.
It completes successfully: **1,523 library tests and 64 related integrations
pass**. The two object-string contract integrations include the dual string
carrier/runtime matrix and the isolated scalar-protocol class/interface
dependency checks. Final post-dependency historical identity and Java string
gates are recorded in `.tmp/object-string-contracts-v886-final-identity-gate.log`
and `.tmp/object-string-contracts-v887-final-java-gate.log`.
The v881 release build predates this dependency correction and is not frozen or
used for a full-slice measurement; a fresh release build is required.
The v888 fresh release build follows successful post-dependency gates: all
fifteen selected historical identity cases and all eleven selected Java string
differential cases pass. Formatting and whitespace checks pass as well.
Frozen v108 is `.tmp/new-java2rust-v108-native-object-string-contracts`
(21,179,400 bytes, SHA-256
`fe464ed0d1724a1a0b4a4247818d1f0031cb9eb7c1dd2658b845bc98574011c2`). Its full-slice output is
`.tmp/jgrapht-core-1.5.3-focused-v108-native-object-string-contracts`, with outer
log `.tmp/jgrapht-v108-native-object-string-contracts-run.log`. No competing
compiler/test workload is launched during that measurement.
V108 completes the quiet 104-file measurement. Translator failures and generated
parse failures are zero. Cargo check still fails with **1,740 actual diagnostics
(1,662 typed + 78 untyped)**. Invalid markers remain **76 across 41 files**;
all 42 native allocation guards remain (39 shared-to-plain, three plain-to-shared).

The header/source-file multiset audit removes **27** occurrences and adds **one**
versus v107: the two facade Result<String> formatting failures, all thirteen
native String `ok_or_else` failures, Planarity's inference error, five
BoyerMyrvold type mismatches and six invalid borrows of already borrowed nodes
disappear. The added Planarity error is `.to_string()` on `&Option<String>`.
The generated callback shows its actual cause: it returns
`Ok(java_generic_object_string_result(...))` without discharging the inner
Result, so the checked mapper's output is a nested Result. Joining consequently
reads the wrong layer. This is a callback call-result/exception ABI disagreement,
not a reason to add Display to Option or patch the joining renderer by text.

Against v106, v108 removes 87 header/file occurrences and adds 52; against
accepted v101, it removes 183 and adds 56. Those counts are not independent root
causes and do not justify promotion. The BoyerMyrvold nullable append/display
joins, invalid owned nullable `.cloned()` and spurious try projections still need
work, as do the separately identified collection-conversion/variance boundaries.
V101 remains the accepted incremental baseline.

The next coherent batch should minimize generic virtual toString inside checked
stream mapping/joining, including null results and catchable overriding errors,
and make its native call-result ABI authoritative in callback effect/result
planning. Do not make lambda creation itself throw. Reuse the existing checked
stream/callback protocols, then reconcile nullable append/display branch
normalization with the same solved source result shapes. Preserve the new
native, scalar-support, boxed-result and string-identity oracles. The two older
GraphWalk implicit generic-concatenation projections also still need the common
object-string protocol; do not add Rust Display bounds to Java V.

V108 takes **277.838 seconds**: preflight 39.346, translation 161.282 (emission
104.296), assembly 60.866 and Rust checking 12.352. V107 took 279.667 seconds.
These are unpaired correctness runs, not a performance improvement claim.
Signature saturation itself takes 4.388 seconds; speculative fixpoint
optimization is not the next task.

### Checked virtual-string callback ABI and return/display ordering (after v108)

The next candidate uses one selected-intrinsic result-ABI query for open virtual
Object.toString and known native implementations. Ordinary call-result planning
and stream callback lowering consume that query; callback rendering discharges
the virtual Result before wrapping its nullable String payload in the checked
iterator protocol. Borrowed/nullable callback operands reuse the common generic
object-string receiver projection rather than adding another reference or
guessing from rendered code. Method references use the same typed projection.

The direct runtime regression is `tests/fixtures/checked_virtual_strings.java`
and `tests/checked_virtual_strings.rs`: nullable results render as `null`, an
overriding IllegalStateException remains catchable, null receivers throw NPE,
and another checked call inside the mapper must not create a nested Result.
Its independent Java oracle is
`tests/corpus/construction/CheckedVirtualStringsImmediateOracle.java`.

An initial larger regression also exposed an older, separate unsupported seam:
returning a mapped Stream from a method, then consuming that stored stream.
The complete Java reproducer is preserved unchanged in
`tests/fixtures/checked_virtual_strings_deferred.java`, with oracle
`tests/corpus/construction/CheckedVirtualStringsOracle.java`. Java passes the
creation/consumption checks, but the translator still emits an ordinary
`values.stream().map(...)` on a Rust slice and cannot lower joining on a stored
JavaStream. Frozen v108 produces five compile errors for this reproducer;
the initial callback-ABI patch removes two, leaving three deferred/stored-stream
errors. Do not count deferred translation as passing, remove its cases, eagerly
execute callbacks at stream creation, or snapshot aliased collections to hide
this gap. A future lazy-pipeline batch must wire the existing checked runtime,
propagate its physical element nullability through source signatures, and keep
creation effects distinct from consumption effects. This batch is explicitly
limited to consumed callback ABI and nullable return/display ordering, not that
larger pipeline feature.

The new nullable-join regression is
`tests/fixtures/nullable_string_joins.java`, with integration
`tests/nullable_string_joins.rs` and independent Java oracle
`tests/corpus/construction/NullableStringJoinsOracle.java`. It exercises nullable
generic source calls through array elements, branch joins, and chained fields.
The initial v890 focused gate passes the consumed callback regression, but the
join fixture exposes nullable return ownership mapping applied to Result rather
than its Option payload, plus a second `?` on an already normalized display view.
Return lowering now consumes the exception carrier before owning nullable String
elements. The OptionalString display producer records that its result is a
completed plain view, so enclosing fallible branches cannot add a second `?`.
No generated Rust or pinned Java is repaired. V891 passes 1,522 library tests
and exposes one structural expectation that must include the new plain-view
marker; emitted text is unchanged. V892 passes all 1,523 library tests and the
consumed-callback runtime integration (including the method-reference cases).
The nullable-join fixture then has one remaining String-versus-&str conditional
join, rather than the earlier invalid Option display/second-try projections.
Fallible display conditionals now join completed owned text on both branches
and record that the conditional itself is a plain value.

V893's existing method-reference guard rejects an initially overbroad matching-
owner route: it accidentally enables getter inference on an unknown external
class. The corrected context-aware route requires a proven scoped unbounded
type-parameter declaration, preserving the old unknown-owner refusal and
source/native precedence. Its existing output-shape unit now additionally
checks the nullable generic toString result. V894 passes **all 1,523 library
tests and 32 related integrations**: consumed virtual strings, nullable joins,
native/open string contracts, retained streams, source callback contracts,
collectors and virtual/identity strings. Independent javac/java oracles pass
for both new completed fixtures. Historical identity checks in v895 pass all
15 selected cases; v896 passes all 11 selected Java differential cases. Their
previously documented exclusions remain. V897's conditional-expression sweep
has exactly the same 14 passing and eight failing test names as v880; those
eight previously controlled gaps are not counted as fixed. V898 builds a fresh
release candidate after these gates (1m30s). Frozen v109 is
`.tmp/new-java2rust-v109-checked-virtual-string-abi`, 21,192,648 bytes, SHA-256
`b012aedc4b6acb3afb2471b75e6f9884dcebb8b2fc9cd9180d55fbea4f48ab43`.
Its quiet 104-file run is recorded at
`.tmp/jgrapht-core-1.5.3-focused-v109-checked-virtual-string-abi`, with outer log
`.tmp/jgrapht-v109-checked-virtual-string-abi-run.log`. No competing
compiler/test workload ran during the measurement. The runner completes with
exit 1 because generated Rust still fails cargo check, not because of a timeout
or translator failure. All 104 files translate; translator/generated parse
failures are zero. There are **1,737 actual diagnostics (1,659 typed + 78
untyped)**, **76 markers across 41 files**, and the same 42 native allocation
guards. The header/source-file multiset removes exactly three occurrences
versus v108 and adds zero. The removals are the Planarity &Option<String>
display error caused by its nested Result mapper, and both extra try operators
on BoyerMyrvold &str display views. Its two spurious bool try operators remain;
do not mistake multiset cancellation or a header count for fixing those.

The run takes **276.482 seconds**: preflight 39.484, translation 159.153
(emission 102.961), assembly 61.420 and Rust checking 12.323. V108 took 277.838
seconds; these unpaired correctness runs do not establish a speed improvement.
No promotion has occurred; v101 remains the accepted incremental baseline.

The next related batch should reconcile selected source-call result/storage ABI
across nested owners and string-consuming boundaries. Four BoyerMyrvold
conditionals still wrap nullable source method results into a second Option;
four other string consumers still format Option<String>, and one owned nullable
node still receives borrowed-option `.cloned()`. Minimize these with nested,
generic, corpus-neutral source owners and shared field/array receivers, reusing
the existing signature, source-owner and slot conversion machinery rather than
adding class-name or string-rendering cases. Also retain the two GraphWalk
implicit generic-concatenation projections for the common virtual object-string
consumer path. The completed callback/nullable-join regressions must stay green;
the escaping mapped-stream reproducer remains explicitly unfinished. Completion
still requires the entire selected slice and graph oracles, not this sub-batch.

### Selected overload/storage contracts across nested source owners (after v109)

The previous goal turn made verified progress: v109 removes three actual
diagnostics without adding a header/file occurrence versus v108. The goal is
still incomplete; optimization remains finished rather than being restarted.

The new corpus-neutral fixture is `tests/fixtures/nested_nullable_strings.java`,
with `tests/nested_nullable_strings.rs` and independent oracle
`tests/corpus/construction/NestedNullableStringsOracle.java`. It includes two
same-leaf nested Node declarations with different generic arities, a nullable
boolean-argument toString overload beside a non-null zero-argument overload,
array-element calls and a formatting consumer. The integration executes both
standalone and whole-program batch output. Java prints
`[null]:[present]:[null]`. The initial standalone probe also exposes the older
shared source-field array-write lifetime/snapshot gap in v899; the minimized
call-result fixture constructs its array in one initializer instead, without
claiming that separate mutation seam is fixed.

V901 reproduces two batch-only Option<Option<String>> joins, exactly matching
the corpus failure, plus four generic-arity failures across its two generated
modules. Return-shape lookup in emitted context tries a bare member name before
the overload family, allowing toString(boolean) to consume zero-argument
toString's result contract. It now uses the existing argument-type/fact-aware
overload selector and arity checks; a bare name cannot silently bypass them.

Raw generic argument planning previously indexed only generic declarations.
Consequently a known non-generic nested peer could borrow a same-leaf generic
declaration's erased arguments. The declaration map now records zero arity as
positive information and reuses the existing representation owner lookup for
aliases. The new representation unit checks generic, non-generic, ambiguous
and missing owners. V902 passes all **1,524 library tests and five integrations**,
including the new standalone/batch runtime probe and the v109 regressions.

The related owned-option failure leads to
`tests/fixtures/chained_reference_assignments.java` and
`tests/chained_reference_assignments.rs`, with independent oracle
`tests/corpus/construction/ChainedReferenceAssignmentsOracle.java` (Java prints
`true:14:true`). Indexed assignment returns the converted RHS temporary, not
an indexed-read loan. Shape inference now reuses its write-slot shape. The first
probe exposes a split policy: array lowering inserts nullable reference storage,
but the shared indexed assignment projection omits it. The existing lowerer
policy is moved into one shape-projection helper reused by reads, writes and
array consumers; it is not duplicated as a callback or expression special case.
The existing storage-shape assertion is updated to require nullable String
storage while retaining its read-borrow/write-owned assertions.

The same executable probe checks that mutation through the assigned object is
visible through both array slots, and that Java String reference identity is
preserved. Storage demand now accounts for assignment expressions returning a
reference already stored elsewhere; array stores charge the retained source
type rather than the enclosing owner when that type is known. Primitive
assignment results do not establish object sharing. The corresponding storage
unit checks these positive and negative obligations. Local-assignment lowering's
existing proven-present RHS optimization is preserved; the indexed result ABI
is not generalized blindly onto that distinct producer.

V903 completes with the pre-centralization assignment-shape unit failure and
four chained-assignment compile errors; its other selected integrations pass.
V904's 1,526 library tests pass, as do its nested-source, nullable-join and other
completed callback/string integrations. It catches two actionable integration
failures rather than hiding them: the historical captured-array oracle returns
1 instead of -6 after a null store, and the new chained fixture has two source
constructor/allocation mismatches. Making the shared array write shape nullable
exposed a wrong optimization: ClearOptionalTarget selects `.take()` on an
indexed read-back copy, so it clears a temporary rather than the array cell.
The clear-place proof now admits only direct local/field places; array targets
keep the existing checked mutable write path and Java evaluation/exception
ordering. A new assertion covers this negative place proof.

The array initializer's non-null payload path says it crosses an assignment
boundary but actually lowers through an argument slot. It now uses the existing
payload assignment slot, including idempotent canonical shared allocation,
before wrapping nullable element storage. This avoids constructing a bare
source payload inside an array of handles without a new allocation special
case. The next combined gate must verify the mutation/identity assertions, not
just compilation. V904's log is
`.tmp/nested-nullable-strings-v904-storage-gate.log`.
V905 (`.tmp/nested-nullable-strings-v905-final-batch-gate.log`) passes **all
1,526 library tests and 61 related integrations**, including the unchanged
captured-array oracle and all three chained-assignment runtime checks. The
historical indexed-storage gate v906 passes. V907's array assignment filter
passes seven cases and fails
`array_assignment_with_side_effecting_receiver_preserves_java_order`: one
slice-versus-Vec argument clone error and two obsolete plain-result assertions.
Frozen v109 independently reproduces the product error and already emits Result
for both methods the old harness treats as plain. The control Java, untouched
frozen Rust, translation log and rustc log are
`.tmp/assignment-order-array-receiver-v109-control.*`. This older returned-array
alias/ownership seam is not patched by copying a slice into a detached Vec, and
the failed test is not counted as passing. V908 passes all 15 selected
historical identity cases; v909 passes all 11 selected Java differential cases.
Existing documented exclusions remain. V910 builds the fresh release candidate.
No full corpus rerun is made
between these individual corrections.
V911 additionally passes the historical shared-reference array initializer
integration and the array-argument sequence-view check. Its two other selected
tests fail rendered-text assertions: external conversion adapters and nested
object-array initializers. Both independent controls produce **byte-identical
complete Rust** with frozen v109 and the fresh candidate; source inputs,
manifest, both outputs and translation logs are in `.tmp/v911-frozen-controls/`.
Those two old failures are not counted as passing or as newly introduced.

The v910 release build passes in 1m30s. Frozen v110 is
`.tmp/new-java2rust-v110-nested-source-storage-contracts`, 21,185,656 bytes,
SHA-256 `caf8b52c269f72335e186404f600a3e68f337f6ab2c8d2b71c84e4644ee0baec`.
Its quiet 104-file run is recorded at
`.tmp/jgrapht-core-1.5.3-focused-v110-nested-source-storage-contracts`, with log
`.tmp/jgrapht-v110-nested-source-storage-contracts-run.log`. No competing
compiler/test workload was launched during the measurement.
The runner completes with exit 1 for generated Rust cargo-check failures; this
is not a process timeout. All 104 files translate, and translator/generated
parse failures are zero. Actual diagnostics fall from v109's 1,737 to **1,730
(1,652 typed + 78 untyped)**. Markers remain **76 across 41 files** and native
allocation guards remain **42** (39 shared-to-plain, three plain-to-shared).

The diagnostic header/source-file multiset removes **eight** occurrences and
adds **one** versus v109: four double-Option branch joins, three further type
mismatches and the owned nullable-node `.cloned()` error disappear, all in
BoyerMyrvold. The added E0716 is at its chained shared-field array write (line
792), after the bad `.cloned()` no longer masks that write's receiver problem.
The neutral control `.tmp/shared-array-field-write-v109-control.java`, with its
untouched frozen Rust, translation log and rustc log alongside, independently
reproduces E0716 in frozen v109. That path reads/clones the field array and then
borrows the temporary copy for mutation. Extending a temporary's lifetime alone
would still mutate a detached Java array, so it is not the required fix.

The run takes **284.635 seconds**: preflight 39.380, translation 163.806
(emission 106.355), assembly 64.226 and Rust checking 13.000. Signature saturation
remains 4.327 seconds. V109 took 276.482 seconds; these unpaired correctness
measurements do not establish a performance change or justify restarting the
completed optimization phase.

Next, reuse/generalize the existing mutable-place/native-allocation machinery
for shared source-field array writes, preserving Java receiver/index/RHS
evaluation order, actual allocation identity and RefCell/Mutex guard lifetimes.
Cover both current-self companions and another shared receiver, nested/chained
stores, null/bounds exceptions and RHS reads or rebinding. Do not repair the
generated code, snapshot aliased arrays, or merely hold a cloned Vec longer.
The four nullable toString formatting consumers and the two GraphWalk implicit
generic-concatenation consumers remain candidates for the same selected-result
and common object-string protocol; the overload correction does not by itself
fix those formatting paths. Preserve the new null/mutation/identity probes and
the unchanged captured-array oracle in the next batch.
No generated Rust or pinned Java is patched. No promotion has occurred; the
full selected-slice objective is still incomplete.

## Stored source-array allocations and physical places (after v110)

The next batch addresses the frozen-control shared-field array defect, not
another optimization pass. Array fields now require retained allocation slots
(except admitted associated constants); static/thread-shared storage uses the
sync placement. The existing qualifier graph carries array-field aliases into
locals, arguments and returns. This does not retain unrelated local arrays or
equate collection value-domain transfers with outer allocation retention.
Alias auditing now consults proven field slots as well as binding identities;
a shadowing local cannot borrow a same-spelled field's allocation proof.

Checked reads and writes capture the allocation handle before evaluating the
index. Simple writes evaluate the RHS before the null/bounds check; compound
writes first read/check the old element and release the read guard before the
RHS. Only the store holds a mutable allocation guard, so RHS reads and field
rebinding do not redirect the Java place or conflict with an owner loan.
Primitive prefix/postfix updates reuse that executor. Completed compound
arithmetic is not treated as Result merely because its RHS is fallible;
integer division/remainder use the checked Java arithmetic protocol.

Scope-resolved usage also records explicit null writes to source fields from
other classes. Those writes widen the actual declaring field's nullability,
not every same-spelled field. Clearing a nullable field uses the shared physical
assignment-place path: calling `.take()` on an owned/cloned field read would
silently leave the original slot unchanged. Shared field reads preserve
catchable parent-null checks. Fresh array defaults and Java array clones must
use the solved allocation contract; Java clone copies array contents into a
fresh allocation, not the Rc/Arc handle of the original array.

The independent Java oracle and translated-runtime probe are
`tests/corpus/construction/SharedFieldArraysOracle.java`,
`tests/fixtures/shared_field_arrays.java` and `tests/shared_field_arrays.rs`.
They retain aliases, self/other-owner rebinding, RHS reads, chained null stores,
static sharing, local-only arrays and distinct simple/compound exception order.
The expanded probe adds prefix/postfix, checked division, clone independence and
argument mutation. The Java oracle passes. V914 compiled the original probe
and passed alias/rebinding behavior but exposed an uncaught parent-null panic.
V915 passed the existing captured-array and chained-reference probes but exposed
a nullable clear of a detached read. V916 passes the complete original
shared-field probe, captured arrays, chained references and nested nullable
strings, plus 1,523/1,527 library and 11/12 native-allocation cases. Its four
library failures include old physical-layout assertions and unhandled retained
array clone; its native-allocation failure rejects an overly broad outer-map
retention edge. Neither gate is counted as fully passing. The follow-up adds
the explicit fresh-clone projection and restricts the new root edge to arrays.
V917 passes all **1,527 library tests and 26 integration tests**, including all
12 native-allocation cases and ten retained-nested-collection runtime cases.
The old array-clone, owner-guard and trait-return rendered-layout assertions
now assert the retained physical contract; the new execution oracle checks
clone independence and argument mutation instead of relying only on those
spellings. The final v918 check adds source-object shallow-clone aliasing and
explicitly stages cloned contents before releasing their allocation handle.
V918 passes all three selected clone library tests and all **40 integration
tests**, including the expanded source-object shallow-clone execution and 14
support-closure cases. The independently compiled Java oracle passes all cases
as well. Formatting and `git diff --check` pass. V919 builds the frozen release
candidate for the single full-slice measurement after this related batch.
Logs are `.tmp/shared-field-arrays-v91{4,5,6,7,8}-*.log`.

No new full-slice measurement or promotion has been made for this batch yet.
Generated Rust and pinned corpus Java remain untouched.

## Completion and expansion

The selected slice is not yet translated successfully: emitted code still has
invalid placeholders and compile errors. Representative graph construction,
traversal, mutation and shortest-path results must still be matched against the
established Java oracle. Do not expand
the emitted production scope until those gates pass and the established
translator/corpus gates have been checked. Broader source context can be used
earlier when it is needed to diagnose a general resolution rule.
