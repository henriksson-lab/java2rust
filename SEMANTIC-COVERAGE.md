# Semantic coverage: what has actually been checked

**Answer to "do the findings cover all semantic gaps?" — no, and not close.**

The audits cover roughly a third of Java's semantic surface. Coverage is heavily
skewed: operators, statements and literals are well probed; conversions, arrays,
interfaces, and concurrency were barely touched until the latest pass; several
areas have **zero** probes.

This document is the map, indexed by JLS chapter so the gaps are enumerable
rather than a matter of judgement. It exists because "we found N bugs" says
nothing about what remains.

Legend: **✅ probed, correct** · **❌ probed, defect filed** · **◐ partial** ·
**⬜ never probed**


---

## Scoping decisions on record (2026-07-30)

These are **choices**, not defects. Items they cover are excluded from the map
below or reinterpreted as noted.

| # | decision | consequence |
|---|---|---|
| 1 | **Concurrency is in scope** ("yes, if possible") | `synchronized` erasure and thread-local statics become defects — [0030](claude_audits/0030-java-statics-are-thread-local.md), `CONCURRENCY-SCOPE.md`. Changes the shared-representation choice from `Rc/RefCell` to a `Local`/`Shared` policy **before** [0002](claude_audits/0002-object-aliasing-has-no-representation.md) is implemented. |
| 2 | **`==` on classes must mean pointer equality.** On `String`, value comparison is accepted **with a warning emitted into the generated code** for later audit | [0028](claude_audits/done/0028-reference-equality-lowered-as-structural-equality.md) is closed for shared source-class handles: `==` lowers through `Rc::ptr_eq`/`Arc::ptr_eq`, while `.equals()` keeps its separate override path. `String ==` remains the explicit value-comparison policy exception and emits `AUDIT(java-string-identity)`. |
| 3 | **Array/index bounds semantics need not be mimicked** — good code does not rely on them | `ArrayStoreException`, `IndexOutOfBoundsException` vs Rust panic: **out of scope**. Rust's panic is an acceptable substitute. |
| 4 | **Java floating-point semantics are not to be copied** | `strictfp`, `-0.0`, rounding-mode fidelity: **out of scope**. Does **not** cover float *constants* — `Double.NaN` and the infinities must still have their real values, see [0031](claude_audits/0031-stubbed-constant-census.md). `Math.round` half-up vs Rust's half-away-from-zero is a *library* difference, not FP semantics — flagged below as an open question. |
| 5 | **`HashMap`/`HashSet` iteration order is out of scope** | Java does not specify it. But `LinkedHashMap`, `LinkedHashSet`, `TreeMap`, `TreeSet` **do** specify order and must be preserved. |

**Resolved (decision 6): `Math.round` must be translated properly.** Confirmed
broken and filed as [0032](claude_audits/0032-java-lang-math-is-mistranslated.md),
which also found five further `Math` defects and carries tested replacement code.
Library-function contracts like `Math.round`, `Math.max` NaN propagation, and
`Math.abs` integer wrapping are therefore **in scope**, and are not covered by
decision 4 — that decision governs arithmetic and rounding *modes*, not the
documented behaviour of library methods.

---

## Ch 3 — Lexical structure

| topic | status |
|---|---|
| decimal/hex/octal/binary int literals | ◐ parsed; values not differentially checked |
| float/double literals, hex floats | ⬜ |
| underscores in literals (`1_000_000`) | ⬜ |
| char literals and escape sequences | ◐ |
| unicode escapes (`\uXXXX`) — Java processes these *before* tokenising | ⬜ **known-tricky** |
| text blocks | ❌ [0020](claude_audits/0020-text-block-content-not-processed.md) |

## Ch 4 — Types and values

