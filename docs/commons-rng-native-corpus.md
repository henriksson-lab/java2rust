# Commons RNG 1.0 native generator extension

Pin `4581a4520315657a4219b37c81f5db80e4a4e43c`, tag RNG_1_0. The exact
**24 production files** are in `scripts/focus-sets/commons-rng-1.0-native.txt`:
all previously verified11 source64 files plus13 source32 files. New concrete
generators: ISAACRandom, KISSRandom, MersenneTwister, MultiplyWithCarry256,
Well512a, Well1024a, Well19937a, Well19937c, Well44497a and Well44497b.
AbstractWell, IntProvider, RandomIntSource and nested IndexTable are included.
Every production method in this manifest is in scope, not just raw draws.

Explicit exclusions established before the baseline: JDKRandom's JDK Random/
Java serialization adapter, TwoCmres, factory/reflection, sampling, examples,
benchmarks and package-info metadata. This is not the complete Commons RNG
project. Do not shrink the manifest to avoid a difficult translation family.

The runner `scripts/run-commons-rng-1.0-native.sh` reuses the source64 pipeline,
validates24 entries/files and compares staging against untouched pinned Java.
The original source64 runner still defaults to its original11-file manifest.

## Entry gate

First measure the complete fixed scope with the verified V158 translator.
Before translator edits, derive correct representative Rust rewrites and trace
producer/storage/signature/consumer contracts. Particularly inspect stateful
abstract AbstractWell with explicit constructor, inherited mutable state and
nested static IndexTable; private seed helpers; static final table storage;
WELL subclass super calls; ISAAC/Mersenne retained arrays and constructor seed
ownership. Existing stateless declaration-only admission intentionally does
not admit AbstractWell: do not weaken that proof simply to remove errors.

Record baseline, rewrite scope, architectural feasibility and coherent batches.
Substantial redesign must be explained upfront before an incremental campaign.
The original source64 oracle remains a required regression gate. New Java/Rust
oracles must cover every new generator, representative seeds, state replay/
defensive copies, bounded draws, floats/doubles, byte sentinels and exceptions.
Only general translator fixes are deliverables; no pinned Java/generated repairs.

## V158 baseline (2026-10-21)

Frozen verified debug binary `.tmp/new-java2rust-rng-verified-v158-debug`, SHA256
`c9de79095f43ca9de9b0cb705a1bf3480154aef296e773611293918d2e366466`.
Output `.tmp/commons-rng-1.0-native-v158-baseline`; log
`.tmp/commons-rng-1.0-native-v158-baseline-run.log`, **terminal1** due to generated
Rust. All24files translate, parse failures0, cargo_type_errors_valid1,
**36 diagnostics (30 typed,6 allocation guards)**, registry marker file1
(ISAACRandom), no unsupported IR statements/members/expressions and no-op facade
call sites0. These occurrences are not independent bug counts.

Isolated total89.054s: translation59.501, assembly20.246, check4.926, sweep1.193.
Resolver benchmark reused; no competing compilation/oracle during measurement.
This debug measurement is not a release speed comparison. No current evidence
calls for reopening open-ended optimization before the coherent correctness batch.

`scripts/run-commons-rng-1.0-native-java-oracle.sh` passes independently against
the unchanged24-file Java scope. It emits **12601 lines** for all ten new
generators and seven seeds each: empty,0,1,-1,MIN/MAX,mixed, and1536elements.
It verifies seed defensive copying, folds2048 warmup draws across complete
state-buffer cycles, then reuses the source64 caller's readable draws, bounded
draws, state replay, floats/doubles, byte sentinels and exceptions. Output
`.tmp/commons-rng-1.0-native-java-oracle/output.txt`, log
`.tmp/commons-rng-native-java-oracle-v1.log`, terminal0. Existing source64 Java
oracle rerun remains2865lines ending oracle=ok; helper visibility is the sole
change to its Java caller, not to the pinned library sources.

## Whole-path generated rewrite audit

