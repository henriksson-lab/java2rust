# Overnight session log

Work done against the goal *"finish all the issues"*. **Nothing committed** —
everything below is in the working tree, as instructed.

Audits closed: **0025, 0026, 0030, 0031, 0037, 0038, 0039**.
Advanced: **0009, 0018, 0028, 0002b**.
Active: **7** (0002, 0002b, 0007, 0008, 0018, 0023, 0028) — of which 0002,
0002b, 0023 and 0028 are now understood to be **one interlocked change**, see
the last section.

`FROMCLAUDE.md` is the reviewer-facing write-up of the same batch. This file is
the session log, and it additionally records the **unfinished** 0002b work at
the end, which `FROMCLAUDE.md` does not cover.

---

## Verification status

The latest full `scripts/gate.sh --all` was **green — all 17 targets plus
`--lib`, zero deltas** against `tests/baselines/suite-results.tsv`.
`cargo fmt`, `cargo check -q`, and `git diff --check` are clean. A follow-up
audit pass fixed additional dirty-change issues and moved the current baselines
for `output_markers`, `slot_shape_design`, `type_coercion`, and `--lib`.

That run covered everything in the "Completed" section below. It does **not**
cover the 0002b change in "In flight" — see that section.

| target | baseline | note |
|---|---|---|
| `differential` | 1/0 | **19** fixtures now (+`NestedStatic`, +`EvalOrder`, +`EvalOrderSiblings`, +`ObjectArray2D`, +`StaticArray`) |
| `idiom_gaps` | 538/146 | unchanged; regressed twice during the session and restored both times |
| `output_markers` | 16/0 | was 9; includes no-op facade constructor call-site counter |
| `slot_shape_design` | 514/80 | was 499 |
| `idiom_sweep_script` | 46/0 | was 45 |
| `type_coercion` | 109/0 | current shaped-expression expectations |
| `--lib` | 152/0 | was 106; the pre-existing `coercion` failure is fixed |
| everything else | unchanged | |

Every behavioural claim is checked against the Java oracle: `javac`+`java` vs
translate+`rustc`+run.

---

## Completed and gated

### 0037 — nested class reads an enclosing `static` (closed)

Two defects stacked; the second was only visible once the first was gone.

1. The capture classification ignored `static`, so a `static` nested class
   reading a `static` field lowered to `self.__outer.borrow().n` — inside a
   method with no `self`.
2. With the capture gone the read resolved through the shared shape environment
   (which carries every type's statics under their bare names, since Java has
   them in scope) while the accessor projection hardcoded `Self::`, emitting
   `OuterIn::__static_n()` for a field `Outer` owns (`E0599`).

The *write* side was already correct — assignment planning carries a
`rust_owner`, the bare-name read path had none. `ShapeEnv` now records the
declaring type beside each static-storage name.

Files: `src/resolve.rs`, `src/expr_shape.rs`, `src/access_lowering.rs`,
`src/lower.rs`. Fixture: `tests/differential/NestedStatic.java` (`1,1,4,9,2`).

### 0030 — Java statics were `thread_local!` (closed)

Now `std::sync::Mutex<Option<T>>` with an `AtomicBool` init guard; a poisoned
lock is recovered rather than propagated.

The defect is **invisible to single-threaded behaviour**, so there are two tests
on purpose: `tests/differential/StaticSharing.java` pins behaviour against the
oracle, and `static_storage_is_shared_across_threads_not_per_thread` pins the
storage *identity* — the weaker check the audit itself proposes while the
harness has no threaded fixture, and the one that would catch a revert.

### 0031 — stubbed constant census (closed)

Group 3 **did not reproduce**: all five constants have hand-written
`modeled_semantic_field_typed_value` entries, which fixes five constants and
does not scale. One census row was a false positive —
`SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH` is declared `= 0` in Java.

