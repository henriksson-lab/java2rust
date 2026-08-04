# 0024 — `Integer.MAX_VALUE` and friends are stubbed to `0`

Status: **validated** (runtime-confirmed; live in the FastQC corpus)
Severity: **critical** — silently breaks the most common bounds idiom in Java
Kind: facade constant coverage; trivially fixable

## Symptom

The numeric limit constants are emitted as `0`:

```rust
impl Integer { pub const MAX_VALUE: i32 = 0;
               pub const MIN_VALUE: i32 = 0; }
impl Long    { pub const MAX_VALUE: i64 = 0i64; }
impl Double  { pub const MAX_VALUE: f64 = 0.0f64; }
```

Live in the corpus, not only in probes:

```
$ grep -n 'MAX_VALUE: i32 = ' .tmp/corpus-batch-current-fastqc-…/generated-crate/src/lib.rs
2292:    pub const MAX_VALUE: i32 = 0;
2293:    pub const MIN_VALUE: i32 = 0;
```

## Why this is worse than a stubbed method

The canonical Java min/max idiom becomes a no-op:

```java
int best = Integer.MAX_VALUE;
for (int x : xs) { if (x < best) best = x; }     // never true: best is 0
```

```java
int best = Integer.MIN_VALUE;
for (int x : xs) { if (x > best) best = x; }     // only positives ever win
```

Both compile, run, and return wrong answers. Bounds checks, sentinel
initialisation, saturating clamps, and overflow guards all silently invert or
short-circuit. In a bioinformatics corpus — scoring matrices, alignment maxima,
quality thresholds — this is precisely the code that matters.

It was found indirectly: a probe of `Integer.MIN_VALUE / -1` returned `0`
instead of `-2147483648`. My first reading was a division-overflow bug; the
actual cause was `MIN_VALUE == 0`. Worth noting because the symptom pointed at
the wrong subsystem.

## Root cause

Same mechanism as [0016](0016-core-jdk-facades-are-no-op-stubs.md): an
unmodelled external member gets a `Default::default()`-valued shell. For a
`const`, the shell is a compile-time constant, so there is not even a runtime
failure to observe — the wrong value is baked in.

Constants are a *worse* case than methods for this fallback, because a stubbed
method could at least be made to `unimplemented!()`; a `const` cannot fail at
use. It must have the right value or not exist.

## Fix

Add the exact values to the `javalibs` model. This is a bounded, mechanical list:

| Java | Rust |
|---|---|
| `Integer.MAX_VALUE` / `MIN_VALUE` | `i32::MAX` / `i32::MIN` |
| `Long.MAX_VALUE` / `MIN_VALUE` | `i64::MAX` / `i64::MIN` |
| `Short`, `Byte`, `Character` limits | `i16`/`i8`/`char` equivalents |
| `Double.MAX_VALUE` / `MIN_VALUE` | `f64::MAX` / `f64::MIN_POSITIVE` (note: Java's `MIN_VALUE` is the smallest *positive* value, **not** `f64::MIN`) |
| `Double.POSITIVE_INFINITY` / `NEGATIVE_INFINITY` / `NaN` | `f64::INFINITY` / `NEG_INFINITY` / `NAN` |
| `Float.*` | `f32` equivalents |
| `Integer.SIZE` / `BYTES` | `32` / `4` |

The `Double.MIN_VALUE` mapping is the one to get right — `f64::MIN` is the most
negative value, while Java's `Double.MIN_VALUE` is `4.9e-324`. Mapping it to
`f64::MIN` would be a new silent bug in the opposite direction.

## Broader requirement

A stubbed `const` must be an **error, not a default**. Whatever generates facade
shells should refuse to emit a constant it has no value for, and raise a
translator diagnostic instead. There is no safe placeholder for a constant, and
`0` is the worst possible choice because it is a plausible value.

## Regression tests worth adding

```java
int best = Integer.MAX_VALUE; for (int x : new int[]{5,3,9}) if (x < best) best = x;  // 3
int worst = Integer.MIN_VALUE; for (int x : new int[]{-5,-3}) if (x > worst) worst = x; // -3
assert Double.MIN_VALUE > 0;                       // pins the MIN_VALUE mapping
assert Long.MAX_VALUE > Integer.MAX_VALUE;
```

The `Double.MIN_VALUE > 0` assertion is the cheap guard against the mapping
mistake noted above.
