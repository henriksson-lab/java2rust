# Claude audits

Validated defects and structural findings for the Java→Rust translator.
One file per issue, written so another agent can pick it up without re-deriving
the evidence.

Each file states: symptom, minimal repro, **verified** evidence (rustc output or
runtime output), root cause with `file:symbol` anchors, whether it needs a local
fix or new machinery, and a proposed direction.

Java sources for every repro are in `claude_audits/repro/`.

## Convention: completed issues move to `done/`

When an issue is fixed, its file is **moved to `claude_audits/done/`** — it is not
deleted. If a numbered file is missing from this directory, look in `done/`
before concluding anything is lost, and **do not recreate it**.

Already fixed and moved:

| # | issue |
|---|---|
| [0012](done/0012-unsupported-diagnostics-emit-invalid-rust.md) | `compile_error!` emitted invalid Rust, voiding its file's other diagnostics |
| [0013](done/0013-nested-type-names-unmangled-in-downcast.md) | `downcast::<GuideTree.Node>` parse error |
| [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md) | move analysis was five name-keyed approximations of one relation |
| [0021](done/0021-instanceof-pattern-variable-not-bound.md) | `instanceof` pattern bindings dropped outside `if` conditions |
| [0022](done/0022-types-are-reparsed-from-source-text.md) | source types were re-parsed from text, hiding a dropped array dimension. **Scoped:** closed for the source route; four defensive `parse_type_text` fallbacks remain in `parser.rs` by design, measured at 0 hits |
| [0034](done/0034-object-array-allocation-produces-an-empty-vec.md) | `new T[n]` allocated length 0, so every index panicked |
| [0035](done/0035-idiom-gaps-suite-never-terminates.md) | the largest behavioural suite never returned, so it had no baseline |
| [0025](done/0025-interface-default-methods-are-discarded.md) | interface `default` methods: five defects, four sharing one nested-name-lookup cause |
| [0033](done/0033-receiver-mutability-not-propagated-through-lambdas.md) | a method mutating self only inside a lambda was inferred `&self` |
| [0036](done/0036-string-concat-type-inference-is-exponential.md) | type inference was exponential in `+` chain length; a dozen segments took minutes |
| [0037](done/0037-nested-class-outer-static-read-is-capture.md) | nested class reading an enclosing `static` lowered as an outer-*instance* capture |
| [0038](done/0038-two-dimensional-object-array-carrier-disagrees.md) | `String[][]` was declared, built, written and read as three different carriers |
| [0039](done/0039-static-array-fields-lose-their-value-and-their-writes.md) | `static int[] a = new int[3]` had length 0, and every `a[i] = v` was silently discarded |
| [0040](done/0040-external-type-name-matching-recomputes-on-the-hot-path.md) | External type-name matching rebuilt its alias lists per pack member — **81 s to translate 7 methods** | performance | fixed, memo |
| [0041](done/0041-slot-shape-design-had-no-generated-crate-checks.md) | `slot_shape_design` asserted on rendered Rust with **no** check that it compiled — 596 tests, 0 generated-crate checks | test coverage | 99 checked; 21 genuine failures and 31 harness-only ones enumerated |

Issues 0012 and 0013 were the parse-error pair in Tier 0 of
[SUGGESTIONS.md](SUGGESTIONS.md), so that step is complete: corpus error counts
are no longer being voided by files that fail to parse. Worth re-running the
corpora to see what those two files were hiding.

## How to reproduce any of these

```sh
cargo build --release
./target/release/new-java2rust claude_audits/repro/<Name>.java > /tmp/out.rs
rustc --edition 2021 --crate-type lib --crate-name probe --emit=metadata --out-dir /tmp /tmp/out.rs
```

For the runtime-divergence issues, append a `fn main()` to `/tmp/out.rs` and
build it as a binary instead.

**Concurrency scope: [../CONCURRENCY-SCOPE.md](../CONCURRENCY-SCOPE.md)** — what
"concurrency is in scope" costs, and the representation decision it forces
*before* 0002 is implemented.

**Coverage map: [../SEMANTIC-COVERAGE.md](../SEMANTIC-COVERAGE.md)** — what has
and has not been checked, indexed by JLS chapter. Short version: these findings
cover roughly a third of the semantic surface; concurrency and arrays are
near-zero.

**What is left to audit: [../AUDIT-BACKLOG.md](../AUDIT-BACKLOG.md)** —
prioritised remaining territory, including an *exhaustive* grammar-coverage diff.

