# Round 5 plan — the ten requested issues

Scope: 0002, 0002b, 0006, 0007, 0008, 0009, 0010, 0018, 0021, 0022.

Everything below is measured against the tree on **2026-08-03** (HEAD `b391645`
plus the uncommitted audit files), not against the state the issues were filed
in. Several have moved a long way; two are further along than their own files
say, and one is *less* closed than `NEXT-BATCH.md` records.

---

## 1. Where things actually stand

| issue | filed as | measured now | what is actually left |
|---|---|---|---|
| **0002** | aliasing → clone | 0002a done; `src/aliasing.rs` reports `observable_alias` (1 hit / 196 files) | nothing to *repair* — needs 0002b |
| **0002b** | design | storage strategy and `storage_strategy::Escape` now exist; the old unused `EscapeLatticeValue` descriptor was deleted under 0062 | remaining shared-representation steps |
| **0006** | 5 approximations | closed on soundness; `statement_suffix_*` = 0; queries are binding-keyed | 2 `HashSet<String>` overrides on `LowerCtx` (lines 259–260), load-bearing for **expression-bodied lambdas only** |
| **0007** | 58% `Raw` | `Shaped` exists, 4 families migrated. **600 `Raw` / 1038**, 12 `Shaped`, 52 round trips (39 in `coercion.rs`), 56 distinct fragments | `OptionalProjected`/`MutBorrowed`; remaining clone producers; delete the comment-matching arm; the lint |
| **0008** | machinery unreachable | steps 1–3 done. `lower.rs` **158** `fn … -> bool`, `stdlib.rs` **195**, `lattice` refs in `lower.rs` still 0 (deliberate) | step 4 — the hardcoded library predicates. The mass is in `stdlib.rs`, not `lower.rs` |
| **0009** | identity = Rust name | `JavaCallableKey`/`JavaCallableSlotKey` exist in `src/java_signature.rs` and are consumed by **exactly one file** (`inherited_members.rs`) | adopt it as *the* identity: `usage::CallableKey::Method` still carries a Rust name, and `lower.rs` still has 20 `.name ==` comparisons |
| **0010** | 3 disagreeing deciders | fixpoint exists and is primary | **three residual writers**: `exception_plan.rs:424` (`plan_method_exceptions`), `representation_lowering.rs:1528` (`\|=` over trait implementors), `call_plan.rs:1549–1567,2237` (direct `method_has_thrown_exceptions`). No `?`-site invariant |
| **0018** | metric blind | differential harness + coverage counters landed | `count_invalid_patterns` is **8 bare string literals** with nothing tying them to the emitters — one had already drifted. `idiom_gaps` still hangs, so no behavioural baseline exists |
| **0021** | pattern var unbound | **statement `if` form is fixed** (verified below) | binding is dropped in every **expression** position — verified `E0425` |
| **0022** | types from text | `parse_type_node` dispatches on `node.kind()`; `type_parameter`/`type_bound` handled | 4 text paths remain on the *source* route, incl. all formal parameters and casts; the `_ =>` arm still text-parses silently |

### 0021 — re-measured, because its status was wrong in two directions

`type_pattern` is 0 refs in `parser.rs`, which reads as "unfixed"; in fact
`Expr::InstanceOf` carries `binding: Option<String>` and the handler reads the
`name` field, so the grammar node is simply not the one tree-sitter produces.
`NEXT-BATCH.md` recorded 0021 as "already closed". Both are wrong. Verified at
HEAD:

```java
public int a(Object o) { if (o instanceof String s) { return s.length(); } return -1; }   // ✅
public int b(Object o) { if (!(o instanceof String s)) return -1; return s.length(); }    // ✅
public int c(Object o) { if (o instanceof String s && s.length() > 2) { … } }             // ✅
public int d(Object o) { return (o instanceof String s) ? s.length() : -1; }              // ❌
public boolean g(Object o) { return o instanceof String s && s.isEmpty(); }               // ❌
public int h(Object o) { boolean ok = o instanceof String s; return ok ? s.length() : 0; }// ❌
```

