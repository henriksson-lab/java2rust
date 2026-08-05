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

---

## Update 2026-08-04 — groups 1 and 2 closed and guarded; group 3 remains

Status: **partial.** The two groups this file called highest-value are fixed and
now have both a value check and a shape check. Group 3 (domain constants from
source-adjacent libraries) is untouched and is what keeps this issue open.

### Groups 1 and 2 are correct in the emitted code

Verified by translating a class that touches the whole census surface:

```rust
pub const PI: f64 = std::f64::consts::PI;      pub const E: f64 = std::f64::consts::E;
pub const NaN: f64 = f64::NAN;                 pub const NaN: f32 = f32::NAN;
pub const POSITIVE_INFINITY: f64 = f64::INFINITY;
pub const NEGATIVE_INFINITY: f64 = f64::NEG_INFINITY;
pub const MAX_VALUE: f64 = f64::MAX;           pub const MIN_VALUE: f64 = f64::from_bits(1);
pub const MAX_VALUE: i32 = i32::MAX;           pub const MIN_VALUE: i64 = i64::MIN;
```

`Double.MIN_VALUE` is `f64::from_bits(1)` — the smallest *positive* subnormal,
which is what Java means, not `f64::MIN`. That is the one entry in this group
most likely to be got wrong, so it is worth naming.

### Both checks this file asked for now exist

**Values**, against the Java oracle: `tests/differential/Constants.java` was
extended from 4 assertions to 10, covering the cases this file argued are worse
than merely wrong because they *invert control flow*:

- `Double.NaN != Double.NaN` must stay **true** — as `0.0` it flips to false-ing
  the wrong way whenever the value under test is zero;
- `Double.POSITIVE_INFINITY > Double.MAX_VALUE`, and the negative mirror;
- the sentinel idiom itself: `double maxSeen = Double.NEGATIVE_INFINITY;` over
  all-negative input must return the maximum, which seeded with `0.0` it does
  not;
- `Math.E`, `Float.NaN`, `Float.POSITIVE_INFINITY`, `Long` bounds.

Java and the translation both print `3,-3,true,true,true,true,true,-3,true,true`.

**Shape**, independent of any fixture reading the value:
`jdk_constants_are_not_stubbed_to_plausible_zeros` (`tests/output_markers.rs`)
translates the census surface and fails if any emitted `pub const` is `0`,
`0.0`, `false` or `Default::default()`. This is the test this file specified,
and it catches a constant nothing happens to read.

Verified to fire, rather than assumed: mutating `Math.PI` back to `0.0` fails it
with `constants stubbed to a plausible zero: ["PI = 0.0"]`. A detector nobody has
seen fire is the failure mode
[0018](0018-the-compile-error-metric-cannot-see-silent-failures.md) is about,
and two of `count_invalid_patterns`'s clauses had exactly that shape.

### What is still open

**Group 3** — `NO_LOG10_PERROR`, `UNKNOWN_SEQUENCE_LENGTH`, `SIZEOF_BCF_HEADER`,
`GZIP_HEADER_READ_LENGTH`, `DEBUG_MODE_ENABLED`. These are not JDK constants;
they belong to classes in the corpus's own dependency set and are stubbed rather
than translated. The shape test above would catch them *if* the corpus that
produces them were translated in a test, so the guard is in place and the
lowering is not. This needs the resolver work this file already points at, and
shares a root cause with [0016](done/0016-core-jdk-facades-are-no-op-stubs.md).

The general rule this file argues for — *a constant with no known value must be
a translator error, never a default* — is enforced for unknown **primitive** and
**named** external constants, which already emit `compile_error!` with the owner
and field named (`output_markers::unmodelled_static_constant_message`). What is
missing is not the diagnostic but the resolution: group 3's owners are reachable
and simply are not being read.

---

## Update 2026-08-05 — group 3 resolved from owner facts, and the census re-measured

### What group 3 turned out to be

The five constants this file names as group 3 —
`NO_LOG10_PERROR`, `UNKNOWN_SEQUENCE_LENGTH`, `SIZEOF_BCF_HEADER`,
`GZIP_HEADER_READ_LENGTH`, `DEBUG_MODE_ENABLED` — **no longer reproduce**, and
the reason is worth recording rather than celebrating: each of them has a
hand-written entry in `src/external_packs.rs`, e.g.

```rust
modeled_semantic_field_typed_value("BCF2Codec", "SIZEOF_BCF_HEADER", "int", "5"),
```

Re-running the census over `htsjdk_vcf` with the `variant`/`samtools` slice as a
context-summary source confirms it: `SIZEOF_BCF_HEADER = 5` and
`UNKNOWN_SEQUENCE_LENGTH = 0`, both correct, and both correct *because a human
typed the value into the translator*.

