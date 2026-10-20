# JGraphT generated-rewrite feasibility audit

## Evidence and limits

This audit inspects the pinned, untouched 104-file slice and frozen v125 output
at `.tmp/jgrapht-core-1.5.3-focused-v125-null-checked-borrows`.
Its summary records producer SHA-256
`afa2ce12ac663036c6e4a157646fa1e05135604719f14eedec8660559cdebdd3`.
All 104 files generate, but the assembled crate has 1,539 diagnostics and is
**not accepted**. The corpus sources and generated output were not modified.

The examples below identify necessary rewrite shapes, not proven minimal
whole-crate patches. Six standalone Rust mechanism experiments pass in
`.tmp/jgrapht-rewrite-audit-probe.rs`; these exercise plausible replacement
contracts without the failing crate's unrelated dependencies. A deliberately
invalid borrowed-self callback in
`.tmp/jgrapht-rewrite-audit-borrowed-callback.rs` fails with E0521 as expected.
Neither experiment is translator regression coverage, a Java oracle, or proof
that an integrated rewrite is sufficient. No corpus build was repeated.

## Representative rewrites

### 1. Retained map operations and live key views

Observed in `00056_BaseIntrusiveEdgesSpecifics.rs`:

- `edge_map` is `Option<Rc<RefCell<JavaSemanticHashMap<E, Option<IE>>>>>`.
- `contains_edge` calls `contains_key` directly on the Rc.
- `remove` similarly calls `remove` on the handle.
- `get_edge_set` materializes `keys().cloned().collect()` and caches the result.

The first two need short-lived `borrow()`/`borrow_mut()` projections after the
Java null check. The last is **not** fixed by adding those projections: Java
uses `Collections.unmodifiableSet(edgeMap.keySet())`, a cached live view, not a
snapshot. Its returned/cached carrier and consumers must support backing-map
retention, live membership/size/iteration, and rejection of writes. Existing
iterators still need their Java invalidation rules; a live view does not imply
a snapshot iterator or permission to change iteration semantics.

Map child reads that survive the map loan must clone the child's Java reference
handle inside the borrow, not clone its object payload. For example:

```rust,ignore
let child = { map.borrow().get(&key).cloned() };
// The map loan ends here; subsequent map mutation and child access are legal.
```

This is valid only if the stored child carrier's Clone means reference copy.
It is not permission to add `IE: Clone` and copy arbitrary payloads.

Scope: operation projection is local, but views and child results are a
cross-function representation/signature/conversion contract. Constructors,
fields, aliases, returns and interface adapters must agree on the same carrier.
In `lower.rs`, `expr_is_shared_native_map` currently identifies the older
tracked Arc/RwLock map path; `adapt_call_target_receiver` emits read/write guards
for it. Canonical Rc/RefCell maps must use the existing
`shared_storage_receiver_projection` policy, not be relabelled as legacy maps.
Tracked live-entry behavior cannot silently be replaced by a plain map.

Experiment: a retained child survives removal, child writes remain visible to
an alias, a key view sees subsequent removal/insertion, and view writes fail.
Full iterator/modification-count behavior remains to be established.

### 2. Generic edge fields and Java reference identity

Java `BaseIntrusiveEdgesSpecifics.addIntrusiveEdge` reads and writes
`e.source`/`e.target`, tests null, compares endpoints by reference identity, then
stores the same edge in the map. Generated code instead accepts
`Option<&mut IE>`, emits null-comparison placeholders, accesses fields directly
on the type parameter, and clones IE for insertion.

The generated `IntrusiveEdgeBound` in shared support exposes only Java clone,
not field capabilities. A generic Rust type cannot support `.source` field
syntax just because its bound represents a Java superclass. Existing
`IntrusiveEdgeValue` source/target accessors demonstrate a reusable dispatch
mechanism, but currently return `Option<Box<dyn Any>>` with invalid clone
requirements; blindly calling them is not a complete solution.

Required shape: solved generic-bound field getter/setter capabilities, lowered
field access through those capabilities, and identity-preserving nullable
erased-reference carriers for Object-valued endpoints. Edge insertion must
retain the original reference. The accessor result metadata must preserve
nullability and dispatch as well as the generic substitution. Existing source
hierarchy dispatch/accessor generation should be generalized, not duplicated.

GraphMetrics independently exposes the same reference-identity gap: Java
`v1 == v2` (source line 326) becomes `v1.as_ref() == v2.as_ref()` (generated
line 182). Adding PartialEq would choose value equality, not Java reference
identity. Null-aware handle identity must work for generic V, including boxed
and erased cases. JavaMapKey equality is a separate equals/hashCode protocol,
not a substitute for `==`. Identity hash used by the comparator must agree with
the same object identity.

Scope: representation/framework work spanning generic slots, erasure, bound
capabilities, source hierarchy adapters, comparison lowering and map children.
It is not an expression-only patch or an extra Clone/PartialEq bound.

Experiment: generic field getters/setters and nullable Rc identity work for a
vertex payload with neither Clone nor PartialEq; two equal-valued distinct
allocations remain distinct. This does not yet validate Java boxed identity,
runtime casts, identityHashCode, or all closed-enum carrier combinations.

### 3. Deferred callbacks capturing the Java receiver

Java `GraphIterables.edges` constructs a `LiveIterableWrapper` with
`() -> getGraph().edgeSet()`. Generated `edges(&self)` passes
`move || self.get_graph()...` to `Supplier::from_fallible_fn`, whose callback
requires `'static`. `move` moves a borrowed reference, not its owner.

Required shape: retain the receiver's Java object handle and invoke getGraph
when the callback runs. Clone the receiver handle at construction, release
construction-time loans, acquire short loans at invocation, and return a live
collection view. Capturing the result of getGraph eagerly is not generally
equivalent: an override can have effects or return a different graph later.
Snapshotting edgeSet is also wrong.

Existing `_with_outer` companions already carry explicit owner handles. The
missing contract is how source interface/default dispatch reaches a retained
receiver entry point without retaining an active Ref/RefMut guard. This can
change interface receiver/bridge signatures, not merely closure text. Do not
add Rust lifetimes throughout Java object APIs to preserve an accidental borrow.

Experiment: a callback retaining an Rc observes later mutation and survives the
caller's handle being dropped; the original borrowed-receiver shape fails
E0521 in an isolated negative control. The corpus compiler currently reports
other errors in these methods; this lifetime issue is established by inspecting
the generated contract and the negative control, not by attributing its existing
E0521 diagnostic (which actually belongs to DoublyLinkedList) to GraphIterables.

### 4. Constructor Result through interface/view conversion

The same GraphIterables methods have a separate local contract failure:

```rust,ignore
Ok({
    let object = Rc::new(RefCell::new(LiveIterableWrapper::new_supplier(...)?));
    Iterable::from_fallible_fn(/* callback using object */)
}?)
```

The constructor Result is already discharged. The final Iterable is not Try;
remove the second propagation semantically by preserving the discharged-effect
metadata through the adapter. Keep the inner `?` and the method's Result.
`lower_native_functional_object_for_slot` builds an adapted block but does not
carry this metadata as the existing storage wrappers do.

Scope: local effect-result projection across adapters, with nested and nullable
paths included. It does not fix the deferred callback's receiver lifetime.
Experiment: constructor failure propagates once; successful construction makes
an infallible facade; construction is evaluated once per attempt.

### 5. Generic return ABI and trait bridges

In `00080_AbstractGraphBuilder.rs`, `build` and its trait use `Option<&G>`.
In `00081_GraphBuilder.rs`, the concrete and `_with_outer` methods return
`Option<G>`. Its generated trait bridge calls the owned-returning helper and
then `.as_ref()`, producing E0515 at line 182.

A temporary binding cannot make the returned reference outlive the bridge.
Required shape: one canonical inherited/specialized return contract across
base, subclass, trait and bridge. Where the graph escapes, return a copied Java
reference handle; do not clone the graph object. A borrowed return is an
optimization only when the whole ABI and lifetime proof permit it.

Scope: cross-function signature solving and bridge adaptation, linked to the
reference-carrier work in item 2. Experiment: a bridge returns an owned handle,
the builder can be dropped, and graph mutation remains visible through an alias.

### 6. Native collection superclass storage and qualified outer this

`ArrayUnenforcedSet` extends ArrayList and implements Set. Generated struct
contains mod_count and PhantomData but no element storage. Collection/capacity
constructors emit unsupported-super placeholders. The nested SetForEquality
helper's `ArrayUnenforcedSet.this.iterator()/size()` becomes unresolved This
calls; its inherited AbstractSet equals/hashCode are absent.

Required shape: an actual ArrayList superclass storage component, forwarding
of inherited operations and super constructor arguments, and qualified-this
resolution to the existing captured outer handle. AbstractSet algorithms must
use the helper's overridden iterator/size. Set membership type does not license
deduplication: this class deliberately relies on callers to enforce uniqueness.
Replacing it with a native HashSet changes behavior.

Scope: framework support for native-superclass composition, construction and
inherited dispatch, plus qualified outer-this resolution and real collection
object protocols. Existing retained native-list storage and source inheritance
machinery should supply the pieces. Experiment only proves the proposed list
backing preserves duplicates and negative-capacity failure; equality, hashCode,
iterator invalidation and outer dispatch still require translator/oracle tests.

## Replanned execution order

The former recommendation to fix map projections plus Result tags alone is
superseded. That would improve compilation but leave live-view and identity
semantics unresolved. The practical next architectural unit is a **reference
boundary contract batch**, not another diagnostic-count reduction campaign:

1. Establish canonical reference-copy, null and identity operations for generic,
   erased and hierarchy-backed slots using existing carrier machinery. Include
   generic-bound field projections and inherited return/bridge contracts.
   First inspect the available erased carrier before selecting any new runtime
   abstraction. No unrelated object-model rewrite is presumed necessary.
2. Carry that contract through retained-map producers, child reads/writes and
   live unmodifiable key views. Distinguish canonical cell maps from legacy
   tracked-map backends; end container loans before invoking source callbacks.
3. Retained receiver dispatch for escaping/default-method callbacks, together
   with once-only Result projection through facade adapters. Validate callback
   invocation timing, post-construction mutation and reentrant loan behavior.
4. Native-superclass storage/dispatch and qualified outer-this, including
   ArrayUnenforcedSet's inherited list behavior and AbstractSet protocols.

These are dependency-linked milestones, not claims that four implementation
fixes will finish JGraphT. Within each milestone, batch representation,
signatures, conversions and lowering together; use minimized translated Java
controls with independent Java oracles. Then measure the untouched whole slice
after a substantial complete contract batch. Do not expand scope yet.

Feasibility conclusion: there are concrete Rust shapes for the inspected hard
cases; no demonstrated need for algorithm-wide rewrites of graph algorithms.
However, several cases require translator representation and ABI changes, and
an integrated corrected representative has not yet compiled against the actual
generated support. The experiments narrow architectural uncertainty; they do
not establish a completion estimate or exhaust the 1,539 diagnostics. Existing
generic Object hashCode/getClass/toString, wildcard/cast and further collection
protocol errors remain unclosed and must be covered before calling this a full
corpus feasibility proof.

## Minimized translator boundary target

`tests/fixtures/generic_reference_boundaries.java` now combines generic field
reads/writes/null checks, inherited edge fields, reference retention and
identity, and a generic inherited builder return. Its independent entry point
is `tests/corpus/construction/GenericReferenceBoundariesOracle.java` (compile
both files together). Java prints **0**. Frozen v125 generates the fixture
without a translation error, but rustc metadata checking reports **20 errors**:
four null-comparison placeholders, nine generic field-access errors, two
unpropagated carrier Display/Clone bounds, one generic reference equality error,
and four generic argument carrier mismatches. Raw evidence is retained as
`.tmp/generic-reference-boundaries-v125{.rs,-translate.log,-rustc.log}`.
This is a development target, not a passing translator regression yet.

The argument mismatches show a further whole-path detail: one generic formal
expects `Option<&Rc<RefCell<T>>>`, while argument adaptation produces
`Option<&RefCell<T>>`. The batch must borrow the solved Java carrier rather
than blindly dereference its storage handle. Null actual specialization must
agree with that same carrier. The generated bound trait is empty for a class
with only fields, confirming that accessor generation is a missing capability,
not just a wrong field spelling.

The first implementation component resolves scoped type-parameter receiver
bounds (including bound chains and retained generic arguments), then reuses
the existing specialized field-signature projection for expression reads and
assignment targets. This preserves solved nullability/allocation/dispatch
metadata instead of constructing a new shape from the field's Java type. It
does not yet generate bound accessors, repair generic identity/call adaptation,
or make the fixture compile; those remain parts of the same architectural batch.

Verification of this component: `cargo test --lib bound_field -- --nocapture`
passes both new tests (build 55.55s; session 76397 terminal exit zero).
Reusing that test executable, all **30 expr_shape tests** and **11 types tests**
pass (session 3757 terminal exit zero). Changed-source rustfmt --check and
git diff --check pass. No release build or whole-corpus rerun is justified yet;
v125 remains the measured corpus checkpoint. The complete generic-reference
fixture remains a known-failing development target, not an acceptance gate
claimed green by these shape tests.

## Bound-field capability component

The next staged component adds a common bound-field contract in
`representation_lowering.rs`: solved field types, reserved getter/setter names,
inherited storage naming and generic specialization are reused for bound
declarations, concrete canonical-storage impls, hierarchy-carrier forwarding and
monomorphic erased forwarding. Static/private fields are excluded; a private
declaration blocks a same-name ancestor field; final fields have no bound
setter. Native value and mutable shared storage support setters. Immutable
shared or anonymous-backed families do not advertise an unsupported setter;
their generic mutation capability still needs upstream representation work.
Erased forwarding copies a shared handle before writing, rather than requiring
unique ownership of the erased wrapper or copying its object payload.

Generic field **reads** now route through the bound getter before plain optional
field lowering. Generic Java field-write expressions/statements have not yet
been routed; publishing a setter does not establish that lowering contract.
The existing hierarchy field getter also reuses the common erased-reference
copy projection instead of emitting Clone on Box<dyn Any>. Value-backed enum
carriers gain an exclusive-borrow setter alongside the existing interior-mutable
shared setter policy. These carrier paths still need dedicated runtime coverage;
the current controls exercise concrete and erased backends.

`tests/class_bound_fields.rs` has two generated-runtime controls. They exercise
translated generic reads, direct bound getter/setter capabilities for concrete
and inherited fields, private-field hiding, canonical Rc/RefCell storage,
cloned erased-wrapper aliases, typed reference-valued children, and Object-valued
children. Reference copies preserve Rc identity, and writes remain visible to
the original object. These Rust harness assertions do not repair generated code.

First run (session 22637) compiled the generated capability code but failed two
harness calls that supplied Option<&T> to a generated non-null &T parameter.
After correcting the harness, both new tests and both null_checked_borrows
controls passed (session 53141). The expanded hiding/reference-child controls
and private-shadowing policy then pass in **session 92822, terminal exit zero**:
build **51.65s**, two class-bound integrations **11.17s**, two checked-identity
integrations **15.62s**. No library-wide gate, release candidate or corpus rerun
is claimed for this staged component. The full generic_reference_boundaries
target remains unresolved: generic writes, canonical carrier argument/identity
contracts and inherited return ABI are still work in the same batch.

## Generic field stores and nullable value boundaries

Generic field stores now route through the same bound setters for lexical
receiver locals, provided the RHS cannot rebind that local. This admission
proof reuses `value_writes_name`; arbitrary/effectful receiver places and
receiver rebinding are not claimed handled. The RHS is evaluated into a fresh,
scope-safe temporary before acquiring a setter loan or checking the receiver
for null. Both assignment statements and assignment expressions use this path.
Reference-valued assignment results reuse the existing reference-assignment
copy helper, generalized to accept a fresh temporary name. A proven-present
RHS remains outside Option as the expression result; only the stored copy is
wrapped. Nullable RHS/results remain nullable. Setter availability is shared
between declaration and lowering, and now checks nested source implementors
as well as top-level types.

The expanded control exposed two additional producer/consumer gaps, fixed
together with the store contract:

- Slot-directed owned field reads bypassed the generic getter and emitted a
  nonexistent Rust field on `&&E`. They now consume the getter's owned carrier.
  Signature return/local nullability also recognizes fields read through a
  class bound, reusing declaration facts and the conservative bound-member
  reference policy rather than forcing a non-null result.
- Assignment-expression nullability now follows its RHS through the existing
  source-return/scoped nullability analysis. A null/reference ternary used to
  discard the nullable slot ABI and synthesize a default object on the null
  branch. Expression shape inference now preserves the other branch's complete
  carrier and lifts nullability via the existing reference-cast projection.
  This rule handles either ordering of the null branch; it is not a general
  join redesign for all conditional expression shapes.

`tests/fixtures/class_bound_writes.java` and its separate
`ClassBoundWritesOracle.java` print **0** under Java. The translated runtime
control covers direct writes, assignment-result values, RHS field reads,
reentrant RHS mutation, throwing RHS before a null-receiver check, a caught
null store after RHS evaluation, and temporary-name collision. The shared
reference control additionally covers nullable getter returns, null clearing,
present and nullable reference assignment results and preserved child identity.

Verification history is explicit: session **93761** passes the three bound-field
integrations. Session **64895** exposes a wrong Option wrapper on a non-null
reference assignment result; session **73962** then exposes missing nullable
assignment return inference. Session **32887** compiles that control but catches
the lost-null ternary at runtime, while the chained-reference regression passes.
After the shared shape fix, session **97028**, terminal exit zero, passes all
**seven focused tests**: three bound-field integrations, two checked-identity
integrations, chained reference assignments, and nullable reference cast shapes.
Build **47.58s**; respective test-file durations **11.34s**, **14.40s**,
**10.46s**, **2.19s**. No generated Rust was repaired, and no failing assertions
were removed.

The broad library gate adds a direct metadata regression for null ternaries
over retained nested native containers and closed-enum references, in both
branch orders. Two test-only compile mistakes (literal variant name and a
missing ViewShape qualification) were corrected; sessions **25934** and
**88286** are terminal exit 101, not successful gates. The corrected gate is
**session 65937, terminal exit zero**, log
`.tmp/jgrapht-bound-field-library-v2.log`: **1,553 library tests pass**, build
**47.78s**, tests **83.47s**, 16 threads. No build/test session from this batch
remains live. Changed-file formatting and diff checks pass.

The full minimized generic-reference fixture was retranslated without repairs
using the current debug translator and metadata-checked with rustc. Evidence:
`.tmp/generic-reference-boundaries-field-contracts{.rs,-translate.log,-rustc.log}`,
session **92329 terminal zero** for the diagnostic command (rustc itself fails,
as the retained log shows). It has **six remaining compile errors**, versus
20 in frozen v125 and eight in the intermediate field-only probe. No generic
field-access/null-comparison placeholders remain in that minimized output;
`BoundaryAccess.source` now returns a nullable reference carrier. The six are
two Display/JavaAppendValue generic-bound mismatches, one generic reference
equality error, and three generic actual/formal carrier mismatches (including
typed null and borrowing Rc versus borrowing its RefCell payload). These are
not six remaining corpus errors or proof of complete Java semantics.

Next continue the reference-boundary milestone: preserve the canonical generic
argument carrier through specialization/coercion, share formatting capability
requirements with the invoked Display implementation, and provide Java reference
identity without substituting value equality. Inherited graph-builder trait
bridges still require their own representative regression; the plain generic
builder in this minimized fixture does not exercise every such bridge. Keep
the map/view, escaping callback and native-superclass milestones intact.
There is no new release candidate, corpus count, acceptance or scope expansion.

## Canonical generic carrier borrows and source-SAM effects (in progress)

Direct source generic parameters now retain a physical `BorrowTarget::Carrier`
through call-plan specialization. A Rust `&T` must borrow the entire value
substituted for T, for example `&Rc<RefCell<Node>>`, not `&RefCell<Node>` or
`&Node`. The slot renderer derives that type from the authoritative owned
carrier, with nullable `Option` outside the reference. Existing native shared
container contracts and non-direct generic parameters retain their own ABI.
Lowering compares actual/formal physical types, uses reborrows where needed,
and never recreates a missing owner handle by cloning an object payload.
For handle-valued field/call expressions it stages a handle copy before the
callee runs, releasing parent interior-mutability guards before reentry.

The focused oracle also exposed and now covers several connected boundaries:

- Generic field access/store owns its nullable-receiver projection once,
  rather than unwrapping a refined `&T` twice.
- Fallible numeric/logical expression producers discharge their Result at
  their Java value boundary, before the value is staged as an argument.
- An all-null, non-return-context generic binder uses its declared upper
  bound, or qualified `java.lang.Object`; recursive/dependent unresolved bounds
  and return-context inference are not guessed. Qualified Object rendering is
  being checked against the same erased carrier as unqualified Object.
- JavaAppendValue forwarding requires the Display contract it actually
  invokes, instead of reconstructing incomplete generic Clone constraints.
- Source-interface lambdas use the solved source input/return shapes and
  source trait carrier, not a same-named external facade. Captured canonical
  Rc/Arc handles are cloned before move-capture, preserving caller aliases.

The first expanded runtime probe exposed four compile errors across these
boundaries. After fixing them, the successful-callback control passed. Adding
a throwing source-SAM callback then exposed a separate real failure: its body
was not connected to the source interface's exception ABI and emitted panic
instead of a catchable Java exception. Source method callback arguments now
connect their typed synthetic lambda bodies to the declaring SAM through the
existing contract-equality/effect solver. Creation is not invocation; unused
throwing callbacks must not make their creators fallible. Method references
have one-way effect edges because their adapter, not the referenced method,
implements the SAM ABI. This is not a claim of complete inherited SAM,
constructor callback, field/factory lambda or source method-reference lowering.

Evidence so far: Java `GenericCarrierBorrowsOracle` prints **0**, including
mutation/retained aliases, all-null arguments, callback reentry and caught
IllegalArgumentException. Build v5 (session **97314**, terminal zero) took
**50.09s**; runtime session **35180**, terminal zero, passes the expanded
throwing control in **12.98s**. Its unit invocation initially used a short
name with `--exact` and ran zero tests; the subsequent real unit execution
exposed an incorrect expectation that nullable callback invokers were
infallible. The separate Clean interface must remain infallible, but calling
a possibly-null receiver can itself throw NPE. The corrected unit also adds
an infallible method-reference target control.

Final v6 build **86560** is terminal zero (**52.13s**). Gate session
**92248** is terminal zero, reusing its binaries: both new focused unit
controls pass, then **1,557 library tests** pass (**85.39s**), the expanded
generated carrier-runtime oracle passes (**11.72s**), all **three bound-field
integrations** pass (**11.37s**), and all **30 generic-method binding tests**
pass (**14.10s**). Logs are
`.tmp/jgrapht-generic-carrier-{library,runtime,class-fields,method-binding}-v6.log`.
Java oracle command **71360** is terminal zero and prints **0**. Formatting
and diff checks pass. No process from this batch remains live; there is no new
release build or full-corpus run.

The unrepaired minimized generic-reference probe at
`.tmp/generic-reference-boundaries-carrier-v5{.rs,-translate.log,-rustc.log}`
has **three compile errors**: generic `Option<&T>` equality plus two references
to the missing `JavaLangObject` type for `same(null, null)`. The previous
formatting and concrete generic-carrier mismatches are gone. Translation still
reports unsupported inheritance and exits nonzero, so rustc was explicitly run
on its emitted output; this is a diagnostic probe, not accepted translation.
The Object rendering follow-up is not measured yet. Generic Java identity
still needs a representation-aware protocol and correct identity-sensitive
placement; adding PartialEq or comparing addresses of Rc wrapper slots would
be incorrect.

Earlier focused v3 evidence remains **3 bound-field**, **1 carrier** (before
throw control), **30 generic-binding**, and **1 logical-chain** passes.
Type coercion has **118 passes / 1 expectation mismatch**: the expected raw
expression is already tagged PlainValue by committed HEAD's
`JavaDisplayPlan::OptionalString` implementation. This is source-evidenced
pre-existing test/product disagreement, not a passing gate or an assertion
to silently remove. No current full-corpus measurement or acceptance follows
from these minimized results.

### Next identity contract: representation and capability, not PartialEq

The v6 unrepaired probe removes both qualified-Object errors and now has just
the generic `Option<&T>` equality error. Log
`.tmp/generic-reference-boundaries-carrier-v6-rustc.log`; diagnostic session
**72138**, terminal nonzero as expected. The CLI's unsupported-inheritance
diagnostic remains; this is not accepted translation or a behavior oracle.

The smallest correct rewrite requires a Java-reference identity operation on
the canonical generic carrier, lifting it through nullable operands (two nulls
equal; one null unequal). `Rc::ptr_eq`/`Arc::ptr_eq` can implement that operation
for handles; existing hierarchy-carrier `java_identity_eq` can provide its
case dispatch. `std::ptr::eq` on references to Rc wrapper slots is wrong:
separate handle slots can point to the same Java object. PartialEq and
JavaMapKey are also wrong contracts because their value/equals semantics can
disagree with Java reference operators.

The complete batch must connect three paths before claiming this solved:

1. Detect identity-sensitive generic binders and propagate the capability
   through selected source calls, retained fields and return/bridge contracts.
   `item_assembly::SourceCallableCapability` already has selected-call
   reachability for JavaMapKey/JavaAny/display; extend/generalize that mechanism
   instead of building a second per-emitter inference loop.
2. Ensure source actuals reaching those binders have stable reference identity
   in storage. Reuse `storage_strategy`'s identity/alias placement requirements;
   an otherwise inline value cannot acquire correct alias identity merely by
   adding a trait implementation. Canonical borrowed generic arguments must
   continue to borrow the handle, not its payload.
3. Supply implementations/conversions for actual canonical representations,
   including hierarchy carriers, erased Object and identity-aware strings.
   Scalar-backed boxed Java references expose a further representation issue:
   equal scalar values do not establish boxed-object identity. Do not silently
   use scalar equality or claim arbitrary boxed-reference support.

Runtime controls should distinguish equal-content distinct objects from aliases,
override equals without changing `==`, exercise repeated references from fields
and generic returns, and cover null/null, typed null, bound/unbound generic
forwarding, closed-enum and erased crossings. Rc/Arc rendering and placement
controls must accompany the runtime cases. The previous borrowed-field and
callback controls remain necessary: fixing identity must not reintroduce
payload clones, parent-guard reentry failures or lost Java exceptions.

## Generic identity capability and reference-copy batch (v126 candidate)

`JavaReferenceIdentity` now provides the retained allocation token independently
of equals/hashCode. Rc/Arc implementations use their pointee address, borrowed
and boxed carriers delegate, JavaString uses its retained string allocation,
and hierarchy carriers select the concrete variant's allocation. The nullable
`java_reference_equals` boundary handles both-null and one-null without a
receiver dereference and supports compatible heterogeneous carriers.
No PartialEq constraint or structural equality is substituted for Java `==`.

Method- and owner-scoped binder requirements reuse the existing
`SourceCallableCapability` selected-call reachability. The same instantiated
requirements feed source-family sharing and string-identity selection. Generic
argument rendering/lowering still borrows the canonical carrier. Completed
carrier borrows are now protected from both erased-reference copy recovery and
the older all-handle-enum owned-ABI adapter, through one completion predicate.
BorrowTarget is part of recorded argument-shape matching; a payload borrow
cannot prove that a canonical carrier borrow has already been produced.

Erased identity dispatch and erased reference copying now share their canonical
source-handle inventory, including monomorphic source hierarchy carriers.
An unknown erased representation fails explicitly, rather than comparing
wrapper-slot addresses or values. This is not yet arbitrary native boxed-value,
array/list value-carrier, plain Object allocation or reified generic-erasure
identity support. Virtual generic capability/return bridges also retain their
separate milestone; do not infer those contracts are solved by this batch.

The broader controls exposed three additional producer/consumer gaps:

- Local subtype assignments consulted the usage TypeEnv, which contains
  declaration binders rather than evolving lexical locals. The shared subtype
  query now reuses lexical binding facts across local, field, argument,
  constructor and return boundaries. This correctly selects the existing
  BoundaryEdge hierarchy carrier instead of silently assigning an Rc subtype
  to an Rc superclass payload.
- Erased local aliases were excluded from owned-value copy recovery. They now
  use the existing erased-reference copy kernel (with optional-carrier
  preservation), gated by the existing binding/liveness evidence.
- Per-file erased runtime registrations must survive shared-support assembly.
  Opt-in additive registry regions are merged deterministically only when their
  surrounding ABI and fallback are identical; incompatible producers fail
  visibly. Three fast Python controls pass, including compiling and executing
  the actual assembled two-module registry. The test is wired into the Rust
  script integration gate. Ordinary unmarked method selection is unchanged.

Java `GenericReferenceIdentityOracle` and the expanded
`GenericReferenceBoundariesOracle` both print **0**. Controls distinguish
equal-valued distinct objects from aliases despite an equals override, retain
generic owner fields and returned aliases, forward generic calls, compare
nulls, preserve string identity, and cross erased and superclass carriers.
An additional generated-runtime Rust control checks Arc/Mutex alias tokens,
mutation and boxed delegation. No generated Rust or pinned Java was repaired.

Verification history: v2 passes the original reference-boundary runtime but
exposes two erased-argument type mismatches in the broader oracle. V3 removes
those and exposes a moved erased alias plus the missed local superclass
carrier. V5 passes the broader identity oracle, but the expanded superclass
case exposes a stale adapter's `&Option<Carrier>` instead of `Option<&Carrier>`.
The v5 broad gate still passes **1,557 library tests**, session **44650**
terminal zero (**86.30s**); that is not evidence for the later adapter fix.

V6 build **41469** is terminal zero (**54.20s**). The latest generated runtime
logs show **2 identity**, **1 carrier/callback**, and **3 bound-field**
integrations passing, including all expanded controls. Their command is
session **78293**, collected terminal zero. Script build
takes **1.75s**. Three selected script integrations pass, including the new
registry integration, and the two alias-assembly Python tests pass. The full
v6 library command **75098** is collected terminal zero: **1,558 pass**, no
failures (**87.94s**). Logs use `.tmp/jgrapht-generic-identity-*-v6.log`.
These verification commands overlap; their timings are not translator
performance measurements.

### Release/corpus measurement started

Session **24477** is the v126 measurement chain, started after every build and
test command above was collected terminal. It checks changed-file formatting
and the diff, builds release into
`.tmp/jgrapht-v126-generic-reference-identity-build.log`, then freezes and
byte-compares `.tmp/new-java2rust-v126-generic-reference-identity` and writes
`.tmp/jgrapht-v126-generic-reference-identity-binary.sha256`. Only after those
steps succeed does it run the usual 10-job untouched 104-file slice, with
`--skip-resolver-bench`, root
`.tmp/jgrapht-core-1.5.3-focused-v126-generic-reference-identity`, log
`.tmp/jgrapht-v126-generic-reference-identity-run.log`.
Collect this exact session; do not restart from an absent/unchanged log alone.
No other batch process remains live. Do not edit translator source while the
candidate is being frozen/measured. The latest completed full-corpus checkpoint
remains v125, unaccepted; no v126 diagnostic count, promotion or expansion is
claimed yet. Compare diagnostic identities/additions/removals and markers as
well as totals. BorrowTarget adds physical metadata to debug-printed shapes;
cosmetic debug-header differences must not be mistaken for new semantic
allocation guards. Graph behavior and all whole-slice completion requirements
remain unproven.

### V126 collected; next reference-contract follow-up

Session **24477** is collected terminal exit **1**, the expected generated-Rust
failure, not a stuck job. Release build succeeds in **1m37s**; frozen binary
**21,537,048 bytes**, SHA-256
`5a50b6ce66ae74af51615134d745f568ec60a04968150014d1cd758bed2dcb2d`.
The runner labels this prebuilt profile unknown because the caller omitted
NEW_JAVA2RUST_BUILD_PROFILE; the retained release build log and byte comparison
prove it is the release executable. Do not rerun solely for that label.

All **104 files translate**, zero translator/generated-parse failures.
Diagnostics **1,430 = 1,350 typed + 80 untyped**, markers **73 in 31 files**,
allocation guards **38**, unchanged (32 plain-to-shared, six reverse).
The raw full-header/first-generated-file multiset removes **136**, adds **27**,
net **-109** against v125; all headers were counted, with no cosmetic metadata
normalization needed. Evidence: `.tmp/compare-jgrapht-diagnostics.py`, validated
to reproduce v125's exact count and zero self-delta, and
`.tmp/jgrapht-v125-v126-diagnostic-delta.txt`.

Removals include 98 generic V equality failures, four erased endpoint
comparisons, twelve generic bound field accesses and four null-comparison
placeholders, plus clone-bound failures. Additions are **21 missing
V: JavaReferenceIdentity bounds** and **six RefMut temporary lifetime errors**.
The capability bodies use top-level `TypeEnv::learn_stmt` then an untyped
recursive expression walk: variables declared inside nested loops/blocks are
absent, and loop/resource locals can leak outside their actual scope. Fix the
shared capability traversal with lexical environments, not per-method bounds.
The six lifetime errors are retained collection bulk mutation calls whose block
tail retains a guard temporary until after its staged receiver local drops.
Stage the owned bulk-call result before leaving the producer block; this is not
permission to move borrowed values outside their guards or append semicolons to
generated files. These form the next coherent follow-up before the map/live-view
milestone. Both failure families remain visible and have not been counted as
fixed merely because other errors disappeared.

Total runtime **276.930s**: translation **190.075s**, assembly **65.544s**, check
**16.947s**, sweep **3.702s**. Global analysis **47.587s**: semantic **12.907s**,
representation **5.903s**, signatures **24.600s** (collection **13.914s**,
fixpoint **4.616s**, plan build **5.822s**). Slots **3,881**, graph nodes
**16,441**, edges **20,990**, iterations **13** remain unchanged. These unpaired
runs do not establish a causal optimization benefit. No competing build/test
ran during the corpus measurement. The candidate is unaccepted; 104-file graph
behavior and whole-slice readiness remain unproven, with no scope expansion.

### Scoped capability and owned bulk-result follow-up (in verification)

The shared callable-capability walk now carries lexical type environments
through blocks, loops, resources, catches, lambdas and local/anonymous members.
Reference identity, value equality, erased-value and display capability queries
reuse that walk; conversion-specific compatibility queries still retain their
older separate environment. Untyped lambda parameters mask outer names rather
than inventing inferred types. This is not a claim that all target-inferred
lambda or nested generic-owner scope problems are solved.

Bulk AddAll stages its owned result before leaving its receiver block, through
a reusable result-staging helper which preserves discharged exception metadata
without marking an undischarged Result as propagated. The regression includes
returned/discarded results, aliased targets, self-addition and unsupported
mutation through an unmodifiable view.

The same regression exercises nested generic array loops. Retained arrays now
capture their canonical allocation once, stage length separately, and copy
each current element under a short guard before executing the body. They do
not snapshot the array or retain a guard across Java writes. Existing owned
iteration binding conversion and generic Clone requirement inference are
reused. Nullable array initialization uses existing retained-child recovery;
implicit enhanced-for null dereference enters the existing exception analysis.
Parameter rebinding participates in the existing native reference-flow graph,
so replacement arrays receive the canonical carrier too.

The untouched Java fixture/oracle returns **0**, including live element writes,
array parameter rebinding while the original iteration remains stable, reading
the rebound parameter, null guards and a caught null enhanced-for receiver.
V4 combined build session **18490** completed successfully (**51.99s**, retained
log `.tmp/jgrapht-scoped-capability-build-v4.log`; its handle is now absent).
V4 runtime session **92611** is terminal **101**: the two prior identity
integrations pass; the new integration exposes two typed-null argument carrier
mismatches, not the prior array-rebinding placeholder. V4 library session
**64209** is collected terminal **0**, **1,561 pass**, **81.17s**, log
`.tmp/jgrapht-scoped-capability-library-v4.log`.

The typed-null follow-up bypasses plain-value argument shortcuts for reference
casts of null, constructs the destination slot's existing typed null directly,
and shares cast-null recognition with generic-carrier and ordinary slot
lowering. A new focused unit covers plain/shared-local/shared-sync array
carriers and rejects primitive casts/non-null expressions. Combined build
**14920** is pending, log `.tmp/jgrapht-scoped-capability-build-v5.log`.
No runtime pass, new full-library gate or corpus measurement is claimed for
this latest follow-up yet. The accepted scope remains unchanged and incomplete.

V5 build **14920** is collected terminal zero (**51.27s**); the focused null
argument unit passes (**0.01s**). Runtime chain **9071** is terminal **101**:
the expanded fixture now compiles, and its Java comparison returns **7** instead
of **0**. The old two integrations pass. List/array follow-ons behind `&&` did
not run. The failure is substantive: an unmodifiable view passed through a
source method becomes the writable backing allocation, because the old view
contract is only an expression/local marker. Do not remove that assertion or
accept compilation alone.

The retained native collection backing abstraction is being extended with
collection-backed unmodifiable wrappers, rather than marking the original
allocation immutable or snapshotting its data. A distinct wrapper retains the
source allocation; reads and modification-count queries delegate to it,
including through nested wrappers. Mutation policy lives in the wrapper and
crosses ordinary source signatures with the existing canonical carrier.
Local and synchronized backends remain separate, with no Rc inside the
synchronized backend. Existing checked collection and iterator operations
enforce the wrapper policy, including empty bulk insertion/clear and absent
removal. This does not implement map key views or all Collection object
protocols; synchronized reentrant/multi-owner equality remains unaudited.

