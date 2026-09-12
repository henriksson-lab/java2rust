# 0008 — The inference machinery exists but does not reach lowering

Status: **validated by inspection** (structural)
Severity: medium-high — explains why lowering keeps growing ad-hoc predicates
Kind: architecture / wiring

## Observation

The project has real inference machinery:

- `src/lattice.rs` — `LatticeDescriptor<T>`, `RankedLatticeJoin<T>`, and value
  domains: `DispatchLatticeValue`, `EscapeLatticeValue`
  (`LocalOnly | Returned | Stored | UnknownBoundary`),
  `InitializationLatticeValue`, `NULLABILITY_VALUES`, `OWNERSHIP_VALUES`,
  `VIEW_VALUES`.
- `src/fact_saturation.rs` — fixpoint saturation over facts.
- `src/graph.rs` — graph utilities.
- `src/usage.rs` — a real call graph (`CallGraph`, `CallableKey`,
  `CallGraphEdge`) with usage facts.

Follow-up 2026-09-10: the shared SCC utility in `src/graph.rs`
(`StronglyConnectedComponents` and `strongly_connected_components`) is
crate-local. It remains live in `signature.rs` and `semantic_analysis.rs`, but
is no longer exported as public crate API.

Follow-up 2026-09-12: `fact_saturation` is crate-internal as well. The finite
and worklist saturation helpers remain live in the upstream facts, semantic
analysis, and signature layers, but they are no longer exported as public crate
API. The historical lowering observation below is unchanged: lowering still
does not consume the saturation machinery directly.
Follow-up cleanup tightened this further by spelling the internal saturation
engine pieces as `pub(crate)` inside the private module: `SaturationStep`, the
finite/worklist result carriers, the worklist policy/inserter, and the
finite/worklist entry points. The small public DTOs that existing public queries
return (`FiniteFactDomain`, `FiniteSaturationPolicy`, and `WorklistStats`) stay
public but private-backed.

Follow-up cleanup: the ranked lattice join carrier and rank helpers in
`lattice.rs` are crate-local now too. Signature solving still consumes them for
requirement precedence, but the lattice utility is an internal analysis
primitive rather than public crate API.

None of `lattice`, `fact_saturation`, or `graph` is referenced from
`src/lower.rs`:

```
$ grep -l 'lattice' src/*.rs
fact_saturation.rs  lib.rs  lattice.rs  semantic_analysis.rs
resolver_benchmark.rs  signature.rs

$ grep -l 'fact_saturation' src/*.rs
lib.rs  facts.rs  semantic_analysis.rs  signature.rs

$ grep -l 'crate::graph' src/*.rs
semantic_analysis.rs  signature.rs
```

The machinery is confined to the upstream fact/signature layer. `src/lower.rs`
— 23.7 kloc, the largest consumer of semantic information — reaches none of it.

## What lowering does instead

It re-derives the same information locally, with 206 `-> bool` predicates and
six ad-hoc name-set fields on `LowerCtx`
(see [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md)).

Predicate counts by module:

| module | `-> bool` fns |
|---|---|
| `lower.rs` | 206 |
| `stdlib.rs` | 200 |
| `signature.rs` | 136 |
| `call_plan.rs` | 43 |
| `semantic_analysis.rs` | 38 |
| `coercion.rs` | 30 |
| `facts.rs` | 30 |
| `conversion_plan.rs` | 22 |
| `usage.rs` | 12 |

Reading the `lower.rs` names, they cluster into a small number of relations
that each *want* to be one analysis:

- **Liveness / move legality** (~17 predicates) — see 0006.
- **Shape specificity / subsumption** (~10): `shape_has_more_generic_information`,
  `lowered_type_slot_has_more_generic_information`,
  `initializer_shape_is_more_specific_for_local`,
  `local_shape_should_inherit_initializer_shape`,
  `cast_target_erases_slot_generic_detail`,
  `java_types_have_same_erased_identity`, `same_concrete_java_type`, …
  These are hand-rolled comparisons on a **partial order over shapes**. A
  partial order with an explicit `join`/`meet` and a subsumption test would
  replace all of them, and would make the "which shape wins" question answerable
  once instead of per site. `src/lattice.rs` already has the vocabulary
  (`OWNERSHIP_VALUES`, `VIEW_VALUES`, `RankedLatticeJoin`) — it is simply not
  applied to slot shapes at lowering time.