`d` emits `if o.is::<String>() { s.length() } else { -1 }` — note `s.length()`,
**raw Java**, which is [0019](done/0019-unhandled-expressions-emit-raw-java-source.md)
leaking through the same hole. `g` emits `o.is::<String>() && s.is_empty()`.
Both are `E0425`, confirmed by `rustc`. The rule that landed is "an `instanceof`
that *is* an `if` condition"; everything else drops the binding.

Also probed, and both correctly **loud** (no action needed this round): record
deconstruction patterns and `case X when` guards emit `compile_error!`.

> **Out of scope but worth filing.** `record Pt(int x, int y)` translates to a
> constructor that takes `x`/`y` and returns `Pat2Pt { x: 0, y: 0 }`, with
> accessors that are `unimplemented!`. The constructor is a **silent** wrong
> answer. Not one of the ten; file it.

---

## 2. Dependency order

```
0018 + 0035 (instruments) ──► everything, because every done-when below is an observation
0022 (types from tree)    ──► 0021 residual shares the parser boundary
0009 (adopt the key)      ──► 0010 closure (fallibility is keyed on a derived name today)
0007 (shape tags)         ──► 0002b lowering rewrite, 0006 residual
0002b steps 1–2 (measure) ──► the decision about whether to fund steps 3–5
```

The single most important sequencing claim: **do not start 0002b's lowering
work this round.** Steps 1–2 turn "we do not know how big it is" into a number,
cost days not weeks, and cannot regress anything. Decide with the number.

---

## Phase 0 — restore the measuring instruments (0018)

Every "done when" below is a corpus or test observation, and right now the two
instruments that would make them are broken.

### 0-a. Markers get named constants, and a test per marker

**Where.** `src/main.rs` `count_invalid_patterns` / `count_stubbed_constants`,
and each emitter that produces one of the eight literals.

**Change.** Each marker becomes a `pub const` in the module that *emits* it;
the detector imports it. Then one differential/fixture case per marker that
asserts the count rises when that construct is translated.

**Why now.** One of these had already drifted — `"unsupported Java array
creation"` vs the emitted `"Java array creation"` — and the consequence was
that [0034](done/0034-object-array-allocation-produces-an-empty-vec.md) was scored
as a clean translation on every file that hit it. A detector that can silently
stop matching is worse than no detector, because it reports success.

**Done when.** No detector contains a string literal that is not also a
constant used by an emitter, and each of the eight has a test that moves it.

### 0-b. `idiom_gaps` terminates ([0035](done/0035-idiom-gaps-suite-never-terminates.md))

684 tests, 11 of which hang. There is currently **no obtainable baseline for the
largest behavioural suite**, which is how five regressions reached the tree in
Round 4 while being reported as verified.

**Change.** Per-test watchdog: a hung test must fail with its name, not stall
the suite. Then record the baseline (`ok`/`failed` *names*, not counts) as the
gate for every step below.

**Done when.** `cargo test --test idiom_gaps` returns, and its failure-name set
is checked in as the baseline to diff against.

### 0-c. Fixtures for what this round will fix

Add to `tests/differential/known-gaps/`: `Patterns.java` (the three failing
0021 shapes above). Each moves up a level as its fix lands — that is the
regression test, per the directory's existing convention.

---

## Phase 1 — parser boundary: 0022, then 0021's residual

Both are local to `parser.rs` + the `instanceof` lowering, both are cheap, and
0022 removes the mechanism that made 0021 possible in the first place.

### 1-a. 0022: close the four remaining source-text paths