Built the general mechanism instead: `src/source_constant_values.rs` folds a
`static final` initialiser as a JLS 15.28 constant expression (Java widths and
overflow, logical `>>>`, Java narrowing casts, constant string concatenation)
with a program-wide fixpoint for cross-file references
(`VariantContext.NO_LOG10_PERROR = CommonInfo.NO_LOG10_PERROR`). Anything not
provably constant folds to nothing and the loud `compile_error!` stands.

**Corpus reach**, folding every `static final` declaration in five real corpora:

| corpus | files | `static final` | with initialiser | folded |
|---|---:|---:|---:|---:|
| htsjdk | 801 | 1563 | 1235 | **730** |
| GATK | 2500 | 7679 | 6944 | **4175** |
| picard | 737 | 1859 | 1677 | **805** |
| FastQC | 156 | 500 | 489 | **131** |
| biojava | 53 | 133 | 97 | **87** |
| **total** | **4247** | **11734** | **10442** | **5928** |

**Honest negative:** re-running `htsjdk_vcf` before and after is byte-identical
(same `stubbed_constants` = 4, same `cargo_check_errors` = 59). Every group-3
constant that corpus reaches already has a pack entry. The change is a
generalisation; the reach table is its justification, not the corpus diff.

### 0026 — assignment evaluation order (both residuals fixed; still `partial`)

1. **The value can invalidate a stable-looking index.** `b[m] = m++` writes
   `b[1]` under Rust's order and `b[0]` under Java's. The only symptom is *which
   element* was written, so the obvious probe passes — the fixture reads the
   whole array. My first gate over-fired on every `a[i] = f()` and cost three
   `idiom_gaps` tests; the precise rule is that a Java method cannot reach a
   caller's local, which needs the shape environment to apply since a bare field
   reference and a local are the same `Expr::Var`.
2. **A field assignment place has target subexpressions too.**
   `arr[j++].field = j` is not an array assignment, so no ordered-array path saw
   it. The new path hoists the index and re-lowers the place with the temporary
   substituted, reusing the ordinary field projection.

Fixture: `tests/differential/EvalOrder.java` (`1,0:0:0,15,1,1`) plus three
focused tests including the negative case. Still open and written into the
audit: ragged multi-level receivers, array-store exception ordering, and
`pick(e).field = v` (blocked on 0002).

### 0018 — the metric that cannot see silent failures (advanced; still `partial`)

- **`noop_facade_methods` was 97% a different phenomenon.** On a
  `StringBuilder`-only program it reported 80, of which 78 were synthesised
  `From` impls for unmodelled Java upcasts. Split out as
  `unmodelled_upcast_conversions`; the counter drops 80 → 2 there and 145 → 130
  on htsjdk, where the 15 removed were struct-field initialisers and local
  defaults matched because the old regex looked for `Default::default()`
  *anywhere*.
- **Two implementations of one metric across a language boundary.**
  `scripts/sweep-generated-idioms.py` and `output_markers` disagreed (145 vs
  130). A shared constant cannot fix that, so the agreement is asserted:
  `the_python_sweep_and_the_rust_counters_agree_on_shared_signals` runs the real
  sweep script over a translation and requires the numbers to match.
- **`noop_facade_call_sites` — constructor subset built.** The generated-text
  objection still applies to ordinary no-op facade methods (`to_string`/`clone`/
  `into` collide with unrelated calls), but facade constructors use qualified
  paths like `Foo::new_2(...)`. That subset is now counted and tested; the
  remaining method-call subset still needs a lowering-time diagnostic.

### 0009 — overload identity (minimized identity defect fixed; facade completeness remains)

The same overloaded method used to get a different Rust name depending on which
lowering path reached it:

```rust
Util::pick_int(1), Util::pick_string("ab"),   // static path:   suffixed, correct
u.inst(1),         u.inst("ab"),              // instance path: both collapse
```

