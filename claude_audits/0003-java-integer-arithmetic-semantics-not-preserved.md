# 0003 — Java integer arithmetic semantics are not preserved

Status: **PARTIAL** — was: validated (runtime-confirmed divergence)
Severity: medium-high — wrong results or panics, and the behaviour differs
between debug and release builds
Kind: missing lowering policy (needs a fact + a decision, not a big analysis)


## Resolution

**PARTIAL (audited 2026-08-01).** The current tree has substantial integer
semantic coverage: primitive binary operators use wrapping projections in the
common int/long paths, shift distances are masked, local integral updates wrap,
compound assignment narrowing is shared, and fallible division/remainder can
flow through the shared Result/uncaught-panic policy in several slot contexts.
Keep this audit active until unchecked `ArithmeticException` propagation is
uniform across all expression positions and corpus fixtures cover nested
numeric/comparison/logical/ternary/control-flow cases.

## Symptom

Java integer arithmetic is defined to wrap on overflow (JLS 15.18.2) and to
throw `ArithmeticException` on integer division/remainder by zero (JLS 15.17.2).
Both are lowered to plain Rust operators, which have neither behaviour.

## Minimal repro

`claude_audits/repro/Overflow.java`:

```java
public class Overflow {
    public int run(int a, int b) { return a * b + a; }
    public int div(int a, int b) { return a / b; }
}
```

## Emitted Rust (verified)

```rust
pub fn run(&self, a: i32, b: i32) -> i32 {
    return (a * b) + a;
}

pub fn div(&self, a: i32, b: i32) -> i32 {
    return a / b;
}
```

## Verified divergence

`run(2000000000, 2)`:

| | result |
|---|---|
| Java | `1705032704` |
| emitted Rust, debug profile | **panics**: `attempt to multiply with overflow` |
| emitted Rust, release profile | `1705032704` (accidentally correct — release wraps) |

Confirmed by compiling the emitted file with a `main` and running it:

```
thread 'main' panicked at ovf_main.rs:1671:16:
attempt to multiply with overflow
```

`div(1, 0)`:

| | result |
|---|---|
| Java | throws `ArithmeticException: / by zero` |
| emitted Rust | panics `attempt to divide by zero` (not catchable as a Java exception) |

The overflow case is the more dangerous of the two, because correctness depends
on the cargo profile: the translated program is right in release and crashes in
debug. Any test suite run in debug will disagree with production.

## Also validated: shift distance is not masked

Java masks the shift distance to the low 5 bits for `int` (6 for `long`), so
`1 << 33 == 2` (JLS 15.19). The emitted Rust shifts directly:

```java
public int shift(int a, int n) { return a << n; }
```
```rust
pub fn shift(&self, a: i32, n: i32) -> i32 { return a << n; }
```

| | `shift(1, 33)` |
|---|---|
| Java | `2` |
| emitted Rust, debug | **panics**: `attempt to shift left with overflow` |

Same fix mechanism as overflow: emit `a.wrapping_shl(n as u32)`, which masks
exactly as Java does.

## Verified as *correct* (do not "fix" these)

The same probe confirmed these already match Java, so leave them alone:

- `%` on negative operands: `-7 % 3 == -1` in both Java and Rust (both
  truncate toward zero).
- Narrowing casts: `(byte) 200` → `(big as i8) as i32` → `-56`. Correct.

## Scope

Applies to `+ - * / % << >> ++ --` on `int`/`long`. It also interacts with the
compound assignment desugaring already present, e.g. the emitted
`{ let __java_old_value = i; i += 1; __java_old_value }` for `i++`, which has
the same overflow exposure. Unsigned right shift `>>>` is a separate and more
serious defect — see
[0005](0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md).

## Root cause

Operator lowering (`src/operator_lowering.rs`) maps Java binary operators to
the corresponding Rust operator directly. There is no fact that records "Java
integer semantics apply here", and no policy that selects a wrapping or checked
spelling.

## Proposed machinery

This one does **not** need a new analysis — it needs an explicit, defaulted
policy, and it is a good candidate to fix early because it is cheap and
self-contained.

1. Add an arithmetic-semantics fact to the lowering inputs with, at minimum:
   - `Wrapping` (JLS-faithful): emit `a.wrapping_mul(b)`, `wrapping_add`, …
   - `Checked` (debug-friendly): emit checked ops that raise `JavaException`.
   - `Native` (current behaviour): only when the operand range is *proved*
     not to overflow.
2. Default to `Wrapping` for `int`/`long` arithmetic. It is the JLS behaviour
   and removes the debug/release split.
3. For `/` and `%` on integers, emit a divide-by-zero check that raises
   `ArithmeticException` through the existing `JavaException` machinery, unless
   the divisor is a non-zero literal. The exception plan
   (`src/exception_plan.rs`) already exists, so the throw path is available.
4. Optional refinement later: a simple interval/range analysis to prove
   non-overflow and recover the plain operator where it is safe, which is where
   `Native` earns its place. This is a genuine optimisation, not a
   prerequisite — do not block the fix on it.

## Note

Choosing `Wrapping` by default makes the emitted Rust slightly less idiomatic
(`a.wrapping_mul(b)` instead of `a * b`). That is the correct trade: the
project's stated aim is idiomatic Rust *while preserving Java semantics*, and
silent profile-dependent behaviour is not a preservation of semantics. If
readable output matters for the common case, the range analysis in step 4 is
the way to buy it back.
