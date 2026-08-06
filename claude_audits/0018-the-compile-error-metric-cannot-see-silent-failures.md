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

---

## Update 2026-08-05 (third pass) — the coverage counters, and one that was 97% noise

The three items this file asks for stand at: **1 done, 2 done with one exception
recorded below, 3 done** (15 differential fixtures, run through `javac`+`java`
against `rustc`+run on every gate).

Item 2 asked for five counters. Four exist in `summary.tsv`
(`parse_failures`, `unmodelled_types`, `unsupported_statements` — this file's
`dropped_statements` — and `noop_facade_methods`), plus `unsupported_members`,
`unsupported_expressions`, `unmodelled_call_sites` and `stubbed_constants`
which it did not ask for.

### `noop_facade_methods` was mostly measuring something else

On a program whose only Java API use is `StringBuilder`, the counter reported
**80** — of which **78 were synthesised `From` impls**:

```rust
impl From<ZipOutputStream> for OutputStream {
    /// Unmodelled Java upcast: ZipOutputStream -> OutputStream.
    fn from(_value: ZipOutputStream) -> Self { Default::default() }
}
```

These are erasure — a widened value silently becomes a fresh empty one — and
worth counting. But they are a *different* phenomenon from a modelled API that
does nothing, with a different fix, and they drowned it 39:1. A coverage counter
that is 97% a different signal cannot make erasure visible; it hides it in a new
way, which is this file's own thesis turned on one of its own remedies.

They are now `unmodelled_upcast_conversions`, counted separately and reported in
`summary.tsv`. `noop_facade_methods` drops from 80 to **2** on that program.

Worth noting for whoever picks this up: the upcast surface is emitted for
**every** translation regardless of what the program uses — a Java file that
touches no API at all still gets all 78. So it is a constant offset, not a
per-file cost, and `unmodelled_upcasts_are_not_counted_as_noop_facade_methods`
asserts exactly that so the claim is checkable.

### Two implementations of one metric, on opposite sides of a language boundary

The same drift this module was built to prevent existed one level up:
`scripts/sweep-generated-idioms.py` (which feeds `summary.tsv`) and
`output_markers::count_noop_facade_methods` (which feeds the analysis profile)
are **independent detectors for the same named signal**, and they disagreed —
145 vs 130 on the htsjdk corpus. A shared constant cannot fix this, because one
side is Python.

So the agreement is asserted instead:
`the_python_sweep_and_the_rust_counters_agree_on_shared_signals`
(`tests/idiom_sweep_script.rs`) translates one Java source, runs the real sweep
script over the output, and requires the two to produce the same number for
every shared signal.

### `noop_facade_call_sites` — now built, and the earlier reasoning was half wrong

The previous pass recorded this as unbuildable from generated text. That was
half right, and the half that was wrong is the useful part.

The obstacle named — *the surviving no-op methods are called `to_string`,
`clone`, `into`, so counting `.to_string(` would count mostly unrelated calls* —
is real for **methods**. It does not apply to **constructors**, which are called
through a qualified path: `Foo::new_2(…)` cannot collide with anything.

So the counter counts what is soundly countable. For every facade constructor in
a file whose body is `Default::default()`, it counts calls to that exact
qualified path. Both halves come from one file, so there is no cross-file
inference and no name guessing — and a *modelled* type is excluded by
construction, because its constructor has a real body.

Measured: `java.awt.Font` is genuinely unmodelled, so three `new Font(…)` in one
method give **one** definition and **three** call sites. That ratio is the whole
point — a definition count reports 1 for a cost of 3, which is the "erasure
looks like a saving" problem this file is about. On a real corpus file the
counter reports 5 definitions and 1 call site, so it distinguishes the two
directions rather than tracking one.

Reported as `noop_facade_call_sites` in the analysis profile alongside
`noop_facade_methods`.

**What is still not counted**, stated so the number is not read as complete:
no-op facade *methods* — the `to_string`/`clone` population — remain uncounted at
their call sites, and doing them soundly still needs a diagnostic at facade-call
lowering rather than a text match. The counter is a lower bound on the cost, and
the constructor share of it is exact.

### The original reasoning, kept for the record

The one counter from item 2 that does not exist, and the reason is worth
recording rather than leaving as a silent omission.