The instance naming half is fixed: summary declarations now get Rust overload
names before lowering uses them. The facade half was narrowed too: a summary
type's source is not an unmodelled import, so the wrong generic imported-static
stub is withheld while the type itself is still emitted. The remaining work is
facade completeness — emitting a summary type's real static/constructor surface
instead of merely withholding the wrong answer.

### The gate itself was reporting wrong numbers

Worth reading even if the rest is skipped. `idiom_gaps` keeps *failing*
generated crates in `/tmp` for inspection, but kept them **whole**, `target/`
included. At ~146 failures a run with no bound on runs, the leftovers reached
**10,525 directories / 31 GB** and filled the root filesystem.

1. A gate run came back `537/147` against a `538/146` baseline — one test, the
   exact size of delta this audit set treats as a real regression. I nearly
   attributed it to the change under test.
2. A direct re-run of the same suite on the same tree reported **280 passed /
   404 failed**. Nothing had changed but free space.

Fixed by keeping what is inspected and dropping what is regenerable:
`Cargo.toml` and `src/` stay, `target/` goes, so a failing crate still
reproduces by running `cargo` in its directory. **~450 MB → 1.9 MB per run**,
measured, suite still at `538/146`. The 31 GB of accumulated leftovers was
deleted (only `/tmp/new_java2rust_*`; nothing else in `/tmp` was touched); `/`
went from 0 bytes free to 31 GB.

### Audit bookkeeping is now enforced by tests

Three hand-maintained records of one fact (the plan table, `claude_audits/README.md`,
and `claude_audits/done/`) have drifted before. Three tests in
`tests/output_markers.rs` now check them:
`tracked_plan_status_agrees_with_the_audit_directory`,
`every_audit_has_a_tracked_plan_row`, `audit_readme_links_resolve`.

The second **failed on first run**: 0002b had no plan row, despite three other
audits being blocked on it. Added.

---

## In flight — gated after review, measurement still needs reconciliation

**0002b step 2, the `UnknownBoundary` escape input.** `src/storage_strategy.rs`
is modified. The overnight session had not gated this change; reviewer follow-up
ran `scripts/gate.sh --all` on 2026-08-05 and it stayed at baseline, including
`idiom_gaps 538/146`.

The design document says of the earlier pass:

> "Every input already exists" — that was too strong. `UnknownBoundary` is in
> the domain and is **not yet produced** … the escape input is the one that will
> move the 47 up or down, and it is currently biased **low**.

Under-sharing is a silent wrong answer, so producing that input matters for the
correctness of step 3. The evidence already existed: `usage` classifies every
call argument as `InternalSameType` / `KnownContentRead` / `Unknown`, and a
binding carries its declared Java type. The change reads that and joins
`Escape::UnknownBoundary` for a handle passed to an unknown-boundary call.
`ReceiverForCall` is deliberately excluded — passing an argument gives the
callee a handle it can store; being the receiver does not.

This is a **report-only** path (`report_storage_disagreements`); lowering does
not consult it, so it should not change generated output. The gate confirms it
is output-inert, but the measurement below still has to be reconciled before it
is written into the audit as a settled number.

First measurement, over `examples/` + `claude_audits/repro/` +
`tests/differential/`:

| tier | documented (248 files) | measured now (255 files) |
|---|---:|---:|
| `SharedImmutable` | 14 | **17** |
| `SharedLocal` | 34 | 34 |
| `SharedSync` | 0 | 0 |
| distinct shared types | **47** | **45** |

**I did not reconcile this and it should not be written into the audit as-is.**
`SharedImmutable` rising by 3 while the distinct total *falls* from 47 to 45
needs explaining, and the two numbers are not over the same input — the corpus
grew from 248 to 255 files, partly from fixtures added this session. Possible
causes, unverified: the per-file `plan_storage_strategies` call in my probe
differs from how `lib.rs` invokes it; or the dedup by `tier:name` across files
collapses same-named types that the original count treated separately.

### To resume

