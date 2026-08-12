# Suggestions for handling the idioms we currently fail

Grounded in measured corpus data (FastQC 1451 errors, htsjdk 343, biojava 188)
plus 11 minimal Java probes. Every claim below has a validated issue file behind
it. Numbers are from `.tmp/corpus-*/top-compile-errors.tsv` as of 2026-07-30.

---

## 1. What actually fails, by cause

The per-message ranking in `top-compile-errors.tsv` is misleading, because one
cause produces many differently-worded messages. Regrouped by cause:

### FastQC — 1451 errors

| share | cause | issue |
|---|---|---|
| 271 (19%) | unmodelled external types erased to `Box<dyn Any>`, failing at every method call — almost all AWT/Swing | [0011](done/0011-dyn-any-fallback-fails-at-every-use.md) |
| 214 (15%) | `?` inserted where neither callee nor caller returns `Result` | [0010](done/0010-fallibility-is-decided-three-times-and-disagrees.md) |
| 164 (11%) | overload identity collapse — inherited body under the child's signature | [0009](done/0009-overload-identity-collapses-onto-rust-name.md) |
| 279 (19%) | `E0308` mismatched types — mostly Option-carrier and unit-return, see below | — |

### htsjdk (today's active target) — 343 errors

`E0308` is 152 of 343 (44%). Broken down by the actual mismatch:

| count | mismatch | cause |
|---|---|---|
| ~40 | `expected String, found Option<String>` / `Option<&String>` / `Option<Vec<_>>` / `Option<HashMap<_,_>>` | **optional carrier not adapted at the boundary** |
| ~14 | `expected T, found ()` | callee's return type lowered to unit |
| ~9 | `expected T, found Box<dyn Any>` | [0011](done/0011-dyn-any-fallback-fails-at-every-use.md) |
| 8 | `expected String, found Result<String, JavaException>` | [0010](done/0010-fallibility-is-decided-three-times-and-disagrees.md) |
| ~7 | `&String` vs `&str`, `&[i8]` vs `&String` | borrow/view projection |
| 26 | `E0282` type annotations needed | generics/erasure |

### biojava — 188 errors

Dominated by generics (`cannot find trait GapPenalty`, `expected trait, found
struct GapPenalty`, `undeclared type Out`) and by **two parse errors that void
the error counts for their files** ([0012](done/0012-unsupported-diagnostics-emit-invalid-rust.md),
[0013](done/0013-nested-type-names-unmangled-in-downcast.md)).

### The headline

**Optional-carrier adaptation is the single largest genuine translator defect in
the active corpus.** That matches the "Optional Carriers" hotspot in
`docs/rule-generalization-convergence-plan.md` — this is confirmation with
numbers, and it should be the top priority once the cheap items are cleared.

---

## 2. Suggestions, in the order I would do them

> **Update after a second probe pass.** The most severe defects found are not in
> this error data at all. **8 of 14 validated defects produce zero compile
> errors** — including labelled loops being *deleted from the output*
> ([0014](done/0014-labelled-statements-are-silently-deleted.md)), `static {}` blocks
> dropped ([0015](done/0015-static-initializer-blocks-are-silently-dropped.md)), and
> `StringBuilder` being a no-op with 181 live call sites
> ([0016](done/0016-core-jdk-facades-are-no-op-stubs.md)). Worse, several failure
> modes *reduce* the error count, so the metric can reward erasure. See
> [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md). Tier 0
> below is revised accordingly.

### Tier 0 — the metric is lying to you (do first, ~1–2 days)

**0. Prohibit silent drops.** Every construct the translator does not handle
must appear in the output as `compile_error!`. Today an unrecognised statement
is filtered out and vanishes — no stderr, no marker, clean compile. This is one
change, it prevents an entire class, and without it every other fix is scored by
a metric that a silent drop would score better.
*A translator may refuse to translate; it must never quietly omit.*

**Status: the two parse-error fixes below (0012, 0013) are already done** — see
`claude_audits/done/`. Re-run the corpora; those files' real errors were
previously invisible, so the biojava total in section 1 is now understated.

Then three things that distort the signal:

1. **Fix the two parse errors** ([0012](done/0012-unsupported-diagnostics-emit-invalid-rust.md),
   [0013](done/0013-nested-type-names-unmangled-in-downcast.md)). A file that fails to
   parse reports *zero* type errors, so its real problems are invisible and
   biojava's 188 is not comparable to other corpora. Both fixes are a few lines.
2. **Add a parse gate** separate from the type-check gate, reported in
   `summary.tsv`. Parse failures must never be silently folded into the error
   count.
3. **Stop counting unmodelled-library cascades as translator errors**
   ([0011](done/0011-dyn-any-fallback-fails-at-every-use.md)). 19% of FastQC's errors
   come from ~12 unmodelled Swing classes. Report `unmodelled_types` separately.

