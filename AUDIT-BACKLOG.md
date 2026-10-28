# What is left to audit

A prioritised backlog, ordered by expected yield. Each entry says what to probe,
why it is likely to yield, and what evidence already points there. Findings so
far are in `claude_audits/`.

> **Reconciliation 2026-09-22.** Parser items A1–A5 are now audited in
> [0073](claude_audits/done/0073-remaining-java-grammar-coverage.md). The
> original descriptions below are retained as the hypotheses that were tested,
> not as active work. Switch-expression `yield`, switch rules, type patterns,
> and structured type nodes were already implemented; record/unnamed patterns
> are now explicitly fenced; modules and receiver parameters have direct tests;
> and annotation erasure has a documented policy and corpus census.

> **Completion follow-up 2026-09-22.** Re-running the focused completion gates
> found and closed two current regressions that the historical reconciliation
> alone did not expose: concrete overrides of interface defaults (0075), and
> nested-enum `values()` through `Arrays.asList`/enhanced-for (0076). There are
> now no active numbered audit files at `claude_audits/` top level.

> **Gate reconciliation 2026-09-22.** Audits 0077–0079 restored the populated
> `idiom_gaps` failure-name oracle, repaired packaged-method usage lookup,
> migrated five stale generated-output assertions, and fixed the gate parser so
> nested generated-crate summaries cannot masquerade as the outer suite result.
> The remaining named failures are an explicit queue for related next batches,
> not an empty-oracle success signal.

> **Nullable-array closure 2026-09-22.** Audits 0080–0082 close the related
> nullable-array ABI, optional mutable reborrow, and same-call alias-snapshot
> failures. The release census is 797 passed / 153 failed, with exactly the ten
> targeted failure names removed and no new names added.

> **Two-dimensional array closure 2026-09-22.** Audits 0083–0085 close the
> doubled ndarray-reference ABI, static nested-slice/ndarray carrier boundary,
> and ragged object-array oracle defect. The release census is now 805 passed /
> 145 failed, with eight existing failure names removed and no new names added.

The productive method throughout: **write a ten-line Java file, translate,
compile, run it, diff against Java.** 30 probes have produced 16 defects,
9 of them silent — invisible to any compile-error count. Compile errors only
find idioms that fail loudly.

---

## Rewrite categories preserved from 0062

> **Reconciliation 2026-09-22.** These four candidate families have since been
> exercised by completed audits: classic-for continue updates and nested labels
> by 0002/0007, borrow conflicts by the slot-shape/generated-crate work in
> 0041/0056, escaping captures by 0002/0033/0041, and complex `finally` control
> flow by 0057/0060. They remain useful regression categories, but no longer
> denote unaudited territory.

`RewriteNeed` was deleted as dead code, but four names in that abandoned design
still describe unfinished rewrite categories that should remain visible in the
active backlog: `LoopContinueUpdatePath`, `BorrowConflict`,
`EscapingInnerCapture`, and `ComplexFinally`. Treat these as candidate repro
families for future audits rather than as evidence that a runtime path exists.

---

## A. Parser coverage — now bounded and exhaustively known

This dimension is **finished as an inventory**. Diffing the tree-sitter-java
grammar (151 named node types) against every string literal in `src/parser.rs`
gives the complete set of constructs the parser never mentions. Reproduce with:

```sh
python3 - <<'EOF'
import json, re
G = "~/.cargo/registry/src/*/tree-sitter-java-0.23.5/src/node-types.json"   # expand
named = {n["type"] for n in json.load(open(G)) if n.get("named")}
handled = set(re.findall(r'"([a-z_]+)"', open("src/parser.rs").read()))
print("\n".join(sorted(t for t in named if t not in handled)))
EOF
```

**Rerun this after any parser change** — it is a cheap, exact completion signal.

### Already probed and filed

| construct | result |
|---|---|
| `labeled_statement` | deleted silently — [0014](claude_audits/0014-labelled-statements-are-silently-deleted.md) |
| `assert_statement` | deleted silently — [0014](claude_audits/0014-labelled-statements-are-silently-deleted.md) |
| `switch_expression` (as value) | raw Java injected — [0019](claude_audits/0019-unhandled-expressions-emit-raw-java-source.md) |
| `type_pattern` | binding dropped — [0021](claude_audits/0021-instanceof-pattern-variable-not-bound.md) |
| `static_initializer` | dropped silently — [0015](claude_audits/0015-static-initializer-blocks-are-silently-dropped.md) |
| `type_parameter`, `type_bound` | bounds unreachable — [0017](claude_audits/0017-generic-bounds-dropped-and-method-type-params-undeclared.md), [0022](claude_audits/0022-types-are-reparsed-from-source-text.md) |

### Audited in 0073 — historical probe list

**A1. `yield_statement`** — the third and last statement-level fallback hit.
Expect a silent drop.