1. Reconcile 47 → 45 by re-measuring over the *original* 248-file input, or by
   re-deriving the documented figure with the current code and `UnknownBoundary`
   disabled. Only then write a number into
   `claude_audits/0002b-shared-representation-design.md`.
2. If it does not reconcile cleanly, revert `src/storage_strategy.rs` — it is
   the only file carrying this change, and the rest of the batch is independent
   of it.

---

## Where the remaining nine sit

- **0002 / 0002b** — the shared representation. Steps 1–2 done (plan computed,
  disagreements reported, lowering untouched). Step 3 (`SharedImmutable`, no
  `borrow()`, no panic risk) is the next real work and is a lowering rewrite.
- **0007, 0008, 0023, 0028** — all blocked on 0002b. 0028 in particular is not
  "also fixed" by it; reference identity is *unimplementable* on values and free
  on handles.
- **0009** — the largest remaining source of silent wrongness that is **not**
  blocked on the representation. Failing reproducer is recorded. Best next
  target.
- **0018, 0026** — advanced this session; each has a named, deliberately
  deferred residual written into its audit.


---

# Second half — resumed after the four review items

All four review items done, then work continued. Gate green and baseline
recorded after every step; `cargo fmt --check` clean; **nothing committed**.

## The four review items

1. **Storage numbers reconciled.** The figures were in different units, and the
   audit's own table showed it: `14 + 34 = 48` against a stated total of `47`.
   A type can appear at **two tiers**, because the plan is per-program and each
   fixture is its own program — so `47 = 14 + 34 − 1`. Measured today with only
   the new escape input toggled (same tree, same corpus): `UnknownBoundary` is
   worth **+1 distinct type** (`SharedImmutable` 12 → 13). Smaller than the
   design implied, and said so. The 47 → 43 gap is corpus and translator drift,
   explicitly not claimed as an effect of the change.

2. **The simple-name collision was real, and silent.** `pkg1.Uses.Q =
   Constants.X + 1` folded to **23** using `pkg2`'s `X = 22` instead of 12 —
   a confidently wrong constant, which is what 0031 exists to prevent.
   `ConstantScope` is now a type with an explicit resolution order: qualified
   path wins, then simple name **within the referring package** as Java does,
   then unambiguous, otherwise refuse. Six unit tests including the ambiguous
   case that must fold to nothing.

3. **0009 fixed — and the audit's framing was misleading.** It pointed at a
   large `JavaCallableSlotKey` adoption; the defect was a **missing call**.
   `assign_type_rust_names` was already correct but never ran on context or
   summary programs, so every `MethodDecl` from one had `rust_name: None` and
   both overloads collapsed onto `u.inst(...)`. The static path escaped only
   because it re-solves signatures on an isolated program. Zero corpus movement.

4. **0026 given checkable exit criteria, then closed** — see below.

## Then, working down the remaining issues

### 0038 (new, found and closed) — `String[][]` had three carriers

Found while fixing 0026's ragged residual. `String[][] g = new String[2][2]`
was declared `Vec<Vec<String>>`, built by `ndarray::Array2::from_elem`, written
with nested indexing and read as `g[[i, j]]`. Four sites, three carriers,
rejected by `rustc`. The two Java allocation forms of the same declared type
also disagreed.

The policy already existed and was already right —
`types::ndarray_array2_element_type` requires a **primitive** element, and the
write path asked it. Construction never did, and the read asked a *different
question* (`two_dimensional_element_type_after_optional`, true for any element).
Both now ask the same predicate. Fixture `ObjectArray2D.java` (`x,y,z,v2,2`).

### 0039 (new, found and closed) — static arrays lost their value *and* their writes

Found while writing 0038's fixture, which had used a `static int[]` as a
counter. Two defects on one construct:

- `static int[] counter = new int[3]` had **no arm** in the static-storage
  initial-value lowering, so it fell back to `Vec::new()` — length 0, first
  index panics.
