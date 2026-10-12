# Real Code Corpora

This repo keeps minimized fixtures in `examples/java`, but the old transpiler has
real Java corpuses under `/home/mahogny/github/claude/java2rust/testdata`.
`docs/corpora.tsv` is the trackable manifest for those inputs.

## Policy

- Do not copy whole external corpuses into this repo.
- Treat `examples/java` as minimized rule fixtures.
- Treat `/home/mahogny/github/claude/java2rust/tests/corpus` as the old
  self-contained golden snippet corpus.
- Treat `testdata/*` corpuses in the old repo as external gates.
- When a real corpus exposes a new failure class, add one minimized fixture to
  `examples/java` before changing translator rules.
- Keep corpus progress as reports/gates, not as ad hoc translation logic.
- Do not infer that only the currently rerun focused corpuses have idiom gaps.
  Every corpus below is either a regression guard or an assumed backlog source.
  If a rerun fails `cargo check`, emits invalid idiom patterns, or times out
  before crate assembly, promote its failure classes into
  `docs/corpus-rerun-idiom-gap-plan.md` and keep them tracked until a clean
  rerun removes them.

## Gate Tiers

Tier 0 is always local and small:

- `current-fixtures`
- `old-golden-snippets`

Tier 1 should be the first real-code gate:

- `commons-csv-1.10.0`
- `trimmomatic-0.40`
- `jaligner`
- `jahmm`
- `varscan`
- `htsjdk-fastq`
- `htsjdk-vcf`
- `picard-programgroups`
- `biojava-aaprop`
- `biojava-alignment`
- `jhlabs`

Tier 2 adds broader project shape without immediately jumping to the largest
dependency-heavy corpuses:

- `jsoup`
- `fastqc`
- `gatk-genomicsdb`
- `gatk-disq-spark-datasources`

Tier 3 is for larger real project coverage:

- `bioformats`
- `picard`
- `htsjdk`
- `jts`

Tier 4 is the stress corpus:

- `gatk`

## Inventory

| Name | Java files | Tier | Path |
|---|---:|---:|---|
| `old-golden-snippets` | 42 | 0 | `/home/mahogny/github/claude/java2rust/tests/corpus` |
| `current-fixtures` | 38 | 0 | `examples/java` |
| `commons-csv-1.10.0` | 12 production | 1 | `.tmp/commons-csv-1.10.0-src/src/main/java` |
| `trimmomatic-0.40` | 65 production + 3 test | 1 | `/home/mahogny/github/claude/java2rust/testdata/trimmomatic` |
| `jaligner` | 29 | 1 | `/home/mahogny/github/claude/java2rust/testdata/jaligner` |
| `jahmm` | 102 | 1 | `/home/mahogny/github/claude/java2rust/testdata/jahmm` |
| `varscan` | 18 | 1 | `/home/mahogny/github/claude/java2rust/testdata/varscan` |
| `htsjdk-fastq` | 8 | 1 | `/home/mahogny/github/claude/java2rust/testdata/htsjdk_fastq` |
| `htsjdk-vcf` | 32 | 1 | `/home/mahogny/github/claude/java2rust/testdata/htsjdk_vcf` |
| `picard-programgroups` | 11 | 1 | `/home/mahogny/github/claude/java2rust/testdata/picard/src/main/java/picard/cmdline/programgroups` |
| `gatk-genomicsdb` | 6 | 2 | `/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/tools/genomicsdb` |
| `gatk-disq-spark-datasources` | 10 | 2 | `/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/engine/spark/datasources` |
| `biojava-aaprop` | 33 | 1 | `/home/mahogny/github/claude/java2rust/testdata/biojava_aaprop` |
| `biojava-alignment` | 53 | 1 | `/home/mahogny/github/claude/java2rust/testdata/biojava_alignment` |
| `jhlabs` | 203 | 1 | `/home/mahogny/github/claude/java2rust/testdata/jhlabs` |
| `jsoup` | 195 | 2 | `/home/mahogny/github/claude/java2rust/testdata/jsoup` |
| `fastqc` | 156 | 2 | `/home/mahogny/github/claude/java2rust/testdata/FastQC` |
| `bioformats` | 667 | 3 | `/home/mahogny/github/claude/java2rust/testdata/bioformats` |
| `picard` | 737 | 3 | `/home/mahogny/github/claude/java2rust/testdata/picard` |
| `htsjdk` | 1222 | 3 | `/home/mahogny/github/claude/java2rust/testdata/htsjdk` |
| `jts` | 1435 | 3 | `/home/mahogny/github/claude/java2rust/testdata/jts` |
| `gatk` | 2500 | 4 | `/home/mahogny/github/claude/java2rust/testdata/gatk` |

## Apache Commons CSV differential gate

The Commons CSV checkout is pinned in `docs/corpora.tsv`. With that checkout at
the recorded local path, run:

```sh
scripts/verify-commons-csv-1.10.0.sh
```

The gate translates all 12 untouched production files, rejects translation and
idiom diagnostics, compiles the generated crate, and compares a Java oracle
with a Rust program linked only against the freshly generated output. It covers
quoting and escaped quotes, embedded newlines, alternate delimiters, empty and
null fields, headers, comments, named access, malformed input, and a print/parse
round trip. Pass a different checkout path as the first argument when needed;
the script still verifies the exact upstream commit.

## Trimmomatic differential gate

The Trimmomatic checkout is pinned in `docs/corpora.tsv`. Run:

```sh
scripts/verify-trimmomatic-0.40.sh
```

The gate translates all 65 untouched production files, rejects translation and
idiom diagnostics, compiles the disposable generated crate, and compares Java
and generated-Rust behaviour for core single-record trimming plus an
uncompressed, multi-record FASTQ parse/trim/serialize round trip. It covers
`LEADING`, `TRAILING`, `CROP`, `MINLEN`, and `SLIDINGWINDOW`, a chained trim,
empty/short/drop boundaries, mismatched sequence/quality rejection, paired
keep/unpaired/drop routing, independent mate trimming, and a malformed FASTQ
name line. The Java harness substitutes only the optional
compression dispatcher with an identity implementation; for the plain
`.fastq` fixture this is the same upstream behaviour.