1. **Mixed plain/retained primitive arraycopy (6 guards, ISAAC state export).**
   Fresh Arrays.copyOf results sRsl/sMem/sRem are plain Vec<int>, while output
   s is Rc<RefCell<Vec<int>>>. The existing checked retained-arraycopy route
   admits only shared/shared operands, so the slice route rejects the retained
   destination. For these stable locals, correct hypothetical Rust uses plain
   source slices and a guarded destination slice, preserving allocation identity
   and caller-visible updates. General implementation must stage all five Java
   operands in order, defer null/check/borrow until after evaluation, and snapshot
   source after later-operand side effects. Do not clone an unshared payload at
   operand capture or retain a slice borrow across a later mutation. Investigate
   reuse/generalization of the retained-copy route and native-array retention
   planning rather than replacing compile_error with a payload clone. Add both
   plain-to-retained and retained-to-plain controls and side-effect/alias tests.
2. **Inherited owned-array ABI (4 E0308, KISS/MWC).** BaseProvider.fillState's
   state parameter emits as owned Rc<RefCell<Vec<int>>>, but inherited calls
   use &tmp. Correct call is Rc::clone(&tmp), not tmp.clone().borrow payload or
   changing the callee to a stale borrowed contract. The current declared-method
   argument lookup searches only ty.members; inherited materializations need the
   same exact source-declaration/signature/parameter-shape normalization used
   by direct source calls, with ancestor substitution and companion consistency.
   Cross-function emission contract, not a new ownership representation.
3. **Checked remainder under numeric narrowing (2 E0605, MWC).** Java
   `(int)((c < 0 ? -c : c) % A)` emits checked Result<i64> directly cast to i32.
   Correct propagating rewrite is `checked_remainder? as i32`; use the existing
   Result policy/discharge metadata exactly once before conversion. Keep Java
   signed MIN/-1 and zero-divisor behavior, and add caught ArithmeticException
   plus nested cast controls. Local numeric/effect composition, not an unwrap.
4. **Lexical super static-storage owner (20 E0599).** Materialized
   Well19937a.next inside Well19937c's super helper uses `Self::__static_TABLE`;
   the declaring private static field belongs to Well19937a, not the child.
   Replace with the declaring owner's accessor; likewise Well44497a/b.
   Existing constructor tables are Arc<IndexTable> with Mutex-backed arrays;
   retain that single static allocation, source private visibility and clinit
   behavior. Thread declaring lexical owner through existing field lookup and
   storage-owner emission; do not manufacture duplicate child statics. Test
   private parent statics, child same-name shadowing and plain/shared super paths.
5. **Stateful abstract raw receiver inventory (4 initial E0599).** AbstractWell
   raw nextInt invokes unimplemented abstract next; its raw interface adapters
   invoke suppressed IntProvider::next. Concrete WELL classes already flatten
   index/v and the AbstractWell constructor, call the correctly renamed private
   seed helper, materialize inherited methods, and emit lexical-super bodies.
   Those child paths must stay intact. The correct raw-root rewrite retains its
   state/constructor/private seed/static/nested support but does not pretend its
   unresolved virtual receiver is executable. Omit only virtual-dependent raw
   methods and their complete interface-adapter inventories after proving no
   runtime base value demand and no required constructor/helper route depends
   on such dispatch. Derive dependency closure from source method identities,
   not prefixes. Typed-base values or constructor-time overridable dispatch must
   veto this tier or get a proper receiver-capable solution. Reuse the existing
   admission/value-use analysis; do NOT admit stateful classes by deleting the
   old stateless layout checks. This is a general emission/inventory planning
   extension; current evidence does not require a new Rust storage framework.

## Disposable rewrite experiment (not acceptance)

Copy `.tmp/rng-native-v158-rewrite-audit` contains a separate generated crate,
never used as the translator output or a deliverable. Six files were changed
only to test the hypothetical rewrites:6 mixed-copy calls,4 handle arguments,
2 checked remainder discharges,20 lexical static accesses, and raw abstract
dispatch/adapters. Its first check deliberately showed the whole-contract issue:
removing16 unresolved/transitively dependent raw next-method bodies alone leaves
**38 diagnostics**, because raw adapters still promise those slots. Removing
the corresponding9 raw interface/From impl blocks as well yields **cargo check
terminal0**,6.47s: `.tmp/rng-native-v158-rewrite-audit-check-v2.log`.
Constructors, state fields/private seed helpers, nested table support and all
concrete child bodies remain. Root wrapper structs were left inert to preserve
the copied crate's fixed import inventory; production assembly must instead
derive both imports and adapters consistently from its planned emission set.

This demonstrates manageable, localized generated rewrites and rejects a
method-only workaround; it does NOT establish generated behavior or translator
acceptance. The original unmodified baseline still fails36diagnostics, and no
translator production source has been changed for this extension yet.