| site | today | change |
|---|---|---|
| `parser.rs:1193` formal params | `parse_type_text(type_text)` + `strip_suffix("[]")` + `contains("...")` | read `formal_parameter`'s `type` field, `dimensions` children, and `spread_parameter`; delete `varargs_type_from_param_text` |
| `parser.rs:2495` `parse_named_type_node` | `if raw.contains('<') { parse_type_text(raw) }` | read `type_arguments` children of `generic_type` / `scoped_type_identifier` |
| `parser.rs:2313` cast type | `cast_text.find(')')` then text-parse | read the `type` field of `cast_expression` |
| `parser.rs:2475` `_ =>` arm | silently text-parses **any** unrecognised kind | enumerate what actually reaches it over the corpus first, then make the residue a diagnostic rather than a guess |

**Do 2475 last and by measurement, not by hypothesis.** Instrument it, run the
corpora, and print the kinds. That list is the work; converting it blind risks
turning a working text path into a diagnostic for a node that was fine.

**Watch.** `parse_type_text` must survive — it is correct and required for
genuinely text-sourced types (`javalibs` signatures, external manifests). The
goal is that no *source* path reaches it, not that it disappears.

**Done when.** `parse_type_text` has no caller reachable from `parse_program`,
and the grammar-coverage diff in `AUDIT-BACKLOG.md` shows the type-level kinds
handled.

### 1-b. 0021: bind the pattern variable in expression positions

**Where.** the `instanceof` lowering path (`conversion_lowering::apply_instanceof`
and its callers in `lower.rs`).

**Change.** The binding is currently introduced by a *statement-shaped* rule
(`if let Some(s) = …`). Give it a value-shaped form too:

| Java | Rust |
|---|---|
| `o instanceof T s && P(s)` (value position) | `o.downcast_ref::<T>().is_some_and(\|s\| P(s))` |
| `(o instanceof T s) ? A(s) : B` | `match o.downcast_ref::<T>() { Some(s) => A(s), None => B }` |
| binding escapes the expression (`boolean ok = o instanceof T s; … s …`) | **diagnostic** — Java's definite-assignment scope has no Rust analogue here |

**Watch.** The third row is the important one. It is legal Java, it is not
expressible as an `if let`, and the current behaviour is to drop the binding
and emit raw Java. Emit `compile_error!` rather than guessing — *a translator
may refuse to translate; it must never quietly omit.*

**Done when.** `tests/differential/known-gaps/Patterns.java` moves up a level,
and the escaping-scope case produces a diagnostic rather than `E0425`.

### 1-c. 0006: close the last two overrides

**Where.** `LowerCtx.later_read_names` / `later_mutated_names`
(`src/lower.rs:259–260`), fed at `:5134`, `:10515`.

**Change.** These survive for exactly one construct: a lambda whose body is a
bare **expression** lowers into a loop with no `Stmt` to key the analysis on.
`LivenessAnalysis::for_loop_body` takes a `&Block`. Synthesize the
`Stmt::Expr(body)` **once at the lowering site**, keep it alive for the
duration of that lowering, and key on it — the same trick `for_loop_body`
already uses for block bodies. Then both fields go.

**Watch.** The Round-3 review found these overrides were being *taken over*
rather than **unioned with** the solved answer, which was a real move bug
(`xs.forEach(x -> take(v))` → three `E0382`s). Preserve the union direction
until the synthesized statement is proven to carry the same information.

**Done when.** `LowerCtx` holds no `HashSet<String>`, and
`for_each_expression_lambda_keeps_outer_value_live_across_back_edge` still
passes.

---

## Phase 2 — identity, then fallibility

These are one dependency, in this order. 0010's remaining unsoundness is
"answer *not fallible* when the declaration lookup misses", and the lookup
misses because the key is a derived name.

### 2-a. 0009: adopt `JavaCallableKey` as *the* identity

The key was built and then used in one place. That is the whole finding.

**Where.** `src/java_signature.rs` (defines), `src/inherited_members.rs` (the
lone consumer), `src/usage.rs` (`CallableKey`), `src/facts.rs` (`MethodKey`),
`src/lower.rs` (20 `.name ==` comparisons).

**Change, in this order:**

