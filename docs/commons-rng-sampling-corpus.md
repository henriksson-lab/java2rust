# Commons RNG 1.0 sampling extension

Pin `4581a4520315657a4219b37c81f5db80e4a4e43c` (RNG_1_0), untouched Java.
Purpose: improve the translator through general fixes, not hand translation.

## Accepted checkpoint

V168 completes the fixed 46-file sampling goal. All production Java remains
untouched. All files translate, parse and cargo-check; unsupported/invalid
markers and no-op facade call sites are zero. Independent final-producer gates
pass: collection162, distribution2172, native-generator12601 and source642865
lines. Every floating-point comparison is exact IEEE; no tolerance was added.
Full library1626/0 and focused executable contracts13/0 pass. The detailed
final audit and reproduction commands are at the end; earlier sections record
historical baselines and failures, not the current acceptance status.

## Fixed scope and staging

The complete scope is **46 production files** in
`scripts/focus-sets/commons-rng-1.0-sampling-all.txt`: all22 sampling production
files except package-info.java, plus all24 accepted native-generator files.
Stage1 is the27-file `commons-rng-1.0-sampling-collections.txt`: all24 generators
plus PermutationSampler, ListSampler and CollectionSampler. Stage2 adds all19
distribution production files, including callbacks/interfaces, SamplerBase,
InternalUtils and InternalGamma. All methods, constructors, private helpers and
nested declarations remain in scope. Finishing Stage1 does not complete the goal.

Excluded: factories/reflection, JDKRandom/serialization, TwoCmres, GUI, examples,
benchmarks and newer versions. This is not the entire Commons RNG project.

`scripts/run-commons-rng-1.0-sampling.sh` defaults to the full46 files;
`RNG_SAMPLING_STAGE=collections` selects the fixed27-file first stage. Separate
staging/output roots prevent accidental scope mixing. Both reuse the existing
runner's pin/cleanliness checks, byte comparisons and exact file counts. The
manifest-to-module mapping is shared with the independent Java oracle runner.
Original source64/native defaults remain unchanged.

All46 production files compile unchanged with javac, establishing dependency
closure independently of translator inference. Evidence
`.tmp/rng-sampling-java-closure-v1.log` (terminal0), Java compile log in
`.tmp/commons-rng-1.0-sampling-java-closure/javac.log`. That invocation runs the
existing generator oracle only; it is **not sampler behavioral acceptance**.

## Stage1 entry baseline

Accepted V161 producer `.tmp/new-java2rust-rng-native-support-v161-debug`, SHA256
`d884f9652197b3f9f7c7b8c6c4ce85af9bd6568238d980ae95d46247aa52fd8d`.
Run `.tmp/commons-rng-1.0-sampling-collections-v161-baseline`, terminal1 due to
generated Rust. All27 translate and parse, unsupported statements/members/
expressions0, invalid marker files0, no-op facade call sites0, **5 compiler
diagnostics**. Total138.404s includes11.468s resolver benchmark,90.933s
translation,23.582s assembly and7.277s check. Diagnostic counts are not a plan
and do not expose every behavioral defect.

## Whole-path rewrite audit before implementation

1. **Retained collection default payload.** CollectionSampler<T>.items renders
   as Rc<RefCell<JavaNativeCollection<T>>> and assigned values correctly use
   JavaNativeCollection::list. The ordered constructor placeholder still uses
   Rc<RefCell<Vec<T>>>. Required rewrite changes the payload of that placeholder,
   not field storage or ownership. Its producer must consult the same retained
   native-collection authority as slot type rendering and fresh construction,
   preserving source precedence, optionality, generic elements and local/sync
   wrappers. General local slot-default emission correction, not a new carrier.
2. **Late/transitive mutation capability through overload forwarding.** Both
   short shuffle overloads take rng:&dyn while the selected long overload needs
   &mut dyn. ListSampler's short overload additionally takes list:Vec<T>, silently
   discarding caller-visible in-place mutation. Correct diagnostic signatures
   use &mut dyn and &mut Vec<T>, reborrowing/forwarding list rather than taking
   ownership. Trace binding facts, resolved callee identity, signature solver
   dependencies and emitted parameter shapes; MutableBinding alone is not a
   mutable-reference ABI. Use exact call identities and existing propagation;
   do not match all same-named methods or add an RNG-specific rule. Cross-function
   capability/ownership contract correction, not signature text replacement.
3. **Stored interface argument boundary and shared source receiver.**
   PermutationSampler.rng is Option<Box<dyn UniformRandomProvider>> but sample
   passes `self.rng.as_ref() as &dyn UniformRandomProvider`, an invalid cast of an
   optional Box reference. Diagnostic rewrite gets the mutable dyn payload
   through the existing Java null-check boundary before borrowing for shuffle.
   This makes the raw sample receiver genuinely mutable; its outer companion
   and ListSampler's temporary p must both use Rc<RefCell<PermutationSampler>>,
   not Rc<PermutationSampler>. Propagate capability through the shared-placement
   plan and all producers/consumers, rather than patching only the companion.
   Preserve operand evaluation/null-check timing; the diagnostic tests do not
   prove all nullable-source-call cases. Shared RNG identity must remain a
   handle clone, never a clone of generator state. Existing generated interface
   adapters and clone_box forwarding can carry these shared handles, so no new
   ownership framework is demonstrated necessary for this stage.