- **Nullability refinement** (~12): `expr_type_is_optional`,
  `non_null_refinements_make_optional_expr_an_element`,
  `refined_non_null_receiver`, `exact_erased_object_local_is_optional`, …
  This is flow-sensitive **refinement typing**. A partial version already exists
  (`ExprRefinements` with `non_null_locals`), threaded manually through block
  lowering. `NULLABILITY_VALUES` exists in the lattice module and is unused here.
- **Hardcoded library knowledge** (~15): `is_java_get_class_name`,
  `is_known_exception_supertype_name`, `java_type_is_string_type`,
  `current_type_extends_jdk_input_stream_facade`,
  `receiver_is_known_string_producing_call`, `java_type_name_is_tuple2`,
  `expr_is_tuple2_owner`, … Library-specific facts compiled into the translator
  rather than declared in `javalibs` manifests. The convergence plan already
  calls this out; the count is the argument for prioritising it.

## Why the wiring is the problem

Each cluster above has a matching piece of machinery that already exists one
layer up. The translator is not missing the *idea* of a lattice or a fixpoint —
it is missing a path for lowering to consume one. The result is that every new
corpus failure gets fixed where the failure is observed (in `lower.rs`), because
that is the only place with the context, and the predicate count grows.

This is consistent with the repository's own history: `docs/` contains ~20
sequential `proceed-from-current-corpus-*` plans, which is the signature of
fix-at-the-symptom.

## Proposed direction

1. **Pick one relation and route it end to end** rather than refactoring
   broadly. Liveness (0006) is the best first candidate: it has a confirmed
   miscompilation ([0001](done/0001-loop-back-edge-invisible-to-move-analysis.md)),
   a clear specification, and an obvious query surface.
2. **Give lowering a single analysis handle.** Replace the six `Option<&HashSet>`
   fields with one non-optional `&CallableAnalysis` carrying the results of the
   liveness, nullability, and shape-subsumption queries.
3. **Make slot-shape subsumption a real partial order.** Define
   `subsumes(a, b) -> bool` plus `join(a, b)` on `RustSlotShape` and delete the
   ~10 "more generic information" predicates. This is self-contained and does
   not need a CFG, so it can proceed in parallel with the liveness work.
4. **Move the ~15 hardcoded library predicates into `javalibs` manifests.**
   Mechanical, low risk, and it shrinks the surface that the other work has to
   reason about.

## A caution

The convergence plan notes that "a temporary corpus error-count increase is
acceptable". That is right, but with 206 predicates in one file and no
binding-identity model, a broad simultaneous refactor will produce an error
increase that cannot be attributed. Sequence by relation, keep the old predicate
and the new query both computed behind an assertion during migration, and use
the divergence list as the work queue.

---

## Update 2026-08-03 — steps 1–3 largely done; step 4 is what is left

Status: **partly done**. The headline metric in this file is now misleading and
should not be used to judge it.

### Measurements

| | when filed | 2026-08-03 |
|---|---|---|
| `-> bool` fns in `lower.rs` | 206 | **159** |
| `Option<&HashSet>` name-set fields on `LowerCtx` | 6 | **2** |
| `grep -c lattice src/lower.rs` | 0 | **0** |

### Step 1 — done, twice

*"Pick one relation and route it end to end."* Liveness was the chosen
candidate and is now a real backward fixpoint over binding identity
(`src/liveness.rs`), consumed by lowering through statement identity rather
than through pre-projected name sets. See
[0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md) and
[0001](done/0001-loop-back-edge-invisible-to-move-analysis.md).

A second relation followed, and it is the more interesting one because it was
*composed* from two existing solved facts rather than written from scratch:
`src/aliasing.rs` joins the liveness fixpoint with
`MethodUsageFacts.mutates_self` (already a fixpoint over self-calls in
`src/usage.rs`) to decide whether an inserted clone is observably wrong. That is
the first consumer of `mutates_self` outside signature planning, and it produced
a diagnostic with 1 hit across 196 corpus files — an analysis, not a heuristic.
See [0002](0002-object-aliasing-has-no-representation.md).

The lesson worth carrying: the payoff was not "lowering can now call a lattice",
it was that two analyses that already existed could answer a question neither
could answer alone, once something owned the join.

### Step 2 — done