1. `usage::CallableKey::Method` carries a `JavaCallableSlotKey`, not
   `method_name: String`. This is the load-bearing one — the call graph, the
   fallibility fixpoint and `CallableAnalysis` all key through it.
2. Override/interface-implementation matching in `representation_lowering.rs`
   keys on the slot, not on the Rust name.
3. The Rust name becomes a **late, total** function of the resolved signature
   set — it already nearly is (`method_counts_including_supertypes`), but it is
   still consulted *as* identity in the 20 `lower.rs` comparisons.

**Watch.** Overload-suffix spelling is asserted **textually** in several tests,
and two of them once demanded contradictory conventions. Grep for the literal
names (`f_int`, `to_string_encoding`) before changing anything, and do not rely
on the suite until Phase 0-b lands.

**Done when.** `grep -rl JavaCallableSlotKey src/` lists the call-graph,
signature and lowering modules — not two files — and `usage::CallableKey::Method`
has no `String` method name.

### 2-b. 0010: one writer, and an invariant that proves it

Three residual writers, all now findable:

| site | what it does | change |
|---|---|---|
| `exception_plan.rs:424` `plan_method_exceptions` | re-derives from `facts.method_has_thrown_exceptions` | delete; the `fallibility: None` path exists only for `plan_program_exceptions`, whose sole remaining callers are tests |
| `representation_lowering.rs:1528` | `plan.wrap_returns_in_result \|= …` over trait implementors | this *is* override-set equalization, done a second time in a second place — move it into the fixpoint, which already equalizes |
| `call_plan.rs:1549–1567, 2237` | four direct `method_has_thrown_exceptions` probes on external paths | read `ProgramFallibilityPlan` |

Then the item the issue has been asking for since it was filed:

**2-b-4. The `?` invariant.** At every emitted `?`, assert the callee's plan
says `Result` **and** the enclosing signature is `Result`. Run it over the
corpora behind a debug assertion. Any violation is a *solver* bug with a name,
not one of 214 anonymous downstream `E0277`s.

**Watch.** `plan_method_exceptions` reads `throws` clauses. Round 1 already
folded those into `seed_callable_fallibility` — confirm that is still true
before deleting, or declared-only fallibility is lost silently.

**Done when.** `grep -c 'wrap_returns_in_result' ` shows one assignment outside
tests, and the `?` invariant holds at 0 violations across all three corpora.

---

## Phase 3 — 0002b steps 1 and 2 only: turn the unknown into a number

This is the recommendation the design document itself ends on, and it is the
right call.

### 3-a. Land `StorageStrategy` computing `Value` for everything

**Where.** new `src/storage_strategy.rs`; plan built alongside `RepresentationPlan`.

**Change.** The lattice from `0002b-shared-representation-design.md`
(`Value < SharedImmutable < SharedLocal < SharedSync`, join = max), decided
**per source type** as the join over all uses — mirroring how
`HierarchyStrategy` (`facts.rs:1415`) is already decided. Return `Value`
unconditionally. Assert the corpus output is **byte-identical**.

**Why this step exists.** It proves the wiring before it can break anything.

### 3-b. Turn on the real rule; keep lowering unchanged; report

Decision rule, verbatim from the design:

```
escapes(T) ∈ {Stored, UnknownBoundary} ∨ locally_aliased(T)
  ? (mutated(T) ? (threaded(T) ? SharedSync : SharedLocal) : SharedImmutable)
  : Value
```

All four inputs now have live homes: `storage_strategy::Escape`,
`src/aliasing.rs`, `MethodUsageFacts.mutates_self`, `CallGraph`.
Every type the plan says should be shared but that lowering emits by value is
an `observable_alias`-class diagnostic.

**This also closes 0008 step 1 for a third relation**, and it is the one that
finally makes `lattice` reachable from the lowering side — for a reason that
holds, unlike the shape-order attempt that was correctly rejected in Round 4.