Disposable experiment `.tmp/rng-sampling-collections-v161-rewrite-audit` is copied
from the baseline crate; the actual baseline/pinned sources remain untouched.
The above coherent rewrites compile (terminal0,8.65s,
`.tmp/rng-sampling-rewrite-audit-build-v2.log`). The independent Rust caller then
matches **all162 Java lines byte-for-byte**, including repeated samplers, shallow
object identity, container isolation, partial/full shuffles and exact RNG bound
traces. Driver build/run/diff terminal0; evidence
`.tmp/rng-sampling-diagnostic-{driver-build-v1.log,output-v1.txt,stderr-v1.log,diff-v1.txt}`.
Diff and stderr are empty. This proves feasibility of a hypothetical rewrite,
**not acceptance of manually repaired output**. This experiment preceded the
translator changes described below.

## Independent Stage1 callers

`SamplingCollectionsOracle.java` calls only untouched production samplers.
Its scripted provider supplies bounded input values and records requested
bounds; unused RNG methods deliberately fail to catch accidental extra calls.
The Rust caller provides the same input script with shared state, not a sampler
implementation. Tests cover every scoped overload, domain/k boundaries,
returned-array isolation, list-result isolation, duplicated element references,
equal payloads with distinct identities, null elements, constructor shallow
copying, caller-visible element mutation, and two samplers sharing one RNG.

The pinned CollectionSampler.sample actually samples with replacement, despite
its Javadoc wording: tests follow source behavior, not that misleading comment.
Shuffle has no explicit start validation; endpoint no-ops and actual failing
reads are tested according to Java behavior, not newly imposed validation.

`bash scripts/run-commons-rng-1.0-sampling-collections-java-oracle.sh` passes
independently (terminal0,162lines ending oracle=ok), evidence
`.tmp/rng-sampling-collections-java-oracle-v1.log` and
`.tmp/commons-rng-1.0-sampling-collections-java-oracle/output.txt`.
The maintained Rust caller currently targets the audited hypothetical ABI;
it is not yet a gate passed by translator-produced output.

## Next coherent production batch

Reuse/generalize slot defaults and exact resolved mutation/call capability
propagation, then align nullable stored dyn arguments and shared receiver
placement. Add focused source regressions for both forwarding layers, overloaded
and unrelated-owner controls, generic retained collection initialization,
caller-visible mutation and persistent interface state. Build/test this related
batch together; rerun27 files only at the batch boundary. Preserve the accepted
generator oracles. Add a reproducible production-output Rust/Java comparator;
never promote disposable rewritten output into accepted evidence.

## Full-module entry measurement

The full46-file V161 baseline completed terminal1 at
`.tmp/commons-rng-1.0-sampling-all-v161-baseline`, run log with `-run.log` suffix.
All46 translate/parse; **93 compiler diagnostics,12 invalid marker files**.
Total289.475s includes15.682s resolver,211.674s translation,47.693s assembly,
9.812s check and1.513s idiom sweep. This is early whole-module risk evidence;
distribution fixes remain Stage2 and require their own representative rewrite
audit. Observed families include lexical-super stringification, static array
initialization/lookup, uninitialized boxed sampler fields, parsed comment nodes
and native-array retention, in addition to first-stage capability issues.

## First production batch in progress

Slot default emission now selects the retained-list runtime payload using the
same EffectiveFacts authority as slot type rendering; local/sync wrappers remain
with the existing storage constructor. Generic element types and outer nullable
carriers are preserved. A regression also exposed that the existing LinkedList
empty-add policy ignored qualified spellings; the shared policy now checks the
canonical leaf, not a separate special case in the new default renderer.

Late mutable-argument propagation now uses exact call-site binding identities
for methods and constructors and demands both a mutable binding and a mutable
parameter ABI where needed. The new two-layer/overload/constructor control first
failed on the outer forwarding layer: existing SCC dependency edges pointed only
from argument producers to callee parameters despite backward callee-contract
propagation. Add the missing reverse typed-call dependency so existing SCC
saturation sees the actual bidirectional contract, rather than adding a global
retry loop. The focused forwarding control now passes.

Stored-interface borrowing now admits mutable as well as shared current-field
arguments, selects the actual field place, discharges the outer Java null carrier
before borrowing the dyn payload, and prevents the nonoptional shortcut from
casting an outer Option. Current-object storage treats dynamic source-interface
fields crossing calls as unproven immutable: a mutable Box payload loan can be
required without direct field writes. Existing storage joins choose cell-backed
shared owners; subclasses inherit that lack of an immutability proof. This
conservative decision precedes signatures/layout emission without a circular
signature/storage re-solve. Build/test logs
`.tmp/rng-sampling-contracts-build-v{1,2,3}.log`;
V1 focused controls intentionally exposed the two issues above. Do not treat
the disposable162-line match as acceptance of this source batch.

V2 focused controls pass: late argument capability/overload/constructor control
**1/0** (`.tmp/rng-sampling-late-arguments-v2.log`), local/sync/nullable/generic
retained-list default control **1/0** (`.tmp/rng-sampling-defaults-v2.log`).
V3 inherited interface-owner storage control **1/0**
(`.tmp/rng-sampling-storage-v3.log`), full library **1622/0**,98.71s
(`.tmp/rng-sampling-library-v3.log`). Executable source regression with two
forwarding layers, aliasing owners and a second owner sharing one interface
counter **1/0**,29.78s (`.tmp/rng-sampling-contracts-runtime-v3b.log`). Its initial
test scaffolding incorrectly called unwrap on the correctly infallible i32 run
result; remove that caller-only mistake, keeping all state assertions unchanged.

