# Latest batch — superseded by `OVERNIGHT.md`

Current correction: a follow-up audit pass advanced the verified gate baseline to
`output_markers 16/0`, `slot_shape_design 514/80`, `type_coercion 109/0`, and
`--lib 156/0`; `idiom_gaps` remains `538/146`. See `OVERNIGHT.md` and
`tests/baselines/suite-results.tsv` for the current counts. The older table
below is preserved as historical handoff context.

Current status after the resumed second half: **0025, 0026, 0030, 0031, 0037,
0038, 0039 closed**; **0009, 0018, 0028, 0002b advanced**; active audits are
**0002, 0002b, 0007, 0008, 0018, 0023, 0028**. The detailed current record is
in `OVERNIGHT.md`.

---

# Earlier batch — 0025, 0030, 0037, 0031 closed; 0026 and 0018 advanced; 0009 reproduced

**Nothing committed.** Everything below is in the working tree.

Active audits: **9** (0002, 0002b, 0007, 0008, 0009, 0018, 0023, 0026, 0028).
Closed this pass: **0025, 0030, 0037, 0031**.

## Verification

`scripts/gate.sh --all` — all 17 targets plus `--lib`, **zero deltas** against
`tests/baselines/suite-results.tsv`:

| check | result |
|---|---|
| `cargo fmt --check` | clean |
| `differential` | green, **19** fixtures; current baseline is in `OVERNIGHT.md` |
| `idiom_gaps` | 538 / 146 — **unchanged**, and it was regressed twice on the way |
| `output_markers` | 13 / 0 (was 9) |
| `slot_shape_design` | 512 / 81; current baseline is in `OVERNIGHT.md` |
| `idiom_sweep_script` | 46 / 0 (was 45) |
| `--lib` | 121 / 1; current baseline is in `OVERNIGHT.md` |
| plan row ↔ README ↔ `done/` | now **checked by a test**, see below |

Every behavioural claim is against the Java oracle: `javac`+`java` vs
translate+`rustc`+run.

---

## 0037 — nested class reads an enclosing `static` (**fixed**, moved to `done/`)

Two defects stacked, and the second only became visible once the first was gone.

1. **The capture classification ignored `static`.** A `static` nested class
   reading a `static` field lowered to `self.__outer.borrow().n` — inside a
   method with no `self`. `analyze_inner_type` now runs only for non-static
   nested types, and `outer_fields` filters out statics.
2. **The read then named the wrong type.** With the capture gone it resolved
   through the shared shape environment — which carries every type's statics
   under their bare names, since Java has them in scope — while the accessor
   projection hardcoded `Self::`. From inside `In` that is
   `OuterIn::__static_n()` for a field `Outer` owns (`E0599`).

The **write** side was already right, which is why only the read failed:
assignment planning carries a `rust_owner` and the bare-name read path had none.
`ShapeEnv` now records the declaring type beside each static-storage name.
Same-type reads still emit `Self::`.

`tests/differential/NestedStatic.java` (`1,1,4,9,2`) covers both directions, a
nested static sharing an enclosing one's simple name, and a second nested class
so the owner cannot be guessed from arity.

## 0030 — Java statics were `thread_local!` (**fixed**, moved to `done/`)

Storage is now `std::sync::Mutex<Option<T>>` with an `AtomicBool` init guard; a
poisoned lock is recovered rather than propagated, so a panicking initialiser
cannot make the field permanently unreadable.

Worth stating: **this defect is invisible to behaviour on one thread.** So there
are two tests on purpose — `StaticSharing.java` pins the observable behaviour
against the oracle, and `static_storage_is_shared_across_threads_not_per_thread`
pins the storage *identity* (no `thread_local!`, a `Mutex`, an `AtomicBool`
guard). That second one is the weaker check the audit itself proposes while the
harness has no threaded fixture, and it is the one that would catch a revert.

`Send`/`Sync` propagation stays with 0002; it is a constraint on that design, not
a residue here.

## 0031 — stubbed constants (**closed**, moved to `done/`)

Group 3 **did not reproduce**, and why matters: each of its five constants has a
hand-written `modeled_semantic_field_typed_value` entry. That fixes five
constants and does not scale. One census row was also a false positive —
`SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH` is declared `= 0` in Java, so zero
is the right answer and the census `awk` cannot tell a stub from a real zero.

The general fix is `src/source_constant_values.rs`: a `static final` field in an
available context-summary source now contributes its **declared value**, folded
as a JLS 15.28 constant expression — Java's widths and overflow, logical `>>>`,
Java narrowing casts, constant string concatenation, and a program-wide fixpoint
for cross-file references (`VariantContext.NO_LOG10_PERROR =
CommonInfo.NO_LOG10_PERROR`). Anything not provably constant folds to nothing and
the loud `compile_error!` stands.

**Corpus reach**, folding every `static final` declaration in five real corpora:

| corpus | files | `static final` | with initialiser | folded |
|---|---:|---:|---:|---:|
| htsjdk | 801 | 1563 | 1235 | **730** |
| GATK | 2500 | 7679 | 6944 | **4175** |
| picard | 737 | 1859 | 1677 | **805** |
| FastQC | 156 | 500 | 489 | **131** |
| biojava | 53 | 133 | 97 | **87** |
| **total** | **4247** | **11734** | **10442** | **5928** |

**Honest negative:** re-running `htsjdk_vcf` before and after is byte-identical —
same `stubbed_constants` (4), same `cargo_check_errors` (59), no `pub const`
differs. Every group-3 constant that corpus reaches already has a pack entry, and
the four still unmodelled are not constant expressions at all. The change is a
generalisation; the reach table is its justification, not the corpus diff.