**Done when.** A corpus run reports how many types land in each tier. **Stop
there.** Steps 3–5 (`SharedImmutable`, then `SharedLocal`, then `SharedSync`
after [0030a](0030-java-statics-are-thread-local.md)) are next round's decision,
taken with that number in hand.

**Watch — record it now, before anyone implements a tier.** `RefCell` converts
a Java-legal re-entrant call into a runtime **panic**. The design rule that
avoids it — *lowering never holds a borrow across a call* — must be a property
lowering guarantees, not a convention. And `Rc` cycles leak where Java's GC
collects; that is a real semantic difference no tier fixes, and it should be
stated in the output's documentation rather than discovered.

---

## Phase 4 — 0007, continued

Ordered as the issue's own update specifies; each step is independently
shippable and every query stays monotone (`tag == expected || <old text check>`),
so a producer can be tagged without auditing its consumers.

1. **`OptionalProjected` / `MutBorrowed`** — the `.as_ref()` / `.as_mut()` /
   `.as_deref()` / `.as_deref_mut()` adapters. Last big untagged family, and
   the one Tier 5 (optional carriers) will need.
2. **Tag the remaining clone producers, then delete** the
   `.clone()/* TODO(translation): validate added clone */.into()` arm of
   `rendered_expr_has_clone_recovery`. That comment is currently load-bearing
   syntax; deleting the arm is the concrete, checkable end state.
3. **Retire round trips by inversion** — each surviving `rendered.ends_with(…)`
   is a shape its producer knew. 39 of the 52 are in `coercion.rs`.
4. **Then** the anti-regression lint. Not before: added earlier it just forces
   the checks underground.

**Two checks that must stay** and are not tag candidates:
`first_rust_try_operator_index` (finds a *position*) and
`optional_erased_string_downcast_ref_view_projection` (extracts a *name*). A
tag answers "what is this"; neither is that question.

**Watch the right number.** `Raw` went 592 → 600 while four families were
migrated correctly. Tagging *wraps* `Raw`; it does not remove it, by design.
Judge by the fragment count (56) and the round-trip count (52), not by `Raw`.

---

## Phase 5 — 0008 step 4

The remaining predicate mass is not where the issue's headline metric looks.
`lower.rs` is at **158**; `stdlib.rs` is at **195**.

**Change.** Move the hardcoded library predicates (`is_java_get_class_name`,
`java_type_is_string_type`, `current_type_extends_jdk_input_stream_facade`,
`receiver_is_known_string_producing_call`, …) into `javalibs` manifests.
Mechanical, low risk, and it shrinks the surface every other issue reasons over.

**Do not** chase `grep -c lattice src/lower.rs`. Round 4 tried routing the shape
order through `crate::lattice` and correctly rejected it: the lattice ranks
encode *storage-requirement strength*, the shape order encodes *generic detail
for one erased type*. Different orders over the same values. Phase 3-b makes
the grep non-zero for a reason that actually holds.

**Done when.** `stdlib.rs` predicate count falls materially and the moved
predicates are declared data, not code.

---

## Sequencing, one line each

1. **0-a, 0-b** — instruments. Nothing below is verifiable without them.
2. **1-a** (0022), **1-b** (0021), **1-c** (0006) — parser/lowering-local,
   exact, independent of each other.
3. **2-a** (0009 adoption) → **2-b** (0010 closure). Strict order.
4. **3-a, 3-b** (0002b measurement). Stop at the number.
5. **4** (0007) — needed before 0002b's lowering rewrite, whenever that is funded.
6. **5** (0008 step 4) — parallelisable with anything; no dependencies.

## Discipline, applying to every step

- **Gate on `cargo check --tests`, not `cargo build`.** Round 3 reported a clean
  build that did not compile the test targets.
- **Diff failure *names*, not counts.** Fixtures baseline 21 passed / 20 failed;
  `idiom_gaps` baseline is meaningless until 0-b lands.