The existing verifier now accepts mutually exclusive `--native` and
`--sampling-collections`; defaults remain the original source64 oracle. Sampler
checking requires all162 exact lines, including RNG traces and identity labels,
and builds only untouched output. Python comparator tests **4/0**. Generator
IEEE comparison remains exact.

Frozen V162 `.tmp/new-java2rust-rng-sampling-contracts-v162-debug`,203218584bytes,
SHA256 `790f11ee4de1dc1e71fa9337db731afb9568ade9af81f8bf3766d5227b96ec7c`.
Fresh Stage1 run `.tmp/commons-rng-1.0-sampling-collections-v162-contracts`, log
with `-run.log` suffix, **terminal0: all27 translate/parse/check,0 compiler errors,
0 invalid marker files**. Total171.303s includes19.267s resolver,115.644s
translation,25.066s assembly,6.674s check and0.881s sweep. This overlapped
library/runtime regressions, so it is not an isolated speed comparison.
Production-output sampler parity **does not pass yet**. The verifier completed
terminal1 during Rust caller compilation (`.tmp/rng-sampling-collections-behavior-v162.log`,
`behavior-sampling-collections/driver-build.log`): ListSampler's short list
shuffle still consumes Vec<T>, while the caller deliberately requires a mutable
borrow so it can observe the in-place result. Do not change the caller to move
the Vec: that would hide the semantic bug already found in the entry audit.

The V162 batch removes all5 baseline compiler diagnostics but leaves this
non-diagnostic ownership defect. The exact mutable capability now reaches the
forwarding parameter, yet its conservative public-API Owned baseline wins the
ownership join. Directly mutating public methods already drop that conservative
baseline during initial constraint collection; forwarding methods do not have
that direct-body evidence. The next fix must separate/refine the conservative
public fallback using a proven nonescaping resolved forwarding contract, while
preserving genuine storage/retention/unknown-boundary ownership demands. Do not
blindly delete every Owned constraint or globally prefer mutable borrowing.
Add an executable/public generic-forwarding regression and retention/overload
controls before the next corpus run. Current-object storage and nullable field
argument compilation now agree, as shown by the focused shared-state execution;
sampler acceptance still requires fresh162-line production parity.

## Public forwarding contract follow-through (in progress)

Separate advisory `ApiOwnedFallback` from hard `RequiredOwnership` in signature
constraints. Admission is limited to statically bound native-collection wrappers
whose parameter is only read/forwarded by exact binding identity to resolved
source methods; direct stores, returns, aliases, rebindings, constructor arguments
and opaque calls remain hard boundaries. Existing capture/storage constraints
are still applied as hard requirements. Fallbacks are local defaults, not shape
constraints propagated into another callable. The merge uses a fallback only
when no real ownership requirement exists; a genuine Owned obligation still
wins over MutableBorrow, independent of constraint order. This avoids repeated
constraint deletion or an extra whole-program fixpoint.

New source controls cover two public generic forwarding layers, direct escape/
unknown/alias rejection, a caller-observed in-place replacement, a callee-retained
collection alias, and two mutable interface holders sharing state. The first
constraint control accidentally shared a mutating leaf with its retaining
controls, legitimately lifting that leaf's allocation ABI. Isolate those method
families, and use production semantic facts rather than a raw usage-only model
for JDK method effects. V6 advisory/retention/order control **1/0**
(`.tmp/rng-sampling-api-fallback-unit-v6.log`).

The executable controls exposed related producer/consumer issues, not just an
API spelling problem:

- Native List.set acquired a mutable slot before evaluating its replacement.
  Stage the index and replacement before bounds checks/slot borrowing, retaining
  the existing temporary-receiver lifetime handling. V5 projection controls
  **3/0** (`.tmp/rng-sampling-list-staging-unit-v5.log`).
- Generic Clone-capability analysis inspected only direct get initializers and
  returns; nested get operands (including retained runtime get) could miss their
  T:Clone bound. Reuse the expression walker inside the existing scoped body
  analysis, including conditions, updates, arguments and assignments.
- A mutable named source object passed to a typed source-object parameter could
  still use a cloned value ABI despite caller-visible mutation. Join this alias
  evidence after receiver/inherited mutation collection, independent of source
  order. The initial broad version also marked a checkNotNull(Object) helper;
  V6 full library **1623/1** exposed that existing negative control. Narrow the
  new evidence to exact resolved arguments whose target is a declared source
  object, preserving content-only assertions. Do not waive the failed control.
- Borrowed sequence get results lost physical-reference evidence, letting UFCS
  clone infer Self=&T instead of T when the result was staged in an untyped let.
  Record the borrowed carrier at the get producer and reuse ordinary slot/clone
  conversion; do not depend on an eventual destination to infer cloning.

V6 executable controls: retained collection alias and shared interface state
pass, public replacement still fails on the last borrowed-clone issue (2/1,
`.tmp/rng-sampling-api-fallback-runtime-v6.log`). V7 source build/checks are in
progress at `.tmp/rng-sampling-api-fallback-build-v7.log`. No new corpus run has
been performed since V162; no production162-line parity claim is made yet.

V7 follow-through: record borrowed-result evidence on the sequence-get producer,
and reuse the existing slot-directed get recovery for owned/copy argument slots.
The storage alias evidence is now restricted to exact resolved arguments whose
formal target is a declared source object, rather than every PassedToCall use;
this restores the existing checkNotNull(Object) nonescape control.
Borrowed-clone and null-assertion controls **1/0 each**
(`.tmp/rng-sampling-{borrowed-clone,assertion-storage}-v7.log`).
The public API signature control passes with production semantic facts (V6).
All three executable regressions now pass **3/0**,11.23s
(`.tmp/rng-sampling-api-fallback-runtime-v7.log`), including the nested getter and
the callee-retained alias. Test caller scaffolding accepts either the translator's
infallible unit result or checked Result without weakening the mutation checks.

