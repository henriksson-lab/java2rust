# 0032 — `java.lang.Math` is mistranslated: rounding mode, overload collapse, broken names

Status: **validated** (rustc-confirmed + runtime-validated replacement code)
Severity: high — `Math.round` is a silent wrong answer; four other members fail to compile
Kind: facade modelling; the rounding part needs a helper, the rest are lowering bugs

Project decision (2026-07-30): **`Math.round` must be translated properly.**
This document gives tested replacement code.

## One probe, six defects

```java
public class S1 {
    public long   r(double x)            { return Math.round(x); }
    public int    rf(float x)            { return Math.round(x); }
    public double mx(double a, double b) { return Math.max(a, b); }
    public int    ab(int a)              { return Math.abs(a); }
    public double sq(double a)           { return Math.sqrt(a); }
    public int    fd(int a, int b)       { return Math.floorDiv(a, b); }
    public int    fm(int a, int b)       { return Math.floorMod(a, b); }
}
```

Emitted:

```rust
pub fn r(&self, x: f64) -> i64 { return (x as f64).round() as i64; }          // wrong mode
pub fn rf(&self, x: f32) -> i32 { return (x as f64).round() as i64; }         // wrong mode + E0308
pub fn mx(&self, a: f64, b: f64) -> f64 { return std::cmp::max(a, b); }       // E0277: f64: Ord
pub fn ab(&self, a: i32) -> i32 { return Math::abs(a); }                      // E0308: returns f64
pub fn sq(&self, a: f64) -> f64 { return Math::sqrt(a); }                     // ok
pub fn fd(&self, a: i32, b: i32) -> i32 { return math.floor_div(a, b); }      // E0425: `math`
pub fn fm(&self, a: i32, b: i32) -> i32 { return math.floor_mod(a, b); }      // E0425: `math`
```

```
error[E0425]: cannot find value `math` in this scope        (×2)
error[E0308]: mismatched types                             (×2)
error[E0277]: the trait bound `f64: Ord` is not satisfied
```

And the facade itself collapses the whole overload set onto `f64`:

```rust
impl Math {
    pub fn abs  <T0: Into<f64>>(value: T0) -> f64 { value.into().abs() }
    pub fn round<T0: Into<f64>>(value: T0) -> i64 { value.into().round() as i64 }
    pub fn sqrt <T0: Into<f64>>(value: T0) -> f64 { value.into().sqrt() }
}
```

### 1. `Math.round` uses the wrong rounding mode — the silent one

Java: *"the closest `long` to the argument, with ties rounding to **positive
infinity**"*. Rust's `f64::round` rounds **half away from zero**. Neither of
Rust's primitives matches Java (`round_ties_even` is Java's `Math.rint`, not
`Math.round`).

| | Java | current lowering |
|---|---|---|
| `Math.round(-2.5)` | `-2` | **`-3`** |
| `Math.round(2.5)` | `3` | `3` |
| `Math.round(-0.5)` | `0` | **`-1`** |

Every negative tie is wrong, silently. `mx`, `ab`, `fd`, `fm` at least fail to
compile; this one does not.

### 2. `Math.round(float)` returns the wrong type

Java has two overloads: `round(double) -> long` and `round(float) -> int`. Both
map to the single `-> i64` facade, so the `float` form is emitted into an
`-> i32` signature. `E0308`.

### 3. `Math.max`/`min` on floats → `std::cmp::max`

`f64` is not `Ord`, so this cannot compile. Beyond that there is a **semantic**
difference worth encoding rather than papering over:

- Java `Math.max(NaN, 1.0)` is `NaN` (NaN propagates).
- Rust `f64::max` **ignores** NaN — verified: `f64::NAN.max(1.0)` is `1`.
- Java also distinguishes `Math.max(-0.0, 0.0) == 0.0`; Rust's `max` may return
  either.

Note this is *not* covered by the "don't copy Java floating-point semantics"
decision — that decision is about arithmetic and rounding modes. `Math.max` is a
library function whose contract is explicit, and silently dropping NaN
propagation changes which branch a comparison takes.

### 4. Overload set collapsed onto `Into<f64>`

`Math.abs(int)` returns `int` in Java; the facade returns `f64`. So integer
`abs` loses its type *and* its semantics — Java's `Math.abs(Integer.MIN_VALUE)`
is `Integer.MIN_VALUE` (it wraps), while an `f64` round trip yields
`2147483648.0`.

This is [0009](0009-overload-identity-collapses-onto-rust-name.md)'s
identity-by-name problem appearing in the **facade** model rather than in source
classes: `Math.abs` has four overloads (`int`, `long`, `float`, `double`) and the
model keeps one.