- **For every "N rules → one relation" change** (2-a, 2-b, 3-b), compute both
  answers behind an assertion for one corpus run and diff *before* deleting
  anything. This has caught a wrong subsumption hypothesis **twice** —
  `catch_future_use_names` and the `finally` override — and both times the
  hypothesis looked obviously right.
- **Verify before acting.** Instrumenting whether the suffix fallback was
  reachable took two minutes and turned a guess into a measurement. The 0021
  status in this document was wrong in both directions until it was run.

## What I would not do this round

- **Do not implement any `StorageStrategy` tier.** Steps 1–2 only. The tiers are
  a lowering rewrite whose size is currently unknown, which is precisely what
  steps 1–2 exist to answer.
- **Do not revive `impl From<Child> for Base`.** It compiles and then dispatches
  to the parent's bodies — a compile error traded for a silent wrong answer,
  which is the failure class this audit set exists to remove.
- **Do not try to delete `RustExpr::Raw`.** Making it carry a shape gets the
  benefit; removing it is a different and much larger change.
- **Do not add 0007's lint** until there is a tag for every question it would
  forbid asking textually.
- **Do not judge 0008 by `grep -c lattice`** or 0007 by the `Raw` count. Both
  metrics currently point away from the work.

---

# Round 5 results (2026-08-03)

Verification gate for every step: **all thirteen pre-existing integration suites
at their HEAD numbers**, plus `cargo check --tests` clean. HEAD baseline measured
by stash-and-compare rather than assumed. Nothing committed.

| suite | HEAD | after |
|---|---|---|
| `batch_translation` | 8 / 2 | **9 / 1** |
| `idiom_gaps` | 534 ok / 143 failed / **7 hung, never returned** | **537 / 147 / 0 hung** |
| everything else (11 suites) | — | unchanged |
| `output_markers` (new) | — | 8 / 0 |
| `differential` | 8 fixtures | 9 fixtures (Patterns added) |

`idiom_gaps` going 143 → 147 failures is not a regression: the 7 tests that used
to hang now *return*, and 6 of them fail. Against the post-fix baseline the
delta is **two fixed, zero new**.

## What landed

| phase | issue | outcome |
|---|---|---|
| 0-a | 0018 | marker text owned by `src/output_markers.rs`, counters moved into the library, per-marker fixture test. **Two more dead detectors found** |
| 0-b | 0035 | the suite terminates; baseline checked in |
| 0-c / 1-b | 0021 | **closed** — pattern bindings in `&&`, `\|\|`, ternary and `while`; regression test runs both toolchains |
| 1-a | 0022 | source route no longer reaches `parse_type_text`; **a dropped array dimension found and fixed** |
| 1-c | 0006 | **closed** — no `HashSet<String>` for the move relation on `LowerCtx` |
| 2-b | 0010 | one fallibility store; `?` invariant added, tested, measured at 0 violations |
| 3 | 0002b | steps 1–2 done. **The number is 47** |
| 4 | 0007 | the emitted comment is no longer load-bearing syntax |
| 5 | 0008 | started, and **rescoped** — see below |
| 2-a | 0009 | one live instance closed; full adoption deliberately not attempted |

## Four defects found while implementing, none of them the stated task

The plan's own discipline — measure before converting, verify before deleting —
paid four times, and in every case the bug was adjacent to the item being worked
on rather than in it:

1. **A fixpoint that could not converge.** `add_source_summary_method_surfaces`
   derived "changed" from the *intent* to add a facade method; `add_method` has
   six early returns where it does nothing. Seven tests hung forever. Fixed at
   the cause, and verified byte-identical against the old loop run under a bound.
2. **A dropped array dimension.** `int[][]` is one `array_type` node whose
   `dimensions` child carries both bracket pairs. `parse_array_type_node` wrapped
   once. The cast handler's text re-parse existed to paper over it.
3. **Identity spelled two ways.** `override_groups_for_program` keyed on the Java
   method name while the plan keys on the Rust name, so the fallibility
   fixpoint's override equalization silently did nothing and a second store
   compensated. Found by asking *why* 0010's second store was load-bearing.