Frozen V163 `.tmp/new-java2rust-rng-sampling-api-v163-debug`,203320112bytes,
SHA256 `205cf94538daa2609684670ab7dd03cd82052cf50e1e65285a27dd5064083570`.
Fresh Stage1 run `.tmp/commons-rng-1.0-sampling-collections-v163-api`, log with
`-run.log` suffix: **terminal0,all27 translated/parsed,0 unsupported nodes,
0 invalid marker files,0 no-op facade call sites,cargo check0 errors**.
Total123.060s includes15.735s resolver,76.569s translation,20.396s assembly,
5.178s check and0.750s sweep. Concurrent library work makes this a correctness
checkpoint, not an isolated performance comparison.
V7 full library **1625/0**,79.34s (`.tmp/rng-sampling-library-v7.log`).
The V163 sampler driver compiles but stops after121/162 output lines with
`RefCell already mutably borrowed` in `PermutationSampler::sample_with_outer`.
The companion retains a mutable owner guard for the RNG-field argument while
evaluating the sibling domain-field argument. Raw-receiver permutation tests
pass first; ListSampler calls the shared-receiver companion and exposes the
conflict. Evidence: `.tmp/rng-sampling-collections-behavior-v163.log` and
`behavior-sampling-collections/{rust-output.txt,rust-stderr.log}` under the
V163 output root. The chained native gate did not start. Stage1 is **not**
behavior-accepted.

Next contract batch reuses existing source-interface reference snapshots and
ordered call-argument temporaries. Snapshot the owned interface handle before
borrowing its payload; drop the owner guard before sibling reads and callbacks.
Do not hold a whole-owner guard across a raw-method invocation, because Java
permits reentrant calls into the owner. The strengthened executable regression
adds a sibling retained-array argument to a shared interface holder, preserving
shared counter state across two holders and an alias.

### V164: collection-stage behavioral checkpoint

General lowering change: extend the existing ordered argument staging pass to
source-interface fields on a shared current receiver. Reuse the existing
owned-interface representation proof and clone_box reference-copy contract;
capture the nullable Box and sibling reference handles in source order, then
form the borrowed argument from a mutable local. No owner guard spans invocation.
Owned interface arguments also need this staging: a temporary field-read guard
would otherwise remain live through a reentrant call even after cloning the Box.

The executable holder regression now includes a callback from the counter into
the holder's array-reading method. All three focused runtime controls pass
**3/0**,51.30s (`.tmp/rng-sampling-reentrant-runtime-v10.log`); the earlier sibling
test alone used owned interface parameters, so it did not prove the corpus's
borrowed-interface path. The fresh corpus below does exercise that path.

Frozen producer `.tmp/new-java2rust-rng-sampling-snapshot-v164-debug`,203313272bytes,
SHA256 `fcb25ca7a5afd6c0bd97f04d1d273175a19fca55f5a020a81c8e8766755bdcd1`.
Fresh output `.tmp/commons-rng-1.0-sampling-collections-v164-snapshot`:
**all27 translated/parsed,0 unsupported/invalid markers,cargo check0 errors**.
Terminal0; total197.043s,resolver15.050s,translation136.820s,assembly32.878s,
check6.447s. Overlapped builds mean this is not an isolated performance result.

Production Java/Rust gates both pass, terminal0:

- **162 sampler lines**, exact outputs and RNG consumption traces:
  `.tmp/rng-sampling-collections-behavior-v164.log` and output-root
  `behavior-sampling-collections/result.txt`.
- **12601 native-generator lines**, exact floating-point bits:
  `.tmp/rng-sampling-native-behavior-v164.log` and output-root
  `behavior-native/result.txt`.

Stage1 is now behavior-accepted at V164. Overall46-file completion remains
unproven: Stage2 constructor/storage/lexical fixes and distribution oracles are
still required, as are fresh final generator gates and final library checks.

Next Stage2 batch follows the existing V161 full46 rewrite audit: select named
ternary operands rather than positional children (comments are extras), preserve
lexical-super dispatch before Object stringification, and resolve uppercase
instance fields by declaration rather than capitalization. Instance/static
field lookup should share the existing exact-first, unique-alias naming resolver.
The ternary comment parser control passes **1/0**
(`.tmp/rng-sampling-conditional-parser-v11.log`). A nested uppercase array-cache
and overridden-super stringification executable control is being checked with
the lexical batch; do not claim the batch accepted until it passes.

### V165: lexical and validated-construction batch

The first lexical executable check failed **3/1** (V12): the environment had
resolved the correct uppercase field, but `self_field_projection` snake-cased
that already allocated name a second time. Use exact field projection for the
environment's resolved storage name. Keep declaration lookup centralized:
static and instance fields share exact-first/unique-canonical-alias resolution
in `java_naming`; explicit-this lookup uses the selected declaration too.
`super.toString()` is a lexical invocation, so it bypasses the generic Object
stringification shortcut and uses the existing superclass helper.

V13 executable collection/lexical regressions **4/0**,34.15s
(`.tmp/rng-sampling-lexical-runtime-v13.log`). The nested array cache exercises
both bare and explicit-this uppercase accesses and constructor writes; the
superclass result is checked at runtime, not merely by Rust parsing.

