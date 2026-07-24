# Corpus Fix Live Execution Plan

Date: 2026-07-23

This is the current tracked execution plan for continuing the corpus-fix goal
from this checkout. It starts from the latest documented baseline in
`docs/current-corpus-issue-audit.md` and
`docs/corpus-fix-current-proceed-plan.md`.

## Current Baseline

Local gates were last verified green:

- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet` with 297 tests
- `cargo test --test batch_translation --quiet`
- `cargo test --test type_coercion --quiet`
- `cargo build --release --quiet --bin new-java2rust`

Latest fast feedback corpus:

- Corpus: VarScan
- Output: `.tmp/corpus-batch-current-varscan-after-numeric-format-fixes`
- Result: 18/18 translated, 0 invalid-pattern files
- Offline generated crate check: fails with 10 generated-Rust errors

The reusable owned-`String` and numeric/display cleanup fixtures are now
verified. The next implementation should target the remaining 10-error VarScan
set.

## Execution Rules

- Keep implementation in this checkout.
- Use parallel workers for read-only investigation, fixture discovery, and
  corpus error grouping.
- Merge fixes serially in this checkout after each fixture-sized diagnosis.
- Use 10 jobs by default for focused corpus reruns.
- Preserve the global semantic-analysis direction. Performance work should use
  indices, caching, or reduced cloning, not local shortcuts that prevent later
  whole-program fact propagation.
- Package-specific non-JDK behavior belongs in `javalibs` manifests or shims,
  not in new hardcoded tables in `src/stdlib.rs`.
- GenomicsDB native behavior and DISQ/Spark distributed execution are handoff
  or facade boundaries. Java wrapper code around those boundaries should still
  translate mechanically.

## Phase 1: Finish The Current VarScan 10-Error Set

1. Add and fix a statement-wrapper return fixture.
   - Current evidence: `Copynumber::process_copy_region` emits
     `return Ok(format!(...))` inside a wrapper typed as
     `Result<(), JavaException>`.
2. Add and fix a precise map-return fixture.
   - Current evidence: `VarScan::get_params` returns a concrete
     `HashMap<String, String>` but its signature widens to
     `HashMap<String, Box<dyn std::any::Any>>`.
3. Add and fix a captured array-element move fixture.
   - Current evidence: `File::new(args[1])` inside an `FnMut` exception
     wrapper moves from a captured `&[String]` element.
4. Add and fix a mutable enhanced-for loop variable fixture.
   - Current evidence: a loop over `allele_keys.iter()` later assigns
     `allele = &Self::get_short_indel(allele)`, producing both immutable
     reassignment and temporary-borrow errors.
5. Investigate the remaining `Default::default()?` fallback only after the
   source-super/default-constructor fixture is minimized.

Acceptance: VarScan reaches `cargo check ok` or the remaining rows each have a
minimized fixture and a documented next implementation path.

## Phase 2: Refresh Cross-Corpus Evidence

Once the VarScan 10-error set drops materially, launch parallel read-only
workers to refresh issue grouping across corpuses that already reach generated
crate checks:

- Worker A: `gatk-genomicsdb`, `gatk-disq-spark-datasources`
- Worker B: `htsjdk-vcf`
- Worker C: `jaligner`, `jahmm`
- Worker D: `biojava-aaprop`, `biojava-alignment`
- Worker E: `trimmomatic`
- Worker F: `fastqc` invalid-pattern families

Each worker should return:

- exact output directory;
- translated count;
- invalid-pattern count;
- top compile-error families;
- smallest source files or snippets for fixture extraction;
- whether the issue is ordinary Java, facade handoff, or non-mechanical native
  behavior.

Do not let workers make conflicting implementation edits. Their output should
be diagnoses, fixtures, or candidate patches that can be reviewed and merged
one at a time.

## Phase 3: Shared Fix Order After VarScan

Prioritize repeated idioms across corpuses:

1. Java value ownership and display shaping.
2. Remaining JDK facade surfaces.
3. Numeric coercion and index coercion misses.
4. Switch/state-machine scaffolding.
5. Source-super and constructor delegation variants.
6. Array creation placeholders and remaining `ndarray` 2D cases.
7. Trait/interface assembly and local Java implementation bridging.
8. Support enum/module assembly, especially VCF constants.
9. Guarded casts, downcasts, and `instanceof` shape gaps.
10. Collection/map spelling variants not already covered by native plans.

For each shared family:

1. Add a minimized fixture.
2. Add a negative fixture when simple-name or package overmatching is possible.
3. Implement the translator or `javalibs` fact/shim change.
4. Run focused tests and the affected corpus.
5. Update the audit docs with exact counts and output paths.

## Tracking Updates

Keep these files current after every corpus rerun:

- `docs/current-corpus-issue-audit.md`: latest corpus table and remaining
  families.
- `docs/corpus-fix-next-plan.md`: append completed fix batches with commands
  and output directories.
- `docs/corpus-fix-current-proceed-plan.md`: broad next-action plan.
- `docs/corpus-fix-live-execution-plan.md`: current execution checklist.

## Completion Criteria

This corpus-fix slice is complete when:

- local gates pass;
- VarScan is clean or only has documented non-mechanical/facade-boundary
  residuals;
- small and medium corpuses have fresh rerun outputs from this checkout;
- repeated remaining idioms have fixtures;
- no new non-JDK package tables were added to `src/stdlib.rs`;
- package-specific behavior needed for translation is represented in
  `javalibs` manifests/shims or explicit facade handoff facts.