- Worse and silent: the accessor returns `Vec<i32>` **by value**, so
  `Self::__static_counter()[0] = v` assigns into a temporary and is
  **discarded**. Rust accepts it; nothing warns; the array never changes.

So the write half had to land first, or fixing the initializer would have turned
a panic into a quiet wrong answer. Operands are bound *before* the guard is
taken — not tidiness: `counter[0] = counter[0] + 1` re-reads the same static and
a `Mutex` is not reentrant, so the other order deadlocks. Fixture
`StaticArray.java` (`2,3,8,6,2,150`) uses two writes, and a write whose index
and value both read the static being written.

Residual, stated: a **non-constant** static array initializer still falls back.

### 0026 — closed

All three criteria met: ragged multi-level fixed and now pinned against the Java
oracle (0038 unblocked it); array-store ordering has no work to do until
`ArrayStoreException` exists at all; `pick(e).field = v` explicitly reassigned to
0002. The sibling orders the audit asks be checked — argument order, binary
operand order, `&&`/`||` short-circuiting — are correct and now **pinned** by
`EvalOrderSiblings.java` rather than asserted, with short-circuiting checked
through the side effects the *unevaluated* operand would have had.

### 0028 / 0002b — a correction to the plan, and the `.equals()` half

The most useful result of the second half, and it is a *negative*.

0002b recommends implementing `SharedImmutable` (`Rc<T>`) first because it has
no `borrow()` and no panic risk. Good tactics, wrong plan: that tier has **no
observable behaviour of its own**. A type never mutated after aliasing behaves
identically whether copied or shared — that is what immutable means. Measured,
not argued:

| | Java | translation |
|---|---|---|
| read through the alias | `5` | `5` ✓ |
| `a == b` (same object) | `true` | `true` ✓ |
| `a == c` (distinct) | `false` | **`true`** |
| `a.equals(c)` (distinct) | `false` | **`true`** |

Everything that is not identity already agrees. So a differential gate cannot
tell step 3 from a no-op — both report "nothing changed". The dependency is
mutual: **0028 is unimplementable without the representation, and 0002b step 3
is untestable without 0028.** They are one change, and
`known-gaps/Identity.java` is its acceptance test — 2 of 4 cases failing today.

The table also exposed 0028's "other half", which that audit names and does not
design: Java's `Object.equals` **is** identity unless overridden, so
`a.equals(c)` answers `true` where Java says `false` — the damaging direction,
since a `HashMap` or `contains` built on it **merges entries Java keeps apart**.
The interim measure 0028 asks for is now applied to it:
`java_inherited_equals_identity` reports it, following `extends` so an inherited
override is not flagged, with an unknown supertype assumed to override. Three
tests: positive, overridden negative, inherited-override negative.

## Where the remaining seven sit

- **0002, 0002b, 0023, 0028** — one interlocked change: the shared
  representation plus identity lowering. Now has an acceptance test and a
  corrected sequencing. This is the next real work and it is large.
- **0007** — the optional-projection family first exposed why a coarse
  `RustExprShape` tag was not enough: those arms ask *which of five adapters was
  applied*, and `true` means "skip the adapter". The prerequisite is now built:
  `RustExpr::Shaped` can carry the precise produced `RustSlotShape`, and this
  family uses it before falling back to text for untagged producers.
- **0008** — structural refactor, not blocked on the above, not started.
- **0018** — constructor no-op facade call sites are now counted from generated
  text because `Foo::new_2(...)` is a qualified path. Ordinary no-op method call
  sites remain uncounted and still need a lowering-time diagnostic.


---

# Third round — the representation change, attempted

Gate green throughout, baseline recorded, `fmt` clean, **nothing committed**.

## Reviewed Codex's three changes — all correct

`scope.record(...)` in `owner_constant_scope` (qualified sibling constants),
the fixture extension, and the `lib.rs` comment/call reunion. Verified against
the Java oracle (`11,5,3,7,2,2`) and the gate. The `record` change also fixes
qualified constants as array *elements*, which I checked separately.