It cannot be recovered from generated text. The no-op methods that remain after
the split are named `to_string`, `clone`, `into` — names that collide with
Rust's own, so counting `.to_string(` would count mostly unrelated calls. That
is precisely the kind of detector this file argues against: one whose number
looks like a measurement and is not.

Doing it soundly means recording the fact at **lowering** time, where the
translator knows a call resolved to a no-op facade, rather than recovering it by
regex afterwards. That is the same shape as `unmodelled_call_sites`, which is
emitted (`unimplemented!("… is not modelled")`) rather than inferred, and which
works. The work is a diagnostic at the facade-call lowering site; it is not
started.

### Measured on the corpus

`htsjdk_vcf`, 32 files, with the htsjdk `variant`/`samtools` slice as a
context-summary source:

| | before | after |
|---|---:|---:|
| `noop_facade_methods` | 145 | **130** |
| `unmodelled_upcast_conversions` | (not counted) | 0 in the swept root, **68** in the crate |

The 15 removed were never facade methods: struct-field initialisers and local
defaults, matched because the old regex looked for `Default::default()`
*anywhere* in the file —

```rust
let mut __java_self = VCFHeader { vcf_header_version: Default::default(), … };
let mut i: i32 = Default::default();
```

One more gap found and **not** fixed here, recorded so it is not mistaken for a
zero: the sweep runs over `generated/*.rs` only, and the crate's shared support
module is where the upcast conversions live — hence `0` swept against `68`
present. Every signal the sweep reports is therefore a lower bound on anything
that can appear in support code.

### The gate itself was silently reporting the wrong numbers

Found by accident, and it belongs in this file because it is the same failure
one more level up: **the measurement apparatus can be wrong for reasons that
have nothing to do with the thing measured.**

`idiom_gaps` writes each generated crate to `/tmp` and — deliberately — keeps
the ones that *fail*, so they can be inspected. Reasonable. But it kept them
**whole**, including `target/`, and with ~146 failures per run and no bound on
runs the leftovers reached **10,525 directories / 31 GB** and filled the root
filesystem.

The symptoms, in the order they appeared:

1. a gate run came back `537/147` against a `538/146` baseline — one test,
   easily mistaken for a real regression from the change under test;
2. a direct re-run of the same suite, same tree, reported **280 passed / 404
   failed**. Nothing had changed but the free space.

A one-test delta is exactly the size of signal this whole audit set treats as
meaningful, so the near miss is the point: the number moved for a reason
invisible in the diff.

Fixed by keeping what is actually inspected and dropping what is regenerable —
`Cargo.toml` and `src/` stay, `target/` goes. A failing crate still reproduces
by running `cargo` in its directory. Cost of a full run drops from ~450 MB to
**1.9 MB**, measured, with the suite still at `538/146`.

## Update 2026-08-06 (Codex follow-up) — the constructor counter missed generic impl owners

The `noop_facade_call_sites` counter handled `impl Owner { ... }`, but generic
facades are emitted as `impl<T> Future<T> { ... }`. That spelling has no
whitespace after `impl`, so the scanner skipped the no-op constructor definition
and then undercounted every qualified call to it.

The Rust counter and the Python sweep now parse inherent `impl` owners with or
without generic parameters, and strip the owner's generic arguments before
matching `Owner::new_N(...)` call sites.

The focused-package script also no longer reports zero silently when
`analysis-profile.tsv` is absent. Legacy `--no-batch-translate` runs do not have
the batch analysis profile, so `noop_facade_call_sites` falls back to the idiom
sweep for that metric.

## Update 2026-08-06 (Codex follow-up) — the call-site count is now boundary-aware

The constructor call-site counter was still a raw substring match. That could
count `Font::new_3` inside comments or strings, and could count the suffix of
`MyFont::new_3` as a `Font::new_3` call. The wording above called constructor
call sites exact; that was too strong for the implementation.

The Rust counter and Python sweep now ignore line comments and quoted strings
for this metric and require the matched owner path to start on an identifier
boundary. The focused-package summary fallback also now handles the stale-profile
case: if `analysis-profile.tsv` exists but lacks `noop_facade_call_sites`, the
wrapper uses the idiom sweep instead of returning silent zero.