## Implementation batches and remaining gates

Proceed with a coherent operand/ABI/effect/lexical-owner batch using focused
executable controls, and a raw abstract receiver inventory extension with
typed-base/constructor-dispatch negative controls. Reuse prior mechanisms and
the proven concrete field/constructor/super materialization paths. If that
whole-path proof fails, repeat this audit and surface the gap before proceeding.
Do not repeatedly run all24files for individual edits. After substantial related
changes, rebuild/freeze, measure the unchanged scope, then run the independently
extended Rust/Java oracle and original source64 regression gate. No scope
reduction or support claim is justified by this diagnostic experiment.

Still required: general translator fixes, focused executable regressions,
native Rust test caller/comparison integration, clean untouched24-file cargo
check/marker gate, all12601 native Java/Rust lines and original2865 source64
lines matching, final relevant/library gates and reproducible frozen handoff.

## General contract/numeric batch V159 (2026-10-21)

Production changes now implement the first four audited families, plus the
integral-negation family discovered by the diagnostic runtime probe:

- Inherited source-call arguments use the same specialized implementation-frame
  signature and emitted parameter normalization as lexical-super entries.
  Ordinary and private materializations participate; exact overload identity
  outranks an ambiguous compatibility spelling. The shared super argument helper
  is reused, not a second borrowed/owned parameter policy.
- Primitive casts feed their input through the existing scalar value-slot
  boundary, discharging checked arithmetic exactly once before conversion.
- Lexical fields start at the inherited body's declaring class, not the emitted
  child. Static getter/setter names, owner qualifiers, raw bare reads and field
  shapes agree; inherited environments seed declaring static shapes as well as
  instance fields. The new regression intentionally gives the child's hidden
  table a different Java type. Its first V1 execution compiled but returned15,
  exposing the raw-name path missed by a getter-only change. V3/V4/V5 close it
  without weakening the expected parent/child counter values.
- Retained primitive arraycopy supports mixed storage only with a positive
  no-write/rebinding/class-initialization proof and nonthrowing bare-reference
  operands. Plain Vec/slice borrows occur after scalar evaluation, not before;
  source snapshot occurs after all operands and before destination borrowing.
  Calls, assignments, increment/decrement and potentially initializing statics
  veto this conservative mixed tier. Qualified/indexed plain operands also veto
  it: moving their null/index exception later would change Java exception order.
  Effectful shared/shared copies retain the previous full handle-staging route.
  Source System overrides still take precedence. Numeric index operands reuse
  the ordinary int slot/effect boundary. Native sequence projection now handles
  plain payloads without inventing RefCell borrows, and uses mutable optional
  projection only for plain destination storage.
- Integral unary minus now uses Java unary promotion followed by wrapping
  negation at int/long width. Reuses numeric promotion/coercion and existing
  wrapping-receiver spelling; floating-point minus remains ordinary negation.
  Byte/short/char promote before negation/plus, not after overflowing a small
  Rust carrier. Bitwise complement shares the operand effect boundary.

The last family was not visible to cargo check: the diagnostic native test
caller reached MWC's MIN_VALUE seed, where Rust `-c` panicked in debug. The
pinned source uses a ternary, not Math.abs; the existing Math.abs kernel already
uses wrapping_abs. A separate two-site wrapping-negation diagnostic correction
then makes **all12601 native output lines match Java, including IEEE bits**.
This remains evidence about the hypothetical whole-path rewrite, NOT acceptance
of manually repaired output. Files
`.tmp/rng-native-v158-rewrite-audit/diagnostic-native-{output,stderr}-v2.txt`
(stderr actually has `.log` suffix), build/driver logs
`.tmp/rng-native-diagnostic-{behavior,driver}-build-v2.log`. No further storage
rewrite was needed for this experiment.

Maintained new Rust caller `tests/corpus/rng/NativeGeneratorOracle.rs` exercises
all ten implementations through generated interface adapters. Both Rust callers
reuse `oracle_support.rs` for replay/bytes/exceptions; generator algorithms are
never implemented in the caller. `scripts/verify-commons-rng-1.0-behavior.py
ROOT --native` selects12601line checking and a separate `behavior-native`
artifact directory; default remains the original2865line gate. The refactored
source64 caller passes2865lines against the earlier accepted V158 output
(`.tmp/rng-native-refactored-source64-caller.log`), proving test refactoring only,
not yet source64 behavioral acceptance of the new translator.

