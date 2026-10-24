# Apache Commons Numbers translator campaign

The completed first campaign covers six complete production modules from release 1.3,
untouched commit `ae8abefb51c2c4d1a05882d61cefc8015e493a8f`. Modules are
accepted sequentially; Primes is accepted at V180, Root Finder at V191,
Core at V309, Complex at V335, Quaternion at V373, and Angle at V378
(**6/6**). The final V378 requirement audit and fresh preservation evidence for
all six modules and Commons RNG are recorded below. Core/Complex/Quaternion
retain only their previously documented numerical policies; strict differences
remain visible. Angle passes all 4642 observations exactly, without a numerical
allowance. Historical progress entries describe their own checkpoints, not
current acceptance status. The six-module campaign is complete.
General translator improvements,
not manually repaired generated output, are the deliverable.

The separate Fraction/Arrays follow-up is also accepted at V423, in that order.
Its complete closures, 8,643 exact independent observations, general translator
fixes and fresh preservation of these six modules and Commons RNG are recorded
in [the follow-up audit](commons-numbers-fraction-arrays.md). The original
six-module scope and its numerical policies remain unchanged.

## Source scope and dependencies

The six-module subset of `scripts/focus-sets/commons-numbers-1.3-all.txt`
records all26 production Java files:20 implementation files and6 package-info
files. The shared manifest additionally lists Fraction and Arrays for the
separate subsequent campaign in `commons-numbers-fraction-arrays.md`; those
entries do not expand this completed six-module scope. Metadata files remain
in translation/Java compilation scope. Every declaration, constructor, method,
nested type and overload in these files is included.

| Order | Module | Own files including metadata | Production dependency closure |
|---|---|---:|---|
| 1 | Primes | 3 | JDK only |
| 2 | Root Finder | 3 | JDK only |
| 3 | Core | 11 | JDK only |
| 4 | Complex | 2 | JDK only; Core/RNG are test dependencies |
| 5 | Quaternion | 3 | Core;14 files together |
| 6 | Angle | 4 | Core;15 files together |

Dependencies were checked against pinned module POMs and source imports.
The existing order satisfies production dependencies. Fraction and Arrays have
a separate follow-up campaign in `commons-numbers-fraction-arrays.md`; Gamma,
Field and other modules are not additional targets here. No GUI/examples/benchmarks
or upstream test framework is a production translation target. Independent
callers and relevant upstream tests supply behavior evidence.

`bash scripts/prepare-commons-numbers-1.3.sh` obtains and verifies the exact
clean pin. `NUMBERS_MODULE=primes bash scripts/run-commons-numbers-1.3.sh`
validates the full per-module census, stages byte-identical source files and
uses the normal translation/assembly/check pipeline. Select another named
module to include its recorded production closure. `NUMBERS_SOURCE_ROOT`,
`NUMBERS_STAGING_ROOT` and `NUMBERS_TRANSLATION_ROOT` override locations.

## Primes baseline and rewrite audit

Entry producer V168 is `.tmp/new-java2rust-rng-sampling-object-v168-debug`,
SHA256 `b0637a1e91153ce78afaca55df58cc5a644823fc0786a016bb196f595e5a5fac`.
Baseline `.tmp/commons-numbers-1.3-primes-v168-baseline`, log
`.tmp/numbers-primes-v168-baseline-run.log`, terminal1. All3 files translate
and parse, unsupported/invalid markers0; generated Rust has4 compiler errors,
and1 no-op facade call site. Total34.306s. Compilation alone cannot establish
acceptance: BigInteger valueOf/modPow/intValue reach unimplemented facades.

The source has three public static operations, private utility constructors,
four package-private arithmetic helpers, a constant512-prime table, a derived
last-prime static and a static Entry<Set<Integer>,int[]> wheel table. The wheel
initializer uses stream folds, nested loops and a remainder comparison; factor
lists mutate across helper calls; Miller-Rabin uses arbitrary-precision modular
exponentiation. The hardest contracts are therefore visible in this small scope.

1. The uncaught/static fallible numeric wrapper annotates Ok with the promoted
   operand type (i32), even for comparison results (bool). Keep ordered promoted
   operand temporaries, but derive the Result success type from the operation
   result. Reuse numeric operator planning; do not special-case remainder==0.
   This is a local expression-result contract fix, including other comparisons.
2. Static Entry storage renders Option<(HashSet<i32>,Vec<i32>)>, but initializer
   emission bypasses the existing SimpleImmutableEntry-to-tuple slot lowerer
   and emits a no-op facade constructor. The smallest correction routes static
   writes through the normal typed field-assignment slot, producing Some of
   the modeled entry carrier with correctly lowered components. Trace nested
   component carriers and alias retention before accepting tuple snapshots.
   The getter currently selects tuple field1 without discharging the outer
   nullable Java receiver; reuse the existing null-check/entry projection path.
   These are producer/consumer contract fixes, not a source-specific rewrite.
3. BigInteger currently has only coarse external signatures, not a numeric
   runtime. A general implementation must preserve arbitrary precision and
   modular arithmetic, including invalid modulus and negative-exponent rules;
   using i64/i128 only because this caller starts from int is unacceptable.
   Investigate a mature arbitrary-precision backend and the existing external
   dependency/runtime contracts before implementation. Scope the modeled API
   explicitly and reject unsupported operations rather than fabricating values.

No ownership-framework redesign is demonstrated by these failures. Native
stream reductions already lower to folds; integer overflow/exception behavior
must be checked independently, not inferred from Rust compilation. Batch the
comparison and entry producer/consumer fixes with minimized executable tests,
then reassess BigInteger runtime integration from generated evidence.

### First focused contract batch

Added `tests/numbers_contracts.rs`. All3 executable tests fail on V168:
`.tmp/numbers-contracts-v168-baseline.log`. The comparison test covers all6
operators, integer division/remainder operands, long promotion and static
uncaught-exception context. Entry tests cover field initializer and static
block construction, checked null dereference, discarded getter result and
retained array-component identity.

Numeric promotion now exposes its operation result primitive separately from
its operand primitive; the uncaught Result wrapper uses that result. Nullable
Map.Entry slot matching now unwraps the optional Java type before applying the
existing tuple-construction policy. Native entry receivers discharge null
before tuple projection, with detached component recovery for borrowed
optional receivers. This reuses the entry carrier rather than a facade body.

First intermediate run `.tmp/numbers-contracts-v169.log` passes the comparison
test but still fails2 entry tests. The discarded getter requires owned
component recovery after borrowing its nullable receiver; that follow-up is
being verified. The array-alias control exposes an additional family:
literal-initialized mutable static arrays are emitted as associated constant
slices, and entry components lack end-to-end nested allocation propagation.
Do not weaken this regression to accept snapshots. Required hypothetical
rewrite stores shared array handles inside the entry carrier and propagates
those handles through construction, static storage and getValue; array writes
must use the same allocation. Audit existing retained-native child constraints
and rendering before implementing that batch. This is broader than the four
initial compiler diagnostics and is not yet fixed.

The local Cargo cache includes num-bigint0.4.6, whose BigInt implementation
provides modpow and modinv. The existing external-pack mechanism already
supports mapped Rust types/methods, path-based Cargo dependencies and exception
conversions. Prefer reusing that integration with a Java-compatible numeric
adapter over introducing a second numeric lowering or dependency mechanism.
No BigInteger runtime acceptance is claimed yet.

### Primes compilation checkpoint

V169b frozen producer `.tmp/new-java2rust-numbers-entry-v169b-debug`, SHA256
`59995be9efa2aa35e83fc9c1b85a0f90264d8184301039ad5fb62b10786d7848`.
Fresh maintained-runner gate
`.tmp/commons-numbers-1.3-primes-v169b-entry`, terminal0, all3 files translate/
parse,0 unsupported/invalid markers,0 compiler errors. Total26.630s; log
`.tmp/numbers-primes-v169b-entry-run.log`. This is not behavioral acceptance.

`.tmp/numbers-contracts-v169b.log` is terminal101: comparison and nullable-entry
tests pass, array-alias test still fails. The latter's unchanged literal-array
initializer exposes a pre-existing const-slice representation problem; it must
not be changed to a different initializer simply to bypass that failure.
Its required storage rewrite remains documented above. No full-library or
preserved-corpus acceptance is claimed for this incomplete batch.

Independent caller `tests/corpus/numbers/PrimesOracle.java` compiles/runs against
all3 untouched production files, terminal0,3594 lines ending oracle=ok:
`.tmp/numbers-primes-java-oracle-v1.log`, output under
`.tmp/commons-numbers-1.3-primes-java-oracle/`. Run with
`bash scripts/run-commons-numbers-1.3-primes-java-oracle.sh`.
It calls all3 public operations over a dense -8..1024 range, explicit int/Miller-
Rabin boundary cases and128 deterministic broad inputs. Direct helper calls
cover mutable factor-list appends, bounded trial division, Miller-Rabin base
thresholds and static table contents; algorithms are not reimplemented.
The matching Rust caller/comparison gate remains to be implemented after the
storage/runtime contracts are complete. Goal remains active for all6 modules.

### Retained storage and initialization checkpoints

V170 removes the invalid associated-constant optimization for final Java
arrays: final freezes the reference, not its allocation. Entry component
allocation constraints now travel through the existing storage graph, nested
slot renderer, constructor and accessor plans. Focused executable contracts
passed5/0 (`.tmp/numbers-storage-contracts-v170.log`); library1626/0
(`.tmp/numbers-storage-library-v170.log`). Frozen producer
`.tmp/new-java2rust-numbers-storage-v170-debug`, SHA256
`09c39141db5e1f2cf04c458cef49587b889375577db6a99a2b26a804a63e2de5`.

Preserved RNG gate at that producer:
`.tmp/commons-rng-1.0-sampling-all-v170-storage` translates/compiles all46 files;
native12601, collections162 and distributions2172 exact observations pass in
`.tmp/rng-{native,collections,distributions}-v170-storage-behavior.log`.
These are V170 checks, not acceptance evidence for subsequent producers.

`runtime/java-math` is an explicit, reusable JDK API adapter over pinned
num-bigint0.4.6, integrated through the existing external-pack path-dependency
and exception-conversion mechanisms. Its manifest models only valueOf(long),
modPow, intValue and longValue; it does not claim the complete BigInteger API.
Arbitrary-width arithmetic, negative exponents/inverses, invalid moduli,
modulus1, and low-two's-complement conversions have3 passing runtime tests.
The maintained Numbers runner now supplies this manifest. No production
Java algorithm is replaced. Use CARGO_NET_OFFLINE=true for nested test crates
in the restricted environment.

V173 materializes static blocks and their interleaved field writes into one
internal canonical callable before resolution. Ordinary binding, storage,
liveness, signature and exception analysis now owns initializer locals;
emission calls that body through the existing initialization guard. Separate
block scopes and textual write ordering have executable regressions. This
does not claim complete JLS class-initialization failure/reentrant-forward-read
semantics: those remain a broader uncertainty beyond the demonstrated fixes.

V173 frozen producer `.tmp/new-java2rust-numbers-class-init-v173-debug`, SHA256
`8520e43b1854ed83c970715b731651c599eb69f1ade52ca497e566922c8db405`:
focused Numbers8/0, mapped BigInteger2/0, library1627/0 in
`.tmp/numbers-{class-init-contracts,big-integer-contracts,class-init-library}-v173.log`.
The actual untouched Primes integration still failed10 compile diagnostics
(`.tmp/commons-numbers-1.3-primes-v173-class-init/compile.log`). This illustrates
why focused passes are not corpus acceptance.

The next whole-path audit grouped these into retained collection iteration/
streams, slot-directed array search, and mapped-call boundary contracts. The
initializer guard also discarded a Result because it queried a coarse identity
rather than the emitter's method exception plan. Corrections reuse the existing
shared-allocation cursor (which already supports local and synchronized
handles), checked array-search projection, method exception plan and external
pack contracts. A typed destination guess was found to shadow a qualified
pack's effects with an unqualified conservative guess; inference must not
replace authoritative external contracts.

V175 Primes now has3 compile errors, all int-to-long mapped factory arguments:
`.tmp/commons-numbers-1.3-primes-v175-call/compile.log`. The named receiver,
array-search and collection traversal failures no longer appear. Mapped tests
pass2/0; Numbers8 pass and the new ninth regression exposes raw Rust division
inside specialized stream lambdas. The next coherent batch reuses the ordinary
Java numeric operator lowerer for wrapping arithmetic and checked division,
and preserves the explicit mapped parameter ABI after context fallbacks.
Do not weaken the failing exception regression or accept a Rust panic as a
Java ArithmeticException.

`tests/corpus/numbers/PrimesOracle.rs` now mirrors the independent Java caller,
calling generated algorithms and helpers only. The existing executable oracle
verifier is extended with `--numbers-primes`; no generated source is patched:

```sh
python3 scripts/verify-commons-rng-1.0-behavior.py --numbers-primes TRANSLATION_ROOT
```

At that checkpoint the comparison had not passed; Primes and all later modules
remained unaccepted. Later acceptance evidence follows below.

### Primes accepted checkpoint V180

The statements above describe their individual checkpoints, not the current
result. V179 first matched all3594 Java observations exactly. That execution
also exposed and eliminated a reachable Integer.numberOfTrailingZeros facade
placeholder: leading/trailing zero counts now share the existing bit-count
intrinsic path, with Integer/Long formal widths. Long.bitCount(int) correctly
sign-extends before counting. Source declarations retain precedence. Boundary
regressions additionally exposed Java's special direct-unary-minus decimal
minimum literals; parser normalization now handles those without accepting
invalid positive or parenthesized oversized magnitudes.

Mapped receiver plans now retain their selected Java owner and method, like
facade plans. Argument lowering consumes that contract rather than trying to
rediscover chained temporary types through a weaker TypeEnv. The BigInteger
adapter declares borrowed read-only exponent/modulus parameters, preserving
loop reuse without copying arbitrary-precision payloads. The stream batch uses
the existing numeric promotion/wrapping/checked-division lowerer, typed
accumulators and fallible traversal. Executable controls cover callback
exceptions, overflow, reentrant mutation/CME, and identity-expression mutation
before late cursor binding. No alternative hand-written production algorithm
is used.

Frozen producer `.tmp/new-java2rust-numbers-primes-v180-debug`, SHA256
`62ed06126522fb5763116815468f2c916f9e866953591d1d426217f127126ad4`.

| Requirement | Current authoritative evidence |
|---|---|
| Complete3-file manifest, untouched upstream pin | prepare runner and `.tmp/commons-numbers-1.3-primes-v180-accepted/summary.tsv` |
| Translate, parse, compile, no unsupported/invalid source placeholders | Same summary:3 files,0 failures/unsupported/invalid/compiler errors |
| Every public operation and package algorithm/helper | Java/Rust oracle callers: isPrime, nextPrime, primeFactors, smallTrialDivision, boundedTrialDivision, trialDivision, millerRabinPrimeTest, tables/wheel |
| Independent exact behavior, errors and state | `.tmp/numbers-primes-v180-behavior.log`:3594 identical observations |
| General regression suite | `.tmp/numbers-final-batch-v180.log`: library1628/0, Numbers12/0, mapped BigInteger2/0 |
| Arbitrary-precision runtime contracts | `.tmp/numbers-big-integer-runtime-v3.log`:3/0 |

The two private empty source constructors are emitted; neither contains a
hidden algorithm. Review of both implementation modules shows real method
bodies, direct runtime modPow/bit intrinsics, typed entry/array operations and
no reachable fabricated facade body. The broad generated shared-support file
still includes unused legacy facade declarations (e.g. IntStream.reduce and
Map.Entry.getValue); their presence is not mistaken for a production call.
The sole Default fallback in implementation storage is guarded Java static
preparation storage, not an algorithm replacement. Static wheel construction
stores its entry before externally reachable access; exact wheel observations
and repeated class use pass.

Additional reproducibility hashes:

| File | SHA256 |
|---|---|
| `runtime/java-math/src/lib.rs` | `84322f5151c96dbe949a630510d4651652c37a761412b64a275ddf662fabf721` |
| `runtime/java-math/manifests/big-integer.facts` | `e8098ba611bc8cfbe64ead90a3070a4fccf101f558922d1f77a769848f794ae7` |
| `runtime/java-math/Cargo.lock` | `d3df0ffbef61fa9bbe180b58f60608e56b74d4e08013882e72b3adc460783a3e` |
| `scripts/focus-sets/commons-numbers-1.3-all.txt` | `2768762a5893028a8de4cc52608cf46643b65ae591097b324f4aac8810d3c651` |

```sh
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" \
  cargo test --lib --test numbers_contracts --test big_integer_contracts --no-fail-fast
cargo test --offline --manifest-path runtime/java-math/Cargo.toml
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-primes-v180-debug" \
  FOCUSED_PACKAGE_RUNNER="$PWD/.tmp/translate-focused-package-pre-runtime-api-v182.sh" \
  NUMBERS_TRANSLATION_ROOT="$PWD/.tmp/commons-numbers-1.3-primes-v180-accepted" \
  bash scripts/run-commons-numbers-1.3.sh
python3 scripts/verify-commons-rng-1.0-behavior.py --numbers-primes \
  .tmp/commons-numbers-1.3-primes-v180-accepted
```

Preserved RNG gates pass with the same frozen V180 producer:

| Scope | Evidence | Result |
|---|---|---|
| Sampling/native46-file translation closure | `.tmp/rng-sampling-all-v180-numbers-run.log` |46 translate/parse/compile,0 unsupported/invalid/errors |
| Native generators | `.tmp/rng-native-v180-numbers-behavior.log` |12601 exact observations |
| Collection sampling | `.tmp/rng-collections-v180-numbers-behavior.log` |162 exact outputs/traces |
| Distribution sampling | `.tmp/rng-distributions-v180-numbers-behavior.log` |2172 exact IEEE values/traces |
| Original source64 11-file closure | `.tmp/rng-source64-v180-numbers-run.log` |11 translate/parse/compile,0 unsupported/invalid/errors |
| Original source64 behavior | `.tmp/rng-source64-v180-original-behavior.log` |2865 exact observations |

The first source64 driver attempt used the combined46-file root and failed to
compile because its test caller expects the original11-file scope's array
ABI. That failed attempt is preserved in
`.tmp/rng-source64-v180-numbers-behavior.log`; no generated code or caller was
patched. The unchanged source64 runner and caller pass on their intended
original scope, freshly translated by V180. The other three callers pass on
the complete46-file closure. No older producer is substituted for a final gate.

Primes is accepted: complete production scope, general fixes, independent
3594-line exact comparison, focused/runtime/library regressions, scoped
reachable-body audit and preserved17800 RNG observations. No numerical
tolerance or source exclusion was used. The other five Numbers modules are
not accepted, so the overall goal remains active. Next is Root Finder, with
its own fresh baseline and callback/exception whole-path audit before fixes.

## Root Finder baseline and rewrite audit

V180 baseline `.tmp/commons-numbers-1.3-rootfinder-v180-baseline`, terminal0,
21.955s (`.tmp/numbers-rootfinder-v180-baseline-run.log`). All3 complete
production files translate, parse and compile with0 unsupported/invalid source
markers. This is not acceptance. Scope is BrentSolver (one public constructor,
both findRoot overloads, private brent and equalsZero), SolverException
(varargs constructor, constants and superclass initialization), package-info;
there are no production dependencies beyond JDK and no scope exclusions.

The whole-path audit has found three related contract families before any
Root Finder fixes:

1. Floating ordering changes control flow. Generated findRoot calls
   Double::compare, whose assembled generic PartialOrd facade treats -0.0 and
   +0.0 as equal and does not order NaN as Java does. A concrete independent
   probe against untouched production algorithms demonstrates a failure:
   solver accuracies (1e-14,1e-14,0), f(x)=1e-200*(x-0.1), interval[0,1].
   Java returns0.1; Rust raises SolverException("No bracketing..."). The
   products underflow to signed zero, so this is branch divergence, not an
   acceptable floating tolerance. Ordinary x*x-2 on[0,2] matches at
   1.414213562373095. Evidence:
   `.tmp/rootfinder-v180-java-probe/output.txt` and
   `.tmp/rootfinder-v180-probe.log`; Java/Rust diagnostic callers live in
   `.tmp/RootFinderProbe.java` and `.tmp/rootfinder-v180-probe.rs`.
   Smallest correct Rust rewrite uses existing java_compare_f64 (and the
   analogous f32 kernel for Float), not generic PartialOrd. This is native
   target/result/argument planning plus support selection, preserving parsed
   source precedence; no new numerical algorithm or corpus-specific branch.
2. Source exception construction is bypassed. Invalid interval[2,1] produces
   Java message "2.0 > 1.0", but Rust emits "%s > %s: 2.0: 1.0". Generated
   SolverException::new returns an empty struct and drops its source
   super(String.format(message, formatArguments)) initialization; throw-new
   synthesizes an unrelated colon-joined argument string. The correct rewrite
   must execute the source constructor's initialization/message expression
   through the existing formatter and preserve the JavaException payload,
   classname and inheritance. Trace construction, superclass storage,
   constructor ABI and throw conversion together; patching these particular
   message templates is not a solution. This is a cross-function construction/
   exception contract issue. Audit existing source-exception representation
   mechanisms before choosing a general implementation.
3. Callback invocation currently uses &mut dyn FnMut(f64)->f64 throughout both
   overloads and brent. This expresses repeated mutation and reborrowing, but
   cannot propagate a callback's Java exception. The existing emitted
   DoubleUnaryOperator runtime already offers shared from_fallible_fn and
   applyAsDouble_result. The minimal semantic rewrite uses a fallible callable
   contract (or that existing SAM carrier), routes every call through Result
   and preserves the same callback state across delegation. Trace producer,
   parameter/forwarding signatures and invocation effects; do not introduce a
   second callback representation or silently convert failure to panic. This
   concern is inferred from the generated ABI, not yet a behavioral pass/fail
   measurement; minimized callback-exception controls are required first.

These rewrites are expressible in Rust, and existing comparison, formatting
and fallible SAM mechanisms are candidates for reuse. There is not yet evidence
requiring a wholesale ownership-framework redesign; source-exception storage
and callback ABI interactions remain explicit uncertainties. No generated
production file was edited for these diagnostic probes. Before fixing this
batch, build maintained independent Java/Rust callers covering both overloads,
all validation messages, endpoints/initial root, both bracketing halves,
bisection/interpolation, tiny/huge intervals, signed zero/subnormals and
callback state/order/failure. Compare numerical outputs as actual IEEE bits;
no tolerance is justified by the observed signed-zero control-flow failure.

### Root Finder progress V181–V183

Maintained independent callers are now
`tests/corpus/numbers/RootFinderOracle.{java,rs}`. Java runner
`scripts/run-commons-numbers-1.3-rootfinder-java-oracle.sh` produces104 lines,
terminal0 (`.tmp/numbers-rootfinder-java-oracle-v1.log`). It exercises both
overloads, endpoint/initial roots, both bracket halves, cosine/cubic callbacks,
underflow/subnormal/signed-zero/huge/NaN/infinite cases, every validation
message, callback failures on calls1/2/3/5, repeated calls and a callback alias.
Roots and every callback input are printed as actual IEEE bit strings. The
infinite-bound callback deliberately throws on call1025; that is an ordinary
stateful callback failure, not a solver shortcut or hidden test timeout.

The comparison command is:

```sh
python3 scripts/verify-commons-rng-1.0-behavior.py --numbers-rootfinder TRANSLATION_ROOT
```

V181 general fixes:

- Static Float.compare/Double.compare and boxed floating compareTo share the
  existing IEEE ordering kernels. Argument planning retains formal width and
  widening; parsed source lookalikes still win. No new comparison algorithm.
- Public/protected, invocation-only inline callback parameters seed the
  existing checked callback contract. Existing graph equality propagates it
  through private forwarders; existing emission/adapters produce borrowed
  FnMut returning Result. The retained/owning callback ABI is not changed.
  Native callback values already have a distinct checked protocol.

V181 frozen producer `.tmp/new-java2rust-numbers-rootfinder-v181-debug`, SHA256
`c101d6aae7bb6678900c57d95367e490dd1e0dfe4f87f7237badca3ea52d023b`.
Focused contracts3/0 (`.tmp/numbers-rootfinder-contracts-v181b.log`); full
library1628/0, BigInteger2/0, Numbers12/0
(`.tmp/numbers-rootfinder-preserved-v181.log`). Primes freshly passes3594 exact
observations at `.tmp/commons-numbers-1.3-primes-v181-preserved`.

Root Finder at V181 compiles all3 files. Its full104-line execution completes:
97 lines match, and all7 differences are exception messages. Every numerical
root and callback count/input trace/failure agrees exactly, including signed
underflow and the1025-call exceptional traversal. Evidence:
`.tmp/commons-numbers-1.3-rootfinder-v181-contract/behavior-numbers-rootfinder/`
and `.tmp/numbers-rootfinder-v181-behavior.log`. This is not acceptance.

The subnormal message also exposed a general display discrepancy: shortest
single-significant-digit Rust output was padded with a decorative zero rather
than choosing Java's closest two-significant-digit scientific form. Diagnostic
Java19.0.2/Rust candidate comparison covers23812 observations spanning the first
512 positive subnormal bit patterns for both widths and signed decimal
digit/exponent ranges. All match after the general precision-selection change:
`.tmp/decimal-probe/{reference.tsv,candidate-result.txt}` and diagnostic callers
`.tmp/{DecimalProbe.java,decimal_candidate.rs}`. These are observations, not a
claim that every IEEE state was enumerated. No source production code is
modified by the probes. The same rule is implemented once in stdlib display
support, with focused signed-zero/NaN/infinity/threshold/subnormal regressions.

V182 focused contracts4/0, but its assembled package still used the old
formatter. Investigation found a duplicate runtime body in
`translate-focused-package.sh` overriding the producer's helper. V183 removes
that fork: the package assembler requests
`emit-runtime-support floating-point-display` from the actual producer. The
library/CLI expose the same stdlib factory used by single-file translation.
The runtime snippet is captured with producer and content hashes; assembly
resumes reuse only a verified matching snapshot, not a replacement validator's
runtime. CLI22/0; producer snapshot/resume/tamper contracts3/0.

V183 producer `.tmp/new-java2rust-numbers-rootfinder-v183-debug`, SHA256
`f7d6da029f712f86d9473f02b2ac59604a32cb9dbffa94dee92de2406f5674b5`.
Matching packager snapshot `.tmp/translate-focused-package-runtime-api-v183.sh`,
SHA256 `b03370c66ed3578c837fea51153ac30f3e711ece9128824340640052162593f9`;
`scripts/focused-provenance.sh` SHA256
`55aa94c87bbf7fe1753897ba467cc3b96861eb7acf04b28278b30d84c5c1030f`.
Runtime snippet hash
`368c75b59ef6e1ad4c09615101f38dc98b9db256c6baae757447262eae682e4f`
is recorded under
`.tmp/commons-numbers-1.3-rootfinder-v183-authoritative-runtime/producer-runtime-floating-point-display.rs.provenance.tsv`.

Older producers do not have the runtime-export command. Their matching older
packager was preserved as
`.tmp/translate-focused-package-pre-runtime-api-v182.sh`, SHA256
`d12fea739e7dac2e468f8a273aefa921bb1a77ba3ba234faa46e2765a3910a8b`.
Set FOCUSED_PACKAGE_RUNNER to that archived path for V180/V181 reproduction;
Numbers and RNG runners support this explicit override. Do not mix an old
producer with a current runtime and call that a reproduced old checkpoint.

V183 Root Finder still has7 message differences, but numeric subnormal
fragments now agree (4.9E-324). Primes freshly passes3594 exact observations at
`.tmp/commons-numbers-1.3-primes-v183-preserved`. The new null-callback regression
also passes, including validation before invocation:
`.tmp/numbers-rootfinder-nullable-callback-v183-baseline.log`. Existing nullable
callback adaptation works; no new null representation was needed.

### Required next construction/varargs batch

Do not replace the seven observed templates directly. A new executable
regression in `tests/rootfinder_contracts.rs`,
`source_exception_construction_runs_super_expression_and_trailing_effects`,
requires evaluation of the source super-message expression, dynamic formatting,
trailing constructor side effects and the caught message. Its baseline fails
at compilation before reaching those assertions:
`.tmp/numbers-rootfinder-exception-state-v182-baseline.log`, E0521.
String.format(pattern, Object[]) incorrectly displays the array itself and
passes its borrowed retained handle to an erased-object display operation
requiring static lifetime. This is another manifestation of the whole
constructor path, not a bad test helper. Keep this regression enabled; at the
pre-V184 checkpoint Root Finder contracts had6 passing cases and this one
known failing case. The newer checkpoints below supersede that test count.

| Boundary | Required general contract / existing mechanism to inspect |
|---|---|
| String.format explicit reference-array varargs | Expand reference elements, not a primitive array passed as one Object; reuse varargs applicability and retained-array iteration/reference-copy mechanisms. Preserve evaluation order, null and callback/reentrancy behavior. |
| Source exception superclass state | Existing ExternalExceptionSuperclass delegation currently drops super arguments. Reuse/generalize embedded external-base storage and ordered constructor lowering to execute those expressions and retain message/cause state. |
| Constructed source exception to throw/catch | Convert the actual constructed runtime state through the existing exception carrier/conversion plan; do not synthesize colon-joined argument text or bypass constructor effects. Preserve actual subtype identity. |

Concrete routing points: `lower_string_format_args` and
`string_format_projection`; `ConstructorDelegationPlan::ExternalExceptionSuperclass`
and its trailing variant; `external_inherited_instance_fields_for_type` /
`embedded_external_super_name_for_type`; constructor field ordering and
`source_external_supertype_conversion_impls_for_type`; finally
`lower_throw_expr_with_refinements`. The embedded-super renderer currently
forces a named external Rust carrier, so Throwable's JavaException carrier
must remain consistent across fields, defaults, construction and conversion.
Check typed varargs, constructor delegation and trailing ordering together.
Stateful custom exception fields/overrides may require richer payload retention;
that is an explicit uncertainty, not permission to discard their state. Root
Finder's actual exception carries superclass message state and static constants,
not custom instance fields. Its minimal correct generated rewrite is bounded
base-state construction/forwarding, not a rewrite of Brent's algorithm.

Root Finder and the other four later modules remain unaccepted; the six-module
goal stays active. V183 preservation gates are now terminal and rechecked:

- `.tmp/numbers-rootfinder-preserved-v183.log`: library1628/0,
  BigInteger2/0, provenance3/0 and Numbers12/0.
- `.tmp/numbers-primes-v183-preserved-behavior.log`:3594 exact observations.
- `.tmp/rng-native-v183-numbers-behavior.log`:12601 exact observations;
  `.tmp/rng-collections-v183-numbers-behavior.log`:162;
  `.tmp/rng-distributions-v183-numbers-behavior.log`:2172;
  `.tmp/rng-source64-v183-original-behavior.log`:2865. Source64 uses its
  original11-file closure and caller, not the different46-file sampling ABI.

### Explicit reference-array formatting batch (V184–V185)

`lower_string_format_array_call` handles the fixed-arity Object[] form of
String.format through both ordinary expression lowering and target-slot
lowering. Both use the same typed adapter. Source parameter declarations use
the existing `source_method_param_type` lookup rather than treating a retained
Rust carrier as evidence of Java varargs applicability. Primitive arrays and
arrays explicitly cast to Object remain single arguments, not expansions.

The shared `java_string_format` parser now delegates argument requests to
`java_string_format_with_arguments`. Ordinary scalar callers retain their
existing Vec<String> API. Explicit arrays supply a fallible, indexed callback:
evaluate template and array once, borrow rather than consume the array, and
display only requested elements. Null Object[] supplies null for each request.
Ordinary typed array indexing/reference-copy machinery releases allocation
guards before display, including callbacks that mutate later arguments.
Nullable erased-element display uses the existing checked object-string
protocol, not an infallible Display bridge. Its optional helper accepts both
owned Option values and borrowed Option views, avoiding an additional borrow
that changes a checked read's required block-tail type.

This does not claim complete java.util.Formatter compatibility: the parser's
other pre-existing format-directive/locale limitations are not acceptance
evidence. Root Finder's production templates use supported sequential `%s`.
Source exception constructors are still not fixed by this formatting change.

V184 frozen producer `.tmp/new-java2rust-numbers-rootfinder-v184-debug`, SHA256
`1341320576f9b946807436e0f97ae32c6488d129a9e694a7e03f33ffdab7ec20`.
All3 Root Finder files translate/parse/compile at
`.tmp/commons-numbers-1.3-rootfinder-v184-varargs`; the independent oracle still
completes104 lines and differs at exactly the same7 exception messages. A full
diff confirms no additional numeric, callback or state differences. Logs:
`.tmp/numbers-rootfinder-v184-varargs-run.log` and
`.tmp/numbers-rootfinder-v184-behavior.log`. The V184 broad regression log
`.tmp/numbers-rootfinder-varargs-v184-final-b.log` confirms library1628/0,
provenance3/0 and Numbers12/0. Focused contracts at that intermediate checkpoint
also expose borrowed/owned optional-array read adaptation errors; V185 fixes
that view contract and must be checked separately, not inferred from V184.

The source-constructor regression now compiles and fails its real message
assertion (return1), rather than failing on the original borrowed-array E0521.
The next required product change remains actual source-constructor execution,
stored superclass exception state and throw conversion, as audited above.

#### Additional static erased-array boundary discovered by reentrancy coverage

The enabled regression
`format_element_display_can_reenter_and_mutate_the_same_argument_array`
uses a static Object[] and fails E0277: generated process-wide storage is
`Mutex<Option<Option<Arc<Mutex<Vec<Option<Box<dyn Any>>>>>>>>`.
`dyn Any` has no Send capability, so wrapping its allocation in Mutex/Arc
cannot make the field valid static storage. Root Finder has no such field.

This is a cross-function representation/capability contract, not a formatting
syntax fix. A valid rewrite must carry proven thread-safe erased payloads
through array producers, fields/arguments, writes and erased-reference clone/
display registries. Adding Send only to the static field leaves those
boundaries inconsistent. Thread-local replacement would violate Java static
sharing; an unsafe Send assertion is not justified. No such workaround was
implemented. Keep the reproducer visible and audit that representation before
expanding this batch into a framework redesign. The companion instance-array
alias test separately exercises reentrancy and throwing display without
requiring unrelated static erased-object storage.

V185 producer `.tmp/new-java2rust-numbers-rootfinder-v185-debug`, SHA256
`37b6396f54cc5774b716c0afb25ffa8f3d7731945a71798d5e167e193d776ab2`.
The focused run `.tmp/numbers-rootfinder-varargs-v185.log` has9 passing tests
and2 enabled known failures: source exception-constructor state and static
erased-array Send storage. Importantly the instance-array alias/reentrant and
fallible-display test passes, as do null/reference-array expansion, repeated
array use and primitive/Object-cast non-expansion checks. No test was ignored
or converted into an expected failure.

Fresh V185 corpus gates:

- Primes `.tmp/commons-numbers-1.3-primes-v185-preserved`: all3 files compile,
  no unsupported/invalid patterns,3594 exact observations
  (`.tmp/numbers-primes-v185-preserved-behavior.log`).
- Root Finder `.tmp/commons-numbers-1.3-rootfinder-v185-varargs`: all3 files
  compile, no unsupported/invalid patterns;104-line oracle still differs at
  exactly the same7 exception messages and nowhere else. Logs
  `.tmp/numbers-rootfinder-v185-varargs-run.log` and
  `.tmp/numbers-rootfinder-v185-behavior.log`.

Reproduce with `NEW_JAVA2RUST_BIN=$PWD/.tmp/new-java2rust-numbers-rootfinder-v185-debug`,
`NEW_JAVA2RUST_BUILD_PROFILE=debug`, `CARGO_NET_OFFLINE=true`,
`TMPDIR=$PWD/.tmp/test-tmp`, `NUMBERS_MODULE=rootfinder` (or primes), and
`NUMBERS_TRANSLATION_ROOT` set to a fresh directory, running
`bash scripts/run-commons-numbers-1.3.sh`. Then use the matching
`--numbers-rootfinder` / `--numbers-primes` verifier mode. The focused runner is
unchanged from V183 (SHA256 b03370c66ed3578c837fea51153ac30f3e711ece9128824340640052162593f9).
Root Finder remains unaccepted; the six-module goal is active.

Final V185 batch `.tmp/numbers-rootfinder-varargs-v185-final.log` is terminal:
library1628/0, provenance3/0, Numbers12/0; Root Finder contracts9/2, with
exactly the two enabled failures identified above. The passing array-format
contract also compiles/runs direct checks of owned and borrowed optional
display views and lazy request failure propagation. `cargo fmt --check` and
`git diff --check` pass. No new RNG rerun was performed for this formatting-only
batch; V183's recorded RNG evidence remains its latest checkpoint, not a V185
claim. The goal is incomplete, not blocked. Next implement source exception
base-state construction and throw conversion before another broad corpus run.

### Source-exception construction batch (V186 onward)

The V185 source-constructor failure is a producer/storage/consumer contract,
not seven independent message substitutions. The implementation extends the
existing embedded platform-base mechanism to throwable carriers:

- `__java_super` is physically JavaException consistently across field
  rendering, initialization and From conversions. Parsed source lookalikes
  still control their own constructors and representation.
- Evaluate the real typed superclass arguments before instance field
  initializers and trailing statements. Replay source-parent construction
  with the existing parameter substitution/ordered replay, carrying its
  external-base state into the child rather than recreating an empty base.
- Throw-new invokes the actual source constructor and its existing fallibility
  contract; throw of an existing source value converts that value's base
  state. Closed/source-hierarchy enums convert by their existing variants.
  Runtime subtype tags survive conversions through source supertypes.
- The shared exception carrier distinguishes a null message from an empty
  message, retains causes, and supplies getCause/instanceof metadata through
  the existing exception hierarchy relation.
- Source constructor calls now reuse resolved varargs packing. A varargs
  pack is a proven fresh allocation and constructs its selected retained
  carrier before ordinary slot adaptation; existing arrays still cannot be
  converted by copying into a new allocation.

V186 diagnostics additionally showed Throwable parameters erased to Any and
same-named external constructor summaries bypassing source precedence. The
typed carrier fix belongs in shape projection's provenance-aware native-value
classification. Marking Throwable as a generic platform *facade* instead
clobbers source-hierarchy representations via
`keep_source_platform_facade_subclasses_concrete`; a read-only representation
probe `.tmp/exception_shape_probe.rs` / `.log` established that failure across
IllegalArgumentException, RuntimeException and java.lang.RuntimeException.
The final approach does not change that facade classification. Constructor
source-owner lookup checks the source type spelling as well as a resolved
summary owner, using the existing owner-key mechanism.

Executable regressions require: super-message/trailing effects; superclass,
field and body ordering; stopping before fields/body when super throws; null
and empty messages; causes; this/super delegation; catch subtype metadata;
source-upcast throwing; source lookalike constructors; plus all prior callback
and formatting tests. V187b has14 passing tests and2 failures (the separate
static erased-array reproducer and the upcast/source-precedence regression).
V188 addresses the latter contracts and must be measured separately.

This is not a claim that a base-state-only runtime preserves arbitrary custom
exception fields/virtual overrides or shared exception identity after catch.
Those richer payload requirements remain a framework boundary needing their
own producer/storage/dispatch audit. Root Finder's source exception has only
static constants and inherited message/cause state; no production Java or
generated algorithm has been patched.

### V189–V191 whole-package verification and result-domain repair

V189 producer SHA256
`508d1b1435a19ea51401830a7b49b25de91ab330b1799cebc4c42eb4b32d32a1`:
15 focused contracts pass; the only failure is the enabled, separately audited
static erased-array Send reproducer. Source precedence, source upcasting,
same-class/source-super delegation, null messages, causes and construction
effects all execute successfully. Full library1628/0, BigInteger2/0 and
provenance3/0 passed at this producer.

Whole-package assembly initially failed on Option<String> Display because
`translate-focused-package.sh` had another stale runtime fork: a hardcoded
JavaException struct, inherent methods and Display implementation. The shared
merger already promotes authoritative producer declarations and methods, but
the hardcoded trait implementation won its coherence key. Removed all three
exception-carrier fragments, not just the failing format expression. Existing
producer support now supplies fields, methods and Display together, including
source exception hierarchy facts. Unrelated prelude scaffolding is unchanged.
Resuming assembly of the already translated V189 files then passes all3
files and104 exact oracle observations; Primes passes3594 again. Logs:
`.tmp/numbers-rootfinder-v189-authoritative-exception-assembly.log`,
`.tmp/numbers-rootfinder-v189-authoritative-behavior.log`,
`.tmp/numbers-primes-v189-preserved-behavior.log`.

The assembler is frozen at
`.tmp/translate-focused-package-exception-authority-v190.sh`, SHA256
`589bf2d1087149f94ccecf023b8c7306f1ecf4fd3dc9952103749a65e0663b57`.
The old b03370... assembler remains archived at the V183 path above for older
producer reproduction. `scripts/test_shared_runtime_registries.py` now has5
passing checks, including exception state/Display promotion and a guard
against reintroducing a package-runner exception-carrier fork:
`.tmp/numbers-exception-runtime-authority-tests-v190.log`.

Adding legitimate From conversions into JavaException exposed Rust inference
that had accidentally relied on having only the reflexive From impl:
`Ok(value)?` at a block/conditional boundary does not identify its error type.
V190 pins JavaException in the existing native checked-success projection,
fixing the executable retained-array search regression (Numbers12/0), not
removing the new conversions. Its only library failure was the nCopies
emission snapshot expecting an untyped Ok; both exact snapshots now require
the typed error domain, and the full rerun is1628/0:
`.tmp/numbers-exception-batch-library-v190-snapshot-fixed.log`.

V190 producer `.tmp/new-java2rust-numbers-rootfinder-v190-debug`, SHA256
`0ac86e72aada452ecb22538f5803bfe5fba380a05bbb2e6d3db6dc8c5258de2d`.
Root Finder again compiles all3 files with104 exact observations; Primes all3
with3594. Original11-file RNG source64 passes2865 exact observations. The
46-file RNG preservation build exposed the same inference problem in two
PoissonSampler ternaries, one nullable and one primitive (4 E0282/E0283
diagnostics). This is a result-domain contract, not a sampling algorithm
rewrite or a numerical discrepancy.

V191 routes both conditional arms through that same checked-success producer.
The focused nullable/primitive ternary regression checks both arms and their
call counts. Frozen producer `.tmp/new-java2rust-numbers-rootfinder-v191-debug`,
SHA256 `a74e840cb5593123f6fa56176f53e4fd371668b65e95b75b2c1f5616443c591a`.
Its broad and preservation gates must be recorded when terminal, not inferred
from V190. No Java corpus file or generated deliverable was manually edited.

### Root Finder accepted at V191 (2/6 modules)

All V191 gates are terminal. Untouched source pin remains
`ae8abefb51c2c4d1a05882d61cefc8015e493a8f` with a clean checkout. Root Finder's
complete3-file production manifest (BrentSolver, SolverException and
package-info) translates, parses and compiles with0 unsupported statements,
members or expressions,0 invalid-pattern files and0 Rust errors:
`.tmp/commons-numbers-1.3-rootfinder-v191-accepted-candidate` and
`.tmp/numbers-rootfinder-v191-accepted-candidate-run.log`.

The independent oracle has been expanded, not relaxed:122 exact observations
now include distinct relative/absolute tolerances, zero relative tolerance,
and positive function-value tolerance accepting an initial point or either
endpoint. All previous104 observations remain covered. This separately
exercises the constructor's three fields rather than relying on identical
relative/absolute defaults. Java/generated Rust match exactly:
`.tmp/numbers-rootfinder-v191-expanded-behavior.log`. No numerical tolerance
or expected-value substitution was introduced.

| Scoped operation | Acceptance evidence |
|---|---|
| BrentSolver constructor(double,double,double) | Default and six distinct accuracy cases, including positive function-value tolerance at all three early-return sites. |
| Both findRoot overloads | Independent Java/Rust calls, several initial guesses, endpoints/initial roots, both bracket halves, invalid intervals/estimates, NaN/infinity, signed zero, underflow/subnormal inputs and repeated solver/callback use. |
| Private brent | Roots and complete callback-input IEEE traces for square root, cosine, cubic and scaled functions; callback failure on calls1/2/3/5 through both public overloads. |
| Private equalsZero | Real call paths plus focused static/boxed IEEE comparison matrices, including signed zero, NaNs and subnormal products. |
| SolverException(String,Object...) | All three original format templates and seven validation outcomes execute its generated constructor. Super-message evaluation/state is visible in matched messages, not recreated from throw arguments. |
| Constants and metadata | Source message constants and serialVersionUID are retained; package-info remains in the full census. No nested production types or extra production dependencies exist. |

Generated scoped bodies were inspected: BrentSolver's constructor, both
overloads, brent and equalsZero have real bodies; SolverException's constructor
executes String.format and stores its JavaException superclass state. Generic
prelude SAM/facade scaffolding is not used as a replacement algorithm or
reachable default callback in these paths. `noop_facade_call_sites=0`.

Preservation at the same V191 producer and assembler:

- Primes all3 files compile and3594 exact observations pass:
  `.tmp/commons-numbers-1.3-primes-v191-preserved`,
  `.tmp/numbers-primes-v191-preserved-behavior.log`.
- Full library1628/0, BigInteger2/0, provenance3/0, Numbers12/0:
  `.tmp/numbers-rootfinder-exceptions-v191-final.log`.
- Focused Root Finder/contract tests16 pass; the sole enabled failure is the
  separately recorded static erased-array Send reproducer, outside Root
  Finder's production scope. It has not been ignored, hidden, or asserted as
  an expected failure. This is not a claim that the entire test target is
  green or that arbitrary stateful custom exception payloads are solved.
- All46 RNG sampling/native files compile with0 unsupported/invalid/errors:
  `.tmp/rng-sampling-all-v191-numbers-run.log`. Native12601 exact observations,
  collections162 and distributions2172:
  `.tmp/rng-native-v191-numbers-behavior.log`,
  `.tmp/rng-collections-v191-numbers-behavior.log`,
  `.tmp/rng-distributions-v191-numbers-behavior.log`.
- Original11-file source64 closure separately compiles and passes2865 exact
  observations: `.tmp/rng-source64-v191-numbers-run.log`,
  `.tmp/rng-source64-v191-original-behavior.log`. Total RNG17800 observations.
- Assembly runtime-authority checks5/0, `cargo fmt --check` and
  `git diff --check` pass.

Frozen V191 producer SHA256:
`a74e840cb5593123f6fa56176f53e4fd371668b65e95b75b2c1f5616443c591a`.
Matching assembler SHA256 remains589bf2d1087149f94ccecf023b8c7306f1ecf4fd3dc9952103749a65e0663b57.
Expanded oracle Java/Rust SHA256 respectively:
`3af03eada02f11258bf4320da991fa6ba2a90de6729f68f82d9a696ea14bcd80`,
`efe50eea1ce483f73713b2b40026b655e120fe44092262edd1284e0244cc84ca`.
Verifier SHA256:
`51a28ad25787cb3461e1672088b0fe477a72ee604eef194372b838fa67e32925`.

Reproduce by replacing V185's producer in the command recipe above with
`.tmp/new-java2rust-numbers-rootfinder-v191-debug` and using the archived
V190 exception-authority assembler. Choose fresh per-module output roots.
The verifier's Root Finder mode now requires122 lines. No source modules,
methods, helpers or overloads were removed from scope. Root Finder acceptance
does not complete the six-module goal; next is the complete Core module,
starting with its baseline and generated-code rewrite audit.

## Core V191 baseline and entry rewrite audit (not accepted)

After Root Finder acceptance, ran the full11-file Core manifest with the same
frozen V191 producer and matching assembler:
`.tmp/commons-numbers-1.3-core-v191-baseline`,
`.tmp/numbers-core-v191-baseline-run.log`. All11 files are exercised, including
metadata. Runtime116.107s,0 parse failures,0 unsupported syntax counters,
1 invalid-pattern/translation-failure file (Precision's unmodeled RoundingMode
constant) and65 generated-Rust compiler diagnostics. These are not65
independent design problems. No Core fixes have been made yet.

Source manifest: Addition, ArithmeticUtils, DD, DDMath, ExtendedPrecision,
Multiplication, NativeOperators, Norm, Precision, Sum and package-info.
Production dependencies remain JDK-only. The JDK surface includes
java.math.BigInteger, BigDecimal and RoundingMode: existing java-math support
does not yet model all methods/constants these sources use. No additional
Commons Numbers production module is needed or silently omitted. Full26-file
manifest SHA256:
`2768762a5893028a8de4cc52608cf46643b65ae591097b324f4aac8810d3c651`.

Representative generated rewrites / producer-to-consumer audit:

| Shared family | Concrete generated evidence and smallest correct contract | Classification |
|---|---|---|
| Generic interface equality defaults | Addition's `is_zero` emits `self == self.zero()` with unrelated `&Self` and T and no Java equality contract (12 E0369 diagnostics across default/adapter surfaces). Reuse Java object equality dispatch, including source equals overrides; adding PartialEq mechanically is not a semantic solution. Carry the required equality capability through generic trait signatures and forwarding adapters. | Cross-function capability contract; audit existing Java object/erased equality machinery before choosing bounds. |
| Stateful source enum | Norm methods read `self.two`, `self.three`, `self.array`, but its Rust enum has no such storage; its scaling constants are also absent (14 SCALE_DOWN,6 SCALE_UP and several related diagnostics). Java variants contain two/three/array callback payloads and aliases MANHATTAN(L1), EUCLIDEAN(L2), MAXIMUM(LINF). Valid Rust must retain per-variant initialized payloads/alias identity and their source constructor effects, plus associated static constants. Reuse source enum/object/initializer planning; do not replace norm algorithms with hand-written functions or map names to numeric algorithms. | Source enum storage/construction/consumer contract, not just missing constants. |
| Nested interface capture/binding | Precision's anonymous DoubleEquivalence.compare emits `Self::compare_to_double_double_double(a,b,self.epsilon.get())`; Self is the anonymous implementor, not lexical Precision. Minimal rewrite calls the selected lexical static owner and retains epsilon in its existing capture cell. Existing anonymous-object and lexical-owner mechanisms can express this. | Local owner routing with cross-surface checks (raw and Rc adapters). |
| Primitive native constants and callable values | DD sees Double.MAX_EXPONENT / MIN_EXPONENT as nominal empty structs, arithmetic overflows and Math.floor/ceil method references lack native callable targets. Required values/types must come from authoritative JDK constant/operator plans, and method references must reuse native callback adaptation rather than add algorithm-specific callables. | Local typed constant/native-call planning; audit all widths/related references as a batch. |
| Borrowed retained output arrays | DD frexp expects owned Rc<RefCell<Vec<i32>>> but a composed caller passes &Rc. Minimal Rust repair is a reference-handle copy/reborrow according to the selected signature, not a copied Vec or a removed Java alias. Trace source argument plan, companion signature and array producer together. | Cross-function signature/storage contract; current framework has the required handles. |
| JDK arbitrary precision | BigInteger pow/multiply/compareTo/testBit/shiftRight and ONE/ZERO are missing from the existing adapter. BigDecimal is worse: generated `pub struct BigDecimal;`, a default/no-op constructor and unimplemented arithmetic/rounding bodies are reachable from DD.from, DD.bigDecimalValue and Precision.round. Reuse num-bigint in runtime/java-math and its mapped facts/checked-error adapters. BigDecimal needs real decimal state, exact double-constructor conversion, arithmetic, scaling/rounding modes and double conversion—not a decimal stub, f64-only substitute or a skipped API. | General JDK runtime extension plus typed external binding/constant plans. |

The hardest ownership paths already have useful, semantically appropriate
structure. Generated Sum uses Rc<RefCell<Sum>> receiver companions, returns the
same handle for chaining, stages scalar reads from `other` in guard-scoped
blocks, and computes compound-assignment RHS before borrowing self mutably.
For example add(Sum) reads sum/comp before invoking its mutating helper; the
small hypothetical repair often imagined for sum.add(sum) is already present.
Keep and test self-aliasing, distinct aliases, array inputs and supplier/
consumer use; do not introduce a second object representation prematurely.

DD currently uses Arc<Mutex<DD>> because static ONE/ZERO share that source
type's identity requirements; it and DDMath transfer exponent arrays using
retained handles. Immutable DD arithmetic appears expressible by existing
receiver companions, scoped reads and new-value construction. The baseline
does not establish behavioral correctness or absence of nested-lock hazards.
Inspect complete representative operations (including abs returning this,
frexp/pow output arrays and captured floor/ceil callbacks) and execute alias/
state probes before declaring a rewrite unnecessary. No generated diagnostic
rewrite is a deliverable.

Norm's alias-style constructors copy callback payloads, not enum identity:
MANHATTAN and L1 (likewise the other pairs) remain distinct Java enum objects
with distinct names/ordinals. A representation that maps both constants to
one discriminant would be incorrect even if all numeric norms matched.

Proposed coherent order: (1) typed JDK scalar constants/native references,
lexical anonymous binding and retained-array argument handoff; (2) generic
equality and stateful source-enum storage using their existing authoritative
mechanisms; (3) extend general java-math BigInteger/BigDecimal/RoundingMode
support; then independent full-module callers and IEEE/discrete verification.
Choose the exact grouping from focused reproductions and complete contracts,
not from diagnostic count alone. BigDecimal and Norm are real scope
requirements, not reasons to narrow Core. Root Finder/Primes/RNG remain the
preservation gates. The six-module goal stays active; no blocker is claimed.

## Core first shared-boundary batch (V195–V197; not accepted)

Implemented the first audited family as translator fixes, without changing
pinned Java or generated deliverables:

- `stdlib` now supplies Java int exponent constants and correctly typed
  Float/Double MIN_NORMAL values through the existing constant registries.
- Primitive static field and associated-constant initialization reuse numeric
  promotion/wrapping operations and destination-slot assignment conversion.
  This covers overflow, negation, masked shift counts, and int-to-double
  initialization rather than substituting Rust arithmetic or raw literals.
- Native static method references become typed ordinary calls using the
  existing callback contract and native call planner, including qualified
  owners, overload input types and source-shadow precedence. No Math facade
  implementations or corpus-specific function mappings were added.
- Anonymous object methods retain lexical source ownership separately from
  their physical Rust Self. Resolved, fallback and already-selected StaticSelf
  call paths must all route enclosing static calls to the lexical owner.
- Final argument lowering reuses the handle-aware companion's selected
  emitted signature before specialization. This prevents retained output
  arrays from reverting to provisional borrowed Rc/Arc parameters. Regression
  callers verify writes through aliases, repeated companion calls, fresh
  arrays and temporary receivers; they never clone Vec payloads to repair ABI.

V195 frozen producer:
`.tmp/new-java2rust-numbers-core-v195-debug`, SHA256
`5cbbcb8b5ff68e37bbde168ebb08fe6ecdab3e0cbc126d6d9d8702e2c77325f8`.
Complete Core rerun `.tmp/commons-numbers-1.3-core-v195-first-batch`, log
`.tmp/numbers-core-v195-first-batch-run.log`: all11 files exercised,
0 parse/unsupported syntax failures,1 invalid file (RoundingMode),
**56 compiler diagnostics versus65 at V191**. Constants, Math references,
constant overflow and output-array signature failures are absent. Two
anonymous lexical-static failures remained despite the original small
executable regression passing; that exposed additional fallback routing,
not a reason to accept the module or suppress those calls.

V195 checks:1628 library tests and17 Numbers contracts passed
(`.tmp/numbers-core-first-batch-tests-v195.log`). Primes3594 and Root Finder122
exact observations remained passing in fresh V195 translations. Original
source64's2865 observations also passed. The46-file RNG preservation run
correctly caught a new constant-initialization regression: PoissonSampler's
`static final double PIVOT = 40` became an integer Rust literal. The V197
follow-up restores destination-slot assignment conversion for both static
storage and constants and extends the executable numeric regression to
double-from-int, widened arithmetic and byte narrowing. Do not cite V195 as
a passing complete RNG checkpoint.

V197 also routes StaticSelf without requiring a second resolver/fallback
signature and tests anonymous fallback selection directly. The maintained
anonymous executable fixture uses both standalone and batch entry points,
nested source interfaces, an explicit captured field, default forwarding and
overloaded enclosing statics. The full V197 Core rerun confirms both generated
anonymous adapter surfaces now call `Precision::compare_to_double_double_double`,
not Rust Self.

V197 frozen producer `.tmp/new-java2rust-numbers-core-v197-debug`, SHA256
`b3326b6d5a8918c621ef73c8a4f7c8fb9323f07c72a01cd9dcc16e2d8f69c815`.
The matching assembler remains the archived V190 authority version, SHA256
`589bf2d1087149f94ccecf023b8c7306f1ecf4fd3dc9952103749a65e0663b57`.
V197 library1629/0 and Numbers17/0 pass, including the explicit no-resolver
static-owner regression and both anonymous execution entry points:
`.tmp/numbers-core-first-batch-tests-v197.log`. Fresh Primes3594 and Root
Finder122 exact comparisons pass in `*-v197-preserved` translation roots,
with logs `.tmp/numbers-primes-v197-preserved-behavior.log` and
`.tmp/numbers-rootfinder-v197-preserved-behavior.log`. Original source64's2865
exact observations pass in `.tmp/rng-source64-v197-numbers-behavior.log`.
These gates do not imply Core acceptance or resolve the separately documented
enabled static Object[]/Send limitation.

Complete V197 Core root:
`.tmp/commons-numbers-1.3-core-v197-first-batch`, log
`.tmp/numbers-core-v197-first-batch-run.log`. All11 files exercised;
0 parse failures and0 unsupported syntax counters;1 invalid-pattern file and
**54 compiler diagnostics** remain (65 at V191). All eleven diagnostics
targeted by this batch are absent. The remaining grouped diagnostics are30
for source-enum state/constants,12 for generic Java equality, and12 for the
JDK arbitrary-precision/rounding surface. Counts describe compiler reports,
not independent remaining bugs or evidence of behavioral correctness.
Core still has6 no-op facade call sites and reachable unimplemented decimal
support; none is accepted or excluded. Next is the audited source enum and
generic equality family, followed by real java-math extensions and complete
Core behavioral oracles. Production sources remain clean at the exact pin.

V197 final RNG preservation is terminal and passing: all46 files translate,
parse and compile with0 unsupported syntax,0 invalid files and0 Rust errors
in `.tmp/commons-rng-1.0-sampling-all-v197-numbers`; run log
`.tmp/rng-sampling-all-v197-numbers-run.log`. Independent native12601,
collections162 and distributions2172 observations match exactly in
`.tmp/rng-native-v197-numbers-behavior.log`,
`.tmp/rng-collections-v197-numbers-behavior.log` and
`.tmp/rng-distributions-v197-numbers-behavior.log`. Together with the separately
translated original11-file source64 closure's2865 observations, all17800 RNG
observations pass. The PIVOT regression is fixed through typed initialization,
not a changed oracle or generated-code patch. `cargo fmt --check` and
`git diff --check` pass. This closes the first Core implementation batch;
the six-module goal remains active, with only Primes and Root Finder accepted.

## Enum / equality follow-up audit and shared constants (V198)

Revalidated the V197 remaining failures before choosing implementation.
The enum branch in `lower_type` explicitly emitted an empty associated-const
list, while ordinary classes already use
`item_assembly::associated_consts_for_type`. V198 exposes that existing helper
within the crate and uses it for source enums, including enums without any
source methods. It retains the same typed initializer overrides, wrapping
arithmetic, visibility and field-name planning. No Norm constants or numeric
algorithms are duplicated in enum-specific code.

Executable regression `source_enum_constants_share_typed_class_constant_assembly`
covers a private hexadecimal scaling constant invoked through instance
methods, exact independent IEEE bit expectations, wrapping integer constants,
widened double initialization, Double.MIN_NORMAL, a mutable static counter,
an enum with only constants, names, values, distinct identities and ordinals.
An initial assertion used Math.scalb, an unmodeled native API; the test now
checks independent expected IEEE bits directly. Core's occurrences of
Math.scalb are documentation references, not a new production dependency.
No production API was skipped or acceptance oracle relaxed.

The remaining state issue is not an accessor spelling fix. Existing
`java_enum_payload_accessor_methods` selects the first constructor, evaluates
a limited immutable payload slice or generates default-initialized per-field
state for fields assigned in methods. `class_initialization::materialize`
skips enum constant construction. This cannot express Norm's two constructor
overloads and stored callback aliases with actual initialization effects.
Recreating callbacks at each getter read would change reference identity.

A separate untouched diagnostic source fixture is recorded in
`.tmp/numbers-enum-state-audit/EnumStateProbe.java`. Java compiles and its
independent self-check returns0
(`javac -d .tmp/numbers-enum-state-audit .tmp/numbers-enum-state-audit/EnumStateProbe.java`
then `java -cp .tmp/numbers-enum-state-audit EnumStateProbe`). It checks
constructor event order13423, captured callback values, alias callback
identity versus distinct enum identity, constructor-time ordinal metadata,
per-constant mutable counters, copied enum references and no repeated
initialization. V198's untouched generated output fails rustc with13 missing
field diagnostics. Artifacts: `generated.rs`, `translation.log`, `compile.log`
and `java-oracle.log` in that same directory. It also exposes increment-style
state writes missing from the current assignment-only enum state detector;
this is recorded, not silently counted as solved by the static-constant fix.

Smallest semantically correct enum-state contract: retain unit discriminants
for identity/name/ordinal, but give each constant one initialized payload
allocation. Select the real source constructor for each constant and lower
its arguments, ordered field initialization and body using the existing
constructor and callback contracts. Alias construction copies Java callback
handles, not enum discriminants or regenerated callback bodies. Reuse the
process-wide class-initialization barrier and same-thread reentrancy policy;
do not hold a state mutex while invoking Java callbacks. Constructor `this`
needs metadata/storage before the constant's static reference is published.
Static constant reads and values() also need their proper initialization and
publication boundary, including null/default state on reentrant early reads.
This is a constructor/storage/initialization/consumer extension, not a
per-algorithm mapping. Full effects, callback identity and partial publication
remain to be implemented and tested; no new enum representation is claimed
complete by V198.

The generic-equality audit likewise rules out a superficial Rust-bound fix.
Addition.isZero and Multiplication.isOne call Object.equals between `this`
and unrelated generic T. Existing `java_value_equals<T: JavaMapKey>` requires
the same Rust type on both sides; `java_erased_any_equals` handles primitive
carriers but has no source-object virtual equality registry. Merely borrowing
T or calling that primitive-only helper would replace a compiler error with
an unsupported runtime target. DD overrides the predicates, but that does not
authorize dropping the scoped source interface defaults/helpers.

The correct hypothetical interface rewrite erases the canonical source
receiver through the existing as_any protocol, preserves ordered argument
evaluation, and dispatches checked source equals(Object) via the same exact
method/companion ABI used by source Object protocols. Default identity must
reuse the canonical reference-identity mechanism. Heterogeneous arguments,
null, inherited overrides, throwing equals and live aliasing are required
regressions; a same-carrier-only comparison or blanket PartialEq/Clone bounds
are not equivalent. Extend the existing Object protocol/erasure registry and
capability propagation rather than introduce a second equality algorithm.
This remains a cross-function protocol contract, not a local equality token
replacement. These findings refine the next batch without narrowing Core.

V198 terminal verification:

- Frozen producer `.tmp/new-java2rust-numbers-core-v198-debug`, SHA256
  `bb32fcfe55c5c12276f82bcda66d1b64d75b7c733149ea4a175af36a56b5267d`.
  Matching assembler and26-file source-manifest hashes remain V197's values.
- `.tmp/numbers-core-enum-constants-v198-final-tests.log`:1629 library tests
  and18 Numbers contracts pass. `cargo fmt --check` and `git diff --check`
  pass. The earlier V198 test log's Math.scalb failure is superseded only for
  that test assertion, not treated as a newly implemented native API.
- Full11-file Core rerun
  `.tmp/commons-numbers-1.3-core-v198-enum-constants`, log
  `.tmp/numbers-core-v198-enum-constants-run.log`, terminal1: **27 compiler
  diagnostics, down from54 at V197 and65 at V191**.0 parse failures,
  0 unsupported syntax counters,1 invalid-pattern file (RoundingMode).
  All27 missing enum scaling/static-constant diagnostics are gone.
- Remaining diagnostics:12 generic equality,3 missing Norm callback fields,
  10 BigInteger adapter methods/constants,1 BigDecimal result surface and
  1 RoundingMode constant. Runtime decimal/enum initialization obligations
  are broader than these compiler counts. Core is still unaccepted.
- Diagnostic enum-state source SHA256
  `02b4880b3501ba232fa19eab36913136250d71d86a7fe747ffecb9d0fa016d6c`.
  The fixture is feasibility evidence, not an accepted translation or a
  production Java edit.
- Latest full preservation remains V197: Primes3594, Root Finder122 and
  RNG17800 exact observations. V198 changes only the source JavaEnum branch
  and shared-helper visibility; the46-file RNG closure contains no source
  enum declarations. Do not relabel V197 oracle logs as fresh V198 runs.

The full six-module goal stays active. Next implementation must preserve the
recorded constructor/storage and Object-protocol contracts; do not replace
missing state with per-read callback regeneration, fake defaults, omitted
interface helpers or corpus-specific enum switches.

## BigInteger runtime and binding batch (V199–V207; not accepted)

The next shared batch extends the existing `java-math-runtime` BigInt-backed
carrier rather than adding Core-specific arithmetic. It implements multiply,
integer power, signed comparison, bit testing, signed right shifts and decimal
display, together with modeled ZERO/ONE value expressions. Negative powers and
bit indices, sign extension, negative shift distances and extreme int arguments
retain the JDK exception contract. The independent runtime tests pass6/0:
`.tmp/numbers-big-integer-v199-runtime-tests.log`.

The unchanged Java API probe `tests/corpus/numbers/BigIntegerOracle.java` supplies
542 independent observations, including large values, repeated operand use,
exceptions, static imports, a stored imported value and lexical shadowing. This
probe is not a replacement implementation of Core algorithms. At V201 it does
**not** compile after translation: bare imported ONE was inventoried as an
opaque type, and SAVED_ONE received erased storage. The other3 BigInteger
contract tests and1630 library tests passed. Numbers tests were not reached
by that failing combined command; do not report them as freshly passing.

V201 frozen producer:
`.tmp/new-java2rust-numbers-big-integer-v201-debug`, SHA256
`bbc5b004d14b36c3752bcb834a4b2fc2d159be71ff71d9bac323b443f4ec759b`.
The exact runtime inputs are archived at `.tmp/java-math-runtime-v201`.
The full11-file Core closure at
`.tmp/commons-numbers-1.3-core-v201-big-integer` fails with34 diagnostics:
12 generic-equality,3 Norm callback fields,1 BigDecimal,1 RoundingMode,
9 unqualified mapped return types inside generated Result surfaces, and8
mapped instance calls emitted without their receiver. This is a regression
from V198's27 diagnostics, not an accepted runtime/binding checkpoint.
Evidence: `.tmp/numbers-core-v201-big-integer-run.log` and
`.tmp/numbers-big-integer-v201-batch-tests.log`.

Fresh V201 preservation checks nevertheless pass: Primes3594 observations,
Root Finder122, and Commons RNG17800 across the native, collections,
distributions and original11-file source64 callers. Logs:
`.tmp/numbers-primes-v201-preserved-behavior.log`,
`.tmp/numbers-rootfinder-v201-preserved-behavior.log`,
`.tmp/rng-native-v201-numbers-behavior.log`,
`.tmp/rng-collections-v201-numbers-behavior.log`,
`.tmp/rng-distributions-v201-numbers-behavior.log`,
`.tmp/rng-source64-v201-numbers-behavior.log`.

### Binding rewrite audit and V202 candidate

The needed Rust rewrite is ordinary mapped free-function dispatch preserving
the Java instance receiver (`java_math_runtime::pow(k, e)`), with canonical
crate-qualified success types through Result wrappers. No new arithmetic or
Core-specific rewrite is justified. A minimal source class with a static pow
helper calling `k.pow(e)` reproduces the receiver loss independently of Core.
Tracing facts through semantic analysis shows identical library-pack records
are merged multiple times. The simple-owner fallback counted records, not
distinct pack declarations, disabling the mapped-call path. The V202 candidate
treats identical evidence as one declaration while retaining ambiguity for
conflicting mappings. Its focused fact regression and generated executable
same-name-helper regression both pass. Evidence:
`.tmp/numbers-big-integer-pack-lookup-tests.log` and
`.tmp/numbers-big-integer-receiver-binding-tests.log`.

The same candidate recursively qualifies Result success payloads without
rebinding the exception ABI, including late erased-facade return upgrades.
Bare imported value shape inference now delegates to the existing qualified
member planner after local/field shadow checks, matching final value lowering.
New focused regressions cover these contracts. The V202 batch log
`.tmp/numbers-big-integer-v202-batch-tests.log` proves1633 library tests and4
BigInteger contract tests pass, but the542-observation oracle still fails
generated compilation with4 errors: opaque ONE/local-name collision and erased
SAVED_ONE storage (including its missing Send/Clone capabilities). A small
probe without a bare imported receiver compiles to concrete BigInteger storage;
that weaker probe did not establish the complete binding contract.

V203 additionally guards lexical static declarations against imported mapped
values. All1634 library tests pass, but expanded source-shadow coverage exposes
another boundary: `import static p.Type.VALUE` incorrectly introduces Type as
a simple-name type alias and captures an unrelated source declaration.

The V204 candidate addresses these shared producer errors before downstream
inference: resolution binds proven explicit imported value fields to qualified
field accesses, after local/parameter/field shadow checks. Ambiguous imports
remain unresolved and imports of methods or nested types are not mistaken for
value fields. Lowering no longer imports a declaring type's simple name merely
because one of its members was imported. This retains actual Java name scope
rather than relying on late facade suppression or erased-storage repairs.
V204/V205 library gates each pass1634 existing tests but fail the new import
resolution regression: constant-style parameter/local references retain Java
spelling while the resolver environment stores normalized binding names. The
first import implementation consequently captured real shadowing bindings.
This is a product bug, not an assertion to remove. The V206 candidate checks
both source and normalized binding spellings before qualifying an import, as
the existing lowering binding lookup does. It also retains external field
facts when recording unresolved receivers; dropping them at that boundary
would recreate an unknown VALUE receiver even after qualification.
Focused verification passes in
`.tmp/numbers-import-resolution-v206-focused.log`. The exact V206 compiled
library test binary passes1635/0 in
`.tmp/numbers-big-integer-v206-library-tests.log`; the separate Numbers target
passes18/0 in `.tmp/numbers-v206-contract-tests.log`. Four of5 BigInteger
contract tests pass, including the expanded source/static-import shadow and
same-name-helper executable tests. The542-observation probe still fails
generated compilation with8 errors:4 unqualified JavaMathBigInteger erased
display downcasts,2 opaque ONE/local-binding collision errors, and2 erased
SAVED_ONE Send/Clone errors. Preserve the failure in
`.tmp/numbers-big-integer-v206-integration-tests.log`; no generated comparison
has passed yet.

V206 producer `.tmp/new-java2rust-numbers-big-integer-v206-debug`, SHA256
`65e43f936fdd0f0d1ce4ca45963e274451770ff0c29c10cc921300d9c634d9b3`,
freshly translates all11 Core files but still fails with25 diagnostics:
12 equality,3 enum fields,1 BigDecimal,1 RoundingMode and8 dropped instance
receivers. The9 unqualified Result payload errors are eliminated; the
receiver-loss family is not. Evidence:
`.tmp/numbers-core-v206-big-integer-run.log` and its generated-crate artifacts.
Fresh V206 Primes3594 and Root Finder122 observations pass exactly in
`.tmp/numbers-primes-v206-preserved-behavior.log` and
`.tmp/numbers-rootfinder-v206-preserved-behavior.log`. RNG evidence remains
V201, not relabeled as a fresh V206 run.

### V207 follow-up producer audit

The public diagnostic probe
`.tmp/numbers-big-integer-resolution-probe-v206-core.log`, using real
ArithmeticUtils source plus the production stdlib fact inputs, identifies a
second precedence problem: a simple-name BuiltinExternalModel BigInteger.pow
fact outranks the explicit qualified external-pack contract. The external
receiver planner therefore refuses crate dispatch even though pack lookup now
correctly identifies a unique mapping. V207 retains the imported qualified
owner when selecting receiver API effects. Its synthetic competing-builtin
regression passes in `.tmp/numbers-mapped-owner-v207-focused.log`.

Two related producer repairs are batched with this: TypeEnv statement learning
retains source binding names as well as normalized Rust names (repairing a
proven type-lookup loss in that shared learner), and erased
display downcasts reuse the canonical mapped type renderer rather than
encoding a Java package as an unavailable Rust identifier. Focused regressions
initially fail because the new display test's raw parsed qualified-constant
expression does not exercise the resolver-produced value field shape. The
test is corrected to resolve an actual modeled static value import before
checking the canonical downcast; this correction changes no production code.
All8 mapped-boundary regressions pass in
`.tmp/numbers-mapped-boundaries-v208-focused.log`, including source local type,
canonical Result payload, canonical display downcast and source shadowing.

V207 frozen producer `.tmp/new-java2rust-numbers-big-integer-v207-debug`, SHA256
`44f5ec7d9b286acedb9da37e6501366effa2b96a8d0a713736a614359ebafa68`.
The matching already-compiled BigInteger test executable is reused directly
with the normal offline/scratch environment; its result is4 pass/1 fail in
`.tmp/numbers-big-integer-v207-executable-tests.log`. The display type errors
are eliminated, leaving4 generated errors from the opaque ONE collision and
erased SAVED_ONE storage. The542 comparisons have **not** executed.

Re-auditing real ArithmeticUtils without the rest of Core proves the8 dropped
receivers remain at V207: `.tmp/ArithmeticUtils-v207.rs` and translation log.
In contrast, the public target planner with its solved method environment now
returns ExternalCrateReceiver for the three BigInteger overloads:
`.tmp/numbers-big-integer-resolution-probe-v207-core.log`. This distinguishes
the fixed imported-effect selection from the still-broken full lowering path.
Next trace actual lowering's environment/fact/ABI and static retarget boundaries
against that public plan; an isolated planner pass does not prove full emission.
For ONE/SAVED_ONE, trace original unresolved facts, semantic guesses and the
signature/storage producer, since late shape/name rendering alone has not
repaired those contracts. No new full Core or RNG rerun follows V207 yet;
V206 remains the latest full Core measurement and V201 the latest RNG evidence.
Keep the enum payload and generic Object-equality architectural work recorded
above in scope; neither is solved by this batch.

### V209–V211 CLI ABI scope and stored nominal values

Temporary test-only tracing (removed after diagnosis) proves the library API
selects and emits ExternalCrateReceiver correctly for real ArithmeticUtils:
`.tmp/numbers-core-v209-call-trace.log` and
`.tmp/numbers-big-integer-resolution-probe-v209-emission.log`. The mismatch is
not a late loss in this call plan. CLI manifest ingestion additionally enriches
facts from the Rust runtime source. Its line-based facade ABI inventory never
closed an impl scope, so later free adapters overwrote the actual inherent
methods with an AssociatedFunction receiver ABI. That false metadata explains
the CLI-only eight missing receivers. The scoped inventory now masks Rust
comments/literals before counting braces, records methods only at their impl
body depth, and rejects incompatible Java/Rust argument counts. The regression
covers ordinary/raw strings, char literals, nested comments, sibling impls and
same-name free adapters. V210 CLI tests pass23/0.

V210 producer `.tmp/new-java2rust-numbers-facade-scope-v210-debug`, SHA256
`8e414f2701126bc369b92228e148bdc07f73dfb07626f9277a421684f5c3eef5`,
generates real instance calls for all8 ArithmeticUtils sites. Direct rustc on
that untouched CLI output exposes12 helper-trait errors: impl-local Self return
types escaped into erased-object facade signatures (including unsized slice
implementations). Evidence: `.tmp/ArithmeticUtils-v210.rs` and
`.tmp/ArithmeticUtils-v210-compile.log`. These are not ignored or accepted.
The V211 candidate binds Self to the qualified runtime owner before publishing
ABI facts, reusing the existing Rust identifier substitution helper now shared
through `types.rs`, rather than substituting Self with whatever later receives
the generated helper method. A focused test retains SelfError unchanged while
binding Self in parameters and Result payloads.

The smaller unchanged diagnostic
`.tmp/BigIntegerImportScopeProbe.java` reproduces the phantom ONE facade without
the large oracle's loops. The scoped static-owner inventory recorded only Rust
binding spellings and guessed an unavailable static type for constant-style
Java variable names. It now retains original names for parameters, initialized
and uninitialized locals, enhanced-for variables and lambda parameters, using
the existing shadow-name authority. Scoped catch names were already retained.
Focused inventory coverage preserves genuine unbound external-owner discovery.

The real large oracle's field producer is separately audited in
`.tmp/numbers-import-dispatch-v210.log`: SAVED_ONE has Java type BigInteger and
ConcreteStruct representation, but a NeedsUnknownRuntimeObject observation
forces erased storage during signature planning. An explicit crate mapping is
a nominal carrier contract, like an actual parsed declaration; the shared
dispatch policy now keeps that carrier and leaves unknown-call adaptation at
the call boundary. An unmapped nominal type retains the conservative erased
fallback. This is a general policy extension, not a BigInteger field override.

All10 focused mapped-boundary tests pass in
`.tmp/numbers-mapped-boundaries-v211-focused.log`. Full library1640/0 and CLI24/0
pass in `.tmp/numbers-mapped-boundaries-v211-library-tests.log` and
`.tmp/numbers-facade-scope-v211-cli-tests.log`. The fresh ArithmeticUtils CLI
output now compiles directly, without generated-code edits:
`.tmp/ArithmeticUtils-v211.rs` and `.tmp/ArithmeticUtils-v211-compile.log`.

V211 producer `.tmp/new-java2rust-numbers-mapped-boundaries-v211-debug`, SHA256
`45ef0965c67169d1259d190d416f46aa4a09fcc6861a80a3507778f8b3c73ce4`.
Fresh complete Core11-file translation/parse/check has17 remaining diagnostics:
12 generic equality,3 Norm callback payload fields,1 BigDecimal and1
RoundingMode. All8 receiver-loss errors and the intermediate Self leakage are
eliminated. Evidence: `.tmp/numbers-core-v211-mapped-boundaries-run.log`,
`.tmp/commons-numbers-1.3-core-v211-mapped-boundaries/top-compile-errors.tsv`.
Fresh Primes3594 and Root Finder122 exact observations also pass in their V211
preservation behavior logs. Core is still not accepted.

The full BigInteger probe reaches a new boundary: no opaque ONE collision,
erased SAVED_ONE or bad display downcasts remain, but5 owned-receiver move
errors prevent compilation. Evidence:
`.tmp/numbers-mapped-boundaries-v211-integration-tests.log`. The542 comparisons
have not run yet. The hypothetical Rust changes are small (preserve receivers
at mapped call boundaries), not an ownership representation redesign.

### V212–V213 common receiver and conditional carrier boundary

The earlier mapped receiver clone-recovery hook ran only in signature-aware
receiver adaptation. Scalar and formatting paths could bypass it and consume
live values. The candidate moves that exact liveness/capability-proven recovery
to the common call-emission boundary, covering those paths without assuming
that every read-only foreign ABI accepts a Rust borrow. It does not clone
mutating receivers or introduce BigInteger-specific ownership policy.

The focused executable regression
`mapped_scalar_and_format_receivers_preserve_later_reads` exercises repeated
loop predicates/comparisons, aliased multiplication, later display and later
bit reads. Its first candidate eliminates the move errors but exposes a
conditional carrier bug: fallibility of the already-discharged boolean
condition was incorrectly attributed to plain int ternary branches. The
numeric assignment appended unwrap_or_else to an integer. V213 removes the
condition from that carrier predicate; it remains lowered through its own
boolean slot. A separate Numbers regression verifies both normal behavior and
a caught predicate exception rather than changing the test to avoid ternaries.
Focused verification passes in
`.tmp/numbers-receiver-liveness-v213-focused.log` and
`.tmp/numbers-conditional-v213-focused.log`.

### V213 shared BigInteger batch verified; Core remains unaccepted

The full Java/generated-Rust BigInteger fixture finally compiles and compares
all542 observations exactly, with no floating tolerance or expected-output
substitution. All6 BigInteger executable tests pass, including the smaller
ownership/conditional regression:
`.tmp/numbers-big-integer-v213-executable-tests.log`. The already-compiled test
executable is reused for this run with the normal offline/scratch environment;
no generated source is repaired. This verifies the declared JDK API subset,
not every API of the whole JDK BigInteger class and not acceptance of Core.

Producer `.tmp/new-java2rust-numbers-mapped-boundaries-v213-debug`, SHA256
`8144eb24973e8ddc78f618cb54e2ee967d391b3913b735dfdb40af068735c0d5`.
Runtime source and manifest remain byte-identical to the archived V201 inputs:
`1c209ddc5e047628636e67696f6508c8f9348e7b2edff32427d953b66af301d8` and
`cce1a1014f7859fda9053fee8bb9dc0977c2a0354b9954e7fb657850807c8d1f`.
Assembler remains
`589bf2d1087149f94ccecf023b8c7306f1ecf4fd3dc9952103749a65e0663b57`;
the complete production manifest remains
`2768762a5893028a8de4cc52608cf46643b65ae591097b324f4aac8810d3c651`.

| V213 gate | Result | Evidence |
|---|---|---|
| Full library |1640/0 | `.tmp/numbers-mapped-boundaries-v213-regression-tests.log` |
| CLI/manifest ingestion |24/0 | Same regression log |
| Numbers executable contracts |19/0 | Same regression log |
| BigInteger executable contracts |6/0,542 exact JDK observations | `.tmp/numbers-big-integer-v213-executable-tests.log` |
| Independent runtime tests |6/0 | `.tmp/numbers-java-math-v213-runtime-tests.log` |
| Primes complete3 files | Compile and3594 exact observations | `.tmp/numbers-primes-v213-preserved-run.log`, `.tmp/numbers-primes-v213-preserved-behavior.log` |
| Root Finder complete3 files | Compile and122 exact observations | `.tmp/numbers-rootfinder-v213-preserved-run.log`, `.tmp/numbers-rootfinder-v213-preserved-behavior.log` |
| RNG complete46-file sampling/native closure | Compile,0 unsupported/invalid/errors | `.tmp/rng-sampling-all-v213-numbers-run.log` |
| RNG native behavior |12601 exact observations | `.tmp/rng-native-v213-numbers-behavior.log` |
| RNG collection sampling |162 exact observations | `.tmp/rng-collections-v213-numbers-behavior.log` |
| RNG distribution sampling |2172 exact observations | `.tmp/rng-distributions-v213-numbers-behavior.log` |
| RNG original11-file source64 closure | Compile and2865 exact observations | `.tmp/rng-source64-v213-numbers-run.log`, `.tmp/rng-source64-v213-numbers-behavior.log` |

Formatting and git diff whitespace checks pass. These named gates do not claim
the unrelated, still-enabled static erased-array Root Finder contract has been
fixed, or that every project test target has passed.

The latest full Core checkpoint is V211's17 diagnostics across four families,
not a relabeled V213 corpus rerun. Two of6 target modules remain accepted.
Next coherent runtime batch: audit and implement the actual BigDecimal and
RoundingMode surfaces used by *both* Precision and DD before implementation.
Precision uses decimal string construction, setScale and doubleValue; DD also
needs exact double construction, addition, bigDecimalValue and related display
paths. Merely eliminating the two compile errors would leave reachable fake
bodies and would not meet acceptance. Keep the already recorded enum payload
initialization and generic virtual Object-equality contracts in scope, with
their rewrite/representation audits, and do not narrow the six-module goal.

### Decimal runtime entry audit (V214 candidate; not accepted)

The untouched Core source requires decimal string construction, exact binary
double construction, exact addition, setScale with all eight rounding modes,
double conversion and observable decimal formatting. Precision's string route
and DD.bigDecimalValue's exact-double route are deliberately different and
must not be collapsed into f64 rounding. V211 generated DD delegates to a unit
BigDecimal facade; Precision also reaches unimplemented methods. Two compiler
diagnostics therefore understate the missing behavior.

The minimal valid Rust rewrite keeps both source algorithms intact and maps
their JDK operations to a reusable immutable decimal carrier, represented as
an arbitrary-precision signed coefficient and signed32-bit scale. Existing
num-bigint and mapped constructor/member/call/result mechanisms suffice; no
new source ownership representation is needed. Exact double construction
decodes IEEE bits, so it cannot use a shortest decimal string. Rounding acts
on integer quotient/remainder, and addition aligns scales without floating
conversion. Canonical decimal formatting and float conversion remain separate.
Constructor NumberFormatException and arithmetic exceptions must cross the
existing foreign-exception boundary, not become default values or raw panics.

Primary contracts: [JDK BigDecimal](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/math/BigDecimal.html)
and [JDK RoundingMode](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/math/RoundingMode.html).
The installed JDK probe `.tmp/DecimalContractProbe.java` and its output establish
actual extreme-scale/error observations and BMP decimal digit ranges before
implementation. In particular, zero rescaling must not allocate huge powers,
and exact double/string conversions need distinct independent comparisons.
The runtime kernel, bindings and executable JDK matrix form one batch; no
Core acceptance or new full-corpus run is justified until that batch is tested.

### V214–V219 decimal implementation and binding verification

The new `runtime/java-math/src/decimal.rs` kernel implements the audited
coefficient/scale contract, with the public mapped ABI declared in the runtime
root. The existing manifest now includes BigDecimal, all eight RoundingMode
values and NumberFormatException. It is an explicit API subset, not a claim
that every JDK BigDecimal operation is implemented. Nullable foreign exception
messages use their mapped `getMessage` contract rather than `Display`, which
cannot distinguish Java null from an empty string.

The V215 independent runtime matrix passes all2424 observations from the
installed OpenJDK19.0.2, covering string and exact-double construction,
addition, all rounding modes, formatting, scale, raw double/float bits and
error class/message observations. This test calls the runtime directly, not
the translator, so it proves kernel behavior only. Evidence:
`.tmp/numbers-decimal-v215-contract-tests.log`; runtime unit tests9/0 in
`.tmp/numbers-decimal-v214-runtime-tests.log`. Supplementary invalid digit
messages currently cross Rust String's UTF-8 boundary as `?`, not as Java's
unpaired UTF-16 surrogate. This is not whole-JDK message-identity coverage.

The translated executable in that same V215 log fails with10 diagnostics:
unqualified constructor/return types and duplicate exception conversions.
Audit traces the former to constructor pack membership: a shared pack name
incorrectly makes every prefix in that group a competing owner of one
constructor's type. The V216 candidate uses the same qualified type-ownership
predicate as other member lookups. It also prefers the unique qualified
exception identity over its simple-name alias when both describe one Rust
carrier; it does not arbitrarily collapse different qualified Java classes.
The focused pack-ownership test and both exception-conversion tests pass in
`.tmp/numbers-decimal-v216-pack-tests.log` and
`.tmp/numbers-decimal-v216-exception-tests.log`.

V216's original translated executable passes, as do the full library1643/0,
CLI24/0, BigInteger6/0 and Numbers19/0. Evidence:
`.tmp/numbers-decimal-v216-contract-tests.log`,
`.tmp/numbers-decimal-v216-library-tests.log`,
`.tmp/numbers-decimal-v216-cli-tests.log`,
`.tmp/numbers-decimal-v216-regression-tests.log`. These do not prove Core
acceptance. V216 producer SHA256 is
`5790925bc2cd0ce08963de180ab8b3dc87fdf07a60e7f47202bbf316b8be34c0`.

The fresh whole Core measurement remains17 compilation diagnostics:12 equality,
3 enum payload fields,1 missing decimal subtraction and1 erased constructor
argument. Evidence: `.tmp/numbers-core-v216-decimal-run.log` and
`.tmp/commons-numbers-1.3-core-v216-decimal/top-compile-errors.tsv`. All11 files
are exercised. The earlier audit missed the production `DD.from(BigDecimal)`
route, which reads doubleValue and subtracts the exact decimal encoding of
that double before constructing DD. The corrected scope includes subtraction,
not just addition and DD.bigDecimalValue. Reusing exact addition with a negated
coefficient provides subtraction without another scale-alignment algorithm.

The V217 constructor correction retains actual argument types when external
metadata specifies `_` (an arity-only wildcard), rather than coercing a double
local to erased Object. Its expanded independent JDK matrix adds484 subtraction
observations:2908 total, all passing. The expanded mapped executable also
passes every rounding-mode constant, name/ordinal, float/scale observations,
arithmetic error messages and constructor-before-argument evaluation.
Evidence: `.tmp/numbers-decimal-v217-contract-tests.log` (2 pass,1 fail).

The remaining executable deliberately tests propagation through source method
boundaries. The first failure is an unchecked argument null check that lowering
emits, but fallibility analysis previously recognized only primitive unboxing.
V217 reuses the same optional coercion policy for argument checking. It fixes
the nullable-mode failure but exposes uncaught ArithmeticException at another
source boundary. V218 retains the modeled receiver's nominal callee identity
in unresolved/chained external call-graph edges and seeds fallibility from
mapped Result ABIs, including constructors with no declared Java throws list.
These are shared producer/analysis/lowering contracts, not decimal-specific
panic catches or corpus edits.

V218 focused14/0 and full library1646/0 pass. CLI24/0, runtime9/0,
Numbers19/0 and decimal contracts3/0 pass, including2908 exact JDK observations.
Both translated decimal fixtures now also compile and execute the same source
under Java, separately from the generated-Rust check. The source-boundary
fixture covers NaN/infinity with a null rounding mode, finite null-mode
failure, uncaught arithmetic propagation and an uncaught constructor failure.
Evidence: `.tmp/numbers-decimal-v218-focused-tests.log`,
`.tmp/numbers-decimal-v218-library-tests.log`,
`.tmp/numbers-decimal-v218-final-contract-tests.log`,
`.tmp/numbers-decimal-v218-runtime-tests.log` and
`.tmp/numbers-decimal-v218-numbers-tests.log`.

The BigInteger rerun has5 pass,1 test-harness compile failure: a previously
plain source wrapper now correctly exposes its mapped Result contract, but the
test compares Result directly to90. The follow-up retains the90 assertion and
adds a caught negative-exponent failure through that source wrapper; it passes.
The independent542-observation BigInteger oracle passes in the original rerun.
Evidence: `.tmp/numbers-decimal-v218-regression-tests.log` and
`.tmp/numbers-decimal-v218-static-helper-regression.log`. This is an ABI-aware
harness correction, not an ignored compiler diagnostic or weakened oracle.

Primes3594 and Root Finder122 exact preservation observations pass with V218,
in `.tmp/numbers-{primes,rootfinder}-v218-preserved-{run,behavior}.log`.
Frozen producer `.tmp/new-java2rust-numbers-decimal-v218-debug` SHA256:
`16e2642c4683f4cec9b1d927edee5ba41f36a051cb205d48ee3677a823df5f15`.
Runtime root, decimal kernel and manifest hashes respectively:
`37905e16200c89a94dd844c25201bacc4002c5cd757b1178dcf7f61bba3b9cc3`,
`29d45c23373d112dd7b32090538f39ebb1efc5f68d220a1461ae9234074e5269`,
`81fbe2a9ec3c80fb07d5d36248767569c8e9e93144e6b599ffe9debadc71ddbb`.

Fresh whole Core V218 has16 compilation diagnostics:12 generic equality,
3 Norm payload fields and1 erased double argument to the decimal constructor.
Subtraction and its return/exception bindings are now real mapped calls.
Evidence: `.tmp/numbers-core-v218-decimal-run.log` and
`.tmp/commons-numbers-1.3-core-v218-decimal/top-compile-errors.tsv`.
The residual constructor problem is CLI-enriched metadata: Rust generic T
contributes nullability/borrowing evidence but no Java formal type. V219 keeps
that evidence while using the actual argument type instead of erased Object.
Its focused regression covers both arity-only and ABI-enriched facts. Audit
also found an accidental self-recursive fallback in the no-argument-evidence
constructor query; it now returns None, with focused no-evidence coverage.
V219's focused constructor test passes1/0 in
`.tmp/numbers-decimal-v219-constructor-focused.log` (the earlier unqualified
`--exact` invocation selected0 tests and is not counted as coverage).
The V218 RNG46-file closure compiles with0 unsupported/invalid/errors;
native12601, collections162 and distributions2172 exact observations pass in
`.tmp/rng-sampling-all-v218-numbers-run.log` and the corresponding
`.tmp/rng-{native,sampling-collections,sampling-distributions}-v218-numbers-behavior.log`.
V219 decimal contracts3/0 (2908 runtime/JDK observations plus independent
Java/generated-source probes) and CLI24/0 pass in
`.tmp/numbers-decimal-v219-contract-tests.log`. Producer
`.tmp/new-java2rust-numbers-decimal-v219-debug` SHA256:
`612418701e6a9660a724f1033493b8a7d31f0e7146f9e8bfbca9398ed7b43861`.
The runtime root/kernel/manifest, assembler and source manifest remain identical
to their recorded V218 inputs. V218's original11-file RNG source64 closure
also compiles and passes2865 exact observations in
`.tmp/rng-source64-v218-numbers-{run,behavior}.log`: all17800 preserved RNG
observations pass. These preservation gates use the frozen V218 producer;
they are not relabeled as V219 runs.
V219's fresh whole Core translation/parse/check reaches15 compilation
diagnostics, all in the existing equality/enum families:12 `&Self == T` and
3 missing Norm callback payload fields. Every11 production file is exercised;
parse failures, invalid patterns, unsupported members and stubbed constants
are0. Evidence: `.tmp/numbers-core-v219-decimal-run.log` and
`.tmp/commons-numbers-1.3-core-v219-decimal/top-compile-errors.tsv`.
Untouched generated DD.from now calls
`decimal_subtract(x, &java_math_runtime::BigDecimal::new(z)?)`, with a genuine
double argument and propagated errors. DD.bigDecimalValue and both Precision
round overloads likewise expose Result instead of swallowing foreign failures.
This is a compiler/binding checkpoint, not whole-Core behavioral acceptance.
The runtime's exact V219 source/manifest/lock inputs are archived under
`.tmp/java-math-runtime-v219` for reproduction after later runtime edits.

Compiler counts are not the whole acceptance backlog. The V218 translation
stderr also retains `resolve_call_ambiguous` for the anonymous DoubleEquivalence
implementation's `compareTo(a, b, epsilon)`: early resolution sees the declared
double field epsilon as Unknown. Later lowering emits the correct
`compare_to_double_double_double(a, b, self.epsilon.get())`, but that does not
justify ignoring the resolver diagnostic. Its field/type scope must be fixed
and regression-tested before Core acceptance, alongside equality and enum
state. The same diagnostic persists at V219. Evidence: V218/V219
`translation-logs/batch.stderr` and
`generated-crate/src/generated/00009_Precision.rs`. Entry audit for that
resolution family: `resolve_expr_calls` walks anonymous members with the
outer environment and adds method parameters, but not the anonymous fields;
the later lowering stage has actual payload field types. A correct repair
must model anonymous field scope (including shadowing and explicit this), not
suppress ambiguity diagnostics or special-case the name epsilon. Treat it as
a resolution-environment contract with focused overload/shadowing coverage.
The six-module goal and the enum/equality architectural work remain active.

## Object/equality re-audit and anonymous lexical scope (V220–V221, incomplete)

The V219 Core count remains the authoritative full-corpus checkpoint. No new
module is accepted here and no equality compiler error is claimed fixed.
Re-read the existing V198 equality rewrite audit before implementation. A
runtime registry alone does not close the producer/storage/signature/consumer
contract: current source equals(Object) overrides can accept a non-null
Box<dyn Any>, whereas an actual generic caller's argument may be null. A
same-Rust-type downcast, early false on null, or extra Clone/PartialEq bounds
would change Java virtual dispatch, not implement it. Box<()> is not a safe
invented null sentinel: the existing erased constructor uses it for a newly
constructed Object. Keep true nullability distinct.

Added independent source witness
`tests/corpus/numbers/GenericObjectEqualityOracle.java`. Java self-check returns0
in `.tmp/numbers-object-equality-v220-audit/java.log`. Frozen V219 translation
and direct rustc evidence are `generated-v219.rs`, `translation-v219.log` and
`compile-v219.log` in that directory. Its16 compiler diagnostics cover:

- Generic interface defaults still emit &Self == Option<T>, including their
  synthetic default companions, not a checked virtual Object operation.
- Inlined defaults on concrete classes can unwrap the nullable argument,
  double-erase an existing Object box, or clone a Box<dyn Any> as if it were a
  payload. That is not an identity-preserving Java reference copy.
- Default Object identity on a retained class can compare payload borrows
  instead of canonical handles, and one emitted expression retains a mutable
  borrow while invoking a callback on that same receiver.
- Mutable/inherited equals overrides disagree with read-only interface ABI
  forwarders; throwing equals is not propagated by the enclosing default.
- Generic source storage/argument capability requirements are not carried to
  every consumer; the witness exposes a missing JavaReferenceIdentity bound,
  invalid erased copying and a returned-trait-object lifetime error.

The witness requires heterogeneous String arguments, an override that returns
true for null, inherited overrides, a thrown exception with its message,
default identity versus a distinct object with equivalent fields, a concrete
generic implementor, repeated alias/state use and reentrant same-receiver
calls. Do not remove cases to make a registry-only patch pass.

Correct whole-path plan: give Object equality one checked heterogeneous
contract with a nullable argument; adapt selected source overrides through
`source_hierarchy_equals_method` and `source_object_protocol_method_call`,
which already select companion ABIs without holding storage loans. Keep
canonical handles through receiver/argument erasure. Default identity must use
JavaReferenceIdentity and must feed usage/storage planning before emission.
Extend the existing erased registries and generalize the existing concrete
generic-instance inventory used for source display, rather than maintaining a
second list of source types. Propagate nullable argument, receiver mutation
and Result contracts through interface defaults, copied defaults, shared
companions and forwarding implementations. No identity fast path may bypass
an overriding equals body, which can throw even for self. This remains a
cross-function protocol/storage batch; the current framework has relevant
authorities but not yet a complete implementation.

The remaining Core resolver diagnostic has a separate, proven cause. Inline
anonymous method resolution cloned the enclosing environment and added only
parameters, never the anonymous class's fields. V220 adds a scoped field
environment in TypeEnv, seeds declared and specialized inherited fields,
removes captured-local bindings shadowed by those members, and distinguishes
explicit anonymous this from qualified enclosing this. Method parameters are
then installed using both source and normalized names. Scope clones preserve
the enclosing environment after the expression. This is not an epsilon-name
exception or a diagnostic suppression.

The focused resolver test passes in
`.tmp/numbers-object-scope-v220-focused.log`. The executable regression
`tests/anonymous_field_contracts.rs` independently compiles and runs its Java
source, then translates and compiles the same source; its initial V220 run
fails with4 genuine Rust errors, retained in
`.tmp/numbers-object-scope-v220-executable.log`. The field overloads are now
correct, but two adjacent anonymous-object contracts remain:

1. The primitive helper's real inherent body is emitted, but its two call
   sites use the free name with_parameter instead of a self receiver. The
   correct hypothetical rewrite is a self method call using that exact helper
   signature. Resolve/bind anonymous-owned methods through authoritative
   source method identity, argument and fallibility planning; do not use an
   emitted-text replacement or pretend it belongs to the enclosing class.
2. Qualified Outer.this field reads are emitted as outer_name.this.field,
   without capturing any enclosing instance in the anonymous adapter. Correct
   Rust must retain the canonical enclosing handle, capture it once, and read
   it via the existing shared storage/companion machinery. A copied field or
   cloned payload is not equivalent. The fixture is strengthened to mutate
   the outer field after creating the escaping adapter, requiring the later
   call to observe that update.

These are existing emission gaps exposed by the broader regression, not
accepted behavior or manually patched generated deliverables. Keep the
executable regression enabled. Added inherited-generic-field resolution
coverage; all24 resolver tests pass in
`.tmp/numbers-object-scope-v221-resolver-tests.log`, including both new scope
tests. The strengthened Java witness also returns0 in
`.tmp/numbers-object-scope-v221-audit/java.log`. This does not relabel the
failing V220 generated-Rust executable as passing, and no fresh full Core
measurement is claimed for V220/V221.
Finish the anonymous method/outer-capture contract batch before another full
Core rerun, then continue the checked Object equality batch with the witness
above. The enum-state design and all six original modules remain in scope.

## Anonymous helper/capture contract batch (V222–V226)

Continued the enabled V220/V221 regression without deleting its escaping
outer-update, shadowing or helper cases. The implementation now uses the
existing capture/storage, source overload and synthetic callable authorities:

- Capture discovery carries enclosing receiver types, including their generic
  arguments, independently of whether those receivers declare fields. A
  qualified enclosing-this expression contributes an actual OuterThis capture.
  Existing storage planning makes the captured source object aliasable.
- The source factory's canonical-handle companion recognizes that capture.
  The field-backed anonymous adapter retains the same handle in an explicit
  capture field, typed by `outer_capture_handle_rust_type`. Qualified reads use
  that field through the ordinary shared storage machinery. No outer payload
  or field value is copied as a substitute for the Java reference.
- An initial V223 executable exposed leaked factory `__java_self` inside the
  generated fn item. Adapter/helper contexts now clear that factory handle;
  their own this and captured outer this are separate lexical receivers.
- Scoped helper declaration selection reuses the resolver's Java overload
  rules. Its private candidate labels are selection-only: they do not add fake
  classes or external members. Lowering calls the actual inherent helper on
  self with its declared primitive argument shapes, including widening.
- Call-graph construction allocates all anonymous member identities before
  visiting their bodies. Sibling/forward calls target those identities rather
  than same-named enclosing methods. Helper emission and call lowering consume
  the resulting checked Result contract. Bound helper field reads use the
  existing lexical binder and field shapes, preserving parameter shadowing.

V224 executable contracts pass2/0 in
`.tmp/numbers-anonymous-v224-executable.log`. Each source is independently
compiled and run by Java before translating and executing the same source in
Rust. The original probe observes an outer field change after adapter creation.
The new `AnonymousHelperCallProbe.java` checks an anonymous helper shadowing an
outer static helper, int-to-double argument widening, a division exception
escaping the helper via the interface, and its caught class/message.

V224 focused tests have27 pass,1 fail: the new graph test showed that the
factory was spuriously marked fallible by division inside a deferred anonymous
method body. V225 fixes the *typed immediate-effect* scan to inspect actual
field/constructor initialization but leave method/lambda bodies to their
synthetic callable identities. The global support-feature inventory remains
unchanged; deferred bodies still require arithmetic/exception support. Added
controls retain an eager integer-dividing initializer's exception, distinguish
non-throwing floating division, and keep lambda creation infallible.

V225 focused29/0 and the separate generic enclosing-receiver capture test1/0
pass in `.tmp/numbers-anonymous-v225-focused.log` and
`.tmp/numbers-anonymous-v225-outer-capture.log`. This batch does not implement
the checked Object equality or enum payload designs, and does not claim
arbitrary new anonymous-helper signatures beyond the existing adapter's
supported surface.

### V225 broad checkpoint and V226 default-stack repair

V225 producer `.tmp/new-java2rust-numbers-anonymous-v225-debug`, SHA256
`0c8c78043bd330cce17c7e274b8d16d5bbb33f97c3993b14818b2a932dc253c2`.
Fresh full Core11-file output has no resolver Error diagnostics. The previous
anonymous epsilon overload ambiguity is eliminated. Parse failures, invalid
patterns, unsupported members and stubbed constants remain0. The15 Rust
diagnostics are unchanged:12 generic equality and3 Norm payload fields.
Evidence: `.tmp/numbers-core-v225-anonymous-run.log` and
`.tmp/commons-numbers-1.3-core-v225-anonymous/{translation-logs/batch.stderr,top-compile-errors.tsv}`.
This is not Core acceptance or proof of its remaining runtime behavior.

Fresh preservation with the same V225 producer passes Primes3594, Root
Finder122 and all17800 RNG observations. RNG's complete46-file sampling/native
closure and original11-file source64 closure both compile with0 unsupported,
invalid or compiler errors. Evidence:
`.tmp/numbers-{primes,rootfinder}-v225-preserved-{run,behavior}.log`,
`.tmp/rng-sampling-all-v225-numbers-run.log`,
`.tmp/rng-{native,sampling-collections,sampling-distributions}-v225-numbers-behavior.log`,
`.tmp/rng-source64-v225-numbers-{run,behavior}.log`.

V225 full library1651/0, CLI24/0, anonymous executable2/0 and decimal3/0 pass,
but the large BigInteger test overflows the translator test thread's default
stack. Its other5 tests pass. Evidence:
`.tmp/numbers-anonymous-v225-regression-tests.log`. This regression is not
ignored or accepted. A diagnostic-only rerun with RUST_MIN_STACK=8388608 passes
all542 exact observations in
`.tmp/numbers-anonymous-v225-bigint-stack-diagnostic.log`, establishing finite
stack pressure, not a semantic repair. A sandboxed debugger could not trace
the child (ptrace denied); no debugger result is claimed.

Inspection of the actual debug executable establishes a170192-byte recursive
`lower_expr_with_refinements` frame at V225 (V219:159088 bytes). The giant match
reserves temporaries for unrelated large variants even while lowering a
simple numeric expression. V226 mechanically outlines Call, New, Cast and
FieldAccess arms into non-inlined variant handlers, preserving their bodies
and all pre-dispatch checks. The dispatcher frame is now32688 bytes, about81%
smaller. Evidence: `.tmp/numbers-anonymous-v225-dispatcher-frame.log` and
`.tmp/numbers-anonymous-v226-dispatcher-frame.log`. No test stack limit is
raised as a fix. The new16-level numeric-expression regression checks both
ordinary values and Java integer overflow on the normal test stack.

V226 producer `.tmp/new-java2rust-numbers-anonymous-v226-debug`, SHA256
`4e7b18427b6a2aedc808b03ad18f88a007b0e0eaa985d09b5003af9633bc4a6f`.
Verification on the ordinary environment, with RUST_MIN_STACK unset:

| Gate | Result | Evidence |
|---|---|---|
| Full library |1651/0 | `.tmp/numbers-anonymous-v226-regression-tests.log` |
| CLI |24/0 | Same regression log |
| Anonymous Java/generated-Rust contracts |2/0 | Same regression log |
| Decimal contracts |3/0,2908 exact JDK observations | Same regression log |
| Numbers contracts including nested-expression stack control |20/0 | Same regression log |
| Large BigInteger source translation/execution |1/0,542 exact JDK observations | `.tmp/numbers-anonymous-v226-bigint-default-stack.log` |
| Remaining BigInteger contracts |5/0 | `.tmp/numbers-anonymous-v226-bigint-remaining-tests.log` |

The large BigInteger test is run separately once and excluded only from the
remaining-five invocation to avoid duplicating its expensive run; all6 tests
are covered on V226. Formatting and diff whitespace checks pass. The runtime
root/kernel/manifest, assembler and production manifest remain byte-identical
to their recorded V218/V219 hashes. The broader corpus preservation/Core
measurement above uses V225; it is not relabeled as a V226 full-corpus run
after the mechanical outlining correction. The unrelated enabled static
erased-array Root Finder test remains outside these named gates and is not
claimed repaired.

Next: implement the checked heterogeneous Object equality contract from the
re-audit above, including nullable arguments, canonical receiver storage,
mutable/inherited/throwing overrides and concrete generic instances. Native
Object recognition must retain parsed-source/import precedence rather than
using a simple equals/Object name match. Keep the full equality witness and
Norm initialization design in scope; only2 of6 modules are accepted.

## Object protocol ownership prerequisite (V227)

The next equality batch exposed a classification defect before nullable ABI
or virtual-dispatch changes: `method_overrides_object_equals` accepted any
parameter whose leaf name was Object. Consequently an overload accepting
`other.Object`, an imported source Object, or a lexical nested Object could
be published as a Java Object override and selected by a protocol adapter.

The classifier now consumes the actual owner, resolved program and effective
facts. It distinguishes native Object from parsed source ownership, explicit
imports (including imported nested types), same-package/on-demand source
definitions and generic binders. Only the instance, non-generic, boolean
`equals(Object)` declaration qualifies. Override facts, inherited protocol
selection and reference-identity diagnostics share this classifier; there is
no separate diagnostic-only suffix policy.

The first focused run also proved that the parser discarded explicit
`java.lang.Object` qualification before source resolution. Preserving only
the later classifier could therefore reject a genuine platform override
when an unrelated source Object existed. The parser now retains qualified
Object owners, including non-platform java/javax-prefixed names, through its
tree and hierarchy-type paths. A qualified platform name is not shadowed by
an unrelated simple source Object; an exact parsed definition still wins.

Added classifier/fact/protocol-selection controls, cross-unit ownership
controls, parser qualification coverage and identity-diagnostic consistency
coverage. This is a prerequisite, not completion of checked heterogeneous
equality: nullable argument ABI, generic erasure/identity, mutable/reentrant
and throwing virtual dispatch still need the audited whole-path batch.
The broader `GenericObjectEqualityOracle.java` witness remains unchanged.
No fresh Core or corpus-preservation count is claimed for V227; Core's last
complete measurement remains V225, with15 compiler diagnostics.

Final V227 verification:

Frozen producer `.tmp/new-java2rust-numbers-object-v227-debug`, SHA256
`0b5cac76967e2832be4844f7069d2af7895e8378b4bc49b1914cbd6fee89d2c8`.
Build evidence: `.tmp/numbers-object-v227-producer-build.log`.

- Full library1655/0, including all four new tests and their table-driven
  controls: `.tmp/numbers-object-v227-final-contract-tests.log`.
- Parser integration51/1 in the same log. Its whole-fixture sweep encounters
  the existing explicit unsupported non-this synchronized monitor in
  `examples/java/06-stdlib/SynchronizedBlock.java`. The unchanged frozen V226
  producer (hash recorded above) also emits the exact unsupported statement
  for that fixture: `.tmp/numbers-object-v227-synchronized-v226.rs`, line5002.
  This is not fixed by deleting the monitor or weakening the sweep.
- Cargo stops at the parser target failure, so the semantic-fact target was
  run separately:177/7 in `.tmp/numbers-object-v227-semantic-facts-tests.log`.
  The equals-override integration test passes. Remaining failed tests are
  `external_api_effects_explain_receiver_agnostic_methods_without_receiver_type`,
  `indexed_external_pack_lookup_preserves_member_alias_fallback`,
  `inherited_object_methods_on_this_are_external_read_facts`,
  `inherited_receiverless_domain_getters_are_current_type_external_facts`,
  `nullable_field_storage_uses_option_shape_and_assignment_wrapping`,
  `preserves_public_trivial_accessor_calls_when_api_wrappers_must_survive`, and
  `receiverless_record_collection_accessors_are_current_type_external_list_getter_facts`.
  These additional integration gates are not claimed green or repaired; this
  run alone is not a before/after baseline proving their origin. Their inputs
  have no equals declarations or qualified Object spellings, and the indexed
  pack failure uses only FactSet lookup, outside the changed paths.
- Formatting and whitespace checks pass. No upstream sources or generated
  deliverables were edited, and the broad equality witness is retained.

## Nullable equality ABI and generic reference copying (V228–V229)

Continued the whole-path Object equality batch without reducing the full
witness. Native source equals overrides now receive a MaybeNull parameter
constraint at declaration-ABI collection, before call propagation or adapter
emission. The contextual V227 classifier is reused, so custom Object
overloads are not accidentally given this protocol constraint. A new solver
test covers methods whose bodies ignore the argument and have no null call
sites, as well as qualified native/custom overloads in the same owner.

Extracted the existing concrete generic-instance walk used by source display
into a shared protocol inventory. Declaration and construction types use the
same binder/wildcard and unique-source-owner checks as before. Checked display,
erased copying and erased reference identity now consume that inventory.
Generic copying/identity registers only supported concrete shared class/record
carriers, never a bare generic payload or an unbound/wildcard instance. The
ordinary checked Object string dispatch's storage-shape renderer is shared
with these new registry entries; its canonical Rc/Arc and RefCell/Mutex
placement is not reconstructed by string concatenation.

The initial V228 focused test caught that nominal type rendering omitted the
handle and produced a payload downcast. That incorrect intermediate state is
not accepted. V229 uses storage-shape rendering and the regression checks all
four placements for String, Integer and explicitly constructed Double
instantiations, plus unresolved/wildcard exclusions. Copying clones the handle
without adding a payload or type-argument Clone requirement. The existing
additive registry assembly contract remains unchanged.

Moved the independent Java self-check helper from the anonymous test target
to shared test support; the anonymous fixtures are unchanged and still run.
New executable witnesses:

- `GenericReferenceCopyProbe.java`: source alias mutation, Object erasure,
  erased reference copying, cross-carrier identity and later shared updates.
- `NullableEqualsArgumentProbe.java`: a mutating override that accepts null,
  direct and forwarded calls, and repeated call counts.
- The original `GenericObjectEqualityOracle.java` is now an enabled executable
  integration test, including its inherited, nullable, throwing, generic and
  reentrant cases. It is neither ignored nor narrowed.

All three Java self-checks return0. The two new focused Rust witnesses pass;
the full Rust witness still has13 compiler diagnostics. These span the three
interface/default `&Self == Option<T>` comparisons, mutable forwarding ABI,
inherited dispatch, canonical identity, the generic constructor's erased
parameter, generic capability propagation and a borrowed-reference clone
escaping as Any. The fallibility/reentrancy semantic cases remain unproven
until it compiles and runs. This is not a claim that13 small emission edits
can close the remaining architecture. Current generated source is retained at
`.tmp/test-tmp/new_java2rust_generic_object_equality_1792618211446517646/src/lib.rs`;
diagnostics are in `.tmp/numbers-object-v229-executable.log`.

Frozen V229 producer `.tmp/new-java2rust-numbers-object-v229-debug`, SHA256
`556ea5499537fe4c1ca8dfa3e787a1d3c492260fb78ab6055f376342795aab72`.
Build: `.tmp/numbers-object-v229-producer-build.log`.

Verified gates so far:

- `cargo check --tests` passes, with only the existing unused RustMatchArm
  import warning: `.tmp/numbers-object-v228-check.log`.
- Full V229 library1657/0, CLI24/0, decimal3/0 and Numbers20/0:
  `.tmp/numbers-object-v229-regression-tests.log`.
- Anonymous executable2/0; Object executable2/1 (the full witness described
  above): `.tmp/numbers-object-v229-executable.log`.
- Shared-runtime registry assembly5/0:
  `.tmp/numbers-object-v228-shared-registries.log`.
- Runtime root, decimal kernel, mapped manifest, assembler and complete
  production-source manifest remain byte-identical to the recorded V218/V219
  hashes. No source exclusions, upstream edits or generated repairs were made.

Fresh full Core V229 measurement exercises all11 pinned files, with0 parse
failures, unsupported statements/members/expressions, invalid pattern files
and stubbed constants. Compilation still fails with exactly12 `&Self == T`
diagnostics and3 missing Norm payload fields (`two`, `three`, `array`), with
no new compiler-error family. Evidence:
`.tmp/numbers-core-v229-object-run.log` and
`.tmp/commons-numbers-1.3-core-v229-object/{summary.tsv,top-compile-errors.tsv,translation-logs/batch.stderr}`.
Translation took97903ms, assembly14323ms, and the separately enabled resolver
benchmark24809ms (total143228ms). This is not a controlled speed comparison
with V225, whose resolver benchmark was skipped. No fresh accepted-module/RNG
behavior rerun is claimed at this checkpoint; V225 preservation remains the
last such broad measurement, pending the completed virtual equality batch.

Checked virtual Object dispatch still needs the planned receiver/storage,
nullable erasure, override Result and interface/default forwarding contracts.
Core and the three later modules are not accepted by these focused gates.
The V227 wider-integration failures remain recorded separately, not silently
reclassified as passing.

## V230–V238: checked erased equality and reference storage

Implemented a checked `JavaObjectEquals` protocol for supported canonical
shared class/record carriers. It reuses the source override/companion ABI,
concrete generic-instance inventory and physical storage renderer. Dispatch
preserves receiver/argument evaluation order, invokes overrides even for null
or self arguments, and propagates Java exceptions. Native String/boxed
receivers never reverse-dispatch to the argument's override. Transparent
Rc/Arc erased wrappers preserve the underlying reference identity. Unknown
erased receivers remain loudly unsupported rather than silently comparing
false, including against null. This is not full JDK Object support.

Nullable shared-reference erasure now borrows the Option shell and clones its
canonical handle, without consuming the source shell or requiring payload
Clone. Fresh allocations entering erased locals, fields, returns, casts or
resolved call/constructor arguments acquire the existing borrowed-dispatch
storage requirement, not an invented alias/escape fact. The native Object
type classifier is shared with override recognition. Checked logical-result
wrapping also reuses one explicit JavaException type constructor; unary
operators discharge fallible operands through the ordinary Result policy.

New executable coverage includes heterogeneous/null/self/inherited/throwing
and reentrant equality, concrete generic instances, native receiver direction,
argument evaluation before a null-receiver exception, transparent erased
wrappers, retained nullable aliases, and actual multi-module runtime assembly.
All run through translated production source; no generated repairs or upstream
source modifications were used. A separate logical-expression regression
covers short-circuit order and caught exceptions.

V230–V237 were intermediate diagnostic checkpoints, not accepted gates:
V230 exposed ambiguous Result inference; V231 unary Result propagation;
V232 moved nullable shells; V233 missing fresh erased-allocation storage.
V234's focused probes passed but the full interface witness remained red.
V236 correctly rejected a stale embedded Java fixture, and V237 failed to
compile an incomplete mechanical refactor. Only V238 below represents the
final frozen source of this batch.

Frozen producer: `.tmp/new-java2rust-numbers-object-v238-debug`, SHA256
`09a8523a194d3b8d4847992a44fcd65d8c1da7bdae305637b2f271bf2ef26b97`.
Build evidence: `.tmp/numbers-object-v238-producer-build.log`.

Verified final gates:

- `cargo check --tests` passes (existing unused-import warning only):
  `.tmp/numbers-object-v238-check.log`.
- Library1660/0, CLI24/0, decimal3/0, Numbers21/0; Object contracts4/1:
  `.tmp/numbers-object-v238-regression-tests.log`. The sole failing Object
  contract is the enabled complete generic/interface witness, still with13
  compiler diagnostics. The new erased probe and real assembly executable
  both pass. No full virtual-equality acceptance is claimed.
- Shared-runtime registry union/assembly tests6/0:
  `.tmp/numbers-object-v238-shared-registries.log`.
- Math runtime root, decimal kernel, mapped manifest, assembler and complete
  Numbers production-source manifest retain their recorded hashes.

Fresh Core exercises all11 pinned files with0 translation/parse failures,
unsupported statements/members/expressions, invalid-pattern files or stubbed
constants. It still has15 compiler diagnostics:12 generic/interface equality
comparisons and3 missing Norm payload fields. Translation103683ms,
assembly13965ms, total124329ms; resolver benchmark skipped. Evidence:
`.tmp/numbers-core-v238-object-run.log` and
`.tmp/commons-numbers-1.3-core-v238-object/{summary.tsv,top-compile-errors.tsv,translation-logs/batch.stderr}`.
Core remains unaccepted. Existing canonical DD handles are already present;
the next equality batch must connect interface/default receiver typing,
storage, fallibility and forwarding, not redesign DD speculatively or add
blanket PartialEq/Clone constraints. Norm's initialized per-constant payload
and class-initialization contracts remain a separate architectural batch.

Preservation reruns use the same frozen V238 producer. Primes3594 exact
observations, Root Finder122 exact observations and RNG source642865 exact
observations pass. Evidence:
`.tmp/numbers-primes-v238-preserved-{run,behavior}.log`,
`.tmp/numbers-rootfinder-v238-preserved-{run,behavior}.log`, and
`.tmp/rng-source64-v238-numbers-{run,behavior}.log`.
The complete46-file RNG sampling/native closure also translates and compiles
with0 unsupported/invalid/stubbed placeholders. Native12601,
sampling-collections162 and sampling-distributions2172 exact observations
pass, bringing the preserved RNG total to17800 with source64. Evidence:
`.tmp/rng-sampling-all-v238-numbers-run.log`,
`.tmp/rng-native-v238-numbers-behavior.log`,
`.tmp/rng-sampling-collections-v238-numbers-behavior.log`, and
`.tmp/rng-sampling-distributions-v238-numbers-behavior.log`.
The wider V227 integration failures remain recorded separately and enabled.

## V239–V240: mutable equality protocol storage and forwarding

The whole-path trace exposed a missing storage obligation before interface
dispatch: mutating native `equals(Object)` overrides were not included in the
borrowed-object protocol rule already used for mutating toString/hashCode.
That left value payloads behind immutable trait forwarding surfaces. The rule
now reuses `method_overrides_object_equals`, including its exact signature,
native Object ownership and binder/import/source-shadow checks. An unrelated
equals(String), static equals, or a generic binder named Object does not opt in.
Read-only equals alone still does not require a mutable canonical carrier.

Superclass propagation retains protocol sharing as `borrowed_object_dispatch`
rather than recording a fictitious local alias. Genuine retained/aliased base
requirements still propagate through the existing superclass chain. The first
V239 storage test caught this provenance distinction; that intermediate failed
assertion is not reported as a passing gate.

The expanded storage regression covers mutable/native, inherited, read-only,
overloaded, static and shadowed generic signatures. A new independent executable
`MutableEqualsForwardingProbe.java` exercises interface forwarding generated
without interface-typed caller variables, direct and inherited null/String
arguments, repeated state and a nested receiver mutation. It passes in Java
and generated Rust, without editing generated code.

Final V240 library1660/0 and Object contracts5/1:
`.tmp/numbers-object-v240-regression-tests.log`. All five focused executables
pass, including real multi-module runtime assembly. The complete interface/
generic witness remains enabled and fails with13 diagnostics. Mutable trait
forwarder mismatches are removed; canonical generic companions now also expose
the existing borrowed-generic-erasure lifetime defect. The remaining families
are interface/default and inherited/default comparisons, generic constructor
parameter/storage inconsistency, generic identity capability propagation, and
borrowed generic return erasure. Count stability is not evidence that the
underlying diagnostics are unchanged or that the full protocol is accepted.

Frozen producer `.tmp/new-java2rust-numbers-object-v240-debug`, SHA256
`6a263e4c318e83d891e8fa24be99b610e94a480ba20b17dd24f8a34513717ae0`;
build log `.tmp/numbers-object-v240-producer-build.log`. Formatting and diff
whitespace checks pass. No fresh full Core or preservation rerun is claimed
for this storage-only checkpoint: V238 remains the latest corpus measurement
and accepted-module/RNG preservation evidence. Core remains unaccepted.

## V241–V244: scoped generic contracts and retained reference casts

The next whole-path audit followed the constructor parameter into generic
field storage and back through an erased return. The constructor had been
rendered as Box<dyn Any> while its initialized field remained T. The shared
signature solver now preserves scoped binder dispatch/view after applying
erased-use constraints, for parameter/return/storage roles and nullable slots.
An Object-spelled binder remains a binder; a qualified unrelated name is not
one. Ownership and nullability still come from the solver. This is not a
constructor-only rendering patch or a new blanket Clone/PartialEq requirement.

Retained generic self-field reads now copy through the existing
`java_clone_erased_any_ref` runtime. Both borrowed payload methods and canonical
handle companions borrow the field's Option and copy the runtime carrier,
rather than trying to box a clone of &T. The generic constructor and borrowed
erasure lifetime diagnostics in the complete witness are removed.

Equality-call selection also reuses the exact inherited native override lookup
for this receivers. Override facts identify declaring owners, so absence of a
subclass fact must not cause its copied default to use Rust structural equality.
Inherited/default comparisons in the full witness now call the source override.

`GenericErasedFieldProbe.java` independently checks Java/generated-Rust
constructor/field/erased-return identity, repeated reads, later mutation,
replacement and null storage. It exposed another producer/consumer break:
nullable Object-to-source casts consumed the source Option. The existing
canonical-handle cast path now borrows that Option, downcasts and clones its
handle, preserving null separately from a failed non-null cast. Checked
downcasts transpose the per-element Result before discharging exceptions at
the enclosing expression boundary. The probe also checks a null cast, caught
null dereference, caught ClassCastException, and reuse of the source after
both successful and failing casts. V241's moved-source compile failure was
retained and fixed, not avoided by weakening the probe; V242 passes it.

Capability reachability now follows unresolved unique source self-calls using
the existing conservative source-member selector and receiver specialization.
It shares the existing lexical TypeEnv/infer_expr_type path for this typing;
anonymous scopes clear that receiver rather than inheriting the enclosing
object. Explicit and implicit self-call regressions check transitive reference
capabilities; an ambiguous same-arity overload remains excluded. The two
missing generic identity-bound diagnostics in the complete witness are removed.

V243's full witness has5 compiler diagnostics rather than V240's13:3 trait
default comparisons and2 default-identity payload/companion comparisons. That
does not prove fallibility/reentrancy semantics: the complete witness is still
enabled and unaccepted until it compiles and runs. Its generated source is
`.tmp/test-tmp/new_java2rust_generic_object_equality_1792623178145523181/src/lib.rs`.
The next whole-path equality batch must connect frame-aware interface receiver
typing to the checked runtime/exception plan and existing outer-identity-handle
reachability. The latter already omits invalid payload methods for canonical
receiver identity operations; extend that authority rather than deriving a
handle from a payload borrow or emitting structural identity. Trait defaults
can expose the canonical carrier through their existing qualified as_any
method. Use the existing JavaAny capability propagation for erased generic
arguments and preserve copied-default/forwarder Result contracts. Throwing
overrides must propagate, not be panic-unwrapped by an infallible copied body.

V243 library1662/1 failed only the new qualified-name assertion: its test
helper normalized pkg.T to T before calling the predicate. V244 constructs
the qualified JavaType directly; this is a test setup correction, not a
production-code change or a passing V243 library claim.

Final V244 library1663/0, Object contracts6/1:
`.tmp/numbers-object-v244-regression-tests.log`. The single failing executable
is the complete witness, with the5 compiler diagnostics described above.
Latest generated source:
`.tmp/test-tmp/new_java2rust_generic_object_equality_1792623332491807935/src/lib.rs`.
All focused executables, including the strengthened generic field/cast probe
and real multi-module assembly, pass. Formatting and diff-whitespace checks
also pass.

Frozen producers `.tmp/new-java2rust-numbers-object-v243-debug` and
`.tmp/new-java2rust-numbers-object-v244-debug` are byte-identical (cmp passes),
SHA256 `ce08300141e2c89695e500c15feb6939eee725aea3d8d4f62005de4354662b05`.
Build logs: `.tmp/numbers-object-v{243,244}-producer-build.log`.
The test-only V244 correction did not change the production binary.

Preservation with this exact producer passes: Primes3594 and Root Finder122
exact observations; the complete46-file RNG sampling/native closure compiles
without unsupported/invalid/stubbed placeholders and preserves native12601,
collections162 and distributions2172 exact observations. Source64 compiles
and preserves2865 exact observations, for17800 RNG comparisons total. Evidence:
`.tmp/numbers-{primes,rootfinder}-v243-preserved-{run,behavior}.log`,
`.tmp/rng-sampling-all-v243-numbers-run.log`,
`.tmp/rng-{native,sampling-collections,sampling-distributions}-v243-numbers-behavior.log`,
and `.tmp/rng-source64-v243-numbers-{run,behavior}.log`.
Fresh full Core V244 exercises all11 pinned files, with0 translation/parse
failures, unsupported statements/members/expressions, invalid-pattern files or
stubbed constants. Compilation still fails with15 diagnostics in the same two
families:12 interface/default equality comparisons and3 missing Norm payload
fields. No new compiler-error family is introduced. Translation98983ms,
assembly12402ms, total116607ms; resolver benchmark skipped. This is a fresh
measurement, not a controlled performance comparison. Evidence:
`.tmp/numbers-core-v244-object-run.log` and
`.tmp/commons-numbers-1.3-core-v244-object/{summary.tsv,top-compile-errors.tsv,translation-logs/batch.stderr}`.
No new module is accepted; Core and the three later modules remain incomplete.

## V245–V248: checked interface defaults and canonical default identity

Connected source-interface receiver typing to the existing TypeEnv and checked
VirtualObjectEquals call plan. Interface instance frames now bind their generic
this type in lowering and exception analysis; static and anonymous receiver
scopes do not inherit that binding. The native plan is selected before facade
fallbacks, retains its boolean Result ABI, and rejects resolved non-Object
overloads. Generic arguments reaching this erased boundary acquire the existing
JavaAny capability through the existing source-call reachability analysis.

Trait-default equality uses the qualified as_any hook to expose the canonical
receiver, not a downcast of the argument to Self or structural Rust equality.
Interface-valued field receivers snapshot their reference carrier before
argument evaluation. Receiver/argument staging and null checking share the
checked erased-equality boundary. The exception plan now observes the same
interface frame and call selection, so copied defaults and forwarders preserve
Result contracts rather than panic-unwrapping an override's Java exception.

V245 showed why a nominal receiver type alone is insufficient for ordinary
class emission: it confused a borrowed class payload with its Rc/Arc carrier
and generated borrow/borrow_mut calls on &self. V246 limits the new lowering
frame binding to interface dispatch surfaces. Existing concrete-class payload
and companion frames remain authoritative; source-call capability analysis
continues to bind class this types independently as established in V243.
This is a physical-frame distinction, not a corpus/name exception.

Default Object.equals requires allocation identity even without an override.
A shared semantic classifier and body scan now feed both storage and existing
outer-identity-handle reachability. Own and inherited default bodies acquire
the borrowed-dispatch storage requirement without fictitious local aliases.
Canonical companions invoke the existing checked JavaObjectEquals protocol.
Existing method emission omits an invalid payload-only entry point when a
borrow cannot reconstruct that canonical handle. Source one-argument equals
declarations/overloads remain on the ordinary source-method path; record/enum
semantics are not replaced with Object identity.

New focused coverage checks checked call selection for trait this and interface
values, resolved overload exclusion, and identity storage for own/inherited
defaults with no caller-created alias. `CheckedInterfaceEqualsProbe.java`
executes direct and interface-typed calls, mutation and repeat counts, null
acceptance, inherited implementations, caught throwing overrides and a null
interface receiver. It passes independently in Java and generated Rust.
V246 passed this probe but still had2 default-identity compiler diagnostics in
the complete witness. V247 failed to build due to a missing enum qualification;
no V247 gate is accepted.

Final V248 library1665/0 and Object executables8/0:
`.tmp/numbers-object-v248-regression-tests.log`. The complete, unchanged
`GenericObjectEqualityOracle.java` now compiles and executes successfully,
including heterogeneous/null/inherited/mutating/throwing overrides, canonical
default identity and sibling distinction, repeated generic field replacement,
and same-receiver reentrancy. It remains enabled, with no narrowed assertions,
blanket PartialEq/Clone constraints or repaired generated deliverables. Actual
multi-module runtime assembly also remains green. This closes that executable
witness, not all possible Java Object/interface/overload patterns.

Frozen producer `.tmp/new-java2rust-numbers-object-v248-debug`, SHA256
`977200dc7370e6651243313e1e5417b19eea825faa2e37c696c6c32b9c631714`;
build log `.tmp/numbers-object-v248-producer-build.log`. Formatting and diff
whitespace checks pass.

Fresh full Core V248 exercises all11 pinned files. The12 interface/default
equality diagnostics are removed; only3 compiler diagnostics remain, for
Norm's missing two/three/array payload fields. There are0 translation/parse
failures, unsupported statements/members/expressions, invalid-pattern files
or stubbed constants. Translation96764ms, assembly12223ms, total113817ms;
resolver benchmark skipped. Evidence:
`.tmp/numbers-core-v248-object-run.log` and
`.tmp/commons-numbers-1.3-core-v248-object/{summary.tsv,top-compile-errors.tsv,translation-logs/batch.stderr}`.
This is not Core acceptance or public-operation behavioral proof. The three
diagnostics describe one remaining representation/initialization family,
not three isolated field-emission edits. The earlier whole-path Norm audit
and constructor/alias/class-initialization requirements still apply.

Preservation using this producer passes Primes3594, Root Finder122 and RNG
source642865 exact Java/generated-Rust comparisons:
`.tmp/numbers-{primes,rootfinder}-v248-preserved-{run,behavior}.log` and
`.tmp/rng-source64-v248-numbers-{run,behavior}.log`.
The complete46-file sampling/native closure also translates and compiles with
0 unsupported/invalid/stubbed placeholders and preserves native12601,
sampling-collections162 and sampling-distributions2172 exact observations.
Together with source64 this preserves all17800 RNG comparisons. Evidence:
`.tmp/rng-sampling-all-v248-numbers-run.log` and
`.tmp/rng-{native,sampling-collections,sampling-distributions}-v248-numbers-behavior.log`.
All batch processes are terminal; no background result is being inferred from
an old status file. Core remains unaccepted pending initialized enum payloads
and the complete public-operation behavioral gate; later modules are unchanged.

## V249–V250: enum declaration and constructor-plan boundary

Revalidated the initialized-payload audit before backend work. The first
missing authority was declaration provenance: variant discovery treated every
self-typed enum field as a constant. That includes ordinary instance fields
and static aliases, which must not appear in values(), acquire an ordinal, or
be skipped as constant construction by class initialization. The parser also
dropped the construction expression for zero-argument constants, losing their
source execution site before constructor planning could observe it.

FieldDecl now records the enum-constant declaration flag (the IR's
EnumConstant modifier, analogous to the JVM enum-field flag), distinct from
public/static/final. Variant enumeration, parsed field facts, constant-body
dispatch/payload argument inventory, static-storage exclusion and initializer
materialization share that provenance. Ordinary self-typed fields/aliases are
no longer classified by their type spelling. Zero-argument constants retain
Expr::New, just like other construction sites. No constructor body is being
fabricated or treated as executed by this metadata change.

The shared constructor planner now exposes an ordered EnumInitializationPlan:
constant declaration/member position and ordinal, original argument AST and
constant-specific body, ordered instance fields, and the selected resolved
constructor declaration/call. It indexes the existing ConstructorKey authority
once, rejects duplicate/unresolved identities, and never selects the first or
an arity-only constructor. Implicit no-argument construction is distinct from
unresolved explicit construction. The ordinary resolver now admits an enum's
selected superclass constructor for a constant-specific anonymous body.
Same-arity int/enum overloads, an explicit zero-argument constructor, an alias,
a constant-specific body, implicit defaults and unrelated fields are tested.
Variant-name emission consumes the same declaration inventory without building
the full constructor plan merely to list names.

New independent Java/Rust `EnumDeclarationProbe.java` passes: values/name/
ordinal metadata, alias identity, mutable static selection and repeated values()
remain correct with same-typed ordinary fields present. A class-initializer
regression checks that ordinary enum-typed static fields/aliases remain in
their original ordered initialization stream rather than being skipped.

Promoted the original V198 EnumStateProbe into an enabled executable integration
gate. The versioned Java fixture is byte-identical to the diagnostic original
(cmp passes), and its Java self-check returns0. Its generated Rust still has13
missing calls/operator/index diagnostics:
`.tmp/numbers-enum-v250-executable.log`, generated source
`.tmp/test-tmp/new_java2rust_enum_state_1792625088084174613/src/lib.rs`.
The full gate is not ignored or narrowed. V249's initial integration harness
failed to compile because a temporary formatted Rust driver did not satisfy
the shared helper's static-string contract; V250 uses static drivers. This
test setup correction did not change production code.

Verified current-production gates: library1668/0 and Numbers21/0 in
`.tmp/numbers-enum-v249-regression-tests.log`; enum executables1/1 in
`.tmp/numbers-enum-v250-executable.log` (positive declaration-role probe passes,
complete initialized-state witness fails as described). Formatting and diff
whitespace checks pass. Frozen producer
`.tmp/new-java2rust-numbers-enum-v250-debug`, SHA256
`86ca362a68142c21f5daf125dacb3c6c42845490917add78dbddc33485c03708`, build log
`.tmp/numbers-enum-v250-producer-build.log`.

Backend construction/publication is still pending. Retain unit discriminants
and one default-allocated, then source-initialized payload per constant; reuse
solved field carriers, constructor parameter/delegation plans and the existing
ordered instance_field_initializer_statements execution phase. Lower selected
constructor bodies against that constant's identity, not a reconstructed Rust
struct literal or a getter-time AST evaluator. Own/delegating constructors must
initialize fields exactly once, including aliases copying callback handles.
Payload field operations must release state loans before Java callbacks.

Materialize enum construction in the existing typed class-initializer callable
and key the barrier on actual initializer presence, not just explicit static
blocks. Publish each static enum reference only after its constructor returns;
same-thread reentrant early reads must retain Java's null/default state. The
values-array publication boundary is after constants and before later static
field/block work, not an unconditional list of discriminants during reentry.
This is the next coherent storage/construction/consumer batch. The metadata
plan does not yet replace legacy match-backed accessors or enable eager enum
construction. No fresh full Core/RNG run is claimed for this metadata boundary;
V248 remains the last such measurement/preservation checkpoint, and Core is
still unaccepted with the recorded3 Norm payload diagnostics.

## V251–V261: eager enum storage and typed callback checkpoint

The enum backend now retains unit discriminants for identity and allocates one
default-initialized payload per constant in process-wide OnceLock/Mutex storage.
The typed class-initializer stream executes each resolved constructor and then
publishes its reference. Runtime initialization barriers are keyed by actual
initializer requirements, not only explicit static blocks. Field reads copy
their solved carriers under a short loan; source callbacks run after that loan
has ended. The existing constructor argument/delegation planner and ordered
instance-initializer lowering execute the source bodies. A hidden enum-instance
parameter supplies this/name/ordinal during construction. Accessor-backed
primitive updates stage the receiver once and reuse Java's wrapping/narrowing
prefix/postfix update implementation before storing the result.

These changes are not yet an accepted backend. V252 cargo check --tests passes,
but its two enabled enum executables fail with8 and19 diagnostics. V253 restores
the declaration-role executable; the new independent EnumConstructionProbe
also runs its Java self-check, but generated Rust still fails. This probe adds
same-arity overload selection, this-constructor delegation, once-only instance
initializers, source side effects, reentrant reads of unpublished constants,
metadata and mutable state. It supplements, not replaces or narrows, the full
original EnumStateProbe.

V253/V254 output exposed contract bypasses rather than a reason to patch Java:
nullable element getters borrowed the Option shell, constructor metadata calls
could escape through slot-directed shortcuts, nested enum constants were
described by nominal non-null shapes, and nullable field/call paths reconstructed
struct fields or used nominal receiver inference. The corrective batch shares
physical enum read/accessor shapes with the existing nullable receiver and slot
conversion plans. V255 is the next focused verification checkpoint. Logs:
`.tmp/numbers-enum-v252-check.log`, `.tmp/numbers-enum-v25{2,3,4,5}-executable.log`,
and `.tmp/numbers-enum-v254-receiver-audit.log`.

The full callback witness additionally identifies a separate native-SAM storage
gap. DoubleUnaryOperator currently retains Rc<RefCell<dyn FnMut>>, which cannot
inhabit process-wide synchronized enum storage. The facade's generated Eq
implementation also returns true for every pair and its Hash implementation
does nothing; these are not valid callback identity semantics. A correct general
solution must preserve allocation identity on aliases and propagate synchronized
escape requirements through callback producers, captures, storage and consumers.
Simply replacing Rc with Arc globally would reject existing Rc-capturing
callbacks (see functional_carrier_clones_share_state_without_payload_clone_bounds
and the applied iterator-factory executable), while thread-local enum payloads
or unsafe Send would evade the actual Java contract. Keep the original gate
enabled. Norm uses source SAMs, so independently measure its real contract rather
than assuming this native-facade gap proves Core cannot progress.

V256 completes the related enum consumer batch. Library1668/0 and Numbers21/0
pass in `.tmp/numbers-enum-v256-regression.log`; the independent declaration and
constructor/publication executables both pass. The original full native-callback
state executable is enabled and fails with exactly one compiler diagnostic,
the Rc-based callback's missing Send contract. This is not a passing complete
enum gate and does not address the equality issue described above. Frozen
producer `.tmp/new-java2rust-numbers-enum-v256-debug`, SHA256
`041f5cfcc77105928d769ad6d3b0e07f78ef4ad915e864c648e9347afc00ced6`.

Full Core with that exact producer translates all11 untouched pinned files;
translation95713ms, assembly11952ms, cargo check1702ms, total154107ms including
the39969ms resolver benchmark. `.tmp/numbers-core-v256-enum-run.log` and
`.tmp/commons-numbers-1.3-core-v256-enum/` record the result. The previous3 missing
Norm payload fields are gone; compilation now exposes9 unresolved overloaded
method-reference expressions in its eagerly executed initialization stream.
There are no parse failures or invalid-pattern markers. Core remains unaccepted.

The next whole-path audit is concrete: constructor slots already request
Box<dyn Norm_Two/Three/Array> and source method bodies consume the nullable
stored callbacks correctly, but creation still supplies raw Norm::manhattan/
euclidean/maximum paths rather than typed trait implementations for the exact
overloads. Minimal correct Rust wraps each selected overload as a boxed typed
closure using the SAM descriptor, retaining the array input's physical carrier
and checked-result ABI. lower_functional_slot_lambda already does this for
source lambdas but admitted method references only for native facades.
lower_functional_callback_with_contract_impl inferred method-reference inputs
and results only from native descriptors. Generalize these existing authorities
to source method references, not an enum- or Norm-specific adapter. The V257
candidate shares the source SAM's solved input/result contract and ordinary
typed overload/call lowering; the new SourceSamReferenceProbe independently
checks two/three/array overloads, alias constructors, repeated calls after array
mutation and a local typed method reference. Its executable result is pending
in `.tmp/numbers-enum-v257-executable.log`.

V257's witness rejects4 bare-closure-to-trait casts: only a coherence-restricted
zero-argument blanket implementation exists, and synchronized source interfaces
deliberately do not receive even that blanket. This rules out expanding the
blanket shortcut. V258 generalizes the existing anonymous functional adapter's
item emitter with an allocation/receiver policy, preserving its existing local
call sites. Typed source callbacks use nominal adapters and the existing
source_interface_requires_send decision: local Rc<dyn Fn> or shared
Arc<dyn Fn + Send + Sync>. Cloning retains the callback allocation; invocation
holds no state mutex. This is not yet proof of the erased-object/reference
identity protocol, which still needs separate coverage.

V258 gets the new witness down to one physical-array closure mismatch. Its
adapter and trait expect Rc<RefCell<Vec<f64>>>, but the generated method-reference
lambda annotates Vec<f64>. callback_lambda_param_shape was replacing the solved
SAM input slot merely because the generated lambda also carried an explicit
Java double[] spelling. V259 preserves the matching descriptor slot instead,
sharing the input-shape authority across closure environment, parameter emission
and invocation. Both source SAM contracts and nominal adapters use the existing
trait parameter/return shape planners rather than a new parallel signature
calculation. `.tmp/numbers-enum-v258-executable.log` records the observed
E0631; `.tmp/numbers-enum-v259-regression.log` is the pending combined check.

V259 combined results: library1668/0 and Numbers21/0 pass again. Declaration
and constructor/publication probes pass. Native EnumStateProbe remains red on
Send; SourceSamReferenceProbe now has exactly one explicit unsupported native
allocation conversion, not the former missing trait implementation/array closure
signature errors. Its typed retained-array input reaches an ordinary read-only
source helper whose valid ABI is a borrowed slice. The generic value converter
correctly refuses to detach Rc<RefCell<Vec<_>>> into an owned Vec, but the call
boundary needs an identity-preserving short payload loan instead.

Fresh full Core with frozen V259 producer
`.tmp/new-java2rust-numbers-enum-v259-debug`, SHA256
`7df76072d66a4e692aeee9ba37da9743dee23c4be2af1221d07f9a53121264e2`,
resolves all9 overloaded references. It now reports exactly2 explicit unsupported
conversions at the same retained-array-to-borrowed-slice boundary (manhattan and
maximum). All11 sources translate and parse; these2 compile_error sites also
account for the invalid-pattern scan. No unsupported members, stubbed constants
or no-op facade call sites are reported. Translation114425ms, assembly11939ms,
cargo check1484ms, total132363ms; resolver benchmark deliberately skipped, so
total is not a directly comparable speed claim. Evidence:
`.tmp/numbers-core-v259-enum-run.log`,
`.tmp/commons-numbers-1.3-core-v259-enum/{compile.log,phase-events.tsv,status.tsv}`.

The V260 corrective candidate admits array payload loans only at borrowed
argument boundaries, for matching Java array types and identical rendered
physical payloads. It shares native_allocation_sequence_receiver and the
existing RefCell/Mutex payload projection, preserving optional null checks and
mutable/shared borrowing. It does not clone array contents, alter caller
storage, fabricate an owned conversion, or change the generic converter's
rejection of unsupported ownership changes. Shaped and explicit constructor/
method argument paths share this authority. Focused verification is pending in
`.tmp/numbers-enum-v260-executable.log`.

V260's translator build caught a moved ViewShape in the new payload comparison;
V261 clones that descriptor. Focused V261 verification then passes3/4 enabled
executables: declaration roles, complete constructor/publication probe, and
SourceSamReferenceProbe including mutated retained arrays and local references.
The unchanged complete native-callback witness alone fails on the known Send
diagnostic. `.tmp/numbers-enum-v261-executable.log` records both passes and the
remaining failure; it is not ignored. Frozen V261 producer
`.tmp/new-java2rust-numbers-enum-v261-debug`, SHA256
`998c203051c37871c34b06f400c85a7444fb35e004334332de00528e36be0f4a`.

Fresh complete Primes and Root Finder translation/check plus their independent
3594 and122 exact observations pass with that producer:
`.tmp/numbers-{primes,rootfinder}-v261-preserved-{run,behavior}.log`, output roots
`.tmp/commons-numbers-1.3-{primes,rootfinder}-v261-preserved`.
V261 full Core, library/Numbers and RNG preservation are being measured in
`.tmp/numbers-core-v261-enum-run.log`, `.tmp/numbers-enum-v261-regression.log`,
`.tmp/rng-sampling-all-v261-numbers-run.log` and
`.tmp/rng-source64-v261-numbers-run.log`. No completion of these pending checks
or complete Core behavioral acceptance is implied by the focused probes.

### V261 full Core compilation checkpoint (not module acceptance)

The full maintained-runner Core gate now passes all11 files with the frozen
V261 producer above. Translation116979ms, assembly25046ms, cargo check1809ms,
total149638ms; resolver benchmark skipped. Compiler errors0, parse failures0,
unsupported statements/members/expressions0, invalid-pattern files0, stubbed
constants0, no-op facade call sites0. These are translation/compile/scan results,
not proof of all reachable runtime behavior. The runner still inventories
unused facade scaffolding; do not confuse its definition count with reachable
calls or treat the scanner as a complete semantic audit.

V261 library1668/0 and Numbers21/0 pass in
`.tmp/numbers-enum-v261-regression.log`. The independent enum/callback target
remains explicitly3 passing/1 failing as recorded above. Original RNG source64
passes its full11-file translation/check and2865 exact behavior observations in
`.tmp/rng-source64-v261-numbers-{run,behavior}.log`. The46-file sampling/native
closure also passes translation/parse/check with0 unsupported members,
invalid-pattern files and compiler errors. Fresh native12601, collection
sampling162 and distribution sampling2172 comparisons pass in
`.tmp/rng-{native,sampling-collections,sampling-distributions}-v261-numbers-behavior.log`.
Together with source64 this preserves all17800 RNG observations with the same
frozen producer, including exact floating-point bits and callback/RNG traces.
All verification jobs are terminal; this is measured preservation, not an
inference from V248 or from the focused regressions.

Reproduction uses the maintained runners with NEW_JAVA2RUST_BIN pointing to
the frozen V261 producer, NEW_JAVA2RUST_BUILD_PROFILE=debug,
CARGO_INCREMENTAL=0, CARGO_NET_OFFLINE=true and
TMPDIR=$PWD/.tmp/test-tmp. Set the named NUMBERS_MODULE/NUMBERS_TRANSLATION_ROOT
or RNG_TRANSLATION_ROOT above and pass --skip-resolver-bench. Behavior commands
are `python3 scripts/verify-commons-rng-1.0-behavior.py` with --numbers-primes,
--numbers-rootfinder, --native, --sampling-collections or
--sampling-distributions and the corresponding output root; omit the mode for
source64. Library/Numbers verification uses
`cargo test --lib --test numbers_contracts --no-fail-fast`. The full enabled
`cargo test --test enum_initialization_contracts -- --nocapture` deliberately
still returns failure for the unresolved native callback Send contract.

Next required phase: inventory and independently exercise every Core public
operation/overload, with exact Java/Rust values by default, exceptions, invalid
inputs, callbacks, object/alias state, repeated calls and numerical boundary
branches. Include DD/DDMath, ArithmeticUtils, Precision/DoubleEquivalence,
Norm's six constants/three overloads, Sum, and the generic arithmetic interface
operations/defaults. Re-audit any newly exposed family from its generated
producer/storage/signature/consumer contract. Resolve the still-enabled native
SAM synchronized-escape/identity gate; do not hide it or relabel the three
positive witnesses as complete enum acceptance. Only Primes and Root Finder
are accepted; Core and all later modules remain incomplete.

No fresh accepted-module/RNG preservation is claimed for this in-progress
backend. V248 remains the last such checkpoint; only2 of6 modules are accepted.
Further remaining enum contracts include synchronized native callback escapes
and identity, array-field consumers and nullable external field writes. Do not
infer general initialized-enum correctness from the two positive witnesses.

## V262–V264: Core independent behavior baseline and numeric native-call batch

After V261's complete compile gate, added a caller-only Core Java/Rust oracle
and a maintained Java runner. It links the untouched generated production
library and its exact built java_math_runtime dependency; no emitted source or
upstream production source is repaired. The generic behavior verifier has an
explicit --numbers-core mode, a fixed5357-observation scope, a runtime timeout,
and full mismatch collection for batching. Collection still rejects every
changed key/value, signed zero, raw NaN encoding, state or exception; tests
assert it does not relax comparison. No tolerance has been introduced.

`Core-1.3-declarations.txt` records753 lines of complete production descriptor
inventory, including17 classes/interfaces, private helpers, constructors,
fields, initializers and compiler bridges. Reproduce with
`bash scripts/list-commons-numbers-1.3-core-api.sh`. All11 production source
files remain in the pinned manifest, JDK-only closure. Compiler bridges are
listed but are not additional source overloads. The descriptor inventory is
not itself behavioral coverage proof.

The first oracle caller covers all14 ArithmeticUtils operations; Norm's six
constants, values/valueOf and three numeric overloads; Precision's static
overloads and DoubleEquivalence operations; Sum factories, scalar/array/product
updates, self aliases and repeated state; DD factories, conversions, arithmetic,
identity/equality/hash/string, output exponent arrays and both power forms;
DDMath.pow; and generic arithmetic interface dispatch through DD. Inputs include
signed zero, subnormals, extreme finite values, infinities/NaN, negative/zero
exponents, integral overflow, divide-by-zero, empty/mismatched arrays, mutable
array outputs and cancellation. This is an initial public-API caller, not a
claim of complete branch/boundary coverage: null-reference boundaries,
adjacent-ULP matrices, broader scaling/power ranges, generic default bodies
distinct from DD overrides and numerical branch coverage still require audit.

All5357 observations execute to oracle=ok for both Java and frozen V261 Rust,
but180 exact comparisons fail:
`.tmp/numbers-core-v262-behavior-baseline.log` and
`.tmp/commons-numbers-1.3-core-v261-enum/behavior-numbers-core/differences.txt`.
The maintained compile scanner reported0 no-op call sites, yet reachable
unimplemented numeric facade bodies remain. Compilation and that scanner were
therefore insufficient for acceptance, as the objective anticipated.

| Shared family | Baseline observations | Required producer-to-consumer contract |
|---|---:|---|
| Integer unsigned division/remainder |22 | Reinterpret the32-bit inputs as unsigned, preserve quotient/remainder bit patterns on return, and signal ArithmeticException on zero divisor. Extend the same width-parametric native operation to Long, not a Core-only rewrite. |
| Math.multiplyExact |62 | Use selected int/long overload, stage arguments once in Java order, checked multiplication and the exact integer/long overflow exception. Propagate checked results through the existing call/exception mechanism, including caught and uncaught calls. |
| Math.getExponent |16 | Reuse typed native dispatch, preserve float/double overloads and zero/subnormal/infinity/NaN exponent sentinels. No logarithmic approximation. |
| Math.copySign |70 | Preserve float/double widths and raw sign transfer, including signed zero. Route imports, qualified owners and ordinary typed calls through the same native plan. |
| Integer.toUnsignedLong/imports |5 | Zero-extend32 bits before widening to signed long; carry the true Java result type into constructor/return conversions instead of an inferred facade placeholder. |
| Enum.valueOf failure |1 | Unknown names currently panic with a Rust string instead of the qualified Java IllegalArgumentException contract. This separate metadata/exception gap is not fixed by numeric dispatch. |
| Arithmetic NaN encodings |4 | Sum self-subtraction and DD reciprocal/power produce NaNs with different raw sign bits. Investigate producer arithmetic and Java/JVM guarantees before any comparison policy decision. All four remain failures. |

V263/V264 implement the first five families through NativeJavaStaticMethod,
the existing argument-sensitive JDK result-type authority, shared typed Rust IR
numeric projections, and the existing fallibility traversal/result discharge.
The former Math.toIntExact-specific traversal/reason is generalized to checked
native arithmetic (it already included floorDiv/floorMod), not duplicated.
Both plain catchable-panic and propagating Result contexts share one checked
numeric projection. Source algorithms and Java inputs are unchanged.
The semantic references are the official
[JDK17 Math API](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/lang/Math.html)
and [Integer API](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/lang/Integer.html).

The independent NativeNumericProbe passes with V264: mixed integral widths,
operand evaluation order, unsigned high-bit values and zero division, checked
overflow messages across calls, float/double exponent sentinels, character
promotion, signed-zero copySign and static imports. V263's cheap check caught
an incorrect local variable name in return metadata; V264 corrects it. A second
enabled witness verifies source Math/Integer lookalikes retain their own
declared return contracts alongside fully qualified JDK calls. The combined
library/Numbers/native-numeric run is pending in
`.tmp/numbers-native-numeric-v264-regression.log`. Behavior comparison tests6/0
pass in `python3 scripts/test_rng_behavior_comparison.py`.

Fresh full Core translation is pending in
`.tmp/numbers-core-v264-numeric-run.log`. Its independent Rust caller only
adapts the unsigned-int call sites to their newly correct checked Result ABI;
it does not implement division or alter the expected Java observations.
Keep all180 baseline discrepancies visible until fresh execution measures the
actual change. Core remains unaccepted; only2 of6 target modules are accepted.

### V264 measured runtime checkpoint

Frozen producer `.tmp/new-java2rust-numbers-numeric-v264-debug`, SHA256
`56e623fca71fb1ad646d0ea54155f88e84501f4c86b7ce9e12918b3bf0807477`.
All11 untouched Core files translate/parse/check with0 unsupported syntax,
invalid-pattern files and compiler errors. Translation136913ms, assembly12503ms,
cargo check1714ms, total154150ms with resolver benchmark skipped. Library1668/0,
native numeric2/0 (including source Math/Integer lookalikes), Numbers21/0 and
comparison unit tests6/0 pass. Logs are the V264 run/regression paths above.

The fresh independent Core oracle still executes all5357 observations to
oracle=ok. Exactly175 former facade mismatches are fixed;5 exact mismatches
remain in `.tmp/numbers-core-v264-behavior.log` and
`.tmp/commons-numbers-1.3-core-v264-numeric/behavior-numbers-core/differences.txt`:
unknown enum lookup, two Sum self-subtraction NaN signs, DD's tiny-input negative
power NaN sign and minimum-subnormal reciprocal NaN sign. No comparator or
expected Java observation was relaxed to obtain this reduction. The full
behavior command still fails, correctly; Core is not accepted.

Investigated NaN behavior without changing either implementation: the installed
reference JVM is OpenJDK19.0.2+7-Ubuntu-0ubuntu322.04. Run the same compiled,
untouched Java Core/driver with `java -Xint` and with
`java -Xcomp -XX:TieredStopAtLevel=1`, keeping the normal classpath/main class.
The5357-line outputs are `.tmp/numbers-core-v262-java-interpreted.txt` and
`.tmp/numbers-core-v262-java-c1.txt`. Compared with the saved V264 Java reference,
interpreter mode changes17 observations and C1 changes9. Every changed component
is exclusively a NaN sign bit; no finite value, signed zero, exponent, state,
exception, key or trace changes. All four Rust-vs-default-Java NaN differences
are reproduced by C1 Java. This is concrete JVM-mode-dependent representation
evidence, not a reason to rewrite numeric algorithms or approximate finite
results. The comparator remains exact; any portability policy must explicitly
preserve all other values/state and be justified/tested separately.

The enum lookup discrepancy is a real exception/metadata issue: source enum
value_of panics with an untyped Rust string and an unqualified emitted owner.
Correct failure needs JavaException IllegalArgumentException and the canonical
Java source type name. Reuse/generalize source owner/class-name metadata across
top-level, nested, packaged and co-translated types rather than hardcoding Norm
or guessing packages from Rust identifiers. Audit the pure-enum runtime support
dependency and caught/uncaught paths as well. Existing source_object_java_class_name
and JavaOwnerKey::source_type_in_program helpers currently treat dotted names as
already qualified; nested source paths and merged compilation-unit provenance
must be distinguished before adopting either unchanged.

Fresh V264 Primes3594, Root Finder122 and RNG source642865 observations pass
exactly after their complete translation/compile gates. The46-file RNG closure
also compiles with0 unsupported members/parse failures/invalid-pattern files/
compiler errors. Native12601, collection162 and distribution2172 exact
comparisons all pass in `.tmp/rng-{native,sampling-collections,sampling-distributions}-v264-numbers-behavior.log`;
together with source64 this freshly preserves all17800 RNG observations.
All launched verification processes are terminal. Reproduction is the V261 recipe with the frozen
V264 producer and corresponding output roots. Later modules remain untouched;
Core coverage expansion, the strict remaining discrepancies and the previously
enabled native-SAM synchronized escape/identity gate are all still required.

## V265–V271: source enum lookup and runtime-name contracts

The V264 enum failure required a cross-function contract fix, not just a
different panic message. A minimal wrapper returning `Choice.valueOf(name)`
had an unresolved external `valueOf` call-graph edge. Its caller's Java catch
used Result control flow, while the emitted lookup panicked outside it.
The required rewrite is a real checked enum runtime returning JavaException,
a typed descriptor for javac's implicit method, and ordinary transitive
fallibility propagation. No synthetic source MethodDecl/body is supplied.

V271 implements that checked runtime with a preserved plain Rust caller
wrapper. Unknown names return IllegalArgumentException with the canonical
Java enum owner; null names return NullPointerException with `Name is null`.
The resolver and fallibility seed share the runtime method identity. Removed
the older source-enum valueOf shortcut, which ignored selected overloads.
Declared `valueOf(int)` and the implicit String lookup share the existing
overload counter in both symbol construction and final resolution.

Runtime names now retain parse-time canonical and binary provenance on named
TypeDecls, independent of internal qualification and Rust collision markers.
Enum errors use canonical dots; Object.getClass().getName() uses binary `$`
nesting, preserving literal dollars in identifiers. Enum name()/lookup labels
come from original Java constants, not Rust variant spelling. Pure enums
also explicitly request the JavaException runtime dependency.

The expanded witnesses found two other whole-path defects. Qualified owner
paths represented as FieldAccess chains were missed by merged-name repair;
the reference rewrite now recognizes the complete qualified path. Nullable
String slots could render as unsized Option<str>, and their borrowed payloads
could be prematurely tagged as str before adaptation from String. The shared
payload-shape normalizer is used by both renderers and optional conversion,
so storage, borrowed projection and consumer view agree.

Independent javac/generated-Rust witnesses are enabled in
`tests/enum_lookup_contracts.rs`: packaged/nested/top-level names, lowercase
constant labels, reusable strings, missing/null names, exceptions through a
source wrapper, declared integer overloads, binary nested class names, pure
enum runtime dependencies, and a production multi-file assembly with a.b.C,
x.C and a colliding root ABC. V267 measured1669/0 library and21/0 Numbers but
failed the original enum exception boundary. V269/V270 exposed the additional
contracts above; V270's merged identity and pure-enum witnesses pass, while
the larger wrapper/overload witness still fails. None is skipped or weakened.
A new library regression checks owned/copy/shared/mutable nullable String
rendering through both rendering entry points.

Frozen V271 producer `.tmp/new-java2rust-numbers-enum-names-v271-debug`, SHA256
`09f547bca2ffdcfca77ead6611f4d42f13f34f69209cc9bae55552b8b43e0562`.
Combined verification in `.tmp/numbers-enum-names-v271-regression.log` passes
library1670/0, enum lookup3/0, native numeric2/0 and Numbers21/0. Complete Core
translation in `.tmp/numbers-core-v271-enum-names-run.log`, output root
`.tmp/commons-numbers-1.3-core-v271-enum-names`, translates/parses/checks all11
files with0 unsupported members/statements/expressions, invalid-pattern files,
compiler errors, no-op facade call sites and stubbed constants. Translation
112235ms, assembly12260ms, check1665ms, total131963ms; these timings are a
checkpoint, not a controlled performance claim.

Fresh independent behavior executes all5357 observations to oracle=ok on both
sides. Norm's invalid lookup now exactly matches the Java exception class and
full canonical message. Four exact NaN-sign mismatches remain unchanged in
`.tmp/numbers-core-v271-behavior.log` and the output root's
`behavior-numbers-core/differences.txt`; the full behavior command still fails.
No NaN comparison or expected value was changed. The numerical portability
policy and remaining coverage audit are not completed.

Primes3594 and Root Finder122 freshly pass their complete translation/compile
and exact behavior gates with V271, in
`.tmp/numbers-{primes,rootfinder}-v271-preserved-{run,behavior}.log`.
The46-file RNG closure freshly compiles with0 compiler errors in
`.tmp/rng-sampling-all-v271-numbers-run.log`. Attempting to reuse that crate
for the unchanged source64 caller exposed a harness ABI incompatibility:
NumberFactory's int/byte-array parameters are retained handles in the larger
closure, but the source64 caller expects the smaller closure's owned Vec ABI.
The caller build correctly fails in `.tmp/rng-source64-v271-numbers-behavior.log`;
this is not a measured numerical/translator regression. No source or generated
method was patched to force reuse. The unchanged11-file gate is instead
running in `.tmp/rng-source64-v271-numbers-run.log`, with behavior log
`.tmp/rng-source64-v271-isolated-numbers-behavior.log`. The other three RNG
drivers reuse the46-file artifact, in mode-specific
`.tmp/rng-*-v271-numbers-behavior.log` files. Reproduction follows the V261
recipe with these producer/output paths; source64 uses its isolated root.
All preservation jobs are now terminal: native12601, collections162,
distributions2172 and isolated source642865 exact observations pass, preserving
all17800 RNG observations with the same frozen producer. The11-file source64
translation/parse/check gate also passes with0 unsupported syntax, invalid
patterns, no-op facade call sites and compiler errors. The failed shared-crate
source64 caller experiment remains recorded separately, not relabeled a pass.

### Next whole-path audit: inherited Object operations on native SAM classes

The fresh scan still reports the same21 unmodelled call sites as V264. It
also finds a real acceptance blocker beyond the initial numerical caller:
generated Sum has an inherent `to_string` body that panics with
`java.util.function.DoubleConsumer.to_string is not modelled`, and its Display
implementation delegates to that body. This is reachable, not unused shared
facade inventory. Do not describe the compile gate as proving no fake bodies.

Independent diagnostic consumers confirm it without altering generated code:
`.tmp/NumbersSumObjectDisplayProbe.java` passes against untouched compiled Core,
checking Object.toString's class/hash contract and repeat stability;
`.tmp/numbers_sum_object_display_probe.rs` calls the generated Sum and panics
at that placeholder. Logs are `.tmp/numbers-core-v271-{java-sum-display,sum-display}-probe.log`.
The Rust consumer links the actual library from the Core behavior artifact
manifest, exactly as the maintained behavior driver does.

The rewrite audit must trace synthesized native-interface inherited members
into occupied-method accounting and Object fallback/Display selection.
`collect_source_object_to_string_fallback_items` already provides the shared
Object-operation boundary but will not replace an occupied inherent method.
Its existing default `class@0` formatting is also not a sufficient identity
implementation. Reuse/generalize existing object identity/hash and runtime
class-name mechanisms; do not replace the panic with another fabricated
constant result. Cover native SAM implementations, ordinary source classes,
explicit overrides, inherited overrides and alias/repeated-call identity.
The relevant shared entry points include
`stdlib::query_backed_external_inherited_methods_for_type`,
`stdlib::is_known_object_inherited_method`, and runtime JavaReferenceIdentity
whose Rc/Arc tokens identify allocations rather than handle-slot addresses.
This connects to the previously enabled native-SAM escape/identity audit,
but requires its own concrete ownership and signature evidence before edits.
Core remains unaccepted; only2 of6 target modules are accepted.

## V272–V273: canonical Object display batch in progress

The minimal correct rewrite for a default source Object.toString is a checked
operation on the same reference carrier used by JavaHashCode:
`JavaHashCode::java_hash_code_result(reference).map(|hash| format!("{}@{:x}", binary_class_name, hash))`.
It must call an overriding hashCode exactly once and forward its exception;
neither a Rust structural hash nor a movable payload/handle-slot address is
the default identity. It must not retain a RefCell/Mutex payload loan while
calling source code. Existing canonical hash and checked display adapters
already express this contract, so this batch reuses them rather than adding
another identity representation.

The facade inventory had two producers for inherited native-interface methods:
fact-backed and query-backed. Both now leave Object operations on known native
functional interfaces to the source Object protocols, instead of emitting an
unmodelled DoubleConsumer.to_string body into the source class's occupied
inherent methods. Explicit/inherited source methods still own their bodies.
The checked source display adapter uses virtual JavaHashCode for default
display, preserving a hash override's bounds and Result ABI.

Source call sites snapshot the reference before dispatch and use the same
checked display adapter for concrete default calls and display contexts; null
receiver calls fail while display contexts format null as text. Non-null
source toString overrides can share this route, but nullable-return overrides
must keep their original Java return ABI, not normalize a returned null to
the string "null" in a direct call. The existing virtual Object effect walk
also admits source Object display calls, since default display can invoke a
throwing hash override.

V272's first independent NativeObjectDisplayProbe passes javac but exposes
eleven generated-Rust call-site errors. Canonical protocol bodies themselves
are emitted, but strict context lookup rejects the primary declaration when
its summary copy is also present, leaving calls on Rc<RefCell<T>> as raw Rust
to_string calls. V273 reuses the existing preferred-emitted declaration lookup
instead. It also admits value receivers with a real supported hash override;
only default identity hashing requires a retained allocation. The witness
covers ordinary and native-SAM classes, alias/repeat/mutation stability,
explicit/inherited display overrides, virtual hash counts, throwing hash,
erased display, concatenation, and nulls. The focused run is being measured in
`.tmp/numbers-object-display-v273-focused.log`; no passing result is inferred.

The raw payload Rust Display bridge still needs a separate whole-path audit:
it cannot recover a reference's canonical token from &Self. In particular,
do not claim that removing the imported placeholder alone makes class@0
correct, or silently use a payload pointer as a replacement identity. Core
is not accepted and no complete-corpus rerun has been done for this batch yet.

V273's witness then compiles and executes but returns5: direct default display
uses the canonical reference, while erased Object display still borrows and
formats the payload. V274 factors source erased display through one method
selection function for checked and plain registry entries. Default retained
source classes call the canonical checked display protocol; real source
overrides keep their nullable/Result return contract. All9 enabled
object_equality_contracts tests pass, including the new independent witness,
in `.tmp/numbers-object-display-v274-regression.log`.

Frozen producer `.tmp/new-java2rust-numbers-object-v274-debug`, SHA256
`2eb61ad6599d392295e410e29d44373dc3ed383d8f2058222700ce2ff8213bf0`.
The library/Numbers/enum/numeric batch is being measured in
`.tmp/numbers-object-display-v274-library-regression.log`. Fresh complete Core
translation is being measured in `.tmp/numbers-core-v274-object-run.log`,
output root `.tmp/commons-numbers-1.3-core-v274-object`. The independent Core
oracle now contains5361 observations: four additional checks exercise Sum's
Object display/hash consistency, alias display, stability after mutation,
and erased reference display. They call emitted production protocols and
operations, not replacement numerical algorithms. Exact old comparisons
remain enabled. The comparator unit tests still pass6/0.

The measured V274 library batch passes1670/0, enum lookup3/0, native numeric2/0
and Numbers21/0. Fresh Core translates/parses/checks all11 files with0
unsupported syntax, invalid patterns, no-op facade call sites and compiler
errors. Translation138842ms, assembly13879ms, check1728ms, total157765ms.
Sum no longer contains the unmodelled DoubleConsumer.to_string body;
unmodelled call-site markers drop from21 to20. Both independent callers
complete5361 observations to oracle=ok. All four new Sum object observations
are true and match. The same four exact NaN-sign discrepancies remain in
`.tmp/numbers-core-v274-behavior.log`; the full command still correctly fails.
No comparison was relaxed. Preservation gates are running in the V274
Numbers/RNG run and behavior logs, using the previously validated isolated
source64 ABI rather than retrying the failed shared-crate caller experiment.

Preservation is now measured and all jobs are terminal: Primes3594 and Root
Finder122 pass after their complete translation/compile gates. The46-file RNG
closure translates/parses/checks with0 unsupported syntax, invalid patterns or
compiler errors (translation126449ms, assembly33586ms). Native12601,
collections162 and distributions2172 exact observations pass against that
artifact; isolated source642865 also passes after its full11-file gate.
Thus all17800 RNG observations remain exact with the frozen V274 producer.
The logs are `.tmp/numbers-{primes,rootfinder}-v274-preserved-{run,behavior}.log`,
`.tmp/rng-{sampling-all,source64}-v274-numbers-run.log`, and
`.tmp/rng-{native,sampling-collections,sampling-distributions,source64}-v274-numbers-behavior.log`.
Formatting and whitespace checks pass. Core remains unaccepted for the
documented numerical-policy, coverage, callback and remaining display gates.

### Remaining display consumers: append(Object) and payload bridges

A separate cheap diagnostic identifies the next consumer gap before another
implementation batch. Untouched `.tmp/NativeAppendProbe.java` passes javac and
prints0; the frozen V274 translator emits Rust that compiles but its independent
consumer returns1. The Java class implements DoubleConsumer and overrides
hashCode with an observable counter and42. StringBuilder.append(Object) must
produce `displayprobe.Item@2a` and invoke hashCode once. Generated append passes
the canonical Rc<RefCell<Item>> to JavaAppendValue, whose current blanket impl
borrows the payload and uses Rust Display, reaching class@0 instead. This
explains the mismatch without changing any generated body.

Artifacts: `.tmp/native_append_v274_generated.rs`,
`.tmp/native_append_v274_probe.rs`, `.tmp/native-append-v274-{translate,build,rust,java}.log`.
Reproduce by invoking the frozen producer on the Java source, then rustc2024
on the separate caller, or javac/java on the untouched source.

Next batch must carry the selected Java append overload and its checked
display/effect contract through argument planning and runtime consumers.
Do not stringify every source argument: append(CharSequence) and append(char[])
have different operations from append(Object). Keep receiver/argument order,
no storage loans across callbacks, nullable values, throwing hash/toString,
and already-covered nullable-return overrides. Source identity/storage
inference also currently explicitly recognizes hashCode calls; audit implicit
default Object display and generic/native argument requirements through the
existing StorageFactContext requirement caches before adding another scan.
The remaining raw-payload Display bridge cannot supply canonical allocation
identity by itself; do not replace it with another constant or pointer guess.
These are remaining general translator contracts, not additional corpus scope.

### V275–V285: checked native append and source/native name coexistence

The append audit above requires a cross-function contract repair, not a new
payload Display implementation. `java_append_plan` now supplies the shared
typed native-receiver/Object-overload proof to lowering, exception planning,
storage inference and generic capability inference. CharSequence, primitive,
String and array overloads keep their distinct lowering. Lowering stages the
receiver reference and argument reference once, in order, checks the receiver,
then calls the existing checked Java object-display protocol without retaining
a payload loan. It appends the resulting text through the native runtime.
Reference staging uses existing reference-copy projections; generic locals
are borrowed rather than requiring an unnecessary Clone bound. Generic field
copies and erased generic display propagate their existing Clone/JavaAny/
reference-identity requirements through the established requirement machinery.

The executable untouched-source witness is
`tests/corpus/numbers/ObjectAppendProbe.java`, compared independently with
javac by `tests/native_append_contracts.rs`. It covers both native builders,
canonical default display with observable hash calls and aliases, erased and
generic values, null objects and nullable toString returns, distinct native
overloads, throwing hash/toString, reentrant append and receiver/argument/null
ordering. This witness passes in V280, V281 and V282; those revisions are not
claimed broadly green.

The second witness, `SourceAppendProbe.java`, deliberately puts a source
StringBuilder beside an explicitly qualified java.lang.StringBuilder and
keeps its Java main/PrintStream call. The source append(Object) must not invoke
the argument's throwing toString; the native append must invoke it. This
exposed previously lossy native-owner comparisons, a legacy last-segment
borrowed-argument heuristic and source/native emitted-name collisions. The
batch reuses the existing source-name disambiguation pipeline (including the
single-source fast path), retains parsed source canonical/binary names, and
preserves explicit qualified builder types before parser normalization.
Native type slots use the native carrier mapping; source declaration lookup
must not steal an explicit JDK type through a simple-name fallback.

V282's terminal focused result is one pass and one compile failure:
`.tmp/numbers-append-v282-focused.log`. The remaining failure names the
qualified native constructor `JavaLangStringBuilder::new_0` despite its slot
being native StringBuilder. V283 makes constructor owner projection reuse
the exact-qualified native carrier mapping, without mapping source lookalikes.
V283 passes all1671 library tests, but the same shadow-fixture constructor
failure remains; its ordinary constructor projection was not the active path.
The related suite passes enum lookup3, native numeric2, Numbers21 and object
equality9 in `.tmp/numbers-append-v283-related.log`.

The concrete-slot modeled external constructor path rebuilt a named call,
bypassing the ordinary constructor's owner projection. V284 initially made
that path use the full planned-construction helper. Both append witnesses
pass, but the library gate is1671 passes/1 failure: the existing modeled
GenotypeBuilder regression detects boxed constructor arguments instead of an
actual constructor call. Its provisional representation is erased, while the
concrete-slot path has stronger result evidence; sharing the entire storage
construction loses that distinction. Evidence:
`.tmp/numbers-append-v284-library-focused.log`.

V285 shares only the existing constructor-owner projection, retaining the
concrete result/slot conversion. This is a common owner-name repair, not an
erased-storage ABI change. A unit test independently checks qualified native
builder/buffer and foreign-package source owner projections. The modeled
external constructor regression passes1/0 and both executable append witnesses
pass2/0 in `.tmp/numbers-append-v285-{constructor,focused}.log`. The final
library/Numbers/enum/numeric/object batch is running in
`.tmp/numbers-append-v285-final-regressions.log`; the matching binary build
will be recorded in `.tmp/numbers-append-v285-build.log`.
Formatting and whitespace checks pass. The final batch passes library1672/0,
enum lookup3/0, native numeric2/0, Numbers21/0 and object equality9/0 in
`.tmp/numbers-append-v285-final-regressions.log`; with the two append witnesses,
all1709 tests across these suites pass. The matching binary build is terminal0.

Frozen producer `.tmp/new-java2rust-numbers-append-v285-debug`, SHA256
`6d0d395208fd05251aa55233f729a6d16cc4db6f17c2f2e933c1f1419e49123e`.
Fresh complete Core measurement is running in
`.tmp/numbers-core-v285-append-{run,behavior}.log`, output root
`.tmp/commons-numbers-1.3-core-v285-append`. Preservation measurements use
`.tmp/numbers-{primes,rootfinder}-v285-preserved-{run,behavior}.log` and
`.tmp/rng-{sampling-all,source64}-v285-numbers-run.log`, followed by
`.tmp/rng-{native,sampling-collections,sampling-distributions,source64}-v285-numbers-behavior.log`.
Source64 retains its separate11-file output rather than borrowing the46-file
sampling ABI. The strict Core comparator, including all existing exact NaN
comparisons, is unchanged.

Core's fresh11-file translation/parse/check gate passes: translation101203ms,
assembly12383ms, check1703ms, total140937ms. The strict5361-observation
comparison still fails with exactly the same four NaN-sign differences
(`sum.6.selfsubtract`, `sum.7.selfsubtract`, `dd.6.pow.-3.plain`,
`dd.8.reciprocal`), not new behavioral differences. The command's terminal1
is retained as a failure, not relabeled acceptance. Primes3594, Root Finder122
and isolated source642865 exact observations pass after their complete-source
translation gates. Those three preservation jobs are terminal0. The46-file
sampling/native preservation job is now terminal0: all46 files translate,
parse and check with0 unsupported members, invalid patterns or compiler errors.
Native12601, collections162 and distributions2172 exact observations pass.
Together with the isolated source64 gate, all17800 RNG observations are
preserved. Translation218404ms and assembly60292ms used two emission workers
alongside the other corpus jobs; these are not comparable speed measurements
against the earlier higher-concurrency isolated runs.
No upstream corpus file, generated deliverable or comparison was patched.

### V286–V287: native callback identity and synchronized-escape audit

Revalidation on the current producer confirms the enabled complete enum target
is still3 passes/1 failure, not a stale stopped job:
`.tmp/numbers-native-sam-v286-baseline.log`. The remaining failure is the native
DoubleUnaryOperator's Rc<RefCell<dyn FnMut>> inside the enum's process-wide
OnceLock<Vec<Mutex<Payload>>>. The payload must be Send even with no Java Thread
expression. Replacing the enum's storage with thread-local state or declaring
unsafe Send would change Java semantics and is not a candidate fix.

Whole-path rewrite audit: the native functional-interface shape authority
already knows the SAM signature, but its runtime renderer always emits a local
Rc callback. Representation deliberately keeps the facade concrete, and the
native allocation-domain constraint machinery does not currently project a
synchronized callback carrier through fields, parameters, helper returns and
captured receivers. A runtime-only Rc-to-Arc substitution would reject valid
local Rc captures elsewhere. Moreover, locking an FnMut dispatch cell across
arbitrary Java code can deadlock on reentrant calls. The existing source SAM
adapter already uses shared immutable Fn dispatch with Send+Sync bounds for
the synchronized case; reuse that contract for a separately selected native
synchronized carrier. Mutability belongs to captured Java allocations, not a
lock held around the entire dispatch. The smallest sound rewrite therefore
needs carrier-tier selection, factory/signature/capture propagation and runtime
support together. It is a cross-function representation extension, not just an
enum emission tweak. Local and synchronized values must retain a single Java
identity when viewed through aliases; no detached-copy conversion is valid.

For this exact witness the diagnostic target contract is:

```rust
// Hypothetical ABI only, not a manually translated deliverable.
struct JavaSyncDoubleUnaryOperator {
    callback: std::sync::Arc<dyn Fn(f64) -> Result<f64, JavaException> + Send + Sync>,
}
// Enum payload: operator: Option<JavaSyncDoubleUnaryOperator>.
// make(tag) must RETURN that same carrier, not construct a local Rc callback
// and later try to move it into a synchronized enum field.
// ALIAS copies the Arc/token; FIRST and SECOND have distinct allocations.
```

Reuse candidates for the implementation are the existing
`SignatureConstraint::RequiredNativeAllocation`/nested-domain lattice,
`propagate_assigned_call_native_allocation` and storage-edge propagation,
the functional-interface shape authority, `lower_functional_slot_lambda`,
`lower_native_functional_object_for_slot`, and the existing source SAM's
Send+Sync immutable-dispatch adapter. Native SAMs currently lack the domain
classification/rendering that makes those constraints actionable. A local view
of an already synchronized callback may retain its Arc and token; the reverse
must be solved by lifting the producer/captured allocations, not by copying
the callback into unrelated state. Include separate local-Rc, synchronized
static-return/alias, throwing and reentrant witnesses when implementing it.

An independent identity witness exposes a second contract before that larger
batch: `NativeSamIdentityProbe.java` passes javac, but unchanged generated Rust
compiles and returns2 because the generic facade equality body returns true
for two independently created callbacks. Evidence:
`.tmp/numbers-native-sam-v286-identity-baseline.log`. This witness also checks
two adapters independently produced from one retained source object, distinct
source objects and mutation through those aliases. Comparing only callback
wrapper allocations would fix the first check while breaking source aliases.

V287 replaces native functional facade's fabricated equality/empty Rust hash
with the existing JavaReferenceIdentity protocol. Genuine callbacks identify
their retained callback allocation; cloned/moved facades retain it. A source
adapter snapshots its retained source allocation token before the move closure
takes that reference, preserving identity across independently created adapters.
The native Iterable collection factory uses the same metadata for its backing
collection rather than inventing a callback-wrapper identity. Identity tokens
do not dereference payloads or hold loans across callback execution. This
identity batch does not claim synchronized storage, arbitrary source enum
adapters, erased native callbacks or virtual Object-method completeness.

The initial V287 identity witness passes1/0 and object protocols pass9/0 in
`.tmp/numbers-native-sam-v287-focused.log`. Extended regression checks now
exercise generic JavaReferenceIdentity dispatch, facade moves/clones and
nonempty alias-consistent Rust hashes. V288's terminal combined result in
`.tmp/numbers-native-sam-v288-library-contracts.log` is library1672/0, the
extended callback fixture1/0 (two generated Rust subtests), object protocols9/0,
and the unchanged enabled enum target3 passes/1 failure. The command returns101
solely for the same synchronized enum E0277; it is not called broadly green.
Formatting/whitespace checks pass, and the matching binary build is terminal0.
Frozen producer `.tmp/new-java2rust-numbers-native-sam-v288-debug`, SHA256
`e7e631015bc216c1c0aa1c19df5d33fbd58c6f4ce0e31e3e9a2f585de5307035`.

The last complete Core/accepted-module/RNG measurement remains V285, not this
new producer. Defer another expensive full corpus cycle until the coherent
callback-storage batch or a measurement needed to choose implementation; do
not infer fresh preservation from the focused results. The synchronized enum
failure remains required work, not a skipped test or module acceptance. Core
is still unaccepted, and the six-module goal remains active.

### V289–V295: synchronized native callback storage batch

V289 first seeds native SAM fields through the existing allocation-domain
constraint lattice and correctly treats initialized enum payloads as process-
wide storage. Its new factory-return unit initially fails: marking the enum
field alone does not reach make's return. Field initializers have no ordinary
method usage record. The corrective batch caches resolved field-initializer
value/call/constructor-argument relationships once in SignatureConstraintState,
deduplicates them, and includes them in the existing dependency graph and scoped
assigned-call allocation propagation. It does not scan all initializers on
each saturation iteration, infer constructors by arity or rewrite enum calls
with corpus-specific knowledge. Ordinary field factories share the mechanism.

The functional-interface shape renderer now produces distinct local and
synchronized carriers from one template. Local dispatch retains Rc<RefCell<
FnMut>> and its valid non-Send captures; synchronized dispatch is immutable
Arc<dyn Fn(...) -> Result<...> + Send + Sync>, with no dispatch lock across Java
code. Both retain the canonical identity metadata and reference-copy semantics.
The solved slot selects the carrier for type rendering, lambda construction
and source-object adaptation. Conversion planning no longer discards the
synchronized SAM ABI as if it were an irrelevant outer-wrapper request.
Local-to-synchronized placement changes cannot be hidden by shape annotations.
Generic SAM input/result type arguments are invocation contracts, not stored
fields, so synchronization does not indiscriminately lift those arguments.

V290's focused result is all4 enum contracts passing, including the previously
failing native callback enum, and the local callback identity fixture passing.
Evidence: `.tmp/numbers-native-sam-v290-sync-focused.log`.

The next unchanged independent Java witness is SynchronizedSamProbe: static
callback factories capture a mutable double array and a mutable source Counter,
exercise repeated alias state, recurse through the SAME callback and propagate
and recover from a thrown Java exception. Its separate Rust consumer also
checks Send+Sync, cross-thread invocation and continued local Rc captures.
V291 initially fails on Rc array/source captures and a staged-block binary
expression grammar error; the enabled enum target still passes4/0. Evidence:
`.tmp/numbers-native-sam-v291-capture-focused.log`.

CaptureAnalyzer previously assigned no declared target type to lambdas returned
from a method or initialized into a local. Preserve those declared targets in
CaptureOwner, including target-typed conditional/cast expressions. Storage
planning adds the callback's hidden capture edges to process-wide reachability,
lifting captured source allocations and retained array/SAM bindings rather than
copying their payloads. Native capture targets are indexed once, not rediscovered
by scanning every program capture for each callable. The index uses the existing
CaptureOrigin together with the callable-local BindingId; raw BindingIds alone
are not globally unique. A dedicated regression uses colliding local IDs in two
different methods and requires their synchronized/local domains to stay apart.
The reachability helper now uses a queue rather than repeated whole-graph scans.

The reentrant witness also exposed an independent emission boundary: a staged
block as a binary left operand must be parenthesized in statement-capable Rust
positions. Fix the shared expression emitter, including shape/comment wrappers;
do not rewrite the witness. The focused grammar regression invokes rustc on
stdin. An initial helper referenced an unavailable syn dependency (V293 compile
failure); no parser dependency was added and the corrected check uses the actual
Rust compiler instead.

V292 passes both native callback fixtures and all4 enum contracts in
`.tmp/numbers-native-sam-v292-focused.log`, including mutable array/source
captures, checked reentrancy and the cross-thread/local-Rc boundary assertions.
V294's library batch exposes two further checks without weakening either:
the grammar helper cannot emit metadata to /dev/null, and the existing checked
SAM field-transfer regression rejects an unnecessary local outer-allocation
marker. Emit the grammar check into its own temporary metadata file, and seed
SAM allocation-domain requirements only for synchronization; ordinary local
SAMs already own their retained callback. Their invocation-result nullability
and descendant collection domains remain separate. The source/capture tests
continue to use the same complete Java fixtures.

The corrected V295 library gate passes1675/0 in
`.tmp/numbers-native-sam-v295-batch.log`, including the new factory-return,
callable-scoped capture and actual-Rust-grammar regressions and the unchanged
checked SAM field-transfer test. The complete command is terminal0: enum4/0,
enum lookup3/0, native append2/0, native numeric2/0, native SAM2/0, Numbers21/0
and object protocols9/0, for all1718 tests passing. Formatting and whitespace
checks pass. Frozen debug producer
`.tmp/new-java2rust-numbers-native-sam-v295-debug`, SHA256
`b8ab0faec6a63a1b6d0cc09b493d2db8d82331f93665446f9db56f72feff895d`;
its build is recorded in `.tmp/numbers-native-sam-v295-build.log`.

Fresh complete Core is running in
`.tmp/numbers-core-v295-sync-{run,behavior}.log`, output root
`.tmp/commons-numbers-1.3-core-v295-sync`. Accepted-module preservation uses
`.tmp/numbers-{primes,rootfinder}-v295-preserved-{run,behavior}.log`. RNG uses
`.tmp/rng-{sampling-all,source64}-v295-numbers-run.log` and
`.tmp/rng-{native,sampling-collections,sampling-distributions,source64}-v295-numbers-behavior.log`,
with source64 still using its own11-file artifact. Core's complete11-file
translation/parse/check gate passes with0 unsupported syntax, invalid patterns,
no-op facade call sites or compiler errors; unmodelled call-site markers remain20.
Translation96135ms, assembly17384ms, check1888ms, total147354ms. Both callers
complete5361 observations; the strict comparison returns1 for exactly the same
four NaN-sign discrepancies as V285, not new differences. The exact comparator
is unchanged. Primes3594, Root Finder122 and isolated source642865 observations
pass after their complete translation gates; those preservation jobs are
terminal0. The46-file sampling/native preservation job is also terminal0:
all46 sources translate/parse/check with0 unsupported members, invalid patterns
or compiler errors (translation158168ms, assembly41146ms). Native12601,
collections162 and distributions2172 exact observations pass. Together with
isolated source64 this preserves all17800 RNG observations with the frozen
V295 producer. Every owned verification job for this checkpoint is terminal.
Core remains unaccepted for the documented numerical policy and remaining
complete public-API/branch/invalid-input coverage audit; no later module is
silently substituted for completing that gate. Remaining callback
families such as retained collection auxiliary state, erased native values,
bound reference creation through arbitrary factory expressions and further
virtual Object protocols still require their own whole-path evidence; passing
these probes is not blanket native-interface or Core acceptance.

### V296–V297: expanded numeric boundaries and translated default/null client

The next coverage audit distinguishes numeric overload coverage from inherited
default-body execution. Existing Addition/Multiplication calls through DD hit
DD's overrides, not the production default isZero/isOne bodies. Null-reference
boundaries also cannot be proven by a hand-written Rust caller whose compiled
API currently has non-null carriers. Keep both gaps explicit.

The maintained Core oracle now adds1290 observations: adjacent IEEE encodings
around signed zero, the subnormal/normal boundary, one, extremes, infinities and
both NaN signs, with ULP thresholds -1/0/1/2/3/4/MAX; DD scaling through extreme
int exponents; and ordinary/scaled/DDMath powers including MIN/MAX int exponents.
All algorithms remain calls into emitted production code. Both callers complete
6651 observations. Every added observation matches exactly on V295. The first
run returns1 for the previous four NaN-sign differences; a second identical
caller run returns1 for five, adding `dd.9.reciprocal` in the OLD observation
section. This is recorded JVM-run variability, not a comparator relaxation or
new boundary-value mismatch. Evidence:
`.tmp/numbers-core-v296-boundary-behavior.log`,
`.tmp/numbers-core-v296-frozen-behavior.log`, and
`.tmp/numbers-core-v296-boundary-evidence/{java-output,rust-output,differences}.txt`.
The latter directory records primary caller snapshots and SHA256s in
caller-provenance.json. The verifier's optional --output-dir permits separate
behavior checkpoints while reusing a valid generated artifact. Its six strict
comparator unit tests pass. Note that the first V296 run remeasured V295's
default behavior directory with6651 observations; the original V295 log remains
historical5361-observation evidence, but that directory is no longer an untouched
snapshot of its original caller run. Use separately named evidence directories
for subsequent caller revisions.

CoreContractsOracle.java is a TEST CLIENT, not a production dependency or a
replacement implementation. Its ContractElement implements the production
Addition/Multiplication interfaces and delegates numerical operations to DD.
Its zero/one/equals counters require the actual inherited default bodies to
execute, including argument mutation before equality. It also checks nine null
boundaries in Sum, DD, DDMath and Norm. It passes independently under javac with
contract=0/oracle=ok. Its Rust consumer invokes only the automatically translated
client; no client Java body or generated Rust is manually repaired.

`run-commons-numbers-1.3-core-contracts.sh` validates the original pin, copies and
compares all11 manifest production files unchanged, then adds exactly one
separately marked caller file. The12-file closure is recorded in
contract-source-manifest.tsv; no production file is skipped. The independent
Java runner and --numbers-core-contracts verifier mode retain the same scope.
V296 baseline output `.tmp/commons-numbers-1.3-core-contracts-v296-baseline`
translates/parses all12 with0 unsupported syntax or invalid patterns but fails
compilation with14 errors (translation111619ms, assembly13032ms). Evidence:
`.tmp/numbers-core-v296-contracts-java.log`,
`.tmp/numbers-core-v296-contracts-baseline-run.log`, compile.log and
top-compile-errors.tsv under the output root.

Whole-path rewrite audit identifies two shared roots. Twelve E0502 sites in
Addition/Multiplication/NativeOperators borrow `self` before evaluating a mutable
`self.zero()`/`self.one()` argument. Snapshot the canonical reference through
the existing trait clone_any protocol first, release that borrow, evaluate the
argument, then invoke checked equality on the retained snapshot. Do not reorder
Java-visible operations or clone a receiver payload. Two E0599 sites in flattened
class defaults call ordinary zero/one bodies suppressed solely because their
declared return type is the current class. These factories return NEW objects,
not `this`; retain their ordinary borrowed bodies using the existing transitive
identity-handle requirement analysis. Methods that actually expose/return/capture
`this` still need their canonical companions. These are shared lowering/entry-
point contract repairs expressible in the current framework, not manual method
rewrites or an additional representation design.

V297 applies both repairs in lower_erased_object_equals_call_for_op and
push_method_with_outer_handle_companion. The existing checked-interface equality
fixture now also has a mutating self-typed argument factory and exercises direct
and interface dispatch. All9 object protocol executables pass in
`.tmp/numbers-core-v297-default-focused.log`. The library/related batch is running
in `.tmp/numbers-core-v297-library-related.log`; fresh12-file client translation
and behavior are running in `.tmp/numbers-core-v297-contracts-{run,behavior}.log`,
root `.tmp/commons-numbers-1.3-core-contracts-v297-default`.

These jobs are now terminal and green: library1675/0, enum4/0, enum lookup3/0,
append2/0, numeric2/0, native SAM2/0, Numbers21/0; the separately measured9 object
protocol tests use the same producer, for1718 passing regressions. The fresh
12-file client closure translates/parses/checks with0 unsupported members,
invalid patterns or compiler errors (translation146067ms, assembly13134ms).
Its independent Java/generated-Rust comparison passes, proving the checked
default bodies, mutation counters and all nine null guards, not just compilation.

Frozen producer `.tmp/new-java2rust-numbers-default-v297-debug`, SHA256
`2e0e9199fd7f39b406aece46e3f7e28b62209208fa4430d2cd52151c9666de67`.
Fresh11-file numeric Core also translates/parses/checks with0 unsupported
members, invalid patterns, no-op call sites or compiler errors (translation
103356ms, assembly16093ms). Its6651-observation strict comparison returns1 for
TEN NaN-sign differences in `.tmp/numbers-core-v297-default-evidence`; all are
in the old observation section, and all1290 new boundaries still match. A direct
diff against V296's frozen evidence shows identical Rust outputs and exactly
five changed Java NaN signs. This is stronger evidence of Java-run variability,
not ten newly introduced translator failures; nevertheless the strict gate
remains failed and no comparison was relaxed. Log:
`.tmp/numbers-core-v297-default-behavior.log`.

Primes3594, Root Finder122 and isolated source642865 exact observations pass
after complete-source gates with this producer. Evidence uses
`.tmp/numbers-{primes,rootfinder}-v297-preserved-{run,behavior}.log` and
`.tmp/rng-source64-v297-numbers-{run,behavior}.log`. The combined46-file RNG run
is recorded in `.tmp/rng-sampling-all-v297-numbers-run.log`, followed by
`.tmp/rng-{native,sampling-collections,sampling-distributions}-v297-numbers-behavior.log`.
Inspection now confirms terminal0: all46 sources translate/parse/check with0
unsupported members, invalid patterns or compiler errors. Native12601,
collections162 and distributions2172 exact observations pass; together with
isolated source64 this preserves all17800 RNG observations. Every owned V297
verification job is terminal. Remaining coverage
work includes the explicit NativeOperators inherited-default path, additional
typed invalid-reference boundaries and the numerical-policy/branch audit.
Core remains unaccepted and the six-module goal remains active.

## V298 expanded client baseline and rewrite audit

The expanded untouched-production closure still has all 11 Core files plus
one translated test caller. The Java client passes (`contract=0`, `oracle=ok`)
with NativeOperators inherited defaults, mutation counters and 23 null guards.
Frozen V297 produces exactly three compiler failures in
`.tmp/commons-numbers-1.3-core-contracts-v298-expanded`: two fresh primitive
arrays passed to nullable retained parameters and an unmapped BigInteger.TWO.
Evidence: `.tmp/numbers-core-v298-contracts-{java,run}.log` and the generated
closure's `top-compile-errors.tsv`.

The two array failures need `Some(Rc::new(RefCell::new(vec![...])))`, not an
unshared-to-shared conversion of an existing array alias. The general fresh-array
slot path already constructs the selected allocation and retains element shapes.
The non-null-to-optional argument shortcut bypasses that producer path, lowering
the expression without its destination and then requesting an impossible
allocation conversion. Route fresh array arguments through that existing slot
boundary; leave ordinary alias conversion diagnostics intact. This is a local
boundary-routing repair, not a storage or signature redesign. BigInteger.TWO
needs the same external-pack field mapping used by ZERO/ONE; extend the explicit
runtime manifest for TWO/TEN rather than synthesizing source types or modifying
the client. Focused executable tests must cover constants/static imports and
fresh empty/nonempty arrays plus retained alias mutation.

The numeric caller now adds 12 exact factory-copy observations for signed zero,
subnormals, finite values, infinities and quiet NaNs with both signs/payloads.
All new copies agree in `.tmp/numbers-core-v298-copy-evidence`; its strict
6663-observation comparison fails on seven old arithmetic NaN-sign differences.
No comparator or production arithmetic was relaxed. Core remains unaccepted.

### V298 implementation result and newly exposed nullable upcast

V298 routes fresh arrays through the existing destination-aware slot allocator
and extends the authoritative BigInteger manifest with TWO/TEN. All7 BigInteger
and22 Numbers executables pass, including the new empty/nonempty retained-array
argument/alias probe; the full library passes1675 tests. Logs:
`.tmp/numbers-core-v298-{focused,library}.log`. Frozen producer
`.tmp/new-java2rust-numbers-default-v298-debug`, SHA256
`690ef6135c836cebad1f00589b74b97469fbf7ef0fc80e61af7e011acfceb717`.

The full12-file regenerated client removes all three original compiler errors,
then exposes E0382: `one.map(...)` consumes a nullable concrete source handle
while constructing its nullable interface alias; subsequent reads of `one`
must still work. Evidence: `.tmp/commons-numbers-1.3-core-contracts-v298-fixed`
and `.tmp/numbers-core-v298-fixed-contracts-run.log`. The required rewrite is
to copy the Option's Rc/Arc reference before the consuming payload conversion,
not clone the interface adapter or object payload afterward. The ordinary
non-null upcast and nullable unwrap/wrap paths already recover live references;
the nullable MapElement path falls through without that recovery. Reuse the
existing source-handle authority and move-use analysis at this last conversion
boundary, limited to owned nullable shared-source carriers and consuming
MapElement conversions. This is a local ownership-boundary gap; storage and
signature selection already express the correct Rust contract.

The next coverage increment exercises full-width long and BigInteger exponent
loops with bases0/1/-1 (bounded results), negative validation, parity beyond
64 bits and retained base values. All69 added observations agree when measured
against frozen V297 production output; the6732-observation strict baseline has
four old arithmetic NaN-sign differences, no new mismatch. Evidence:
`.tmp/numbers-core-v299-wide-exponent-baseline-evidence` and its matching
behavior log. This reuses an explicitly older generated artifact as a coverage
baseline, not a claim that V298 numerical output was regenerated or accepted.
All6 strict comparator unit tests pass; no comparison policy changed.

### V299 nullable-map implementation and diagnostic audit

V299 copies a still-live owned nullable source handle before a planned
MapElement conversion, using existing source-handle and move-use authorities.
Frozen producer `.tmp/new-java2rust-numbers-default-v299-debug`, SHA256
`7c2a646b2340b1befc7322124416def663775608821be752f99227238fdbac41`.
The focused nullable alias fixture emits the required
`Clone::clone(&original).map(...)`, and its generated interface forwards calls
through the same Rc<RefCell<Cell>> (including identity returned through the
generic interface). Nevertheless an observable_alias diagnostic rejects the
fixture: the diagnostic recognizes native-SAM and erased-Object canonical
adapters but not object-safe parsed source-interface adapters. Baseline Rust
and diagnostics are retained at `.tmp/numbers-core-v299-alias-generated-baseline.*`.

The diagnostic repair must require a parsed source object-safe interface,
parsed concrete source subtype, proven hierarchy reachability and implemented
shared mutable source storage. Unproved/value-copy aliases remain errors; add
an explicit missing-proof negative control. This aligns diagnosis with the
already-emitted carrier, not a waiver for a corpus name. Keep the independent
Java/generated-Rust nullable alias fixture as executable identity/mutation proof.

An older hand-written Rust test double in nullable_bounded_interfaces lacked
the synthetic clone_any protocol required even by frozen V298 (see
`.tmp/numbers-core-v299-nullable-bounded-v298-baseline.rs`). It now implements
clone_any with a panic sentinel, preserving the test's no-cloning requirement.
The recheck passes in `.tmp/numbers-core-v299-nullable-bounded-recheck.log`.

### V299 runtime audit: nullable shared Copy-field guard lifetime

The V299 complete12-file client translates/parses/checks with0 unsupported
members, invalid patterns, no-op call sites or compiler errors (translation
123820ms, assembly14370ms). Its independent behavior comparison times out at
40 seconds; it is NOT accepted. Reproduction/evidence:
`.tmp/numbers-core-v299-contracts-{run,behavior}.log` and
`.tmp/numbers-core-v299-contracts-evidence/generator-oracle`.
GDB run/interrupt/backtrace identifies a single-thread mutex wait in
DD::subtract_d_d_with_outer at generated DD line215, called by
ContractElement::subtract_with_outer, NativeOperators::subtract and client run
line143. The stopped diagnostic inferior was killed and the debugger exited.

The optional DD argument produces two expressions
`-y.as_ref().ok_or_else(...)?.lock().expect(...).x` and the analogous `.xx` in
one function call. Rust retains the first temporary guard through the full
call expression, so the second field read deadlocks on the same Mutex. Java
reads scalar fields without an enduring loan. Required rewrite: each nullable
Copy field read gets its own owner/guard block, returning only the scalar.
The existing scoped_shared_storage_field_read helper already supplies this
contract for non-null/shared reads, and lower_shared_storage_field_access_plain
already handles nullable checks. Optional plain/slot paths bypassed it. Reuse
that path for primitive fields in both routes, retaining hierarchy/enum accessor
precedence and reference-field borrowing behavior. This is a local routing gap,
not a new allocation or synchronization strategy. Add a focused shared/static
nullable pair probe with repeated direct/unary field arguments and null catches.

### V301 complete-source checkpoint and focused follow-up

Frozen V301 producer `.tmp/new-java2rust-numbers-default-v301-debug`, SHA256
`8719656e376631689c7c342f1805db13237dff19f00e11e5357883ef7cba0ca5`.
Both full Core closures (11 numeric production files, and12 files including the
separate translated client) translate/parse/check. The client now independently
matches Java on its inherited defaults, mutation counts and all23 null guards:
`.tmp/numbers-core-v301-contracts-{run,behavior}.log`, evidence
`.tmp/numbers-core-v301-contracts-evidence`. Numeric6732 observations complete
with just four old arithmetic NaN-sign differences, all69 new width cases and
12 raw copies exact: `.tmp/numbers-core-v301-numeric-{run,behavior}.log` and
`.tmp/numbers-core-v301-numeric-evidence`. Primes3594 and Root Finder122 exact
observations pass after fresh complete-source gates, recorded in
`.tmp/numbers-{primes,rootfinder}-v301-preserved-{run,behavior}.log`.

The full library passes1676 tests, including the alias-checker's missing-proof
negative control. The nullable primitive direct/unary shared-field guard
regression passes. The independent nullable alias fixture gets past the repaired
diagnostic and exposes two compiler gaps (23 of24 Numbers tests pass at this
point): its generic interface identity result contains a null-check `?` inside
an is_some_and bool closure, and its main method's AssertionError(int) is treated
as a Throwable(String) constructor. Logs:
`.tmp/numbers-core-v301-library-nullable.log`. Neither was removed from the
fixture or counted as a passing contract.

Required rewrites: eagerly snapshot both owned canonical identity operands in
Java order before optional pointer testing (including RHS evaluation when the
other side is null); convert AssertionError primitive details using existing
Java string-display plans before runtime exception construction. These are
shared lowering-boundary repairs, not an upstream/generated-source rewrite.
The alias fixture adds both operand orders, side-effect counts and throwing
operands with a null sibling; a primitive detail regression covers integral,
boolean, character and floating-point messages. Keep default strict numeric
comparison unchanged. Core remains unaccepted pending numerical policy and
the final complete coverage audit; no later module has been started.

### V302–V303 focused closure and preservation checkpoint

V302 adds eager owned-reference identity operand snapshots and AssertionError
primitive detail rendering. Its full library passes1676 tests;24 of25 Numbers
executables pass, with the remaining compiler errors showing that consumed
Result evidence must survive the new staging block (otherwise an outer slot
appends `?` to bool). V303 preserves that existing ResultPropagated evidence
on the block, without changing the comparison or exception ABI. The final
focused run passes all25 Numbers tests, the independent Java/Rust nullable
interface alias fixture (including both operand orders and throwing/effectful
operands with a null sibling), the nullable-bounded no-cloning sentinel and
shared nullable interface behavior. Log: `.tmp/numbers-core-v303-focused.log`.
The AssertionError probe checks six primitive detail types, including signed
floating zero, through the existing Java display plans. No generated Rust or
upstream production source was manually changed.

Frozen producer `.tmp/new-java2rust-numbers-default-v303-debug`, SHA256
`69ce4c93f39cbcb785ae13f4f2e44f69b8f4cc6de0ef16c2d61dcd06a7052c0b`.
Final library/object tests pass1676/0 and9/0 in
`.tmp/numbers-core-v303-library-object.log`; together with the27 focused tests
above this checkpoint has1712 passing regressions. Full46-file combined RNG and
isolated11-file source64 preservation runs use
`.tmp/rng-{sampling-all,source64}-v303-numbers-run.log`, followed by
`.tmp/rng-{native,sampling-collections,sampling-distributions,source64}-v303-numbers-behavior.log`.
All are terminal0. The46-file closure translates/parses/checks with0 unsupported
members, invalid patterns or compiler errors (translation178625ms,
assembly33258ms). Native12601, collections162, distributions2172 and isolated
source642865 exact observations pass, preserving all17800 RNG observations.
All6 strict comparator unit tests also pass. Every owned verification job for
this checkpoint is terminal; the failed intermediate runs remain recorded as
diagnostic evidence, not counted as passing gates.
The complete Core client/numeric and Primes/Root Finder evidence immediately
above remains explicitly V301, not silently relabeled as V303. Core remains
unaccepted; numerical policy and final coverage audit are still required.

### V304 numerical-contract audit and proposed comparison boundary

The four V301 strict differences are arithmetic NaN sign bits, not finite
rounding differences. The earlier Java-to-Java interpreter/C1/default-run
experiments reproduce that variability. JLS19 section4.2.3 leaves arithmetic
NaN result encodings to the implementation/hardware; Double.doubleToRawLongBits
instead exposes the actual encoding. Primary references:
https://docs.oracle.com/javase/specs/jls/se19/html/jls-4.html#jls-4.2.3
and https://docs.oracle.com/en/java/javase/19/docs/api/java.base/java/lang/Double.html#doubleToRawLongBits(double).
Changing translator arithmetic to imitate one JVM run would not establish a
portable Java contract. This does not justify normalizing arbitrary hex fields.

Implement a separate, explicit arithmetic-NaN-sign portability mode; retain the
default strict comparator and every strict mismatch report. Only caller-declared
typed `f32-arith(8 lowercase hex digits)` / `f64-arith(16 lowercase hex digits)`
tokens qualify. Match surrounding text, token widths, keys, exceptions, state,
counts and traces exactly. Permit only a sign-bit flip between two NaNs with
identical nonzero fraction/quiet/payload bits. Finite values, signed zeros,
infinities, raw copies, payload changes and unannotated values remain exact.
Record every permitted component, policy name and residual mismatch separately;
never advertise the portable result as IEEE-bit equality.

The producer audit initially qualifies whole operation families, not failing
input keys: Sum.subtract(Sum), DD.reciprocal(), DD.divide(double/DD), and
DD.pow(int) except the0/1 literal/reference-copy branches. Sum subtraction
combines components through DD addition or scalar addition fallback. DD
reciprocal/division finish with twoSum arithmetic; nontrivial plain powers use
Math.pow or extended multiplication/reciprocal. Getter/factory copies,
negate/abs/floor/ceil, scaled powers, scaling/decomposition, hashes, strings,
exponents and all other unreviewed results keep strict observations. Apply the
same annotation at ordinary/interface/boundary call sites of those reviewed
families. No production algorithm or generated deliverable is edited. Add
negative comparator controls before using this mode as evidence, and complete
the public-operation/branch coverage audit before accepting Core.

### V304 contract evidence and public API coverage ledger

Fresh V304 numeric11-file and client12-file closures use the unchanged frozen
V303 producer/hash above. Both translate/parse/check with0 unsupported members,
invalid patterns, stubbed constants, no-op facade call sites or compiler errors.
Logs: `.tmp/numbers-core-v304-final-{run,contracts-run}.log`. The original client
passes its strict independent default/state/null comparison in
`.tmp/numbers-core-v304-contracts-evidence`. These are new measurements, not a
relabeling of the V301 artifacts.

The explicit portability implementation and typed caller annotations pass11
comparator unit tests, including rejection of sign/payload/quiet-bit changes,
finite ULP changes, signed zeros, infinity signs, width/malformed metadata,
unannotated raw bits, keys, traces, scope and state changes. The initial6732
observations pass the portable contract, retaining four strict differences in
`.tmp/numbers-core-v304-portable-evidence`. Strict mode independently still
returns1 with four differences and no success artifact at
`.tmp/numbers-core-v304-strict-evidence`.

The coverage audit identified an under-exercised branch family: many DD unary
operations and conversions only received values whose low component was zero.
The caller now adds1100 observations from ten signed nonzero-low-component
inputs around +/-1, +/-2 and +/-0.5, with a10x10 pair matrix for every add,
subtract, multiply and divide overload; integer multiply; floor/ceil low-part
correction, unary operations, conversions, equality/hash/string, zero/one tests
and unchanged-input checks. All1100 added results match exactly. The full7832
observations pass arithmetic-nan-sign-v1 with four permitted sign components,
all strict differences retained: `.tmp/numbers-core-v304-compensated-evidence`
and `.tmp/numbers-core-v304-compensated-behavior.log`. Only the previously
audited families have annotations; the new floor/ceil/copy/state checks do not.

The current exact same compiled Java caller is also measured with -Xint and
with -Xcomp -XX:TieredStopAtLevel=1. Java-to-Java strict comparison changes26
observations/50 NaN sign components for interpreter mode and16 observations/16
components for C1; both have ZERO portable residual differences. Injecting a raw
NaN copy sign flip, raw copy payload change, alias-state change or compensated
input corruption is rejected by the same portable comparator. Evidence and
asserted fault-injection results: `.tmp/numbers-core-v304-policy-audit.log`;
raw JVM outputs: `.tmp/numbers-core-v304-java-{interpreted,c1}.txt`.

Descriptor census `.tmp/numbers-core-v304-api.txt` includes all17 class files
produced by the11 pinned Java production files (including anonymous/nested
classes, private helpers and compiler bridges), reproduced by
`scripts/list-commons-numbers-1.3-core-api.sh`. The following ledger maps the
declared module API to maintained independent callers; it is not an exclusion
of relevant inherited operations or proof that descriptor presence is execution.

| API family / all declared overloads | Executed caller sections and representative contracts |
|---|---|
| ArithmeticUtils: gcd(int/int,long/long), lcm(same), pow(int/int,long/int,BigInteger with int/long/BigInteger), isPowerOfTwo(long), unsigned divide/remainder(int/long) | arithmetic: zero, sign, MIN/MAX, negative/zero/positive powers, overflow, zero divisors; wide exponents beyond32/64 bits, parity and retained bases; typed nulls in translated client |
| Norm: all6 constants, values/valueOf, of(2 doubles/3 doubles/array) | norms: every constant/ordinal/name/repeated enum identity, every of overload, finite/small/extreme/infinity/NaN, empty/single arrays, invalid/null lookup and arrays |
| Precision: compareTo(epsilon/ULPs); float equals/equalsIncludingNaN(default/epsilon/ULPs); double equivalents plus relative tolerance; round(default/mode), representableDelta, equivalence factory and2 constants | precision/boundaries: all ordered value pairs, NaNs/infinities/zeros, adjacent encodings and subnormal/normal boundaries, negative/zero/positive/MAX ULP thresholds, all rounding modes/scales, invalid epsilon and null mode |
| DoubleEquivalence: compare, eq, eqZero, lt, lte, gt, gte, signum | precision: actual anonymous compare implementation and all7 defaults over ordered/NaN/zero inputs and repeated calls |
| DD factories: of(double/int/long), ofUnsigned(int/long), from(BigDecimal), ofSum/ofDifference/ofProduct/ofSquare/fromQuotient; ZERO/ONE | doubles: representable/inexact wide integers, decimal fractions/extremes, signed zeros/subnormals/infinity/NaN and arithmetic cancellation; raw signed/payload NaN copies remain strict |
| DD observations/conversions: hi/lo, isFinite, doubleValue/floatValue/intValue/longValue/bigDecimalValue | doubles/compensated: special values, MIN/MAX, both nonzero-low signs around integers, exact bits/decimal strings and non-finite decimal exceptions |
| DD unary/scaling: negate, abs, floor, ceil, square, sqrt, reciprocal, scalb, frexp | doubles/compensated/boundaries: both signs and low-part corrections, negative roots, reciprocal extremes, int-scale extremes, output-array writes/empty/null arrays and repeated unchanged inputs |
| DD add/subtract(double/DD), multiply(double/DD/int), divide(double/DD), pow(int/int+long-array) | doubles plus full compensated pair matrix; plain/scaled/DDMath powers include int MIN/MAX and low parts, output exponents exact; typed-null DD operands and output arrays |
| DD equals(Object), hashCode, toString, zero/isZero/one/isOne | doubles/compensated: self/copy/null/wrong type, low-part-sensitive equality/hash/string, canonical ZERO/ONE identity, signed zero and repeated immutable state |
| DDMath.pow(DD,int,long-array), private MDD helpers | doubles/boundaries: ordinary/nontrivial low-part values, positive/negative/extreme exponents, exact exponent writes, empty/null arrays and null input |
| Sum: create, of(double/varargs), ofProducts; add(double/varargs/Sum), subtract(Sum), addProduct/addProducts, accept/getAsDouble | sums: empty/scalar/cancellation/tiny/extreme/NaN/infinite data, repeated mutation, self-add/subtract, reference identity/erased display, mismatched arrays and typed nulls |
| Addition add/zero/negate/isZero, Multiplication multiply/one/reciprocal/isOne, NativeOperators subtract/divide/multiply(int)/pow | DD interface calls cover operations; automatically translated ContractElement client forces actual isZero/isOne inherited bodies through all3 interfaces, mutation/equality counters and canonical reference handling |
| ExtendedPrecision.productLow and source-private arithmetic helpers | reached by DD factories/arithmetic, Sum products, scaled Norm/power paths; tiny/overflow/cancellation branches, array/exponent state and callbacks are observed through public results |

### V305 inherited numeric API baseline (Core still not accepted)

The descriptor census lists declarations, not inherited concrete platform
methods. DD extends java.lang.Number and inherits byteValue/shortValue. Java's
documented defaults invoke virtual intValue and narrow to byte/short:
https://docs.oracle.com/en/java/javase/19/docs/api/java.base/java/lang/Number.html.
The translated client now also calls both through DD, and a separate general
source Number subclass probe checks the narrow results and intValue mutation
counts. Its independent Java run passes. Frozen V303 translation fails with
four E0599 errors: SmallNumber has no byte_value/short_value methods. Evidence:
`.tmp/numbers-core-v305-number-baseline.log`. The full updated Core client also
finishes terminal1 with precisely two E0599 errors for missing byte_value and
short_value on the DD receiver. Evidence:
`.tmp/numbers-core-v305-number-client-baseline.log` and
`.tmp/commons-numbers-1.3-core-contracts-v305-number/top-compile-errors.tsv`.
All11 production files and the separate caller still translate/parse; the
compiler correctly rejects the missing inherited contract. The focused
regression deliberately remains failing until the translator is fixed, not
ignored or weakened. Every owned V304/V305 job is terminal.

Minimal semantic rewrite is two inherited bodies equivalent to
`return (byte)this.intValue()` / `return (short)this.intValue()`, not direct
field casts or a zero/default facade. The dependency must be visible before
usage/storage/signature/fallibility analysis so a mutating/throwing intValue
override retains Java semantics, including subclass overrides. Reusing an
expression-only fallback would miss that dependency. Prefer a shared semantic
materialization of known concrete platform defaults, feeding both primary and
context/summary symbol paths (as class-initializer materialization already
does), or an equally authoritative inherited-body plan used by every analysis.
Require source-owner qualification/shadowing proofs, method override precedence,
nested/abstract/source-subclass coverage, ordered receiver evaluation and caught
exceptions before accepting the fix. Existing source inheritance and numeric
cast lowering express the required bodies; the missing contract is the modeled
platform method's body/delegation edge. Do not add DD-specific methods or alter
upstream/generated deliverables. This is the next coherent translator batch.

### V306–V308 inherited Number implementation and related rewrite audits

`resolve::materialize_platform_number_defaults` now supplies Number's two
concrete default bodies as ordinary Java IR before call resolution. The same
function is used for the full source-symbol inventory and resolved primary/
support programs, so binding, source calls, receiver mutation, fallibility and
numeric narrowing use existing authorities. It checks canonical source
hierarchy identity and source/imported Number lookalikes, preserves explicit
zero-argument overrides and distinct overloads, and is idempotent. Abstract
Number subclasses inherit the four real abstract numeric declarations (no
fabricated bodies) so the defaults can dispatch virtually to concrete source
overrides. The preliminary symbol inventory is only built when a Number parent
candidate exists, not on every corpus or fixpoint iteration.

The general fixture now covers mutation counters, a throwing intValue, null
receivers, polymorphic intValue dispatch, an explicit byteValue override,
byteValue(int) overload preservation, an abstract base and a nested implicit
java.lang.Number subclass. It independently passes under Java. V306's
cross-file crate assembly test passes, but the richer fixture exposes two
E0070 errors: `value.value() = rhs`. The generated value-backed hierarchy
already has `__java_set_field_value(&mut self, ...)`; assignment lowering only
admitted all-shared payloads. Minimal rewrite is the existing setter call, not
a new carrier or copying the payload. V307 shares one setter existence/
receiver-mutability policy between representation emission and assignment;
value payloads require an exclusive carrier borrow, shared mutable payloads
their existing guard, immutable/unsupported/anonymous payloads stay excluded.
The full inherited-number fixture and cross-file behavior then pass. Evidence:
`.tmp/numbers-core-v306-number-focused.log` and
`.tmp/numbers-core-v307-number-focused.log`.

The added nullable value-backed setter probe in V307 reaches the correct
mutable setter but separately refuses `present ? new Leaf() : null` as an
unsupported class upcast, although RootValue and From<Leaf> already exist.
Audit: an old optional-present refusal ignores the solved hierarchy, and
optional element planning can lose its closed-carrier dispatch. Correct
rewrite is `Some(RootValue::Leaf(Leaf::new()))` before using the ordinary
mutable nullable setter. V308 preserves the projected carrier dispatch in
the representation-aware optional construction plan, routes all optional
present expression paths through that plan, and retains the unsupported
upcast diagnostic unless the representation and source-subtype proofs exist.
No new storage/conversion strategy or changed source is needed.

Frozen producer `.tmp/new-java2rust-numbers-default-v308-debug`, SHA256
`f5cec3f87f3ebc911b1cef3828faea4074cb1f5bb2ad8b62beb3ba22b5741e6c`.
Library/Numbers/object/nullable-interface batch:
`.tmp/numbers-core-v308-library-number.log`. Fresh complete numeric/client
closures are running in `.tmp/numbers-core-v308-{numeric,contracts}-run.log`,
roots `.tmp/commons-numbers-1.3-core-v308-default` and
`.tmp/commons-numbers-1.3-core-contracts-v308-default`. Keep the inherited Number
gate explicit; Core has not been accepted on the strength of an older client.

### V309 final Core acceptance audit — accepted (3/6 modules)

The inherited Number dependency is closed by general translator IR/default
body materialization, not patched Java or generated Rust. The V308 numeric
and complete updated client both compile and independently pass, including
DD.byteValue/shortValue. Its1679 library tests pass. The added nullable
value-carrier setter probe initially panics with an untyped Java-null expect:
the existing exception planner seeded nullable formals but omitted nullable
locals. V309 extends that same policy with solved local shapes, excluding
proven-non-null locals, for methods and constructors; the setter discharges
nullability through the existing catchable check. A new plain-object local
read/write/constructor probe and the nullable carrier probe both pass. No
test was removed/ignored. One older library string assertion expected the old
panic-based array-element receiver check; it now requires the typed
NullPointerException check instead. This last change is test-only; all
production gates below use the frozen V309 binary, unchanged by that assertion.

Frozen producer `.tmp/new-java2rust-numbers-default-v309-debug`, SHA256
`ad76ed0ff0f586c99ad550951e39b7dc7b4d2f2d23c01fb12b595f4bc90f4c59`.
Pin remains untouched Numbers1.3 commit
`ae8abefb51c2c4d1a05882d61cefc8015e493a8f`. Its11 production files,17 class
descriptors and JDK-only dependency closure remain complete; required concrete
Number defaults are explicitly modeled above. Metadata/private/helper/overload
scope is unchanged; no Fraction/Gamma/Arrays modules or extra dependencies.

| Required gate | Current evidence |
|---|---|
| All11 production files translate/parse/check, no unsupported/invalid/stubbed/no-op call sites | `.tmp/commons-numbers-1.3-core-v309-default/summary.tsv`, `.tmp/numbers-core-v309-numeric-run.log`;0 errors, translation159679ms, assembly13604ms |
| Complete production plus automatically translated client, including new inherited API | `.tmp/commons-numbers-1.3-core-contracts-v309-default/summary.tsv`, `.tmp/numbers-core-v309-contracts-{run,behavior}.log`, `.tmp/numbers-core-v309-contracts-evidence`;12 files,0 errors, translation170612ms, assembly13802ms; strict client result=0 proves defaults/counters/all23 null guards and2 inherited conversions |
| Every declared public operation/overload and relevant inherited operations | V304 ledger above plus inherited Number audit/fixture; maintained Java/Rust callers, source/default/nested/private census; no declared operation omitted |
| Representative nonzero-low parts, branches, boundaries, state, errors and identity | Expanded numeric comparison7888 observations: `.tmp/numbers-core-v309-identity-evidence` and matching behavior log;1100 compensated matrix cases plus56 explicit reference identity/freshness observations for DD.pow(0/1/2) and abs; getters/raw copies/state/exponents exact |
| Justified portable numeric contract, not hidden raw equality | arithmetic-nan-sign-v1 permits four same-payload arithmetic NaN sign components; strict differences preserved. `.tmp/numbers-core-v309-strict-evidence` still returns1 with four strict differences and no success file. All other bits/state/branches exact |
| Independent policy falsification / cross-JVM evidence | `.tmp/numbers-core-v309-policy-audit.log`: default vs interpreter26 strict differences/50 permitted components, C1 16/16,0 residuals; injected raw copy sign, copy payload, alias state and compensated input corruption rejected. Raw mode files `.tmp/numbers-core-v309-java-{interpreted,c1}.txt`;11 comparator unit tests pass |
| Executable general regressions, including real Java fixtures | `.tmp/numbers-core-v309-number-focused.log`:29 Numbers,9 object protocols,1 nullable bounded and1 shared nullable interface; all40 pass, including mutating/throwing/overridden/overloaded/abstract/nested/cross-file Number behavior |
| Full library and related suites at final boundary | `.tmp/numbers-core-v309-final-library-related.log`:1679 library,7 BigInteger,4 enum init,3 enum lookup,2 append,2 numeric and2 native SAM; all1699 pass. Together with focused run:1739 passing regressions; all terminal0 |
| Preserve accepted Numbers modules | fresh V309 Primes3594 and Root Finder122 exact observations and complete-source gates; `.tmp/numbers-{primes,rootfinder}-v309-preserved-{run,behavior}.log` |
| Preserve all RNG behavior | fresh46-file combined and11-file isolated source64 gates,0 unsupported/invalid/compiler errors; native12601, collections162, distributions2172, source642865 =17800 exact observations. `.tmp/rng-{sampling-all,source64}-v309-numbers-run.log` and `.tmp/rng-{native,sampling-collections,sampling-distributions,source64}-v309-numbers-behavior.log`; all terminal0 |

Reproduce Core by selecting the frozen producer/build profile, fresh staging/
translation roots and `NUMBERS_MODULE=core` with
`scripts/run-commons-numbers-1.3.sh`; compare with
`scripts/verify-commons-rng-1.0-behavior.py <root> --numbers-core --arithmetic-nan-sign-portability --output-dir <new-evidence>`.
Omit the policy flag to obtain the separately preserved strict failure.
Use `scripts/run-commons-numbers-1.3-core-contracts.sh` and verifier
`--numbers-core-contracts` for the full translated client. Caller snapshots,
hashes and policy name are retained in each behavior evidence directory.

Core is accepted under this explicit Java-contract policy, not claimed IEEE-bit
identical for arithmetic NaN signs. All owned V309 jobs are terminal. The
six-module goal remains active: next is complete Complex, then Quaternion and
Angle, with fresh feasibility/rewrite audits and the same preservation gates.

### V310 Complex baseline and whole-path rewrite audit

Frozen V309 producer (hash recorded above), untouched pin, complete two-file
Complex production source set: Complex.java (including private helpers and
ComplexConstructor SAM) and package-info.java. Production imports are JDK-only:
Serializable, ArrayList, List and DoubleUnaryOperator. The POM's Core and RNG
dependencies are test-scoped, not production closure. Baseline artifacts:
`.tmp/commons-numbers-1.3-complex-v310-baseline`, run log
`.tmp/numbers-complex-v310-baseline-run.log`. Generation/parse succeed;
six compiler errors, total129195ms, assembly6433ms. Not accepted.

Concrete rewrite audit before fixes:

- Four diagnostics are the two branches of the acosh expression lambda,
  repeated for ordinary/shared-receiver entry points. The selected source SAM
  result is Arc<Mutex<Complex>>, but the conditional's new Complex branches
  emit bare payloads. Minimal correction constructs the solved shared carrier
  in each selected branch. Producer constructors remain payload constructors;
  the destination return boundary owns allocation. Reuse assignment-slot
  idempotent bare-constructor storage construction at the return-slot boundary,
  including recursive conditional branches. Cross-function physical-contract
  fix, not a new representation or callback-specific shim.
- Two parse errors pass usize start offsets to String search helpers whose
  authoritative ABI is signed i32. Lower these arguments through the Java int
  slot, not Rust indexing, preserving negative start-index behavior. Local
  native-call argument-contract correction; runtime helper unchanged.
- Division has four reachable Math.scalb calls to an unimplemented facade.
  This is a real runtime gap despite successful generation and must be modeled
  generally, with overflow/subnormal/signed-zero boundary regressions. No
  replacement of source division or private hypot algorithms is allowed.
- Callback adapters already share the SAM descriptor with method references,
  carry reference-valued results and release scalar field loans before helper
  calls. nthRoot allocates an ordered Vec of shared result handles. equals
  uses shared-handle downcasts and pointer equality; static objects use shared
  accessors. Existing representations can express these; executable behavior,
  identity and reentrancy remain unproven by compilation alone.
- Parsing uses Rust float parsing and character/byte length paths; Java grammar,
  UTF-16 positions, exception messages and null behavior need explicit probes.
  Transcendental differences require evidence before a numerical policy.

First coherent batch: signed String search starts plus shared callback return
construction, with independent executable Java/generated-Rust fixtures. Then
model scalb and establish the full API/behavior census. No substantial framework
redesign is currently indicated; six compiler errors are not the remaining
semantic workload. Unmodelled/no-op inventory still needs reachability audit;
baseline reports20 unmodelled call signals and0 no-op call sites.

### V311 Complex compiler batch — all production files compile, not accepted

Implemented both V310 compiler families through shared boundaries in lower.rs:
return slots reuse the existing idempotent bare-constructor storage-placement
adapter (so conditional SAM branches allocate the solved handle), and String
indexOf/lastIndexOf offsets use an explicit Java int argument slot rather than
Rust indexing. Constructors, SAM descriptors and runtime search APIs unchanged.
No pinned Java or generated deliverable was edited.

General executable fixtures SharedSamReturnProbe.java and
StringSearchOffsetProbe.java independently execute on Java and generated Rust.
They cover escaping captured factories, direct/conditional/block return bodies,
both branches, fresh allocation versus aliases, synchronized reads, computed
offsets, negative/clamped starts, empty searches and int-overflow boundaries.
`.tmp/numbers-complex-v310-focused.log`:3 native SAM and30 Numbers contracts
pass (33 total). `.tmp/numbers-complex-v311-library.log`:all1679 library tests
pass. Formatting with explicit edition2024 passes. All jobs terminal0.

Frozen `.tmp/new-java2rust-numbers-default-v311-debug`, SHA256
`ecde92c6ff18c3b84f5c3899712bb533535cec912813a9f75dd750fd5e6f87f4`.
Complete two-source run `.tmp/commons-numbers-1.3-complex-v311-default`, log
`.tmp/numbers-complex-v311-run.log`:0 compiler/parse/unsupported/invalid errors,
0 stubbed constants/no-op call sites. Translation159019ms, assembly6694ms,
total181129ms while library verification also ran; not a controlled performance
comparison with V310. Source pin/manifests and dependency closure unchanged.
The generated callback now allocates Arc<Mutex<Complex>> in each selected branch;
search offsets retain wrapping signed int expressions.

Complete javac/javap census of both emitted classfiles, including private helpers,
synthetic lambda, constructor, initialization and nested SAM, saved as
`tests/corpus/numbers/Complex-1.3-declarations.txt`; temporary reproduction/evidence
directory `.tmp/numbers-complex-v310-api`. There are54 public callable descriptors
(including the nested SAM method),35 private callable descriptors (including the
constructor and synthetic lambda), and the static initialization descriptor.

Still not accepted:20 unmodelled call signals remain, including four reachable
Math.scalb calls to an unimplemented facade. Next batch models this general JDK
operation in the existing typed native-call/runtime path, testing both float
and double overloads and scaling boundaries. Installed JDK bytecode evidence
`.tmp/numbers-complex-v311-jdk-math-bytecode.txt` confirms bounded exact-power
scaling (double split into512-bit steps, float via a wide double intermediate),
not naive pow multiplication. Full Complex numeric/API/parse/identity comparisons
and accepted-corpus preservation gates remain outstanding. Goal stays3/6.

### V312–V313 general scaling and first complete Complex behavior baseline

Math.scalb now uses the ordinary typed native-call target and shared scalar
runtime support. Both float/double overloads, Math/StrictMath/qualified/static
imports and source-owner precedence are covered. Exact normal powers of two
and bounded512-bit steps avoid spurious intermediate overflow/underflow for
double; float uses an exact wide product and one final float rounding.
Shared Math return inference now recognizes boxed primitive operands.

New ScalbProbe/ScalbOracle fixtures compare15856 independent JDK request rows,
both Math and StrictMath (31712 exact raw-bit comparisons), covering both signs,
quiet NaN payloads, normal/subnormal/overflow/underflow/tie boundaries, extreme
int exponents and deterministic finite samples. V313's additional null-unboxing
probe exposed that emission-only argument slots hid effects from analysis.
The slots now live in plan_call_arguments_with_facts_and_args, so overload,
physical argument conversion and shaped-call null-effect analysis share one
contract. Numeric overload inference also projects the solved nullable carrier.
No new exception scanner, runtime guard convention or corpus exception added.
`.tmp/numbers-complex-v313-scalb-focused.log`:all4 native numeric tests pass,
including typed caught null failures for Float/Double, widening, evaluation
order, imports and source lookalikes. V312's full1679 library suite and11
comparison-policy tests pass. V313 has focused evidence, not yet a full-suite
or full-corpus claim.

Frozen V312 `.tmp/new-java2rust-numbers-default-v312-debug`, SHA256
`14445f2ce8780dec43529cf1e43498717e04a7f043541417ffdb9240fce24f32`.
`.tmp/commons-numbers-1.3-complex-v312-default`, run log
`.tmp/numbers-complex-v312-run.log`:complete2-source generation/parse/compile
pass,0 unsupported/invalid/compiler errors,0 no-op call sites. No Math.scalb
stub remains in generated Complex. Translation156892ms, assembly6381ms,
total178237ms while the library suite also ran.19 unmodelled inventory signals
remain and require reachability audit, not blanket dismissal.

New independent caller-only ComplexOracle.java/.rs and runner/verifier mode
--numbers-complex exercise14617 ordered observations:all declared public
operations/overloads, static values and repeated identity, signed-zero branch
cuts, normal/extreme/subnormal/nonfinite inputs, immutable state, pair/scalar
matrices, ordered positive/negative roots and parsing/error cases. This is a
first coverage baseline, not final branch/null-boundary acceptance. Java caller
executes untouched pinned production; Rust caller only invokes generated APIs.
Verifier keeps the existing exact comparison default and snapshots both callers.

Pre-scaling preserved output has1241 strict differences, including1156 panics
from the scaling stub: `.tmp/numbers-complex-v311-prescaling-behavior` and log.
V312 has0 panic observations and87 strict differences:
`.tmp/numbers-complex-v312-behavior`, matching behavior log. Precisely:65 finite
transcendental-result observations;1 huge-power infinity-sign difference traced
to amplification of an atan2 last-bit difference;10 arithmetic NaN-sign-only
observations;2 division NaN-payload-selection observations;9 parsing semantics.
No comparison allowance has been enabled or broadened. Both verifier failures
are expected terminal1 evidence, not successful acceptance or a blocker.

New failure-family rewrite audit before the next campaign:

- Scalar parsing: replace Rust's general parse grammar at the shared primitive
  parse projection with typed JDK-compatible float/double runtime parsing.
  Preserve existing null handling/Result propagation. Validate Java whitespace,
  signs, exact special tokens, suffixes, decimal/hexadecimal grammar and correctly
  rounded hex/subnormal conversions. Both overloads need independent raw-bit and
  exception-message matrices. This is a shared runtime/emission change, no
  Complex source rewrite or per-string exception.
- Unicode: generated String length is byte-based, charAt/substring scalar-based,
  and search offsets byte-based, whereas Java indices are UTF-16 units. Minimal
  correction must make all four operations share a UTF-16 contract. Rust char
  and Arc<str>-backed JavaString cannot represent isolated surrogate units;
  do not silently substitute scalar indexing or replacement characters merely
  to pass Complex's invalid-input cases. Full String/char representation scope
  requires architectural review before a redesign campaign.
- Elementary math: compare the actual primitive calls independently, especially
  atan2/cosh, before considering documented numerical allowances. A last-bit
  input error can be amplified into an opposite infinity sign in a huge power;
  a blanket final-value tolerance cannot justify that observation. Compatible
  elementary kernels are expressible in existing runtime/native-call paths;
  no source algorithm replacement is needed. Keep strict residuals visible.
- NaNs: sign-only and payload-selection differences are distinct. The existing
  Core sign-only policy must not silently admit changed payloads or raw copies.

Next coherent batch is scalar parsing plus its independent grammar/rounding
matrix; Unicode and elementary-math gaps remain explicit. Accepted corpus
preservation gates and final Complex acceptance are outstanding. Goal stays3/6.

#### User clarification: motivate FP compatibility work with real usage

Do not treat artificial stress inputs as automatic acceptance requirements or
reasons to reproduce a particular JVM's elementary-function rounding. The
huge-power infinity-sign observation above comes from the caller's artificial
1e200 exponent; retain it as sensitivity evidence, but it does not by itself
justify an atan2 kernel compatibility campaign. No realistic downstream need
has yet been demonstrated for that exact phase at that exponent.

Scaling is motivated by the production division algorithm's explicit exponent
normalization. Signed-zero branch cuts are motivated by Complex's explicit
complex-function conventions. Parsing grammar/errors are ordinary public-API
behavior. Prioritize these concrete paths, reasonable numerical workloads and
invariants. Investigate small elementary-function differences at their primitive
boundary; use a documented numerical contract where justified rather than
requiring one JVM's bits everywhere. Keep stress diagnostics visible and
separate from acceptance coverage; this is not permission to hide ordinary
state/exception/branch bugs with a blanket tolerance. Require an actual code
path plus plausible operational input/downstream need before spending effort
on further FP corner cases. This clarification supersedes any implication above
that every adversarial matrix observation is a mandatory exact-match gate.

### V314 shared Java floating parser — six Complex parsing mismatches fixed

New dependency-free kernel `src/java_float_parse.rs` is used both by source
hexadecimal literal parsing and emitted runtime float/double parsing. It owns
Java trimming/token/suffix/decimal/hex grammar and validation messages. Decimal
conversion delegates to Rust only after lexical validation. Hex conversion
retains leading significand bits plus guard/sticky information and rounds once
to the selected precision, including subnormal results; float is not rounded
through double. Removed the older independently accumulated hex-literal parser
and its potentially underflowing pow multiplier. Existing optional String null
checks, typed NumberFormatException mapping and Result propagation are retained.
Both direct primitive-parse projections and method-reference parse helpers use
the kernel. Source-only and platform-facade support inventories both include it;
the first broader run caught and corrected a missing source-only declaration.

Motivation is ordinary public Complex.parse behavior, including whitespace,
hex/suffixed numbers and rejecting non-Java tokens inf/nan, not reproducing JVM
elementary-function rounding on artificial FP stress inputs. Literal/runtime
reuse also removes a second parser for the same numeric grammar.

Independent FloatParseProbe/FloatParseOracle execute ordinary decimal/config
values, Java decimal/hex round trips, both widths, signs, validation messages,
method references and caught nulls. Minimal rounding/literal boundaries verify
the general parser's conversion contract.972 exact Java raw-bit/error-message
observations pass; the generated source-client fixture also passes, including
minimum-subnormal literals and a float double-rounding regression. A cheap
standalone kernel comparison allowed validation before another expensive build.

`.tmp/numbers-complex-v314-library-focused.log`:1679 library,2 floating parser,
4 native numeric and30 Numbers tests pass (1715 total), terminal0. The native
numeric suite includes V313's general shared-slot null-unboxing regression.
11 strict/portable comparator tests pass; explicit edition2024 formatting and
diff checks pass. These are passing scoped suites, not an all-repository test
claim. Existing rendering expectations were updated to require the Java parser
helper while retaining NumberFormatException and exception-propagation checks.

Five additional focused existing tests were checked. Numeric-parse argument
borrowing and temporary stream/lambda parsing pass. Three have pre-existing
unrelated expectation failures: numeric facade tests still expect panic-recovered
unchecked exact/floor operations instead of existing Result returns; stream
fixture code calls unwrap on existing Vec results; ndarray checks expect old
unretained array declarations instead of existing retained/checked allocation.
Evidence `.tmp/numbers-complex-v314-existing-parse-tests-rerun.log`. Verified
against frozen pre-parser V311 on the same Java inputs:
`.tmp/v311-existing-{jdk_numeric,stream,ndarray}.rs` and matching inputs/logs.
Do not attribute these to this parser batch, silently ignore them, or claim the
five-test run is green. No unrelated expectations were weakened or tests removed.

Frozen `.tmp/new-java2rust-numbers-default-v314-debug`, SHA256
`91197e85f509575747cee73a2f6c6b7a7a00ba628180ceb6b3c6464486bc7823`.
Later edits to test expectations only do not change this production checkpoint.
Complete Complex `.tmp/commons-numbers-1.3-complex-v314-default`, log
`.tmp/numbers-complex-v314-run.log`:2 unchanged production files,0 unsupported,
parse/compiler/invalid errors; total160171ms, translation134670ms (see summary
for authoritative timing), assembly/check complete. Behavior evidence
`.tmp/numbers-complex-v314-behavior`, matching log:14617 observations,0 panics,
81 strict differences versus87 in V312. All six scalar grammar mismatches are
fixed. The only remaining parsing differences are three Unicode indexing cases;
the shared scalar parser itself handles their nonnumeric tokens correctly.
The other78 observations are unchanged numerical diagnostics (65 finite,
10 arithmetic NaN-sign,2 NaN-payload selection,1 huge-power amplification).
Their acceptance relevance follows the user clarification above; a strict
diagnostic failure is not automatically a requirement for JVM-specific kernels.

Fresh accepted Core preservation on this producer:
`.tmp/commons-numbers-1.3-core-v314-preserved`,
`.tmp/numbers-core-v314-preserved-{run,behavior}.log` and behavior directory.
All11 production files compile,0 unsupported/invalid/compiler errors;7888
observations pass under the already accepted arithmetic-nan-sign-v1 policy,
with the same four strict sign differences retained. Total152838ms, translation
113817ms, assembly12452ms. Other accepted Numbers/RNG final preservation gates
have not been rerun for V314 and remain final-boundary work.

All owned V314 jobs are terminal. Complex is not yet accepted. Next: a fully
automatically translated client exercising ordinary composed use, source-level
null/API/identity contracts, and an evidence-based numerical acceptance review.
Unicode indexing remains the explicit String/char architectural gap documented
above; do not disguise it as an FP tolerance issue. Goal stays3/6 modules.

### V315 translated Complex client baseline and contract audit

New client-only ComplexContractsOracle.java is compiled independently against
both untouched production sources, then staged as a separate third primary
source for automatic translation with frozen V314. Java contract=0/oracle=ok:
`.tmp/numbers-complex-v315-contracts-java.log` and Java output directory.
No generated output is manually repaired. Runtime production remains pinned
to the same exact commit; the caller is not upstream production.

The32 guarded outcomes cover ordinary Cartesian parse/string round trips,
immutable arithmetic, equality/hash/aliases/fresh construction, finite proj
returning the original receiver, repeated static-object identity, cubic-root
list identity/iteration/clearing and independent reallocation,8 caught null
paths (5 reference overloads, zero-base pow, parse and a null receiver), root
validation/message and malformed parse. Practical composed cases include AC
phasor current/voltage reconstruction, log/exp, sqrt/square and ordinary small
powers. Their explicitly finite-valued closeness checks use relative/absolute
1e-12, not raw JVM last-bit reproduction. Signed-zero branch-cut sides and
reference/state/exception contracts remain exact. This caller does not loosen
the strict numeric diagnostic comparator or excuse the known Unicode gap.

Whole-path audit before interpreting this new baseline: null calls must widen
the selected Complex reference parameters and parse String parameter, seed
their dereference effects, propagate Results through callbacks/helper calls and
reach the existing typed catch protocol. nthRoot's result must use the same
retained list carrier from its ArrayList producer through return signature,
caller storage, identity, iterator and mutation. Source references remain shared
object handles, not payload clones. Finite proj must retain this allocation,
whereas fresh arithmetic results must remain distinct. Existing shared storage,
native collection and effect mechanisms can express these requirements; inspect
any generated failure before choosing fixes or assuming a redesign.

Shared staging/Java-oracle implementations are now
`scripts/run-commons-numbers-1.3-contracts.sh` and
`scripts/run-commons-numbers-1.3-contracts-java-oracle.sh`. Explicit Core/Complex
wrappers retain their module-specific entry points/defaults. Counts, full pinned
manifests, byte-identical copies and separate caller provenance remain checked.
Both wrappers were exercised with a read-only checking runner:12 complete staged
Core sources and3 Complex sources, with matching manifest lengths. Logs
`.tmp/numbers-{core,complex}-v315-wrapper-staging.log`. Core's Java wrapper also
still returns contract=0/oracle=ok; shell syntax and11 comparator tests pass.
The new verifier mode is --numbers-complex-contracts and links only the tiny
Rust caller to the generated production/client crate. Production-source edits
have not been made in V315; its baseline job/evidence must be inspected next.

#### V315 result: full translated client passes

Complete production plus client baseline
`.tmp/commons-numbers-1.3-complex-contracts-v315-baseline`, run log
`.tmp/numbers-complex-v315-contracts-run.log`:all3 primary files translate,
parse and compile,0 unsupported/invalid/compiler errors,0 no-op call sites.
Translation110583ms, assembly7100ms, check1568ms, total134879ms. Complete
production/test-caller separation is recorded in contract-source-manifest.tsv.
No translator fix was needed for the new client. In particular the generated
public reference parameters widen correctly, catches receive typed null errors,
and nthRoot's retained native collection survives return/storage/identity/
iteration/mutation consistently. Source parse now receives Option<String> in
this client context, rather than being manually wrapped by a Rust caller.

`.tmp/numbers-complex-v315-contracts-final-evidence` and
`.tmp/numbers-complex-v315-contracts-final-behavior.log`:strict contract=0 and
oracle=ok match; both caller snapshots/provenance preserved. The result message
now correctly describes translated client identity/null/finite-workload checks,
not raw floating-bit identity. The small Rust driver only invokes the translated
run method. All32 guards pass independently on Java and generated Rust.
Core wrapper Java and both staging checks also pass; all V315 jobs terminal.

Remaining String-indexing audit: StringLen currently emits byte len;
coercion::string_char_at_result_projection uses chars().nth; substring in
stdlib.rs uses chars()/scalar ranges; search-from slices byte offsets. JavaString
identity storage is Arc<str>, and Java char storage/literals use Rust char. These
cannot preserve arbitrary UTF-16 code units or a substring splitting a surrogate
pair. Fixing only length to chars().count would help BMP examples but is not a
general Java indexing solution. The shared float parser already rejects the
Unicode numeric tokens correctly; validation fails before reaching it because
the enclosing Complex parser uses mismatched indexing units.

A proper general fix needs a coherent String/char contract for UTF-16 length,
charAt, search, substring, numeric char conversion and formatting/identity.
Numeric widening or replacing malformed units must not silently paper over the
representation issue. This is a shared framework scope decision, not a
Complex-specific numeric correction. Practical Complex calculations and source
null/identity contracts now pass; numerical diagnostics and the three malformed
Unicode inputs remain explicit, with no acceptance claim or silent exclusions.

### V316 bounded formatting fix and concrete UTF-16 migration evidence

The requested Unicode scope choice has not been answered; an automatic goal
continuation is not permission to defer the known limitation or start a broad
String/char rewrite. This turn quantified the contracts and fixed an independent
local compiler gap encountered by the conformance probe.

UnicodeIndexingProbe.java/.rs are diagnostic callers, not manually repaired
deliverables. V314's untouched translation initially has8 compiler errors:
Integer.toHexString receives Rust char where its native i32 formal requires
Java widening. The general fix models Integer/Long hexadecimal, octal and binary
formatters as typed native calls. Shared argument planning owns int/long width,
widening and checked unboxing, so exception analysis sees nullable operands.
Structured format-macro emission prints the correct unsigned two's-complement
width; no source-specific casts, late text rewrites or new storage adapters.
Source lookalikes remain source methods. The same original char formatting
calls now compile without modifying the probe to insert manual widening.

IntegralFormatProbe independently passes on Java/generated Rust:char/byte/short
widening,32/64-bit negative widths and minima, static imports, boxed inputs/null
catches and a source SAM method reference. The existing shadow fixture also
proves source Integer.toHexString wins while qualified java.lang.Integer stays
native. `.tmp/numbers-v316-integral-format-focused.log`:all5 numeric and2 float
parser tests pass; the added method-reference guard also passes in
`.tmp/numbers-v316-integral-format-reference.log`. Shared call-plan/lowering
units34+13 pass in `.tmp/numbers-v316-call-unit-tests.log`. Formatting/diff checks
pass. No full corpus rerun after this small native-call batch.

Frozen `.tmp/new-java2rust-numbers-default-v316-debug`, SHA256
`ab7d0688800c72671d16d21d4cd47dcd71ce4a7f57d580f10be01bdfb2d639da`.
The probe is automatically translated with this producer, compiled directly by
rustc and called through its generated public methods. Evidence directory
`.tmp/unicode-indexing-v316`:Java output, original compiler failure, post-fix
generated Rust/compilation logs, final caller build and raw Rust output/stderr.
One probe-only Boolean.toString call initially encountered a separate unmodelled
API. Replaced it with a Java conditional returning the same true/false strings
to isolate indexing; pre-adjustment source/output are retained and Java outputs
are byte-identical. No pinned corpus source or generated Rust was edited, and
Boolean.toString coverage is not claimed by this diagnostic.

| Contract for indexed multilingual text | Java | Generated Rust after widening fix |
|---|---|---|
| length of Aé😀Z |5 UTF-16 units |8 UTF-8 bytes |
| successive charAt units |41,e9,d83d,de00,5a |out-of-bounds panic |
| search positions and from-index |4,2,1,4,4 |UTF-8 boundary panic |
| substring(2,4) equals the emoji |true |false |
| split surrogate substrings and recombine |1,1,d83d,de00,true |out-of-bounds panic |
| cast0xd83d to char, then widen |d83d,d83d |0,0 |

Concrete minimal coherent migration, if Unicode remains in this campaign:

1. Canonical Java code-unit value (u16-backed JavaChar) at primitive storage,
   literal/constant producers and nullable boxed Character slots. Java numeric
   widening reads the code unit; narrowing wraps16 bits. Replace the current
   char::from_u32(...).unwrap_or(NUL) path, which silently destroys surrogates.
2. Lossless UTF-16 String payloads, reusing JavaString's existing shared identity
   and interning mechanisms. Parser/IR literals must preserve isolated units;
   UTF-8-only source literals cannot carry them. Scalar UTF-8 text may be a fast
   view/cache, not the authoritative value or an implicit lossy AsRef<str> path.
3. Unify length, charAt, search, substring, concatenation/builders, equality/hash
   and primitive String conversion on those units. A substring splitting a pair
   is valid Java and must survive until concatenation; replacement characters
   or new exceptions would not be semantically correct. Explicit encodings at
   I/O boundaries must not contaminate internal values or exception messages.
4. Trace producer/storage/formal/return/consumer contracts through the existing
   slot/representation planners, including String views, arrays, append/display
   and JavaException message access. Batch these coherent contracts before
   expensive corpus gates, with this small conformance oracle as the cheap gate.

This is representation/framework work, not six independent String emission
fixes. charAt cannot return a Rust char for every Java unit, and Arc<str> cannot
store every legal substring. A chars().count-only patch would still be wrong.
The probe establishes the need and the rewrite scope; it does not authorize or
implement a partial compatibility substitute. V315's practical translated client
remains passing on its frozen producer; Complex remains unaccepted, goal3/6.

### V317 preservation boundary and bounded elementary-math audit

No Unicode deferral, String/char rewrite or numerical-policy change was inferred
from the automatic goal continuation. Safe remaining preservation work was
completed on the frozen V316 producer; all four translation-producer manifests
confirm SHA256
`ab7d0688800c72671d16d21d4cd47dcd71ce4a7f57d580f10be01bdfb2d639da`.

| Scope | Complete-source/behavior evidence | Result |
|---|---|---|
| Primes | `.tmp/commons-numbers-1.3-primes-v317-preserved`, `.tmp/numbers-primes-v317-preserved-{run,behavior}.log` and behavior directory | all3 sources compile;3594 exact observations |
| Root Finder | `.tmp/commons-numbers-1.3-rootfinder-v317-preserved`, `.tmp/numbers-rootfinder-v317-preserved-{run,behavior}.log` and behavior directory | all3 sources compile;122 exact observations |
| Combined RNG native/sampling | `.tmp/commons-rng-1.0-sampling-all-v317-numbers`, `.tmp/rng-sampling-all-v317-numbers-run.log`; `.tmp/rng-{native,sampling-collections,sampling-distributions}-v317-numbers-behavior` directories/logs | all46 scoped files compile;12601+162+2172 exact observations |
| Isolated RNG source64 | `.tmp/commons-rng-1.0-source64-v317-numbers`, `.tmp/rng-source64-v317-numbers-{run,behavior}.log` and behavior directory | all11 scoped files compile;2865 exact observations |
| Full library | `.tmp/numbers-v317-preservation-library.log` | all1679 pass,78.68s |

Every complete-source gate has0 parse/unsupported/compiler/invalid errors,
0 no-op call sites and0 stubbed constants. All behavior jobs terminal0. RNG
preservation totals17800 exact observations; with Primes/Root Finder the fresh
V316-producer preservation total is21516. Full library and all owned jobs are
terminal0. No repeated restarts or partial-output success claims.
Core's7888-observation V314 preservation remains separately recorded. It was
not regenerated for V316's formatting-only change: inspecting all Core production
sources finds no toHexString/toOctalString/toBinaryString calls. Do not relabel
that earlier evidence as a fresh V316 Core run. Corpus staging remains untouched
and pinned; no acceptance scope or source manifest changed.

Elementary-function evidence motivated by the actual cosh/atan2 differences in
Complex: `.tmp/elementary-v317` contains small independent Java/Rust primitive
callers, raw outputs and a100-decimal-digit mpmath1.3.0 audit. This is diagnostic
mathematical analysis, not a reimplementation of Complex or a new translator
dependency. Host Java19.0.2 and Rust1.98.1 were used.114 ordinary-input
observations (10 cosh,100 nonzero atan2 pairs,4 signed-zero branch-cut cases)
have10 strict last-bit differences. All4 signed-zero cases match exactly.

For the sampled nonzero inputs, maximum absolute errors in units of the nearest
reference float's spacing are:cosh Java0.7024563251/Rust0.4303396891; atan2
Java0.8056948425/Rust0.4944584702. At cosh(1), Java error is0.70245633 ulps and
Rust0.29754367; at atan2(-3,-2), Java0.50554153 and Rust0.49445847. Thus a
different bit is not evidence that Rust is less accurate. Both sampled
implementations satisfy the relevant Java Math accuracy limits:cosh2.5 ulps
and atan2 2 ulps, as documented by the
[Java19 Math specification](https://docs.oracle.com/en/java/javase/19/docs/api/java.base/java/lang/Math.html).
This is evidence over the stated inputs, not a proof for every input or a
blanket output tolerance for composed algorithms. No comparator allowance was
enabled, no original strict difference was discarded and no JVM-matching
elementary kernel was implemented. It supports the user's practical-use rule.

Complex's practical translated client remains green on its frozen checkpoint;
its strict numerical diagnostics and known String/char gap remain explicit.
The Unicode scope decision is still pending. Overall goal remains active3/6,
not complete or blocked: this turn completed real preservation and numerical
classification work without silently narrowing the objective.

### V318 lossless text kernel staged; original Unicode scope retained

Execution clarification:the earlier optional Unicode-deferral question does
not authorize narrowing the active goal and need not become a self-imposed
blocker. No deferral was requested. Keep general correctness in the original
scope and stage a coherent migration, while respecting the user's practical FP
prioritization. No new locale/casing/I/O feature campaign is implied beyond
the contracts needed by the existing text operations and their consumers.

The diagnostic experiment `.tmp/utf16-v318` proves a minimal lossless value
model can express all6 V316 conformance cases and4502 independent Java
observations. It uses16-bit code units plus UTF-16 text, retaining the existing
Arc/intern-pool ownership approach in a test-only object wrapper. The matrix
covers14 scalar/mixed/isolated-surrogate sequences, charAt/substring bounds,
search from negative/large offsets, code-point versus code-unit searches,
concatenation, unsigned-unit ordering, Java hashing and identity/interning.
Java and Rust outputs match exactly; raw sources/outputs/build logs/differences
are retained. This is feasibility evidence, not repaired translated output.

Staged production component `src/java_text.rs` now defines JavaCodeUnit,
JavaUtf16 payload and structured JavaTextBounds. It preserves every16-bit unit,
wraps char narrowing, widens without sign extension, implements UTF-16 indexing,
substring/search/concat/hash/order, and makes scalar UTF-8 conversion explicitly
fallible. No Display/Deref/AsRef<str> conversion silently replaces a surrogate.
Payload values do not own object identity or an interning pool:those stay in
the existing JavaString object layer. Allocation limits remain owned by the
Java allocation boundary; the payload asserts its int-length invariant.

`src/java_text_tests.rs`:4 unit tests pass, including all65536 char values,
wrap/zero defaults, BMP/supplementary positions, split/recombined surrogate
substrings, invalid bounds and code-unit search/hash/order. Evidence
`.tmp/utf16-v318/kernel-unit-tests.log`. New permanent independent JDK component
test `tests/java_text_contracts.rs`, oracle JavaTextKernelOracle.java, passes
4460 payload observations: `.tmp/utf16-v318/kernel-java-contracts.log`. The42
object-identity observations in the prototype matrix are separate; do not
pretend the value payload implements identity. The product source kernel also
passes the full4502 matrix and6-case probe through a test-only Arc/intern
adapter (`product-*.txt`, zero differences). That adapter is diagnostic, not
the translator's active String implementation.

The component is compiled/tested but deliberately not activated in emission.
Frozen `.tmp/new-java2rust-numbers-default-v318-debug`, SHA256
`615339876a9834c9742fdfd257a30d328855a55e21d872dea194bb9f58cf9837`.
Its automatically generated UnicodeIndexingProbe output is byte-identical to
V316 (`current-generated-probe.rs`, cmp terminal0), proving no premature
representation switch or claim of fixed corpus behavior. The current translated
probe still fails its Unicode contracts; Complex is not accepted. No full
corpus rerun was useful for this inactive, additive component. All owned jobs
are terminal; current component/default-build checks pass.

Next coherent integration batch must cover:

- Reuse the existing JavaString identity/interner layer with a UTF-16 payload,
  and select one canonical text/char physical contract through authoritative
  representation/slot policies. Register runtime types in existing shared-item
  inventories instead of inventing new naming/deduplication paths.
- Preserve code units in literals/constant evaluation and primitive char casts;
  Rust char and scalar-only IR literals cannot be the fallback for surrogates.
- Wire owned/borrowed/nullable String and Character producers, formal/return
  signatures and consumers together. String charAt/length/search/substring and
  their compound expressions must share the same index units.
- Adapt append/concatenation, String views, primitive parsing and JavaException
  message/getter boundaries without implicit lossy conversion. Keep encoding
  decisions at explicit interop boundaries. Internal UTF-16 and valid scalar
  text must not be confused merely because both are called String.
- Keep the default representation unchanged until these contracts form a
  coherent batch; use the fast standalone product-kernel/JDK matrix during
  development, then automatically translate the original probe, complete
  Complex+client and accepted-module preservation gates.

No source files were skipped or manually repaired, and no numerical comparator
was relaxed. The full six-module objective remains active,3/6 accepted. The
staged kernel is a migration component, not a new success definition.

### V319 lossless source-literal and constant producers

Completed the producer side of the V318 migration plan without prematurely
switching physical String/char carriers. `Literal` now retains isolated Java
code units as `CharUtf16(u16)` / `StringUtf16(Vec<u16>)`; scalar-compatible
values keep their existing variants. Both variants have the same Java type.
The scalar emitter explicitly diagnoses unsupported lossless storage rather
than substituting zero, U+FFFD, or a fabricated value. This diagnostic is an
intermediate migration boundary, not an acceptable final corpus result.

One shared `java_literal` decoder now handles char, String and text-block
tokens: Unicode escape eligibility/nonrecursive translation before lexical
escapes, repeated-u spellings, all Java lexical escapes, correct two/three-digit
octal limits, text-block newline/indent processing, and raw UTF-16 payloads.
Removed the separate incomplete char decoder and scalar-only String escape
decoder. The Java rules were checked against [JLS 19 lexical structure](https://docs.oracle.com/javase/specs/jls/se19/html/jls-3.html).

Independent javac comparison initially caught tree-sitter's repeated-u spelling
boundary (`"\uuuu0041"` was misparsed before value decoding). Normalize only
equivalent escape spelling, sharing the eligibility scanner; do not decode
backslashes twice. Restore original tree byte/point ranges before extracting
any nodes using [tree-sitter's range-edit API](https://tree-sitter.github.io/tree-sitter/using-parsers/3-advanced-parsing.html).
The new parser regression checks exact original slices across multiple edits,
multibyte/supplementary text, two fields and a later method. Unaffected sources
borrow their input; normalization and coordinate restoration are linear.
This is not a claim of complete source-wide Unicode preprocessing: escapes
outside literals that change tokenization remain a separate existing boundary.

Constant values now use u16 / Vec<u16>, so surrogate-bearing intermediate
constants survive folding. Char casts wrap low16 bits, floating-to-char casts
first use Java's saturating int conversion, numeric promotion widens unsigned
units, and constant concatenation can join separated surrogate halves into a
valid pair. Scalar-only facade rendering remains explicitly fallible. Updated
literal typing, anonymous-call argument typing, owned/non-null literal
classification, runtime-support detection and integral switch-label rendering
to retain the same semantic contracts for both literal forms.

Permanent tests: `java_literal` unit tests cover all65532 char Unicode-escape
spellings that do not require a separate lexical escape, eligibility and
nonrecursion, octal limits, text blocks and invalid input. New
`tests/java_literal_contracts.rs` independently compiles/evaluates2326 literal
tokens with javac and compares their exact code units to the actual parser:
all2048 surrogates,256 octal chars,22 mixed/escape/text-block cases. The oracle
shares tokens, not decoded expectations. The automatically translated
`LiteralEscapesProbe` also passes its10 self-checks on both Java and generated
Rust; no generated output was repaired. Initial failing evidence is retained
in `.tmp/utf16-v319-jdk-contracts.log`; the corrected first run is
`.tmp/utf16-v319-jdk-contracts-rerun.log` (2 tests pass). The final range-restored
producer also passes both tests in `.tmp/utf16-v319-final-jdk-contracts.log`.

Library evidence: initial1691 and subsequent1692-test runs passed. The final
product-code boundary run `.tmp/utf16-v319-final-tests.log` passed1692 tests
and failed only the new source-range test's incorrect whole-field expectation.
Inspection of `parse_field_decl` proves spans name variable_declarator, not
field_declaration; corrected only that test expectation. The focused final
parser run `.tmp/utf16-v319-final-parser-tests.log` passes all4 tests, including
exact original-coordinate checks. No product code changed for that correction.
The final full-library rerun `.tmp/utf16-v319-library-green.log` passes
all1693 tests (0 failed,76.92s). All V319 owned jobs are terminal, and final
formatting / `git diff --check` checks pass.

Complete untouched Complex production (2 files) plus its independently
translated client (1 file) regenerated and compiled: zero parse failures,
unsupported statements/members/expressions, invalid patterns, compiler errors,
reachable no-op calls and stubbed constants. Artifact
`.tmp/commons-numbers-1.3-complex-contracts-v319`; run log
`.tmp/numbers-complex-v319-contracts-run.log`; exact client behavior
`.tmp/numbers-complex-v319-contracts-behavior/result.txt`:
`contract=0`, `oracle=ok` (all32 client guards). That checkpoint's frozen
producer `.tmp/new-java2rust-numbers-default-v319-debug` SHA256
`b038cc49d26a25280806f2b09bd46065bc2198d40ee715683f23698d3b679954`.
The later original-range restoration is only active when spelling normalization
removed extra u characters; none of these3 sources contains that spelling.
Do not relabel the artifact's recorded producer as a different binary.

Final product producer `.tmp/new-java2rust-numbers-default-v319-final-debug`,
SHA256 `76d9c68fb5151790887ddc279f16604584469b021759daaa0e0e0334361c95a1`.
Its automatically translated original UnicodeIndexingProbe is byte-identical
to V318 (`.tmp/utf16-v319-generated-probe.rs`, cmp terminal0): there was no
premature carrier switch and its runtime Unicode failures are still open.
No upstream production sources or generated deliverables were edited. No
numerical comparison policy changed. Accepted earlier-module preservation is
still the explicitly labeled V314/V317 evidence, not an invented fresh rerun.

Reproduce the producer-side gates with:

```
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" cargo test --lib
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" cargo test --test java_literal_contracts
```

Next: reuse JavaString's identity/interner layer with the lossless payload;
connect one canonical String/char physical contract through storage, borrowed
and nullable views, signatures, casts, arrays, append/concat, parsing and
exception-message consumers. Do not turn this producer checkpoint into a
smaller success definition. Complex remains unaccepted; the full goal remains
active,3/6 modules accepted.

### V320 shared String identity connected to the lossless payload

Completed the next object-layer migration batch. The existing emitted scalar
JavaString now delegates its allocation identity, value equality, literal
interning and identity-pointer operations to the producer-owned generic
`JavaStringIdentity<P>` leaf in `src/java_string_identity.rs`. It retains the
same Arc<str> allocations and scalar API; its public source-level storage and
borrowed signatures did not silently change. The staged UTF-16 object uses
that **same** identity/interner mechanism with JavaUtf16 payloads, not a second
object model or an LLM-repaired generated program.

`src/java_utf16_string.rs` combines immutable raw-unit values with fresh
allocation, clone/reference equality, value equality/hash/order, interning,
length/charAt/substring/search and concatenation. Important identity contracts:
whole substrings retain the receiver; proper empty substrings use the interned
empty string; concat with an empty argument retains the receiver, while
runtime +/append allocates a fresh result. Instance intern adopts the actual
receiver when first inserting a value; it must not copy into a new object.
Literal factories and instance interning share one locked lookup/insert path.
An eight-thread regression proves exactly one original reference wins a race.
Whole-substring identity avoids allocating/copying a throwaway payload.

The lossless object intentionally has no AsRef<str>, Deref<str> or Display
implementation. `scalar_string()` is explicit and fallible, preserving raw
surrogates internally. Three compile-rejection tests prevent a later consumer
compilation fix from introducing an implicit lossy scalar contract.

Added the producer-owned `RuntimeSupport::LosslessJavaText` export, available
through `emit-runtime-support lossless-java-text`. Host components and emitted
runtime use the same source files via include_str, not forked implementations.
All five component names (JavaStringIdentity, JavaCodeUnit, JavaTextBounds,
JavaUtf16, JavaUtf16String) are discovered by the existing authoritative
support-name inventory. A permanent assembly test passes the actual exported
runtime through `merge-shared-support-facades.py` for two modules, compiles the
merged output and checks shared nominal types and cross-module intern identity.
No assembly-specific replacement runtime or hard-coded deduplication was added.

Extended the independent JavaTextKernelOracle to4951 exact observations:
the original4460 value/bounds/search cases plus491 reference observations over
14 mixed/scalar/isolated-surrogate payloads, including cross-payload concat,
empty/full substring, fresh/clone, runtime append and receiver-adopting intern.
The caller-only Rust observation code lives in
`tests/corpus/numbers/JavaTextKernelOracle.rs` and delegates all operations to
the production component. Both the host component and separately emitted
runtime match javac/java exactly. Final test log
`.tmp/utf16-v320-object-contracts-final.log`: both value/identity and assembly
tests pass, plus the chained-reference-assignment executable regression.
Earlier4937-observation run `.tmp/utf16-v320-final-contracts.log` also passes
the two append regressions and all3 virtual-object-string regressions.

Full library `.tmp/utf16-v320-library.log`:1698 pass,0 fail,79.56s, including
shared support-name inventory and concurrency coverage. Formatting and
`git diff --check` pass. An attempted broader natural_order_consumers suite
passed3 tests (including identity String ordering) and exposed1 unrelated
nullable-comparator failure with8 compiler errors. Its CLI output is byte-
identical with V319 and V320; frozen V319 output independently fails rustc with
the same8 errors. Evidence `.tmp/utf16-v320-natural-order-{v319,current}.rs`,
V319 compiler log `.tmp/utf16-v320-natural-order-v319-compile.log`, and initial
test log `.tmp/utf16-v320-contracts.log`. No test was skipped or weakened to
claim that unrelated suite green; it remains an existing issue.

Frozen producer `.tmp/new-java2rust-numbers-default-v320-debug`, SHA256
`139e7b5a5661cde6c78bb7075a9d19cb93b84d3d0ee0d4559fc123a39227d5d2`.
Direct CLI export `.tmp/utf16-v320-emitted-runtime.rs` (10763 bytes) compiles
standalone, log `.tmp/utf16-v320-emitted-runtime-check.log`. Automatically
translated UnicodeIndexingProbe `.tmp/utf16-v320-generated-probe.rs` is
byte-identical to V319 (cmp terminal0). This proves the active source-level
String/char representation has **not** yet been switched or fixed. All owned
jobs are terminal. No upstream source, pinned manifests or numerical policy
changed, and no full corpus rerun was useful for a refactor confined to the
scalar identity runtime plus a not-yet-selected lossless carrier. The existing
V319 Complex client and V314/V317 accepted-module evidence keep their original
producer labels.

Next integration audit, still required for the original goal:

- Cross-function representation contract: owned/nullable String storage,
  borrowed views, formal/return signatures, arrays/collections and erased
  references must select one physical carrier. Current StringView rendering
  still unconditionally produces Rust str; changing only owned String mapping
  would therefore be incorrect.
- Primitive and boxed char storage/literals/casts/defaults and charAt must
  select the same lossless code-unit contract. The current Rust-char fallback
  cannot be used for surrogate values retained by V319.
- Append/concat, parsing, exception messages and interop must consume those
  carriers coherently. The component's fallible scalar boundary must not be
  replaced by AsRef<str> to make those consumers compile.
- Reuse the existing whole-unit representation decision rather than adding
  expression-local patches. ASCII source literals are not evidence that a
  program cannot receive non-ASCII or surrogate-bearing runtime arguments.
  A literal-sensitive partial carrier choice would overfit the probe.

Reproduce focused component and existing identity paths with:

```
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" cargo test --lib
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" cargo test --test java_text_contracts --test virtual_object_strings --test native_append_contracts --test chained_reference_assignments
```

This is a verified object-layer migration component, not Complex acceptance or
a new success definition. Goal remains active,3/6 accepted; Complex, Quaternion
and Angle are not finished.

### V321: source String reference views, lifetime provenance and callback contracts

This batch connects existing identity-preserving String storage to real source
borrows; it does **not** yet switch the scalar payload to UTF-16. The baseline
simple relay compiled because signature normalization forced identity String
parameters to owned `JavaString`. That workaround is now removed in favor of
coherent physical views: `&JavaString` / `Option<&JavaString>`, not `&str`.
The exact Java String type selects the policy; unrelated String-like classes
do not inherit it. Scalar, nonidentity views retain their existing policy.

Owned/view conversion clones the reference handle rather than copying scalar
text. Optional and array-element conversions use the same shape/coercion
plans, without projecting through `as_str` or `as_deref`. Literal borrows use
interned handles, and the shared empty view has a stable interned owner.
Source-only identity translations now emit the complete exception/helper
dependency closure, even without platform imports or print calls.

The expanded executable fixture exposed three cross-function failure families:

- A returned/stored source SAM inferred a plain return while its lambda used
  `?` (E0277). The smallest correct rewrite gives the source interface and all
  its implementations one invocation ABI, while keeping callback creation
  separate from invocation. Target-typed source callbacks in method arguments,
  local declarations, assignments, returns and conditional values now reuse
  the existing source SAM descriptor and callback-contract equality solver.
  Method references use direct call edges, not equality with the referenced
  method's signature; a plain method may implement a checked callback.
- A callback inherited its creator's panic policy and panicked inside `Ok`
  instead of propagating a catchable null exception. Its lowering context now
  takes result propagation from the callback's own effective return contract.
  Merely constructing a throwing callback does not make a factory throwing.
- Temporary-backed returns/locals produced E0106/E0716. Minimal correct Rust
  retains an owning `JavaString` handle, using the existing `.cloned()` recovery
  where needed. All identity String locals therefore retain references, and a
  borrowed return requires formal-parameter provenance proved by the existing
  scope-aware return analysis. Unknown call results and mixed fresh/borrowed
  returns are owning. No statement-hoisting framework is needed for these cases.

Escaping callbacks also reuse the existing retained-reference capture mechanism:
opaque JavaString handles, like existing shared reference carriers, are cloned
before a move closure when the creator subsequently reuses them. Borrowed
nullable captures use the ordinary conversion plan to recover owning handles.
No new corpus-specific storage or exception mechanism was introduced. These
changes do not claim new coverage of direct field-initializer/constructor SAM
target typing; the implemented boundaries are those listed above.

Permanent independent Java/generated-Rust fixture
`tests/corpus/numbers/StringReferenceViewProbe.java` has 14 self-check groups:
fresh/equal distinct identity; direct/conditional/nullable relays; arrays;
read-only indexing/search/equality; ordinary Double parsing; returned/stored/
reassigned/conditional callbacks and method references; parameter rebinding;
catchable null callback invocation; temporary local/return/escaping capture;
mixed fresh/formal returns; and capture followed by creator reuse. Both runs
return zero. `tests/string_reference_views.rs` also compiles/runs a source-only
identity fixture and checks the emitted borrowed formal is `&JavaString`.

Latest product batch `.tmp/utf16-v321-final-capture-batch.log` is green:
1702 library tests, chained-reference assignment (1), native append (2), native
SAM (3), virtual object String (3), Java text contracts (2, including 4951 exact
JDK observations and shared assembly), and the 14-group String fixture.
The subsequent test-only source-dependency addition also passes both String
integration tests: `.tmp/utf16-v321-reference-complete.log` (33.77 seconds).
Earlier E0277, panic and lifetime experiments remain in
`.tmp/utf16-v321-{final-contracts,callback-contracts,temporary-contracts}.log`.
No acceptance assertion was removed or weakened.

Producer/artifact provenance is intentionally distinct:

- Main ABI/storage producer `.tmp/new-java2rust-numbers-default-v321-final-debug`,
  SHA256 `69752a4b8a27efb28a79f52b40b61869c72bbdd141d70d8398a1cc72ce498908`,
  generated `.tmp/commons-numbers-1.3-complex-contracts-v321-final`.
  Complete pinned Complex (2 production files) plus the client (1 file) compiles
  with zero parse/unsupported/invalid/compile errors, reachable no-op calls or
  stubbed constants. Independent Java/Rust behavior matches `contract=0` and
  `oracle=ok`, covering all 32 client guards. Logs:
  `.tmp/numbers-complex-v321-final-contracts-run.log` and
  `.tmp/numbers-complex-v321-final-contracts-behavior.log`.
- Latest capture extension producer
  `.tmp/new-java2rust-numbers-default-v321-capture-debug`, SHA256
  `3b644d2b068fd17f8ec45d568f309615fe563f843b2a436d8992b3ebcbf174f1`,
  generated `.tmp/utf16-v321-reference-capture.rs`. Its source snapshot
  `.tmp/utf16-v321-reference-source.java` has SHA256
  `5da6231e103ff99c330347e694b3feb55e59951906d8134f371058fba763446b`.
  The extension is guarded by the identity-String representation flag; the
  complete Complex artifact has no JavaString carrier and does not activate
  it. That artifact retains its actual 69752 producer label, not a relabeled
  latest-producer claim. Non-String capture branches are unchanged.

The latest automatically generated UnicodeIndexingProbe
`.tmp/utf16-v321-unicode-probe.rs` remains byte-identical to V320. This is direct
evidence that source UTF-16 payload/indexing/split-surrogate behavior is still
unfinished, not a passing Unicode gate. Next is the coherent physical String/
char payload and consumer migration: literal/default/numeric casts, indexing,
append/concat, parsing, exception messages and explicit interop boundaries.
An implicit lossy `AsRef<str>` on the lossless carrier is not a valid shortcut.

No upstream production source, pin, manifest, algorithm or numerical tolerance
changed. Accepted-module and RNG evidence remains explicitly V314/V317, not a
fresh broad-corpus claim. Goal remains active, **3/6 accepted**; this batch is
not Complex acceptance.

### V322: connect active String search consumers to the lossless kernel

The next consumer audit found a concrete independently fixable boundary:
`indexOf`/`lastIndexOf` used Rust byte offsets and sliced UTF-8 at Java offsets.
This affects actual Complex parsing (`Complex.java` lines 389/396 search for
the format separator), not a floating-point stress exercise. The existing
UnicodeIndexingProbe already demonstrated incorrect/panicking positions.
Frozen V321 capture producer also automatically translates the new 15-group
StringSearchProbe and compiles, but its independent caller fails with return
code 2 (wrong multilingual positions), rather than zero:
`.tmp/utf16-v322-search-baseline{.rs,-main.rs,-compile.log,-run.log}`.
The baseline producer is the recorded V321 `3b644d...` binary.

Minimal correct Rust routes searches through the existing JavaUtf16 kernel
with UTF-16 offsets and raw-unit needles. No new indexing algorithm or source
statement rewrite is required. The general implementation now does this:

- `JavaTextView` returns a borrowed-or-owned UTF-16 payload. Existing scalar
  `AsRef<str>` inputs explicitly encode at this boundary; JavaUtf16 and
  JavaUtf16String borrow their payload, including isolated surrogates. This
  does not give lossless text an implicit scalar projection.
- The existing search-needle contract now retains code units. Integer needles
  distinguish individual BMP units (including surrogate halves), supplementary
  code points and invalid values. References use the existing blanket adapter.
- Active source helpers and the lossless runtime export both include the same
  authoritative `src/java_string_search.rs`, calling existing kernel searches.
  Contains/prefix/suffix consumers use the same views. Whole-string storage,
  method selection and source operation signatures are not special-cased for
  the corpus or literal contents.

Connecting the runtime consumers exposed an assembly item-span error:
moving a `#[derive]` type left its preceding `#[repr(transparent)]` attached to
the next item. The shared Rust item utility now retains outer attributes
(including multiline/nested forms and quoted delimiters) and documentation;
inner module attributes are not moved. Trait classification is attribute/doc
aware so a documented trait retains its blanket implementations. Structural
tests cover these spans, and real two-module assembly compiles/runs the shared
carrier and search helpers with isolated-surrogate haystacks/needles. The
lossless view is asserted to borrow rather than copy its payload.

A preliminary attempt to insert a complete source translation into the bare
runtime-only assembler fixture also exposed that this fixture lacks the full
package runner's source imports/private-support closure. That experiment is
retained in `.tmp/utf16-v322-focused.log`; it is not a claim of a supported
complete-package assembly configuration. The runtime test uses its actual
two-module runtime scope; complete source assembly is checked separately with
the normal untouched Complex/client runner, not a patched generated program.

Permanent Java fixtures StringSearchProbe (15 groups, identity representation)
and StringSearchScalarProbe (5 groups, scalar representation) independently
return zero and automatically generated Rust does too. They cover multilingual
positions, offsets inside a surrogate pair, integer half/code-point searches,
invalid code points, empty needles, prefix/suffix/contains and ASCII controls.
Focused log `.tmp/utf16-v322-focused-final.log` passes both Java-text tests
(including the unchanged 4951-observation JDK oracle and expanded real assembly)
and all 3 String integration tests. Structural assembly controls pass 4 tests
and runtime-registry controls pass 6:
`.tmp/utf16-v322-{item-final,registry-final}.log`.

Frozen producer `.tmp/new-java2rust-numbers-default-v322-debug`, SHA256
`935d9924cde9f69d866723b4aa84d17b30398a8ef3049f2b1303625a084a0e2d`.
Automatically generated `.tmp/utf16-v322-unicode-probe.rs` now reports correct
`positions=4,2,1,4,4`, log `.tmp/utf16-v322-unicode-run.log`. Other diagnostic
lines remain `length=8`, `units=PANIC`, `wholeSlice=false`, `splitPair=PANIC`,
`numericChar=0,0`. This is an integrated search-consumer improvement, **not**
the completed physical String/char payload switch. Those remaining boundaries
still require the V321 whole-path migration; the scalar payload cannot retain
an isolated-surrogate substring. Goal remains active, **3/6 accepted**.

Final V322 batch `.tmp/utf16-v322-final-batch.log` passes 1702 library tests
(80.34 seconds), chained-reference assignments (1), Java text contracts (2),
native append (2), native SAM (3), String reference/search contracts (3), and
virtual object Strings (3). Reproduce with:

```
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" cargo test --lib --test string_reference_views --test java_text_contracts --test native_sam_contracts --test virtual_object_strings --test native_append_contracts --test chained_reference_assignments
```

Fresh untouched Complex (both production files) plus the translated client
compiles under the frozen V322 producer:
`.tmp/commons-numbers-1.3-complex-contracts-v322/summary.tsv`. All 3 source files
are present; zero translation/parse/unsupported/invalid/compile failures,
zero reachable no-op facade calls, zero stubbed constants. The runner rechecks
the original exact upstream pin and source manifests. Total run 180843 ms
(translation 153252, assembly 7447, check 1591). Independent Java/generated Rust
verification matches all 32 client guards (`contract=0`, `oracle=ok`):
`.tmp/numbers-complex-v322-contracts-behavior/{java-output,rust-output}.txt`
and `.tmp/numbers-complex-v322-contracts-behavior.log`.

```
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v322-debug" NEW_JAVA2RUST_BUILD_PROFILE=debug NUMBERS_CONTRACTS_STAGING_ROOT="$PWD/.tmp/commons-numbers-1.3-complex-contracts-v322-staged" NUMBERS_CONTRACTS_TRANSLATION_ROOT="$PWD/.tmp/commons-numbers-1.3-complex-contracts-v322" CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" bash scripts/run-commons-numbers-1.3-complex-contracts.sh
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/commons-numbers-1.3-complex-contracts-v322 --numbers-complex-contracts --output-dir .tmp/numbers-complex-v322-contracts-behavior
```

All owned jobs are terminal. Formatting and `git diff --check` pass. No upstream
production edits, exclusions, numeric algorithms, FP tolerance changes or
manual generated deliverable repairs were made. Earlier accepted-module/RNG
evidence retains its original V314/V317 labels; this is a fresh Complex/client
preservation check, not a fresh all-corpus claim or Complex acceptance.

### V323: UTF-16 length and native-reference callback/alias contracts

Continued the V322 consumer audit with actual generated String length paths.
The baseline emits `.len() as i32` for source Strings (UTF-8 byte count), but
StringBuilder/StringBuffer/CharSequence use `.chars().count()` (scalar count).
Both disagree with Java's UTF-16 unit count on normal multilingual text.
Minimal correct Rust obtains length from the same code-unit view already used
for search. Scalar inputs can count `encode_utf16` without allocating a new
payload; retained UTF-16 carriers use their existing length directly.

The whole-boundary fixture also exposed related existing cross-function gaps.
Frozen V322 generated `String::length` for an unbound source-SAM reference and
a one-argument closure for a zero-input bound reference. It moved builder
handles into CharSequence, reported observable-alias errors, and lacked the
identity String's builder-constructor input adapter. The initial frozen Rust
compile has five errors (E0599, E0593 and E0277), retained in
`.tmp/utf16-v323-length-baseline-compile.log`. Its original first fixture had
10 groups; the permanent fixture later expanded to 12 groups. The final-source
baseline is separately retained as `.tmp/utf16-v323-length-baseline-final.rs`,
not mislabeled as the earlier identical source. Independent javac/Java runs
return zero.

The smallest semantically correct rewrites are within the existing framework:

- Typed method-reference adapters invoke the ordinary selected method with
  the SAM input/result contract. Unbound references take their receiver from
  input zero; bound references capture once, null-check at creation, and keep
  the exact declared arity. Native and source SAMs must use the same input
  recovery rather than a native-only descriptor query. A borrowed length
  receiver keeps a bound callback reusable instead of accidentally making it
  FnOnce.
- Native callback invocation effects belong to the SAM/lambda ABI, not the
  creator. Native method-reference expansion is now shared by the resolver,
  effect graph and lowering. Its body goes through the existing callback-body
  registration/solver. Only original source AST pointers are registered as
  consumed callbacks; temporary synthetic lambda addresses are never retained
  as lexical identities. Source owners/overloads keep their existing resolver.
- A builder/buffer widened to CharSequence retains its existing allocation.
  Clone the native reference before the consuming `Into` conversion if the
  original is reused; cloning the converted value afterward is too late.
  This reuses ordinary slot conversion and the existing subtype/reuse proof.
  One EffectiveFacts authority identifies the three internally retained text
  facades for both lowering and alias analysis, excluding parsed source
  lookalikes and user hierarchy overrides. No extra Rc/Arc wrapper or copied
  mutable text payload is introduced.
- Identity String values get owned/borrowed constructor-input adapters in the
  existing combined String/platform bridge. This copies constructor content,
  unlike a reference-preserving interface widening; it does not create a
  supposedly lossless scalar projection on JavaUtf16String.

Production edits route StringLen and its facade ABI through
`java_string_length`/JavaTextView. Existing native sequence lengths count UTF-16
units. V322's code-point-to-needle conversion is also reused by the kernel's
`index_of_code_point`, removing its duplicate conversion algorithm.
No new statement-hoisting, ownership or callback-effect framework was needed
for this batch. This does not claim that all text payload/char storage is fixed.

Permanent StringLengthProbe has 12 independent Java/generated-Rust check groups:
identity retention, direct/formal calls, bound/unbound source-SAM references,
repeated callback calls, nullable calls, arrays, live builder and buffer aliases
before/after mutation, source length lookalikes, catchable null callback input,
and eager null failure for a bound reference. StringSearchScalarProbe adds
scalar-representation length checks. The real shared-runtime assembly test also
uses this same length consumer on isolated-surrogate payloads. New unit controls
cover native/static/bound/unbound reference selection, arity and source owner
precedence, plus native-reference metadata source/user override precedence.
Focused `.tmp/utf16-v323-focused-second.log` passes Java text contracts (2,
including the unchanged 4951 exact JDK observations) and all String contracts
(4). The subsequent library suite passes **1704** tests in 80.49 seconds.

Frozen functional producer `.tmp/new-java2rust-numbers-default-v323-debug`, SHA256
`91595c205158bf9f198596820b6615ea69076c0487052304d988ecd614f9d2b7`.
Fixture snapshot `.tmp/utf16-v323-length-source.java`, SHA256
`211591b7ed6385cb260a2f425508aa8445f3f87601e84a1d3919ffe1d3843372`.
Automatically emitted `.tmp/utf16-v323-unicode-probe.rs` now reports
`length=5`, `positions=4,2,1,4,4`. The remaining diagnostic output is still
`units=PANIC`, `wholeSlice=false`, `splitPair=PANIC`, `numericChar=0,0`, recorded
in `.tmp/utf16-v323-unicode-run.log`. The next required boundary remains actual
lossless String/char storage and its producers/append/parsing/exception consumers;
read-only length/search correctness cannot stand in for that migration.

Final V323 validation `.tmp/utf16-v323-final-batch.log` is green: 1704 library
tests, chained-reference assignments (1), floating-point parsing (2), Java text
contracts (2), native append (2), native SAM (3), String contracts (4), virtual
object Strings (3). The additional isolated-surrogate length assertions in real
shared-runtime assembly also pass the final focused rerun
`.tmp/utf16-v323-text-final.log` (2 tests). The frozen V322 final-fixture baseline
has seven compile errors; log `.tmp/utf16-v323-length-baseline-final-compile.log`.
Both baselines are diagnostic evidence, not manually repaired deliverables.

Fresh complete Complex/client artifact
`.tmp/commons-numbers-1.3-complex-contracts-v323-final` is produced by the exact
91595c binary above. All 3 scoped files compile, with zero translation/parse/
unsupported/invalid/compile failures, zero reachable no-op facade calls and zero
stubbed constants. The original upstream pin and complete manifests are
revalidated. Total 152839 ms (translation 123749, assembly 7577, check 1731).
Independent Java/generated-Rust client comparison matches both output lines,
all 32 guards: `contract=0`, `oracle=ok`, under
`.tmp/numbers-complex-v323-final-contracts-behavior` and its `.log`.

An initial corpus preflight attempted to execute the producer while the frozen
binary was still being copied and exited with `Text file busy`, before
translation. Its terminal failure is retained in
`.tmp/numbers-complex-v323-contracts-run.log`. After the copy closed, the normal
runner used a fresh `v323-final` staging/output root. No live job was restarted
or observation timeout treated as a terminal process.

Reproduce the substantial batch and fresh source preservation gate with:

```
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" cargo test --lib --test string_reference_views --test java_text_contracts --test native_sam_contracts --test virtual_object_strings --test native_append_contracts --test chained_reference_assignments --test float_parse_contracts
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v323-debug" NEW_JAVA2RUST_BUILD_PROFILE=debug NUMBERS_CONTRACTS_STAGING_ROOT="$PWD/.tmp/commons-numbers-1.3-complex-contracts-v323-final-staged" NUMBERS_CONTRACTS_TRANSLATION_ROOT="$PWD/.tmp/commons-numbers-1.3-complex-contracts-v323-final" CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" bash scripts/run-commons-numbers-1.3-complex-contracts.sh
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/commons-numbers-1.3-complex-contracts-v323-final --numbers-complex-contracts --output-dir .tmp/numbers-complex-v323-final-contracts-behavior
```

No upstream source edits, exclusions, algorithm substitutions, numeric policy
changes or acceptance relaxations were made. Earlier accepted-module/RNG
evidence retains its actual V314/V317 producer labels. Formatting and diff
checks pass. Goal remains active, **3/6 accepted**; this is not Complex acceptance.

### V324: begin the actual physical char cutover (String payload still pending)

This is a physical source representation change, not another detached runtime
utility. Every Java primitive char/boxed Character slot now uses the existing
transparent, Copy, unsigned-16-bit `JavaCodeUnit`, rather than Rust char.
Native Rust Unicode scalars remain a distinct type for internal codecs/regex
work; a source Java char no longer implicitly claims to be such a scalar.

Baseline and minimal rewrite audit:

- Frozen V323 automatically translates CodeUnitStorageProbe but emits
  `compile_error!` for retained surrogate literals, scalar char storage/defaults,
  and the old `char::from_u32(...).unwrap_or('\\0')` narrowing. Evidence:
  `.tmp/utf16-v324-char-baseline.rs` and `.log`. Independent Java returns zero.
- Minimal hypothetical Rust stores JavaCodeUnit directly, promotes through its
  unsigned numeric view and narrows through int before retaining low16 bits.
  The diagnostic `.tmp/utf16-v324-char-rewrite.rs` compiles/runs using the same
  existing kernel, including a surrogate switch arm. This framework can express
  the required source storage/signature/consumer contract without new ownership
  machinery. The tuple field is public because every u16 is valid and emitted
  cross-module match patterns/constant literals must construct the same type.
- Initial actual generated code exposed two remaining scalar assumptions:
  array defaults still constructed Vec<char>, and explicit/nested/boxed casts
  emitted primitive `as` on the nominal code-unit carrier. Both now use the
  canonical source-char default/numeric projections. Failed compile evidence:
  `.tmp/utf16-v324-code-units.log`; corrected executable evidence:
  `.tmp/utf16-v324-code-units-second.log`.

The coherent producer/storage batch changes char literals (including isolated
surrogates), defaults, formal/return/field/array slots, boxed runtime target
metadata, casts, promotion, compound updates, static constants and native return
type rendering. Representation and constant assembly now reuse the existing
primitive Rust-type mapper instead of maintaining separate char mappings.
Nested explicit char casts recover the numeric view instead of attempting Rust
casts on the nominal carrier. Integer/long narrowing retains low16 bits;
float/double first use Java's int conversion, not unsigned saturation.

String.charAt now uses the existing JavaTextView/JavaUtf16 indexing kernel and
returns JavaCodeUnit, including either half of a supplementary pair. Signed
Java indices are retained through the call plan; negative/out-of-range values
use the existing catchable StringIndexOutOfBoundsException protocol. The legacy
scalar `.chars().nth()` projection helpers are removed rather than retained as
a parallel producer. Source-only char units get the same core nominal runtime
even when no String/platform helper is selected.

Arrays' equality, hash and comparison protocols, natural ordering, map/tree key
support and erased primitive clone/equality also recognize the code-unit type.
Character/char array comparison preserves the unsigned unit difference where
required, not just normalized ordering signs. Boxed target rendering reuses the
same primitive type authority. No source/API name identifies the corpus.

Permanent CodeUnitStorageProbe has 15 independent Java/generated-Rust check
groups: surrogate literals/returns/parameters, unsigned promotion/comparison,
array/static defaults, wraparound updates, nested casts/narrowing, boxed and
erased recovery, array equality/hash/exact comparison, indexed supplementary
reads and caught negative/end bounds. A separate source-only executable test
checks carrier/dependency emission. Six existing unit failures were obsolete
Rust-scalar spellings/IR expectations, updated to the actual carrier and checked
helper contract, not relaxed Java behavior assertions. The initial library run
is retained in `.tmp/utf16-v324-batch.log` (1698 pass,6 snapshot failures).

Current substantial batch `.tmp/utf16-v324-final-batch.log` passes all 1704
library tests (78.41 seconds), code-unit contracts (2,12.80 seconds), Java text
contracts (2, including the unchanged 4951 JDK comparisons and real assembly),
and String reference/length/search contracts (4,26.36 seconds). Reproduce with:

```
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" cargo test --lib --test code_unit_contracts --test java_text_contracts --test string_reference_views
```

Frozen **migration** producer
`.tmp/new-java2rust-numbers-default-v324-codeunit-debug`, SHA256
`454de3e3778fc20377053f976465c50718b507551b4a732364a3a825827966ae`.
Automatically emitted `.tmp/utf16-v324-unicode-probe.rs` compiles and the original
Unicode diagnostic now reports:

```
length=5
units=41,e9,d83d,de00,5a
positions=4,2,1,4,4
wholeSlice=false
splitPair=PANIC
numericChar=d83d,d83d
```

Log `.tmp/utf16-v324-unicode-run.log`. This closes actual scalar char storage/
indexed-read corruption but not the original complete Unicode gate: substring
still produces scalar String payloads and cannot retain a split surrogate.

Important unfinished migration boundary: no JavaCodeUnit Display or implicit
scalar projection was added to make old text consumers compile. Char append/
format/String construction, native char-array/text/I/O facade signatures and
Character.toChars still need the coherent lossless String/consumer migration.
The latest producer is not asserted preservation-clean for those paths. Older
accepted module/RNG evidence and the green V323 Complex/client preservation
checkpoint retain their original producer hashes; they are not relabeled V324.
The full six-module objective remains active, **3/6 accepted checkpoints**.

Fresh complete-source migration audit, not an acceptance claim:
`.tmp/commons-numbers-1.3-complex-contracts-v324-char-baseline` includes both
untouched production files plus the client, with the original pin/manifests
revalidated. All 3 translate/parse, with zero unsupported/invalid constructs or
reachable no-op calls, but compilation now has **10 errors**: 7 missing
JavaCodeUnit-to-JavaAppendValue adapters and 3 attempts to use scalar Display/
ToString. All are the predicted char-to-text boundary. Logs:
`.tmp/numbers-complex-v324-char-baseline-run.log` and the artifact's
`compile.log`, `top-compile-errors.tsv`, `summary.tsv`. Run total 153403 ms.
Do not reuse V323's green client comparison as evidence that V324 compiles.

Representative generated paths are Complex::to_string / its shared companion,
the expected-format expression in parse_complex, and parsing_exception_msg's
checked append(Object) chain. Minimal correct rewrites cannot merely implement
Display for the unit: JavaAppendValue currently returns scalar Rust String,
which cannot represent an isolated unit. Its output and StringBuilder storage/
toString result must retain UTF-16, together with owned/borrowed String slots,
literal/native producers, exception messages and explicit scalar I/O boundaries.
Source/native ABI roles must remain distinguished; a blanket replacement of
every runtime Rust String would incorrectly change native transport/codecs too.

Diagnostic `.tmp/utf16-v324-char-text-rewrite.rs` compiles/runs against the
producer-exported `.tmp/utf16-v324-lossless-runtime.rs`. It rewrites the actual
message-builder shape using the existing UTF-16 object's append/substring
operations, not a replacement numerical algorithm. It preserves an isolated
surrogate in the result, invokes the checked Object display callback once,
propagates its failure, renders null message/error/input correctly, and preserves
then recombines split pair substrings. Evidence
`.tmp/utf16-v324-char-text-rewrite-{compile,run}.log`. This is feasibility
evidence only, never a manually translated deliverable or an alternate corpus
acceptance harness. The existing storage/callback/coercion machinery can express
the corrected ABI; the outstanding work is a coherent text producer/consumer
cutover, not an ownership-framework blocker. No artificial scalar fallback,
new panic/default body, FP tolerance change or upstream edit was used.

The final focused `.tmp/utf16-v324-text-final.log` also passes both Java text
tests (2.11 seconds), including a compile-rejection guard against silently adding
Display to JavaCodeUnit to satisfy the new Complex errors. The full current
CodeUnitStorageProbe source is frozen at `.tmp/utf16-v324-char-source.java`;
SHA256 `7fb96d135ab71d019fd6303f4c5522221bfe1f08d75abf5327c7c6d1dc8e039b`.
the earlier frozen V323 baseline remains labeled as the initial fixture, not
the later expanded one. All owned jobs are terminal. Formatting and
`git diff --check` pass. The 10 Complex compile errors remain open migration
work, not a blocked goal, a passing preservation gate, or grounds for narrowing
the original six-module objective.

### V325: physical String/append cutover, integration in progress

This is an unfinished representation migration, **not an accepted checkpoint**.
The original six-module scope, upstream pin, production manifests and dependency
closure remain unchanged; acceptance is still 3/6. V324's green library/focused
checks and V323's compiling Complex/client are evidence for their respective
frozen producers only, not for this worktree.

Whole-path continuation of the V324 rewrite audit: select the existing
JavaUtf16String payload as JavaString program-wide during semantic analysis,
not only when an identity comparison happens to appear. Source String literals
(including isolated surrogates), nullable/erased literal slots and empty-value
factories now select that same carrier and interner. Low-level explicitly
native scalar fixtures retain their separate FactSet default. No Display,
AsRef<str> or implicit scalar projection was added to JavaString/JavaCodeUnit.

JavaAppendValue and builder constructor inputs now return JavaUtf16. Both
StringBuilder and StringBuffer physically store Arc<Mutex<JavaUtf16>>, retain
their existing allocation aliases and produce immutable JavaString snapshots.
Unit mutation, truncation/zero growth, insertion, deletion and replacement use
the shared UTF-16 kernel; substring delegates to the lossless String object.
The JavaTextView contract accepts native scalar inputs, raw JavaCodeUnit slices
and vectors, and recursively borrowed views without making arbitrary Java text
satisfy Rust scalar-string bounds. The assembly owner classifier also recognizes
slice/array and tuple implementation targets, which need no nominal declaration.

The first source gates revealed the predicted scalar consumers, rather than an
ownership/framework impossibility. `.tmp/utf16-v325-latest.log` records 2/2 kernel
tests passing with **5049 exact JDK observations** in both host and emitted code
(the original 4951 plus 98 production-kernel builder mutation observations),
including real shared-runtime assembly and implicit-scalar compile rejection.
At that prefix all four source String gates fail to compile. After the literal
slot fix, `.tmp/utf16-v325-checkpoint.log` records the new LosslessStringProbe
failing with 31 generated compile errors; the independent Java run succeeds.
Those failures are not ignored or converted into passing acceptance results.

The next coherent batch carries logical Object/stringification results through
the erased result protocol, collection/Optional display and array/object display
traits using JavaLogicalString (the already-selected carrier, not a second
payload implementation). JavaString identity is retained for direct erased
toString, null results become Java text "null", and checked exceptions still
propagate. Collection/map formatting joins raw payloads; source toString
adapters reuse the shared checked result protocol. Native builder Object
downcasts call lossless snapshots rather than Rust Display. Rust formatter
output has an explicit adapter that replaces invalid units at that host output
boundary only; it is not a Java concatenation/path/writer conversion. The
source-only fixture includes regression assertions for surrogate payloads,
identity, nullable results, collection/Optional/deep-array text, checked failures
and map callback order. An intermediate prefix log
`.tmp/utf16-v325-erased-prefix.log` reduced the source-only compile boundary to
two remaining array/object-display signature errors, addressed by this batch.

Verification for the logical-text prefix is terminal in
`.tmp/utf16-v325-logical-text-current.log`: both kernel/assembly tests pass
(2.22 seconds), the source-only test including the new executable runtime
assertions passes, and the other three String source tests fail compilation
(15.18 seconds for all four). LosslessStringProbe still fails compilation with
27 errors (13.84 seconds); its Java self-check still passes. No generated
deliverable was modified. The source-only gate was previously failing at the
shared scalar protocol and is now executable, but this is not whole-platform
preservation.

The batch library check `.tmp/utf16-v325-library-wip.log` finishes with
**1685 passed / 19 failed** in 77.57 seconds. Failures include scalar-String
shape assertions and genuine generated compilation failures in checked hash,
applied SAM and native Iterable regressions; do not report this suite as green.
The native Iterable failure additionally exposed logical-text dependencies
inside its intentionally standalone cursor-support scope. A follow-up moves
the cursor-to-text/map-to-text consumers into the existing module-level display
adapter support, keeping cursor mechanics independent of Object display. Its
focused rerun is `.tmp/utf16-v325-iterator-text-scope.log`: it remains failing
with 18 common platform text-consumer compilation errors, but no missing
JavaLogicalString/java_erased_text_join scope errors remain. No full-suite
result is inferred from that follow-up. All owned verification jobs are
terminal; formatting and `git diff --check` pass.

The logical-text prefix producer is frozen at
`.tmp/new-java2rust-numbers-default-v325-logical-text-wip-debug`, SHA256
`bb4d8cf39a8cc6083ab591f04fa67d0672ea2074f904d73091e0d2cc01391dc3`.
Build log `.tmp/utf16-v325-producer-build.log`; this freeze precedes the small
cursor/display-support relocation. Reproduction:

```sh
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" \
  cargo test --no-fail-fast --test string_reference_views \
    --test java_text_contracts --test lossless_string_contracts
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true TMPDIR="$PWD/.tmp/test-tmp" \
  cargo test --lib
```

Full Complex
and preservation reruns are deliberately deferred while common generated
runtime contracts do not compile. Remaining known families include live
CharSequence views, native output/Writer and path adapters, source concatenation
and formatting, Map.Entry result/format contracts, default source/trait
toString results, scalar assumptions in equality/parsing helpers,
exception-message storage and native char-array APIs. Fix these through authoritative shared
mechanisms; do not restore a scalar fallback or repair generated deliverables.
Floating-point stress diagnostics remain distinct from real-workload acceptance;
this migration changes no numerical tolerance or upstream algorithm.

### V326: close platform text storage/transport and source concatenation contracts

Continuation of V325's whole-path audit, not a new corpus or a narrowed goal.
The generated failures traced CharSequence's scalar storage to live builder
snapshots, JavaAppendValue's new payload to scalar Writer/PrintStream consumers,
Map.Entry/trait default stringification to old scalar result signatures, and
String.equals/concat to scalar formatting. These are shared storage/signature/
consumer changes; existing shared allocations, UTF-16 payloads, checked object
display and concat analysis can express the needed rewrites. No ownership
framework redesign, upstream source edit or generated-deliverable repair is
required. All six modules, the original pin/manifests and preservation scope
remain unchanged; still 3/6 accepted checkpoints.

Implemented as coherent text batches:

- CharSequence retains JavaString for immutable inputs and the existing live
  builder/buffer handles for mutable inputs. Its text view snapshots live units;
  length, charAt and subsequence use the same UTF-16 core. String constructors
  borrow their input instead of consuming a live Java reference.
- StringWriter stores shared JavaUtf16, exposes immutable JavaString snapshots
  and keeps Java's write-after-close behavior. Writer/BufferedWriter and char
  range methods consume JavaTextView, including source JavaCodeUnit arrays.
- Byte sinks use JavaUtf8Encoder, a separate stateful transport leaf. A high
  surrogate survives writes and flushes, can combine with a later low surrogate,
  and becomes Java's '?' replacement at end of input or for malformed input.
  OutputStreamWriter clones share encoder state; PrintStream delegates its text
  to the same writer model, with persistent standard-stream encoders. This is
  UTF-8 support, not a claim that the preexisting charset selector now models
  all other encodings.
- Rust Display bridges are explicit host-output consumers, never logical Java
  text conversion. Native Map.Entry text joins raw payloads and uses the shared
  checked result contract. Source trait default toString has the selected
  JavaLogicalString result, including its native ASCII fallback conversion.
- Source String value equality projects raw units for owned and borrowed
  non-null operands. Dynamic concat uses the existing Java concat analyzer,
  evaluates/converts segments in sequence, and appends UTF-16 payloads into a
  fresh String. Literal-only constant expressions reuse the existing constant
  folder and interner, including surrogate and brace literals. No format-string
  parsing or generated-Rust rewrite is used. Named constant-variable scope is
  not newly claimed by this literal-expression fold.
- Primitive/String append values now have one authoritative source,
  `src/java_append_value.rs`, included by both native builders and source-only
  concat. Object append uses the same lossless text-segment conversion while
  retaining its staged receiver/argument evaluation and checked callback path.

The native path boundary has an explicit checked UTF-16-to-scalar conversion:
malformed units and NUL are rejected rather than replaced. The old infallible
path factories still use uncaught-exception wrappers; checked source dispatch
and exact path-bearing exception messages are not asserted validated by this
batch. The runtime's logical Java String never implements Display/AsRef<str>.

Evidence prefixes: `.tmp/utf16-v326-platform-prefix.log` reduces the end-to-end
fixture to 11 compile errors, then `.tmp/utf16-v326-platform-batch.log` reduces it
to one concat/Display error and passes String search. The new independent
TextTransportOracle compares **154 exact observations** covering immutable and
live sequence views, writer snapshots/aliases, UTF-8 writes split across calls,
flush, close, post-close IOException and malformed input. It passes in that log
(13.54 seconds), as do the 5049-observation kernel/shared-assembly tests.

The subsequent source-concat prefix
`.tmp/utf16-v326-logical-source-batch.log` exposes a real new-String input move
and source-only support closure, both addressed in the final batch. It also
retains failing ObjectAppendProbe/SourceAppendProbe evidence for the unfinished
exception-message ABI and display/nullable consumers; those checks are not
ignored or weakened. The new source-only
concat fixture has a separate Java oracle main and explicitly asserts no
StringBuilder facade is emitted, so it cannot accidentally pass by depending on
the full platform runtime.

Final focused `.tmp/utf16-v326-text-current.log` is terminal:
kernel/assembly 2/2 pass (5049 observations, 2.37 seconds); lossless source
String and source-only concat 2/2 pass (16.95 seconds); String-reference tests
3/4 pass (28.49 seconds), including length, search and source-only identity.
The remaining fixture has exactly three compilation errors: the float parser's
AsRef<str> input and two nullable-equality scalar projections. The independent
154-observation transport gate passes again (15.36 seconds). Expanded
LosslessStringProbe now covers 12 zero-result groups, including char/nullable
concat, literal constant interning, arithmetic grouping and fresh dynamic
concat identity; both original Java and generated Rust run those checks.

The full library suite `.tmp/utf16-v326-library-wip.log` is **1686 passed /
18 failed** (90.47 seconds), not green. The previously failing executable
checked-hash, applied-SAM and standalone-Iterable runtime tests now pass. The
remaining failures are retained for ABI/shape assertion audit, with examples
expecting Rust String instead of JavaString or the former char-only writer
signature. No assertions were removed or relaxed to obtain these results.

Frozen producer `.tmp/new-java2rust-numbers-default-v326-platform-text-wip-debug`,
SHA256 `cee20725a0827b86b4caca5d65f715b7e2ee6fec341e62d5343fff935b3232fa`;
build log `.tmp/utf16-v326-producer-build.log`. Complete pinned Complex/client
rerun `.tmp/commons-numbers-1.3-complex-contracts-v326-platform-text` revalidates
the exact upstream commit and both untouched production files plus the one
client. All three translate and parse, with zero unsupported IR constructs,
invalid patterns, stubbed constants or reachable no-op calls. Compilation
fails with **84 errors**. This is explicitly not an accepted module or a
preservation-clean producer. Log
`.tmp/numbers-complex-v326-platform-text-run.log`, plus artifact `summary.tsv`,
`compile.log`, `top-compile-errors.tsv`, and `translation-producer.tsv`.
Total 156045 ms: translation 128679 ms, assembly 7405 ms, check 1599 ms. The
signature fixpoint is 460 ms; emission dominates at 89567 ms. Do not attribute
this run's latency to signature fixpoint iteration.

Fresh assembly/consumer audit changes the next batch ordering:

1. **Shared assembly contract loss precedes the remaining parser work.** The
   per-module source gates pass, but the shell assembly stage injects handwritten
   JavaErasedCollectionDisplayElement/JavaErasedOptionalDisplayElement traits
   returning scalar String (`scripts/translate-focused-package.sh`, erased
   display prelude). These contradict the producer's lossless trait results.
   Separately, trait implementations targeting the JavaString alias disappear
   from shared output: 57 missing From<String>, one missing From<&str>, missing
   String collection/Optional implementations and two incompatible CodeUnit
   implementations are downstream symptoms. The generated root aliases
   JavaString to JavaUtf16String but lacks those implementations. Fix promotion
   through canonical alias ownership and reuse the producer's authoritative
   display contract; do not add another handwritten String special case or
   patch this generated root. A small multi-module executable gate should
   exercise the real shell assembly route before another full corpus run.
2. **Source message/parsing ABI:** Complex's two zeroth-root throws and seven
   parsing_exception_msg consumers now correctly produce JavaString, while
   JavaException still stores Option<Rust String>. The minimal rewrite changes
   constructor/storage/getMessage/logical toString together; merely formatting
   the message would discard units. Reuse the existing scalar numeric parser
   behind a checked text-input bridge, preserving its grammar/rounding kernel
   and original lossless error text rather than adding AsRef<str> to String.
3. **Remaining source/native leaves:** the client calls String.trim; source
   Display currently passes JavaString to f.write_str, and native println still
   selects a Rust format path. Route actual output through the existing explicit
   transport boundary, not an implicit Display implementation. Nullable equality
   must select unit/carrier views consistently. All are local or cross-function
   ABI consumers of the already-selected representation, not evidence that the
   numerical algorithm requires a manual Rust rewrite.

Reproduction (same offline Cargo/TMPDIR settings as V325):

```sh
cargo test --no-fail-fast --test java_text_contracts --test lossless_string_contracts \
  --test string_reference_views --test text_transport_contracts
cargo test --lib
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v326-platform-text-wip-debug" \
  NEW_JAVA2RUST_BUILD_PROFILE=debug \
  NUMBERS_CONTRACTS_STAGING_ROOT="$PWD/.tmp/NEW-v326-staged" \
  NUMBERS_CONTRACTS_TRANSLATION_ROOT="$PWD/.tmp/NEW-v326-complex" \
  bash scripts/run-commons-numbers-1.3-complex-contracts.sh
```

All owned jobs are terminal. Formatting and `git diff --check` pass. No FP
tolerance, scope exclusion or prior producer's acceptance label changed.

### V327: producer-owned logical text survives whole-package assembly

V326's 84-error complete-source result is the baseline, not 84 independent
algorithm bugs. The assembly audit demonstrated two cross-module losses:
private trait declarations were absent from the promotion catalog, and trait
implementations whose only owner was a type alias were stripped without being
attached to the canonical declaration. The package shell additionally injected
obsolete scalar erased-text traits/implementations, contradicting the producer.
The minimal valid rewrite retains the producer's declarations/impls together;
it does not introduce a new representation or manually repair generated Rust.

General fixes reuse the existing merger's owner graph. Trait and inherent
implementations now follow the same cycle-safe alias chain; private support
traits participate in the same discovery/closure as private structs/enums and
are widened only to pub(crate) on promotion. Existing generic aliases keep
their applied arguments and impl bodies. The shell's handwritten collection/
Optional text traits, primitive macro and special-case stripping are removed.
Their authoritative producer bodies, macro invocations and implementations
are promoted normally. Unrelated ClassLoader seeding is not redesigned here.

The permanent alias/private-trait executable regression fails before the fix:
`.tmp/utf16-v327-alias-producer-baseline.log` contains a root with aliases but
neither the private trait nor its From implementation. After the batch,
`.tmp/utf16-v327-assembly-unit-current.log` passes all **12** alias and runtime
registry controls in 1.559 seconds, including actual rustc execution and a
source guard against restoring handwritten scalar text contracts in the shell.
The initial import-only invocation without PYTHONPATH is not product evidence;
reproduction uses `PYTHONPATH=scripts`.

The existing verified raw-output resume mechanism avoids retranslation while
isolating the assembly fix. A copied V326 artifact at
`.tmp/commons-numbers-1.3-complex-contracts-v327-assembly-from-v326` reassembles
with its original producer hash and intact raw modules; **84 errors become 17**.
Log `.tmp/numbers-complex-v327-assembly-resume.log`. The original V326 artifact
is preserved. No generated algorithm/body was edited, and this control is not
relabeled as output from a newer producer.

The first small real-shell fixture (`.tmp/utf16-v327-shell-assembly`) then
exposes two errors in a retained collection-display callback: its explicit
return annotation still says Rust String. The producer fix changes the whole
callback boundary to JavaLogicalString and normalizes native scalar leaves
through its existing Into contract. The same batch closes the remaining
audited trim/output leaves:

- String.trim delegates to a shared UTF-16 trim range (units <= U+0020), retains
  unchanged receiver identity and preserves malformed units. CharSequence reuses
  that operation instead of maintaining another trim algorithm. Inherent Java
  String.toString returns the same object; neither operation is rewrapped as a
  fresh allocation by native call lowering.
- Source Rust Display implementations delegate all four checked/nullable
  String-result contracts through the existing result protocol and explicit
  native formatter adapter, not f.write_str on JavaString.
- System print/println selects lossless text conversion followed by a core
  UTF-8 transport operation. It shares the same standard-stream encoder state
  as explicit PrintStream aliases, without requiring the full platform facade
  merely to print. The char[] overload keeps content/null-receiver semantics
  distinct from concatenation's Object array spelling. No implicit String
  Display/AsRef<str> or duplicate encoder implementation is added.

Final focused `.tmp/utf16-v327-logical-leaves.log`: kernel/shared assembly 2/2
pass with **5077 exact JDK observations** (5049 preserved plus 28 trim value/
identity observations), 2.43 seconds; source String/concat 2/2 pass, 16.09
seconds; String reference gates 3/4 pass, 28.10 seconds; transport's 154 exact
observations pass, 13.01 seconds. The one failing reference fixture retains the
known float-parser AsRef<str> error and two nullable-equality scalar projections.
The source-only concat probe additionally checks Unicode non-breaking spaces,
raw-surrogate trim, and String.toString identity against Java.

The new permanent real-package gate is
`scripts/test-lossless-text-assembly.sh` with two complete Java fixture modules
under `tests/corpus/numbers/text-assembly`. It invokes the actual shell runner,
not only the Python merger. `.tmp/utf16-v327-shell-assembly-final` translates,
assembles and compiles both modules, passes its independent Java zero-contract
and Rust executable test, and runs the generated main with byte-identical
stdout `contract=0`. It exercises inter-module String identity/copy, malformed
UTF-16 collection and append(Object) text, alias-owned From<String>/From<&str>
implementations and dynamic String output. Only test callers are copied into
the generated crate; production files/root are never repaired. Runner total
30552 ms, Rust test build 5.78 seconds, executable test 1/1 pass.

Producer `.tmp/new-java2rust-numbers-default-v327-text-assembly-debug`, SHA256
`f01904e23d9a0eb36dd1e32090bced6da031771e552dfb06ef437af163fe2c0b`;
build log `.tmp/utf16-v327-producer-build.log`. Assembly source snapshots are
retained at `.tmp/utf16-v327-frozen-assembly-scripts`, with SHA256:

- translate-focused-package.sh:
  `bf21081863b859e3891cd74200433a4ed771475d7afa3f1be68ca92a35188b49`
- merge-shared-support-facades.py:
  `6baf97ef59589ccd7962b3ed5d462d3e67fe4ac91878714fc9c3463fe71aded1`
- rust_item_utils.py (reused unchanged this turn):
  `0a787271763e8d71f09b9cd85463176246dfddcb3b4c408afb6adfb80c8a139f`

Fresh full pinned Complex/client
`.tmp/commons-numbers-1.3-complex-contracts-v327-final` revalidates both untouched
production files, the exact upstream pin and the one separate client. All three
translate/parse with zero unsupported IR, invalid patterns, stubbed constants
and reachable no-op calls. It still fails compilation with **13 errors**:
**11 exception-message ABI mismatches** (nullable getMessage and source
constructors/parsing_exception_msg returning JavaString into scalar storage)
and **2 numeric-parser input AsRef<str> requirements**. No remaining alias
implementation, scalar erased-trait, trim, source Display or stdout-format
errors occur in this compile result. Log `.tmp/numbers-complex-v327-final-run.log`
and artifact `compile.log`, `top-compile-errors.tsv`, `summary.tsv`.
Total 197250 ms; resolver 38645, translation 142826, assembly 9100 and check
1665 ms. This overlaps the library check, so it is not an isolated performance
comparison against V326.

Full library `.tmp/utf16-v327-library.log` remains red: **1685 passed / 19
failed**, 91.89 seconds. The added failure asserts the previous direct
f.write_str rendering instead of the shared String-result/formatter protocol;
the other ABI/shape assertion failures still require explicit audit/update.
None are ignored, deleted or relaxed. This is not acceptance or preservation
cleanliness; earlier accepted-module/RNG results keep their original producers.
The six-module scope is unchanged, still **3/6 accepted checkpoints**.

Next coherent batch: change JavaException constructor/storage/getMessage and
logical toString contracts together, retaining raw units and nullable/identity
behavior; bridge source text to the existing scalar numeric parser without
forking its grammar/rounding or projecting error messages lossily. Then close
nullable equality through the same carrier/unit policy, audit the intentional
ABI assertions and rerun the known checked append/exception regressions. These
are the already-audited whole-path contracts, not requests to add more corpuses.
The parser's host/literal API must remain usable without emitted text types.

Reproduction uses the same offline Cargo/TMPDIR settings as V325/V326:

```sh
PYTHONPATH=scripts python3 -m unittest scripts/test_rust_item_aliases.py \
  scripts/test_shared_runtime_registries.py
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v327-text-assembly-debug" \
  NEW_JAVA2RUST_BUILD_PROFILE=debug \
  bash scripts/test-lossless-text-assembly.sh "$PWD/.tmp/NEW-v327-text-assembly"
cargo test --no-fail-fast --test java_text_contracts --test lossless_string_contracts \
  --test string_reference_views --test text_transport_contracts
cargo test --lib
# Complete Complex/client: use its standard contracts runner with the frozen
# V327 producer and fresh NUMBERS_CONTRACTS_STAGING_ROOT/TRANSLATION_ROOT.
```

All owned jobs are terminal; formatting, shell syntax checks and
`git diff --check` pass. No source exclusions or FP tolerance changes were made.

### V328: lossless exception messages and source numeric parsing

The complete V327 baseline has 13 compile errors in two already-audited
contracts: JavaString exception messages and scalar-only numeric parser inputs.
The required rewrite carries nullable logical text through construction,
storage, getters and logical exception formatting; native diagnostics format
only at their explicit output adapter. Source Float/Double parsing must reuse
the existing scalar grammar/rounding kernel without replacing malformed UTF-16
in observable error messages. These are cross-function runtime contracts, not
changes to Complex algorithms or numerical tolerances.

`src/java_exception.rs` is now the shared exception-state implementation. The
producer selects JavaString or native Rust String through JavaExceptionText;
message getters clone the reference carrier rather than round-trip text.
Reader deferred errors retain the actual exception instead of formatting it
into a new scalar-message IOException. Slot-directed platform Throwable
construction now reuses the ordinary constructor's class/source-owner decision
and message/cause lowering. Modeled Throwable return types survive the opaque
facade filter, so nullable chained getCause dereferences use the existing Java
null-receiver conversion. addSuppressed supplies mutation metadata, and owned
unretained array-valued calls reuse the existing move-out checked indexing
path rather than returning a loan from a temporary Vec. This does not claim
Throwable object-reference identity or a complete native Reader redesign.

`src/java_float_text.rs` bridges JavaTextView input to the existing numeric
kernel. After Java trim, only ASCII can be a numeric token; non-ASCII units
use a grammar sentinel while original units remain authoritative for generic
input-error text. Float/Double parse, boxed valueOf, String constructors and
method references share the context-aware lowering path. Host/literal parsing
keeps its scalar API and unchanged rounding algorithm. Nullable String equals
compares units at the same required-receiver boundary; nullable object display
uses a carrier-inferred null fallback. No numerical tolerance changed.

Permanent source/JDK regressions cover raw message units, message reference
identity, absent versus empty messages, causes, suppressed exceptions and
throw/catch; parser coverage retains all 972 existing exact requests and adds
36 raw-UTF16 requests with exact error-message units. These malformed text
cases exercise the parser boundary used by Complex, not hypothetical FP
arithmetic stress. V328's intermediate run
`.tmp/utf16-v328-boundaries-final.log` passes both parser tests, both lossless
String tests, both native append tests and all four String-reference tests;
the exception test then exposed the mutation/temporary-array issues addressed
above. Final `.tmp/utf16-v328-exception-array-final.log` passes the expanded
exception probe (1 test, 15.29 seconds), both text-kernel checks (2.35 seconds,
including the existing 5077-observation JDK matrix), and the transport probe
(1 test, 12.99 seconds). No malformed message is scalar-formatted to pass.

Full library `.tmp/utf16-v328-library-final.log`: **1704 passed, 0 failed**,
83.61 seconds. The preceding full run has 1697/7: five intentional runtime/
carrier assertion changes and two executable Rust harnesses with redundant
ambiguous `.into()` constructor arguments. The final pass preserves their
structural assertions and executable checks; explicit type-directed null
fallbacks remain checked as structured Rust IR, not just text snapshots.

Frozen producer `.tmp/new-java2rust-numbers-default-v328-exception-text-debug`,
SHA256 `8ed3a460c0093b53e166b8fa467f3de17790b75d5e56307c86afde7134695177`.
Build `.tmp/utf16-v328-producer-final2-build.log`; assembly scripts retain
exactly the V327 hashes above. The producer was frozen before the final
test-only assertion/harness edits; no product behavior changed afterward.

Fresh complete `.tmp/commons-numbers-1.3-complex-contracts-v328-final`, staging
`.tmp/numbers-complex-v328-text-staged`, revalidates pin
`ae8abefb51c2c4d1a05882d61cefc8015e493a8f`, both untouched production files
and the separate client, retaining production/contract/batch manifests.
All three translate/parse/compile: **0 compile errors**, 0 unsupported IR,
invalid patterns, stubbed constants or reachable no-op calls. Inventory-only
unmodelled/opaque facade signals are not relabeled zero. Total 186047 ms,
resolver 14024, translation 159454, assembly 7676, check 1757. This overlapped
other verification and is not an isolated speed comparison.

Independent client `.tmp/numbers-complex-v328-final-contracts-behavior`
matches both Java output lines and all 32 guards: `contract=0`, `oracle=ok`.
Broad operation oracle `.tmp/numbers-complex-v328-operation-final-behavior`
executes all **14617** observations and remains strictly red with **78 exact
differences**. A direct diff against V314's recorded differences proves the
only change is removal of the three Unicode parsing failures (emoji, accented
letter, fullwidth digit). All 26 parsing cases now match, including exception
class/message; the 78 preexisting FP diagnostic differences are unchanged:
65 elementary-result last-bit differences, 10 arithmetic NaN-sign cases,
2 division NaN-payload cases and 1 artificial huge-power amplification case.
No tolerance or NaN allowance is enabled. Per the practical-FP rule, these
diagnostics do not themselves justify a new corner-case implementation campaign.
The caller was updated for the actual nullable/Result/retained-list/String ABI;
it still calls generated operations and compares the same ordered observations.
Its earlier terminal driver-build failure is retained separately at
`.tmp/numbers-complex-v328-operation-behavior`.

The real two-file shell assembly regression
`.tmp/utf16-v328-shell-assembly` also passes: runner compilation, its executable
Rust contract and byte-identical Java/generated stdout `contract=0`.

Reproduction (offline Cargo and TMPDIR settings as above):

```sh
cargo test --lib
cargo test --no-fail-fast --test exception_text_contracts --test float_parse_contracts \
  --test native_append_contracts --test string_reference_views \
  --test lossless_string_contracts --test java_text_contracts --test text_transport_contracts
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v328-exception-text-debug" \
  NEW_JAVA2RUST_BUILD_PROFILE=debug \
  NUMBERS_CONTRACTS_STAGING_ROOT="$PWD/.tmp/NEW-v328-staged" \
  NUMBERS_CONTRACTS_TRANSLATION_ROOT="$PWD/.tmp/NEW-v328-output" \
  bash scripts/run-commons-numbers-1.3-complex-contracts.sh
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/NEW-v328-output \
  --numbers-complex-contracts --output-dir .tmp/NEW-v328-client-behavior
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/NEW-v328-output \
  --numbers-complex --output-dir .tmp/NEW-v328-operation-behavior
```

Old library assertions are migrated only where the intentional carrier/literal
or explicit native-formatting ABI changed; control-flow, mutation, nullable
and generic-forwarding assertions remain. No tests are ignored or removed.
Earlier accepted-module/RNG preservation evidence retains its original
producers. All six modules remain in scope; this is not module acceptance.

Additional executable preservation sweep
`.tmp/utf16-v328-exception-api-preservation.log` is **19 passed / 13 failed**:
BigInteger 5/2, comparator 1/0, Java hashing 1/5, bulk membership 1/0,
RootFinder contracts 11/6. This is stronger evidence than the green library
alone and prevents calling this producer preservation-clean. The sweep is
terminal, not a timeout. These are newly measured current-state failures;
their historical introduction is not inferred from the V328 label.

Next whole-path audit/batch, based on emitted code rather than counts:

- A mapped `java_math_runtime::to_string(...) -> Rust String` is assigned to
  `let first: JavaString` without conversion. The minimum rewrite converts
  once at the foreign-result boundary; distinguish the mapped physical ABI
  from its Java String semantic type through producer, return-shape planning,
  assignment/return consumers and nullable/Result wrappers. Do not wrap every
  source String or add a scalar AsRef implementation. The retained diagnostic
  `.tmp/utf16-v328-mapped-liveness-diagnostic.log` reproduces this error without
  changing the generated crate. Native floating formatting has the same
  scalar-result/local-signature gap in RootFinder's display regression.
- JavaString lacks the JavaHashCode consumer contract despite having the
  UTF-16 hashing leaf and map-key equality. Reuse the same unit hash kernel
  for direct, nullable, erased and collection dispatch. The wrapper-factory
  probe also lowers case-insensitive text comparison to scalar
  eq_ignore_ascii_case; retain Java's text semantics through an explicit
  unit-aware operation, not an infallible scalar-view escape hatch.
- Source-erased object formatting still has native String return leaves in
  a JavaString-returning function for some source types. The existing generic
  source formatter must normalize logical text on every branch, including
  fallback/class metadata branches. Separately, typed Map.Entry display still
  uses native format on logical String fields; its current text-only unit
  assertions are not proof that generated Rust compiles. Add an executable
  raw-unit/mutation regression before replacing that assembly with the shared
  logical join/append contract.
- String.format retains AsRef<str> templates and Result<String> argument
  callbacks. Its contract must move together: template parser, literal spans,
  argument rendering, callback ABI and output carrier, while keeping argument
  evaluation/reentrancy and unused-element checks. This is cross-function
  runtime work, not a call-site cast. One reentrant formatting probe also
  exposes non-Send erased Any in synchronized static storage; audit that
  storage/callback contract independently rather than claiming a String
  conversion resolves it.
- Some Rust-only callers still expect Result<String> or compare exception
  message references directly with str. Adapt those callers explicitly to
  the selected carrier while preserving their exact expected observations;
  do not supply lossy Display/AsRef solely to compile tests. The extended
  BigInteger oracle itself compiles production and then fails this caller ABI.

These are local leaf/metadata and cross-function contract changes within the
approved text migration. No new corpus, upstream rewrite or numerical
compatibility campaign is justified by this sweep. Close these executable
preservation gaps and the outstanding reachability/requirement audit before
accepting Complex or advancing to Quaternion. The goal remains active,
**3/6 accepted checkpoints**. All owned V328 jobs are terminal; diff checks
pass, and the frozen producer and failed evidence remain intact.

### V329: shared hash/text consumers and mapped result boundaries

The V328 preservation baseline is 19/13, not the green library alone. This
batch follows the emitted-code audit above and leaves the formatting/storage
contracts as a separate cross-function batch. No FP arithmetic algorithm,
comparison policy or upstream production source changes.

General producer/runtime fixes:

- JavaString implements JavaHashCode by delegating to its existing UTF-16
  unit hash. Native String hash entry points also use that payload kernel;
  nullable, erased and collection consumers retain their checked protocols.
- Boolean.parseBoolean's source intrinsic recognizes its ASCII keyword from
  JavaTextView units. It no longer asks JavaString for a scalar
  eq_ignore_ascii_case method. This is not a substitute implementation of the
  separate, general Unicode String.equalsIgnoreCase operation.
- Native Float/Double.toString results enter the source String carrier at the
  intrinsic boundary, reusing the unchanged numerical formatting kernel.
- Foreign mapped receiver methods whose Java result is String normalize the
  native text value once, preserving Option, Result and Result<Option> layers.
  Selection uses the original Java method key, not a renamed Rust function,
  for checked-result metadata. Explicit custom Rust-return contracts remain
  under their existing adapters; source methods are not fresh-wrapped. The
  normalizer moves the Rust expression rather than cloning every call AST.
- Erased collection downcasts call their existing logical deep-display
  protocol instead of Rust Display/to_string. This fixes the observed
  JavaLinkedHashSet return mismatch without scalar round-tripping its text.

Focused `.tmp/utf16-v329-consumer-prefix.log`: BigInteger **7/7**, 120.28
seconds, Java hashing **6/6**, 16.29 seconds, lossless String **2/2**, 14.60
seconds. The String probe now includes raw-surrogate direct/nullable hash
consumers and Boolean parsing of null/mixed-case/malformed text; its Java
execution remains the independent source oracle. Rust-only numeric callers
now project scalar output explicitly and retain the exact expected strings.

The renamed mapped-method unit matrix
`.tmp/utf16-v329-mapped-contract-matrix.log` passes all four wrapper shapes,
including a negative source-method control. The new executable regression
`tests/mapped_text_contracts.rs` then discovered a real signature gap that
text/IR assertions missed: nullable foreign calls were correctly normalized
to Option<JavaString>, but enclosing source methods were inferred non-null
and emitted a false expect("Java null receiver"). The first executable
failure is retained at `.tmp/utf16-v329-mapped-executable.log`; this is product
evidence, not a bad test. A later intermediate host type-check failure in
`.tmp/utf16-v329-mapped-executable-current.log` is separately retained.

The return-signature visitor now consults the authoritative external effect
using the existing signature-evidence policy before falling back to the builtin
stdlib nullability table. Resolved and unresolved receiver paths share this
lookup, and parsed source ownership still takes precedence. The executable
fixture supplies a separate foreign Rust runtime module, translates Java
wrappers without editing them, and verifies non-BMP units, null results and
checked exceptions across all four physical result shapes.

The RootFinder prefix `.tmp/utf16-v329-rootfinder-prefix.log` is **13/17**,
up from 11/17. Its four remaining failures are formatting-array expansion,
source exception construction through formatting, instance-array callback
reentry and static-array callback reentry. The latter additionally exposes
the erased Any/Send storage gap already recorded above. No format test has
been excluded or relaxed. String.format's current AsRef<str> template,
Result<String> argument callback, scalar output and literal-template shortcut
must migrate coherently; native numeric directive rendering should remain
shared, not fork into a new FP kernel. Static erased storage requires a
separate capability/representation audit consistent with CONCURRENCY-SCOPE.md;
do not repair it with unsafe Send or per-thread copies of Java static state.

Final substantial verification `.tmp/utf16-v329-batch-current.log`:

- **1705/1705 library tests**, 84.28 seconds.
- **6/6 hashing**, 15.98 seconds; **2/2 lossless String**, 15.00 seconds.
- **1/1 mapped executable**, 11.03 seconds: all four result shapes compile
  and run, including native non-BMP text, both null-return paths and both
  checked exception paths. The signature fix closes the false null panic.
- **13/17 RootFinder contracts**, 20.13 seconds; precisely the same four
  formatting/storage failures described above. The command exits 101 for
  that target, not a successful overall preservation gate. All remain enabled.

Frozen producer `.tmp/new-java2rust-numbers-default-v329-text-consumers-debug`,
SHA256 `bdbd60ccd7f0ef5e75e0dad0f1604de96586e5ba37d69c2d6b4e0c56636bfce2`,
build log `.tmp/utf16-v329-producer-build.log`. Assembly scripts are unchanged
at the V327/V328 hashes. All owned jobs are terminal. Formatting and
`git diff --check` pass. No native scalar Display/AsRef escape hatch was added.

Reproduce with the recorded offline Cargo/TMPDIR environment:

```sh
cargo test --no-fail-fast --lib --test mapped_text_contracts \
  --test java_hash_contracts --test lossless_string_contracts --test rootfinder_contracts
cargo test --test big_integer_contracts
```

The complete Complex/client V328 result keeps its actual frozen producer;
no claim of a V329 complete-corpus or fresh accepted-module/RNG preservation
rerun is made. The next coherent implementation is the formatting template/
argument/output cutover and its reentrant storage audit, followed by the
outstanding executable Map.Entry display audit and comprehensive preservation
gates. The goal remains active, all six modules in scope and **3/6 accepted
checkpoints**.

### V330: one lossless formatting contract, including lazy display and relays

V329's complete library is green but RootFinder contracts are 13/17, with
four formatting/storage failures. The baseline and whole-path audit are the
V328/V329 sections above. The minimum formatter rewrite changes template
iteration, literal/argument storage, callback signature and output together;
native numerical directive rendering remains shared. A literal-template Rust
format shortcut cannot retain this contract for JavaString arguments or lazy
display. The static erased-array failure is independently a representation
capability problem, not evidence that the text rewrite needs another lock.

`src/java_string_format.rs` is the shared emitted formatter implementation.
The existing numeric directive renderer was moved, not replaced by another
FP algorithm. Templates/literal spans and argument text travel as UTF-16
units; the producer selects JavaString or the explicit native scalar output
boundary through JavaFormatString. Text directives never project malformed
units to scalar strings. Uppercase text preserves unpaired units while
reusing the existing scalar character transformation. This does not claim
complete Java Formatter locale/flag/index/type-conversion coverage; the
existing directive grammar's other limitations are not relabeled acceptance.

Source fixed reference-array calls retain their original lazy indexed-read
path, with a JavaString callback/output ABI and the shared logical optional
display helper for nullable String elements. Array read guards end before
user display code. Expanded varargs instead materialize their typed carriers
left-to-right and dispatch requested displays by index. This works for both
literal and dynamic templates, preserves unused-argument evaluation without
executing unused toString callbacks, and does not force borrowed array views
through a fictitious Any/'static escape.

The initially attempted synthetic Object[] packing is deliberately superseded,
not left as a special case: executable evidence showed that it imposed an
unjustified 'static bound on borrowed array arguments. Both paths now share
the same formatting kernel and counted request protocol. Counts are captured
without retaining array guards; a null argument array retains Java's repeated
null behavior. Missing requests throw MissingFormatArgumentException with the
directive message rather than returning fabricated empty text or leaking an
array bounds failure. The subtype edges reuse JDK_EXCEPTION_PARENTS.

The existing virtual-object-dispatch exception effect now includes native
formatting, using the ordinary call planner for source-owner precedence.
It also applies to synthetic callable bodies. This preserves arbitrary Java
callback Throwable values and keeps validation errors catchable through
source relays, instead of unwrapping them into formatted Rust panic strings.
The intermediate RuntimeException-only effect was replaced by this shared
Throwable-aware protocol, not retained as a second formatter scanner.

An adjacent initializer boundary was audited from emitted
Box::new(raw) expressions that consumed a still-live JavaString. The unslotted
typed-array initializer now shares the ordinary retained-child recovery path;
reused concrete references are cloned before their carrier is erased, not
after constructing an Any box which already consumed them. This is a general
array/erasure fix, not a change to the Java fixture or generated deliverable.

Permanent `FormatTextProbe.java` and `tests/format_text_contracts.rs` compare
untouched Java/generated execution of raw-surrogate templates and arguments,
non-BMP text, unused argument construction versus display, empty varargs,
uppercase raw text, numerical directives, percent/newline escapes, callback
exceptions with raw messages, missing-argument messages/subtypes, callback
Error values and a deferred Supplier whose creation must not execute its body.
The two-file real assembly fixture additionally formats raw text in a peer
module and checks the result in the client, retaining all prior checks.

Evidence and failed prefixes remain separate:

- `.tmp/utf16-v330-format-prefix.log`: the first formatter run exposes empty
  argument type inference, old nullable String-array scalar display and the
  expected static-storage error. Its emission-only single-array assertion is
  obsolete after the common typed callback path, not proof of runtime behavior.
- `.tmp/utf16-v330-format-current.log` is a terminal host type-check failure
  (a missing RustType Ref lifetime field), fixed before the next invocation.
- `.tmp/utf16-v330-format-current2.log` and
  `.tmp/utf16-v330-format-typed-arguments.log` preserve the ownership/borrowed
  array failures which motivated the whole-path replan.
- `.tmp/utf16-v330-format-retained-elements.log` passes the initial raw-text
  executable. `.tmp/utf16-v330-counted-format.log` then shows that the source
  formatter relay was inferred infallible and turned a missing-argument
  exception into a Rust panic. This was a product contract gap, not a bad test.
- `.tmp/utf16-v330-library.log`: **1705/1705**, 95.89 seconds, at the preceding
  formatter-effect prefix. It is not mislabeled as the final producer check.

Final `.tmp/utf16-v330-complete-format-batch.log`:

- **1705/1705 library tests**, 78.29 seconds.
- **1/1 expanded formatting/JDK fixture**, 14.32 seconds.
- **2/2 lossless String**, 16.11 seconds; **1/1 mapped text executable**,
  12.08 seconds.
- **15/17 RootFinder contracts**, 14.33 seconds. Three of V329's four
  formatting failures now pass: reference-array expansion/unused display,
  instance-array callback reentry/errors and source exception construction
  through formatting. The old static erased-array failure remains.

The other remaining failure is newly measured executable coverage, not a
claim that an old runtime gate had passed: the former primitive-array/cast
assertion only checked emission spelling. It is now backed by an independent
Java/generated caller, `FormatArraySingleArgumentProbe.java`, checking that a
primitive array or an array cast to Object is one array-object argument, not
flattened contents. Current output asks a borrowed &[i32] for Rust to_string;
the shared logical array-object display contract is missing. The test remains
enabled and red, rather than restoring a weaker snapshot assertion.

Next architectural audit must address both missing representations:

1. Static Mutex storage contains Box<dyn Any> without Send. Existing source
   static-reachability lifting and erased clone/storage/signature/consumer
   paths need one capability contract. Adding locks, unsafe Send, or per-thread
   copies of process-wide Java statics would not be a valid fix.
2. Array-object display needs retained Java allocation identity and runtime
   component-class information, including covariance, null and cast views.
   A descriptor guessed solely from the declared array parameter would pass
   a small example but fail Object[] views of String[]; do not overfit the
   new executable by adding that shortcut. Reuse/generalize array allocation,
   class naming and reference-identity mechanisms together.

Frozen producer `.tmp/new-java2rust-numbers-default-v330-lossless-format-debug`,
SHA256 `343d468125ecfd76500710593e8133b105f856d3f5bcd0b8a69f59227407ee30`,
build `.tmp/utf16-v330-producer-build.log`. Assembly scripts retain their
V327/V328 hashes. `.tmp/utf16-v330-format-shell-assembly` passes the actual
two-file runner (0 compile errors, 23139 ms), executable Rust contract and
byte-identical Java/generated stdout `contract=0`, including peer formatting
of malformed UTF-16. Production modules/root were not patched; only the
existing independent Rust caller/example are added by the test script.

Reproduction uses the same offline Cargo/TMPDIR environment:

```sh
cargo test --no-fail-fast --lib --test format_text_contracts \
  --test rootfinder_contracts --test lossless_string_contracts --test mapped_text_contracts
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v330-lossless-format-debug" \
  NEW_JAVA2RUST_BUILD_PROFILE=debug \
  bash scripts/test-lossless-text-assembly.sh "$PWD/.tmp/NEW-v330-format-assembly"
```

All owned jobs are terminal; formatting and diff checks pass. The substantial
batch has made progress but the integration command remains red on the two
recorded cases. Before committing to a broad representation redesign, recheck
the accepted production corpus gates on this producer and distinguish actual
preservation regressions from newly exercised ancillary gaps. Establish their
historical baseline with a frozen earlier producer rather than assuming which
turn introduced them. Scope remains the six pinned modules, not implementing
every previously unexercised JDK Formatter operation before advancing.

These are concrete framework/storage gaps beyond the completed formatter
text cutover. No upstream edits, manual generated-code repairs, exclusions, FP
tolerance changes or numerical compatibility campaigns were made. The complete
Complex/client V328 and accepted-module/RNG results retain their actual
producer labels; this is not a V330 full-corpus/preservation claim or module
acceptance. The goal remains active, all six modules in scope, **3/6 accepted**.

### V331: production preservation baseline and source text-boundary batch

Rechecked the previously accepted production modules before extending the
ancillary Formatter/array representation campaign. All three runs used the
frozen V330 producer SHA256
`343d468125ecfd76500710593e8133b105f856d3f5bcd0b8a69f59227407ee30`;
their `translation-producer.tsv` files retain that provenance. The standard
runners verified the untouched release pin and complete manifests again.

| Complete source scope | Baseline artifact | Compile errors |
| --- | --- | ---: |
| Primes: all 3 production files | `.tmp/commons-numbers-1.3-primes-v331-preservation` | 2 |
| Root Finder: all 3 production files | `.tmp/commons-numbers-1.3-rootfinder-v331-preservation` | 3 |
| Core: all 11 production files plus independent translated client | `.tmp/commons-numbers-1.3-core-contracts-v331-baseline` | 22 |

Logs are `.tmp/numbers-primes-v331-preservation-run.log`,
`.tmp/numbers-rootfinder-v331-preservation-run.log`, and
`.tmp/numbers-core-v331-baseline-run.log`. All three commands terminated with
failure, not an observation timeout. No new preservation claim follows from
the historical 3/6 accepted count.

Representative rewrite/contract audit:

- Primes `NUMBER_TOO_SMALL` and Root Finder `SolverException.TOO_LARGE`
  remain legitimate Rust `&'static str` associated constants. Calls instead
  attempted `JavaString::clone(&Owner::CONST)`. The minimal correct value is
  `JavaString::intern_literal(Owner::CONST)`, with a loan only when the solved
  callee argument is borrowed. Retain const-compatible storage and existing
  Java interning; normalize both lexical/imported and qualified reads through
  one source-constant projection. Expression shape must describe the resulting
  owned reference, not the borrowed scalar storage. This is an emission plus
  producer/consumer shape-contract fix, not a new String representation.
- Core arithmetic error concatenations borrow those constants as `&&str`.
  The existing append forwarding implementation unnecessarily required a
  sized, clonable referent. Forward an already borrowed append capability
  instead; this also covers unsized char-unit slices without copying or
  inventing Display/AsRef<str> for logical String objects.
- Core's null `Norm.valueOf` client passes `Option<&JavaString>` into an
  enum helper still accepting `Option<&str>`. The enum name-check boundary
  must consume the selected logical text ABI, compare original UTF-16 units,
  and retain the original units in an invalid-name exception message. Keep
  the existing class-initialization/error-relay behavior and native scalar
  callers through JavaTextView. Enum name and reflective class-name producers
  likewise require logical String results for Java consumers. Enum-only units
  exposed missing checked-hash and erased-text support dependencies; select
  their existing authoritative runtime owners rather than duplicating leaves.
- One separate Core production failure is
  `BigDecimal::new(JavaString::fresh(java_double_to_string(x)))` in
  `Precision.round`. The external runtime's BigDecimalInput supports native
  String/&str and numbers, not logical source strings. A hypothetical checked
  scalar projection proves the ordinary numeric case is expressible, but a
  blanket expect/lossy conversion is not a valid general bridge: invalid Java
  input must retain Java NumberFormatException behavior and UTF-16 diagnostics.
  Next audit should cover mapped constructor input facts, native numeric text
  parsing and error-message return transport together. Do not implement a
  corpus-specific Precision/Double.toString peephole or a second decimal
  algorithm. This is a cross-function runtime/FFI contract, not a numerical
  rounding or broad ownership-framework problem.

New permanent cross-file `stringconstants` Java fixtures exercise lexical,
qualified and static-import reads, identity, concatenation, formatting,
receivers and a shadowing local through the real package assembly route.
EnumLookupProbe additionally checks a malformed UTF-16 invalid-name message.
The Rust-only Primes/RootFinder observation drivers now explicitly project
their known-scalar messages at the output boundary; the Java observations and
expected behavior remain unchanged.

Prefix evidence, explicitly not final-source verification:
`.tmp/utf16-v331-library.log` passed **1705/1705** in 80.72 s before the
final source-only runtime dependency and constant-shape corrections.
`.tmp/utf16-v331-source-boundaries-current.log` passed the existing full enum
Java fixture (including malformed-name addition), formatting and both older
lossless-text tests; the pure-enum support check and new cross-file constant
call check exposed the subsequent dependency/borrow corrections. Earlier
`.tmp/utf16-v331-source-boundaries.log` also records a test-only missing oracle
path, corrected without weakening the behavior gate. Final verification is
recorded below once terminal evidence exists; no generated output was patched.

Final focused batch `.tmp/utf16-v331-source-boundaries-final.log` is terminal
and green: **3/3 enum lookup** (30.47 s), **1/1 formatter** (14.61 s),
**3/3 lossless text** (26.80 s), including actual two-file package assembly
and execution. The append check covers `&&str` and borrowed raw unit slices.
The shared constant read shape now describes the materialized Java reference;
the new test's borrowed-return relay passes without patching generated Rust.

Frozen final producer
`.tmp/new-java2rust-numbers-default-v331-source-text-boundaries-debug`, SHA256
`da4067c0070b486bdbb4f44921b34e01ebac6456b8287076cd76a8789791216a`,
build `.tmp/utf16-v331-final-producer-build.log` (51.64 s). The earlier
`.tmp/utf16-v331-producer-build.log` is a prefix build, not this producer.
Assembly scripts retain the V327/V328 hashes recorded above.

Reproduction (same offline Cargo/TMPDIR environment as preceding checkpoints):

```sh
cargo test --test lossless_string_contracts --test enum_lookup_contracts \
  --test format_text_contracts --no-fail-fast
cargo test --lib --test enum_initialization_contracts --no-fail-fast
NUMBERS_MODULE=primes JOBS=2 \
  NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v331-source-text-boundaries-debug" \
  NEW_JAVA2RUST_BUILD_PROFILE=debug \
  NUMBERS_STAGING_ROOT="$PWD/.tmp/NEW-primes-staged" \
  NUMBERS_TRANSLATION_ROOT="$PWD/.tmp/NEW-primes-output" \
  bash scripts/run-commons-numbers-1.3.sh
# Root Finder: same command with module=rootfinder and fresh artifact paths.
# Core: run-commons-numbers-1.3-core-contracts.sh with fresh
# NUMBERS_CONTRACTS_STAGING_ROOT / NUMBERS_CONTRACTS_TRANSLATION_ROOT.
```

Final library/enum-initialization and full production preservation results
belong to `.tmp/utf16-v331-final-preservation-tests.log` and
`.tmp/numbers-{primes,rootfinder,core}-v331-final-run.log`; inspect their
terminal summaries before making preservation claims.

The frozen V331 producer's complete Primes and Root Finder reruns are terminal
and green, both **0 compile errors**. Independent caller comparisons
`.tmp/numbers-primes-v331-final-behavior.log` and
`.tmp/numbers-rootfinder-v331-final-behavior.log` pass **3594** and **122**
observations respectively, with exact exception text, discrete/helper mutation
behavior, IEEE roots and callback traces/failures. This closes the measured
V330 production regressions in these two modules; it is not Core/Complex/RNG
preservation evidence.

The same frozen revision passed **1705/1705 library tests** in 78.71 s, but
the added enum-initialization preservation sweep was **1/4**, with three
compile failures on `.name()` called on the nullable reference returned by
`__java_enum_constant`. This exposed a missing physical receiver contract in
the new enum-name path. The follow-up source correction reuses the existing
`source_java_enum_static_variant_shape` and accessor shape mechanisms rather
than inventing a second nullability rule. Its executable results are recorded
in `.tmp/utf16-v331-enum-physical-contract.log`; the preceding frozen producer
and library results must not be relabeled as this later source revision.

Core's full 12-file V331 rerun is also terminal, with **1 compile error**
instead of the baseline 22: only `JavaUtf16String: BigDecimalInput` at the
recorded `Precision.round` constructor boundary remains. This production
evidence prioritizes the reusable native numeric text-input bridge next,
before broad ancillary array-object display/static-erasure redesigns. No
Core behavior comparison can run until its generated crate compiles.
The physical enum follow-up passes **4/4 initialization** (14.15 s) and
**3/3 enum lookup** (28.18 s), including construction/publishing order,
reentrant observations, payload state and identity checks.

Follow-up command is terminal and completely green: **11/11 executable
integration tests**, comprising the 4 initialization and 3 lookup checks above,
**1/1 formatting** (14.36 s) and **3/3 lossless text** (24.69 s).
Final follow-up producer is frozen separately as
`.tmp/new-java2rust-numbers-default-v331-enum-physical-debug`, SHA256
`23cb9491cf5720a78216588c16e521b8d444eb7a84d7a78978bd8644df1173b1`;
`.tmp/utf16-v331-enum-physical-producer-build.log` confirms the current
binary was up-to-date after the executable batch built it. Complete-module
and 1705-test library evidence above retains the earlier V331 producer label;
the final small physical-enum correction has focused executable coverage,
not a silently relabeled full-corpus rerun. All owned processes from this
checkpoint are terminal. Formatting and diff checks pass. Next coherent batch
is the native numeric String-input/error-transport contract needed by Core;
full library and broader preservation belong at that substantial boundary.

FP scope follows AGENTS.md: the production failures here are representation
errors. No tolerance or IEEE compatibility policy was changed, and artificial
stress-only discrepancies do not authorize an unmotivated numerical campaign.
All six modules remain in scope and the goal remains active.

### V332: explicit mapped UTF-16 text ABI and the native decimal boundary

Entry evidence is V331's untouched complete Core/client output: generation
and parsing succeed for all 12 files, with exactly one remaining compile
failure at `Precision.round`: native `BigDecimalInput` cannot accept the
logical Java String produced by `Double.toString`. This is not a decimal
rounding discrepancy or an ownership-framework redesign. A scalar expect
would compile the ordinary number but is not a valid whole-path solution.

The concrete rewrite contract is: evaluate Java argument expressions in order,
retain their actual String/null carriers until every argument has been
evaluated, encode the original units for the native invocation, run the
existing decimal coefficient/scale parser, and transport nullable exception
messages back in the same units. Numeric constructor overloads must retain
their existing input types. UTF-8-only Rust String and Java UTF-16 are distinct
ABIs; inventing AsRef<str>/Display on JavaString or specializing on
Precision/Double.toString would erase that distinction.

Implemented through existing authoritative mechanisms:

- Mapped pack declarations may opt into `string-abi=utf16`, recorded as
  `ExternalStringAbi` on the existing pack fact. The default remains `utf8`;
  invalid spellings are diagnosed. The UTF-16 physical ABI uses owned Vec<u16>
  values, with null represented by the existing Option. Source-owned calls
  remain under their solved source signatures, not the mapped pack path.
- Constructor and mapped receiver argument lowering reuse ordinary slot
  planning, the existing argument temporary representation and block wrapper.
  String sources retain physical optional/borrowed carrier evidence; null
  literals use formal String metadata rather than an invented argument type.
  The actual encoding/null check follows complete argument evaluation. Indexed
  generated names avoid same-scope temporary shadowing in multi-argument calls.
- Mapped String results reuse the existing plain/nullable/Result/Result<Option>
  normalization, selecting JavaString::from_units for opted-in packs. Existing
  explicit custom Rust-return contracts keep their precedence. External
  exception conversion reuses the declared getMessage mapping and applies
  the same unit-preserving nullable transport instead of Display.
- The java-math decimal parser now has one UTF-16 iterator kernel; native &str
  inputs encode into that same kernel. Coefficient/scale, exponent, rounding,
  addition and binary conversion algorithms are unchanged. BigDecimalInput
  accepts unit vectors/slices as well as its existing numeric/native inputs.
  NumberFormatException stores optional units; offending surrogate characters
  are retained in logical messages. The older native message/Display functions
  are explicitly output adapters; new *_utf16 functions serve the logical ABI.
  BigDecimal, RoundingMode and NumberFormatException opt in through their
  reusable JDK pack declarations, not corpus-specific lowering branches.

Permanent executable coverage adds a Java/JDK decimal input probe for ordinary
numeric text, BMP decimal digits, isolated/supplementary invalid input, exact
message units and null input. A separate mock foreign crate—not java-math—
checks multi-argument construction, argument side effects before a null failure,
source reuse, plain/nullable/checked/checked-nullable return shapes, and raw
surrogate messages. Existing UTF-8 mapped return tests remain enabled.

`.tmp/utf16-v332-native-text-general.log` is terminal and green: **4/4 decimal
tests** (23.04 s), including the unchanged **2908-request JDK runtime matrix**,
and **3/3 mapped-text tests** (12.63 s). Earlier failed logs are retained:
`native-text-prefix` has one translator-host variable-shadowing compile error;
`native-text-extended` exposed temporary name collisions and null-literal
formal type loss in the new unrelated-pack executable. The parser fact test
initially lacked the declared member needed to establish a simple-name pack
alias; it now declares that member rather than changing name resolution.

Frozen producer `.tmp/new-java2rust-numbers-default-v332-native-utf16-debug`,
SHA256 `1301a35fcaaf216dbd8ba692dda4730d91468d4507b3b5c5080b755160bafd78`.
Final build confirmation `.tmp/utf16-v332-final-producer-build.log` is terminal
and up-to-date after executable testing built that binary. Runtime snapshot:

| Input | SHA256 |
| --- | --- |
| java-math manifest | `ac1fe96b0b8a02e4faff4760b7d079e81682748467dcdec554e1a8d7e401068a` |
| java-math lib.rs | `37e8b26c4811902864b1f39fab4646402dda5587fe8ad930c7e40acf64ba9861` |
| java-math decimal.rs | `702c43f1613c4501b489381a96acf8c9091d39b26e7e92f1313058a448711546` |

Assembly scripts retain the V327/V328 hashes. Full Core/client runner output
is `.tmp/commons-numbers-1.3-core-contracts-v332-native-text`, staged inputs
`.tmp/numbers-core-v332-native-text-staged`, outer log
`.tmp/numbers-core-v332-native-text-run.log`. Inspect terminal results before
claiming compile/behavior preservation. CoreOracle.rs has only caller ABI
adaptations: explicit scalar output for known-scalar numeric/error text and
logical String values/equality at erased input/display boundaries. The Java
oracle and all requested observations remain unchanged.

Reproduction uses the same offline Cargo/TMPDIR environment:

```sh
cargo test --test big_decimal_contracts --test mapped_text_contracts --no-fail-fast
cargo test --lib
JOBS=2 NEW_JAVA2RUST_BUILD_PROFILE=debug \
  NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v332-native-utf16-debug" \
  NUMBERS_CONTRACTS_STAGING_ROOT="$PWD/.tmp/NEW-core-staged" \
  NUMBERS_CONTRACTS_TRANSLATION_ROOT="$PWD/.tmp/NEW-core-output" \
  bash scripts/run-commons-numbers-1.3-core-contracts.sh
```

The prefix `.tmp/utf16-v332-library.log` passed 1705/1705 in 85.34 s before
the final argument-name/null-literal corrections. Final-source library evidence
belongs to `.tmp/utf16-v332-final-library.log`, not that prefix: **1705/1705
pass** in 85.17 s on the final frozen revision. No upstream or
generated production source edits, numerical algorithm changes, new tolerances,
source exclusions or newly accepted modules are claimed here. The goal remains
active for all six modules.

V332 production/behavior results are now terminal:

- All **11 untouched Core production files plus the independently translated
  client** compile: 0 failed translations/parse failures, 0 unsupported
  statements/members/expressions, 0 invalid patterns, 0 stubbed constants and
  0 reachable no-op facade calls. The runner verified the exact release pin
  and complete production/dependency manifest. Total runner time 223973 ms,
  including 159299 ms translation and 2163 ms cargo check; timings overlap
  concurrent verification and are not an isolated performance benchmark.
- `.tmp/numbers-core-contracts-v332-native-text-behavior` passes exact Java/
  generated output (`contract=0`, `oracle=ok`), exercising all 34 client guards.
- `.tmp/numbers-core-v332-native-text-behavior-final` passes all **7888**
  observations under the previously accepted **arithmetic-nan-sign-v1** policy.
  Four strict differences remain recorded, not hidden: sum.6.selfsubtract,
  sum.7.selfsubtract, dd.6.pow.-3.plain and dd.8.reciprocal. Every other
  observation is exact; no tolerance was added or expanded.
- Fresh complete Primes and Root Finder runs use the same V332 producer and
  current runtime/manifest: `.tmp/commons-numbers-1.3-primes-v332-preserved`
  and `.tmp/commons-numbers-1.3-rootfinder-v332-preserved`. Both have 0 compile
  errors/unsupported IR/invalid patterns/stubbed constants/reachable no-op
  calls; their corresponding `.tmp/numbers-*-v332-preserved-behavior` artifacts
  pass **3594** and **122** exact observations.

The first two numerical Core attempts stopped at Rust-only caller compilation:
the old caller assumed required references and infallible outputs, while the
generated module/client closure correctly retains nullable parameters and
fallible operations. These failures are preserved in
`.tmp/numbers-core-v332-native-text-behavior{,-current}`. Caller adaptations
wrap the same non-null references in Some, propagate Result with `?`, and use
the emitted mutable trait-receiver ABI. No native arithmetic or generated
production algorithms were substituted; all requested observations/inputs and
Java expected results remain unchanged. The final driver has no ignored Result
warnings. Broad Commons RNG preservation retains its earlier actual checkpoint
labels and must be rechecked at a later substantial/final boundary, not inferred
from these Core/Primes/Root Finder results.

All V332-owned processes are terminal. Formatting and diff checks pass. The
next module is **Complex**, whose latest complete/client evidence is still
V328, not V332. Revalidate that module on the frozen producer, then classify
remaining numerical differences using the practical FP scope before changing
any numerical contract. Quaternion and Angle remain in scope after Complex;
historical accepted module count remains **3/6**, not goal completion.

### V333: current Complex baseline and independently traced numerical residuals

Previous goal turn made concrete progress: the general mapped UTF-16 bridge
closed Core's last production compile failure and restored all three earlier
module behavior gates. This turn revalidates Complex before any FP campaign.

Frozen producer remains V332, SHA256
`1301a35fcaaf216dbd8ba692dda4730d91468d4507b3b5c5080b755160bafd78`.
`.tmp/commons-numbers-1.3-complex-contracts-v333` uses all **2 untouched
production files** (Complex.java and package-info.java) plus the independent
translated ComplexContractsOracle client. The standard runner verifies release
pin `ae8abefb51c2c4d1a05882d61cefc8015e493a8f`, complete manifest and source
identity again. Rechecked imports and module POM: production depends only on
modeled JDK Serializable, ArrayList/List and DoubleUnaryOperator. Core and RNG
dependencies in the POM are test-scoped, not an omitted production closure.
The preexisting full declaration manifest remains
`tests/corpus/numbers/Complex-1.3-declarations.txt`.

Outer log `.tmp/numbers-complex-v333-run.log`: all 3 files translate/parse/
compile, **0 compiler errors/unsupported IR/invalid patterns/stubbed constants/
reachable no-op facade calls**. Inventory warnings remain inventory, not
reachable fake bodies. Total 144031 ms, translation 117516 ms, check 2028 ms;
not an isolated performance benchmark. Strict independent client behavior
`.tmp/numbers-complex-v333-contracts-behavior` passes `contract=0`, `oracle=ok`
and all **32 guards**. Complete operation stream
`.tmp/numbers-complex-v333-operations-behavior` has **14617 observations**,
strictly red with **78 differences**. Its differences file is byte-identical
to V328's, not a fresh collection of string/ownership regressions.

Ordinary-input primitive audit, rather than a new JVM-compatibility campaign:

| Traced primitive | Java error in ULPs | Generated error in ULPs | Nearest binary64 |
| --- | ---: | ---: | --- |
| cosh(1), reached by Complex.cosh(1,+0i) | 0.7024563251 | 0.2975436749 | `3ff8b07551d9f550` |
| atan2(-3,-2), exactly Complex.arg(-2,-3i) | 0.5055415298 | 0.4944584702 | `c00145385fa3af71` |

`scripts/audit-complex-elementary-boundaries.py` reads the actual independent
observation files, checks matching ordered domains and evaluates these two
references with mpmath 1.3.0 at **100 and 200 decimal digits**, yielding the
same nearest bits and error estimates. JSON evidence:
`.tmp/numbers-complex-v333-elementary-reference-audit.json`. This is a
development diagnostic dependency only, not a translator/runtime dependency
or an acceptance policy. At these two measured inputs, the native Rust result
is closer to the mathematical value; reproducing the JVM bit would reduce
accuracy rather than correct an observed translation error.

The [Java Math contract](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/Math.html)
allows implementation-dependent elementary results subject to accuracy and
monotonicity requirements; cosh has a 2.5-ULP bound and atan2 a 2-ULP bound.
This differs from the specified-algorithm obligations of
[StrictMath](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/StrictMath.html).
Complex uses Math, not StrictMath. NaN result encodings are also not a universal
Java arithmetic guarantee; see [JLS 4.2.3](https://docs.oracle.com/javase/specs/jls/se21/html/jls-4.html#jls-4.2.3).
None of those statements justify ignoring raw-bit copies, signed-zero branch
sides, classification changes, state or exceptions.

Permanent `ElementaryMathProbe.java` / `ElementaryMathOracle.java` and
`tests/elementary_math_contracts.rs` exercise **automatically translated Math
wrappers**, independent JDK observations, the two reference bounds and exact
signed-zero/branch-side bits. StrictMath output is collected only as diagnostic
JDK context, not claimed as translated coverage. Focused test passes in
11.23 s: `.tmp/numbers-complex-v333-primitive-tests.log`. No product source or
numeric comparison default changed, so another full library/corpus sweep after
this additive component check would not provide a new producer measurement;
V332's 1705/1705 library and earlier-module evidence retain their actual labels.

Diagnostic rewrite feasibility experiment, explicitly **not a deliverable**:
`.tmp/complex-math-replay-v333.43ECko` copies the complete generated crate into
a fresh isolated directory. First rename the ten native cosh/atan2 call lines
to diagnostic f64 trait methods served by a persistent JVM Math process,
without changing inputs, Complex algorithms or the public operation caller.
Strict residuals fall **78 -> 14**: 63 finite differences plus the artificial
huge-power phase difference disappear; two finite powers and 12 NaN-encoding
differences remain. Log `.tmp/numbers-complex-v333-two-kernel-replay.log`,
artifact `behavior/`. Then route native exp/sin/cos through the same diagnostic
server too (23 more lines). Strict residuals fall **14 -> 12**, with **no
finite residuals**: `.tmp/numbers-complex-v333-five-kernel-replay.log`,
artifact `behavior-five-kernels/`. Those 12 are the previously observed four
multiply NaN-sign cases, two divide NaN-payload cases and six root NaN-sign
cases. Both strict diagnostic commands correctly exit1; neither is acceptance.
This isolates the finite discrepancies to elementary backends and shows that
no algorithm/ownership-framework rewrite is required for the measured cases.

The automatically translated production artifact is unchanged throughout:
Complex Rust SHA256
`002ab83e58c285ce576e5d5db7cd2b3e83420c2b4c8a2118ff9a8e777a5c2900`
before and after replay. Diagnostic code is labeled DIAGNOSTIC-ONLY; the JVM
server is not added to product/runtime dependencies. No input-specific result
table was substituted: the server evaluates the selected Math primitives for
every actual argument, caching only repeated pure calls.

The one extreme power has exponent 1e200. The observed atan2 difference is
4.440892098500626e-16 radians, magnified to roughly 4.44e184 radians before
trigonometric range reduction. Its opposite infinity sign is **not** called
a small final-value error. It remains sensitivity evidence, not motivation to
port a JVM kernel without a real downstream need, per the user's FP scope.

Next coherent acceptance batch should reuse the **pinned upstream numerical
reference contract**, rather than fit a tolerance to these 78 rows. Inspected
`CReferenceTest.java` and all 13 existing reference resources. Published test
limits are acos/acosh/cosh/sinh/tanh/exp=2 ULP, asinh=3, atanh/log/sqrt=1,
multiply=0, divide=7 and complex pow=9. The comparison canonicalizes NaNs but
requires matching signs (including opposite zeros). Its standard global
override is zero; do not enable diagnostic/global overrides to obtain a pass.
`TestUtils` defines explicit flagged-row handling; preserve/report any flagged
rows and all dataset manifests rather than silently deleting test inputs.
Numerical reference comparison must call the real generated production APIs,
retain strict special-value/control/identity/parsing checks, and validate Java
against the independent reference as well. Keep the broad raw-bit report
unchanged as diagnostics; do not invent a blanket final-value tolerance,
arithmetic NaN payload allowance or an operation-name whitelist merely to pass.
Mutational tests must reject changed signs/classes, wrong roots/order, branch
or exception/state changes and deviations beyond the stated numerical bound.

Reproduction (usual offline Cargo/TMPDIR environment):

```sh
JOBS=2 NEW_JAVA2RUST_BUILD_PROFILE=debug \
  NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v332-native-utf16-debug" \
  NUMBERS_CONTRACTS_STAGING_ROOT="$PWD/.tmp/NEW-complex-staged" \
  NUMBERS_CONTRACTS_TRANSLATION_ROOT="$PWD/.tmp/NEW-complex-output" \
  bash scripts/run-commons-numbers-1.3-complex-contracts.sh
python3 scripts/verify-commons-rng-1.0-behavior.py <output> --numbers-complex
python3 scripts/verify-commons-rng-1.0-behavior.py <output> --numbers-complex-contracts
cargo test --test elementary_math_contracts -- --nocapture
python3 scripts/audit-complex-elementary-boundaries.py <java-output.txt> <rust-output.txt>
```

All owned jobs are terminal; formatting/diff checks pass. Complex is **not yet
accepted**: the bounded reference gate is the next implementation step, not
an unexecuted plan mislabeled completion. Quaternion and Angle remain in scope;
the goal stays active, **3/6 historically accepted**.

## V334 independent reference gate and RNG preservation baseline

The V332 frozen producer was reused without generated-source repairs for the
complete V333 Complex artifact. The new explicit
`--numbers-complex-reference` verifier mode passed **1,863 requests** for both
Java and generated Rust against the release's independent reference datasets.
Evidence: `.tmp/numbers-complex-v334-reference-final.log` and its sibling
behavior directory. The earlier baseline run also passed; these are not two
different producer checkpoints.

`scripts/complex_reference.py` verifies the exact release commit and bytes of
`CReferenceTest.java`, `TestUtils.java` and all 13 reference data files. It
records their hashes, operation counts, limits, original row numbers and the
request hash in `reference-manifest.json`. The limits are upstream policy,
not fitted Java/Rust deltas: acos/acosh/cosh/sinh/tanh/exp 2 ULP;
asinh 3; atanh/log/sqrt 1; multiply 0; divide 7; pow 9. Both callers invoke
untouched production algorithms. Requests and the manifest are snapshotted
into the behavior output; verifier and reference-policy hashes are recorded.
No rows are excluded (this pin has no semicolon-flagged rows).

The numerical gate preserves signed zeros and nonfinite classes more strictly
than a bare bit-distance comparison. It checks both backends against the
independent answer, so agreement on the same wrong result cannot pass. Seven
focused Python tests exercise malformed/truncated/reordered outputs, changed
scope/bounds/manifest bytes, shared wrong answers and special-value distinctions;
all pass, as do the 11 existing strict-comparison tests. The default broad
Complex comparison remains strict and unchanged. The reference run retains
**139 strict backend differences**; these differ from the previously recorded
78 in the separate broad operations domain and must not be conflated.

Fresh Commons RNG preservation with the same frozen V332 producer exposed a
real logical-string ABI regression: full Source64 (11 files) has 6 compile
errors; complete sampling (46 files) has 26. Every reported error is a native
`Class.getName()` result returned by a Java `String` method, including synthetic
outer-receiver variants. Both translation jobs are terminal failures, not
behavior successes. Artifacts:
`.tmp/commons-rng-1.0-source64-v334-preserved` and
`.tmp/commons-rng-1.0-sampling-v334-preserved`; their outer logs are
`.tmp/rng-{source64,sampling}-v334-preserved-run.log`. The attempted Source64
behavior build also fails compilation and proves no behavioral preservation.

Minimal rewrite audit: the native reflective runtime may retain its Rust
`String` storage, but its Java call result must become `JavaString` once before
storage/return/argument consumers. The existing plain-expression-only adapter
misses slotted returns and duplicated outer bodies. This is a shared call-result
contract correction, not a runtime representation redesign or an RNG-specific
patch. V335 moves the adapter to the authoritative call-target application
boundary and adds `ClassNameProbe` covering returns, locals, argument and
constructor storage, plus a source-owned `getName()` negative control.
Verification and fresh preservation are pending; Complex remains unaccepted,
with **3/6 historically accepted** and the full goal active.

Complex public-surface audit against `Complex-1.3-declarations.txt` finds
**53 public method/overload entries**, no public constructor, and three public
constants. `ComplexOracle` calls every entry: the three factories and parse;
both real and imaginary accessor names; abs/arg/norm and three classifications;
conj/negate/proj; both add and subtract overloads plus imaginary/from variants;
both multiply and divide overloads plus imaginary variants; exp/log/log10;
both pow overloads; sqrt; all six circular and six hyperbolic functions;
nthRoot; equals/hashCode/toString. The Cartesian construction covers the
private constructor. The numeric matrix repeats operations on retained values
and checks their final state, allocation/alias identity, classifications and
all root-count branches; parse has valid and malformed text. The translated
client additionally covers null receivers/arguments, catchability/messages,
list allocation/mutation, shared constants, signed-zero branch cuts and an AC
phasor workload. Private functional-interface/method-reference helpers are
compiled and exercised through these public operations, not silently removed;
there is no public callback-taking API. This is operation/representative-branch
coverage, not a claim that every private branch is executed.

V335 focused executable regression passes (1/1, Java independently returns
zero, generated Rust executes all six guards), log
`.tmp/class-name-v335-focused.log`. Formatting and diff checks pass.
The full library plus `lossless_string_contracts`, `enum_lookup_contracts`
and `mapped_text_contracts` batch is running; log
`.tmp/class-name-v335-batch-tests.log`. A build/freeze/fresh complete sampling
run is queued on the cargo build lock, not claimed successful. Its frozen
binary will be `.tmp/new-java2rust-numbers-default-v335-class-text-debug`,
with hash in `.tmp/class-name-v335-producer.sha256`, and its new output is
`.tmp/commons-rng-1.0-sampling-v335-class-text` (outer log
`.tmp/rng-sampling-v335-class-text-run.log`). This is the complete 46-file
manifest, not a reduced error-only subset. The native 24-file and Source64
11-file manifests are both subsets of that manifest, verified before reuse;
their distinct behavior callers must still run against the fresh artifact.

The queued build has now completed successfully (20.08 s), freezing V335 at
SHA256 `52d113dcc72ae535ac4ea74c8f8121898a9f21b5393f65fe93647d77e06947f8`.
Fresh sampling translation is running. A same-producer complete Complex/client
translation is also running in
`.tmp/commons-numbers-1.3-complex-contracts-v335-class-text`, outer log
`.tmp/numbers-complex-v335-class-text-run.log`, chained to independent client
and reference verifications under `.tmp/numbers-complex-v335-class-text-*`.
These pending runs supersede neither the actual V334 failures nor V334's
successful reference gate until terminal evidence has been inspected.

The V335 test batch is now terminal and fully green:
**1705/1705 library tests**, **3/3 enum lookup**, **4/4 lossless string**
(including the new reflective result regression) and **3/3 mapped text**.
Log `.tmp/class-name-v335-batch-tests.log`; respective runtimes 138.23,
62.65, 33.73 and 14.25 seconds under concurrent corpus work. No unsupported
assertion helpers or ignored failing tests were used. Same-producer complete
Core/client preservation is now running as well, chained to its client and
existing arithmetic-nan-sign-v1 numerical gates; output
`.tmp/commons-numbers-1.3-core-contracts-v335-preserved`, outer log
`.tmp/numbers-core-v335-preserved-run.log`. No new tolerance was introduced.

Fresh V335 Complex translation and both chained gates are now terminal
successes: all 2 production files plus the translated client compile, with
0 translation/parse/compile errors, unsupported IR, invalid patterns, stubbed
constants or reachable no-op facade calls. The client passes both output lines
and all 32 guards. Both backends again meet all **1863 pinned independent
reference requests** (139 strict backend differences retained). The generated
Complex production file's SHA256 is unchanged from V333:
`002ab83e58c285ce576e5d5db7cd2b3e83420c2b4c8a2118ff9a8e777a5c2900`.

The separate full public-operations run is also terminal, deliberately strict
and still red: **78/14617** observations differ. Both the complete generated
output and `differences.txt` are byte-identical to V333. Thus the earlier
independent primitive accuracy audit and diagnostic-only pure-kernel replay
still explain the exact same residuals; they have not been replaced by a
new whitelist or relaxed default comparator. Evidence:
`.tmp/numbers-complex-v335-class-text-{contracts,reference,operations}-behavior`.
Total translation runner time 255599 ms under concurrent verification, with
186938 ms translation and 3496 ms cargo check. Complex acceptance still awaits
RNG/earlier-module preservation. Same-producer fresh Primes then Root Finder
preservation has additionally been launched sequentially; outputs
`.tmp/commons-numbers-1.3-{primes,rootfinder}-v335-preserved` and their
`.tmp/numbers-*-v335-preserved-{run,behavior}.log` files are pending, not passes.

V335 earlier-module preservation is now terminal and successful. Complete
Primes (3 files), Root Finder (3 files) and Core (11 production files plus its
translated client) each have 0 translation/parse/compile errors, unsupported IR,
invalid patterns, stubbed constants and reachable no-op facade calls. Behavior
gates pass **3594 exact Primes observations**, **122 exact Root Finder
observations**, the Core client contract, and **7888 Core observations** under
the unchanged arithmetic-nan-sign-v1 policy. Core's four allowance records are
byte-identical to V332; every other observation remains exact. Evidence paths
are the previously listed V335 preservation outputs and behavior directories.
The complete sampling translation has emitted all 46 files and is still in
the assembly/verification pipeline; no RNG behavior success is claimed yet.

Complete V335 sampling translation/assembly/check is now terminal success:
**46/46 files**, 0 translation/parse/compile errors, unsupported IR, invalid
patterns, stubbed constants or reachable no-op facade calls. Total 492043 ms
(319262 ms translation, 107414 ms assembly, 10497 ms check), with concurrent
verification, not an isolated benchmark. This closes the actual 26-error
Class.getName production regression without repairing generated output.

The first attempt to reuse that artifact for the Source64-only Rust caller
stopped at caller compilation: two NumberFactory int-array conversion sites
expect slice/owned-vector ABI in the original 11-file scope, but the larger
sampling closure selects shared arrays. This is not a failed generated-library
compile or a successful behavior comparison. Failed caller evidence remains
`.tmp/rng-source64-v335-class-text-behavior/driver-build.log`. No caller checks
were removed or rewritten. Native/collections/distributions comparisons are
now running against the complete 46-file artifact. Source64 is instead being
freshly translated in its original complete 11-file scope at
`.tmp/commons-rng-1.0-source64-v335-class-text`, log
`.tmp/rng-source64-v335-class-text-run.log`, chained to the unchanged Source64
caller with output `.tmp/rng-source64-v335-standalone-behavior`.

Three RNG gates now pass on the full 46-file artifact: **12601 native-generator
observations**, **162 sampling-collections observations** and **2172 sampling-
distributions observations**, exact including float bits and RNG traces.
Outputs `.tmp/rng-{native,sampling-collections}-v335-class-text-behavior` and
`.tmp/rng-sampling-distributions-v335-logical-text-behavior`.
The initial distributions Rust caller failed on three stale native-String
types, recorded in
`.tmp/rng-sampling-distributions-v335-class-text-behavior/driver-build.log`.
Only its test RNG's toString return and display-input types were migrated to
JavaString, with an explicit checked `scalar_string()` host-output boundary.
All algorithms, inputs, traces, exception checks and display assertions remain
unchanged, as does the Java caller. The same generated library was reused;
there was no new translator change or generated-output repair. Source64's
separate original-scope translation/behavior gate is still pending.

### V335 Complex acceptance checkpoint: 4/6 modules

The last preservation gate is now terminal success: all **11 Source64 files**
translate/parse/compile with zero unsupported IR, invalid patterns, stubbed
constants or reachable no-op facade calls; the unchanged caller passes all
**2865 observations**, including exact floating-point bits. Evidence:
`.tmp/commons-rng-1.0-source64-v335-class-text/summary.tsv` and
`.tmp/rng-source64-v335-standalone-behavior`. Total runner time 150531 ms.

Complex is accepted on frozen producer V335 under the explicitly recorded
practical numerical contract, not a claim of universal JVM bit identity.
Requirement audit: complete untouched release source/dependency and private/
nested declaration manifests; successful complete production/client compile;
all 53 public operation/overload entries exercised by the full strict matrix;
32 translated-client guards for state, allocation/aliasing, invalid/null and
branch-cut behavior; both backends within all pinned independent reference
bounds; exact earlier-module/RNG preservation subject only to Core's unchanged
four NaN-sign allowances; full library and related executable regressions
green. All these claims have terminal evidence above on the same producer.
No production source is excluded or edited, no generated deliverable was
manually repaired, and no default comparator was relaxed.

The strict Complex matrix's 78 recorded differences remain visible and fully
unchanged: allowed elementary-kernel rounding, arithmetic NaN encoding, and
the previously audited artificial huge-phase stress input. The independent
reference gate, rather than a fitted Java-output tolerance or row whitelist,
is the numerical acceptance gate; branch/state/errors/signs/zero and nonfinite
classes in that gate and the practical client remain protected. The earlier
diagnostic-only JVM-kernel rewrite is feasibility evidence, never accepted
generated output. This follows the user's production-motivated FP rule.

All V335-owned verification jobs are terminal. **4/6 modules accepted**;
Quaternion and Angle are still required and the overall goal remains active.
Next baseline is the complete Quaternion module (Quaternion.java, Slerp.java,
package-info.java) plus the explicit **11-file Core production closure**:
14 files total, all already listed in the pinned full manifest. Rechecked
Quaternion POM declares Core as a production dependency; its source imports
Precision plus JDK Arrays, Serializable, ToDoubleFunction, BiPredicate and
DoubleFunction. No other Numbers module is introduced. Establish the untouched
baseline and inspect its hardest generated ownership/nested callback/enum
initialization contracts before any new fix campaign.

### V336 Quaternion baseline in progress (no translator fixes yet)

Baseline uses the same frozen V335 producer and untouched release pin. Complete
14-file production scope is staged by `NUMBERS_MODULE=quaternion` through
`scripts/run-commons-numbers-1.3.sh`, output
`.tmp/commons-numbers-1.3-quaternion-v336-baseline`, outer log
`.tmp/numbers-quaternion-v336-baseline-run.log`, staging
`.tmp/numbers-quaternion-v336-baseline-staged`. No caller or source subset has
been substituted. The runner checks the full manifest against upstream files.

Independent javac compilation of all 14 files succeeds, output
`.tmp/numbers-quaternion-v336-java-classes`, log
`.tmp/numbers-quaternion-v336-javac.log`. Full private/public/nested declaration
inventory (291 lines, 9 class files including javac synthetic artifacts) is
`tests/corpus/numbers/Quaternion-1.3-declarations.txt`. Quaternion has **33 public
method/overload entries** and five public constants, two private constructors;
Slerp has one public constructor and typed apply(double), plus its compiler
bridge. Nested Type/Default/Normalized callback tables, parsing-exception
constructors, and Linear/Spherical retained outer receivers are included.

Source audit identifies the hardest whole-path checks for the generated-code
feasibility audit: enum instances retaining three initialized functional
fields whose static helpers call other callbacks and read Quaternion state;
normalization/polar-form methods that must return the same allocation in the
appropriate states; a Slerp constructor that stores a nonstatic inner callback
retaining its outer Slerp; repeated callback dispatch without borrow-reentry
failure; specialized parse exceptions with causes and lossless text. These
are concrete source paths, not yet a classification of generated failures.
Inspect representative generated output and derive producer/storage/signature/
consumer rewrites before implementing any fixes. Neither a redesign nor
feasibility success is claimed while generation is pending.

V336 independent Java operation oracle is now implemented and terminal green:
`tests/corpus/numbers/QuaternionOracle.java`, runner
`scripts/run-commons-numbers-1.3-quaternion-java-oracle.sh`, log
`.tmp/numbers-quaternion-v336-java-oracle.log`. It compiles all 14 untouched
production files and produces **4532 ordered observations** including the
completion marker. Every Quaternion public overload is called; Slerp is called
directly and through DoubleFunction, repeatedly, across linear/spherical and
shortest-path branches, endpoints and extrapolation. Inputs include rotations,
normalization state/identity, scalar/vector factories and array copy behavior,
invalid lengths/nulls, equals/hash/string, exception causes and signed-zero/
nonfinite validation paths. Error and string output uses UTF-16 code units,
not lossy console replacement, including malformed-surrogate parse input.
This is caller-only expected-output evidence, not Rust behavior acceptance.

Initial generated-code audit is now possible for Slerp (Quaternion.java itself
is still lowering). Actual generated storage is `Rc<RefCell<Slerp>>`, with
`start/end: Option<Arc<Mutex<Quaternion>>>`; Linear/Spherical retain that outer
allocation. The constructor already stages the start value, allocates the
outer with defaults, and only then creates/stores the chosen callback carrying
that same outer allocation. No provisional need to redesign that staging.

One visible producer/storage/consumer mismatch occurs in both inner apply
bodies: `Quaternion::get_w_with_outer(Clone::clone(&self.__outer.borrow().start))`
(and x/y/z, start/end) forwards an optional field to a required-handle
companion. Minimal hypothetical correction is to clone the field's Option
while its borrow is live, release that borrow, null-project the cloned handle
with the existing catchable Java NPE policy, then call the same companion.
No signature weakening, field-default fabrication, or generated repair is
needed. This is provisionally a cross-function physical-slot projection bug,
not a representation redesign. Trace: declared outer field storage/lexical
seeding already exists in `seed_outer_this_capture_fields`,
`seed_lexical_outer_instance_fields` and `current_instance_field_shape_with_env`;
the shared companion receiver branch currently prioritizes a source-call
return shape or nominal expression inference instead. Reuse the authoritative
stored-field shape/optional receiver planner, not an Slerp-specific unwrap.

Borrow audit: canonical `apply_with_outer` holds at most an immutable borrow
of the callback slot while these callbacks read outer fields; that is not by
itself a RefCell mutable-reentry failure. The borrowed payload method must not
be substituted for the canonical handle companion in the Rust caller. The
remaining hard audit is Quaternion's enum/static callbacks and self-identity
returns; there is no final error count or whole-module feasibility verdict
until that output and the terminal baseline are available. No translator fix
has been made for Quaternion yet.

### V336 terminal baseline and V337 whole-path plan

The full 14-file V336 baseline is now terminal: 0 translation/parse errors,
unsupported IR, invalid patterns, stubbed constants or reachable no-op facade
calls, but **50 Rust compile errors**. These inventories do not imply behavior
acceptance. Full evidence is in its summary, compile log and top-error table.
Total 801918 ms: resolver preflight 101539, translation 607291, assembly 76882,
check 9047. Quaternion.java alone took 412888 ms to emit. No files were skipped.

Generated-code feasibility audit, including the difficult retained callbacks:

| Contract | Actual output / minimal semantically correct rewrite | Classification |
| --- | --- | --- |
| Nullable captured field | Slerp inner getters pass Option<Arc<Mutex<Quaternion>>> to required-handle companions. Project the authoritative stored slot before the call, preserving Java NPE propagation and short borrows. | Cross-function field/read/exception contract |
| Enum field vs method names | Payload getters `norm()`/`norm_sq()` collide with source methods taking a Quaternion. Allocate accessor names centrally and use the same mapping for declarations, reads and updates. | Shared naming/emission |
| Enum callback constructor | __java_enum_new drops generic arguments from its three native synchronous functional parameters, producing Box<dyn Any> defaults instead of Quaternion/f64 domains. Keep the typed callback carrier from caller through parameter to payload setter. | Constructor signature/storage contract |
| Nullable enum constants | Switch tests emit Some(constant.__java_enum_constant()), doubling Option; factories also pass published optional constants to required parameters. Use the physical slot contract. Switch labels denote discriminants, not runtime static-field reads; null selectors must still throw NPE rather than fall into default. | Enum projection/switch semantics |
| Callback reentry | Quaternion::norm/normSq/isUnit hold the Quaternion mutex while invoking a Type callback which reads that same Quaternion. Copy the enum/optional-enum slot into a short-lived owned temporary, release the object lock, then dispatch. Do not clone the Quaternion payload or replace its shared identity. | Local borrow-scope emission, executable deadlock regression needed |
| Nested static lambda typing | A Default.IS_UNIT callback emits nonexistent Precision::equals rather than its resolved overload. Preserve the typed lambda/declaring-owner context through support/assembly; inspect owner output versus support candidates before choosing the fix. | Resolution/assembly context, exact root not yet proven |
| Native text boundary | Quaternion.parse calls scalar AsRef<str> split with logical UTF-16 values; Core's required BigInteger exponent path appends the native object by value, unlike the earlier nullable caller closure. Route through existing lossless split/foreign-text conversion policies. | Shared text/foreign ABI |
| Source exception initialization | A NumberFormatException facade impl names a nonexistent Rust type, and the private parsing-exception constructor emits free init_cause(cause). Preserve native exception base, runtime subtype, message and cause through the source constructor. | Exception/facade support |

The current framework can express the inspected state-preserving rewrites via
owned slot temporaries, existing shared handles, callback carriers and source
constructor plans; no numerical algorithm rewrite is indicated. However,
initCause's initialization/alias contract needs a deliberate design check:
JavaException currently stores message/cause by value and has no initCause
operation. Do not disguise a general Throwable identity/lifecycle gap with an
unchecked setter or fabricated method. Slerp's generated strong outer/inner
reference cycle also merits a lifecycle audit before final acceptance; current
state/identity reasoning is not a claim of Java GC equivalence. Neither issue
authorizes silently expanding into a GC redesign. Typed public Slerp-to-SAM
conversion will additionally need a translated-client gate; the independent
Java caller alone has not proved that path in Rust.

### V337 implemented physical-field batch and measured lowering optimization

`OuterReferenceProbe` isolates live nullable outer fields captured by a retained
inner callback, including repeated reads, null failure caught by Java, then
field restoration. The first fixture also included an unrelated self-alias
initializer; its three-error evidence is retained in
`.tmp/outer-reference-v337-baseline.log`. Removing that incidental initializer
leaves the intended one-error nullable-handle baseline in
`.tmp/outer-reference-v337-isolated-baseline.log`; no target behavior check was
removed. The initial physical-slot correction compiled but exposed an
uncatchable null panic (`.tmp/outer-reference-v337-projected.log`).

The paired general fix now works: shared companion receiver lowering prefers
the exact stored instance/outer field shape, and shaped exception analysis
seeds lexical outer fields using the same ShapeEnv binding/hiding mechanism
and solved slots as lowering. The nullable receiver now participates in the
existing fallibility fixpoint rather than panicking through an infallible ABI.
`tests/outer_reference_contracts.rs` and the fixture are permanent regressions.
The initial paired batch passes all 3 executable tests across that target and
anonymous_field_contracts; `.tmp/outer-reference-v337-batch-tests.log`.

Selective profiling used frozen V335 and existing NEW_JAVA2RUST_PROFILE_LOWERING
with all 14 analysis inputs but `--emit-primary-file` restricted to Quaternion,
no repeated resolver benchmark. Log `.tmp/numbers-quaternion-v337-lowering-profile.log`,
output `.tmp/numbers-quaternion-v337-lowering-profile`. This diagnostic filtered
output has non-emitted dependency placeholders and is **not** a valid corpus
deliverable/acceptance artifact. It identifies Quaternion.toString's nine-
append statement as dominant: 75934 and 78885 ms in its two emitted bodies,
out of 179318 ms source-type lowering. Other method bodies are at most 2497 ms.

Small isolated 4/6/8-append diagnostic sources reproduce superlinear cost on
V335: source-type lowering **3086/11533/30745 ms**, wall **16.49/24.09/48.65 s**.
Fixtures `.tmp/BuilderChain{4,6,8}.java`; logs/outputs/times
`.tmp/builder-chain-v337-{4,6,8}.*` and `*-time.txt`.
V337 adds a positive native-builder return-shape proof at shape inference:
unresolved one-argument append on an admitted native builder returns that same
required builder carrier. It reuses java_append_plan and EffectiveFacts native
text admission, preserves resolved declarations and source/user lookalikes,
and avoids repeated generic recursive prefix planning. No global cache or
numerical shortcut was added.

Candidate producer `.tmp/new-java2rust-numbers-default-v337-outer-chain-debug`
SHA256 `4421f543e97a0f06f13b345db48a7aab870bc402e4ceb870bcab36c95ef0ecd0`.
Same-source/profile post-change lowering is **911/2435/3962 ms**, wall
**12.63/13.76/17.99 s**, respectively. Evidence
`.tmp/builder-chain-v337-new-{4,6,8}.*` and `*-time.txt`; build log
`.tmp/outer-chain-v337-producer-build.log`. This proves the targeted improvement,
not linear complexity or a whole-corpus speedup claim.

Final focused batch is terminal green: **2 anonymous-field**, **2 native append**
and **2 outer-reference/builder-chain** executable tests; log
`.tmp/outer-reference-v337-chain-batch.log`. BuilderChainProbe exercises the
actual formatting idiom, allocation identity and unpaired UTF-16 units; native
append tests preserve source lookalikes, checked display callbacks, evaluation
order and aliases. No new full-corpus or full-library success is claimed yet.
The accepted 4/6 checkpoint remains V335. Next coherent implementation batch is
enum naming, callback constructor slots, nullable enum switch/argument
projection and short receiver borrows; follow with the text/exception boundary
batch. Rerun full Quaternion and the full library at substantial boundaries,
not after each individual change. Reaudit any new families and explain a real
representation redesign before expanding scope.

### V338 enum/callback contract batch

Implemented the related source-enum fixes through shared mechanisms:

- `java_naming::source_enum_field_accessor_name` uses the existing identifier
  allocator and declaration lookup, reserving emitted method/runtime names and
  other field spellings. Initialized payloads and legacy computed/state
  accessors, plus lexical/qualified reads, share this mapping. Updates retain
  the field's setter name rather than deriving a setter from a renamed getter.
- Enum constructor shells now pass through the existing representation-aware
  constructor signature projection **before** adding the synthetic enum
  identity parameter. This keeps functional domains (source handles and boxed
  primitive representations) from being erased into default Any arguments.
- Switch case labels use enum discriminants, not publication-aware constant
  reads. Nullable source-enum selectors still have a checked null projection
  before matching/default; shaped exception analysis identifies switch-selector
  sites even when the body has no other call, using body-local addresses only.
  Ordinary enum constant reads remain publication-aware.
- The existing scoped shared-field reader, previously admitted for primitive
  `this` fields, now admits source enum/optional-enum values too. A copied enum
  slot leaves the Quaternion owner guard's scope before callback dispatch;
  no object payload or alias identity is copied, and no second borrow framework
  was introduced.

`EnumCallbackProbe` and `tests/enum_callback_contracts.rs` permanently exercise
typed ToIntFunction/BiPredicate enum payloads, callback reentry through a shared
mutable Entry, field/method name collisions with occupied suffixes, mutable
payload updates, repeated calls and alias identity, null enum-field switches,
null method receivers and restoration. It additionally checks a nullable enum
parameter switch with a default case: null must throw NPE, never select default.
These are executable Java/generated-Rust contracts, not string-only assertions
or manual production rewrites.

Initial focused batch passes **8 tests** (1 new callback, 4 enum initialization,
3 enum lookup), log `.tmp/enum-callback-v338-focused.log`. After adding the
nullable-parameter check, the substantial-boundary verification is terminal
green: **1705/1705 library tests** (162.01 s under concurrent corpus work) plus
the expanded callback integration test (28.10 s), log
`.tmp/enum-callback-v338-library.log`.

Frozen V338 producer:
`.tmp/new-java2rust-numbers-default-v338-enum-contracts-debug`, SHA256
`1a00da3b56dac96d4f40b9403c1e2441b31a015af77efcff7daf389aa1670209`;
build log `.tmp/enum-callback-v338-producer-build.log` (78 s), hash record
`.tmp/enum-callback-v338-producer.sha256`. Complete untouched Quaternion+Core
regeneration is running at
`.tmp/commons-numbers-1.3-quaternion-v338-enum-contracts`, staging
`.tmp/numbers-quaternion-v338-enum-contracts-staged`, outer log
`.tmp/numbers-quaternion-v338-enum-contracts-run.log`. Invocation uses the normal
14-file `NUMBERS_MODULE=quaternion` runner with `--skip-resolver-bench` only:
the redundant diagnostic preflight was already measured in V336, while actual
global parse/resolve/semantic/representation/signature work and complete source
emission/checking still run. No production file or acceptance gate is skipped.
Await terminal compiler output before assigning a new error count or starting
the next text/exception batch. Historical acceptance remains **4/6 on V335**;
V338 has not yet proved complete-module or earlier-module/RNG preservation.

V338 complete-corpus job is now terminal: all 14 files translate and parse,
with zero unsupported IR, invalid patterns, stubbed constants and reachable
no-op facade calls. Rust compilation improves from **50 to 7 errors**. The
remaining diagnostics are six boundary families, not 7 independent algorithm
bugs: one nonexistent Precision::equals overload target, one owned BigInteger
append conversion, one required constructor enum argument still receiving the
published Option, two split AsRef<str> bounds on logical UTF-16 strings, one
missing init_cause, and one phantom NumberFormatException facade impl. See
`.tmp/commons-numbers-1.3-quaternion-v338-enum-contracts/{summary.tsv,compile.log,top-compile-errors.tsv}`.
No runtime-oracle success is inferred from the compile-error reduction.

The same whole-module output confirms the intended enum repairs: norm_/norm_sq_
payload getters coexist with source norm/normSq methods; __java_enum_new keeps
all Quaternion/f64 native functional arguments; switch labels are single-level
Some(discriminant); and norm/normSq copy the optional Type slot in a scoped
field-read block before dispatch, without retaining the Quaternion mutex.
The actual generated Quaternion file took **76540 ms** to emit versus 412888
in V336. Total runner 451174 ms (translation 406563, assembly 34607, check
3224); concurrent library work and skipped duplicate preflight mean this is
not an isolated whole-run performance comparison.

Diagnostic snapshot `.tmp/quaternion-v338-before-assembly.rs` preserves the
actual completed producer output before assembly, without edits. It proves
Precision::equals is already wrong in the owning Default.__static_IS_UNIT
initializer before any merge/hoist, so the next fix must trace typed native
functional result/boxed argument overload resolution in that initializer,
not blame the assembly winner policy without evidence. The other remaining
required-enum argument is in the ordinary Quaternion.of factory's nested new
expression; enum payload constructor signatures themselves now compile.

All V338-owned jobs are terminal; formatting and diff checks pass. Next batch
should close these shared conversion/initializer/exception boundaries with
focused executable probes, then remeasure the complete scope. Overall goal
remains active, **4/6 accepted**; full Quaternion behavior (the 4532-observation
caller and translated client), preservation gates, Quaternion lifecycle audit
and Angle are still required.

### V339 typed initializer and argument/text boundary batch

The next coherent batch closes three independently reproduced boundary roots:

- Explicit constructor argument lowering now consults the existing
  publication-aware enum-field-to-slot adapter before nominal requiredness can
  bypass the actual optional constant carrier. Required and nullable constructor
  slots share the adapter; null and enum identity remain distinct.
- Required mapped foreign values in Java text contexts dispatch the declared
  mapped `toString` through ordinary call lowering. The existing pack ABI,
  ownership and result/text adapters remain authoritative. A no-Rust-Display
  foreign fixture with renamed UTF-16 `toString` proves this is not an invented
  Rust formatting requirement or a hardcoded BigInteger conversion.
- Field/local initializers and assignment contexts reuse the resolver's existing
  expected-type lambda resolution, already used for returns and callback
  arguments. The cross-file static BiPredicate reproducer selected a nonexistent
  overload before this fix; its boxed primitive/native functional domain now
  resolves the actual double overload. Anonymous-class fields use the same hook.

Focused executable results are terminal green: 4 boundary contracts, 1 expanded
enum callback contract and 3 mapped-text contracts, log
`.tmp/boundary-v339-typed-initializers.log`. Earlier in the batch, all 7
BigInteger contracts passed (`.tmp/boundary-v339-first-batch.log`). That initial
run also exposed an incorrect inherent-vs-free-function ABI in the custom test
runtime; the fixture was corrected to its declared mapped UTF-16 ABI, not the
product ABI changed to accommodate it. The intermediate overload reproducer
also found a separate fresh source-object/native-SAM carrier mismatch; the
overload test uses static values to isolate resolution. That additional family
is recorded, not claimed fixed or excluded from any production corpus gate.

Frozen producer `.tmp/new-java2rust-numbers-default-v339-typed-boundaries-debug`,
SHA256 `9638c4c75d8fd51ab51ef417e014d92a100776eb028fa7ce4ac7d4109b737760`;
build log `.tmp/boundary-v339-producer-build.log`. The complete untouched 14-file
Quaternion+Core measurement is running with that producer at
`.tmp/commons-numbers-1.3-quaternion-v339-typed-boundaries`, outer log
`.tmp/numbers-quaternion-v339-typed-boundaries-run.log`, using the same normal
runner and `--skip-resolver-bench` as V338. No new whole-corpus error count,
full-library pass or module acceptance is inferred before terminal evidence.
Earlier accepted scope remains 4/6 on V335.

The V339 whole-corpus job is now terminal: 14 files translate and parse with
zero unsupported IR, invalid patterns, stubbed constants or reachable no-op
facade calls. Rust compilation has **4 errors**, down from 7: two logical-text
split bounds, missing `init_cause`, and an impl for nonexistent
`NumberFormatException`. All three intended V339 root families disappeared
from the actual complete generated output. Logs and summary are under the
paths above; total 242974 ms, translation 207827 ms, assembly 26008 ms, cargo
3384 ms. This is evidence of progress, not Quaternion runtime acceptance.

### V340 literal split boundary (in progress)

Quaternion.parse uses the source `static final String FORMAT_SEP = " "`.
Its input and malformed-token error messages can contain arbitrary UTF-16.
The required hypothetical Rust rewrite is a UTF-16 delimiter-range split,
feeding existing identity-preserving substring construction, not a lossy
`AsRef<str>` bridge. This is a local native-intrinsic boundary fix; the array
element, source signatures and String storage already use the correct carrier.

The implementation positively specializes native String.split only for
nonempty, non-surrogate literal patterns proven by existing constant folding
and lexical source-field lookup. Local shadowing, source-owned split methods,
empty/escaped/metacharacter/surrogate patterns must not select this path.
The original argument and call staging remain intact. A shared UTF-16 kernel
uses existing substring identity, retains the whole receiver when no delimiter
is consumed, removes trailing empties only after a match with zero limit, and
preserves the original tail for positive limits. This does not claim or
silently replace broader regex support; other patterns retain the existing
regex route. A full UTF-16 regex-engine upgrade is not required by this
production path and is not folded into the batch without motivating usage.

Focused independent Java/generated-Rust coverage is running in
`tests/literal_split_contracts.rs` plus lossless String preservation, log
`.tmp/literal-split-v340-focused.log`. It includes raw surrogates, supplementary
characters, repeated delimiters, all limit signs/extremes, no-match/empty-input
identity and null receivers. Admission controls separately check that the
specialization does not reinterpret unproven patterns or source lookalikes.
No V340 pass or frozen producer is claimed yet.

The first focused run is terminal red for the new split tests, while all four
existing lossless String contracts pass. Evidence exposed two additional
whole-path requirements rather than a kernel-only repair: the later
`retarget_native_java_string_call_from_shape` must preserve/reestablish the
same proven specialization, and a fresh native split array must use the
existing fresh-array destination allocation adapter when solving requires
shared storage. Both paths now reuse those existing mechanisms; the fixture
continues returning/retaining arrays rather than dropping the failing case.
Initial evidence remains `.tmp/literal-split-v340-focused.log`.

The phantom exception impl is also traced to its producer: final inherited
external-super surface synthesis runs after mapped-facade suppression and
reintroduced methods on the erased Java superclass name. It now consults
`external_packs::embedded_external_super_rust_name`, the same authority used
for actual embedded storage. The executable JavaException carrier is reserved
from opaque facade synthesis, like other executable builtins; its existing
message/cause methods remain the callable implementation. No new alias,
unimplemented body or file exclusion is used. A source-exception executable
probe covers two different platform bases, nullable/raw-UTF-16 messages,
message reference identity and typed catches, both with and without the mapped
math pack that exposed the original final-surface reintroduction.

The corrected coherent batch is running the full library suite plus split,
exception-text and lossless String integration tests, log
`.tmp/literal-split-v340-carrier-batch.log`. Constructor `initCause` is still
unimplemented: the present value-carried JavaException cause API cannot simply
pretend to provide arbitrary post-publication Throwable mutation/identity.
The observed production idiom is an immediate, single prepublication
initialization after `super(message)`; a constructor-state lowering proof may
support that general idiom without claiming a full mutable Throwable redesign.
Any such proof must exclude already-initialized cause constructors, source
overrides, constructor escape/earlier instance effects and repeated/self cause
cases rather than silently installing an unchecked public initCause setter.

V340 carrier-batch verification is terminal: **1705/1705 library tests pass**
(127.77 s), existing exception-message contract passes and all four existing
lossless String contracts pass. The new split and source-exception integration
tests remain red in that checkpoint. A diagnostic frozen V340 producer is
`.tmp/new-java2rust-numbers-default-v340-literal-carriers-debug`, SHA256
`14c1e35c422ee7024207d8cc31ca8250fa4ff9ba2f31c0f705be68193e748ae3`;
it is not an accepted or fully green producer.

### V341 completing the text-carrier batch and preparing client coverage

The remaining focused failures identify three concrete details:

- A checked nullable String receiver yields `&JavaString`. Staging its borrow
  with the existing call-argument temporary mechanism before the helper call
  prevents Rust expected-type inference through `?` from demanding an owned
  String. It preserves evaluation order and later uses without a payload copy.
- Throwable types must be reserved from earlier opaque-facade discovery as
  well as the final inherited-super pass. The builtin facade predicate now
  consults the same existing throwable-carrier classification used in type
  positions, instead of creating a second NumberFormatException value type.
- Qualified constant proof uses existing primary-first source declaration
  lookup. A merged support clone of the same current declaration must not make
  a qualified reference ambiguous merely because multiple context programs
  contain it; genuine source-name ambiguity still rejects specialization.

The new cases are retained, not removed from the test suite. Focused split,
source/native exception and lossless-text verification is running at
`.tmp/literal-split-v341-focused.log`; no final V341 green result is claimed yet.

Added `QuaternionContractsOracle.java` and a caller-only Rust launcher for a
translated Java-client gate. Its 35 failure guards cover ordinary operations,
normalization metadata/identity, input/output array copies, null and invalid
inputs, raw-surrogate parse-error causes, constants, Slerp direct/callback
dispatch and repeated calls. It calls production algorithms; no replacement
quaternion algorithm is embedded. Independent Java compilation/execution with
all 14 pinned production files passes: `contract=0`, `oracle=ok`, log
`.tmp/numbers-quaternion-contracts-java-oracle.log`. This does not claim that
the client has translated or that the separate 4532-observation comparison has
passed.

The existing generic contract runners now explicitly handle Quaternion's
Core dependency closure, recording all 14 production paths and the separate
test caller, enforcing exact source counts, pinned source validation and byte
comparisons. Core/Complex selection is retained. The next complete generated
measurement should include this client, not hide public null-argument or
callback boundaries behind a Rust-only caller with closed-world non-null ABIs.
Audit any newly exposed families before another implementation campaign.

V341 verification is terminal: both literal split tests pass (including the
admission controls), all four existing lossless String contracts pass, and the
existing exception-message contract passes. The added source-exception probe
no longer encounters phantom facade types, but exposes two Option-vs-required
message comparison errors on inherited getters. Resolved-IR diagnostics
`.tmp/source-exception-v341-resolved-ir.log` confirm these calls remain
unresolved source-subclass calls; emission projects to the embedded native
base, while expression-shape fallback has no matching native receiver name.
The identity assertions are retained. Core/Complex independent Java client
gates also pass using the generalized runner, logs
`.tmp/numbers-{core,complex}-contracts-runner-v341-java.log`.

### V342 inherited throwable accessor result contract (in progress)

The hypothetical rewrite for the source-subclass getter mismatch is not a
new message representation: the emitted getter already returns the correct
`Option<JavaString>`. Shape analysis must select that same inherited native
accessor contract before consumer comparison/slot conversion. This is a
cross-function contract fix through existing hierarchy facts and native
lowered-type construction, not a special equality patch or corpus exception.

`stdlib::native_throwable_accessor_contract` now owns the existing four
accessor result/nullability pairs previously repeated in native return-type
and nullability tables. Those tables and inherited expression-shape fallback
reuse it. Fallback requires a positively known native throwable ancestor,
excludes parsed-source ancestors, follows source-signature lookup, and respects
explicit parsed-source/user override contracts. An added source getMessage
override control checks that a source body still wins. No arbitrary external
method is reinterpreted as a throwable accessor.

Verification is running the full library plus the source/native exception,
literal split and lossless String contracts, log
`.tmp/literal-split-v342-inherited-contracts.log`. The next complete baseline
includes all 14 untouched production files plus the explicit translated Java
client, staging `.tmp/numbers-quaternion-v342-contracts-staged`, output
`.tmp/commons-numbers-1.3-quaternion-v342-contracts-baseline`, outer log
`.tmp/numbers-quaternion-v342-contracts-baseline-run.log`. Its producer is a
frozen candidate, not accepted evidence before terminal gates. Additional
public-null/callback failures must be audited rather than ignored to recover
the smaller production-only count. Overall acceptance is still 4/6 on V335.

Frozen V342 candidate:
`.tmp/new-java2rust-numbers-default-v342-inherited-text-debug`, SHA256
`e2a0a0c2c190cb8bc56e978787217638ddbfa85a7d30a214ef6345bc725d20b6`.
The baseline runner has validated the upstream pin and recorded exactly 15
manifest entries (14 production plus the client). This is not a terminal
compile or behavior result.

V342 is now terminal. The full library passes **1705/1705** (107.78 s), both
literal-split tests pass, and all four existing lossless String tests pass.
The source-exception probe succeeds without the foreign pack, including its
source override control, but fails with the mapped pack on one inherited
getMessage identity comparison. Existing native exception text behavior passes.
The follow-up priority-only V343 test still reproduces that one error; it is
not recorded as a fix.

The complete V342 **15-source** baseline also finishes: all 14 untouched
Quaternion+Core production files and the Java client translate and parse,
with zero unsupported IR, invalid patterns, stubbed constants or reachable
no-op facade calls. Compilation now has **one error**, missing `init_cause`
in the source parsing-exception constructor. There are no residual split or
phantom exception type diagnostics, nor additional public-null/callback client
compile errors at this checkpoint. Total 254782 ms; translation 223994 ms,
assembly 21320 ms, cargo 3266 ms. This is a compile baseline, not a behavior
pass; the 35-check client has not yet run as Rust.

### V344 declaration-vs-compatibility signature priority (in progress)

Rather than keep changing fallback order speculatively, a standalone host
diagnostic reconstructs semantic facts, representations and signatures for
the mapped exception probe. Evidence
`.tmp/source-exception-v343-shape.log` proves the same inherited getMessage
call has `Optional<String>` without signatures but required, erased-view
String with signatures. Its only source-owner API fact is conservative with
unknown return/nullability; the precise mapped NumberFormatException fact is
nullable. The false authority is the legacy Rust-name compatibility signature,
not the actual source declaration or the physical native getter.

`SignaturePlan::has_declared_java_method_family` reuses the existing Java
declaration-key index. Unresolved source-call shape lookup lets a proven native
inherited accessor precede legacy compatibility entries only when no actual
Java source family owns the call. Genuine source signatures and explicit
source/user contracts retain precedence. The source override regression stays
in the fixture. Focused verification is running at
`.tmp/exception-text-v344-declaration-priority.log`; no pass is claimed yet.

Remaining constructor feasibility plan: the actual generated parsing-exception
constructor is a straight-line `super(message); initCause(cause);` followed by
publication. A sound, general constructor reduction can reuse the existing
platform message-and-cause construction path, without adding an unchecked
public Throwable setter. Admission should prove a direct native exception
base; an uninitialized-cause zero-argument/String constructor (not cause-only,
message-and-cause or `this(...)` delegation); exactly one immediately following
discarded native initCause call; no source initCause override, intervening
instance initializer/effect or escape; and a cause parameter/null, never this.
Only the proven trailing call may be subsumed, preserving message evaluation
before cause capture and all source-class tagging. Check nullable cause
conversion through the existing constructor adapter too: generated exception
support already implements `From<&JavaException>` by cloning the runtime
carrier. Reuse that adapter rather than inventing a redundant nullable-cause
conversion. Test ordinary source subclasses,
null message/cause and raw UTF-16 cause messages. General post-publication
Throwable mutation, repeated/self initCause and exception identity are not
automatically implemented by this reduction; explain any required broader
representation change before undertaking it.

V344 is terminal red on the same single mapped getMessage comparison. Its
declaration-family wrapper was not a valid proof: the wrapped family API
deliberately includes synthetic exact-Rust-name compatibility entries. The
attempted wrapper and consumer bypass are removed rather than retained as an
unverified second ownership mechanism.

### V345 correcting the inherited signature producer (in progress)

Tracing beyond the compatibility lookup identifies
`signature::external_inherited_method_signature`: it constructed the return
from the fact's bare nominal Java type with no nullability/native-result
constraints. Thus a mapped inherited native getMessage acquired required,
erased-view String even though the executable embedded carrier returns
`Option<JavaString>`. This is the producer of the conflicting signature shown
by the host diagnostic, not a new equality rule.

That producer now positively recognizes accessors on native throwable bases
and reuses the same shared native accessor/known-lowered-result contract used
by native declarations and shape fallback. Non-throwable compatibility APIs
and genuine source method signatures retain their existing paths. No separate
declaration-family predicate or consumer-specific bypass is needed. Current
focused verification is `.tmp/exception-text-v345-signature-producer.log`;
await its terminal result. No full-library or earlier-corpus preservation is
claimed for edits made after the V342 library run. The next substantial batch
gate should include this repair and the remaining constructor-cause work.

V345 focused verification is now terminal green: **2/2 exception text tests**
(56.01 s), including both native and mapped-pack source-subclass cases, raw
UTF-16/null message behavior, message reference identity, typed catches and the
source override control. This closes the additional regression without
removing its assertions. Formatting, shell syntax and diff checks pass. All
jobs started in this goal turn are terminal. The next batch remains the
constructor-only cause proof followed by complete runtime gates; acceptance
is still **4/6**, not inferred from the one remaining compile error.

Frozen V345 focused-green candidate:
`.tmp/new-java2rust-numbers-default-v345-inherited-signature-debug`, SHA256
`6dca3db1f8ae0b53d712a3c3685567e519998aceab85410710951dc4f9f8f4e8`.
It has not been substituted into the V342 baseline artifact or represented as
an already accepted full-library/full-corpus producer.

### V346–V347 prepublication cause constructor batch

The remaining production compile failure is addressed by a constructor plan,
not by adding an unchecked public Throwable setter. Structural admission in
`constructor_plan::prepublication_cause_initialization_candidate` requires
exactly `super(...); initCause(parameter-or-null);`, a discarded call on this/
super, the exact trailing expression scheduled by delegation, no intervening
effects/escape or instance field initializer, and no source initCause or
fillInStackTrace override. Semantic lowering additionally requires a direct
native exception base, a zero-argument/String super overload and a native or
source-embedded Throwable/null cause. Cause-only, message-and-cause and this
delegation cannot be mistaken for cause-uninitialized construction.

The matching superclass-state producer reuses
`lower_platform_exception_constructor` with message/cause arguments and the
existing source-runtime class tagging. Only the proven trailing call is
subsumed. It preserves message evaluation before the side-effect-free cause
parameter read, nullable message/cause conversion and logical text identity.
No mutable post-publication initCause API or new exception representation is
claimed. The native no-arg/String state and overridable stack-trace hook were
checked against [OpenJDK Throwable](https://raw.githubusercontent.com/openjdk/jdk17u/master/src/java.base/share/classes/java/lang/Throwable.java).
Two cheap planner regressions retain positive receiver/null variants and
negative escape, observed-return, repeated/self cause, initializer, alternate
receiver, delegation and override cases.

V346 full verification is terminal: **1707/1707 library tests pass** (121.37 s)
and both literal-split tests pass. The new executable cause probe initially
fails on chained accessor typing, not constructor emission: the generated
constructors already construct the correct message/cause state, but outer
getMessage consumers lose the solved getCause result type.
`.tmp/exception-cause-v346-batch.log` retains that evidence.

The shared throwable-accessor shape path now uses the solved receiver shape
(recursing through existing producer analysis) rather than TypeEnv guesses,
and recognizes a direct native receiver as well as a proven native ancestor.
This keeps source.getCause().getMessage() nullable/logical through comparisons
and subsequent charAt calls. Source signatures and explicit contracts still
own precedence. No consumer-only unwrapping or removal of checks is used.

V347 focused exception tests are terminal green, **3/3** (55.36 s), log
`.tmp/exception-cause-v347-chained-results.log`. The cause probe verifies a
side-effectful message producer evaluated once, null messages/causes, ordinary
source subclasses of three platform exception families, unqualified/this/super
forms, raw-surrogate cause messages, message identity, typed catches and
repeated observations. Existing native and mapped source-exception controls
also pass. The full library result above belongs to V346, before the small
chained-receiver refinement; final preservation is still required.

The client retains its original 35 guards and adds a 36th callback-factory
guard. `asCallback(Slerp)` is translated Java, allowing the independent Rust
numeric caller to exercise generated source-to-SAM conversion rather than
constructing that adapter by hand. Independent Java client remains green,
`.tmp/numbers-quaternion-v346-contracts-java.log`. Added the complete
`QuaternionOracle.rs` caller and explicit numeric/client modes to the existing
strict verifier; the expected numeric count is the Java caller's unchanged
4532 observations. Comparator policy is unchanged; 11 existing comparator
tests pass (`.tmp/numbers-quaternion-v346-comparator-tests.log`). Neither Rust
runtime gate has passed yet.

The complete 15-source V347 regeneration is running at
`.tmp/commons-numbers-1.3-quaternion-v347-contracts`, staging
`.tmp/numbers-quaternion-v347-contracts-staged`, outer log
`.tmp/numbers-quaternion-v347-contracts-run.log`, using a frozen producer and
the normal complete runner with only redundant resolver preflight skipped.
Acceptance remains **4/6** until complete module behavior and preservation
requirements are met.

Frozen V347 candidate:
`.tmp/new-java2rust-numbers-default-v347-constructor-causes-debug`, SHA256
`e7846eb9fae7d14424ca5d17ae87d01ef07dc4f2f64a61e6976c0532268025b0`.

### V347 runtime findings and V348–V349 equality batch

The V347 complete 15-source producer run above is terminal green: all 14
production files plus the separate Java client translate and compile, with
zero parse/unsupported/invalid/stubbed-constant errors and no reachable no-op
facade calls. Total 338854 ms: translation 305616, assembly 24445, cargo 4131.
This is a compilation checkpoint, not module acceptance.

Strict client execution fails guard 16: independently returned vector arrays
are allocated separately, but their generated `==` compares contents. The
complete numerical caller produces all 4532 observations with 26 exact
differences (`.tmp/numbers-quaternion-v347-numeric-behavior/differences.txt`):
four boxed Double.equals errors (NaN and signed zero), ten VM-generated NPE
diagnostic strings, and twelve Slerp component-rounding observations (four
inputs repeated through direct/callback/repeat paths, 1–8 ULP differences).
Exception types and control flow match; factory copy results match exactly.
Neither runtime gate is accepted and the comparator has not been relaxed.

The V348 constructor-admission audit tightens the prepublication cause
reduction to native constructors proven to preserve ordinary nullable-message
and initially-unset-cause behavior. It reuses the platform exception hierarchy,
excludes legacy sealed-cause/specialized-message constructors, rejects source
overrides of both initCause and fillInStackTrace, and retains the existing
argument evaluation/storage/getter path. JDK bytecode evidence is retained at
`.tmp/exception-constructor-v348-jdk-bytecode.log`. All 1708 library tests and
all three executable exception tests pass
(`.tmp/exception-cause-v348-transparent-batch.log`).

V349 whole-path equality plan: boxed primitive producers already carry the
correct native primitive/optional value, and the erased runtime equality
kernel already implements canonical-NaN/distinct-signed-zero semantics and
wrapper-type distinctions. Admit canonical native boxed receivers to that
existing kernel, snapshot the receiver before evaluating its argument, then
perform the existing delayed null check. Parsed source/import ownership keeps
source lookalikes on their own dispatch path. For arrays, allocation/storage/
signatures already retain shared handles; admit shared array operands to the
existing ordered native reference-comparison path, not Rust structural `==`.
This is consumer admission plus evaluation staging, not a new array storage
or floating-point algorithm. Focused Java/Rust fixtures cover the observed
contracts, nulls, reassignment during RHS evaluation and source lookalikes.
Their first verification is running at `.tmp/value-identity-v349-focused.log`.

The V349 first test build exposed a test-helper lifetime error (fixed without
changing Java guards). The executable retry then passes the source-lookalike
control but exposes two related whole-path gaps: nullable array locals compared
to retained arrays are not themselves retained, and selected primitive-wrapper
factories lose their physical primitive result at some destination shortcuts.
The V350 batch therefore adds array comparisons to the existing unary storage
retention demands (no alias/value-flow edge between independent operands), and
finishes selected native boxed-factory producers before normal slot adaptation.
Boxed equals consumes the selected physical receiver result, not an external
nominal summary. All original fixture guards remain intact. Verification of
the new three-test group and existing object-equality integrations is running
at `.tmp/value-identity-v350-batch.log`; no corpus regeneration has run yet.

V350 verification finishes with six of nine existing object-equality tests
passing; three report code 1 at source `instanceof String` checks. Cargo stops
before the new group, so it has no V350 result. The frozen V347 producer
independently exhibits the same mismatch: JavaString is boxed by the producer,
but its guard asks Any for Rust String (`.tmp/erased-object-v347-baseline.rs`).
This predates the equality batch. V351 fixes the shared erased-runtime target
helper for logical String by asking the existing owning-slot renderer, so both
type tests and downcasts follow the same String/JavaString carrier policy.
The source ownership/type policy is unchanged. The boxed fixture additionally
checks Quaternion's exact production cast-and-equals idiom. Verification of
both groups now uses `--no-fail-fast`, log `.tmp/value-identity-v351-batch.log`.

V351 is terminal: all nine prior object-equality integrations pass, including
multi-module assembly. New array identity and source-lookalike probes pass.
The boxed probe has one remaining compile error in its side-effectful Object
argument producer: the external-call return shortcut emits a native scalar but
labels it already erased, bypassing ordinary slot boxing. V352 carries the
selected intrinsic's primitive result ABI through that shortcut, using the
same authoritative native-call result policy as planned method-call shapes.
No Java guard is removed. Focused verification is running at
`.tmp/value-identity-v352-batch.log`.

V352 focused verification is terminal green: **9/9 existing object-equality
integrations** (40.52 s) and **3/3 new value/identity integrations** (20.48 s).
Independent javac execution and generated Rust agree on every unchanged
fixture guard, including production-style boxed FP equals, evaluation order,
null handling, factory/Object returns, array aliases and source lookalikes.
Formatting and scoped diff checks pass. A full library run and complete
Quaternion regeneration/runtime/preservation gates remain required for this
batch; do not attribute V348's full-library result to V352 or mark Quaternion
accepted from these focused results. Acceptance remains **4/6**.

Consistent with the user's FP scope, Quaternion's real component-wise boxed
equals call motivates fixing its NaN/signed-zero behavior. Slerp's small
rounding differences remain diagnostic evidence pending a practical accuracy
assessment, not a demand to emulate JVM elementary functions bit for bit.
No speculative/extreme-only FP case is added as an acceptance blocker.
For context, the pinned upstream SlerpTest uses EPS=1e-7 for each quaternion
component and rotation/vector checks. The 12 recorded strict Slerp difference
rows have maximum component absolute difference 2.220446049250313e-16. This
is accuracy evidence, not a comparator-policy change or proof of every
algorithmic invariant; source/control/identity/exception checks remain separate.

### V352 complete regeneration, runtime evidence and lifetime audit

The previous goal turn made implementation progress (V352: twelve focused
tests green). This continuation verifies the substantial batch rather than
redefining acceptance around those tests. Full library **1708/1708 passes**,
114.88 s, `.tmp/numbers-v352-library.log`.

Frozen producer `.tmp/new-java2rust-numbers-default-v352-value-identity-debug`
has SHA256 `062dfe37f9d44681642af04e64bcaac95de15e9abe849a9ff3b36d6473968fd6`.
The complete 15-source Quaternion+Core/client regeneration is terminal with
zero compile/parse/unsupported/invalid/stubbed-constant errors. Broad legacy
inventory still reports 133 unmodelled types, 20 calls and 62 conversions; these
are not evidence that all incidental runtime facades are implemented.
Output `.tmp/commons-numbers-1.3-quaternion-v352-contracts`, staging
`.tmp/numbers-quaternion-v352-contracts-staged`, outer log
`.tmp/numbers-quaternion-v352-contracts-run.log`. Total 307709 ms, translation
268291, cargo check 4387. Use the same complete contract runner with the frozen
producer, debug profile, offline Cargo/TMPDIR environment and only redundant
resolver preflight skipped. The producer provenance and full source manifest
are retained by the runner.

Both runtime jobs are terminal, not accepted. The client advances from guard
16 to guard **32**. All 4532 numerical observations plus footer are produced;
the **four genuine boxed-equality differences are gone**. Strict comparison
retains **22 differences**: ten implementation-specific automatic NPE messages
and twelve previously recorded Slerp rounding rows. All other observations
match. Evidence `.tmp/numbers-quaternion-v352-{client,numeric}-behavior` and
matching `-run.log` files. No comparator change, tolerance exemption or removal
of Java guards is made.

Guard 32 whole-path audit: the generated source-call argument is
`Some(Box::new(Option<Arc<Mutex<Quaternion>>>))`, whereas Java Object receives
null or the Quaternion reference itself. The native callback argument path
already maps the Option element correctly. The selected emitted source method
ABI is optional, but the external-call slot shortcut accepts it only when its
earlier external summary already has the generic Slot variant. V353 admits
the authoritative emitted source ABI regardless of that external-summary
variant, then reuses normal element-wise optional/dispatch conversion. New
`NullableSourceEqualityProbe.java` covers direct/callback results as equals
arguments, null preservation, Object copies, type tests and erased calls.
No source or generated production algorithm is rewritten. Focused execution
of four new and nine existing integrations is running at
`.tmp/value-identity-v353-nullable-source.log`.

Independent geometric diagnostic `scripts/audit-quaternion-interpolation.py`
uses mpmath 1.3.0 at 80 decimal digits to check all 96 observed interpolation
outputs for finite positive-polar unit results and exact callback/repeat
agreement, plus 48 spherical observations for plane/angular-position residuals.
It verifies scalar geometry, not a second translation/interpolation algorithm,
and is not a comparator portability policy. With residual limit 1e-14, both
Java and Rust pass: maximum squared-norm residuals 2.14e-16 / 2.27e-16,
plane residuals 2.72e-17 / 3.63e-17, angular residuals 1.28e-16 / 1.07e-16.
`.tmp/quaternion-v352-geometric-audit.json` records input hashes, precision,
version and metrics; the V347 report is also retained. Diagnostic negative
probes reject incomplete/duplicate output, wrong phase, wrong plane, infinity
and wrong norm. This supports treating the small backend rounding differences
as accuracy diagnostics, without concealing discrete/source-state failures.

The caller-only lifetime probe `.tmp/quaternion-lifetime-probe.rs`, linked
against unchanged V347 production artifacts, confirms that both Linear and
Spherical Slerp owners remain alive after the owner and external callback are
dropped (`.tmp/quaternion-lifetime-v347-probe.log`). It also verifies that an
external callback remains usable after dropping the explicit owner. Current
cycle: owner → SAM closure → inner strategy → strong outer owner.

Architectural options were surfaced to the user before implementation. A weak
outer alone is unsound for a generally escaping inner callback. A bounded
alternative is a capture plan that copies initialized final-field reference
carriers instead of the whole outer, only when analysis proves no outer
identity, outer-method dispatch, mutable-field access or observable outer
lifetime dependency. For this source the required inputs are final start/end
Quaternion handles, read after assignment in every constructor path. The
hypothetical rewrite stores those same handles in Linear/Spherical and uses
ordinary field projections; it does not copy Quaternion payloads. General
implementation must coordinate capture facts, synthesized fields, constructor
handoff, outer-field projection and initialization proof, and must retain the
current outer capture when proof fails. Transitive back-references and lifetime
observers remain explicit analysis concerns. No weak-reference substitution or
capture redesign has been implemented pending the requested direction.

Primes and Root Finder V352 complete production preservation builds are
terminal green (three sources each); numerical callers also pass exactly,
**3594 / 122 observations**, at
`.tmp/numbers-{primes,rootfinder}-v352-preserved-behavior-run.log`. Other final
preservation gates and the lifetime issue remain open. Acceptance stays **4/6**.

V353 is terminal: the nine existing and three earlier new integrations pass,
but the new nullable-source regression fails guard 1. The initial admission
change alone is insufficient. Inspection of its unchanged generated output
shows the same Box<Option<reference>> defect as the production client. The
shared companion already attaches the correct solved result shape; the
external-call slot path discards that producer record. V354 consumes it before
argument-temporary wrapping and ordinary destination adaptation. No change to
the actual source-return representation or Object equality runtime is needed.

The broader fixture additionally exposes a late signature dependency: a
reference-returning call passed directly to an Object parameter has no local
binding, and the early syntactic return summary does not yet know that nullable
field storage will lift its return. V354 retains resolved argument-result
producers in method/constructor call usage. Directed MethodReturn→parameter
dependency edges schedule and propagate only outer nullable-storage demands
through the existing SCC solver and scoped storage mutation helpers. They are
not alias edges and do not copy producer concrete types, native allocations or
ownership back into unrelated parameters. Reference casts/conditional branches
retain result producers; primitive/unboxing boundaries and expanded-varargs
elements do not widen the wrong outer parameter. This is an additive
cross-function contract repair within the existing framework, not a separate
nullability solver or a corpus-specific call rule.

The fixture keeps all guards and adds constructor/Object storage checks.
Full library plus all thirteen focused integrations are running together at
`.tmp/value-identity-v354-producer-nullability-batch.log`. V354 is not frozen
or accepted yet; V352 preservation does not prove V354 preservation. Diagnostic
geometry checks also reject removal/duplication of actual interpolation rows
and callback/repeat divergence, in addition to the numerical negative probes
above. The current geometric report includes both caller hashes.

V354 is terminal: 1707/1708 library tests and all nine existing equality
integrations pass; the nullable-result fixture still fails guard 1. The single
library failure is the useful same-receiver argument-conversion guard, not an
unrelated assertion: newly propagated nullable parameter storage exposes an
invalid `Box<Payload> as Option<Box<dyn Data>>` cast. The new result dependency
does work: `same` now correctly accepts two nullable Object parameters. The
remaining failures are consumers, not a need for another producer model.

V355 consumer audit/plan: shaped and explicit arguments have early subtype/
reuse shortcuts before canonical slot lowering. Route nullable emitted source
call results through the complete slot adapter exactly once; these expression
results have no later local use requiring a second reference recovery pass.
The present-value subtype shortcut now declines nullable destinations, leaving
null mapping/wrapping to the existing optional coercion plan rather than
unconditionally unwrapping and casting a Box to Option. Same-receiver argument
staging preserves the selected physical producer shape and copies it into an
owned temporary, rather than reconstructing a nominal shape that forgets its
nullable carrier. These are shared boundary fixes, not quaternion methods.

The old structural reentrancy guard now requires null-preserving mapping and
the trait-object cast, forbids the invalid cast to an Option, and retains its
staging/companion/no-held-borrow requirements. Added executable
`NullableReceiverArgumentProbe.java` independently verifies same-object and
cross-object calls, present/null payloads, repeated writes and owner aliases;
this strengthens coverage beyond relying on a particular clone spelling.
The earlier nullable-source fixture keeps every guard. All fourteen focused
integrations are running at `.tmp/value-identity-v355-nullable-consumers.log`.
No claim of a green V354 library suite or a passing Quaternion client is made.

The lifetime diagnostic was also rerun against unchanged V352 output and still
confirms both cycles (`.tmp/quaternion-lifetime-v352-probe.log`). The requested
capture-design decision remains separate from these boundary repairs.

V355 finishes with the nine older and three original integrations green. The
nullable source fixture still fails guard 1; the new executable receiver probe
initially fails on pre-existing constructor self-publication (`alias=this`
before constructing its canonical handle), not on its argument conversion.
That separate source fragment is retained in the earlier library test/output
evidence. The focused executable probe uses ordinary post-construction aliases
to isolate the observed call/borrow/null contract; no pinned production source
or previously accepted executable guard is changed.

V356 introduces opt-in argument-ABI tracing
(`NEW_JAVA2RUST_TRACE_ARGUMENT_ABI=1`, cached once per process). Its two-test
run is terminal: the receiver/alias/null executable probe passes, while the
source Object-result probe still fails guard 1. The trace proves source owner
resolution and the produced/target optional shapes are already correct at
argument admission. Thus neither another nullability inference rule nor a new
equality kernel is justified. Log `.tmp/value-identity-v356-argument-abi-trace.log`.

The remaining bypass is earlier inside ordinary slot lowering:
`lower_concrete_external_call_for_erased_slot` precedes the source-call adapter.
When its external call metadata is absent, its fallback reconstructs a
non-null T from the resolved Java declaration, even for a known emitted
Option<T> source method. V357 makes that early erasure boundary consume the
same selected source method ABI and result-propagation policy before applying
the existing slot conversion. It does not invent new Object boxing or nullable
mapping. All fourteen focused integrations are running together at
`.tmp/value-identity-v357-source-erasure.log`. No full-corpus regeneration is
repeated until this consumer family has focused evidence.

V357 is terminal green: **all fourteen focused integrations pass** (nine
existing object-equality tests, 32.06 s; five value/identity tests, 15.35 s).
The new source/null/Object/callback/constructor guards now execute correctly,
and the post-construction same-receiver probe passes without held-borrow
failures or losing null/aliases. This confirms the early erasure boundary as
the missing handoff.

V358 removes the temporary nullable-source-argument routing helper and its
opt-in tracing. The saved V356 trace remains evidence, not a new permanent
debug API. Verification must demonstrate that ordinary argument conversion,
the authoritative source ABI at the early erasure boundary, the existing
optional subtype mapper and the scheduled result-nullability edge suffice;
no extra per-argument recovery layer is retained. Full library and all fourteen
integrations are running together at `.tmp/value-identity-v358-canonical-batch.log`.
Only after that build is confirmed current will its producer be frozen for
the next complete Quaternion regeneration. V352's strict numerical and
preservation records remain separate and are not attributed to V358.

V358 verification is now terminal green without the temporary routing layer:
**1708/1708 library tests** (90.15 s), **9/9 existing equality integrations**
(32.60 s) and **5/5 value/identity integrations** (21.51 s). The previously
failing structural reentrancy guard also passes. Log
`.tmp/value-identity-v358-canonical-batch.log`.

Frozen candidate
`.tmp/new-java2rust-numbers-default-v358-canonical-source-erasure-debug`, SHA256
`82d9a942f55cdc0882e6717a1eb186c18d28b3caa41ad37a2f975cf299d19f27`.
Complete Quaternion/Core/client regeneration is running at
`.tmp/commons-numbers-1.3-quaternion-v358-contracts`, staging
`.tmp/numbers-quaternion-v358-contracts-staged`, outer log
`.tmp/numbers-quaternion-v358-contracts-run.log`. It uses that frozen producer,
the normal complete contract runner, debug profile, offline Cargo/TMPDIR
environment and only redundant resolver preflight skipped. Do not restart an
active translation merely because an observation times out. Its compile and
runtime results are not yet known; acceptance remains **4/6**. The ownership
cycle, automatic-NPE text decision and remaining final preservation gates are
still separate open items.

### V358 runtime closure and capture feasibility experiment

The preceding goal turn made implementation progress; this continuation polls
its actual translation handle and collects new runtime/preservation evidence.
The complete V358 Quaternion/Core/client build is terminal green: all 15
sources, zero parse/unsupported/invalid/stubbed-constant/compile errors. Total
220732 ms, translation 193639 ms. This is the frozen V358 producer recorded
above; no source/algorithm editing is involved.

The unchanged translated client now passes **all 36 guards**, two exact output
lines (`.tmp/numbers-quaternion-v358-client-behavior`). The strict numerical
caller produces all **4532** observations with exactly the previous **22**
diagnostic differences, not the old boxed-equality failures. Its Rust output
is byte-identical to V352; no new state, identity, exception-type or arithmetic
divergence appears. The comparator still exits nonzero and retains every
difference at `.tmp/numbers-quaternion-v358-numeric-behavior/differences.txt`.
The separate 80-digit geometric audit also passes and records current hashes
at `.tmp/quaternion-v358-geometric-audit.json`. This is not yet acceptance:
the callback cycle and automatic-NPE message policy remain open.

While capture implementation awaits the requested direction, a strictly
isolated hypothetical-rewrite experiment has now been executed. Directory
`.tmp/quaternion-capture-diagnostic-v358` is explicitly marked DIAGNOSTIC-ONLY
and uses a different Cargo package/library name, not the translator verifier's
expected generated-library identity. It copies V352 generated source and
changes **only** `00014_Slerp.rs`: Linear/Spherical store the same nullable
start/end Quaternion handles rather than the whole outer; their constructors
copy those two final-field references, and sixteen field reads project them
directly. All interpolation expressions, method ABIs, class identities and
callers remain unchanged. `diff -qr` confirms no other generated source differs.
The diagnostic patch is `representation.patch`, SHA256
`6ab945f0e3bde395daac7c0b3f7bd2d1799d0875a83e8f5920d72dac5f4ff595`.

That diagnostic crate compiles (7.00 s). Its lifetime probe verifies, for both
Linear and Spherical, that the external callback still works after the explicit
owner is dropped, but the owner is no longer retained once that callback is
dropped. Its complete numeric caller output is byte-identical to V352 **and**
V358 (4532 observations, SHA256
`36ae894ce5cb202d07aad7488c81710cffdfcc9427c4425f17b49eda23c1de67`).
Evidence `lifetime-output.txt`, `numeric-output.txt`, `numeric-stderr.txt` and
`.tmp/quaternion-capture-diagnostic-v358-build.log`. This proves a small
representation rewrite can address the observed cycle; it is not an accepted
translation, translator fix, or substitute for general capture analysis.

General implementation still needs one authoritative capture plan consumed
by synthesized storage fields, constructor handoff and lexical field reads.
Required proofs include final-reference identity (not payload copies), field
assignment dominating each capture construction, no outer identity/method or
mutable-field requirements, and no observable outer-lifetime dependency.
In particular, constructing an inner object before assigning its outer's final
fields must not freeze default values; an escaping callback must not lose a
required owner. Source private strategy containment and rejection of unknown
finalization/lifetime observers are candidate bounded proofs, not assumptions
to silently apply to all inner classes. The production translator has **not**
been changed to implement this representation experiment.

V358 preservation of accepted modules is additionally terminal green:
Core's complete 11 production files plus its translated client compile; the
client passes and **7888** observations match under the unchanged
arithmetic-nan-sign-v1 policy (four strict differences retained). Complex's two
production files plus client compile; all 32 client guards and **1863** pinned
independent-reference requests pass. The strict full **14617** operations run
still has exactly **78** differences; both Rust output and differences are
byte-identical to the accepted V335 checkpoint. No bounds or comparison policy
has been changed. Paths `.tmp/commons-numbers-1.3-{core,complex}-contracts-v358-preserved`
and `.tmp/numbers-{core,complex}-v358-*-behavior`, with matching outer run logs.

The standalone 11-source RNG Source64 build also passes, with **2865 exact
observations** including floating-point bits, at
`.tmp/commons-rng-1.0-source64-v358-preserved` and
`.tmp/rng-source64-v358-preserved-behavior`. Complete 46-source sampling
preservation is still running at `.tmp/commons-rng-1.0-sampling-v358-preserved`;
fresh V358 Primes/Root Finder builds are also running at their corresponding
`v358-preserved` paths. Their pending states are not passes. Acceptance remains
**4/6** until all Quaternion requirements are closed; Angle has not begun.

V358 preservation is now fully terminal and green. Primes and Root Finder
retain their complete three-file scopes and pass **3594 / 122 exact output
lines**. Complete RNG sampling emits/checks all **46** files with zero parse,
unsupported, invalid, stubbed-constant, reachable-no-op or compile errors
(392303 ms total). Its unchanged native, collection-sampling and distribution
callers pass **12601 / 162 / 2172 exact lines**, including floating-point bits
and state/RNG traces. Evidence `.tmp/numbers-{primes,rootfinder}-v358-preserved-behavior`
and `.tmp/rng-{native,sampling-collections,sampling-distributions}-v358-preserved-behavior`,
with matching `-run.log` files. Core's four allowance records are also
byte-identical to the accepted V335 records. Every job owned by this
continuation is terminal. No preservation failure is being left as a pass.

Count clarification: the strict numerical verifier counts total output lines,
including `oracle=ok`. Quaternion has **4531 keyed observations plus that
footer (4532 lines)**; earlier journal shorthand calling these 4532
observations does not imply an extra observation beyond the recorded file.
No expected counts or caller checks have changed.

Replan after the executed representation experiment: the initial request for
direction was made while the need for a larger ownership redesign was still
uncertain. The measured small rewrite can be expressed by the existing
capture/field/constructor machinery. A conservative, general final-field
capture analysis and emitter integration are normal implementation within the
already authorized translator goal, not a new GC/JDK scope. Proceed with that
bounded implementation, reusing authoritative source identities and storage
shapes and retaining existing captures when proof fails; no user approval is
being inferred from a preselected/unanswered option. If the required proof
cannot be expressed without a meaningful framework/scope expansion, surface
that new evidence before expanding. The automatic-NPE message comparison
choice remains open; strict diagnostics and source-specified message checks
stay intact. Quaternion is still not accepted, and Angle remains unstarted.

### V359 bounded enclosing-environment implementation (verification pending)

The production fix now reuses capture facts and the solved field-storage ABI.
A private final contained strategy may store a tuple of final outer fields
instead of a strong outer handle, but only after a source proof establishes
initialization before every construction and rejects strategy escape, outer
identity/method use, shadowing, mutable outer fields and unknown control flow.
Reference captures feed the existing alias/storage propagation. Post-solve
admission accepts only primitive scalars or canonical source Rc/Arc handles;
native containers and value/erased carriers retain the old representation.
The same admission drives struct storage, constructor initialization and reads.
No numerical production algorithm or upstream source has been changed.

Lifetime/reflection/native observers disable reduction across merged units;
unproved static initializers also retain enclosing handles. New proof tests
cover safe references, fallback families, conditional initialization and merged
observer facts. An independent Java/Rust fixture additionally requires payload
alias mutation, a callback that keeps its owner alive, and owner release after
the final callback drops. These are pending tests, not claimed passes. The
library-only compile check passed (26.20 s); the first test build found an
existing visitor test call needing the newly explicit replacement argument.
That call is updated without changing its expected clone assertion. Quaternion
remains unaccepted pending executable and complete-source verification.

V359's complete library run passed **1712/1712** tests (83.97 s), including all
four source-proof tests. The enum callback and three native SAM integrations
also passed. Existing nullable-outer and builder contracts passed; the new
lifetime fixture failed at Rust compile, exposing a shared emitter bug rather
than a failed escape proof: `RustType::Tuple([T])` emitted `(T)` while tuple
values and field reads correctly used `(value,)` and `.0`. V360 corrects the
authoritative type renderer and adds explicit zero/single/multiple-arity type
regressions. Executable tests are rerunning at
`.tmp/outer-capture-v360-executable-batch.log`. No generated fixture was repaired
manually, and no lifecycle assertion was removed.

V360 executable verification is terminal green: **7/7** integrations (one
enum, three native SAM and three outer-reference contracts). The new lifetime
fixture verifies the translated callback keeps its owner alive and usable,
preserves shared mutable payload state, then releases the owner after the last
external callback drops. This is translator-generated output, not the earlier
manual representation experiment. V361 additionally detects fully qualified
lifetime API calls and method-local observer declarations, reusing the
existing qualified static owner resolver. The final library/outer-reference
batch is running at `.tmp/outer-capture-v361-final-batch.log`; the tuple arity
regression is included. Complete Quaternion regeneration remains pending.

V361's new tuple-arity and all four proof regressions pass in the final batch;
remaining library/executable checks are still live. The successfully built
producer is frozen at
`.tmp/new-java2rust-numbers-default-v361-final-field-captures-debug`, SHA256
`308b58061d91cc9824253071d40c122fe8ae0e8237945b0b23434d50a00c6df4`.
The full 14-production-file closure plus translated client is regenerating at
`.tmp/commons-numbers-1.3-quaternion-v361-contracts`, staging
`.tmp/numbers-quaternion-v361-contracts-staged`, outer log
`.tmp/numbers-quaternion-v361-contracts-run.log`. Command uses the normal complete
Quaternion contracts runner, that frozen binary, offline/debug/TMPDIR settings
and `--skip-resolver-bench`; no source exclusions or output repairs. Its runtime
and lifetime results are not yet known. Acceptance remains 4/6.

V361's final batch is terminal green: **1713/1713 library tests** (88.94 s)
and **3/3 outer-reference integrations** (38.94 s). The production Quaternion
Rust client now also asserts owner lifetime/release for both linear and
spherical strategies, without replacing production calculations or changing
its Java-comparison output. The old V358 artifact fails this strengthened
gate as expected at the retained-owner assertion; evidence
`.tmp/numbers-quaternion-v358-lifetime-negative` (the initial malformed CLI
invocation was corrected before this negative run).

Source inspection during the V361 regeneration found an over-conservative
observer restriction: Quaternion and Core DD implement Serializable. That
marker does not itself observe an owner's lifetime. V362 keeps actual
serialization calls/streams, reflection, weak references, finalizers and native
observers as vetoes, but no longer globally vetoes an unrelated Serializable
payload. A focused regression covers this distinction. The frozen V361
regeneration remains separate evidence; it cannot prove the intended capture
reduction while its marker veto is present. V362 focused tests are running at
`.tmp/outer-capture-v362-observer-tests.log` before a new producer is frozen.

V362's five focused proof tests pass (0.27 s); the executable batch has also
passed the nullable-outer and new lifetime/alias tests and is finishing the
builder contract. V361 complete generation terminated successfully with zero
compile errors, but generated Slerp strategies visibly retain Rc outer fields
as predicted by its Serializable veto. This is not lifetime acceptance.
Frozen V362 producer
`.tmp/new-java2rust-numbers-default-v362-final-field-captures-debug`, SHA256
`d7a1ff04fc5ef584a8ff5fc84340e3cd23f5b8aabddeca9766a538f0563118df`.
Complete Quaternion/Core/client regeneration is now running at
`.tmp/commons-numbers-1.3-quaternion-v362-contracts`, staging
`.tmp/numbers-quaternion-v362-contracts-staged`, outer log
`.tmp/numbers-quaternion-v362-contracts-run.log`, using the same normal
complete-source runner/settings as V361. The strengthened production client
adds lifetime assertions without changing expected output. Counts and upstream
source scope remain unchanged; neither this run nor its numerical gates is
claimed passing before execution.

V362 executable batch is terminal green: all **3/3** outer-reference tests,
including builder, live nullable fallback and cycle-free final-field capture
(37.10 s). V362 complete
translation handle is still live; poll that run rather than starting another.

V362 complete generation is now terminal green: all **15** scoped sources
(14 production plus translated client), with zero parse/unsupported/invalid/
stubbed-constant/compile errors and zero reachable no-op facade call sites.
Total 206218 ms, translation 179121 ms. Both generated private Slerp strategies
store tuples of the canonical start/end quaternion handles rather than strong
Slerp handles, through the translator change (not repaired generated code).
The strengthened client/lifetime verifier and unchanged strict 4532-line
numeric verifier are running at `.tmp/numbers-quaternion-v362-{client,numeric}-behavior`
with matching `-run.log` files. Their runtime outcomes are not yet known.

V362 runtime closure is terminal: all 36 Java-client guards and both new Rust
strategy lifetime/release paths pass (two exact output lines). Strict numeric
output has all **4532 total lines**, including footer, and remains byte-identical
to V358 (SHA256 `36ae894ce5cb202d07aad7488c81710cffdfcc9427c4425f17b49eda23c1de67`).
Its 22 retained strict differences are also byte-identical: ten VM-generated
NPE messages and twelve 1–8 ULP interpolation rows. The independent geometric
audit passes all 96 results/48 spherical cases, residuals 2.27e-16 norm,
3.63e-17 plane and 1.07e-16 angular position (limit 1e-14); report
`.tmp/quaternion-v362-geometric-audit.json`. No arithmetic or comparison
threshold was changed by the capture fix. The old V358 artifact fails the new
lifetime gate, confirming it catches the actual retained-owner defect.

An explicit portable-vs-JVM-helpful-NPE wording decision has been requested;
source-written exception messages, types and control flow remain exact.
Quaternion is not yet marked accepted. Preservation runs for all four earlier
modules and RNG source64/complete sampling are now started on frozen V362, each
with fresh `v362-preserved` staging/output/log paths. The attempted shortcut of
linking Core's existing caller to Quaternion's closure does not compile: that
closure lacks CoreContractsOracle's null/effect seeds and has a different
specialized ABI (76 Rust caller type errors). This is not a Core behavior pass
or a reason to patch its caller; the original complete Core+client runner is
being used for preservation instead.

V362 Primes and Root Finder preservation is terminal green on their complete
three-source scopes: **3594 / 122 exact observations**, including exception
messages, helper mutation, IEEE roots and callback traces. Complete standalone
Source64 generation also compiles; its 2865-observation verifier is now running.
Core, Complex and complete 46-source RNG sampling remain live, not passes.

Source64's **2865 exact observations** pass. Complete Core+client and
Complex+client regenerations also compile; original client/numeric/reference
preservation gates are now running on those artifacts, not the ABI-incompatible
Quaternion closure shortcut. RNG sampling is still assembling its full scope.

Additional concrete NPE portability evidence: the *unchanged Java* oracle,
executed with `-XX:-ShowCodeDetailsInExceptionMessages`, still emits 4532 lines
and changes only the same ten VM-generated message observations:
`factory.null.{pure,scalar}`, `parse.18`, `null.{add,subtract,multiply,dot,equals.eps}`,
`slerp.null.{start,end}`. All other output is identical. Artifact
`.tmp/quaternion-v362-java-no-helpful-npe.txt`; command uses the existing pinned
Java oracle classes. Thus these strings depend on a JVM flag, not a changed
production algorithm or source-written message. The requested policy decision
remains open; this experiment does not silently waive strict discrepancies.

V362 Core preservation is terminal green on its original complete 11-source
plus client scope: client passes and **7888** numeric lines match under the
unchanged arithmetic-nan-sign-v1 policy; all four allowance records are
byte-identical to V358. Complex's complete two-source plus client scope also
passes its 32 client guards and **1863** pinned independent-reference requests.
Its strict **14617**-line run retains exactly **78** differences; both output
and difference report are byte-identical to accepted V358/V335. No comparison
policy/bound was changed. Paths `.tmp/numbers-{core,complex}-v362-*-behavior`
and matching outer logs hold the independent runtime evidence.

Complete **46-source RNG sampling** compiles with zero parse, unsupported,
invalid, stubbed-constant, reachable-no-op or compile errors (230821 ms total).
Its native, collection and distribution runtime verifiers are running at
`.tmp/rng-{native,sampling-collections,sampling-distributions}-v362-preserved-behavior`.
Source64's separate complete 11-source runtime gate already passes 2865 exact
observations. These are normal complete regeneration/behavior commands on
the recorded frozen V362 producer, not reused V358 runtime claims.

All V362 preservation jobs are now terminal green. Complete RNG sampling's
native, collection and distribution verifiers pass **12601 / 162 / 2172**
exact lines respectively, including floating-point bits and state/RNG traces.
All four earlier Numbers modules and RNG source64/native/sampling retain their
recorded acceptance gates on this producer. No owned job from this continuation
is still running. Quaternion's implementation/lifetime failure is closed;
its strict diagnostic classification/policy decision remains before module
acceptance, and Angle has not begun. Goal remains active at 4/6 accepted.

### V362 strict NPE boundary audit (policy still unanswered)

The preceding continuation made concrete progress: production Quaternion
lifetime verification, numerical/geometric evidence and all preservation
gates completed. Automatic goal continuation is not an answer to the explicit
NPE wording question and is not permission to weaken the comparator.

Current remaining NPE failures cross three general operation families:
array-length reads (`of(double,double[])`), parameter field reads (static
arithmetic and epsilon equality), and parameter method invocation (`parse`
String.indexOf and Slerp positivePolarForm). The ten failures use parameters
as the immediate null origins, but JVM rendering also depends on original
Java operation/signature and parameter ordinal, not emitted Rust helper names.
Generated Rust confirms all these checks still emit the existing generic
`Java null receiver` exception through Option projections.

Architectural trace: `LowerCtx` retains original method/constructor declarations
and binding-aware ShapeEnv can identify parameters; resolved Java calls retain
original declaring type and argument types. However the shared low-level
null-check constructors receive a lowered Rust expression/projection, not that
source provenance. `coercion::JAVA_NULL_RECEIVER_EXPECT` is additionally an
internal discriminator used by optionality idempotence and raw/structured
projection recovery (including `java_optionality_already_discharged`,
`is_java_null_receiver_expect_args` and mutable alias recovery). Replacing
generic message literals would conflate error text with these structural
contracts and risks double projection. It is not a justified corpus fix.

A faithful general helpful-NPE feature therefore needs source dereference
provenance carried independently of error text through source, slot/conversion
and exception lowering, with evaluation-order/alias/shadowing tests and
conservative unknown-origin fallback. Full JVM-equivalent messages also need
an explicit compiler-debug-info/VM-configuration contract. No new provenance
campaign or output-specific replacement has been started while the requested
policy decision remains unanswered. Existing exact exception types,
source-written messages and strict reports are preserved.

The remaining full-library check on the *final V362 source* is now running at
`.tmp/outer-capture-v362-full-library.log` (the prior full 1713-test suite was
V361; V362's added marker distinction had focused proof/executable coverage).
This is a final batch check, not another corpus regeneration or a claim that
the unresolved strict message gate passed.

Final V362 library verification is terminal green: **1714/1714 tests**, zero
failures/ignored tests (87.30 s). The pinned upstream worktree is clean at
`ae8abefb51c2c4d1a05882d61cefc8015e493a8f`; the frozen producer still hashes to
`d7a1ff04fc5ef584a8ff5fc84340e3cd23f5b8aabddeca9766a538f0563118df`.
All owned jobs are terminal. This continuation added the concrete NPE
cross-layer audit and finished the current-source full library requirement;
it did not reinterpret automatic continuation as approval of portable
messages. The explicit policy/direction question remains before Quaternion
acceptance and sequential Angle translation. Goal is still active, not
complete or blocked merely because direction would resolve this ambiguity.

### V363 bounded source-parameter NPE diagnostic batch

Replan under the existing strict default: no portability exception has been
approved, but a bounded general source-parameter feature can be attempted
within the existing expression context and structured projection machinery.
This is not a campaign to emulate every JVM bytecode/debug-info configuration,
and it does not relax any strict gate. Unknown origins retain the existing
diagnostic rather than guessing a bytecode-local name.

The source expression/slot entry boundaries compute a parameter diagnostic
from original callable declarations and binding identities. It is temporary
lowering context, consumed by existing catchable/optional receiver adapters.
The new structured projection visitor updates only the generated default NPE
error body of a check whose operand still identifies that exact parameter.
It does not alter the internal expect/idempotence marker, arbitrary user
exception messages, other parameters, computed field origins or arbitrary
callback bodies. Java lambdas/method-reference frames conservatively disable
parameter ordinal diagnostics because javac synthesizes a different frame.
Source runtime names reuse the existing source-name mechanism; unresolved
overload descriptors decline rather than infer an incorrect Java signature.

First library compile check passed (26.67 s); the structured projection unit
test passes. A separate synthetic-callback frame regression is added. New
independent Java/Rust fixture `ParameterNpeProbe.java` exercises nullable array
length, a source field read with a Rust-keyword parameter, source/native method
invocation, normal repeated values and exact preservation of a source-written
NPE message. Its executable test is running at
`.tmp/null-diagnostic-v363-contract.log`; no production corpus is rerun until
this focused batch has executable evidence. Frozen V362 and all its acceptance/
preservation evidence remain untouched and separate from this unverified batch.

V363 focused execution passes array length and the keyword-named parameter
field case, but fails the source invocation assertion: accessor inlining
reclassifies the operation as a field read. Native String search also initially
has an unresolved overload descriptor. V364 added declared String search
parameter metadata (String versus widened int, optional int offset), verified
against the local JDK `javap java.lang.String` signatures. Its two focused unit
tests pass; the combined Cargo filter selected zero integration tests, so the
integration was rerun explicitly rather than claimed covered. A diagnostic
attempt to decline nullable getter inlining exposes an existing unchecked
`Option<&T>.read()` path and is not retained as the solution.

V365–V367 preserve the optimization by carrying source operation provenance
across synthetic field expansion. Compiler-generated NPE error values have a
typed RustExprShape tag created by the existing exception constructor; source
exceptions do not acquire it even if their prose is identical. Expanded-source
context preserves invocation origin before Raw emission can erase the tag.
Unknown operation context remains generic rather than falsely becoming a field
diagnostic. V366 consequently exposes the remaining source-name lookup issue:
combined context lookup declines duplicate primary/support declarations, so
the original invocation plan is absent. V367 uses the existing primary-first
`find_context_type_decl_by_erased_name` authority, not a new name matcher.
The native String diagnostic is now exact in generated focused output; the
source invocation regression is rerunning at `.tmp/null-diagnostic-v367-contract.log`.
No full production regeneration or acceptance-policy change has been made.

Diagnostic formatting here is checked against local OpenJDK/javac **19.0.2**,
default compiler debug settings and helpful-NPE messages enabled, not claimed
to emulate every VM configuration. A local JShell experiment confirmed the
multi-argument signature separator is `", "`; its default remote engine was
unavailable under the sandbox, and the local engine was stopped after emitting
the result because its preference-sync thread continued retrying read-only
storage. No permission escalation or VM-dependent test relaxation was used.

V367's independent Java/Rust fixture is terminal green (20.71 s), including
exact source invocation/native String diagnostics and source-written messages.
V368 strengthens it with retained/shared array aliases, parameter 2 after a
double-width primitive, widened char/int String search with two arguments,
constructor initializers and normal repeated calls. **1717/1717 library tests**
(82.13 s) and **8/8 integrations** pass: enum callback 1, native SAM 3, expanded
NPE fixture 1, outer-reference/lifetime 3. Log
`.tmp/null-diagnostic-v368-batch.log`. No acceptance check has been waived.

Review found one additional synthetic callback boundary: inlined multi-argument
forEach can lower directly into a loop rather than the ordinary lambda entry.
V369 disables ordinal attribution there too, with an explicit Map.forEach
capture regression and a disabled-frame guard at the diagnostic consumer.
Focused four-unit-plus-executable verification is running at
`.tmp/null-diagnostic-v369-focused.log`; the global Cargo filter is now
`null_diagnostic`, which matches both unit module and renamed integration test.
Once verified, freeze this producer and regenerate the complete Quaternion
closure; do not infer production success from focused probes alone.

The earlier portability question is no longer being used as an implementation
blocker: the existing strict default authorized this bounded source-provenance
repair, and it now has independent executable evidence. No unanswered option
has been treated as approval to relax wording. Full VM-equivalent unknown
origins/debug configurations remain outside this bounded feature, not silently
claimed solved. Quaternion still needs its production gates on the new producer.

V369's four focused units and expanded executable regression pass, including
the inlined Map.forEach synthetic-frame guard. V370 additionally reuses the
preferred emitted-declaration reader before the primary/erased-name fallback,
so duplicate batch support/summary copies cannot erase source runtime names.
Both single-file and *cross-package complete batch assembly* diagnostic
fixtures pass (2/2, 24.53 s), including the exact qualified Java invocation
name `producer.Datum.getValue()` and normal source calls. No acceptance-policy
waiver is needed to continue the strict implementation path.

Frozen producer `.tmp/new-java2rust-numbers-default-v370-source-null-diagnostics-debug`,
SHA256 `707d59cd54c9655026bf3e4bd860b856fd8f1e83aa03213f7110dcb204ca0915`.
Complete Quaternion/Core/client regeneration is running at
`.tmp/commons-numbers-1.3-quaternion-v370-contracts`, staging
`.tmp/numbers-quaternion-v370-contracts-staged`, outer log
`.tmp/numbers-quaternion-v370-contracts-run.log`, normal complete contracts
runner with the frozen binary, debug/offline/TMPDIR settings and
`--skip-resolver-bench`. Production results are pending, not inferred from
the focused fixtures. V362 accepted-module/RNG preservation remains separately
recorded; it is not attributed to V370.

The independent geometric auditor now has an explicit opt-in `--compare`
compatibility gate, without changing the strict verifier. It first audits
all 96 rotations and 48 spherical phase/plane results for **both** Java and
Rust at the existing 1e-14 residual bound, requiring exact repeat/callback
components. It then requires the complete 4532-line frame, ordered unique
keys, and exact every noninterpolation observation, exception/message, linear
path, endpoint and zero component. Only finite nonendpoint spherical component
rounding can differ; strict differences are retained in its report, not
silently deleted. This is the previously investigated ordinary rotational
workload, not a new arbitrary libm extreme target.

Four comparator regression tests reject exception/message changes, missing/
reordered/duplicate state rows, linear/endpoint drift, zero signs, nonfinite
values and malformed observations. The new gate rejects V362 output at its
remaining automatic-NPE wording mismatch (`.tmp/quaternion-v362-compatibility-negative.log`),
proving that the numerical policy does not waive those discrete failures.
Its first positive production run awaits V370 runtime output.

V370 complete production regeneration is terminal green (174074 ms total),
and the strengthened client/lifetime gate passes all 36 Java guards and both
Rust release paths. Strict numerical output has **16** differences: the same
12 spherical rounding rows plus four generic NPE messages (`null.add`,
`null.subtract`, `null.dot`, `slerp.null.start`). Thus six of the ten original
NPE wording failures are fixed without any portability waiver, not all ten.
The explicit compatibility gate correctly still rejects this output.

Remaining whole-path audit: arithmetic argument lowering can call the shared
field reader directly, before the expression entry context is established.
Derive parameter dereference provenance at that authoritative field operation
boundary too. Shared-owner constructor prefix staging has correct source
parameter binding identities and propagating Result mode (verified by focused
trace); its companion receiver path calls the common *known optional receiver*
projector directly, bypassing the wrappers that consumed diagnostic context.
Consume context at that common producer before later slot/Raw transformations,
not by rewriting strings after emission.

Independent `NpeStagedOwner` regression now exercises two normalized source
references, an intervening local, a retained private native-SAM strategy and
both constructor-null positions, plus repeated successful calls. This also
exposed a separate immutable Rc capture read: a getter became
`self.__outer.0.value` although storage is `Option<Rc<Data>>`. Minimal correction
is the ordinary checked optional-handle projection before the same field read,
not payload cloning, new storage, or changing the Java fixture to mutable data.
V372 reuses the enclosing-field binding/storage reader in physical value shape
inference and routes accessor fields through the existing checked optional/
shared field readers. The staged/immutable regression passes (17.45 s), with
both exact constructor messages and normal alias/callback behavior intact.

V373 removes the temporary origin trace, adds a floating-field arithmetic
argument regression for parameter 2 (the production add/subtract/dot family),
and runs a coherent full-library plus related executable batch at
`.tmp/null-diagnostic-v373-batch.log`. No production corpus is rerun after each
leaf repair. Five numerical-gate regressions now pass, additionally proving
that finite unit-length no-op quaternions cannot satisfy the independent
plane/phase check. V370 remains separate immutable evidence until the current
batch and subsequent complete regeneration are verified.

V373 is terminal green: **1718/1718 library tests** (93.73 s) and **9/9 related
integrations** (enum 1, native SAM 3, diagnostic single/batch 2, outer reference
and lifetime 3). The expanded executable fixture additionally verifies the
production-family floating-field addition inside an argument and its exact
parameter-2 error message. No temporary null-origin trace remains. The four
remaining production NPE misses were fixed at shared field-read and known
optional-receiver operation boundaries; the new immutable-capture consumer
continues to use canonical Rc handles, not cloned payloads.

Frozen candidate
`.tmp/new-java2rust-numbers-default-v373-null-projection-contracts-debug`, SHA256
`17269f9d4a3fdcdcf488f2728baf71c288b0f4a2fcdcf6328be30f333c8001dc`.
Complete Quaternion/Core/client regeneration and all earlier Numbers/RNG
preservation regenerations are running concurrently on this same producer.
Quaternion output `.tmp/commons-numbers-1.3-quaternion-v373-contracts`, staging
`.tmp/numbers-quaternion-v373-contracts-staged`, outer log
`.tmp/numbers-quaternion-v373-contracts-run.log`. Earlier Numbers module outputs
use `.tmp/commons-numbers-1.3-{primes,rootfinder}-v373-preserved` and
`.tmp/commons-numbers-1.3-{core,complex}-contracts-v373-preserved`; RNG outputs
`.tmp/commons-rng-1.0-{source64,sampling}-v373-preserved`. Fresh matching
`v373-preserved-staged` directories and outer logs retain the usual full source
checks. All runs use normal complete runners, frozen debug/offline/TMPDIR
settings and `--skip-resolver-bench`. None is yet a production runtime pass.

### V373 Quaternion acceptance checkpoint: 5/6 modules

The complete pinned **14-production-file Quaternion+Core closure**, plus its
separate translated Java client, translates/parses/compiles with zero
unsupported statements/members/expressions, invalid patterns, stubbed constants
or reachable no-op facade calls. Manifest
`.tmp/commons-numbers-1.3-quaternion-v373-contracts/contract-source-manifest.tsv`
matches the untouched upstream inventory. The existing complete declaration
inventory `tests/corpus/numbers/Quaternion-1.3-declarations.txt` includes all
public/private constructors, overloads, nested Type/Default/Normalized,
parsing-exception constructors and both interpolation strategies. Core's
complete declarations/API coverage retain their independently verified gate.

Current runtime evidence:

- `.tmp/numbers-quaternion-v373-client-behavior`: all **36** translated Java
  client guards pass, with two exact output lines, plus the Rust lifetime gate
  for both linear and spherical strategies. Callbacks keep their owner alive
  while usable and release it after the final callback drops.
- `.tmp/numbers-quaternion-v373-numeric-behavior`: all **4531 keyed observations
  plus footer** execute against all public Quaternion overloads and Slerp
  construction/direct/callback/repeated calls, identity/state, array copy,
  parsing, invalid/boundary inputs and exception/cause paths. **All ten
  previously mismatching VM-generated NPE messages now match exactly**, as do
  all other discrete and noninterpolation observations. No message portability
  waiver was taken. The unchanged strict verifier deliberately still exits
  nonzero for the **12** retained finite spherical rounding rows.
- `.tmp/quaternion-v373-compatibility.json`: the explicit
  `quaternion-geometric-compatibility-v1` gate passes the complete ordered frame
  and both independent 80-digit geometric audits. Rust maxima are **2.27e-16**
  unit-norm residual, **3.63e-17** plane residual and **1.07e-16** angular residual,
  below the recorded **1e-14** limit. Only investigated finite nonendpoint
  spherical rounding is allowed; linear paths, endpoints, exact zero signs,
  nonfinite classes, exceptions/messages and all other observations remain
  strict. Callback/repeated components must be bit-identical. This ordinary
  rotation/extrapolation workload and the independent geometry motivate the
  numerical allowance; no arbitrary extreme/libm parity campaign was added.

Reproduce runtime gates with the unchanged generic behavior verifier's
`--numbers-quaternion-contracts` / `--numbers-quaternion` modes on the recorded
V373 translation root, then:

```sh
python3 scripts/audit-quaternion-interpolation.py --compare \
  .tmp/numbers-quaternion-v373-numeric-behavior/java-output.txt \
  .tmp/numbers-quaternion-v373-numeric-behavior/rust-output.txt
python3 scripts/test_quaternion_compatibility.py
```

Five numerical-gate regression tests pass, including rejected exceptions,
missing/reordered/duplicate keys, endpoint/linear drift, signed zero/nonfinite
changes and a wrong finite unit-length interpolation. Older V362 output is
rejected at its NPE-message mismatch, not accepted by this numerical gate.
No generated deliverable was patched; no upstream file or target scope changed.
The verified general batch reuses authoritative capture/storage/receiver/name
contracts and preserves source-written exceptions. Full source verification
is **1718 library tests** plus **nine related executable integrations**, all
green, on the frozen producer hash recorded above.

Preservation is now terminal green on the **same V373 producer**: Primes **3594**
and Root Finder **122** exact lines; complete Core+client **7888** lines under
the unchanged four-record arithmetic NaN-sign policy; Complex's 32 client
guards and **1863** pinned independent-reference requests pass. Complex's
strict **14617**-line output and **78** differences are byte-identical to V362
and accepted V335; Core's four allowance records are likewise unchanged.
Complete RNG source64 **2865**, native **12601**, collection sampling **162**
and distribution sampling **2172** observations pass exactly, including bits
and RNG/state traces. The complete **46-source** sampling closure compiles
(208561 ms total) with zero unsupported/invalid/stubbed/compile/reachable-no-op
errors. Every owned V373 job is terminal. This proves preservation, not merely
reuse of older-producer claims.

Quaternion is accepted as module **5/6** under these recorded gates. Angle
remains required, and the overall goal is active, not complete. Next establish
its full pinned baseline and inventory, then audit actual generated code before
any new translator fix campaign.

### V374 Angle entry: pinned closure and baseline

Quaternion's V373 acceptance is recorded above; Angle is the final required
module, not an expansion to a seventh target. The untouched checkout remains
`ae8abefb51c2c4d1a05882d61cefc8015e493a8f` with a clean upstream worktree.
Angle's POM declares only Core as a production dependency. Source inspection
confirms `CosAngle` imports `Norm` and `Sum`; the other imports are the JDK
`DoubleSupplier` and `DoubleUnaryOperator`. The complete production closure is
therefore **all 11 Core files plus all four Angle files**, including both
package-info files. No Fraction, Gamma, Arrays or other module is needed.
The authoritative source manifest is the existing pinned focus manifest,
filtered to `commons-numbers-{core,angle}/src/main/java/`; the normal runner
checks this list against the full upstream file inventory and byte-compares
each staged source.

Angle's four files are `Angle.java`, `CosAngle.java`, `Reduce.java`, and
`package-info.java`, all under `org/apache/commons/numbers/angle`. The declaration
census is reproducible with `bash scripts/list-commons-numbers-1.3-angle-api.sh`:
it compiles the complete 15-file closure, then uses `javap -p -s` on every Angle
class file, including private `Normalizer` and all three concrete nested units.
Core retains its separately recorded full declaration inventory.

Baseline uses the unchanged frozen V373 producer
`17269f9d4a3fdcdcf488f2728baf71c288b0f4a2fcdcf6328be30f333c8001dc`,
output `.tmp/commons-numbers-1.3-angle-v374-baseline`, staging
`.tmp/numbers-angle-v374-baseline-staged`, and log
`.tmp/numbers-angle-v374-baseline-run.log`, using the ordinary module runner with
`NUMBERS_MODULE=angle`, debug/offline/TMPDIR settings and
`--skip-resolver-bench`. A launched baseline is not yet an acceptance claim.

Source-level audit priorities before any new translator fix campaign:

- `Angle.value` is private final base storage; Turn/Rad/Deg constructors must
  initialize it through `super`, and inherited suppliers/hash/equals and every
  conversion must read the same value. Same-unit conversions return the
  original Java identity, not a new payload copy.
- `equals(Object)` distinguishes concrete units, null, foreign objects,
  canonical NaNs and signed zero. Producer/upcast/erased argument/cast-back
  contracts must agree, including inherited method dispatch.
- Each factory and static operator field must build the correct nested type.
  Escaping normalizers contain an immutable `Reduce`; native SAM coercion,
  static initialization and repeated callback calls must preserve its bounds
  and period without borrowing a temporary or fabricating a result.
- `CosAngle.value` passes both arrays through existing Core Sum/Norm contracts;
  zero-length/dimension errors, input immutability and repeated calls must not
  be hidden by a numeric tolerance.

The existing framework has inherited storage, native SAM and identity
mechanisms, so source inspection alone does not justify a redesign or any
fix. Inspect actual generated producers/storage/signatures/consumers next.
The acceptance workload will exercise all unit factories/conversions,
inherited public operations, static constants/operators, all normalizer
branches, Reduce construction/direct/callback calls and CosAngle. Near-bound
rounding checks are motivated by the production Normalizer correction and
upstream precision regressions; no unrelated extreme libm campaign is planned.

### V374 generated-code feasibility audit

The complete baseline is terminal: 15 files generated, zero unsupported
statements/members/expressions or stubbed constants, but **12 Rust errors**:
three native callback placement diagnostics and nine unresolved overloaded
Norm method references (three names × three input signatures). One invalid
pattern file records the callback compile diagnostics. No reachable no-op
facade calls were found. No module acceptance is claimed.

Concrete rewrite/contract evidence (generated files are diagnostic evidence
only and remain untouched):

1. `Angle_Turn/Rad/Deg::normalizer` select
   `__JavaSyncDoubleUnaryOperator`, but `lower_native_functional_object_for_slot`
   reports the newly built facade as default local allocation. Its captured
   `Angle_Normalizer` is `Rc`, and `Normalizer.reduce` is `Rc<Reduce>`, so merely
   deleting the diagnostic or changing the return type would not be correct.
   Minimal hypothetical Rust is one canonical `Arc<Normalizer>` retaining
   `Arc<Reduce>`, captured by the selected synchronized native facade; the
   facade's reported produced shape must match its actual carrier. The
   existing process-wide storage reachability graph currently includes native
   lambda captures but not the hidden retained implementing-object edge for a
   named source SAM. Extend that same graph through source subtype resolution,
   not a new per-corpus placement heuristic. This is a cross-function/storage
   contract repair, using existing representations rather than redesign.
2. Norm's enum constructors emit `Arc::new(Norm::euclidean)` (likewise
   manhattan/maximum), while production helpers correctly have mangled
   two-/three-/array overload names. Accepted Core's same-producer output
   instead contains typed adapter closures calling those exact overloads.
   Minimal rewrite is the already supported typed source method-reference
   adapter, including array storage and fallible invocation. Trace why the
   complete multi-package support context loses that resolution before fixing;
   do not add aliases or guess a helper from argument count alone.
3. A runtime naming inconsistency is visible before compilation succeeds:
   `AngleValue::get_class` uses `Angle.Rad` while concrete fallback get_class
   uses `org.apache.commons.numbers.angle.Angle$Rad`. `equals(Object)` relies
   on these class tokens before its checked Angle cast. Hypothetical correct
   Rust retains the same canonical Arc through erasure/downcast and gets the
   same binary source class identity through either carrier. Reuse the stored
   source-name metadata for both producers, not a second name-mangling rule.
   This is a local metadata-consumer repair with cross-path regression tests.

Base-field constructor flattening already initializes the inherited value,
the inherited suppliers/hash read it, and same-unit `_with_outer` conversions
clone the original Arc. AngleValue's dispatch arms use those companions.
Normalizer's arithmetic, interval early-return and upper-bound correction are
present as actual production algorithms. CosAngle calls generated Sum and
Norm with both original shared arrays. These mechanisms can express the
needed rewrites; no broad ownership/framework redesign is indicated by this
baseline. Batch the callback placement and naming/context fixes with focused
contracts, then regenerate once to expose any further family.

### V375 first Angle batch: verified mechanisms, new client boundaries

General changes implement the V374 rewrite plan: the existing static/capture
reachability graph now includes named native SAM implementing objects through
the same source/JDK subtype resolver used for interface aliasability; its
transitive field/parent walk lifts the retained payloads too. Native source
adapter emission records the allocation of the facade it actually constructed
(selected from the target slot), rather than pretending a synchronized facade
is local and attempting to migrate it afterwards. The resolver indexes original
canonical/binary source names as aliases to the same declaration, preserving
overload selection when an aggregate multi-package Program has no package.
Hierarchy class-token emission consumes existing binary-name metadata.
No Java production source or generated deliverable was edited.

Full library **1721/1721** passes, log
`.tmp/numbers-angle-v375-library-r2.log` (80.42 s runtime), including three new
focused tests for imported owner overload selection, nested binary class names,
and synchronized named-SAM inherited payload reachability. An initial test-only
assertion typo used a nonexistent `parameter_types` field; it was corrected to
the actual `arg_target_types` before the successful full run. All **four**
native SAM executable integrations pass, including the new
`NamedGlobalSamProbe` (14.56 s), log `.tmp/numbers-angle-v375-native-sam.log`.
The existing enum callback integration passes, but the new imported-enum
executable regression still fails as described below; it is not a green gate.
All **11** generic comparator regressions pass unchanged.

Frozen producer
`.tmp/new-java2rust-numbers-default-v375-angle-contracts-debug` SHA256
`de1b6c881f8a7a130e701b3d09ac67a01274d8927321610990a2f292dfe1c4e2`.
Complete **15-production-file + one translated client** regeneration is
terminal in `.tmp/commons-numbers-1.3-angle-v375-contracts`, staging
`.tmp/numbers-angle-v375-contracts-staged`, log
`.tmp/numbers-angle-v375-contracts-run.log`. Its manifest explicitly separates
all production files from `AngleContractsOracle.java`. Use frozen binary,
debug/offline/TMPDIR settings, wrapper
`bash scripts/run-commons-numbers-1.3-angle-contracts.sh --skip-resolver-bench`.
The old 12-error family is gone: generated normalizers now capture canonical
Arc objects and retain Arc<Reduce>; all nine Norm references use typed calls
to the correct existing overloads; hierarchy class names use binary identity.

The new client deliberately exercises more than the production-only baseline.
Its Java **28-guard** contract passes on untouched upstream;
`.tmp/numbers-angle-v375-client-java` records the independent run. A separate
independent Java numerical caller produces **4642** observations under
`.tmp/numbers-angle-v375-numeric-java` (no Rust runtime success claimed yet).
The declaration inventory is now stored at
`tests/corpus/numbers/Angle-1.3-declarations.txt` (all seven Angle class files,
private members, descriptors, nested types and initializers). The Rust numeric
caller only calls generated algorithms and explicitly keeps local Reduce
callback carriers distinct from synchronized static normalizers; it is ready
for compilation after the complete generated crate is fixed.

Current complete-client result is **51 compile errors**, zero unsupported
statements/members/expressions and reachable no-op calls, but **20 stubbed
external constants**. Do not accept or omit those paths. Fresh generated-code
audit identifies the next coherent batch:

1. **Nested static source storage (49 emitted errors, one family).**
   `Angle.Rad.ZERO` and normalizer operator fields are emitted as associated
   constants although the production declaration generates `__static_ZERO()`
   and corresponding operator accessors. Parallel external facades named
   Rad/Turn/Deg are spuriously emitted, including inherited constant names.
   Minimal hypothetical rewrite selects the existing source nested declaration,
   uses its already-solved field shape and normal static accessor, and suppresses
   the false external facade. `static_storage_owner_type_name` currently only
   accepts `Expr::Var`, while Java's qualified nested type is a FieldAccess
   chain. Reuse `source_type_name_from_expr`/the existing nested source lookup
   (respect value shadowing and authoritative bodyful-vs-summary selection)
   instead of flattening names or adding associated aliases. Audit both lower
   and facade-support discovery consumers; fixing only emitted spelling is
   insufficient.
2. **Nested expression Result contract (two compile errors).** In the erased
   argument `a.equals(angle((unit+1)%3,0.25))`, the inner remainder emits
   `Result<i32,...>` in an `i32` formal without `?`. The surrounding client
   already returns Result. A separate focused fixture initially exposed the
   analogous extra Result in a ternary return; its repeated-call check is now
   an equivalent `if`/return to keep that overload regression focused (all
   assertions remain). Preserve evaluation order and use the existing common
   argument/return effect adaptation; do not specialize for a known divisor or
   rewrite the acceptance client to avoid this production-reachable family.
3. **Retained source SAM exceptions (runtime regression).** The imported-enum
   overload fixture now selects the correct array helper, but the retained
   source `Array.call` trait still returns plain f64. Its method-reference
   adapter unwraps an optional input with `expect("Java null receiver")` and
   a fallible helper with panic formatting. Its caller then cannot catch the
   Java NPE. The null assertion is retained and the regression is deliberately
   red (`.tmp/numbers-angle-v375-enum-r2.log`); do not remove it. Correct minimal
   Rust shares one fallible descriptor through trait declaration, adapter,
   stored field and invocation, returning the existing JavaException Result.
   Trace retained source-interface callback effects through the authoritative
   call graph/exception planner before changing either ABI consumer. This is
   cross-function exception-contract work, not a new ownership representation.

No new FP tolerance was introduced. Ordinary conversions, wrapped headings,
the actual upper-bound correction, vector directions and upstream-motivated
scale handling form the numerical workload. NaN/zero/nonfinite observations
remain recorded strictly; differences must be investigated after compilation.
No V375 earlier-module/RNG preservation pass is claimed yet. All owned jobs
from this checkpoint are terminal; next implement this audited batch, focused
regressions first, then one complete Angle regeneration. Overall status stays
**5/6 accepted, Angle incomplete, goal active**.

### V376 Angle boundary batch

The V375 three-family plan is implemented using existing authorities:

- Qualified nested type expressions now enter normal source static storage
  lookup, with value-binding shadowing checked at the root of the qualified
  expression. Static reads and inherited constants reuse lexical source field
  lookup to select the declaring class, and numeric promotion reads that same
  declaration rather than an unknown child field. No associated aliases or
  field-specific values were invented.
- The exact batch `SourceEmissionPlan` now identifies declaration-only copies
  of co-emitted source classes, so facade discovery/suppression uses the same
  declaration keys as actual primary emission. It no longer generates fake
  source-summary static constants for nested Value/Rad/Turn/Deg aliases whose
  definitions are supplied by other primary files. Original package metadata
  is authoritative: a same-leaf foreign declaration must not be claimed by an
  emitted source class, even if a simple Rust alias exists.
- Matching argument value carriers still discharge their evaluation effects.
  The shaped argument shortcut now uses the existing Result projection for a
  nested checked arithmetic producer. Conditional lowering separately joins
  branch effects; a fallible condition alone no longer makes plain branch
  values Result-wrapped. This also closes the analogous synthetic ternary
  finding from V375 without rewriting the acceptance client.
- Source SAM arguments to `new`, `this(...)` and `super(...)` now register their
  typed body against the existing SAM call-graph descriptor, as source method
  arguments/field assignments already did. Exception planning therefore shares
  one Result ABI among stored trait, adapter and caller. Construction remains
  lazy: it does not execute or throw from the callback body.

Focused executable evidence before the final package-identity guard:
`.tmp/numbers-angle-v376-focused-r3.log` passes **3/3** new Angle lowering
contracts (25.57 s), **2/2** enum callback contracts including the retained-null
assertion (25.95 s), and **5/5** native/source callback integrations (31.47 s).
The latter includes direct source callback constructors, this/super delegation,
method references, lambdas, repeated calls, caught errors and exact invocation
counts (creation executes nothing). The initial synthetic interface name `Fn`
collided with an existing generated convenience alias and Rust runtime Fn
traits; its name was changed to `StoredOperation` to isolate this regression's
exception contract. No assertion/exception/identity workload was removed.
That separate convenience-alias reservation issue is not an Angle production
finding and is not represented as fixed.

The nested-static fixture additionally covers inherited primitive constants,
field initializer identity and repeated static native callback reads; early
focused failures exposed both the summary-facade leak and missing inherited
numeric promotion before the successful batch. The retained overload fixture
now keeps the correct array helper's Result through null input instead of
panicking. Eleven generic comparator regression tests remain green.

Final source verification is running in
`.tmp/numbers-angle-v376-final-batch.log`: full library plus those ten related
executable integrations, including a new same-leaf foreign-package ownership
unit guard. Complete Angle+Core+client regeneration uses fresh `v376-contracts`
translation/staging roots and the `v376-angle-boundaries-debug` frozen producer
(hash is to be recorded after its build completes). Neither a launched run nor
these focused passes proves Angle acceptance or previous-module preservation.
No upstream/generated deliverable was patched and no numerical policy changed.

### V376 complete generation and runtime audit: not accepted yet

Frozen producer SHA256
`dd052a942fde5b1b25ee0fed0e440d15e2de6ade3b3d2b9e93c8ba6feea3e7e5`
at `.tmp/new-java2rust-numbers-default-v376-angle-boundaries-debug` generated
all **16** files (15 production + separate client) into
`.tmp/commons-numbers-1.3-angle-v376-contracts`. Normal wrapper staging and
manifest checks cover every Core/Angle file byte-for-byte at the unchanged pin.
Terminal generation reports zero translation/parse failures, unsupported
statements/members/expressions, invalid patterns, stubbed constants and compiler
errors. Translation took 142590 ms (89685 ms emission); cargo check 2925 ms.
The generated client now reads normal static accessors, and its nested remainder
crosses the Result boundary with `?`. All V375 51 compile errors are eliminated.

Final source batch is terminal green in
`.tmp/numbers-angle-v376-final-batch.log`: **1722/1722 library** (88.42 s),
**3/3 Angle lowering**, **2/2 enum callback**, and **5/5 native/source callback**
executable integrations. This includes the final package-identity guard, not
only the earlier producer's test run. No source edit followed the producer
freeze. Reproducible full private/declaration census files for Primes and Root
Finder have also been added, completing the six-module declaration inventory
set; `NUMBERS_MODULE=primes|rootfinder bash scripts/list-commons-numbers-1.3-api.sh`
reproduces them from complete untouched closures.

Independent client execution is **red**, not accepted:
`.tmp/numbers-angle-v376-client-behavior/rust-stderr.log` reports guard **5**
(fresh equal angle compares unequal). Numeric caller's first Rust compile
used a helper not exported by this assembled crate; the test-only driver now
compares the existing generated `JavaReferenceIdentity` tokens directly.
No generated code was changed. Fresh
`.tmp/numbers-angle-v376-numeric-behavior-r2` completes all **4642** Java and Rust
observations, retaining **156** strict differences: **114** equality results
and **42** NaN-sign-only normalization results. Every other observation matches
exactly, including all finite numerical values, signed zeros, constants,
conversion identity, concrete class names, vector results/errors and repeated
calls. The strict verifier still exits nonzero; no allowance was applied.

The next whole-path audit is grounded in these runtime results:

1. **Erased runtime class tokens (114 discrete failures).** All concrete and
   hierarchy class-token producers now agree on original binary names, but
   `Angle.equals(Object)` invokes `get_class()` on its erased boxed argument.
   The generated `JavaErasedObjectFacade` implementation returns the fixed
   `JavaClass::<()>::for_name("java.lang.Object")`, regardless of the actual
   canonical Arc or AngleValue stored inside. Thus `getClass().equals(...)`
   rejects even self/fresh values before the checked cast and value comparison.
   The static no-op call scanner reports zero sites, but this runtime result
   proves a reachable fabricated class-token fallback remains; that scanner
   alone is not acceptance evidence.

   Minimal correct Rust obtains the argument's class from its actual retained
   source handle/carrier metadata, without copying its payload or fabricating
   Object.class. Existing erased-object runtime generation in `stdlib.rs`
   already has source clone/identity/equality downcast registries and canonical
   storage helpers (`source_erased_shared_reference_types`, generic-instance
   visitation). Concrete source fallbacks and AngleValue already provide class
   metadata. Reuse those authorities for erased class dispatch and its nullable
   receiver boundary; do not add an Angle equality exception. Alternatively,
   any comparison-specific optimization must preserve exact-class versus
   instanceof distinctions, null exceptions and canonical Arc/carrier cases.
   The current comparison-specific helper only recognizes binary `==` and
   does not even wrap immutable shared placements, so blindly routing Class
   `.equals` there is insufficient. This is a general erased dispatch/metadata
   contract repair, not an ownership redesign.
2. **Math.max discards an arithmetic NaN operand (42 FP differences).** All
   affected rows are the seven normalizer variants with ±infinity inputs,
   each direct/repeated/composed. Java and Rust direct Reduce already both
   produce negative quiet NaN. The production Normalizer then calls
   `Math.max(lo, normalized - period)`; existing `java_math_max_f64` replaces
   any NaN operand with canonical positive `f64::NAN`. This erases its sign
   (and potentially payload), explaining exactly the observed 42 rows.
   A bounded general repair is to retain the selected NaN operand in the
   existing scalar min/max kernels, with focused operand/zero checks. This
   follows a real production path and does not demand a new elementary-function
   implementation or an arbitrary extreme-argument parity campaign. No new
   tolerance or accepted policy is required or assumed at this checkpoint.

Next batch these mechanisms with executable erased-class controls (self/fresh,
foreign/sibling classes, base/carrier paths and null receiver) and the existing
min/max support, then regenerate Angle once. Earlier-module/RNG preservation
on V376 has not been run and is not claimed. All owned V376 jobs are terminal.
Overall status is still **5/6 accepted, goal active**.

### V377 erased-class / scalar operand batch

Implemented the V376 runtime audit through existing authorities. All concrete
source class-token producers now share `java_naming::source_binary_class_name`.
The erased-object runtime adds an additive `runtime_class` registry using the
same canonical storage rendering, retained handle types, source metadata and
concrete generic-instance visitation as its existing clone/identity/equality
registries. Closed source class carriers query their existing runtime class
metadata, while concrete source handles use their original binary class names
without taking a payload loan or cloning an object. Native boxed primitives,
String/Object and standard primitive array carriers retain their corresponding
class identity. Unknown metadata fails explicitly rather than inventing
Object.class. This is not a claim to implement all Java reflection or anonymous
subclass metadata; no such unsupported dispatch is permitted in acceptance.
The facade's erased getClass method now calls this runtime, and support
discovery explicitly requests it. Additive assembly uses the existing generic
registry merger, not a per-corpus support-file patch.

The four existing float/double min/max kernels now retain the first NaN
operand, or the second when only it is NaN, keeping the existing signed-zero
ordering. Focused checks cover the actual infinity-derived normalization NaN,
operand retention, finite values and zero signs in both widths. No new libm
implementation or tolerance was introduced.

The erased-class regression also exposed one producer-contract loss before
the batch was frozen: `new double[]{4}` boxed as Object became `Vec<i32>`.
Its declared component type was bypassed by the plain array-expression path.
The minimal correct rewrite is a typed Vec<f64> with ordinary widening before
erasure. Typed initializer rvalues now reuse `lower_array_expr_for_slot` and
the explicit source array type, so existing element coercion, annotations,
narrowing and empty-array handling supply the producer contract. No generated
array or test input was hand-repaired; integer literal 4 remains unchanged,
and empty double/long-array controls were added.

`.tmp/numbers-angle-v377-focused-r2.log` passes **5/5** executable Angle lowering
contracts (24.69 s), including self/fresh/sibling/foreign equality, mutable
source aliases, base/carrier erasure, class names, null receiver and typed-array
class identity. The unchanged first run failed at the array-type guard,
identifying the real producer problem; no assertion was removed. Seven
shared-registry Python regressions pass, now including the new registry marker's
order independence/idempotence/union behavior. A real cross-compilation-unit
class-token integration is included in the final batch too.

Frozen producer `.tmp/new-java2rust-numbers-default-v377-erased-class-debug`,
SHA256 `33f3c3a59cb65d47e19b7376dea45cab86904989a09475023ede040d4a923e9e`,
is the already-built normal CLI exercised by the focused tests. No translator
source edit followed that successful build/freeze. Fresh normal complete
Angle+Core+client generation is running in
`.tmp/commons-numbers-1.3-angle-v377-contracts`, staging
`.tmp/numbers-angle-v377-contracts-staged`, outer log
`.tmp/numbers-angle-v377-contracts-run.log`. Full library plus related executable
verification is running in `.tmp/numbers-angle-v377-final-batch.log`. These are
pending evidence, not acceptance or earlier-module preservation claims.

### V378 final Angle acceptance and six-module completion audit

Final frozen normal debug CLI:
`.tmp/new-java2rust-numbers-default-v378-additive-erased-copy-debug`, SHA256
`77aecc615c5562cc3c3aa083dc52f9463148a9b9559b37f7912f2fa71e85a95c`.
The built `target/debug/new-java2rust` has the identical hash. No translator
source was changed after freezing this producer. All eight final generation
roots below record this hash in `translation-producer.tsv` and `summary.tsv`.

The final cross-unit test exposed an assembly contract not visible in a
single-file regression: erased-reference copying selected one compilation
unit's source downcasts, losing source types introduced by other units. The
minimal correct result is the union of existing source/typed-entry/value-entry
copy arms. V378 marks those arms with the additive `reference_copy` registry
and reuses the existing generic registry merger, just as `runtime_class` does.
The strengthened cross-unit caller passes an erased source reference through
`echo(Object)` and checks retained identity, not merely its class name. There
is no corpus-specific assembly hook or generated-code repair.

Final source verification is terminal green:
`.tmp/numbers-angle-v378-final-batch.log` has **1722/1722 library**, **6/6 Angle
lowering**, **2/2 enum callback**, and **5/5 native/source callback** tests.
The strengthened cross-unit test also passes independently in
`.tmp/numbers-angle-v378-cross-unit.log`. Cheap final rechecks pass all **11**
behavior comparator, **7** shared-runtime registry, and **5** Quaternion policy
regressions. These include negative controls for numerical-policy misuse;
the strict comparators themselves have not been relaxed.

#### Final production and behavior evidence

Every translation root below has a complete `batch-manifest.tsv`, successful
`summary.tsv`, zero failed/unsupported/invalid/stubbed/compile errors and zero
no-op facade call sites. Contract roots additionally retain
`contract-source-manifest.tsv`, separating untouched production from the one
test-only Java caller. Counts include package metadata. The source manifests
and every staged production file were rechecked against the clean pinned
checkout, not inferred solely from compile success.

| Module | Final translation root under `.tmp/` | Production + separate client | Runtime evidence under `.tmp/` |
|---|---|---:|---|
| Primes | `commons-numbers-1.3-primes-v378-preserved` | 3 + 0 | `numbers-primes-v378-behavior`: 3594 exact observations |
| Root Finder | `commons-numbers-1.3-rootfinder-v378-preserved` | 3 + 0 | `numbers-rootfinder-v378-behavior`: 122 exact observations, including callback traces and errors |
| Core | `commons-numbers-1.3-core-contracts-v378-preserved` | 11 + 1 | `numbers-core-v378-client-behavior` and `numbers-core-v378-numeric-behavior`: client passes; 7888 observations with unchanged four arithmetic NaN-sign components |
| Complex | `commons-numbers-1.3-complex-contracts-v378-preserved` | 2 + 1 | `numbers-complex-v378-client-behavior`: 32 client guards; `numbers-complex-v378-reference-behavior`: 1863 independent-reference requests pass |
| Quaternion | `commons-numbers-1.3-quaternion-contracts-v378-preserved` | 14 + 1 | `numbers-quaternion-v378-client-behavior`: 36 guards and both callback lifetime/release checks; `numbers-quaternion-v378-numeric-behavior`: 4532 observations, accepted by the unchanged independent geometry policy |
| Angle | `commons-numbers-1.3-angle-v378-contracts` | 15 + 1 | `numbers-angle-v378-client-behavior`: all 28 guards; `numbers-angle-v378-numeric-behavior`: all 4642 observations exactly match |

Angle's complete declaration inventory was checked against both maintained
callers: all three factories, all nine concrete conversions and inherited
conversion dispatch, getAsDouble/hashCode/equals, all constants and static
operators, all three normalizer factories, Reduce construction/direct/SAM
invocation, and CosAngle.value. The matrix covers same-unit reference identity,
self/fresh/null/foreign/cross-unit equality, NaN equality and signed zero,
conversion cycles, normalizer near-upper-bound correction, repeated/composed
callbacks, vector dimension errors, null inputs and input immutability. Private
Normalizer and conversion constructors are exercised through public factories;
Core's full production closure is independently preserved, not merely compiled
as Angle support. No Angle floating-point allowance was introduced.

Core's allowance JSON is byte-identical to V373. Complex's complete **14617**
strict Java/Rust output lines and **78** strict differences are byte-identical
to V373 (and its recorded accepted V335 comparison). Its independent-reference
gate has no failures; its separate **139** strict reference-workload differences
remain recorded. Quaternion's complete outputs and **12** finite interpolation
rounding differences are also byte-identical to V373. Both strict verifiers
deliberately exit **1**, not success. The separate retained
`.tmp/quaternion-v378-compatibility.json` passes
`quaternion-geometric-compatibility-v1`: all discrete, exception/message,
noninterpolation, endpoint and linear observations exact; callback/repeat bits
identical; independent 80-digit norm/plane/angular residuals below **1e-14**.
Rust maxima remain about **2.27e-16**, **3.63e-17**, and **1.07e-16**. These are
the already investigated practical numerical policies, not new tolerances
chosen for V378. Artificial extreme elementary-function stress is diagnostic,
not an additional JVM-rounding compatibility campaign.

Commons RNG is freshly preserved on the same producer and untouched pin
`4581a4520315657a4219b37c81f5db80e4a4e43c`. Translation roots
`commons-rng-1.0-source64-v378-preserved` (**11** files) and
`commons-rng-1.0-sampling-v378-preserved` (**46** files) pass all generation
gates. Runtime roots `rng-source64-v378-behavior`, `rng-native-v378-behavior`,
`rng-collections-v378-behavior`, and `rng-distributions-v378-behavior` pass
**2865**, **12601**, **162**, and **2172** exact observations respectively,
including floating-point bits, callbacks and RNG/state traces. All final owned
processes have been polled terminal; none is merely launched or waiting.

#### Requirement-by-requirement closure

| Requirement | Evidence and conclusion |
|---|---|
| Six complete untouched modules, correct production dependency order | Exact Numbers pin remains `ae8abefb51c2c4d1a05882d61cefc8015e493a8f`; clean upstream worktree; all 26 own source files in the focus manifest match upstream. POM/import audit records JDK-only first four modules and complete 11-file Core closure for Quaternion/Angle. No seventh module or silently omitted dependency. |
| Every constructor/helper/nested type/overload included | Six maintained `tests/corpus/numbers/*-1.3-declarations.txt` inventories (`javap -p -s`) plus complete source/batch manifests; no method-level translation filters. Public-operation coverage ledgers and Java/Rust callers are retained at each acceptance checkpoint above; Angle mapping is recorded here. |
| Baseline and whole-path feasibility audit before fix campaigns | Per-module entry/baseline/generated rewrite sections above, including repeated audits at new runtime/ownership/aliasing/exception families. V374 baseline and V376 runtime audit precede the final Angle batches. |
| General fixes through authoritative contracts; no repaired deliverables | Final source changes reuse storage reachability, source ownership/naming, callback descriptors, typed array slots, erased-object dispatch and generic additive assembly. Fresh untouched-source regeneration and separately recorded producer/caller provenance establish deliverables; earlier diagnostic rewrites are not used. |
| Complete generation, parsing and compilation without reachable fake bodies | All six final roots and both RNG roots pass; actual executable callers supplement the static scanner. In Angle, source-abstract base conversion shells cannot be constructed by Java and concrete unit dispatch is exercised. Shared support still contains unused legacy facade declarations; definition counts are not represented as zero or as proof of reachability. Unsupported reflection/anonymous-class metadata outside these source programs is not claimed implemented. |
| Independent behavioral coverage, exact state/discrete/error/identity behavior | Current retained caller snapshots/hashes, outputs, client guards and operation ledgers; Angle strict 4642, earlier modules and RNG freshly rerun. Harnesses call linked generated production algorithms, not replacement Rust implementations. |
| Investigated, explicit numerical contract | Strict differences remain visible. Only unchanged Core arithmetic NaN-sign, Complex independent-reference and Quaternion geometric policies apply. Angle requires exact bits. No branch/state/error drift is waived. |
| Regression/preservation and reproducibility | Frozen producer hash, eight complete generation roots, full library and related executable suites, all behavior evidence above and commands below. Existing user changes remain unreset; no commit was made. |

#### Reproduction

Use fresh output/staging roots rather than overwriting frozen checkpoints.
For example, regenerate the complete final Angle+Core closure and translated
client, then run both independent runtime gates:

```sh
export CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true
export TMPDIR="$PWD/.tmp/test-tmp"
export NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-numbers-default-v378-additive-erased-copy-debug"
export NEW_JAVA2RUST_BUILD_PROFILE=debug JOBS=2
NUMBERS_CONTRACTS_STAGING_ROOT="$PWD/.tmp/angle-reproduce-staged" \
NUMBERS_CONTRACTS_TRANSLATION_ROOT="$PWD/.tmp/angle-reproduce" \
  bash scripts/run-commons-numbers-1.3-angle-contracts.sh --skip-resolver-bench
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/angle-reproduce \
  --numbers-angle-contracts --output-dir .tmp/angle-reproduce-client
python3 scripts/verify-commons-rng-1.0-behavior.py .tmp/angle-reproduce \
  --numbers-angle --output-dir .tmp/angle-reproduce-numeric
cargo test --lib
cargo test --test angle_lowering_contracts --test enum_callback_contracts --test native_sam_contracts
```

Other modules use the corresponding maintained contracts wrappers and verifier
modes; Primes/Root Finder use `NUMBERS_MODULE` with the plain module runner.
Core numeric verification explicitly adds `--arithmetic-nan-sign-portability`.
Complex runs both `--numbers-complex-contracts` and
`--numbers-complex-reference`; retain the nonzero `--numbers-complex` strict
report too. Quaternion runs `--numbers-quaternion-contracts`, retains the
nonzero `--numbers-quaternion` strict report, then passes its actual Java/Rust
output paths to `scripts/audit-quaternion-interpolation.py --compare` as in the
V373 commands above. Declaration inventories reproduce with the maintained
API scripts; all prior checkpoint commands remain recorded in this journal.

No production files, public operations or required dependencies are excluded.
Existing scope exclusions remain upstream tests/examples/benchmarks/GUI and
unrequested modules. Final audit proves **6/6 accepted** and earlier Numbers
and RNG behavior preserved under the explicit policies, not universal Java/JDK
support or universal cross-backend floating-point bit identity.

## Acceptance requirements

Before fixing each new module, repeat the baseline and whole-path rewrite
audit. Require complete translation/parse/check and no reachable fabricated
bodies. Independent Java/Rust callers cover every public operation/overload,
representative branches, invalid/boundary inputs, repeated state and callbacks,
plus relevant private/package helpers. Discrete behavior must match exactly;
float comparison defaults to IEEE bits. Investigate discrepancies before any
justified tolerance; never conceal state or branch drift.

For Primes, cover dense small integers, deterministic broad samples, Miller-
Rabin thresholds/pseudoprimes, the positive int ceiling, factor multiplicities,
large semiprimes, nextPrime transitions and validation exception messages.
Check the static wheel contents, helper list mutation and repeated class use.
No primality/factorization algorithms may be reimplemented in the Rust caller.
Keep previous modules and Commons RNG native/source64/sampling gates passing.
Use focused regressions during coherent batches; broad gates belong at batch
boundaries and final acceptance. Freeze producers and record final evidence.
