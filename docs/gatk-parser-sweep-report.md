# GATK Parser/Translation Sweep

Date: 2026-07-13

Corpus root:
`/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java`

## Commands

Build:

```sh
cargo build
```

Sweep, with helper/output files only under `/tmp`:

```sh
ROOT=/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java
BIN=/data/henriksson/github/claude/new_java2rust/target/debug/new-java2rust
OUT=/tmp/new_java2rust_gatk_sweep
rm -rf "$OUT"
mkdir -p "$OUT/stdout" "$OUT/stderr"
find "$ROOT" -name "*.java" | sort > "$OUT/files.txt"
: > "$OUT/status.tsv"
i=0
while IFS= read -r f; do
  i=$((i+1))
  id=$(printf "%05d" "$i")
  rel=${f#"$ROOT"/}
  if timeout 5s "$BIN" "$f" > "$OUT/stdout/$id.rs" 2> "$OUT/stderr/$id.err"; then
    status=ok
  else
    status="exit_$?"
  fi
  printf "%s\t%s\t%s\n" "$id" "$status" "$rel" >> "$OUT/status.tsv"
done < "$OUT/files.txt"
```

Aggregations:

```sh
awk -F '\t' '{count[$2]++} END {for (k in count) print count[k], k}' /tmp/new_java2rust_gatk_sweep/status.tsv | sort -nr
grep -hE '^[A-Za-z]+ [A-Za-z0-9_]+:' /tmp/new_java2rust_gatk_sweep/stderr/*.err | awk '{code=$2; sub(/:$/, "", code); count[code]++} END {for (k in count) print count[k], k}' | sort -nr
grep -hE 'compile_error!|Java array creation|missing switch selector|missing iterable|missing instanceof lhs|unsupported Java statement|unsupported Java expression|TODO\(translation\)' /tmp/new_java2rust_gatk_sweep/stdout/*.rs | sed 's/^[[:space:]]*//' | sort | uniq -c | sort -nr
```

## Counts

| Metric | Count |
|---|---:|
| Java files swept | 1595 |
| Completed with exit status 0 | 1594 |
| Timed out | 0 |
| Panicked/non-zero | 1 |

The one non-zero file was
`org/broadinstitute/hellbender/utils/tsv/DataLine.java`, which panicked at
`src/signature.rs:2266` with `signature solver must plan every lowered local`.
That is a downstream lowering/signature issue, not a parser edit target.

## Top Diagnostics

| Diagnostic code | Count | Notes |
|---|---:|---|
| `unsupported_exceptions` | 1164 | Try/catch and catch clauses are common in GATK. |
| `unsupported_wildcard` | 587 | Generic wildcard syntax is frequent. |
| `unsupported_constructor_super` | 525 | `super(...)` constructor delegation. |
| `resolve_call_ambiguous` | 145 | Often caused by raw/unknown expressions flowing into overload resolution. |
| `unsupported_inheritance` | 118 | Lowering gap, not parser-specific. |
| `unsupported_anonymous_class` | 92 | Anonymous class bodies are common enough for a fixture. |
| `resolve_constructor_ambiguous` | 59 | Often follows unknown expression/type shapes. |
| `signature_constraint_conflict` | 56 | Downstream signature planning. |
| `parse_no_class` | 56 | Mostly top-level `enum` and `@interface` declarations, plus package/info-style files. |
| `signature_clone_recovery_requires_audit` | 24 | Downstream ownership/signature planning. |
| `non_object_safe_trait_fallback` | 20 | Representation/lowering gap. |
| `unsupported_finally` | 17 | Finally clauses are present but less frequent than catch. |

## Top Fallback Signatures

| Fallback signature | Count |
|---|---:|
| `compile_error!("unsupported Java statement: try/catch/finally")` | 384 |
| `compile_error!("unsupported Java constructor: super(...) requires inheritance lowering")` | 285 |
| `compile_error!("unsupported Java instanceof shape: actual=Concrete, target=Concrete")` in `if` conditions | 121 |
| `compile_error!("unsupported Java statement: unsupported statement node: line_comment")` | 56 |
| `compile_error!("unresolved Java constructor delegation")` | 29 |
| `compile_error!("unsupported Java expression: lambda")` in call positions | 1796 occurrences across 490 files |
| `compile_error!("unsupported Java expression: method reference")` in call positions | 722 occurrences across 298 files |
| `compile_error!("unsupported Java expression: class literal")` | 334 occurrences across 200 files |
| `/* Java array creation: ... */ Vec::new()` | Present in 188 files |

## Recommended Parser-Gap Fixtures

Add minimized fixtures under `examples/parser-gaps` before parser changes:

| Fixture candidate | Real GATK examples | Why it matters |
|---|---|---|
| Top-level enum declaration | `metrics/MetricAccumulationLevel.java`, `utils/Nucleotide.java` | Currently reports `parse_no_class`; parser should produce an enum type or a deliberate unsupported enum node, not no class. |
| Top-level annotation type | `engine/DoNotSubclass.java`, `utils/config/SystemProperty.java` | `public @interface` also reports `parse_no_class`. |
| Package-info / no-type compilation unit | `utils/tsv/package-info.java` | Needs an explicit empty/no-type diagnostic distinct from parser failure. |
| Line comment inside block/loop body | `tools/filediagnostics/CRAMAnalyzer.java`, `tools/funcotator/FuncotatorEngine.java` | Comments leak as `unsupported statement node: line_comment`; parser should skip comments in statement lists. |
| Wildcard generic bounds | Many files with `List<?>`, `Class<? extends ...>` | High-frequency diagnostic; fixture should cover unbounded, extends, and super wildcards. |
| Anonymous class expression | GATK table readers and comparators | Common expression/body shape, currently represented as unsupported anonymous class. |
| Method reference in stream/callback call | plugin descriptors and walkers | High-frequency expression fallback in real code. |
| Lambda expression in chained calls | filters, stream pipelines, validators | High-frequency expression fallback that also causes unknown types in resolution. |
| Array creation with dimensions and nested primitive arrays | PairHMM/read utility files | Already documented partly, but GATK adds multi-dimensional and computed-dimension cases. |
| Try/catch/finally statement | CRAM/IO-heavy files | Dominant real-code statement gap; fixture should include catch, multi-catch, resources, and finally as separate cases. |

## Tracked Fixtures

Added parser-gap fixtures under `examples/parser-gaps` for GATK-derived
frontend gaps:

- `GatkTopLevelEnum.java`
- `GatkTopLevelAnnotation.java`
- `GatkPackageInfo.java`
- `GatkLineCommentStatement.java`

These are asserted in `tests/parser_gaps.rs` as expected current gaps.

## Notes

This was a translation sweep, not a parser implementation change. The parser
implementation was not edited. The report intentionally separates parser-shaped
gaps from downstream lowering/signature gaps so the next fixture additions can
stay small and focused.

The first idiom-gap fixtures from this sweep are tracked separately under
`examples/idiom-gaps` with assertions in `tests/idiom_gaps.rs`. The companion
reports are `docs/gatk-idiom-sweep-report.md` and
`docs/gatk-idiom-fixture-candidates.md`.