Extend the existing receiver-independent constructor initialization prefix,
rather than adding new partially initialized storage. The prefix can contain
validation/local/effect statements between definite field assignments when
the existing IR walkers prove no constructor-receiver mention and no opaque
raw leaf. Preserve source order, stop at receiver access/escape or repeated
field assignment, and require the original struct fields to be generated
placeholder defaults. This removes unavailable defaults only when the actual
field value can be evaluated before allocation; it does not fabricate values
or weaken receiver-use barriers.

New checked-constructor executable control **1/0**,33.03s
(`.tmp/rng-sampling-constructor-runtime-v13.log`): two trait-object fields,
validation before and between initialization, a computed local, exact ordered
constructor effects, and an invalid argument that must initialize neither
field. V12 library (before the prefix extension) **1626/0**,122.17s; V13 full
library **1626/0**,113.04s (`.tmp/rng-sampling-constructor-library-v13.log`).
Comparator Python controls **4/0**.

Frozen V165 `.tmp/new-java2rust-rng-sampling-lexical-v165-debug`,203296064bytes,
SHA256 `9db1193117f55dc3935e371d6c3be7b6588e243086a58fb8af5dce52bf06183b`.
The fresh full46 measurement at `.tmp/commons-rng-1.0-sampling-all-v165-lexical`
(run log with `-run.log` suffix) is **terminal1: all46 translate/parse,
0 unsupported nodes,1 invalid-marker file,8 compiler diagnostics**, down from
V161's93 diagnostics/12 marker files. Total318.581s includes resolver25.983s,
translation233.472s,assembly44.774s. This is a diagnostic checkpoint, not
distribution-behavior acceptance. Both new behavior fixtures and all1626 library
tests pass; no distribution oracle has been accepted.

### Remaining V165 rewrite audit / next coherent batch

All eight diagnostics are in three files and four producer/consumer contracts:

1. **Retained array argument shape (one diagnostic/marker), InternalUtils**:
   `FactorialLog.withCache` passes the existing retained `LOG_FACTORIALS` array
   to the constructor's `Option<&Rc<RefCell<Vec<f64>>>>` parameter. The minimal
   correct argument is `Some(&self.LOG_FACTORIALS)`, borrowing the existing
   allocation handle, not converting a fresh native Vec into a new identity.
   Source field access now names the right storage, but the argument-shape
   path still reports the field as unshared. Trace field binding/actual-shape
   evidence into the existing native-carrier borrow conversion. Do not add a
   deep-copy adapter to bypass the identity-preservation diagnostic.
2. **Static retained array initialization and access (four diagnostics),
   AhrensDieterExponentialSampler**: the static slot is
   `Mutex<Option<Arc<Mutex<Vec<f64>>>>>`, but its getter fallback and class-init
   setter still produce bare Vec, and the element-write path indexes the Arc
   instead of the locked payload. Reuse solved storage wrapping for default and
   initializer producers, then existing retained-array guards at writes. Stage
   index/value before acquiring the payload lock; avoid recursive class-init
   accessors or holding the registry lock across arbitrary operand effects.
3. **Explicit primitive casts around calls (two diagnostics), PoissonSampler**:
   `(int)Math.min(mean,2*PIVOT)` loses its narrowing around a staged shared-owner
   callback; `(int)Math.min(nextPoisson(mean),Integer.MAX_VALUE)` also loses the
   outer narrowing and long promotion of the second min operand. Minimal Rust
   must retain Java's floating-to-int conversion at the first call boundary,
   and perform long min then narrow to int in sample. Trace cast producer and
   call-result/outer-companion shortcuts, using existing numeric conversion
   rules. Do not use checked try_into or cast before the min.
4. **Conditional-null field evidence (one diagnostic), PoissonSampler**:
   `factorialLog = mean<PIVOT ? null : create().withCache(...)` is still stored
   as nonoptional Rc, requiring an invalid Default in the null arm. The actual
   contract is optional reference storage, None/Some assignment, and ordinary
   null discharge at use. `semantic_analysis::expr_observed_non_null` currently
   treats any expression except a literal null as nonnull, including this
   ternary. Join arm nullability through the existing lattice and carry that
   evidence into field storage; cover constructor assignments and field
   initializers, nested conditionals and nonnull controls. Do not derive a
   fabricated Default for FactorialLog.

These rewrites fit the current reference/storage/argument/numeric framework;
they are cross-boundary contract corrections, not a demonstrated need for a new
ownership representation. Batch their source fixes and focused controls before
the next full46 compile. After compile cleanliness, build independent per-sampler
oracles; compilation alone does not establish numerical or state correctness.

### Value-contract follow-through (V14/V15)

Added four executable source controls in `tests/sampling_value_contracts.rs`.
The V14 baseline fails all four, independently reproducing retained constructor
array borrowing, static retained-array initialization/writes, promoted-call
numeric casts and conditional-null fields
(`.tmp/rng-sampling-value-baseline-v14.log`). The initial cache fixture also
exposed unrelated direct uppercase accesses through other-object receivers;
the cache now models the pinned private field through nontrivial get/set
methods, keeping the array-argument regression about its actual contract.
That other-receiver naming gap has not been claimed fixed.

The V15 source batch:

- Admits any solved retained native handle to the existing generic carrier
  borrow adapter, not only native maps. Physical Rust carrier equality still
  gates admission; no placement-changing copy adapter is introduced.
