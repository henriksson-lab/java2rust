# Corpus Fix Current Proceed Plan

Date: 2026-07-23

This plan starts from the current checkout after the VarScan-focused idiom
batch. The translator itself builds and the local idiom suite is green; the next
work is to reduce remaining real corpus failures without drifting away from the
global semantic-analysis design.

## Current Baseline

Local verification already completed:

- `cargo fmt --check`: passes
- `cargo check --quiet`: passes
- `cargo test --test idiom_gaps --quiet`: passes, 297 tests
- `cargo test --test batch_translation --quiet`: passes, 5 tests
- `cargo test --test type_coercion --quiet`: passes, 85 tests
- `cargo build --release --quiet --bin new-java2rust`: passes

Latest focused VarScan rerun:

- Output:
  `.tmp/corpus-batch-current-varscan-after-numeric-format-fixes`
- Result: 18/18 translated, 0 invalid-pattern files
- Offline generated crate check: fails as generated Rust
- Error count: 10

Current VarScan top error families:

| Count | Family |
|---:|---|
| 3 | statement-wrapper `return Ok(String)` inside a `Result<(), JavaException>` block |
| 2 | `HashMap<String, String>` vs `HashMap<String, Box<dyn Any>>` map erasure mismatch |
| 1 | ambiguous `Default::default()?` trait associated function |
| 1 | captured `args[_]` move in `FnMut` closure |
| 1 | immutable enhanced-for loop variable reassignment |
| 1 | temporary borrow from reassigned enhanced-for loop variable |

Recently closed VarScan families include primitive `Map.get` arithmetic
unboxing, assignment expressions in value position, prefix update expressions,
fallible `readLine` assignment conditions, inline 2D row replacement values for
`ndarray::ArrayView1::from(&vec![...])`, `Thread.sleep`, `System.in`, and
typed `OutputStream` support for `PrintStream` constructors.
`Map.get(...)` values used in display/string-concat slots now keep the optional
carrier long enough to map to a Java-style display string, so `Option<&String>`
no longer flows directly into `format!`.
Reusable owned `String` locals now clone at repeated owned consuming slots, and
explicit primitive casts plus arithmetic display conversion no longer produce
the VarScan moved-string or mixed `f64 * f32`/`f64 * String` numeric buckets.

## Constraints

- Preserve the global semantic-analysis path. Indexing, caching, and clone
  reduction are fine; changes that make later whole-program facts harder to
  apply are not.
- Add minimized idiom fixtures before or with each corpus-derived fix.
- Keep package-specific non-JDK behavior in `javalibs` manifests or shims when
  the fix depends on library knowledge.
- Treat GenomicsDB native behavior and DISQ/Spark distributed execution as
  handoff/facade boundaries. Ordinary Java wrapper code around those boundaries
  should still translate.
- Use 10 jobs by default for focused corpus reruns.

## Phase 1: Tighten The VarScan 10-Error Set

Fix the remaining small, high-confidence VarScan families first because this
corpus is fast and already has no invalid-pattern noise.

1. Fix statement-wrapper `return Ok(String)` mismatches.
   - Target Java methods that return `String` while a local exception wrapper is
     currently typed as `Result<(), JavaException>`.
   - Expected translator behavior: either make the wrapper carry the enclosing
     method result type or lower early returns outside a statement-only wrapper.

2. Fix `getParams` map erasure precision.
   - Target methods whose local map is consistently `HashMap<String, String>`
     but whose inferred return slot widens to `Box<dyn Any>`.
   - Expected translator behavior: use source-summary/local return facts before
     fallback erasure.

3. Investigate the ambiguous `Default::default()?` emission.
   - Determine whether it is a missing concrete return slot, an exception
     lowering fallback, or a generated shim signature problem.
   - Add the smallest fixture that reproduces the ambiguous associated function
     form before fixing.

4. Add a fixture for captured array element moves in closures.
   - Target the `cannot move out of *args[_]` residual introduced by a closure
     that captures a Java array/slice and then moves an owned element.
   - Expected translator behavior: borrow or clone the indexed element according
     to the target slot.

5. Add a fixture for mutable Java enhanced-for loop variables.
   - Target `for (String allele : alleleKeys) { allele = getShortIndel(allele);
     ... }`.
   - Expected translator behavior: introduce a mutable local shadow with owned
     lifetime when the loop variable is reassigned.

## Phase 2: Refresh Cross-Corpus Evidence

After the VarScan set drops materially, rerun the focused corpuses that already
reach generated crate checks. Record translated count, invalid-pattern count,
top compile errors, and output directory.

Run small and medium corpuses first:

- `picard-programgroups`
- `htsjdk-fastq`
- `gatk-genomicsdb`
- `gatk-disq-spark-datasources`
- `htsjdk-vcf`
- `jaligner`
- `jahmm`
- `biojava-aaprop`
- `biojava-alignment`
- `trimmomatic`

Then rerun larger corpuses when the smaller set stops exposing shared fixes:

- `fastqc`
- `jhlabs`
- `jsoup`

## Phase 3: Promote Repeated Families To Shared Fixes

Group failures across corpuses and fix them in this order:

1. Java value ownership and display shaping.
2. Remaining JDK facade surfaces.
3. Numeric coercion and array-index coercion misses.
4. Switch/state-machine scaffolding.
5. Source-super and constructor delegation variants.
6. Array creation placeholders and remaining `ndarray` 2D cases.
7. Trait/interface assembly and local Java implementation bridging.
8. Support enum/module assembly, especially VCF constants.
9. Downcast and `instanceof` shape gaps.
10. Collection/map spelling variants not already covered by native plans.

For each family:

- add one minimized fixture per generalized idiom;
- add negative tests when package/simple-name overmatching is a risk;
- implement the smallest translator or `javalibs` fact/shim change;
- run the focused fixture and `cargo test --test idiom_gaps --quiet`;
- rerun the smallest affected corpus;
- update `docs/current-corpus-issue-audit.md` with exact counts.

## Phase 4: Keep The Tracker Current

Update these files as work proceeds:

- `docs/current-corpus-issue-audit.md`: latest corpus result table and top
  remaining families.
- `docs/corpus-fix-next-plan.md`: append completed fix batches with exact
  verification commands and output directories.
- `docs/corpus-fix-current-proceed-plan.md`: keep this file as the live
  next-action plan when priorities change.
- `javalibs` notes/manifests: only when a fix is package-specific or facade ABI
  driven.

## Completion Criteria For This Slice

- VarScan either reaches `cargo check ok` or every remaining failure is a
  documented non-mechanical/facade boundary.
- The current small/medium corpus set has fresh rerun outputs from this
  checkout.
- Every repeated remaining idiom has a fixture, even if the implementation fix
  is deferred.
- No new non-JDK package table is added to `src/stdlib.rs`.