## 0026 — assignment evaluation order (**both residuals fixed**, still `partial`)

**1. The value can invalidate a stable-looking index.**

```java
int[] b = new int[3]; int m = 0;
b[m] = m++;      // Java writes b[0]; Rust's order writes b[1]
```

The only symptom is *which element* was written — `b[0]` reads `0` either way,
so the obvious probe passes. The fixture reads the whole array.

The first attempt at the gate over-fired on every `a[i] = f()` and cost three
`idiom_gaps` tests. The precise rule is that **a Java method cannot reach a
caller's local**: a local index is disturbed only by an explicit write to that
same name, while a field or array-element index keeps the conservative rule.
Telling those apart needs the shape environment, because a bare field reference
and a local are the same `Expr::Var`.

**2. A field assignment place has target subexpressions too.**
`arr[j++].field = j` is not an array assignment, so no ordered-array path saw it.
The new path hoists the index and re-lowers the place with the temporary
substituted, reusing the ordinary field projection rather than hand-building a
second one.

`tests/differential/EvalOrder.java` (`1,0:0:0,15,1,1`) plus three focused tests
including the negative (a literal index must *not* be hoisted).

Still open and written into the audit: ragged multi-level receivers, array-store
exception ordering, and `pick(e).field = v` which is blocked on 0002.

## 0018 — the metric that cannot see silent failures (**advanced**, still `partial`)

Two findings, both instances of this audit's own thesis applied to its own
remedies.

**`noop_facade_methods` was 97% a different phenomenon.** On a program whose
only Java API use is `StringBuilder` it reported 80 — of which **78 were
synthesised `From` impls for unmodelled Java upcasts**. Those are erasure and
worth counting, but they are a different thing with a different fix and they
drowned the signal 39:1. Now `unmodelled_upcast_conversions`, counted separately
and reported in `summary.tsv`. The counter drops 80 → 2 there, and 145 → 130 on
htsjdk, where the 15 removed were struct-field initialisers and local defaults
the old regex matched because it looked for `Default::default()` *anywhere*.

Also recorded, not fixed: the upcast surface is emitted for **every**
translation regardless of what the program uses, so it is a constant offset, not
a per-file cost. A test asserts exactly that.

**Two implementations of one metric, across a language boundary.**
`scripts/sweep-generated-idioms.py` (feeds `summary.tsv`) and
`output_markers::count_noop_facade_methods` (feeds the analysis profile) are
independent detectors for the same named signals and they disagreed — 145 vs 130
on htsjdk. A shared constant cannot fix that, so the agreement is asserted:
`the_python_sweep_and_the_rust_counters_agree_on_shared_signals` runs the real
sweep script over a translation and requires the numbers to match.

**`noop_facade_call_sites` — constructor subset built.** The earlier "cannot be
recovered from generated text" reasoning is still true for ordinary no-op facade
methods: the surviving names are `to_string`, `clone`, `into`, so counting
`.to_string(` would count mostly unrelated calls. It is not true for facade
constructors, which are called through qualified paths like `Foo::new_2(...)`.
That constructor subset is now counted and tested; the remaining method-call
subset still needs a lowering-time diagnostic.

Also: the sweep covers `generated/*.rs` only, so every number it reports is a
lower bound on anything in the crate's shared support module — 68 upcasts sit
there and are swept as 0.

## 0009 — overload identity (**identity fixed for the minimized case; facade completeness remains**)

Probing the remaining `.name ==` comparisons found a live case where the *same*
overloaded method got a different Rust name depending on which path reached it:

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

---

## The gate itself was reporting wrong numbers

Worth reading even if you skip the rest. `idiom_gaps` keeps *failing* generated
crates in `/tmp` so they can be inspected — reasonable — but kept them **whole**,
`target/` included. At ~146 failures a run with no bound on runs, the leftovers
reached **10,525 directories / 31 GB** and filled the root filesystem.

How it showed up:

1. a gate run came back `537/147` against a `538/146` baseline — one test, which
   is exactly the size of delta this whole audit set treats as a real regression;
2. a direct re-run of the same suite on the same tree reported **280 passed /
   404 failed**. Nothing had changed but free space.

I nearly attributed (1) to the change under test. Fixed by keeping what is
actually inspected and dropping what is regenerable: `Cargo.toml` and `src/`
stay, `target/` goes, so a failing crate still reproduces by running `cargo` in
its directory. **Cost of a full run: ~450 MB → 1.9 MB**, measured, suite still at
`538/146`.

I also deleted the 31 GB of accumulated leftovers (only `/tmp/new_java2rust_*`;
nothing else in `/tmp` was touched). `/` went from **0 bytes free to 31 GB**.

## Bookkeeping is now enforced, and it immediately found a gap

Three hand-maintained records of the same fact (the plan table, this
directory's README, and `done/`) have drifted before. Three tests in
`tests/output_markers.rs`:

- `tracked_plan_status_agrees_with_the_audit_directory` — a row that claims
  completion must have its file in `done/`, and vice versa;
- `every_audit_has_a_tracked_plan_row`;
- `audit_readme_links_resolve` — no dangling links, no audit without a row.

The second failed on first run: **0002b had no plan row**, despite three other
audits being blocked on it. Added.

## Suggested next

0009's adoption step, migrating one caller at a time behind
`CallableKey::source_method` — the reproducer above is the failing case to start
from. It is also the largest remaining source of silent wrongness that is *not*
blocked on the shared representation (0002b), which is what 0007, 0008, 0023 and
0028 are all waiting on.