- Wraps fresh static-array initializer payloads with solved native allocation
  storage before optional-reference construction. Nonconstant array
  initializers use the ordinary slot-aware path rather than an empty fallback.
  Shared static element writes bypass the old by-value registry-index shortcut
  and reuse the ordered, checked retained-array write path.
- Preserves primitive casts at the common slot boundary before call/slot
  shortcuts; min/max arguments use the selected overload's promoted scalar
  type. The source control checks a long min before narrowing, once-only
  receiver effects, double narrowing, NaN and both infinities.
- Recursively joins ternary arm nullability (including reference casts) through
  the existing field lattice for constructor assignments and field initializers;
  nonnull-arm controls remain nonoptional. Bare assignments retain their
  existing local/call uncertainty treatment inside each arm.

V15 build succeeds (`.tmp/rng-sampling-value-build-v15.log`). Full library
**1626/0**,113.88s (`.tmp/rng-sampling-value-library-v15.log`). Focused results:
retained constructor array and nullable fields pass, numeric casts still fail,
and static-array initialization deadlocks. The run was explicitly interrupted
(terminal130) after the live deadlock and generated code were inspected, not
restarted on an observation timeout. No new full46 gate is claimed.

V16 follow-through addresses the producer paths, not test expectations:

- Native shape inference planned calls from arity without their operand types,
  silently selecting min/max's default int result. Use the existing typed-args
  call planner for shapes too; planned scalar return shapes prefer the selected
  native overload over a nominal same-arity summary. Retain the existing
  primitive conversion planner at the explicit-cast boundary.
- The ordered retained-array store excluded nonpropagating contexts, so static
  initialization fell back to holding the target's Mutex guard during RHS
  evaluation. Permit that context and isolate only the checked store in the
  existing Result-scope/uncaught-exception adapter. Allocation/index/value
  capture and the payload guard order remain shared with normal method stores.

V16b focused/library test build succeeds
(`.tmp/rng-sampling-value-build-v16b.log`); V16 executable and full library
checks are running. A V16 compile attempt first caught a misspelled LoweredType
accessor; it was corrected before the successful build, not suppressed.

V16 focused controls **4/0**,43.64s; full library **1626/0**,111.56s
(`.tmp/rng-sampling-value-{runtime,library}-v16.log`). Frozen V166
`.tmp/new-java2rust-rng-sampling-values-v166-debug`,203326872bytes,
SHA256 `728c096f9825c769790e93eb387253787b9d2c8be7703c8fa402e9a7da2be51f`.
Fresh all46 `.tmp/commons-rng-1.0-sampling-all-v166-values` is terminal1 with
**0 unsupported nodes,0 invalid markers,1 compiler diagnostic**. All eight
previous diagnostics are gone. Total203.901s includes resolver23.454s,
translation123.489s,assembly42.913s,check6.686s. Timings are not isolated speed
comparisons.

The remaining E0282 is another uncaught-context contract: static exponential
table construction computes a float expression involving the fallible factorial
method. Operand exceptions are already discharged, so an arithmetic `Ok(value)`
has no remaining witness for its error type before unwrap_or_else. State the
producer's existing `JavaException` error type explicitly in uncaught mode;
keep propagating-context emission unchanged. Extend the static-array regression
with nested floating arithmetic around a checked long-returning divisor,
including actual ordered table values, rather than annotating generated output.

V17b build succeeds; focused controls **4/0**,12.17s and library
**1626/0**,109.87s (`.tmp/rng-sampling-value-{build,runtime,library}-v17b.log`).
Frozen V167 `.tmp/new-java2rust-rng-sampling-static-result-v167-debug`,
203330008bytes,SHA256
`131553663af81a212924f9a49be7f1f3ddc6b14e3a5f6d766d63d467bcb23e5b`.
Fresh full46 `.tmp/commons-rng-1.0-sampling-all-v167-static-result` is running.

### Distribution oracle construction (not acceptance)

New independent Java caller `tests/corpus/rng/SamplingDistributionsOracle.java`
runs all12 distributions against an independent deterministic entropy source,
logging exact interface-level draw operands/results for every sample. It uses
41 configurations with24 repeated draws each: small/large Poisson means,
gamma/beta algorithm families and shape order, Gaussian caching, wide/full
integer ranges, one-element Zipf, continuous boundaries, and stateful inverse-CDF
callbacks shared across two sampler instances. It also calls factorial,
resized/cached factorial log and internal logGamma helpers, including invalid
inputs and NaN/infinity. Sampler algorithms remain solely in pinned sources.
Unused entropy-source entry points fail loudly; a draw budget bounds rejection
failures. toString checks its meaningful structural contract rather than
comparing Object identity text across runtimes.

The shared Java runner now optionally accepts a qualified main-class name,
allowing a package-local oracle to exercise internal helpers without changing
upstream visibility. Default generator callers remain unchanged. New wrapper:
`bash scripts/run-commons-rng-1.0-sampling-distributions-java-oracle.sh`.
The initial Java caller completed2064 lines. Adding stateful shared callbacks
produces **2168 lines**, terminal0
(`.tmp/rng-sampling-distributions-java-v2.log` and
`.tmp/commons-rng-1.0-sampling-distributions-java-oracle/output.txt`).

