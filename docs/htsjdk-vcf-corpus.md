# HTSJDK VCF translator corpus

## Provenance

The production corpus is the untouched contents of
`/home/mahogny/github/claude/java2rust/testdata/htsjdk_vcf`.

- Upstream: `https://github.com/samtools/htsjdk.git`
- Tag: `5.0.0`
- Commit: `fb578787cd8aa2f5b3900b0f3150993d5841e013`
- Upstream path: `src/main/java/htsjdk/variant/vcf`
- Census: 32 Java production files

Provenance was verified on 2026-10-14 by comparing Git blob IDs for every
fixture file with the file at the corresponding path in tag `5.0.0`: all 32
matched. Tags `5.0.1` and `5.0.2` each differed in one file, so `5.0.0` is the
exact revision rather than merely a compatible release.

## Exact production-file census

```text
AbstractVCFCodec.java
VCF3Codec.java
VCFAltHeaderLine.java
VCFCodec.java
VCFCompoundHeaderLine.java
VCFConstants.java
VCFContigHeaderLine.java
VCFEncoder.java
VCFFileReader.java
VCFFilterHeaderLine.java
VCFFormatHeaderLine.java
VCFHeader.java
VCFHeaderLine.java
VCFHeaderLineCount.java
VCFHeaderLineTranslator.java
VCFHeaderLineType.java
VCFHeaderVersion.java
VCFIDHeaderLine.java
VCFInfoHeaderLine.java
VCFIterator.java
VCFIteratorBuilder.java
VCFMetaHeaderLine.java
VCFPassThruTextTransformer.java
VCFPedigreeHeaderLine.java
VCFPercentEncodedTextTransformer.java
VCFReader.java
VCFRecordCodec.java
VCFSampleHeaderLine.java
VCFSimpleHeaderLine.java
VCFStandardHeaderLines.java
VCFTextTransformer.java
VCFUtils.java
```

## Reproduction command

```sh
cargo build --release
scripts/translate-focused-package.sh \
  --primary /home/mahogny/github/claude/java2rust/testdata/htsjdk_vcf \
  --out .tmp/htsjdk-vcf-current \
  --skip-resolver-bench \
  --jobs 10 \
  --offline-cargo
```

Generated Rust under `.tmp` is diagnostic evidence and is never patched by
hand.

## 2026-10-14 compile checkpoint

The fresh post-fix verification run on 2026-10-14 exercised all 32 files and
reported:

- translation failures: 0
- parse failures: 0
- unsupported statements, members, and expressions: 0
- invalid-pattern files: 0
- actual rustc errors: 0; the assembled crate passes `cargo check`
- explicit unmodelled facade call sites: 870 (the sweep counts facade copies
  in each generated module, so this is a triage signal rather than a count of
  distinct executable gaps)
- opaque fallback types: 452
- facade methods returning placeholders: 631
- unmodelled upcast conversions: 64

The compile checkpoint is closed. The authoritative runtime/differential gate
is `scripts/verify-htsjdk-vcf-5.0.0.sh`; it always translates all 32 untouched
production files before comparing representative Java and generated-Rust
behavior. Its small Java-only stubs define dependency boundaries outside this
corpus and do not replace or modify any production source. The expanded gate
passed byte-for-byte against that freshly generated crate.

## Runtime dependency boundary

The isolated 32-file package does not contain HTSJDK's `variantcontext`,
Tribble stream, or writer implementations. Consequently, a full
`VariantContext` record decode/encode round trip cannot be claimed from this
corpus alone: those generated calls remain explicit external facades, and the
gate does not invent their behavior.

The differential gate executes the record logic that is owned by this corpus
and does not require those implementations: scalar INFO-field formatting,
VCF floating-point formatting and rounding, occurrence counting, and missing
FORMAT-field recognition. Java dependency-summary classes exist only to let
`javac` type-check and load the untouched `VCFEncoder.java`; none of their
placeholder methods are called by the oracle. Header parsing/rendering,
metadata lookup, enums/count/type behavior, percent decoding, malformed input,
and header encode/parse round trips are exercised directly as well.
