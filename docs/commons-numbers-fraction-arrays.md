# Commons Numbers Fraction and Arrays translator campaign

Status: **both modules are accepted at V423**, Fraction first and Arrays second,
from untouched release 1.3 commit
`ae8abefb51c2c4d1a05882d61cefc8015e493a8f`. The earlier six-module acceptance
and numerical policies remain in `commons-numbers-corpus.md`; the additional
entries in the shared focus manifest do not retroactively expand that goal.
Preserve all accepted Numbers and Commons RNG behavior. General translator
improvements, not manually repaired generated output, are the deliverable.

## Final acceptance (V423)

The current-state audit passed in
`.tmp/fraction-arrays-v423-requirements-audit.json`; reproduce it with
`python3 .tmp/fraction-arrays-v423-final-audit.py`. It checks the frozen producer
and all 114 translator/runtime input hashes, all thirteen terminal generation
jobs and their complete closure sizes, all zero-error/unsupported/reachable-no-op
gates, regression evidence, complete declarations, owner mappings, independent
behavior outputs and preservation policies. No upstream production source was
modified and no generated Rust was repaired for these acceptance runs.

| Acceptance requirement | Current evidence |
|---|---|
| Fraction full production closure | 17 files: all 6 Fraction + 11 Core; `.tmp/commons-numbers-1.3-fraction-v423-production` |
| Fraction abstract extension and nullable client | Complete 18-file closures at the corresponding `v423-extension` and `v423-boundary` roots |
| Arrays full production closure | All 11 files; `.tmp/commons-numbers-1.3-arrays-v423-production`; nullable client adds one unchanged caller source |
| Source/declaration completeness | Fresh javap manifests match the maintained inventories exactly; source census has 33 types, 428 methods, 32 constructors and 128 fields; all 460 named callables map to their emitted owners |
| Fraction behavior | 3,551 exact independent observations, covering all 129 public declarations, inherited bridges, stateful continued-fraction callbacks, arithmetic, conversions, parsing and exceptions |
| Arrays behavior | 5,052 exact algorithm/helper observations plus 20 typed-null and 20 direct null-exception contracts; all 43 public/inherited declarations mapped |
| Regression checks | 1,789 passing tests: 1,738 library, 5 Arrays, 7 identity, 7 exception text, 13 Fraction, 5 native SAM and 14 numeric runtime |
| Earlier Numbers/RNG | All 15 behavior jobs meet the existing policies; raw Complex and Quaternion Java/Rust/difference files are byte-identical to V422; fresh Quaternion geometric audit passes |

The Fraction independent modes contain 220 GCF, 1,560 Fraction, 1,318 BigFraction,
396 continued-fraction extension, 29 boundary, 5 exception-identity and 23 null
exception rows. Arrays contains 4,364 Selection, 407 sort/enum, 112 counter and
169 helper rows, plus the two twenty-row nullable supplements. Source-declaration
presence and public invocation are separate checks; neither is represented as
exhaustive path coverage. Private helpers and mutation/aliasing paths have
dedicated observations in addition to API mappings.

The production-only abstract ContinuedFraction has no legal Java instance;
its abstract getA/getB declarations are not replacement algorithm bodies.
The separate unchanged concrete extension is translated through the normal
pipeline and independently exercises inherited evaluation, virtual getA/getB,
escaping mutable Supplier state, aliasing, repeated calls and callback failures.
Its generated hierarchy dispatch selects the concrete implementation. Unused
external facade definitions are not behavior evidence: acceptance separately
requires zero facade call sites reaching unimplemented methods. SortInPlace's
production calls reach the checked native collection-sort kernel, not the
unused Collections facade definition.

The entry baselines and whole-path rewrite audits below remain historical
evidence. General fixes cover runtime arbitrary-precision arithmetic/decimal
contracts, producer/storage/call shapes, dynamic and stateful callbacks, enum/
varargs/array identity and ordering, and precise exception provenance. The final
diagnostic batch and its rationale are recorded next.

## Strict diagnostic follow-up (V423)

V422 frozen producer
`.tmp/new-java2rust-numbers-default-v422-local-null-provenance-debug`
(SHA256 `88ccfa3e4882fdd94685c02ef0ef62574fe1a07aa42e539b111d91d74daa2308`)
completed all thirteen production/contract generation jobs. Independent callers
matched 5,052 Arrays rows, another twenty typed-null Arrays contracts, and
3,549 Fraction rows. All 460 named source callables were located in their
correct generated owners. Earlier Numbers/RNG preservation completed under
the existing policies; Complex/Quaternion raw observations matched V421.
These V422 results alone did not establish acceptance of either new module.

The final exception audit added twenty direct Arrays exception-message cases
and two Fraction null-Supplier calls. Five messages differ: three Selection
null-index arrays in an enhanced array loop, and two GeneralizedContinuedFraction
Supplier calls. The independent probes preserve exception class, mutation and
state observations; no message waiver is applied. Evidence is retained in
`.tmp/arrays-v422-null-exception-r1` and
`.tmp/fraction-v422-public-null-exception-r2`.

The smallest correct enhanced-loop rewrite needs the source JVM array temporary
slot, rather than the named iterable's slot. This is a local emission change
using the cached binding/scope model: account for javac's array/length/index
temporaries, reuse the existing checked-array projection, and preserve nested
operand diagnostics. `LocalNullMessageProbe` now exercises both a null iterable
and a null row, with independent Java expectations.

The Supplier issue crosses the invoking-only callback ABI. Its null value is
already captured lazily, but a synthetic adapter currently loses the original
`gen.get()` frame. The minimal whole-path rewrite preserves null provenance in
a private exception flag: a missing native facade returns the flagged failure
only when invoked; the first actual source SAM invocation contextualizes and
consumes it. Retained/non-invoking callback representations remain governed by
the existing callback plan. User exceptions are never recognized by message
text. Diagnostic descriptors reuse shared SAM metadata with JVM-erased inputs,
so an instantiated `Function<String, String>` still reports `apply(Object)`.
`NullableSamNullMessageProbe` covers lazy unused nulls, forwarded/padded calls,
erasure, stateful callbacks, and explicit user NPEs with the same generic text.
Focused verification passed: all seven exception-text executable contracts in
`.tmp/exception-text-v423-batch-r1.log`, including both new regressions, and the
eleven-case local-slot probe in `.tmp/local-null-v423-regression-r2.log`.
`git diff --check` passed. The subsequent complete V423 verification is recorded
in the final acceptance table above.

The V423 producer is now frozen as
`.tmp/new-java2rust-numbers-default-v423-null-invocation-provenance-debug`,
SHA256 `8b935a5652282a5b0a1e3274064c9ef9818d00030137dc980e2f755296d1a007`.
`.tmp/v423-producer-inputs.json` records 114 translator/runtime input hashes.
All 1,738 library tests, five Arrays contracts, seven identity-resolution
contracts, seven exception contracts, thirteen Fraction lowering contracts,
five native SAM contracts and fourteen numeric-runtime tests passed.
Terminal generation jobs are in `.tmp/v423-generation-terminal.json`; Fraction
and Arrays behavior jobs are in `.tmp/v423-fraction-behavior-terminal.json` and
`.tmp/v423-arrays-behavior-terminal.json`; preservation is in
`.tmp/v423-behavior-terminal.json`. Frozen caller copies and source/producer
provenance are retained in each behavior root. No new numerical or exception
message allowance was introduced. The existing Core four NaN-sign allowances,
Complex reference policy (78 strict differences), and Quaternion geometry
policy (12 strict differences) remain explicit; strict outputs are retained.

## Production scope and verified dependencies

The shared `scripts/focus-sets/commons-numbers-1.3-all.txt` now includes all
own production files for these modules. The maintained module runner compares
each selection with the full pinned upstream inventory and stages byte-identical
files; the API runner compiles that same complete closure and uses `javap -p -s`
on every target class, including private/nested/synthetic declarations.

| Order | Module | Own production files | Required production closure |
|---|---|---:|---|
| 1 | Fraction | 6 | All 11 Core files + 6 Fraction files = 17; JDK |
| 2 | Arrays | 11 | All 11 Arrays files; JDK only |

Fraction files: BigFraction.java, ContinuedFraction.java, Fraction.java,
FractionException.java, GeneralizedContinuedFraction.java and package-info.java.
The POM declares Core as production; imports use ArithmeticUtils and
NativeOperators. JDK dependencies include BigInteger, BigDecimal, RoundingMode,
Objects, Serializable and Supplier. No other Numbers module is required.

Arrays files: BitIndexUpdatingInterval.java, HashIndexSet.java, IndexSupport.java,
KeyUpdatingInterval.java, MultidimensionalCounter.java, QuickSelect.java,
Selection.java, SortInPlace.java, Sorting.java, UpdatingInterval.java and
package-info.java. Its POM's Math3 and RNG dependencies are test-scoped;
production imports are JDK Arrays/ArrayList/List/Collections/Comparator only.
No test-scoped dependency is silently included or replaced by a stub.

## Fraction entry baseline (V379)

Use the unchanged V378 accepted producer:
`.tmp/new-java2rust-numbers-default-v378-additive-erased-copy-debug`, SHA256
`77aecc615c5562cc3c3aa083dc52f9463148a9b9559b37f7912f2fa71e85a95c`.
The current normal debug CLI has the identical hash. No translator or runtime
fix precedes this baseline; only runner selections/source manifests were added.

Baseline translation root `.tmp/commons-numbers-1.3-fraction-v379-baseline`,
staging `.tmp/numbers-fraction-v379-baseline-staged`, outer log
`.tmp/numbers-fraction-v379-baseline-run.log`. Launched with the normal runner,
`NUMBERS_MODULE=fraction`, `JOBS=2`, debug/offline settings and
`--skip-resolver-bench`. Completion and generated-code feasibility are not yet
claimed. Declaration inventory is
`tests/corpus/numbers/Fraction-1.3-declarations.txt`.

Source-inspection audit priorities (not yet conclusions from generated code):

- BigFraction retains immutable arbitrary-precision numerator/denominator
  values, overloads arithmetic, compares and hashes them, constructs decimal
  results and implements exact rational-to-binary rounding. Audit mapped call
  results, borrowed versus owned arguments, constructor overloads and runtime
  API coverage end to end. Existing runtime support is deliberately a subset;
  do not confuse a facade compiling with a real numeric implementation.
- Fraction exercises checked integer arithmetic, normalization and parsing,
  self-return identity and NativeOperators generic/inherited operations. Exact
  integer and exception observations should expose bugs without FP tolerance.
- ContinuedFraction's abstract getA/getB methods are invoked by an anonymous
  mutable Supplier retaining the evaluation point and enclosing instance; its
  counter must persist through calls, virtual dispatch must reach the subclass,
  and callback errors must cross the same effect boundary as evaluation.
- GeneralizedContinuedFraction has six value overloads plus its shared evaluate
  helper; preserve exact supplier invocation order/count, early convergence,
  nonconvergence/errors and the first-coefficient/b0 distinction. Exercise
  plausible convergent workloads and production safeguard branches, not an
  unrelated elementary-function rounding campaign.

Before implementing fixes, inspect actual generated producer/storage/signature/
consumer failures and classify concrete minimal rewrites. A diagnostic count
alone is not the plan.

### V379 terminal baseline and generated-code rewrite audit

The complete 17-file baseline is terminal **1**, not accepted. All files
translate and parse, unsupported statements/members/expressions and invalid/
stubbed markers are zero. Rust compilation reports **105** errors. Total
342829 ms: translation 300747 ms, assembly 35036 ms, check 3630 ms. Static
no-op-call scanning reports zero, but the supplier finding below proves that
scanner result is not sufficient evidence of semantic completeness.

Concrete failures and smallest correct whole-path rewrites:

1. **External numeric contract coverage.** BigFraction's two-argument
   constructor emits `num.gcd(den)` with nullable `den`, followed by unmodeled
   divide calls; read-only numeric fields are sometimes moved or passed as
   Option rather than borrowed nonnull values. BigDecimal construction receives
   BigInteger without a corresponding input implementation and division falls
   back to nonexistent divide_1/2/3 methods. Parsing emits an unrelated
   `BigInteger::new` facade rather than the mapped runtime constructor.
   Minimal correct Rust discharges Java null at the source boundary, borrows
   immutable numeric operands, calls real arbitrary-precision arithmetic, and
   propagates the declared numeric errors. Extend the existing external-pack
   manifest and numeric backend together, not a parallel arithmetic emitter.
   First batch: BigInteger unary/binary integer/bit operations and Java hash,
   with borrowed operand metadata and negative/two's-complement tests. Follow
   with UTF-16 parsing and BigDecimal integer construction/division, reusing
   existing decimal rounding instead of a second rounding policy. These are
   reusable runtime + cross-function argument/effect contracts.
2. **Anonymous state and lexical enclosing dispatch.** Generated
   ContinuedFraction.evaluate contains
   `let gen: Supplier<Rc<Coefficient>> = Default::default()`, completely losing
   the anonymous object's `n`, `x` capture and getA/getB calls. Minimal correct
   Rust constructs one actual Supplier adapter, retains its own mutable counter
   and captured x, and calls getA/getB through the canonical enclosing receiver
   (virtual subclass dispatch when instantiated by a client). Each call updates
   the same adapter state; an exception retains the incremented counter and
   does not invoke the later getB. Existing anonymous source-interface and
   native Supplier adapters must share the relevant field/capture lowering;
   blindly enabling a source-interface shortcut cannot work because JDK
   Supplier is modeled natively, not a source TypeDecl. Existing source carrier
   dispatch and retained-outer planning can express the intended representation;
   no GC or ownership-framework redesign is currently demonstrated. This is a
   native-adapter/storage/virtual-call contract batch, not a method-name patch.
3. **Shared self-return / interface consumer ABI.** Generated BigFraction
   has only add/subtract `*_with_outer` implementations for shared-self
   operations, while some consumers still name non-existent ordinary helpers.
   Minimal correct Rust routes these calls through the same canonical Arc and
   solved owner signature, retaining Java `this` identity. Trace static helper,
   inherited NativeOperators default and source receiver paths before changing
   call emission; do not add guessed wrapper signatures. Existing shared-self
   source call planning is the authority. Cross-function call/signature batch.
4. **Text and display boundaries.** Fraction parsing emits `.parse()` on
   JavaUtf16String; BigFraction parsing emits `.replace()` on it. BigInteger
   string concatenation lacks JavaAppendValue. Minimal rewrites use existing
   UTF-16 Java text/parse/replace and mapped-native display adapters, preserving
   nulls, exception messages and original values; adding string methods to the
   wrong carrier would conceal a producer/consumer ABI error. Reuse existing
   text/append authorities. These local projection/mapped-runtime contracts
   should be batched with executable caller coverage.

Focused control `StatefulOuterSupplierProbe.java` retains a mutable anonymous
Supplier from an abstract owner's factory, calls through aliases, mutates the
original concrete owner, creates independent generators and throws between
counter increment and the second virtual call. Java passes all nine guards.
The first harness invocation used `contract=0` where the shared helper expects
plain `0`; only that output framing was corrected. The actual second run
`.tmp/fraction-v379-supplier-focused-r2.log` reaches generated-Rust execution
and fails. No product fix or assertion removal is hidden by the framing repair.
Its generic regression is `tests/fraction_lowering_contracts.rs`; no corpus
algorithm is implemented there. Both declaration inventories now exist.

No translator/runtime implementation has changed at this audit checkpoint.
Proceed through the coherent batches above, retain the failed baseline, and
rerun complete Fraction only after a substantial related batch. Arrays remains
pending, not removed from the objective.

### V380 first numeric batch

Implemented the first audited numeric batch in reusable
`runtime/java-math/src/integer.rs`, exported by the existing runtime crate.
Operations are arbitrary-precision add/subtract/divide/gcd/abs/negate/signum,
bitLength/getLowestSetBit/flipBit and Java's magnitude-word hashCode. Negative
bitLength uses the infinite two's-complement complement; division truncates
toward zero; bit flipping preserves negative sign extension and checks invalid
indices/range. Backend gcd reuses num-integer, already present transitively;
it is now an explicit pinned dependency. The external-pack manifest supplies
borrowed operands, return types and error effects for these actual APIs.
No Fraction algorithm was rewritten in Rust.

`.tmp/fraction-v380-runtime-tests.log` is terminal green, **11/11 runtime unit
tests**, including new unbounded arithmetic and two's-complement/hash cases.
The independent Java fixture `BigIntegerArithmeticProbe.java` exercises all
new operations, 200-bit values, reused operands, zero/negative inputs, caught
numeric errors and a caught typed-null argument. The first generated run
`.tmp/fraction-v380-integer-focused.log` reaches that null check and fails:
the borrowed mapped operand uses `.expect("Java null receiver")`, bypassing
the Java catch. No prior arithmetic guard fails.

Minimal correct rewrite replaces the invocation's required-value projection
with the existing fallible Java-null projection before foreign Result wrapping.
`apply_call_target_with_lower_context` now applies that existing mechanism at
the final mapped-call ABI boundary. This is shared across mapped libraries,
not specific to BigInteger, and does not change the foreign numeric algorithms.
The unchanged Java guard is retained. Final focused
`.tmp/fraction-v380-integer-focused-r2.log` is terminal green, **1/1 translated
Java/generated-Rust numeric regression**, including null/error cases. Source
compilation took 60 seconds and the complete focused regression 34.15 seconds.

Mapped-runtime preservation is terminal green in
`.tmp/fraction-v380-mapped-preservation.log`: all **4/4 BigDecimal integrations**
(25.45 seconds) and **7/7 BigInteger integrations** (53.91 seconds) pass.
No full Fraction regeneration is claimed: remaining mapped constructors,
decimal division, UTF-16 parsing/display, shared-self call ABI and anonymous
Supplier contracts are still required. The supplier regression remains a
known real runtime failure (`Supplier.get is not modelled`), not a skipped
acceptance requirement. Full library and prior-corpus preservation belong at
a substantial later boundary after these coherent batches, not after each API.

Temporary-storage incident: the root filesystem was full during baseline
observation. Removed only the validated, regenerable 33 MB Cargo `target`
directory of an already completed receiver-liveness regression; its source and
lockfile were retained. New regression TMPDIR is
`.tmp/fraction-arrays-test-tmp` on the data filesystem. The original baseline
was polled to terminal failure, not restarted on an observation timeout.

### V381 decimal division and arity-aware mapped parameter contracts

BigFraction's three bigDecimalValue overloads require exact division,
rounding at the dividend's scale, and rounding at an explicit scale, all from
BigInteger-backed decimal construction. The runtime now models these real APIs
using unbounded coefficients. Exact division reduces the rational and removes
denominator factors 2/5; rounded division and setScale share a single exact
quotient/remainder rounding authority. No binary floating point or copied
Fraction algorithm is involved. Scale choices follow the
[JDK API contract](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/math/BigDecimal.html).