The Java oracle still returns **0** after adding live backing writes,
parameter-mediated rejected set/empty-addAll/clear, and iterator invalidation.
The generated-runtime control additionally checks nested wrappers, distinct
wrapper allocation, local/sync backing reads, modification counts, Send+Sync
of the sync carrier, and set membership/equality in both directions.
V6 build **1299** is collected terminal zero (**50.79s**); runtime chain
**96150** is pending in `.tmp/jgrapht-scoped-capability-*-v6.log`.
A subsequent small backing-aware `contains` edit is not yet claimed verified
by that executable. No new release/corpus run has started.

V6 runtime chain **96150** is collected terminal **101** (**15.26s**): its
independent backend runtime control passes, and the Java fixture advances from
failure **7** to failure **17**. The latter is a unique empty unmodifiable list
whose planner still chooses Vec, so a method-mediated clear silently succeeds.
The fix seeds retained-view policy/backing requirements in the existing native
factory storage graph, independent of other alias/escape evidence, and shares
the element domain without equating source/view nullability. A focused usage
test covers a unique view and its backing. This is a general factory property,
not a special case for empty lists.

V7 build **42671** is collected terminal zero (**51.32s**). Focused view-policy
test passes (**0.22s**). Runtime chain **27519** and full-library **20659** are
pending, logs `.tmp/jgrapht-scoped-capability-*-v7.log`. This build includes the
backing-aware contains change. Do not reuse older builds as evidence for this
candidate. Remaining audit questions include inline fresh/typed-null backing
expressions (without a solved source storage node), Collection-vs-List/Set
object equality protocols, synchronized multi-owner/reentrant operations, and
unsupported retained collection operations. Map key views remain a separate
unclosed representation/operation boundary. No whole-slice readiness is claimed.

### V7 gates collected; v127 release/corpus chain started

Both pending processes are collected terminal **0**. Runtime chain **27519**
passes **3 identity/scoped-contract integrations** (**32.55s**), **2 retained
list integrations** (**28.33s**) and **1 retained-array integration** (**11.74s**).
The scoped-contract generated crate passes both the independent Java-result
comparison and backend runtime controls. Full-library **20659** passes
**1,563 tests**, zero failures (**75.31s**). These gates overlap, so their times
are not isolated translator performance measurements. Expanded independent
Java oracle session **58720** is collected terminal zero, output **0**. Focused
null carrier coverage also passed in v5; its helper is unchanged in v7.

After collecting every test/build process terminal, session **58896** starts
the v127 chain: changed-file formatting/diff checks, release build, fresh binary
freeze/byte comparison/SHA-256, then the untouched 104-file runner with 10 jobs,
explicit release producer profile, and `--skip-resolver-bench`.

- Build: `.tmp/jgrapht-v127-scoped-reference-contracts-build.log`.
- Frozen binary: `.tmp/new-java2rust-v127-scoped-reference-contracts`.
- Hash: `.tmp/jgrapht-v127-scoped-reference-contracts-binary.sha256`.
- Root: `.tmp/jgrapht-core-1.5.3-focused-v127-scoped-reference-contracts`.
- Run: `.tmp/jgrapht-v127-scoped-reference-contracts-run.log`.

Collect this exact process before restarting anything; it was confirmed live on
launch. No competing tests/builds remain, and translator source must remain
unchanged during freezing/measurement. V126 remains the latest collected full
measurement. No v127 counts, marker reductions, timing or graph-behavior
acceptance is claimed yet. Compare raw diagnostic additions/removals as well
as totals with `.tmp/compare-jgrapht-diagnostics.py`. Goal and 104-file scope
remain incomplete; the remaining audit questions above are not waived.

Release phase is now observed successful: **1m51s**, frozen **21,543,552 bytes**,
SHA-256 `5a81661ee7696b151047e09e745166649ac47e1b1772776812c2e195584fa8e4`.
Session **58896** remains live in the corpus phase; collect the same handle.
These observations are not a corpus result.

Read-only inspection while waiting confirms another whole-path map obligation:
the canonical JavaSemanticHashMap has no structural modification counter;
the legacy JavaTrackedHashMap uses entry identity/observers with Rust Eq/Hash
contracts, so it cannot simply replace the JavaMapKey canonical map. Existing
key-view helpers already implement local removal/clear/bulk-query behavior,
but key-set values crossing fields/returns currently materialize a set snapshot.
The next map/view design must reuse those operation policies while adding
retained backing and appropriate iterator invalidation, not merely project
borrow()/borrow_mut() at ordinary calls. Existing collection-backed wrappers
do not yet provide a map-key backing operation table. No implementation choice
or narrower acceptance criterion follows from this provisional inspection;
collect the diagnostic delta before choosing the next substantial batch.

### V127 collected; retained map/view batch selected from the delta

Session **58896** is collected terminal **1**, generated-Rust failure rather
than an infrastructure failure. All **104** files translate, zero translator
or generated-parse failures. Diagnostics are **1,406 = 1,313 typed + 93 untyped**,
invalid markers **86 in 41 files** (v126: 73 in 31), allocation guards **33**
(27 plain-to-shared, six reverse; v126: 38). Exact raw diagnostic identity/file
comparison removes **118** and adds **94**, net **-24**:
`.tmp/jgrapht-v126-v127-diagnostic-delta.txt`. No errors or marker classes were
hidden or normalized away. The candidate remains unaccepted.

The removed set includes seven staged-receiver guard lifetime errors and eight
of the 21 missing reference-identity obligations, plus allocation/collection
carrier failures. Thirteen reference-identity obligations remain: an inspected
case is inherited BaseShortestPathAlgorithm.getPathWeight calling the concrete
BellmanFord getPath implementation, while capability reachability follows the
lexically resolved declaration. Clone analysis already has effective inherited
current-receiver dispatch and ancestor substitutions; reuse/generalize that
mechanism for the other capability queries rather than creating another
dispatch resolver or adding whole-owner bounds.

The additions are predominantly consumers exposed by retained collection
contracts: **18 unsupported operations**, **15 AsRef/slice display obligations**,
**13 collection equality comparisons** still emitted as Rust PartialEq, and
**20 self borrow conflicts**, plus carrier mismatches. Most new unsupported
operations are collection hashCode; GraphWalk also needs equals and traversal
operations. This does not justify restoring plain-value carriers: that would
reintroduce the demonstrated unmodifiable-view and aliasing semantic failures.
Object equality/hash/display and reentrant source calls need real operation
contracts, not blanket PartialEq/Hash or a held cell guard around callbacks.

Total **261.863s**: translation **175.910s**, assembly **64.193s**, check
**17.514s**, sweep **3.563s**. Global analysis **53.261s**, semantic **16.090s**,
representation **7.495s**, signature **25.650s** (collection **15.796s**,
fixpoint **4.352s**, plan **5.242s**). Slots **3,881**, nodes **16,441**, edges
**21,022**, iterations **13**. No competing tests/builds ran. Unpaired changes
from v126 do not prove a causal speedup; producer hash/profile are recorded in
the summary. No graph behavior acceptance or expansion follows from the count.

Next substantial batch: the retained **map/live-key-view producer-to-consumer
contract**, maintaining the audit's architectural order. Include canonical
map allocation flow, short-loan child reference reads/writes, returned/cached
key views retaining their backing, distinct unmodifiable wrappers, and backing
structural-change observation for iterators. Reuse native storage graph,
collection backing/operation policies, and existing local key-view semantics.
Do not turn keySet into an owned set snapshot or mark the backing map immutable.
Collection object-protocol consumers and virtual capability/return contracts
remain explicit linked obligations, not silently declared solved by that batch.

New maintained regression `tests/fixtures/retained_map_views.java`, driver
`tests/retained_map_views.rs`, and independent oracle
`tests/corpus/collection/RetainedMapViewsOracle.java` cover method-mediated
map aliases and child reference identity after removal, live returned writable
and unmodifiable key views, write rejection including absent remove/empty clear,
nonstructural replacement versus structural invalidation, and iterator removal.
Oracle session **43045** is collected terminal **1** for the composite command:
Java itself prints **0**; frozen-v127 translation produces output, and rustc
metadata check fails with **four errors**. Evidence:
`.tmp/retained-map-views-v127{.rs,-translate.log,-rustc.log}`. No output repairs.
The generated-runtime integration has been added but not built/run or claimed
passing. The fixture also exposes **signature_fixpoint_budget_exceeded at 397
iterations**, unlike the converged whole slice. Its mixed LinkedHashMap/Map
carrier transfers, returned key-set snapshot and source child/carrier mismatches
must be traced together; a compiler-only patch is not sufficient. Diagnose the
nonconvergent transfer rather than increasing its finite budget or weakening
the fixture to avoid the native map-family boundary. No process remains live.

### Map/view precondition: monotone carrier consensus

Read-only profile linked against the existing debug library confirms the
specific nonconvergent arm: `call_argument_collection_carrier_to_callee`
changes on **397/397** sweeps, with one saturation diagnostic. Evidence:
`.tmp/inspect-retained-map-plan.rs`, `.tmp/retained-map-plan-v127.log`.
The old refinement collects only calls whose carrier differs from the current
parameter. Matching callers disappear from its consensus, so Map and
LinkedHashMap alternately appear unanimous. Merely counting matching witnesses
would fix this example but still permit cycling through forwarded parameters.

The shared method/constructor parameter refinement now keeps a finite monotone
carrier evidence set per slot. Unique evidence may select that carrier; once
conflicting kinds reach a slot, they cannot be forgotten and the outer carrier
returns to its declaration family. This evidence is part of solver state
equality. Capture/wildcard refinement remains independent and keeps current
declaration-scoped type arguments. Constructor state equality also now includes
its parameter Java types, so an actual constructor carrier update cannot be
mistaken for an unchanged sweep. No iteration-budget increase or skipped warning.

This is a consistency precondition, **not** a solution to Java map identity:
falling back to the declared family must eventually use a canonical retained
carrier, not collect or deep-clone a different implementation into it.
The map/view runtime and source conversion remain unresolved.

Combined build **5047** is collected terminal zero (**51.85s**). Focused chain
**9319** is collected terminal zero: **142 signature tests** pass (**2.10s**),
the new full-fixture convergence integration passes (**2.60s**), and the
existing native collection boundary runtime passes (**11.99s**). The signature
unit exercises both witness arrival orders, late conflict, repeated stable
observations and constructor state equality. Inspector **46604** is collected
terminal zero; `.tmp/retained-map-plan-consensus-v1.log` shows **4 iterations,
zero diagnostics**, the former oscillating arm changes once. Logs:
`.tmp/jgrapht-map-view-consensus-{build,signature,plan,boundaries}-v1.log`.
No new whole-library/corpus run or passing map/view Java-runtime integration
is claimed; the latter deliberately remains enabled and unresolved. No live jobs.

Producer-to-consumer implementation constraints for continuing the map batch:

- `EffectiveFacts::retained_native_collection_kind` intentionally describes the
  one-element List/Set runtime. Do not simply add Map to it: payload rendering,
  constructors and child projections assume one element. Generalize allocation
  retention authority separately, with parsed-source precedence intact.
- Native map aliases/captures currently miss that retention seed, and keySet
  calls are not modeled as retained-view backing relationships. Add root
  retention and explicit key-child mapping through the existing storage graph;
  do not equate source/view nullability or blindly map all map children into
  a one-element key-view carrier. Preserve synchronization tier constraints.
- Retained Map/LinkedHashMap callers need one physical reference ABI preserving
  the allocation's iteration policy. Family consensus alone cannot authorize
  collecting a map parameter into another map implementation.
- Reuse canonical JavaMapKey map behavior and checked native collection cursor
  policies. Add structural-change observation to the canonical map; value
  replacement must not invalidate a key cursor, insertion/removal must. Key
  views are distinct objects retaining the source, supporting removals but not
  additions; unmodifiable wrappers reject mutations even when no change would
  result. Existing collection-backed wrappers are reusable once the actual
  key-view producer exists.
- Map reads must copy canonical child handles inside a short map loan, retain
  Java null versus missing-key behavior, and release loans before reentrant
  Java code. Whole method/constructor argument, return, field, local and
  capability contracts must agree. A local `borrow()` projection or extra
  Clone bound is not the complete batch.

Post-consensus runtime baseline **78417** is collected terminal **101**,
**14.40s**, `.tmp/jgrapht-map-view-consensus-runtime-v1.log`. It fails before
execution with **19 rustc diagnostics**: one key-set snapshot collect-to-handle
failure and repeated map-family conversions which lose the shared source child
carrier (`Option<Rc<RefCell<RetainedMapValue>>>` versus a payload value). The
correctly stable declared Map ABI makes those formerly inconsistent calls
uniformly visible; fewer compiler diagnostics from a nonconverged signature
state would not be correctness evidence. Do not undo consensus or specialize
each call independently to hide these. This confirms the next batch's canonical
map/reference ABI requirement. All processes are now collected terminal.

### Retained key-view runtime and storage wiring (batch still incomplete)

The canonical JavaSemanticHashMap now tracks structural changes: insertion,
successful removal, clear and removals during retain update its counter;
replacement of an existing value and value-only retain do not. Key-position
and indexed removal use the same existing JavaMapKey/comparator policy as the
map. Retain records removals as they occur, not only after all callbacks return.

The retained collection backing runtime now supports an operation projection.
Local and synchronized map-key projections retain the original map allocation,
not a key snapshot. Length/get/position/removal/clear/modification-count queries
delegate to that backing; snapshots used by existing read-only slice consumers
copy keys under a short map loan and release it before their callback runs.
Map values need no Clone bound. Key values must already have canonical Java
reference-copy carriers; this is not permission to clone object payloads.
RemoveOnly policy permits key removals/clear and rejects insertion; the existing
unmodifiable wrapper retains its own stricter policy. Existing checked cursors
observe the backing counter and retain their removal state across aliases.
Synchronized projections contain no Rc, and require Send for captured storage.

Combined build **50110** is collected terminal zero (**58.93s**), log
`.tmp/jgrapht-map-view-runtime-build-v1.log`. Focused **62128** passes **14
external generic-carrier tests** (**14.11s**), including expanded generated Rust
runtime controls for all the above. They cover non-Clone values, reference keys
with non-Clone payloads, equal-valued distinct keys, retained backing lifetime,
iterator replacement/invalidation/removal, empty/nonempty insertion rejection,
unmodifiable absent removal/empty clear, callbacks after the map loan ends, and
local/sync projections with a cross-thread backing mutation. Identity/scoped
contract **15030** passes **3 integrations** (**18.88s**). Both are collected
terminal zero. These gates overlap and are not performance measurements.
No source map/view Java-runtime integration pass is claimed.

After that build, storage wiring adds an explicit root-only view-backing edge
to the existing native allocation graph. Roots propagate retention/sync tier,
but child paths are mapped separately (map key child zero to view element zero),
never blindly copying the map value domain into key-view storage. Outer source
and view nullability are not equated. EffectiveFacts distinguishes retained
map payloads from one-element collection payloads, while sharing an allocation
retention authority for native aliases/captures. Parsed source precedence is
preserved. Hash/linked retained map rendering selects the canonical semantic
map payload; sorted/comparator-specific producers are not newly claimed
supported. A new signature unit checks that key-view retention does not lift
the map's unrelated List-valued child.

Current-source **cargo check --lib**, session **80814**, is collected terminal
zero (**24.79s**), log `.tmp/jgrapht-map-view-planning-check-v1.log`.
The new planner unit has **not yet been built or run**: the test executables
from 50110 predate storage wiring. Reuse the runtime evidence for that unchanged
kernel, not as evidence that the new planner/front end is correct. No live jobs.

Continue this same batch with retained map constructors, canonical handle
argument/return adaptation, short-loan owned child reads/writes, and key-view
factory lowering. Native call argument planning already exposes solved child
shapes in `native_collection_argument_plan_for_target`; reuse it rather than
patching final Rust argument text. Query ABI needs explicit care: String can be
a borrowed str, nullable keys can be Option<&K> rather than &Option<K>, and
borrowed concrete source payloads are not reconstructible owner handles.
Do not invent a Rust PartialEq/Hash bound or clone an arbitrary key payload to
force those carriers to match. Existing generic canonical-carrier borrowing,
clone/null/result metadata and key equality protocols supply pieces of this
boundary; account for the whole producer/consumer contract before another
expensive no-run rebuild. Java key-view fixture and enabled failing runtime
integration remain intact. V127 is still the latest measured whole slice.

### Retained map frontend follow-through (in progress)

Constructors, canonical shared map argument/return adaptation, short-loan owned
map child operations and live key-view factories are now wired through lowering.
Key-view nullability transfers map key qualifiers explicitly, not the legacy
map sequence domain (which denotes values). Generic key-view capabilities use
the shared callable capability graph: keys need JavaMapKey/Clone, captured
values need Any but not Clone. The map carrier consensus remembers conflicting
callers monotonically instead of oscillating between Map and LinkedHashMap.

Combined v2 no-run build passed (50.64s). Its focused gates passed 143 signature
tests and 14 runtime carrier tests. The expanded Java oracle returned zero.
The Java-to-Rust integration is still failing: v2 emitted four compiler errors
(erased child Clone, over-borrowed nullable string query, chained setter
receiver, erased reference comparison), and an alias diagnostic incorrectly
excluded maps despite their solved shared carriers. These are not acceptance.

The next candidate reuses the native container alias proof for maps, unifies
nullable Object boundary classification across signature/projection/lowering,
uses the retained semantic map's borrowed string query ABI, and admits chained
shared setter receivers through the existing field-assignment lowering.
V3 combined build passed in 52.18s. The new nullable Object projection unit,
143 signature tests and all three generic reference-identity integrations pass.
The map integration gets past alias diagnostics and now emits only one Rust
error: erased-object versus retained source-handle `!=` still selects PartialEq.
Logs: `.tmp/jgrapht-map-view-lowering-{build,runtime,signature,identity}-v3.log`.
The next candidate extends the existing generic reference-identity renderer to
erased boundaries when both operands have supported identity carriers, reusing
the runtime allocation-identity protocol rather than introducing another
equality mechanism. V4 combined build passed in 55.10s; both retained-map
integration tests pass (16.43s), including the generated runtime oracle returning
zero. All three identity integrations pass (15.34s), and all 13 shape projection
units pass. Logs use the same `jgrapht-map-view-lowering-*-v4.log` prefix.
The full library gate passes **all 1,566 tests** (75.26s), collected session
95504, `.tmp/jgrapht-map-view-lowering-library-v4.log`. No further source edits
have been made since the V4 build. This clears the batch for a frozen release
measurement of the untouched 104-file slice, not goal completion.
No whole-slice rerun or performance claim is made for this unfinished batch.

Frozen V128 measurement has now started (session **77079**; collect this handle
before starting another run). Release build **74558** is collected terminal
zero, **1m46s**, `.tmp/jgrapht-v128-retained-map-views-build.log`.
Binary `.tmp/new-java2rust-v128-retained-map-views`, **21,579,224 bytes**, SHA-256
`10bda78e76e6bf0411c207eb7715804bc454b937aa4f6a32bfe5a107db08a165`.
Runner uses explicit release profile and `--skip-resolver-bench`; prior resolver
evidence is reused, while pinned-source validation, all 104 translations,
assembly, cargo check and placeholder sweep remain enabled. Output root
`.tmp/jgrapht-core-1.5.3-focused-v128-retained-map-views`; log
`.tmp/jgrapht-v128-retained-map-views-run.log`. No source edits or competing test
workloads during this measurement.

V128 session **77079** is now collected terminal one (generated Rust cargo
failure). All 104 files translate; translator and generated parse failures are
zero. Diagnostics **1,336 = 1,249 typed + 87 untyped**, versus V127's 1,406.
The exact diagnostic-header/first-generated-file multiset comparison removes
**136** and adds **66** (net -70), not 70 independently fixed bugs. Allocation
guards fall **33 -> 22**, markers **86/41 files -> 85/39 files**. Comparison
`.tmp/jgrapht-v127-v128-diagnostic-delta.txt`.

Total **263.335s**: translation **177.918s**, assembly **64.306s**, cargo check
**16.728s**, sweep **3.684s**. Global analysis **49.690s**, signatures **24.224s**
(collect **13.869s**, fixpoint **4.600s**, plan **5.499s**); emission **114.002s**.
Unpaired timings are not a speedup claim or grounds for reopening optimization.

Removals include retained-map get/insert/remove/contains/size and key-view
carrier failures across directed/undirected graph specifics and shortest-path
implementations. Additions expose nested Pair key-carrier disagreements,
missing Clone/JavaMapKey capabilities, five unsupported retained-map operations
(putIfAbsent/values), and canonical borrow errors. Some prior computeIfAbsent
errors change backend spelling rather than disappear. The candidate is **not
accepted as a successful corpus translation**; graph behavior gates and scope
expansion remain unfulfilled.

The immediate follow-through batch targets canonical reference borrows, not
arbitrary extra reference-depth query implementations. Nullable query carriers
must be independent of nullable stored-key carriers: null is absent when K
excludes null, not a null receiver error. Map query planning preserves that
nullable query and canonical generic carrier; the runtime query protocol handles
its absence without widening storage. The shared nullable-borrow conversion
also stops manufacturing &&T from an existing Option<&T>. Current-this identity
uses the companion's canonical handle rather than a RefMut payload; changing
the companion body lets existing emission rules omit invalid borrowed helpers.

Maintained minimized fixture `canonical_carrier_queries.java` covers nullable
generic foreach queries, null read/remove against non-null key storage and both
directions of generic current-this identity, including distinct objects/null
and mutation counts. Independent Java oracle returns zero. Combined build
**99321**, `.tmp/jgrapht-canonical-query-build-v1.log`, passes (59.97s). Its 143
signature tests, three identity integrations, generic-borrow integration and
prior map/view integration pass. The new regression fails compilation only at
remove(null): an earlier legacy argument shortcut bypasses the canonical query
plan and emits &Default::default(). Retained map queries now select their
canonical argument plan before that shortcut; build **95409**,
`.tmp/jgrapht-canonical-query-build-v2.log`, passes (52.40s), along with all 14
runtime generic-carrier units. The new integration now compiles all query
boundaries and current-this identity, but exposes a staging error at
owner.matches(owner): the argument temporary moves owner before receiver
evaluation. The shared same-receiver argument staging now copies the proven
canonical handle through the existing nullable reference-carrier copier;
ordinary argument expressions retain their existing owned materialization.
Combined build **49777**, `.tmp/jgrapht-canonical-query-build-v3.log`, passes
(51.44s). All three retained-map integrations pass (16.36s), including the new
canonical-query/current-this Java-runtime oracle and direct non-null-key map
null-query controls. All three identity integrations pass (15.83s). Logs
`.tmp/jgrapht-canonical-query-{runtime,identity}-v3.log`.
The generic-borrow integration passes (11.69s). Full library gate **8426** is
collected terminal one: **1,565 pass, one fails** (73.23s),
`.tmp/jgrapht-canonical-query-library-v3.log`. The failing immediate-keySet-alias
unit exposes an actual bypass, not merely a stale assertion: output declares
Rc<RefCell<JavaSemanticHashMap>> and a live projected Set, then emits
values.remove("a") directly. The old unit had asserted that raw-map spelling.
Retained view receivers now defer that legacy alias shortcut to the canonical
native collection operation backend. The unit instead requires a live-key-view
factory and remove_result and forbids raw values/keys.remove calls; expanded
source-runtime controls require successful removal even when the mapped value
is null. Independent Java oracle still returns zero. Build **20113**,
`.tmp/jgrapht-canonical-query-build-v4.log`, passes (51.25s). All three map/query
integrations pass (37.15s under concurrent lowering tests), including immediate
alias removal of a null-valued mapping. Lowering gate **99692** is collected:
337 pass and one textual assertion fails (54.99s). Isolated **58926** confirms
the emitted source body is now correct: live projection construction followed
by its guarded remove_result call. The negative raw-map text assertion instead
matches an unrelated Vec helper named values in runtime support. The unit is
now scoped to the source owner's implementation and requires guarded projected
removal there, while still forbidding raw source values/keys.remove. Captured
output `.tmp/jgrapht-canonical-query-key-alias-unit-v4.log` was inspected against
that corrected assertion. **Only this test-only assertion-scope edit is unbuilt**;
bundle its focused rerun into the next combined build rather than rebuilding
the whole translator solely for a string assertion. There are no live jobs.
No fresh full corpus measurement has been
run for these follow-ups. Sorted-map null policies and new retained-map
putIfAbsent/values support are not claimed by this batch.

Next coherent map batch: putIfAbsent and live values views, plus canonical
child-carrier/capability obligations exposed in V128. Reuse operation projections,
qualified storage-flow edges and the selected-call capability graph. Values views
must map map value child one to collection child zero (not copy the key path),
retain backing identity/modification counts, and remove keys even for null or
duplicate values. Do not require V: JavaMapKey merely to construct/iterate a
values view: equality capabilities belong to the operation that invokes them.
Callback-based comparison/compute must release implementation-only map loans
before Java callbacks and account for structural edits; an eager copied Vec or
a held RefCell/Mutex guard is not an adequate general replacement.

### Live values / putIfAbsent batch (in progress)

The backing relation now carries its selected map child index, rather than
implicitly assuming key child zero. Root retention stays separate; qualified
nullability maps map child one to values-view element zero bidirectionally,
without joining key and value domains. The native allocation graph uses the
same indexed relation and existing dependency edges. Expression/value-slot
projection and foreach child inference preserve that solved storage carrier.

Local/sync value projections reuse the live collection runtime and checked
cursor protocol. Construction/iteration require no value equality or key Clone
bound. Search receives its operation's equality witness, copies canonical value
references under a short map loan, and calls equality after releasing it.
Removal checks for backing structural mutation before consuming a found index;
duplicate/null values remove one mapping, not every equal value. Existing
unmodifiable policy is reused. Generic callable capabilities select Clone on
the values binder and Any on captured binders through the existing reachability
graph, rather than adding all-purpose JavaMapKey bounds.

putIfAbsent uses the same canonical map payload, parameter staging and child
copy/null protocols. The runtime takes a null predicate and reference copier;
replacement of a stored null retains its key and modification counter. Missing
insertion is structural. The runtime method itself needs neither K: Clone nor
V: Clone. Source generic clone capabilities belong to its copied value binder.

Combined build **16538** passes in **50.27s**,
`.tmp/jgrapht-map-values-build-v1.log`. Independent
`RetainedMapValuesOracle` returns zero. Runtime kernel gate **2596** passes all
14 generic carrier tests (**11.70s**), including non-Clone keys, value types
without equality traits, local/sync backings and cross-thread insertion, null
and duplicate removal, putIfAbsent structural counters and comparison reentry
into the map producing ConcurrentModificationException rather than RefCell
failure. All 143 signature tests and the previously pending scoped key-alias
assertion pass. Three prior map/query integrations pass.

The new source map-values integration fails compilation with five errors:
source generic putIfAbsent forwarding loses nullable return evidence, and
iterator identity comparisons lose consumed-Result metadata inside logical
operators. Current candidate generalizes source nullable-map-child return
classification (get/remove/put/putIfAbsent, preserving parsed source precedence)
and marks pure operator comparisons as values after consuming operand Results,
without marking custom Java equals calls that may introduce their own Result.
New focused units check qualified value-view nullability and that identity
comparison propagation remains idempotent. Combined build **84805**,
`.tmp/jgrapht-map-values-build-v2.log`, passes (**48.92s**). All **144** signature
tests pass, including the new qualified values-view check; all three identity
integrations pass. The focused comparison unit is in equality_symmetry_tests
(the initial operator_lowering::tests filter selected zero tests and is not
evidence); the correct module was subsequently run and collected successfully.
The source map-values integration is down to one compiler error: a legacy
native-call wrapper replaces the operation's physical Option<V> metadata with
Option<Option<V>>, then emits map(|value| value.expect(...)). Native slot calls
now honor the producer's recorded carrier rather than re-deriving the legacy
syntactic type. An added generated API control instantiates the source generic
values forwarding function with a non-Clone key and an equality-less value,
guarding against accidental broad bounds beyond runtime mechanism controls.
Combined build **81404**, `.tmp/jgrapht-map-values-build-v3.log`, passes
(48.62s). Source integration compilation then reaches only the new Rust harness
control: its deliberately non-Clone key also lacked JavaMapKey, which the source
Map<K,V> API already requires. The control now supplies key equality but still
does not implement Clone; its value still supplies no equality witness. This
tests the intended key-copy/value-equality obligations, not absence of the
existing intrinsic key capability. All source Java behavior remains unchanged.

Broad library **36584** is collected terminal one: **1,566 pass, two fail**
(72.46s), `.tmp/jgrapht-map-values-library-v3.log`. Both failures assert legacy
raw-map spelling for immediate values-view remove/clear. Their replacements
inspect only source owner implementations and require live value factories,
guarded projection removal/clear and guarded length, forbidding raw source map
or values-view mutations. No other translator changes follow V3. Combined
build **94815** passes (49.76s), `.tmp/jgrapht-map-values-build-v4.log`.
Runtime **9255** is collected terminal zero: **all four map/query integrations
pass** (15.89s), including the live values Java-runtime oracle and generic API
bound control. Alias/generic-borrow controls **8076** are collected terminal
zero: **nine values-alias unit tests** pass (9.89s), including both corrected
source-scoped assertions, and the **generic-borrow integration** passes
(11.57s). Logs `.tmp/jgrapht-map-values-{runtime,alias-units,borrows}-v4.log`.
Combined with the unchanged production code's 1,566 passing broad library
results and the corrected assertion reruns, this clears the batch for corpus
measurement; it is not a claim of a fresh all-library invocation after V4's
test-only edits. No source edits or other tests are planned during measurement.

Release build **99095** is collected terminal zero (**1m32s**), log
`.tmp/jgrapht-v129-map-value-contracts-build.log`. Frozen binary
`.tmp/new-java2rust-v129-map-value-contracts`, **21,557,512 bytes**, SHA-256
`d4f179bd977d9392a22f69c78af8c151ad6052e31dabb854445d84567438e815`.
Measurement **90273** is running; collect this exact handle before starting
another run. Same untouched 104-file runner, explicit release profile and reused
resolver bench evidence (`--skip-resolver-bench`). Root
`.tmp/jgrapht-core-1.5.3-focused-v129-map-value-contracts`, log
`.tmp/jgrapht-v129-map-value-contracts-run.log`. No new corpus result is claimed.

Remaining limits are not erased by these controls: erased Box<dyn Any> values
cannot use Clone-based value iteration without a generalized reference-copy
carrier; Collection identity-only equals/hash semantics are not Set semantics;
key-equality and same-view callback reentrancy need whole-path treatment; sorted
map null/comparator contracts, computeIfAbsent and nested Pair key-carrier
disagreements remain outside this implemented portion. Do not replace a live
values view with a construction-time snapshot or hand-repair generated Rust.

### V129 measurement and nested storage-contract follow-through

Frozen measurement **90273** is collected terminal one (generated Rust cargo
failure). All **104** files translate; translator and generated parse failures
are zero. Diagnostics **1,324 = 1,242 typed + 82 untyped**, versus V128's 1,336.
Exact diagnostic-header/first-generated-file comparison removes **17** and
adds **5**, net -12, not twelve independently solved bugs. Allocation guards
remain **22**; markers fall **85/39 files -> 80/38 files**. Comparison
`.tmp/jgrapht-v128-v129-diagnostic-delta.txt`. The five unsupported retained-map
operation errors disappear, as do the payload-based identity and &&V query
errors. New headers expose existing unsatisfied key capabilities for two
putIfAbsent calls, two remaining nested Pair query mismatches, and one boxed
numeric field update. No corpus acceptance, graph-oracle pass or expansion.

Total **256.306s**, translation **173.110s**, assembly **63.588s**, cargo check
**15.392s**, sweep **3.531s**. Global analysis **48.618s**, signatures **23.319s**
(collect **13.080s**, fixpoint **4.318s**, plan **5.667s**), emission **110.506s**.
Unpaired timings are not a speedup claim or a reason to reopen micro-optimization.

The next minimized fixture `nested_map_contracts.java` combines a generic
factory-created set, map storage, a source return alias and later null-element
insertion, with a generic source key hierarchy. Its independent Java oracle
returns zero. Frozen/current-pre-follow-up debug output has six compiler errors:
the returned set has a different element carrier from its consumers, null
queries then ask for Default on a non-defaultable source object, and a generic
source factory misses two Clone witnesses when copying borrowed parameters to
owned constructor storage. The default errors are **not** evidence for adding
Default to arbitrary Java classes; a manual generic Default mechanism already
exists and is unrelated. This minimized case does **not yet reproduce** the
separate original Pair<Option<V>,V> key disagreement; keep those claims distinct.

The audit identified a graph gap: whole-container reference-transfer edges
propagate allocation identity but were excluded from nullable-element transfer.
Projected storage also discarded its declaring root type, so sequence-depth
qualifiers could not distinguish map value child one from key child zero.
Usage slots now preserve root type through projections. The existing graph
distinguishes directional value flow, proven container alias and map-view
mapping. Aliases transfer descendant storage qualifiers bidirectionally via
structural paths, never outer binding nullability; copied/filtered scalar value
flows stay directional. No identity-breaking fresh-container adapters are added.

Build **38222** passes (**50.40s**),
`.tmp/jgrapht-nested-storage-build-v1.log`. All **144** signature units, **76**
usage units and **four** prior map/query integrations pass. Inspector linked to
that debug library shows five fixpoint iterations, no diagnostics, consistent
nullable-element factory/map/return carriers, and only **two** remaining compiler
errors (missing AMethod/BMethod Clone). Read-only artifacts:
`.tmp/nested-contracts-alias-v1-{plan-and-source.log,diagnostics.log,rs,rustc.log}`.

The current candidate reuses solved caller/constructor parameter shapes and the
existing Rust carrier Clone analysis. A borrowed generic parameter copied into
an owned constructor slot requires its witness; Rc/Arc carrier shielding avoids
inventing payload Clone bounds. A maintained fifth integration now runs the
full nested-map fixture. Build **83699** found one omitted argument at the older
class-level capability walk; that call retains its prior context-free analysis.
Combined build **68312**, `.tmp/jgrapht-nested-storage-build-v3.log`, passes
(50.04s). All **five map/query integrations** pass (38.10s under concurrent
gates), including the full nested-map Java-runtime oracle. All **three identity
integrations** pass (36.38s). Collected sessions **30998** and **93264**; logs
`.tmp/jgrapht-nested-storage-{runtime,identity}-v3.log`. Broad library **60080**,
`.tmp/jgrapht-nested-storage-library-v3.log`, is collected terminal zero:
**all 1,568 library tests pass** (74.98s). No other tests remain live.
No fresh corpus measurement or resolution of the unreproduced original Pair key
issue is claimed.

Release build **31464** is collected terminal zero (**1m32s**), log
`.tmp/jgrapht-v130-shared-storage-domains-build.log`. Frozen candidate
`.tmp/new-java2rust-v130-shared-storage-domains`, **21,669,496 bytes**, SHA-256
`5013ec829ef6cdcfd46adc6d9400df58ddb36b4d2689ab694ef40f206fb95c0f`.
Measurement **29939** is collected terminal one (generated Rust cargo failure).
No translator edits or competing tests during build/freeze/measurement. Same
untouched 104-file runner, explicit release profile, `--skip-resolver-bench`.
Root `.tmp/jgrapht-core-1.5.3-focused-v130-shared-storage-domains`, run log
`.tmp/jgrapht-v130-shared-storage-domains-run.log`. Record binary hash/size and
compare error identities, markers, allocation guards and timings against V129.

V130 finishes with all **104** files translated and zero translator/parse
failures. Cargo diagnostics **1,286 = 1,203 typed + 83 untyped**, versus 1,324:
exact header/first-generated-file comparison removes **57**, adds **19** (net
-38, not distinct bug counts). Report `.tmp/jgrapht-v129-v130-diagnostic-delta.txt`.
Markers **81 in 39 files**, allocation guards **23**, versus 80/38 and 22.
Total **253.009s**, translation **170.207s**, assembly **63.018s**, cargo check
**15.543s**, sweep **3.546s**. Global analysis **47.934s**, signatures
**22.785s** (collect 12.818, fixpoint 4.276, plan 5.458), emission **108.327s**.
No speedup claim from unpaired timings and no corpus acceptance.

Most removals are map/factory/return carrier mismatches. New guard in
TreeSingleSourcePathsImpl.getDistanceAndPredecessorMap is a real missing
retained `Collections.unmodifiableMap` contract: returning a copied map would
lose live backing and read-only policy. Keep the guard until a general map
view representation supports both. WeakChordalityInspector also exposes a
separate argument conversion issue: a nullable boxed Integer used as a retained
list index is unboxed, then treated as optional again (`i32.as_ref()`). Trace
the produced carrier through shaped arguments and liveness before changing it.

### V130 follow-through: completed Copy-value argument conversions

