# Selected smaller corpus: Commons RNG 1.0 source64 generators

Pinned [Apache source](https://github.com/apache/commons-rng/tree/RNG_1_0),
commit `4581a4520315657a4219b37c81f5db80e4a4e43c`, annotated tag RNG_1_0.
Apache-2.0 licensed. Version1.0 is intentional: a bounded Java6-era numerical
target, not a claim of support for the latest release or the whole project.

## Fixed scope

**SplitMix64, MersenneTwister64 and XorShift1024Star, including their complete
11-file source dependency closure** listed in
`scripts/focus-sets/commons-rng-1.0-source64.txt`. Include every method in those
files: primitive generation, bounded draws, byte filling, seeding, state
save/restore and conversion helpers. Both abstract base classes and all source
interfaces remain in scope. No manual Rust or Java repairs.

This is a selected generator component, not the full commons-rng-core module.
TwoCmres, source32, JDKRandom serialization, simple factory/reflection,
sampling, examples and benchmarks are outside this selected component.
Do not later shrink the11 files to just nextLong to manufacture completion.

Prepare and run:

```sh
bash scripts/prepare-commons-rng-1.0.sh
NEW_JAVA2RUST_BIN=... bash scripts/run-commons-rng-1.0-source64.sh
```

The runner checks commit/cleanliness, byte-compares staging and requires11
files. Source files are copied from the unchanged client-api/core modules.

## Current checkpoint

**V158: the agreed11-file component passes.** All files translate and parse,
assembled Rust passes cargo check, and there are **zero invalid markers** or
allocation guards. The generated-code driver matches all **2865 Java output
lines**, including identical floating-point bits, state replay, byte filling,
seed/state defensive copies and invalid-input exception checks. Final library
gate **1614/0**; focused inheritance, array and call-staging gates pass.

General fixes cover retained primitive-array copying/aliasing, owned rebound
parameter handles, defensive-copy producers, explicit-constructor call ABI,
proven declaration-only abstract roots and copied array-length guard lifetimes.
No generated Rust or pinned Java was repaired. Baseline V152 had56diagnostics
and19markers/7files; V155 had16abstract-root diagnostics; V156 compiled but its
first behavioral run exposed the lifetime bug. Historical sections below retain
those intermediate observations. This is support for the fixed three-generator
component, not a claim about the full Commons RNG project or parked JGraphT.

## Evidence and feasibility gate (2026-10-20)

JGraphT checkpoint is complete; string-similarity and JAMA were rejected
after actual baselines/generated audits (see their separate documents), due
to UTF-16/local-class and primitive-matrix-row representation redesigns.
Commons RNG instead exercises existing scalar, one-dimensional retained-array,
source-inheritance and interface-adapter representations.

First measured entire core+API31files as comparison:
`.tmp/commons-rng-1.0-v152-baseline`, terminal1, translator0/parse0,
cargo_type_errors_valid1,343 compiler diagnostics,39.602s. This broader module
is not the selected scope. It includes JDK serialization and unrelated
source32/static/nested-class mechanisms, unlike the three-generator component.

Then measured the exact11-file closure:
`.tmp/commons-rng-1.0-source64-v152-baseline`, runner log
`.tmp/commons-rng-1.0-source64-v152-baseline-run.log`, terminal1.
Translator failures0, parse failures0, cargo_type_errors_valid1,
**56diagnostics**, registry markers19/7files, no-op facade call sites0.
Total13.989s: resolver0.358, translation2.659, assembly8.196,
check1.637, sweep0.521. Same frozen V152 release, SHA256
`7692d388eb4fc976b3bb0f526a301474dffc84a8b57c558c77051167336b1e81`.
Counts are not independent root-cause counts or an acceptance claim.

## Generated-rewrite audit and coherent batches

1. **Retained primitive-array boundaries** (32 allocation guards plus direct
   handle len/index/as_mut_slice failures). NumberFactory.makeByteArray and
   BaseProvider.fillState pass retained source/destination arrays into
   System.arraycopy's plain slice contract. Rewrite the intrinsic adapter,
   not allocation identity: evaluate operands once in Java order, validate
   bounds/nulls, copy the source element range before mutably borrowing an
   aliasing destination, and reuse existing checked arraycopy semantics.
   Arrays.copyOf must allocate fresh arrays; Arrays.fill must update the
   existing allocation. Reuse physical-slot projection for handle len/index.
   These are cross-function/intrinsic contracts, not new representations.
2. **Reassigned borrowed parameters** (MersenneTwister64 seed). Generated
   `seed = &Rc::new(...)` borrows a statement temporary, and reassignment
   attempts mutation through Rc. A Java parameter that is reassigned must
   use an owned local handle initialized from the incoming handle, not a
   persistent borrow. Propagate this through signature/body-local shape and
   existing alias lowering; clones must clone handles, not seed payloads.
3. **Abstract inherited dispatch and companions** (11 ambiguities and3
   incomplete trait implementations). BaseProvider/LongProvider are abstract
   but emitted concrete/Rc interface implementations include partial or
   incompatible methods. Do not select an arbitrary same-named trait method
   or insert panic bodies. Use declaring-source method identity and concrete
   subclass emission inventory. Preserve copied/inherited concrete bodies
   and dynamic receiver identity, and avoid pretending a non-instantiable
   abstract storage struct provides unresolved methods. Check both generated
   plain and shared-self bodies. This is emission/dispatch contract work;
   existing flattened source bodies/adapters are relevant. Re-audit if this
   cannot be represented without a new abstract-object framework.
4. **State and numeric consumers**. Generated SplitMix64.next already uses
   wrapping add/multiply and unsigned shifts. Verify rather than rewriting it.
   State is boxed source-interface data whose as_any/downcast must agree on
   Rc payload identity; getState/constructor copyOf make defensive copies.
   Verify restore dispatch reaches each concrete override, not the base
   UnsupportedOperationException body. No escaping closures or Java method-
   local classes occur in the selected scope; mutable escaping interface
   receivers and saved-state objects are the corresponding hard cases.

Decision: select this bounded component. Required hypothetical rewrites fit
the existing retained-array and concrete source-adapter model, with no
lossless-string or matrix-storage migration. This is a feasibility judgment,
not proof all56 errors will disappear after one batch. Preserve the full
scope and repeat the whole-path audit if deeper dispatch gaps emerge.

## Acceptance / oracle requirements

All11 files must translate/parse without failures or invalid placeholders,
assemble and cargo check. Independent Java runs must cover all three generator
sequences (multiple seeds), inherited bounded and primitive APIs, byte-fill
offsets/tails, save/restore replay and defensive-copy behavior, malformed state
and invalid bounds, and seed input aliasing. Use exact integer/byte results;
explicit floating comparison rules for generated unit-interval values. Run
relevant retained-array, interface, inheritance, exception and numeric focused
regressions plus a final full-scope gate. No generic placeholder facade may
replace any source implementation.

Independent reference gate now passes: `tests/corpus/rng/GeneratorOracle.java`
compiled with all11 untouched production sources, terminal0, **2865 output
lines**, ending `oracle=ok`, no compiler/runtime stderr. Output
`.tmp/commons-rng-1.0-java-oracle/output.txt`. Reproduce with
`bash scripts/run-commons-rng-1.0-java-oracle.sh`. Includes five scalar seeds,
all three generators, MT empty-seed parameter reassignment,64 raw draws per
case,16 save/restore replay checks, bounded/boolean/float/double APIs,
byte-fill lengths0through17 with sentinels, input seed isolation, state
defensive copies, conversion round trips and invalid bounds/state exceptions.
This is independent Java evidence only: generated Rust behavior is still
unverified and the selected translation is not yet supported.

Next implementation batch: retained primitive arraycopy/copyOf/fill adapters
plus body-local ownership for reassigned array parameters. Follow with
abstract/inherited dispatch and full component/oracle verification. Do not
rerun the JGraphT stress slice for each fix in this new campaign.

## Retained primitive-array contract batch in progress

Production changes now stage same-element primitive retained-array System
arraycopy operands as allocation references (all five Java expressions once,
before null checks/borrows), snapshot primitive source contents and release
its guard before a possibly aliasing destination write. Reuses the checked
System.arraycopy_result range helper and local/synchronized storage
projections. Non-retained/mixed/reference-array cases retain their existing
paths; no guard is silently replaced by a payload copy at a source boundary.

The existing retained native-container alias helper now also admits arrays
when their complete physical Rust carriers match. It clones the Rc/Arc
reference, never the array payload. System arraycopy-only users no longer
introduce stream constants without requesting the platform I/O runtime.

The shared emitted_source_method_param_shape contract owns handles for
mutated/rebound retained array parameters. Existing mutable-parameter shadow
emission can therefore retain a fresh allocation rather than borrowing a
statement temporary, while writes before rebinding still affect the caller.
Initial regression verification exposed call sites falling back to older
borrowed signature plans; exact emitted-declaration argument plans now take
precedence there too. Source-declared System.arraycopy is excluded from both
the new adapter and the older checked-intrinsic dispatch. Staged index locals
are explicitly i32, avoiding Rust unsigned inference for negative indices.

Maintained fixture `tests/fixtures/retained_primitive_array_intrinsics.java`
and its independent Java oracle pass with0. It exercises both overlap
directions, single argument evaluation, source mutation during argument
evaluation, invalid ranges with no writes, caller-visible parameter mutation,
and rebinding an empty array without rebinding the caller. Focused Rust test
also checks source-defined System precedence by generated execution.

V1 test build44.91s; generated runtime0/1 exposed missing stream support and
an array-field alias move. V2 build56.14s; original overlap runtime1/0 in11.35s.
V3 build50.13s; expanded tests expose the call-ABI/source-owner/index issues
above, retained rows0/1 with the same borrowed/owned mismatch, array argument
nullability1/0. V4 combined build is running under session19607, log
`.tmp/rng-array-contracts-build-v4.log`. Do not treat V2's narrower passing
fixture as verification of the full current source. The11-file corpus's
latest measured diagnostic count remains56; no newer corpus run is claimed.

V4 build is **terminal0**,49.02s. Generated primitive-array and source-System
execution **2/0** (13.78s), retained array rows **1/0** (14.36s), argument
nullability **1/0** (0.48s). Logs
`.tmp/rng-array-{intrinsics-runtime,rows,argument-nullability}-v4.log`.
Expanded Java oracle remains0; no pinned/generated source was patched.
The exact emitted declaration's parameter plan now precedes older signature
fallbacks, and both the old/new native arraycopy dispatches defer to a proven
source declaration. This closes the V3 follow-through regressions without
removing assertions or exempting retained rows.

Building the current CLI for one11-file diagnostic measurement next, using a
frozen debug candidate to avoid an unnecessary optimized-build cycle. It is
not an isolated performance comparison with the release V152 baseline.
Broader library/interface/constructor/staging gates remain to be run at the
next substantial integration boundary; the current focused passes are not
claimed as full translator regression acceptance.

### V153 diagnostic measurement and production-edition follow-through

Frozen debug candidate `.tmp/new-java2rust-rng-array-contracts-v153-debug`,
202,298,368bytes, SHA256
`216cb29c5c0ecbb9d35b362510a2b211be4003d8553c99747b220d4216223c90`.
CLI build reused current artifacts,0.03s. Output
`.tmp/commons-rng-1.0-source64-v153-array-contracts`, terminal1:
all11 files, translator0/parse0, cargo_type_errors_valid1, **41 diagnostics**,
registry markers **5/3files** (baseline56diagnostics,19markers/7files).
Total43.810s, translation27.323, assembly8.243, check1.706, resolver3.486,
sweep0.514. Debug versus release timings are not comparable speed evidence.

The temporary seed reference, Rc as_mut_slice and shared-to-plain allocation
guards are gone. Five plain-to-shared allocation guards remain, along with
abstract dispatch/companion failures and copyOfRange physical projections.
Newly exposed:14 E0597s in arraycopy's generated destination borrow. Tests
used edition2024, but production corpus crates use edition2021 with different
tail-temporary drop order. Fix the producer, not the corpus edition: bind the
checked call Result to a local statement so the guard is dropped before the
captured allocation locals, then consume the Result at the block tail.

V5 source contains that follow-through. The shared generated-crate verifier
now accepts an explicit edition while preserving its old default; both new
arraycopy execution tests explicitly request production edition2021. Build
session75614 is running, log `.tmp/rng-array-contracts-build-v5.log`.
V153 predates the lifetime fix;41 remains the measured count until rerun.

V5 build **terminal0**,45.01s. Explicit Rust2021 generated execution **2/0**
(11.48s), retained rows **1/0** (11.92s), argument nullability **1/0** (0.20s).
Logs `.tmp/rng-array-{intrinsics-runtime,rows,argument-nullability}-v5.log`.
The verifier's existing edition2024 default is unchanged; the new regression
chooses2021 explicitly. No generated code or pinned source changes.

V153 multiset comparison records30 removed and15 added diagnostic occurrences
(27 allocation guards removed;14 of15 additions are the now-focused-tested
Rust2021 lifetime family). Read-only comparison
`.tmp/rng-v152-v153-diagnostic-delta.txt`. One new mismatch is an explicit
constructor calling the rebound-array source method through an older borrowed
argument plan. That boundary is not covered by the implicit-constructor
fixture and remains open; fix the constructor's source-call ABI, not the
generated MersenneTwister64 caller. Arrays.copyOf/copyOfRange still consume
the retained carrier as though it were the element slice, yielding a Vec of
handles or a plain Vec where a fresh retained array is required. Those
producer/consumer contracts and abstract dispatch form the next batches.

V154 confirmation is running under session46854 after all focused tests
terminated, root `.tmp/commons-rng-1.0-source64-v154-array-contracts`, log
`.tmp/commons-rng-1.0-source64-v154-array-contracts-run.log`. Reuses applicable
resolver evidence (no resolver changes); still validates pins, translates all
11files, assembles and cargo checks. No competing build/test workload.

V154 is now **terminal1** solely due to generated Rust diagnostics:
translator0, parse0, cargo_type_errors_valid1, **27=22typed+5untyped**,
allocation guards5, registry markers5/3files, no-op facade call sites0.
The14 E0597 lifetime errors are gone. Relative to V152:30 header/file
diagnostic occurrences removed and1 added (the constructor argument ABI),
net-29. Comparison `.tmp/rng-v152-v154-diagnostic-delta.txt` is read-only;
these are not independent bug counts. Full selected scope remains unchanged.

Frozen current candidate `.tmp/new-java2rust-rng-array-contracts-v154-debug`,
202,298,976bytes, SHA256
`979ea294e3413c8d63dd981d5321cb3bedf3626a116e94d14ce0b7fc10cc1b14`.
Total39.595s: translation25.559, assembly9.371, cargo check1.714,
sweep0.525. Debug measurements do not establish a performance delta against
the optimized V152 baseline. No corpus/build/test process remains live.

## Fresh copy / constructor batch (after V154)

New lowering captures copyOf/copyOfRange arguments before borrowing the
physical array payload. Shared inputs clone their allocation handle; plain
inputs lend their actual value. Bounds are staged as Java i32 values, so a
bound expression can mutate the retained source without a live guard.
The existing checked copy helpers perform element copying/padding and bounds
checks; the array-clone destination wrapper is factored out and reused to
attach a fresh Rc/Arc only to proven new allocations. Aliases remain on their
separate identity-preserving path. Initial admission covers rank-one primitive
arrays; reference arrays retain their established lowering.

Constructor trailing instance calls now reuse the emitted resolved source
argument contract and the selected callee's Result ABI instead of a raw
solver argument plan/name heuristic. No generated constructor is repaired.

Focused fixture `tests/fixtures/retained_array_copies.java` covers byte and
long copies, defensive constructor/getter copies, padding, bound evaluation
mutating the source, invalid ranges/sizes, and an explicit constructor calling
a helper that rebinds an empty seed. Its unchanged independent Java wrapper
prints **0:0**. A second generated execution control proves a source class
named Arrays keeps copyOf's own aliasing behavior, not the JDK semantics.

V1 build48.26s. New Rust2021 runtime1/1: source precedence passes but catching
an invalid range from a source helper fails because native copy effects were
not seeded in exception planning. Existing intrinsics2/0, rows1/0 and
constructors3/0 pass. V2 adds NativeArrayCopyThrow via the shared typed
immediate-effect traversal and existing native static-call classifier/JDK
subtype catch rules, including synthetic callable bodies. Bounds/size/null
effects are distinguished, and parsed source owners remain excluded.

V2 build49.69s, all generated execution passes: copies/source precedence2/0
(19.83s), intrinsics2/0 (19.06s), rows1/0 (20.61s), constructors3/0 (23.53s).
These overlapping gate timings are not isolated performance measurements.
Logs `.tmp/rng-array-copies-{runtime,intrinsics,rows,constructors}-v2.log`.

V3 adds a focused planner control for unhandled/RuntimeException-handled
native effects and source-owner exclusion. It also rejects non-copy method
names before expensive planning, and keeps fresh array slot adaptation scoped
to array destinations. Combined integration/library build is live under
session47237, log `.tmp/rng-array-copy-build-v3.log`. V154/27 diagnostics is
still the latest full component measurement; no newer acceptance claim.

V3 combined build **terminal0**,86s, exact native-copy effect/source-owner
planner control **1/0** (0.01s). Current copy/constructor/source-Arrays runtime
**2/0**; log `.tmp/rng-array-copies-runtime-v3.log`. Full library is running
under session7273, log `.tmp/rng-array-copy-library-v3.log`. Earlier V2
intrinsics2/0, rows1/0 and constructor3/0 remain applicable prior evidence;
V3's extra filters do not change their paths. All gate processes except the
library suite are terminal. No fresh corpus measurement or completion claim
until the library results are collected.

V3 library **1608pass/1fail** (95.59s). The sole failure is the old rendering
assertion `shared_self_field_mutable_argument_uses_mutable_owner_guard`, which
expects fill(&Rc) even though the shared emitted ABI now takes an owned
handle. Generated code passes an Rc clone and mutates the original allocation,
as independently verified by the array mutation/rebinding runtime controls.
V4 updates that assertion to require the owned parameter and Rc-cloned field
argument, retaining all original negative/checked-read assertions. Adds an
OuterArrayArgument runtime case with an inner class to exercise the exact
shared-outer companion path. Expanded Java reference prints **0:0:0**.

V4 combined build **terminal0**,61s, log
`.tmp/rng-array-copy-build-v4.log`. Production implementation is unchanged
from V3; only test expectations/coverage changed. Reuse the1608 passing V3
cases, rerun the renamed assertion exactly and the expanded generated runtime
instead of another identical full library cycle. Exact assertion log
`.tmp/rng-array-copy-owner-unit-v4.log`; generated runtime
`.tmp/rng-array-copies-runtime-v4.log` is running under session from the
current tool result. This is not a claim of a fresh all-green library run.

V4 exact updated owner-handle unit **1/0** (9.88s) and expanded Rust2021
generated runtime **2/0** (12.54s), including the inner-class/shared-outer
case. Both handles are terminal0. Together with the1608 unchanged V3 unit
passes and V2 integration gates this closes the sole obsolete assertion
without claiming a second full library run. Current source is ready for one
unchanged11-file measurement, V155 debug, after all gates terminated.

## V155 checkpoint and next abstract-root audit

V155 is **terminal1** due to generated Rust only. All11files, translator0,
parse0, cargo_type_errors_valid1, **16typed diagnostics**, allocation guards0,
registry markers0/0files, no-op facade call sites0. Compared with V154,
11header/file diagnostic occurrences removed and **zero additions**; read-only
comparison `.tmp/rng-v154-v155-diagnostic-delta.txt`. The source constructor
ABI and five fresh-copy guards are closed in this complete unchanged scope.

Frozen debug candidate `.tmp/new-java2rust-rng-array-copies-v155-debug`,
202,416,424bytes, SHA256
`557290237c3867843b8cd31416e5ca7ef73f425dba0fac7c55d49cd16b5663ab`.
Output `.tmp/commons-rng-1.0-source64-v155-array-copies`, runner log
`.tmp/commons-rng-1.0-source64-v155-array-copies-run.log`.
Total43.821s: translation28.949, assembly9.852, check1.723, sweep0.833;
resolver benchmark skipped with unchanged applicable evidence. CLI build0.03s.
No performance claim from these debug-versus-release observations.

Remaining whole-path evidence:

- BaseProvider's concrete-looking inherent methods call unresolved virtual
  interface slots. Flattened RestorableUniformRandomProvider and its parent
  UniformRandomProvider both expose them; Rust sees ambiguous methods.
  Arbitrarily selecting either existing stub would compile but not implement
  Java virtual dispatch. Concrete generator copies need their own overrides.
- LongProvider.next is abstract, yet its shared companion takes Rc<LongProvider>
  and tries to call an &mut self method. Immutable Rc cannot supply that loan;
  cloning the payload would lose reference identity/state.
- In representation_lowering.rs, trait_impl_for_type uses filter_map over
  trait_impl_method, whose immutable_shared_receiver branch returns None for
  mutable implementations. It then emits an incomplete Rc trait impl anyway.
  A trait adapter needs an all-method capability proof, not partial emission.
- In the selected pinned Java, BaseProvider/LongProvider occur only as class
  declarations, imports and extends edges, not typed fields/parameters/returns
  or explicit instances. Existing lowering materializes their instance bodies
  on concrete generators. Static helpers (LongProvider.nextBytesFill and
  BaseProvider bound/state utilities) must still be emitted and used.

Before editing this family, decide the abstract declaration/materialization
contract: use existing source hierarchy/adapter mechanisms for runtime
polymorphic receivers, and emit only support which can satisfy its complete
receiver contract. A declaration-only abstract root is not a concrete Java
object. Any demand-based omission must prove its instance bodies remain fully
materialized on concrete children and that static/constructor/lexical-super
support survives; it must not merely remove failing functions or interfaces.
Include negative controls where an abstract base is used as a typed value,
and executable inherited virtual calls/save-restore/byte filling. If existing
representations cannot provide that whole-path proof, surface the gap before
substituting a partial adapter or growing another local-fix chain.

The11-file scope and all behavior requirements remain unchanged. Cargo check
and the generated-vs-Java2865-line behavior gate are still incomplete. All
build/test/corpus handles are terminal; goal remains active.

## V156 abstract declaration/materialization checkpoint

General admission lives in `src/abstract_declaration.rs`, recorded once in the
representation plan. Admit only stateless abstract class declarations with
concrete source descendants, no constructors/nested members, no runtime typed
uses, and a fully admitted abstract ancestor chain (or Object). Unknown/opaque
uses veto admission. Typed fields/parameters/returns/locals, array elements,
casts, instanceof, generics, lambdas and anonymous/local subclasses are negative
controls. Runtime-valued abstract bases keep their existing representation.

Admitted roots retain their struct/type token, constants, static helpers and
storage support. Existing child method materialization and lexical-super
inventories remain authoritative; unsupported raw instance constructors,
companions, partial adapters and erased runtime/display registrations are not
invented for those declaration-only owners. Concrete descendants remain in
erased clone/identity/display inventories. No RNG names are special-cased.

Verified V2: classifier3/0, erased-runtime inventory1/0, executable inheritance
regression1/0 (14.85s, Rust2021), lexical-super4/0 (33.39s), full library
**1613/0** (77.71s). Java inheritance oracle prints0. Library evidence here is
before the subsequent array-length lifetime correction.

Frozen V156 debug `.tmp/new-java2rust-rng-abstract-v156-debug`,202804520bytes,
SHA256 `9087ab613785329b854b94cef172cd066a2f74c0a41b30b017165d205cdc3cb8`.
Output `.tmp/commons-rng-1.0-source64-v156-abstract`, terminal0. Full11files,
translator0/parse0, cargo check ok/0errors, invalid markers0, no-op facade call
sites0. Total45.150s: translation30.158, assembly9.861, check1.803, sweep0.591;
resolver benchmark skipped with applicable prior evidence. No concurrent tests
or builds competed with this measurement. This is not a release speed claim.

`scripts/verify-commons-rng-1.0-behavior.py` builds the generated crate unchanged
and separately links `tests/corpus/rng/GeneratorOracle.rs` as a test driver.
It invokes the independent Java reference, compares all output keys/integer/
byte values exactly and floats by identical IEEE bits (decimal spelling can
differ). Three cheap comparison tests reject changed values, reordered keys,
one-ULP differences, signed-zero changes and incomplete runs.

The first real V156 driver run compiled but panicked in LongProvider's byte
writer: `nextBytesFill(this, bytes, 0, bytes.length)` lowered its last argument
to `bytes.borrow().len() as i32`, keeping a Rust temporary Ref alive through
the entire callee. Correct Java rewrite is a copied scalar evaluated before
entry, not a cloned byte payload or suppressed write. The general array-length
producer now reuses `staged_owned_result_block_projection` to bind the scalar
inside a block before returning it. This covers shared native array carriers,
including optional receivers and mutex-backed placements through existing
receiver/null projection. Expanded inheritance fixture exercises the same
length argument followed by callback-driven writes. The subsequent V3 gates
and V157/V158 complete behavior runs pass, as recorded below.

## Final V158 acceptance and handoff (2026-10-21)

Frozen debug translator `.tmp/new-java2rust-rng-verified-v158-debug`,202814864bytes,
SHA256 `c9de79095f43ca9de9b0cb705a1bf3480154aef296e773611293918d2e366466`.
Final output `.tmp/commons-rng-1.0-source64-v158-verified`; runner log
`.tmp/commons-rng-1.0-source64-v158-verified-run.log`, terminal0. All11files,
translator0/parse0, cargo check ok/0errors, invalid markers0/0files, allocation
guards0, no-op facade call sites0. Total43.120s: translation29.714, assembly8.074,
check1.582, sweep0.489. No competing tests/builds during the corpus measurement;
resolver benchmark reused. These debug timings are not a release speed delta.

V157 already passed the complete Java/generated Rust oracle. V158 adds the
conservative enclosing-instance-layout veto and its negative control; generated
`src` trees are byte-identical between V157 and V158 (`diff -qr`, terminal0).
The final V158 oracle was nevertheless rebuilt and rerun, **terminal0**, with
**2865 matching lines, identical IEEE float/double bits** and both completion
sentinels. Evidence: `.tmp/rng-behavior-v158.log` and the final output's
`behavior/{result.txt,java-output.txt,rust-output.txt,rust-stderr.log}`. Generator
logic is exclusively the unmodified translated source. The separate Rust file
is a test caller, not a replacement implementation.

Final relevant verification:

- Full library V4 **1614/0**,87.66s, `.tmp/rng-abstract-lib-v4.log`; this includes
  four abstract-admission controls, erased runtime inventories and existing
  source-ABI/exception/representation tests.
- Expanded Rust2021 inheritance/length/callback regression V4 **1/0**,35.56s,
  `.tmp/rng-abstract-runtime-v4.log`; independent Java wrapper prints0.
- V3 array-intrinsic runtime **2/0**,11.55s, and source-call staging **4/0**,
  20.77s, `.tmp/rng-abstract-{intrinsics,staging}-v3.log`. V4's only subsequent
  semantic change is the stricter captured abstract-owner veto; these fixtures
  have no such owners. Earlier lexical-super **4/0** covers non-abstract owners
  and retains applicable evidence. Prior retained-copy/row/constructor/deque
  gates remain recorded above and in the JGraphT checkpoint.
- Python comparison gate **3/0**, `python3 scripts/test_rng_behavior_comparison.py`;
  full-run verifier also requires2865lines and clears stale success reports.
- `cargo fmt --all -- --check`, `git diff --check` and pinned checkout status
  checks pass. No commits, user-change resets or source/generated repairs.

Reproduce with a freshly built translator or the frozen candidate:

```sh
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-rng-verified-v158-debug" \
  NEW_JAVA2RUST_BUILD_PROFILE=debug \
  RNG_TRANSLATION_ROOT="$PWD/.tmp/commons-rng-source64-recheck" \
  bash scripts/run-commons-rng-1.0-source64.sh --skip-resolver-bench
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/commons-rng-source64-recheck
```

The earlier goal's deque/source-ABI/static-owner batch and one104-file JGraphT
checkpoint are complete and recorded in `docs/jgrapht-core-corpus.md`: V152,
1054diagnostics,144removed/48added vs V151,74markers/31files. JGraphT remains
unsupported and parked; it is not represented as a passing corpus. Alternative
corpus audits and the fixed RNG scope remain intact. The selected smaller
corpus acceptance requirements are now satisfied; all recorded handles are
terminal and there is no remaining required implementation step in this goal.