The first mapped caller exposed borrowed enum arguments where the Rust API
requires copied values. The existing fact/index model already supports
arity-specific argument effects, but manifest parsing discarded that axis.
The manifest's optional `args=COUNT` now fills that existing key, permitting
index 1 to be RoundingMode at arity 2 and int at arity 3. The generic parser
regression checks separate overloads, legacy fallback, invalid/duplicate arities
and out-of-range indices. Same-arity overload discrimination is not claimed.
`docs/external-facts.md` records the syntax. No new overload dispatch engine or
BigDecimal-specific coercion rule was added.

The independent `BigDecimalDivisionOracle.java` matrix makes **12548** requests
across all three division overloads, all eight modes, sign/zero/tie/exact and
nonterminating cases, negative/positive scales and arbitrary-precision integer
inputs. Output compares decimal spelling, scale, double/float bits and error
class/message. It exercises runtime APIs, not replacement Fraction math.
The first run exposed different zero-error diagnostics for exact versus
rounded overloads; the bounded diagnostic adapter now retains the JDK's
compact/inflated integer-division distinction, while results remain unbounded.
The second run identified HotSpot's warmup-dependent preallocated fast-throw
exception erasing a message. The Java matrix now disables
OmitStackTraceInFastThrow so original API diagnostics are compared consistently;
no expected message or failing request was removed. The caller collects all
mismatches before failing to avoid one-error-at-a-time cycles.

Terminal evidence:

- `.tmp/fraction-v381-division-matrix-r3.log`: **1/1** matrix test passes all
  **12548** exact Java observations (3.99 seconds).
- `.tmp/fraction-v381-decimal-focused-r2.log`: **2/2** prior mapped decimal
  integrations and the **1/1** new translated Java division/BigInteger-input
  client pass, including repeated reads and all null/error guards (24.09 and
  26.13 seconds).
- `.tmp/fraction-v381-argument-arity-r2.log`: **1/1** generic arity/index
  regression passes. Its first invocation lacked the required return-nullability
  token in the synthetic API declarations; only that manifest framing was
  corrected, not the overload assertions.
- `.tmp/fraction-v381-runtime-tests.log`: **12/12** unit tests passed before
  the final zero-diagnostic refinement; the V382 batch reruns the final runtime.

The installed independent oracle is OpenJDK **19.0.2+7-Ubuntu-0ubuntu322.04**.
OpenJDK API/source documentation informed the boundary audit; actual acceptance
observations come from this installed Java runtime, not assumed JDK 21 outputs.

### V382 integer String construction and shared UTF-16 numeric text

Production BigFraction.parse calls BigInteger(String) on its cleaned numeric
parts. The prior fallback facade cannot implement this boundary. Added a
modeled UTF-16 constructor returning the existing NumberFormatException and
mapped BigInteger.toString through the existing UTF-16 output boundary.
BigInteger and BigDecimal now share `numeric_text::decimal_digit`; no second
Unicode-digit table was introduced. Decimal normalization is passed to the
arbitrary-precision backend once. Sign/zero/invalid-group validation retains
original Java UTF-16 error units; backend FromStr remains a separate native
convenience API and is not substituted for Java validation.

`BigIntegerParseProbe.java` independently checks large/signed/Unicode inputs,
retained input/result reads, malformed signs, invalid digit groups, unpaired
surrogate and supplementary-digit errors, and null. The existing arithmetic
and decimal clients are rerun alongside it. New runtime unit coverage checks
the same digit sharing and raw error units. Final V382 focused/runtime/previous
numeric integration logs are terminal green:

- `.tmp/fraction-v382-runtime-tests.log`: **13/13** runtime unit tests.
- `.tmp/fraction-v382-mapped-focused.log`: **3/3** independent translated
  integer arithmetic, decimal division and integer parsing clients (33.84 s).
- `.tmp/fraction-v382-numeric-preservation.log`: **5/5** decimal integrations
  (including both independent matrices) and **7/7** BigInteger integrations
  (47.80 s for the latter). This also preserves older toString/scalar/format
  users after the UTF-16 boundary change.

Normal CLI build completed in `.tmp/fraction-v382-producer-build.log`. Frozen
producer `.tmp/new-java2rust-numbers-default-v382-numeric-contracts-debug`,
SHA256 `5be554367e16f0497b64d3c5c357c8f3214ba037d7215b1e14dea3684ef802d7`.
No translator/runtime edit follows this freeze at this checkpoint. A substantial
numeric/API batch now justifies one complete Fraction regeneration, rather
than a full run after each API. It is running with the normal runner on all
17 untouched source files in
`.tmp/commons-numbers-1.3-fraction-v382-numeric`, staging
`.tmp/numbers-fraction-v382-numeric-staged`, outer log
`.tmp/numbers-fraction-v382-numeric-run.log`, owned session **90844**. Settings
remain debug/offline, JOBS=2, --skip-resolver-bench and the data-backed TMPDIR.
Its terminal result remains pending. Neither this launch nor focused passes
prove Fraction or Arrays acceptance, full-library acceptance, or preservation
of the earlier entire corpora. Anonymous supplier state/virtual outer dispatch,
source text projections and shared-self consumer ABI remain in the recorded
whole-path plan; the actual regenerated result will select the next batch.

### V382 terminal regeneration and V383 interface ABI plan

The complete 17-file V382 regeneration is terminal **1**: all files translate
and parse, unsupported/invalid/stubbed markers remain zero, and compilation
now reports **41** errors versus V379's 105. This is evidence of progress, not
acceptance. The anonymous Supplier default is still a known semantic failure
even though static no-op scanning reports zero. Current remaining families:
required borrowed BigInteger arguments versus nullable physical carriers,
immutable shared interface forwarding, UTF-16 replace/integral parsing,
mapped-native append, and associated effect/ownership projections. The same
untouched closure and frozen producer were used; no generated file was edited.

Refined interface feasibility audit: BigFraction.add(BigFraction) and subtract
can return `this`. Inherent method emission correctly omits their borrowed
payload entry points and emits only `_with_outer(Arc<...>, ...)`. Trait emission
currently generates both `Trait for BigFraction` and `Trait for Arc<BigFraction>`
forwarding to those absent payload methods. Creating a new Arc from a payload
clone would compile but violate Java identity, so it is not a valid rewrite.

Minimal correct Rust implements the interface on canonical storage when an
implemented slot is companion-only, and forwards its methods through existing
`_with_outer` entries. Reuse the existing shared-adapter machinery, extended
to direct immutable Rc/Arc handles rather than only RefCell/Mutex handles.
Determine companion-only ABI from the actual emitted method inventory, not a
second syntactic guess about whether a body needs `this`. Interfaces whose
payload methods remain valid should retain their existing payload impls.
Inherited source methods already have emitted subtype companions and should
use that same declaring/forwarding owner authority. This is a cross-function
emission/trait-consumer contract repair, not a representation redesign.

`ImmutableInterfaceIdentityProbe.java` provides a source-only independent
control: two inherited generic interface levels, immutable/static-reachable
canonical objects, overloaded add, direct/interface/generic-bound calls,
return-this identity, fresh results, retained input state and caught null.
It is being measured on the unchanged V382 implementation before this batch.

### V383 canonical immutable interface forwarding and hierarchy qualification

Implemented the refined interface ABI plan using the actual inherent method
inventory produced by lowering. Trait emission now detects a compatible slot
whose borrowed method is absent and whose canonical-handle companion exists.
For these immutable shared implementors it reuses the existing shared adapter
machinery with direct Rc/Arc storage; forwarding invokes the real companion
and retains the original handle. It does not generate an impossible payload
impl or clone a payload to reconstruct identity. Existing valid payload impls
are retained for other interfaces. Companion naming now shares one
`java_naming::outer_handle_companion_name` function with ordinary companion
emission. Specialized interface contracts are collected once and reused for
admission and method emission, including inherited slots.

The independent generic/nested regression exposed an earlier naming fault:
the resolver qualified method parameter types but tried to rewrite generic
extends/implements edges by exact string lookup. `I<Nested>` therefore remained
unqualified, disconnecting override compatibility and fallibility. An
intermediate representation-local normalization made the mismatch visible as
Result-versus-value forwarding errors; it was removed, not retained as a
parallel naming mechanism. The final repair parses each hierarchy edge and
uses the existing binder-aware nested-type qualifier and Java type renderer
in the resolver, before signature/effect/representation consumers run.

`ImmutableInterfaceIdentityProbe.java` now tests inherited generic interface
levels, both static-reachable and local-only immutable object paths,
overloaded add, direct/interface/generic-bound calls, canonical return-this,
fresh results, retained input state, null failure and subsequent reuse. The
first Rust test helper used a trait named Value, colliding with the Java
nested type's convenience alias; only the helper was renamed. The genuine
baseline then fails in interface dispatch with incompatible parameter types.
No Java assertion was removed. The first canonical-adapter run still failed
on the earlier hierarchy-name/effect mismatch, motivating the resolver repair.

Final focused evidence is terminal green:

- `.tmp/fraction-v383-interface-arguments-r2.log`: **2/2** unit tests, including
  nested argument resolution and preservation of a type parameter when a
  same-named source class exists. The initial unit compile missed the `super::`
  qualification on a test-only helper; that was corrected without changing
  the tested contract.
- `.tmp/fraction-v383-immutable-interface-focused-r3.log`: **1/1** executable
  Java/generated-Rust control, **15.44 seconds**, with all ten guards.

The full library plus value-identity, native/source SAM and enum callback
integrations finished green in `.tmp/fraction-v383-library-related.log`:
**1723/1723** library tests and **12/12** related integrations (five value
identity, five native SAM and two enum callback tests). The normal V383 build
also finished successfully; frozen producer
`.tmp/new-java2rust-numbers-default-v383-interface-contracts-debug` has SHA-256
`a817cc367642e53731663a560f501e6bc616329873f5ffde816a55b6a8af3534`.
No V383
complete Fraction regeneration or earlier-corpus preservation is claimed yet.
The last measured complete Fraction count remains V382's **41**; focused
success is not a measured new full-corpus count. Supplier, mapped field-value
producer/consumer ABI, UTF-16 text and append issues remain in scope.

### V384 fixed-width comparison contract audit

The V382 Fraction output shows two missing JDK operations: signed
`Long.compare` cross-products in Fraction.compareTo and unsigned
`Long.compareUnsigned` overflow comparisons in BigFraction conversion.
The emitted imported facades contain fake bodies, and the former incorrectly
requires a receiver. These are missing native API models. Minimal correct
rewrites compare two values in the declared Java width, reinterpret that width
as unsigned when requested, and return exactly -1/0/1 without subtraction
overflow. This is a local native-call model plus argument/return contract
extension; it requires no new object representation.

Implement both Integer and Long through existing native call planning,
primitive argument shaping (including widening and checked unboxing), and the
shared comparison kernel. Extend the independent numeric caller with extrema,
equality, cross-width inputs, static import, operand evaluation order and null
unboxing. Extend the source-owner lookalike control to ensure source methods
retain their declared contracts. The initial complete native numeric suite
passed the extended comparison caller and both scaling controls, but failed
the radix-formatting and source-owner callers. Their generated code shows
Rust `format!` values reaching logical UTF-16 String consumers unchanged;
one fully qualified call also casts JavaCodeUnit directly to u32 rather than
first widening to the selected native formal. Extend the existing lower-level
native argument shaping to radix formatting and the existing native String
result normalization to its output, so qualified calls and synthesized method
references use the same boundary. These are the same numeric/text ABI family;
no fixture assertion is removed. The rerun
`.tmp/fraction-v384-native-numeric-r2.log` is terminal green: **5/5** independent
Java/generated-Rust integrations, including both previously failing controls.
The full library suite also finished green in
`.tmp/fraction-v384-library.log`: **1723/1723**, **78.84 seconds**. Diff
whitespace checks pass. No new complete Fraction count or acceptance is
claimed; the next full regeneration should include the related mapped-field
ABI fixes rather than incur a complete corpus cycle for these numeric calls
alone.

Next mapped-field investigation: `plan_call_target` currently selects
`resolved_external_instance_call_rust_name`'s generic FallbackReceiver before
consulting `plan_external_crate_receiver_call`. This is a concrete candidate
for the production multiply calls bypassing the pack's free-function borrowed
argument ABI. Verify with a minimized holder using mapped immutable numeric
fields and same-named source methods, including nullable operands, reuse and
getters. Check source-owner authority before changing precedence; do not add
Borrow<Option<BigInteger>> runtime adapters to hide carrier errors.

### V385 mapped numeric field/signature audit

The independent `MappedNumericFieldsProbe.java` uses immutable numeric fields,
source methods with the same names as numeric APIs, borrowed getters, chained
arithmetic, repeated reuse and null receivers/operands. The entry baseline
`.tmp/fraction-v385-fields-baseline.log` fails with ten Rust errors after Java
passes. Its arithmetic already selects mapped free functions correctly,
while getters declare a bare BigInteger and return Option<&BigInteger>.
Callers expect the nullable/borrowed getter shape and become inconsistent.
Some chained accesses also omit the nullable source receiver projection.

One confirmed cause is a late special case in source method lowering:
`rust_crate_mapped_external_rust_type(method.return_type)` overwrites the
signature plan using the bare declared Java type, discarding nullability and
ownership. It then returns early, skipping remaining parameter ABI work.
Minimal correct rewriting retains the slot-rendered mapped payload within
the signature plan's Option/borrow wrappers and uses the same result contract
at call sites. The authoritative slot renderer already maps named foreign
types; remove the duplicate override rather than create a second renderer.
This is a cross-function signature/body/consumer contract correction. An
overloaded source multiply is added to distinguish the separate suspected
mapped-target precedence fault. The first follow-up still fails because method
assembly has the same bare-type override; give the solved shape priority there
as well. The accessor fallback also needs ordinary optional receiver adaptation
when shared-object adaptation is inapplicable.

After these repairs the next generated code confirms correct
`Option<&BigInteger>` getter signatures and nullable Pair receiver checks, but
exposes a downstream projection error: `.as_ref()` on the returned
Option<&BigInteger> produces &&BigInteger, which does not implement the
runtime's Borrow<BigInteger> ABI. Select the existing borrowed optional
receiver projection using the actual slot-rendered physical type, in both
ordinary and explicitly fallible receiver paths. Owned optional storage keeps
its existing projection. No runtime Borrow widening or cloned payload is used
to conceal the mismatch. The final fixture includes source multiply overloads
for Pair, int and BigInteger. Adding the last overload made an untyped Java
null call ambiguous; the caller now explicitly casts that null to Pair,
preserving the intended null-operand assertion. Final focused evidence
`.tmp/fraction-v385-fields-r4.log` is terminal green: **1/1** independent
Java/generated-Rust caller, **25.11 seconds**, retaining all twelve guards.
The shared planner precedence suspicion did not reproduce in this caller;
no speculative precedence change was made.

Terminal V385 batch evidence:

- `.tmp/fraction-v385-library-numeric.log`: **1723/1723** library tests
  (**78.01 seconds**), **5/5** BigDecimal (**24.90 seconds**), **7/7** BigInteger
  (**49.86 seconds**) and **5/5** native numeric (**18.44 seconds**).
- `.tmp/fraction-v385-mapped-preservation.log`: **4/4** mapped Fraction-lowering
  controls (**32.81 seconds**), including the new field regression. The two
  source-only controls are outside this focused numeric filter; this is not
  a claim that all Fraction behavior is green. The separate stateful anonymous
  Supplier control remains a known open failure, not an acceptance exclusion.
- Normal producer build `.tmp/fraction-v385-producer-build.log` succeeded.
  Whitespace checks pass.

A fresh full 17-file Fraction closure run now uses frozen producer
`.tmp/new-java2rust-numbers-default-v385-mapped-slots-debug`, with fresh root
`.tmp/commons-numbers-1.3-fraction-v385-mapped-slots`, untouched staging root
`.tmp/numbers-fraction-v385-mapped-slots-staged` and log
`.tmp/numbers-fraction-v385-mapped-slots-run.log`. No new complete Fraction
error count is claimed before it reaches a terminal result.
The frozen producer SHA-256 is
`0885d2d79e9fa896e67068b0fb850d4f4b4628f3474b83040b2d049147dfbd6b`.

The complete V385 run is terminal (exit 1): all **17** production files
generate and parse, unsupported statements/members/expressions, invalid
patterns and stubbed constants remain zero. Rust compiler errors fall from
V382's **41 to 27**. Total time is **243529 ms**: translation **213156 ms**,
assembly **25458 ms**, check **2025 ms**. Remaining errors: twelve optional
BigInteger operand ABI failures, four mapped numeric append failures, three
UTF-16 integral parse failures, two missing literal replacements, two integral
format/String result mismatches, and four immutable field moves. Supplier
correctness is still open independently of these compile counts; static
no-op call scanning reports zero but does not prove callback implementation.

### V386 literal String replacement contract audit

Production Fraction.parse and BigFraction.parse invoke String.replace with a
literal comma. V382 emits `.replace` directly on JavaUtf16String, which has no
such operation. The independent `StringReplaceProbe.java` baseline passes
Java but fails generated Rust with 17 missing-method errors in
`.tmp/fraction-v386-replace-baseline.log`.

Minimal correct rewriting operates on UTF-16 units, preserves literal
non-overlapping replacement (not regex rules), and returns an existing or
fresh String handle according to the actual operation. Both char/char and
CharSequence/CharSequence calls can use the existing JavaTextView boundary;
add its one-code-unit adapter and a payload replacement operation, then let
the existing String identity layer return the appropriate handle. Remove
the unconditional fresh wrapper only for this now identity-aware native
operation. This is a local runtime/emission contract extension, not an object
representation redesign. Existing null argument and receiver projections
remain authoritative.

Installed OpenJDK 19 observations from diagnostic-only
`.tmp/StringReplaceIdentityAudit.java` establish that no-match and equal
single-unit replacements retain this, while successful equal multi-unit and
empty-pattern/empty-replacement operations allocate fresh objects, including
empty input. These observations are now executable guards in the independent
caller, alongside isolated surrogates, splitting a surrogate pair, empty
patterns, removal, literal dollar/backslash contents, nullable arguments,
CharSequence values and retained source aliases. Focused implementation
`.tmp/fraction-v386-replace-r1.log` is terminal green: **1/1**, **12.87 seconds**,
retaining all twenty guards. V385's corpus measurement uses its frozen binary.