Rust counterpart `SamplingDistributionsOracle.rs` calls generated algorithms
through sampler interfaces and mirrors only test entropy/callback behavior.
New verifier mode `--sampling-distributions` requires all2168 lines, exact
floating-point values and exact draw/callback/display/exception observations.
No tolerances have been introduced. A bad display remains a failing comparator
observation while allowing other numeric observations to be collected, not a
passing placeholder. Comparator controls **5/0**, including rejection of changed
draws, callback counts, display results and a one-ULP distribution change.
The Rust distribution gate has not yet run; this coverage is preliminary, and
runtime/branch/numerical discrepancies and remaining boundary coverage must be
audited before acceptance. Stage1 and generator gates also need fresh final
producer evidence. The overall goal remains active and incomplete.

### V167: full46 compile checkpoint and first distribution runtime audit

The fresh full46 run is **terminal0: all46 translated/parsed,0 unsupported
nodes,0 invalid markers,0 no-op facade call sites,cargo check0 errors**.
Summary at `.tmp/commons-rng-1.0-sampling-all-v167-static-result/summary.tsv`;
total314.745s,resolver26.234s,translation224.126s,assembly48.997s,check7.795s.
This proves compilation of the full fixed scope, not completion of the goal.

Fresh gates against that exact generated crate both pass, terminal0:

- Native12601 lines, exact floating-point bits:
  `.tmp/rng-sampling-native-behavior-v167.log` and output-root
  `behavior-native/result.txt`.
- Collection162 lines, exact outputs and RNG traces:
  `.tmp/rng-sampling-collections-behavior-v167.log` and output-root
  `behavior-sampling-collections/result.txt`.

First new distribution gate **terminal1**, not accepted:
`.tmp/rng-sampling-distributions-behavior-v167.log`. The driver compiles, writes
751/2168 lines, then panics at `Math.log1p is not modelled` when beginning the
first beta case. Authoritative output/stderr are under output-root
`behavior-sampling-distributions/`. No result.txt acceptance file exists, and
the verifier has not yet copied Java output there because execution failed;
the independent2168-line reference remains in the Java-oracle output directory.

An independent exact comparison of the completed prefix finds **0 numeric
differences,0 RNG-trace differences,15 display differences**. Thus360 draws
across uniform, exponential, gamma, Gaussian and lognormal cases currently match
exactly; this is prefix evidence only, not a gate or a basis for tolerances.
All15 display observations are `ContinuousSampler@0`, rather than the actual
class override. The Rust caller deliberately records `display=bad` so these
remain comparator failures, without discarding other observations.

Next batch has two evidenced general runtime-model/dispatch contracts:

1. `Math.log1p` currently reaches an unimplemented facade despite clean source
   markers and a successful compiler gate. The pinned scope also calls
   `Math.expm1` in Zipf; audit the complete used Math surface before another
   behavior pass. Reuse the existing native unary-f64 method planner and stable
   `ln_1p`/`exp_m1` primitives, preserving Java source precedence, effect and
   result-shape selection. Cover near-zero accuracy, signed zero, infinities and
   NaN with focused source regressions. Do not replace these with log(1+x) or
   exp(x)-1, and do not treat unreachable generic support stubs as acceptance of
   reachable missing operations.
2. Invoking Object.toString through a source-interface value currently uses the
   trait's synthetic fallback instead of the implementation's source override.
   The inherent sampler overrides and lexical-super helpers compile, but the
   generated shared interface adapters do not forward that Object protocol.
   Trace the existing source-object display/result and adapter mechanisms,
   including shared handles and nullable/fallible String overrides; reuse their
   authoritative ABI rather than manually fixing each sampler or bypassing
   dynamic dispatch in the oracle. Add a generic interface/override executable
   regression before changing emission.

These failures are newly exercised runtime paths, not extra corpus files or a
scope change. Beta, inverse samplers, Pareto, discrete/Poisson/Zipf and internal
helper behavior after the stopped prefix remain unverified. Final source64
2865-line evidence, full per-sampler acceptance, remaining boundary/branch
coverage audit and final-producer gates are still owed. Goal stays active.

### V168: stable math and checked interface Object dispatch batch

The native unary-f64 planner now maps Math.log1p/expm1 to ln_1p/exp_m1,
preserving source-class precedence. Source-interface Object.toString calls use
the existing checked nullable String/JavaException protocol and canonical
shared source handles. Receiver owner guards are dropped before callbacks;
unregistered external Rust implementations retain their trait override.
Synthetic Rust String bridges adapt the same checked source dispatch rather
than replacing source overrides with the interface-name fallback.

Assembly originally discarded later per-file source downcast registrations.
Reuse the existing explicit additive-registry protocol (already used for
reference identity), now with an object_string region. Merge registrations
into existing shared functions, including function-only dependency closures.
Require matching surrounding templates and reject incompatible registrations
for the same downcast type. No inferred arbitrary function-body merger remains.

Current focused evidence:

- `.tmp/rng-sampling-runtime-focused-v20b.log`: 4/0 executable regressions,
  including tiny values, IEEE boundaries, source Math precedence, generic
  interface-only dependencies, shared state, reentrant holder access,
  nullable/throwing overrides, inherited interfaces and a foreign Rust override.
  The preceding v20 run failed solely because the Rust test caller passed Box
  where the generated describe API accepts a borrow; corrected that caller.
- `.tmp/rng-sampling-runtime-library-v20.log`: 1626/0 library tests.
- `.tmp/rng-sampling-object-registries-v20.log`: 4/0 merger tests, including a
  compiled function-only multi-module assembly and incompatible-case rejection.
  Existing shared identity registries and Rust item alias tests also pass, 3/0
  each.