**Start with [SUGGESTIONS.md](SUGGESTIONS.md)** — prioritised suggestions for
handling the idioms we currently fail, grounded in corpus error counts.

## Confirmed defects

| # | Issue | Evidence | Fix size |
|---|---|---|---|
| [0001](done/0001-loop-back-edge-invisible-to-move-analysis.md) | Loop back edges invisible to move analysis → `E0382` | rustc | needs machinery (0006) |
| [0002](0002-object-aliasing-has-no-representation.md) | Object aliasing translated as `clone()`; wrong result **and** `E0599`. `E0599` fixed; the divergence is now *reported* (`observable_alias`) but not repaired | rustc + runtime | 0002a done / 0002b large |
| [0002b](0002b-shared-representation-design.md) | **Design, not a defect**: the shared-representation plan that unblocks 0002b, 0023, 0028 and concurrency. Every analysis it needs already exists | — | design ready, steps 1–2 are small |
| [0003](done/0003-java-integer-arithmetic-semantics-not-preserved.md) | Integer overflow panics in debug, wraps in release; `/ 0` not a Java exception; shift distance unmasked | runtime | medium, self-contained |
| [0004](done/0004-string-concat-absorbs-nested-arithmetic.md) | `"t " + (a + b)` → `"t 12"` instead of `"t 3"` | runtime | **small — exact fix given** |
| [0005](done/0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md) | `>>>` parsed as `>>`; wrong for all negative operands | runtime | small, mechanical |
| [0009](0009-overload-identity-collapses-onto-rust-name.md) | Child's own method body **silently replaced** by an inherited overload; 14 files | corpus (FastQC) | medium — better key |
| [0010](done/0010-fallibility-is-decided-three-times-and-disagrees.md) | `?` emitted where neither callee nor caller returns `Result`; 214 errors | corpus (FastQC) | medium — fixpoint |
| [0011](done/0011-dyn-any-fallback-fails-at-every-use.md) | `Box<dyn Any>` fallback fails at every method call; 271 errors | corpus (FastQC) | medium — fallback shape |
| [0012](done/0012-unsupported-diagnostics-emit-invalid-rust.md) | `compile_error!` with unescaped quotes → file fails to **parse**, hiding all its other errors | corpus (biojava) | **tiny** |
| [0013](done/0013-nested-type-names-unmangled-in-downcast.md) | `downcast::<GuideTree.Node>` → parse error | corpus (biojava) | **tiny** |
| [0014](done/0014-labelled-statements-are-silently-deleted.md) | **Any labelled loop is deleted from the output**, no diagnostic anywhere | runtime | small — parser arm |
| [0015](done/0015-static-initializer-blocks-are-silently-dropped.md) | `static { … }` blocks dropped silently | runtime | medium |
| [0016](done/0016-core-jdk-facades-are-no-op-stubs.md) | `StringBuilder`/`FastMath`/`Pattern` are no-ops; 181 live call sites | runtime + corpus | medium |
| [0017](done/0017-generic-bounds-dropped-and-method-type-params-undeclared.md) | Generic bounds dropped; method type params never declared; ~36% of biojava | probe + corpus | large |
| [0019](done/0019-unhandled-expressions-emit-raw-java-source.md) | Unhandled expressions emitted as **raw Java source**; hits `switch` expressions | rustc | small |
| [0020](done/0020-text-block-content-not-processed.md) | Text block keeps incidental indentation → wrong string | runtime | small |
| [0023](0023-inner-class-construction-always-panics.md) | **Every non-static inner class panics on construction**; compiles clean | runtime | blocked on 0002b |
| [0024](done/0024-numeric-limit-constants-stubbed-to-zero.md) | **`Integer.MAX_VALUE` is `0`** — breaks every min/max idiom; live in corpus | runtime + corpus | **tiny** |
| [0025](done/0025-interface-default-methods-are-discarded.md) | `default` method bodies dropped → runtime panic | runtime | small — Rust supports it natively |
| [0026](done/0026-assignment-evaluation-order-differs-from-java.md) | Rust evaluates assignment RHS before the target; Java is the reverse | runtime | small |
| [0027](done/0027-compound-assignment-implicit-narrowing-missing.md) | `byte b; b += 300;` → `E0308`; missing JLS 15.26.2 narrowing cast | rustc | **tiny** |
| [0028](done/0028-reference-equality-lowered-as-structural-equality.md) | `==` on classes was structural, not identity — **violated decision 2** | runtime | fixed: `Rc`/`Arc::ptr_eq`, two differential fixtures |
| [0029](done/0029-static-field-mutation-emits-invalid-rust.md) | `static n; n++;` → `E0067` invalid left-hand side | rustc | small |
| [0030](done/0030-java-statics-are-thread-local.md) | Java `static` fields are `thread_local!` — per-thread copies | inspection | split 0030a/b |
| [0031](done/0031-stubbed-constant-census.md) | **`Math.PI` is `0.0`**; full census of 27 stubbed constants across 3 corpora | corpus | **tiny for the fix** |
| [0032](done/0032-java-lang-math-is-mistranslated.md) | `Math.round(-2.5)` → `-3`, Java `-2`; plus 5 more `Math` defects. **Tested replacement code included** | rustc + runtime | medium, one focused pass |