**One residual it did not reach**, now fixed: a constant owned by a *different*
type. `new int[Sizes.M]` still fell back to `Vec::new()` — and that fallback is
length **0**, so the first index panics. Not a missed optimisation. The fold now
starts from the whole program's declarations via
`resolve::fold_constant_declarations`, the same fixpoint the summary-source path
uses. Fixture extended to `11,5,3,7,2,2,4,4`.

## 0040 (new, found and fixed) — the translator was 6× slower than it needed to be

Investigating "why do three-line tests take 70–100 s" found
`external_type_name_matches` rebuilding its alias list — `to_snake_case`,
`canonical_external_receiver_type_key`, several allocations — **once per
external-pack member**, inside a linear scan. `perf`: 38% of samples there.

It is a pure function of its argument, so memoizing is sound:

| | before | after |
|---|---:|---:|
| 7 methods + summary source | 78.7 s | **12.4 s** |
| one test | 85.6 s | **12.8 s** |
| `slot_shape_design` (592 tests) | 58–83 s | **24.5 s** |
| `differential` (20 fixtures) | 116 s | **39 s** |

A `ContextSignatureCache` for the per-call-site signature solve was built first
and **reverted**: 11.15 s with, 10.52 s without. The negative is recorded so the
next person skips that path.

## 0002b — two wrong inputs, and the estimate halves

Attempting step 3 found two over-approximations in the plan itself, both firing
on very common shapes:

- `mutated` counted **constructor** field writes, though the bit asks whether a
  value is mutated *after sharing* and construction precedes any alias. So
  `final`-field value classes — exactly the population the design names for
  `SharedImmutable` — were landing in the harder `RefCell` tier.
- `escape` charged the **owner** for `this.v = v` "for want of a type
  environment", but the field's declared type is right there. Every class whose
  constructor assigns a parameter to a field was marked `Stored`, including when
  `v` is an `int`.

| tier | documented | now |
|---|---:|---:|
| `SharedImmutable` (`Rc<T>`) | 14 | **19** |
| `SharedLocal` (`Rc<RefCell<T>>`) | 34 | **4** |
| **distinct needing sharing** | **47** | **22** |

The tier carrying the `RefCell` hazard is **four types**, not thirty-four.

## 0002b step 3 — attempted, measured, reverted

Wrapping a `SharedImmutable` type in `Rc<T>` at the representation-aware
rendering entry point produced **zero deltas** — not "no regressions", *no
effect*. That is the finding: **the Rust type of a source class is never
rendered through a representation-aware path.**

Counted: **49** call sites use the facts-only rendering variant against **11**
that have the plan; `item_assembly.rs` — struct fields, parameters, return
types — mentions `representations` **zero** times.

And the switch is **atomic**: render a field as `Rc<Key>` while a parameter is
still `Key` and the output does not compile. Unlike every other migration in
this audit set there is no monotone half-done state.

So the next unit of work is threading the plan through all 49 sites *without
consulting it* — provably inert, gate-verifiable — and only then flipping the
switch for the 19 types together with 0028's identity lowering. The inert hook
was removed rather than left in place: it would read as "started" when nothing
reaches it.

## Honest status against "fix all issues"

Seven audits remain open. None is fixable by a small change, and each now has a
measured next step rather than a description:

| | what remains | next unit of work |
|---|---|---|
| 0002, 0002b, 0023, 0028 | one interlocked change | read the remaining handle-adaptation failures one by one; the simple wrap/unwrap rule was measured wrong |
| 0007 | residual raw-string expression checks | continue migrating only checks whose tag means "already correct"; optional projection now carries precise slot shape |
| 0008 | step 4 | a design task: decide what the external-pack manifest grammar must gain |
| 0018 | no-op facade method call sites | constructor call-site counting is built; ordinary method call sites still need a lowering-time diagnostic |

What changed this session is that each of those is now a number and a named
obstacle instead of a direction.