`CallableAnalysis` (`src/callable_analysis.rs`) is the single handle, carrying
`usage`, `call_graph`, `signatures`, `exceptions` and `fallibility`;
`LowerCtx.callable_analysis` is non-optional. Four of the six ad-hoc name-set
fields are gone. The two that remain are load-bearing and documented as such: a
lambda whose body is a bare expression lowers into a loop with no `Stmt` of its
own, so there is no statement for the analysis to key on, and the override is
unioned with — not preferred over — the solved answer.

### Step 3 — done as far as it is honest to take it

The ~10 "more generic information" predicates are one method,
`RustSlotShape::has_more_generic_detail_than` (`src/slot_shape.rs`), and the
last pass-through wrapper in `lower.rs` is deleted; call sites use the method.

**What was deliberately not done, and why.** The obvious next move is to express
that order through `crate::lattice` and finally make the `grep -c lattice` count
non-zero. It was tried and rejected: the lattice ranks encode *storage
requirement strength* (`Owned` 40 > `SharedBorrow` 10), while the shape order
here is *generic detail for the same erased type*. They are different orders
over the same values. Routing one through the other would raise the metric this
file publishes while making the code less precise — for instance replacing
`matches!(shape.dispatch, DispatchShape::Concrete)` with a rank comparison turns
a variant test into a "top of the lattice" test, which stops being equivalent
the moment a second variant shares the top rank.

So `grep -c lattice src/lower.rs` is still 0, and that is the right answer.
The count was a proxy for "lowering re-derives what upstream already solved",
and that underlying problem *has* moved — 206 → 159 predicates, and two whole-
program relations now consumed rather than approximated. **Judge this issue by
the predicate count and the list of relations lowering consumes, not by the
lattice grep.**

The lattice reaching lowering remains correct as an *end state*, but it needs
the ownership/view/nullability axes folded into a single shape order first, with
`join` and `subsumes` defined on `RustSlotShape` — the doc comment on
`has_more_generic_detail_than` already marks the spot. That is a design task, not
a wiring task.

### Step 4 — not started, and now the largest remaining item

The ~15 hardcoded library predicates (`is_java_get_class_name`,
`java_type_is_string_type`, `current_type_extends_jdk_input_stream_facade`,
`receiver_is_known_string_producing_call`, …) are unchanged, and `stdlib.rs`
still holds 195 `-> bool` predicates — more than `lower.rs`. This is the
mechanical, low-risk work this file recommended, it is where the remaining
predicate mass actually is, and nothing in the current state blocks it.

---

## Update 2026-08-03 (second pass) — step 4 started, and the lattice finally has a reason to reach lowering

### Step 4, honestly scoped

Two of the hardcoded library predicates are moved: the JDK exception-supertype
set and the `java.io` input-stream facade set now live in `src/javalibs.rs` as
named constants with one query and one test each, and `lower.rs` asks the
library module instead of embedding the names.
Follow-up 2026-09-10: the Broad IOUtils program detector is crate-local too;
its direct import/qualified-use assertions live beside `javalibs.rs`, while
integration coverage stays on the public platform-facade support item and
generated output.
Follow-up 2026-09-10: five remaining `javalibs` helper queries used only by
crate internals are crate-local as well:
`facade_sequence_element_type`,
`generic_facade_interface_erases_to_named_rust_type`,
`native_java_static_owner_alias`, `preferred_static_owner_alias`, and
`wildcard_import_static_type_aliases`. The live callers remain in
type/slot/signature lowering, call planning, stdlib, and import handling; none
needs these helpers as public crate API.

The rest of the list in this file needs re-reading, because it conflates two
different things:

- `is_known_exception_supertype_name`, `current_type_extends_jdk_input_stream_facade`
  — genuinely *library* knowledge: facts about the JDK compiled into lowering
  rules. These are the movable ones.
- `is_java_get_class_name` (`matches!(name, "getClass" | "get_class")`),
  `java_type_is_string_type` — **core language** knowledge, not library
  knowledge. Moving `String` into a library manifest would be
  miscategorisation, not progress.

And the destination is not what the issue assumes. `src/javalibs.rs` is Rust,
not a manifest format; the real manifest system is `external-pack` lines parsed
by `src/external_packs.rs` into a `FactSet`. Expressing these predicates as
manifest *data* is therefore a design task — deciding what the manifest grammar
must gain — not the mechanical move this file describes. **That reframing is the
main output of this pass on step 4**, and it should be settled before anyone
budgets the rest.