Maintained `boxed_collection_indices.java` reproduces four `i32.as_ref()`
errors before the fix (log `.tmp/jgrapht-boxed-indices-before.log`). Its
independent Java execution returns zero. It exercises retained list set,
insert, read and cursor indices, null-index exceptions and a reused boxed
argument passed to an ordinary source method. The IR trace confirms the
primitive unbox producer already records its completed CopyValue slot;
`adapt_reused_owned_argument` recognizes that match but still chooses
CloneOptionalBeforeConsumingUnwrap from the old source shape, appending a
second optional projection. The pass now preserves completed CopyValue
conversions, while keeping ownership recovery for non-Copy reference results.
Temporary tracing was removed. Combined no-run build **88019**, log
`.tmp/jgrapht-boxed-indices-build.log`, is collected terminal zero (48.82s).
All **three retained-list integrations** pass (33.28s), including the new
Java-matched runtime regression; **338 lowering units** pass (54.15s), all
**five map/query integrations** pass (44.24s), and all **three identity
integrations** pass (38.33s). Collected sessions 62031, 3574, 43954 and 55433;
logs `.tmp/jgrapht-boxed-indices-{runtime,lowering,map,identity}.log`.
These concurrent gate timings are not performance measurements. No full
corpus rerun after this bounded follow-up; V130 remains the measured baseline.

Next coherent capability batch: source dispatch/forwarding must propagate the
capabilities of the emitted ABI, not only operations found in a declaration's
own body. V130 still shows BellmanFord.getPathWeight calling getPath without
its V identity/key witnesses, and Graph/ShortestPathAlgorithm companion impls
calling concrete methods without their identity witnesses (including AsSubgraph).
Reuse the finite callable-capability traversal and existing inherited/interface
implementation selection and binder substitution machinery. Minimize these
cases before editing; do not add blanket bounds to entire generic impls.
The missing live read-only map wrapper and original nested Pair key carrier
remain separate unresolved contracts, not fixed by this argument batch.

### Source dispatch and identity-capability batch (in progress)

The maintained `inherited_dispatch_capabilities.java` combines a renamed
superclass binder A -> child T, an inherited body calling an override, a
generic caller of that inherited body, a generic concrete-to-interface return,
and an unrelated count-only interface. Java returns zero. Runtime control
requires the count-only interface for a payload with neither Clone nor the
identity protocol, preventing indiscriminate adapter bounds.

The capability worklist now selects the emitted receiver method. Inherited
bodies use the existing binder-aware specialization, in the receiver's
declaration frame (not repeatedly expanded call-site types). Direct declarations
remain borrowed; visited owner/method/binder keys keep recursive traversal
finite. The same selection feeds identity-carrier discovery. Reference identity
is now a structured trait-implementation capability alongside Clone/key equality,
reusing selected interface contracts and conversion propagation. Forwarding
requirements are scoped to the selected interface; non-Copy ownership recovery
is unchanged.

Cheap library check **25501** passes (19.64s). Combined build **62593** passes
(49.93s); all **1,568 library tests**, **five map/query integrations** and
**three retained-list integrations** pass. Three prior identity integrations
pass, but the new runtime fixture fails with two nonexistent `same_t_t` calls.
Logs `.tmp/jgrapht-dispatch-capabilities-{check,build-v1,library-v1,map-v1,list-v1,identity-v1}.log`.
The first abstract-base draft additionally exposed abstract-carrier issues;
the maintained concrete-base fixture isolates this dispatch contract without
claiming those separate abstract-carrier failures solved. Frozen V130 applied
to the maintained fixture yields four compiler errors: two bad call names and
two missing adapter identity witnesses; read-only artifacts
`.tmp/jgrapht-dispatch-capabilities-before{.rs,-rustc.log,-translate.log}`.

Root cause of the remaining names: inherited-slot deduplication compares
Parent<A>.same(A,A) against Child<T>.same(T,T) without rebasing. It invents a
second overload, then lowering invents an un-emitted overload name. The shared
callable-slot key now accepts owner substitutions with method-binder shadowing;
class and interface inheritance use the existing ancestor projection before
deduplication. A focused unit covers renamed and concrete class arguments,
retained real overloads and a redeclared generic default method. Combined
build **43107**, `.tmp/jgrapht-dispatch-capabilities-build-v2.log`, is collected
terminal zero (48.90s). All **1,569 library tests** pass (79.95s), all **four
identity integrations** pass (38.42s), all **five map/query integrations** pass
(44.04s), and all **three retained-list integrations** pass (39.70s).
Collected sessions 90537, 49346, 37449 and 55205; logs
`.tmp/jgrapht-dispatch-capabilities-{library,identity,map,list}-v2.log`.
Formatting checks pass. These concurrent gate timings are not benchmarks.
No fresh corpus measurement or acceptance of this candidate yet.

Release build/freeze **73277** is collected terminal zero (1m32s).
Log `.tmp/jgrapht-v131-dispatch-capabilities-build.log`; on success the command
copies the binary to `.tmp/new-java2rust-v131-dispatch-capabilities`, writes
`.tmp/jgrapht-v131-dispatch-capabilities-binary.sha256` and reports byte size.
No translator source edits or competing tests during build/freeze/measurement.
Next run the same untouched 104-file runner with explicit release profile and
`--skip-resolver-bench`, output root
`.tmp/jgrapht-core-1.5.3-focused-v131-dispatch-capabilities`. Compare identities,
markers, allocation guards and phase timings against V130 before choosing the
next root-cause batch. The previous bounded boxed-argument follow-up is included.
Frozen V131 is **21,674,832 bytes**, SHA-256
`2e1c63ec934c55bbcb1ba3537922fde3f4db32c2971696aef9d2c0c3cfd82bad`.
Measurement **39576** is collected terminal one (generated Rust cargo failure).
Run log `.tmp/jgrapht-v131-dispatch-capabilities-run.log`. Same untouched
104-file runner, caller-declared release profile and `--skip-resolver-bench`.

### V131 measurement: dispatch gains and measured analysis regression

All **104** files translate, with **zero translator and generated parse
failures**. Cargo diagnostics **1,251 = 1,168 typed + 83 untyped**, versus
V130's 1,286. Exact header/first-generated-file comparison removes **39**, adds
**four** (net -35, not distinct bug counts), report
`.tmp/jgrapht-v130-v131-diagnostic-delta.txt`. Removed: 18 V key witnesses,
five E key witnesses, 12 V identity witnesses and all four i32.as_ref errors
in WeakChordalityInspector. Markers remain **81 in 39 files**; allocation
guards remain **23**. No corpus acceptance or graph-oracle pass.

The four additions are missing V identity requirements at concrete AsSubgraph
to Graph conversions inside collection insertions: one each in
AbstractStrongConnectivityInspector and KosarajuStrongConnectivityInspector,
two in BiconnectivityInspector. They are not new adapter-body failures: the
adapter now has the correct requirement, but its collection-element consumer
does not propagate it. `callable_source_interface_conversion_requires_capability`
currently examines resolved formal argument types, which can erase a native
collection's element to Object. Follow-through must reuse native method child
argument metadata and solved/declared storage types, excluding co-translated
source lookalikes, rather than placing blanket bounds on graph classes.

Total **352.249s**, translation **271.281s**, assembly **63.332s**, cargo check
**13.431s**, sweep **3.532s**. Global analysis **145.701s**, semantic
**78.779s** (V130 14.440), representation **38.095s** (6.783), signatures
**24.920s** (22.785; collect13.918, fixpoint4.310, plan6.459), emission
**111.906s** (108.327). Signature fixpoint remains13 iterations, zero plan
diagnostics. Assembly/emission are roughly unchanged; this is evidence for a
bounded analysis follow-through, not open-ended micro-optimization.

Both slow phases call `source_call_reference_identity_types` repeatedly,
constructing a fresh capability traversal for every source invocation. The
newly complete dispatch/conversion reachability amplifies that repeated work.
Next batch should pair collection-destination capability propagation with an
explicit immutable-program-scoped capability index/cache reused by semantic
and storage analysis. Cache completed reachability results, never provisional
false answers on recursive edges; include receiver/declaration-frame identity
and binder shadowing. Avoid pointer-keyed cross-program/global state or cached
answers that survive a changed program/solved storage context. Measure the
bounded follow-up against frozen V131 after focused parity/regression gates.
All build/test/measurement handles are terminal and collected. No translator
source edits were made during the frozen measurement.

### Collection destinations and program-scoped identity index

The expanded maintained inherited-dispatch fixture adds generic List and Map
insertions of the concrete implementor into interface-typed storage. Independent
Java execution returns zero. V131 output reproduces two missing T identity
witnesses; artifacts `.tmp/jgrapht-collection-capabilities-before-v2{.rs,-rustc.log,-translate.log}`.
The first diagnostic draft also called a mutating interface method through an
immutable collection read and exposed a separate mutable-interface loan issue
(`-before.rs`, `-before-rustc.log`). The maintained insertion control reads the
interface counter instead, observes an original-alias mutation through both
stored references, and keeps existing direct mutable-interface calls. This
isolates insertion conversion without claiming that distinct mutable-read
boundary fixed.

`callable_source_interface_conversion_requires_capability` now uses native
method stored-argument child metadata for insertion destinations, not just
JDK-erased resolved formals. Original source-interface conversion and selected
implementation logic own the witness analysis. Co-translated source receiver
types are excluded; the new source List lookalike negative control passes.

`SourceReferenceIdentityRequirements` owns completed binder requirements for
one borrowed immutable Program. Keys include receiver/declaration frame,
selected method and declaration span. Call-site substitutions happen after the
lookup, so Node and String instantiations share work without sharing answers.
Only completed finite-reachability queries are cached; recursive active false
answers are not. Semantic string planning reuses one index for its walk, and
storage collection uses an explicit context with its own index. No global or
pointer-keyed cross-program state, and no solved storage-dependent result is
stored in this source-only index. Direct/renamed inherited receiver frames,
recursive calls, repeated hits, distinct instantiations, separate programs and
source collection lookalikes have focused unit coverage.

Cheap library check **29505** passes (19.53s). Combined build **79495** passes
(47.93s), `.tmp/jgrapht-capability-index-build-v1.log`. All **43 assembly
units** pass (1.59s), including the new controls. All **four identity
integrations** pass (14.78s), all **five map/query integrations** pass (44.90s),
and all **three retained-list integrations** pass (42.76s). Broad library
**21557** passes **all 1,571 tests** (74.68s). Collected handles 11960, 17364,
94578, 50954 and 21557; logs
`.tmp/jgrapht-capability-index-{units,runtime,map,list,library}-v1.log`.
These gate timings are not corpus performance evidence. No fresh full corpus
measurement yet; V131 remains the measured baseline (1,251 diagnostics).

Release build/freeze **69374** is collected terminal zero (1m32s).
Log `.tmp/jgrapht-v132-capability-index-build.log`. On success it copies to
`.tmp/new-java2rust-v132-capability-index`, writes
`.tmp/jgrapht-v132-capability-index-binary.sha256` and reports bytes. Keep
translator source frozen and no competing tests during build/measurement.
Next use the same pinned untouched 104-file runner, explicit release profile,
`--skip-resolver-bench`, root
`.tmp/jgrapht-core-1.5.3-focused-v132-capability-index`. Compare exact diagnostic
identities, markers, guards and semantic/representation timings against V131.
Frozen binary **21,678,136 bytes**, SHA-256
`5740968354ea66ac175e45a9938837520d847de011a967bbe7c735581ae35276`.
Measurement **96466** is collected terminal one (generated Rust cargo failure).
Log `.tmp/jgrapht-v132-capability-index-run.log`. Explicit release profile,
same untouched 104-file slice, `--skip-resolver-bench` only.

### V132 result and bounded demand-filter follow-through

All **104** files translate with **zero translator/parse failures**. Cargo
diagnostics **1,247 = 1,164 typed + 83 untyped**. Comparison with V131 removes
exactly the four newly exposed insertion identity-witness errors and adds
**zero**, report `.tmp/jgrapht-v131-v132-diagnostic-delta.txt`. Markers remain
**81 in 39 files**, allocation guards **23**. No full-corpus acceptance or
graph-oracle success is claimed.

Total **308.898s**, translation **227.869s**, assembly **63.487s**, cargo check
**13.297s**, sweep **3.564s**. Global analysis **104.200s**, semantic **51.535s**
(V131 78.779), representation **24.923s** (38.095), signatures **23.867s**
(24.920; collect13.273, fixpoint4.322, plan6.037), emission **110.296s**
(111.906). The scoped index recovers part of the measured regression but leaves
semantic/representation well above V130's 14.440/6.783 seconds.

Inspection identifies an exact discarded-work boundary, rather than a reason
for open-ended micro-optimization. Semantic identity planning retains only
String results; storage retains only types with declared source storage names.
Both previously classified all binders, including V/E call-site substitutions
that cannot contribute anything to those consumers. The candidate now checks
caller demand against the call's substitution values before reachability, then
filters the returned types with that same predicate. A declaration cache entry
still contains its full completed binder requirements, never demand-partial
answers. Logical invariant: every output type must come from those substitution
values, so an empty relevant subset proves the consumer's result empty without
querying a body. String planning also stops source-call queries once its
monotone representation flag is already true.

New unit compares demanded queries with filtering the full query, including
an irrelevant Node-to-String demand that must not create a body-cache entry,
then Node/String requests sharing a declaration frame. Prior recursion,
receiver-frame, instantiation and cross-program controls are retained.
Combined no-run build **84720** is collected terminal zero (48.32s), log
`.tmp/jgrapht-identity-demand-build.log`. All **44 assembly**, **64 storage**
and **seven semantic** units pass (1.58s/0.21s/0.42s), all **four identity
integrations** pass (16.17s), all **five map/query integrations** pass (17.43s),
and all **three retained-list integrations** pass (15.88s). Collected handles
20951, 40767, 7946, 53704 (semantic/storage commands ended immediately).
Logs `.tmp/jgrapht-identity-demand-{assembly,storage,semantic,runtime,map,list}.log`.
Formatting checks pass. No full library rerun
for this narrow discarded-result filter; the V132 1,571-test gate remains prior
evidence, not proof for the changed consumers. A fresh frozen measurement is
required for diagnostic parity and the remaining runtime question.

Release build/freeze **87680** is collected terminal zero (1m32s).
Log `.tmp/jgrapht-v133-identity-demands-build.log`. On success it copies to
`.tmp/new-java2rust-v133-identity-demands`, writes
`.tmp/jgrapht-v133-identity-demands-binary.sha256` and reports bytes. No translator
edits or competing tests during build/freeze/measurement. Next use the same
untouched 104-file runner, explicit release profile and `--skip-resolver-bench`,
root `.tmp/jgrapht-core-1.5.3-focused-v133-identity-demands`. Require diagnostic
identity/marker/guard parity against V132 and inspect semantic/representation
timings before considering the bounded speed follow-through finished.
Frozen V133 is **21,656,232 bytes**, SHA-256
`964185a4b34740b24621585944686f7a06f94978f7de9a57a3421b0e32ef6e57`.
Measurement **7292** is collected terminal one (generated Rust cargo failure).
Log `.tmp/jgrapht-v133-identity-demands-run.log`, same untouched 104-file runner,
explicit release profile, `--skip-resolver-bench`. Also compare generated source
bytes against V132: this demand filter is intended to change work, not output.

### V133 result: bounded performance follow-through finished

All **104** files translate with zero translator/generated parse failures.
Diagnostics remain **1,247 = 1,164 typed + 83 untyped**, with **zero exact
diagnostic additions/removals**, markers **81 in 39 files**, allocation guards
**23**. Entire assembled `generated-crate/src` trees are **byte-for-byte
identical** to V132 (`diff -qr` is empty), not merely equal error totals.
Reports `.tmp/jgrapht-v132-v133-{diagnostic-delta,generated-source-diff}.txt`.

Total **251.291s** (V132 308.898), translation **170.341s** (227.869), assembly
**63.445s**, check **13.244s**, sweep **3.576s**. Global analysis **44.047s**,
semantic **10.578s** (51.535), representation **4.818s** (24.923), signatures
**24.730s** (collect13.867, fixpoint4.332, plan6.298), emission **112.883s**.
The targeted regression is removed; whole-run timing is back around V130's
253.009s despite the dispatch improvements. Close this bounded speed work and
return to translation contracts, not further micro-optimization. No corpus
acceptance, graph-oracle pass, scope expansion or goal completion.

### Next rewrite audit: native-container reference/value comparison contracts

Maintained `native_collection_identity.java` has an independent Java result
zero. Frozen V133 output fails with **12 E0369 errors**, artifacts
`.tmp/jgrapht-native-identity-before{.rs,-rustc.log,-translate.log}`. It covers
Set/List/Map aliases versus equal-but-distinct containers, nullable operands,
generic helpers, enhanced-for borrowed children, map reads and inline fresh
allocations. Both reference comparison and explicit collection value-equals
controls are included, so pointer equality cannot accidentally replace equals.

Concrete evidence: `sameSet<T>` receives a retained left parameter but a plain
right parameter. A retained Set/List is compared through Rust PartialEq, which
asks for payload equality rather than allocation identity. The generic/erased
identity lowering already uses the reusable JavaReferenceIdentity protocol,
but its gate excludes ordinary native retained allocations. Usage's Eq/Ne
branch records null-index evidence, not allocation retention. Inline
`new HashSet<String>() == new HashSet<String>()` emits two plain HashSets and
structural equality, a latent wrong result even after compiler errors disappear.
Explicit collection `.equals` also reaches raw Rust comparison on mixed
plain/retained carriers and needs its separate value-equality route.

Batch the whole path: seed retention for observable native identity operands
without equating unrelated child nullability domains; reuse canonical handle
borrowing and the existing identity protocol for retained operands; materialize
fresh inline allocations through the existing retained factory/slot mechanism
(typed temporary IR is available if inference/lifetimes require it). Do not
add payload PartialEq/Clone bounds or fake structural equality to silence
errors. Keep `.equals` distinct, reusing native JavaMapKey collection equality
where its contract applies; preserve null receiver checks and Java operand/
argument evaluation order with existing staging helpers. The runtime already
implements JavaMapKey for native collection storage and its local Rc carrier
without a blanket T: Clone requirement. Existing factory policy, source-owner
precedence, sync placement, nullable slots and no-payload-capability controls
must survive. This is implementable with current slots, factories and IR;
no need to hand-repair generated output or broaden corpus scope.
All prior process handles are terminal/collected; no translator edits since
the V133 freeze. The new Java fixture is diagnostic input, not a passing Rust
integration yet.

### Native comparison batch: complete-path implementation and focused gate

The new batch seeds native allocation retention at observable comparisons,
without aliasing the operands' independent child nullability domains. Reference
comparisons reuse `JavaReferenceIdentity`; fresh allocations reuse retained
construction/factory lowering. List/Set/Map value equality remains a separate
`JavaMapKey` operation, staged after argument evaluation and receiver null
checks. `Objects.equals` uses the existing native static-call planner, including
source-owner precedence, rather than a second name-matching implementation.

The minimized fixture exposed two additional contracts along this path:
plain-map reads must preserve their retained child's allocation metadata and
record already-flattened optional projections; generic collection value
equality must propagate `JavaMapKey` through unresolved platform static calls.
A null-only generic instantiation then exposed the missing `Box<dyn Any>` key
bridge. It now delegates to the existing erased-object equality kernel;
nested Rust boxes are transparent carriers. This does not add source-class
equals dispatch to that kernel: unsupported erased targets still fail explicitly,
and a regression verifies that the guard remains. No payload Clone/PartialEq
requirements or generated-code repairs were introduced.

Build V7, handle **82186**, is collected terminal zero, **49.59s**; formatting
check **67323** is collected zero. Logs use
`.tmp/jgrapht-native-identity-{build,runtime,library,map,list}-v7.log`.
Focused integration **80296**, library **83892**, map **48445**, and list
**60911** are running the freshly built binaries. The fixture tests alias versus
equal-but-distinct containers, nullable roots and elements, generic calls,
enhanced-for children, map reads, inline allocations, and argument side effects
before a null-receiver exception. Runtime controls require no payload traits
for identity, and check erased primitive/string equality, canonical NaNs,
signed zero, nested boxes and unsupported-target rejection.

Do not infer corpus acceptance from these focused gates. Once they finish,
freeze the candidate and compare the untouched 104-file slice against V133.

All V7 gates are now collected terminal zero: **1,573 library tests**
(79.68s), **5 identity integrations** (46.12s), **5 map integrations**
(45.00s), **3 list integrations** (42.05s). The unchanged Java fixture, compiled
with `javac` under `.tmp/java-native-identity-oracle`, independently returns
**zero** (handle **5620**, collected zero). Identity integration includes
the new fixture and runtime controls, not merely Rust compilation.

Source is frozen for V134 release build and subsequent isolated measurement.
Producer `.tmp/new-java2rust-v134-native-comparisons`, build log
`.tmp/jgrapht-v134-native-comparisons-build.log`; compare output root
`.tmp/jgrapht-core-1.5.3-focused-v134-native-comparisons` to V133 after the
untouched pinned 104-file runner finishes. Do not run competing tests or edit
translator sources during the build/measurement. The selected corpus and
full goal remain incomplete.

V134 release build **40439** is collected terminal zero, **1m31s**.
Frozen producer is **21,731,184 bytes**, SHA-256
`6207e09c521d7b9cc1bab36ba9f74f643b5aa2bd652f6805fd2908a3fce2801f`.
Untouched focused runner **34455** is live, explicit release profile,
`--skip-resolver-bench`, log `.tmp/jgrapht-v134-native-comparisons-run.log`.
Collect this exact handle; do not restart the runner or build another producer
just because observation yields no output. No competing tests or product edits
during this measurement.

### V134 measured result and next whole-path audit

Runner **34455** is collected terminal one (generated cargo failure). All
**104** selected files translate, with **zero translator or generated parse
failures**. Diagnostics are **1,225 = 1,142 typed + 83 untyped**, markers
**81 in 39 files**, allocation guards **23**. Exact diagnostic comparison
`.tmp/jgrapht-v133-v134-diagnostic-delta.txt` removes **23** and adds **one**:
17 invalid structural collection reference comparisons, five mismatches, and
one moved `reachable` error disappear. The addition is a mixed-carrier
`containsAll` source error in `GraphTests`; it must not be hidden by the net
reduction or called a passing corpus gate.

Total **250.576s**, translation **169.462s**, assembly **63.370s**, generated
cargo check **13.483s**, idiom sweep **3.577s**. Global analysis **43.724s**,
semantic **10.464s**, representation **4.871s**, emission **112.238s**. Timing
is stable versus V133, so there is no new evidence for an optimization phase.

The new diagnostic is at generated `00005_GraphTests.rs:266`:
`degree5.containsAll(reachable)` has a plain semantic set receiver and a retained
native collection argument. The raw bulk lowering stages both expressions but
passes `&Rc<RefCell<JavaNativeCollection<...>>>` to a plain `.iter().all(...)`
consumer. This is the same incomplete producer/consumer contract that the new
retention evidence exposes, not a reason to undo retention. Existing AddAll
already has an explicit mixed-carrier path; RemoveAll uses projected live
membership helpers. ContainsAll and RetainAll still fall through the plain
consumer path.

Next coherent audit/batch: mixed-carrier read/membership bulk operations. Reuse
the existing checked live iterator and projected equality/nullability mechanisms
across plain/retained receivers and sources. Do not substitute a payload guard
held across equality callbacks, an eager snapshot with changed fail-fast
behavior, or a direct generated `.borrow().iter()` repair. Minimize cases for
containsAll/retainAll, nullable source/receiver evaluation order, source aliasing
and callback mutation before changing the lowering. Extend the existing bulk
mechanism rather than inventing a second native carrier ABI. Full corpus and
graph behavior gates remain incomplete; no scope expansion or goal completion.

The maintained `tests/fixtures/mixed_bulk_membership.java` now minimizes this
next contract independently of JGraphT. Java returns **zero**, including
containsAll/retainAll/addAll/removeAll results, source-alias preservation,
nullable sources, and argument side effects before a null receiver throws.
Frozen V134 output fails direct rustc with exactly **six E0599** diagnostics:
three `.iter()` and three `.contains()` calls on retained handles. There are no
additional generic/ownership failures in this fixture. Artifacts:
`.tmp/mixed-bulk-membership-v134{.rs,-translate.log,-rustc.log}`; direct rustc
handle **91898** is collected terminal one. This is a diagnostic fixture, not
a passing Rust integration yet. AddAll/RemoveAll controls compile already;
reuse their carrier staging and live membership machinery for the missing
ContainsAll/RetainAll paths. All build/measurement/test handles from this
checkpoint are terminal and collected; no release build or corpus runner is
left running.

### Mixed-carrier bulk membership implementation (in progress)

Generalized the retained-source RemoveAll lowering into a membership bulk
path for ContainsAll/RetainAll as well. RetainAll and RemoveAll share checked
filter kernels (native cursor or plain retain callback); ContainsAll consumes
the live source cursor and performs projected membership, not snapshot iteration
or a guard held across Java equality. Nullability projection and equality
direction follow query-versus-stored element roles. Clone/lifetime requirements
reuse solved retained-iteration shapes; semantic equality capability propagation
uses the existing native call planner and preserves co-translated Set precedence.
Keep the pre-existing RemoveAll fallback for ineligible carriers.

Build V1 **55487** is collected zero, **49.62s**; **45 assembly units** pass
(**52012**, collected zero). The new runtime integration **31000** is collected
failed: the initial six handle-method errors are replaced by three immediate
closure-invocation precedence errors and one nullable receiver `?` incorrectly
embedded inside retain's non-fallible callback. Fix both contracts generally:
closures have low expression precedence in the emitter; checked receiver/source
projections are staged before callback construction, including empty collections.
No generated repairs or weakened assertions.

The expanded Java fixture still independently returns zero; it now includes
generic helper signatures, retained-to-retained ContainsAll and independent
nullable element domains. New runtime controls cover nonstructural source
updates during iteration, structural invalidation, false-result short circuit,
partial retain progress on checked failure, self aliasing, and fixed-size
mutation rejection. Build V2 **35780** is live; log family
`.tmp/jgrapht-mixed-bulk-{build,runtime,assembly}-vN.log`. Collect this handle,
then test the focused integration before a broad gate/freeze. V134 remains the
last measured corpus result; no acceptance or completion is claimed.

V2 build **35780** is collected zero, **49.46s**. Its broad library gate
**28046** passes **1,575 tests** (80.63s); identity/map/list gates **79233**,
**92541**, **18066** pass 5/5/3. Focused mixed gate **98127** is collected
failed with one nullable-element E0308: immediately calling an untyped match
closure inferred `Option<&String>` rather than its actual `&Option<String>`
operand. Generalized the shared equality builder to expose a body projection
usable inside already-typed callbacks, without an extra inference boundary.
The emitter precedence fix remains covered independently.

Complete the carrier matrix in the same batch: a retained receiver with a plain
source now uses borrowed plain iteration plus checked live receiver membership
or mutation. A plain receiver with retained source and two retained operands
use the corresponding live-source/native cursor paths. Retain/Remove share a
native filtering primitive accepting checked membership; plain filtering keeps
the existing partial-progress kernel. Clone/lifetime obligations apply only to
the retained sides being iterated, not the plain collection payload merely
because another operand is retained. The fixture includes both directions and
all four bulk operation controls; expanded independent Java still returns zero.

V3 build **50836** is collected zero, **49.49s**. New live verification handles:
mixed **97632**, library **68239**, identity **27121**, map **42864**, list
**10598**; logs `.tmp/jgrapht-mixed-bulk-*-v3.log`. All older handles are
collected. Wait for these gates before freezing a release candidate. No new
corpus measurement or full-goal completion yet.

All V3 handles are collected terminal zero: **1,575 library tests** (80.68s),
new mixed bulk integration **1** (40.51s), identity **5** (41.84s), map **5**
(47.19s), list **3** (42.79s). The mixed gate runs Java-matched behavior plus
two runtime control tests, not just compile checks. Expanded Java fixture
independently returns zero; `git diff --check` is clean on the changed product
files and the batch was formatted before its combined build.

Freeze sources for V135 release build and isolated corpus measurement. Build
log `.tmp/jgrapht-v135-mixed-bulk-build.log`, producer
`.tmp/new-java2rust-v135-mixed-bulk`; output root
`.tmp/jgrapht-core-1.5.3-focused-v135-mixed-bulk`. Compare exact diagnostics,
markers, guards, timings and generated contracts against V134. Do not run
competing tests or edit translator code while building/measuring. Relevant
sorted/comparator/synchronized carrier families remain outside this existing
local native carrier path; no unrelated fallback is silently replaced. Full
selected-slice cargo/oracle acceptance and expansion decision remain pending.

V135 release build **47234** is collected terminal zero (**1m31s**).
Frozen producer is **21,710,176 bytes**, SHA-256
`a531ecdca5ad1d0c068b682bd2baea47aa06727c3ca1302edebbaec1446932b7`.
Untouched 104-file measurement **66725** is live, explicit release profile,
`--skip-resolver-bench`, log `.tmp/jgrapht-v135-mixed-bulk-run.log`.
Collect this handle rather than restarting on an observation timeout. No
competing workloads or translator edits during the measurement.

### V135 measured result

Runner **66725** is collected terminal one (generated cargo failure). All
**104** files translate with **zero translator/generated parse failures**.
Diagnostics are **1,218 = 1,140 typed + 78 untyped**, with **seven removed and
zero added** versus V134. Invalid markers decrease **81 -> 76**, still in
**39 files**; allocation guards remain **23**. Exact report:
`.tmp/jgrapht-v134-v135-diagnostic-delta.txt`. Removed failures are five
unsupported retained bulk-operation markers (ChordalityInspector/GraphWalk)
and the retained `.iter()`/`.contains()` consumer errors in GraphTests and
BergeGraphInspector. Full crate check remains failing, not accepted.

Total **250.734s**, translation **169.914s**, assembly **63.302s**, check
**13.260s**, sweep **3.562s**. No new timing regression or reason to restart
optimization. All candidate build/test/measurement handles are terminal and
collected. Continue with a representative whole-path audit of the next shared
root cause; do not mistake 1,218 diagnostics for that many distinct bugs or
declare graph behavior/expansion complete.

### Next feasibility/rewrite audit: Java hash contracts, not Rust hash bounds

V135 still reports 20 unsupported retained collection operations, including
`AbstractGraph.hashCode`'s vertex-set hash inherited into graph implementations;
26 `&V.hash_code()` errors, and repeated Rust `Hash` obligations. The current
runtime is semantically unsuitable even where it compiles:
`java_list_hash_code`/`java_set_hash_code`/`java_map_hash_code` return container
length; `java_value_hash_code` runs Rust DefaultHasher. Do not fix this by merely
adding Rust Hash or JavaErasedObjectFacade bounds. The latter's erased hash
facade also returns a default value, not real Java dispatch.

New maintained `tests/fixtures/java_hash_contracts.java` independently returns
**zero** under Java. It covers order-sensitive list hashing (4066), plain and
retained sets (195, including null), entry-xor map aggregation (6), generic
method/class hash calls, custom source hashes, wrapper/string hashes, stable
default-object hash across payload mutation/aliases, and a throwing hash override
caught through a generic call. Frozen V135 fails direct rustc with three native
operation guards, two missing generic Hash bounds and one Rust HashSet source-key
insertion error. Artifacts `.tmp/java-hash-contracts-v135{.rs,-translate.log,
-rustc.log}`; Java classes under `.tmp/java-hash-contracts-oracle`. Handles
**98217**, **81037**, **57934** are collected failed rustc results, not live jobs.
No translated output was edited or accepted.

Concrete whole-path rewrite plan, implementable with existing machinery:

- A checked Java object-hash capability returns `Result<i32, JavaException>`.
  Primitive/string implementations delegate to the existing exact
  `JavaArraysHashElement` kernels rather than copying constants or bit logic.
  Null aggregate elements contribute zero; lists fold 31 from one, sets sum
  from zero, maps sum key/value hash XORs. Do not use deep-array hashing as a
  substitute for Object identity hash on arrays.
- Generic hash invocation and inherited signatures carry this capability via
  the existing source-call reachability/trait-forwarding analysis. Use canonical
  call planning for inherited Object methods rather than Rust method spelling.
  Propagate the operation's possible Java exception through exception planning,
  result shapes and existing call-result policy. An infallible trait with
  `expect` on a throwing source override would fail the maintained oracle.
- Source implementations delegate the actual selected `hashCode` override
  (including mutable/fallible receiver ABI); default Object hashing uses the
  existing canonical reference-identity protocol, not mutable payload hashing
  or the address of a temporary handle slot. Preserve source-owner precedence.
- Retained collection aggregation must walk the existing checked live cursor
  and release allocation guards before invoking element hashes, just as the
  completed bulk-membership path releases guards around equality callbacks.
  Plain carriers can borrow their elements without new payload Clone bounds;
  retained iteration capabilities come from solved physical child shapes.
- The source-key fixture exposes an adjacent producer/storage contract:
  `HashSet<Rc<HashContractToken>>` is chosen despite the existing JavaMapKey
  implementation on that canonical identity handle. Reuse semantic set carrier
  selection for source-reference keys; do not derive structural Rust Eq/Hash
  on mutable Java payloads just to silence the error. Trace constructor,
  insertion, storage, generic arguments and iteration together.

The existing IR, checked runtime helpers, storage shapes and capability graph
can express these rewrites; no new output-repair framework is needed. Implement
as a coherent hashing batch, with minimized Java-matched behavior before a
fresh full corpus measurement. Generic `getClass` needs a separate real runtime
type-dispatch contract; it was inspected but is not solved by this hash batch.
All previous gates remain applicable to V135; the new hash fixture is diagnostic
input, not yet a passing Rust integration. Full goal stays active/incomplete.

### Checked hash contract implementation, first verified stage (not frozen)

Implemented `java_checked_hash_runtime_support` and its separate native
collection portion in `stdlib.rs`. `JavaHashCode` returns a checked Java result;
primitive/string implementations delegate existing `JavaArraysHashElement`
kernels. Plain collection aggregation uses the Java list fold / set sum / map
entry-XOR formulas with wrapping arithmetic and null-zero leaves. Native
collection hashing walks the established checked live cursor without holding a
cell guard during user hashing. Raw Vec/slice deliberately do NOT implement
object hashing: those Rust types also represent Java arrays, whose Object hash
is identity-based. Proven plain List calls have a separate checked list helper.
Do not add a blanket Vec hash implementation to make generic array calls compile.

Added `java_hash_plan.rs`, the shared HashKind/receiver proof for native
List/Set/Map versus generic/erased Object dispatch, with parsed-source precedence.
The existing source-call capability reachability, owner/inherited field lookup,
interface-conversion/forwarding requirements and method/class binder emission
now carry `JavaHashCode`; there is no second call-graph solver. Runtime support
selection emits the declaration when hash syntax introduces this capability,
including exception/string leaf support, with native runtime separately gated.