Frozen producer `.tmp/new-java2rust-rng-sampling-object-v168-debug`,
203336728 bytes, SHA256
`b0637a1e91153ce78afaca55df58cc5a644823fc0786a016bb196f595e5a5fac`.
Fresh full46 run launched at
`.tmp/commons-rng-1.0-sampling-all-v168-object`, log
`.tmp/rng-sampling-all-v168-run.log`; no result is claimed here yet.
Distribution acceptance and fresh final-producer generator gates remain owed.

### V168 final acceptance and requirement audit

Same frozen producer/hash as above; no production edits after freezing.
The full46 run finishes terminal0: files46, failed translations0, parse
failures0, unsupported statements/members/expressions0, invalid marker files0,
stubbed constants0, no-op facade call sites0, cargo check errors0. Total218.462s,
resolver17.996s, translation160.565s, assembly31.478s, check4.912s. These are
acceptance timings, not an isolated performance experiment.
`generated-shared-support-conflicts.tsv` confirms Object-string registrations
were merged from34 variants, not selected from the first source file.

Independent behavior gates, all terminal0 on freshly generated, unedited code:

- Full46 collection oracle162 lines:
  `.tmp/rng-sampling-collections-behavior-v168.log`.
- Full46 distribution oracle initially2168 lines:
  `.tmp/rng-sampling-distributions-behavior-v168.log`. Final coverage audit
  added zero/negative element count and zero/negative exponent Zipf constructor
  exceptions to both callers; expanded2172-line gate also passes:
  `.tmp/rng-sampling-distributions-behavior-v168b.log`. No comparator tolerance
  changed, and all floats, integer draws, RNG tokens, callbacks, displays and
  exception observations match.41 sampler configurations each draw24 times;
  interfaces/abstract base are exercised through actual implementations,
  static initialization and both internal helpers through samplers/direct calls.
- Full46 native-generator oracle12601 lines:
  `.tmp/rng-sampling-native-behavior-v168.log`.
- Fresh separate original11-file source64 closure, same producer:
  `.tmp/commons-rng-1.0-source64-v168-object`, terminal0 translation/check;
  `.tmp/rng-source64-v168-run.log`. Independent2865-line exact IEEE gate:
  `.tmp/rng-source64-behavior-v168.log`.

Each root's behavior directories contain Java/Rust output, build/run logs and
terminal result.txt.2168/2172 are oracle observations, not compiler diagnostics
or a proof for every possible input. Coverage includes all12 distribution
classes and constructors, gamma regimes/beta swaps, repeated Gaussian cache
use, shared stateful inverse callbacks, integer overflow-range paths, small/
large Poisson, Zipf near exponent1 and one element, validation exceptions,
factorial/cache resizing and logGamma branch/IEEE boundaries. Stage1 retains
every overload, identity/shallow-copy/aliasing checks and shared RNG traces.

Fresh focused acceptance:
`.tmp/rng-sampling-contract-batch-v20.log` has constructor1/0, collection/
capability4/0 and value/static/shape4/0. Runtime4/0 and library1626/0 evidence
is listed above. The existing interface Object regression also passes:
`.tmp/rng-object-interface-idiom-v20.log` (1/0). Python comparator controls5/0,
registry merger4/0, existing identity registry3/0 and item alias3/0 pass.
`git diff --check` passes; pinned upstream `git status --short` is empty.

Reachability audit: production generated methods contain no unimplemented
algorithm bodies. Generated support still includes unused broad facade stubs
(26 no-op facade definitions, zero production no-op call sites), an unused
Long facade in SplitMix64 and9 synthetic interface anonymous() panic helpers.
There are no Long:: or anonymous() call sites in the production modules; these
helpers are not pinned Java declarations. Native Math call coverage was checked
against the complete used surface (E/PI, abs, ceil/floor, cos/sin, exp/expm1,
log/log1p, min/max, pow, sqrt). Checked Java exceptions, interface String bridge
panic adapters and numeric zero-fill in Arrays.copyOf are real modeled
semantics, not fabricated algorithm fallbacks. This is acceptance of the fixed
production scope, not a claim that all emitted support or all Commons RNG is
implemented.

Reproduce with a translator built from this checkpoint (or the frozen binary):

```sh
export CARGO_INCREMENTAL=0 TMPDIR="$PWD/.tmp/test-tmp"
export NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-rng-sampling-object-v168-debug"
export NEW_JAVA2RUST_BUILD_PROFILE=debug
export RNG_TRANSLATION_ROOT="$PWD/.tmp/commons-rng-1.0-sampling-all-v168-object"
RNG_SAMPLING_STAGE=all bash scripts/run-commons-rng-1.0-sampling.sh
python3 scripts/verify-commons-rng-1.0-behavior.py "$RNG_TRANSLATION_ROOT" --native
python3 scripts/verify-commons-rng-1.0-behavior.py "$RNG_TRANSLATION_ROOT" --sampling-collections
python3 scripts/verify-commons-rng-1.0-behavior.py "$RNG_TRANSLATION_ROOT" --sampling-distributions
export RNG_TRANSLATION_ROOT="$PWD/.tmp/commons-rng-1.0-source64-v168-object"
bash scripts/run-commons-rng-1.0-source64.sh
python3 scripts/verify-commons-rng-1.0-behavior.py "$RNG_TRANSLATION_ROOT"
cargo test --lib
cargo test --test sampling_contracts --test sampling_constructor_contracts --test sampling_value_contracts --test sampling_runtime_contracts
```

The fixed goal is complete. No follow-on corpus is silently included in this
acceptance or substituted for an unfinished stage.