### The lattice now has a consumer, for a reason that holds

`grep -c lattice src/lower.rs` is still 0, and this file's previous update was
right to say chasing that number was wrong. What changed is upstream:
`src/storage_strategy.rs` computes an ordered lattice
(`Value < SharedImmutable < SharedLocal < SharedSync`, join = max) whose domain
mirrors `EscapeLatticeValue` — the value domain that has sat in `lattice.rs`
since it was written with no analysis behind it.

That is a **third** whole-program relation routed end to end, after liveness and
aliasing, and the first one whose shape the `lattice` module actually anticipated.
Worth recording precisely because the previous update rejected a *forced* use of
the lattice: the difference is that this one needed the order, rather than
needing the metric to move.

### Counts

| | filed | first pass | now |
|---|---|---|---|
| `-> bool` fns in `lower.rs` | 206 | 159 | **158** |
| `-> bool` fns in `stdlib.rs` | 200 | 195 | 196 |
| relations solved once and consumed by lowering | 0 | 2 | **3** |

The predicate count barely moved, and that is the honest reading: this pass
spent its effort on correctness (a diverging fixpoint, a mis-keyed override
group, a dropped array dimension) rather than on the count. The count is a proxy;
the relations row is the thing it was a proxy for.

---

## Update 2026-08-05 (third pass) — the tables were surveyed, and several encode a rule they have drifted from

Step 4 asked for the hardcoded library predicates to be moved. A survey of the
*enumerated* decisions found something more useful than a relocation target:
several of these tables are the extension of a rule, and at least four have
drifted from it in ways that are silent defects rather than verbosity.

### Fixed this pass

**`is_known_java_subtype`'s exception arms were membership lists, and
`catch (IOException e)` matched three types.** A thrown `ZipException`,
`SocketException`, `UnsupportedEncodingException`, `InterruptedIOException`,
`MalformedURLException`, `UnknownHostException`, `NotSerializableException` or
`NoSuchFileException` did **not** match a handler Java says it must — and a
caught exception silently escaping is not something the generated program can
report.

Replaced by `JDK_EXCEPTION_PARENTS`: parent **edges**, with the relation as
their reflexive-transitive closure. This is still a table, deliberately — no
name rule can know that `ZipException` is an `IOException`, unlike
`is_known_exception_type`'s `ends_with("Exception")` — but it is a table of
*facts* rather than of *answers*, and one edge now reaches every ancestor
instead of needing an entry per ancestor.

The tests caught a second instance of the same mistake **in the replacement**:
the dispatch still named `"IOException" | "RuntimeException"` as the targets it
would answer, so `catch (ZipException e)` could not match a `JarException` even
though the edge existed. The target set is now derived from the table too.
That is the argument for the edge form in miniature — the list shape reappeared
one level up and only a law-shaped test found it.

**Four nullability entries claimed `DefinitelyNonNull` for methods that return
null.** `Attributes.getValue` (absent attribute), `JarFile.getManifest` (no
manifest — folded into the same arm as `entries`, which genuinely is non-null),
`Graphics.getClip` / `getClipBounds` (no clip set), and
`Class.getClassLoader` (bootstrap-loaded classes, i.e. every `java.*` type).

This direction is not benign: `known_external_call_lowered_type` wraps in
`Option` only for `MaybeNull`/`DefinitelyNull`, so a wrong non-null is a
**dropped null check**, not a missed optimisation.

### Also fixed this pass

**The JLS §5.1.2 widening relation was encoded four times, and three were
wrong.** `types::primitive_widens_to` is now the single definition; the three
rank scales and the 19-pair table all delegate to it, and the pairs survive as
`primitive_widening_pairs_match_the_jls` — the table is now the *check* on the
rule rather than a fourth copy of it.

The three facts the ranks got wrong, each with its own test so a regression says
which one returned:

- **nothing widens to `char`.** The rank form made `char`→`short` and
  `short`→`char` distance **0** — scored as *exact* matches — so `f(short)` tied
  with `f(char)` for a `char` argument, and `byte`→`char` reported `widens` and
  emitted a conversion Java rejects.
- **`char` widens only to `int` and above.** It is a separate root, not a point
  on the numeric chain.
- **`boolean` participates in nothing.** `resolve.rs` guarded it;
  `conversion_plan.rs` did not, so `boolean`→numeric reported `widens`.