That is a fix for five constants, not for the class. It also does not scale:
every new corpus needs a new table entry, and a wrong entry is undetectable.

Two smaller corrections to the census itself, found while re-measuring:

- `SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH` is declared
  `public static final int UNKNOWN_SEQUENCE_LENGTH = 0;` — **zero is the right
  answer**. The census `awk` cannot distinguish a stub from a constant that
  genuinely is `0`, so this row was a false positive.
- `VariantContext.NO_LOG10_PERROR = CommonInfo.NO_LOG10_PERROR` is defined in
  terms of a constant in *another file*, which is why it needs the cross-file
  handling below rather than a per-owner pass.

### The general fix: read the value the translation already parsed

The information was never missing. A context-summary source is parsed, and
`collect_field_declaration_facts_for_type` recorded each field's *type* and
*kind* while dropping its **initialiser** — so a constant whose value was
sitting in the parsed declaration reached the facade with none.

`src/source_constant_values.rs` folds a Java constant expression (JLS 15.28)
into the Rust literal facade emission wants: literals, unary and binary
arithmetic at Java's widths and overflow, `>>>` as a *logical* shift, casts with
Java's narrowing, constant string concatenation, and references to other
constants. Anything it cannot prove constant yields nothing and the loud
`compile_error!` stands — being wrong here would be worse than being silent,
which is this file's own argument.

Cross-file references are why it is a program-level fixpoint rather than a walk:
`VariantContext.NO_LOG10_PERROR = CommonInfo.NO_LOG10_PERROR` cannot resolve
until `CommonInfo` has been folded, and declaration order does not decide that.
Rounds continue until one adds nothing.

Two spellings had to be handled because the parser snake-cases identifiers: a
reference to `SIZEOF_BCF_HEADER` arrives as `Var("sizeof_bcf_header")`, and
`CommonInfo.X` as `FieldAccess(Var("common_info"), "X")`. Every alias is
recorded rather than guessed at.

### Corpus measurement — reach

Measured by folding every `static final` declaration in five real corpora:

| corpus | files | `static final` | with initialiser | **folded** |
|---|---:|---:|---:|---:|
| htsjdk | 801 | 1563 | 1235 | **730** |
| GATK | 2500 | 7679 | 6944 | **4175** |
| picard | 737 | 1859 | 1677 | **805** |
| FastQC | 156 | 500 | 489 | **131** |
| biojava (alignment) | 53 | 133 | 97 | **87** |
| **total** | **4247** | **11734** | **10442** | **5928** |

Before this change **zero** of these contributed a value through the
source route; every one that reached a facade got a hand-written table entry, a
`compile_error!`, or a plausible default. 5928 now carry the value their own
source declares.

The unfolded remainder is mostly `new Something(...)`, array and collection
initialisers, and calls — none of which are constant expressions, and all of
which correctly stay loud. FastQC's low ratio (131 of 489) is this: its
constants are largely `new Color(...)`.

### What this does *not* change on the measured corpus

Honest negative result: re-running `htsjdk_vcf` before and after produces
**byte-identical** generated output — same `stubbed_constants` (4), same
`cargo_check_errors` (59), no diff in any `pub const`. Every group-3 constant
that corpus reaches already has a pack entry, and the four constants still
unmodelled there are not constant expressions at all:

```
JavaNioCharsetStandardCharsets.ISO_8859_1      (JDK object constant)
JavaUtilRegexPattern.CASE_INSENSITIVE          (JDK)
HtsjdkVariantVariantcontextWriterOptions.INDEX_ON_THE_FLY   (enum constant)
HtsjdkVariantVariantcontextAllele.NO_CALL      (`new Allele(...)`)
```

So the change is a generalisation, not a corpus-visible repair, and the reach
table above is the measurement that justifies it.

### Tests

- `static_final_constants_from_summary_sources_carry_their_declared_values` and
  `non_constant_static_final_initializers_stay_unmodelled`
  (`tests/slot_shape_design.rs`) — the end-to-end path, both directions of the
  rule.
- Nine unit tests in `src/source_constant_values.rs` covering Java's widths and
  overflow, logical `>>>`, narrowing casts, floating values with no Rust literal
  form (`1.0` must not render as `1`; infinity and NaN), constant string
  concatenation, cross-constant references, and the refusals: a call, another
  class's field, `null`, and integer division by zero.
- Verified against the Java oracle: a two-file `javac` build printing
  `9,-1.0,BCF\2\2`, and the translation printing the same.

### What keeps this issue open

Nothing in groups 1–3 that this file specified. The remaining surface is the
JDK object constants above (`StandardCharsets.ISO_8859_1`,
`Pattern.CASE_INSENSITIVE`), which are group 2's kind — a lookup table entry
each — and enum constants, which are a different mechanism entirely.
