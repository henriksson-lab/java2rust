# 0031 — Census: every stubbed constant, and why `0` is the worst placeholder

Status: **validated** (measured across three corpora)
Severity: **critical** — `Math.PI` is `0.0` in generated code
Kind: facade constant coverage; broader than
[0024](done/0024-numeric-limit-constants-stubbed-to-zero.md)

[0024](done/0024-numeric-limit-constants-stubbed-to-zero.md) reported the four
numeric-limit constants found by probing. This is the **complete census**, and
the class is considerably larger and worse.

## Full census

Measured with:

```sh
awk '/^impl [A-Za-z0-9_]+ \{/{owner=$2}
     /pub const/{if($0 ~ / = (0|0i64|0i32|0\.0f64|0\.0f32|0u8|false|Default::default\(\))/)
                  print owner"."$3" = "$6}' <crate>/src/lib.rs
```

### FastQC — 18 stubbed constants

```
Double.MAX_VALUE            = 0.0     Float.MAX_VALUE            = 0.0
Double.MIN_VALUE            = 0.0     Float.MIN_VALUE            = 0.0
Double.NEGATIVE_INFINITY    = 0.0     Float.NEGATIVE_INFINITY    = 0.0
Double.NaN                  = 0.0     Float.NaN                  = 0.0
Double.POSITIVE_INFINITY    = 0.0     Float.POSITIVE_INFINITY    = 0.0
Integer.MAX_VALUE           = 0       Long.MAX_VALUE             = 0
Integer.MIN_VALUE           = 0       Long.MIN_VALUE             = 0
Math.E                      = 0.0     Math.PI                    = 0.0
JavaAwtFont.BOLD            = 0       JavaAwtFont.PLAIN          = 0
```

### htsjdk — 8

```
Double.NaN                                  = 0.0
Double.POSITIVE_INFINITY                    = 0.0
Double.NEGATIVE_INFINITY                    = 0.0
VariantContext.NO_LOG10_PERROR              = 0.0
SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH   = 0
BCF2Codec.SIZEOF_BCF_HEADER                 = 0
HtsjdkSamtoolsUtilIOUtil.GZIP_HEADER_READ_LENGTH = 0
HtsjdkVariantUtilsGeneralUtils.DEBUG_MODE_ENABLED = false
```

### biojava — 1 (`Integer.MIN_VALUE`)

## Three distinct groups, three different fixes

### 1. Mathematical constants — `Math.PI = 0.0`, `Math.E = 0.0`

In a corpus of statistics and sequence analysis, `Math.PI == 0` makes every
Gaussian, geometric, and trigonometric computation silently produce garbage.
Nothing fails; numbers are just wrong. This is the single worst individual
entry in this census.

Fix: `std::f64::consts::PI`, `std::f64::consts::E`. Trivial.

### 2. Float special values — `NaN` and the infinities as `0.0`

Worse than merely wrong, because they invert control flow:

```java
double best = Double.NEGATIVE_INFINITY;
for (double x : xs) if (x > best) best = x;     // becomes `x > 0.0` — negatives never win

if (score == Double.NaN)   // Java: always false (NaN != NaN)
                           // emitted: `score == 0.0` — true whenever score is 0
```

Sentinel initialisation, saturation guards, and "is this unset" checks all
break, and `NaN`-as-`0.0` turns an always-false comparison into a
sometimes-true one.

Fix: `f64::NAN`, `f64::INFINITY`, `f64::NEG_INFINITY`, `f32` equivalents.

Note the interaction with the project decision that **Java floating-point
semantics need not be copied** (2026-07-30). That decision is about
*arithmetic* behaviour — rounding, `strictfp`, `-0.0`. It does not license
`NaN == 0.0`: these are constant *values*, and Rust's IEEE-754 `f64` already
provides the right ones. Getting them right costs nothing and is independent of
that decision.

### 3. Domain constants from source-adjacent libraries

`NO_LOG10_PERROR`, `UNKNOWN_SEQUENCE_LENGTH`, `SIZEOF_BCF_HEADER`,
`GZIP_HEADER_READ_LENGTH`, `DEBUG_MODE_ENABLED`. These are **not JDK** — they
belong to classes in the corpus's own dependency set, and they are being
stubbed rather than translated.

`GZIP_HEADER_READ_LENGTH = 0` is a read length of zero. `SIZEOF_BCF_HEADER = 0`
is a header size of zero. These will produce empty reads and mis-parsed binary
formats, silently.

This group matters most for the *design*: it shows the fallback is not confined
to unmodelled third-party surfaces. Any constant the resolver fails to reach —
including from co-translated code — silently becomes `0`.

## The general rule this argues for

**A constant with no known value must be a translator error, never a default.**

Unlike a method, a `const` cannot fail at its use site: there is no body to put
`unimplemented!()` in. The value is baked in at compile time and is
indistinguishable from a real one. And `0`/`false` are the worst possible
placeholders, because they are *plausible* — they pass type checking, they pass
review, and they change behaviour rather than stopping it.

Concretely:

1. Whatever generates facade shells must **refuse** to emit a `const` it has no
   value for, and raise a diagnostic naming the owner and field.
2. If a placeholder is unavoidable for compilation to proceed, it must be
   poisoned rather than plausible — e.g. `const X: i32 = panic!("…")` in a const
   context, or a getter function returning `unimplemented!()` instead of a
   `const`. Failing loudly at first use beats silently computing with `0`.
3. Add `stubbed_constants` to `summary.tsv`. Today the FastQC crate ships 18 of
   these and reports zero compile errors.

## Priority

The fix for groups 1 and 2 is a lookup table of about 30 entries and is the
highest value-per-effort item in the entire audit set: it is smaller than
[0004](done/0004-string-concat-absorbs-nested-arithmetic.md), and `Math.PI = 0`
corrupts arithmetic everywhere it appears.

Group 3 needs resolver work and shares a root cause with
[0016](done/0016-core-jdk-facades-are-no-op-stubs.md).

## Detection test worth adding

Cheap and catches the whole class permanently:

```
assert no generated `pub const` has a value of 0, 0.0, false, or Default::default()
unless the Java source declares that value
```

Plus value assertions for the mathematical ones:

```java
assert Math.PI > 3.14 && Math.PI < 3.15;
assert Double.POSITIVE_INFINITY > Double.MAX_VALUE;
assert Double.NaN != Double.NaN;          // must stay false-comparing
assert Double.MIN_VALUE > 0;              // smallest positive, not f64::MIN
```