V5 final source gates (all terminal0): full library **1615/0**,78.69s,
`.tmp/rng-native-library-v5.log`; source contract execution **1/0**,27.97s,
`.tmp/rng-native-contracts-v5.log`; primitive intrinsic runtime **3/0**,24.91s,
`.tmp/rng-native-arrays-v5.log`; lexical-super **4/0**,58.26s,
`.tmp/rng-native-super-v5.log`. The source contract fixture covers MIN/-1,
MIN negation at both widths, int-before-long conditional promotion, small
integer/char unary promotion, checked division under negation, nested narrowing
and caught zero divisors, inherited int/long array overloads, static identity
and hidden static field types. Independent Java wrapper prints0. The new mixed
copy regression covers both directions, three concatenated fresh-copy inputs,
alias identity, array length operands, checked scalar indices and exceptions.
Admission unit controls reject later mutation/rebinding/calls, unproven static
initialization and throwing plain operand captures. Python comparison3/0 and
git diff --check pass.

Frozen candidate `.tmp/new-java2rust-rng-native-contracts-v159-debug`,
202896320bytes, SHA256
`9018ab84108f621469fc7942e8ba48ced4fd541e45f2a1148de39f7966708f26`.
The complete untouched24-file V159 measurement aborted during parallel emission
with a worker stack overflow (terminal1); it produced no usable compiler count.
An authorized GDB backtrace (`.tmp/rng-v159-stack-gdb-authorized.log`) showed
bounded ordinary lowering recursion, not an unbounded IR cycle. Debug expression
lowering already uses roughly158KiB per frame. The scalar-slot changes crossed
the default2MiB worker budget. Focused AbstractWell/ISAAC emission succeeded
with both8MiB and32MiB worker stacks, analyzing all24 files.

Parallel emission now creates named scoped workers with at least8MiB of virtual
stack, preserving larger RUST_MIN_STACK overrides. Unused pages are not committed.
A serial/parallel byte-equivalence regression with nested array/scalar operands
also compiles and executes the generated result: **1/0**,21.14s,
`.tmp/rng-native-worker-stack-runtime-v6.log`.