## Structural findings

These explain why the defects above recur, and are the argument for what
machinery to add.

| # | Finding |
|---|---|
| [0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md) | **Closed.** One backward liveness fixpoint over binding identity; no name-set approximation left on `LowerCtx` |
| [0007](0007-rust-ir-is-mostly-opaque-strings.md) | 58% of `RustExpr` is `Raw(String)`, forcing 55 distinct substring checks — including parsing operators with `contains(" + ")` and matching on an emitted comment. **Partly addressed**: `RustExpr::Shaped` records the answer at the producer; 4 adapter families migrated, queries monotone so migration cannot regress |
| [0008](0008-inference-machinery-does-not-reach-lowering.md) | `lattice.rs` / `fact_saturation.rs` / `graph.rs` are unreachable from `lower.rs`; 206 ad-hoc `-> bool` predicates re-derive them locally. **Partly addressed**: 206 → 159 predicates, `CallableAnalysis` is the single handle, two whole-program relations now consumed. The lattice grep is still 0 *deliberately* — read the update before using it as a score |
| [0022](done/0022-types-are-reparsed-from-source-text.md) | `parse_type_node` re-parsed types from text. **Closed for the source route**, not for the file: the ~4.9M remaining `parse_type_text` calls are all `javalibs`/manifest input (the correct use), and four defensive fallbacks stay in `parser.rs`, documented and measured at 0 hits over the corpus |
| [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md) | **8 of 14 validated defects produce zero compile errors**, and several failure modes *lower* the error count — the metric can reward erasure |

## Suggested order

See [SUGGESTIONS.md](SUGGESTIONS.md) for the reasoning and the corpus numbers.
In brief:

0. **0014 silent-drop prohibition** — make every unhandled construct emit
   `compile_error!` instead of vanishing. Cheapest change here and it prevents a
   whole class ([0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)).
   Then the **0012, 0013** parse errors: a file that fails to parse reports zero
   type errors, so these distort the corpus measurements today.
1. **0004, 0005, 0014, 0016** — small, exact, verified; each a silent wrong
   answer or silent code loss.
2. **0011** — replace the `Box<dyn Any>` fallback with typed opaque shims.
   Highest leverage per unit of work; removes ~19% of FastQC's errors.
3. **0003**, **0002a** — arithmetic policy; `Clone` obligation tracking.
4. **0010** — fallibility as a fixpoint. Cheapest end-to-end demonstration that
   routing an analysis through the pipeline works; the call graph already exists.
5. **0009** — method identity by Java signature. Most severe correctness bug.
6. **Optional carriers** — largest genuine defect class (~40 of htsjdk's E0308).
7. **0006 → 0001**, **0007**, **0002b**, **0008** — binding identity and CFG
   liveness, mandatory shape tags, shared representation. Design-heavy.

## Scope note

Not an exhaustive sweep, but the parser-coverage dimension **is** now exhaustive:
see [../AUDIT-BACKLOG.md](../AUDIT-BACKLOG.md) for the grammar diff and the
remaining territory, ranked by expected yield.

30 probes have produced 16 validated defects, **9 of them silent** — they
compile cleanly and return wrong values, delete code, or panic at runtime, so no
compile-error count would ever surface them. The method that found them costs
about ten minutes each: write a ten-line Java file, translate, compile, run,
diff against Java.

That method is now automated: `tests/differential.rs` runs `javac`+`java` as
the oracle against translate+`rustc`+run and compares stdout, over the fixtures
in `tests/differential/`. Fixtures reproducing an open defect live in
`tests/differential/known-gaps/` so the harness stays green as a gate; moving
one up a level is the regression test for the fix. 0034 was found this way,
along with six arithmetic and formatting defects before it.