The same measured text boundary family includes Fraction.toString's
Integer.toString result: a native Rust String reaches a logical UTF-16 String
slot unchanged. Extend native Integer/Long toString modeling for both the
one-argument decimal and two-argument signed radix operations; use the existing
fresh String result boundary. Fixed-width radix formatting (toHexString etc.)
remains unsigned, while toString(value, radix) includes the sign. The bounded
formatter works from unsigned magnitude, including MIN_VALUE, and clamps
out-of-range formatting radices to decimal according to the Java API.
`native_integral_text_argument_plan` now owns the width/unboxing/radix formals
for both format families and is reused by shared effect/argument planning and
lowering, replacing their duplicate fixed-radix formal construction.

Extend the independent IntegralFormatProbe and source-owner control with
decimal/signed-radix extrema, char/short widening, default radices, boxed null
failure, fresh result identity and source toString lookalikes. The combined
lossless String, Java text and native numeric suite is running in
`.tmp/fraction-v386-text-numeric.log`; no new complete Fraction count is claimed.

That combined run finishes with **2/2** Java text tests (**2.35 seconds**),
**5/5** lossless String tests (**24.67 seconds**) and **4/5** native numeric
tests. The source-owner control exposes a distinct actual naming boundary:
static source toString(int) receives the bare `to_string` name, suppressing
the inherited zero-argument Object fallback, while the generated display
adapter still calls that name with &self. The erased-display collision test
also classifies Java spelling instead of the emitted method name.

Extend the existing resolver overload allocator to count inherited
Object.toString() when an argument-taking source toString is present, using
the existing canonical overload suffix function. Display collision admission
must consume that emitted name. This preserves the source static method and
the existing Object protocol rather than reimplementing a display operation.
The native numeric rerun is terminal green in
`.tmp/fraction-v386-native-numeric-r2.log`: **5/5**, **17.41 seconds**. All
expanded decimal/radix and source-shadow guards are retained. A full library
plus Java text, lossless String and object/String contract gate completed in
`.tmp/fraction-v386-library-text.log`: **1723/1723** library (**76.78 seconds**),
**2/2** Java text (**2.25 seconds**) and **5/5** lossless String
(**24.54 seconds**) pass. The extra Object/String suite is **0/2**, so the
combined command is exit 101 and its chained producer build did not run.
Its first failure demands that a scalar facade scope omit Any runtime,
whereas the current UTF-16 producer emits that runtime; its second exposes
missing UTF-16 toUpperCase plus scalar-only test helpers (`as_str` and
String-only Text implementations) from the earlier text migration.

Both relevant producer behaviors were rechecked with the unchanged frozen
V385 binary: `.tmp/fraction-v386-object-protocol-v385-baseline.rs` contains
`java_erased_any_to_string`; the exact Java fixture in
`.tmp/fraction-v386-object-contracts-v385-baseline.rs` emits
`java_erased_object_to_string(&buffer).to_uppercase()` although JavaUtf16String
has no such method. These are retained open earlier-migration issues, not
claimed green or silently removed checks. Their generators/callers were not
patched as part of literal replacement/formatting. A normal producer build is
run separately and completed successfully in
`.tmp/fraction-v386-producer-build.log`. The frozen producer is
`.tmp/new-java2rust-numbers-default-v386-literal-text-debug`; no full V386
Fraction acceptance is claimed.
Its SHA-256 is
`e83d4fdb239fdf4112581a39c6bd3c4a9fc8d7f1d618b744e72f2ce524b8c5db`.

### Next canonical mapped-call boundary audit

A small independent diagnostic source,
`.tmp/MappedCanonicalFieldsAudit.java`, reproduces the production twelve-error
family with the unchanged V385 binary. Its generic Product interface,
static-reachable canonical immutable implementor, return-this method and
mapped BigInteger fields produce method-style numeric calls with owned
optional operands, e.g. `self.numerator...multiply(value.numerator)`, rather
than the declared mapped free function with a checked borrowed operand.
No generated code was edited. Output is
`.tmp/fraction-v387-canonical-fields-audit.rs`; resolved Java IR is in
`.tmp/fraction-v387-canonical-fields-resolved.log`. The numeric calls are
unresolved in Java IR, so the earlier hypothesis about a resolved external
method rename is not the demonstrated cause of this example.

The authoritative planner tries facade receiver ABI before mapped-crate
receiver ABI. Verify which inferred facade fact is admitted for these calls,
then ensure an explicit mapped pack's operation retains its own selected
provider/argument contract. Promote the canonical source pattern into the
executable field regression. This audit confirms the framework already can
express the required free-function/nullable-borrow rewrites; no runtime
Borrow<Option<T>> or representation redesign is justified.

### V387 actual manifest-ingestion provider repair

The decisive difference is the CLI ingestion path, not canonical storage
itself. Direct `parse_external_pack_manifest` plus translation passes this
numeric example, including when the Java caller is kept outside translation.
The CLI additionally runs `enrich_external_pack_facade_abi`, parses runtime
inherent methods and publishes a method ABI for the same Rust name as an
explicit pack free-function mapping. For BigInteger.multiply the inherent
parameter is `impl Borrow<Self>` while the actual exported free function
requires `&BigInteger`; the inferred method ABI therefore bypasses the pack's
receiver-first invocation and checked borrowed argument. CLI behavior is the
corpus entry point, so the independent regression now invokes the real CLI
with the production manifest. Its Java caller is compiled independently and
only `CanonicalMappedFieldsProbe.java` is translated. Its first direct Rust
caller used reference expectations for owned getter results; the diagnostic
fixture now exposes fields and the caller observes those stored values
directly, retaining the seven arithmetic/reuse/identity guards.

`.tmp/fraction-v387-canonical-cli-baseline.log` is terminal red after Java
passes: two actual `Option<BigInteger>: Borrow<BigInteger>` errors in library
methods. Unlike the earlier API-only attempts, this reproduces the measured
production failure. `MappedNumericFieldsProbe` also retains all original null
and getter checks and adds canonical/static and generic-interface controls;
its baseline remains green with fourteen guards.

Repair the existing Rust ABI parser to inventory root public free functions
separately from inherent methods, preserving scope/literal masking. When an
explicit mapped receiver-first function exists with matching Java arity plus
one receiver, leave its declared pack contract authoritative instead of
inferring an ABI from a same-named inherent method. Keep existing inherent-only
and constructor enrichment. Add a CLI ingestion unit control with both a
generic inherent method and borrowed free-function adapter, and extend the
existing parser scope control. No planner precedence change or numeric
runtime widening is made. CLI units and the executable CLI regression are
terminal green in `.tmp/fraction-v387-cli-units.log` (**25/25**) and
`.tmp/fraction-v387-canonical-cli-r1.log` (**1/1**, **18.48 seconds**), with
the Java oracle and all seven Rust arithmetic/reuse/identity guards. The
modified nullable-field/source-generic control remains green in
`.tmp/fraction-v387-canonical-fields-baseline.log` (**1/1**, fourteen guards).
The direct-API canonical control also passes independently, but only the
CLI regression is evidence for the ingestion repair. No planner or runtime
change was required. The library source is unchanged from V386's passing
1723-test gate; the new product changes are in the CLI, covered by all 25
binary unit tests and the executable generated-code regression.

Normal producer build `.tmp/fraction-v387-producer-build.log` succeeds.
Frozen `.tmp/new-java2rust-numbers-default-v387-provider-abi-debug` has SHA-256
`63a06ecb1eb91cde654329c4312f1b14df0e956d314ab4f203c2299994216f64`.
A fresh full closure run is active with root
`.tmp/commons-numbers-1.3-fraction-v387-provider-abi`, untouched stage
`.tmp/numbers-fraction-v387-provider-abi-staged`, log
`.tmp/numbers-fraction-v387-provider-abi-run.log`. The last measured complete
count remains V385's 27 until this run reaches a terminal result.

V387 is now terminal (exit 1): all **17** files generate and parse, with zero
unsupported statements/members/expressions, stubbed constants and invalid
patterns. Rust errors fall **27 to 11**: four BigInteger append contracts,
three UTF-16 integer parse calls, and four immutable-field moves. All twelve
optional-operand errors disappear. Total **243657 ms**: translation
**213168 ms**, assembly **25527 ms**, check **2057 ms**. The stateful Supplier
behavior remains independently open; this compile count is not acceptance.

### V388 UTF-16 integral parsing contract audit

The measured V385 Fraction.parse output has three `.parse::<i32>()` calls
on JavaUtf16String (after trim/substring), which cannot compile. The legacy
Rust scalar parser also has different accepted digits and exception messages.
Minimal correct rewriting passes an optional borrowed logical String plus
radix and selected Java width to one UTF-16 integer parser, then applies the
existing Java exception policy and primitive result cast. Keep null until the
parser so NumberFormatException, including null-before-invalid-radix ordering,
is preserved. This is a local numeric/text runtime and call-boundary repair;
it does not require changing String or numeric representations.

The new independent `IntegralParseProbe.java` passes installed Java (zero),
with thirty guards covering signs, leading zeros, decimal/fullwidth/Arabic
digits, fullwidth radix letters, int/long extrema, byte/short ranges, radix
boundaries, trim at the caller, no implicit whitespace trimming, raw surrogate
error text, null and radix precedence, failure then reuse. The shared parser
uses the same BMP decimal digit leaf as BigInteger/BigDecimal, not a copied
table or lossy scalar conversion. Byte/Short parse through the Integer range
before their narrower range check, matching their distinct diagnostics.
The generated runtime includes this parser only with the logical UTF-16 String
support; the legacy scalar boundary remains separately selected. All native
parse overloads use one optional borrowed String formal for this boundary.
Focused verification `.tmp/fraction-v388-integral-parse-r1.log` is terminal
green: **1/1**, **14.29 seconds**, all thirty guards. The caller additionally
checks directly typed null (two more guards) in the batched gate, now terminal
green. `.tmp/fraction-v388-library-numeric.log` proves **1723/1723** library
(**75.12 seconds**), **25/25** CLI and **6/6** native numeric integrations
(**18.03 seconds**). `.tmp/fraction-v388-mapped-preservation.log` proves all
**5/5** mapped controls (**32.69 seconds**), including the actual CLI/provider
boundary. The immutable-interface and known-red stateful Supplier controls
are outside this focused numeric filter, not waived for acceptance.
Normal producer build `.tmp/fraction-v388-producer-build.log` succeeds.
Frozen `.tmp/new-java2rust-numbers-default-v388-integral-text-debug` has SHA-256
`5761a6aee5f72ddbde36c021dfe59545ba71c2ed3c2a8eb858967970af2428e4`.
No complete V388 count is claimed; V387's terminal full-closure result remains
eleven errors. Batch the mapped concat and field-move repairs before another
complete corpus cycle.

### Remaining mapped text/field producer audit after V387

The four append diagnostics are actual BigFraction numerator/denominator
concatenations. `lower_lossless_append_text_value` routes object segments
through `lower_string_value_of_arg_for_display`, which already has
`lower_required_mapped_object_string`. That helper explicitly rejects every
optional physical carrier, even when the logical field is non-null. The
fallback consequently passes Option<BigInteger> to JavaAppendValue without a
mapped text adapter. Extend the existing mapped stringification boundary to
nullable stored values: observe the input once, preserve null as literal
"null", and use the selected mapped toString contract only in the present
branch. Preserve evaluation/conversion order and retained field ownership;
do not add corpus-specific BigInteger append implementations or lossy Display
conversion. A focused field/nullable/valueOf/concat caller should prove that
shared route before a new whole-corpus measurement.

The other four remaining V387 errors move Option<BigInteger> fields out of
borrowed payloads or Arc handles into a source constructor's owned optional
formal. Required-payload field arguments already have checked clone recovery;
the optional owned case must use the same capability/physical-slot authority
without relying on a later-read heuristic (Java retains its receiver even on
the last syntactic field use). A copy-with-one-field-changed control should
cover both present and absent retained fields, and direct and canonical
receiver paths. These are producer/constructor-argument ownership contracts,
not evidence that the representation framework cannot express Fraction.

### V389 nullable mapped text and constructor field recovery

Extend the existing independent MappedNumericFieldsProbe with copy-with-one-
field-changed methods, both nullable fields, mapped text concatenation and
String.valueOf, preserving the previous fourteen guards. Its new baseline
`.tmp/fraction-v389-text-fields-baseline.log` passes Java and fails generated
Rust with seven errors: two missing BigInteger append capabilities, one
Option<BigInteger> Rust Display call, and four illegal owned field moves.
The caller now has twenty-one guards including absent-field copying and
retained originals.

The precise ownership gap is at final invocation reconciliation. Ordinary
calls already invoke `reconcile_owned_self_field_arguments`; constructor
argument lowering does not. Earlier fast paths deliberately exclude nullable
targets and then use liveness suitable for locals, which cannot authorize
moving an owned field out of a borrowed Java receiver. Reuse the same final
field-recovery function for constructor arguments, factor their shared plan
shape extraction, and consult the existing optional element recovery
capability as well as the direct payload capability. Existing recovery
markers prevent duplicate cloning. This is a shared ABI-boundary correction,
not a BigInteger field pattern.

Mapped text now extends the existing required mapped toString helper to a
borrowed nullable match. The input is observed once; the absent branch emits
literal "null" and the present branch reuses the selected provider's ordinary
toString lowering (including return encoding and exception policy). Physical
Option<&T> payloads are not borrowed a second time. No foreign JavaAppendValue
implementation or Rust Display conversion is introduced. Preserve propagated
result evidence through the conditional, and preserve nullable text results
when the declared toString contract permits them. Focused verification is
terminal green in `.tmp/fraction-v389-text-fields-r1.log`: **1/1**,
**31.40 seconds**, retaining all twenty-one guards. A full library plus value
identity and mapped text gate is running in
`.tmp/fraction-v389-library-identity-text.log`, followed by all five mapped
controls in `.tmp/fraction-v389-mapped-preservation.log` and a normal producer
build. These gates are now terminal green: **1723/1723** library
(**75.76 seconds**), **3/3** mapped text (**12.46 seconds**), **5/5** value
identity (**15.14 seconds**) and **5/5** mapped controls. The normal producer
build succeeds. Earlier byte/short/int/long parsing and CLI ingestion evidence
remains valid; neither kernel nor ingestion code changed in this batch. No
new complete Fraction count is claimed yet.

The mapped-control gate took **34.68 seconds**. Frozen producer
`.tmp/new-java2rust-numbers-default-v389-mapped-text-fields-debug` has SHA-256
`b3125429bcd7b3d0478cca1b827da0196ed4d49b2828aacd844aa91a074edd99`.
A fresh full closure run is active at
`.tmp/commons-numbers-1.3-fraction-v389-mapped-text-fields`, with untouched
staging `.tmp/numbers-fraction-v389-mapped-text-fields-staged` and log
`.tmp/numbers-fraction-v389-mapped-text-fields-run.log`.

### V389 complete-closure measurement

V389 full closure is terminal (exit 1): all seventeen files generate and
parse with zero unsupported statements/members/expressions, invalid patterns
and stubbed constants. Compiler errors fall **11 to 1**. The sole error is
an assembly dependency in the new integral parser: the flat runtime support
collector preserves java_integral_parse_text but drops its nested
java_numeric_digits module. All prior mapped text and field-move diagnostics
disappear. Total **260287 ms**: translation **230060 ms**, assembly
**25304 ms**, check **2052 ms**. No compilation/behavior acceptance yet.

### V390 shared digit runtime assembly repair

Publish the already shared digit leaf as a uniquely named flat runtime kernel,
`java_numeric_decimal_digit`, matching other emitted runtime helpers. Both
BigInteger/BigDecimal continue importing this same function from their private
numeric_text module; the generated String runtime includes its declaration
directly, without an unnecessary namespace wrapper that the flat collector
does not retain. No digit table or parsing algorithm is duplicated. Add an
independent caller through the actual production crate assembler, not only
the single-module test compiler, retaining all thirty-two parse guards. Its
focused gate is `.tmp/fraction-v390-parser-assembly.log`.

That first thin assembly gate exposed a second general defect: shared-trait
macro implementations were promoted only when the macro appeared in multiple
input files. A single-source library lost its primitive display implementations
after prelude stripping. Remove that source-count restriction while retaining
the shared-dependency proof and existing macro-family duplicate checks.
`shared_merger_preserves_single_source_macro_impls_after_prelude_stripping`
compiles and runs a merger-produced single-source library; the independent
integral parser caller runs all thirty-two guards through production assembly.
Terminal verification: **15/15** support closure controls
(`.tmp/fraction-v390-support-closure.log`), **7/7** native numeric controls
(`.tmp/fraction-v390-native-numeric.log`, **26.37 seconds**), **13/13** runtime
library controls (`.tmp/fraction-v390-java-math.log`). Normal build succeeds.
Frozen producer `.tmp/new-java2rust-numbers-default-v390-flat-digit-runtime-debug`
SHA-256 `ff80b4245b3830a42a3f02f6b143968d46e17abb688b91570fb16445f7606c11`.
Fresh full Fraction closure is running in
`.tmp/commons-numbers-1.3-fraction-v390-flat-digit`, staged at
`.tmp/numbers-fraction-v390-flat-digit-staged`, log
`.tmp/numbers-fraction-v390-flat-digit-run.log`. No behavioral acceptance claimed.

The complete V390 closure is terminal **exit 0**: all **17/17** files
generate and parse, **0** compiler errors, **0** unsupported
statements/members/expressions, invalid patterns and stubbed constants.
Total **247874 ms**, translation **215867 ms**, assembly **26751 ms**,
check **2368 ms**. This is compilation evidence only: the known reachable
default Supplier body still prevents behavioral acceptance despite a zero
no-op call-site scan.

### V391 anonymous primitive initialization prerequisite

Both source-class and source-interface anonymous object emitters rejected a
primitive instance field without an initializer. Java initializes these slots
before construction; the minimal rewrite is the existing planned slot default
inside the same Cell/value storage used for explicitly initialized fields.
Admit primitive defaults in both emitters and reuse
slot_render::default_expr_for_slot_with_facts; do not add a second zero table.
Uninitialized reference slots remain outside this change because their null
storage contract requires a separate producer/consumer audit. New independent
Java/generated-Rust control AnonymousPrimitiveDefaultsProbe covers all eight
primitive defaults, interface and source-base objects, aliases, repeated state,
and independent allocations. This is a Supplier prerequisite, not its fix.
Focused gate: `.tmp/fraction-v391-anonymous-defaults.log`.

