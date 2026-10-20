# JAMA 1.0.3 feasibility comparison

Official [NIST release](https://math.nist.gov/javanumerics/jama/), archive
`https://math.nist.gov/javanumerics/jama/Jama-1.0.3.tar.gz`, SHA256
`a8f4c8f3a4a5d5f8908304638d03aa181b7c71debb2e3c08bb9dc14fcc350a75`.
Complete seven-file production core is in
`scripts/focus-sets/jama-1.0.3-production.txt`: Matrix, five decompositions and
Maths. The upstream test and example are callers, not part of that library
scope. Matrix read/print methods are included, not carved out.

Preparation/runner: `bash scripts/prepare-jama-1.0.3.sh` and
`NEW_JAVA2RUST_BIN=... bash scripts/run-jama-1.0.3.sh`. The archive hash is
checked, source files compared against the archive, and staging byte-compared.

## Baseline and independent Java check (2026-10-20)

Same frozen V152 binary as the JGraphT/string comparisons, SHA256
`7692d388eb4fc976b3bb0f526a301474dffc84a8b57c558c77051167336b1e81`.
Output `.tmp/jama-1.0.3-v152-baseline`, log `.tmp/jama-v152-baseline-run.log`.
Terminal1: sevenfiles, translator-error files1 (Matrix still emits output),
parse failures0, cargo_type_errors_valid1, compiler diagnostics903,
registry markers35/6files, six no-op facade call sites. Total25.215s:
resolver1.071, translation5.127, assembly9.123, check8.319, sweep0.917.

Untouched Java production files and TestMatrix were compiled from source,
not the archive's prebuilt class files, under
`.tmp/jama-1.0.3-java-oracle/classes`. Upstream TestMatrix completes with
**0errors, 0warnings**; logs `javac.log` and `TestMatrix.log` in that directory.
This proves the Java reference is usable, not correctness of translated Rust.

## Generated rewrite audit

- Naming/source lookup: Matrix field declared `A` but constructor writes free
  variable `a`; Eigenvalue fields V/H are read as free v/h, or self.h although
  declaration is H. Fix declaration/use identity through the shared naming
  contract, resolving source fields before class-name heuristics. Diagnostics
  also invent facade types A/B/X for source variables whose Matrix methods
  should resolve to source declarations. Local/shared resolution-emission
  fixes, with case-sensitive shadowing controls; not manual text replacement.
- Representation: Matrix.A and getArray signatures use shared Array2<f64>.
  Constructor code uses `.len()` for Java outer-array length (wrongly m*n),
  and row extraction emits `.row(i).to_vec()`, then attempts RefCell writes.
  Even compiling that copy would lose updates. LU/Cholesky retain row aliases;
  Matrix(double[][]) and getArray expose the original outer and row identities.
  Correct hypothetical representation is a retained outer vector of nullable
  retained row vectors, not a flat rectangular array. getArray clones only
  the outer handle; getArrayCopy allocates independent rows. Replacing a row
  must not invalidate a previous alias. Reuse existing reference-row carrier
  and evaluation-order machinery, avoiding guards across RHS computations.
- Whole-path impact: allocation/defaults, field types, parameter/return ABI,
  local aliases, indexed reads and compound writes, copy/clone, tokenizer
  Vector<double[]> construction and getMatrix overloads must all agree.
  Current native ndarray selection is type-driven in several slot-render
  paths, rather than a proven no-row-identity optimization. Disabling it only
  at one producer cannot express the correct rewrite end-to-end.
- I/O: missing DecimalFormat setters/symbols plus tokenizer/Vector runtime
  effects remain part of full Matrix support. They cannot be stubbed merely
  because numerical decompositions are the main interest.
- No source generic inheritance or escaping callbacks exist in this core;
  the hard cases are primitive nested-array identity and numeric evaluation.

## Decision

Do not select JAMA as a quick replacement for JGraphT. Its seven-file size
hides a whole-path primitive-matrix representation change. Existing retained
reference-row machinery provides a plausible future design, but promoting it
through all numerical Array2 policies is a separate architectural batch, not
a handful of local compiler fixes. Preserve the baseline and full scope;
do not remove getArray, copying or I/O to claim support. Compare a scalar/
one-dimensional-array numerical component before choosing the new campaign.