Verification: kernel gate **73072** is collected zero (47.62s build, 10.50s
runtime). Metadata checks **73743**/**71305** are collected zero (19.81s/19.72s).
Combined library build **88323** is collected zero (46.44s); fresh binary gate
**41639** passes **three tests** (10.60s): source-call/owner/inherited hash
capabilities, operation classification/source precedence, and checked runtime.
Runtime checks cover exact list/set/map results, null, UTF-16 strings, canonical
NaN/signed-zero leaves, wrapping arithmetic, non-Clone plain elements, throwing
hashes, reentrant nonstructural source updates and structural fail-fast behavior.
Logs `.tmp/jgrapht-checked-hash-{kernel-v1,plan-check,capability-check,
contract-build-v2,contract-gates-v2}.log`. All handles are terminal/collected.

This is an in-progress architectural batch, NOT an accepted translator/corpus
candidate. No release or corpus run is warranted yet. Remaining integration:

- Route proven calls through checked helpers (plain List versus semantic/native
  value dispatch), with canonical receiver borrowing/null checks and existing
  result policy. Replace old RustHash/length shortcuts, not source overrides.
- Seed and propagate checked hash fallibility using the existing shaped-effect
  and source-call exception machinery; the generic throwing-override fixture
  must catch its Java exception, not panic or default a result.
- Generate source hash implementations on authoritative reference/value carriers:
  delegate actual inherited/custom methods using existing receiver/outer-handle
  conventions, or default to canonical allocation identity. Add required method
  capabilities to these implementation surfaces, not unrelated class methods.
- Close constructor/storage/source-key semantic set selection and iterator
  Clone/static obligations. In particular, hashes requested via generic source
  calls must discharge retained child iteration requirements using the existing
  source requirement index and solved shapes, not syntax-only Clone guesses.
- Keep erased/hierarchy/synchronized/retained-map cases honest: extend existing
  canonical dispatch where supported and retain explicit gaps otherwise. Live
  map entry hashing needs its entry-identity/value-update contract traced before
  any snapshot implementation; callback mutation must not be silently changed.

The maintained Java hash fixture is still diagnostic input, not passing Rust.
The last frozen corpus result remains V135: 1,218 diagnostics, 76 markers in 39
files, 23 allocation guards. Broader regressions and Java graph oracles have not
been rerun for this partial hash batch; do not claim their V135 evidence proves
this newer work. Continue the same batch rather than measuring this halfway state.

### Checked hashing integration and expanded boundary gate (in progress)

Wired the shared hash-call proof into both expression and slot lowering before
generic/List-view fallbacks. Checked helpers use canonical carrier borrowing,
receiver-null checks, existing result conversion, and produced integer shape
evidence. Shaped exception planning seeds open hash dispatch as a possible Java
Throwable, feeding the existing source-call fallibility solver. Source concrete
class implementations select declared/inherited hash methods (binder-specialized
for inherited requirements), use established outer-handle companions where
available, and forward their real Result ABI. Default source hashing uses the
canonical `JavaReferenceIdentity` allocation, not mutable field hashing.

Shared source keys now select the existing semantic set backend through common
signature/render/constructor authority instead of asking Rust HashSet for
structural Eq/Hash on reference payloads. Retained native hash iteration reuses
the existing physical child Clone/static requirement analysis. Runtime support
declarations are available across source-class module boundaries, not only in
the module containing a hash invocation.

Build V1 **84907** passes (48.95s); units **6825** pass **78**, identity **32613**
passes five, bulk **77702** passes one. Hash gate **33165** compiled the fixture
and reached a pre-existing unmodelled `Double.valueOf` at runtime. Added typed
primitive wrapper valueOf planning/lowering, preserving existing primitive value
carriers and reusing scalar slot widening/unboxing. String overloads retain
their parsing path; ordinary source-owner proof still controls dispatch.
Independent `primitive_value_factories.java` returns Java zero and covers all
wrapper families, numeric widening, string parsing and null unboxing.

Build V2 **25385** passes; units **4785** pass **108**. Hash/wrapper gate **99995**
has **one pass, one failure**: the complete initial hash fixture matches Java,
while wrapper testing reaches an already-modelled Double bit helper hidden by
an inferred generic facade. Explicit executable static model registration now
outranks inferred ABI methods while refusing to replace source declarations.
Double/Float bit/hash models use this common registration boundary.

Build V3 **77343** passes (49.77s). Hash gate **36959** passes **both Java-matched
integrations** (39.02s); identity **40936**, bulk **94848**, map **81792**, list
**16979** pass 5/1/5/3. Library **55313** is collected **failed: 1,578 pass, one
new facade precedence unit fails** (82.14s). Its unindented declarations expose
`facade_method_has_name` matching a literal leading space rather than using the
existing canonical name parser. The earlier arity-alias suspicion was wrong;
do not retain that workaround. Canonical matching fixes both whitespace forms
and the test keeps unique-signature/no-unimplemented-body assertions.

V4 **38378** is collected compile failure from that discarded alias workaround;
it is removed. The source cache field-name typo in metadata check **48422** and
primitive factory missing type-helper qualification in **4687** were corrected
before the passing combined builds. No product/generated-code repairs hidden.

Before accepting, expanded the hash fixture with a mutating source override,
a null generic receiver and an immutable object observed only through generic
hash calls; independent Java still returns zero. Mutable hash overrides reuse
the established `borrowed_object_dispatch` storage rule (including inherited
mutation) already used for virtual toString, rather than cloning their payload.
The completed per-program reference requirement index is generalized to
`SourceReferenceRequirements`, preserving its identity constructor/type alias
and adding a checked-hash capability mode. Storage reuses the same declaration
cache/substitution/demand filtering to retain source allocations observed through
generic hash calls. String hashing does not request String reference identity.

Build V5 **95642** is live; log `.tmp/jgrapht-checked-hash-integration-build-v5.log`.
All older build/test handles are collected, no release/corpus runner is live.
After this build, run the expanded hash/wrapper integration and the focused
facade/storage/reference-requirement units before any broad gate or freeze.
V135 remains the last measured corpus result. Direct default-object source call
fallbacks, closed/erased hierarchy dispatch, arbitrary generic carrier iteration
requirements and retained-map entry hash reentrancy still need honest coverage;
do not equate passing current hash fixtures with finishing those families or
the selected slice. No full goal completion or scope expansion.

V5 **95642** is collected zero (**50.53s**). Expanded Java-matched hash/wrapper
gate **37453** passes both integrations (**13.79s**); focused gate **2944**
passes **89** assembly/storage/requirement/classification/model units (**1.63s**).
The mutable-override, nullable receiver and no-preexisting-alias immutable hash
cases are now compiled and executed, not merely hypothetical designs. The
storage index generalization and borrowed object-protocol reuse close them.

V5 broad/neighboring verification is running: library **75412**, identity
**61703**, bulk **18633**, map **45241**, list **26612**. Logs
`.tmp/jgrapht-checked-hash-integration-{library,identity,bulk,map,list}-v5.log`.
Do not start a release build or corpus measurement until these gates finish.

All V5 broad/neighboring gates are now collected terminal zero: **1,579 library
tests** (80.15s), identity **5** (45.58s), bulk **1** (37.37s), map **5**
(45.13s), list **3** (41.57s). Together with the two hash/wrapper integrations,
this is **16 integrations**. The new facade model precedence test and borrowed
hash receiver storage cases are included in the passing library suite.

Freeze the source candidate for V136 release build and isolated untouched
104-file measurement. Producer `.tmp/new-java2rust-v136-checked-hash`, build
log `.tmp/jgrapht-v136-checked-hash-build.log`; prospective output root
`.tmp/jgrapht-core-1.5.3-focused-v136-checked-hash`. Compare exact diagnostics,
markers, guards and timings against V135; any additions need inspection, not
concealment behind a lower net count. The remaining hash-family gaps and full
corpus/Java graph oracle requirements are not declared solved. No competing
tests or translator edits during build/freeze/measurement.

V136 release build **12779** is collected terminal zero (**1m32s**). Frozen
producer is **21,666,136 bytes**, SHA-256
`8a9e2ac09bf7fe49711bea1c6db794605c9a7d12c5caae2897dd9f94c5092893`.
Untouched 104-file runner **38180** is live with explicit release profile and
`--skip-resolver-bench`; log `.tmp/jgrapht-v136-checked-hash-run.log`. Collect
this exact handle rather than restarting on an observation timeout. Source is
still frozen; no competing tests. V135 remains the last completed measurement
until this runner finishes.

### V136 measured result and follow-through audit

Runner **38180** is collected terminal one (generated Rust cargo failure).
All **104** selected files translate with **zero translator/generated parse
failures**. Diagnostics are **1,160 = 1,099 typed + 61 untyped**. Normalized
header/file multiset comparison removes **64** and adds **six** versus V135;
report `.tmp/jgrapht-v135-v136-diagnostic-delta.txt`. Invalid markers fall
**76/39 files -> 59/28 files**; allocation guards remain **23**. Removals include
26 generic vertex hash-method errors, 17 retained-collection operation guards,
and 21 obsolete Rust Hash obligations. Additions are not waived: two Clone
requirements in HierholzerEulerianCycle, two JavaHashCode requirements in
PairValue forwarding, and two Clone requirements in UnorderedPair field hashing.
The full selected crate still fails; no graph oracle acceptance or expansion.

Total **259.430s**, translation **177.877s**, assembly **63.674s**, generated
check **13.500s**, sweep **3.692s**. Global **44.264s**, semantic **10.537s**,
representation **4.944s**, signatures **24.897s**; emission **119.238s**.
Analysis remains stable; the ~8.7s run increase is principally emission of the
larger contract. No reason to restart an optimization phase.

The six additions identify a connected follow-through batch rather than new
payload Clone requirements to add blindly:

- Guard-refined hash receivers lose their physical nullable field carrier.
  `UnorderedPair.hashCode` generates `Some(&Clone::clone(&self.first))` for a
  stored `Option<A>` field, imposing A: Clone and hashing the optional carrier
  rather than borrowing its present Java value. Use the existing authoritative
  field-shape query before conversion, preserving refinements as evidence rather
  than pretending the stored slot changed. Hierholzer's fields expose the same
  mismatch. Correct emitted shape is a borrow from `field.as_ref()` followed by
  the checked/non-null receiver boundary, not a payload clone.
- Shaped exception analysis must see inherited fields in the receiver's generic
  frame. UnorderedPair currently emits infallible `hash_code -> i32` with panic
  unwrapping for its inherited A/B hash calls. Reuse specialized inherited-field
  and solved field-shape machinery for the effect environment; do not special-
  case hash method names or hide a throwing override behind `expect`.
- Closed carrier methods currently collect where-clauses from the root body,
  not every selected payload override. `PairValue.hash_code` carries only
  A/B: Clone but calls the UnorderedPair override requiring A/B: JavaHashCode.
  Reuse structured capability queries and binder substitution at actual selected
  forwarding arms, and apply the same contract to derived JavaMapKey/hash
  bridges instead of globally over-bounding unrelated methods.

New maintained `tests/fixtures/inherited_hash_contracts.java` independently
returns **zero** under Java. It covers nullable generic fields, guarded receiver
hashing, inherited fields/overrides, and different superclass/subclass binders
(`HashParent<T>`, `HashChild<U>`). Frozen V136 fails direct rustc with **ten**
diagnostics. Artifacts `.tmp/inherited-hash-contracts-v136{.rs,-translate.log,
-rustc.log}`; handle **66951** is collected failed. The fixture is diagnostic
input, not a passing Rust integration yet.

Besides the new Clone/capability errors, this minimized whole-path test exposes
two existing blockers that must be included in the same architectural reasoning:
the carrier emits `HashChild<U>` inside `HashParentValue<T>`, leaking the child's
unbound binder (and creating a spurious opaque U facade). Correct the variant's
type arguments in the root frame using the existing ancestor-argument and
binder-aware substitution mechanisms; its layout, field accessors, methods and
capability bounds must all agree. For this direct renaming case, the payload is
`HashChild<T>`, not a separately invented U type. Non-invertible generic ancestor
arguments need the existing existential/representation policy or an honest gap,
not an arbitrary type substitution. Finally, direct source `.hashCode()` still
routes some calls through Rust `java_value_hash_code`; connect source/hierarchy
protocol dispatch without stealing source overrides or qualified super dispatch.
Do not implement these as generated-file corrections. Existing IR and generic
frame machinery express the required rewrites; no output-repair framework or
corpus-specific source edits are authorized.

All verification/build/measurement handles are terminal and collected. Continue
the hashing/receiver/hierarchy contract follow-through from this audit. V136 is
the latest frozen measured result; full goal stays active and incomplete.

### Receiver/hierarchy follow-through implementation (V137 candidate, not frozen)

Physical hash receiver borrowing now prefers authoritative instance/static field
slots, not guard-refined Java spelling; borrowing uses the intact stored carrier
and then performs the required receiver check. The minimized output now uses
`self.value.as_ref().ok_or_else(...)`, not cloning `Option<T>`. Shaped effect
environments incorporate inherited field slots through existing ancestor type
substitution and slot specialization, preserving nearest-field/own-field hiding.

Closed carrier payload names now use the root declaration frame. Direct,
invertible ancestor-parameter mappings reuse the existing ancestor argument
query; both enum layout and ordinary carrier payload-name consumers now spell
Child<T> inside RootValue<T>. Structured/non-invertible mappings retain their
existing gap rather than substituting arbitrary type names. Selected forwarding
method capabilities are gathered from actual payload implementations, rebased
through declaring->payload->root substitutions and attached to that method's
where-clause. JavaMapKey forwarding uses the same query, not global Clone bounds.

Direct parsed-source hash calls use Object protocol dispatch (never native Set
policy for a source class named Set), with qualified `super` left on its existing
dispatch path. Concrete payloads behind closed carriers can emit their selected
checked hash implementation; supported carriers delegate to those actual
payload protocols. Source receiver hashing seeds the existing family retention
mechanism, preserving default allocation identity. No output repairs.

Metadata check **95768** found only a temporary-default refinement borrow
lifetime error, fixed by a local binding before build V1. Build **62622** passes
(49.11s). Focused units **48890** pass **160** (11.96s); hash **39411** passes
the two existing fixtures, while the new non-Clone control omitted the generated
phantom marker field. It is corrected without adding Clone to the payload.
Identity **66008** fails one existing boundary case because hash forwarding was
emitted for an unshared hierarchy whose payloads have no hash protocol; bulk
**66559** passes. All these handles are collected.

Expanded independent Java hierarchy fixture still returns zero, including a
nullable unguarded field (must throw NPE) and a throwing subclass override.
Build V2 **48057** passes (49.53s); hash **29069** passes all **three** integrations
(43.92s), including a non-Clone generic field control and both new exception
cases. Library **94462** passes **1,579** (81.84s), bulk **99213** and list **57437**
pass; identity **17773** and map **71448** each fail one unavailable-protocol
case. All handles collected.

Inspection found the first eligibility edit was attached to runtime-class
metadata rather than hash emission. Restore unconditional non-anonymous
get_class metadata, and gate only hash forwarding. The final design shares
`source_java_hash_method_for_type` and `source_java_hash_protocol_is_supported`
between payload implementation emission and carrier eligibility; it checks
selected methods, storage, receiver ABI, and declaration provenance consistently.
No fabricated hash for plain payloads without a stable identity. Added explicit
reflection-without-hash and inherited-field-effect regression units.

Build V3 **41038** is collected zero (49.81s). Fresh verification is live:
hash **20041**, library **54749**, identity **37153**, bulk **89940**, map **47060**,
list **81226**. Logs `.tmp/jgrapht-hash-hierarchy-*-v3.log`. Wait for these exact
handles before freezing/measuring. V136 remains the last frozen 104-file result;
no new corpus result, graph-oracle acceptance or full-goal completion yet.

V3 gates are terminal: library passes **1,581**, identity **five**, map **five**,
bulk **one** and list **three**. Hash passes two and fails the inherited generic
carrier case: no `JavaHashCode` implementation for `HashParentValue<String>`.
The shared payload lookup compared a generic use name (`Child<U>`) only against
the carrier's base variant name (`Child`), returning no payloads. Consequently
both eligibility and selected-method capability queries missed generic payloads.
The shared lookup now accepts the canonical base name as well as the full use
name, with a focused regression covering both forms; this is not a per-corpus
hash special case.

Build V4 is terminal and successful (49.80s), as recorded in
`.tmp/jgrapht-hash-hierarchy-build-v4.log`. Reusing these built executables,
verification is live: hash **57197**, library **69046**, identity **59939**,
map **97589**. Logs `.tmp/jgrapht-hash-hierarchy-*-v4.log`. Collect these before
freezing a candidate; V136 remains the latest measured corpus result.

V4 verification is now collected and successful: **1,582 library tests**
(83.51s) and **17 integrations**: hashing three (39.66s), identity five
(46.27s), map five (50.68s), bulk one (36.40s), list three (38.37s).
The inherited hash fixture now passes with different root/child binders,
nullable guarded and unguarded fields, throwing virtual overrides and a
non-Clone generic payload control. No pinned Java or emitted Rust was repaired.
Source edits are frozen for a release build and V137 corpus measurement.

Release **42038** is collected zero (1m33s). Frozen producer
`.tmp/new-java2rust-v137-hash-hierarchy` is **21,839,304 bytes**, SHA-256
`98fb3d67354474fd47d127bab6e71958e8af9b7f5ef8873bfb0151d767dbc958`.
The untouched 104-file V137 run is live as **48269**, log
`.tmp/jgrapht-core-v137-hash-hierarchy-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v137-hash-hierarchy`. No competing builds or
tests. Collect its terminal result and normalized diagnostic delta against V136
before choosing the next substantial batch.

### V137 measured result and next rewrite boundary

Run **48269** is terminal/collected (exit one: generated Rust fails cargo
check). All **104** files translate, with **zero translator failures and zero
generated parse failures**. Diagnostics are **1,151 = 1,090 typed + 61 untyped**,
versus V136's 1,160. Normalized header/file multisets show **16 removed and
seven added**; report `.tmp/jgrapht-v136-v137-diagnostic-delta.txt`. Invalid
markers remain **59 in 28 files**, allocation guards **23**. This is not
selected-slice acceptance or full-goal completion.

All six V136 additions motivating this batch are removed (Hierholzer V/E Clone,
PairValue A/B JavaHashCode, UnorderedPair A/B Clone), as are Pair's A/B Clone
bridge errors. Other changed reports include Pair map-query and conversion
errors, a DoublyLinkedList match-arm mismatch, and the nested SetForEquality
hash call. Do not interpret these header removals as proof of their whole paths.

The seven additions are six `get_query`/`remove_query` failures in directed and
undirected fast lookup, plus missing `JavaHashCode` on the nested
`ArrayUnenforcedSet_SetForEquality<E>`. Inspection establishes:

- The six query errors now name the selected key contract correctly:
  `PairValue<Option<V>, V>: JavaMapKey` needs `V: Clone` through its selected
  equals implementation. Caller methods such as `remove_from_index` still have
  only `V: JavaMapKey`. The capability obligation must be propagated from the
  actual nested source key/protocol back to the enclosing callable and its
  forwarders; attaching unrelated blanket bounds is not a solution. The older
  stored/query Pair type-shape disagreement also remains a separate obligation.
- SetForEquality extends external `AbstractSet<E>` and supplies iterator/size,
  but no local hash override. Its Java inherited hash traverses that iterator;
  generated code has neither this inherited protocol nor inherited equals.
  V136's missing `hash_code` is now a missing checked protocol, not a newly
  working path regressing. Correct support must model external inherited
  collection behavior through source virtual iteration and checked callbacks,
  preserving source precedence and enclosing-instance access. A default
  allocation hash would silently change Set semantics and is unacceptable.

Next coherent batch: audit and exercise nested source protocol capability
propagation (map-key equals/hash requirements, substitutions and forwarding)
with a minimized Java-matched map fixture before editing the shared requirement
machinery. Keep external inherited collection protocols as an explicit, larger
representation/dispatch follow-through, with their own minimized iterator and
callback evidence rather than a SetForEquality-specific implementation.

Isolated total **263.127s**, translation **180.343s**, assembly **64.544s**,
cargo check **13.805s**, sweep **3.761s**; comparable to V136's 259.430s.
There is no new timing evidence justifying a renewed optimization phase.
All gates/build/run handles in this batch are terminal and collected.

### Nested source key capability batch (V138 candidate, not frozen)

New maintained `nested_key_capabilities.java` independently returns **zero**
under Java. Its retained map uses a generic source hierarchy with a selected
equals implementation that snapshots a generic field. Frozen V137 produces
exactly **three** missing selected-protocol Clone errors on get/remove/contains
(after removing unrelated raw instanceof from the initial minimization and
retaining the map through its real key view). Baseline artifacts
`.tmp/nested-key-capabilities-v137{.rs,-translate.log,-rustc.log}`; baseline
handles **62242**, **30044** are terminal/collected. No generated Rust repairs.

Extracted the existing carrier method-selection/rebasing query as structured
`source_hierarchy_selected_method_frames`, reused by emitting method bounds and
by map-key requirement propagation. Equals selection likewise uses one shared
query. The native map call is proven by existing native planning, excluding
source lookalikes; only actual key-protocol operations demand selected method
capabilities. Requirements are substituted declaring->root->caller and flow
through existing callable reachability/forwarding. Size-only access does not
demand key-protocol Clone; a non-Clone runtime control enforces this.

Build V1 **16974** passes (49.24s); focused runtime **17385** passes the new Java
oracle and non-Clone size control (13.06s); assembly units **95655** pass **47**
(1.58s). Review found that identity ancestor queries intentionally return empty
maps: the extracted frames must explicitly include root identity bindings and
rebase direct child override binders. Fixed centrally with an explicit frame
regression. Build V2 **46828** is live, log
`.tmp/nested-key-capabilities-build-v2.log`; collect before broader gates.
V137 remains the latest measured corpus result; the full objective is active.

V2 build **46828** and gates are collected successful: **1,583 library tests**
(81.07s), map **six** (48.98s), hash **three** (43.11s), identity **five**
(47.12s). Handles **4597**, **41892**, **42043**, **74596** are terminal.
To avoid a Clone-only fix, expanded the same Java fixture so selected equals
also invokes its generic field's hash; Java still returns zero. Generic-method
and class-method capability rendering now reuse their existing translation
cache/storage context for identity/hash/key queries, rather than falling back
to a context-free search that cannot see the selected key carrier. This permits
the structured protocol obligations to propagate beyond Clone. Build V3
**97471** is live, `.tmp/nested-key-capabilities-build-v3.log`. Prior V2 gates
do not verify these latest edits; collect V3 and rerun applicable gates before
freezing. No new full corpus measurement yet.

V3 build **97471** is collected successful (49.35s). Gates are terminal and
collected: **1,583 library tests** (80.09s), map **six** (43.30s), hash **three**
(32.44s), identity **five** (42.84s), handles **15295**, **5518**, **95428**,
**74920**. The expanded selected-equals fixture now proves Clone **and**
JavaHashCode obligations cross the generic source key/map/caller/forwarder
boundary; the non-Clone size-only control still passes. After these behavioral
edits, three now-unused production wrappers were marked `cfg(test)` (their only
remaining consumers); their test-mode bodies are unchanged. Source edits are
otherwise frozen for the release candidate. The release build verifies the
production cfg surface; no output or pinned input repairs.

Release **62189** is terminal/collected zero (1m32s), with no new warnings.
Frozen `.tmp/new-java2rust-v138-nested-key` is **21,848,712 bytes**, SHA-256
`fe8186fdbc846b70ac8afb7789a1be7cecbbc3c0b8e3df546652d15c1710ce8f`.
V138 untouched 104-file runner **84747** is live, log
`.tmp/jgrapht-core-v138-nested-key-run.log`, output
`.tmp/jgrapht-core-1.5.3-focused-v138-nested-key`. Source remains frozen and
there are no competing tests/builds. Collect and compare with V137 before
claiming any corpus improvement.

### V138 measured result and source-carrier boundary follow-through

Runner **84747** is terminal/collected (exit one: generated Rust fails cargo
check). All **104** selected files translate, with **zero translator failures
and zero generated parse failures**. Diagnostics **1,134 = 1,073 typed + 61
untyped**, down from V137's 1,151: **25 removed, eight added** by normalized
header/first-generated-file multiset. Report
`.tmp/jgrapht-v137-v138-diagnostic-delta.txt`. Invalid markers remain **59 in
28 files**, allocation guards **23**. Full goal remains active/incomplete.

All six targeted fast-lookup missing key-protocol obligations are removed.
Inherited-field capability environments and context-aware searches additionally
remove missing E JavaMapKey/Clone witnesses from uniform/weighted intrusive-edge
specifics. The eight additions expose downstream obligations after those
requirements are available, rather than establishing those whole paths:

- Six are the earlier Pair key-domain disagreements: stored
  `PairValue<Option<V>, V>` versus queried/constructed `PairValue<V, V>` or
  `UnorderedPair<V, V>`. These were already present before V137 and were masked
  by the selected-equals bound failure. Do not add cross-instantiation equality
  or rebuild payload allocations merely to satisfy this report; establish where
  source generic arguments gained the extra Option and preserve one physical
  source carrier through storage, queries and source factories.
- Two newly type-checkable get results (uniform line 57, weighted line 91)
  already contain IntrusiveEdgeValue/IntrusiveWeightedEdgeValue, but lowering
  maps them through the corresponding enum variant constructor again. The
  constructor requires Arc<Mutex<payload>>, not an already wrapped carrier.
  Correct result conversion must consume the actual stored source-carrier
  shape, reusing the shared physical map-child projection/conversion planning,
  rather than treating every Java source type name as a raw payload.

Next audit/batch should focus on these source-carrier map boundaries, with
minimized cases for physical source-key generic domains and already-wrapped
source values. Trace storage producers, resolved factory types, projections and
query/result consumers before changing anything; the previous nested-map
fixture explicitly did not reproduce the Pair<Option<V>,V> issue. Determine
whether the two symptoms share an actual root cause before batching edits.
External AbstractSet inherited iteration/equality/hash remains a distinct,
larger dispatch capability; do not substitute identity behavior.

Isolated total **260.176s**, translation **178.445s**, assembly **64.144s**,
cargo check **13.218s**, sweep **3.682s**: stable against V137's 263.127s.
No renewed optimization phase is justified by this measurement. All handles
started in the nested-key batch are terminal and collected; no work is running.

### Source-carrier map read/identity batch (V139 candidate, not frozen)

Maintained `source_carrier_map_values.java` independently returns **zero** under
Java. It exercises a retained map containing base/child source values,
get/put/remove return values, mutation through a returned alias, key-view
liveness, stored null and missing keys. Frozen V138 fails with **eight**
diagnostics: three double enum wraps, four invalid Rc identity comparisons on
carrier enums, and an invalid optional-carrier setter projection. Artifacts
`.tmp/source-carrier-map-values-v138{.rs,-translate.log,-rustc.log}`; handle
**5767** is terminal/collected failed. No generated-file corrections.

Read projection used `storage_arg_shape_with_effective_facts`, which does not
know solved source carriers. Reuse the representation-aware insertion/storage
projection for native materialized reads, including get/put/remove and sequence
iteration consumers of that helper. Build V1 **7726** passes (50.07s); runtime
**70224** fails with only the remaining **five** identity/mutation errors
(13.58s), confirming that all three double-wrap diagnostics are removed.

Whole-path inspection also finds that StoredNode was Rc<RefCell<...>> but
StoredLeaf remained a plain payload even though an aliased base reference held
it. The inherited receiver-storage pass now propagates ancestor sharing demand
as well as inherited mutation; it traverses all ancestors rather than stopping
at the first mutating ancestor. Added an explicit storage regression for both
base and child. Nullable source-carrier identity uses the existing checked
reference-identity lowering, rather than Rc::ptr_eq on enum values. Nullable
field mutation uses the existing carrier projection after checking its payload
type, rather than borrowing the enum as a raw RefCell. These reuse existing
contracts; no fake cross-instantiation equality or cloned payload allocation.

Build V2 **77807** is live, log `.tmp/source-carrier-map-build-v2.log`.
Collect and test before freezing/measuring. The separate Pair<Option<V>,V>
key-domain mismatch has not been reproduced by this fixture or fixed. Keep it
explicit; do not infer that it shares this read-dispatch root cause. V138
remains the latest frozen measured result; the full goal is active/incomplete.

V2 build **77807** and gates are collected: map **seven** pass (43.16s), list
**three** pass (36.04s), identity **five** pass (42.53s), handles **48469**,
**28224**, **12141**. Library **43199** has **1,582 pass / two fail** (80.91s).
Both failures assert the previous unshared child constructor/upcast spelling;
generated output now uses an Rc-backed child in the same base carrier. Updated
the expected conversion spelling and explicitly asserted the Rc child variant,
without removing the original no-invalid-upcast/default invariants.

Expanded Java oracle still returns zero with a subclass setter override and a
null receiver whose increment argument must run before NPE. Source hierarchy
setter inlining would bypass the override (and early-check null); like the
existing getter carrier path, setters now stay on ordinary virtual source-call
lowering rather than replacing a statically selected method with a field write.
Build V3 **56375** is live, `.tmp/source-carrier-map-build-v3.log`. V2 fixture
gates do not verify the expanded behavior or latest setter edit. Collect V3
and verify before freezing; no V139 corpus run has started.

V3 build **56375** passes (49.92s). Library **72671** passes **1,584** (83.85s),
list **45424** passes **three** (46.52s), identity **95956** passes **five**
(43.39s). Map **5984** passes six but the expanded fixture returns **seven**:
the null receiver was checked before its increment argument. Subclass override
dispatch and all earlier alias/mutation/null-value checks now pass; this is an
actual ordering bug, not a test assertion issue. All V3 handles collected.

Retained source hierarchy calls with arguments and a known source result ABI
now reuse `staged_instance_call_projection`: copy the canonical carrier (never
a payload guard), evaluate argument values/conversions, then adapt/check the
receiver and invoke the selected virtual method. The ordinary receiver,
argument, result-ABI and optional-erased-view planners are reused. Staging is
limited to proven retained source carriers, where copying preserves allocation
identity, rather than cloning arbitrary unshared objects. Build V4 **37538** is
live, `.tmp/source-carrier-map-build-v4.log`. Collect, then verify the expanded
runtime test and neighboring gates; do not claim V4 is passing from V3 evidence.

V4 **37538** builds, and all handles are collected: map **72400** fails the
new fixture at compile time (two E0596s: staged receiver requires a mutable
binding for `as_mut`), library **67396** has **1,583 pass / one output assertion
failure** (84.86s), identity **81434** and list **46236** pass five/three
(44.47s/46.55s). The shared staged-call renderer now has an explicit mutable
receiver-binding option; existing callers keep their unchanged immutable mode.
V5 build **14750** is terminal/collected successful; no V5 runtime claim.

Review of the changed worker output also exposed a real materialization issue:
staging a raw `List.get` expression cloned `Option<&Carrier>` instead of the
checked materialized Java reference. Snapshot operands now go through the same
slot lowering and native materialized-result shape used by ordinary reads.
Updated the worker assertion to require staged virtual invocation and prohibit
cloning the raw list bounds Option, retaining the no-payload-borrow checks.
Expanded maintained Java coverage to non-null list carrier mutation and null
list elements with argument effects before NPE. Build V6 **62116** is live,
`.tmp/source-carrier-map-build-v6.log`. Verify this complete operand/staging
contract before release; no corpus measurement has started for the batch.

V6 **62116** builds (50.45s). Focused handles **56398**, **71571** are terminal
and collected failed: the runtime fixture has one compile error, a raw field
setter on a nullable list element; the worker assertion prohibited even a
correct checked `get(...).ok_or_else(...)` snapshot. Tightened that assertion
to require the actual bounds check rather than reject every get expression.
Closed-carrier receiver classification now takes the materialized native
collection result shape first, so nullable list elements also stay on virtual
dispatch rather than the trivial field-inline path. This is the same physical
read contract used by snapshot lowering, not a nullable-list spelling special
case. Build V7 **61438** is live, `.tmp/source-carrier-map-build-v7.log`.
V139 remains unmeasured; V138 is authoritative for corpus counts.

V7 **61438** builds (48.68s). Worker unit **48503** passes (9.56s). Expanded
runtime **79483** compiles but returns **ten** (13.49s): map receiver ordering,
identity, overrides and non-null list mutation pass; nullable list-element
argument ordering still fails. Classification for staging still used the
inferred Java expression shape even though its operand materialization used the
physical read shape. Both now use one physical shape before classification and
snapshotting. Added an index-failure-before-argument-effects control alongside
null-element-after-argument-effects. Build V8 **49548** is live,
`.tmp/source-carrier-map-build-v8.log`. Collect before running the complete
fixture. No passing runtime or corpus claim for these latest edits yet.

V8 build **49548** is terminal/collected successful. Focused runtime **28832**
passes (13.57s), worker unit **84392** passes (9.77s): all expanded Java oracle
cases are now verified, including null receiver arguments after receiver
evaluation but before NPE, and bounds failure before argument evaluation.
Broad verification is live using the same built binaries: library **25201**,
map **38409**, list **98564**, identity **37444**. Additional neighbor-only
build **12868** compiles checked-borrow/hash/bulk integration executables
against the already-built library, log
`.tmp/source-carrier-map-neighbor-build-v8.log`. Collect these handles, run the
new neighbor binaries, then freeze/release/measure if all applicable gates pass.
No V139 release or corpus process has started. V138 remains the latest measured
result, and the separate Pair key-domain mismatch remains outstanding.

V8 broad gates are terminal/collected successful: **1,584 library tests**
(86.58s), map **seven** (50.14s), identity **five** (46.28s), list **three**
(42.44s), hash **three** (29.80s), bulk **one** (30.33s), checked borrows **two**
(31.12s): **21 outer integrations**. Neighbor build **12868** passes (2.59s).
Neighbor handles **95339**, **17055**, **36069** are collected too.

Final ordering review found that wrapping borrowed-argument temporaries outside
the staged call would evaluate those temporaries ahead of the receiver. When
they exist, the already-materialized receiver is now prepended as a fresh
binding to the existing temporary list; all loans remain alive through the
call and source operand names remain unshadowed. Added a structural regression
for receiver/argument effect ordering. Empty-temporary paths keep their same
shape. Build V9 **44117** is live, `.tmp/source-carrier-map-build-v9.log`.
Collect before final gates and release. The full goal remains incomplete;
there is still no new full corpus result beyond V138.

V9 build **44117** is collected zero (49.25s). Final gates all pass and are
terminal/collected: **1,585 library tests** (86.04s), map **seven** (52.37s),
identity **five** (51.21s), list **three** (48.94s), hash **three** (44.38s),
bulk **one** (46.58s), checked borrows **two** (48.14s), **21 integrations**
total. Handles **96473**, **34085**, **55688**, **27559**, **53546**, **30309**,
**8218**. Logs `.tmp/source-carrier-map-*-v9.log`. Source edits are frozen for
release and full untouched-slice measurement. The Pair key-domain and external
inherited collection-protocol gaps remain explicit; no acceptance is claimed.

Release **87389** is terminal/collected successful (1m33s). Frozen producer
`.tmp/new-java2rust-v139-source-carrier-map` is **21,884,696 bytes**, SHA-256
`9784ac192500d80571dc06dd0a4c5227ab48eadc15c329576362bafa27386a92`.
Untouched 104-file measurement **74890** is live, log
`.tmp/jgrapht-core-v139-source-carrier-map-run.log`, output
`.tmp/jgrapht-core-1.5.3-focused-v139-source-carrier-map`. No source edits or
competing builds/tests during measurement. Collect this exact handle and
compare with V138 using `.tmp/compare-jgrapht-diagnostics.py` before selecting
the next batch. V138 remains the latest completed corpus measurement.

### V139 collected and refreshed batch selection

Runner **74890** is terminal/collected failed solely at generated Rust cargo
check. All **104** files translate, with **zero translator failures and zero
generated parse failures**. Diagnostics **1,126 = 1,065 typed + 61 untyped**:
**eight removed, zero added** versus V138. Report
`.tmp/jgrapht-v138-v139-diagnostic-delta.txt`. The two targeted intrusive-edge
double-wrap errors disappear, along with three TreeSingleSourcePathsImpl type
mismatches and three DoublyLinkedList lifetime/argument errors. Markers remain
**59 in 28 files**, allocation guards **23**. No corpus acceptance, graph
oracle gate or expansion is claimed. No builds/tests/runners remain live.

Isolated total **257.292s**, translation **175.571s**, assembly **64.029s**,
cargo check **13.313s**, sweep **3.695s**: stable versus V138's 260.176s. The
eight removals are real progress, but the implementation required repeated
whole-path follow-through and does not justify continuing on tiny diagnostic
islands or restarting runtime micro-optimization. The bottleneck is incomplete
cross-boundary contracts, not a new measured translator slowdown.

Refreshed full-slice evidence: **30 of 36** non-Try propagation diagnostics are
GraphIterables (18) and DefaultGraphIterables (12), matching audit sections 3/4
above. The adapter initializes a source object with its constructor Result
already discharged, then creates a plain Iterable facade, but an enclosing
Java-expression effect appends another `?` to that facade. The same path also
captures a borrowed receiver for a deferred supplier. Fixing only the final
question mark would leave the ownership/temporal behavior unproven.

New maintained diagnostic input `tests/fixtures/deferred_source_views.java`
independently returns **zero** under Java. It combines an interface default
method, a checked source Iterable wrapper constructor, and a deferred supplier
calling a mutable source getter. The oracle verifies no getter invocation at
construction, observation of replacement lists on successive iterations,
getter side effects, deferred null failure, and immediate null-supplier
constructor failure. Frozen V139 has **11 diagnostics**: three allocation
guards, five non-Try propagations, two type mismatches and one Rc move failure.
Artifacts `.tmp/deferred-source-views-v139{.rs,-translate.log,-rustc.log}`;
handle **84248** is terminal/collected failed. The fixture is not registered as
a passing integration yet. No source implementation edits in this audit turn.

Next coherent batch is the deferred-view construction/capture contract:

1. Preserve the producer's discharged construction effects across
   `lower_native_functional_object_for_slot`; its resulting facade is a plain
   value even though invoking its callback may later return Result. Reuse the
   existing structured effect markers, not emitted-text question-mark removal.
2. Retain an owned canonical receiver/interface carrier for escaping implicit
   `this` calls. Existing source trait `clone_box` and shared implementation
   adapters provide candidate ownership mechanisms, but verify dispatch to a
   shared adapter rather than cloning a raw borrowed source payload. An owned
   callback must not retain a Ref/RefMut guard or eagerly call the getter.
3. Preserve the source getter's physical nullable/native collection return
   through Supplier -> source Iterable -> native Iterable conversion. No
   retained-to-plain copy, eager collection snapshot or erased alias fallback.
4. Trace any required generic Any/Clone obligations through default bodies,
   inherited concrete methods and interface forwarders, then validate the
   whole minimized Java behavior before the next expensive corpus run.

These are local effect-metadata plus cross-function ownership/return-contract
changes. Existing explicit owner companions, shared interface adapters and
native facade callbacks can express the required shape; their integration is
not yet proven. If borrowed-default receiver dispatch cannot acquire ownership
through those mechanisms, surface that precise framework gap and redesign its
receiver entry point before further local patches. The Pair key-domain audit
and external inherited AbstractSet protocols remain queued, not solved or
dropped. V139 is the latest frozen measured corpus result; full goal active.

### Deferred-view implementation checkpoint (not a corpus acceptance)

The maintained `deferred_source_views` integration now exercises deferred
receiver identity, replacement collection visibility, getter side effects,
nullable Supplier results, and immediate null-Supplier rejection. The Java
oracle returns zero. The fixture explicitly reads a null Supplier result as
null before checking invocation-time failure, so eager rejection cannot mask
a lost nullable result contract.

Implemented lexical `this` capture facts (including generic owner arguments
and shadowing), resolution of lambdas targeted to native SAM constructor
parameters, reuse of owned receiver companions/trait clone-box adapters,
plain-value effect metadata for constructed iterable facades, and retained
getter return-shape projection. Source-interface conversion now uses the
existing liveness-aware clone mechanism before consuming a reused Rc.

Focused builds V1/V2/V3 exposed the complete path: unresolved native SAM
lambda bodies first hid receiver captures; then structural Supplier result
nullability was misclassified as sequence-element nullability; then checked
constructor-parameter-to-field transfers omitted SAM result qualifiers.
The structural path classifier now distinguishes actual sequences/maps from
SAM generic arguments. The V4 build completed in 51.56s. Its full library run
passed 1,585 tests, with one failure in an exact flow-count assertion because
callback return transfers now add a legitimate second edge. Runtime V4 still
fails generated compilation: the field now correctly stores
`Supplier<Option<Iterable<T>>>`, but invocation shape projection still loses
the result Option, causing an invalid null-comparison marker and a method
call on `Option<Iterable<T>>`. Artifacts
`.tmp/deferred-source-views-{build,lib,runtime}-v4.log`.

The next candidate adds one shared native-SAM invocation result-shape
projection used by expression inference and preferred call-result planning.
This is still an unverified in-progress batch; V139 remains the latest frozen
104-file measurement. No generated Rust or pinned corpus source is repaired.

V5 follow-through: the shared native-SAM result projection fixes both the
nullable `get()` comparison and checked `iterator()` receiver. The maintained
deferred-view runtime test **passes** (13.71s); its field-shape unit passes too.
The independent Java fixture was recompiled unchanged and returns **0**.
The exact flow regression now asserts the additional canonical local-to-return
edge, not merely a relaxed count. A nominal native-SAM eligibility check avoids
duplicating recursive physical inference for ordinary call chains. The batched
gate build passed in 51.42s; library handle **24420** and ten-suite integration
handle **16669** are live. Collect both before starting the isolated release
and full-slice measurement. Build log
`.tmp/deferred-source-views-build-gate.log`, library log
`.tmp/deferred-source-views-lib-gate.log`; focused passing runtime log
`.tmp/deferred-source-views-runtime-v5.log`.

Both gates are terminal/collected **passed**: **1,587 library tests** and
**31 integrations** across ten suites (constructor callback values, deferred
source views, functional carrier arguments, native functional objects,
receiver identity, retained callback state, retained streams, reusable
callbacks, source callback contracts, and virtual callback contracts).
The gate build took 51.42s; library gate 75.36s. `git diff --check` on the
affected implementation and regression files is clean. Release V140 is now
building at `.tmp/deferred-views-release-v140.log`; no tests remain live.
Collect and freeze the producer, then measure the untouched 104-file slice
before selecting another batch. Full goal remains active and incomplete.

Release **95353** is terminal/collected passed (1m35s). Frozen producer
`.tmp/new-java2rust-v140-deferred-views`, **21,907,792 bytes**, SHA-256
`1b34815e4165d2d2b79fa13e0fc204299e079fa527e6b13ceecfc98c8370a26e`.
The isolated 104-file runner **52019** is live; log
`.tmp/jgrapht-core-v140-deferred-views-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v140-deferred-views`. No competing tests/builds
are running. Collect this runner before claiming diagnostic progress; V139 is
still the latest completed corpus measurement.

### V140 collected; callback contract follow-through selected

Runner **52019** is terminal/collected failed at generated Rust cargo check.
All **104** files translate; **zero translator failures and zero generated
parse failures**. Diagnostics **1,086 = 1,028 typed + 58 untyped**, versus
V139's 1,126: **74 removed / 34 added** in the normalized header/first-source
multiset, report `.tmp/jgrapht-v139-v140-diagnostic-delta.txt`. All **30**
targeted non-Try errors disappear (GraphIterables 18, DefaultGraphIterables 12).
Invalid markers **57 in 27 files**, down from 59 in 28; allocation guards
**20**, down from 23. This proves neither assembled acceptance nor complete
graph behavior. No builds/tests/runners remain live.

Total **266.734s**: translation **184.975s**, assembly **64.135s**, check
**13.243s**, sweep **3.715s**. Versus V139 257.292s, the increase is in
translation, not assembly/check; this single modest difference is not evidence
for reopening the completed optimization phase. The active bottleneck remains
cross-boundary semantic contracts.

Inspection of additions identifies concrete follow-through, not 34 distinct
bugs or permission to ignore regressions:

1. `LiveIterableWrapper.supplier` now correctly has physical
   `Supplier<Option<Iterable<Option<E>>>>`; invocation and nullable receiver
   checking are correct. Its source `iterator()` signature still promises
   `JavaIterator<E>`, while the native invocation returns
   `JavaIterator<Option<E>>` (compile.log line 16150). Required rewrite is to
   propagate the qualified element domain through the source method return
   and implemented Iterable contract, not unwrap elements, snapshot data or
   change Java. Extend the minimized deferred fixture with nullable generic
   elements and test typed cursor consumption.
2. AsSubgraph's new missing borrowed `get_all_edges` / `get_edge` calls trace
   to receiver-capture eligibility. `getAllEdges` uses `edgeSet::contains`, a
   **bound field-value reference evaluated at construction**, not a lambda
   rereading the enclosing receiver. `resolve::analyze_method_reference`
   currently sets `capturing_receiver = true` for every target, so a field
   becomes `CaptureSource::This` before its existing ThisField fallback.
   This overstates the canonical-owner requirement, suppresses borrowed
   methods, and propagates to callers. Preserve This for `this::method` /
   `super::method`; reuse ThisField/local receiver-value capture for
   `field::method` and `this.field::method`. Nested lambdas must still retain
   their own lexical receiver when they evaluate such a reference later.
   Validate replacement-field timing and lexical shadowing explicitly.
3. FastLookupGraphSpecificsStrategy exposes callback result ownership and
   scoped capability obligations (`&dyn Specifics: From<Rc<RefCell<...>>>`,
   missing V key bounds). Trace the actual native-SAM factory result through
   source-interface conversion before changing any bounds. Other new E Clone
   obligations and renamed compute-if-absent errors need the same whole-path
   audit; do not label them harmless from counts.

The Pair key problem is **not solved**: eight old Pair query errors disappear,
but six qualified `Option<PairValue<V,V>>` versus
`Option<PairValue<Option<V>,V>>` query errors replace them. External inherited
AbstractSet protocol support also remains queued. Next coherent batch is the
callback receiver-value vs lexical-owner distinction plus instantiated result
contract projection; these share the new capture/return evidence above. Use
focused regressions and one batched build before the next full measurement.
V140 is now the latest frozen corpus measurement. Full objective stays active.

### Bound-reference and qualified cursor follow-through (in progress)

V140's additions were reproduced with maintained fixtures before claiming any
new fixes. `deferred_source_views.java` now checks null String elements through
both concrete and interface typed cursors; frozen V140 fails with six generated
type errors. `bound_field_reference_values.java` checks field replacement,
explicit field references, lexical shadowing, a lambda which creates a bound
reference later, and creation-time versus invocation-time Java null failures.
Both independent Java oracles return **0**.

The first candidate distinguishes immediate bound receiver values from lexical
receiver captures and records ordinary cursor creation/forwarding element-domain
edges without equating cursor and collection allocations. Build V1 passed
(50.54s); the new capture-facts unit and the expanded field/cursor-signature unit
pass. The maintained runtime gate still fails: nested bound references are
ignored by `expr_reads_self`, so their enclosing lambda's companion remains a
borrowed delegate; nullable source iterator results now correctly widen but
source-object-to-native-SAM conversion still uses nominal type arguments. The
new heterogeneous generic caller also exposes the need for shared Iterable
element storage to agree with its callback parameter's qualified ABI.

V2 is building at `.tmp/callback-cursor-build-v2.log`, handle **76506**. It
adds selected source-SAM result flows to the existing value-domain graph,
joins only the descendant storage domain of native Iterable callback returns,
recognizes bound-reference receiver reads inside enclosing lambdas, and reuses
the existing catchable-Java-null projection for immediate receiver evaluation.
No corpus rerun until the whole minimized runtime behavior passes. V140 remains
the latest frozen corpus result; full goal active and incomplete.

V2 build passed in **48.89s**. Both minimized generated-runtime regressions
now **pass** (14.38s), as does the expanded field/cursor signature unit. This
includes replacement-field timing, nested lambda timing, bound receiver null
checks at creation, late receiver null checks at invocation, and nullable
generic elements through concrete and native-interface cursors. The new source
SAM projection derives its result paths from canonical SAM metadata and uses
selected source method identities; it does not fabricate fallback result
shapes when the source return cannot be projected.

The exact graph-edge regressions now verify the forward outer result edge,
paired descendant-only storage edges and subsequent canonical return edge.
A read-only Iterator capability regression checks that parsed source and user
overrides still take precedence over modeled native cursors. Batched gate
build **11622** is live at `.tmp/callback-cursor-build-gate.log`; run library
and the nine selected integration suites from those built binaries, then
freeze/measure once gates pass. V140 remains the latest measured corpus.

Gate build **11622** passed (50.42s). Library **99915** and integration
**93007** are terminal/collected **passed**: **1,589 library tests** (74.62s)
and **36 integrations** across nine suites (checked identity resolution,
constructor callback values, deferred source views, native functional objects,
retained callback state, retained streams, reusable callbacks, source callback
contracts and wildcard callback carriers). Both Java fixtures independently
return zero. No generated/pinned Java repairs, ignored failures or weakened
coverage. Release V141 is building at
`.tmp/callback-cursor-release-v141.log`; collect, freeze and measure the full
104-file slice before choosing another batch. V140 remains authoritative for
corpus counts. Full goal remains active and incomplete.

Release **24595** is terminal/collected passed (1m32s). Frozen producer
`.tmp/new-java2rust-v141-callback-cursor-contracts`, **21,900,344 bytes**, SHA-256
`0e1a510021701864268f782b5b7f3954c6d0e7eaa8a96c83c766e0de110e8045`.
Isolated 104-file runner **75875** is live at
`.tmp/jgrapht-core-v141-callback-cursor-contracts-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v141-callback-cursor-contracts`. Source frozen;
no competing tests/builds. Collect this runner before claiming corpus progress.

### V141 collected; iteration and call-staging batch selected

Runner **75875** is terminal/collected failed at generated Rust cargo check.
All **104** files translate, **zero translator failures and zero generated
parse failures**. Diagnostics **1,086 = 1,026 typed + 60 untyped**: the total is
unchanged versus V140, with **12 removed / 12 added**, report
`.tmp/jgrapht-v140-v141-diagnostic-delta.txt`. Markers **58 in 27 files**
(V140: 57), allocation guards **22** (V140: 20). These increases are explicitly
not acceptance or permission to suppress emitted bodies. No processes remain
live. Total **263.062s**, translation **181.343s**, assembly **63.865s**, check
**13.481s**, sweep **3.694s**: comparable to V140, no optimization restart.

The exact intended rewrites are present: LiveIterableWrapper.iterator and its
companion now return `JavaIterator<Option<E>>`; its mismatch disappears. The
four missing borrowed AsSubgraph get_edge/get_all_edges/remove_vertex errors
also disappear. Other removals include GraphWalk, BreadthFirstIterator and
CrossComponentIterator cursor result mismatches.

Additions inspected by enclosing function, not merely total counts:

- Six AsSubgraph errors arise in restored borrowed bodies: two old native
  allocation guards in remove_vertex, raw stream/contains calls in get_edge
  and degree_of, and overlapping receiver/argument borrows in
  set_edge_weight_v_v_double. The stream/guard families existed before V140
  suppressed these bodies; keep them visible and fix the contracts.
- Four Graphs neighbor/predecessor/successor methods now pass
  `Option<Option<E>>` where `Option<E>` is required. Their enhanced-for
  iterator actually produces the qualified Option<E>, but
  `lower_source_iterable_iterator_expr` still derives its produced element
  from `plan.element_type` (nominal E), then loop binding conversion wraps
  `next_result()` in Some. Required rewrite is to use the selected iterator
  call's physical return child shape; never strip null or special-case E.
- Two BiconnectivityInspector errors in perform_lazy_inspection hold a
  mutable borrow of self.graph while evaluating get_edge_source/target on
  the same field. The newly qualified result changed the compiled expression
  enough to expose these loans; semantic evaluation order requires retaining
  the receiver object, evaluating arguments with short loans, then invoking
  the selected method. This is the same call-staging family as the restored
  AsSubgraph nested self call.

Next substantial batch: materialized source/native iteration element contracts
through enhanced-for binding and calls, plus short-loan receiver/argument
staging. First extend the maintained nullable cursor fixture with foreach
values passed to a nullable-accepting source helper (to detect double wrapping),
and add a minimized mutable source-interface field whose nested arguments
observe/reassign that field. Reuse the existing preferred call-result shape,
materialized-read projection and staged source-call machinery. For a borrowed
`this`, arguments can be staged before acquiring the Rust receiver loan since
Java's receiver value is already known; for a field receiver, retain its
canonical reference before arguments and delay its null failure until after
argument evaluation. Do not clone payloads or retain Ref/RefMut guards.
Validate exception order and receiver replacement, not just compilation.

Native-SAM factory result ownership/scoped capabilities, Pair qualified key
domains and external inherited AbstractSet protocols remain queued. V141 is
the latest frozen measurement; full goal remains active and incomplete.

### Physical iteration and short-loan calls (in progress)

The expanded deferred-view fixture checks foreach through both a source
Iterable implementation and a chained native Iterable result, passing nullable
elements to a source helper. Frozen V141 fails this maintained input with three
type errors, including two double-Option string accesses. A new maintained
`source_call_staging.java` checks a read-only outer self call with a mutating
later argument, same-field interface calls, field replacement during arguments,
getter-produced receivers, receiver identity, delayed receiver NPE and early
argument exceptions/unboxing. Both Java oracles return **0**.

V1 build passed (50.24s); it removes all seven nested-call borrow errors, but
two field-read moves remain. V2 build passed (49.64s); the staging fixture now
compiles, but execution exposes an infallible replacement() ABI which panics
on null. The stronger iteration fixture's double wraps are gone; a source
parameter returned as a native lazy Iterable still borrows its payload rather
than retaining the canonical source handle. These are general contracts, not
reasons to weaken either fixture.

Current V3 build **95155** is live at
`.tmp/iteration-call-staging-build-v3.log`. It reuses RetainedByCall for a
source receiver converted to a native SAM, so existing escaping-storage and
representation policies can own the handle. The existing shaped immediate
effect walker now checks nullable call receivers and argument unboxing using
solved shapes, exact call/argument plans and Java catch scopes, rather than
only nullable parameter names. Staged unboxing uses the shared catchable null
projection. Core changes also project foreach elements from the iterator's
physical result and use the shared source-interface reference-copy proof for
receiver snapshots and owned field reads. No corpus rerun until full minimized
behavior and relevant gates pass. V141 remains the latest frozen measurement.

V3 build **95155** passed (49.52s). The staging oracle now **passes**
(14.17s), including canonical receiver identity under field replacement,
left-to-right argument effects, null receiver rejection after arguments,
argument exception precedence and nullable unboxing before later effects.
The expanded deferred-view suite also **passes** (two tests, 15.05s),
including both previously failing foreach shapes and canonical-handle retention
through a source forwarding parameter. The existing non-throwing interface
override control still passes; nullable receiver effects did not make that
control fallible.

Added a shaped-null-check unit covering bare/explicit field receivers,
primitive-formal unboxing, local shadowing, Java catches, deferred lambda
creation and null-qualified static calls. Added a canonical RetainedByCall
assertion for the source-to-lazy-Iterable forwarding parameter. Initial gate
build **35920** failed only because the latter test called find directly on
the Vec returned by all_binding_facts; the test now calls into_iter first.
No product change for that compiler error. The corrected gate build is live
at `.tmp/iteration-call-staging-build-gate-v2.log`; collect before running
the full library and ten selected integration binaries. No corpus rerun yet.

Corrected gate build **21109** passed (50.21s). The shaped-null-check control
passes (including static-call and deferred-creation negative controls).
Library **72946** collected **1,586 passed / four failed** (73.01s); all four
failures are old emitted-spelling assertions: three require panic-based expect
instead of the new checked Java NPE projection, and one requires an old raw
list get spelling instead of the now materialized bounds-checked read. The
assertions now require checked NPE/bounds projections; no-move/no-payload-clone
negative controls remain unchanged.

Integration runner **55274** collected **30 passed / one failed** across ten
suites. The only failure is the trait-reference reborrow harness assuming a
plain integer from boxed_run(); its nullable source getter dereference now
correctly owns a Result ABI. The harness now unwraps and still requires the
exact value 5 and original mutable-storage reuse. No product changes after
these gates. Final gate build **96848** is live at
`.tmp/iteration-call-staging-final-gate-build.log`: rerun the four updated
library assertions, trait-reference suite, and retained-map views (including
source carrier identity and bounds-before-arguments behavior). Reuse the
unchanged-product evidence for the other 1,586 library and 30 integration
passes rather than repeating those expensive gates. V141 remains the latest
frozen corpus measurement; full goal active.

Final gate build **96848** passed (50.22s). All four updated library controls
pass; trait-reference reborrow suite passes after its harness ABI update.
The retained-map suite passed five tests and failed two: its size-only generic
key harness also assumed a plain integer and now uses the existing Value
helper (still exact zero, still NoClone key); the other failure is a **real
regression** introduced by preferring nominal external return shapes over
native view inference. `map.values()` was lowered as an Rc-backed retained
view but sent down plain `.iter()` because the preferred external summary
discarded the inferred backing allocation.

The iteration selector now preserves positively inferred shared native
allocation shapes before consulting preferred call summaries. This restores
the retained map view protocol without weakening qualified source iterator
results. No generated Rust repaired. Retained gate build **61551** is live at
`.tmp/iteration-call-staging-retained-gate-build.log`. Collect, then run the
full library (one product follow-up since the previous library run) and the
five selected suites: retained map views, trait-reference reborrows, source
call staging, deferred source views and retained streams. Earlier unaffected
integration evidence can be reused; no release/corpus run until these pass.

Retained gate build **61551** passed (50.10s). Fresh full library **27010**
passed **all 1,590 tests** (85.00s). Final five-suite runner **73288** is
terminal/collected **passed**, including all seven retained-map regressions,
both deferred-view regressions, all twelve retained-stream tests, the staging
oracle and trait-reference reborrows. Combined with applicable earlier
unaffected results, **38 unique integration tests across eleven suites** pass.
All intermediate failures above were fixed or their ABI-specific assertions
updated to require the new checked behavior; no remaining gate failure is
ignored. The Java oracles independently return zero.

Release V142 is now building at `.tmp/iteration-call-staging-release-v142.log`.
No tests remain live. Collect/freeze and measure on the untouched 104-file
slice before selecting further work. V141 remains the latest frozen corpus
result; selected-slice acceptance, graph behavior and expansion are unproven.

Release **34007** is terminal/collected passed (1m43s). Frozen producer
`.tmp/new-java2rust-v142-iteration-call-staging`, **21,909,128 bytes**, SHA-256
`801cfca9b17adc32497f4d1a33109d594ee467a0c25b51fc81c16396f02a06e7`.
Isolated 104-file runner **7794** is live; log
`.tmp/jgrapht-core-v142-iteration-call-staging-run.log`, output root
`.tmp/jgrapht-core-1.5.3-focused-v142-iteration-call-staging`. No competing
tests/builds. Collect before claiming corpus improvement; V141 remains the
latest completed measurement. The full objective is still active/incomplete.

### V142 collected; physical staging follow-through and bounded cost audit

Runner **7794** is terminal/collected failed at generated Rust cargo check.
All **104** files translate, **zero translator failures and zero generated
parse failures**. Diagnostics **1,076 = 1,021 typed + 55 untyped**, versus
V141 1,086: **77 removed / 67 added** in the normalized diagnostic multiset,
report `.tmp/jgrapht-v141-v142-diagnostic-delta.txt`. Markers **54 in 25 files**
(was 58 in 27); allocation guards **17** (was 22). All **38** prior `cannot
borrow *self as mutable` errors disappear, as do the two targeted self.graph
immutable/mutable conflicts, another mutable graph conflict and a repeated
graph mutable-borrow conflict. Graphs' double-wrapped loop values are gone.
No assembled acceptance, graph behavior gate or expansion is claimed.

Total **387.080s**, translation **240.331s**, assembly **112.970s**, check
**24.502s**, sweep **8.554s**, versus V141 263.062s overall / 181.343s
translation. This is a material measured slowdown, but not yet attributable
solely to translator code: assembly/check/sweep also slowed. Raw generated
Rust grew only **56,085,071 -> 56,325,279 bytes** (~0.43%); assembled generated
modules **4,897,437 -> 5,137,107 bytes** (~4.89%); shared support
**696,009 -> 696,524 bytes**. These size changes alone do not explain the
observed timing ratios. No competing tests/builds were run, but external host
load has not been excluded. No processes remain live.

There is also a concrete avoidable repeated-work path in the new effect scan:
`block_has_unhandled_shaped_call_null_check` calls
`plan_call_target_with_facts_and_arity` per call; that constructs a fresh
ExternalFactLookupIndex, whose lazy alias index scans the full fact set on
first lookup. Reuse the existing indexed call-planning API and
ExprShapeContext.with_external_index within the immutable pass; examine the
argument planner's analogous repeated index creation. Reject non-interface
receiver snapshots cheaply before repeated physical shape inference where
sound. This is a **bounded follow-up on newly measured work**, not permission
to restart an open-ended optimization phase. Measure CPU/wall or controlled
old/new comparisons before attributing the whole slowdown to those paths.

The 67 additions must not be ignored. Inspected examples identify shared
physical-contract follow-through:

1. Eight TraversalListener mutable-box Clone errors: a loop variable `l` is
   physically `&mut Box<dyn TraversalListener<...>>`, but snapshot eligibility
   infers an owned box and emits `Clone::clone(&l)`. Fix producer/binding
   physical ownership metadata and reuse canonical reference copying; do not
   clone the mutable loan or replace it with a fresh listener payload.
2. Four temporary-lifetime errors include AbstractGraph.to_string staging
   `self.vertex_set()?.as_ref()` into a temporary reference. Keep the owned
   producer alive, then derive its borrowed argument after ordered producer
   evaluation. Staging a loan alone is not an owned Java-reference snapshot.
3. Twelve additional `Option.borrow()` errors expose required collection
   materialization checks: e.g. GraphMetrics builds a vector from
   `graph.vertex_set()?.borrow().cloned_items()`, although vertex_set returns
   Option<Rc<...>>. Preserve the selected call's Result and Option separately,
   evaluate once, check Java null and borrow only the present allocation.
4. Four new AsSubgraph propagation-in-bool-closure errors require inspecting
   conversion/stream callback effect scopes. Use a fallible callback contract
   or an effect-preserving branch projection, not outer-context `?` inside a
   plain Rust callback. Do not strip propagation or eagerly run callbacks.

Other additions include 17 mismatches, five extra dyn Any Clone obligations,
and opaque callback/downcast result issues. Next coherent batch should repair
these physical producer/loan/consumer contracts plus the bounded new cost
path, using small runtime regressions before another full measurement.
Earlier queued native-SAM factory ownership/capabilities, Pair key domains,
external inherited AbstractSet protocols and generic runtime-class lowering
are still unsolved, not dropped. V142 is the latest frozen corpus result;
the full goal remains active and incomplete.

### Physical values / retained references batch (in progress after V142)

New maintained fixture `physical_call_values.java` independently returns zero
under Java. Frozen V142 fails it with `&mut Box<ValueListener>: Clone`; the
baseline also shows collection getters cloning Vec payloads and copy() using
an infallible expect on a nullable source. The fixture now checks live returned
collection aliases, staged getter lifetimes, callee-side effects before a null
argument dereference, distinct copy allocation, single source evaluation,
nullable copy NPE, mutable produced arguments and listener reentrancy/CME.

Changes reuse existing mechanisms: iter_mut bindings record MutableBorrow of
the physical carrier; call argument temporaries now carry explicit mutability
and keep owned temporary producers alive before borrowing. Selected source
return ABI and positive native allocation inference share
physical_call_value_shape for sequence reads. Copy-constructor lowering no
longer builds a discarded first argument IR before materializing the actual
source. Stored collection iteration and existing-reference returns seed the
existing retention graph, not payload copies or long-lived field loans. Root
nullable value flow is recorded separately from descendant allocation-domain
sharing, so a callee may receive null and execute its own effects before
dereferencing it. Retained field iteration reuses the canonical owner-handle
field materializer to release owner guards before callbacks. Nullable native
collection-copy sources seed the existing checked-null effect analysis.

The bounded cost follow-up reuses one external lookup index per immutable
shaped-effect pass, passes it through ExprShapeContext and indexed target
planning, and avoids building argument plans when no nullable argument can
unbox. Snapshot eligibility rejects non-interface nominal receivers early.
No claim that these changes explain V142's assembly/check slowdown.

First build **31362** failed only at one remaining test fixture's old tuple
temporary representation; it now uses CallArgumentTemporary, and an unused
local was removed. Build **23650** passed (52.24s). Both source-call runtime
regressions pass (15.11s), including reentrant listener clearing producing
ConcurrentModificationException rather than a RefCell panic or snapshot loop.
The shaped-null and temporary-IR focused units pass. No generated/pinned Java
repairs. The gate candidate also adds resolved implicit-instance-call evidence
to self-read detection, mutable owned-argument coverage and indexed/non-indexed
effect equivalence controls.

Gate build **41495** passed (50.96s). The extended physical-value runtime
control passed (35.48s), including mutable produced arguments. The library
gate recorded **1,588 passed / 2 failed** in
`.tmp/physical-call-values-lib-gate.log`: the list-view identity control must
now check the two directional root-nullability edges, while the hierarchy
collection control exposed unnecessary retention of fresh unique local
returns. Keep that hierarchy expectation: stored aliases and caller-owned
returns require retention, but a fresh unaliased local may move its value
carrier. The retention seed has been narrowed accordingly; the cast control
still forbids return-to-source nullability backflow.

Integration handle **69827** is now missing and no final result was retained;
do not count those suites as passed. Refined combined build **39741** passed
(1m00s), and both formerly failing library controls passed unchanged in scope
(`.tmp/physical-call-values-refined-controls.log`). Full library **13998**
passed **1,590 / 0** in 170.75s, recorded in
`.tmp/physical-call-values-refined-lib.log`. Ten integration suites **91428**
passed **34 / 0**, including the extended physical-value regression; durable
results are in `.tmp/physical-call-values-refined-integrations.log`.
Formatting was applied after the build (whitespace only), and the touched
Rust paths pass `git diff --check`.

Release **19248** passed in 1m52s
(`.tmp/physical-call-values-release-v143.log`). Frozen V143 producer is
`.tmp/new-java2rust-v143-physical-call-values`, **21,935,784 bytes**, SHA-256
`d2482a6fd5d8b23becc591cab1ab9b471fe006ead02da3db34657a6e69253fce`.
Isolated 104-file runner **91590** wrote
`.tmp/jgrapht-core-v143-physical-call-values-run.log`, output
`.tmp/jgrapht-core-1.5.3-focused-v143-physical-call-values`. It has been collected:
**1,078 = 1,013 typed + 65 untyped diagnostics**, **55 removed / 57 added**
versus V142, zero translator/generated-parse failures, **59 source markers in
26 files**, **18 allocation guards**. Total **325.738s** (translation229.849,
assembly66.938, check20.333, sweep7.970). See
`.tmp/jgrapht-v142-v143-diagnostic-delta.txt`. No processes remain live. V143
is now the latest measured result; full goal active/incomplete.

### Retained collection protocol follow-through (next batch after V143)

V143 removes all eight mutable-listener-reference Clone obligations and all
four E0716 argument-temporary lifetime failures. It exposes contracts missing
from the same retained-reference path, not evidence that retaining Java
allocations should be reverted:

1. Listener and graph sets now construct `JavaNativeCollection::set` with
   `JavaMapKey::java_map_key_eq`, but `Box<dyn TraversalListener<V,E>>` and
   `Box<dyn Graph<V,E>>` have no Java key protocol. Ten explicitly typed plus
   six inferred boxed-listener key diagnostics and two graph-key diagnostics
   are added. The general fix must dispatch Java Object equality through the
   canonical source carrier, including overrides; pointer equality of a
   freshly cloned Rust box is not Java identity. Reuse existing source key,
   reference-identity and trait-forwarding mechanisms across implementations,
   shared adapters, box forwarding and consumers. Do not add a blanket
   identity-only boxed-interface key implementation that discards equals.
2. Five inherited listener fields and AsSubgraph's two set fields initialize
   an Rc/RefCell around `Default::default()` before actual constructor writes.
   `JavaNativeCollectionStorage` has no Default. Trace the existing Java
   constructor initialization/nullability proof and retained construction
   path; a global empty-collection Default would invent a Java allocation
   where a field should initially be null. This is an initialization contract,
   not permission to weaken the retained runtime's bounds.
3. Seven new unsupported retained-collection operations are collection
   formatting (six in planarity nodes, one in NeighborCache). The ordinary
   renderer intercepts ToString but its retained renderer lacks it. Formatting
   must use Java element toString behavior and release storage/owner loans
   before element callbacks; borrowing a cell while executing arbitrary Java
   formatting is not safe reentrancy. Reuse cursor/effect and formatting
   mechanisms, rather than emitting an arbitrary Rust debug fallback.
4. Existing GraphMetrics `new ArrayList<>(graph.vertexSet())` still emits
   `graph.vertex_set()?.borrow().cloned_items()` although the source trait
   returns `Result<Option<Rc<...>>>`. The selected *emitted trait ABI*, not
   merely nominal inference or one declaration's summary, must drive the
   presence check and producer lifetime before reading storage. Several other
   retained consumers still call Vec/map methods directly on handles.

A new maintained minimized fixture,
`tests/fixtures/nullable_source_collection_results.java`, independently returns
zero under Java. Frozen V143's actual copy body correctly checks nullable
presence, but the translation fails on its separately emitted generic
interface cast adapter: it attempts `.into_iter().map(...).collect()` on a
retained Rc allocation. The diagnostic is at
`.tmp/nullable-source-collection-v143-rustc.log`; original generated source and
translation log are preserved alongside it. The added source-call regression
has not yet been built/passed. This distinction matters: it is evidence for
the generic retained-carrier adapter gap, not a reproduced GraphMetrics null
check. Reproduce that remaining discrepancy through co-translated source
context before changing precedence again. Generic reference views must retain
allocation identity; element-copy conversion is only valid at an actual Java
copy boundary. Assess reuse of existing erased/list-view projections before
designing another adapter.

These are cross-function carrier/protocol contracts. Implement coherent
producer/initialization/signature/conversion/consumer changes with focused
identity, overridden-equality, null and callback controls before the next
expensive corpus measurement. Earlier callback-effect scopes, native-SAM
factory ownership/capabilities, Pair key domains, external AbstractSet and
runtime-class gaps remain queued; none is claimed fixed by V143.

### Retained protocol consumer implementation (in progress after V143)

Generic retarget eligibility now checks solved root and nested native allocation
contracts as well as ownership. A retained dependent collection needs a live
view; the existing element-value converter must not advertise that capability
or emit an invalid unused adapter. Return-shape checks specialize inherited
owner binders into the interface contract before testing dependent storage.
This is an availability-proof correction, **not** implementation of generic
retained views. Actual unsupported retarget requests must remain explicit.
The new shape control covers ordinary owned values, shared roots, shared
descendants, unrelated binders and borrowed inputs.

Ordinary retained-collection ToString now supplies a typed display callback to
the existing physical renderer. A checked runtime cursor releases allocation
loans before invoking the callback, preserving callback exceptions and live
modification checks. Callback values use existing Java display/source-method
lowering, not Rust Debug or deep-array formatting. Retained formatting joins
the catch-aware RuntimeException effect analysis; its cheap candidate and
name filters must include ToString. Stream-callback and sync formatting still
have explicit gaps and are not claimed implemented by this ordinary-call path.
Boxed-interface key protocols and constructor initialization remain pending.

New fixture `retained_collection_formatting.java` independently returns zero
under Java, checking source toString calls, a null String result, an element
exception and owner/collection reentrancy ending in CME. Build **11558** passed
(1m15s), and the new retarget shape unit passed. Runtime **65486** recorded
**3 passed / 1 failed** at `.tmp/retained-protocol-consumers-runtime-v1.log`:
the nullable generic interface copy now passes, as do both existing physical
call/ordering controls. Formatting failed to compile because String.valueOf's
non-null shared-source path fell through to Rust Display; render also lacked
Result because the early effect candidate excluded ToString. Both missing
connections have been repaired using the existing source display and effect
paths. Refined combined build **48046** passed (52.23s), and runtime **36674**
passed **4 / 0** in 18.37s at
`.tmp/retained-protocol-consumers-runtime-v2.log`. The retarget shape control
also passed on this candidate. The early RuntimeException name filter was
added during the first build; the refined build includes it definitively.
The twelve retained-stream neighbors **71183** passed **12 / 0** (60.24s),
including production shared-runtime assembly, with durable output at
`.tmp/retained-protocol-consumers-streams-v2.log`. Three existing source,
hierarchy-carrier and erased-display controls **66388** also passed, recorded
in `.tmp/retained-protocol-consumers-display-controls-v2.log`. Together with
the retarget unit this is four focused unit controls and sixteen integrations;
it is not a fresh full-library/corpus gate. No processes remain live.
No new corpus run is warranted
until this coherent batch is verified and the remaining related contracts
have been addressed. V143 remains the latest frozen measured corpus result.

For the remaining boxed-interface key contract, inspecting the existing erased
equality helper rules out a tempting shortcut: `java_erased_any_equals` handles
primitive/string values but deliberately panics on unknown source objects.
Forwarding every boxed interface key to it would compile yet fail for source
listeners, especially generic instances. The existing `as_any` forwarding
already exposes canonical shared handles, which should be reused by a real
object-equality dispatch contract; cloned Rust box addresses are not a fallback.
Likewise, a collection Default must not be introduced merely to satisfy
constructor placeholders. Trace field initialization order and early receiver
exposure before choosing initialization staging versus a genuinely nullable
construction-time field slot. These are the next related implementation
steps, not additional completed capabilities.

### Constructor initialization prefix (in progress after V143)

Constructor lowering now stages the leading receiver-independent field
assignments before constructing the Rust struct. Eligibility requires that
the original literal contain only known compiler placeholder defaults (or
the inert generic marker); it stops at the first unknown statement, receiver
read/escape, raw expression or repeated field. Source expression evaluation
order is retained in fresh locals, with parameter/local-name collision
avoidance. This removes the placeholder type obligation where Java's actual
initializer can execute before the receiver is first needed; it does not add
an empty native-collection Default, hoist across a callback, or claim to solve
construction-time nullability for fields observed before initialization.

New fixture `retained_constructor_initialization.java` independently returns
zero under Java. It covers superclass/child initialization order 12345, a live
returned list alias and an initializer exception stopping at order 12.
Frozen V143 fails with a retained-storage placeholder mismatch and five
inherited-static owner errors (`.tmp/retained-constructor-initialization-v143-*`).
Build **91658** passed (49.55s); the new prefix-staging unit passed. Runtime
**35494** had 2 passes and 1 failure: the placeholder mismatch was removed,
leaving the five inherited-static calls emitted as `Self::mark` in the child.

Build **17606** passed (49.96s), adding the distinction between lexical owner
and emitted constructor frame. Its routing unit passed, but runtime **57148**
still failed. Build **94573** passed (49.49s), binding resolved static owners
earlier in the shared call planner; three focused controls passed, but runtime
**50619** still had the same five owner failures. These changes alone were not
the root-cause correction. Do not keep rebuilding variants of that assumption.

A linked diagnostic driver and gdb trace of the already-built translator found
the exact failed lookup without another instrumented translator build:
`.tmp/constructor-static-probe-lookup.log` shows both primary and support
programs contain `RetainedConstructionBase`. Strict combined lookup rejects
their distinct AST addresses even though they describe the same Java owner.
The retargeter now reuses the existing preferred-emitted owner lookup first,
with conservative fallback. Its control includes duplicate copies of one owner
as well as refusal of genuinely ambiguous package peers. No global weakening
of strict owner lookup is being introduced.

Build **53005** passed (49.43s). Runtime **62526** passed **3 / 0** in 12.52s
at `.tmp/constructor-prefix-staging-runtime-v4.log`, including the unchanged
new Java-order/exception oracle. Three focused controls also passed at
`.tmp/constructor-prefix-staging-controls-v4.log`, including the expanded
same-owner-copy and ambiguous-package-peer cases. Source-call integrations
**90034** passed **4 / 0** (17.78s), recorded in
`.tmp/constructor-prefix-staging-source-calls-v4.log`. Broader library **21621**
finished **1,591 passed / 1 failed** (142.93s) at
`.tmp/retained-protocol-constructor-lib-gate.log`. The sole failure is
`optional_source_supertype_field_assignment_uses_carrier_handle`: its rendering
assertion required a default field followed by `__java_self.value = ...`.
The actual output stages exactly the same Child-to-carrier conversion into
`__java_initial_field_0`, then initializes the struct field directly. The
assertion now checks that form while retaining its carrier-type and no-gap
checks. Evidence is in `.tmp/constructor-prefix-carrier-control-v4.log`.
This test-only edit has **not yet been rebuilt/re-run**; include that control
in the next combined build rather than paying another full build/suite cycle
just for an observed literal-form expectation. Production code is unchanged
since the collected v4 gates. No processes remain live. V143 remains the
latest frozen corpus result; no pinned/generated output repairs.

Next: complete the boxed-interface Object equality contract before another
corpus measurement. Inventory both dynamic dispatch and effects: the current
`JavaMapKey` method and retained projection comparator return plain bool,
whereas a source `equals(Object)` may throw. Any general interface key bridge
must preserve canonical handle identity, overrides and exception behavior,
not merely add a bool pointer-comparison impl to make the new set calls compile.

### Checked equality producer/consumer contract (in progress after V143)

`JavaMapKey` now has a checked result method, following the existing
`JavaTreeKey` pattern. Its legacy bool method remains available; checked
reference and Option forwarding must preserve overrides rather than silently
falling back to bool. Source hierarchy carriers emit the checked method using
the selected Java equals override and its existing exception plan/bounds;
identity-only hierarchies return a checked identity result. The retained
projected search has a checked-comparator entry point and releases storage
loans before comparison; the old bool entry point delegates compatibly.
This is a foundation for boxed Object dispatch, **not** completion of boxed
interface key support or migration of all ordinary map/set operations.

The existing checked-hash runtime control now also checks equality error
propagation, nullable/reference forwarding, query-versus-stored direction,
short-circuiting and reentrant nonstructural mutation. New source fixture
`checked_key_equality.java` independently returns zero under Java and exercises
an inherited source equals override, a null Object and a throwing override.
Its Rust probe invokes the checked key protocol in addition to the Java run
method, rather than merely verifying compilation.

Build **39613** passed (1m00s). Controls **27689** passed all four selected
tests, including the previously updated constructor rendering assertion.
Integrations **22205** had **10 passed / 1 failed**: the new source oracle
returned 1 because ordinary Object erasure boxed the owned hierarchy enum,
but instanceof checked an extra Rc around that enum. Frozen V143 compiles
this fixture yet has the same incorrect owned-carrier predicate; diagnostics
alone would miss it. Logs: `.tmp/checked-key-protocol-*-v1.log` and
`.tmp/checked-key-equality-v143-*`.

Build **96027** passed (54.88s). Representation-aware instanceof now uses the
emitted value-carrier ABI, with optional erased-object support shared by plain
checks and pattern projection. The initial cast follow-through handled the
ordinary expression path but missed nullable/target-slot coercion: runtime
**74186** failed with CCE after the corrected positive guard. The fix is now
in shared coercion: both paths reuse owned-carrier recovery alongside payload
handle recovery. Build **23238** passed (52.26s); source oracle **96188** passed
(12.09s), including null and thrown-source-equality behavior. Checked runtime
and constructor assertion controls **43058** also passed.

The owned recovery's runtime type must carry instantiated generic arguments,
unlike a variant constructor that can infer them. Cast policy and coercion now
record `carrier_type` separately from the enum constructor name, reusing the
fact-aware generic-argument renderer. The generic rendering control checks
that instantiated type metadata. Build **36334** failed only at an integration
fixture literal missing the new `optional` flag; this literal was updated.
Build **11065** passed (2.13s), reusing the already-built producer code.
Integrations **2467** passed **11 / 0**: four Java-hash contracts, three
constructor boundaries and four source-call staging tests. Focused controls
**30217** passed four selected library controls and seven erased-object
conversion neighbors. Logs: `.tmp/checked-key-protocol-integrations-v5.log`
and `.tmp/checked-key-protocol-controls-v5-collected.log`. No corpus rerun
yet; V143 remains the latest frozen measured result.

The next boxed-interface bridge must dispatch Java Object behavior through
canonical concrete handles, including generic instances, not the unknown-type
erased equality fallback or cloned-box addresses. In the pinned corpus,
AbstractGraph overrides equals and compares graph structure after class and
identity checks, so an identity-only Graph key implementation is incorrect.
Keep the existing explicit generic live-view gap in scope when connecting that
Object boundary; do not replace it with element-copy retargeting.

### Checked bulk membership consumers (after V143)

The retained/mixed `containsAll`, `retainAll` and `removeAll` lowering now
selects the checked JavaMapKey method rather than the legacy bool adapter.
Retained filtering shares the existing cursor/partial-progress machinery;
checked equality runs without borrowing either allocation. Legacy bool runtime
entry points delegate through compatible wrappers. Mixed plain-source search
uses a checked, short-circuiting iterator helper instead of `Iterator::any`,
so a Java exception does not become a Rust panic or an illegal `?` in a bool
closure. Existing optional-element matching rules remain unchanged; this does
not claim complete nullable Object equality semantics.

The expanded `checked_key_equality.java` returns **0** independently under
Java. It checks throwing query equality in containsAll and partial removals
before exceptions in both ArrayList retainAll and removeAll, with retained
aliases. Its generated Rust oracle passes too. The mixed-membership integration
adds checked-runtime reentrant mutation of both allocations, error propagation,
partial progress and short circuiting, while retaining existing bool-API controls.

Build **40579** passed (57.74s), log
`.tmp/checked-bulk-protocol-build-v1.log`. Four hash/equality integrations passed
(19.35s) at `.tmp/checked-bulk-protocol-integrations-v1.log`. That shell runner
then failed only because it attempted a nonexistent old mixed-test binary name;
the actual built mixed-test binary was run separately as **37544** and passed
(15.67s), log `.tmp/checked-bulk-protocol-mixed-v1.log`. Checked-runtime control
**97476** passed, log `.tmp/checked-bulk-protocol-runtime-v1.log`.
Broader library **92795** passed **1,592 / 0** (272.15s, four test threads),
log `.tmp/checked-bulk-protocol-library-v1.log`. This includes the constructor
assertion follow-through and the checked producer/conversion/consumer changes.
Formatting and `git diff --check` passed. No gate remains live.
No new corpus measurement or generated/pinned source repairs. V143 remains
the measured corpus baseline.

### Boxed-interface Object contract: concrete rewrite audit

New diagnostic source `tests/fixtures/interface_object_contracts.java` returns
**0** under Java (`.tmp/interface-object-contracts-java-oracle.log`). It uses
an identity-equality implementor, a structural cross-implementor override and
a throwing override through one interface. Both implementors deliberately
return hash 1, so LinkedHashSet membership actually invokes the relevant query
equals rather than skipping comparison due to different Java hash buckets.
It covers cloned interface aliases, asymmetric equals, interface hash dispatch
and a catchable equality exception. This is an **unfixed audit probe**, not a
passing maintained translation test.

Linked current-library translation (no translator rebuild) and direct rustc
produce **13 errors**, including two explicit unsupported Object-to-interface
checks/casts, three identity operator errors, four missing equals calls,
missing JavaHashCode and three set operations lacking JavaMapKey. Evidence:
`.tmp/interface-object-contracts-baseline.rs`,
`.tmp/interface-object-contracts-translation.log`,
`.tmp/interface-object-contracts-rustc.log`. Counts are manifestations, not
13 independent bugs. No generated code was hand-edited.

Producer-to-consumer findings and required rewrite scope:

- Interface values already box canonical Rc concrete handles, and clone_box
  preserves those handles. Existing as_any exposes the handle, not the adapter.
  Preserve these producers rather than comparing Box addresses or cloning
  payload values. A canonical owned-erasure operation is missing: borrowing
  as_any alone cannot provide the owned Object argument required by source equals.
- The trait surface lacks virtual Object equality/hash/identity capabilities.
  Implementor adapters must select source overrides using their existing
  signatures and exception plans. Shared, boxed, inherited and generic adapter
  paths must forward the same contract; generic arguments must not be erased
  by assuming that every instance has Object type parameters.
- Identity operators must use canonical reference identity separately from
  equals; structural equals cannot implement Java ==. Boxing another alias must
  not change identity. Use/generalize existing JavaReferenceIdentity machinery.
- Source equals can inspect/cast its Object argument to an interface. Current
  Any erasure retains a concrete handle but cannot recover its interface
  projection. A dynamic Object bridge therefore needs checked interface recovery
  as well as equality dispatch. Pinned AbstractGraph adds invariant generic
  interface recovery and retained views to this boundary; neither unknown-Any
  equality nor copied generic collections is an adequate substitute.
- Ordinary semantic sets/maps and retained set comparators still have bool
  consumers. Adding only a checked trait method would leave catch behavior
  wrong. Migrate their operation/effect contracts together and release loans
  before invoking Java code. The checked bulk work above is reusable machinery,
  not a claim that these remaining consumers have been migrated.

This is a cross-function Object representation/signature/adapter change, not
a local impl-generation fix. Next batch must address the complete dynamic
boundary and maintain the above behavior oracle, with generic/inherited and
nullable cases added as those paths are connected. Some nullable query/stored
comparisons currently bypass source equals(null); keep that explicitly open
rather than claiming the checked key default resolves all null behavior.

### Canonical owned interface erasure (verified after checked bulk batch)

The next missing producer capability is now implemented, without pretending
that all virtual Object dispatch is solved. Source interfaces expose
`clone_any` beside `as_any` and `clone_box`. The first returns an owned clone
of the **canonical concrete carrier**, not a Box containing an interface view.
Cell/mutex adapters clone handles without opening a guard or cloning a payload.
Direct generic Rc/Arc wrapper carriers expose/clone their inner handle. Boxed
forwarders delegate using the selected interface, inherited interface-upcast
wrappers delegate to their stored view, and invariant-generic retarget views
delegate to their original view. These latter two borrowed `as_any` paths also
now expose the original object instead of their adapter structs.

Named and anonymous producer implementations share the same support-method
generator. Five previously handwritten anonymous/lambda support surfaces in
`lower.rs` were replaced with that generator, avoiding another partial ABI
update. Raw non-Clone payload behavior follows the existing clone_box support
policy; canonical shared-handle paths do not require payload Clone. This work
does not claim to normalize every native-SAM identity representation.

Shared conversion planning selects a distinct owned interface-to-Object
operation with instantiated interface metadata. Its virtual result is already
owned: no clone of a borrowed Rust Box, second box, or ownership recovery on
the erased result is emitted. Proof requires a parsed-source object-safe
interface, excludes externally mapped traits and already-erased values, and
currently covers **owned Object destinations**. Optional mapping reuses the
ordinary element conversion. Borrow-only Object destinations, ordinary Object
virtual methods, interface recovery and nullable equality remain open.

New maintained `interface_object_erasure.java` independently returns **0**:
mutable alias recovery through inherited interface views, direct child views,
immutable handles, nullable interface-to-Object return and anonymous/lambda
producer compilation. The generated behavior test passes too. Its Rust probes
also verify canonical type/identity with generic non-Clone payloads and held
RefCell/mutex loans, direct local/sync wrappers, generic retarget views and
boxed forwarding. No generated output or pinned Java was edited.

Build **8982** passed (1m04s), with initial oracle **41334** passing (17.22s).
Initial representation emission controls **71750** had 42 passes / 3 failures:
exact output literals lacked the new method; those expectations were updated
without removing their existing checks. Follow-through build **55519** passed
(1m01s), log `.tmp/interface-object-erasure-build-v2.log`. Representation
controls **78762** passed **71 / 0** (16.81s), including those three controls,
at `.tmp/interface-object-erasure-library-v2.log`. Integrations **42989** passed
the new erasure test, four hash contracts and the mutable trait-reborrow test
(six top-level tests); log `.tmp/interface-object-erasure-gates-v2.log`.

The same runner's full conversion suite had 119 passes / 1 failure: the
optional string display AST assertion omitted the existing PlainValue metadata
wrapper (producer comment and output explicitly require that proof). Its
expected AST now includes that wrapper while retaining the complete expression
check. Test-only build **63398** passed in **3.26s**, reusing producer artifacts.
Final **98896** passed **120 / 0 conversion tests** and the strengthened generic
erasure oracle (13.54s), at `.tmp/interface-object-erasure-gates-v3.log`.
Java log: `.tmp/interface-object-erasure-java-oracle.log`. Formatting and diff
checks pass. No build/test/corpus process remains live; V143 remains the frozen
corpus measurement. The preceding full-library 1,592/0 evidence predates this
producer batch; do not relabel it as a fresh full gate for these changes.

Next full-contract decision: canonical owned erasure alone cannot provide
arbitrary erased-interface recovery. In particular an implementation may have
generic parameters not recoverable from the target interface's parameters;
enumerating guessed concrete Any types at the cast site is not general proof.
Before connecting equality, choose how the Object carrier retains both its
canonical reference and recoverable interface views/virtual capabilities.
Producer-side type instantiations know those extra parameters, whereas a
consumer-side cast table does not. Reuse the new canonical clone operation
for any such descriptor rather than replacing it with payload cloning. Keep
checked dispatch, canonical identity and comparator loans in the same design.

### Concrete source equality and ordinary set consumers (in progress)

Before virtual interface dispatch, close its concrete-key prerequisite.
`source_java_map_key_impl_item_for_type` now selects custom source equals on
canonical concrete handles instead of declining all overrides. Equality and
hashing share `source_object_protocol_method_call`: selected ABI, ancestor
specialization, scoped method bounds, checked result and handle companion.
Other-key erasure explicitly clones Rc/Arc, not an inferred reference to it.
Generic Any erasure adds scoped `'static` obligations, not payload Clone.

Hash/set matching needs reference identity *separately* from explicit equals.
`JavaMapKey::java_map_key_identity_eq` defaults false, forwards through references
and Options, and source concrete/hierarchy carriers implement canonical identity.
Ordinary semantic hash sets and linked sets share checked query-first lookup,
then mutate only after success. Legacy bool entry points delegate compatibly.
Lowering and effect analysis share the physical semantic-set operation proof
for add/remove/contains; caught Java exceptions are not Rust panics. This does
**not** migrate ordinary maps, bulk constructors/extend, retained allocation
comparators or boxed-interface Object dispatch. Null overrides remain open.

New maintained `concrete_checked_keys.java` has independent Java **0** results
for ordinary and static-root variants. It checks asymmetric equality/query
direction, insertion/removal atomicity on exceptions, source-caller propagation,
receiver reentrancy, identity shortcuts versus an explicit throwing self-equals,
and immutable generic key parameters. Rust protocol probes include nullable/
borrowed identity forwarding and generic non-Clone type arguments. A separate
static-root fixture makes storage genuinely Arc/Mutex; its runtime test is
bounded so a callback mutex deadlock cannot hang verification indefinitely.

Initial build **3382** passed (1m02s), but hash integration **70401** had
3 passes / 2 compile failures: cloning `&other` erased a borrowed reference,
not the owned handle. Explicit handle clone fixes this. Build **66050** and
hash **91630** passed **5 / 0** (18.50s). Effects **57877** passed **32 / 0**;
interface erasure **49330** passed. Generic strengthening then exposed a
separate alias bug: a non-Clone T vetoed cloning Rc<Generic<T>>. Source/native
reference and all-handle hierarchy proofs now share one clone-recovery test
across assignments and call arguments. This is physical handle capability,
not a new T: Clone requirement.

Build **70597** passed (54.98s). Its broad library **89349** passed
**1,592 / 0** in **169.64s**, log `.tmp/concrete-checked-keys-library-v3.log`.
This predates the following ownership/sync follow-through, not a fresh proof
of those edits. Initial generic oracle failed on the missing alias clone.
Build **24815** passed (58.34s). Generic library **2651** passed **71 / 0**
(17.24s); generic carrier/reference/erasure integrations passed **7 / 0**.
The concrete source oracle including the new non-Clone probe passed too.

The first sync test setup used external storage facts, which did not select
an Arc carrier in this pipeline. Replace that setup with an actual Java static
root, not a relaxed type assertion. Test-only build **65120** passed (1.01s).
Real sync test **21824** then exposed six compiler errors: two local/static
nullability collisions and four payload-versus-handle parameter mismatches.
Bare static semantic-name lookup and static shape lookup now respect lexical
locals. Source Object dispatch now uses generated companions for both local
and sync handles: those companions already support Arc/Mutex projections.

Build **92243** passed (53.81s). Signatures **93482** passed **145 / 0** (5.13s),
neighbors **83246** passed **7 / 0**, and five hash contracts passed; sync still
had four parameter mismatches. Removing the solver's old sync exclusion alone
was insufficient: a second post-solver emission normalizer still admitted only
Rc<RefCell> parameters. Both authorities now preserve canonical **all-shared**
source parameters, including immutable Rc and Arc/Mutex, rather than lending
payloads to operations requiring Java reference identity. The new immutable
generic search oracle exercises that same contract, not just mutable keys.

Build **67342** passed (1m10s). Hash **25391** then had 5 passes / 1 **runtime
timeout**, no compiler errors: the real Arc probe exposed a mutex temporary
retained across `this.value == other.value` for the same object. Signature
controls passed **145 / 0** and ownership/erasure neighbors passed **7 / 0**.
The ordinary receiver field reader already had a named-guard drop scope; it is
now factored into one scoped reader and reused for primitive current-receiver
reads. Writes keep a distinct mutable place. Initial v8 failed because an
unqualified field increment still treated its operand as an ordinary local
and tried to assign to the new read block. That update now captures its mutable
field place once, reusing the existing primitive update planner, as explicit
receiver.field increments already do. No string repair or lifetime extension.

Build **71872** (v9) passed (1m03s). Hash **35953** passed **6 / 0** (16.15s),
including the real Arc/Mutex bounded runtime, explicit self-equality, immutable
generic key parameter, and an equals(int) overload before equals(Object).
Neighbors **22208** passed **7 / 0**. Logs:
`.tmp/concrete-checked-keys-hash-v9.log` and
`.tmp/concrete-checked-keys-neighbors-v9.log`. The overload check required
source selection to reuse semantic analysis's Object-override predicate;
lowering diagnostics now delegate to that same predicate too.

Selector follow-through: declaring-owner override facts do not necessarily
include a child which merely inherits equals. The source declaration/ancestry
lookup no longer mistakes a missing child fact for default identity. A new
focused selector control tests inheritance and overload rejection with no
pre-populated owner facts. Build v10 failed at that new unit's wrong resolver
helper invocation (API requires &program plus diagnostics). Corrected test-only
call; **35072 / v11 passed** (1m18s), log
`.tmp/concrete-checked-keys-build-v11.log`. Final behavior **49539** passed
**13 / 0** (six key/hash, one generic-carrier borrowing, five generic-reference
identity, one interface-erasure integration), at
`.tmp/concrete-checked-keys-integrations-v11.log`. Both Java variants return 0
at `.tmp/concrete-checked-keys{,-sync}-java-oracle.log`.

Broad library **3754** finished **1,592 passed / 1 failed** (189.12s), log
`.tmp/concrete-checked-keys-library-v11.log`. The sole failure was the rendering
test `outer_handle_companion_passes_mutable_parameter_by_binding_name`:
the primitive-field mutation now correctly requires an inlined handle-body,
not the former thin delegate. The fixture now has both that method and a
local-only mutable-parameter method which still delegates; it checks correct
argument binding names there and separately checks the captured mutable field
place in the inlined companion. No existing semantic checks were removed.
Test-only build **20960 / v12** passed (59.21s). Focused control **28963** passed
(9.29s), at `.tmp/concrete-checked-keys-companion-control-v12.log`.
Do not call these cumulative results a fresh full 1,593/0 run: production code
is unchanged since v10, and only the failing test fixture/assertion changed.
Formatting and diff checks pass. No test job remains live.

The signature/guard-scope changes are now substantial enough that a fresh
corpus result is useful before designing the full dynamic Object descriptor.
Release build **45842** passed and V144 was frozen/measured. Its 104 translations
completed, but a new assignment-place parse regression makes the provisional
1,091 diagnostics unsuitable as a typed-error baseline. Registry markers fell
59 to 54; see `docs/jgrapht-core-corpus.md` for immutable producer identity,
timings and the diagnostic delta. V143 remains the latest parser-clean baseline.

The assignment follow-through reuses the existing write-place builder and
staged RHS for assignment values and nullable predicates. Current-self scalar
reads require proven receiver-layout ownership, resolving the captured outer
field mix-up without disabling guard-scoped reads on duplicated context copies.
Debug v3 passed seven behavior integrations and 48 assignment controls; the
untouched BoyerMyrvold single-source output parses. Release **4072** passed
(2m14s); V145 is frozen and isolated corpus **12160** finished. All 104 files
translate with zero generated parse failures; four targeted diagnostics are
removed with none added versus V144. Cargo check remains failing (1,087
diagnostics). Producer identity, timings, paths and scope are recorded in
`docs/jgrapht-core-corpus.md`. Next probe traces inherited current-this calls
and companion-only bodies, keeping concrete receiver ownership distinct from
declaration-level exception and signature contracts.
No generated/pinned repairs.

Object design finding: `JavaErasedValue` already retains a runtime value and
its producer-supplied clone operation. Extend/reuse that mechanism for checked
source operations and typed interface views; do not introduce an unrelated
second descriptor or guess missing generic parameters at cast sites. Projection
capabilities must come from producer-side instantiated source types, with
source-method bounds propagated through adapters. That larger interface/Object
contract remains the next batch, not completed by the concrete-key work above.

### Inherited current-self call contracts (in progress after V145)

V145's parser-clean measurement permits a focused follow-through on the
inherited entry-point family before the larger dynamic Object redesign. The
smallest diagnostic rewrite is not to add borrowed methods or clone payloads:
an inherited `self.view()` whose callee captures `this` must instead forward
the current canonical child handle to `Child::view_with_outer`. Its callers
must transitively retain that handle too. The original inner view must observe
later child writes, not a reconstructed base value.

`tests/fixtures/inherited_self_calls.java` reproduced the boundary without any
translator diagnostic: the base correctly had only a handle-aware `view`, but
the flattened child's inherited `through` called nonexistent borrowed `view`.
Current-this selection searched direct declarations and inherited **private**
helpers only. Explicit receiver selection already searched ordinary inherited
members. These now share an inherited selector, preserving exact overload
selection, null argument admissibility and ambiguity rejection. Transitive
handle analysis uses that same selection rather than leaving child callers in
borrowed-only form. This is a cross-function receiver-contract fix within the
existing framework, not a new reference representation.

Producer-to-consumer identities are deliberately distinct: the flattened
concrete child owns the emitted helper and specialized parameter/return slots;
the original declaration owns its exact exception ABI. `SourceCompanionMethod`
carries both owners. Explicit inherited call result handling now consults the
declaration, while signature lookup prefers the exact emitted child name.
No generated Rust or corpus Java is repaired.

Build **87530 / v1** passed (53.36s). Initial runtime **40226** passed (17.05s),
covering an escaping inner view observing later writes, implicit/explicit-this
inherited calls, same-arity overloads and a throwing inherited chain. Companion
controls **56464** passed **13 / 0** (27.58s). Logs:
`.tmp/inherited-self-calls-{build,runtime,controls}-v1.log`.
The subsequent named-owner contract and expanded direct-call/generic probes
are not covered by that result. Combined build **37608 / v2** is live at
`.tmp/inherited-self-calls-build-v2.log`. Both updated independent Java probes
return zero, at `.tmp/inherited-self-calls-java-oracle-v2.log`. The second probe
renames the inherited generic binder (`T` to `U`), passes it through the
inherited method and exercises checked exception propagation. Collect v2 and
run the two behavior probes and relevant existing neighbors before accepting
this batch. No new corpus run or acceptance is claimed; V145 remains frozen.

V2 build **37608** passed (59.44s). Runtime **24170** has **one pass / one
failure** (26.77s): the generic capture field and constructor spell
`GenericCallBaseValue` without `<T>`, also contaminating the field-derived Clone
bound. This is not an inherited-call regression to hide by dropping generics.
The field assembler and constructor lowerer had separate outer-capture type
builders, both short-circuiting to the unparameterized hierarchy name. Both
now reuse `slot_render::rust_source_outer_capture_type`, which builds the
lexically instantiated Java type and uses ordinary storage/representation
rendering. It retains the existing hierarchy carrier instead of adding a
second handle. Duplicate custom capture/storage wrappers are removed.

V2 controls **25093** passed **13 companion + 22 inherited** tests. Neighbors
**86270** passed **14 behavior integrations** (assignment, six key/hash, one
generic borrowing, five generic identity, one interface erasure). Exact logs:
`.tmp/inherited-self-calls-{runtime,controls,neighbors}-v2.log`. The capture
renderer follow-through is newer than these gates; combined build **53164 /
v3** is live at `.tmp/inherited-self-calls-build-v3.log`. Collect that build
before rerunning the focused probes and renderer/assembly controls. No fresh
full-library or corpus result is claimed for the current unfrozen worktree.

V3 **53164** passed (58.87s). Renderer/assembly **98759** passed **35 + 49**
controls. Runtime **49389** again has one pass / one failure (19.13s): all three
missing generic arguments are fixed; the remaining error is the source
hierarchy enum's `derive(Clone)` imposing `T: Clone` despite exclusively
shared-handle payloads. The existing source-struct field-wise clone mechanism
is generalized: `clone_where_clause_for_emitted_types` now serves both source
struct fields and generic hierarchy variant payloads. Generic enums clone
their actual payloads instead of syntactically constraining every binder.
Non-generic enum rendering remains unchanged. The generic probe includes a
compile-time proof that `GenericCallBaseValue<T>` can clone without `T: Clone`.

Combined build **22522 / v4** is live at
`.tmp/inherited-self-calls-build-v4.log`. Collect it, run the two behavior
probes, and if green run the broader library and existing neighbors at this
coherent entry-point/capture/clone boundary. The worktree is not frozen;
V145 is still the latest measured corpus baseline.

V4 build **22522** passed (1m01s), without warnings. Runtime **62569** passes
**2 / 0** (21.19s), including the unconstrained generic carrier clone proof,
direct inherited-call catches and live captured-child identity. Final logs:
`.tmp/inherited-self-calls-{build,runtime}-v4.log`.
Fresh full library **87383** passed **1,593 / 0** (175.70s) at
`.tmp/inherited-self-calls-library-v4.log`; neighbors **86697** passed **14 / 0**
at `.tmp/inherited-self-calls-neighbors-v4.log`. All validation handles are
terminal/collected. Formatting and scoped diff checks pass.
Release **59197** passed (1m39s), log
`.tmp/inherited-self-calls-release-v146.log`. V146 is frozen at
`.tmp/new-java2rust-v146-inherited-self-contracts`, **21,980,368 bytes**, SHA-256
`215dedda97bdfd7b3d4d9ac9c969634bb14b3e245b2940d0f4516d97592720d9`.
Isolated pinned corpus **10199** finished at
`.tmp/jgrapht-core-v146-inherited-self-contracts-run.log`, output
`.tmp/jgrapht-core-1.5.3-focused-v146-inherited-self-contracts`. All 104 sources
translate, with zero generated parse failures; cargo check fails with 1,082
diagnostics. The 39 removed / 34 added normalized entries, added duplicate
guard occurrences and timings are detailed in `docs/jgrapht-core-corpus.md`.
V146 is the latest parser-clean measurement, not acceptance.

Next related audit: current-object calls inside copied inherited bodies must
select overrides by the Java callable slot, not require the resolver's base
declaration owner to equal the concrete child. Both direct and intermediate
overrides need the same child generic frame used by inherited-member pruning.
Preserve private and explicit-super binding, overload identity, canonical
handles, covariant success shapes and exact selected exception ABI. Reuse
`resolved_source_method_decl`, `source_ancestor_type_arguments` and
`JavaCallableSlotKey` rather than adding another name/arity override matcher.
The two new Java probes at `tests/fixtures/{virtual_self_dispatch,
generic_virtual_self_dispatch}.java` cover direct/intermediate overrides,
covariant self returns, live alias observations, throws and renamed/specialized
generic binders. Independent Java oracles and pre-fix Rust baseline **58563**
are running at `.tmp/virtual-self-dispatch-{java-oracle,baseline}.log`.

### Virtual self-return follow-through (unfrozen, in progress)

Baseline **58563** finished with **one pass / one failure** (14.20s). Both
Java oracles return zero. The renamed/specialized generic dispatch probe is a
passing control; the covariant-self probe has 16 compiler errors. Besides the
missing borrowed `self__` calls, it proves a representation requirement gap:
an inherited method returning `this` may return a subclass allocation, even
without an explicit Java cast or assignment from a subclass expression.
Existing hierarchy carriers can express the required views; do not make a
base snapshot or clone the payload to emulate that return.

Implemented together:

- Seed the ordinary subtype-dispatch requirement for source classes with
  subclasses and return-position receiver flow. The existing item-assembly
  `block_returns_this` predicate now delegates to a shared semantic predicate,
  including casts/conditional returns and excluding nested callback returns.
- Resolve current-object overrides by specialized `JavaCallableSlotKey`,
  reusing source-declaration lookup and ancestor substitutions. Direct and
  intermediate overrides share the selector with inherited and explicit-call
  paths; private helpers stay declaration-bound and explicit-super is excluded.
- Preserve the concrete receiver throughout transitive analysis while tracking
  the selected declaration separately for signatures, exception ABI and cycle
  keys. Normalize current-object arguments through the selected emitted ABI.
- Use a target-specific carrier projection when `this` is returned through
  multiple hierarchy views. Preserve optionality and produced-shape evidence.
- Adapt selected covariant companion success values to the Java call's static
  view with the existing slot-conversion plan, separately from Result handling.

Build **83200 / v1** passed (1m05s), log
`.tmp/virtual-self-dispatch-build-v1.log`. A test-only build **55720** passed
(1.04s) to include the additional F-bounded fluent probe; its Java oracle is
zero at `.tmp/fluent-virtual-self-dispatch-java-oracle.log`. Controls **16024**
passed **13 companion + 22 inherited** tests. Runtime **91960** finished
**3 passed / 2 failed** (33.67s), log `.tmp/virtual-self-dispatch-runtime-v1.log`.
The two old live-view probes and the renamed-generic control still pass. The
covariant probe is down to three compiler errors; the fluent probe has one.
No active test/build process remains, and this is not an accepted candidate.

Remaining producer-to-consumer gaps, identified from the raw generated output:

1. `adapt_source_hierarchy_carrier_covariant_return` still only wraps a concrete
   payload when it belongs to one unique carrier. With both BaseValue and
   MiddleValue this fails to adapt the forwarding match arms. Generalize that
   existing adapter using actual solved variant return shape and target carrier
   shape; reuse `adapt_trait_impl_forwarded_return`/slot conversions, including
   Result success mapping, rather than duplicating textual wraps.
2. F-bounded source-class trait forwarding emits
   `impl FluentVirtualBaseBound<B> for Rc<RefCell<FluentVirtualLeaf>>` with a
   leaked owner binder `B` (which the facade layer unfortunately provides as a
   placeholder type). It then declares `step -> B` but forwards an Rc<Leaf>.
   Specialize the **impl header and method/field contracts** using the existing
   ancestor bindings and shape-specialization helpers. Do not use Any casts to
   pretend this implementation is valid for an arbitrary B. Inspect
   `source_class_bound_trait_impl_emissions_for_type`, its signature builder and
   forwarded return adapter. The raw diagnostic artifact is
   `.tmp/fluent-virtual-self-v1.rs`, produced by the latest debug rlib via
   `.tmp/inspect_virtual_self_v1`; no generated code was edited.
3. The completed identity comparison in `lower_generic_reference_identity_*`
   leaves no PlainValue marker, so an enclosing condition adds `?` to a bool
   after the throwing operand has already been discharged. Reuse the existing
   `RustExprShape::PlainValue` effect boundary; keep the operand's exception and
   evaluation order intact. The error is the `checked(false) != leaf` condition.

Batch these related forwarding/value-contract follow-ups before another product
build. Also move the cheap return-position test before the source-subtype scan
in the new semantic seed to avoid scanning the program for every class with no
self-return. This is not a new optimization phase. No fresh broad library,
release or corpus run is warranted until the focused probes pass. V146 remains
the frozen measured baseline; the full objective remains incomplete.

### Forwarding contracts in the instantiated bound frame (in progress)

The next coherent changes reuse existing mechanisms rather than repairing the
emitted probes:

- Hierarchy forwarding now consults the selected variant's solved return slot
  and calls `adapt_trait_impl_forwarded_return` for covariant class returns,
  including cross-carrier rewraps and Result success mapping. The old specialized
  generic-return path is preserved.
- Source-class bound trait impl headers use ancestor bindings, and method
  signatures, effective compatibility checks, argument/return adapters and field
  capability signatures use that same instantiated frame. Owner substitution
  respects method binders. Direct generic-reference arguments borrow their
  carrier, not a copied class payload. The source declaration remains the
  signature/exception lookup key.
- Completed generic/source reference identity comparison uses the existing
  PlainValue marker, preventing a second `?` after operand effects are lowered.
- The new self-return requirement seed tests method return flow before scanning
  for source subclasses, avoiding unnecessary scans for unrelated classes.

The fluent Java probe now additionally stores its self-typed argument in a
self-typed field, reads it back and checks alias identity. The expanded oracle
at `.tmp/fluent-virtual-self-dispatch-java-oracle-v2.log` is zero. Build
**87666 / v2** failed on one mechanical Rust move of non-Copy ViewShape; using
its clone fixes that compiler error. Build **51633 / v3** passed (1m03s).
Runtime **84502** is **4 passed / 1 failed** (31.17s): covariant forwarding,
identity comparison, the two old live views and generic-renaming control now
pass. The remaining five E0053 diagnostics are **one header contract mismatch**,
not five independent bugs: methods use Rc<RefCell<Leaf>> but the trait argument
was rendered as nominal Leaf. Controls **8114** pass **13 + 22 + 2** tests.
Logs `.tmp/virtual-self-dispatch-{build,runtime,controls}-v3.log`.

Follow-through replaces nominal rendering of instantiated contract arguments
and capability predicates with the existing
`rust_type_for_java_generic_arg_with_representations`. Bound forwarders reuse
`lower::emitted_rewritten_source_param_shape` (now crate-visible) to match the
post-substitution, post-representation method ABI. The fluent fixture also
checks a borrowed generic argument via `read(B other)` so this does not merely
fix owned self returns. Build **13287 / v4** passed (1m09s). Runtime **92336**
has four passes / one failure: the remaining compile error forwards &Rc into
an owned Rc parameter. Neighbors **46892** pass all **14** integrations. Logs
`.tmp/virtual-self-dispatch-{build,runtime,neighbors}-v4.log`.

The common `trait_impl_forwarded_arg_with_facts` now delegates borrowed-carrier
to owned argument recovery to the ordinary slot-conversion plan, alongside its
existing owned-to-borrowed route. A focused unit checks actual routing and
non-no-op ownership recovery. Build **25443 / v5** passed (1m01s); all three
argument controls pass. The fluent probe now also mutates the argument through
an alias (`bump(B other)`), oracle zero. Runtime **92894** compiles all five
probes but still has four passes / one failure: a **real RefCell alias panic**,
not an assertion issue. The source `read`/`bump` methods do not use their own
receiver, so equal lowered bodies had been turned into thin delegates holding
`__java_self.borrow_mut()` while the argument accesses the same allocation.
Raw translator output is `.tmp/fluent-virtual-self-v5.rs`; no repairs.

The shared guard policy now permits thin delegation only for a conservative
non-reentrant primitive-parameter body without receiver/reference accesses or
calls (immutable, guard-free sharing is unchanged). Otherwise retain the
already lowered handle-aware body. Direct and inherited companions share this
rule, and actual receiver mutability drives any remaining guard projection.
Build **67679 / v6** passed (1m02s); runtime **30824** passes **5 / 0** (28.95s),
including the live alias mutation. Companion controls **6373** pass **13 / 0**.
Logs `.tmp/virtual-self-dispatch-{build,runtime,companion-controls}-v6.log`.

Guard elision also affects an existing cleanup contract: deferred iterator
removes were flushed only in thin direct delegates. Cleanup is now one scoped
Result boundary shared by inlined/delegated and direct/inherited companions.
An immediate closure discharges temporary payload guards before flushing,
including on an early `?`/throw, then returns the original Result. The existing
error policy is preserved. Names avoid parameter collisions. The maintained
`shared_iterator_cleanup.java` probe forces mutable sharing and checks retained
backing-list state after both a normal return and a throw; independent oracle
`.tmp/shared-iterator-cleanup-java-oracle-v7.log` is zero.

Build **52645 / v7** passed (59.74s), log
`.tmp/virtual-self-dispatch-build-v7.log`. Test-only refresh and six runtime
probes **19851** are live at `.tmp/virtual-self-dispatch-runtime-v7.log`;
neighbors **19419** (the previous 14 plus four source-call staging and two
callback integrations) are live at `.tmp/virtual-self-dispatch-neighbors-v7.log`.
The three fresh argument controls pass. V7 runtime **19851** finished with
**5 passed / 1 failed**: the inline body shadows its &mut iterator parameter,
moving that reference into the cleanup closure and preventing the later flush.
The scope now takes typed iterator parameters and is called with explicit
reborrows, preserving the outer parameter for cleanup. Build **60974 / v8**
passed (1m07s); exact iterator gate **59698** passed (15.62s), log
`.tmp/virtual-self-dispatch-iterator-v8.log`.

V7 neighbors **19419** passed the original 14 and three of four staging tests,
but stopped at the retained-formatting staging failure (callbacks not run).
The failing generated `owner.calls++` captured a mutable child place through
the temporary borrow used to read the current object's owner field (E0716).
`stage_shared_field_place_receiver` now retains the proven canonical Rc/Arc or
all-handle carrier receiver independently, registers its physical slot in a
scoped environment, then reuses the normal write-place builder and primitive
update planner. Stable local handles and direct this/super places are unchanged;
arbitrary by-value payloads are not cloned. Receiver evaluation remains once,
and the completed staged update carries PlainValue evidence.

Build **55354 / v9** passed (1m03s), log
`.tmp/virtual-self-dispatch-build-v9.log`. Current live gates:

- **93651** finished **6 / 0** (33.89s), `.tmp/virtual-self-dispatch-runtime-v9.log`.
- **9288**, twenty neighbors, `.tmp/virtual-self-dispatch-neighbors-v9.log`
  (staging/callbacks first, then assignment/hash/borrowing/identity/erasure).
- **62864**, fresh full library, `.tmp/virtual-self-dispatch-library-v9.log`.

All twenty neighbors finished successfully; full library **62864** finished
**1,594 / 0** (185.86s). Release **13934** finished successfully (2m11s),
log `.tmp/virtual-self-dispatch-release-v147.log` (handle now absent).
V147 is frozen as `.tmp/new-java2rust-v147-virtual-self-contracts`,
**22,055,176 bytes**, SHA-256
`8201112fd404389d52c6062696b7aa1543d806b771dffaff2f4f4b967e1641d3`.
Isolated corpus **36753** finished (exit 1 for generated-Rust errors) at
`.tmp/jgrapht-core-v147-virtual-self-contracts-run.log`, output
`.tmp/jgrapht-core-1.5.3-focused-v147-virtual-self-contracts`.
No competing builds/tests ran during measurement. V147 records 104 translated,
zero generated parse failures, and **1,229 = 1,164 typed + 65 untyped** errors;
guards 23, registry markers 63/25 files. Delta from V146: **74 removed / 221
added** (`.tmp/jgrapht-v146-v147-diagnostic-delta.txt`). 167 additions repeat an
existing header/file; all seven added marker occurrences repeat existing
failures in emitted companion bodies. The other 54 additions require audit;
counts and repeated headers alone do not establish correctness.

Targeted self-override failures and the 24 DoublyLinkedList enum downcast/is
errors disappear. Next contract audit includes nullable covariant returns
(not covered by the non-null probes), concrete super-implementation entry
points in handle-aware companions, and the newly exposed parameter/return
ABI mismatches. Existing primitive/ref alias safety must not be weakened.
The nullable root/intermediate/leaf fixture's runtime gate **71607** fails
with E0308: leaf `return this` omitted the optional carrier. Independent Java
oracle **51500** returns zero. This is fixed through ordinary element-to-slot
conversion of the canonical handle. Covariant forwarding now checks subtype
reachability on payload Java types rather than optional wrappers. The extended
fixture also exercises a non-null override behind a nullable base contract;
updated Java oracle `.tmp/nullable-virtual-self-java-oracle-v2.log` is zero.
Combined debug build **76317** passes (1m14s), log
`.tmp/nullable-virtual-self-build-v2.log`. Fresh runtime **12178** passes
**7 / 0** (25.19s), `.tmp/nullable-virtual-self-runtime-v2.log`, and focused
callable/conversion units pass **32 / 0**, `.tmp/nullable-virtual-self-unit-v2.log`.
Formatting passes. This follow-up is newer than V147, not corpus-measured.
V147 total 398.093s; emission 185.376s versus V146 131.410s, global analysis
roughly unchanged. Name/arity rejection before virtual-slot specialization
is now implemented as a prefilter only, retaining complete specialized slot
comparison for selection. A pairwise header-policy unit covers generic,
overloaded, snake-case-alias, static and varargs methods. Speedup is unmeasured.

Next architectural audit: `AsUndirectedGraph` calls missing
`__java_super_graph_delegator_{get_all_edges,get_edge,degree_of}` helpers in
both ordinary and handle-aware bodies. `lower_overridden_super_body_helpers`
and `source_matching_override_for_super_method` still compare raw erased
parameter spellings, unlike the instantiated virtual-slot mechanism. Verify
superclass lookup and helper emission first, then minimize renamed generic
binders, cross-method super calls, inherited overrides and aliased arguments.
The required rewrite is a lexical (non-virtual) superclass implementation on
the existing concrete receiver allocation, with that implementation's exact
parameter/Result ABI; it must not copy a superclass payload or hold a guard
across potentially reentrant argument/callback access. Reuse the solved slot,
source ancestor bindings and shared companion machinery. This is a proposed
next batch, not yet an established diagnosis for every missing-helper error.

## Lexical-super implementation contracts (in progress)

Two new maintained probes separate multi-file renamed generic binders from
shared receiver/argument aliasing (`tests/lexical_super_calls.rs`). Initial
gate **29847** fails both: generic calls reconstruct a `SuperBase` payload
(missing its private phantom field); a hierarchy-typed local's inherited field
read is mistaken for an Rc payload. The generic probe also exposes inherited
carrier forwarding in the wrong binder frame. Both Java runs return zero.
Artifacts: `.tmp/lexical-super-runtime-v1.log`, retained batch output
`.tmp/test-tmp/new_java2rust_batch_generic_lexical_super_1792511632636454637`.

The helper entry lookup did not erase a parameterized extends spelling, while
helper discovery paired only same-named overrides with raw erased parameter
types. Discovery now inventories actual resolved lexical super calls through
ancestors and transitively through selected helper bodies. It reuses ordinary
inherited specialization and companion emission. Calls on a canonical handle
use the helper's static handle-aware entry, not a guarded payload call; ordinary
source calls prefer the helper before the payload-copyback fallback. Summary
and opaque external boundaries retain their existing policy.

Usage analysis previously overwrote a resolved lexical owner with the raw
direct-parent extends spelling (`SuperBase<B>`), disconnecting mutability
propagation. Preserve resolved owners, using the syntactic parent only for an
actual `super` placeholder. Resolved super calls also now retain their direct
call-graph edge instead of becoming unknown external calls. The generic probe
includes mutation followed by a throw to check this effect/ABI path.

Inherited carrier forwarders now specialize the member into the carrier
owner's generic frame. Hierarchy field-access proof now includes inherited
fields; this preserves the carrier accessor rather than projecting an Rc
borrow from an enum-valued local. The original alias-field assertions remain.

Combined build **5218 / v4** passes (1m13s),
`.tmp/lexical-super-build-v4.log`. Live gates: new probes **76103**,
`.tmp/lexical-super-runtime-v4.log`; 95 existing `super_` controls **77559**,
`.tmp/lexical-super-controls-v4.log`; seven self-call neighbors **60151**,
`.tmp/lexical-super-neighbors-v4.log`. Collect these exact handles before another
build or broad measurement. V147 remains the latest measured corpus baseline;
no corpus run is live. Fixed-type generic payload returns and covariant helper
ABIs still require evidence; no blanket super/inheritance completion claim.

V4 gates are terminal: new probes **76103** fail both (one moved Rc after
cloning an enum wrapper, plus two fixed-type generic carrier return errors);
seven neighbors **60151** pass **7 / 0**. Super controls **77559** finish
**41 passed / 54 failed** (106.04s). Many require the old copyback text, but
unclassified failures must not be called pre-existing or harmless.

V5 fixes source-reference reuse before a closed-carrier conversion, shared by
argument and assignment consumers. It requires a variable, owned source and
destination, proven reference Clone and a dispatch change, then reuses the
normal slot planner. This preserves the original handle before an enum
constructor can consume it. Payload methods are specialized into the concrete
variant frame before covariant/generic return forwarding. Generic class-carrier
returns reify the same existing construction invariant as scalar generic
returns, only when substituting the carrier owner's ancestor bindings exactly
produces the actual return type. Canonical handles move unchanged; no object
payload copy or permissive reinterpretation is introduced.

Build **24368 / v5** passes (1m21s), `.tmp/lexical-super-build-v5.log`.
New probes **97283** pass **2 / 0** (34.04s),
`.tmp/lexical-super-runtime-v5.log`; neighbors **14361** pass **7 / 0**
(22.72s), `.tmp/lexical-super-neighbors-v5.log`. Independent Java oracle log
`.tmp/lexical-super-java-oracles-v4.log` contains two zeroes (current fixtures).
Twenty-seven copyback-specific assertion blocks now require the corresponding
lexical helper and prohibit temporary-parent copyback; their exact existing
runtime effect/return checks remain unchanged. The hidden-field assertion also
retains its distinct storage-slot checks. Non-mutating helper checks were
updated consistently. Initial test-only compile **75782** caught a private
naming API reference; literal expected helper identities replace it.
Test-only refresh **92818** passes (11.12s),
`.tmp/lexical-super-controls-build-v5b.log`.

Live: **24095** focused mutating controls,
`.tmp/lexical-super-mutating-controls-v5.log`; **84020** full library,
`.tmp/lexical-super-library-v5.log`; **18577** frozen/current output comparisons
for three unmodified single-source control cases,
`.tmp/lexical-super-baseline-comparison-v5.log`. Collect exact handles.
No release or corpus run is live. General covariant superclass helper ABI
differences remain an explicit unproven edge, not a completed subsystem.

V5 full library **84020** passes **1,595 / 0** (166.52s). Mutating controls
**24095** finish **24 passed / 3 failed** (73.30s), all three at Rust consumers
passing slices instead of the existing retained-array reference ABI. Their
test inputs now use canonical Rc/RefCell arrays, preserve every state assertion
and unwrap fallible calls. Test-only build **17530** passes (9.81s), log
`.tmp/lexical-super-controls-build-v5c.log`.

Follow-through **11474** finishes **5 passed / 1 failed** (21.55s),
`.tmp/lexical-super-controls-followthrough-v5.log`. The three array consumers
and two non-mutating controls pass. The hidden-field test reveals a **real
runtime regression**: `"0,11"` instead of `"1,10"`. The source superclass
helper's `this.n` incorrectly selects the child's hidden field, while its bare
field accessor already selects the correct inherited storage slot. Do not
weaken that expected result. Explicit current-instance field reads, places,
mutable arguments and retained-array guards now prefer the existing method
environment's lexical field-access binding. Inherited declaring-field lookup
also uses primary/emitted-source precedence before cross-context fallback.

Frozen/current comparison **18577** finishes with byte-identical output for
`explicit_object_super_constructor_is_noop`,
`flattened_super_constructor_substitutes_arguments_inside_control_flow`, and
`object_super_clone_clones_the_dynamic_source_receiver`. These specific
failures predate this batch; other unclassified failures from the 95-test
sweep remain unclassified. Log `.tmp/lexical-super-baseline-comparison-v5.log`,
artifacts `.tmp/lexical-super-baseline-probes/`.

Release **59061** finished successfully (2m29s),
`.tmp/lexical-super-release-v148.log`, but **is not frozen/accepted** because
of the hidden-field regression. The worktree is now newer. Combined V6 build
**41215** is live, `.tmp/lexical-super-build-v6.log`. Collect it and run the
hidden-field, lexical/self-call and relevant field-binding controls before
another release/corpus measurement. No release or corpus process is live.

V6 build **41215** passes (1m09s), `.tmp/lexical-super-build-v6.log`.
Controls **58984** finish **29 / 1** (36.12s),
`.tmp/lexical-super-controls-v6.log`: the hidden-field oracle now passes.
The failure is a real constructor regression in `SuperMethodCall_Derived::new`,
which emits `Some(self.label)` before a receiver exists. The environment-first
rule was applied to constructor initializer bindings as though they were
lexical method storage slots. V7 restricts that precedence to inherited method
bodies (`current_method_decl` and lexical owner present, no constructor).
Constructor expression bindings retain their previous lowering authority.
The stronger primary/emitted-source precedence remains for inherited field
declaration lookup.

V6 runtime **91807** passes **2 / 0** plus **7 / 0**,
`.tmp/lexical-super-runtime-v6.log`; field/inherited units **97872** pass
**28 / 0**, `.tmp/lexical-super-field-units-v6.log`. Formatting **27292** and
diff whitespace checks pass. V7 combined build **51809** is live at
`.tmp/lexical-super-build-v7.log`. Collect it, then check both the failing
constructor and hidden-field tests together, plus relevant lexical/self-call
controls. V5's 1,595 library passes are prior evidence, not a fresh V7 result.
No release/corpus process is live; the successful V5 release remains unfrozen.

V7 build **51809** passes (1m10s). Controls **17301** finish **2 / 1**:
hidden fields and inherited lexical scope pass, but the same constructor still
emits `Some(self.label)`, so restricting environment precedence alone was not
sufficient. Artifacts `.tmp/lexical-super-field-constructor-controls-v7.log`.
Runtime **30144** passes **2 / 0** plus **7 / 0**,
`.tmp/lexical-super-runtime-v7.log`; field units **28277** pass **28 / 0**,
`.tmp/lexical-super-field-units-v7.log`. Formatting **18623** passes.

V8 restores the pre-batch declaring-field lookup policy (removing V6's
unnecessary primary-source preference), while retaining the scoped lexical
environment binding for inherited method reads/writes. This isolates the
other change from the constructor regression; it is not yet proven fixed.
Combined build **70538** is live, `.tmp/lexical-super-build-v8.log`. Collect
that exact handle, then rerun the constructor and hidden-field controls
together. If construction still fails, inspect the actual lowering role and
initializer bindings before more edits; do not keep adjusting lookup order.
The 104-file objective remains incomplete and no release/corpus is live.

V8 build **70538** passes (1m11s). Crucial joint gate **93257** passes
**3 / 0** (17.56s), `.tmp/lexical-super-field-constructor-controls-v8.log`:
hidden superclass storage, constructor parameter initialization and inherited
lexical scope now pass together. The scoped environment binding supplies the
method fix; the prior cross-context declaring-field lookup policy is retained.
This avoids changing constructor/initializer lookup as a side effect.

Final V8 gates are live: fresh library **96304**,
`.tmp/lexical-super-library-v8.log`; two lexical plus seven self-call probes
**66931**, `.tmp/lexical-super-runtime-v8.log`; release **39901**,
`.tmp/lexical-super-release-v148b.log`; formatting/diff checks **23459**.
Collect exact handles. Release overlaps verification, not corpus timing.
Do not rerun completed older gates or reuse the earlier V5 release by mistake.
After all gates finish successfully, freeze the new release as
`.tmp/new-java2rust-v148-lexical-super-contracts` (record bytes/SHA), then run
the untouched 104-file slice in isolation at
`.tmp/jgrapht-core-1.5.3-focused-v148-lexical-super-contracts`, log
`.tmp/jgrapht-core-v148-lexical-super-contracts-run.log`. Only the separate
resolver benchmark may be skipped using applicable prior evidence. Compare
V147/V148 diagnostic additions/removals, markers and phase timings before
choosing another substantial batch. No corpus process is live yet.

Final V8 library **96304** finishes **1,595 / 0** (129.94s). Runtime **66931**
finishes **2 / 0** (83.32s) plus **7 / 0** (34.11s). Formatting/diff **23459**
finished successfully. These durations overlap builds/other verification and
are not performance measurements. Release **39901** remains live; frozen
V148 binary and output root are confirmed absent. Collect that exact release
handle, then freeze and measure as above without competing builds/tests.

Release **39901** finishes successfully (3m11s). V148 is frozen as
`.tmp/new-java2rust-v148-lexical-super-contracts`, **22,026,392 bytes**, SHA-256
`ab832c20b57471c269db35445f551654ec346997172300a5644d940d31186ee8`.
Isolated corpus **3177** is live, output
`.tmp/jgrapht-core-1.5.3-focused-v148-lexical-super-contracts`, log
`.tmp/jgrapht-core-v148-lexical-super-contracts-run.log`. All verification and
build processes are terminal; no competing workload runs during measurement.
Collect this exact handle and compare against V147 before further substantial
edits. No acceptance or current V148 diagnostic count is claimed yet.

V148 **3177** is terminal (exit 1): 104 translated, zero generated parse
failures, valid type-error counts, **1,169 = 1,102 typed + 67 untyped** errors.
Guards 23; registry markers 63/24 files. Delta from V147: **79 removed / 19
added**, `.tmp/jgrapht-v147-v148-diagnostic-delta.txt`. Lexical-super missing
methods disappear in graph/specifics/list views. Twelve enum `.lock()` errors
also disappear. Three added closure identities are line-number moves. Other
additions still need classification. Total 450.213s (translation 313.469,
assembly 102.156, check 26.670, sweep 7.151), no competing builds/tests.
Global analysis 55.250 → 79.917s and emission 185.376 → 211.879s; this is not
a stable performance comparison.

The two added boxed NodeIterator → ListNodeIterator coercion markers are a
real implementation-contract issue, not just an unrelated old gap. The newly
emitted `__java_super_*_get_wrapped` helper inherits the child override's
covariant return signature even though it lowers the superclass body, whose
stored `orig` and declared return type are NodeIterator. Required rewrite:
the helper returns the **declaring superclass implementation's solved ABI**,
specialized into the concrete receiver's generic frame; the child's explicit
cast stays at the caller. Receiver allocation/lexical field binding and the
declaration's Result policy remain separate authorities.

Next priority is to generalize signature selection for lexical implementation
entries and reuse that same selected contract for body environment, emitted
function shell and super-call argument/result adaptation. Do not repair the
marker with an implicit downcast in the helper or copy the child virtual ABI.
Formatting is a subsequent candidate: implicit retained collection display
still uses `java_collection_to_string(AsRef<[T]>)`, unlike the existing live,
callback-safe explicit `.toString()` renderer. Reuse its callback/cursor path
and checked effect analysis when that batch is selected; do not borrow a guard
across element formatting or change null concatenation into a null dereference.

Maintained minimal reproduction added:
`tests/fixtures/covariant_super_contract.java`, test
`covariant_super_helpers_keep_the_declaring_implementation_return_contract`
in `tests/lexical_super_calls.rs`. It checks both ordinary and handle-aware
parent-helper headers before generated-runtime verification; the Java body
also checks successful alias recovery and ClassCastException for a wrong
payload. Gate **2751** fails as intended with concrete evidence:
`__java_super_covariant_parent_get(&self)` incorrectly returns
`Result<Option<Rc<CovariantLeaf>>, JavaException>` rather than the parent's
CovariantItemValue carrier. Log `.tmp/covariant-super-contract-runtime-v1.log`.
Independent `.tmp/covariant-super-contract-java-oracle.log` is **0**.
This new regression is currently failing; no production follow-through has
been applied after frozen V148.

Implementation routing audit:

- `lower_method_with_exception_owner` always obtains the current emitted
  owner's virtual method signature. Add an explicit selected source
  implementation contract for helper body lowering; specialize the declaring
  method's solved parameter/return/local plans with existing ancestor bindings
  and binder-aware shape projection, retaining receiver/effect identities.
- `item_assembly::method_function_shell_with_facts_exception_plan_and_support_contexts`
  re-queries the global plan for parameters, returns, receiver and borrowed
  lifetimes. It must consume the **same selected contract** as the body, with
  the normal entry remaining a wrapper for ordinary virtual emission. Keep
  the global plan for other call/callback queries; do not clone an entire
  SignaturePlan per helper or rewrite a finished Rust return type.
- Generalize/reuse `source_bound_contract_shape_in_frame` and
  `shape_projection::specialize_slot_shape_java_type` rather than inventing
  another generic/physical-slot substitution algorithm. Preserve local
  binding IDs, nested collection views, ownership and nullability.
- `call_plan::plan_call_expr_with_external_index` tries trivial getter inlining
  before super-boundary selection. Existing child `getWrapped` bodies thus
  also contain a standalone-Super error. Prevent source getter elision from
  turning a lexical super call into an ordinary receiver access; keep casts at
  the caller, and use the selected implementation's argument/success-value ABI.

No build/test/release/corpus processes remain live. Current worktree production
source still corresponds to frozen V148; only the new maintained regression
and documentation are newer. Next step is this coherent contract batch, not
another corpus rerun or the deferred formatting batch.

## Declaring implementation ABI follow-through (in progress)

`MethodEmissionContract` now carries one explicitly selected signature and
its source declaration identity through body lowering and function assembly.
Lexical helpers instantiate the **declaring** method's solved params, return
and locals in the concrete receiver frame. Body environments, shell receiver,
parameters, returns, borrowed lifetimes and callback metadata consume that
contract. Other calls still query the global plan; no full SignaturePlan is
cloned per helper. Super-call argument planning and produced success-shape
metadata reuse the same implementation-signature projection.

The existing bound-contract shape projector is generalized into
`shape_projection::specialize_source_contract_shape`; its old bound wrapper
delegates to it. Method-plan specialization shares that policy, preserves
local BindingIds, mutability/proven-nullability evidence and nested views,
and excludes method-shadowed class binders. Two focused units cover these
invariants. Trivial accessor planning rejects `super` rather than emitting
an ordinary receiver access and moving the cast into the wrong place.

Cheap check **16914** passed (21.55s). Combined V2 build **7564** passes
(1m03s), `.tmp/covariant-super-contract-build-v2.log`. Shape units pass
**15 / 0**, `.tmp/covariant-super-contract-units-v2.log`, and thirty super
controls **51660** pass **30 / 0** (33.99s). New runtime **44807** is **2 / 1**:
the covariance helper headers and caller-side ClassCastException path are
now correct, but two immutable Rc carrier forwarding arms still call the
deliberately omitted borrowed `identity` method. Raw fixture output is retained
under `.tmp/test-tmp/new_java2rust_covariant_super_contract_1792515169481515451`.

V3 aligns hierarchy forwarding with companion eligibility for **all implemented
shared** payloads, not just interior-mutable ones. Immutable Rc/Arc also needs
its canonical handle when Java returns/captures `this`. Build **79323** passes
(1m24s), `.tmp/covariant-super-contract-build-v3.log`. Exact covariance runtime
**34140** passes (20.48s), `.tmp/covariant-super-contract-runtime-v3.log`.

A follow-through audit finds callback bodies still looking up the generated
helper name instead of the declaration used by the shell. LowerCtx now carries
the same method contract for callback-key selection, explicitly cleared in
constructor/interface-body entry scopes. The multi-file generic-super fixture
now exercises successful callbacks on direct/inherited receivers and a callback
division-by-zero caught as ArithmeticException. Updated independent Java
oracle `.tmp/covariant-super-callback-java-oracle.log` is zero (handle **41185**
terminal). V4 build **42266** caught missing Debug/test-context initialization
for the new context field; those are fixed together. V4b combined build
**84210** is live, `.tmp/covariant-super-contract-build-v4b.log`. Collect it,
run the three lexical probes plus self-call neighbors, then broad library and
relevant controls at this batch boundary. No release or corpus is live.

V4b build **84210** succeeds (1m31s). Collected gates: covariance and aliased
lexical-super runtime probes pass, self-call neighbors **88051** pass **7 / 0**,
and super controls **85049** pass **30 / 0**. Library **73845** is **1596 / 1**;
its lone failure asserts the old borrowed `toString` forwarding form rather
than the now-canonical inherent shared companion. That assertion is updated
without changing its source-method qualification requirement.

The expanded callback probe in **34674** fails with five real type errors:
carrier forwarding shells use stored `IntUnaryOperator`, while concrete
methods and call sites use `&mut dyn FnMut(i32) -> Result<i32, JavaException>`.
The lexical helper body and shell themselves agree. This is a missing
hierarchy callback-boundary contract, not a reason to repair emitted Rust.
`item_assembly::source_method_callback_param_type` now shares the existing
ordinary-shell callback renderer with carrier/anonymous-dispatch shells;
the authoritative CallableAnalysis is passed through so callback effects are
not guessed from the enclosing method's Result ABI. Callback storage and
retaining policies still come from SourceCallbackPlan.

Cheap V5 check **51361** catches five test callers needing the explicit optional
analysis argument; those are updated together. Formatted/diff-checked combined
V5 build **57140** is live, `.tmp/covariant-super-contract-build-v5.log`.
Collect this exact handle, then verify the callback probe, broad library and
relevant neighbors before freezing V149. V148 remains the measured failing
104-file baseline; no release/corpus is live.

V5 combined build **57140** succeeds (1m13s); formatting and diff checks pass.
Fresh lexical runtime gate **30895** writes
`.tmp/covariant-super-contract-runtime-v5.log`; fresh full library **96933**
writes `.tmp/covariant-super-contract-library-v5.log`. Reuse V4b's passing
seven self-call neighbors and thirty super controls for this callback-only
follow-up rather than repeating unchanged paths. Collect the two fresh gates,
then build/freeze `.tmp/new-java2rust-v149-declaring-implementation-contracts`
and measure at `.tmp/jgrapht-core-1.5.3-focused-v149-declaring-implementation-contracts`.

V5 lexical runtime **30895** is terminal and green **3 / 0** (83.34s), including
the expanded direct/inherited callback success and ArithmeticException cases.
Library **96933** remains live; release build **78437** is also live at
`.tmp/covariant-super-contract-release-v149.log`. These verification/build
durations overlap and are not performance measurements. Collect both handles;
only after success freeze the candidate and run the isolated corpus. No V149
binary/output has yet been frozen or measured.

Library **96933** is now terminal and green **1597 / 0** (127.62s).
Only release **78437** remains live. Collect it rather than rebuilding; after
freezing V149, use the existing corpus runner with the recorded release profile
and a fresh output root, then compare diagnostic identities against V148.

Release **78437** is terminal zero (3m33s). Frozen V149 producer:
`.tmp/new-java2rust-v149-declaring-implementation-contracts`, **22,007,376 bytes**,
SHA-256 `4ad84f2aec44d8e4c6be2f56286adb23b6ef11f58dba831fa291a96c13964dc2`.
Corpus **56736** is live, output
`.tmp/jgrapht-core-1.5.3-focused-v149-declaring-implementation-contracts`, log
`.tmp/jgrapht-core-v149-declaring-implementation-contracts-run.log`.
It uses the recorded release profile, ten workers, unchanged pinned-source
validation and the full 104-file closure; only the separate resolver benchmark
is skipped using its applicable prior evidence. No competing builds/tests.
Collect this exact handle rather than restarting. Compare against V148 using
`.tmp/compare-jgrapht-diagnostics.py`, including the two iterator coercion
markers, diagnostic additions/removals and phase timings. Do not infer batch
acceptance from minimized tests alone or start the deferred formatting batch
before collecting this measurement.

## V149 measured: constructor/initializer inventory follow-through

Corpus **56736** is terminal exit 1 (generated Rust), **104 translated**,
**zero translator/generated parse failures**, `cargo_type_errors_valid=1`.
Cargo diagnostics **1165 = 1103 typed + 62 untyped**; allocation guards **23**;
registry invalid markers **60** in **24** files. Producer provenance matches
the frozen V149 SHA/size/release profile above. Delta
`.tmp/jgrapht-v148-v149-diagnostic-delta.txt` is **5 removed / 1 added**:
two NodeIterator-to-ListNodeIterator coercion markers and three standalone-super
errors disappear. The one new typed failure is a missing lexical `setList`
helper in `DoublyLinkedList.ReversedListNode`'s constructor.

Untouched Java constructor executes `super.setList(list)`; generated constructor
correctly calls that implementation entry but the helper inventory visits only
method bodies. This is an inventory scope gap exposed by vetoing trivial-super
accessor elision, not a need for a different return/representation ABI. Required
rewrite: inventory lexical super calls in emitted constructors and instance
field initializers with the same ancestor resolution, transitive helper-body
walk and declaration-contract projection already used for methods. Deduplicate
helpers once across all entry contexts; do not virtual-dispatch to the override.

That follow-through is now implemented. Work items reference blocks or field
initializer expressions; helper insertion dedup bounds transitive work.
Maintained `constructor_lexical_super.java` checks generic constructor writes,
instance initializer dispatch, a throwing override, successful stored reads
and null preservation. Independent javac/java oracle is **0**. Focused combined
build **77370** is live at `.tmp/constructor-super-contract-build-v1.log`.
Collect it and run the four lexical runtime probes, reusing applicable prior
library/neighbors/controls evidence. Do not run another full corpus merely for
this small follow-through; choose the next coherent batch after its regression
is sound. Source is newer than frozen V149; no release/corpus is live.

Isolated V149 total **501.756s**: translation **366.014s**, assembly **99.566s**,
check **28.594s**, sweep **6.796s**. Global analysis **86.672s**, emission
**254.262s** (V148: 79.917/211.879). No own competing build/tests ran during
measurement; these single observations do not establish a stable performance
regression or justify restarting open-ended optimization. The 104-file slice
still fails acceptance; graph behavior oracles and expansion remain open.

Constructor inventory V1 build **77370** succeeds (1m09s). Runtime **21890**
finishes **3 / 1**: all previous lexical probes pass; the new constructor probe
proves both helpers exist but exposes a separate shared ABI normalizer bug.
The selected parent plan carries `Option<U>`, but inherited method rewriting
overwrites its nominal type with bare effective source `U`. The helper shell
therefore accepts `U`, while the constructor correctly passes `Option<U>`;
Rust infers a nested `Child<Option<U>>`. Raw crate:
`.tmp/test-tmp/new_java2rust_constructor_lexical_super_1792517155650265024`;
log `.tmp/constructor-super-contract-runtime-v1.log`.

`emitted_rewritten_source_param_shape` now uses the existing optionality-
preserving slot-specialization projector instead of replacing the solved type
wholesale. Representation normalization consumes that preserved type. A focused
unit covers nonnull/nullable owner-binder renaming, ownership and source-plan
immutability; the runtime keeps its null case (not weakened). Obsolete Rust-name
allocation in the now-body-based helper inventory is removed in the same batch.
Combined library/lexical build **8796** is live at
`.tmp/constructor-super-contract-build-v2.log`. Collect it, run the four lexical
probes and fresh library gate (shared normalizer is broader than inventory),
then choose the next coherent batch without a redundant corpus rerun.

V2 build **8796** is terminal zero (1m20s); formatting/diff checks pass.
Fresh four-probe lexical runtime **90233** is live at
`.tmp/constructor-super-contract-runtime-v2.log`; fresh **1598**-test library
**23097** is live at `.tmp/constructor-super-contract-library-v2.log`.
Collect these exact gates before further source changes. No release/corpus
process is live. V149 is still the latest measured incomplete corpus baseline.

## Constructor follow-through verified; next batch: retained display contracts

Collected fresh gates: **90233** passes **4 / 0** (84.59s); **23097** passes
**1598 / 0** (128.38s), including the new optionality-preserving binder-rewrite
unit. The constructor/initializer inventory and nullable ABI follow-through are
verified. Do not spend another release/corpus cycle on them alone; carry them
into the next coherent batch. No build/test/release/corpus process remains live.

Selected next batch is retained native collection **display**, not isolated
AsRef implementations. V149 still has 28 dominant retained generic collection
AsRef diagnostics plus other source-element display obligations. Representative
output passes Rc/RefCell storage directly to `java_collection_to_string`, and
generic wrappers acquire a physical `JavaArraysDeepToStringElement` bound that
does not express Java reference dispatch. Explicit native `.toString()` already
has a live fail-fast cursor and a checked per-element callback; implicit concat
still uses the older slice/deep-display path. A collection borrow/owner-field
guard cannot remain live across translated element code, even if a trait impl
would make the emitted Rust compile.

Maintained `retained_collection_formatting.java` is expanded (old explicit
display checks retained) with field concatenation, generic element display,
nullable collection spelling, display-before-suffix evaluation, callback
exception propagation and mutation during display. Independent javac/java
oracle is **0**. Current untouched generated baseline
`.tmp/retained-display-concat-baseline.rs` (CLI **90957**, terminal zero) fails
rustc metadata compilation **62583** with **8 E0277 errors**; log
`.tmp/retained-display-concat-baseline-check.log`. It emits infallible String
signatures for concat methods, direct guard-backed field display, and the
invalid slice/deep-element obligations. No generated repair was applied.
The expanded maintained runtime is intentionally not yet passing and was not
part of the preceding four-probe/library verification.

Whole-path implementation plan (one coherent batch, not one compile per edit):

- Share a shape/source-precedence-aware display boundary between explicit
  native ToString and implicit concat/display conversions. Reuse EffectiveFacts,
  child-shape projection and the existing checked callback renderer rather than
  adding storage-specific AsRef/Display shims. Concat null is `"null"`; explicit
  invocation on null still throws. Arrays must retain their separate Java
  display semantics.
- Stage a cloned Java reference before invoking any formatter callback, so
  outer owner guards also end before callbacks. Recurse through nested native
  child collections using their solved physical carriers. Preserve left-to-right
  operand evaluation and stop before suffix evaluation if display throws.
- Reuse the existing Java erased/source Object string-result kernels for
  generic/null element display. `java_generic_optional_to_string` is infallible
  today and cannot be used to hide source-toString exceptions inside a checked
  callback. Preserve a null toString result's Java spelling.
- Extend the existing retained-native intrinsic-effect classifier to implicit
  display sites with the same route proof and catch/learning policy, not
  unchecked `?` insertion or an enclosing-method Result guess. Its current fast
  prefilter and expression walker only see explicit get/toString/mutator calls.
  Audit callback/lambda and generated-support detection too.
- The current local cursor formatter accepts only Rc/RefCell native collections;
  audit sync handles and map/collection views before selecting the physical
  bridge. Reuse storage/cursor operations and release guards between reads and
  callbacks; do not silently snapshot live views or retain mutexes across Java.
  Map formatting needs its own braces/key-value presentation, not list spelling.
  Add relevant nested/view/sync/map controls at this batch boundary rather than
  fixing just the first local-list fixture.

Classification: shared conversion/effect/physical-operation contract change;
existing callback/cursor and representation machinery can express the required
rewrites. No evidence requires replacing the overall translator framework.
Next action is implement this batch, then combine focused regression/library
builds and only then a fresh full 104-file measurement including constructor
follow-through. V149 remains the measured, failing baseline.

## Retained display V1 implementation and fresh evidence (in progress)

Implemented a coherent first pass across `stdlib`, `lower`, `conversion_plan`,
`exception_plan` and `item_assembly`:

- `JavaSharedAllocationAccess` abstracts short-lived read/write operations for
  Rc/RefCell and Arc/Mutex. The existing live collection iterator now uses this
  abstraction; collection and map display use checked callbacks with guards
  released before Java execution. Map display uses map modification counts and
  key/value presentation, not list formatting. No snapshot repair was added.
- `lower_native_collection_display` stages the physical source and uses solved
  child shapes for callbacks. Explicit native ToString and display slots route
  through it; nullable concat spells null while explicit null invocation throws.
  Non-retained iteration has checked callback helpers instead of slice/deep
  display. The retained operation renderer also allows the sync callback path.
- `native_collection_display_kind` centralizes source precedence and native-kind
  proof. `implicit_java_display_operands` shares concat planning/platform owner
  checks with intrinsic effects. Retained-native effect seeding now visits
  implicit display and non-retained ToString sites, using the existing catch
  and statement-learning machinery.
- A checked generic-optional string kernel reuses the source/erased Object
  result protocol. ArrayDeepDisplay is split from CollectionDisplay so actual
  Arrays.deepToString retains its old capability rather than being weakened by
  the collection protocol change. **Current CollectionDisplay adds Clone+Any;
  the unnecessary Clone for borrowed-only display is a known V1 regression,
  not the intended final contract.**

Maintained formatting oracle is expanded further with nested collections,
unmodifiable and map-value views, retained maps, a static/sync collection case
and a source element that reenters an owner with no collection alias getter.
Independent javac/java is zero. All original explicit formatting assertions
remain. Cheap checks **87377** (lib, 22.43s), **99826** (tests, 28.61s) pass;
formatting/diff pass. Combined build **18190** passes (57.40s), log
`.tmp/retained-display-contract-build-v1.log`.

Collected V1 gates (all terminal, no process remains live):

- Library **41841**, `.tmp/retained-display-contract-library-v1.log`: **1598 / 0**
  (92.66s). This is not runtime acceptance of the display batch.
- Expanded formatting **20490**, `.tmp/retained-display-contract-runtime-v1.log`:
  **0 / 1** (39.29s), nine compiler errors. Raw crate
  `.tmp/test-tmp/new_java2rust_retained_collection_formatting_1792518438194403053`.
- Cached-source neighbor build **66740** passes (2.07s). Generic display
  **53832**, `.tmp/retained-display-generic-neighbors-v1.log`: **0 / 1** (34.20s),
  eleven errors. Nullable display **73065**,
  `.tmp/retained-display-nullable-neighbors-v1.log`: **1 / 1** (36.04s), twelve
  errors. The source-collection-lookalike control passes.

Root-cause follow-through to batch before another expensive build:

1. **Generic callback dispatch**: direct generic children can be nonoptional T
   or method binders, while the old nullable-display helper only recognizes
   nullable owner binders. They fall through to Rust `T.to_string()` (E0599).
   Use the checked Java Object result kernel for both optional/nonoptional and
   owner/method binders, respecting borrowed physical carriers. Do not add a
   Rust Display bound as a substitute for Java dispatch.
2. **Borrowed read contract**: V1 copies every non-retained child and imposes
   Clone even on slice-only generic display. Existing
   `tests/generic_collection_display.rs` explicitly proves no Clone is needed.
   Preserve that requirement. Borrow native elements for borrowed-only display;
   only retained cursors require canonical Java-reference copies. Derive
   capability obligations from solved physical shapes, not nominal List<T>.
   `CloneStorageContext` already holds signatures/representations/facts/program,
   and `expr_shape::shape_env_for_method` / ExprShapeContext are reusable for
   a shape-aware proof. Nested children and map components must follow the same
   policy. The custom Rust NoClone test uses the legacy deep-array trait as its
   display customization; do not simply delete/change expectations to mask the
   missing borrowed Java display contract.
3. **Static/instance field identity and storage**: GenericCollectionDisplay's
   `this.values = initial` incorrectly selects SyncCollectionDisplay's static
   setter; the static collection itself is emitted as Mutex<Rc<RefCell<...>>>,
   not a Send-safe canonical sync allocation. Static reads/operations lose the
   retained shape (`iter`/`push`/`clear` directly on Rc). These are not caused by
   the new iterator trait: frozen V149 on the *same expanded fixture* reproduces
   them. Baseline **15885** is terminal failure, **25 errors**:
   `.tmp/retained-display-expanded-v149-{baseline.rs,check.log}`. Do not rename
   fields or remove the static case to hide it. Two alias-return moves and
   nested/view shape mismatches also need review after owner identity is fixed.
4. **Borrowed handle staging**: nullable display exposes `&Rc` being inferred
   as the allocation-access H rather than Rc. Normalize/copy the canonical
   reference using existing ownership/borrow-target evidence before formatting;
   do not add a `'static` implementation for borrowed guards/references.
5. Some neighbor errors are Rust harnesses expecting String where correctly
   checked display now returns Result<String>. Adapt those through the existing
   String/Result test-value convention while preserving every Java result,
   null, exception and no-unnecessary-capability assertion. String.format
   currently remains infallible in the generic neighbor despite invoking the
   new checked display path: extend implicit-site effect proof to that API
   with platform/source precedence, not a name-only heuristic.

The source builds and library passes but the display batch is **not accepted**.
No release/corpus measurement should start until these shared contracts and
runtime neighbors pass. V149 remains the latest measured incomplete corpus.

## Display V2 follow-through: field/storage and generic dispatch

General fixes are batched across allocation constraints, environment seeding,
call lowering and reference cloning. Stored-array allocation policy now also
covers supported native field containers: local allocations for instance fields,
sync for static/sync owners, including nested native child paths. Rectangular
array row-view exceptions are preserved. Support/primary context seeding uses
SignaturePlan shapes and protects lexical instance/local bindings. Static
assignment declines proven instance targets, and the static inline-Vec mutation
shortcut declines retained fields in favor of the common operation renderer.
Generic checked display recognizes nullable/nonnullable owner and method binders.
Reference evaluation uses the canonical payload cloner, including Option<&Rc>
versus &Option<Rc>, rather than accidentally retaining a borrowed handle.

Two units cover solved lexical/static field precedence and local/sync nested
allocation domains. Cheap checks **83327** (lib, 24.94s) and **10499** (tests,
31.57s) pass. V2 combined build **35967** passes (1m07s). Collected gates:

- Primary **47993**, `.tmp/retained-display-contract-runtime-v2.log`, **1 / 0**
  (46.42s): nested/view/map/sync/no-alias-getter callback cases pass. This is the
  fixture before the additional valueOf/Objects cases below.
- Library **70716**, `.tmp/retained-display-contract-library-v2.log`, **1598 / 2**
  (112.85s). New units pass. The failures expect inline Vec fields/push staging.
  Assertions are now updated to require the retained nullable element carrier
  and handle < argument < mutable allocation borrow ordering. Those updates
  have not yet been rerun; semantic expectations are not removed.
- Nullable producer/metadata gate **41532** passes zero, artifacts
  `.tmp/retained-display-nullable-v2.rs` and
  `.tmp/retained-display-nullable-v2-check.log`. The prior twelve-error neighbor
  has no product compiler error before attaching its Rust harness. Nullable and
  generic harnesses now use the standard String/Result value adapter, preserving
  all expected values/null cases/NoClone checks.

Additional coherent follow-through is written: String.format implicit effects
use platform/source precedence; native Objects.toString(value, default) stages
both arguments before callbacks, preserves eager default evaluation, and uses
default on null. The full physical/child route is proved before staging. New
valueOf/default/null Java oracle checks pass **0**. Cheap V5 check **45350** passes
(35.20s). Formatting/diff checks pass.

Combined V3 build **36712** is live at
`.tmp/retained-display-contract-build-v3.log`, covering lib, source_call_staging,
lexical_super_calls, checked_stream_runtime, generic_collection_display and
nullable_collection_display. Collect it instead of rebuilding. Verify updated
primary/nullable, relevant runtime neighbors and library. No release/corpus is
live; V149 remains the latest measured incomplete baseline.

Still open: borrowed-only generic display copies children and adds unnecessary
Clone to CollectionDisplay. NoClone assertions remain and are expected to fail
until the borrowed contract is fixed. Retained cursors copy Java references;
borrowed slice/set/map display must not need Clone. Derive that distinction from
solved shapes (CloneStorageContext already holds signatures/representations and
expr_shape::shape_env_for_method is reusable). The test's custom Rust NoClone
provides the old deep-array display trait; the Any kernel does not know that
foreign type. Preserve its customization and expected values with a checked
borrowed leaf protocol, not a production fixture registration or Rust Display
bound. Generalizing the existing leaf protocol with a separate checked Object
operation is a candidate; actual array deep formatting must stay distinct from
Object/collection display. Do not delete NoClone calls or change expected
numbers. The overall display batch is not yet accepted or ready to freeze.

V3 build **36712** is terminal zero (1m11s). Fresh gates are live:
source_call_staging **16364** (all four tests), nullable display **2237** (two),
full library **15247** (1600), checked stream runtime **56671** (one), lexical
super **94089** (four). Logs respectively:
`.tmp/retained-display-contract-runtime-v3.log`,
`.tmp/retained-display-nullable-neighbors-v3.log`,
`.tmp/retained-display-contract-library-v3.log`,
`.tmp/retained-display-checked-stream-neighbors-v3.log`,
`.tmp/retained-display-super-neighbors-v3.log`.
Collect these exact handles before further production edits. Generic display's
known borrowed/NoClone contract is not rerun merely to rediscover the same
failure; that remains the next implementation capability after gate collection.
No release/corpus run is live.

## Borrowed display and typed Object follow-through

The subsequent rewrite audit separates two independent obligations: a display
operation on a borrowed element and a live retained cursor's copy of its Java
reference. The former must not require Clone. Lowering now borrows nonshared
collection children; RetainedDisplayCopy uses solved storage shapes and the
same stored-child projection as lowering, propagating only real copy needs
through selected source calls. Rc/Arc payloads shield their type parameters.

The checked leaf operation is distinct from array deep formatting. The old
leaf protocol retains backward-compatible custom Rust implementations, while
generated source objects and hierarchy carriers provide checked Object
adapters through the existing source method-call planner. Generic source
objects are dispatched as typed payloads, not guessed through an erased Any
registry. Callback null text and exceptions remain observable.

Fresh borrowed V2 build passes. Staging **65551** passes **4 / 0** (47.46s),
generic NoClone **56533** passes **1 / 0** (36.14s), nullable **65770** passes
**2 / 0** (45.67s). Independent expanded Java oracle **72093** returns **0**.
Full library **3888**, checked stream **54392**, lexical-super **49554** are
running; release **79784** is building at
`.tmp/retained-display-release-v150-build.log`.

Prior borrowed V1 exposed two real follow-through gaps, both fixed before
these gates: nullable source Object text needed owned String conversion, and
module-level display adapters had incorrectly been embedded into standalone
iterator support. The isolated Iterable component test is preserved unchanged;
the dependency is removed from the core runtime rather than hidden in its test.

Collect remaining gates and release. Only after passing, freeze V150 and run
the unchanged 104-file corpus in isolation, compare diagnostics and markers
against V149, then audit remaining whole-path failure families. No generated
Rust or pinned Java corrections are deliverables; the selected slice still
has no passing corpus or graph-behavior acceptance result.

Remaining borrowed V2 gates collected green: full library **1600 / 0**
(108.50s), checked streams **1 / 0** (31.49s), lexical-super **4 / 0**
(50.37s). All twelve integration probes pass. Only release **79784** is live;
collect, freeze, and measure the unchanged corpus without competing tests or
builds. This establishes batch regression evidence, not corpus acceptance.

Release **79784** passes (2m12s). Frozen V150 is **22,125,760 bytes**, SHA-256
`864d62687108779e50819b8c751786a76cfeb1c456c577817e02639911a3bc5f`, path
`.tmp/new-java2rust-v150-retained-display-contracts`. Isolated unchanged
104-file corpus **17532** is live at
`.tmp/jgrapht-core-1.5.3-focused-v150-retained-display-contracts`; log
`.tmp/jgrapht-core-v150-retained-display-contracts-run.log`. No competing own
tests/builds. Collect and compare V149 before more production edits. This is
a measurement in progress, not an accepted or completed translation.

### Read-only audit while V150 is measured

Producer provenance is verified against the frozen SHA/size/profile above.
Session **17532** remains live; emission finishes for all 104 files, with
translation phase **287863ms**. Assembly/check results are pending; do not
compare a partial diagnostic log or start a competing build/test.

Raw V150 AbstractGraph.to_string_from_sets uses the correct retained display
cursor, but its V bound contains JavaObjectDisplay/Any without Clone despite
the cursor copying Option<V>. The new display-copy scanner's root-method
lookup uses body pointer identity, then falls back to conservative parameter
types even when it found a signature by Rust name. Copied/specialized bodies
therefore risk losing the solved retained parameter shape. The already-existing
retained_callable_shape_context matches body equality plus exact params/return
and emitted method identity, and specializes inherited/wildcard methods in the
same frame as lowering. Reuse/generalize that helper rather than maintaining
a second weaker callable resolver. Confirm with fresh compiler output and a
generic retained-parameter/inherited-body regression; keep borrowed NoClone
and Rc/Arc payload shielding controls.

The older 56 get_class-on-&E diagnostics are duplicated through inherited graph
formatting. Their general runtime-class contract remains queued: JavaClass
tokens, generic caller capabilities, exact concrete hierarchy class, nullable
receiver checks and erased-object dispatch must agree. Do not add a facade
bound whose implementation is a stub, use Rust type names as Java class names,
or confuse monomorphized Rust types with erased Java generic classes. Select
the next architectural batch only after the current delta is reviewed; the
display parameter-contract follow-through takes precedence if confirmed.

### V150 terminal delta and selected follow-through

Corpus **17532** is terminal exit 1, generated Rust, with **1088 diagnostics
(1022 typed + 66 untyped)**; 104 files translate, zero translator/parse failures,
valid diagnostic scan, 23 guards, registry markers **64 / 25 files**. Provenance
matches the frozen release. Total **399.164s** (translate287.863, assemble86.604,
check18.985, sweep5.006); global69.782, emission195.809. Measurements were
isolated from own builds/tests but are unpaired, not stable speed conclusions.
V149 comparison is `.tmp/jgrapht-v149-v150-diagnostic-delta.txt`: **167 removed /
90 added** header/file occurrences. Renamed display bounds and closure source
locations are included; these are not distinct defect counts.

Critically, assembly drops `type Value = T` from both JavaSharedAllocationAccess
impls while retaining the trait's associated declaration. Its method-only
promotion path reconstructs impls with extract_methods, losing all associated
definitions. These E0046 errors can prevent downstream bodies being checked:
the 56 vanished get_class diagnostics are **not fixes**, as invalid calls
remain in generated source. New retained-operation markers, erased Clone,
capability, source facade precedence and allocation consumers remain open.
No acceptance/promotion is justified by the lower total.

Selected coherent follow-through now written:

- Display copy proof reuses retained_callable_shape_context instead of its
  duplicate pointer-only lookup; copied and inherited bodies keep solved
  parameter/local shapes. Expanded runtime fixture covers overloaded inherited
  parameter display and retained alias identity.
- Shared structural helper detects associated type/constant impl contracts;
  trait promotion selects those impls atomically through the existing preferred
  variant selector. Associated definitions cannot be paired with independently
  selected methods from another variant. Ordinary method-only merge is unchanged.
- Three fast Python controls pass, including an actual production merger,
  rustc compilation and execution of an Rc/RefCell generic trait with an
  associated type and private associated constant. Nested method-local aliases
  and const fn are not mistaken for associated definitions.
- Cheap library check **93675** passes (27s). Expanded Java oracle **12588**
  returns **0**. Combined build **48578** is live at
  `.tmp/retained-display-follow-through-build-v1.log` (library, staging, generic
  display, iterator production assembly, idiom script tests).

Collect this build and verify focused runtime/assembly/library gates. Source
is newer than V150; no release/corpus process is live. Do not run a full corpus
after each follow-up; finish the coherent producer/consumer contracts first.

Combined follow-through build **48578** passes (1m04s). Associated-contract
declarations are likewise selected whole, preserving their types/constants
when duplicate trait declaration text differs. Python V2 gate passes **3 / 0**
including actual merger/rustc/runtime; built shared_support script gate passes.
Fresh gates live: staging **55716**, generic **24941**, iterator production
assembly **10936**, library **82977**. Logs
`.tmp/retained-display-follow-through-{runtime,generic,iterator,library}-v1.log`.
Collect without restarting. No release/corpus run is live. V150 is not promoted.

### Forwarding capability closure after V150

Collected V1: full library **1600 / 0**, borrowed generic **1 / 0**, staging
**3 / 1** (inherited native field explicit display wrongly reaches Rust
ToString), iterator assembly **0 / 1** (handwritten consumer Step lacks the
already-required clone_any slot; translated library compiled). The test double
now supplies a cloned concrete Step, without changing behavior assertions.

Whole-path follow-through is written across lowering, generic proof and
representation adapters. Native explicit display can use solved physical
field evidence before type-only planning; the existing native-kind proof
retains source precedence. Retained-display copy proof joins the common Clone
query rather than only signature rendering, so interface/caller obligations
cannot disagree with source method bounds. ObjectDisplay/ArrayDeepDisplay join
the shared TraitImplCapability and selected implementation/caller conversion
paths. Source Rust Display and checked Java display reuse one inherited
toString selector and declaring-owner specialization/bounds computation.

Expanded Java fixture adds generic interface forwarding/conversion, inherited
generic toString, thrown leaf display, alongside overloaded retained parameters
and alias identity. Oracle **45893** returns **0**. Cheap tests check **44248**
passes (38.56s). Combined build **3426** is live at
`.tmp/retained-display-forwarding-build-v1.log` (lib/staging/generic/iterator
assembly/super/nullable/streams). A new second iterator assembly probe runs the
whole expanded display fixture through the production two-module merger; check
the built target lists it, since it was added while this build was running.
Collect then verify the coherent batch, preserving unchanged NoClone controls.
No release/corpus is live; do not promote the masked V150 diagnostics.

Forwarding build **3426** passes (1m00s). Iterator binary confirms both assembly
probes are present. Live fresh gates: staging **97788**, generic NoClone
**12640**, production iterator/display assembly **10782** (two probes), full
library **37143**. Logs
`.tmp/retained-display-forwarding-{runtime,generic,assembly,library}-v1.log`.
Collect without restarting. Built super/nullable/stream neighbors remain useful
at this batch boundary; no rebuild is needed. No release/corpus is live.

Collected forwarding V1: library **1600 / 0**, generic **1 / 0**, super **4 / 0**,
nullable **2 / 0**, streams **1 / 0**. Staging **3 / 1**; production assembly
**1 / 1**, with the original iterator probe passing. No associated Value errors
remain in the actual production merger. Expanded display has two production
errors (inherited U:Clone proof and inherited field Rust ToString), plus a
standalone duplicate inherited-toString fallback not reproduced in assembly.

Follow-through now shares the lowering inherited-field frame with retained
callable analysis; native display proof moves before the Object fallback that
actually intercepted the field call; Object fallback inventory honors the
shared effective inherited-toString selector even for a generic child without
own methods. No fixture assertions removed. V2 **25532** failed solely for a
missing Program import, now corrected. V3 combined build **94851** is live at
`.tmp/retained-display-forwarding-build-v3.log` (lib/staging/assembly/generic).
Collect it and run expanded standalone/production probes first; do not rerun
all green neighbors after each small follow-through. No release/corpus is live.

V3 build **94851** passes (1m02s). Targeted standalone display **75979**,
production display assembly **64569**, and unchanged NoClone **13021** are live
at `.tmp/retained-display-forwarding-{runtime,assembly,generic}-v3.log`. Collect
before repeating a broad gate; V1 library1600 and super4/nullable2/stream1 already
pass. No release/corpus is live. Expanded display acceptance is still unproven.

V3 standalone **75979** passes **1 / 0** (24.07s); actual production display
assembly **64569** passes **1 / 0** (35.51s); unchanged NoClone **13021** passes
**1 / 0** (15.04s). Inherited field/parameter, overloaded selection, interface
forwarding/conversion, generic inherited toString and callback behavior now
execute correctly in standalone and assembled Rust, matching the Java oracle.

Final library **71568** and V151 release **97919** are live at
`.tmp/retained-display-forwarding-library-v3.log` and
`.tmp/retained-display-forwarding-release-v151.log`. Collect both, freeze and
measure the unchanged corpus in isolation. This batch's passing small gates do
not imply corpus acceptance; V150 remains masked/unaccepted. Compare the next
result against V149 as well as V150 before selecting a new architectural batch.

Final library **71568** passes **1600 / 0** (89.70s); only release **97919**
remains live. Pinned checkout commit/unchanged tracked status is revalidated.
Collect release, freeze V151 with exact provenance, and measure in isolation.

Release **97919** passes (2m29s). Frozen V151 producer
`.tmp/new-java2rust-v151-display-forwarding-contracts` is **22,088,464 bytes**,
SHA-256 `40236cc99f3bfdf92f4773e124d561baa2955b715e15025b8075a739afbda631`.
Isolated unchanged 104-file corpus **32015** is live at
`.tmp/jgrapht-core-1.5.3-focused-v151-display-forwarding-contracts`; log
`.tmp/jgrapht-core-v151-display-forwarding-contracts-run.log`. No competing own
builds/tests. Collect this exact handle, verify producer provenance, compare
V149/V150, and audit newly visible whole-path families before further edits.
There is no V151 result or corpus acceptance claim yet; full goal stays active.

### V151 unmasked measurement and nullable capability closure

Corpus **32015** completes exit 1 with **1150 diagnostics = 1084 typed + 66
untyped**. All 104 files translate, no translator/parse failures, diagnostic
scan valid, guards23, registry64/25files. Provenance matches frozen V151.
Total402.306s: translation274.847, assembly101.010, check20.104, sweep5.647;
global59.549, emission196.317. No competing own builds/tests. These unpaired
observations do not establish a stable performance effect.

V150 comparison: **41 removed / 103 added** header/file occurrences. Missing
associated Value and forwarding display/Clone errors disappear. The expected
**56 get_class** errors reappear: they were masked, not fixed in V150. Thirty
nullable display leaf errors emerge, including AbstractGraph.to_string_from_sets:
method bound V:JavaObjectDisplay does not imply Option<V>'s legacy leaf bound.
V149 comparison: **102 removed / 87 added** (net -15), not distinct bug counts.
Neither result is accepted/promoted. Exact deltas are
`.tmp/jgrapht-v150-v151-diagnostic-delta.txt` and
`.tmp/jgrapht-v149-v151-diagnostic-delta.txt`.

Protocol reasoning: all JavaObjectDisplay implementations already come from
the JavaArraysDeepToStringElement+Any blanket adapter, but the trait declaration
did not expose those invariants. Declare its leaf/Any supercontract so generic
optional wrappers close compositionally; do not add arbitrary caller bounds,
register fixture-specific types, or introduce overlapping blanket/Option impls.
The old trait is a leaf compatibility hook; actual deep-array traversal remains
a separate operation from checked Object display. New NoClone Option/nested
Option controls and generic retained source-object null-element cases retain
all old assertions. Expanded Java oracle returns0.

Combined build **73095** is live at
`.tmp/retained-display-nullable-closure-build-v1.log` (generic/staging/assembly).
Collect, run targeted probes, and carry this small follow-through into the next
coherent root batch without another full corpus run just for this change.

Read-only queue audit: retained renderer still lacks QueueElement/LastElement,
Peek/Poll/Remove/LastRemove; stack.peek/pop and BFS removeFirst hit that boundary,
and inline push writes remain. Adding endpoint spellings alone is insufficient:
the same allocation's queue/deque orientation, empty exception/null result,
ArrayDeque vs LinkedList null policy, returned reference ownership, aliases,
mutation counts and iterator behavior must survive constructor/field/interface
contracts. Existing native storage policies/cursors and call planner should be
generalized together. Current backing's Vec/slice API makes front removal O(n);
do not claim it is an efficient deque backend or silently use queue policy for
priority/blocking queues. Reassess against fresh output before implementation.

Nullable closure build **73095** passes (52.76s). Live targeted probes: generic
nested-option NoClone **12843**, standalone **82869**, production assembly
**70569**, logs `.tmp/retained-display-nullable-closure-{generic,runtime,assembly}-v1.log`.
Collect; do not rebuild just to observe results. V151 stays the measured failing
baseline; source is newer. No release/corpus or broad library run is live.

Nullable closure gates collected green: **12843** generic/nested-option
NoClone **1 / 0** (15.65s), **82869** standalone **1 / 0** (23.47s), **70569**
production assembly **1 / 0** (34.32s). No live process. Carry this verified
follow-through into the next coherent batch, without a dedicated corpus cycle.

Next selected batch: retained queue/deque field consumers (stack.peek/pop/push,
BFS queue.removeFirst), starting with a neutral maintained Java oracle and
frozen-V151 translation baseline. Audit all producer/storage/signature/alias/
consumer boundaries and generic reference-copy capabilities before editing.
Exercise null/empty differences, endpoint orientation, argument order, returned
reference identity, alias mutations and existing cursor invalidation. Reuse
native operation planning and policy/cursor machinery; keep priority/blocking
queue policies separate and document actual backend complexity. Runtime-class
metadata remains queued. This is not an accepted corpus or narrowed goal.

### Stored deque full-path baseline and implementation

New maintained neutral Java fixture `retained_deque_contracts.java` independently
returns0. Frozen V151 translates it, but rustc metadata check fails26 diagnostics
at `.tmp/retained-deque-v151-baseline-check.log`; raw source and producer log are
`.tmp/retained-deque-v151-baseline.{rs,log}`. Actual needed rewrites are shared
constructor/storage/signature/qualifier/call/result contracts, not just method
spelling replacements: e.g. StoredDeque<T>.values stores Option<T>, while push
unconditionally unwraps and calls inline Vec syntax, peek borrows/clones/unwraps
an unsupported endpoint, and addFirst(null) fails to widen LinkedList storage.

First coherent implementation is written across planner, lowering, shape and
effect/capability proofs, plus common native runtime. QueueInsert represents
front/back and void/bool insertion metadata; stored-child flow now sees these
operations. ArrayDeque's constructor records a null witness for its actual
physical child, sets non-List iteration policy, and validates copies. Shared
endpoint helpers preserve owned-reference results and distinguish absent from
stored-null before flattening to Java null. The same result-shape projection
is used by inference, ordinary calls and stream lowering. Clone proof and
throwing endpoint effects use existing scoped selected-call machinery.

Policies stay allocation-owned across Queue/Deque/Collection aliases; LinkedList
continues allowing null. Deque addAll keeps accepted prefixes on null failure;
rejected insertion does not mutate count. Its non-List flag must not turn
ArrayDeque into a Set: equality/hash use identity for that concrete policy.
Backend remains Vec with O(n) front edits, documented honestly. Priority and
blocking queues are not replaced with this policy.

The maintained fixture covers local/generic/sync carriers, source reference
identity through mutation, empty/null behavior, aliases, copy boundaries,
captured cursor invalidation, and argument callbacks. Isolated native component
test also gains kernel policy controls. Cheap tests check3435 passes31.82s;
combined build **36680** is live at `.tmp/retained-deque-contract-build-v1.log`
(library/new deque/staging/generic). Collect and run exact component/new runtime
probes first. Do not assume new generated runtime passes or run a full corpus
merely for each follow-up. No release/corpus is live; full goal remains active.

Build **36680** passes58.21s. Exact native component **20341** passes **1 / 0**
(15.84s), `.tmp/retained-deque-native-component-v1.log`. Generated runtime
**62363** fails six compiler diagnostics (19.92s),
`.tmp/retained-deque-contract-runtime-v1.log`; artifacts
`.tmp/test-tmp/new_java2rust_retained_deque_contracts_1792525616352036953`.
No process remains live. Unsupported operation/inline insertion/null-comparison
baseline errors disappear, but the probe has not compiled/executed successfully.

Full-path follow-through audit:

- `box.first/last/peek/removeLast().value` receives actual Arc<Mutex<DequeItem>>
  but uses payload field syntax. Field lowering currently chooses
  source_receiver_declared_java_type/infer_expr_type ahead of instantiated
  physical result metadata. Reconcile generic source-call return substitution
  with common shared field receiver planning, not endpoint-specific lock text.
- StoredDeque.peek still emits returnT and an absence unwrap; Queue.peek must
  return null when empty even if callers only use present results. Add an
  unused generic helper signature regression so this proof comes from the
  native semantic contract, not call-site widening. Java fixture also gains
  empty box.peek assertion18 (after the preceding Rust run). Native peek/poll
  actually produce owned optional references, not borrowed Option<&payload>;
  maintain one Java nullable carrier at field/assignment/return boundaries.
- Local `copy = new ArrayDeque<>(alias)` remains Vec<Option<Arc<...>>> with no
  retained allocation policy. It creates nested peek optionals, and the later
  new ArrayDeque(linked-with-null) statement must throw (assertion14, not yet
  reached). Construction policy must survive all relevant local/argument/result
  slots, not only fields. Assess a canonical policy-carrying deque carrier
  through existing allocation constraints and transfer graph. Do not add
  per-variable hacks or silently use list policy for priority/blocking queues.

Finish this coherent ownership/nullability/allocation batch before another
full measurement. V151 stays the authoritative failing corpus, source is newer;
the overall goal remains active and unachieved.

### Deque follow-through: policy carriers and physical result consumers

The follow-through now uses one source-guarded runtime-policy query for
Deque/ArrayDeque/LinkedList. Signature/local baselines and factory-to-abstract
storage demands seed the existing allocation graph; Queue alone does not force
this backend. Temporary/discarded constructors use the same policy wrapper.
Raw backing construction must be separate from that wrapper: the V2 probe
exposed a second retained allocation inside nullable-element widening for an
empty generic deque. Copy payloads preserve child domains but take the new
factory's nominal class and non-null root.

Native peek/poll metadata now describes owned optional values. Native API
nullability feeds source return contracts independently of callers (the unused
generic helper signature gate passes). Field reads, writes and optional
receiver projection must consume the instantiated physical source-call ABI,
not just the original Java generic return spelling. Ordinary generic peek also
needs the same actual-child Clone proof as retained peek; removing a value does
not. The existing retained-map alias adapter is generalized to native
containers, admitting only equal physical carriers and copying handles rather
than backing values.

V2 combined build passes (61s), no-caller signature gate passes 1/0 (2.09s),
staging gates pass 4/0 (24.43s), generic display passes 1/0 (15.26s), and the
expanded independent Java oracle returns0, including assertions18/19. The
generated deque probe still fails7 compilation diagnostics (18.15s), exposing
double construction, remaining nullable receiver consumers and non-retained
peek Clone proof; these are not seven independent bugs. Logs:
`.tmp/retained-deque-{build,contract-runtime,source-staging,generic-display}-v2.log`
(the build log is named `retained-deque-build-v2.log`). No generated Rust or
pinned corpus source was edited.

Those coherent follow-through changes and additional source-precedence/backend
and abstract-slot regressions are under verification in V3b; do not treat this
as accepted or rerun the full corpus before the focused contracts pass.

V3b build passes61s; constructor gates3/0 (21.42s) and map-view gates7/0
(27.42s) pass. The policy control exposed qualified ArrayDeque recognition;
the source-guarded policy now recognizes the platform leaf consistently.
V3 also exposed typed raw constructors choosing nominal rather than solved
stored children. Empty/capacity raw construction now reuses the existing
slot-aware helpers, avoiding both nested allocation and nominal Vec<T>.

V4 build passes74s; the complete library passes **1604 / 0** (93.54s), including
no-caller nullable endpoint, abstract Collection factory placement, source
precedence/backend exclusion, and explicit substitution of Unknown binder forms.
The shared substitution utility now supports both Named and Unknown type
parameter spellings under an explicit substitution map; unrelated unknowns are
unchanged. Generated deque still fails7 compiler diagnostics (43.27s while
library gates ran concurrently): one empty generic constructor null-witness
inference hole plus six field consumers. No runtime acceptance. Its null
witness now has the actual stored-child parameter type, and an internal
physical-source-result/field-receiver regression is being built in V5 to
isolate the remaining metadata gap before further downstream edits.

Logs `.tmp/retained-deque-{build,contract-runtime,library}-v4.log`,
`.tmp/retained-deque-{map-gates,constructor-gates}-v3.log`. V151 remains the
latest full-slice measurement. Do not infer corpus success from these gates.

V5 build passes68s. The new internal shape regression fails with concrete
evidence: physical `box.first()` is `Optional<Object>`/ErasedObject even though
the emitted method returns `Option<T>` and its receiver instantiates T as
DequeItem. The final source-return overlay was replacing the receiver-bound
source ABI with resolution's erased Object summary. The existing guard only
recognized an unerased binder in `resolved.return_type`. It now identifies an
owner binder from the selected source declaration (respecting method binder
shadowing), preserving the already-instantiated emitted ABI when resolution
has erased that binder. Native endpoint consumers likewise obtain the physical
child from the existing retained endpoint result projection rather than letting
an external nominal summary replace it. No endpoint-specific field repair.
V6 is the next verification candidate; V4 library evidence remains prior
evidence, not proof of V6 or of deque runtime behavior.

V6 generated probe still fails six field consumers (18.81s); typed constructor
null-witness errors are gone. The diagnostic-rich V7 internal gate corrects
the preceding hypothesis: resolution already reports DequeItem and the direct
source result is Optional<DequeItem>, but the complete source result becomes
Optional<Object>. The later representation-aware body-return helper inferred
erasure from *any* facts-versus-representation Rust type mismatch, including
ordinary source Rc/Arc/cell storage. This is not proof of erased Object.
Its erasure test now positively identifies Any payloads (including through
Option), not an arbitrary changed carrier. V8 verification is pending. The
maintained internal test retains the full selected-signature/direct/source/
resolved-call/receiver diagnostics for future failures. No generated output
repair was used.

V8 build passes60s and the internal source-result/field-receiver test passes
1/0 (3.11s); staging4/0 (35.02s), generic display1/0 (23.64s) also pass.
The standalone deque runtime still fails the same six field diagnostics
(29.13s). Comparing internal and real pipeline context reveals another boundary:
the prepared callable context defaults to an empty emitted-source inventory
outside batch translation, while the internal test supplies one explicitly.
The common lowering entry now extends the supplied exact inventory with the
current compilation unit (always emitted), instead of treating own source
methods as facades merely because no surrounding batch was supplied. This
unifies standalone and co-translated ABI selection at their shared authority.
V9 is under verification; do not claim this closes all six consumers yet.

V9 build passes78s; full library **1605/0** (97.21s), staging4/0 (47.32s),
generic display1/0 (35.04s). Generated deque still fails **four** diagnostics
(50.39s alongside the broad gates): first/last/removeLast source calls now
select Arc field access but omit their nullable receiver projection; copy.peek
assignment still projects an Option element directly rather than its Arc
payload. Plain source peek and the caught-null read no longer fail compilation.
Logs `.tmp/retained-deque-{build,library,contract-runtime,source-staging,generic-display}-v9.log`.

The internal regression is expanded to use planned storage facts, actual
callable fallibility and Propagate mode (matching standalone translation), and
to test the copied deque's solved local shape and writable field projection.
Its prior passing metadata-only/unpropagated case was insufficient to prove
those consumers. V10 exact gate is pending at
`.tmp/retained-deque-physical-source-v10.log`; collect its rendered projection
and shape evidence before further edits. No release/corpus was started.

V10 initial test build attempted private SignaturePlan.methods access; V10b
uses the existing method_signature_by_rust_name accessor. V10b exact internal
gate passes1/0 (3.86s), build53.59s. It uses real storage/fallibility planning,
Propagate mode, a rendered nullable source read and the copy local's solved
write projection. This still does not reproduce the four generated-body
diagnostics: the actual consumer/frame or an earlier competing lowering path
must be observed directly. Do not infer from this helper gate that the emitted
caller is fixed. All handles are terminal; no process/release/corpus is live.

### Real-body trace: Result discharge is not null discharge

V11 adds opt-in `NEW_JAVA2RUST_TRACE_FIELD_RECEIVERS` tracing, disabled by
default. The focused integration build passes51.33s. Real-body trace
`.tmp/retained-deque-field-trace-v11.log` confirms first() has the correct
Optional<DequeItem> shape and the optional plain-read helper produces
`first()?.as_ref().expect("Java null receiver")`. Its null check disappears
later in `make_java_null_expect_catchable`: that routine treated *any* Try
under as_ref (or any ResultPropagated receiver) as optionality already
discharged. Result<Option<T>> needs two independent checks, not one.

V12 replaces those two broad shortcuts with the existing shared
`java_optionality_already_discharged` proof. A minimized regression requires
both `?` operators and NPE conversion, and verifies rewrite idempotence. The
maintained source-field regression now checks the expression after catchability
rewriting, not merely its pre-rewrite projection. The fallback nullable field
write also uses the physical value shape and existing shared-storage element
receiver projection; a nullable reference to Arc/Mutex must not access the
Arc wrapper as a payload field. Constructor/source/alias changes remain intact.
V12 combined verification is pending; no corpus/release is live.

V12 build passes61s. Deque generated runtime **1/0** (17.58s) finally matches
the unchanged Java oracle0; minimized two-layer null check1/0 and expanded
physical source gate1/0 (3.90s) pass. Full library **1606/0** (108.17s), staging4/0
(31.18s), generic display1/0 (19.61s), nullable display2/0 (43.84s), checked
streams1/0 (35.82s), production assembly2/0 (65.15s) pass. These concurrent
gate times are not isolated performance measurements.

Broader gates expose two coherent follow-through contracts, not deque kernel
failures: constructor gates2pass/1fail and map gates6pass/1fail each report one
E0282 for a static generic factory's unused class owner binders. Catalog
retargeting discarded Java owner identity and re-looked up a declaration in a
narrow frame, falling back to a bare Rust owner. V13 preserves the positive
catalog's (Java owner, Rust owner) pair and reuses the existing representation
raw-owner erasure renderer. No new wildcard/Any convention.

Lexical super gates3pass/1fail expose a second boundary: a base's unresolved T
return was incorrectly trusted as receiver-specialized in a Child<String>
frame, so nullable String equals missed its receiver check. The source binder
preservation guard now requires that the declaring binder has actually been
substituted or remains a proven in-scope binder; otherwise the resolver's
concrete return overlay must finish specialization. Added minimized positive/
negative owner-erasure and unresolved-binder controls. V13 verification pending.

Release V12 build session12779 is terminal0 (139s), log
`.tmp/retained-deque-release-build-v12.log`, but **predates V13 follow-through**.
Do not freeze/measure target/release as a current candidate until rebuilding
after the broader regression gates are fixed. No corpus measurement is live.

### V14 verified batch and V152 diagnostic checkpoint (2026-10-20)

The V13 qualified-owner control exposed a catalog declaration key versus
package inventory key mismatch. V14 admits only an unambiguous catalog-proven
Rust alias with already planned raw arguments, through the same static-owner
renderer; it does not invent erasures. The inherited-binder follow-through
also passes its positive and negative controls.

V14 test build54.17s; library **1608/0** (88.72s), deque **1/0** (44.00s),
constructor **3/0** (36.29s), map **7/0** (50.00s), lexical-super **4/0**
(62.09s), qualified static owner **1/0** (0.01s). Logs are
`.tmp/retained-deque-{build,library,contract-runtime,constructor-gates,map-gates,super,static-owner}-v14.log`.
V12 production assembly2/0, nullable display2/0, checked streams1/0, staging4/0
and generic display1/0 remain prior evidence, not newly rerun V14 results.

Current release build is terminal0, 105s, log
`.tmp/retained-deque-release-build-v14.log`. Frozen candidate
`.tmp/new-java2rust-v152-retained-deque-contracts` is 22,423,696 bytes,
SHA256 `7692d388eb4fc976b3bb0f526a301474dffc84a8b57c558c77051167336b1e81`.
The isolated unchanged 104-file checkpoint is running under session89102,
root `.tmp/jgrapht-core-1.5.3-focused-v152-retained-deque-contracts`, runner log
`.tmp/jgrapht-core-v152-retained-deque-contracts-run.log`. The independent
resolver benchmark is skipped; translation, assembly and cargo check remain
enabled. No competing build/test is running.

The new goal makes this a diagnostic checkpoint, not an obligation to close
all remaining JGraphT errors. Next select a smaller headless corpus using a
fresh baseline and generated-rewrite feasibility audit, avoiding prior corpora.

V152 is now **terminal1**, due to generated Rust diagnostics rather than a
runner/translation failure. Summary reports104 files, translator failures0,
parse failures0, cargo_type_errors_valid1. Exact diagnostic multiset:
**1054 = 978typed + 76untyped**, allocation guards38, registry markers74 in
31files. V151 had1150=1084+66, guards23, markers64/25files. Read-only comparison
`.tmp/jgrapht-v151-v152-diagnostic-delta.txt` records144 removed and48 added
header/file occurrences, net-96 (not independent root-cause counts).

Removed include30 V display-leaf bounds,15 Option.borrow failures,26 type
mismatches, checked deque operations and nullable endpoint field access.
Added include15 identity-preserving allocation guards,24 generic static-owner
JavaMapKey bounds, missing Partitioning/Embedding traits, one retained set
retain call and a nullable bucket projection. Source fixes remove failures but
also expose deeper/changed contracts; do not claim uniformly closed families.
Remaining architectural gaps include erased Object/graph/interface ownership,
generic key/Clone/display capabilities, callbacks with checked effects, and
source/allocation representation boundaries. No graph behavioral acceptance.

Isolated timings: total319.334s, translation229.072s, assembly71.063s,
check13.777s, sweep4.706s; global signature28.876s (fixpoint5.237s),
emission160.332s. Prior V151 total402.306s is an unpaired observation, not a
speed claim. All raw evidence is under the V152 output root above. This
completes the requested JGraphT diagnostic checkpoint; retain it as an
incomplete stress target rather than starting another full fix campaign.