4. **Add value-level differential testing.** Compile and run the Java and the
   Rust for a dozen deterministic entry points and diff the output. This is the
   only technique that finds the silent class at all: every one of 0003, 0004,
   0005, 0014, 0015, 0016 was found by *running* a program, none by compiling
   one. It need not be elaborate to pay for itself.

Without this, every ranking below is measured through a distorting lens — and
the top of the FastQC ranking is currently a decision you already made correctly
(don't model Swing), not a defect.

### Tier 1 — cheap, exact, validated (each is hours, not days)

| fix | why now |
|---|---|
| [0014](done/0014-labelled-statements-are-silently-deleted.md) add the `labeled_statement` parser arm | code is currently **deleted**; `java_ir` loops already have `label`, and `RustStmt::Loop`/`Break`/`Continue` already carry labels, so lowering is nearly free |
| [0016](done/0016-core-jdk-facades-are-no-op-stubs.md) implement `StringBuilder`, `Arrays`, `Pattern`, `FastMath` for real | bounded work; removes silent empty/zero results at 181 live call sites in one corpus |
| [0005](done/0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md) `>>>` → add `BinaryOp::UShr` | silent wrong answer; parser destroys the information, so nothing downstream can compensate |
| [0004](done/0004-string-concat-absorbs-nested-arithmetic.md) string concat absorbing nested `+` | silent wrong answer; exact fix written out |
| [0002a](0002-object-aliasing-has-no-representation.md) track `Clone` obligations | removes `E0599 no method named clone`; land with a diagnostic, since it makes semantically-wrong output compile |
| [0003](done/0003-java-integer-arithmetic-semantics-not-preserved.md) arithmetic policy | removes the debug/release behaviour split |
| [0015](done/0015-static-initializer-blocks-are-silently-dropped.md) `static {}` blocks | at minimum a diagnostic; the full class-init ordering fix is larger |

These are independent of each other and of everything below.

### Tier 2 — the fallback shape (this is the highest-leverage design change)

Replace `Box<dyn Any>` with **opaque named shims per unmodelled type**, methods
synthesised from observed call sites, one diagnostic per type
([0011](done/0011-dyn-any-fallback-fails-at-every-use.md)).

Payoff is disproportionate: it removes ~19% of FastQC's errors, lets files that
touch one unmodelled type compile the rest of their content, and converts a
symptom-ranked error list into a cause-ranked one. It also unblocks whatever
those 271 errors are currently masking — which is unknown today, and worth
knowing before committing to Tier 4.

### Tier 3 — one solved relation, end to end: fallibility

[0010](done/0010-fallibility-is-decided-three-times-and-disagrees.md) — 214 errors,
and the cheapest possible demonstration that routing an analysis end to end
works, because the call graph already exists in `src/usage.rs` and no CFG or
binding identity is needed.

Solve `fallible(m)` as a least fixpoint over the call graph, then make signature
planning and `?` insertion both *read* it. Add the invariant assertion (at every
`?`, callee and caller must both be `Result`) so violations surface as a handful
of attributable analysis failures instead of 214 downstream type errors.

Do this before the harder analyses. It is the proof that the pattern pays, and
it will teach you what the migration ergonomics are.

### Tier 4 — the two identity problems

These are the same defect at two levels: **identity modelled as a name string**.

- **Method identity** ([0009](done/0009-overload-identity-collapses-onto-rust-name.md)) —
  key inheritance and override matching on `(name, erased param types)`, never on
  the Rust name or arity. Assign Rust names as a late, total function of the
  resolved signature set. This is the **most severe correctness bug found**: a
  child's own method body is silently discarded, in 14 files.
- **Binding identity** ([0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md)) —
  assign a `BindingId` at each declaration; stop keying liveness on `String`.
  Prerequisite for the CFG work.

Method identity first: it is smaller, it is a correctness bug rather than a
compile bug, and it does not need new machinery — only a better key.

### Tier 5 — the optional carrier (largest genuine defect)

~40 of htsjdk's 152 `E0308`s are one shape: an `Option<T>` reaching a slot that
wants `T`, or `Option<&T>` where `&T` is wanted.

Suggested approach, consistent with the convergence plan but sharper:

1. Make every nullable-producing path return
   `LoweredExpr { expr, actual: RustSlotShape }` — never a bare rendered string.
   Today carrier creation is scattered and each site independently chooses
   between `Some(v)`, `Some(&v)`, `.as_ref()`, `.as_deref()`, `.cloned()`,
   `.expect(…)`.
2. Give `SlotConversionPlan` sole authority over the final spelling. One
   function, `adapt(actual, target, use_context) -> RustExpr`, total over the
   shape product — with an explicit `Unsupported` arm that produces a
   *diagnostic*, not a wrong spelling.
3. The `expected T, found ()` cluster (~14) is adjacent and probably cheaper:
   a callee whose return type was lowered to unit. Worth a quick separate probe
   before folding it in — it may be a signature bug rather than a carrier bug.

The reason this is Tier 5 rather than Tier 1, despite being the biggest defect:
it is the one that most needs the shape tag from
[0007](0007-rust-ir-is-mostly-opaque-strings.md) to exist first, and it benefits
from the migration ergonomics learned in Tier 3.

### Tier 6 — structural, only after the above

- **Mandatory shape tags on lowered expressions**
  ([0007](0007-rust-ir-is-mostly-opaque-strings.md)). 58% of `RustExpr` is
  `Raw(String)`, forcing 55 distinct substring checks in `coercion.rs` —
  including `contains(" + ")` to detect an operator and a match against an
  emitted `/* TODO */` comment. Make `Raw` constructible only through a helper
  that requires a shape. This stops regrowth; it does not need a big-bang
  rewrite.
- **Liveness over a real CFG** ([0006](done/0006-move-analysis-is-five-approximations-of-one-relation.md)
  → [0001](done/0001-loop-back-edge-invisible-to-move-analysis.md)).
- **Shared object representation** ([0002b](0002-object-aliasing-has-no-representation.md)).
  The largest and most design-heavy. Note it is *not* currently a large source of
  corpus errors — `E0382` is only 5 in FastQC — because clone insertion hides it
  as **wrong behaviour** rather than failed compilation. Its cost is invisible to
  the error count, which is an argument for tackling it deliberately rather than
  when the metric demands it.
- **Generics/erasure** — biojava's trait-vs-struct confusion and htsjdk's 26
  `E0282`. Not yet analysed in depth; deserves its own probe pass before design.

---

## 3. Two suggestions about method, not about code

### Fix causes, not corpus instances

`.tmp/` contains **3569 corpus-rerun directories**, named
`…-after-source-enum-valueof-slot`, `…-after-trailing-super-throw`,
`…-after-child-prefix-option-string` — one per attempted fix. `docs/` holds ~20
sequential `proceed-from-current-corpus-*` plans. This is the signature of
fixing at the symptom, and it is why `src/lower.rs` now has 206 `-> bool`
predicates, ~17 of which ask the same liveness question in slightly different
ways.

The suggestion is not "stop doing corpus-driven work" — the corpus is the best
signal available. It is: **before fixing a corpus instance, check whether it is
an instance of an issue in `claude_audits/`.** If it is, either fix the cause or
record the instance under that issue. A fix that adds a predicate to `lower.rs`
should be treated as a diagnosis, not a resolution.

### Keep the divergence list when migrating

For every "replace N local rules with one solved relation" change (Tiers 3–6),
compute both the old and the new answer behind an assertion for one corpus run,
and diff. Each divergence is either a bug being fixed or a deliberate
approximation nobody wrote down. That list is the actual work queue, and it is
the only way to attribute the error-count increase that
`docs/rule-generalization-convergence-plan.md` correctly anticipates. With 206
predicates in one file, an unattributed increase is indistinguishable from a
regression.

---

## 4. Coverage note

Second probe pass (7 more files) results, for calibration:

| probe | verdict |
|---|---|
| labelled `continue`/`break` | **deleted silently** ([0014](done/0014-labelled-statements-are-silently-deleted.md)) |
| `static { … }` block | **dropped silently** ([0015](done/0015-static-initializer-blocks-are-silently-dropped.md)) |
| `StringBuilder` | **no-op facade** ([0016](done/0016-core-jdk-facades-are-no-op-stubs.md)) |
| generic bounds, `<U>` methods | **broken** ([0017](done/0017-generic-bounds-dropped-and-method-type-params-undeclared.md)) |
| `switch` fallthrough | **correct** — verified across 4 inputs |
| inner-class capture of outer field | compiles; semantics not yet verified |
| `try`/`finally` with `return` in `try` | **correct**; `return` inside `finally` is unsupported **and correctly diagnosed** |
| `Integer` boxing `==` (128) | diverges from Java (`true` vs `false`); may be deliberate — confirm intent |

Still unprobed: interface default methods, generic wildcards and raw types,
anonymous/local classes, varargs, `String` interning (`==` currently lowers to
value equality), array covariance, and `finally` interacting with `break`.

The method costs about ten minutes per probe: write a ten-line Java file,
translate, compile, run, diff against Java. Across 18 probes it found nine
defects, **six of them silent** — invisible to any compile-error count. Two of
those (0014, 0015) delete code outright.

One caution learned the hard way during this pass: two probes initially looked
like defects and were not. `switch` fallthrough appeared broken because its
`StringBuilder` was a no-op; and a high stub ratio initially read as "the
translator emits hollow programs" when in fact translated code is only 0–8%
no-op and the stubs sit in the facade layer. **Always confirm which layer the
failure is in before filing it** — otherwise the corpus grows fixes for
non-problems, which is the dynamic these audits exist to interrupt.