| topic | status |
|---|---|
| default field values (`0`/`null`/`false`) | ✅ verified `"0/null/false"` |
| integer ranges and wrapping | ❌ [0003](claude_audits/0003-java-integer-arithmetic-semantics-not-preserved.md) |
| numeric limit constants, `Math.PI`, `NaN`, infinities | ❌ [0024](claude_audits/0024-numeric-limit-constants-stubbed-to-zero.md), [0031](claude_audits/0031-stubbed-constant-census.md) — **`Math.PI` is `0.0`** |
| reference identity `==` on shared source classes | ✅ [0028](claude_audits/done/0028-reference-equality-lowered-as-structural-equality.md) — shared source-class handles use pointer equality |
| boxing identity (`Integer` cache) | ❌ same root cause as 0028 |
| `NaN` / `±0.0` / infinity comparison | **out of scope** (decision 4) |
| `float`/`double` precision, `strictfp`, rounding | **out of scope** (decision 4) |
| `String` reference vs value equality | ◐ [0028](claude_audits/done/0028-reference-equality-lowered-as-structural-equality.md) — accepted per decision 2 with `AUDIT(java-string-identity)` diagnostics |

## Ch 5 — Conversions and promotions

| topic | status |
|---|---|
| binary numeric promotion (`byte+byte→int`, `char+int→int`) | ✅ correct |
| narrowing casts (`(byte) 200`) | ✅ correct |
| `(int)` of `NaN` / infinity | ✅ correct (`0` / `i32::MAX`) |
| compound-assignment implicit narrowing | ❌ [0027](claude_audits/0027-compound-assignment-implicit-narrowing-missing.md) |
| widening in method invocation / overload selection | ⬜ |
| boxing/unboxing conversion contexts | ⬜ |
| `long → double` precision loss | ⬜ |

## Ch 6 — Names and scoping

| topic | status |
|---|---|
| shadowing (field vs local, nested scopes) | ◐ implicated by [0006](claude_audits/0006-move-analysis-is-five-approximations-of-one-relation.md); **no direct probe** |
| qualified names, static imports | ⬜ |

## Ch 8 — Classes

| topic | status |
|---|---|
| field/instance init order | ⬜ (only the *static* side probed) |
| `static { }` blocks | ❌ [0015](claude_audits/0015-static-initializer-blocks-are-silently-dropped.md) |
| mutating a `static` field (`n++`, `n += 1`) | ❌ [0029](claude_audits/0029-static-field-mutation-emits-invalid-rust.md) — emits invalid Rust |
| overloads: name + arity | ❌ [0009](claude_audits/0009-overload-identity-collapses-onto-rust-name.md) |
| overloads: same arity, different types | ⬜ **strong suspect** — key is `(String, usize)` |
| inner class construction | ❌ [0023](claude_audits/0023-inner-class-construction-always-panics.md) |
| inner class capture *access* | ✅ correct (`__outer.borrow()`) |
| anonymous / local classes | ⬜ |
| varargs | ✅ correct |
| constructor chaining `this(...)` / `super(...)` | ◐ corpus reports explicit unsupported cases |
| `final`, instance initializer blocks | ⬜ |

## Ch 9 — Interfaces

| topic | status |
|---|---|
| default methods | ❌ [0025](claude_audits/0025-interface-default-methods-are-discarded.md) |
| `static` interface methods | ⬜ |
| `private` interface methods (Java 9) | ⬜ |
| constants in interfaces | ◐ |
| diamond / conflicting defaults, `Iface.super` | ⬜ |
| generic bounds referencing interfaces | ❌ [0017](claude_audits/0017-generic-bounds-dropped-and-method-type-params-undeclared.md) |

## Ch 10 — Arrays — **weakest area**

| topic | status |
|---|---|
| default element values | ⬜ |
| multi-dimensional / ragged arrays | ◐ `int[][]` maps to `ndarray`; unverified |
| array covariance / `ArrayStoreException` | **out of scope** (decision 3) |
| `arraycopy`, `Arrays.*` | ❌ `Arrays` is a no-op stub ([0016](claude_audits/0016-core-jdk-facades-are-no-op-stubs.md)) |
| bounds-check exception vs Rust panic | **out of scope** (decision 3) |
| `array.length` vs `len()` on non-`usize` indices | ◐ |

## Ch 11/14 — Exceptions and statements