The initial focused command raced a test-file edit and failed to parse the
test harness; corrected before the rerun. The first executable run
`.tmp/fraction-v391-anonymous-defaults-r1.log` passed javac but exposed a
preexisting source-class adapter limitation: returning a compound boolean
expression is rejected by its arithmetic-only return-expression renderer,
then construction falls through to a missing abstract-base constructor.
The emitted interface adapter already had all eight correct defaults.
Keep this failing generated artifact as diagnostic evidence at
`.tmp/fraction-arrays-test-tmp/new_java2rust_anonymous_primitive_defaults_1792765293971112558`.
For the initialization control, observe the source-class defaults through
ordinary primitive getters and compare in the independent caller; all eight
defaults and alias/repeated-state assertions remain required. No boolean
return-renderer fix or acceptance of that failure family is claimed.
Both semantic admission checks now share the same FieldDecl predicate as
both emitters, rather than retaining initializer-only admission rules.
Current gate: `.tmp/fraction-v391-anonymous-defaults-r2.log`.

R2 reached the actual anonymous source-class adapter with all eight correct
defaults, then exposed a distinct constructor policy gap: implicit default
constructors were rejected for every abstract class, including a base payload
that an anonymous subclass must initialize. Admit the existing implicit
constructor emitter when the authoritative representation plan requires an
anonymous-class virtual carrier. Do not manufacture a Java abstract instance
or add a corpus-specific constructor. The r3 independent Java/generated-Rust
control is now terminal green **1/1**, **13.16 seconds**, retaining all eight
primitive observations on both projections and seven state/default guards.
`.tmp/fraction-v391-anonymous-defaults-r3.log` records this result.

The existing anonymous-field controls pass **2/2** in
`.tmp/fraction-v391-anonymous-preservation.log`. That command stopped at the
anonymous-display suite, whose generated code mixes JavaUtf16String and
String in the anonymous carrier and lacks a carrier display protocol. Check
this separately against the frozen V390 producer before attributing it to
this primitive-initialization batch; do not weaken or remove that suite.
The storage preservation and library gates continue separately in
`.tmp/fraction-v391-anonymous-storage.log` and
`.tmp/fraction-v391-library.log`.

Storage preservation is terminal green **1/1**, **11.04 seconds**. Frozen
V390 CLI output for the exact anonymous-display source reproduces all three
product compiler diagnostics (UTF16/String return-arm mismatch and two
missing JavaArraysDeepToStringElement implementations) without any V391
changes. Baseline source/output/check log:
`.tmp/AnonymousObjectDisplayV390Audit.java`,
`.tmp/fraction-v390-anonymous-display-baseline.rs`,
`.tmp/fraction-v390-anonymous-display-baseline.log` (rustc exit 1).
Thus the display regression remains open and unchanged; no all-green claim
is made for that suite. No new complete corpus run is needed for this
initialization prerequisite before the larger Supplier projection batch.

V391 full library verification is terminal green **1723/1723**,
**74.81 seconds**, in `.tmp/fraction-v391-library.log`. Diff whitespace and
Rust formatting checks pass. The native Supplier state/outer-dispatch
regression is still required and has not been fixed by these prerequisites.
Normal build succeeds. Frozen V391 producer
`.tmp/new-java2rust-numbers-default-v391-anonymous-defaults-debug`, SHA-256
`6b656ec638420799530156914249165f91b90f427928666a0571d4163fa07dc6`.

### V392 native anonymous SAM state projection

Generalize the existing object-backed anonymous interface emitter to native
SAM carriers. A local signature view derived from the authoritative SAM
descriptor supplies parameters/results; it is never registered as a source
type or emitted as a trait. Source and native projections use the same
field/capture storage, binder-aware field normalization and statement body
renderer. Native methods expose the existing fallible callback contract and
the carrier retains one Rc-backed anonymous object through from_fallible_fn.
Bind anonymous fields before body admission so bare increments are receiver
updates, retaining lexical local shadowing. Conversion to an owned, borrowed
or nullable destination uses the existing slot conversion plan.

Independent Java/generated-Rust StatefulNativeSamProbe covers IntSupplier,
Supplier<Term> and IntBinaryOperator: default state, captured scalar input,
multiple inputs, alias identity, separate allocations, retained result
objects, repeated calls, and mutation-before-exception ordering. Terminal
green **1/1**, all **9 guards**, **13.02 seconds** in
`.tmp/fraction-v392-native-anonymous.log`. This does not yet prove enclosing
virtual receiver capture or public abstract extension behavior.

### V393 implicit anonymous enclosing receivers

Add SourceSymbols::anonymous_enclosing_call_receiver as the shared proof for
capture analysis, capture-field discovery and call emission. It first uses
the existing scoped overload resolver for anonymous implementations/helpers,
then checks inherited base members, excludes inherited Object operations,
and proves the selected instance owner reachable from the nearest typed
enclosing receiver. Reuse the resolver's existing SourceSymbols and external
facts during capture collection; do not rebuild an index or snapshot outer
field values. Anonymous receiver scopes are stacked explicitly and suspended
inside named local classes. Scoped signatures omit bodies in the stored
selection view. Emission rewrites proven receiverless calls to qualified
Outer.this, then uses ordinary outer-handle and virtual-call lowering.

The new resolver control proves a generic Holder<T> capture through an
inherited Parent method, rejects captures for a widening anonymous helper,
an inherited anonymous-base method and anonymous Object.hashCode. Terminal
green **1/1**, **0.01 seconds** in
`.tmp/fraction-v393-implicit-captures.log`. The required complete stateful
outer Supplier control is running in `.tmp/fraction-v393-outer-supplier.log`;
no pass is claimed yet.

V393 outer Supplier is terminal red (**10 compiler errors**, **13.15 seconds**).
Its persistent state and requested outer field are emitted; the failures
identify three whole-path gaps: optimized slot calls bypass expression-level
receiver rebinding, inherited factories do not classify the declaration's
captured this as the concrete receiver, and declaration-only abstract pruning
omits a dynamic carrier referenced by hidden capture storage. Generated
evidence: `.tmp/fraction-arrays-test-tmp/new_java2rust_stateful_outer_supplier_1792766574527905519`.

### V394 canonical receiver and hidden value contracts

Normalize proven enclosing calls before optimized slot lowering as well as
ordinary expression lowering. Inherited companion admission recognizes the
capture origin's own receiver as the current concrete receiver, retaining
the distinction from a different lexical enclosing object. Abstract-owner
admission now feeds capture types through the same ValueTypeUses collector
as explicit fields/locals: a typed capture is real storage demand.

The outer Supplier gate falls **10 to 3 compiler errors** in
`.tmp/fraction-v394-outer-supplier.log`, **14.25 seconds**. Concrete factory
callbacks now call the retained canonical object. Remaining errors are all
raw abstract owner/carrier mismatches: a base factory supplies Rc<Base> to
an environment expecting BaseValue, and abstract mutator companion stubs
try to mutate through immutable Rc<Base>. Evidence:
`.tmp/fraction-arrays-test-tmp/new_java2rust_stateful_outer_supplier_1792766905174246021`.
The minimal correct rewrite has no Base runtime variant: Java cannot
instantiate that abstract class; the carrier dispatches concrete variants.

### V395 abstract layout versus dynamic runtime instances

Record abstract source declarations once in the representation plan. Exclude
them from concrete hierarchy variants while still walking through abstract
intermediate parents to reach concrete descendants. Preserve the abstract
base's layout, constructors and statically bound construction helpers; its
runtime carrier still dispatches full methods materialized on concrete
descendants. Factor shared dispatch item emission so this construction path
and ordinary classes use the same carrier/virtual-trait emitter. Raw abstract
instances do not acquire fabricated callable stubs or object protocols.
This is a representation/emission contract batch, not a manual callback or
continued-fraction rewrite. Focused gate:
`.tmp/fraction-v395-outer-supplier.log` (running).

V395 is terminal green **1/1**, all **9 original guards**, **13.94 seconds**.
Java and generated Rust retain increment-before-throw, call order, external
owner mutation, captured scalar, Supplier aliases/independent state and prior
result values. No generated code or pinned source was repaired manually.

### V396 dynamic-base and abstract-intermediate strengthening

Strengthen StatefulOuterSupplierProbe: call the factory through a Series
parameter, add an abstract Middle between Series and Concrete, and add a
second concrete implementation with different coefficients. Retain every
original guard and add two assertions for alternative virtual dispatch and
retained result/identity behavior (**11 guards** total). Declaration-only
abstract roots retain their carrier dispatch items through the same factored
emitter, even when only the raw layout/static namespace is otherwise needed.
This closes the intermediate-parent conversion inventory as well as the
direct captured root. The complete nine-test Fraction lowering control suite
is running in `.tmp/fraction-v396-lowering-controls.log`, followed by all
library tests in `.tmp/fraction-v396-library.log`.

V396 is terminal **7/9** controls green (**35.51 seconds**), with two
unwaived failures. The stronger dynamic control exposes read-only Alternative
implementations forced to &mut self by the override group's ABI while their
canonical storage is immutable Rc<T>; a thin companion incorrectly delegates
to that &mut method. The primitive-default control exposes a missing Rust
Display dependency for an abstract base composed by an anonymous adapter.

### V397 companion borrowing follows physical storage

Generalize the existing shared-guard delegation proof to take the emitted
receiver ABI. An immutable Rc/Arc payload cannot delegate to &mut T; retain
the already-lowered companion body in that case. Both named and inherited
companion producers use the same proof. Keep the existing fallback display
support when an anonymous adapter composes an abstract base payload; no new
display algorithm or default string is introduced, and the previously
recorded UTF16 anonymous-display suite remains separately open.

Terminal green: **9/9** Fraction lowering controls (**35.65 seconds**) in
`.tmp/fraction-v397-lowering-controls.log`, including the strengthened
**11-guard** outer-Supplier control; **1724/1724** library tests
(**85.84 seconds**) in `.tmp/fraction-v397-library.log`. Broader SAM,
capture, nested-interface and anonymous storage preservation is running in
`.tmp/fraction-v397-sam-preservation.log`. Normal build succeeds; frozen
producer `.tmp/new-java2rust-numbers-default-v397-native-anonymous-debug`,
SHA-256 `d6af03f98ea4143a1b1dcd6ecf6832a6976bcff2fbf05320e37039224906b42d`.
The fresh full seventeen-file Fraction closure is running at
`.tmp/commons-numbers-1.3-fraction-v397-native-anonymous`, staged at
`.tmp/numbers-fraction-v397-native-anonymous-staged`, log
`.tmp/numbers-fraction-v397-native-anonymous-run.log`. Compiler/behavior
acceptance of the full module is not yet claimed.

V397 full closure is terminal **exit 0**: all **17/17** files generate and
parse, **0** compiler errors, unsupported statements/members/expressions,
invalid patterns and stubbed constants. Total **313627 ms**, translation
**280328 ms**, assembly **26471 ms**, check **2400 ms**. Library compilation
and regression gates ran concurrently; this is not an isolated speed
comparison. Inspecting production ContinuedFraction output confirms a
persistent Supplier state object, n Cell initialized zero, captured x and
canonical outer handle, and real getA/getB callback invocations replacing
the former Default::default Supplier. Public module behavior/overload coverage
is still incomplete.

Broader V397 preservation is terminal green: **2/2** anonymous field/helper
controls (**17.94 seconds**), **1/1** anonymous storage (**17.91 seconds**),
**5/5** capture controls (**0.30 seconds**), **5/5** native SAM controls
(**23.04 seconds**) and **2/2** nested interface identity (**11.65 seconds**).

### V398 lexical order of nested receiver captures

Review finds deterministic sorted capture-field order is not lexical receiver
precedence. Share the actual enclosing-instance chain between field discovery
and call rebinding; filter that ordered chain to the retained fields rather
than treating sorted field names as Java scope order. Extend the resolver
control with ZOuter.AInner, where both enclosing receivers inherit the same
Parent.read method and the anonymous body also explicitly uses ZOuter.this.
Both typed captures must be retained and the implicit call binds the nearest
one. The first gate's behavioral assertions pass but an owner-entry count
assumption fails: existing inner-class capture collection visits this nested
anonymous owner twice. Replace that incidental count with an exact set of
five required callable origins; keep every per-owner capture assertion.
No owner-deduplication change is claimed. Current gates:
`.tmp/fraction-v398-lexical-captures-r1.log` and
`.tmp/fraction-v398-outer-supplier-r1.log`.

V398 focused verification is terminal green: lexical capture control **1/1**
(**0.19 seconds**), strengthened stateful outer Supplier **1/1**
(**16.08 seconds**). The V397 seventeen-file evidence remains the latest full
production run; V398 changes precedence only when multiple lexical enclosing
receivers are present. Do not attribute a fresh full run to V398.

### Mapped numeric identity entry audit

The next acceptance audit follows the actual BigFraction input-field-getter
path. Upstream getNumerator returns the stored BigInteger reference; the
generated getter currently returns self.numerator.as_ref().cloned(), while
the provider's derived BigInteger Clone deep-copies BigInt. A standalone
audit with a final mapped field and getter is Java-green (exit 0, stdout 0)
but V397 CLI output fails Rust compilation: a borrowed optional getter is
borrowed twice in reference comparison (Option<&&BigInteger> versus
Option<&BigInteger>). Independently, two distinct equal BigIntegers are
lowered with Rust value equality for Java ==, visibly contradicting Java
reference identity. Removing only the extra as_ref cannot fix that semantic
contract. Exact evidence lives in `.tmp/fraction-mapped-identity-audit`
(source, frozen generated output, independent Rust caller and logs; cargo
exit 101). No generated correction is applied.

Preserve the audit as the required five-guard MappedNumericIdentityProbe
and executable mapped_numeric_references_preserve_identity_across_fields_and_returns
regression in tests/fraction_lowering_contracts.rs. Its baseline gate is
`.tmp/fraction-v398-mapped-identity-baseline.log`. The previous nine controls
remain green evidence; no ten-control all-green claim is made.

Minimal correct contract: immutable mapped references need a clone-stable
allocation/token distinct from numeric value equality. Inputs, retained fields,
getters and nullable borrowed views must retain that token; independent equal
constructor results must retain distinct identities. Follow existing
JavaStringIdentity<P>/JavaUtf16String and generated JavaReferenceIdentity
mechanisms before adding a new representation. The string identity leaf
already has a generic Arc payload and separate value/identity operations;
consider generalizing that leaf. External packs currently expose numeric
Clone but no mapped reference-identity provider evidence. A general provider
contract plus shared reference-comparison/optional-borrow lowering is needed,
not BigInteger-named comparisons or Rust PartialEq for Java ==. This is a
runtime/manifest/producer-storage-signature-consumer batch; no identity fix
or whole-module behavioral acceptance is claimed yet.

The maintained identity baseline is terminal red (**16.94 seconds**, exit
101): Java passes all five guards, Rust fails the same extra-borrow E0308.
Current producer builds and formatting/whitespace checks pass. Frozen V398
`.tmp/new-java2rust-numbers-default-v398-lexical-receivers-debug`, SHA-256
`e06213eb555060c426d98c241008024bbf2dd6e3995465b56bf849aa5903eb73`.
All owned V392–V398 test/build/full-closure handles are terminal and collected.
Next coherent batch is the immutable mapped-reference identity contract,
followed by complete independent public-API behavior coverage for Fraction.
Arrays has not entered baseline/fix work and remains in the original goal.

### V399 immutable mapped reference identity contract

Reuse the generic Arc-backed JavaStringIdentity<P> leaf through the public
JavaImmutableReference<P> alias. Clone retains the same allocation; payload
value equality/hash remain separate from java_identity_eq. BigInteger now
stores its BigInt in that shared leaf, so copying a Java reference is a cheap
handle clone rather than a deep numeric copy. Keep numeric algorithms and
Borrow adapters intact; explicit payload projections replace only backend
operator/owned-decimal extraction boundaries. Positive abs retains this;
valueOf's small canonical references are retained; String construction,
including zero, creates a fresh allocation.

Add an explicit external type mapping contract
reference-identity=RustMethod, requiring rust-type and a clone-stable carrier.
The method is a pure infallible &self/&Self identity comparison, separate
from PartialEq/equals. Select it with the same provenance/alias rules as
the type mapping, including a user override that removes the contract.
Parsed source owners retain control. Generalize the existing optional String
comparison snapshot/match machinery for these mapped carriers, including
direct operands and concrete Clone UFCS to handle Option<&T> getters without
introducing Option<&&T> comparisons. Retain the left allocation while
evaluating the right; evaluate both sides when the left is null.

The original five-guard identity regression is terminal green **1/1**,
**18.17 seconds**, `.tmp/fraction-v399-mapped-identity.log`. Runtime numeric
controls pass **13/13** in `.tmp/fraction-v399-runtime-identity.log`.
No generated code was edited. This is a runtime/manifest/storage/return/
operator contract batch; no type-name-specific equality policy is added.

### V400 identity boundary and preservation gates

Strengthen MappedNumericIdentityProbe from five to **twelve guards** without
removing prior observations: absent fields, both comparison orientations,
distinct equal values through optional getters, right-side evaluation after
a null left operand, fresh zero constructors versus the canonical ZERO,
valueOf/ONE identity and positive abs identity. Add runtime allocation/value
separation coverage and manifest validation/provenance controls.
Runtime verification is terminal green **14/14** in
`.tmp/fraction-v400-runtime.log`. Complete Fraction lowering controls are
running in `.tmp/fraction-v400-lowering-controls.log`, followed by external
pack/semantic fact, String assembly/lossless behavior, BigInteger and
BigDecimal preservation in `.tmp/fraction-v400-identity-preservation.log`.
No new complete production closure or all-public-API pass is claimed yet.

### V401–V403 mapped identity verification and provider reuse

The twelve-guard probe exposed a physical-carrier mismatch: equality's fallback
replaced a proven non-null local's emitted `T` shape with the callable-wide
nullable plan after that local was passed to an optional parameter. Retain the
binding-aware non-null local shape at this boundary. Initializer shape reuse
also now requires equal optionality. V402 passes all **1,724** library tests
(`.tmp/fraction-v402-library.log`) and all **10** lowering controls
(`.tmp/fraction-v402-lowering-controls.log`).

Reuse the same immutable allocation leaf and explicit manifest identity method
for BigDecimal, including retaining the receiver for unchanged setScale. The
mapped numeric probe now has fifteen guards, including decimal field/getter
identity, distinct equal constructions, value equality, and unchanged scale.
V403 runtime controls pass **14/14** (`.tmp/fraction-v403-runtime.log`).

Broader V402 preservation passes BigDecimal **5/5**, BigInteger **7/7**,
JavaText **2/2**, and lossless String **5/5**. External-pack tests report
**184/198** and semantic-facts tests **171/184**, so neither full suite is
claimed green. The frozen V398/current CLI comparison reproduces identical
source bodies for six directly comparable external-pack failures; see
`.tmp/fraction-v402-broad-baselines/summary.tsv`. This comparison excludes
runtime preludes and cannot classify custom-fact or analysis-only failures.
The remaining failures are unclassified, not waived.