Also asserted as laws rather than cases: widening is a strict partial order
(irreflexive, antisymmetric, transitive), and the distance is defined *exactly*
where widening is — which is what stops an illegal conversion outranking a legal
one.

**Three smaller omissions, each behind a fallback that hid it.**
`canonical_external_receiver_type_key`'s `"type" => "Type"` arm was dead — the
table is the inverse of `to_snake_case`, which escapes the keyword to `type_`,
so the key a producer generates was not in the table and fell through
`other => other`. `is_known_functional_interface` was missing eleven members of
its own naming grammar — every `*UnaryOperator` present, no `*BinaryOperator`,
which is the tell that a product was being enumerated. And
`native_java_call_lowered_type` had `IntegerParseInt` and `FloatParseFloat` but
not `DoubleParseDouble`, behind a `_ => None`.

Checked and **not** a defect: `to_snake_case` escapes only five keywords, but
`rust_raw_ident_if_needed` raw-escapes downstream, so a Java field named `move`,
`ref`, `mut`, `fn` or `impl` compiles correctly as `r#move` and friends —
verified end to end rather than assumed from the list's length.

### The last recorded item: the two enumerations of one key space

`known_external_method_return_type` (185 keys) and `external_api_return_nullability`
(154) enumerate the same `(receiver, name, arity)` space and had drifted: 14 keys
in the first were absent from the second, and the two used three different
receiver-matching disciplines, so `java.lang.System` got a return type and
`Unknown` nullability.

Closed by **deriving** rather than listing. A `primitive` or `void` return
cannot be null, whatever any table says about the method — that is a property of
the type, not a fact about the API — so the nullability fallback now reads the
kind of the answer the *other* table already gives, with the receiver
canonicalised once so the two spellings cannot diverge again.

This is axis (a) of the survey's suggested decomposition. Axis (b) —
`ExternalApiRole`, which `external_api_receiver_mutability` already uses this way
for mutability — is still available if the residue proves large enough to be
worth it; it is now a size question rather than a correctness one.

The test written for it caught the drift immediately: `Integer.parseInt` answered
`DefinitelyNonNull` while `java.lang.Integer.parseInt` answered `Unknown`, which
is exactly the receiver-discipline split the survey predicted.

**One test moved and it was a stale expectation, not a regression.**
`external_numeric_domain_getters_are_read_only_facts` asserted `Unknown` for four
numeric getters; three of them have a modelled `int`/`long` return and now
correctly derive `DefinitelyNonNull`. The fourth exposed something worth
recording: `ExternalApiEffectFact` is keyed by `(receiver, method)` with **no
arity**, while nullability is now per-arity, so what the fact holds depends on
the arity the source called with. The assertion was scoped to what the test is
named for — read-only-ness — and the arity-keying gap is noted here rather than
papered over.

### The shape of the finding

The survey's own summary is the useful part: `slot_render.rs`'s
`slot_rust_type_policy` is the counter-example — it composes
`ownership × (view, dispatch, java_ty)` with each axis matched once and no
cross-product enumerated. Every defect above is a cross-product that was
enumerated instead.

Codex follow-up 2026-09-12: the erased Java type identity predicate moved out of
`lower.rs` and into `slot_shape.rs`, next to `RustSlotShape`'s
generic-detail order. Lowering still asks the same question at the same call
sites, but there is no longer a local duplicate deciding whether two slot shapes
share one erased runtime type. Guard:
`slot_shape::tests::erased_identity_ignores_named_generic_arguments_only`.

Codex follow-up 2026-09-12: the related "has generic or sequence detail" query
is now owned by `slot_shape.rs` too. `RustSlotShape` exposes
`has_generic_or_sequence_detail()`, and lowering uses that method where it
already has a shape instead of re-deriving detail from raw `JavaType` locally.
This keeps raw-erasure cast and exact generic external-call lowering on the same
slot-shape query as the generic-detail order. Guards:
`slot_shape::tests::generic_or_sequence_detail_covers_named_args_and_facade_sequences`,
`type_coercion::cast_semantics_policy_names_generic_raw_wildcard_and_optional_result_casts`,
`idiom_gaps::raw_list_cast_over_typed_external_call_uses_target_slot_shape`,
and `idiom_gaps::static_external_call_to_generic_set_slot_uses_external_return_shape`.