| topic | status |
|---|---|
| `switch` fallthrough | ✅ verified across 4 inputs |
| labelled `break`/`continue` | ❌ [0014](claude_audits/0014-labelled-statements-are-silently-deleted.md) |
| `assert` | ❌ [0014](claude_audits/0014-labelled-statements-are-silently-deleted.md) |
| `try`/`finally` with `return` in `try` | ✅ correct |
| `return` inside `finally` | ✅ correctly *diagnosed* as unsupported |
| `finally` with `break`/`continue` | ⬜ |
| exception thrown from `finally` replacing in-flight | ⬜ |
| try-with-resources close order, suppressed exceptions | ⬜ |
| checked/unchecked propagation | ❌ [0010](claude_audits/0010-fallibility-is-decided-three-times-and-disagrees.md) |
| `switch` on `String` / enum / `null` selector | ⬜ |
| enhanced `for` over `Iterable` vs array | ◐ |

## Ch 15 — Expressions

| topic | status |
|---|---|
| string concatenation typing | ❌ [0004](claude_audits/0004-string-concat-absorbs-nested-arithmetic.md) |
| `>>>` | ❌ [0005](claude_audits/0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md) |
| shift-distance masking | ❌ [0003](claude_audits/0003-java-integer-arithmetic-semantics-not-preserved.md) |
| argument evaluation order (left-to-right) | ✅ correct |
| **assignment target vs RHS order** | ❌ [0026](claude_audits/0026-assignment-evaluation-order-differs-from-java.md) |
| `instanceof` with pattern binding | ❌ [0021](claude_audits/0021-instanceof-pattern-variable-not-bound.md) |
| `switch` expressions | ❌ [0019](claude_audits/0019-unhandled-expressions-emit-raw-java-source.md) |
| conditional operator (`?:`) result typing | ⬜ |
| `&&`/`||` short-circuit with side effects | ⬜ |
| binary operand evaluation order | ⬜ |
| `%` and `/` on `MIN_VALUE` | ⬜ (earlier probe was masked by [0024](claude_audits/0024-numeric-limit-constants-stubbed-to-zero.md)) |
| lambdas, method references | ◐ parsed; semantics unverified |

## Ch 12 — Execution

| topic | status |
|---|---|
| class init triggered on first use, init order | ❌ [0015](claude_audits/0015-static-initializer-blocks-are-silently-dropped.md) |
| initialisation circularity, default-value reads | ⬜ |

## Ch 17 — Threads and locks — **zero coverage**

| topic | status |
|---|---|
| `synchronized` methods / blocks | ❌ erased, no lock — in scope per decision 1; see `CONCURRENCY-SCOPE.md` |
| `static` fields shared across threads | ❌ [0030](claude_audits/0030-java-statics-are-thread-local.md) — `thread_local!` gives each thread its own copy |
| `volatile`, `wait`/`notify`, `java.util.concurrent` | ⬜ |

---

## Honest summary

- **Well covered**: operators and arithmetic, statement forms, parser coverage
  (that one dimension *is* exhaustive, via the grammar diff in
  `AUDIT-BACKLOG.md`).
- **Newly opened, more to find**: conversions, interfaces, evaluation order.
  The latest pass ran 7 probes and produced 4 defects — the hit rate is not
  falling yet.
- **Barely touched**: arrays (Ch 10), instance initialisation order, lambdas and
  method references, anonymous/local classes.
- **Zero coverage**: concurrency (Ch 17), unicode escapes, most floating-point
  edge cases.

All scoping questions raised earlier have been answered — see the decisions
table at the top. One new question is open: `Math.round` half-up (decision 4).

## Why the count of found bugs is not a coverage measure

30+ probes have produced 20 validated defects. That ratio has stayed roughly
constant across four batches, which means probing is still in the regime where
**new areas yield new defects on first contact**. Every area marked ⬜ above
should be assumed to contain defects at a similar rate until probed.

The only way out of that regime is item **D1** in `AUDIT-BACKLOG.md`:
differential testing against real Java execution, so coverage stops depending on
someone thinking to write the probe.