**A2. Java 21 patterns**: `record_pattern`, `record_pattern_body`,
`record_pattern_component`, `underscore_pattern`, `guard`, `switch_rule`.
Expect raw-Java injection or silent drops. Priority depends on whether the
corpora use Java 21 — check first, this may be zero-value today.

**A3. Annotations**: `annotation`, `marker_annotation`,
`annotation_argument_list`, `element_value_pair`, `element_value_array_initializer`.
Almost certainly ignored, which is *usually correct* — but `@Override`,
`@FunctionalInterface`, and `@Deprecated` carry semantics the translator could
use, and silently ignoring `@Nullable`/`@NonNull` discards nullability
information the codebase works hard to infer elsewhere. **Worth checking
whether nullability annotations are being thrown away** — that is a real
opportunity, not just a gap.

**A4. Type-level nodes** (`array_type`, `annotated_type`, `type_arguments`,
`scoped_identifier`, `wildcard`, …). Confirmed unreachable because
`parse_type_node` re-parses text ([0022](claude_audits/0022-types-are-reparsed-from-source-text.md)).
Probe the fragile shapes specifically: `List<@NonNull String>`,
`Map<String, List<Integer>>` (nested `>>`), `T[]` of generics, varargs `T...`,
`Outer<A>.Inner<B>`.

**A5. Modules** (`module_declaration` and the `*_module_directive` nodes) and
`receiver_parameter`. Low value; note and move on.

---

## B. Semantic divergences — highest yield per probe, all silent

> **Reconciliation 2026-09-22.** B2–B7 are covered by the completed arithmetic,
> evaluation-order, collection, synchronization, and exception audits. B1's
> identity, interning, split, and hash-code paths already had differential
> coverage; its remaining non-ASCII ordering hypothesis produced
> [0074](claude_audits/done/0074-string-compareto-was-rust-byte-order.md), now
> fixed with a UTF-16 oracle. The descriptions below are retained as historical
> probe rationale rather than active issues.

Every finding in this class compiles cleanly and returns wrong values, so
**none** would ever appear in corpus error counts
([0018](claude_audits/done/0018-the-compile-error-metric-cannot-see-silent-failures.md)).
Six of seven probes here have already hit.

**B1. String semantics.** `==` on `String` currently lowers to *value* equality;
Java is reference equality. Same for `Integer` outside the −128..127 cache
(`Boxing.java` confirms: emitted `true`, Java `false`). Both may be deliberate
"friendly" choices — **confirm intent, then document the decision**, because
right now the divergence is undocumented either way. Also probe: `intern()`,
`String.split` regex semantics, `compareTo` ordering vs Rust's `Ord` on
non-ASCII, `hashCode` stability if anything depends on it.

**B2. Numeric conversions and promotion.** Partly covered by
[0003](claude_audits/0003-java-integer-arithmetic-semantics-not-preserved.md)/[0005](claude_audits/0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md).
Still unprobed: binary numeric promotion (`byte + byte → int`,
`int + long → long`, `char + int → int`), compound-assignment implicit
narrowing (`byte b; b += 300;` is legal Java and truncates), `float`↔`double`
rounding, `long → double` precision loss, `(int)` of `NaN`/infinity (Java
defines these; Rust `as` saturates — likely a real divergence), and
`Math.round` half-up vs Rust's half-away-from-zero.

**B3. Evaluation order.** Java fixes left-to-right evaluation of operands and
arguments. Probe `f(i++, i++)`, `a[i()] = b[j()]`, and side-effecting operands
of `&&`/`||`. Any reordering during lowering is a silent wrong answer.

**B4. Integer division and modulo edge cases.** `Integer.MIN_VALUE / -1`
overflows in Java (wraps) and **panics** in Rust. `%` with `MIN_VALUE`
likewise. One probe.

**B5. Ordered collection contracts.** `HashMap`/`HashSet` iteration order is
**explicitly out of scope** — Java does not specify it, so source that depends on
it is already broken. But `LinkedHashMap` (insertion order), `LinkedHashSet`,
`TreeMap`/`TreeSet` (sorted), and `List` order **are** specified and must be
preserved. Probe what those map to; a `LinkedHashMap` lowered to
`std::collections::HashMap` would be a real defect.

**B7. `synchronized` is erased.** `synchronized` methods become `&mut self` and
`synchronized(this){…}` becomes a plain block — no lock. Single-threaded output
makes this harmless, and Rust's `&mut self` even enforces intra-thread
exclusivity. **Scoping question, not necessarily a defect:** decide whether
concurrent Java is in scope. If it is not, `synchronized` should raise a
diagnostic rather than being silently dropped; if it is, this needs a real
`Mutex` representation.