Frozen V160 `.tmp/new-java2rust-rng-native-contracts-v160-debug`,202906176bytes,
SHA256 `5c8d9a346967bc6cc28861e54bc62d7b7bef71ac4129f90c36fe54d28c98b84f`.
Full24 run `.tmp/commons-rng-1.0-native-v160-contracts` completed terminal1:
all24 translate/parse, allocation guards0, invalid markers0, **6 typed errors**.
Elapsed82.122s (translation57.916, assembly16.975, check3.941, sweep0.674).
The baseline-to-V160 diagnostic delta removes32 occurrences and introduces2:
four errors remain in raw AbstractWell virtual dispatch; two expose lost
constant ownership across flattened constructor frames (parent K incorrectly
rebound to the deepest child's Self).

The constructor follow-through stages static arguments in the caller's lexical
scope before entering the selected constructor, and explicitly qualifies
constructor-prefix constants so later flattening cannot rebind Self. V7 focused
gates pass: source contracts **1/0**,14.69s,
`.tmp/rng-native-contracts-v7.log`; retained constructor boundaries **3/0**,12.68s,
`.tmp/rng-native-constructors-v7.log`. The new three-level fixture deliberately
shadows both a compile-time constant and a lazy static in the deepest child;
the independent Java wrapper returns0. No fresh full-corpus count is claimed
for this follow-through yet.

Stateful abstract raw receiver/adapter inventory is now implemented and under
full-corpus verification. Its new
construction-only admission proof is separate from the stateless declaration-only
layout proof: no base-value demand, concrete descendant materialization,
noncapturing layout, known field/constructor-free ancestors, and only statically
bound private/final own-receiver calls during construction and in retained
helpers. Escaping this, virtual construction dispatch, opaque code and callbacks
veto admission. Keep state/constructors/static/nested support; omit the impossible
raw virtual surface through the same admission across method, adapter, erased
clone/identity/display and object-fallback inventories. Nested inventories recurse
even when the enclosing raw root is excluded.

V9 focused admission tests **8/0** (`.tmp/rng-native-admission-v9.log`), and
executable source contract regression **1/0**,16.99s
(`.tmp/rng-native-contracts-v9.log`). The latter now has two different concrete
virtual overrides called through a source interface, inherited abstract-root
bodies, private constructor initialization, and a static nested table used as
both its concrete type and erased Object. Independent Java prints0.
V9 library gate passes **1619/0**,77.10s (`.tmp/rng-native-library-v9.log`).
Frozen V161 `.tmp/new-java2rust-rng-native-support-v161-debug`,203189320bytes,
SHA256 `d884f9652197b3f9f7c7b8c6c4ce85af9bd6568238d980ae95d46247aa52fd8d`.
Corpus output `.tmp/commons-rng-1.0-native-v161-support`, run log with `-run.log`
suffix. This gate overlaps the library run, so its elapsed time is not an isolated
performance comparison.

## V161 acceptance gates (2026-10-21)

Full fixed24 scope: **terminal0**,24 translated,0 translation failures,
0 parse failures,0 unsupported statements/members/expressions,0 invalid marker
files,0 allocation guards, cargo check0 errors,0 no-op facade call sites,
0 stubbed constants. Authoritative summary is
`.tmp/commons-rng-1.0-native-v161-support/summary.tsv`.
Elapsed119.907s includes a fresh15.225s resolver benchmark plus80.140s
translation,16.644s assembly and4.556s check; this overlapped the library gate
and is not a like-for-like performance comparison with the isolated V160 run.

Independent Java/generated Rust native oracle: **12601 lines match**, including
identical IEEE float/double values. All ten generators and all seven seeds run,
with defensive seed copy checks,2048-draw buffer-cycle checksums, replay,
bounded draws, boolean/float/double results, byte slices/sentinels and malformed
state/foreign-state/invalid-argument exceptions. No generated Rust was repaired.
Evidence `.tmp/rng-native-behavior-v161.log` and
`.tmp/commons-rng-1.0-native-v161-support/behavior-native/{result.txt,java-output.txt,rust-output.txt,rust-stderr.log}`.

Original source64 scope rerun with the **same frozen V161 translator**:
`.tmp/commons-rng-1.0-source64-v161-regression`, terminal0,all11 translated/parse,
check0 errors,invalid markers0. Independent original oracle **2865 lines match**,
IEEE values identical (`.tmp/rng-source64-behavior-v161.log` and its `behavior/`
artifacts). This is fresh compatibility evidence, not reuse of V158 output.

Final focused checks, all terminal0 against the current translator library:
source contract **1/0** (V9,16.99s), retained constructors **3/0**
(`.tmp/rng-native-constructors-v9.log`,13.23s), primitive array intrinsics **3/0**
(`.tmp/rng-native-arrays-v10.log`,14.31s), lexical super **4/0**
(`.tmp/rng-native-super-v10.log`,27.94s), nested-operand serial/parallel emission
and execution **1/0** (`.tmp/rng-native-worker-stack-v10.log`,24.06s).
Python behavioral comparator tests **3/0** and git diff --check pass.
The full library **1619/0** includes the conservative admission veto controls.
The frozen binary and current built CLI have identical SHA256 hashes.

The idiom sweep still reports broad lexical signals from dormant shared-runtime
facades and fallback type names; these counts are not compiler errors or proof
that the entire platform API is supported. Scope remains exactly the pinned
24-file manifest, not factories, serialization or the entire Commons RNG project.

Reproduce from this translator worktree (no upstream/generated source edits):

```sh
CARGO_INCREMENTAL=0 cargo build --bin new-java2rust
NEW_JAVA2RUST_BIN="$PWD/target/debug/new-java2rust" NEW_JAVA2RUST_BUILD_PROFILE=debug \
  RNG_TRANSLATION_ROOT="$PWD/.tmp/commons-rng-native-recheck" \
  bash scripts/run-commons-rng-1.0-native.sh
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/commons-rng-native-recheck --native
NEW_JAVA2RUST_BIN="$PWD/target/debug/new-java2rust" NEW_JAVA2RUST_BUILD_PROFILE=debug \
  RNG_TRANSLATION_ROOT="$PWD/.tmp/commons-rng-source64-recheck" \
  bash scripts/run-commons-rng-1.0-source64.sh
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/commons-rng-source64-recheck
```

Use new empty output roots for fresh measurements. The runners verify the pin,
manifest count and staged source equality before translation. The frozen V161
binary/hash above identifies the accepted producer independently of later builds.
