<!-- Update 2026-08-03 at the end of this file: the harness found 0034, and the
     invalid-pattern metric was looking for a string nothing produced. -->

# 0018 — The compile-error metric is blind to the most severe defects, and rewards erasure

Status: **validated by construction** (every claim below is a confirmed issue in this directory)
Severity: process — it shapes what gets fixed
Kind: measurement

## The claim

The project's primary quality signal is `cargo_check_errors` in
`summary.tsv` / `top-compile-errors.tsv`. Of the fourteen defects validated in
this directory, **eight produce zero compile errors**:

| issue | what happens | compile errors |
|---|---|---|
| [0014](done/0014-labelled-statements-are-silently-deleted.md) | labelled loops **deleted from the output** | 0 |
| [0015](done/0015-static-initializer-blocks-are-silently-dropped.md) | `static { … }` blocks dropped | 0 |
| [0016](done/0016-core-jdk-facades-are-no-op-stubs.md) | `StringBuilder`, `FastMath`, `Pattern` do nothing | 0 |
| [0004](done/0004-string-concat-absorbs-nested-arithmetic.md) | `"t " + (a+b)` → `"t 12"` not `"t 3"` | 0 |
| [0005](done/0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md) | `>>>` behaves as `>>` | 0 |
| [0003](done/0003-java-integer-arithmetic-semantics-not-preserved.md) | overflow panics in debug, wraps in release | 0 |
| [0009](0009-overload-identity-collapses-onto-rust-name.md) | child's method body **discarded** | (the body loss itself is silent) |
| [0002](0002-object-aliasing-has-no-representation.md) | alias → copy changes results | (semantic half is silent) |

A corpus can therefore reach zero errors while the program deletes loops, skips
class initialisation, computes with a no-op `FastMath`, and builds empty
strings. FastQC calls `StringBuilder` no-ops **181 times across 157 files** and
that contributes nothing to its error count.

## The worse problem: the metric can be improved by erasing

The failure modes are not merely invisible — several of them *reduce* the error
count:

- Emitting a no-op facade instead of a real implementation removes every type
  error that a real signature would have produced ([0016](done/0016-core-jdk-facades-are-no-op-stubs.md)).
- Dropping an unsupported statement removes every error its lowering would have
  raised ([0014](done/0014-labelled-statements-are-silently-deleted.md), [0015](done/0015-static-initializer-blocks-are-silently-dropped.md)).
- Erasing an unmodelled type to `Box<dyn Any>` compiles at the construction site
  ([0011](done/0011-dyn-any-fallback-fails-at-every-use.md)).

So the gradient the metric defines does not always point at a better translator.
This is worth stating plainly because `.tmp/` holds **3569 corpus-rerun
directories**, each named after one fix — an enormous amount of work steered by
this signal.

## Also: parse failures silently void a file's error count

A file that fails to **parse** reports zero *type* errors, so its real problems
are invisible and its corpus total is not comparable with others. Two such files
exist today ([0012](done/0012-unsupported-diagnostics-emit-invalid-rust.md),
[0013](done/0013-nested-type-names-unmangled-in-downcast.md)) — one of them created
by the diagnostic mechanism itself.

## What to add

Compile-error count is a *necessary* gate, not a sufficient one. Three additions,
cheapest first:

1. **Silent-drop prohibition (cheapest, highest value).** Every construct the
   translator does not handle must appear in the output as `compile_error!`.
   Add a test that an unsupported construct produces a diagnostic. This converts
   0014 and 0015 from invisible into loud, and prevents the whole class.
   *A translator may refuse to translate; it must never quietly omit.*
2. **Coverage counters in `summary.tsv`**: `noop_facade_methods`,
   `noop_facade_call_sites`, `unmodelled_types`, `dropped_statements`,
   `parse_failures`. Track them alongside the error count so erasure is visible
   as a cost rather than invisible as a saving.
3. **Differential testing.** Compile and run both the Java and the Rust for a
   small set of corpus entry points and diff the output. This is the only
   technique that finds 0003/0004/0005/0016 at all — every one of them was found
   by running the program, and none by compiling it.

Item 3 is the real answer, and it need not be expensive to start: a dozen
`main`-style entry points with deterministic output would have caught six of the
eight silent defects above. The probe method used for this audit — write a
ten-line Java file, translate, compile, run, diff against Java — found five
silent defects in about two hours.

## Recommendation

Keep driving work from the corpus; it is a good signal. But treat
"errors: 0" as "compiles", not as "works", and add item 1 before the next round
of corpus fixes — otherwise each fix risks being scored by a metric that a
silent drop would score better.


## Update 2026-08-02 — differential harness landed

`tests/differential.rs` runs each fixture in `tests/differential/` through
**both** toolchains — `javac`+`java` for the oracle, translate+`rustc`+run for
the subject — and compares stdout. Skips cleanly if a toolchain is missing.
Eight fixtures cover the silent classes this issue is about: integer overflow
and shift semantics, string concatenation, `StringBuilder`, switch fallthrough,
labelled `continue`, static initializer ordering, `Integer.MAX_VALUE`/`Math.PI`,
overload/override dispatch, exceptions with `finally`, and generic bounds.

It found a real defect on **every** fixture batch added:

| found | defect |
|---|---|
| Arithmetic | negated integer literals left as `{integer}` — ambiguous `wrapping_shr`/`wrapping_rem`; `(-1) as u32` inferred the literal as unsigned then negated it; `200 as i8` rejected as out of range |
| Overloads | a `double` concatenated as `3` where Java prints `3.0` |
| Exceptions | `?` applied to the whole `format!` of a concatenation whose segments already propagate |
| Generics | a Java class named `Box` aliased over `std::boxed::Box`, making `JavaException` infinitely sized |
| Generics | generic values render as `""` — recorded in `tests/differential/known-gaps/` |