**B6. Exception semantics.** `finally` interacting with `break`/`continue`
(not just `return`, which was checked and is correct); exception from a
`finally` block replacing the in-flight one; `try-with-resources` close order
and suppressed exceptions; stack unwinding through `Result` propagation.

---

## C. Object model and dispatch — largest design surface

> **Reconciliation 2026-09-22.** This section is covered by completed audits:
> interface defaults by 0025; anonymous/local/inner capture by 0002, 0023,
> 0033, and 0037; hierarchy dispatch and `super` by 0058 and 0066–0069;
> overload identity by 0009, 0065, and 0067; and enum constant bodies,
> constructors, abstract dispatch, `values()`, `valueOf()`, ordering, and
> comparison by 0059 plus their generated-crate and differential gates. The
> entries below are historical hypotheses, not active work.

**C1. Interface default methods.** Not probed at all. Java allows a default
implementation with `super` calls to specific interfaces
(`Iface.super.method()`). Interacts with the trait/struct split already
implicated in [0017](claude_audits/0017-generic-bounds-dropped-and-method-type-params-undeclared.md)'s
`E0404 expected trait, found struct`.

**C2. Anonymous and local classes, and lambda capture.** Inner-class
construction is now known to panic at runtime
([0023](claude_audits/0023-inner-class-construction-always-panics.md)); the
*access* side (`__outer.borrow()`) is already correct. Still unprobed:
**anonymous** classes, **local** classes, and lambda capture of mutable locals
(Java requires effectively-final; Rust closures do not, so the shapes differ).

**C3. Static/dynamic dispatch and `super`.** Corpus already reports
`unsupported Java constructor: super(...) requires inheritance lowering`
(18 occurrences in biojava, 6 in htsjdk) — an explicit, correctly-diagnosed
gap. Worth auditing what shapes are covered vs refused.

**C4. Overload resolution beyond arity.** [0009](claude_audits/0009-overload-identity-collapses-onto-rust-name.md)
found name+arity collision. Not yet probed: same-arity overloads distinguished
only by parameter *type* (`f(int)` vs `f(String)`), and overloads across an
inheritance chain. The existing key is `(String, usize)`, so same-arity
overloads are a strong suspect.

**C5. Enums with bodies, abstract methods, and `values()`/`valueOf()` ordering.**

---

## D. Whole-program behaviour

> **Reconciliation 2026-09-22.** D1 landed under 0018; D2's silent-facade
> census and loud fallback policy landed under 0016/0018/0050; and D3's marker,
> failure-name, and corpus compile censuses landed under 0018/0052/0053. These
> are maintained gates rather than outstanding audits.

**D1. Differential testing harness.** Not an audit — the *infrastructure* that
makes all of section B cheap. A dozen deterministic entry points, run under
both Java and the translated Rust, output diffed. This is the single highest-
value item in this document, because it converts the entire silent class from
"found by hand-probing" into "found automatically".

**D2. Facade coverage census.** Which JDK/library types are no-op stubs, ranked
by call-site count across all corpora.
[0016](claude_audits/0016-core-jdk-facades-are-no-op-stubs.md) found
`StringBuilder`, `FastMath`, `Arrays`, `Pattern` by inspection of one corpus;
a census would give the ranked list. Cheap to build from the existing
generated crates:

```sh
grep -rh 'Default::default() }$\|self.clone() }$' <crate>/src/lib.rs
```

**D3. Dropped-construct census.** Once the silent-drop prohibition
([0018](claude_audits/done/0018-the-compile-error-metric-cannot-see-silent-failures.md)
item 1) is in place, re-run every corpus and count what appears. That number is
the honest measure of current coverage, and today it is unknown.

---

## Recommended next three actions

> **Current status 2026-09-22.** The recommendations below have all been
> completed or superseded by the reconciliation notes above. New audit work
> should start from a fresh corpus/differential discrepancy rather than treating
> this historical list as open inventory.

1. ~~Verify `InnerCapture` semantics (C2)~~ — **done, and it hit**:
   constructing any non-static inner class panics at runtime
   ([0023](claude_audits/0023-inner-class-construction-always-panics.md)).
   It also raises the priority of
   [0002](claude_audits/0002-object-aliasing-has-no-representation.md): the
   missing shared representation does not merely cause aliasing bugs, it blocks
   inner classes outright.
2. **One probe batch for section B** — `HashMap` ordering (B5), evaluation order
   (B3), numeric promotion (B2), `MIN_VALUE / -1` (B4). Roughly an hour, and
   based on the hit rate so far, expect two to four findings.
3. **Check whether nullability annotations are discarded** (A3) — an
   *opportunity* rather than a bug: the codebase infers nullability with
   considerable machinery while the source may be stating it outright.

Sections C and D are larger and should follow the Tier 0 fixes in
`claude_audits/SUGGESTIONS.md`, because D1 and D3 depend on the silent-drop
prohibition being in place to be meaningful.