4. **A binding that resolved to the wrong declaration.** At a `VarDecl`, "is this
   name written later?" resolved to the *outer* binding, so `byte[] values = …;
   Arrays.fill(values, …)` produced a non-`mut` local. The name-set union had
   been masking it.

Only the first was predicted by the plan. That is the argument for the
measure-first sequencing rather than against it: each was found by instrumenting
the thing the plan said to instrument.

## Corrections to the plan's own premises

- **0002b's "every input already exists" was too strong.** The old
  `EscapeLatticeValue` was a value-domain descriptor with no analysis behind it
  and has since been deleted; the escape input had to be written as
  `storage_strategy::Escape`. The other three inputs were reused as-is.
- **0008's step-4 list conflates library knowledge with core-language
  knowledge**, and `javalibs.rs` is Rust, not a manifest. The genuinely movable
  predicates were moved; expressing the rest as data is a design task, not a
  mechanical one.
- **0021's status was wrong in both directions**, and the case the issue called
  hardest (negated scope) already worked, while a case it did not list (ternary)
  emitted raw Java.

## What is left, precisely

- **0009** — `usage::CallableKey::Method` still carries a Rust name.
  `CallableKey::source_method` is the seam to migrate behind, one caller at a
  time. 20 `.name ==` comparisons remain in `lower.rs`.
- **0010** — four direct `method_has_thrown_exceptions` probes in `call_plan.rs`
  on the external-call path; `plan_call_exception_propagation` does not receive
  the solved plan.
- **0002b** — steps 3–5. Population: 14 `SharedImmutable`, 34 `SharedLocal`,
  0 `SharedSync`. Do `SharedImmutable` first.
- **0007** — the `OptionalProjected` / `MutBorrowed` family, then the lint.
- **0008** — decide what the external-pack manifest grammar must gain before
  budgeting the rest of step 4.
- **New, found this round**: `record Pt(int x, int y)` emits a constructor that
  takes its arguments and returns `Pt { x: 0, y: 0 }`, discarding them. A silent
  wrong answer; deserves its own issue.

## One more correction, found in final verification

Deleting the comment-matching arms from `rendered_expr_has_clone_recovery`
broke two `--lib` idempotence tests that construct an *untagged* `RustExpr::Raw`
already carrying the clone comment. The integration suites did not catch it,
because in the real pipeline every such expression is tagged.

Resolved by distinguishing two things the original arms conflated: **tolerating a
comment** and **matching its wording**. `strip_trailing_block_comments` removes
`/* … */` structurally before the `ends_with` checks, so an annotated clone is
still recognised whatever the annotation says, and no particular text is
load-bearing. `cargo test --lib` is back to its one documented pre-existing
failure (`coercion::tests::jdk_collection_static_projections_own_expression_shapes`).

Worth stating because it is the general shape of this whole round: the fix is
almost never "restore the string check" or "delete the string check", it is
"work out which property the string check was standing in for".

## Follow-on pass: the two residuals closed

| item | outcome |
|---|---|
| **0010 residual** | `plan_call_exception_propagation` split so lowering reaches an external-only entry point with no source-method probe. Measured 0 probe hits over 100 files *before* splitting, then made unreachability structural. `lower.rs` no longer names the probing function; a test asserts it. **0010 closed.** |
| **0009 interface slots** | Trait-slot candidates ordered by `JavaCallableSlotKey` so the exact Java signature wins. An *ordering* change, not a filter — narrowing the set would lose implementations that spell a parameter type differently. |

Final `idiom_gaps`: **538 passed / 146 failed / 0 hung** — one further test fixed
(`input_stream_read_byte_array_uses_three_arg_overload_with_full_buffer`, an
overload-selection case), zero new failures.

### Two more defects found while testing the 0009 change

1. **A nested type name reached `as &dyn` unmangled** (`&dyn Outer.Inner`, not
   valid Rust). 0013 had fixed this for `downcast::<T>` targets only, so the same
   name failed elsewhere — a fix applied at one symptom rather than at the name.
   Now `normalize_emitted_java_type_name`, covering every `dyn` position. Fixed.
2. **An interface method satisfied by an inherited method emits no `impl` at
   all.** Filed as `tests/differential/known-gaps/InterfaceOverloads.java` with a
   diagnosis.

That makes **six** defects found by implementing this plan that the plan did not
predict, every one adjacent to the item being worked on rather than in it.

## Deliberately still not done, and why

- **0002b steps 3–5.** The plan's own "What I would not do this round" says:
  *"Do not implement any `StorageStrategy` tier. Steps 1–2 only."* The number is
  now in hand (14 `SharedImmutable`, 34 `SharedLocal`, 0 `SharedSync`), which is
  what steps 1–2 existed to produce. Implementing a tier is a lowering rewrite
  that changes the representation of 47 types, and the design requires the
  *never hold a borrow across a call* rule to be a guarantee before any of it
  lands. That is the next round's work, decided with this number — doing it here
  would be overriding the sequencing this plan was written to enforce.
- **0009 full adoption.** `usage::CallableKey::Method` still carries a Rust name.
  Both live instances found this round are fixed and the seam
  (`CallableKey::source_method`) exists, but converting the key type touches the
  call graph, the fallibility fixpoint, signature planning and 20 `lower.rs`
  comparisons at once — the unattributable-change shape both 0009 and 0008 warn
  against.


---

# Review fixes (2026-08-03)

Three findings from review, all correct.

## 1. Audit bookkeeping was inconsistent

I reported 0010 as "closed and moved to `done/`". It was closed; it was **not**
moved. Moved now, with links repointed and verified (0 dangling links across
`claude_audits/`).

Worse, I had not touched `docs/claude-audit-refactoring-plan.md` at all — the
*tracked* status table, and the one a reader would check first. It still marked
0006, 0010, 0021 and 0022 partial. Updated, along with 0002, 0007, 0008 and
0018, and rows added for 0033/0034/0035 which were missing entirely.

The table and the directory are now checked against each other: every row marked
`done`/`fixed locally`/`done (scoped)` has its file under `claude_audits/done/`,
and no row still marked open does. That check runs to **0 inconsistencies**, and
is the thing I should have been running rather than reporting from memory.

## 2. `cargo fmt --check` failed

Fixed by running `cargo fmt`; the check is now clean.

Worth splitting the cause honestly. Measured at HEAD by stash-and-compare, **7
files were already unformatted before this round**: `src/aliasing.rs`,
`src/coercion.rs`, `src/conversion_plan.rs`, `src/item_assembly.rs`,
`src/rust_ir.rs`, `tests/differential.rs`, `tests/idiom_gaps.rs`. So the gate was
already red. I had formatted only the files this round *created*, on the
reasoning that reformatting untouched files adds unrelated diff — wrong call
when formatting is a merge gate. Five of those seven are files this round did
not otherwise modify, so their hunks are pre-existing drift rather than a
consequence of this work.

## 3. The 0022 claim was overstated

"Source route no longer reaches `parse_type_text`" is stronger than what was
measured. Four defensive fallbacks remain in `parser.rs`; what was measured is
that they fire **0 times** over the corpus and the type-shape probes, and the
audit file's own wording ("closed for the source route") was already the right
scoped claim. The status tables now carry that nuance instead of the flat claim:
`done (scoped)` in the plan, and the README says closed for the source route
*not for the file*, naming the surviving fallbacks.

## Verification after the fixes

`cargo fmt --check` clean, `cargo check --tests` clean, `cargo test --lib` at its
one pre-existing failure, all thirteen integration suites at their HEAD numbers
(`batch_translation` still +1), `output_markers` 8/0, `differential` green,
`idiom_gaps` matching the recorded baseline exactly.
