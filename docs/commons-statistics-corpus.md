# Apache Commons Statistics translator campaign

Status: translator fixes in progress. Ranking core and the expanded null/
inherited-operation client pass compilation and behavior checks; the remaining
public-operation/callback/reachable-support audit is open. No Statistics module
is accepted.
All five production modules are in scope. Modules/slices are implementation
units only; they do not reduce final production or dependency coverage.
General translator/runtime improvements are the deliverable. Never repair
generated Rust or alter pinned Java to obtain acceptance.

## Pins and complete scope

Statistics 1.3 is a stable release listed on the
[official download page](https://commons.apache.org/proper/commons-statistics/download_statistics.cgi).
The checked-out release tag resolves to the following exact commit. Version
properties and production POMs require Numbers 1.3 and RNG 1.6.

| Source | Pin |
|---|---|
| Statistics 1.3 | `2937eb2e711483d8ea9dc216af45c16fd0066b77` |
| Numbers 1.3 | `ae8abefb51c2c4d1a05882d61cefc8015e493a8f` |
| RNG 1.6 | `618490cf3f072ddc327de90109fac04f404831cb` |

`scripts/prepare-commons-statistics-1.3.sh` checks these clean pins. The existing
Numbers pin and accepted producer are unchanged. RNG 1.0 preservation remains
required; its results cannot substitute for the newly required RNG 1.6 sources.
The new dependency is within the authorized complete production closure.

`scripts/commons-statistics-1.3-corpus.py` reads production POM dependencies,
checks version properties, walks their complete closure, inventories every
production Java file and stages byte-identical copies. Test dependencies are
excluded explicitly; unknown production dependencies fail rather than vanish.

| Module | Own Java files | Complete closure | Initial order |
|---|---:|---:|---|
| Ranking | 5 | 5 | First: small dependency-free architectural probe |
| Descriptive | 58 | 80 | Next: full baseline overlaps Ranking diagnosis |
| Distribution | 39 | 172 | After required Numbers and RNG 1.6 dependencies |
| Interval | 6 | 178 | After Distribution |
| Inference | 27 | 273 | After Ranking, Descriptive and Distribution |

Own sources total **135 files**, including package metadata. The complete union
is **279 files**: 135 Statistics + 71 Numbers + 73 RNG. Numbers production
dependencies are Core (11), Arrays (11), Fraction (6), Gamma (30), Combinatorics
(10) and Root Finder (3). RNG includes all client-api (8) and sampling (65)
production files. No RNG Simple/Core modules are production dependencies here.
Do not confuse test-only RNG dependencies with these production requirements.

The parent lists BOM and documentation aggregation modules with zero Java
production files. Examples (including benchmarks), distribution archives and
upstream tests/frameworks are outside production algorithm translation scope;
tests may inform independent acceptance probes. Field, Primes, Complex,
Quaternion and Angle are not new production dependencies, though preservation
of previously accepted modules remains required.

The complete dependency graph, source paths and SHA256 manifests are recorded
at `.tmp/statistics-1.3-entry-scope/production-scope.json` and
`production-sources.tsv`. All 279 sources compile together with javac; terminal
session 61955 exited zero. The corresponding `javac.log`, classes, argument file
and full `javap -p -s` inventories are retained in that scope directory.
Inventories include private, nested, anonymous and compiler-generated members:
Ranking has 7 JVM types, Descriptive 82, Distribution 48, Inference 45 and
Interval 12. Declaration presence is not public API invocation or behavior proof.

## Entry producer and baseline gates

Use the accepted frozen V423 producer:
`.tmp/new-java2rust-numbers-default-v423-null-invocation-provenance-debug`,
SHA256 `8b935a5652282a5b0a1e3274064c9ef9818d00030137dc980e2f755296d1a007`.
Every translator/runtime input still matches `.tmp/v423-producer-inputs.json`.
No translator fix preceded these entry baselines.

Ranking complete-source baseline:
`.tmp/commons-statistics-1.3-ranking-v423-entry`, log
`.tmp/statistics-ranking-v423-entry.log`, terminal exit **1**. All five files
generate, with zero unsupported members/statements/expressions and zero stubbed
constants. Generation takes 23.931 seconds; complete run 39.064 seconds.
Two parse failures and eight reported compile errors expose four root families
duplicated across ordinary and outer-handle method variants. Parse-invalid
output can mask later type errors; these counts are not the full remaining work.

Descriptive's complete **80-file** baseline completed at
`.tmp/commons-statistics-1.3-descriptive-v423-entry`, log
`.tmp/statistics-descriptive-v423-entry.log`, session **22928**, terminal exit
**1**. Its staged
closure is byte-identical and includes every Descriptive, Core and Arrays file.
All 80 files emitted; five have translation error status (DoubleStatistics,
IntMax, IntMin, LongMax and LongMin), five fail the Rust parse scan, and the
invalid-output compiler pass reports 172 errors. There are zero unsupported
members/statements/expressions, zero stubbed constants and zero reachable no-op
facade call sites. Invalid Rust means compiler totals are not a reliable count
of independent problems. The rewrite audits below guide the implementation.

The run takes 1,490.304 seconds: emission 1,105.528 seconds, global analysis
201.466 seconds and assembly 116.262 seconds; cargo check takes 5.388 seconds.
Do not spend another full run on each small fix. The largest emitted file,
UInt192, takes 144.724 seconds. Signature saturation itself takes 3.454 seconds;
constraint collection and plan construction, plus repeated lowering, are more
significant here than fixpoint iterations. First compare an optimized producer
against the same current debug source on accepted Arrays before another large
Statistics rerun; do not assume a speedup without checking generated output.

## Ranking whole-path rewrite audit

Generated examples below are from
`generated-crate/src/generated/00002_NaturalRanking.rs` in the entry baseline.
Hypothetical rewrites are diagnostic plans, not generated-code deliverables.

1. **Object-array comparator sorting (local call emission plus existing runtime
   contract).** `apply` emits `Arrays::sort(ranks, |a,b| ...)`, leaving an
   unresolved owner. `ranks` is a retained mutable array of nullable shared
   DataPosition references. The minimal correct rewrite sorts the same array
   allocation with the existing checked comparator/sort machinery; lend its
   payload briefly, unpack checked element receivers and preserve comparator
   failures. Preserve stable tie order and Java Double comparison semantics.
   Do not rebuild/replace the array to hide ownership or remove nullable slots.
   Trace the native Arrays call plan, callback input/result contract, retained
   array projection and shared sort kernel together. The preceding Arrays
   campaign already exercises checked collection comparators; reuse that
   mechanism rather than write a Ranking sort adapter.

2. **Reference-array copy and result joins (producer/signature/consumer
   contract).** `createNonNaNRankData` currently returns an owned
   `Vec<Option<Rc<DataPosition>>>`; `createMappedRankData` returns the retained
   array handle. `createRankData`'s conditional needs a single result carrier,
   but currently emits the intentional unsupported-allocation-change diagnostic.
   The copyOf path also assumes `.iter()` on its source and pads with Default.
   The correct rewrite preserves original array identity on no-copy paths,
   allocates exactly one fresh destination for copyOf, shallow-copies element
   identities and pads reference arrays with Java null. Existing
   `lower_java_arrays_copy_call_with_shape` and
   `fresh_java_array_value_for_slot` implement checked primitive copies and
   fresh-allocation attachment; extend/reuse their positive producer proofs.
   Trace result planning and conditional arms before choosing propagation of
   one canonical carrier or a proved unique-result attachment. Never relax
   arbitrary allocation-conversion guards or snapshot an aliased input.
   Reference rows, null padding and source mutation during bound evaluation
   need focused tests; a primitive-only extension is insufficient.

3. **String producer at a nullable exception-message boundary (local producer
   completion/slot adaptation).** The unreachable exhaustive-switch fallback
   contains a real Java `new IllegalStateException(String.valueOf(nanStrategy))`.
   Rust currently passes a scalar String directly to `from_constructor`, whose
   message parameter is `Option<JavaExceptionText>`. The planned String.valueOf
   shortcut in slot lowering returns before the destination conversion. Finish
   the existing selected String producer, then apply the shared logical-text
   and optional-slot contract. Do not patch only throwable constructors or
   assume all Java string payloads are scalar UTF-8. Regressions should include
   optional arguments/returns, explicit null versus the string "null", and
   logical text preservation. Source-owned methods named valueOf retain
   precedence over native JDK recognition.

4. **Expression-bound method references and default RNG (structured receiver
   contract plus reusable JDK runtime support).** `new SplittableRandom()::nextInt`
   is stored as textual MethodReference.target, then emitted as invalid Rust
   `new SplittableRandom()::next_int`. Existing bound-reference planning only
   recognizes named locals/fields. The receiver expression must instead be
   evaluated exactly once when the reference is created, retained by the
   callback and resolved through the ordinary instance-call contract. Creation
   exceptions and a null receiver occur at reference creation, not first call;
   invocation failures follow the existing fallible SAM contract. Preserve the
   same callback allocation through local `r`, `randomIntFunction`, returns and
   repeated shuffles. A source-level structured receiver must participate in
   resolution, binding/capture discovery, liveness and effect/representation
   analysis; reparsing raw text only in the emitter would lose those contracts.
   This is a bounded source-IR/planning extension, not permission to replace the
   Ranking algorithm. SplittableRandom is a separate missing native JDK runtime
   provider, requiring the real seeded mixing/bounded-generation semantics and
   persistent state; the existing Random runtime is not interchangeable.

Current framework can express the required retained arrays, nullable shared
elements, checked comparator kernels and stateful fallible SAMs. Receiver
expression metadata must be extended before the fourth family is correctly
expressible throughout analysis. No wider framework redesign is proven needed;
surface contrary evidence if the audit discovers one. The planned first batches
are array producer/result/sort contracts, native string slot completion, then
the structured bound-reference/JDK-provider contract. Batch related edits and
verify generic fixtures before repeating the full Ranking corpus.

## Behavior acceptance requirements

Ranking callers must exercise all constructor overloads and enum/native
interface operations, all NaN and tie strategies, stable order, signed-zero and
infinity ordering, repeated state, empty/all-NaN arrays, input preservation and
output alias/freshness. In particular FIXED with all NaNs returns the original
input array; other paths cannot be judged only by matching numeric values.
Use deterministic supplied IntUnaryOperator callbacks to compare exact random
tie outputs, callback bounds, order, shared state and errors. The default
unseeded JDK path is independently nondeterministic: test its permitted
permutation/range and lazy retained-generator contract, retaining raw draws,
rather than claim equal draws from independent processes. Seeded native JDK
provider regressions must compare exact sequences. This is a distinction in
the Java API contract, not a numerical tolerance or replacement algorithm.

Descriptive must audit mutable accumulator composition, recursive generic
statistic results, merge compatibility/aliasing, configured builders and
retained primitive consumers. Whole-program compilation alone does not prove
that a composed callback still updates the original accumulator. Its concrete
rewrite audit will be based on the pending generated output.

### First Descriptive generated-code audit

The first emitted `00025_DoubleStatistics.rs` already demonstrates why the
80-file baseline is useful even before compiler totals exist. These are
source/IR and contract observations, not a claim of complete diagnosis:

- `getResult` emits the real source expression
  `stat instanceof DoubleStatistic ? ((DoubleStatistic) stat)::getAsDouble : stat`
  as an unsupported trait-to-trait instanceof and a raw textual cast-method
  reference. This shares Ranking's structured expression-receiver family.
  The smallest correct result is a new StatisticResult view capturing the same
  underlying statistic allocation. It must forward reads to live state while
  preserving the source interface distinction: returning `stat` unchanged
  would expose DoubleStatistic mutation operations and change instanceof.
  Trace concrete implementers through stored result trait objects, runtime type
  tests/casts, receiver capture and generated functional-object interfaces.
  Reuse existing source hierarchy/erased identity and SAM wrapper mechanisms;
  audit their capability before choosing a common carrier or trait bridge.

- Nullable accumulator fields are moved by expressions such as
  `owner.lock().sum_of_logs.map(|value| Box::new(value) as Box<dyn StatisticResult>)`.
  A field borrowed through an owner cannot be consumed. The minimal correct
  rewrite clones the stored canonical allocation handle under a short guard,
  drops that guard, then adapts the handle to the result interface. Rebuilding
  or cloning an accumulator payload would detach observable state. The same
  issue occurs in combine argument lowering, including an escaping
  `&mut value` built inside an Option.map closure. Resolve whole producer /
  storage / signature / consumer contracts instead of adding another borrow.

- Fields show a mixture of `Arc<Mutex<Min>>`, `FirstMomentValue`, and bare `Sum`;
  the source contains both Numbers Core Sum and Descriptive Sum. Verify owner
  resolution and representation before interpreting bare Sum as correct or
  wrong. Native synchronization placement has already lifted retained mutable
  accumulator captures. The smallest correct composition preserves those
  handles through constructor parameters, fields, composed consumers, merge
  inputs and result suppliers, with no long-lived guard across callbacks.

- The emitted outer-handle `accept` currently lends a stored consumer through
  `owner.lock().consumer` while invoking it. Source consumers update contained
  accumulator handles, so a deadlock has not been demonstrated here. Still
  audit callback receiver extraction and self-combine before acceptance: Java
  permits aliases, and nested receiver/argument guards must not lock the same
  non-reentrant Rust mutex twice. Use owned handle staging and existing
  checked-reentrancy mechanisms, not a corpus-specific self-alias exception.

This is an additional cross-interface/view contract family, not merely many
small type errors. Finish its whole-path audit before starting a Descriptive
fix campaign. Ranking remains the first acceptance target; prioritize the
shared expression-bound reference contract once the array/text batches are
verified. Wider representation redesign is not yet established, and no module
has been narrowed to avoid these difficult paths.

Additional terminal evidence identifies naming/owner propagation as an early
architectural batch, before interpreting every representation failure as a
new ownership redesign. Both Core Sum and Descriptive Sum are already emitted
with distinct authoritative names (for example
`OrgApacheCommonsStatisticsDescriptiveSum`), yet some consumers still request
bare `Sum`. Inheritance diagnostics also qualify Descriptive moment subclasses
under `org.apache.commons.numbers.arrays`. Trace source canonical names and
per-owner import/package scope through resolution, signature shapes and type
rendering; determine whether that wrong qualification affects lookup or is
only diagnostic text. Reuse assigned names and owner descriptors rather than
adding imports or another collision-mangling policy.

The compiler families include 37 incompatible source SAM `apply` signatures,
36 unbound `get_as_double` calls, constructor/chained-expression method
references collapsed into fake type names, native Consumer interfaces treated
as traits despite being runtime structs, and retained Arc/Rc placement conflicts.
Some are consequences of invalid/omitted declarations; these numbers do not
prove equally many fixes. Quantile also emits a cast after a block without the
parentheses required in that Rust expression position, a separate local IR
emission issue. Root source-name, bound-receiver and contract fixes should
precede another whole-closure error census.

## V424 first implementation batch: native String producer completion

The Ranking exception-message family is fixed generally in `src/lower.rs`.
The selected native String producer is canonicalized through the existing
logical String carrier and then consumes the destination's shared slot plan.
The same helper runs before both ordinary slot shortcuts and the separate
source-call shaped-argument shortcuts. The first focused attempt fixed
optional returns/messages but exposed the borrowed-argument bypass; completing
only throwable constructor emission would have left that root cause intact.
`From<T> for T` retains existing logical payloads and String reference identity.

Source-owned factories retain their actual selected implementation. Native
argument planning and String producer recognition check emitted source owners;
the legacy simple-owner fallback now checks the existing canonical source-type
lookup for any source declaration, generalizing its earlier enum-only exclusion.
This matters for a source class whose simple name is String; adding an Into
conversion to its wrong scalar result would compile the wrong algorithm.

`NativeStringSlotsProbe.java` exercises enum valueOf results in exception,
optional and borrowed destinations, null versus the text "null", another
source factory named valueOf, unpaired UTF-16 units and String reference identity.
`SourceStringFactoryProbe.java` verifies a source-owned nested String factory.
Both executable regressions pass, along with seven exception-text and five
lossless-string tests: **14 tests**, terminal exit zero in
`.tmp/statistics-native-string-v424-contracts-r5.log`. Source changes do not
repair generated output or alter any pinned Java. Full module/preservation
acceptance is still pending.

`ReferenceArrayCopiesProbe.java` is prepared for the next contract batch; its
Java baseline returns zero. It covers shallow reference identity, null padding,
conditional return carriers, repeated nested-row aliases and mutations during
copy-bound evaluation. Its entry standalone Rust check reproduces allocation
and retained-source iteration failures. Two missing-ndarray errors in that
standalone check are harness dependency omissions, not product-error counts;
use the focused package runner's existing ndarray shim for that fixture when
testing it, rather than interpreting plain rustc as a complete build harness.

Frozen current debug producer:
`.tmp/new-java2rust-statistics-v424-string-slots-debug`, SHA256
`532dde2e5186d342462ed9e8750eb8238be6fefdcdea135754671b8ab15276a6`.
`.tmp/statistics-v424-producer-inputs.json` records source/runtime input hashes.
An optimized build of exactly the same source completed in 1m51s:
`.tmp/new-java2rust-statistics-v424-string-slots-release`, SHA256
`e5e38d98bf6e4e2c9cf380732d097a8094fe4c735bf21488246191f051adaad6`.
Both complete Arrays translation comparisons finished with exit zero. All
12 generated Rust files (eleven source modules plus crate root) are
**byte-identical** across debug/release producers. Debug translation takes
60.967s versus release 6.763s (9.0x faster); full pipeline 86.335s versus 28.053s
(3.1x faster). Terminal jobs and byte/timing comparisons are recorded in
`.tmp/statistics-v424-speed-terminal.json` and
`.tmp/statistics-v424-producer-speed-comparison.json`. All 5,052 independent
Arrays observations also match Java exactly on the current optimized producer:
`.tmp/statistics-v424-arrays-preserved-behavior`, terminal exit zero, log
`.tmp/statistics-v424-arrays-preserved-behavior.log`. This is a measured
producer build-profile improvement, not an algorithmic translator optimization
or a claim about all future corpus timings. Use the optimized frozen producer
for subsequent expensive checks.

The refreshed complete Ranking closure at
`.tmp/commons-statistics-1.3-ranking-v424-string-slots`, log
`.tmp/statistics-ranking-v424-string-slots.log`, completed in **13.047s**, exit
one. The exception-message type errors are gone. Six compiler errors remain,
duplicated across three planned families: object-array comparator sorting,
fresh-array/result allocation contracts, and expression-bound method references
with the missing SplittableRandom provider. Two parse failures remain in that
last family. This confirms the first product fix on the actual untouched
corpus without claiming Ranking acceptance. Work next on the coherent
reference-array producer/return/sort contracts, then the structured receiver /
JDK provider batch; reuse the shared receiver work for Descriptive afterward.

No Statistics module or dependency is accepted yet. Fresh earlier Numbers/RNG
preservation and a requirement-by-requirement final audit remain mandatory.

## V425–V427 reference-array contract batch

The reference-array failures shared storage and callback contracts across
producers and consumers. The implementation extends the existing mechanisms:

- Resolved returned-call results now connect method-return storage domains,
  including call results in conditional arms. A retained consumer therefore
  receives the same allocation contract from each producer.
- Native `Arrays.copyOf`/`copyOfRange` admit reference arrays. The new outer
  allocation remains fresh while copied children retain their actual nullable
  and shared storage domains. Nested arrays copy row handles, not row payloads.
- Local array aliases use the existing shared allocation domain. Conditional
  value flow connects each arm independently to its destination, including a
  named local array returned on one arm and a copied array on the other.
- Object-array comparator `sort` and `parallelSort` reuse the existing checked
  comparator kernel, mutation facts and callback input-shape propagation.
  Native recognition respects source-owned utility classes. Stored comparators
  receive the physical nullable element contract used by the array.

These are cross-function storage-contract changes and local native-call
planning/emission changes, with no new corpus-specific representation.
`ReferenceArrayCopiesProbe` covers fresh outer identity, shallow object and row
identity, null padding, bounds evaluation before reading the source payload,
conditional call returns and conditional named-local returns.
`ReferenceArraySortProbe` covers array aliases, stable ties, changed element
state, reused stored comparators, `parallelSort`, lazy null comparator handling,
non-comparable values and thrown comparator exceptions. `SourceArraySortProbe`
checks that a source-owned class named `Arrays` retains its own algorithm and
callback arity. Independent Java and translated Rust execute these fixtures.

The shadowed-binding unit test now introduces a synchronized late constraint:
ordinary local array aliases already seed unsynchronized retention, so the
stronger late constraint continues to prove binding-occurrence isolation.

V426 passed all 1,738 library tests and 30 focused/adjacent contracts (1,768
total), log `.tmp/statistics-v426-array-returns-r1.log`. The final V427 alias
consistency changes passed 17 affected contracts, including the expanded
`parallelSort` case, log `.tmp/statistics-v427-ordering-aliases-r1.log`.
The optimized V427 producer is frozen at
`.tmp/new-java2rust-statistics-v427-ordering-contracts-release`, SHA256
`77df5f017dc88b4ffc69ec180b14e15d394cd09c398ce8736a9e6e45ff26ccfb`.
`.tmp/statistics-v427-producer-inputs.json` records the current source/runtime
hashes. The final build recheck completed successfully without rebuilding.

Fresh V427 complete Numbers Arrays (11 files) and Fraction/Core (17 files)
closures compile, as do their Arrays null-boundary (12), Fraction extension
(18) and Fraction null-boundary (18) closures. Terminal commands/results are
in `.tmp/statistics-v427-generation-terminal.json`. Independent behavior
preservation is recorded separately below; compilation alone is not behavior
acceptance.

All ten V427 independent behavior jobs completed with exit zero:
`.tmp/statistics-v427-behavior-terminal.json`. Arrays matches 5,052 ordinary
observations plus 20 null-boundary and 20 exact exception observations (5,092
total). Fraction matches 3,551 observations across all seven caller modes.
The coverage audits map all 43 Arrays and 129 Fraction public/inherited
declarations to those observations:
`.tmp/statistics-v427-arrays-api-coverage.tsv` and
`.tmp/statistics-v427-fraction-api-coverage.tsv`. These 8,643 exact matches prove
this batch preserves those accepted closures, not acceptance of Statistics or
the remaining Numbers/RNG preservation closures. Other historical numerical
policies remain unchanged and must be rechecked at the next substantial/final
preservation boundary.

The complete unchanged Ranking closure now emits all five files with only two
compiler errors, both invalid raw expression-bound method-reference targets.
It has no unsupported members/statements/expressions, stubbed constants or
reachable facade no-op call sites. Parsing still fails, so its remaining type
error count is not authoritative and Ranking is **not accepted**. Current
output: `.tmp/commons-statistics-1.3-ranking-v427-ordering-contracts`, command log
`.tmp/statistics-ranking-v427-ordering-contracts.log`, total 15.509 seconds.
The previous six-error V424 result is historical evidence only.

## Next batch audit: expression-bound receivers

Ranking constructs `new SplittableRandom()::nextInt`. Descriptive additionally
uses `((DoubleStatistic) stat)::getAsDouble` and constructor/chained setter
receivers. The current `Expr::MethodReference { target: String, method }`
discards receiver structure in `parser::parse_method_reference_node`.
`resolve_expr_calls` cannot resolve calls inside that string, ordinary semantic
walkers see a leaf, and lexical emission mistakes the whole expression for a
type owner. Capture analysis separately reparses the string, losing the
authoritative resolved call identities. Fixing the printed path alone cannot
establish correct evaluation, retention or exception behavior.

The smallest correct conceptual rewrite for `make().configure(k)::next` is:

```text
receiver = evaluate make().configure(k) now;
requireNonNull(receiver) now;
callback = retain receiver's canonical handle;
on invocation(args): call receiver.next(args) through its selected contract;
```

The receiver's constructor, casts, field reads, argument effects and exceptions
belong to callback creation. Only `next` and its argument/result adaptation
belong to invocation. A later field or variable reassignment must not replace
the captured receiver. Repeated invocations must mutate the same allocation;
cloning the source payload is incorrect. A callback escaping its creator must
own that handle. Unbound `Type::method` and `Type::new` remain distinct from
bound expressions; capitalization alone does not resolve that distinction.

The existing `lower_bound_functional_method_reference` already evaluates a
receiver once, adapts an owned capture and synthesizes an ordinary typed call
inside a move callback. Extend that path with a frontend-retained receiver AST
and source overload resolution based on its actual type. Reuse the ordinary
source/native call and slot contracts rather than a new callback method-name
lookup or raw-text parser in the emitter. Preserve source target spelling only
for unresolved type-owner syntax and diagnostics.

The required whole path is:

1. Parse and retain receiver structure; nested-type qualification and call
   resolution visit it before callback adaptation.
2. Binding/capture analysis, inner-class discovery, ordinary and outside-lambda
   semantic walkers visit receiver evaluation at creation time. Usage/storage
   analysis connects its allocation to the escaping callback capture.
3. Call graph and exception planning distinguish creation effects from the
   referenced invocation; typed source overload resolution uses all SAM inputs.
4. Source and native SAM lowering capture once through the existing bound path.
   Stream callback plans must retain this structured information rather than
   reducing it back to `&str`.
5. Regression cases cover constructor, chained call, cast and field receivers;
   repeated state, escape, alias mutation, later reassignment, null/cast/
   constructor exceptions at creation, and overloaded source invocation.

This is an IR/cross-function contract extension, not evidence that arbitrary
manual rewrites or a new object representation are needed. It touches several
walkers and must be completed as a coherent batch. Descriptive's separate
trait-to-trait `StatisticResult` view remains an additional representation
contract to audit after receiver structure is available; returning the original
`DoubleStatistic` view would expose the wrong interface.

`SplittableRandom` also needs a reusable native JDK provider: constructor/state,
seeded mixing, bounded `nextInt`, alias retention and validation effects. Its
absence must not be hidden by replacing Ranking's algorithm or supplying a
different RNG in the generated corpus. Test seeded provider behavior against
Java independently; test unseeded Ranking with range/permutation/state
invariants and use the public deterministic callback overload for exact Ranking
algorithm comparisons. Default seeding is nondeterministic. Inspect primary
OpenJDK implementation evidence before implementing the provider.

## V428 structured receiver implementation and verification

The IR now retains an optional receiver expression alongside method-reference
owner spelling. Simple ambiguous names keep the established lexical/type-owner
resolution; expression receivers are parsed from tree-sitter nodes. Nested-type
and import qualification, call resolution, creation-time semantic/effect walks,
capture analysis, storage discovery, alias analysis and support discovery visit
the retained tree. Capture analysis no longer reparses the receiver spelling.
Stream callback plans also retain the expression rather than discarding it.

`lower_bound_functional_method_reference` reuses the retained expression and
ordinary source call resolution with all SAM input types. It stages and checks
the receiver once and retains the existing canonical allocation in the callback.
The call graph visits receiver creation before checking whether the callback's
invocation contract was already consumed, so consuming an adapter does not
erase constructor/cast/setter effects. Typed callback invocation edges use the
same resolved receiver and overload contract.

`BoundExpressionReferenceProbe` independently executes Java and generated Rust.
It covers escaping constructor/chained receivers, repeated mutation, a cast
receiver sharing state with its caller, field replacement after capture,
overload selection, division failure and null failure at creation, a
source-defined SAM and a native String expression receiver. This passed in
`.tmp/statistics-v428-bound-receivers-r1.log` and the expanded version passed
in `.tmp/statistics-v428-receivers-batch-r2.log`.

The first broad batch passed all 1,738 library tests, five native SAM, two
retained-state, seven source-callback and six Statistics contracts. One of two
existing bound-stream tests failed on nullable String equality: a borrowed
`&JavaUtf16String` was compared with an owned String carrier. A diagnostic
translation of that exact original fixture using the **frozen V427 producer**
reproduces both compiler errors, proving this predates the receiver extension:
`.tmp/statistics-v428-stream-prior.java`, `.tmp/statistics-v428-stream-prior.rs`,
`.tmp/statistics-v428-stream-prior-check.log`. This diagnostic output is not a
deliverable and was not manually corrected.

The fix retains an owned clone of the canonical Java String handle when
creating that bound stream callback. Java identity and UTF-16 contents remain
shared through the existing String runtime; native equality receives its usual
carrier. This also avoids retaining a borrow of the nullable local container.
The final regression batch completed successfully: all 1,738 library tests
and 22 affected/adjacent contracts pass (1,760 total), including both bound
stream tests. Logs: `.tmp/statistics-v428-receivers-batch-r3.log` and
`.tmp/statistics-v428-release-build.log` (optimized build also exits zero).
Frozen producer: `.tmp/new-java2rust-statistics-v428-bound-receivers-release`,
SHA256 `11050f2586eb50ac698fa47c990b036ccf47cf4e0a5b2a81093f459c5dee5ba2`;
source/runtime hashes: `.tmp/statistics-v428-producer-inputs.json`.

All seven fresh generation jobs are terminal, commands/results in
`.tmp/statistics-v428-generation-terminal.json`. The five Arrays/Fraction
preservation closures compile and pass their existing source/placeholder
gates. All 81 generated Rust files and all five Cargo manifests are
byte-identical to V427. Runtime hashes, Java inputs, independent callers and
verifier scripts are unchanged, and all ten earlier behavior jobs exited zero.
The equivalence proof `.tmp/statistics-v428-preservation-equivalence.json`,
reproducible using `.tmp/statistics-v428-preservation-equivalence.py`, therefore
reuses their 8,643 exact observations and 172 public API mappings. This avoids
rerunning unchanged callers; it does not extend their coverage to Statistics.

Ranking now has **zero parse failures**. The two remaining type errors both
come from `SplittableRandom.nextInt`: the generic unmodelled facade returns
`()`, while the native `IntUnaryOperator` correctly requires `i32`. The two
calls are now explicitly counted as reachable unmodelled facade sites. Output:
`.tmp/commons-statistics-1.3-ranking-v428-bound-receivers`; generated
`NaturalRanking` stages `SplittableRandom::new()` once per callback, then calls
the retained receiver on invocation. Thus the receiver IR/capture fix is
confirmed on the actual corpus, while the missing JDK provider still blocks
compilation and acceptance.

This does not establish support for every expression-bound stream callback:
the stream renderer still has separate capture/slot contracts to validate for
fresh non-null receivers and escaping pipelines. The current end-to-end probe
proves native/source SAM capture paths. No Statistics module is accepted, and
the SplittableRandom provider remains absent pending the next implementation
batch.

### Refreshed Descriptive audit: inherited interface contracts

The complete 80-file closure at
`.tmp/commons-statistics-1.3-descriptive-v428-bound-receivers` finished in
261.476 seconds (optimized producer), compared with the historical 1,490.304
seconds debug baseline. It still has five error-status files, five parse
failures and 172 reported compiler errors. Counts are not independent failure
families while parsing fails. Source files remain unchanged.

`StatisticResult` extends the native `DoubleSupplier`, `IntSupplier` and
`LongSupplier` interfaces. It supplies default integer/long conversions and a
default BigInteger conversion, leaving **inherited** `getAsDouble` as its sole
abstract operation. The generated `StatisticResult` trait omits that inherited
operation, and default bodies emit bare `get_as_double()` calls. Consequently
the structured expression references in `DoubleStatistics` still bypass the
typed bound-reference adapter: `source_lambda_contract` calls
`functional_interface_method`, which only searches direct abstract declarations.
`resolve::is_source_functional_interface` separately counts direct non-static
methods, including defaults. These are inconsistent approximations of the same
Java interface contract.

The correct next architectural batch computes a shared effective interface
method set across source and native ancestors, specializes generic signatures,
applies default/abstract overrides, and derives the SAM descriptor from the
remaining abstract operations. Use that same evidence for resolver applicability,
call graph/callback contracts, emitted trait requirements/default-body calls,
implementation forwarding and nominal callback adapters. The inherited
`getAsDouble` requirement must appear in the generated trait, and default
conversion bodies must invoke it on the same receiver. Do not add an explicit
declaration to the pinned Java, pick an arbitrary first parent, or teach each
consumer a separate `StatisticResult` exception. General probes should include
source/native mixed ancestry, default overrides, multiple parents, generic
specialization and overloaded non-SAM methods before refreshing this module.

This is a cross-function interface contract extension; the current concrete
source/native SAM probe does not prove inherited SAM support. The separate
trait-to-trait result view and source-name/storage families from the entry
audit still need fixes after the effective interface descriptor is available.

### JDK provider follow-up

The existing generated `Random::next_int_result` validates its argument but
returns zero for every valid bound (`stdlib::external_facade_default_method`).
It is not a validated random provider and must not serve as an implementation
template for SplittableRandom. The complete Statistics/RNG dependency import
scan currently finds `SplittableRandom` only in Ranking; `RandomAccess` is a
separate marker interface, not an RNG import. Build real shared-state JDK
support and independent seeded/validation/state tests. Primary algorithm
evidence inspected so far:
[OpenJDK SplittableRandom](https://raw.githubusercontent.com/openjdk/jdk8u/master/jdk/src/share/classes/java/util/SplittableRandom.java).
Confirm the oracle JDK version's behavior before fixing a seeded compatibility
contract. The no-argument constructor remains nondeterministic; its acceptance
checks must exercise actual random state rather than require identical seeds.

## V429–V430: reusable SplittableRandom and assembly contracts

`src/java_splittable_random.rs` supplies a canonical shared-state JDK carrier
`JavaUtilSplittableRandom`. Rust Clone retains one allocation; `split()` creates
an independent generator. The modeled production surface is default/seeded
construction, all three `nextInt` forms, unbounded `nextLong`, and `split`.
Integer mixing, wrapping state advancement, bounded rejection and invalid-range
validation are real implementations. Validation precedes state mutation. This
is reusable JDK support, not a replacement for any Ranking algorithm. Other
SplittableRandom operations are not claimed as implemented.

The default constructor initializes process-wide seed advancement from time
and process state. Its values are nondeterministic; seeded compatibility is
checked against the Java oracle. No claim is made about JVM-specific default
seed values or security-property behavior. The existing unrelated Random
zero-return facade is unchanged and remains unsuitable as an RNG implementation.

The shared native-reference carrier authority now covers this runtime alongside
the existing internally shared text carriers, with source/user precedence.
Alias diagnostics and owned recovery use that proof. The same carrier registry
keeps explicit platform qualification through parsing. Scope-aware API owner
selection preserves a qualified value owner when shortening it would rebind
to a different lexical source type. Native constructor overload identity uses
the existing arity contract, independent of which overloads a particular caller
happened to exercise. Source-owned and explicitly qualified native random types
are tested together in `SourceRandomProbe`.

Facade argument planning also honors declared primitive parameter metadata:
byte/short/char operands widen through the ordinary slot conversion instead of
reaching an int-taking runtime as i8/i16. ABI ownership/view selection remains
authoritative. A generic `NumericFacade` unit control verifies widening and
refuses implicit long/double/boolean narrowing; the random probe checks an
actual byte-bound call and an int seed widened to long.

`SplittableRandomProbe` runs independently on OpenJDK **19.0.2+7** and through
the translator. Seven seeded trace folds cover 7,616 returned draws, with power
of two and rejection bounds, signed-width-overflow ranges, split children and
parent continuation. These are seven exact folded traces, not 7,616 separately
compared rows. It also checks aliases, receiver identity, escaping bound
callbacks, invalid arguments/messages without state advancement, and default
draw ranges. Oracle input/command/version provenance is in
`.tmp/statistics-v429-random-oracle/provenance.json`; current-source oracle
recompilation reproduces all recorded folds exactly.

### Private associated constants in shared support

The first complete Ranking check exposed an assembly boundary missed by the
standalone runtime fixture: `merge-shared-support-facades.py` extracted only
single-line `pub const` definitions, dropping the provider's private
`GOLDEN_GAMMA` while promoting the methods that use it. The fix preserves
private/restricted constants and complete multiline initializers, visits only
impl-level declarations, and excludes method-local constants and `const fn`.
It reuses the cached structural scanner in `rust_item_utils.py`; immutable
delimiter/terminator results keep caching usable. No constant was moved or
duplicated into generated corpus code.

All 14 assembly controls pass, including actual merge/compile/run checks for
private and multiline constants:
`.tmp/statistics-v430-assembly-contracts-r2.log`. Reassembling a copy of the
frozen V429 Ranking output using `--resume-assembly` alone changes its three
missing-constant errors to zero compiler errors:
`.tmp/commons-statistics-1.3-ranking-v429-private-constants-resume` and
`.tmp/statistics-ranking-v429-private-constants-resume.log`. This isolates the
assembly correction without retranslation or manual Rust repairs.

### Current producer and verification

Frozen producer: `.tmp/new-java2rust-statistics-v430-random-contracts-release`,
SHA256 `751dc4f41b369ba882a88f53c9fac6d13009cb901752896b03db52e93e597d4f`.
`.tmp/statistics-v430-producer-inputs.json` records producer, runtime and assembly
inputs, including the new runtime source. After the build, one incorrect test
assertion was corrected: a lexical source declaration shadows the short name,
while the explicitly qualified JDK owner stays distinct. The frozen build's
exact source snapshot and byte-identical production prefix prove this is a
`cfg(test)`-only difference; the release producer was reused deliberately.

The complete library run passed 1,739 checks and failed that incorrect added
assertion; the corrected exact unit then passes:
`.tmp/statistics-v430-final-contracts-r2.log` and
`.tmp/statistics-v430-source-precedence-unit-r4.log`. Together they cover all
1,740 library checks. The intervening `r3` unit command selected zero tests and
is not verification evidence. All 24 adjacent/Statistics contracts pass on the
current source, including mixed source/native ownership:
`.tmp/statistics-v430-adjacent-contracts-r3.log`.

All seven fresh generation jobs are terminal:
`.tmp/statistics-v430-generation-terminal.json`. The complete original Ranking
closure now passes translation, parsing, compilation and placeholder gates:
`.tmp/commons-statistics-1.3-ranking-v430-random-contracts` (five files, zero
unsupported/stub/invalid/reachable-unmodelled sites). The five Numbers
preservation closures also pass. Their 81 Rust files and Cargo manifests remain
byte-identical to V427; runtime, pinned inputs, independent callers and scripts
are unchanged. `.tmp/statistics-v430-preservation-equivalence.json` reuses the
8,643 exact observations and 172 API mappings on that evidence. This proves
those closures only; broader final preservation is still outstanding.

### Independent Ranking behavior and remaining API boundary

The maintained independent callers `RankingOracle.java` and `ranking_oracle.rs`
invoke the generated production algorithms. Their **207 rows match exactly**:
`.tmp/statistics-v430-ranking-core-behavior/{java.tsv,rust.tsv,diff.txt}`. The
206 detailed rows cover all 25 NaN/tie strategy combinations over eight input
families plus all six public constructors. Observations include exact IEEE
output bits, source-provided exception messages as UTF-16 units, input
preservation, output aliasing (including all-NaN FIXED input), getters, repeated
use and expanding tie storage. RANDOM is exact through the public deterministic
callback overload. One further row checks default native randomness over 16
repeated tie permutations with input preservation; it does not compare
nondeterministic permutations between processes.

Ranking is **not accepted yet**. A separate translated `RankingBoundary` client
is valid on Java (returns zero), but the full six-file translation still has
16 compiler errors:
`.tmp/commons-statistics-1.3-ranking-v430-boundary`. This client exercises typed
null constructor arguments and their validation order/messages, null input,
source RankingAlgorithm dispatch, native UnaryOperator/Function views, and
inherited compose/andThen. The runner's `STATISTICS_EXTRA_SOURCE` adds such a
caller beside the unchanged complete production closure; it never replaces or
edits pinned sources.

Representative contract failures and smallest required rewrites:

- `UnaryOperator<Option<Vec<f64>>>` is assigned directly to
  `Function<Option<Vec<f64>>, Option<Vec<f64>>>`. A real parent-interface view
  must retain the same callback allocation and its input/result ABI.
- Stored native SAM generic array children use value Vec slots while generated
  Ranking producers/consumers use retained Rc array handles. Connect the source
  method parameter/return domains to the SAM input/result child domains rather
  than snapshotting the arrays or wrapping an unrelated outer allocation.
- Function compose/andThen are absent. Their adapters must retain both callbacks,
  validate null arguments at composition time, invoke them in Java order,
  release each callback guard before invoking the next, and propagate results,
  exceptions and generic child domains through the same contracts.
- Nullable constructor probes widen stored enum fields, but several comparisons
  still emit `Option<NaNStrategy> == NaNStrategy::FIXED`. Carry the actual field
  slot into comparison planning; preserve null checks and validation order
  rather than suppressing nullable public inputs.

These join the effective inherited-interface/SAM contract batch already exposed
by Descriptive. No evidence yet requires replacing the object framework; the
needed representations exist, but parent-view and producer/storage/consumer
contracts must be connected. Full operation/overload/inherited API coverage,
stateful/throwing/reentrant callback cases, enum inherited operations and a
declaration/API/behavior coverage audit remain before Ranking acceptance. All
other Statistics modules and the full goal remain open.

## V431: functional array domains and imported enum identity

The additional Ranking client now has **three compiler errors instead of
sixteen**. The complete six-file run is
`.tmp/commons-statistics-1.3-ranking-v431-boundary`; translation and parsing
complete with no unsupported statement/member/expression or invalid-pattern
sites. The remaining errors are the UnaryOperator-to-Function parent view and
the missing Function compose/andThen operations. This is still a failing API
boundary, not module acceptance.

The fixes connect existing authorities through the whole invocation path:

- `FunctionalTypeSource::type_argument_projection` describes both ordinary
  generic invocation values and applied values such as Iterator<T>.
- Usage connects native SAM inputs to source implementation parameters and
  source returns to SAM result children. Native SAM views map input/result
  positions rather than assuming equal generic argument indices. Invocation
  arguments enter the same graph, including literal-null inputs and retained
  fresh arrays. Ordinary non-SAM calls avoid the extra storage-environment
  construction.
- Array/reference retention and nullability use the existing signature storage
  graph. Facade rendering now consumes its solved native SAM child allocation
  requirements. Callback parameter/result lowering and invocation result
  inference preserve those same qualifiers through a shared projection helper.
  Creating an adapter describes the qualifiers it actually emitted instead of
  reverting to an unqualified nominal facade.
- Enum static variants carry their resolved source declaration identity. A
  cross-file import alias previously prevented the equality planner from
  recognizing that an optional field and a required variant had the same Java
  enum type. The comparison now uses its existing optional-value plan; this
  does not suppress null constructor inputs or validation.

The new maintained regressions include a source Function<int[],int[]> that
mutates and retains inputs, returns the original array, is reused through a
callback alias, observes subsequent writes, and throws on null without
replacing retained state. Independent Java and generated Rust callers return
zero. A separate three-file/two-package enum fixture reproduces the imported
nullable-field comparison, with an additional same-file control. Both enum
fixtures check valid values and null rejection/message behavior. The initial
same-file control passed before the fix; the imported case failed with two
Option/enum comparison errors and now passes.

All **11 Statistics contracts** pass in
`.tmp/statistics-v431-storage-contracts-r4.log`. All **16 adjacent native/source
callback, retained-state and bound-stream-reference contracts** pass in
`.tmp/statistics-v431-adjacent-contracts-r1.log`. The focused usage-graph test
passes in `.tmp/statistics-v431-storage-usage-r2.log`. Earlier r1/r3 runs record
the reproductions and an intermediate compiler ownership error; they are not
passing verification evidence.

The complete library run passes 1,740 tests and fails only the strict
`stored_sam_result_flows_use_canonical_destination_slots` expectation, which
previously omitted the newly connected input edge. The updated test explicitly
checks that contravariant edge and retains the original result-edge assertions;
its exact focused rerun passes. Together these cover all 1,741 library checks:
`.tmp/statistics-v431-library-contracts-r1.log` and
`.tmp/statistics-v431-canonical-storage-unit-r2.log`. Current terminal evidence
and remaining work are recorded in `.tmp/statistics-v431-batch-evidence.json`.

Frozen debug producer:
`.tmp/new-java2rust-statistics-v431-functional-storage-debug`, SHA256
`a323a63ddb144aef0a4271d68e4dbbeeabfe496fe6fcea3b96f20f2825d44a26`.
`.tmp/statistics-v431-producer-inputs.json` records its current source/runtime
and assembly inputs. After the build, the strict storage-graph unit was updated
to assert the new Function input edge separately from the original result
edges. Its exact build-time source snapshot and identical production prefix
record this deliberate `cfg(test)`-only difference. No pinned Java or generated
deliverable was edited.

The complete original five-file Ranking closure still passes all
source/parse/compile gates at `.tmp/commons-statistics-1.3-ranking-v431-core`.
Its six Rust files and Cargo manifest are byte-identical to V430, and its
pinned Java, independent callers and external runtime are unchanged.
`.tmp/statistics-v431-ranking-core-generated-equivalence.json` proves reuse
of the **207 exact Ranking observations** on that basis. This does not establish
the still-failing additional client's inherited operations, nor broader final
Numbers/RNG preservation.

The next architectural batch remains native functional inheritance and
composition: retain callback identity when exposing a parent interface, and
connect composition inputs/intermediate/results, retained state, eager null
validation and deferred exceptions. It should reuse typed callback lowering
and shared functional metadata, and verify stateful/throwing/reentrant paths.
The solved array/enum contracts above are now available to that batch. The
effective inherited source SAM descriptor needed by Descriptive remains open.

In the current generated client, composition still declares unqualified result
children (`Function<Rc<RefCell<Vec<f64>>>, Vec<f64>>`) while the existing
Function receiver has optional retained-array children. Merely adding Rust
methods would leave that producer/storage/consumer mismatch. The next batch
must model the generic Java contracts and their domain relationships:

| Operation | Invocation/storage relationships |
| --- | --- |
| UnaryOperator<T> to Function<T,T> | One retained callback and the same Java identity; input/result domains follow SAM positions. A null view remains null. |
| Function<T,R>.compose(Function<V,T>) to Function<V,R> | Composed input enters `before`; its result enters the receiver; the receiver result becomes the composed result. |
| Function<T,R>.andThen(Function<R,V>) to Function<T,V> | Composed input enters the receiver; its result enters `after`; the latter result becomes the composed result. |

Use the existing generic-call inference/expected-slot machinery where it can
express V and wildcard bounds; audit that boundary before introducing another
inference path. The composed object has a fresh identity and retains both
callbacks. Validate its supplied callback at creation, propagate invocation
errors in order, and release each invocation guard before the next call.
Exercise using the same stateful callback in both positions, independently
retained aliases, null arguments and failures in each stage. Audit nested
reentry against the actual local/synchronized callback representation rather
than assuming that sequential guard release alone proves all reentrant uses.

## V432: native functional parent views and assignment domains

The complete six-file Ranking boundary now has **two compiler errors**, both
missing Function compose/andThen methods:
`.tmp/commons-statistics-1.3-ranking-v432-boundary`. Translation and parsing
finish without unsupported statement/member/expression sites. This is not
Ranking acceptance; no other module is accepted by this result.

`NATIVE_FUNCTIONAL_PARENTS` owns the generic parent projections for
UnaryOperator<T> to Function<T,T> and BinaryOperator<T> to BiFunction<T,T,T>.
Hierarchy lookup and lowering share it. Native views reuse the source-object
typed SAM adapter, retain the actual solved receiver shape, invoke its ordinary
method and preserve its Java reference-identity token. They preserve null as
null and do not eagerly invoke the callback. Source-aware native-carrier
selection still rejects source-owned functional lookalikes.

Cross-interface native callback comparisons now reuse the existing staged
identity comparison path. Copying each callback handle before evaluating the
next operand preserves Java evaluation order even when the second operand
reassigns the first callback's original binding. These canonical facade handles
do not acquire an extra Rc/RefCell wrapper.

That reassignment probe exposed a general storage omission: statements recorded
native value-domain transfers, but nested assignment expressions did not.
Both now call `record_native_assignment_value_domains`; an assignment's result
uses the target slot and forwards its RHS reference domain. A later replacement
callback consequently has the same solved generic array ABI as the binding it
replaces. No generated-code patch or array snapshot is involved.

`FunctionalParentViewProbe.java` checks unary/binary parent views, repeated
conversions and identity, retained array input/result identity and mutations,
null-view creation/invocation, and reassignment during identity comparison.
The independent Java and generated Rust tests pass. All **28 integration
contracts** pass in `.tmp/statistics-v432-parent-contracts-r4.log`, and all
**80 usage-analysis units** pass in
`.tmp/statistics-v432-usage-contracts-r1.log`. Earlier r1/r2 runs are failure
reproductions; the focused r3 verifies the final parent-view probe.

Frozen producer: `.tmp/new-java2rust-statistics-v432-parent-views-debug`, SHA256
`1ef8bdb431b5fef910828d0b853675d1fe8a901a927e531fa59f758ea4d47f39`.
`.tmp/statistics-v432-producer-inputs.json` records exact current source/runtime
and assembly hashes; there is no source-difference exception for this producer.
Broader final Numbers/RNG preservation remains outstanding.

### Composition entry gate: additional failing contract families

The production Descriptive `Statistics.composeDoubleConsumers`,
`composeIntConsumers` and `composeLongConsumers` repeatedly use primitive
consumer andThen. Composition is therefore a production shared root, not only
an inherited-API stress case in the Ranking caller.

Three maintained **diagnostic** callers are additional inputs, outside the
pinned production closures. They are not yet registered as passing integration
contracts and their current generated failures remain required work:

| Diagnostic | Evidence and purpose |
| --- | --- |
| `composition-audit/FunctionalLambdaDomainsAudit.java` | Array identity lambda, nullable input/result, source-call forwarding, and source side effects before null dereference. |
| `composition-audit/FixedCompositionAudit.java` | Primitive consumers used by Descriptive, same callback in both stages, fresh composite identity, creation-time null checks, deferred stage order/errors, and primitive unary compose/andThen order. |
| `generic-composition-audit/GenericCompositionAudit.java` | Function<String,int[]> chained with Function<int[],String>, with an intermediate retained array absent from the composed Function<String,String>; both composition directions, repeated retained state, creation-time null validation and errors in either stage. |

All three return zero under Java. Exact commands, source hashes, Java version
and outputs are in `.tmp/statistics-v432-composition-audit-java-oracles.json`.
The two-file translation `.tmp/statistics-v432-composition-audit` has nine
compiler errors: seven missing primitive composition methods, one null-result
comparison placeholder, and one dangling borrowed return. The separate generic
translation `.tmp/statistics-v432-generic-composition-audit` has four missing
Function composition method errors. These counts describe these probes only.

Representative generated-code audit and smallest correct rewrites:

- The identity lambda currently has physical type
  `Function<Option<Rc<RefCell<Vec<i32>>>>, Rc<RefCell<Vec<i32>>>>` and unwraps its
  input before returning it. Java permits the identity function to return null.
  Its result must share the input's optional retained-reference domain, and its
  body must return that reference unchanged.
- The forwarding lambda unwraps its optional input before calling `update`.
  Java `update` increments `calls` before dereferencing the input. Its source
  parameter must receive the nullable domain; null rejection belongs at the
  dereference after the increment, not at the adapter boundary.
- `update` currently takes an owned Rc but returns `&'a Rc` by emitting
  `Ok(&input)`, borrowing its owned local parameter. It must return an owned
  copy of the canonical handle. Trace that contract through source return
  planning, the forwarding lambda result and the final facade consumer.
- Generic compose/andThen require method variable V inferred from the expected
  result and callback argument. `resolve_lambda_with_expected_type` currently
  handles only a direct lambda; it does not apply that expected type to a native
  generic composition call. External effect facts express receiver placeholders
  but have no method-binder declaration. Reuse the resolver's scoped
  `collect_method_type_substitutions`/substitution machinery, with an explicit
  native method contract, rather than inventing binder inference in emission.
- Distinguish input, intermediate and result value domains. The generic probe
  makes the intermediate array disappear from the result's Java type, so
  assigning every callback domain to the outer result's generic child indices
  cannot work. Build contextual callback parameter/result projections from the
  actual receiver/argument/result endpoints, and preserve them through source
  calls and callback-local aliases.

This remains a cross-function contract extension using representations that
already exist, not evidence for replacing the object framework. First connect
callback-local input/result domains and repair owned-handle returns; then add
the shared composition contracts, generic contextual inference, native/runtime
implementation and creation/invocation effect separation as a coherent batch.
The current usage code intentionally excludes callback-local names from capture
flows; removing that guard would incorrectly resolve a lambda parameter to a
same-named enclosing binding. Keep lexical identities explicit. Existing
call-graph lambda identities and signature CallArg/CallResult nodes are reuse
candidates; those signature nodes currently do not own mutable storage
constraints, so merely pointing a new flow at them is insufficient. Audit
whether projected existing endpoints suffice before adding stored call-site
domains. Preserve local/synchronized capture requirements and test nested
reentry before claiming complete composition support.

## V433: scoped callback value domains and canonical retained returns

The V432 lambda-domain diagnostic is now a passing integration contract,
including additional alias, rebinding and nested-capture cases. This batch
extends the existing signature/storage graph rather than adding a separate
callback solver:

- `LambdaParam` now carries an optional `BindingId`. Resolution assigns source
  lambda parameters and declarations inside their bodies identities in the
  enclosing callable's lexical local frame, including expressions, nested
  blocks and anonymous members. Target typing retains inferred Java parameter
  types on the IR. Capture frames and lowering environments reuse those IDs.
  Synthetic/idless compatibility inputs retain their conservative fallback.
- Signature local plans include lambda parameters. Native SAM input children
  connect to those parameter slots, and callback returns connect to the SAM
  result slot. Source calls and local assignments then use the ordinary
  declaration graph, so nullability and retained array storage reach producers,
  local aliases, source parameters/returns and final consumers. Rebinding also
  uses that graph and the existing mutable-binding policy.
- Callback result destinations are scoped during usage traversal. Transient
  body-identity plans are consumed on entry and cleared before publishing facts;
  a callback return is not recorded as a return from its enclosing factory.
  Callback-local names are not guessed to be enclosing captures.
- Retained native return slots materialize owned canonical handles. A source
  method taking Rc/Arc by value can no longer emit a borrowed return into that
  local parameter. This changes return ownership, not Java allocation identity
  or the rules for borrowed invocation parameters.
- Reusing ordinary value transfer exposed a missing root-presence edge for
  List-to-Iterable views. That shared path now preserves nullable producers as
  well as descendant element storage. The deferred-source regressions check
  nullable results, post-capture mutation and creation/invocation timing.

`FunctionalLambdaDomainsAudit` now checks nullable identity, source forwarding
with a side effect before null dereference, callback-local aliases, parameter
and local rebinding, and a same-named static field. It also checks a parameter
escaping into a returned Supplier: null stays null, the returned array is the
original allocation, and later mutation remains visible. The generated
`update` accepts/returns an optional owned Rc; it increments `calls` before its
checked dereference. No adapter unwraps the input early.

The broader tests also exposed a **pre-existing** nested bound-reference bug.
The V432 producer reproduces E0521 at
`.tmp/statistics-v432-bound-field-baseline`. `() -> field::method` retained the
owner in its valid handle companion, but incomplete self-read classification
left an invalid `&self` entry point emitted too. Self-read analysis now uses
the canonical bound-reference receiver resolver, seeds lambda parameter scopes
and excludes shadowed/static names. The existing companion-selection rule can
therefore remove the invalid borrowed entry point. Direct `field::method`
still captures its current value; the nested lambda reads the field later.
`BoundFieldReferenceValues` verifies that distinction and replacement/null timing.

### Current verification and producer

- **1,741 library tests pass** on the final source:
  `.tmp/statistics-v433-final-library-contracts-r6.log`.
- **43 callback/view/stream integration contracts pass**:
  `.tmp/statistics-v433-final-callback-contracts-r5.log`. The subsequent extended
  nested-parameter regression also passes:
  `.tmp/statistics-v433-nested-callback-domain-r6.log`.
- **18 Arrays/Fraction adjacent contracts pass**:
  `.tmp/statistics-v433-numbers-adjacent-contracts-r6.log`. These are focused
  preservation checks, not the final full Numbers/RNG preservation audit.
- All 33 resolver units and 27 capture-filtered library units pass as additional
  focused evidence. The earlier full r1 run found three failures: a missing
  Iterable root-presence edge and structural assertions that did not account
  for the newly represented null seeds/lambda parameters. The product edge was
  repaired; the assertions now explicitly check the new domains, reverse input
  edges and absence of outer-field capture. Full r4 and final r6 runs pass.

Final frozen producer:
`.tmp/new-java2rust-statistics-v433-scoped-views-debug`, SHA256
`c23daf07587f76a2b0d704d3dc8854fec9660d51bdd5bbcac7b0ca68dd9e3db4`.
`.tmp/statistics-v433-scoped-producer-inputs.json` records exact current
source/runtime/assembly inputs, with no source-difference exception. The earlier
`v433-callback-domains-debug` producer and its manifests describe an intermediate
revision and are not the final producer.
`.tmp/statistics-v433-scoped-batch-evidence.json` collects the final terminal
states, verification counts, source hashes and remaining acceptance work.

The complete five-file Ranking core passes again at
`.tmp/commons-statistics-1.3-ranking-v433-scoped-core`. Its six Rust files,
Cargo manifest and lockfile are byte-identical to V430. More strongly, fresh
independent Java/Rust callers match **207 rows exactly** at
`.tmp/statistics-v433-scoped-ranking-core-behavior`. The maintained
`scripts/verify-commons-statistics-ranking.py` checks source/compile gates and
the complete production-source manifest, freezes callers and hashes, records
tool versions/commands and compares exact observations. This is core behavior
coverage; inherited API acceptance remains separate.

The aggregate scan still reports the same 18 unmodelled support call-site
markers and 62 support upcasts as V430, all attributed to
`generated-shared-support.rs` by `idiom-sweep.tsv`. No corresponding
unimplemented/compile-error markers occur in the emitted production modules.
The core behavior verifier does not establish global support reachability;
the final module acceptance audit must still prove that required public paths
cannot reach fake support bodies.

The expanded six-file Ranking client still has **two missing Function
composition methods**:
`.tmp/commons-statistics-1.3-ranking-v433-scoped-boundary`. The refreshed
two-file composition audit has **seven missing primitive composition methods**
and no null-comparison/borrowed-return placeholders:
`.tmp/statistics-v433-scoped-composition-audit`. Its lambda fixture has expanded
since V432, so the old and new diagnostic counts are not a same-input benchmark.
All four current Java audit callers return zero; hashes and observations are in
`.tmp/statistics-v433-audit-oracles/observations.json`. No Statistics module is
accepted yet.

### Next coherent batch

Add shared native composition contracts and expected-type method-binder
inference, reusing the resolver's scoped substitution machinery. The new
callback parameter slots make it possible to connect an inline callback input
and result to different existing endpoints: compose feeds the argument's result
into the receiver input; andThen feeds the receiver result into the argument
input. Do not overlay both on the composed result's generic positions: the
generic audit's intermediate array is absent from Function<String,String>.
Use actual planned callback parameter/result shapes and ordinary typed calls
when physical nullability differs across the invocation boundary.

Composition creates a fresh callback identity and retains both operands.
Keep synchronization/capture demands separate from identity and from unrelated
generic child positions. Preserve receiver/argument evaluation order, eager
null validation, deferred stage effects/errors and guard release between calls.
Exercise the fixed-signature consumer families used by production Descriptive,
generic type-changing Function chains, aliases and reentry. The effective
inherited source SAM descriptor and all remaining production modules/dependencies
remain required work.

## V434: primitive composition and virtual default-method boundary

The next rewrite audit found a production requirement beyond merely adding
`andThen`: `Statistics.DOUBLE_NOOP`, `INT_NOOP` and `LONG_NOOP` are anonymous
consumers that override it to return the supplied callback. Replacing those
overrides with the interface default would change identity, allocation and
null behavior. This is a cross-function adapter contract, not a change to the
pinned algorithms.

The implementation batch introduces shared operation metadata for the three
primitive consumers and three primitive unary operators. Default composition
validates its argument eagerly, returns a fresh callback, invokes stages in
the specified order and propagates each stage's exception. Local callback
guards end before the next stage is entered. Native carriers also retain
optional virtual composition dispatch for named and anonymous source objects;
overrides own validation and can return null or an existing callback. Their
formal/result storage must agree with the interface boundary. Named source
overrides continue through ordinary resolved source calls and typed callback
lowering; anonymous overrides reuse the existing object-backed helper bodies.

Physical carrier requirements use root-only edges. Composition operands share
the selected local/synchronized ABI; a synchronized callback propagates that
demand into its captured native state. The reverse implication is invalid:
a local callback can retain synchronized state. Neither relation joins generic
children or asserts Java object identity. Composition result-shape inference
and argument lowering use the receiver's actual carrier instead of a nominal
local facade. The general facade ABI planner now types literal null from the
selected Java formal, and native SAM receiver staging evaluates arguments
before the required-receiver check.

The expanded `FixedCompositionAudit` compares Java and generated execution for
all six primitive families, stage order, exceptions and suppression of the
second stage after failure, repeated/same callback use, retained arrays,
escaping compositions, receiver/argument evaluation order, fresh default
identity, anonymous static overrides and named stateful overrides accepting
null. The expanded probe passes in
`.tmp/statistics-v434-primitive-composition-r9.log`; r1–r8 are diagnostic
iterations. The frozen producer is
`.tmp/new-java2rust-statistics-v434-primitive-composition-debug`, SHA256
`0d98aa0b90598d170eb46f1de35365f1049292bb266fc4f00c1119312b6b8420`;
`.tmp/statistics-v434-producer-inputs.json` records its exact inputs.

The complete Ranking core translates with zero parse/compile/unsupported/
invalid-pattern failures at `.tmp/commons-statistics-1.3-ranking-v434-core`.
Fresh independent callers match **207 exact rows** at
`.tmp/statistics-v434-ranking-core-behavior`. This does not close inherited
generic composition or the final reachable-support audit.

The broad run at `.tmp/statistics-v434-library-adjacent-contracts-r10.log`
finished: **1743 library tests and 59 integration tests passed**. Its primitive
probe was rejected by the fixture provenance guard because its compiled
`include_str!` predated the later fixture edit; this was not a generated-code
failure. Cargo then skipped the final stream-bound-reference binary. Preserve
that distinction; the whole command was not green.

The final absent-NPE-message refinement and strengthened fixture pass fresh
independent Java/Rust controls at
`.tmp/statistics-v434-final-composition-behavior/provenance.json`. Both
`FixedCompositionAudit` and `FunctionalLambdaDomainsAudit` return zero,
with matching rows. The two-file translation has zero parse/compile/
unsupported/invalid failures. Its frozen release producer is
`.tmp/new-java2rust-statistics-v434-final-composition-release`, SHA256
`9c69d107ce5ef3fd9637e8d0dbf13fb6baabeaa3114a08607f5c7a66f7a706b7`;
`.tmp/statistics-v434-final-producer-inputs.json` records unchanged build inputs.

The complete 80-file Descriptive refresh at
`.tmp/commons-statistics-1.3-descriptive-v434-composition` is terminal with
**five parse failures and 170 compiler observations**; diagnostic totals are
not independent failure counts while parsing fails. Its debug run took
1594.855 seconds. Source manifests remain complete and unchanged. The known
inherited source/native interface, owner/name and result-view families remain.

Generic Function/BiFunction composition, scoped method-binder inference,
inherited source SAM discovery and general recursive/reentrant local callback
dispatch remain required. Sequential guard release does not prove arbitrary
callback reentry. No Statistics module is accepted by this batch alone.

### Array element and nested-carrier follow-up

The explicit 80-file-context, one-file-emission audit at
`.tmp/statistics-v434-statistics-composition-slice-r2` finds another contract
boundary in `Statistics.composeIntConsumers`/`composeLongConsumers`: the
factory result and static no-op use a synchronized callback, while callbacks
read from the parameter array retain local facade types. This is diagnostic
emission only, with assembly/cargo deliberately skipped. It also exposes the
existing `composeDoubleConsumers` parameter refinement to a StatisticResult
carrier and an erased DoubleConsumer field/return; those whole-path failures
are not explained away by the isolated composition probe.

`PrimitiveCompositionArrayAudit.java` isolates the array boundary without the
unresolved StatisticResult inheritance. The frozen V434 baseline has one
local/synchronized type mismatch. The smallest correct rewrite is a common
`Option<__JavaSyncIntConsumer>` carrier for the array child, loop binding and
composition argument. Keep the array allocation and retained element references
distinct: changing an array entry after factory creation must not replace the
callback captured earlier. Empty/all-null lists, singleton identity, expanded
varargs and an explicit mutable array, repeated state, stage errors and null
array validation are exercised.

The fix extends existing enhanced-for native reference transfers to SAM
elements. A second required fix reuses
`synchronized_functional_slot_rust_type` when overlaying nested child slots;
the previous overlay only constructed collection allocation shells and left
callback elements local despite solved synchronized constraints. The focused
Java/generated-Rust test passes at
`.tmp/statistics-v435-composition-array-contracts-r2.log`. Both rendering paths
have a regression for nested callback carriers. Broader current-source checks
are recorded separately from the older V434 results.

The composition argument-plan helper also now rejects non-composition names
and arities before recursively inferring receiver shapes. It is queried at
shared lowering boundaries, so doing that inference for ordinary calls repeats
fluent-prefix work. The same explicit full-context emission slice will measure
this change. Run library tests with normal host concurrency; bound generated
crate integration concurrency separately. The V434 library run's two-thread
cap cost 577.35 seconds and should not be reused as the default procedure.

### V435 terminal verification

The facts-only concrete renderer also selected the legacy opaque-interface
fallback for native generic SAMs. It now delegates to the native type renderer,
retaining the Java arguments. The nested callback test therefore covers both
array elements and `Function<String,IntConsumer>` results in both rendering
paths. An existing assembly assertion now requires `Comparator<String>` as an
owned value rather than bare Comparator's erased default Object carrier.

The full library run at `.tmp/statistics-v435-library-r4.log` has 1743 passing
tests and that single obsolete expectation failure. Only its cfg(test)
expectation changed afterward. Fresh renderer **38/38** and assembly **48/48**
controls pass in the r5 logs. Do not describe the earlier whole command as
green. The final adjacent integration run at
`.tmp/statistics-v435-final-adjacent-contracts-r5.log` is green: **63/63**,
including all 15 Statistics controls, Numbers Arrays/Fraction controls,
native/source callbacks, retained state/streams and bound-reference contracts.
The test-source manifest records 1092 Java/Rust hashes, unchanged after the run.

The final release producer is
`.tmp/new-java2rust-statistics-v435-final-callback-domains-release`, SHA256
`c3f46b78cf38a8177c179572b6dfa57d6ee831446c144cbc88586ad0e0d86fc2`.
The final debug producer is
`.tmp/new-java2rust-statistics-v435-final-callback-domains-debug`, SHA256
`662c1367c4e7eb94f7bcba8d2d6f9ce4969e204bd7d0b7f7035f75c4fb90cce3`.
Their input manifests record the same final source; all hashes were rechecked.
`.tmp/statistics-v435-final-batch-evidence.json` collects terminal evidence and
the limits of the library result.

The complete Ranking core passes again at
`.tmp/commons-statistics-1.3-ranking-v435-final-core`. Fresh independent callers
match **207 exact observations** at
`.tmp/statistics-v435-ranking-final-core-behavior/input-provenance.json`.
This remains core behavior evidence, with generic inherited composition and
the reachable-support audit still open; no Statistics module is accepted.

The same explicit 80-file-context diagnostic Statistics.java slice now emits
matching synchronized array/loop/composition carriers for IntConsumer and
LongConsumer. It is still diagnostic emission rather than a compiled module.
Its observed lower phase was 61.420 seconds versus 37.070 seconds before this
batch; concurrent build/test load and changed storage contracts prevent
attributing that difference to the name guard. **No speedup is established.**
Keep optimized producers for larger closures and avoid repeating the full
26-minute debug regeneration per small change. The unresolved DoubleConsumer
erasure/parameter narrowing and inherited StatisticResult contracts remain
separate whole-path work.

## V435 entry audit: generic composition value contracts

`GenericCompositionDomainsAudit.java` adds contextual inline callbacks,
parameter rebinding without changing an intermediate producer's nullability,
nested escaping suppliers, and static/local callback composition. Its Java
oracle returns zero; source hash and observation are recorded at
`.tmp/statistics-v435-generic-domains-java-oracle/observations.json`.
The two-file audit baseline at `.tmp/statistics-v435-generic-domains-baseline`
uses the frozen V434 producer, has zero parse/unsupported/invalid failures and
nine missing composition methods. These diagnostics are not nine independent
bugs and do not establish that adding runtime methods will suffice.

Representative generated rewrites and their complete paths:

| Case | Required contract and rewrite scope |
|---|---|
| `parse.andThen(value -> { value[0]++; return value; })` | Instantiate the method binder from the expected `Function<String,int[]>`; type the lambda formal as the receiver result and feed its result into the composed output. Producer, formal, mutation and returned array must use canonical reference storage, preserving one allocation. |
| `parse.andThen(value -> { int[] saved=value; value=null; return String.valueOf(saved[0]); })` | Preserve a non-null producer result while admitting a nullable mutable lambda binding. Pass `Some(array_handle)` through an ordinary typed invocation adapter into the nullable formal; do not widen the producer simply because its consumer rebinds a local. The retained alias remains the original array. |
| `format.compose(value -> new int[]{Integer.parseInt(value)})` | Infer the new input binder from the expected composed input. The argument's array result feeds the receiver's array input even though neither composed generic argument is an array. Connect existing invocation endpoints, not coincident result-generic positions. |
| `parse.andThen(value -> () -> value)` | The composed result is a native Supplier retaining the intermediate array. Thread the result's nested storage contract into the inner lambda, retain its canonical allocation, and keep parse/invocation exceptions at their respective stages. An untyped bare Rust closure is insufficient. |
| `SHARED.andThen(value -> value)` | Carry the static receiver's synchronized root ABI into the composed callback and the retained operand. Project array child contracts independently; the local destination cannot invent a new local facade or copy a synchronized array payload. |

The minimal Rust boundary in the rebinding case has a producer result
`Rc<RefCell<Vec<i32>>>` and a consumer formal
`Option<Rc<RefCell<Vec<i32>>>>`. A typed adapter invokes the consumer with
`Some(result)`; its local assignment to `None` affects neither the producer's
contract nor `saved`. For synchronized paths the same proof uses Arc/Mutex.
This is a cross-function contract extension using existing local bindings,
slot plans, invocation adapters and root capture edges. No new object
representation is justified by these cases. The resolver's declared-binder
substitution machinery should own contextual inference; unresolved binders
must not silently become erased objects.

Generic default-method overrides remain a separate uncertainty: an override of
`Function.compose<V>` is polymorphic in V, so the fixed-signature primitive
override table cannot simply be reused as a monomorphic callback. The pinned
Statistics production scan contains only the three primitive `andThen`
overrides already covered by V434. Do not infer general generic override
support from that fact, or silently replace an encountered override with a
default factory. The next implementation must preserve an explicit diagnostic
boundary until that dispatch contract is representable.

## V436: scoped generic-composition typing

The generic-composition entry audit remains the scope for this batch. The first
implementation step adds a shared `NativeGenericCompositionContract` for
Function's `compose`/`andThen` and BiFunction's `andThen`, including their
UnaryOperator/BinaryOperator parent views. Receiver arguments and the fresh
method argument remain separate structural inputs; no invented Java binder
name can capture an actual receiver type named `V`. Argument-sensitive result
inference and contextual callback typing consume this same contract.

Target-result typing now supplies the hidden intermediate array type to an
inline callback, and recursively types an expression-bodied callback's returned
lambda. Explicit method type witnesses also supply callback and result types
without an assignment target. Source-declared functional interfaces keep their
own method contracts. Native parent projection now accepts fully qualified
`java.util.function` names while rejecting foreign qualified lookalikes.
Unrelated calls are rejected by method name/arity before extra receiver
inference; no measured performance improvement is claimed.

The original source-defined Function control exposed another real discrepancy:
resolver callback discovery counted concrete default methods as abstract, while
lowering excluded their bodies. Both now use
`source_callbacks::declared_single_abstract_method`; resolver selects the actual
abstract declaration rather than the first nonstatic method. The unchanged
control puts the default first and still requires the lambda parameter to be
String. A separate Java/generated-Rust fixture exercises an escaping source
callback, its abstract operation repeatedly and its concrete default helper.
This shared declaration query does not solve the inherited/native effective
method inventory needed by StatisticResult, nor claim that it does.

This is front-end contract work, not generic composition acceptance. It does
not yet add runtime methods or connect the intermediate producer, nullable
callback formal, captured allocation and result storage domains. In particular
the nine-method baseline must not be reported as resolved by these changes.
Generic virtual overrides and synchronized/local capture boundaries remain
explicit architectural work from the V435 audit. No Statistics module is
accepted, and the full Descriptive baseline is not rerun for this typing step.

The four focused contract/resolver controls pass in
`.tmp/statistics-v436-generic-composition-resolver-r4.log`. R2 had a test-only
ambiguous `.into()` call; R3 exposed the default-method discovery bug above.
Both are corrected while retaining the original source-precedence assertion.
Current source/test input manifests are
`.tmp/statistics-v436-source-inputs.json` and
`.tmp/statistics-v436-test-inputs.json` (115 and 1,093 inputs respectively),
rechecked against the terminal tested tree.

Terminal verification:

- Full library: **1,748/1,748**, no ignored tests,
  `.tmp/statistics-v436-library-r1.log`.
- Adjacent integration: **34/34** across Statistics (16), native SAMs (5),
  source callback contracts (7), retained callback state (2), deferred source
  views (2) and bound stream references (2),
  `.tmp/statistics-v436-adjacent-contracts-r1.log`.
- `cargo fmt --check` and `git diff --check` pass.

`.tmp/statistics-v436-typing-batch-evidence.json` records final log hashes,
source/test manifest hashes and the seven test executable hashes. All jobs are
terminal. This step builds no new frozen CLI producer and runs no complete
Statistics or Numbers/RNG corpus acceptance campaign; V435's frozen producer
and corpus observations remain historical evidence for that earlier snapshot.
Next connect the typed composition's independent invocation value endpoints to
storage and emission, retaining the nullable-local adapter and generic override
requirements from the entry audit.

## V437: generic-composition invocation value domains

The shared generic composition contract now describes value transfers by SAM
invocation endpoints, separately from type instantiation. Function `andThen`
connects composed input to receiver input, receiver result to argument input,
and argument result to composed result. BiFunction preserves both input
positions; `compose` reverses the first two stages. Usage projects those
endpoints using the existing native SAM descriptors, so operator parent views
do not depend on coincident generic argument indices.

Named native callback operands retain their independent references through
callback-environment edges. They are not aliases of the fresh composed result.
These edges carry synchronization requirements from a result to its captures;
a local result can still retain a synchronized operand. Inline callbacks use
their canonical parameter bindings and the existing scoped callback-result
domain. No synthetic call-argument/result graph node is introduced without
constraint storage. The inline callback's result domain can be the receiver's
input (compose) or the composed result (andThen), including a nested Supplier
result whose body retains the intermediate array.

Lambda capture recording is shared with ordinary stored lambdas. Invocation
value transfers now also preserve native functional references themselves,
alongside the existing array/container allocation transfers. Rebinding an
inline callback parameter to null remains a local presence constraint:
producer-to-consumer nullability is directional, while array allocation
identity is shared. This does not reuse the reverse presence edge needed by a
stored callback's single formal/interface ABI.

The focused control exercises named compose/andThen, BiFunction, an
intermediate Supplier, nullable local rebinding and an escaping nested
Supplier. In addition to usage edges, it solves signatures and requires the
producer result to remain non-null while the rebound callback local becomes
nullable. All five generic contract/resolver/storage controls pass in
`.tmp/statistics-v437-composition-storage-r2.log`.

This step still does not emit generic default composition factories. It covers
named native operands and contextually typed inline Function arguments with
existing storage roots. Direct source-object arguments, method references,
un-targeted/chained factory expressions, applied/wildcard endpoint projections,
and generic virtual overrides need their existing source/native invocation
contracts connected before claiming general composition support. They are not
scope exclusions. No module acceptance or current whole-corpus error reduction
is claimed from these semantic tests.

The next emission path should stage receiver then argument exactly once before
receiver null checking, then validate the argument before returning a fresh
callback. Use ordinary typed SAM calls to bridge a non-null intermediate
producer to a nullable callback formal, and lower nested callback results
through their solved slots. `callback_lambda_param_shape` currently uses an
explicit supplied interface shape before its declared Java parameter type;
the ephemeral inline argument facade must therefore use the canonical local
plan's independent formal shape, rather than forcing it to the producer's ABI.
The source-object adapter must diagnose unsupported polymorphic composition
overrides instead of silently installing a default implementation. These are
whole-path requirements from the entry audit, not reasons to add nominal Rust
methods first and repair resulting mismatches individually.

The frozen current debug producer is
`.tmp/new-java2rust-statistics-v437-composition-storage-debug`, SHA-256
`48e3eebbd16573df1ea44cee5d699e0b8fb98d8f4b3693599bf8ec2e22efe4aa`.
Its 115 input hashes and build provenance are in
`.tmp/statistics-v437-source-inputs.json`. The fresh two-file diagnostic
translation is `.tmp/statistics-v437-generic-domains-current`, reproduced with:

```sh
NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-statistics-v437-composition-storage-debug" \
NEW_JAVA2RUST_BUILD_PROFILE=debug CARGO_NET_OFFLINE=true \
TMPDIR="$PWD/.tmp/fraction-arrays-test-tmp" \
bash scripts/translate-focused-package.sh \
  --primary tests/corpus/statistics/generic-composition-audit \
  --out .tmp/statistics-v437-generic-domains-current-reproduce \
  --skip-resolver-bench --jobs 2 --offline-cargo
```

This diagnostic command terminates with exit 1: zero parse/unsupported failures,
one invalid-pattern file, and ten compiler observations. Nine are the expected
missing composition methods; the tenth is an explicit refusal to convert a
value array into an unrelated retained allocation. The fresh code shows why:
`parse` now produces the canonical Rc/RefCell array, and `saved` correctly
requires that retained reference, but the bare callback parameter is still
emitted as `Vec<i32>`. The typed inline-facade path must read its canonical
local plan. The escaping Supplier is likewise still a bare nested closure, and
compose's array allocation/parse exception still follows the untargeted closure
path. These are connected emission contracts, not independent fixes to the
generated code. Named parse/format adapters now agree on their canonical
intermediate Arc/Mutex array without changing either public result argument.
No reduction in corpus errors or isolated speed improvement is claimed.

Terminal verification for this semantic batch:

- Full library: **1,749/1,749**, no ignored tests,
  `.tmp/statistics-v437-library-r1.log`.
- Adjacent integration: **34/34**, including all 16 Statistics regression
  contracts, `.tmp/statistics-v437-adjacent-contracts-r1.log`.
- `cargo fmt --check` and `git diff --check` pass.

All jobs are terminal. The current 1,093 test inputs are recorded in
`.tmp/statistics-v437-test-inputs.json`; log, executable, producer and manifest
hashes are in `.tmp/statistics-v437-storage-batch-evidence.json`. Complete
Statistics/Numbers/RNG module acceptance was not rerun for this semantic step.

## V438: typed generic composition factories

Function/BiFunction default composition now lowers to an evaluated factory
whose fresh native callback invokes the retained operands through ordinary
typed SAM calls. UnaryOperator/BinaryOperator reuse their shared parent
contracts. No nominal runtime methods with erased intermediate types are added.
The existing argument/result conversion plans bridge each physical invocation
boundary, including `Some(array_handle)` for a nullable consumer formal and a
native Supplier facade retaining the intermediate array.

Factory lowering stages receiver and argument once, in Java order, before
checking the receiver and then the argument for null. The callback body executes
only at invocation. Native generic composition effect facts record unchecked
factory exceptions and argument escape without inventing a nongeneric return
type or facade ABI. Opaque facade methods are suppressed for these operations.
Both null casts and named nullable native operands retain their destination
carriers during staging. Default factories create fresh Java identities while
each operand retains its existing identity and mutable state.

An inline argument facade takes its input from the canonical lambda local plan
and its output from the receiving invocation endpoint. Explicit supplied SAM
parameter shapes still own their ABI; an otherwise untargeted lambda now falls
back to its canonical binding plan before using a context-free Java spelling.
Native composition expression shapes project the appropriate operand inputs
and result independently, including nested native allocations and collection
views. This also allows typed unused factory results (notably eager validation
inside try blocks) to use the same lowering path as assignments and returns.

The original two Java/Rust audit probes pass unchanged. Additional production-
style controls cover argument side effects before a null receiver check,
argument exceptions taking precedence, BiFunction input positions, inherited
operator views and repeated use of the same callback in both stages, preserving
the original mutated array. All four focused controls pass in
`.tmp/statistics-v438-generic-composition-r3.log` (three positive Java/Rust
programs and one explicit unsupported-dispatch control). R1 passed the nullable
local/nested-Supplier probe and exposed nullable operand recognition and unused
factory lowering in the named-source probe; R2 passed both original probes.

Generic source overrides require genuinely polymorphic default-method
dispatch. The source-to-native adapter now detects a selected source
compose/andThen implementation and emits
`native generic functional composition override requires polymorphic dispatch`
instead of silently replacing it with the default factory. The pinned source
scan previously found no such generic overrides; this is an explicit remaining
framework boundary, not a scope exclusion or a completed Java API claim.
Direct source-object arguments still need the source invocation endpoints
connected in storage analysis; un-targeted lambda chains and wildcard/applied
endpoint contracts also remain open as described in V437. The whole Commons
Statistics goal and its complete dependency/public-operation requirements are
unchanged.

The first current Ranking-boundary rerun exposed a second physical contract
distinction: the stored function accepted a nullable array input while the
composed callback's input was non-null. Lowering must not equate those physical
prefixes to instantiate the fresh method argument. The contract now exposes
that argument separately; ordinary typed SAM invocation supplies the nullable
receiver formal. Resolver target inference retains its logical applicability
checks. The focused order probe adds this production-motivated nullable-input
case; all four controls pass in
`.tmp/statistics-v438-generic-composition-r4.log`.

Final frozen debug producer:
`.tmp/new-java2rust-statistics-v438-final-generic-composition-debug`, SHA-256
`2ea37c76d34325c2dcdff11b9b04dc3d47c13cf0c3e6b68377263c743571b73f`.
Final 115 source/runtime/assembly inputs and 1,094 test inputs are recorded in
`.tmp/statistics-v438-final-source-inputs.json` and
`.tmp/statistics-v438-final-test-inputs.json`, rechecked after terminal tests.
The earlier producer `9480fbe6…` and its manifests remain evidence for the
pre-nullable-input-correction snapshot, not the final tree.

Final verification:

- Full library: **1,749/1,749**, no ignored tests,
  `.tmp/statistics-v438-library-r2.log`.
- All Statistics integration regressions: **20/20**,
  `.tmp/statistics-v438-statistics-final-r5.log`.
- The broader **68/68** adjacent integration run, including Fraction/Arrays,
  retained streams and native/source callbacks, passed before the final scoped
  nullable-input correction: `.tmp/statistics-v438-adjacent-contracts-r1.log`.
  That run has its own earlier source/test manifests. It is not relabelled as a
  final-revision broad run; the final correction is exercised by the current
  focused/Statistics checks and real Ranking-boundary rerun.
- Complete Ranking core (five production files):
  `.tmp/commons-statistics-1.3-ranking-v438-final-generic-core`.
- Complete Ranking core plus unchanged expanded client (six files):
  `.tmp/commons-statistics-1.3-ranking-v438-final-generic-boundary`.
- Complete three-file composition audit:
  `.tmp/statistics-v438-final-generic-domains-current`.

All three final translation/assembly checks have zero parse errors, compiler
errors, unsupported IR, invalid-pattern files and no-op facade call sites.
The independent Ranking core comparison at
`.tmp/statistics-v438-ranking-final-core-behavior` retains **207 exact rows**.
Separate Java and Rust callers at
`.tmp/statistics-v438-final-extra-behavior/ranking-boundary` execute the expanded
translated client and both return zero. The composition-batch directory beside
it executes all three unchanged positive Java programs and their jointly
translated Rust counterparts; all three return zero. These aggregate contract
results are separate from the detailed core rows and do not constitute a full
API coverage census. Sources, generated crates, caller inputs, commands,
compiler outputs and terminal provenance are retained in those directories.

The core comparison is reproducible with the maintained verifier and fresh
output roots:

```sh
python3 scripts/verify-commons-statistics-ranking.py \
  --translation .tmp/commons-statistics-1.3-ranking-v438-final-generic-core \
  --out .tmp/statistics-v438-ranking-final-core-behavior-reproduce
```

For the expanded source translation, set `STATISTICS_MODULE=ranking`,
`STATISTICS_EXTRA_SOURCE="$PWD/tests/corpus/statistics/ranking-acceptance/RankingBoundary.java"`,
the frozen producer/profile variables above and a fresh
`STATISTICS_TRANSLATION_ROOT`, then run
`scripts/run-commons-statistics-1.3.sh --skip-resolver-bench`. Extra behavior
callers can be rerun with `cargo run --offline --manifest-path` against their
retained probe manifests; their independent Java commands are recorded in
`input-provenance.json`.

`.tmp/statistics-v438-final-batch-evidence.json` binds the final and earlier
scoped evidence to their respective manifests and frozen producers. All jobs
are terminal. `cargo fmt --check` and `git diff --check` pass. Ranking remains
unaccepted pending the remaining relevant public/inherited operation coverage,
retained/stateful/reentrant callback checks and reachable-support audit. The
complete other modules and final Numbers/RNG preservation requirements remain
open.

## V439: local native callback reentry

`RankingReentry.java` adds two concrete acceptance probes: recursive invocation
of the same stored IntUnaryOperator, and a supplied random callback reentering
the same NaturalRanking instance on tied data. There are no concurrent calls.
Both Java probes return zero with the full pinned Ranking closure:
`.tmp/statistics-v439-reentry-java-oracle/provenance.json`.
The unchanged V438 final producer translates all six files and compiles them
without unsupported/parse/compiler/invalid-pattern failures at
`.tmp/commons-statistics-1.3-ranking-v439-reentry-baseline`. Both Rust probes
then panic at the same native IntUnaryOperator dispatch borrow, recorded in
`.tmp/statistics-v439-reentry-baseline-behavior/provenance.json`. The generated
body holds `self.callback.borrow_mut()` while user code invokes that same
callback recursively. This is a real Ranking callback path, not a speculative
floating-point or threading requirement.

The smallest correct rewrite moves state mutability out of exclusive closure
dispatch: Java captures retain object/array references, and invoke a shared
callable. Existing source outer-companion methods and field/array storage plans
remain responsible for the actual state access. This is a runtime invocation
and capture-storage contract extension. It does not require copying payloads,
changing pinned algorithms, suppressing recursive calls or converting Rust
borrow panics into invented Java exceptions.

The common `JavaLocalCallback<A,R>` runtime now supports shared `Fn` invocation
and the existing mutable `FnMut` convenience contract separately. Its manual
Clone implementation copies only the existing reference, with no argument or
result Clone requirement. Callback identity uses the existing
JavaReferenceIdentity protocol rather than the address of a movable enum slot.
There are no unnecessary `'static` bounds on A/R: only closure captures require
that lifetime, preserving comparator operations over borrowed inputs. The
synchronized carrier keeps its existing Arc/Fn/Send/Sync contract.

Generated native lambdas, method references, anonymous/source adapters and
primitive composition overrides select shared constructors. Rust callers keep
their mutable constructors. Primitive default composition uses shared dispatch
as well, so a recursive composed callback does not acquire a new exclusive
guard around otherwise reentrant operands. Source object adapters retain their
original Java identity token. Captured native arrays/containers now demand
canonical retained storage even when no second lexical alias survives; their
mutation remains in the same allocation, rather than moving a mutable Vec into
an exclusive closure.

The standalone `FunctionalReentryAudit` includes repeated recursion, an
exception followed by successful reuse, and reentry through a primitive
andThen result. It also exposed a preexisting argument boundary gap: inline
primitive-composition callbacks were emitted as bare invoking-only Rust
closures, although the runtime retains an optional native facade. The selected
composition argument plan now uses the ordinary owned assignment-slot adapter
and Java reference-copy plan, retaining final field-read reconciliation and
the existing operand evaluation/null-validation order.

Focused native/Statistics/bound-reference checks pass **28/28**, including the
new recursive probe, in
`.tmp/statistics-v439-shared-callback-contracts-r3.log`. The full library passes
**1,749/1,749** in `.tmp/statistics-v439-library-r1.log`. R1 found unnecessary
generic lifetime bounds in the new helper; R2 passed the five native adapter
checks and exposed the inline primitive argument boundary above; both are
corrected without narrowing the callback API or weakening the recursive test.

The first broader run exposed a source-interface default body whose deferred
Supplier called a mutable method through its captured boxed view. Replacing
FnMut dispatch alone made that captured view immutable to the Rust closure.
The required rewrite is small and follows ordinary typed receiver staging:
capture the existing `clone_box` view once, then copy that reference into a
private mutable boxed view for each callback invocation. The inspected
DeferredView adapter's clone_box clones the same Rc/RefCell source handle;
`values_with_outer` mutates that source's calls/current fields. It does not
clone the source payload or snapshot its list. The existing preservation test
continues to require deferred construction, live reassignment, repeated state
increments and delayed null exceptions. Native boxed-interface arguments
already use the same reference-copy protocol when ordinary typed calls stage
their receivers; this fix handles lexical interface `this` replacement.

Final frozen producer:
`.tmp/new-java2rust-statistics-v439-final-shared-callbacks-debug`, SHA-256
`2f19a31077f3753fc3e0f0923d964a698b01da3614a2a569c14586494d71ecef`.
The 115 source/runtime/assembly inputs and 1,096 test inputs are in
`.tmp/statistics-v439-final-source-inputs.json` and
`.tmp/statistics-v439-final-test-inputs.json`. The earlier `db6b15c7…` producer
and its manifests describe the snapshot before the boxed-view correction.

Terminal checks:

- Final full library: **1,749/1,749**, no ignored tests,
  `.tmp/statistics-v439-library-r2.log`.
- Final deferred-view/Fraction/retained-callback/stream/source-callback groups:
  **36/36**, `.tmp/statistics-v439-preservation-contracts-r2.log`.
- The native/Statistics/bound-reference **28/28** and Arrays **5/5** checks
  passed before the final boxed-interface-this correction. They retain that
  earlier source manifest; they are not relabelled as a final-revision 69-test
  run. The final correction is exercised by the final full library and the
  deferred-view group above.
- Complete final Ranking core (five files), core + boundary client (six), and
  core + reentry client (six) all compile with zero parse/unsupported/invalid
  failures, stubbed constants and no-op facade call sites:
  `.tmp/commons-statistics-1.3-ranking-v439-final-{core,boundary,reentry}`.

The earlier shared-callback producer's behavior at
`.tmp/statistics-v439-core-behavior` matches **207 exact Java/Rust rows**.
`.tmp/statistics-v439-extra-behavior/reentry` records both formerly failing
recursive programs now returning zero on Java and Rust; the boundary directory
beside it records the expanded null/inherited-operation client returning zero.
Source/caller/generated-code hashes, commands, outputs and terminal provenance
are retained. These are distinct checks, not a combined public API census.

For valid final-revision reuse,
`.tmp/statistics-v439-final-generated-equivalence.json` compares **all** generated
Rust files plus Cargo.toml/Cargo.lock for each complete crate before/after the
boxed-view correction: eight inputs for core and nine each for boundary and
reentry. Every input is byte-identical. The final producer regenerates each
complete closure and passes its assembly/compile gates, so the recorded behavior
applies to those identical final crate inputs without another expensive replay.
This equivalence covers these three crates only, not arbitrary corpora.

`.tmp/statistics-v439-final-batch-evidence.json` ties each scoped result to its
producer and source/test manifests. All jobs are terminal; cargo fmt and diff
checks pass. These changes concern native SAM carriers; they do not establish
reentrant dispatch for every source-defined interface adapter or finish the
whole module acceptance audit. Ranking's remaining relevant enum/public API
coverage and reachable-support checks, the other complete Statistics modules
and final Numbers/RNG corpus preservation remain open.

## V440: Ranking enum acceptance audit (in progress)

The independent `RankingEnums.java` client exercises both complete five-value
enums: names/ordinals/toString, all 25 pairwise comparisons per enum,
valueOf success and invalid UTF-16/null inputs, equality/hash/class metadata,
fresh values-array identity and mutation, and null receiver/argument evaluation
order. The pinned Java closure plus client passes all six groups (six zero
rows); commands, source hashes and outputs are recorded in
`.tmp/statistics-v440-enum-java-oracle/provenance.json`.

The complete six-source V439-producer baseline is terminal at
`.tmp/commons-statistics-1.3-ranking-v440-enum-baseline`: zero parse failures,
15 valid Rust compiler errors. This is not an accepted module/client. Audit
of its generated caller identifies the following whole-path work:

- **Enum comparison emission:** source and external-facade emitters returned
  comparison sign instead of ordinal difference. The previous regression
  compiled adjacent comparisons with a canceling sum and missed this behavior.
  Both emitters now share the ordinal-difference method, verified below.
- **Fresh array producer contract:** enum-values slot paths map to an owned
  `Vec<Option<E>>` and return before attaching the destination's canonical
  Rc/RefCell allocation. `first`, its alias and `second` already have matching
  shared storage/consumer plans. Reuse `fresh_java_array_value_for_slot`, the
  existing creation/clone/copy boundary, rather than copying aliased payloads.
- **Checked invocation contract:** nullable enum compare arguments remain
  `&Option<E>`/`None` while the emitted method requires `Borrow<E>`. Null enum
  receiver checks also precede argument evaluation, and name() uses a Rust
  expect instead of a catchable Java exception. Stage receiver and arguments
  once, then perform receiver/argument checks through the existing ordered-call
  machinery. Nonthrowing comparisons must retain their existing simple ABI.
- **String comparison storage:** name()-to-array-element equals compares an
  owned JavaString directly with `&Option<JavaString>`. Normalize both operands
  through the authoritative String value/nullable contracts.
- **Enum object/class contract:** equals/getClass/getDeclaringClass are absent,
  and JavaHashCode is not implemented. Reuse identity/class metadata rather
  than manufacture corpus-specific methods; class literals must retain source
  canonical names. Constant-specific subclasses require a distinct runtime
  class from the declaring enum where applicable. Do not equate ordinal with
  cross-class object identity or require JVM-process-identical identity hashes.

These are local emissions plus shared invocation/storage contract corrections;
the current audit does not establish a need for a new enum representation.
The complete enum client remains the acceptance boundary, including the
null-ordering cases that compile-error counts alone cannot detect. Ranking's
reachable-support audit and every other production module remain open.

The strengthened comparison regression passed at
`.tmp/statistics-v440-enum-compare-regression.log`: generated Rust runs the
nonadjacent weighted comparison and returns -180 (the previous sign-only body
would return -90). `EnumFreshArrayAudit.java` independently checks fresh array
identity, aliases/null mutation, isolation, receiverless values() in an enum
method, its returned allocation and nonadjacent comparisons against Java.

The initial array-adapter revision was incomplete: the focused suite recorded
21 passes and one failure (`.tmp/statistics-v440-contracts.log`). Inspecting
the generated failing local revealed a third path in
`lower_unresolved_static_owner_value_call_for_slot` that returned before the
two source enum paths. All three now share the same fresh-array finisher.
The intermediate complete Ranking core compiled, but the expanded enum client
still had 15 errors; those V440 intermediate artifacts do not prove the final
array correction.

Final V440 revision evidence:

- Frozen debug producer
  `.tmp/new-java2rust-statistics-v440-r2-enum-arrays-debug`, SHA256
  `6490da64e5cf122d329b79c40f340c2787552f687e3c208c1a5f31ed51ca5adf`.
  `.tmp/statistics-v440-r2-source-inputs.json` records 115 source/runtime/build
  inputs; `.tmp/statistics-v440-test-inputs.json` records the test snapshot.
- **22/22** Statistics contracts, including the independent fresh-array Java
  oracle and generated-Rust caller: `.tmp/statistics-v440-contracts-r2.log`.
- **5/5** Arrays preservation contracts, including initialized-enum fresh array
  arguments: `.tmp/statistics-v440-arrays-preservation.log`.
- Runtime comparison regression **1/1**:
  `.tmp/statistics-v440-enum-compare-regression-r2.log`.
- Complete five-file Ranking core regeneration passes parse/compile gates with
  zero unsupported/invalid/stubbed-constant/no-op-call-site observations:
  `.tmp/commons-statistics-1.3-ranking-v440-r2-core`.
- Fresh independent core behavior replay passes **207 exact Java/Rust rows**:
  `.tmp/statistics-v440-r2-core-behavior/input-provenance.json`. This replay
  invokes generated algorithms and does not reuse earlier generated-code hashes.
- Complete core + enum client regenerates with zero parse failures and **13**
  compiler errors, down from 15. Both fresh-array errors are eliminated:
  `.tmp/commons-statistics-1.3-ranking-v440-r2-enum-batch`.

The remaining enum invocation/String comparison/object/class families described
above are not accepted. The current full-library suite and whole Numbers/RNG
corpus preservation have not been rerun for this small emission/storage batch;
the preceding V439 full-library evidence remains scoped to that earlier
revision. Ranking's other API and reachable-support checks, Descriptive and all
other complete production modules remain required. The goal remains active.

## V441: inherited enum contracts and borrowed String inputs

The next audit traced the two String comparison errors to their producer,
rather than adding an equality special case. `collect_method_symbols` recorded
source-declared methods and compiler-provided valueOf, but omitted inherited
final Enum.name()/ordinal(). The emitter already implemented these operations;
their unresolved result types could reach equality planning despite name()
physically producing a JavaString. Source enums now publish those two typed
instance signatures, so existing nullable String equality projection handles
the destination array elements. This is resolver metadata, not a new String
representation or a corpus-specific name rule.

The existing enum name/toString lowering used a raw Rust expect for nullable
receivers. It now passes that existing projection through
`make_java_null_expect_catchable_in_propagating_ctx`, the shared Java exception
adapter, allowing a Java catch to observe NullPointerException.

`EnumNameStringAudit.java` supplies an independent Java/generated-Rust contract
for literal/fresh/null String array elements, required enum-array receivers,
null local and null array enum receivers, ordinal results, and a source-defined
name(int) overload. The initial version passed at
`.tmp/statistics-v441-enum-name-r1.log`; the expanded version and preservation
checks are recorded below. Checked compareTo invocation and enum identity/class
metadata remain separate open paths; none is silently dropped from acceptance.

The intermediate complete enum client confirmed that both String equality
type mismatches disappeared. The compiler count stayed at 13 because the
previous failures masked two downstream borrowed-String-construction errors:
`new String(names[i])` emitted a consuming `.ok_or_else` on the checked array
read's `&Option<JavaString>`. The smallest correct rewrite is to borrow the
payload (`array.get(i)?.as_ref().ok_or_else(...)`) before passing it to the
fresh String factory. The source array allocation and slot stay intact; the
constructor still creates a new String identity. The common String constructor
lowerer now uses that borrowed nullable view, independent of enum calls.
The regression additionally checks distinct constructed identity, unchanged
source contents, and NullPointerException for a nullable array element.

The intermediate producer is
`.tmp/new-java2rust-statistics-v441-enum-names-debug`, SHA256
`2d46eb1de9d76e24dc4e2719b4684801b5ff3ee7385967767e8bc1a470b0d6a8`.
Its full library (1,749/1,749), Statistics contracts (23/23), complete core
compilation and 207 exact core behavior rows passed. Those results precede
the constructor-borrow correction; they must not be relabelled as final-revision
full-library results. The intermediate enum-client evidence is at
`.tmp/commons-statistics-1.3-ranking-v441-enums`; final source/test snapshots
are `.tmp/statistics-v441-r2-{source,test}-inputs.json`.

Final frozen producer:
`.tmp/new-java2rust-statistics-v441-r2-enum-string-borrows-debug`, SHA256
`0a13a1d5fb03e63052bf8105e128db750a2d79826dfac868299bb8fc10c61a15`.
Final-revision terminal evidence:

- Statistics contracts **23/23**, including the expanded independent Java/Rust
  enum/String contract: `.tmp/statistics-v441-contracts-r2.log`.
- String library preservation **108/108**:
  `.tmp/statistics-v441-string-preservation-r2.log`.
- The selected legacy constructor command records **one pass and two failed
  assertions**: `.tmp/statistics-v441-constructor-preservation-r2.log`. The
  failing assertions demand the old native Rust String emission. The same
  necessary assertions mismatch frozen V440 output; the complete constructed
  return fixture output is byte-identical before/after this batch.
  `.tmp/statistics-v441-legacy-snapshot-audit/provenance.json` records exact
  producer/source/output hashes and the necessary assertions. This establishes
  the pre-existing snapshot mismatch, not a green legacy test run. Tests are
  unchanged; their modernization remains open.
- Complete five-file core regenerates and compiles with zero parse failures,
  unsupported/invalid observations, stubbed constants and no-op call sites:
  `.tmp/commons-statistics-1.3-ranking-v441-r2-core`.
- Complete six-file expanded enum client has zero parse failures and **11**
  compiler errors, down from 13:
  `.tmp/commons-statistics-1.3-ranking-v441-r2-enums`. Three errors concern
  nullable compare arguments; eight concern equals/hash/class metadata.
- Fresh independent core behavior comparison passes **207 exact Java/Rust
  rows**: `.tmp/statistics-v441-r2-core-behavior/input-provenance.json`.

Formatting/diff checks pass. The complete enum client, its runtime null-order
checks, reachable-support acceptance, all other production modules and broad
Numbers/RNG preservation remain open; no Statistics module is accepted.

## V442: checked enum comparison invocation

The three remaining compareTo compiler errors exposed one cross-function
contract: nullable array reads/arguments reached a Borrow<E> method without
discharging Java null, while nullable receiver checks ran before argument
evaluation. A minimal correct rewrite snapshots `(receiver, argument)` first,
then checks the receiver, checks the argument and invokes the existing ordinal
comparison. In particular, `array[0].compareTo(replace(array))` must compare the
old receiver value even if argument evaluation changes array[0]; no array guard
may survive across that argument call. This requires storage/effect/call
planning together, not merely an emitted-method parameter change.

- The existing source enum symbol table now includes inherited typed
  compareTo(E), alongside name()/ordinal().
- `source_enum_comparison_operand_type` uses the authoritative enum
  representation/declaration identity across nested/imported spellings and
  rejects unrelated receiver types/argument overloads. Both effect planning
  and lowering consume this one query.
- Shaped call effect planning recognizes nullable/literal-null enum comparison
  arguments, allowing an uncaught failure to propagate through callers.
  Proven non-null comparisons do not acquire an unnecessary Result ABI.
- Lowering snapshots immutable enum identities through ordinary owned slots,
  then reuses `staged_instance_call_projection` and the existing required-value
  Java null check. It clones/copies reference carriers, not mutable payloads.

`EnumCompareAudit.java` independently checks all pairwise values, receiver/arg
evaluation once and in order, an argument exception taking precedence over a
null receiver, null argument propagation through a source method, receiver-array
mutation during argument evaluation, bound comparison method references and
their null failures, initialized enum constants, and an ordinary source class
whose same-named method accepts null. The Rust caller additionally requires
the plain comparison result to type-check as i32, proving its infallible ABI.
The first focused version passed at `.tmp/statistics-v442-comparison-r1.log`;
the expanded version passes with all **24/24** Statistics contracts at
`.tmp/statistics-v442-contracts.log`.

The original comparison regression retains its -180 runtime assertion and
emitted ordinal-difference checks. Its constant/call assertion now recognizes
the staged invocation; it passes at
`.tmp/statistics-v442-comparison-preservation.log`.

Final frozen producer
`.tmp/new-java2rust-statistics-v442-enum-comparison-debug`, SHA256
`10dfb6c11bbc2e771b5cf47198c248892517bb6a4405d8be43974230d5b669c1`,
is tied to `.tmp/statistics-v442-{source,test}-inputs.json`. Final checks:

- Statistics contracts **24/24** and comparison preservation **1/1**, above.
- Enum/shaped-call/null-receiver library selection **21/21**:
  `.tmp/statistics-v442-library-contracts.log`. This is not a final-revision
  rerun of the entire 1,749-test library.
- Complete five-file core passes parse/compile and unsupported/invalid,
  stubbed-constant and no-op-call-site gates:
  `.tmp/commons-statistics-1.3-ranking-v442-core`.
- Fresh independent core comparison passes **207 exact Java/Rust rows**:
  `.tmp/statistics-v442-core-behavior/input-provenance.json`.
- The full six-file enum-client closure has zero parse failures and **eight**
  compiler errors, down from 11:
  `.tmp/commons-statistics-1.3-ranking-v442-enums`. All three comparison
  argument errors are gone. Generated null-order code now stages the argument
  before receiver unwrapping; checked array reads snapshot enum identities.

The eight remaining errors are three equals calls, two hash trait failures,
two getDeclaringClass calls and one getClass call. The next batch must connect
the enum's existing type/discriminant identity to the authoritative erased
Object/equality/hash protocol and source binary class metadata. In particular,
equals(Object) must handle an erased argument containing the same enum; merely
returning false for non-enum static types would be wrong. Inspect
`java_erased_any_equals_result` and its generated source registrations before
introducing another identity representation. Class literals currently receive
the correct emitted enum token type but still render a short Java spelling;
the source-name contract must be fixed through the common class-literal
boundary rather than patched with Ranking names.

Formatting and diff checks pass. Enum object/class metadata, the complete
client's runtime acceptance and reachable-support audit, all other production
modules and broad Numbers/RNG preservation remain open. No Statistics module
is accepted; this goal remains active.

## V443: enum Object identity, erasure and class metadata

The remaining eight enum-client errors came from missing Object protocols and
runtime class tokens. The rewrite audit followed concrete constants through
typed values, copied array elements, Object erasure, checked casts and consumers:

- The source enum is a copied discriminant, but erased reference/clone
  registrations assumed Rc/Arc payloads. Register the actual enum carrier.
- Emit stable addressable per-constant identity tokens, using the existing
  JavaReferenceIdentity protocol. Atomic-byte static tokens cannot merge or
  move. Existing default Object hashing projects that identity; JVM-process
  identity hash numbers are not cross-process oracle values.
- Connect enum equality to JavaObjectEquals and the existing erased dispatch
  registry. Downcast to the same enum type before comparing discriminants;
  a heterogeneous Object argument may contain that exact constant. Never
  manufacture false solely from the argument's static type.
- Snapshot enum receivers through ordinary owned slots before argument
  evaluation, reusing the existing erased equality call pipeline.
- Add inherited hash/getClass/getDeclaringClass method signatures. Existing
  hierarchy-aware method counts and overload naming remain authoritative.
- Class-token methods use original source binary names; scalar class literals
  now resolve those names at their shared contextual boundary before calling
  the common token emitter. Rust type tokens remain separate from Java names.
- Checked Object-to-enum casts reuse the existing borrowed checked-cast/clone
  path used for canonical handles; they must not consume a reused Object box.

`EnumObjectAudit.java` independently covers equality in both Object/enum views,
heterogeneous/null arguments, erased alias identity, checked casts preserving
their source, stable typed hashes across values-array copies, packaged/nested
class names, class-literal equality, argument-before-null-receiver order, simple
constant-specific subclass metadata, erased class metadata and nullable/default
toString calls. The initial generated-code audit found a missing direct
hash_code entry point for owned values-array results and a consuming downcast;
both use the common identity/cast contracts now.

The intermediate revision passed 25 Statistics tests and **1,749/1,749** library
tests and regenerated both complete Ranking crates. It nevertheless failed the
independent six-group enum runtime replay immediately: an imported nullable
enum toString call erased Option<E> itself and panicked in display dispatch.
`.tmp/statistics-v443-enum-behavior/input-provenance.json` records that terminal
failure. Compilation was insufficient acceptance evidence. The final correction
routes default source enum Object views through the existing nullable enum
receiver adapter before cross-unit erased fallback. Source-declared toString
dispatch keeps its existing precedence.

Final frozen producer:
`.tmp/new-java2rust-statistics-v443-r3-enum-object-views-debug`, SHA256
`d7aaded7f0d4a2b7636aa995f8d40359db5b0bf3f7e40dd451cd913abf80634a`.
`.tmp/statistics-v443-r3-{source,test}-inputs.json` records final inputs.

- Final Statistics contracts: **25/25**,
  `.tmp/statistics-v443-contracts-r3.log`.
- Final enum/toString/class-literal library selection: **35/35**,
  `.tmp/statistics-v443-library-contracts-r3.log`. The earlier full-library run
  at `.tmp/statistics-v443-library.log` precedes the final toString correction
  and is not relabelled as a final-revision full-suite result.
- Complete five-file core and six-file core + enum client pass parse/compile,
  unsupported/invalid, stubbed-constant and no-op-call-site gates:
  `.tmp/commons-statistics-1.3-ranking-v443-r3-{core,enums}`.
- Fresh core comparison: **207 exact Java/Rust rows**,
  `.tmp/statistics-v443-r3-core-behavior/input-provenance.json`.
- Fresh enum-client comparison: **six exact zero rows** for metadata,
  comparisons, lookup, identity, arrays and null ordering,
  `.tmp/statistics-v443-r3-enum-behavior/input-provenance.json`. Independent
  Java/Rust callers, all six Java inputs and generated crate inputs are hashed.

Two concrete follow-up boundaries prevent a general enum/Object acceptance claim:

1. `EnumErasedHashAudit.java` calls the same enum hash through Object. Its Java
   oracle returns zero. The fresh one-source diagnostic at
   `.tmp/statistics-v443-erased-hash-entry/translation` fails with one valid
   compiler error: Box<dyn Any> does not implement JavaHashCode. Add checked
   dynamic hash dispatch alongside existing erased Object registries, preserving
   source hashCode overrides and exceptions; do not substitute pointer hashing
   for every erased object. This fixture is a recorded failing baseline, not a
   skipped or passing regression test.
2. Simple constant-specific subclasses pass, but counting only enum bodies is
   insufficient when constructor arguments also contain anonymous classes.
   `.tmp/statistics-v443-anonymous-name-audit/provenance.json` records a JVM
   example producing E$2 and E$4, while inspection of the current emitter's
   body-only counter predicts E$1 and E$2. This is a known metadata limitation;
   no Rust parity result is claimed for that diagnostic. Move anonymous runtime
   identity allocation to an earlier shared metadata pass, following the
   existing original-source-name design, and consume it in enum/erased class
   emission. Do not add per-corpus suffix corrections.

Formatting/diff checks pass. Ranking's complete public-view/reachable-support
audit is still open, as are the other complete production modules and broad
Numbers/RNG preservation. No Statistics module is accepted; the full goal remains
active.

## V444: checked erased hashing and canonical Object construction

The V443 erased-hash baseline failed on Box<dyn Any>: JavaHashCode. The smallest
correct rewrite is a checked runtime type dispatch to the same typed hash
protocol emitted for each canonical payload. A blanket address hash would
discard source overrides, mutations and exceptions. This is a runtime/assembly
contract extension; it does not change the enum or source-object representation.

- `java_erased_hash_runtime_support_item` emits checked erased dispatch and
  JavaHashCode forwarding for dyn Any / Box<T>. Source registrations use the
  canonical storage renderer and existing typed-hash capability proof. Concrete
  generic instances use the existing shared source-instance inventory.
- Hierarchy forwarding and dynamic registration now share
  `source_hierarchy_hash_protocol_is_supported`; registered carriers cannot
  assume a hash implementation merely because they have reference identity.
- Primitive/String leaves delegate to existing JavaHashCode implementations.
  Retained plain Object allocations use their stable reference identity.
  Unregistered payloads retain an explicit unsupported boundary; other mapped
  JDK payloads still need registered hash contracts when exercised.
- The `object_hash` registry uses the existing additive shared-support merger.
  Its order-independence/idempotence controls now include that registry kind.
- The expanded source-override probe initially found two nullable receiver ABI
  errors: the checked receiver could be an owned erased box where the helper
  requires a borrow. The required-value projection is now borrowed at that
  boundary, preserving the Java null check and the collection null-element hash
  policy separately.
- Inspection also found new Object() being erased and subsequently downcast
  again. Its constructor now publishes a retained Rc token inside the erased
  box, and the slot path retains that producer ABI. Clone/identity registries
  know the canonical plain Object carrier. Parsed source classes named Object
  remain under source ownership. Plain Object aliases preserve identity/hash.

`EnumErasedHashAudit.java` now has a positive Java/Rust regression, including a
null typed-enum receiver. `ErasedHashDispatchAudit.java` additionally checks a
mutable source hash override, repeated calls/state observation, exceptions and
continued use afterward, a concrete generic class, inherited/overridden hash
dispatch through a factory return, String and boxed integer values, plain Object
aliases and a null erased receiver. The initial two-error output is preserved
at `.tmp/statistics-v444-hash-dispatch-r1.log`; both focused contracts pass at
`.tmp/statistics-v444-hash-contracts-r2.log` after the shared boundary fixes.

Final frozen producer:
`.tmp/new-java2rust-statistics-v444-erased-hash-debug`, SHA256
`339d4da387259711d1d1b6a57eb9c4abed4f8aac2a0f326a8b4fc7dbc74f5a2f`.
Final source/test/control manifests and terminal evidence are tied together in
`.tmp/statistics-v444-final-batch-evidence.json`.

- Statistics contracts **27/27**, `.tmp/statistics-v444-contracts.log`.
- Full library **1,749/1,749**, `.tmp/statistics-v444-library.log`.
- Shared registry controls **9/9**, `.tmp/statistics-v444-registry-tests.log`.
- Complete core (five files), core + enum client (six), and core + imported-enum
  erased-hash client (six) pass parse/compile and unsupported/invalid,
  stubbed-constant/no-op-call-site gates:
  `.tmp/commons-statistics-1.3-ranking-v444-{core,enums,erased-hash}`.
- Fresh core behavior: **207 exact Java/Rust rows**,
  `.tmp/statistics-v444-core-behavior/input-provenance.json`.
- Fresh extra-client behavior: **six exact enum groups** and **one exact erased
  hash contract**, `.tmp/statistics-v444-extra-behavior/{enums,erased-hash}`.
  `RankingErasedHashAudit.java` exercises the actual imported NaNStrategy through
  Object, including an erased alias and null receiver. It verifies cross-module
  assembly, beyond the standalone enum regression.

Identity hash numbers are process-specific; these probes compare stable
typed/erased/alias properties within each Java/Rust process, not JVM-process
pointer numbers against Rust addresses. Typed value hashes and override results
remain exact. No generated algorithm is patched and pinned sources are unchanged.

The V443 anonymous-class naming counterexample remains open. Its allocation must
move to the shared original-runtime-name metadata stage, accounting for creation
sites in constructor arguments rather than adding local suffix exceptions.
Ranking's final public-view/reachable-support audit, the other complete
Statistics modules and broad Numbers/RNG preservation remain required. No
Statistics module is accepted; the full goal remains active.

## V445: Descriptive checked-array widening and evaluation order

The next implementation slice prioritizes Descriptive's production generation
boundary over the still-open anonymous-runtime-name metadata work. V434's
Quantile output has four Rust parse failures at conditional branches containing
an implicitly widened checked array read. Its fifth parse failure is in
DoubleStatistics; that older expression-bound method-reference output predates
the structured-receiver fixes and needs a fresh measurement.

The minimal Quantile rewrite is `({ capture; checked_read? }) as f64` inside the
conditional branch. This is a local emission fix, not a new numeric algorithm:
reuse the emitter's existing block-left-operand grammar rule for casts as well
as binary operators, including shape/comment wrappers and raw block fragments.
The independent rustc grammar experiment is recorded at
`.tmp/statistics-v445-grammar-audit/provenance.json` (bare block fails; grouped
block compiles). The emitter's 30 tests pass in
`.tmp/statistics-v445-emitter-tests.log` after this change.

`ConditionalArrayWideningAudit.java` isolates int/long/float array reads in
conditional numeric promotion, the unselected null branch, checked bounds,
index side effects and null/throw ordering. Its original baseline fails with
three `expected expression, found as` errors:
`.tmp/statistics-v445-widening-baseline.log`. After the emitter fix, all 27
pre-existing Statistics contracts pass, but the new contract returns 9:
`.tmp/statistics-v445-contracts.log`. This is a real second failure family:
`values.as_ref().ok_or_else(...)?` runs before the index expression, whereas
Java evaluates the index before checking the selected array for null.

The smallest correct whole-path rewrite captures the nullable array reference,
evaluates the index (including its side effects and exceptions), then checks
null and bounds. Extend the existing retained-array read/projection path to
nullable plain arrays: its existing source producer and optional slot adapters
capture a borrowed optional slice, or retain an owned temporary. Shared arrays
continue retaining their canonical allocation handle. Consumers use the same
checked-index and result-propagation machinery after index evaluation. Do not
clone array payloads or hold an allocation guard during an index callback.
The fixture also checks that an index exception takes precedence over the
later null check. This is a general evaluation-contract fix; pinned sources
remain unchanged. The focused final contract passes at
`.tmp/statistics-v445-widening-r2.log`. Broad final validation at
`.tmp/statistics-v445-library-and-contracts.log` passes **1,749 library tests
and 28 Statistics contracts**. The frozen debug producer is
`.tmp/new-java2rust-statistics-v445-array-read-debug`, SHA256
`0a03bda669157e31fc6439dff0ebde16d898b121c164fc9c203cc853514322d4`.
Its 115 source inputs and 1,131 test inputs are recorded in
`.tmp/statistics-v445-{source,test}-inputs.json`. The
three-file/full-80-file-context Descriptive diagnostic refresh is still pending;
no module is accepted. The live run is
`.tmp/statistics-v445-descriptive-slice-r2` (tool session 95042); completed
checks, manifests and the pending run are recorded separately in
`.tmp/statistics-v445-batch-evidence.json`. The first slice invocation exited
2 before translation because `--emit-list` requires an explicit `--focus-list`;
the live second invocation supplies all 80 source paths. Do not count that
configuration rejection as a production generation failure or restart the
second run while its session is live.

### Next architectural batch: effective source/native interface contracts

The fresh small diagnostic at
`.tmp/statistics-v445-inherited-native-audit/InheritedNativeResultAudit.java`
uses the same inheritance pattern as StatisticResult: a source interface extends
DoubleSupplier and IntSupplier, supplies a default `getAsInt`, and inherits
`getAsDouble` as its sole abstract method. A concrete implementation and a
forwarded bound method reference exercise both producers. Its independent JVM
run returns zero (`java-provenance.json`); current generated Rust reports eleven
compiler errors (`rust-provenance.json`, `compile.log`). These are observations,
not eleven independent root causes.

The generated source trait omits `get_as_double`; three default-body variants
instead call an unresolved bare `get_as_double()`. Native fallback discovery
then emits conflicting generic supplier facades, and the source method-reference
producer cannot use a correctly typed effective SAM. Patching one emitted trait
method would leave resolver selection, default-body calls, implementor forwarders
and callback construction inconsistent.

The required conceptual contract is one abstract `get_as_double(&self) -> f64`
and a default `get_as_int` that invokes that virtual receiver method before the
numeric conversion. Concrete implementors, boxed/shared forwarders and the
callback adapter must implement the same selected signature. Native parent
descriptors must contribute specialized contracts; nearer source defaults and
overrides must suppress parent abstract entries with the same Java signature.
This needs a shared effective-method inventory used by resolver, interface
emission and callback discovery. Reuse the existing source ancestry/generic
specialization and native functional descriptors. The current specialized
inventory borrows source MethodDecls and skips native ancestors, while
`declared_single_abstract_method` explicitly answers only a declaration query;
neither alone can represent this contract today. This is a cross-function
contract batch with a shared inventory change, not an isolated trait-printing
fix. Additional source/native bridge and result-view failures remain subject
to a full producer/storage/consumer audit when this inventory is available.

The bounded diagnostic rewrite is compiled and executed at
`diagnostic-provenance.json` in that audit directory: **45 added / 36 removed
Rust lines**, preserving the original generated output separately. It supplies
the inherited virtual slot to the trait and all five concrete/shared/boxed
forwarders, routes defaults through `self` with Result propagation, removes
the two erroneous fallback supplier methods, and returns a source adapter
holding the existing DoubleSupplier facade. The caller passes a facade built
from the existing shared callback factory and invokes both result operations.
Both rustc and the diagnostic contract exit zero. This is feasibility evidence
for this representative case only, never a corpus deliverable or acceptance.

The experiment exposes an additional whole-path constraint: `forwarded` is
currently inferred as taking `&mut dyn FnMut() -> f64`, even though its returned
source method-reference adapter must retain that supplier after the factory
returns. Correct inventory/SAM discovery must inform callback escape planning
so the producer, parameter and adapter storage agree on the owned, cloneable
native facade. Default and forwarding signatures must carry the same exception
contract. The existing callback carrier can express this rewrite; neither a
new borrow framework nor a replacement callback algorithm is necessary for
this case.

## V446: normalized source/native abstract interface surfaces

The implementation reuses the existing platform-Number normalization approach:
materialize inherited abstract contracts on source interface surfaces before
method naming, binding, semantic analysis and signature planning. These are
abstract declarations, never substitute bodies. `Modifier::InheritedContract`
records derived provenance. The pass is shared by source-symbol construction,
primary resolution and context declaration naming. It first completes direct
native SAM parents from the authoritative native functional descriptors, then
uses the existing source ancestry/generic-specialization walk to complete source
descendants. Both walks collect changes against immutable input before applying
them, so declaration order does not affect inheritance. A repeat is idempotent.

Effective abstract-method selection now merges overriding defaults before
filtering concrete bodies and excludes private/static helpers. Specialized
contracts also substitute method bounds and throws types. This preserves the
declared/default distinction while giving resolver, trait emission, implementor
forwarding and SAM discovery the same normalized abstract surface.

Focused foreign-lookalike checks exposed two shared name-identity issues:
non-generic hierarchy parsing discarded package names, and representation lookup
accepted any qualified name ending with a source type's name. Hierarchy parsing
now retains the owner just as the generic path already did. Source lookup reuses
original canonical/binary source metadata (or the actual compilation-unit
package), rather than accepting arbitrary suffixes. Explicit foreign/native
owners therefore remain distinct from local same-leaf source interfaces.

Callback escape analysis now resolves ambiguous single-name method-reference
targets through the existing lexical receiver classifier. This fixes
`supplierValue::getAsDouble` retaining a callback whose normalized formal is
`supplier_value`; it no longer gets an invocation-only borrowed ABI. Native
method-reference adaptation also admits the native SAM descriptor directly,
allowing the existing typed callback/call-graph machinery to connect checked
invocation effects without making creation execute the callback. Native inherited
support generation rejects invented arities that contradict that descriptor;
the conservative method-reference arity probe must not synthesize a one-argument
`DoubleSupplier.getAsDouble` overload on source implementors.

`InheritedNativeResultAudit.java` exercises native parents, a further source
subinterface, concrete/default dispatch, a returned method-reference adapter,
repeated retained state, a throwing callback caught through the default method,
and recovery after that exception. The independent JVM/generated-Rust contract
passes in `.tmp/statistics-v446-native-result-r4.log`. A focused graph test proves
native supplier invocation is fallible while the adapter factory and unused
callback creation remain infallible:
`.tmp/statistics-v446-native-supplier-effect-r4.log` (one selected test; the
integration binary in that command has zero matching names and is not counted).
The earlier R3 integration failure had eight compiler observations caused by
invented native overloads; it is retained at
`.tmp/statistics-v446-native-result-r3.log`.

The intermediate R3 full library passes **1,754 tests** at
`.tmp/statistics-v446-library-r3.log`. That run predates the last descriptor/
effect fixes; final full-library/Statistics validation and an optimized producer
build have now finished. Final R4 validation passes **1,755 library tests and
29 Statistics contracts** at
`.tmp/statistics-v446-library-and-contracts-r4.log`. The optimized build completes
in 170 seconds (`.tmp/statistics-v446-release-build.log`). Frozen producer:
`.tmp/new-java2rust-statistics-v446-native-contracts-release`, SHA256
`46eedea5d0f2d06aa1e9f662348400c946aa30b8fa3cb6ccae95be835176d8c7`.
Source/test manifests are
`.tmp/statistics-v446-{source,test}-inputs.json`. No Statistics module is accepted.

The frozen V445 diagnostic refresh is terminal, exit zero in profiling mode:
80 analyzed files, three emitted files, 1,523.471 seconds, assembly/compiler
deliberately skipped. Quantile and StatisticResult parse with rustfmt's read-only
stdout mode. DoubleStatistics still has the expression-bound cast/reference
parse failure, together with explicit unsupported source-interface cast and
moment-class upcast markers; parsing the successful units is not compilation
or module acceptance. Evidence:
`.tmp/statistics-v445-descriptive-slice-r2/completed-files-parse-evidence.json`.
Quantile's four historical cast parse errors are gone. This entire diagnostic
run uses V445, not V446. Its expensive debug analysis/lowering motivates using
the optimized producer for the next complete-module refresh. V445 terminal
status is recorded at `.tmp/statistics-v445-batch-evidence.json`; the old source
manifest intentionally no longer matches the V446 worktree.

The V446 complete-module refreshes are terminal:

- `.tmp/commons-statistics-1.3-ranking-v446-core`: complete five-file closure,
  exit zero, zero parse/compiler/unsupported/invalid/stubbed-constant/no-op-site
  failures. Fresh independent behavior passes **207 exact rows** at
  `.tmp/statistics-v446-core-behavior/input-provenance.json`.
- `.tmp/commons-statistics-1.3-descriptive-v446-native-contracts`: complete
  80-file closure, eight emission workers, exit one. **Zero parse failures**,
  five translation error-status units, seven invalid-pattern units and **177
  compiler observations**. There are zero unsupported member/statement/expression
  counters, zero stubbed constants and zero reachable no-op call-site counters;
  explicit unsupported conversion markers remain, so these zero counters do
  not establish acceptance. Unlike the V434 parse-invalid run, this compiler
  pass is marked type-error-valid. Counts 170 and 177 are not directly comparable
  independent-bug totals. Timing: translation 76.339 seconds, assembly 54.722
  seconds, compiler 3.579 seconds, total **136.114 seconds**.

Use an optimized producer for complete-module refreshes and keep debug builds
for focused development checks. The release build cost is amortized over the
module campaign. These timings include different code and worker settings;
they establish practical workflow cost, not an isolated algorithm speedup.
Ranking's public-view/reachable-support acceptance audit and broader Numbers/RNG
preservation are still open. No Statistics module is accepted.
Terminal checks, frozen producer identity, current input manifests, complete
staged-closure verification and behavior provenance are tied together at
`.tmp/statistics-v446-final-batch-evidence.json`. All jobs from this batch are
terminal; there are no live handles to resume.

### Next rewrite audit: source callback type and receiver contracts

The fresh compiler output identifies these concrete related batches. Hypothetical
Rust rewrites below describe general translator work; generated code remains
untouched.

| Representative failure | Required rewrite through the whole path | Scope and remaining uncertainty |
|---|---|---|
| 37 `RangeFunction::apply` mismatches: `DoubleStatistics_Builder.min` stores `RangeFunction<Vec<f64>, Arc<Mutex<Min>>>`, but its callback implementation accepts `Rc<RefCell<Vec<f64>>>`. | Instantiate the source generic argument with the solved Java array reference carrier, then use that same instantiation for fields, returned adapters, method parameters and callers. Existing callback bodies already require an allocation handle. Never bridge these types by copying a Vec. Include the static `NO_VALUES` array, which currently uses an Arc/Mutex carrier, in the connected-domain audit. | Cross-function storage/signature contract; generalize the existing native callback/generic child-slot mechanisms to source SAMs. The framework can express handle-valued type arguments. Determine the actual connected sharing/sync domain before choosing its carrier; do not force every primitive array into one global policy. |
| 12 `get_as_double` and 12 `get_as_long` mismatches: Interpolation's anonymous `LongStatisticResult` emits `&self`, while its inherited StatisticResult surface requires `&mut self`. | Select anonymous trait-implementation receiver signatures from the same effective contract/receiver plan used by the emitted trait, including inherited defaults and Rc forwarders. Read-only implementation bodies can still fulfill that receiver signature. | Shared emission-contract fix, not a new receiver ABI policy. Trace both object-backed and closure adapters rather than changing one printed receiver. |
| 29 bare `self` uses inside `add_with_outer(__java_self, ...)`, e.g. the retained `self.sum_of_logs` bound-reference receiver. | Snapshot the field through the current canonical self handle, retain its callback reference, perform the Java method-reference null check at creation, and release the owner guard before invocation. Reuse the existing once-only bound-reference capture machinery. | Local producer/capture emission plus its storage contract; test ordinary and outer-handle methods, subsequent field rebinding, null creation and callback reentry. |

These batches should share authoritative slot/signature/receiver adapters; a
per-callback type table or a payload-copy conversion would preserve the wrong
Java aliasing semantics. Other explicit families remain: duplicate-leaf Sum
name mapping (18 observations), incorrect moment-class package/upcast identity,
native consumer views, boxed source-interface casts/result views and sync
constraints. In particular the class-upcast diagnostics use the Numbers Arrays
package for Statistics moment classes; confirm original declaration identity
before treating every such marker as an independent hierarchy redesign.

## V447: anonymous receiver signatures and canonical self-field reads

The two focused entry fixtures reproduce the real corpus failures:
`AnonymousReceiverContractAudit.java` has four incompatible receiver signatures;
`FieldReferenceCaptureAudit.java` has two bare-self failures in outer-handle
companions. The JVM checks pass before translation; the failing generated crates
are retained at `.tmp/statistics-v447-source-callback-baseline.log`.

Object-backed anonymous adapters now select each receiver from the same
`trait_method_receiver_plan` used by the emitted interface. The selected body
signature is reused for the adapter and its direct Rc implementation. Native
anonymous facade implementations retain their existing receiver contract.

Explicit this-field reads now project the canonical current self handle through
the existing shared-storage receiver projection. Bound-reference capture can
construct such source expressions after the outer-handle body rewrite; resolving
the receiver before adapter text generation avoids embedding unrewritable
`self.field` text in a generated static companion. The intermediate R2 complete
80-file Descriptive run is terminal at
`.tmp/commons-statistics-1.3-descriptive-v447-callback-receivers`: zero parse
failures, **143 compiler observations** (177 previously), all 24 anonymous getter
receiver errors eliminated, ten bare-self errors eliminated. The remaining 19
bare-self observations identify unqualified field reads inside returned lambdas
and constructor/cast expressions, rather than the explicit bound-reference path.

The final source change normalizes proven unshadowed current-object field reads
to explicit receiver accesses at the common expression and slot boundaries.
Captured enclosing-object fields, parameters and local shadows keep their own
resolution. The strengthened field fixture distinguishes method references
(snapshot the selected function at creation) from lambdas (read the current field
at invocation), verifies subsequent rebinding and clearing, and checks eager
versus delayed null failure. Both focused contracts pass at
`.tmp/statistics-v447-source-callback-r3.log`. The R3 library run found one
spelling-dependent assertion in the static-object storage test. Paired frozen
R2/current R3 output (`.tmp/statistics-v447-static-copy-audit/routing.diff`) shows
the initializer still calls the same `copy_with_outer` companion; only that
companion's internal fallback `(*__java_self).copy()` became a directly lowered
constructor/body read. The assertion now checks the initializer's actual named
companion invocation, rather than the incidental fallback call.

Final R4 validation passes **1,755 library tests and 31 Statistics contracts**:
`.tmp/statistics-v447-library-and-contracts-r4.log`. Frozen producer:
`.tmp/new-java2rust-statistics-v447-callback-fields-release`, SHA256
`bdd66bba245464f1383e26e8ebc0123fddc77348580243dd26664dde55f88faf`.
The final complete 80-file Descriptive refresh is terminal, exit one, at
`.tmp/commons-statistics-1.3-descriptive-v447-final-callback-fields`: **zero parse
failures, 124 compiler observations**, five translation error-status files and
seven invalid-pattern files, total 130.251 seconds. All 24 anonymous receiver
signature observations and all 29 bare-self observations are gone; the remaining
families were not hidden by a parse failure. The complete final Ranking core
compiles successfully at `.tmp/commons-statistics-1.3-ranking-v447-final-core`.
Its independent behavior comparison passes **207 exact rows**, recorded at
`.tmp/statistics-v447-final-core-behavior/input-provenance.json`.
No module is accepted.

### Source generic array feasibility audit: solver requirements are already present

`.tmp/statistics-v447-source-array-audit/SourceArrayCallbackAudit.java` uses a
true source `Transform<T,R>`, a named array-retaining implementor and a returned
bound-reference adapter. The JVM returns zero. Unmodified generated Rust has
eight observations: three explicit allocation-conversion markers, three apply
signature mismatches, one caller argument mismatch and one result comparison
mismatch (`provenance.json`, `command-2.log`). Its nominal trait arguments are
`Vec<i32>` while implementations, captured storage and caller locals already
use `Rc<RefCell<Vec<i32>>>`.

The diagnostic-only rewrite changes the trait instantiations throughout producer/
storage/forwarders/clones/consumers, preserves the returned reference carrier,
removes the two resulting unnecessary conversion attempts and restores the
discarded apply expression at the caller. **23 added / 21 removed Rust lines**
compile and execute the alias/mutation contract successfully
(`diagnostic-provenance.json`). Original output is preserved separately. This is
feasibility evidence, never translated output delivered by the tool.

The public-API shape probe at `shape_probe.rs`/`shape-probe.log` establishes a
more precise root cause than missing fixpoint propagation: the solver already
gives both abstract `T` input and `R` result shared/interior-mutable native
allocation requirements. Its enclosing `Transform<int[],int[]>` slots have
empty `nested_native_allocations`, so the source generic instantiation loses
those requirements. The `sync/` variant adds a static array, motivated by
Descriptive Builder's static `NO_VALUES` array: both abstract slots correctly
become synchronized, **but the concrete Keep implementation's parameter, retained
field and result remain local Rc carriers**. Thus the sync case also needs the
existing physical requirement propagated across the specialized interface/
implementation relation and then through the implementation's array alias flows.
Receiver/borrow ownership may differ across an implementation bridge; physical
allocation identity cannot be converted by copying an Rc into an Arc. Reuse and
extend the existing native-allocation constraints rather than inventing another
placement solver or a global array policy.

The first sync diagnostic, which only changed trait arguments/results, fails;
its files are preserved with `diagnostic-naive-` prefixes. The complete sync
rewrite also lifts the concrete array parameter/field/result and their guard/
clone operations, restores the bound delegate call discarded by the incompatible
conversion, and compares returned array references through the existing identity
helper. **37 added / 35 removed lines** compile and pass, alongside the zero JVM
result (`sync/diagnostic-provenance.json`, `sync/provenance.json`). The object
owner remains local Rc; only the connected array carrier is lifted. This is
bounded evidence that the existing Rust representations can express the rewrite,
not proof that the translator now emits it.

The `identity/` variant also returns a fresh, equal-content array through the
generic callback. Its JVM result is zero. Repairing just the carrier contracts
lets generated Rust compile, but its test returns three: the generic call's array
comparison uses Rust payload equality instead of Java reference identity. The
preserved `diagnostic-naive-*` artifacts capture that failure. Routing the two
staged result comparisons through the existing `java_reference_equals` helper
produces a passing diagnostic rewrite, **33 added / 31 removed lines**
(`identity/diagnostic-provenance.json`). The next batch must carry physical result
shapes through comparison lowering as well as signatures, preserving evaluation
order and distinguishing retained aliases from fresh copies. These are diagnostic
experiments; the general translator repair remains pending.

Two concrete downstream loss points are visible in the implementation:
`specialized_interface_method_return_shape` rebuilds generic result storage and
discards solved physical allocation metadata; `project_nested_native_allocations`
intentionally stops at source carriers, while parameterized dyn-trait rendering
receives only Java arguments. The next architectural batch should project solved,
binder-scoped generic requirements into source instantiation child shapes, then
reuse one shape-aware generic renderer/return projection for fields, factories,
named/shared/closure adapters, calls and typed runtime registries. Preserve
canonical source dispatch and optionality alongside the physical requirements.
Use original declaration/MethodKey identities, respect method type-parameter
shadowing and source ancestor substitutions, preserve nullability separately
from physical type-argument requirements, and cover local and static/sync arrays.
Re-render argument types from Java/slot structure before formatting the
trait path; do not parse/replace emitted Rust strings or add RangeFunction rules.

Final batch evidence is `.tmp/statistics-v447-final-batch-evidence.json`. It ties
the frozen producer to the current source/test input manifests, terminal checks,
complete selected source closures (including original/staged hash equality),
Ranking behavior and the three diagnostic array rewrites. Full module acceptance
and broader Numbers/RNG preservation checks remain outstanding.

## V448: source generic array allocation contracts

V447's local/static/fresh-copy rewrite audit is now implemented through general
translator mechanisms. `SourceOverrideNativePaths` distinguishes concrete native
paths from declared type-variable roots. Existing override joins can carry a
physical allocation requirement through a specialized binder and its descendants;
concrete non-native implementation slots continue to reject that qualifier.
The focused solver check proves that local versus synchronized requirements reach
the concrete parameter, result and retained field, rather than stopping on the
abstract `T`/`R` declaration.

`SourceGenericAllocationPlan` projects solved declaration requirements onto
owner/index binders. It excludes shadowing method/constructor type parameters,
uses canonical source owners with unambiguous aliases, and reuses the existing
source ancestry substitution on direct parent edges. It is a derived rendering
contract, not a second placement solver. Source instantiations annotate the
existing `nested_native_allocations` vocabulary on solved fields, locals,
parameters and results. Optionality remains independent. The rendering context
shares this solved contract across a complete batch.

The canonical trait-path renderer and nominal/closed carrier argument renderers
consume those child shapes before formatting Rust paths. Named implementation
trait paths now use that same renderer. Generic result storage and resolved-call
representation projection preserve physical allocation metadata while selecting
owned or concrete views. No printed Rust paths are parsed or rewritten, and no
RangeFunction-specific rule was added.

Maintained fixtures `SourceArrayIdentityAudit.java` and `SourceArraySyncAudit.java`
cover a source two-argument interface, a retaining named implementation, an
escaping bound reference, repeated invocation, input/result/field aliases,
mutation after return, fresh equal-content copies, and a static array passed
through the same callable. The identity fixture also uses a generic interface
ancestor. The R2 focused run reduced each fixture to two implementation signature
errors, revealing a separate legacy trait-argument builder. R3, after using the
canonical renderer there, passes both JVM/generated-Rust contracts at
`.tmp/statistics-v448-source-arrays-r3.log`. R4 strengthens inheritance/binder
coverage and passes **1,759 library tests and all 33 Statistics contracts**
(`.tmp/statistics-v448-library-r4.log`, `statistics-v448-contracts-r4.log`).
All pinned production Java remains untouched.

Frozen R4 producer:
`.tmp/new-java2rust-statistics-v448-source-generic-arrays-release`, SHA256
`1d8643e8684f23b4f47929287a67cdcaf79bd8be04f1f528b8b65e29d2e79fbe`.
The complete 80-file Descriptive closure is terminal, exit one, at
`.tmp/commons-statistics-1.3-descriptive-v448-source-generic-arrays`: **zero parse
failures and 87 compiler observations**, down from 124. All 37 `apply` trait
signature observations are gone. Five error-status files and seven invalid-pattern
files remain. Elapsed time is 168.236 seconds with other validation jobs running;
this is not evidence of a translator performance improvement. Ranking's complete
five-file closure compiles at `.tmp/commons-statistics-1.3-ranking-v448-core` and
passes **207 exact behavior rows** at `.tmp/statistics-v448-core-behavior`.

The wider preservation check passes **13 Fraction contracts** and **26 of 30
Numbers contracts**. Four Numbers failures remain; preservation is not green:

- Primitive AssertionError details construct Rust String where the lossless Java
  message ABI expects JavaString; char formatting also uses an unsupported
  `JavaCodeUnit::to_string`. The nullable-interface alias fixture encounters the
  same assertion-construction problem on its failure branch.
- The initializer-failure test observes the actual ArithmeticException, but its
  panic-message check expects `/ by zero` as plain text while the panic currently
  prints internal UTF-16 units. Audit the exception reporting contract before
  changing either the producer or the assertion.
- A source `Math::floor` bound reference emits `Math::floor_1` while the generated
  source class declares `floor`. Direct and bound-reference dispatch must reuse
  the same authoritative source method name.

`.tmp/statistics-v448-preservation-audit/producer-comparison.json` records paired
V447/V448 translations of all four exact sources. Every generated Rust output is
byte-identical across the frozen producers. Thus these are existing gaps, not
new V448 regressions; they remain required repairs. Full log:
`.tmp/statistics-v448-numbers-fraction-contracts-r4.log`.

Next work should repair those preservation failures and audit source identity/
naming as a coherent path. The 87 Descriptive observations include 18 missing
Sum references, eight missing BigInteger references, 17 moment-class upcasts
whose diagnostic owners incorrectly use the Numbers Arrays package, and four
NativeOperators method-name observations. Those counts suggest related naming
boundaries, not proof of a single root cause. Audit the existing JavaOwnerKey
qualification against retained SourceTypeNames in merged programs as part of
that work, including the derived generic argument registry; do not create another
per-emitter alias table. Allocation views, native Consumer representation,
result-interface recovery/casts and Send requirements also remain.

`.tmp/statistics-v448-final-batch-evidence.json` ties terminal checks to the frozen
producer, current source/test manifests, verified complete original/staged source
closures, behavior and preservation comparisons. No Statistics module is accepted;
full public-operation/reachable-support audits and broader Numbers/RNG
preservation remain open.

## V449: preservation repairs and source-name collision contracts

The V448 preservation audit proved four Numbers failures predated that batch.
V449's initial focused repair passes all 30 Numbers contracts
(`.tmp/statistics-v449-numbers-r1.log`). General fixes are:

- Primitive AssertionError details now use the existing logical append producer
  and JavaString payload constructor. The scalar fallback reuses the existing
  String.valueOf lowering. No Rust Display conversion is used for lossless char
  messages. Existing checks remain, with additional unpaired-surrogate and
  single-evaluation observations.
- JavaException Debug retains all structured/lossless fields and also includes
  the existing Java Display formatter's readable exception text. Existing panic
  boundaries therefore report `/ by zero` rather than only opaque UTF-16 units;
  the initializer-failure assertion is unchanged.
- Static source calls, including calls synthesized for bound references, choose
  the emitted declaration's authoritative source_method_rust_name through the
  existing emitted_source_callable_for_resolved_call lookup. No Math-specific
  suffix rule is added.

The next naming audit identified concrete corrupted contracts in the full
Descriptive output. Nested collision rewriting passed each nested declaration's
own name as the replacement, so a Builder's reference to Numbers Core Sum became
Builder. Generic inheritance strings such as StatisticAccumulator<Sum> were only
rewritten on whole-string equality. The maintained rewrite now carries one
selected replacement through nested/local declarations, parses hierarchy strings
as Java types, rewrites those nodes, and uses the existing java_type_text renderer.
It also updates same-package/explicit-import uses, respects class/method/
constructor type-parameter shadowing, and includes throws/permits references.

R3's added regression first failed because its expected-type helper itself
shortened qualified generic arguments. That audit also exposed a real hierarchy
parser loss: nested qualified arguments were parsed by the shortening text parser.
The hierarchy parser now recurses in its qualification-preserving mode, including
arrays and extends/super wildcard bounds. Exact expected JavaType nodes and a
qualified hierarchy round-trip check cover both boundaries.

JavaOwnerKey::source_type_in_program now reads retained SourceTypeNames for
unqualified declarations instead of borrowing the merged program's package;
explicit renamed identities retain precedence. This same authority feeds callable
indexes and the derived generic allocation registry. The new owner test supplies
an intentionally unrelated merged package. This is not a claim that every Java
lexical name-resolution boundary has been audited.

R4 passes **1,762 library tests**. Frozen producer:
`.tmp/new-java2rust-statistics-v449-text-source-names-release`, SHA256
`604e5129f99d5afabd5392bdd7c1294754580532dc1cb47637c92ed6c2ee1948`.
The strengthened primitive-message and source-method-reference Java callers both
return zero at `.tmp/statistics-v449-java-contracts/provenance.json`.
The complete Ranking core compiles and passes **207 exact behavior rows**
(`.tmp/commons-statistics-1.3-ranking-v449-core`,
`.tmp/statistics-v449-core-behavior/input-provenance.json`).
The final combined run passes **13 Fraction, 30 Numbers and 33 Statistics
contracts**, terminal exit zero at `.tmp/statistics-v449-contracts-r4.log`.
All four Numbers failures from V448 are repaired with their existing assertions
retained; the primitive-message test is strengthened.

The complete 80-file Descriptive closure is terminal, exit one, at
`.tmp/commons-statistics-1.3-descriptive-v449-text-source-names`: **zero parse
failures and 69 compiler observations**, down from 87. All 18 missing-Sum
observations disappear. Four files retain translation error status (five before),
seven have invalid patterns, elapsed time 155.373 seconds with concurrent checks.
No performance improvement is claimed from these timings.

The next audit has two concrete starting points. Seventeen moment upcast
observations still use the Numbers Arrays package: the separate
`context_type_decl_context_name` helper manufactures names from the merged
Program package instead of reusing declaration identity. Its consumers include
hierarchy diagnostics/lookup and require a whole-path check, not just corrected
error text. Eight BigInteger references arise from real UInt96/UInt128/UInt192
sign-magnitude conversions: the mapped provider currently declares only the
one-argument String constructor, while production calls `(signum, byte[])`.
Extend that actual constructor contract and existing native byte-array ABI;
do not add a placeholder BigInteger struct. Allocation views, native Consumers,
NativeOperators generic method naming, result recovery/casts and Send remain.

Rewrite scope and uncertainties are recorded at
`.tmp/statistics-v449-rewrite-audit.json`; terminal checks, current input manifests,
source closure hashes and behavior are tied together at
`.tmp/statistics-v449-final-batch-evidence.json`. Broader Numbers/RNG production
preservation and complete public-operation/reachable-support audits remain open.
No Statistics module is accepted.

## V450: sign/magnitude provider and existing hierarchy-carrier widening

The V449 complete closure has 69 compiler observations. The next rewrite audit
confirmed that FirstMoment/SumOf* source hierarchy carriers and concrete shared
handle conversions already exist. Several later conversion paths still reject
any target whose slot dispatch is not literally ClosedEnum; these guards now
reuse the existing broader hierarchy-carrier predicate, including optional
unwrapping and avoiding another root allocation around the carrier. Context
owner naming now delegates to the shared JavaOwnerKey authority.

A maintained factory regression exercises leaf results through base and
intermediate carriers, direct construction, virtual reads, retained mutation and
cross-carrier identity. It exposed one further missing whole-path contract:
From<MiddleValue> for BaseValue. The existing supertype conversion generator now
emits that widening as a match moving each unchanged concrete payload handle
into the parent's corresponding variant, after checking the planned variant
subset. It does not copy object state. Anonymous virtual payloads require their
own adapter and are not silently discarded. The focused contract passes at
`.tmp/statistics-v450-hierarchy-r4.log`.

The original synthetic nested/abstract hierarchy and a generic-only intermediate
carrier-demand case expose additional planning gaps. Their exact sources and
failure logs are retained under `.tmp/statistics-v450-audit` and named in
`.tmp/statistics-v450-rewrite-audit.json`. The maintained conversion regression
uses concrete stateful hierarchy owners and an explicit intermediate factory
return, matching the actual Descriptive moment producers. Those separate planner
gaps are not claimed fixed.

The explicit java-math provider now models BigInteger(int, byte[]) through
from_sign_magnitude, reusing the existing borrowed byte-array argument ABI.
It copies unsigned big-endian octets into immutable arbitrary precision storage,
validates signum and zero/nonzero consistency, and preserves exact
NumberFormatException messages. Local JVM observations cover null precedence,
all three valid signs, invalid signs, empty/zero/nonzero magnitudes
(`.tmp/statistics-v450-audit/sign-magnitude-java.log`). The maintained JVM/generated
Rust contract checks input snapshotting, leading zeros, signed high bytes,
128-bit width, mutation/alias inputs, validation messages, argument evaluation
order and preservation of the existing String constructor. It passes at
`.tmp/statistics-v450-magnitude-r2.log`; all 15 runtime tests pass at
`.tmp/statistics-v450-runtime-r4.log`.

### Newly exposed JDK dependency: ByteBuffer

The combined constructor probe first fails because ByteBuffer.allocate is an
unimplemented generic facade, not a stateful JDK implementation. Production
Int128/UInt96/UInt128/UInt192 use allocate, putInt/putLong and array to build
BigInteger magnitudes. The original combined probe and failure are preserved;
its ByteBuffer operation has not been replaced with a manual Rust algorithm.
The focused constructor test uses direct input octets to prove that API contract
independently; this is not acceptance of the still-untranslated buffer path.

The next coherent batch must add a real heap-buffer subset with shared position
state and an identity-preserving backing byte-array view. A copied Vec from
array() would be wrong. The external/native call contract needs an explicit
allocation qualifier for the array-return ABI, applied to expression shapes and
seeded into the existing assigned/returned/initializer storage constraints.
Reuse typed callable identity, external lookup indexes and native alias/allocation
propagation. Verify default big-endian writes, repeated buffer and array aliases,
position/remaining, overflow before mutation, negative capacity and immutable
BigInteger snapshots against JVM callers. The concrete design and uncertainties
are recorded in `.tmp/statistics-v450-rewrite-audit.json`.

Final checks pass **1,762 library tests, 14 Fraction contracts, 30 Numbers
contracts, 34 Statistics contracts and 15 numeric-runtime tests**. Logs:
`.tmp/statistics-v450-library-r4.log`, `statistics-v450-contracts-r4.log` and
`statistics-v450-runtime-r4.log`.
Frozen release producer `.tmp/new-java2rust-statistics-v450-carrier-magnitude-release`
has SHA256 `9174fc5735623c1075d7b1a5781d8b9de15a3cbbeaf75d814bec14e4ae95e5ec`.

The complete 80-file Descriptive refresh is terminal, exit one, at
`.tmp/commons-statistics-1.3-descriptive-v450-carrier-magnitude`: **zero parse
failures, 44 compiler observations** (69 before), four error-status files and
four invalid-pattern files (seven before), elapsed 140.911 seconds. All seventeen
class-upcast observations and eight missing-BigInteger-name observations are
gone. This count is a diagnostic frontier, not a count of all remaining bugs:
the independent buffer probe proves the unimplemented ByteBuffer dependency
even though the full crate's current compiler output does not yet report it.

The complete Ranking core compiles at
`.tmp/commons-statistics-1.3-ranking-v450-core` and passes **207 exact behavior
comparisons** at `.tmp/statistics-v450-core-behavior/input-provenance.json`.
`.tmp/statistics-v450-final-batch-evidence.json` ties these terminal results to
the producer, current source/test manifests, complete original/staged source
hashes, maintained contracts, rewrite audit and the outstanding buffer proof.
The next implementation batch is the shared heap ByteBuffer / native array-return
ABI described above. Native allocation views, Consumer facade/interface views,
NativeOperators method naming, result casts/recovery and Send remain after it.
Broader preservation and full public-operation/reachable-support audits remain
required. No Statistics module is accepted.

## V451: shared heap ByteBuffer and native return allocation contracts

The V450 buffer gap is implemented in `src/java_byte_buffer.rs`, included in the
existing linked platform support. The production-motivated subset is allocate,
putInt, putLong, array, position, remaining and hasRemaining. Buffer clones share
position/state, and array() returns the same mutable signed-byte allocation.
Writes check full capacity before changing bytes or position and use default
big endian order. Negative capacity and overflow use checked Java exceptions;
BufferOverflowException now belongs to the existing binary-name/subtype catalog,
with a null message and RuntimeException parent. Other buffer APIs are not
claimed supported.

A selected facade ABI can now describe native return allocation paths. The
contract is attached to its callable key and Rust provider path: a replacement
provider does not inherit another provider's layout. Expression shapes and
source storage nodes consume the same contract. Existing native storage joins
propagate it through assigned locals, returns, arguments, constructor retention,
fields and initializer calls. No second placement solver or array snapshot is
introduced. The assigned-call propagation pass now uses the common node writer;
its previous duplicated writer omitted fields despite existing initializer edges.

The rewrite audit also found shared owner and result-boundary bugs. Qualified
Java owners now survive resolver lookup, source shadow checks, emitted owner
retargeting and recursive/static external return planning. Native storage type
enrichment inserts only source-symbol-proven fields, so package/type fragments
cannot become fabricated value fields. Plain selected Rust facade ABIs take
precedence over fallback pack effects, including fluent calls. Native carrier
adaptation records its solved slot and plain-value evidence together after
exception handling; cloning/borrowing/staging cannot reapply a Result operation
or wrap the payload in another Option. Real fluent mutations are classified
through the same native shared-carrier registry instead of being mislabeled as
noop facade calls.

`ByteBufferBackingAudit` compares JVM/generated execution with a same-named
source class present. It checks shared position and backing identity across
locals, source returns, arguments, constructor retention, instance/static fields,
source calls in initializers and fluent SDK calls, plus bidirectional byte
mutation, widening, big endian writes, overflow before mutation, null overflow
message and negative allocation. The focused regression passes at
`.tmp/statistics-v451-buffer-r14.log`. `ByteBufferMagnitudeProbe` preserves the
original production-style buffer-to-BigInteger path; no generated Rust is patched.
The concrete failures and whole-path repairs are recorded in
`.tmp/statistics-v451-rewrite-audit.json`.

Final checks pass **1,763 library tests, 15 Fraction contracts, 30 Numbers
contracts, 35 Statistics contracts and 15 numeric-runtime tests**. The maintained
buffer-to-BigInteger contract passes in the full Fraction suite. Logs are
`.tmp/statistics-v451-library-r15.log`, `statistics-v451-contracts-r15.log` and
`statistics-v451-runtime-r14.log`; numeric-runtime inputs are unchanged and their
hashes are verified in the current source manifest.

Frozen release producer `.tmp/new-java2rust-statistics-v451-bytebuffer-abi-release`
has SHA256 `d432d9e797c6e2d64a46adf66234f72df2edeeb56b27d1772c67a4b2e9ac9489`.
The complete five-file Ranking closure compiles and passes **207 exact behavior
comparisons** at `.tmp/statistics-v451-core-behavior/input-provenance.json`.
The complete 80-file Descriptive refresh has **zero parse failures, 44 compiler
observations, four error-status files and four invalid-pattern files**, elapsed
191.962 seconds while preservation checks also ran. The compiler frontier is
unchanged: the independently exposed ByteBuffer gap was masked by earlier errors.
Current generated Int128/UInt96/UInt128/UInt192 magnitude paths now invoke the real
checked buffer implementation and borrow its canonical backing allocation for
the mapped BigInteger constructor; they contain no buffer placeholder body.
This does not establish complete Descriptive behavior acceptance.

`.tmp/statistics-v451-final-batch-evidence.json` ties terminal checks and corpus
results to the frozen producer, **117 source/runtime and 1,140 test input hashes**,
complete original/staged pinned-source verification, maintained callers and
rewrite audit. Cancelled checks of superseded snapshots are recorded separately
and are not final validation evidence. No Statistics module is accepted, and the
all-module objective remains active.

### Next related rewrite audit: native Consumer views and composition

Ten current trait/struct observations and seventeen placement observations occur
at the primitive Consumer interface boundaries. For example, DoubleStatistics
passes seven canonical mutable statistic handles into composeDoubleConsumers;
its generated varargs element conversions incorrectly demand a different physical
carrier. Statistics.composeDoubleConsumers also loses its array container and
callback result to StatisticResult/Any, unlike the synchronized native Int/Long
callback contracts. These observations should be audited as a coherent family.

Before implementing the next batch, trace concrete source handles through their
Consumer views, varargs storage, composition, escaping aggregate field and
invocation, including later result/interface casts. Reuse current functional,
SAM and shared-handle machinery. A closure capturing the original handle can
preserve mutation while still inventing a new view identity or erasing its source
runtime type; derive concrete diagnostic Rust rewrites that prove all three
properties before selecting that representation. The current evidence does not
yet establish whether the existing view framework suffices or needs an extension.
Concrete locations and uncertainties are in the V451 rewrite audit. NativeOperators
naming, result recovery/casts, Send and other allocation conversions also remain,
as do complete module behavior/support audits and broader Numbers/RNG preservation.


## V452: native SAM views in packed arguments and array child domains

The V451 native Consumer audit now has a maintained JVM/generated-Rust caller,
`PrimitiveConsumerViewAudit`. Its baseline has five compiler errors: source
Int/Long implementors were packed as raw handles rather than native SAM views,
and a DoubleConsumer reference could not follow the synchronized storage domain
of a retained array. The audit traces concrete source handles through composite
source interfaces, native views, explicit array storage, packed varargs, source
composition, returned callbacks and invocation. It also checks repeated and
cross-view Java identity, shared mutation, null elements, long-width values and
exception ordering/short circuit. The final source result parent combines native
DoubleSupplier/LongSupplier with its own default getAsLong, matching the mixed
source/native hierarchy pattern more closely. The first repaired focused test passes at
`.tmp/statistics-v452-consumer-r4.log`; the final caller additionally invokes the
same method with an explicit retained array and checks exact call counts.
That extension and an existing callback-array contract exposed a second problem:
lowering reran overload applicability after storage enrichment and repacked an
explicit array as one element. Source-method varargs emission now consumes the
same resolver-selected storage projection. The previous applicability fallback
is retained for calls without per-actual resolution evidence; constructors have
no such method-call coercion metadata and are not claimed repaired here.
Both explicit-array regressions pass in the R6 focused logs, and the stronger
source/native hierarchy fixture passes at `.tmp/statistics-v452-consumer-r8.log`.

Packed source-method arguments now use the resolver's per-actual conversion
metadata to address the final formal array at child zero. Ordinary arguments and
explicit arrays retain their formal root. The previous usage loop zipped actuals
with formal types: it attached the first packed value to the whole array and
missed the rest. `resolved_argument_storage_projection` reuses the existing
per-argument target query; signature-side expanded-varargs recognition consumes
the same projection, including specialized generic explicit arrays.

Varargs element emission now uses ordinary functional-slot lowering, so source
implementors get the existing typed native SAM adapter, with its original shared
receiver, identity token and default-method overrides. Fresh reference-array
initializers record each child value domain through the existing usage graph;
they do not alias outer allocations. Native callback alias transfers also apply
to structural child endpoints, rather than only root-to-root assignments. These
repairs reuse the existing storage/allocation solver and let callback producers,
array children, loop bindings, composition and returned values agree on their
local or synchronized ABI.

The focused caller exposed two further boundaries. Fresh source construction
must publish a requested qualifier only when the existing canonical class
placement proves it satisfies that requirement; allocation goes through the
existing idempotent handle-construction helper. Boxed source interfaces forward
as_any to the original source allocation. Native SAM operation views now use that
protocol for their Java identity instead of giving each interface-view adapter a
fresh identity. These are producer/view repairs; existing references are not
snapshotted or migrated into replacement allocations.

Final checks pass **1,763 library tests, 15 Fraction contracts, 30 Numbers
contracts, 36 Statistics contracts and 15 numeric-runtime tests**. The final
Statistics suite includes both the strengthened view caller and the existing
callback-array preservation contract. Logs are `.tmp/statistics-v452-library-r8.log`,
`statistics-v452-contracts-r8.log` and `statistics-v452-runtime-r8.log`.

Frozen release `.tmp/new-java2rust-statistics-v452-consumer-views-r8-release` has
SHA256 `c1ec977b09f0d2b1e61adc75da39d62ffbf5269ecd515662651a00ea60802baf`.
The complete five-file Ranking closure compiles and passes **207 exact behavior
comparisons** at `.tmp/statistics-v452-r8-core-behavior/input-provenance.json`.
The complete 80-file Descriptive refresh at
`.tmp/commons-statistics-1.3-descriptive-v452-consumer-views-r8` is terminal,
exit one: **zero parse failures, 27 compiler observations (44 before), four
error-status files and four invalid-pattern files**, elapsed 137.193 seconds.
All seventeen audited constructor Consumer-view placement errors are gone;
those producers now build native SAM views retaining their canonical source
handles. The count remains a compiler frontier, not a count of all bugs.

Full-context DoubleConsumer composition still loses its array/result contract:
Statistics.composeDoubleConsumers is rendered with an Arc/Mutex boxed
StatisticResult parameter and Any result. The mixed source/native focused caller
passes, so the complete context's generic/interface pressure must be audited
separately. This batch does not claim that representation problem solved.
Ten primitive Consumer generic-bound observations, four NativeOperators naming
observations, result recovery/casts, Send and three other placement observations
remain in the current compiler output. Further failures may be masked by them.

`.tmp/statistics-v452-final-batch-evidence.json` binds terminal results to the
frozen producer, **117 source/runtime and 1,141 test input hashes**, complete
original/staged pinned-source verification, maintained callers and the V452
rewrite audit. The earlier R5 snapshot's failed contracts and intermediate
27-observation result are preserved as diagnostic evidence, not final validation.
No Statistics module is accepted. The all-module goal, broader Numbers/RNG
preservation and complete behavior/reachable-support acceptance remain active.

Generic `T extends DoubleConsumer`/IntConsumer/LongConsumer remains a separate
open contract: a concrete native Rust facade cannot serve as a Rust trait bound.
The next implementation must derive an invocation protocol from existing native
SAM metadata and source/shared-handle forwarding, preserving mutation, Java
identity and Result ABI through generic producers and consumers. A name-only
replacement would expose missing implementations and could discard exceptions.
The V452 audit records this uncertainty; it is not claimed fixed by the view and
varargs batch.


## V453: checked primitive Consumer bounds

The maintained `GenericConsumerBoundAudit` caller begins with primitive native
facade structs incorrectly used as generic Rust trait bounds. Its minimal
rewrite needs a capability protocol, selected checked invocation, propagated
Result signatures, and source forwarding on the original object. The audit is
recorded at `.tmp/statistics-v453-audit/consumer-bound-plan.md`.

A metadata-derived primitive Consumer capability now separates a generic bound
from its owning facade. The same scoped bound query selects checked callback
call-graph edges and lowering. Local and synchronized native values delegate to
the existing `accept_result` ABI. Source implementors reuse source ancestry,
method naming, selected exception plans and canonical storage; shared handles
call existing `with_outer` companions without holding payload locks. The source
protocol forwarding helper also supports a mutable uniquely owned receiver.
Source interface boxes forward the canonical virtual method and comparisons
reuse `as_any` plus the existing retained-allocation identity protocol.

For stable generic locals, primitive arguments are evaluated before creating a
mutable loan and checking null. Invocation mutates T itself, so a uniquely owned
payload returned from a generic helper retains its updated state. Computed
owning call results can be moved into a staged receiver. Other generic receiver
places still require an identity-preserving snapshot and fail explicitly rather
than cloning an arbitrary mutable T. This boundary is not claimed solved.

The caller covers all three primitive consumers, concrete and boxed source
objects, retained native callbacks, Java reference equality, returned mutation,
wide longs, reentrant invocation, exception message/state/short circuit, null
with empty/nonempty inputs, and argument evaluation before a null invocation.
Additional generated-Rust callers exercise local and synchronized checked
facades directly. An existing mapped numeric field contract caught an optional
source allocation interaction: the canonical Rc/Arc placement must be built
before wrapping the fresh reference in Some. That repair retains existing
constructor fallibility ordering.

The frozen R6 release producer is
`.tmp/new-java2rust-statistics-v453-consumer-bounds-r6-release`, SHA-256
`331f023e456e18943000c835c720ba872bedec8874f4e95b72709f13c9c11d9c`.
The manifests pin 117 source/runtime/Cargo inputs and 1,142 test inputs.
Terminal preservation checks pass: 1,764 library tests, 15 Fraction contracts,
30 Numbers contracts and 37 Statistics contracts. Shared runtime registry
checks pass all nine tests. The prior 15 numeric runtime tests are reusable only
after verifying that every numeric runtime input, its Cargo files and the linked
Java string identity implementation remain unchanged. Complete Ranking again
translates and compiles its five production files and agrees on 207 exact
independent Java/Rust behavior rows.

Complete Descriptive R6 covers all 80 production/dependency files: no parse
failures, 174 compiler observations, four translation-failed files and four
invalid-pattern files, in 144,139 ms. None of the ten invalid Consumer-struct
bound errors remain. V452's 27 observations were an early checking frontier,
not a complete bug count. Five representative faulty patterns are verified in
both frozen outputs, including source callback `create_from_range` calls on a
RangeFunction whose actual method is `apply`, Statistics `add_2` naming drift,
moving `self.moment` from a shared receiver, and invalid carrier field reads.
This does not prove every newly reported diagnostic is pre-existing; attribution
and regressions remain necessary. The histogram and comparisons are in
`.tmp/statistics-v453-rewrite-audit.json`.

The renewed whole-path audit also identifies a semantic constructor defect:
flattening `super(sc, ss, m1, n)` emits an inherited field self-assignment instead
of propagating the `ss` argument. Existing hierarchy carriers already retain
Arc/Mutex handles and expose the necessary field projections; the minimal
correct repairs use those projections, preserve lexical constructor parameter
bindings, and clone retained reference handles before consuming optional casts.
The escaping StatisticResult callback must retain the original source object
and support reentrant checked invocation; changing Fn to FnMut alone is not a
correct solution. These examples currently indicate contract/projection work
within the existing representations, with source-box identity still requiring
proof in each adapter path.

Next is a coherent source callback invocation/retained-result contract batch:
keep the selected SAM method distinct from its initializer's static factory,
reuse canonical generic overload names, and trace source result-interface
storage through wrappers and invocation. Moment hierarchy constructor/carrier
repairs form a separate batch. The existing DoubleConsumer whole-array/result
type loss persists and its connection to these failures is not yet established.
Final pinned-source and current-input verification is recorded in
`.tmp/statistics-v453-final-batch-evidence.json`. No Statistics module is accepted
by this focused batch; the full multi-module goal remains active.

## V454: stored source callbacks, exact namespaces and checked field contracts

Stored source callback values need the same selected method, receiver identity
and checked ABI at initialization, storage and invocation. The maintained
`StoredSourceCallbackAudit` exercises a static factory whose type spelling
collides with a callback field, generic three-input stored/copied/bound source
SAMs, a mutable source receiver retained through a source interface, recursive
callback invocation, exception recovery, eager null capture, an exact capitalized
local that shadows its type, and changed referent state after capture.

The source method-reference resolver and bound emitter had separate namespace
queries, both allowing normalized field lookup before checking a type owner.
They now share `TypeEnv::method_reference_value_binding` with effect discovery:
exact Java local/field bindings first, known source types and static aliases
next, normalized compatibility fallback last. Once the static factory is
correctly selected, the existing typed reference-to-lambda path supplies array
carrier/borrow adaptation and the source method's Result ABI.

The ordinary call emitter already copied an owned source-interface box before
arguments and invocation, preserving its underlying handle and releasing owner
field guards. Its outer gate excluded all zero-argument calls. Those source
interface calls now use the same snapshot path, including calls generated inside
retained method-reference callbacks. Each invocation can mutate its own box view
of the original object without holding a callback storage loan across user code.
The source with_outer forwarders keep reentrant mutations on the canonical
allocation. No FnMut conversion or payload-copy adapter was introduced.

A stronger invalid-input caller exposed a separate fallibility gap: returned,
assigned and passed source SAMs registered their deferred bodies, but source
field declarations did not. Fields now reuse that contract registration, with a
field-initializer lexical key that separates callback identities from other
fields and constructor-body callbacks. The interface invocation acquires the
source factory's checked effect; constructing the holder does not execute that
factory. A library regression verifies both properties and distinct field roots.
This registration covers deferred callback contracts, not every eager field
initializer effect.

The first generation boundary was a false alias report: storing a reference in
`holder.field` was treated as aliasing `holder` itself. Diagnostic places now
name the stored field, track mutations through either reference and observe the
matching field reads. Positive alias detection remains covered; an existing
assertion now expects `holder.node` rather than the containing `holder`. An
additional negative case excludes unrelated mutation of the containing object.

The rewrite audit is `.tmp/statistics-v454-audit/callback-contract-plan.md`.
The R5 extended JVM/generated-Rust behavior caller and the final complete
Statistics contracts pass. The frozen producer is
`.tmp/new-java2rust-statistics-v454-source-callbacks-r6-release`, SHA-256
`0e4f65cf780910e936601a7dfdfdd6061349ce2ad8d8807aed51176b9c050062`.
Current-input manifests pin 117 source/runtime/Cargo files and 1,143 test inputs.
Terminal checks pass: 1,768 library tests, 15 Fraction contracts, 30 Numbers
contracts, 38 Statistics contracts and nine shared runtime registry tests.
Prior numeric runtime evidence is reused only after verifying all its runtime,
Cargo and linked source inputs are unchanged. Complete Ranking still translates
and compiles all five files and matches 207 exact independent behavior rows.

Complete Descriptive R6 covers all 80 production/dependency files: no parse
failures, 164 compiler observations, four translation-failed files and four
invalid-pattern files, total 179,617 ms. Comparing diagnostic message counts
with V453 removes exactly nine create_from_range-on-RangeFunction errors and
the mutable captured-box Fn error, without adding a new message group. Counts
remain a compiler frontier rather than independent bug counts. The histogram
and differences are in `.tmp/statistics-v454-rewrite-audit.json`; verified
original/staged source and frozen producer provenance is in
`.tmp/statistics-v454-final-batch-evidence.json`.

Next is the moment hierarchy constructor/carrier batch already audited in V453:
retain existing Arc-backed carrier references before consuming optional casts,
reuse generated carrier field projections, keep super-call Result handling on
the selected helper ABI and preserve lexical bindings through delegated
constructor arguments. The inherited field self-assignment from
`super(sc, ss, m1, n)` is a semantic blocker independently of compilation.
Generic overload naming, numeric provider routing, source result-interface
recovery and the DoubleConsumer whole-array/result type loss remain distinct
families. No Statistics module is accepted by this batch; the full goal remains
active.

## V455: delegated constructor bindings and hierarchy carrier projections

The maintained `HierarchyConstructorAudit` covers four inheritance levels,
nonzero inherited fields, a count above 32 bits, ordered effectful super
arguments, copy constructors, retained optional hierarchy fields, repeated
narrowing, canonical alias mutation and independent copied state. Its extended
caller also checks null-copy exceptions and repeated failed downcasts without
consuming the stored carrier. The initial generated code both failed to compile
and silently assigned an inherited field from itself in scalar constructors.

Recursive constructor replay recorded parameter access aliases without their
lexical local shapes/types. It now publishes those already-proven physical
argument bindings through the existing typed local/access-name API. Existing
argument materialization, substitution and evaluation order remain in use.
The production Cubed/Fourth constructors now initialize `sum_squared_dev: ss`;
copy constructors project the selected hierarchy carrier through its getters.
This audit covers read-only delegated parameters, not all possible mutating
constructor-parameter or constructor-local frames.

Shared-object field lowering now declines hierarchy enums so the carrier getter
path handles them. A shared accessor query uses the same declaration and emitted
field naming authorities as getter emission, including inherited fields and
parser aliases; accessor facts distinguish Java spelling from an already chosen
Rust name. A regression distinguishes Java `self`/`self__` and their distinct
emitted accessors. Consuming subtype conversions now copy retained field
references through the existing clone policy, including nullable carriers,
preserving canonical object handles. Constructor explicit-slot arguments also
reuse destination-carrier null construction for reference casts of null,
matching ordinary argument and varargs lowering.

The whole-path audit is
`.tmp/statistics-v455-audit/constructor-carrier-plan.md`; production constructor
excerpts are `.tmp/statistics-v455-audit/production-constructors.txt`. The frozen
producer is `.tmp/new-java2rust-statistics-v455-hierarchy-r6-release`, SHA-256
`a5fa2ac346fdce8b593ac423663eb850848872f07f70eda5cea52c658959bd74`.
Manifests pin 117 source/runtime/Cargo inputs and 1,144 test inputs. Terminal
checks pass: 1,769 library tests, 15 Fraction, 30 Numbers and 39 Statistics
contracts, plus nine shared-runtime registry tests. The 15 numeric-runtime tests
reuse prior evidence only after proving all eight runtime/Cargo/linked-source
inputs unchanged. Complete Ranking still compiles all five files and matches
207 exact independent behavior rows.

Complete Descriptive covers all 80 production/dependency files: no parse
failures, 138 compiler observations, four translation-failed files and four
invalid-pattern files, total 185,463 ms. Compared with V454, all 16 carrier move
errors, seven raw carrier lock reads and three raw field projections disappear;
no new diagnostic message group appears. The semantic scalar-constructor fix
is additional to that compiler reduction. The frontier audit and verified
original/staged source provenance are
`.tmp/statistics-v455-rewrite-audit.json` and
`.tmp/statistics-v455-final-batch-evidence.json`.

The next related batch is selected lexical super-method helper contracts:
Cubed/Fourth calls propagate Result from helpers that return carriers directly,
and Fourth converts an argument to Squared's carrier while calling Cubed's
helper. A remaining FirstMoment factory field write also treats its carrier as
a raw mutex handle. Concrete examples, minimal rewrites and the existing shared
contract/setter mechanisms to trace are recorded in
`.tmp/statistics-v455-audit/next-super-method-contract.md`. Source overload
naming, numeric provider routing, source result-interface recovery and whole
DoubleConsumer array/result carrier loss remain distinct families. No Statistics
module is accepted; the full goal remains active.

## V456: lexical super implementation contracts and carrier field writes

The maintained `SuperImplementationAudit` reproduces the four-level moment
hierarchy's overload-specific super calls and carrier writes. It verifies
nonzero aggregate state, repeated combination, retained aliases, partial state
after a checked failure and subsequent recovery, inherited and direct super
calls, checked calls in return position, and original-object identity returned
through a superclass implementation. Field-write checks cover parser-normalized
camel-case declarations, an effectful receiver and RHS, null-check ordering,
and a throwing RHS taking precedence over a null receiver.

Helper inventory, call-side argument/Result planning and transitive receiver
identity requirements now reuse one lexical superclass implementation query.
The selected declaration feeds the existing frame-specialized signature and
parameter projection. The final shared argument-lowering pass also gives that
helper contract priority, including its declaration-specific generic
instantiation; otherwise it could overwrite an earlier correct plan with the
preceding ancestor's carrier. Existing varargs and argument staging remain in
use. Fallible statement lowering no longer adds an unconditional question mark
to every super call: ordinary call lowering applies the selected helper's ABI.

The existing canonical-receiver requirement scan now follows lexical super
edges. This prevents emission of borrowed callers that invoke identity helpers
requiring the original object's handle; existing handle-aware companions and
inherited-method emission supply the correct path. The four existing lexical
super tests pass, including cross-file generic binders, covariant returns,
constructor/initializer helper inventory and reentrant receiver identity.

Carrier field writes reuse `source_instance_field_decl` rather than comparing
Java access spelling directly with parser-normalized declarations. The setter
name comes from the same declaration used by emission. For nullable canonical
shared carriers, lowering snapshots the reference before evaluating the RHS,
then checks null and invokes the setter. No allocation guard spans user RHS
code. The existing mutable value-carrier path remains; this new ordering
regression covers canonical shared references. The setter emission regression
now checks this staged form instead of requiring the obsolete inline form.

The whole-path plan and baseline diagnosis are
`.tmp/statistics-v456-audit/implementation-plan.md`. The extended JVM/generated
Rust caller passes, as do the final 1,769 library tests, 15 Fraction, 30 Numbers
and 40 Statistics contracts, and nine shared-runtime registry tests. Prior
15-test numeric-runtime evidence is reused only after proving all eight
runtime/Cargo/linked-source inputs unchanged. Manifests pin 117 source inputs
and 1,145 test inputs. The frozen producer is
`.tmp/new-java2rust-statistics-v456-super-contracts-r6-release`, SHA-256
`c7260a259746b62cf7f5e2b87d02f503732533131f73d3865e25998c70876d98`. Complete Ranking still compiles all five files and matches 207 exact
independent behavior rows.

Complete Descriptive covers all 80 production/dependency files, with
0 parse failures, 133 compiler
observations, 4 translation-failed files and
4 invalid-pattern files, total
137,247 ms. The exact diagnostic differences and
verified original/staged source/producer provenance are
`.tmp/statistics-v456-rewrite-audit.json` and
`.tmp/statistics-v456-final-batch-evidence.json`. Counts remain compiler
observations, not independent bug counts. No Statistics module is accepted.

Next is reusable numeric dependency support actually called by Descriptive:
BigInteger double/exact integer conversions and shiftLeft, and BigDecimal
integer conversion. The runtime and its explicit mapped API manifest currently
lack those operations; production examples and whole-path requirements are
recorded in `.tmp/statistics-v456-audit/next-numeric-provider-plan.md`. Source
overload naming, source result-interface recovery and whole DoubleConsumer
array/result carrier loss remain separate families. The full goal remains
active.

## V457: mapped numeric conversions and retained call snapshots

Production BigIntegerStatisticResult and Statistics call BigInteger double,
exact int and exact long conversions; Int128 calls shiftLeft, and Statistics
uses BigDecimal.toBigInteger. The existing arbitrary-precision runtime and
explicit mapped API manifest now implement those five operations. Double
conversion reuses the pinned num-bigint backend's sticky-bit/nearest-even
conversion through num-traits 0.2.19, already present transitively and now an
explicit dependency. Exact conversions use checked backend conversion and
Java ArithmeticException messages. Left/right shifts share the directional
magnitude/range implementation, including negative distances and reference
retention for no-op shifts. Decimal integer conversion reuses scale reduction
with DOWN and existing range checks; it does not introduce separate rounding
or caller algorithms.

The maintained NumericConversionsProbe compares JVM and generated Rust for
signed boundaries and overflow exceptions, chained and retained receivers,
integer-to-double observations around 2^53 and a 65-bit sticky-bit boundary,
signed decimal truncation and scales, original aliases, repeated state, negative
shifts, nullable receivers, argument effects/failures and static receiver
rebinding during argument evaluation. These paths motivate exact primitive
conversion checks; no elementary-function JVM rounding policy was added.
The Java API contracts are [BigInteger](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/math/BigInteger.html)
and [BigDecimal](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/math/BigDecimal.html).

The caller exposed an owned-holder field read moving its mapped immutable
reference. Existing field-owned recovery now also handles mappings with an
explicit reference-identity contract and a proven clone capability. It copies
the canonical reference through the existing optional/required slot adapters,
covering both getter expansion and direct reads without consuming the holder.

Mapped readonly reference calls now reuse owned argument staging: receiver
capture precedes Java argument effects and primitive conversions, while the
required receiver projection and final borrowed-parameter projections happen
at invocation. The physical snapshot remains authoritative even when an
argument rebinds its original static field. Shared selected-owner argument
lookup feeds both final mapped lowering and snapshot reborrowing; re-inferring
a chained receiver had lost its mapped return identity. Owned temporaries also
use the existing reuse/liveness recovery policy so repeated local references
remain usable after calls.

The complete Fraction suite caught a preservation regression in the initial
staging implementation: borrowing expression temporaries across a tuple
statement produced E0716, and unnecessary receiver slot re-entry overflowed the
ordinary debug test stack. The final path reuses existing owned argument
temporaries, performs final loans at invocation, and lowers already-owned
receiver producers once through the expression path. All 16 Fraction contracts,
including the existing field/arithmetic probe, pass on the ordinary test-thread
stack. A larger stack was used only to diagnose the failed intermediate output;
it is not part of reproduction or the delivered fix.

The audit is `.tmp/statistics-v457-audit/implementation-plan.md`. Fresh terminal
checks pass: 1,769 library tests, 16 Fraction, 30 Numbers and 40 Statistics
contracts, 19 numeric-runtime tests and nine shared-runtime registry tests.
Manifests pin 117 source/runtime/Cargo inputs and 1,146 test inputs. The frozen
producer is `.tmp/new-java2rust-statistics-v457-numeric-r11-release`, SHA-256
`5ca7246b0fb9f5ea8de6ef30954d9ff810141e17db5dfcf3ebd1276fcc839000`. Ranking still compiles its complete five-file closure and matches 207
exact independent behavior rows. Complete Descriptive covers all 80 production
and dependency files: 0 parse failures,
119 compiler observations, 4
translation-failed files and 4 invalid-pattern
files, total 197,758 ms. Its previous frontier was
133 observations; the exact diagnostic differences and source/producer checks
are `.tmp/statistics-v457-rewrite-audit.json` and
`.tmp/statistics-v457-final-batch-evidence.json`. Counts are compiler
observations, not independent bug counts. No Statistics module is accepted.

Next is generic callable identity/name routing: Statistics.add_2/add_4 calls do
not name the emitted bound-specific generic overloads, and NativeOperators
trait/implementation naming differs after inherited generic multiplication.
Examples, minimal rewrite scope, existing slot/naming authorities and unresolved
selection questions are in `.tmp/statistics-v457-audit/next-generic-name-plan.md`.
Visibility, allocation and source result-interface/DoubleConsumer carrier
recovery remain separate failure families. The full goal remains active.

## V458: scoped generic overload and inherited interface identity

Resolver overload de-duplication previously collapsed generic binders named T
without considering their declared upper bounds. In Statistics.add, this lost
IntConsumer/LongConsumer alternatives before applicability checks and left
calls naming nonexistent add_2/add_4 methods. The shared Java signature erasure
now follows scoped declared bounds, arrays and first intersection bounds, with
cycle termination. Resolver keys compose owner, projected receiver and method
binder environments in their lexical precedence. Method binders shadow owners;
ancestor substitutions retain child bounds even when binder spellings coincide.

The full library suite caught an intermediate regression: Derived<T> extending
Base<A=T> acquired an invented same_t_t overload, preventing reference-identity
requirements from reaching its real override. Hierarchy, unqualified and
effective receiver candidate de-duplication now share the projected receiver
erasure environment. The existing identity-index regression passes unchanged;
new units cover alpha-renamed ancestors, same-spelling stronger child bounds,
method-over-owner shadowing, distinct bounds, array erasure, F-bounds and cycles.

Synthetic InheritedContract methods now preserve the actual source ancestor's
slot/name when it is available; native-only materialized contracts remain
available. This rule is shared by resolver hierarchy lookup and specialized
interface contract selection. Real child redeclarations still select their
own surface. Interface upcast bridges match the parent and selected child
through existing specialized JavaCallableSlotKey and naming authorities,
preserving the parent public signature while invoking the selected child's
emitted name. The existing covariant Object-return adapter uses that same
selection. No missing-name aliases or corpus-specific fallback dispatch were
added.

Maintained independent JVM/generated callers cover bound-discriminated static
overloads across source files, all three primitive consumer bounds, wide long
values, arrays and ranges, source subinterfaces, callback state/identity,
partial checked failure, invalid ranges and null invocation. A second caller
covers inherited generic/primitive overloads, explicit child redeclarations,
upcasts and canonical reference identity. The concrete audit is
`.tmp/statistics-v458-audit/implementation-plan.md`.

Fresh terminal checks pass 1,772 library tests and all 16 Fraction, 30 Numbers
and 42 Statistics contracts. Nine shared-runtime registry tests also pass. Numeric-runtime checks reuse
the R4 artifact with 19 passing tests after verifying unchanged runtime inputs.
Manifests pin 117 source/runtime/Cargo inputs and 1,149 test inputs. The frozen
producer is `.tmp/new-java2rust-statistics-v458-generic-r5-release`, SHA-256
`0eebd71b0ffb0e4d79e0803c575be609cd58be0e5bf83ececaa1a9f9e572751a`. Ranking's complete five-file closure compiles and matches 207 exact
independent behavior rows. Complete Descriptive includes all 80 production and
dependency files: 0 parse failures,
107 compiler observations,
0 translation-failed files and
4 invalid-pattern files, total
145,885 ms. Its previous frontier was 119 compiler
observations; exact diagnostic differences and frozen-input verification are
`.tmp/statistics-v458-rewrite-audit.json` and
`.tmp/statistics-v458-final-batch-evidence.json`. These are compiler observations,
not independent bug counts. No Statistics module is accepted.

The next candidate batch is lexical nested source-type identity: IntStatistics
and LongStatistics factories allocate their own Builder while storing locals
as DoubleStatistics_Builder and selecting double-array build contracts. The
minimal rewrite must preserve one enclosing owner's identity through local
types, constructors, captures, selected signatures and results. Existing lexical
qualification and source identity authorities are identified in
`.tmp/statistics-v458-audit/next-nested-type-plan.md`; determine the first wrong
producer before editing. Collection callbacks, erased/result-interface recovery,
private getter-field visibility and literal typing remain separate families.
The full all-module goal remains active.

## V459: source declaration identity before body evidence

IntStatistics and LongStatistics factories had correctly resolved lexical
Builder declarations and constructors, but emitted local storage and factory
returns selected DoubleStatistics_Builder and its double-array build contract.
Both the combined 80-file context and isolated primary resolver AST preserve
the correct owner; the fault is downstream signature/declaration lookup.

The shared Java signature source alias matcher now preserves explicit nested
owner disagreement across dotted, binary and flattened names, including array
rank. Broad external API fact aliases remain unchanged. Signature declaration
lookup uses that source identity relation. Exact emitted/canonical/binary names
first select a coherent declaration family; ambiguous short names cannot select
an unrelated owner merely because it has more implementation bodies. Within
one original canonical Java identity, the existing body-evidence policy still
prefers a complete declaration over bodyless summaries. When original names are
unavailable, the existing complete source suffix matcher supplies the alias
comparison. Nested type canonicalization now reuses this same selection policy.
No Builder-specific exceptions, new type-name printer or new storage carrier
were introduced.

The direct regression failed before the fix: requesting Second.Builder returned
First.Builder because the latter had more method bodies. It now also checks
ambiguous leaves, a richer top-level Builder competing with a qualified nested
name, exact top-level selection and same-identity full/summary copies. Existing
flattened-package and qualified-top-level protections continue to pass. Two
maintained independent JVM/generated callers exercise sibling builders in one
compilation unit and across source files, distinct primitive widths/result
types, factories, chains, canonical self identity, repeated references,
escaping native consumers, checked partial state/recovery, immutable result
snapshots and method-binder shadowing. Both behavior callers already passed at
baseline and are preservation coverage; the direct lookup test reproduces the
fault. The audit and baseline artifacts are under
`.tmp/statistics-v459-audit/implementation-plan.md`.

The original debug-symbol build ran out of disk before executing the regression.
A compact test build confirmed its semantic failure. A later release archive
and one Fraction fixture also failed solely with ENOSPC. Only superseded build
cache executables/library/object files from completed work were removed;
sources, logs, pinned inputs and frozen producers were preserved. Current tests
use CARGO_PROFILE_TEST_DEBUG=0 to reduce artifact size, with ordinary thread
stacks and unchanged assertions/behavior. These failed infrastructure runs are
retained separately from final terminal evidence.

Fresh terminal checks pass 1,774 library tests, 16 Fraction, 30 Numbers and
44 Statistics contracts, plus nine shared-runtime registry tests. Numeric-runtime
checks reuse the 19-test V458 R4 result after verifying unchanged runtime inputs.
Manifests pin 117 source/runtime/Cargo inputs and 1,153 test inputs. The frozen
producer is `.tmp/new-java2rust-statistics-v459-nested-r3-release`, SHA-256
`f0e9168384058c4f898112035c2b2a2901e592a98375173cdef7f6a4035dc020`. Ranking's complete five-file closure compiles and matches 207 exact
independent behavior rows. Complete Descriptive includes all 80 production and
dependency files: 0 parse failures,
85 compiler observations,
0 translation-failed files and
4 invalid-pattern files, total
146,658 ms. The previous frontier was 107 compiler
observations. Concrete Int/Long builder storage and factory returns are verified;
18 mismatched argument/result observations and four erroneous builder downcasts
are removed. Four existing forEach receiver errors now name their correct
Int/Long owners rather than DoubleStatistics_Builder, remaining in the same
callback family. Exact diagnostic differences and frozen-input verification are
`.tmp/statistics-v459-rewrite-audit.json` and
`.tmp/statistics-v459-final-batch-evidence.json`. Counts describe compiler
observations, not independent bugs. No Statistics module is accepted.

The full all-module objective remains active. Remaining callback/collection,
result-interface/erased value, allocation, visibility and primitive typing
families require their own whole-path audits and coherent batches. The next
candidate is Set.forEach(b::add): the existing statement planner only accepts
lambda syntax. Eager bound receiver capture, null/effect order, checked consumer
invocation, source override and iterator aliasing requirements are recorded in
`.tmp/statistics-v459-audit/next-foreach-plan.md`.

## V460 — platform collection actions and synthesized callback contracts

Platform Collection/Iterable.forEach now stages the collection receiver and one
Consumer value in Java evaluation order, checks nulls, and reuses enhanced-for
iteration and native SAM invocation. Bound factories run once; argument failures
precede null collection checks; empty sources still reject null actions. Stored
consumers retain their solved input domain and canonical allocation. Source
implementations take precedence over platform and stream shortcuts. Static
bound references use the ordinary static value producer. Qualified native SAM
names share declaration/constructor carrier naming. Inline source callback
signatures project the selected emitted slot, including nullable arguments and
retained array allocations, rather than reconstructing a declaration-only ABI.

The usage graph transfers collection element domains into callback inputs and
scalar stored-reference domains into container children, preserving branch
non-null refinements and existing canonical null/cast handlers. Initialized enum
values reuse publication-aware slot adaptation; enum key equality follows
canonical singleton identity. Shared support assembly recognizes qualified
thread_local! groups through the existing macro/item scanner and moves each
group atomically with its users. Synthesized native carriers are explicitly
requested from existing facade support, including files with no Consumer import.
No pinned corpus or generated-code fixes were used.

Independent maintained JVM/generated callers cover bound and stored actions,
receiver factories, empty/null receivers/actions/bound receivers, exception and
side-effect order, source rebinding, checked partial state/recovery, source
forEach overrides, nullable enum elements, aliased concurrent mutation and local
builder identity. The second caller has no imports or Consumer declarations and
exercises both bound references and lambdas. A focused retained-array ABI unit
checks canonical shared callback argument storage. The compiled registry test
proves two thread-local cells remain one shared state group after promotion.
The baseline/rewrite audit is `.tmp/statistics-v460-audit/implementation-plan.md`.

Fresh terminal checks pass 1,777 library tests, 16 Fraction, 30 Numbers and
46 Statistics contracts. Ten shared-runtime registry and five Rust-item parser
checks pass on unchanged frozen pipeline inputs. The 19-test numeric-runtime
result is reused after checking unchanged source hashes. Frozen manifests cover
117 source/runtime/Cargo inputs, 1,155 test inputs and nine pipeline inputs.
The frozen producer is `.tmp/new-java2rust-statistics-v460-foreach-r10-release`,
SHA-256 `f5c6c89b00eac8b1c9e45bf5890d7474f6f5375943f4a224e4b75e49354e016d`. Ranking's complete five-file closure compiles and matches 207 exact
independent JVM rows. Complete Descriptive includes all 80 production/dependency
files: 0 parse failures, 73 compiler
observations, 0 translation-failed files and
4 invalid-pattern files, total 148,235ms.
The previous 85 observations include twelve callback/unprojected-builder errors
removed by this batch. Diagnostic differences are recorded in
`.tmp/statistics-v460-rewrite-audit.json`; full frozen-input/provenance verification
is `.tmp/statistics-v460-final-batch-evidence.json`. Intermediate R8 and R9
failures remain diagnostic evidence and are superseded by R10 terminal checks.
Counts are compiler observations, not independent bugs. No Statistics module is
accepted; the full all-module objective remains active.

The next candidate batch is source getter boundary preservation and Java
integer promotion in emitted wrapping operations. Representative minimal
rewrites and producers/storage/signatures/consumer paths are recorded in
`.tmp/statistics-v460-audit/next-accessor-integer-plan.md`. Remaining complete
Descriptive families also include result-interface recovery, allocation,
primitive Consumer/erased storage and synchronization. Ranking still needs its
full acceptance audit, followed by Distribution, Interval and Inference and the
final preservation/requirement audit.

## V461 — emitted accessor boundaries and promoted constant receivers

Source getter and setter inlining now preserves the method boundary when its
field owner is emitted in a different Rust module. The guard uses the existing
emitted-source callable inventory and leaves same-unit inlining and external
facade contracts under their existing planners. The field-borrow result shortcut
uses the same guard, so consumers use solved callable return storage. Private
fields remain private. This fixes Int128.lo64(), IntSum.getSum() and the related
cross-file accumulator/getter family without widening storage visibility or
copying mutable state. Raw and structured integer literals now share the
existing signed-decimal predicate and Java promotion renderer before wrapping
operations. Constants such as Integer.BYTES retain int arithmetic even when the
result is assigned to long; explicit long promotion retains long arithmetic.
ByteBuffer.capacity() uses the existing immutable capacity state and ordinary
API/ABI catalogue, completing an actual maintained caller's state observation.

A maintained three-file JVM/generated caller reproduces the baseline failures:
six private-field accesses, two ambiguous wrapping receivers and one separately
missing ByteBuffer.capacity surface. The completed caller verifies retained
getter-result identity, private scalar getter/setter storage, side-effectful
receiver evaluation, checked null calls, int overflow before widening, long
promotion and fixed capacity versus changing position/remaining space. A focused
unit covers positive and negative raw constant receivers at int and long widths.
The rewrite audit is `.tmp/statistics-v461-audit/implementation-plan.md`.

Terminal checks pass 1,778 library tests, 16 Fraction, 30 Numbers and 47 Statistics
contracts. Library/release results use R4 source inputs, verified identical to
final R5; integration contracts were rerun at R5 after repairing an accidental
fixture-source replacement. All pre-existing Statistics test text is verified
byte-for-byte against its V460 hash after removing the new test. Intermediate
failed/cancelled logs remain diagnostic evidence. Ten registry/five Rust-item
checks are reused under identical pipeline hashes, and 19 numeric-runtime tests
under unchanged numeric-runtime hashes. Frozen manifests pin 117 source/runtime/
Cargo inputs, 1,158 test inputs and nine pipeline inputs. The producer is
`.tmp/new-java2rust-statistics-v461-accessors-r5-release`, SHA-256
`c8231238ee84b0b0181417ab43c3aebe1f0e33d23ad9762671cef26a3ccdb8c3`. Ranking's complete five-file closure compiles and matches 207 exact JVM
rows. Complete Descriptive includes all 80 production/dependency files:
0 parse failures, 47 compiler observations,
0 translation-failed files and 4
invalid-pattern files, total 150,117ms, compared with 73 previous
compiler observations. Diagnostic differences and complete frozen provenance are
`.tmp/statistics-v461-rewrite-audit.json` and
`.tmp/statistics-v461-final-batch-evidence.json`. Counts are compiler occurrences,
not independent bugs. No Statistics module is accepted; the full objective
remains active.

Remaining allocation/static-array and factory/callback/result-interface families
need their own whole-path audits. Candidate examples and uncertainties are in
`.tmp/statistics-v461-audit/next-allocation-plan.md`. Ranking acceptance, complete
Distribution/Interval/Inference closures and the final preservation/requirement
audit also remain.

## V462 — static allocation and typed constructor reference contracts

Published source arrays now keep their canonical allocation through const-looking
initializer shortcuts. A static array read uses ordinary static storage and slot
adaptation; clone still creates a fresh outer allocation. Native method references
participate in the existing value-domain scan and connect selected source method
inputs/results and constructor arguments to SAM invocation domains. Source
constructor references share ordinary-new visibility, overload and generic
selection through SourceSymbols; emission lowers a typed lambda containing the
selected New expression. Existing constructor argument, checked-result and output
allocation adapters retain ownership of those contracts. Typed callback call-graph
edges include source constructor calls, keeping exception signatures consistent.

Focused owner regressions also exposed lookup gaps: storage dependency lookup
discarded dotted nested owners when emitted keys were flattened. Field
and method lookup now reuse V459's complete source-owner suffix matcher, exact-first
unique selection and member-name indexes. The indexed/unindexed regression keeps
Alpha.Builder and Beta.Builder distinct and rejects ambiguous leaves. Constructor
references reuse the existing declaration-aware contextual owner lookup. Batch
source-owner renaming now rewrites nested Java type suffixes as well, preserving
Sum.Builder when the imported Sum is disambiguated against another package's Sum.
No new name-mapping scheme, generated-code patch or source-corpus edit was used.

Three maintained independent JVM/generated callers verify repeated empty/nonempty
static-array identity, clone freshness, mutation visibility, retained publication,
stored reference-returning callbacks, overloaded zero/one/two-argument constructors,
array-retaining constructors, distinct results sharing the original input, native
and source SAMs, checked failure/recovery, imported same-leaf source constructors
and sibling int/long builder static arrays. All original regression text is
preserved. Initial baseline/R1/R2/R3 diagnostics and the concrete rewrite audit
are `.tmp/statistics-v462-audit/implementation-plan.md`. R2's complete production
run replaced six bogus vector conversions with explicit missing-sync boundaries;
its unqualified Sum reference was also intermediate evidence. R4 and final R5
still have those seven production gaps despite the focused owner cases passing.
R5 adds tested qualified-package owner matching and same-package constructor
lookup, but those changes did not close the actual production paths. The next
action is to capture their concrete lowering context and signature/source-symbol
witness before further implementation changes.

Fresh terminal checks pass 1,779 library tests, 16 Fraction, 30 Numbers and
50 Statistics contracts. Ten registry/five Rust-item checks are reused under
identical frozen pipeline hashes, and 19 numeric-runtime tests under unchanged
numeric-runtime hashes. Manifests cover 117 source/runtime/Cargo inputs, 1,164
test inputs and nine pipeline inputs. The frozen producer is
`.tmp/new-java2rust-statistics-v462-references-r5-release`, SHA-256
`a8397904d9f51197a561cbd2d06860bfcff543137a9eca6532bed1e45d9d7dd8`. Ranking's complete five-file closure compiles and matches 207 exact
independent JVM rows. Complete Descriptive covers all 80 production/dependency
files: 0 parse failures, 46 compiler
observations, 0 translation-failed files and
4 invalid-pattern files, total 146,801 ms,
compared with 47 previous compiler observations. Exact differences and complete
input/provenance verification are `.tmp/statistics-v462-rewrite-audit.json` and
`.tmp/statistics-v462-final-batch-evidence.json`. Compiler occurrences are not
independent bug counts. No Statistics module is accepted; the all-module goal
remains active.

Remaining families and representative rewrite requirements are recorded in
`.tmp/statistics-v462-audit/next-interface-plan.md`. Ranking acceptance, complete
Distribution/Interval/Inference closures and the final preservation/requirement
audit remain. Only two obsolete completed test executables in the task's Cargo
cache were removed for disk headroom; source, logs, pins and frozen producers
were preserved, with paths/sizes recorded in the batch audit directory.

## V463 — exact callable inventories and constructor owners

Constructor-reference resolution and its synthetic New fallback now reuse the
qualification-preserving hierarchy type parser. The production trace showed
that the ordinary type parser discarded the package from the selected Statistics
Sum when another Sum was also present. The focused resolver regression checks
both owner identities and the selected constructor arity.

Storage analysis now enumerates the existing exact Rust-name usage inventory.
The legacy Java-name alias map allowed later build overloads to overwrite the
zero-argument build body's facts. Exact Rust-name lookup also prefers that
inventory. A failing-before/passing-after regression demonstrates the lost
static-array requirement; an independent JVM/generated-Rust caller verifies
identity and mutation through zero/one/three-argument factories. Existing tests
are preserved, and temporary diagnostic instrumentation was removed.

Final checks pass 1,782 library tests, 16 Fraction, 30 Numbers and 51 Statistics
contracts. Ranking's complete five-source closure compiles and matches 207 exact
JVM behavior rows. Registry/parser and numeric-runtime checks are reused with
identical frozen input proofs. Complete Descriptive includes all 80 pinned
production/dependency files: 0 parse failures, 0 translation failures, four
invalid-pattern files and **68 compiler observations**, compared with 46.

The original six build/build_with_outer allocation boundaries and unresolved Sum
constructor are closed. Propagating the static-array requirements exposes 29
subsequent RangeFunction method-reference boundaries, so the total has increased.
All 29 raw boundaries occur in generated source-SAM adapters. Method-reference
value-domain analysis currently covers native SAM descriptors; the next batch
must generalize that mechanism to parsed source SAMs, preserving symbolic generic
input/result projections and inherited specialization. Compiler observations are
not independent bug counts; no Statistics module is accepted.

Frozen producer: `.tmp/new-java2rust-statistics-v463-callables-r3-release`, SHA-256
`7a479875457b889861142d8b26dbe6fcddc285f9e489239f6ae0374327267717`.
Complete frozen provenance is `.tmp/statistics-v463-final-batch-evidence.json`;
rewrite/context audit and next batch are
`.tmp/statistics-v463-audit/implementation-plan.md`. Production body witnesses and
the downstream adapter inventory are in the same audit directory. The final
source/test/pipeline manifests cover 117/1,165/9 inputs. Complete Descriptive took
184,983ms alongside contract verification; this is not a controlled speed result.
Remaining source-interface capability/erased views, anonymous ancestor dispatch,
Send placement, collection indexing and receiver issues still require their
recorded whole-path audits. Ranking acceptance, Distribution/Interval/Inference
complete closures and broader preservation/final requirement checks remain.

## V464 — parsed source-SAM allocation and nullable ABI contracts

Parsed source SAM method references now reuse the existing invocation value graph
for argument/result allocation placement. The resolver exposes symbolic generic
projections from the existing normalized SAM declaration, preserving independent
binders before substitution and inherited reordering. Native descriptors remain
authoritative for native interfaces. Invocation inputs/results and constructor
reference inputs use the same projections. Fixed source SAMs use their ordinary
callable nodes. Root nullability stays on the source callable ABI, preventing a
nullable formal Option<A> from also instantiating A as Option<Payload>.
Child interfaces inherit Send requirements through the existing instantiated
ancestor inventory, preserving the generated owning ancestor adapter contract.

The maintained independent JVM/generated caller covers stored multiargument
references, global/local array aliases and mutation, inherited SAMs, fixed array
signatures, source-SAM constructors retaining array inputs, invalid ranges and
null-input exceptions. Focused regressions also check nested symbolic projections
and separate sync/local domains when two type arguments are both int[]. Existing
Statistics test text is preserved exactly. Final checks pass 1,785 library tests,
16 Fraction, 30 Numbers and 52 Statistics contracts. Ranking's complete five-file
closure compiles and matches 207 exact JVM behavior rows. Registry/parser and
numeric-runtime results are reused under unchanged frozen input proofs.

Complete Descriptive includes all 80 pinned production/dependency sources, with
0 parse failures, 0 translation failures, four invalid-pattern files and **68
compiler observations**, unchanged from V463. The 29 production callback storage
boundaries remain; these focused contracts are not module acceptance.

A complete-context diagnostic over those same 80 staged sources proves the new
production flows exist: typed RangeFunction fields connect to the correctly
selected Min/Max/SumOfLogs/etc source parameters. The remaining gap is subsequent:
canonical source generic allocation qualifiers are built/projected AFTER the
signature constraint fixpoint. This changes rendered callback arguments without
propagating the strengthened requirement through their storage edges. The next
batch must integrate canonical owner/binder requirements into the existing
monotone solver, reusing current binder projection and ancestry rules. Rebuilding
full signature plans repeatedly or adding local Arc/Rc conversions would leave
the dependency model unresolved. Concrete evidence and the proposed solver batch
are `.tmp/statistics-v464-audit/next-generic-solver-plan.md`; the later anonymous
ancestor producer audit is in `next-producer-plan.md` in that directory.

Frozen producer: `.tmp/new-java2rust-statistics-v464-source-sam-r4-release`, SHA-256
`6641d4c1e7cf01af640c492f7bd92faf5fe1866ad72f305f7c9df3cb8491377a`.
Final provenance: `.tmp/statistics-v464-final-batch-evidence.json`, with source/test/
pipeline manifests covering 117/1,166/9 inputs. Complete Descriptive took
136,151ms; this concurrent run is not a controlled speed measurement. Temporary
full-context diagnostic instrumentation was removed and every frozen input hash
was restored exactly. No Statistics module is accepted; the full all-module goal
remains active, including broader preservation and independent acceptance gates.

## V465 — canonical generic qualifiers participate in saturation

Canonical source generic allocation requirements now participate in the existing
signature dependency graph and monotone solver. A cached occurrence/use index
joins each source-owner/binder's symbolic declaration slots, concrete native
instantiations and inherited substitutions. It uses one declaration anchor per
binder, avoiding quadratic all-pairs links. Concrete-use requirements join back
into the canonical binder, keeping returned aliases on the same allocation as
inputs. Native eligibility applies on the concrete side; nullability, ownership
and retention keep their existing mechanisms.

The index and final allocation summary reuse owner identities, ambiguity-aware
aliases, ancestry substitution and the shared `types::type_parameter_paths`
traversal. Source SAM analysis uses that same traversal, including shadowing and
inherited reordering. No complete SignaturePlan is rebuilt inside an extra outer
fixpoint. Final generic rendering still uses the canonical summary, now backed by
constraints propagated to selected source reference parameters and results.

A failing-before/passing-after regression reproduces the actual late-projection
route through a nested builder and generic factory helper. The independent
JVM/generated caller checks repeated global and empty-array identity, returned
input aliases, local mutation, range validation and null exceptions. The index
regression checks linear edge growth and non-native eligibility. Existing tests
are preserved exactly. Final checks pass 1,787 library tests, 16 Fraction,
30 Numbers and 53 Statistics contracts. Ranking's full five-source closure
compiles and matches 207 exact independent JVM rows. Registry/parser and numeric
runtime checks are reused under unchanged frozen input proofs.

Complete Descriptive covers all 80 pinned production/dependency sources, with
0 parse failures, 0 translation failures, four invalid-pattern files and **39
compiler observations**, down from 68. All 29 targeted RangeFunction callback
allocation boundaries are absent from both raw production files and diagnostics;
no new diagnostic message groups appear. Complete provenance is
`.tmp/statistics-v465-final-batch-evidence.json`; exact differences and raw body
hashes are in `.tmp/statistics-v465-rewrite-audit.json` and the V465 audit directory.
Frozen producer: `.tmp/new-java2rust-statistics-v465-generic-r7-release`, SHA-256
`4760df4334ef3674124533b9fbe3e3c48aeebd5803623f351f9607b14f8bb60f`. Final source/test/pipeline manifests cover
117/1,167/9 inputs. Complete Descriptive took 145,521ms; signature saturation was
856ms, with no new repeated plan-construction loop. Concurrent runs do not prove
a controlled speed improvement.

A separate mixed array/String generic probe remains unsupported: V464 has three
compiler observations and V465 two, closing its array boundary while retaining
the existing non-native String allocation-metadata problem. Its exact producers
and diagnostics are `.tmp/statistics-v465-audit/string-mixed-evidence.json`;
this diagnostic is not an accepted corpus or passing behavioral test. Preserve
it when auditing physical eligibility for generic qualifiers.

Remaining production families include fresh anonymous ancestor construction,
source-interface capability/erased views, native consumer representation, Send
implementor placement, Quantile checked scalar fills and JavaClass capability
calls. Consumer composition currently loses the array root and callable element
representation; fix its producers/storage/signatures rather than adding array
operations to StatisticResult. Prior concrete interface producer audits remain
applicable and must be checked against this final output. No Statistics module
is accepted; Ranking acceptance, other complete module closures, broader
preservation and final independent acceptance/requirement audits remain.

## V466 — interface destinations, native consumers and retained identity

Complete Descriptive is down from **39 to 21 compiler observations**, with all
80 pinned production/dependency sources, zero parse/translation failures and two
invalid-pattern files (previously four). Constructor and method argument
propagation now share exact binding/expanded-varargs rules, preserving the native
consumer array root and callable elements. Native facade declarations retain
that ABI instead of acquiring source-interface erasure from incidental uses.

Fresh anonymous objects are constructed in their proven destination ancestor
view, retaining state, identity, generic substitutions and default overrides.
Scalar String/source generic arguments ignore metadata that does not describe a
physical outer carrier; concrete native-to-symbolic generic boundaries retain
their instantiated allocation qualifier. Nullable source constructors wrap the
shared payload inside Some using the existing idempotent construction policy.

Source interfaces now expose the existing retained identity protocol through an
object-safe operation, forwarded through boxes, ancestors and generic retargets.
Named/shared/anonymous adapters use their canonical allocations. Native views
preserve source identity and composition overrides; inherited fixed composition
calls share a nullable/synchronized result plan, preserve Java argument/null
ordering and propagate exceptions. Synchronized native ancestry contributes
source-interface capability requirements without imposing Sync on ordinary
static-only interfaces. The remaining anonymous mutable capability gap is
recorded below; this batch does not prove that every callback representation is
supported.

The assembly pipeline preserves valid explicit dyn-trait capability impls while
rejecting bare struct impls after a declaration becomes a trait. Reference,
lifetime/auto-trait and transparent wrapper owner projection reuse
scripts/rust_item_utils.py. Seven structural merge/compile controls and ten
registry controls pass. Final contracts pass 16 Fraction, 30 Numbers and 56
Statistics checks, including the three new independent JVM/generated fixtures.
All 1,791 library tests passed on the frozen R11 Rust/test inputs; R13 changes
only pipeline files and reuses that library section under identical input
hashes. Its original combined invocation failed later in the Statistics suite;
R13 reruns all three contract suites successfully. Numeric runtime19 is reused
under unchanged runtime hashes.

Frozen producer: `.tmp/new-java2rust-statistics-v466-interfaces-r11-release`,
SHA-256 `f272090b4e9541c45eeb229989a96b7df7800fc2a4961130cbe60099d8185f51`.
R13 source/test/pipeline manifests cover 117/1,170/9 inputs. Pipeline-only
corrections replay verified complete raw translations through production
assembly, avoiding another Rust rebuild and translation run; original producer,
source scope and every raw hash are checked in the reuse certificate. Ranking's
five-source closure compiles and still matches 207 exact independent JVM rows.
Full R11 Descriptive took 187,891ms; R13 assembly replay took 86,563ms and reused
its translation. These concurrent measurements do not establish a controlled
translator speed improvement.

Current provenance is `.tmp/statistics-v466-final-batch-evidence.json`, with
frontier differences and producer/body witnesses in the V466 audit directory.
The next related batch is typed source-SAM casts/escaping bound references and
source-interface capability views. The production typed casts currently become
erased conversions instead of zero-argument owned adapters. A separate valid
Java anonymous mutable native-view probe returns0 on the JVM but has two
remaining generated errors: implementation parameter binding and the raw
Cell state's Sync requirement. Its rewrite audit calls for distinguishing a
retained object's capability view from raw payload capability, preserving
reentrancy and the existing shared receiver machinery. This is an outstanding
diagnostic, not an accepted contract. Remaining Descriptive also includes static
interface implementor placement/forwarders, checked Quantile fills, a mutable
optional receiver and JavaClass assignability. No Statistics module is accepted;
Ranking acceptance, the other full module closures and final preservation remain.

## V467 — typed source callback producers and reference result presence

Complete Descriptive improves from **21 to 14 compiler observations** across all
80 production/dependency sources, with zero parse/translation failures and one
invalid-pattern file (previously two). Seven explicit source-SAM cast failures
are removed. A typed lambda or method reference now constructs its declared
source interface before the existing destination/ancestor coercion. It reuses
normalized SAM selection, callable signatures, generic ancestry, default bodies,
exception ABI and retained capture conversion. Callback identity comes from its
own existing callable allocation, independently of the captured receiver.

Source callback inputs/results join ordinary callable storage roots. Directional
presence flows also cover scalar source references, while native allocation and
retention propagation retain their eligibility guards. Qualified nested source
owners reuse the declaration catalog so writes through computed generic returns
select the existing mutable shared projection. Optional clone shortcuts respect
dispatch/view changes. Borrowed optional-local comparisons use the physical slot
type without introducing an extra borrow; array-element places keep their own
projection path.

The new independent JVM/generated caller covers primitive defaults, bound and
lambda captures, receiver evaluation once, alias identity, mutation after capture,
creation-time versus deferred null failures, exception recovery, generic nullable
reference results/defaults, computed-result mutation, and nullable enum comparisons
in both operand orders. All earlier Statistics fixture text/assertions remain.
A Fraction Rust caller adapts its source constructor's newly inferred optional
denominator with Some and adds a None-storage assertion; every prior arithmetic,
identity and state assertion remains, and pinned Java/provider ABI is unchanged.

Final verification passes **1,791 unit, 16 Fraction, 30 Numbers and 57 Statistics
contracts**. Ranking's complete five-source closure compiles and matches 207 exact
independent JVM rows. Seven assembly and ten registry controls and 19 runtime
checks are reused under verified unchanged pipeline/runtime inputs. Source/test/
pipeline manifests contain 117/1,171/9 inputs. Failed intermediate checks, including
one ENOSPC compilation before tests, remain recorded. No performance improvement
is claimed from concurrent timings.

Frozen producer: `.tmp/new-java2rust-statistics-v467-source-sam-r13-release`,
SHA-256 `2b948b75f5ac2ea10a0a58aeda540578a14dae1e841bddb832c23e2b366890b2`.
Current provenance: `.tmp/statistics-v467-final-batch-evidence.json`; rewrite and
next-batch plans are in `.tmp/statistics-v467-audit/`. The next six related errors
are static-root source-implementor placement and owned array-handle forwarding.
The remaining eight observations are genuine source capability instanceof/cast
(two), checked Quantile fills (four), an optional mutable receiver (one), and
JavaClass assignability (one). The previously documented anonymous mutable Sync
facet redesign also remains outstanding. **No Statistics module is accepted.**
Ranking coverage/acceptance, other complete module closures, broader preservation
and final requirement audits remain within the active all-module goal.

## V468 — static interface storage and concrete forwarding contracts

Complete Descriptive improves from **14 to 8 compiler observations** across all
80 pinned production/dependency sources, with zero parse/translation failures
and one invalid-pattern file. The six related NaNTransformer implementation and
array-forwarding failures are removed through shared translator mechanisms.

Static storage reachability now keeps source interfaces as intermediate graph
vertices and follows inherited interface implementations into retained concrete
objects and their fields. It reuses the native callback implementor-edge walk
and the existing worklist closure, projecting results to the requested inventory.
Local-only objects remain outside static reachability. Ordinary trait forwarders
now reuse the same representation-aware emitted parameter projection as the
source-bound path before normal argument conversion. This preserves shared array
handles when concrete method emission requires ownership after solving.

The independent StaticInterfaceStorageAudit caller covers a static factory result
retained through an interface, inherited and direct immutable implementors, array
mutation, copy versus alias policies, repeated calls, and exceptions/recovery.
The frozen prior producer fails the identical fixture with eight compiler errors;
the updated translator passes independent JVM/generated behavior. A storage
regression checks implementor/payload reachability and a local-only control.
All previous unit/contract fixtures and assertions remain unchanged.

Verification passes **1,792 unit tests, 16 Fraction, 30 Numbers and 58 Statistics
contracts**. Ranking's complete five-source closure compiles and matches 207 exact
JVM rows. Unchanged pipeline/runtime input hashes justify reusing the seven
assembly, ten registry and 19 runtime controls. The focused check preceded the
broader batch boundary; frozen source/test/pipeline manifests cover 117/1,172/9
inputs. No pinned Java changes or generated-code repairs are deliverables.

Frozen producer: `.tmp/new-java2rust-statistics-v468-implementors-r1-release`,
SHA-256 `be8d83705d0df4bdc2ebf8338cb17491dcf7fe46da0a3f14512ba942a5e30d8f`.
Provenance: `.tmp/statistics-v468-final-batch-evidence.json`; rewrite and next
checked-argument audits are in `.tmp/statistics-v468-audit/`. Remaining Descriptive:
four checked Quantile fill observations, one optional mutable bound receiver,
two genuine runtime source capability projections, and JavaClass assignability.
The separate anonymous mutable Sync-facet architecture gap is still outstanding.
**No Statistics module is accepted.** Ranking acceptance/coverage, all later full
module closures, broader preservation and the final requirement audit remain in
the active all-module goal.

## V469 — checked fill operands, live rows and callback borrow timing

Complete Descriptive improves from **8 to 4 compiler observations** across all
80 pinned production/dependency sources, with zero parse/translation failures
and one invalid-pattern file. The four checked Quantile fill mismatches disappear.
Fill values now use their selected element Argument slot, so the existing
fallible conditional plan consumes its Result at the call boundary.

For nonconstant/effectful operands, native fill evaluates the Java target
reference, range bounds and value before checking invocation null/range conditions
and borrowing the payload. Shared array references reuse canonical allocation
retention and the ordinary ordered statement/receiver plans. The constant path
remains direct. Nullable plain carriers defer payload projection as well.
Indexed range temporaries have distinct names, preserving both endpoints.

The same audit identified two row boundaries. Nested array mutation obtains its
retained row handle through the existing indexed-read shape instead of applying
fill to an Option. Dense row mutation stages the parent/index, reuses the checked
row validator before later arguments, and acquires row_mut for the final call.
It neither snapshots rows nor holds a matrix guard across callback evaluation.
The validator is shared with the existing row-read path.

CheckedFillAudit independently runs on the JVM and through production batch
assembly plus a separate Rust caller, reusing existing ndarray support. It checks
exact argument order, reentrant destination mutation, range writes, callback
exceptions before null/range validation, empty branches/arrays, null callbacks on
unselected branches, same-matrix callback mutation and invalid-row-before-callback
ordering. The prior frozen translator fails the identical fixture with four
compiler errors. Two existing matrix helper controls had stale ABI assertions;
frozen prior outputs prove the shared layouts predate this batch. Those controls
now compile and execute runtime assertions for live nested rows and all dense
rows, preserving their original mutation purpose. No Java fixture is weakened.

Final checks pass **1,792 unit tests, 16 Fraction, 30 Numbers and 59 Statistics
contracts**, plus both matrix controls. Ranking's complete five-source closure
compiles and matches 207 exact JVM rows. Unchanged pipeline/runtime input hashes
justify reuse of the seven assembly, ten registry and 19 runtime controls. Frozen
source/test/pipeline manifests cover 117/1,173/9 inputs. Matrix-control reuse is
explicitly proven against identical production and matrix-test inputs; the final
fixture alone adds the production-assembly caller. Intermediate failures and
baseline outputs remain recorded in the V469 audit directory.

Frozen producer: `.tmp/new-java2rust-statistics-v469-fill-r5-release`,
SHA-256 `c6115035c79cf66f0e358d569022ad5229e1c68179bf1d25fad03233dce77ac5`.
Provenance: `.tmp/statistics-v469-final-batch-evidence.json`. Remaining Descriptive
observations are the optional mutable bound receiver, genuine source-interface
capability instanceof/cast (two), and JavaClass assignability. The separate
anonymous mutable Sync-facet architecture gap is still outstanding. **No
Statistics module is accepted.** Ranking acceptance/coverage, the other complete
module closures, broader preservation and the final requirement audit remain in
the active all-module goal.

## V470 — selected mutable receiver contracts and generic reborrows

Complete Descriptive improves from **4 to 3 compiler observations** across all
80 pinned production/dependency sources, with zero parse/translation failures
and one invalid-pattern file. Statistics.combine now invokes its selected mutable
interface method through the correct nullable receiver projection.

A public diagnostic consumer proves the receiver has an intersection bound, the
selected declaration is the accumulator interface and its solved receiver plan
is mutable. The old single-bound shortcut was not the production path: ordinary
adaptation lost that selected ABI while attempting fallback lookup through T.
Receiver lowering now consumes the same exact source-signature lookup already
used for argument planning. Physical optional borrowed references join the existing
ordered invocation protocol: mutable carriers are reborrowed, shared references
are copied, and argument effects precede receiver checking. Existing retained
owned/interface/native-facade snapshots keep their established priority.

The single-bound result path delegates to ordinary call lowering. Equality's
non-recursive operator bridge remains and shares the selected receiver/projection
and staging protocol. No T payload is copied merely to obtain mutation capability.
The new JVM/generated caller exercises repeated intersection-bound merge/update,
read-only calls after mutation, retained aliases, a plain unique payload, null
refinements, argument exception/null ordering, single-bound results, receiver
exceptions and recovery. The frozen prior producer has seven mutable-borrow errors
on the identical fixture. Existing fixtures and assertions remain unchanged.

Checks pass **1,792 unit tests, 16 Fraction, 30 Numbers and 60 Statistics contracts**,
both matrix-row controls, and three targeted bound/enum/object-equality controls.
Ranking's complete five-source closure compiles and matches 207 exact JVM rows.
Unchanged pipeline/runtime inputs justify reuse of seven assembly, ten registry
and 19 runtime checks. Frozen source/test/pipeline manifests cover 117/1,174/9
inputs; current-state hashes, original/staged pinned sources, diagnostic differences
and emitted-body witnesses are verified in the batch evidence.

Frozen producer: `.tmp/new-java2rust-statistics-v470-nullable-receiver-r2-release`,
SHA-256 `db7c4fe998b89b1a81ffe9005bab2c463e42a166128a8b73758f0716bfaf6655`.
Provenance: `.tmp/statistics-v470-final-batch-evidence.json`. The remaining three
observations concern genuine interface capability instanceof/cast and JavaClass
assignability. The next runtime-capabilities audit traces retained owned views,
anonymous/generic producers, borrowed consumers and class ancestry metadata;
these need a capability/metadata contract, not type-name equality. The separate
anonymous mutable Sync-facet gap is still outstanding. **No Statistics module is
accepted.** Ranking acceptance/coverage, the other complete module closures,
broader preservation and final requirement audits remain in the active full goal.

## V471 — runtime class metadata and class-token call contracts

The complete 80-source Descriptive closure has **two compiler observations**,
down from three: both are source-interface capability instanceof/cast boundaries.
There are no parse or translation failures; one invalid-pattern file remains.
This is compilation progress, not module acceptance or proof that every runtime
class comparison is implemented.

Class tokens now canonicalize binary names and array descriptors and expose
assignability based on existing nominal hierarchy facts. Structural rules cover
primitive identity and array covariance/Object/Cloneable/Serializable. A visited
worklist avoids repeated recursive expansion of transitive nominal ancestry.
Incomplete native metadata remains explicit unsupported behavior rather than an
invented false result. Conditional subtype returns request the existing hierarchy
carrier, preserving runtime getClass identity. Source parameter emission, body
inference and calls share the owned immutable token ABI; erased and nullable
Class arguments retain destination marker/nullability. Batch assembly uses an
unconditional token Clone implementation without requiring its marker to Clone.

The maintained JVM/generated batch oracle covers inheritance, interfaces,
subtype-return identity, nested binary names, primitive/array comparisons, null
arguments and receiver-null argument evaluation order. Its exact previous-producer
baseline had 30 compiler observations. Current checks pass **1,792 unit tests,
16 Fraction, five lossless-text, 30 Numbers and 61 Statistics contracts**,
plus seven assembly and ten registry controls. Ranking's complete five-source
closure compiles and matches 207 exact JVM rows. Source/test/pipeline manifests
cover 119/1,175/9 frozen inputs and verify pinned original/staged Java unchanged.

An additional class-token control passes nullable storage but fails bounded
reflective enum construction with three compiler observations. The previous
frozen producer reproduces the same three failures: PhantomData propagates Rc
Send requirements into enum static state, and a generated accessor uses handler
instead of handler_. This existing gap is recorded, not counted as a pass.

The production rewrite audit also identified an outstanding runtime case:
FirstMoment inherits native DoubleConsumer. Negative class comparisons through
that boundary still encounter incomplete native metadata. The next metadata
batch must reuse the existing native functional ancestry model, add independent
behavior coverage, and establish correct negative answers before accepting this
operation. Source-interface capability views remain a separate architectural
batch, including retained identity and escaping bound references.

Frozen producer: `.tmp/new-java2rust-statistics-v471-class-assignability-r2-release`,
SHA-256 `32d27e932681678f3125b8977eadd4e23f6ee62f48674316c0589867b7064481`.
Provenance: `.tmp/statistics-v471-final-batch-evidence.json`; rewrite/runtime-gap
plan: `.tmp/statistics-v471-audit/implementation-plan.md`. **No Statistics module
is accepted.** The full-module goal remains active.

## V472 — complete modeled native functional ancestry

Negative class queries through native functional interfaces now use complete
known ancestry from the existing SAM model and native parent-edge catalog.
This closes the runtime gap found in FirstMoment's DoubleConsumer boundary.
Source hierarchy edges participate in native dependency discovery, and are
collected once for both boundary discovery and metadata emission. The native
inventory closes over UnaryOperator/Function and BinaryOperator/BiFunction.
Unmodeled native families retain explicit unsupported behavior; this does not
claim universal native reflection support.

A new maintained two-file JVM/generated oracle covers source parent/child/sibling
comparisons through native DoubleConsumer, native positive/negative comparisons,
operator inheritance direction, arrays, binary names and getClass. The exact
frozen V471 baseline compiles it but exits with UnsupportedOperationException on
a negative source-class comparison. Current generated code passes. The original
class-assignability oracle and every existing fixture/assertion are unchanged.

Checks pass **1,792 unit tests, 16 Fraction, five lossless-text, 30 Numbers and
62 Statistics contracts**. Ranking's complete five-source closure compiles and
matches 207 exact JVM rows. Unchanged pipeline/runtime hashes justify reuse of
seven assembly, ten registry and 19 numeric-runtime checks. The complete
80-source Descriptive closure still has two capability compiler observations,
zero parse/translation failures and one invalid-pattern file. Source/test/pipeline
manifests cover 119/1,177/9 frozen inputs and verify pinned originals/staged Java.

The next capability batch has concrete rewrite evidence. A **diagnostic-only**
scratch copy of the generated Descriptive crate replaces its two unsupported
capability expressions with retained views over the existing 12 named shared
implementors. It compiles, and independent Java/Rust callers agree that a Mean
result observes later mutation and survives release of the original handle.
The generated Statistics.checkCombineAssignable method and moment producers
remain unchanged in that diagnostic crate; all 25 null/root/second/third/fourth
moment pairs match Java's success and exception-type observations. These are
contextual production-path experiments, never manually repaired deliverables or
module acceptance. The maintained general metadata callers run unpatched.

A finite registry cannot support arbitrary anonymous/generic implementors.
Three additional feasibility callers cover retained named/anonymous aliases,
null/failed casts, generic state/wildcard membership and native functional
ancestry. All pass on the JVM; translation exposes 25 compiler observations
with shared roots: missing capability operations, generic callback binders,
fresh wildcard caller specialization, native facet support discovery, and
anonymous native parameter/state lowering. The architecture audit traces owned
views through producers, storage, signatures and escaping consumers and selects
shared prerequisite batches. It reuses named owning-handle method lowering and
generic retarget adapters; it does not count unsupported cases as false casts.
The previously measured bounded reflective enum construction gap remains open.

Frozen producer: `.tmp/new-java2rust-statistics-v472-native-class-ancestry-r2-release`,
SHA-256 `48babe67ed69a6c4664bff7438d98df12e53e86605a98f3f1852d6d1051fc8b8`.
Provenance: `.tmp/statistics-v472-final-batch-evidence.json`; implementation audit:
`.tmp/statistics-v472-audit/implementation-plan.md`; next architectural batch:
`.tmp/statistics-v472-audit/interface-capability-protocol.md`. **No Statistics
module is accepted.** All production modules/dependencies remain in the active goal.

## V473 work paused

The user paused the goal during the interface-capability prerequisite batch.
The latest constructor replay/liveness edit is formatted but **not tested**.
The last focused run still failed on one unnecessary generic constructor clone;
no V473 preservation or complete-corpus result is claimed.
Resume details, exact artifacts, current input snapshots and next commands are
in [the paused handover](commons-statistics-handover.txt). V472 remains the last
fully verified batch; the full production-module objective is unchanged.

## Reproduction

```sh
export CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true
export TMPDIR="$PWD/.tmp/fraction-arrays-test-tmp"
export NEW_JAVA2RUST_BIN="$PWD/.tmp/new-java2rust-statistics-v465-generic-r7-release"
export NEW_JAVA2RUST_BUILD_PROFILE=release
bash scripts/prepare-commons-statistics-1.3.sh
python3 scripts/commons-statistics-1.3-corpus.py \
  --inventory .tmp/statistics-scope-reproduce
STATISTICS_MODULE=ranking \
STATISTICS_TRANSLATION_ROOT="$PWD/.tmp/statistics-ranking-reproduce" \
  bash scripts/run-commons-statistics-1.3.sh --skip-resolver-bench
```

Use fresh staging/output roots. Baseline Ranking initially used the pinned
source directory directly; the maintained runner now stages the same complete
five-file closure with explicit dependency/source manifests.

After translating the complete Ranking core, reproduce its independent behavior
comparison with a fresh evidence directory:

```sh
python3 scripts/verify-commons-statistics-ranking.py \
  --translation .tmp/commons-statistics-1.3-ranking-v465-r7-core \
  --out .tmp/statistics-ranking-core-behavior-reproduce
```