All fixed except the last, which is filed with its diagnosis. Note none of these
would have been caught by a compile-error count: three produced clean-compiling
wrong output, and two only surfaced because a second toolchain disagreed.

`tests/differential/known-gaps/` holds fixtures that reproduce an open defect;
they are excluded from the run so the harness stays usable as a gate. Moving one
back up a level is the regression test for its fix.

---

## Update 2026-08-03 — the harness keeps paying, and a guard was miscalibrated

`tests/differential.rs` has now found a defect in every batch of fixtures added
to it. The latest is [0034](done/0034-object-array-allocation-produces-an-empty-vec.md):
`new Node[1]` for a reference element type allocates an *empty* `Vec`, so the
first index panics.

What makes 0034 worth recording here rather than only in its own file is that
**two independent detectors missed it, for two different reasons**:

1. **The corpus does not exercise it.** Zero of the 196 files under `examples/`
   reach the object-array fallback. A metric computed over that corpus cannot
   see a defect the corpus never triggers, no matter how the metric is defined.
   This is a distinct failure mode from the ones catalogued above — not "the
   metric cannot see silent failures" but "the sample cannot see the idiom".

2. **The guard was looking for a string nothing produced.**
   `count_invalid_patterns` (`src/main.rs`) matches
   `"unsupported Java array creation"`; the emitted marker read
   `"Java array creation"`. So the fallback was emitted on every object-array
   allocation, and counted as a clean translation. Fixed by making the marker
   match.

That second one generalises. `count_invalid_patterns` is a list of **twelve
string literals** that must each stay in sync with a marker emitted somewhere
else in the codebase, with nothing checking the correspondence. One had already
drifted. The others are unverified:

```rust
line.contains("compile_error!")
    || (line.contains("downcast::<") && line.contains(".expect(\"invalid Java cast\")"))
    || line.contains(".add_all(")
    || line.contains(".put(")
    || line.contains("unsupported Java lambda")
    || line.contains("unsupported Java method reference")
    || line.contains("unsupported Java array creation")
    || …
```

**Recommended follow-up**: give each marker a named constant that both the
emitter and the detector use, or a test that translates one fixture per marker
and asserts the pattern count rises. A metric whose detectors can silently stop
matching is worse than no metric, because it reports success.

### Known-gap fixtures

Fixtures reproducing an open defect live in `tests/differential/known-gaps/`,
which the harness does not read, so it stays usable as a gate. Current contents:

| fixture | issue | java | translation |
|---|---|---|---|
| `Generics.java` | erased-display no-op | `5,true,false,11` | `,true,false,` |
| `Aliasing.java` | [0002](0002-object-aliasing-has-no-representation.md) | `5\|7\|3\|9` | `0\|0\|0\|0` |
| `ObjectArray.java` | [0034](done/0034-object-array-allocation-produces-an-empty-vec.md) | `4,3,1,ab` | panic |

Moving one up a level is the regression test for its fix.

---

## Update 2026-08-03 (second pass) — the detectors are wired to their emitters, and two more had drifted

The follow-up this file recommended — *"give each marker a named constant that
both the emitter and the detector use, or a test that translates one fixture per
marker and asserts the pattern count rises"* — is done, and doing it found more
of exactly what it predicted.

### What the audit of the twelve string literals found

`src/output_markers.rs` now owns every marker's text; `src/main.rs` holds no
literal of its own, and the counters moved into the library so they can be
tested at all. Auditing the list turned up **two further dead detectors**, on top
of the array-creation one already known:

| clause | emitted text | status |
|---|---|---|
| `"unsupported Java array creation"` | `"Java array creation"` | drifted — the 0034 case |
| `"unsupported Java lambda"` | `"unsupported Java expression: lambda"` | **never matched anything** |
| `"unsupported Java method reference"` | `"unsupported Java expression: method reference"` | **never matched anything** |

The last two were redundant as well as misspelled: those messages are emitted
inside `compile_error!`, which the first clause already counts. Both clauses are
deleted rather than corrected — a second, differently-spelled detector for text
another clause already matches is the drift this module exists to prevent.

Also found: the lowering arms behind them are **unreachable**. `lower_expr`
handles `Expr::Lambda` and `Expr::MethodReference` before consulting the
unsupported fallback, so `UnsupportedExpressionLowering::Lambda` cannot be
produced. A detector for a marker no code path can emit is worse than no
detector.

### The guard, and proof it has teeth

`tests/output_markers.rs` requires every registry entry to have either a Java
fixture or a named dedicated test, translates each fixture, and asserts both
that the marker text appears **and** that the corresponding counter rises above
a clean-translation baseline.

Verified by mutation, which is the part that matters:

- Renaming the shared constant does **not** fail the suite — correct, because
  the emitter and detector move together. That is what a shared constant buys.
- Making the *lowering* stop emitting its marker **does** fail, with
  `marker unsupported_java_array_creation did not raise invalid_patterns: 0 vs
  baseline 0`. That is the 0034 failure mode, and it is now caught.

### A metric that could not terminate

The largest behavioural suite could not produce a baseline at all: 7 of 684
`idiom_gaps` tests hung forever ([0035](done/0035-idiom-gaps-suite-never-terminates.md)),
so regressions in it were undetectable by construction. Fixed at the cause — a
`while … {}` fixpoint that derived "changed" from the *intent* to add a facade
method rather than from the outcome. The suite now returns in ~8 minutes, and
its failure-name set is checked in as `tests/baselines/idiom-gaps-failures.txt`.
