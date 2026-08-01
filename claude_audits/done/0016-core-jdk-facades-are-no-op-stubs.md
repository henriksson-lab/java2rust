# 0016 — Core JDK facades are no-op stubs, so correct-looking code silently produces nothing

Status: **FIXED** — was: validated (runtime-confirmed; live call sites in the FastQC corpus)
Severity: **critical** — output compiles, runs, and produces empty/zero results
Kind: facade coverage, and a fallback that cannot be distinguished from a real implementation

## Symptom

`StringBuilder` — one of the most common types in Java code — is emitted as a
facade whose methods do nothing:

```rust
impl StringBuilder {
    pub fn append<T0>(&self, _arg0: T0) -> Self { self.clone() }   // discards the argument
    pub fn new() -> Self { Default::default() }
    pub fn to_string(&self) -> String { Default::default() }        // always ""
    pub fn length(&self) -> i32 { Default::default() }              // always 0
}
```

Every `StringBuilder` use therefore yields `""`. Confirmed by running a probe:
a method appending `"one"` and `"two"` returned `""`.

## This is live in the corpus, not just in probes

FastQC generated code calls the no-op directly:

```rust
// CliScenario::to_string
return StringBuilder::new_0()
    .append(&"Scenario: ".to_string())
    .append(self.name.as_ref())
    .append(&System::line_separator())
    .append(&"  FastqFile: ".to_string())
    …
    .to_string();          // -> ""
```

`StringBuilder` appears **181 times across 157 files** in that corpus, and the
crate reports these as zero compile errors.

The same shape affects other entirely-stubbed types in that build:

| methods | type |
|---|---|
| 22 | `StringBuilder` |
| 17 | `OrgApacheCommonsMath3UtilFastMath` — every math function returns `0` |
| 8 | `JavaUtilArrays` |
| 5 | `Pattern` / `JavaUtilRegexPattern` — regex |
| 3 | `OrgApacheCommonsMath3SpecialGamma`, `ArithmeticUtils` |

A numerical application whose `FastMath` returns `0.0` everywhere still compiles.

## Scope: what this is *not*

I initially read a high stub ratio as "the translator is emitting hollow
programs". That was wrong, and the correction matters for prioritisation.
Separating real translations from the support layer:

| corpus | `src/generated/` (real) | `src/lib.rs` (facades) |
|---|---|---|
| jaligner | 221 fns, 11 no-op (4%) | 206 fns, 163 no-op |
| picard | 6 fns, 0 no-op (0%) | 49 fns, 30 no-op |
| FastQC | 2040 fns, 178 no-op (8%) | 919 fns, 579 no-op |

**Translated source code is fine** — 0–8% no-op. The stubs are concentrated in
the external-facade layer, which is expected for genuinely unmodelled libraries.
`SmithWatermanGotoh::align` in jaligner is a real, fully translated
implementation.

The defect is narrower and sharper than "everything is hollow": **`StringBuilder`,
`FastMath`, `Arrays`, and `Pattern` are not exotic externals.** They are core
JDK / ubiquitous-library types that any Java corpus will hit constantly, and
they are being served by the same silent fallback used for AWT.

## Root cause

The default facade shell generates a signature-compatible method with a
`Default::default()` body when no model exists. That is indistinguishable, to
every downstream consumer *and to the compile gate*, from a working
implementation. It converts a coverage gap into silent wrong behaviour.

## Fix direction

1. **Implement the small set of core types for real.** `StringBuilder`/
   `StringBuffer` are trivial over `String`; `Arrays`, `Pattern`, and
   `FastMath` map onto std/`regex`/`f64` methods. This is bounded, high-value
   work — far cheaper than the analyses in the other issues, and it removes a
   whole class of silent wrongness.
2. **Make no-op bodies loud.** Until a type is modelled, the generated body
   should be `unimplemented!("java.lang.StringBuilder.append is not modelled")`,
   not `Default::default()`. Failing at runtime with a precise message is
   strictly better than returning `""`. This is the same argument as
   [0011](0011-dyn-any-fallback-fails-at-every-use.md): fallbacks must name
   their cause.
3. **Report facade coverage as a first-class metric.** `summary.tsv` should carry
   `noop_facade_methods` and `noop_facade_call_sites`. Today a corpus can reach
   "0 compile errors" while calling 181 no-ops, and nothing surfaces that.

Step 2 conflicts with step 1's short-term ergonomics (it will turn quiet corpora
loud). Do step 1 for the handful of core types **first**, then step 2, so the
noise that appears is genuinely unmodelled territory.

## Regression test worth adding

```java
StringBuilder sb = new StringBuilder();
sb.append("one").append(2).append('x');
return sb.toString();          // must be "one2x", must never be ""
```

A test that asserts on *values* rather than on compilation. Note that no
compile-error-based gate can catch this issue — see
[0018](0018-the-compile-error-metric-cannot-see-silent-failures.md).

## Status 2026-08-01 — core defect fixed

The reported silent-no-op class is resolved. `StringBuilder`/`StringBuffer` now accumulate for real (`Fallthrough.java` returns `"onetwo"`/`"threeother"` instead of `""`), `Math.sqrt`, `Arrays.sort`, and `Arrays.toString` map to real implementations, and unmodelled facade methods now emit `unimplemented!("X.m is not modelled")` rather than `Default::default()` — so the fallback is loud, as this issue asked. `Math.PI` is `std::f64::consts::PI` ([0031](0031-stubbed-constant-census.md)).

**Remaining, lower severity:** `Pattern`/`Matcher` are listed as builtin facade types (so nothing synthesises them) but no implementation is emitted, giving `E0433: undeclared type JavaUtilRegexPattern`. That is a *loud* failure, not the silent class this issue is about. Also ~78 `impl From<X> for Y { Default::default() }` upcast conversions remain no-ops; they convert between placeholder structs that carry no data, so they lose nothing that exists today — they become real defects only once those types are modelled.


## Update 2026-08-01 — regex facade closed

`Pattern`/`Matcher` have hand-written structs, but the package-qualified
`JavaUtilRegexPattern`/`JavaUtilRegexMatcher` were listed as builtin facade
types — which *excludes* them from method synthesis — while nothing ever emitted
them. Any `Pattern.matches(...)` therefore produced `E0433: undeclared type`.

Removing only the qualified names from `is_builtin_external_facade_type` lets
them take the standard unmodelled-external path: a typed shim whose methods are
`unimplemented!("...is not modelled")`. The probe goes from 2 errors to **0**.
`Pattern`/`Matcher` stay builtin, since removing those duplicated the
hand-written structs.

The `impl From<X> for Y` upcast conversions still return `Default::default()`.
They convert between facade placeholders that hold no state, so nothing is lost
*today*; they now carry a doc comment naming the unmodelled upcast so the gap is
visible rather than looking like a working conversion. They become real defects
the moment either type gains a field.

## Update — closed

The regex facade compiles (probe 2 errors to 0). The `impl From<X> for Y` upcast
conversions now carry a doc comment naming the unmodelled upcast, so a
value-discarding conversion is visible rather than looking like a working one;
they convert between placeholders that hold no state, so nothing is lost today.

Status: **closed** for the silent-no-op class this issue describes. The focused
regex runtime-surface probe compiles after updating its generated Rust caller to
the current borrowed-`String` signature.