An independent public GCF caller now covers all six value overloads,
Coefficient factory/getters, callback counts, raw floating-point result bits,
and exception type/hierarchy/message observations. Java and generated Rust
match exactly on **220 rows** in `.tmp/fraction-v402-gcf-public-r2`.
The maintained runner is `scripts/verify-commons-numbers-fraction-gcf.sh`;
the Java oracle is never translated. This used the intact V397 generated
production closure with the current numeric runtime, not a fresh V402 full
translation. Fraction/BigFraction public coverage and ContinuedFraction's
abstract extension contract remain acceptance work.

V403 also passes all **10/10** focused lowering controls, including the full
fifteen-guard mapped identity probe (`.tmp/fraction-v403-lowering-controls.log`).
BigDecimal **5/5** and BigInteger **7/7** preservation remain green in
`.tmp/fraction-v403-math-preservation.log`, including independent JDK division
requests. Frozen V403 producer SHA256 is
`8e16748845c29109877ab0b9a6124e5ecdd1be3fb58fd33d224c2ef1b180606f`.
Its fresh seventeen-file translation is running in
`.tmp/commons-numbers-1.3-fraction-v403-mapped-identities`.

### V404 executable Fraction API audit: integral signum boundary

Add independent javac-only `FractionOracle.java` and Rust consumer
`fraction_oracle.rs`; invoke the existing behavior runner with `fraction`.
The Java side produces **1,114 rows**, covering construction, constants,
all unary/arithmetic overloads, power, compare/equals/hash, parsing, numeric
conversions, text, result identity, self-aliasing and unchanged inputs.
This is API coverage preparation, not a successful acceptance claim.

The intact V397 generated production library aborts on its first observation:
`Integer.signum is not modelled` (runtime facade in generated lib.rs:2690).
Evidence is `.tmp/fraction-v403-fraction-public-baseline` and its runner log.
No generated-code edit was made. The minimal correct rewrite evaluates the
operand once, performs Java widening/checked unboxing to the declared int or
long parameter, invokes signed integer signum, and returns int. This is a
local native emission/argument contract fix; no Fraction-specific handling
or representation redesign is required.

Add IntegralSignum to the existing typed native numeric plan, primitive
argument shaping, return inference and expression projection, with existing
source-owner shadowing rules. Integer and Long share this implementation.
Native numeric controls pass **7/7**, including static import, MIN/zero/MAX,
char/int widening, boxed operands/null errors, single evaluation, source
shadowing and production crate assembly in
`.tmp/fraction-v404-native-numeric.log`. No fresh full V404 result yet.

### V404 full closure and independent Fraction behavior

V403 completes all seventeen files with parse/invalid/stub counts zero and
cargo check zero errors: **257,738 ms** total, **225,471 ms** translation,
**26,843 ms** assembly and **2,469 ms** cargo check. The independent GCF
caller matches all **220 rows** against this fresh closure in
`.tmp/fraction-v403-gcf-public`.

Frozen V404 producer SHA256 is
`92dcc77a4a2eb046565f7fdd8701546fd050fe07aa29de62f8e367027182d6d1`.
The fresh closure `.tmp/commons-numbers-1.3-fraction-v404-integral-signum`
also completes all seventeen files, with zero compile errors and zero invalid
patterns, in **290,538 ms**. All **1,724** library tests pass in **84.13 s**
(`.tmp/fraction-v404-library.log`).

The Fraction caller now additionally invokes all twelve NativeOperators
interface operations on aliased receivers. All **1,282 Java/Rust rows match
exactly** against V404 in `.tmp/fraction-v404-fraction-public`. This proves
the tested Fraction paths, not acceptance of the entire Fraction module:
BigFraction and the actual ContinuedFraction abstract extension still need
their independent behavior gates. Inherited Number defaults, null arguments,
and a final public-operation/declaration coverage audit remain explicit work.

### V405 BigFraction erased immutable cast audit