### 5. `Math.floorDiv` → `math.floor_div(...)`

The owner type name is emitted **lowercased as a value**: `math.floor_div(a, b)`
references an undefined variable `math`. Some static-receiver path applied
identifier snake-casing to a type name. Two `E0425`s, and the same bug will hit
any unmodelled static method on `Math`.

Also note `floorDiv`/`floorMod` are *not* `/` and `%`: they round toward
negative infinity, so `Math.floorDiv(-7, 2)` is `-4` (not `-3`) and
`Math.floorMod(-7, 2)` is `1` (not `-1`). Rust's `div_euclid`/`rem_euclid` are
close but differ for negative divisors — `floorMod(7, -2)` is `-1` while
`7.rem_euclid(-2)` is `1`. Map these explicitly rather than by analogy.

## Tested replacement for `Math.round`

Validated against 18 `f64` and 12 `f32` cases, all passing, including the
JDK-8010430 edge case (`Math.round(0.49999999999999994)` must be `0`, not `1` —
a naive `(x + 0.5).floor()` reproduces the pre-Java-7 bug):

```rust
pub fn java_math_round_f64(x: f64) -> i64 {
    if x.is_nan() { return 0; }
    let f = x.floor();
    // x - f is exact: for |x| >= 2^52, x is integral and f == x; otherwise both
    // are representable and their difference is < 1 and exact.
    let rounded = if x - f >= 0.5 { f + 1.0 } else { f };
    if rounded <= i64::MIN as f64 { i64::MIN }
    else if rounded >= i64::MAX as f64 { i64::MAX }
    else { rounded as i64 }
}

pub fn java_math_round_f32(x: f32) -> i32 {
    if x.is_nan() { return 0; }
    let f = x.floor();
    let rounded = if x - f >= 0.5 { f + 1.0 } else { f };
    if rounded <= i32::MIN as f32 { i32::MIN }
    else if rounded >= i32::MAX as f32 { i32::MAX }
    else { rounded as i32 }
}
```

Covers Java's documented special cases: `NaN → 0`, `-∞ → MIN_VALUE`,
`+∞ → MAX_VALUE`, saturating clamp outside the integer range. (`±∞ - ±∞` is
`NaN`, the `>= 0.5` test is false, and the clamp then produces the right
bound — this is why the clamp is written on `rounded` rather than on `x`.)

## Fix list

| member | correct mapping |
|---|---|
| `round(double)` | `java_math_round_f64` above |
| `round(float)` | `java_math_round_f32` above, returning `i32` |
| `rint(double)` | `f64::round_ties_even` |
| `max`/`min` (float/double) | NaN-propagating helper, **not** `f64::max` and not `std::cmp::max` |
| `max`/`min` (int/long) | `std::cmp::max` / `min` — correct only for the integer overloads |
| `abs(int)`/`abs(long)` | `i32::wrapping_abs` / `i64::wrapping_abs` (Java wraps at `MIN_VALUE`) |
| `abs(float)`/`abs(double)` | `f32::abs` / `f64::abs` |
| `floorDiv`/`floorMod` | explicit floor-semantics helpers; **not** `div_euclid`/`rem_euclid` |
| `sqrt`, `sin`, `cos`, `exp`, `log`, `pow`, `cbrt`, `hypot` | direct `f64` methods |
| `PI`, `E` | `std::f64::consts::{PI, E}` — currently `0.0`, see [0031](0031-stubbed-constant-census.md) |

The overload set must be keyed by parameter type, not collapsed — see
[0009](0009-overload-identity-collapses-onto-rust-name.md).

## Regression tests worth adding

```java
Math.round(-2.5)   == -2;    Math.round(2.5)  == 3;
Math.round(-0.5)   == 0;     Math.round(0.5)  == 1;
Math.round(0.49999999999999994) == 0;          // must not be 1
Math.round(Float.NaN) == 0;
Math.abs(Integer.MIN_VALUE) == Integer.MIN_VALUE;
Math.floorDiv(-7, 2) == -4;  Math.floorMod(-7, 2) == 1;
Math.floorMod(7, -2) == -1;                    // where rem_euclid would give 1
Double.isNaN(Math.max(Double.NaN, 1.0));
Math.abs(-3) == 3 && Math.abs(-3) instanceof int;   // return type, not double
```

`Math` is heavily used in every corpus here (`FastMath` is a commons-math
mirror of the same surface and is itself a no-op stub —
[0016](0016-core-jdk-facades-are-no-op-stubs.md)), so this is worth doing as one
focused pass over the whole class rather than member by member.
