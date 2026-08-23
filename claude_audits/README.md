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
| [0042](done/0042-collection-mutation-is-five-lists-that-disagree.md) | Collection mutability/recognition now routes through shared typed native-collection policy; static argument mutators feed `mutates_self`; table-driven lowering dispatch is optional cleanup |
| [0043](done/0043-rust-keyword-escaping-is-four-lists-and-two-strategies.md) | Rust-keyword escaping now has one shared policy; fields, static fields, method/constructor params, locals, enhanced-for vars, ordinary callback lambdas, stream pipeline lambda params, and stream block locals allocate emitted names from source identity |
| [0044](done/0044-external-receiver-owner-key-is-computed-twelve-times.md) | External-fact **owner key** disagreement fixed across field-style keys, high-risk API receiver policy, parsed/lowered method-reference targets, usage modeled-receiver boundaries, known-builtin keys, aliasing mutator detection, external static-call owner facts, qualified static-owner Java walkers, and `lower.rs` source-context/static-retarget owner candidates |
| [0046](done/0046-slot-shape-type-identity-is-a-hand-refreshed-cache.md) | `RustSlotShape.type_identity` was a hand-refreshed cache of a pure function of `java_ty`; fixed by deleting the field and deriving `type_identity()` on demand |
| [0047](done/0047-facade-slot-algebra-is-nine-hand-copied-recursions.md) | `FacadeSlot` borrow predicates, slot→shape mapping, Rust-type→slot parsing, and ABI/effect precedence are centralized/fixed |
| [0048](0048-erased-static-trait-body-is-rewritten-by-substring.md) | The erased-static trait body is rendered to text then rewritten by **unanchored substring** replacement | compile error | `it.cfgVersion()` → `it.Self::version()`; the structured body was already in hand |
| [0049](0049-erased-object-owner-loses-the-static-call-target.md) | A static call to an erased-object class is re-attributed to the *calling* type | **silent** | `Alpha.run()` becomes `Self::run()` and picks the caller's overload |
| [0050](done/0050-builtin-pack-facts-and-name-aliases-rebuilt-per-call.md) | Built-in pack aggregate/method/type aliases are cached; production callers borrow built-in facts or use narrower iterators; the clone-returning aggregate API is compatibility-only |
| [0051](0051-derived-fields-refreshed-at-some-mutation-sites.md) | Three more stored derivations refreshed at only some mutation sites | latent + 1 live | a stale `ClosedEnum { variants }` emits an enum missing variants and `value_of` arms |
| [0052](0052-the-example-corpora-are-asserted-on-but-never-compiled.md) | **58 of 189** checked-in fixtures do not compile; only **3** emit `compile_error!` | measurement | 918 tests assert on rendered Rust, **486 never compile it** |
| [0053](0053-the-gate-compares-counts-not-names.md) | The gate diffs integers over a baseline of **264 accepted failures**, so a regression masked by a fix is invisible | measurement | the only behavioural oracle passes vacuously when `javac` is absent |
| [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md) | Compile-error counts were blind to silent wrong output and could reward erasure; fixed with marker/counter ownership, differential tests, a terminating idiom baseline, shared-support sweep coverage, and no-op facade call-site accounting |
| [0008](done/0008-inference-machinery-does-not-reach-lowering.md) | Inference stayed upstream while lowering re-derived facts locally; fixed for the audited scope by routing solved relations through `CallableAnalysis`, consolidating slot-shape specificity, landing storage strategy as a whole-program relation, and replacing drifted hardcoded library/enumeration tables with rule-backed queries |
| [0009](done/0009-overload-identity-collapses-onto-rust-name.md) | Overload identity collapse fixed: source method identity now uses erased Java signature/staticness across inherited suppression, summary surfaces, exception override equalization, interface candidates, and usage/call planning |

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
| [0009](done/0009-overload-identity-collapses-onto-rust-name.md) | Child's own method body **silently replaced** by an inherited overload; fixed by Java signature identity | corpus (FastQC) | done |
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
| [0023](0023-inner-class-construction-always-panics.md) | Non-static inner-class construction has non-panicking companions when an outer handle exists, but ordinary outer methods still call the panic fallback until shared receivers land | runtime | blocked on SharedLocal/shared receivers |
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
| [0008](done/0008-inference-machinery-does-not-reach-lowering.md) | **Closed for the audited scope.** Lowering consumes solved relations through `CallableAnalysis`; slot-shape specificity is centralized; storage strategy is a whole-program lattice consumer; and the recorded hardcoded library/enumeration drift has rule-backed tests |
| [0022](done/0022-types-are-reparsed-from-source-text.md) | `parse_type_node` re-parsed types from text. **Closed for the source route**, not for the file: the ~4.9M remaining `parse_type_text` calls are all `javalibs`/manifest input (the correct use), and four defensive fallbacks stay in `parser.rs`, documented and measured at 0 hits over the corpus |
| [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md) | **Closed.** The compile metric is no longer the only quality signal: marker/counter ownership, fixture-backed detector tests, the differential harness, the `idiom_gaps` failure-name baseline, shared-support sweep inputs, and no-op facade call-site accounting are all gate-covered |

## Suggested order

See [SUGGESTIONS.md](SUGGESTIONS.md) for the reasoning and the corpus numbers.
In brief:

0. **0014 silent-drop prohibition** — make every unhandled construct emit
   `compile_error!` instead of vanishing. Cheapest change here and it prevents a
   whole class ([0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md)).
   Then the **0012, 0013** parse errors: a file that fails to parse reports zero
   type errors, so these distort the corpus measurements today.
1. **0004, 0005, 0014, 0016** — small, exact, verified; each a silent wrong
   answer or silent code loss.
2. **0011** — replace the `Box<dyn Any>` fallback with typed opaque shims.
   Highest leverage per unit of work; removes ~19% of FastQC's errors.
3. **0003**, **0002a** — arithmetic policy; `Clone` obligation tracking.
4. **0010** — fallibility as a fixpoint. Cheapest end-to-end demonstration that
   routing an analysis through the pipeline works; the call graph already exists.
5. **0009** — method identity by Java signature. Done; keep new callable surfaces keyed by erased signature/staticness.
6. **Optional carriers** — largest genuine defect class (~40 of htsjdk's E0308).
7. **0006 → 0001**, **0007**, **0002b** — binding identity and CFG
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