The new independent BigFraction callers cover all public factory and arithmetic
overloads, unary operations, getters, power, floating/integer conversions,
equals/compare/hash/text, decimal overloads/all eight rounding modes, parsing,
retained BigInteger references and unchanged state. The Java caller produces
**1,203 rows**. Two initial Rust caller mistakes (Arc's ToString resolution
and the provider's UTF16 constructor argument) were corrected in maintained
caller code, without changing production or generated code.

Against intact V403, the first **89 rows match exactly**, then equals on an
unequal BigFraction panics at generated BigFraction.rs:484. The instanceof
guard checks `Arc<BigFraction>` but its refined cast emits
`downcast_ref::<BigFraction>()` and a `&BigFraction` local. The correct minimal
rewrite downcasts to the actual shared allocation handle and clones that
handle into the local; consumers then keep the same object and ordinary
canonical dispatch. Producers already erase the shared handle correctly.
Evidence: `.tmp/fraction-v404-bigfraction-public-r1`.

The six-guard ImmutableErasedCastProbe reproduces this without numeric types:
local and static-reachable immutable objects, equal/distinct/self/null and
unrelated objects, repeated reads. Its baseline aborts with invalid Java cast
in `.tmp/fraction-v404-immutable-cast-baseline.log`. Both the refined local
shape and the cast runtime type wrongly limit owning handle recovery to
interior-mutable placements. Generalize these existing placement checks to
all implemented shared placements, retaining value-represented borrowing.
This is a shared cast/storage contract correction, not an equals algorithm
rewrite or a new representation. Focused verification is in progress;
no BigFraction acceptance result is claimed.

The completed correction passes all **11/11** focused lowering controls in
**35.83 s**, `.tmp/fraction-v405-r1-lowering-controls.log`. The preceding
in-flight V405 attempt still used the earlier local-shape revision and failed
the new cast test (ten existing controls passed); its failure is preserved in
`.tmp/fraction-v405-lowering-controls.log`. The successful run was rebuilt
after both shape and downcast projections were corrected.

Frozen V405 producer SHA256 is
`12c0523d03e7d21e2727fec58e2d8eafc399662ded9bd94b22b3be727c4dd3b4`.
The fresh complete closure is running in
`.tmp/commons-numbers-1.3-fraction-v405-immutable-erased-casts`; the independent
BigFraction caller must be rerun against this production result before
claiming that the original failure is resolved throughout the module.

V405's full seventeen-file closure is now terminal green: zero parse/invalid
patterns and zero cargo errors, **260,943 ms** total (**228,911 ms**
translation, **26,715 ms** assembly, **2,323 ms** cargo check). The generated
BigFraction equals now stores `Arc<BigFraction>` after downcasting to exactly
that carrier and cloning its handle, consistent with the instanceof guard.

All three independent public callers match exactly against this fresh V405
closure: GCF **220/220**, Fraction **1,282/1,282**, and BigFraction
**1,203/1,203** (**2,705 rows** total). Evidence roots are
`.tmp/fraction-v405-public-{gcf,fraction,bigfraction}` and corresponding logs.
The original BigFraction cast failure is resolved in production output,
without generated-code patches or numerical tolerances. These matrices still
do not replace the ContinuedFraction extension check, final declaration/error
coverage review or accepted-corpus preservation gates.

The focused erased-value preservation subset passes **75/75** in **10.05 s**
(`.tmp/fraction-v405-erased-preservation.log`). It supplements the eleven
executable lowering controls; the complete library pass above belongs to
V404 and is not reattributed to V405.

### Actual ContinuedFraction source-extension entry audit

Standalone production emission contains raw abstract getA/getB traps: those
are not an implementable external Rust subclass API. The intended whole-source
translation must include Java subclasses so the existing hierarchy planner can
build the concrete canonical carrier. Test this actual contract by translating
ContinuedFractionExtension.java alongside all seventeen untouched production
files. Its only algorithms are coefficient generation and observation; run
delegates through a ContinuedFraction-typed alias to upstream evaluate.
No manually rewritten continued-fraction evaluator is introduced.

The independent javac-only ContinuedFractionOracle observes both evaluate
overloads, small/non-small initial coefficients, convergence/errors, default
and explicit limits, ordered getA/getB callbacks, retained mutations after
exceptions, alias-visible factor changes, and repeated evaluation. Its
**396 Java rows** are in `.tmp/fraction-v405-continued-java.tsv`; the source
extension's counters/trace expose callback order and canonical owner identity.
The bounded coefficient budget prevents runaway default-limit experiments.

The module runner now accepts an explicit `NUMBERS_EXTRA_SOURCE` fixture,
copies it separately under staging/acceptance, verifies its bytes, and increases
the expected file count. The pinned production inventory and sources remain
unchanged. Baseline generation is running in
`.tmp/commons-numbers-1.3-fraction-v405-extension`, with the same frozen V405
producer and all **17 production + 1 extension** sources. Inspect its generated
callback/capture/hierarchy path and compile result before starting any new fix
family.

The full **17 production + 1 extension** baseline is terminal green:
zero cargo errors/invalid patterns, **270,326 ms** total, **237,353 ms**
translation and **2,240 ms** cargo check. The generated run method constructs
the existing ContinuedFractionValue carrier with the canonical Rc<RefCell>
subtype handle; inherited evaluate methods delegate through that same handle.
The source extension needed no new translator fix after the earlier shared
callback/capture/abstract-owner work.

The maintained independent Rust caller now matches all **396/396** Java rows
exactly in `.tmp/fraction-v405-public-continued`; the runner's `continued`
mode verifies all seventeen pinned sources plus the exact maintained extension
fixture before building either caller. The oracle Java itself is never
translated. Total independent parity at V405 is **3,101 rows** across the
three standalone production callers and the source-extension contract.

### Remaining acceptance work after V405

Neither module is accepted yet. Review the full declaration inventory against
the maintained callers, including relevant inherited Number defaults,
BigFraction interface dispatch, null argument contracts and uncovered public
error branches. Extend the focused boundary controls where evidence requires
it. Complete current-producer preservation for the six previously accepted
Numbers modules and Commons RNG under their existing numerical policies.
Only then accept Fraction and enter the complete Arrays baseline/rewrite audit.
Arrays remains entirely in scope; its prepared inventory is not translation
or behavioral evidence. All V403–V405 owned test/build/translation/behavior
handles are terminal and collected at this checkpoint.

### V406 public-boundary coverage and nullable callback audit

The inherited Fraction byteValue/shortValue implementations already exist
in the production output; the remaining gap was the independent caller.
Observe both in every Fraction result. Add all twelve NativeOperators
operations to BigFraction's independent caller. These additions pass against
the intact frozen V405 production output without translator changes.

Also cover explicit invalid approximation epsilon/iteration/denominator
arguments, Fraction constructor MIN_VALUE sign/reduction cases, and practical
overflow/convergence regressions present in upstream FractionTest. BigFraction
MIN_VALUE powers use only zero/one/minus-one, exercising that branch without
an enormous artificial numeric allocation. Expanded caller results are exact:
Fraction **1,560 rows**, BigFraction **1,318 rows**, in
`.tmp/fraction-v406-r1-public-{fraction,bigfraction}`. Previous observations
remain included; no tolerance or ignored case was added.

An explicit co-translated FractionNullBoundary source fixture calls public
reference parameters with typed null values. javac confirms all **23** selected
paths throw NullPointerException: parsing, BigInteger factories/arithmetic,
Fraction/BigFraction arithmetic/comparison, decimal rounding modes, and native
Supplier evaluation. The fixture checks exception class/control flow; it does
not require JVM-generated helpful-null diagnostic wording. Explicit stable
exception messages remain covered by the independent arithmetic/parsing callers.
Baseline evidence is `.tmp/fraction-v406-boundary-java.tsv`; complete-source
translation is in `.tmp/commons-numbers-1.3-fraction-v406-null-boundary`, using
the same frozen V405 producer with all seventeen production sources plus this
one fixture. No source or generated algorithm was patched.

Inspection already identifies a callback contract mismatch: the inline native
SAM adapter records its nullable physical Optional<Supplier<T>> as the Java
semantic type as well. Its synthesized get call is consequently selected as
Optional.get, returning Supplier<T> and NoSuchElementException, instead of
invoking Supplier.get to return T with a lazy NullPointerException. A correct
rewrite retains the optional physical carrier but binds the synthesized
receiver's declared Java Supplier type separately. Reuse ShapeEnv's existing
insert_local_shape_with_java_type contract, the native SAM descriptor, and
ordinary checked receiver/invocation lowering. Creation must remain lazy and
evaluate the argument once. This is a local adapter/type-environment correction,
not a representation redesign. The independent eleven-guard
NullableNativeSupplierProbe also retains genuine Java Optional behavior.
Await the complete baseline and focused regression result before concluding
this is the only new null-boundary family.

The full V406 baseline is terminal: **four E0308 diagnostics**, all the same
Supplier-versus-result mismatch (two boundary callers and two production
ContinuedFraction adapters whose formal contract became nullable). No other
compile family appears in the complete eighteen-source closure. The focused
probe also reveals three adjacent consumer contracts: nullable native facade
locals misclassified as bare closures, a fallible callback result projected
as a field before discharging Result, and genuine Optional<SourceType>.get
rejected because its source payload was mistaken for its method owner.

V407 batches these corrections: preserve the declared SAM Java type in the
synthetic environment; recognize the optional carrier's native facade;
consume fallible call receivers through the ordinary value slot before a raw
field projection; and honor an explicitly declared JDK Optional's method
owner before testing source ownership of its payload. Retain source-owner
shadowing checks and existing invocation/error kernels. The eleven probe
guards, complete Fraction lowering controls and native-SAM preservation suite
are being checked together, before another full source run.

V407 focused verification is terminal green: Fraction lowering **12/12**
in **35.74 s**, native SAM preservation **5/5** in **14.22 s**,
`.tmp/fraction-v407-callback-controls.log`. The original new probe's failures
are retained in `.tmp/fraction-v406-nullable-supplier-baseline.log`.
No guard was removed to make it pass.

Frozen V407 producer SHA256 is
`107084098675047b8258c086db709014798c113ff3fcff4a2d301a060638f3ed`.
Full null-boundary regeneration is running in
`.tmp/commons-numbers-1.3-fraction-v407-null-boundary`. The caller fixture
now also includes six comparisons through Java Comparable<Fraction> and
Comparable<BigFraction>, making that erased interface entry explicit.
The independent javac-only FractionBoundaryOracle produces **29 rows** in
`.tmp/fraction-v407-boundary-java.tsv`. Its Rust peer calls the separately
translated adapter; neither caller implements fraction arithmetic or comparison.
The behavior verifier's `boundary` mode checks the complete pinned production
closure plus exact maintained fixture before executing either caller.

The V407 complete library gate passes **1,724/1,724** in **77.34 s**
(`.tmp/fraction-v407-library.log`). Fresh same-producer preservation passes
all three Primes files and **3,594** exact independent observations, plus all
three Root Finder files and **122** exact independent observations. Evidence:
`.tmp/commons-numbers-1.3-{primes,rootfinder}-v407-preserved` and
`.tmp/numbers-{primes,rootfinder}-v407-behavior`. No fresh preservation claim
is made yet for the other four accepted Numbers modules or Commons RNG.

The full V407 boundary closure clears all four prior Supplier diagnostics.
Its two remaining E0308 diagnostics are in the newly added Comparable locals:
the interface storage is Box<dyn Any>, whereas factories produce canonical
shared Fraction handles. Merely adding Box::new would leave the compareTo
operation erased and is not a semantic solution.

The smallest correct rewrite is a typed Comparable<T> interface view that
retains the source object's canonical handle and forwards compareTo(T) to its
existing implementation, preserving identity, mutations and exceptions. The
existing object-backed native SAM adapter already performs this contract for
named implementors (including source identity tokens), and Comparable has one
abstract operation with input T and int result. Generalize the authoritative
native interface descriptor rather than adding Fraction-specific conversion
logic. Keep the existing JavaComparable/Ord support for generic type bounds.
Audit storage rendering, source-to-interface construction, invocation ABI,
source-owner precedence and generic-bound interaction together. The new
NativeComparableProbe covers mutable source state, alias identity, interface
parameter calls and the existing generic String comparison path.

V408 adds Comparable's descriptor (one T argument, int result) to the existing
native functional interface registry. Generic carrier arity, emitted callable
runtime, source implementor adaptation and original identity tokens all derive
from that descriptor. The probe's prior observable-alias rejection disappears;
its remaining resolver errors show that JDK String's Comparable<String>
supertype was never modeled. V409 projects fixed JDK Comparable<Self> arguments
through the existing invariant/wildcard matcher, retaining source-owner
precedence; it does not accept arbitrary Comparable type arguments. A focused
resolver control accepts Comparable<String> and rejects Comparable<Integer>
for a String argument. The first attempted unit insertion was accidentally
inside a Java fixture string, selected zero tests, and is not verification;
it was moved into the Rust test module and is being rerun by exact full name.

One independent Rust test helper name collided with the generated Value alias;
rename the helper to OracleExitCode without changing product code or guards.
Inspection of the now typed Comparable output also identifies two ordinary
native invocation boundaries: compareTo may already have the shared Rust
name compare_to, and reused source arguments need the same owned-reference
clone recovery as other calls. V410 accepts the authoritative snake-case
method spelling alongside the Java descriptor name and reuses
adapt_reused_owned_argument for native SAM inputs. The thirteen focused
controls and native SAM preservation are being rerun together. No V408–V410
full production or preservation pass is claimed yet.

The corrected exact-name generic-bound test passes **1/1** in
`.tmp/fraction-v410-comparable-bounds.log`, including the invariant negative
case. V410's twelve existing lowering controls also pass; the new Comparable
control still exposes emission/invocation integration gaps. V411 prevents
generic-bound trait promotion from replacing an authoritative native SAM
carrier, using EffectiveFacts (including user/source precedence), and keeps
declared Java receiver types separate from physical callback slots. This
removes the conflicting empty Comparable trait. The two remaining E0599
diagnostics both come from the generic-bound call shortcut emitting
compare_to on a native callable or facade. V412 restricts that shortcut to
actual type-parameter receivers, allowing concrete native interface values to
use ordinary descriptor-based invocation. Focused verification is pending;
no generated fixture was repaired and no assertion was removed.

V413's temporary diagnostic traces show that both failing calls bypassed the
ordinary native invocation and generic-bound helpers entirely. The slot-directed
external-call fast path used the known compareTo/int return summary to emit a
generic Rust method call before native SAM dispatch could run. V414 makes that
path defer to the authoritative native invocation contract when the declared
receiver/method match a native SAM. Remove the temporary trace code.
The unchanged Comparable regression is now terminal green **1/1**, **12.98 s**,
`.tmp/fraction-v414-comparable-control.log`: source identity, aliased mutable
state, interface parameter calls and generic String ordering all execute.
The complete library and thirteen-control/native-SAM batch is being checked
before fresh whole-corpus behavior claims. Frozen V407 preservation above is
not reattributed to V414.

V414 verification is terminal green: complete library **1,725/1,725** in
**81.77 s**, Fraction lowering **13/13**, native SAM preservation **5/5**
in **15.44 s**, with logs `.tmp/fraction-v414-library.log` and
`.tmp/fraction-v414-callback-controls.log`. A test-only extension adds
Comparable to the existing explicit source/user override precedence control;
that focused check is running separately. Production code is unchanged by
that test data extension.

Frozen V414 producer SHA256 is
`c74a40c442d38f26a149b275b9be2164543a68cf3913f8e500f43676d20136ff`.
Fresh independent whole-source runs are in progress at
`.tmp/commons-numbers-1.3-fraction-v414-{native-comparable,extension,null-boundary}`:
respectively all seventeen production files, production plus the actual
ContinuedFraction subclass, and production plus the null/Comparable caller.
These share one producer and run independently; no completed artifact is
overwritten. Their runtime and coverage ledgers remain pending.

`scripts/audit-commons-numbers-fraction-coverage.py` maps every public javap
declaration, compiler bridge, protected getA/getB contract and inherited
Fraction byteValue/shortValue operation to explicit independent request labels.
It requires nonempty, byte-identical Java/Rust outputs from all five callers
and rejects unmapped declarations or absent observations. It records operation
coverage, not inferred complete branch coverage; the source/rewrite and
representative-branch audit remains required. The ledger has not yet been
accepted against the pending V414 boundary output.

### V414 terminal source/behavior gates and remaining exception identity audit

All three fresh contexts complete with zero compile errors and invalid
patterns: seventeen-source production **279,746 ms**, eighteen-source actual
extension **290,865 ms**, and eighteen-source null/Comparable context
**294,658 ms**. Each preserves the complete pinned production closure.
All five independent callers pass exactly: GCF **220**, Fraction **1,560**,
BigFraction **1,318**, actual ContinuedFraction extension **396**, boundary
**29**; **3,523 rows** total. Logs and paired outputs are retained in
`.tmp/fraction-v414-public-{gcf,fraction,bigfraction,continued,boundary}`.
The extended source/user-precedence control passes **1/1** in
`.tmp/fraction-v414-comparable-precedence-r1.log`; the first command selected
zero tests under an incorrect module path and is not counted as verification.

The initial declaration ledger passes **129** public/bridge/protected/inherited
entries in `.tmp/fraction-v414-public-api-coverage.tsv`, including anonymous
ContinuedFraction$1.get and its erased Supplier bridge. This is operation
coverage, not proof that all remaining error/reflection observations are right.

The final error-contract review detects that the earlier callers intentionally
observe simple exception class names. They cannot establish the full Java
binary class identity. The new independent FractionExceptionIdentityOracle
checks five production paths with complete getClass().getName(), arithmetic
classification and exact UTF16 messages. V414 has **four name mismatches**:
FractionException is missing its package (two paths), IllegalArgumentException
is missing java.lang, and ArithmeticUtils.NumbersArithmeticException lacks
its package and JVM nested-class dollar spelling. NumberFormatException's
complete identity already matches. All five messages and arithmetic flags
match. This is not a numerical discrepancy or a changed acceptance policy.
Evidence: `.tmp/fraction-v414-public-exception-identity/diff.txt`; the independent
caller builds/runs successfully and the exact comparison exits **1**.

Whole-path evidence: ordinary source-object get_class already emits the proper
binary name through java_naming::source_binary_class_name. Exception superclass
construction instead tags with ty.name; JavaException.get_class forwards this
short internal label unchanged to JavaClass::for_name. The exception parent
registry and source catch aliases also use internal declaration names. A
correct batch must reuse the existing source binary-name authority and native
exception name metadata through construction, registry/class lookup and subtype
matching, preserving existing catch/cause/suppressed behavior and avoiding
namespace collisions. Do not simply qualify a display string while leaving
the hierarchy contract inconsistent, or strip application package names to
match native aliases. Inspect the existing parent/alias and class-info emitters
before implementing this batch. Nested source names need JVM dollar spelling;
simple-name observations must remain Java getSimpleName semantics.

The coverage auditor now also requires the independent `--exception` output
pair to match before it succeeds. Its previous 129-entry report does not waive
this new concrete failure. Neither Fraction nor Arrays is accepted. The other
four prior Numbers modules and Commons RNG still need current-producer
preservation after the final correctness batch. All owned V406–V414 processes
are terminal and collected at this checkpoint; no job is being restarted.

### V415 exception binary-identity contract batch and V416 Object-root control

The V414 identity failures require a cross-function metadata contract, not a
new exception representation. ExceptionFact now records the original source
binary name through the existing java_naming::source_binary_class_name authority.
Constructor and base-conversion emission pass both the compiler dispatch key
and that binary name to JavaException. Its existing class_name() observation
retains the internal key; getClass and Throwable text use the binary name.
Causes and suppressed clones retain both. Catch and instanceof targets consume
the same source metadata, and source parent edges retain package and nesting.
Native names come from the JDK exception catalog, including java.io/java.util
namespaces. Runtime parent lookup no longer treats an arbitrary application
package as a native exception merely because the leaf names agree.

The new ExceptionBinaryIdentityProbe independently runs on Java and translated
Rust and covers nested inherited exceptions, source IOException versus native
IOException, cause-only messages, causes, suppressed exceptions and a literal
dollar in a declaration identifier. Its initial Java signature declared the
checked helper too broadly; that fixture declaration was corrected. The first
valid translation then exposed an invalid `pub type Literal$Dollar` convenience
alias. The smallest correct rewrite is `LiteralDollar` for the Rust alias while
preserving the original Java binary identity. The general producer fix routes
nested convenience aliases through the shared Java-to-Rust naming helper and
uses its existing identifier-part conversion for simple dollar-containing names.
No generated Rust or pinned source is repaired.

The valid control then exposes three producer/consumer gaps. A caught value's
physical JavaException type was not admitted by cause-only constructor selection;
it incorrectly emitted a String argument. Source-to-native-ancestor conversion
could ignore the primary source unit and consult only imported external method
owners, missing an implicit java.lang base despite an emitted From implementation.
The correction uses the same embedded_external_super_name_for_type proof as that
implementation and retains the requirement that source conversion code is emitted.
Finally, the parser had already shortened explicit java.io.IOException to
IOException before lexical nested qualification, so subsequent resolution chose
the source nested IOException. A standalone linked diagnostic records parsed and
resolved trees in `.tmp/fraction-v415-{parsed,resolved}.txt`; retaining exact JDK
exception qualification at parse time repairs the first lost-identity boundary.
Constructor refinement and slot shortcuts also respect an explicit owner instead
of accepting a shortened external summary as evidence for another source class.

All four exception text/cause contracts pass the final V415 metadata batch
in **32.76 s**, including the new control. Complete library verification is
**1,725/1,725** in **86.88 s**, Fraction lowering contracts **13/13** in
**40.51 s**, and native SAM contracts **5/5** in **15.80 s**. Evidence is in
`.tmp/fraction-v415-exception-contracts-r5.log` and
`.tmp/fraction-v415-batch-regressions.log`. Corpus caller
simple-name observations now also account for the known nested exception dollar
separator, matching their Java getSimpleName observations; these pinned exception
identifiers have no literal dollar. The separate full binary-name oracle remains
mandatory and unchanged.

Frozen V415 SHA256:
`47d17da4b37c31c2c006ebef052e748d56146a797f599d50297e30a75423fbb8`.
Fresh complete production, extension and boundary contexts all pass translation,
parsing, invalid-pattern scans and cargo check with zero errors: **17 files /
314,108 ms**, **18 files / 335,913 ms**, **18 files / 341,256 ms**. Roots are
`.tmp/commons-numbers-1.3-fraction-v415-{production,extension,boundary}`.

All six independent Java/generated Rust comparisons pass exactly, **3,528 rows**:
Fraction **1,560**, BigFraction **1,318**, GCF **220**, actual ContinuedFraction
extension **396**, null/Comparable boundary **29**, full exception identity **5**.
All five full-name observations now agree (four previously differed), including the
ArithmeticUtils$NumbersArithmeticException binary spelling. Output roots are
`.tmp/fraction-v415-public-{bigfraction,continued,boundary,exception}` and
`.tmp/fraction-v415-public-{fraction,gcf}-r2`. The coverage auditor passes all
**129** inventoried API/bridge/protected/inherited declarations, now including
its required full-identity output-pair guard; ledger:
`.tmp/fraction-v415-public-api-coverage.tsv`.

Parallel callers initially exposed a harness race: Fraction's first Rust output
was byte-identical to GCF's 220 rows because their common executable path in a
shared CARGO_TARGET_DIR was overwritten between build unlock and execution.
This was a test-runner failure, not a translated-algorithm discrepancy. The
maintained runner now derives a distinct executable/package name from each
checkpoint path. Concurrent fresh Fraction/GCF reruns pass exactly, with their
different correct row counts. The original failed evidence is retained.

V416 adds only the Object-root binary-name alias to this translator batch.
The focused identity control additionally checks both Object aliases and rejects
an unrelated qualified IOException name: **1/1**, **20.63 s** in
`.tmp/fraction-v416-object-root-control.log`. Frozen V416 SHA256:
`edca3420f063c76b230f51eed776ad4bc78726a43e944018ecbd9065aeaae2d3`.
Do not attribute the V415 library/full-Fraction gates to V416. Avoid repeating
all three full Fraction contexts for that one catalog entry alone; include the
final producer at the next substantial verification boundary.

Fresh V416 preservation passes Primes **3 files / 34,392 ms**, **3,594 exact
behavior rows**, and Root Finder **3 files / 30,881 ms**, **122 exact rows**.
Translation roots: `.tmp/commons-numbers-1.3-{primes,rootfinder}-v416-preserved`;
behavior roots: `.tmp/numbers-{primes,rootfinder}-v416-behavior`.
Core, Complex, Quaternion, Angle and Commons RNG still need current-producer
preservation under their existing explicit numerical policies. Neither module
is accepted yet; finish those gates and the final Fraction requirement review
before the Arrays baseline/rewrite audit. No upstream source was changed.

### V417 preservation audit: finalized hierarchy inventories and canonical erasure

V416 completes fresh Fraction's three generation contexts and all six existing
independent comparisons, **3,528 exact rows**, plus the **129** declaration ledger.
Its final library gate is **1,725/1,725** in **110.58 s**. Primes, Root Finder,
Core, Complex, Quaternion and RNG behavior are preserved under the existing
policies. Angle preservation instead exposes **three E0599 errors**: portable
Object-cast recovery emits an `AngleValue::Angle` arm while representation
planning correctly excludes the abstract Angle payload. V416 Angle is not a
passing gate. The old V378 enum contained the abstract payload, which is why
the independently reconstructed cast inventory had previously compiled.

The minimal correct rewrite removes the abstract payload branch while retaining
all actual concrete payload handles and carrier recovery. Trace: finalized
representation variants -> facts-only conversion planning -> both casts and
instanceof -> reference clone/identity registration -> erased Object storage.
The portable planner had rebuilt a separate target-plus-direct-children list,
also missing transitive concrete descendants. V417 publishes the final source
hierarchy carrier inventory into FactSet once after planning, in both single-unit
and shared-batch paths. Cast and instanceof planners consume that inventory.
Declaration-only abstract owners remain excluded because they emit no carrier.
This is a shared representation-contract correction, not an Angle condition.

The strengthened ErasedClassProbe retains its eleven existing guards and adds
three for immutable reference identity and a leaf below an abstract intermediate
class. It additionally exposes an erased carrier clone failure: abstract-owner
filtering had discarded actual carrier registrations as well as impossible raw
abstract objects. V417 retains the actual carrier's clone/identity registration.
Object erasure of a source hierarchy now clones its canonical payload reference,
using the same factored match kernel as hierarchy equality and natural ordering.
This allows an Object erased through one base carrier to recover through another
valid abstract ancestor without boxing a base enum as the Java runtime class.
Anonymous virtual payloads retain their carrier when no statically named payload
can be exposed. No Java source or generated deliverable is manually repaired.

Final focused V417 verification is **1,725/1,725 library** (**78.55 s**), **6/6
Angle lowering** (**27.92 s**) and **13/13 Fraction lowering** (**38.59 s**), in
`.tmp/fraction-v417-final-batch.log`. The earlier complete exception suite also
passes **4/4** (**34.33 s**) in `.tmp/fraction-v417-carrier-contracts-r2.log`.
All earlier failing controls remain retained; tests are not weakened.

Frozen producer:
`.tmp/new-java2rust-numbers-default-v417-carrier-inventory-debug`, SHA256
`1c100ae35ceb64304c5d001d620345b3076d0f97c4c9de335802749474cb0002`.
All eleven complete generation contexts below pass zero failed/unsupported/
invalid/stubbed/compile errors and zero no-op facade call sites:

| Context | Files (including separate client where present) | Total ms |
|---|---:|---:|
| Fraction production | 17 | 301565 |
| Fraction extension | 18 | 317282 |
| Fraction null/Comparable boundary | 18 | 321881 |
| Core contracts | 12 | 172536 |
| Complex contracts | 3 | 117236 |
| Quaternion contracts | 15 | 269416 |
| Angle contracts | 16 | 205429 |
| Primes | 3 | 34955 |
| Root Finder | 3 | 33004 |
| RNG source64 | 11 | 65286 |
| RNG complete sampling/native closure | 46 | 332940 |

Roots follow `.tmp/commons-numbers-1.3-fraction-v417-{production,extension,boundary}`,
`.tmp/commons-numbers-1.3-{core,complex,quaternion,angle}-contracts-v417-preserved`,
`.tmp/commons-numbers-1.3-{primes,rootfinder}-v417-preserved`, and
`.tmp/commons-rng-1.0-{source64,sampling}-v417-preserved`.
The requirement audit `.tmp/fraction-v417-requirements-audit.json` verifies the
producer and summary gates and records numeric-runtime input hashes.

All existing Fraction callers again match **3,528 exact rows**, in
`.tmp/fraction-v417-public-{gcf,fraction,bigfraction,continued,boundary,exception}`.
The maintained Fraction runner now snapshots the independent Java and Rust
callers, source extension where present, runner and producer metadata, and hashes
every translated input and caller. Java compiles the retained snapshot; Rust's
main is copied from its retained snapshot. No generated algorithms are replaced.

All accepted-module behavior gates pass on V417: Primes **3,594**, Root Finder
**122**, Core **7,888** with the unchanged four arithmetic NaN-sign components,
Complex's **32** client guards and **1,863** reference requests, Quaternion's
**36** client guards and **4,532** numerical observations under the unchanged
geometry policy, and Angle's **28** client guards and **4,642 exact** observations.
The numeric Angle caller's old boxing helper named the impossible abstract
variant; it now invokes the generated general erasure API. Its input matrix and
all output observations remain unchanged, not skipped. Its first failed build
is retained; the passing runtime root is
`.tmp/numbers-angle-v417-numeric-r2-behavior`.
Complex and Quaternion raw Java outputs, raw Rust outputs and strict difference
files are byte-identical to V378. Strict verifiers deliberately return **1** for
the already recorded **78** Complex and **12** Quaternion rounding rows; this is
not represented as a strict pass. The separate reference and 80-digit geometry
gates pass; `.tmp/quaternion-v417-compatibility.json` retains the latter evidence.
RNG source64/native/collections/distributions pass **2,865 / 12,601 / 162 / 2,172**
exact observations in `.tmp/rng-*-v417-behavior`. All owned jobs at this checkpoint
are terminal and collected, including failed diagnostic runs.

### V417 final Fraction review: direct null-exception message gap

Neither Fraction nor Arrays is accepted yet. The prior null boundary caller
checks exception type but catches it inside the translated adapter, hiding its
message. The new FractionNullExceptionOracle and independent Rust caller invoke
the public generated algorithms directly with typed null inputs. They observe
full exception identity, NPE classification and exact UTF16 messages. The seven
requests reuse the existing untouched eighteen-source boundary translation;
neither the pinned sources nor the translated adapter is modified.

All seven exception identities/classifications match. Messages **3/7** match;
**four** differ in **three general paths**:

| Requests | Java behavior | Current generated Rust |
|---|---|---|
| 0, 1: Fraction/BigFraction.parse(null) | `String.replace(java.lang.CharSequence, java.lang.CharSequence)` parameter dereference diagnostic | generic `Java null receiver` |
| 2: BigFraction.of((BigInteger)null) | supplied `Objects.requireNonNull` message `numerator` | supplied message evaluated then discarded |
| 6: BigFraction.add((BigInteger)null) | `java.math.BigInteger.signum()` parameter dereference diagnostic | generic `Java null receiver` |

Requests 3–5 (source BigFraction.add and both compareTo paths) match fully.
The new comparison builds/runs and exits **1** at the exact diff:
`.tmp/fraction-v417-public-null-exception/diff.txt`. Decoded messages are retained
in the requirements audit JSON. This is a concrete error-contract gap, not an FP
allowance or a newly expanded numerical campaign. The coverage auditor now also
requires the `--null-exception` output pair before succeeding. Older 129-entry
reports do not waive this observed failure.

Next coherent batch: reuse source_parameter_null_diagnostic and its checked
projection visitor for native text and mapped receivers. Its existing String
descriptor fallback only covers search methods; generalize the authoritative
native overload descriptor to replacement rather than inventing a corpus label.
Keep java.lang.CharSequence qualified in diagnostic argument types; String/Object
have different JVM short-name rules. Resolve modeled Java owner names through
existing external type metadata, preserving source declaration precedence.
For Objects.requireNonNull, call_lowering currently binds the second argument
but unconditionally uses the generic receiver unwrap; carry the actual nullable
message into the existing JavaException constructor contract, preserving argument
evaluation order and avoiding accidental eager evaluation of Supplier overloads.
Add focused regressions for the three contracts, then regenerate the relevant
boundary context after the coherent batch. No architectural redesign is required.

Arrays source preparation additionally identifies Selection.select(int[], int[])
and its subrange overload as genuine same-allocation argument cases: indices may
be destructively modified, and the public API does not require distinct arrays.
The initial generated-code rewrite audit must cover this alongside SortInPlace's
sequential repeated/key-alias companion permutation. A copied index vector or
two simultaneous exclusive Rust borrows cannot be assumed semantically correct.
This is pre-baseline evidence only; no Arrays translator fix or acceptance is
claimed, and the full baseline/rewrite feasibility gate is still required.

### V418 null-message contract batch (verification in progress)

Expanded the independent public null-exception matrix from seven to twenty-one
requests, adding both BigInteger numerator/denominator positions, all nullable
arithmetic overload families and BigDecimal conversion rounding modes. Frozen
V417 builds and executes all twenty-one; exception identity/classification match
throughout, while eleven messages differ. Evidence is retained in
`.tmp/fraction-v417-public-null-exception-expanded-r2` (strict diff exit 1).
Its initial attempt failed only because the new Rust caller used a nonexistent
`BigInteger::one`; the corrected caller uses the existing `value_of(1)` API.

The coherent general fix reuses native String overload descriptors, mapped Java
type identities, checked null projection markers and the existing JavaException
payload. Explicit `Objects.requireNonNull(value, message)` keeps nullable UTF16
message identity and eager argument order, even for a present value. Unsupported
Supplier overloads are not accidentally treated as eagerly evaluated Strings.
Foreign required-argument diagnostics are optional metadata on the existing
selected argument-effect contract, rather than a parallel table matched by fact
equality. This ensures even identical later pack overrides can remove a message.
BigDecimal's rounding-mode diagnostic is supplied by its native runtime pack.
The replacement visits only the outer required-value projection, leaving nested
argument evaluation and staged operand exceptions intact.

The expanded NativeNullMessageProbe checks all four paths, including divide,
scaled divide and unchanged-scale setScale with null rounding modes. Before the
final metadata representation cleanup, all five exception-text and seven checked
identity integration tests passed in
`.tmp/fraction-v418-null-contracts-r3.log`. The earlier batch additionally passed
1,725 library tests and thirteen Fraction lowering tests in
`.tmp/fraction-v418-batch-regressions.log`. Three checked-Optional test caller
assertions were corrected to unwrap the already-existing Result ABI; frozen V417
generated that same ABI from the exact test input. No product assertion was
weakened. Current-tree final batch evidence will be recorded separately.

No fresh V418 corpus pass or Fraction acceptance is claimed at this checkpoint.
The next gate is a frozen-producer eighteen-file boundary translation and exact
twenty-one-request comparison, followed by the complete coverage audit.

V418 gate update: `.tmp/fraction-v418-final-batch.log` is terminal green:
**1,729** library, **7** checked-identity, **5** exception-text and **13** Fraction
lowering tests. Frozen producer
`.tmp/new-java2rust-numbers-default-v418-null-contracts-debug` has SHA256
`526cc52b457c29ee6058b4488630f1b434d63a36614ed1bd9b73a2c10d6969fb`.
Complete production, extension and boundary roots
`.tmp/commons-numbers-1.3-fraction-v418-{production,extension,boundary}` are all
terminal zero, with **17 / 18 / 18** files and zero generation, compile,
unsupported, invalid-pattern, stubbed-constant and no-op-call-site counts.
The direct twenty-one-request null-exception comparison now passes exactly in
`.tmp/fraction-v418-public-null-exception`; the separate twenty-nine-row boundary
comparison passes in `.tmp/fraction-v418-public-boundary`. The latter includes
two null Suppliers with type/control checks; it does not establish JVM enhanced
NPE message parity for those adapters. The remaining algorithm callers and
coverage audit are still pending at this update.

Post-freeze review also added null-message equality to the existing arity-erased
contract-agreement predicate. The three metadata regressions pass in
`.tmp/fraction-v418-null-arity-regressions.log`, including conflict rejection when
no arity is known and agreement after the messages coincide. This one-line
semantic correction and its test are later than the frozen producer; do not
attribute them to the corpus artifact above.

To overlap independent work, the complete eleven-file Arrays entry baseline now
runs with that frozen producer at `.tmp/commons-numbers-1.3-arrays-v418-baseline`.
Its terminal result and generated-code rewrite audit are pending. This advances
measurement only; no Arrays fixes or acceptance precede that audit, and Fraction
remains the first acceptance target.

### V418 Fraction complete caller gate and Arrays entry rewrite audit

All seven V418 independent callers are terminal zero, collected, with **3,549**
exact observations: gcf **220**, fraction **1,560**, bigfraction **1,318**,
continued **396**, boundary **29**, exception **5**, null-exception **21**.
Their roots are `.tmp/fraction-v418-public-MODE`. The coverage auditor succeeds
and writes **129** public declarations to `.tmp/fraction-v418-public-coverage.tsv`.
Complete production/extension/boundary totals are **267,031 / 274,701 / 276,057 ms**.
The final combined earlier-corpus preservation remains a campaign completion
gate; V417's complete preservation evidence above remains intact and is not
mislabelled as V418. JVM enhanced null-Supplier messages remain outside the
twenty-one direct-message observations; the two Supplier null paths have exact
type/control checks in the boundary fixture.

Arrays baseline is terminal **1**, **91,645 ms**, with all **11** source files
generated and no unsupported statements/members/expressions, no parse failures,
no stubbed constants or no-op call sites. It has **13** Rust compile diagnostics
and one invalid-pattern file. Counts are evidence, not the implementation plan:

| Family | Concrete generated failure | Minimal correct rewrite / classification |
|---|---|---|
| Diverging loops | BitIndexUpdatingInterval nextIndex/previousIndex plus canonical wrappers end in `while true` returning unit | Emit Rust `loop` for a Java constant-true loop; preserve breaks/labels. Four sites, local emission. |
| Nullable dynamic test | IndexSupport countIndices calls `as_any` on `Option<&dyn UpdatingInterval>` | Test the optional payload with `is_some_and`; null is false. Local emission through existing optional/reference projection. |
| Shared-array text | MultidimensionalCounter toString passes `Option<Rc<RefCell<Vec<i32>>>>` to a slice-only utility, then returns Rust String as JavaString | Borrow within a scoped array-input visitor and use authoritative Java text result adaptation. Utility/runtime contract, no array copy or escaping borrow. |
| Comparator representation | SortInPlace stores a synchronized native comparator but Collections.sort forces an unsynchronized value conversion; its helper also demands natural ordering of non-Comparable PairDoubleInteger | Invoke the selected comparator through its native callable contract without changing allocation identity. General nullable-comparator natural fallback must still throw the right runtime error; a supplied comparator cannot impose Comparable on its inputs. Cross-call contract. |
| Companion arrays | SortInPlace.apply takes `&mut [f64]` and `&mut Array2<f64>`, copies one row and treats another as a shared handle | Keep key and row allocations as reference handles, with nullable independent rows. Propagate row allocation requirements through producers, parent storage, signatures, indexed reads and writes. Reference-row layout must take precedence over rectangular optimization when row identity escapes. Framework representation extension using existing nested allocation metadata. |

The highest-risk rewrite is concretely tested in an isolated generated-crate
copy at `.tmp/arrays-v418-rewrite-audit`, never in the baseline or deliverables.
Four copied files change: the four terminal loops, nullable dynamic test, scoped
array text conversion, and SortInPlace.apply's reference-row signature/accesses
and comparator invocation. The sort algorithm's list/indices/per-companion-copy
phases remain the same. The copied complete eleven-file crate compiles with
exit **0** (`check.log`). Its independent Java and diagnostic-Rust callers match
**16** observations exactly (`java.tsv`, `rust.tsv`, empty `diff.txt`): ascending
and descending sorts, repeated companion allocation, key-as-companion, repeated
key alias, mixed alias order, ragged/null validation before mutation, and both
integer Selection multi-index overloads with same-allocation inputs, including
duplicate indices. This is feasibility evidence only, not translated acceptance.
The initial baseline remains untouched and failing.

Existing Selection signatures already use shared integer-array handles and
short-lived read/write guards, and all eight same-allocation selection probes
work without modifying that generated algorithm. UpdatingInterval split paths
already clone the common backing array handle into a new interval and use the
canonical interface forwarders. SortInPlace's enum stores the native synchronized
comparator once; the diagnostic uses its existing compare_result operation,
preserving allocation identity. There are no other escaping callbacks in this
production closure. MultidimensionalCounter already copies its constructor sizes,
allocates independent toMulti outputs and returns copied getSizes values.

The framework has the necessary handle and nested-placement vocabulary; the gap
is consistent selection/propagation of the reference-row physical layout.
`slot_render` currently chooses Array2 from Java rank/type alone, while row locals
can already require Rc handles. `shape_projection` carries child placements and
`signature::propagate_native_collection_child_allocations` has the reusable graph
for propagating them. Audit that path and use one shared physical-layout decision
in rendering, construction, length/access and row binding, rather than fixing
only SortInPlace's emitted syntax. Preserve proven rectangular workloads.

Next coherent batches: (1) terminal loops, nullable instanceof and scoped
shared-array text with executable controls; (2) the row-layout producer/storage/
signature/consumer contract together with stored native comparator invocation
and a non-Comparable-key control. Batch compile and corpus cycles across these
related edits. Re-audit new runtime families, especially row replacement/null
rows, stable NaN/signed-zero ordering and interval split aliases. No Arrays
translator fix or acceptance is claimed by this audit.

### V419 Arrays general contract work (initial runs; results below)

The first local batch passes its executable Java/Rust probe in both String
representations: `.tmp/arrays-v419-local-contracts-r1.log`, **1/1** integration
test with two generated crates. `ArrayBoundaryProjectionProbe` checks returning
conditionless/labeled loops, nullable interface type tests and pattern binding,
local/static shared-array formatting, mutation through aliases and null arrays.
The implementation reuses loop assembly (`for (;;)` must produce Rust `loop`),
adds a composable optional-payload type-test/downcast projection, gives the
array-to-string input protocol a scoped visitor supporting Rc/RefCell and
Arc/Mutex, and extends existing native-text normalization to Arrays formatting.

The next batch is not green or corpus-measured yet. General parameter alias
seeding recognizes matching array allocations at root/row positions across
formal parameters. Materialized array-row references use existing structural
child-nullability and nested-allocation constraints; no corpus-name condition or
generated-code repair is involved. Stored native SAM adapters retain their
actual synchronized/local allocation. Nullable ordering comparator lowering
captures the native value once, invokes it through compare_result when present,
and uses lazy natural ordering when absent. A known non-Comparable concrete
source object raises the existing ClassCastException only upon comparison;
explicit comparators impose no Comparable bound. Callback planning uses the
authoritative ordering-consumer query with the lexical typed walker to require
a value carrier through forwarding, since null selects natural ordering.

The new `ReferenceRowsProbe` and `StoredComparatorProbe` remain unweakened.
First combined run `.tmp/arrays-v419-storage-contracts-r1.log` is **1/3** passing:
row producers/varargs show storage-boundary failures, and comparator forwarding
plus chained fields show ABI failures. The second run
`.tmp/arrays-v419-storage-contracts-r2.log` is also **1/3**: callback value ABI now
works, leaving four chained-field checks; row packing has no fake spread tail,
but whole-array metadata still reaches primitive elements and fixed prefix
arguments have a handle ownership mismatch. No fresh full Arrays count is
inferred from these focused results; baseline remains V418's **13** diagnostics.

The row whole-path re-audit identifies reusable resolver evidence rather than a
new heuristic. `ResolvedCall.arg_target_types` retains formal array slots,
whereas `arg_coercions` records each actual argument's selected target. The
diagnostic inspector `.tmp/arrays-v419-inspect-r2.txt` shows rotate(double[],
double[]...) expanded rows targeting double[], and its explicit outer-array
calls targeting double[][]. The argument-target query now prefers that per-arg
evidence; its boundary shape consumer and varargs constraint projection share
the query. Backward constraints use exact argument bindings and project native
allocation/nullability paths through varargs element zero. Lowering reuses the
existing overload applicability report to distinguish explicit arrays from
array-valued elements, constructs fresh row arguments in their destination
slots, inherits stored child placement, and removes implicit tail spreading
(Java has no such spread syntax). The scoped physical-result query additionally
uses native List.get's stored child shape for chained fields.

Third combined verification is running in
`.tmp/arrays-v419-storage-contracts-r3.log`; its terminal result is pending.
After that result, batch any remaining whole-path repairs before a full Arrays
regeneration. Add focused plan regressions for argument-target precedence and
varargs row-path projection, then perform the library/affected corpus gates at
the coherent boundary. Preserve frozen V418 evidence and the isolated diagnostic
rewrite; neither is current V419 acceptance output.

### V419 completed batch and independent observations

The third storage run reduced the failures to row packing and one synchronized
callback boundary; the fourth run passes all three contracts, including both
String representations in the boundary probe. Frozen producer
`.tmp/new-java2rust-numbers-default-v419-reference-rows-debug`, SHA256
`69b9a59d085a9d3a9440fdcac861b10bcecddb0983ef69b20cf29d2054bf8a2e`, generates
all **11** untouched Arrays files with zero failed, unsupported, parse-invalid,
stub, no-op call-site or compiler gates. The production run took **95523 ms**.
Root: `.tmp/commons-numbers-1.3-arrays-v419-production`.

The coherent regression batch `.tmp/arrays-v419-batch-regressions-r2.log` passes
**1734 library, 3 Arrays, 7 checked-identity, 5 exception-text, 13 Fraction and
5 native-SAM tests**. Initial stale unit expectations for FnMut/value-comparator
ABI and native-SAM root edges were corrected with explicit replacement
assertions; descendant-edge assertions remain.

The maintained independent Arrays callers and runner exercise public Selection
overloads, complete mutated arrays and index arrays, repeated/key/companion
aliases, sort validation before mutation, enum operations, counter conversion
and defensive copies, index-set failures and interval splitting/reuse. In
`.tmp/arrays-v419-public-r1`, **2888 Selection, 112 Counter and 92 helper rows**
match exactly. Of **402 sort rows**, **352** match exactly; **50** differ solely
in JVM-generated NullPointerException wording. Their exception classes and all
mutation state match. The raw strict comparison is retained and exits **1**.
No tolerance or automatic diagnostic exemption has been applied.

Java reports `Cannot read the array length because "<local6>" is null`; Rust
reports `Java null receiver`. The JDK documents this generated message as
implementation-specific, unlike an explicitly supplied constructor message:
https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/NullPointerException.html#getMessage()
An asynchronous acceptance-policy question is pending; elapsed time is not
approval. Both modules remain formally unaccepted while final requirements are
being audited.

Additional direct finite-input tests call all **15** primitive Sorting helper
overloads (sort, sort3, sort5 and both median4 operations), and split/reuse a
1024-key interval across its SCAN_SIZE=256 binary-search boundary. They produce
**169 exact helper rows** in `.tmp/arrays-v419-helpers-r2` (77 added rows).
`scripts/audit-commons-numbers-arrays-coverage.py` maps all **43** public and
relevant inherited declarations to exact observations. It still exits **1** for
the 50 unresolved sort requests; coverage alone is not acceptance.

All eleven V419 preservation translations are terminal with exit **0**: Fraction
production/extension/null boundary, Core, Complex, Quaternion and Angle contract
closures, Primes, Root Finder, RNG source64 and complete sampling/native closure.
Their job commands and logs are in `.tmp/v419-preservation-jobs.json`.
The seven Fraction independent callers match **3549** exact rows, and
`.tmp/fraction-v419-public-coverage.tsv` maps **129** public/inherited APIs.
Fresh outputs: `.tmp/fraction-v419-public-{gcf,fraction,bigfraction,continued,boundary,exception,null-exception}`.

Fresh earlier-module behavior outputs are `.tmp/v419-preserved-*-behavior`:
Primes **3594**, Root Finder **122**, Angle **4642**, RNG source64/native/
collections/distributions **2865/12601/162/2172** exact rows. Core passes **7888**
under the existing four arithmetic NaN-sign component policy. Core, Complex,
Quaternion and Angle translated contract clients pass. Complex meets **1863**
independent reference bounds. Complex and Quaternion strict comparisons still
exit **1** with the known **78/12** rows; all three raw artifacts (Java, Rust,
strict differences) are byte-identical to V417. The unchanged 80-digit Quaternion
geometry gate passes in `.tmp/quaternion-v419-compatibility.json`.

### V420 varargs boundary follow-up audit (in progress)

The independent ObjectVarargsShapeProbe adds seven Java controls: explicit
Object[] is the complete pack; an Object[] after another trailing expression is
one Object; int[] is always one Object; each holds with and without a fixed
prefix; an empty pack has length zero. V419 generated code has eight compile
failures from three shared causes. The minimal correct rewrites are:

* Classify explicit packs using the resolver's original formal Java array type,
  not the solved ABI's nullable element type. Primitive int[] cannot become
  Object[]; the existing overload-applicability query owns that decision.
* An explicit shared Object[] passes an Rc handle clone, not a deep-cloned Vec.
  The existing reused-owned-argument adapter supports this; bypass only the
  legacy unshared erased-vector copy helper for shared allocations.
* Boxing an array as Object boxes its existing Rc carrier. Object's lack of a
  native collection placement is not an instruction to unwrap/copy that array.
  Preserve allocation when the selected dispatch is EraseObject; retain the
  unsupported-placement guard for actual collection-to-collection changes.

These are local planner/emission repairs using the current representation,
not a framework redesign. Trace is original formal type -> pack/element choice
-> destination row/element shape -> existing shared handle -> erased Object.
The executable control and two legacy tests that incorrectly require implicit
tail spreading must pass with real Java packing semantics before freezing the
next producer. V419 evidence above is frozen and is not relabeled as V420.

The follow-up additionally found that overload applicability recursively applied
scalar boxing/widening to array elements: int[] incorrectly matched Object[]
and long[]. Both the applicability report and legacy score now share the
primitive-array component guard. Array references widen to Object, Cloneable
and Serializable; nested int[][] can still widen to Object[] through its
reference-valued component. A focused resolver test covers positive and negative
cases, including fixed versus variable-arity Object... selection.

Erased-reference clone recovery now checks the actual physical value is erased,
rather than choosing the erased clone helper solely from its destination.
Concrete native handles use proven carrier cloning followed by boxing. This
fixed the new control's runtime panic after its compiler errors were resolved.
The shared native erasure test retains the negative guard against actual
array-to-array placement changes. `.tmp/arrays-v420-batch-regressions.log`
passes **1736 library / 4 Arrays / 7 checked identity / 5 exception text /
13 Fraction / 5 native-SAM tests**.

The legacy context-only regression exposed two additional boundaries. Summary
facades declare a different ABI from emitted source. Declaration rendering and
varargs packing now reuse `source_summary_facade_varargs_param_shape`, rather
than packing according to a source signature that is not emitted. The inherited
constructor context call now reuses normal shared-source receiver adaptation,
including RefCell/Mutex access, instead of directly invoking a payload method
on a handle. The two legacy tests now require two separately packed trailing
references and forbid implicit tail spreading; both pass in
`.tmp/arrays-v420-legacy-{varargs,inherited}-r4.log`. The additional shared facade
shape unit passes **1/1** in `.tmp/arrays-v420-facade-unit-r2.log` (the earlier
short-name `--exact` invocation ran zero tests and is not verification).

Frozen producer `.tmp/new-java2rust-numbers-default-v420-varargs-erasure-debug`,
SHA256 `7893a5b56e121ceadd189092db982d483f3baeaac3b32ead9a0ea512037bf889`,
starts fresh production and preservation runs in
`.tmp/v420-preservation-jobs.json`. They also include a separate complete Arrays
closure plus ArraysNullBoundary, a co-translated client checking **20** typed
root-null exception and pre-mutation guards. The runner's `null-boundary` mode
requires exactly the eleven upstream files plus that unchanged client and
checks all twenty Java controls return zero as well as comparing Rust output.
These supplement the direct production API callers; they do not establish
JVM-generated message parity. Terminal results are pending at this checkpoint.

The expanded V419 direct API matrix is retained in `.tmp/arrays-v419-public-r6`:
**4364 Selection rows**, including interior quantiles, dense reversed index
requests, invalid from/to and empty subranges, **112 Counter**, **169 helper**
and **407 sort/enum rows**. All except the same **50** generated NPE-message rows
match exactly; invalid enum names and explicit-null enum messages match too.
The first enum caller attempted a nonexistent JavaString::from_str factory;
its failed build is retained in r5, corrected to the existing from_scalar in
r6. No generated source changed. The complete API coverage audit still maps
**43** declarations and correctly reports a strict failure for those 50 rows.

### V421 root-null entry audit: enum receivers and fresh ragged arrays

All twelve V420 production/preservation roots (including Arrays production)
are terminal with exit **0**. The additional Arrays boundary root generates all
**12** files but exits **1** with two missing enum accessor diagnostics:
`SortInPlace::__static_ASCENDING` and `__static_DESCENDING`. No production file
is omitted. Direct V420 Arrays callers in `.tmp/arrays-v420-public-r1` reproduce
V419's **4364/407/112/169** rows and the same **50** message-only differences.
The supplemental root therefore reveals a real Java-producer path gap that
direct Rust calls to enum variants alone could not establish.

Audit `.tmp/arrays-v420-null-rewrite-audit/generated-crate` is a copy only.
Replacing the two nonexistent enum getters with the enum's existing published
constant accessor exposes a third compiler failure: `new double[0][]` is emitted
as `Vec<Option<_>>` for a callee expecting `Option<&mut [Option<Rc<RefCell<Vec<f64>>>>>]>`.
The minimal Rust rewrite is a fresh empty vector borrowed and wrapped in Some
for the duration of the call. This copied crate compiles after the third
diagnostic correction (`check-r2.log`, exit **0**). Neither original generated
crate nor pinned Java was changed; the copy is not an acceptance deliverable.

Both required changes fit the current representation and are local boundary
emission repairs. Static-storage canonicalization recognized only representation
table variants and missed enums with instance state. It now reuses the complete
source enum-constant lookup, including publication semantics, so constants cannot
be converted back into ordinary static-field accessors at call receivers.
Ordinary enum-typed static aliases remain ordinary storage; the existing source
constant classifier owns that distinction.

Ragged creation's checked/unchecked producer helpers returned the raw Vec before
destination conversion. Both now converge on the existing
`fresh_java_array_value_for_slot`: non-null owned producer -> selected native
allocation -> borrow and/or outer Option required by the callee. Dimension
evaluation and negative-size checks stay in the producer. This is the same
destination conversion used by array copy/clone, rather than a varargs-specific
Some/borrow spelling. `NullableArrayCreationProbe` uses initialized enum
receivers, zero/nonzero ragged allocations, typed null key/outer rows and a
negative size; focused Arrays/Fraction/native-SAM and library results are pending
in `.tmp/arrays-v421-{boundary,library}-regressions.log`.

Both gates are now terminal with exit **0**: **1737 library tests** (84.07 s),
**5 Arrays / 13 Fraction / 5 native-SAM integrations**. Frozen V421 producer is
`.tmp/new-java2rust-numbers-default-v421-enum-array-boundaries-debug`; its fresh
SHA256 is `97a0b20695e917bcbce984cf942c7aa11f13de1fb41e718542b17930ed27994b`.
Its fresh
thirteen production/preservation/boundary jobs are recorded in
`.tmp/v421-preservation-jobs.json`. No module acceptance is inferred while these
jobs and current-producer behavior checks are pending.

V421 Arrays production and the complete root-null client closure are now both
terminal **0**, taking **108110 / 116139 ms**. The maintained runner against
`.tmp/commons-numbers-1.3-arrays-v421-boundary` passes **20/20** Java/Rust null
controls in `.tmp/arrays-v421-null-r1`. All controls require the expected NPE and
unchanged data; these assertions do not compare implementation-specific NPE
text. No hypothetical rewritten file is used in either root.

Direct APIs in `.tmp/arrays-v421-public-r1` produce **5052** observations:
**4364 Selection / 407 sort-and-enum / 112 Counter / 169 helpers**. There are
**5002 exact rows**. The strict runner and API audit still return **1** because
of the same **50** null-row messages. The component-level inspection
`.tmp/arrays-v421-message-differences.json` proves every differing label ends
in the null-row case, both exception classes are java.lang.NullPointerException,
all output state is identical, and the only two texts are precisely the JVM
`<local6>` length message and `Java null receiver`. It explicitly records
`policy_approved: false`; no comparison was weakened. The API map remains **43**
declarations in `.tmp/arrays-v421-public-coverage.tsv`.

All **13** V421 generation jobs are now terminal and collected with exit **0**.
The current-state requirement audit is
`.tmp/fraction-arrays-v421-requirements-audit.json`: exact frozen producer hash,
expected closure sizes **17/18/18/12/3/15/16/3/3/11/46/11/12**, all source and
generated paths present and all nine generation/compile/no-op-call-site gates
zero. Both source/declaration manifests remain pinned; the complete upstream
checkout is still `ae8abefb51c2c4d1a05882d61cefc8015e493a8f`.

All seven fresh Fraction callers pass **3549 exact rows**, with **129** APIs
mapped in `.tmp/fraction-v421-public-coverage.tsv`. All prior-module behavior
checks pass under their unchanged explicit policies: Primes **3594**, Root
Finder **122**, Core **7888** (four arithmetic NaN-sign components), Angle
**4642**, RNG source64/native/collections/distributions **2865/12601/162/2172**.
The Core/Complex/Quaternion/Angle translated clients pass. Complex's **1863**
independent reference bounds and Quaternion's 80-digit geometry audit pass.
Complex/Quaternion raw Java, Rust and strict-difference files are byte-identical
to V419; their strict verifiers still return **1** for the known **78/12** rows,
not falsely reported as exact passes. Numeric runtime **14/14** unit tests pass
in `.tmp/fraction-arrays-v421-runtime-tests.log` (zero doctests is separate).

Support-residue review: the broad idiom scanner still reports unused generic
facade declarations. In particular JavaUtilCollections.sort appears as a stub
declaration in SortInPlace's module, but no production call invokes it: actual
sorting uses the native comparator path exercised by the independent matrix.
These are not repaired algorithm deliverables or evidence of callable API
coverage. Source unsupported gates and no-op facade call sites are zero; do not
claim every unused support facade is implemented.

Neither new module is formally accepted at this checkpoint. Current evidence
leaves the generated-NPE-message policy decision pending; strict output and
failure statuses remain intact. After that decision, rerun the acceptance audit
under the explicit chosen contract rather than inferring completion from green
translation or aggregate observation counts alone.

### V421 declaration-completeness revalidation

Fresh compilation and full `javap -p -s` censuses from the pinned production
sources are byte-identical to both maintained declaration manifests:

```sh
NUMBERS_MODULE=fraction NUMBERS_API_OUT="$PWD/.tmp/numbers-fraction-v421-declaration-audit" bash scripts/list-commons-numbers-1.3-api.sh > .tmp/numbers-fraction-v421-declarations.txt
NUMBERS_MODULE=arrays NUMBERS_API_OUT="$PWD/.tmp/numbers-arrays-v421-declaration-audit" bash scripts/list-commons-numbers-1.3-api.sh > .tmp/numbers-arrays-v421-declarations.txt
cmp tests/corpus/numbers/Fraction-1.3-declarations.txt .tmp/numbers-fraction-v421-declarations.txt
cmp tests/corpus/numbers/Arrays-1.3-declarations.txt .tmp/numbers-arrays-v421-declarations.txt
```

Both commands and comparisons are terminal **0**. The machine-readable census
`.tmp/fraction-arrays-v421-jvm-declaration-audit.json` records every field,
method, constructor, initializer and descriptor, including private, nested,
anonymous and compiler-generated members. It complements the public API maps;
public-only observation counts are not substituted for the complete census.

A diagnostic source inspector uses the translator's public parser/resolver and
assigned Rust names, rather than reimplementing its overload mangling. Across
Fraction's complete Core dependency closure and Arrays it records **33 resolved
types, 428 methods, 32 constructors and 128 fields**. Of the 428 methods, **419**
are source declarations, **6** are the materialized inherited Number byte/short
defaults and **3** are class initialization bodies. All **460** resolved named
callables map to definitions or trait declarations on the correct emitted Rust
owner, including shared-support interfaces, nested-type qualification, shared
receiver companions and enum construction functions. The structural check
reuses `scripts/rust_item_utils.py` for impl owners and delimiter scanning.

Artifacts are `.tmp/fraction-arrays-v421-source-inventory.tsv` and
`.tmp/fraction-arrays-v421-callable-presence.json`; the inspector and audit
sources are retained alongside them. This is declaration-presence evidence,
not behavior or exhaustive branch coverage. Anonymous callback bodies may become
closures; their complete JVM declaration record and the independent continued
fraction callback order/state/error observations remain separate evidence.
No translator, runtime, pinned source or generated algorithm was edited in this
revalidation. The requirements audit still records acceptance as unproven until
the generated-NPE wording decision is supplied.

### V422 strict diagnostic path: bound locals without corpus exceptions

The waiver question has not been answered or applied. A further review found
an available implementation path within the existing diagnostic mechanism:
`source_parameter_null_diagnostic` already reconstructs default-JVM parameter
messages and applies them only to marked checks of the original bound operand.
The remaining SortInPlace failures are the same operation on a bound local.
For its ordinary lexical for-loop, JVM registers are this/x/yList/yListLen/len/
j/y, so the row is register 6. The minimal correct Rust change is solely the
message on the existing null check; evaluation, mutation and exception timing
must remain untouched. No corpus name or register number is embedded in the
translator fix.

The reusable analysis records local-register provenance by BindingId once in
the existing per-method usage facts, sharing it across their clones. It counts
the source receiver and parameter widths (long/double consume two registers),
lexical declarations and for-loop initializers; block/branch/loop scopes release
their registers. Lowering queries that same source-method identity, rather than
rescanning bodies per dereference or deriving JVM provenance from Rust storage.
The existing marked-projection diagnostic applies to the exact bound reference;
explicit library messages, operand failures, staging aliases and callbacks keep
their own diagnostics.

A counterexample in the independent Java probe showed why declaration count
alone is insufficient. The active OpenJDK 19.0.2 javac emits no storage for
`final double padding = 1`, so the following array local occupies register 1,
not 3. The local-declaration IR previously discarded `final`. It now retains an
`is_final` fact, populated by the existing modifier parser and preserved through
the constructor substitution copy. Synthetic/test declarations set it explicitly.
The slot model reuses `source_constant_values::fold_constant_expr` with lexical
constant scopes and declared-type casts. Unknown constant references or compiler
lowerings with unmodeled hidden registers decline provenance, rather than
inventing a slot. Try/switch/enhanced-for/pattern/lambda bodies and constructor
synthetic parameters remain outside this local-register model; this does not
change their execution or exception type.

`LocalNullMessageProbe` independently checks nine real Java failures: static and
instance locals, wide parameters, lexical register reuse, ordinary loop locals,
shadowed bindings, final constant omission, String invocation and field reads.
Both the Java baseline and translated Rust pass in
`.tmp/local-null-v422-regression-r2.log`. The first focused run also passed its
original six cases. Native `javap` evidence is retained under
`.tmp/local-null-v422-java`; the probe's baseline initially caught two incorrect
oracle expectations (constant slot and String's abbreviated owner), which were
corrected against actual JVM output before Rust acceptance. No comparison is
relaxed.

The first broader build found old test-only VarDecl literals inside macro token
trees missing the new field. They now set `is_final: false`; source copies retain
the actual fact. The structural audit tool used syn for direct Rust expressions
and patterns, and compiler locations caught macro-embedded fixtures. Broad
library/exception/Arrays/Fraction/native-SAM gates are pending in
`.tmp/local-null-v422-batch-regressions-r2.log`. Fresh full-corpus evidence on a
frozen producer is still required; V421's strict failure remains recorded until
the actual translated algorithms match.

The corrected broad batch is terminal **0**: **1738 library / 5 Arrays /
6 exception-text / 13 Fraction / 5 native-SAM tests**, with the library taking
80.48 s. Frozen producer
`.tmp/new-java2rust-numbers-default-v422-local-null-provenance-debug`, SHA256
`88ccfa3e4882fdd94685c02ef0ef62574fe1a07aa42e539b111d91d74daa2308`, starts
fresh full closures and preservation jobs recorded in
`.tmp/v422-preservation-jobs.json`. Results are not inferred from prior producers.

### Historical V389 Supplier whole-path audit (implemented V392–V398)

The existing stateful Supplier regression remains required. A minimal correct
Rust rewrite needs one persistent anonymous state object, Java default zero
for its int field, captured x, and the canonical enclosing receiver. Native
Supplier clones must share this object; construction must not invoke getA/B;
get increments before virtual getA/getB and exceptions retain that increment.
This is expressible using the existing native from_fallible_fn carrier and
object-backed anonymous field/capture machinery; no payload snapshot or
manual continued-fraction algorithm is needed.

Current boundaries: native anonymous Supplier has no source TypeDecl, so
`lower_native_functional_object_for_slot` rejects it; the object-backed
anonymous interface adapter requires a source interface and rejects fields
without explicit initializers. It already supports mutable field cells,
captured locals, qualified enclosing handles, helper methods and statement
lowering. Capture analysis currently ignores receiverless instance calls
while analyzing anonymous members (`capturing_receiver=false`); explicit
qualified Outer.this requests do carry OuterThis metadata. The implicit
enclosing binding must be proved against the selected method owner, respecting
anonymous self methods and inherited Object methods, before canonical outer
storage can be planned correctly.

Prefer generalizing the shared anonymous state/statement/capture layer and
projecting it to a native SAM carrier, alongside its source-trait projection.
Use the existing native SAM descriptor for arity, parameter/result shapes and
fallibility, and Java default slot initialization for absent field initializers.
An early anonymous-to-named type normalization is another possible design,
but existing synthetic local capture normalization only admits local/parameter
captures and does not yet solve this enclosing-instance case. Re-audit the
adapter body contracts before choosing the implementation batch. No Supplier
fix or fresh behavioral pass is claimed in V389.

Further inspection identifies existing reusable boundaries and two ordering
requirements for that next batch. Scoped anonymous helper calls already use
resolve::select_scoped_instance_method (the ordinary overload/substitution
resolver) and anonymous callable effect facts in lower.rs. Receiver binding
must use that same selection evidence before classifying a receiverless call
as an enclosing-instance capture; matching a method name alone is insufficient.
The global resolver currently resolves anonymous member bodies in the lexical
source owner's context, so its raw selected call alone is also insufficient
to disprove a shadowing anonymous helper.

The existing binder-aware bind_anonymous_field_reads_in_block handles write
targets and unary updates as well as reads, with local shadowing. Apply its
field binding before admitting the native stateful body: the production bare
n++ is otherwise classified as a local increment, contributes no receiver
write, and fails the receiver-statement proof. Reuse the same body renderer
and native SAM descriptor for both admission and emission. Capture storage
already consumes every anonymous owner's typed captures to preserve allocation
identity; prove the selected outer capture upstream rather than adding a
separate native-Supplier storage heuristic. Source-class compound boolean
return rendering remains a separate recorded diagnostic family.

The complete library by itself currently emits ContinuedFraction's abstract
getA/getB as raw-self stubs, whereas the stateful control also contains its
Concrete source subtype. Verify the enclosing receiver's dynamic carrier
through factory, capture field, callback signature and getA/getB consumer in
that control. Acceptance must additionally exercise the public abstract
extension contract with an independent caller; compiling the seventeen-file
library or exercising only GeneralizedContinuedFraction cannot establish
that contract. Do not accidentally retain a cloned base payload or invoke
the abstract raw-self stubs from a concrete callback.

## Arrays entry preparation

The complete inventory/dependency check and maintained runner selection have
been prepared. The V418 baseline now overlaps the final Fraction checks, as
recorded above; inspect it and complete the rewrite audit before any fix campaign.
No Arrays translator fix or acceptance is claimed.
SortInPlace's varargs companion arrays may repeat or alias the key array; Java
applies each companion permutation sequentially to the then-current array.
Do not assume disjoint arrays or change this to a simultaneous snapshot of all
arguments. Also audit enum comparators/nested pair objects, mutable partition
helpers/intervals, subrange boundaries and multidimensional counter arrays.

## Reproduction entry commands

```sh
export CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true
mkdir -p .tmp/fraction-arrays-test-tmp
export TMPDIR="$PWD/.tmp/fraction-arrays-test-tmp"
export NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v423-null-invocation-provenance-debug"
export NEW_JAVA2RUST_BUILD_PROFILE=debug JOBS=2
NUMBERS_MODULE=fraction \
NUMBERS_STAGING_ROOT="$PWD/.tmp/fraction-reproduce-staged" \
NUMBERS_TRANSLATION_ROOT="$PWD/.tmp/fraction-reproduce" \
  bash scripts/run-commons-numbers-1.3.sh --skip-resolver-bench
NUMBERS_MODULE=fraction bash scripts/list-commons-numbers-1.3-api.sh
NUMBERS_MODULE=arrays bash scripts/list-commons-numbers-1.3-api.sh
```

Use fresh roots, not frozen evidence directories. No upstream production file
or API is excluded. Upstream tests/examples/benchmarks are not production
translation targets; they may inform independent behavior callers. Completion
requires both full modules, focused/full regression evidence and fresh earlier
Numbers/RNG preservation, followed by a requirement-by-requirement audit.
