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

## Gate Tiers

Tier 0 is always local and small:

- `current-fixtures`
- `old-golden-snippets`

Tier 1 should be the first real-code gate:

- `trimmomatic`
- `jaligner`
- `jahmm`
- `varscan`
- `htsjdk-fastq`
- `htsjdk-vcf`
- `biojava-aaprop`
- `biojava-alignment`
- `jhlabs`

Tier 2 adds broader project shape without immediately jumping to the largest
dependency-heavy corpuses:

- `jsoup`
- `fastqc`

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
| `trimmomatic` | 68 | 1 | `/home/mahogny/github/claude/java2rust/testdata/trimmomatic` |
| `jaligner` | 29 | 1 | `/home/mahogny/github/claude/java2rust/testdata/jaligner` |
| `jahmm` | 102 | 1 | `/home/mahogny/github/claude/java2rust/testdata/jahmm` |
| `varscan` | 18 | 1 | `/home/mahogny/github/claude/java2rust/testdata/varscan` |
| `htsjdk-fastq` | 8 | 1 | `/home/mahogny/github/claude/java2rust/testdata/htsjdk_fastq` |
| `htsjdk-vcf` | 32 | 1 | `/home/mahogny/github/claude/java2rust/testdata/htsjdk_vcf` |
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
