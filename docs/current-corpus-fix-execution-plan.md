# Current Corpus Fix Execution Plan

This plan starts from the current local state after the signature scope-kind
optimization and the longer FastQC rerun.

## Current Evidence

- `cargo fmt --check`, `cargo check --quiet`, `cargo test --test idiom_gaps --quiet`,
  `cargo test --test batch_translation --quiet`, and
  `cargo build --release --quiet --bin new-java2rust` have passed for the
  current translator changes.
- FastQC now reaches a normal wrapper summary in
  `.tmp/corpus-batch-current-fastqc-after-scope-kind-skip-long`:
  - 156/156 files translated.
  - 0 translation failures.
  - `batch_global_signature_fixpoint` completed in 573014 ms.
  - crate assembly completed in 203601 ms.
  - this run predated the local `ndarray` shim, so `cargo_check` failed only
    because the generated crate could not get the cached `ndarray` dependency.
  - invalid-pattern scan completed with 45 affected files.
- Focused generated crates that emit `ndarray::` now use a generated local
  `ndarray` path shim. VarScan and JAligner reruns with `--offline-cargo`
  both move from `dependency_cache` failure to real `generated_rust` failure.
- The old FastQC duplicate-owner signature panic is no longer the active
  blocker.
- Java-derived Rust keyword identifiers now render as raw identifiers in
  ordinary emitted positions. The latest VarScan rerun no longer has Rust parser
  failures for the Java local name `in`.
- `System.out`/`System.err` statement calls now lower to Rust output macros
  instead of unresolved `Out::`/`Err::` pseudo-types, and Java exception logging
  keeps `getLocalizedMessage()` on the runtime support while turning
  statement-side `printStackTrace` into an explicit no-op.
- Java string API normalization now covers `toUpperCase`, `toCharArray`,
  literal `replace`, `replaceFirst`, `replaceAll`, and `matches`, including the
  focused wrapper shared-support imports for generated corpus crates.
- Constructed Java `String` returns now keep owned `String` return slots unless
  the method body exactly returns a borrow-safe parameter alias. This removes
  VarScan's temporary-reference return family from generated files.
- Java `String` equality now normalizes non-optional string operands to string
  views, including `Vec<String>` index expressions and borrowed parameters.
  This removes the VarScan `String`/`&String` comparison family.
- JDK facade call planning now covers `Math.log`, `Math.exp`, `Double.isNaN`,
  `DecimalFormat.format`, `File.exists`, reader `ready`, `System.exit`, and
  `BitSet.get` through explicit native call targets and return facts.
- Java collection `toArray`, including casted `map.keySet().toArray(new
  String[0])`, now lowers through native collection conversion to an owned
  `Vec<_>`.
- Java integer arithmetic used as an array index now casts the whole expression
  to `usize`, removing the `i32 + usize` and `indexed by i32` family from
  VarScan's `FishersExact` output.

## Priority 1: Separate Environment Gates From Translator Bugs

The current `ndarray` dependency-cache failure hid real generated-Rust
typecheck diagnostics for FastQC and the other matrix/array-heavy corpuses.

Actions:

1. Generate a local path dependency shim named `ndarray` for focused generated
   crates that contain `ndarray::`.
2. Keep the shim narrow and corpus-check oriented: `Array2`, `arr2`, `row`,
   `row_mut().assign()`, and `ArrayView1::from()`.
3. Rerun the affected corpuses after the dependency gate is removed:
   - FastQC
   - BioJava AAPROP
   - BioJava alignment
   - JAligner
   - VarScan
   - JAHMM
   - Trimmomatic

Acceptance:

- Dependency-cache failures are no longer mixed with generated-Rust failures.
- Each affected corpus has either a real `cargo check` result or an explicit
  dependency-gated status.

Current status:

- Wrapper dependency handling is done and covered by
  `focused_package_offline_cargo_uses_local_ndarray_shim`.
- VarScan:
  `.tmp/corpus-batch-current-varscan-after-compound-index-cast`, 18/18
  translated, 0 invalid-pattern files, offline `cargo check` fails as
  `generated_rust` with 84 errors. The old `super(...)`, keyword-identifier,
  `Out::`/`Err::`, `System.err.println`, `printStackTrace(System.err)`,
  `to_upper_case`, `to_char_array`, `String: Pattern`, `replace_first`,
  `replace_all`, temporary-reference return, `String`/`&String` comparison,
  `Math.log`/`Math.exp`, `Double.isNaN`, `DecimalFormat.format`,
  `File.exists`, reader `ready`, `System.exit`, `BitSet.get`, and collection
  `toArray`, and compound index-cast failures are gone; remaining failures are
  type/coercion families such as mismatched types, result `?` incompatibility,
  option arithmetic/unboxing, moved string values, unresolved `thread`,
  lower-case `system.in`, immutable parameter reassignment, mixed
  numeric/string coercions, and `OutputStream` support.
- JAligner:
  `.tmp/corpus-batch-current-jaligner-after-ndarray-shim`, 29/29 translated,
  offline `cargo check` fails as `generated_rust` with 629 errors.
- BioJava AAPROP:
  `.tmp/corpus-batch-current-biojava-aaprop-after-ndarray-shim`, 33/33
  translated, offline `cargo check` fails as `generated_rust` with 740 errors.
- BioJava alignment:
  `.tmp/corpus-batch-current-biojava-alignment-after-ndarray-shim`, 53/53
  translated, offline `cargo check` fails as `generated_rust` with 188 errors.
- JAHMM:
  `.tmp/corpus-batch-current-jahmm-after-ndarray-shim`, 102/102 translated,
  offline `cargo check` fails as `generated_rust` with 120 errors.
- Trimmomatic:
  `.tmp/corpus-batch-current-trimmomatic-after-ndarray-shim`, 68/68
  translated, offline `cargo check` fails as `generated_rust` with 856 errors.
- Remaining rerun: FastQC with the local shim, when the next long run is worth
  the time. The existing long FastQC run already has complete invalid-pattern
  and idiom-sweep data for the next fixtures.

## Priority 2: Fix FastQC Invalid Idioms By Generalized Fixtures

FastQC now produces usable idiom data. Fix these in count/risk order, one
fixture family at a time.

Current FastQC signals:

| Count | Files | Signal | Layer |
|---:|---:|---|---|
| 88 | 36 | `compile_error!` placeholder | missing lowering/planning cases |
| 40 | 14 | `downcast::<T>().expect("invalid Java cast")` | coercion and representation dispatch |
| 10 | 5 | Java collection `.put` style call | native collection call planning |
| 1 | 1 | `None` assigned into concrete storage | slot optionality |

Recent progress:

- Source-backed `super(args)` constructors now inline direct source superclass
  field initialization into flattened child structs for simple constructor
  bodies.
- JDK `FileReader` superclass constructors are modeled as no-state facade
  construction, which removes the VarScan `SmartFileReader` `super(...)`
  placeholders.
- Raw Rust identifier emission and scanner handling are in place for Java names
  that collide with Rust keywords, including raw optional locals such as
  `r#in`.
- Native statement lowering is in place for `System.out`/`System.err`
  `print`/`println`; JavaException runtime/wrapper support includes
  `get_localized_message`, and `printStackTrace` statement calls are lowered
  away.

Idiom sweep detail adds these broader families:

| Count | Files | Signal | Layer |
|---:|---:|---|---|
| 250 | 5 | synthetic switch state-machine scaffolding | switch planning |
| 35 | 22 | `super(...)` constructor placeholder | inherited initialization |
| 33 | 17 | Java array creation placeholder | array shape planning |
| 12 | 7 | Java collection method spelling | collection call planning |
| 5 | 3 | unsupported `instanceof`/cast narrowing | representation dispatch |
| 1 | 1 | constructor delegation placeholder | constructor initialization |

Actions:

1. For each top family, add or update a minimal fixture in `tests/idiom_gaps.rs`
   before changing the translator.
2. Start with the highest-value generated-Rust blockers:
   - `super(...)` constructor lowering for inherited initialization;
   - Java array creation shapes that still emit placeholders;
   - collection method planning for `.put`, `.addAll`, and related native
     Rust container calls;
   - guarded `instanceof` narrowing and cast elimination.
3. Keep FastQC examples as regression references, but make fixtures package
   neutral unless package knowledge belongs in `javalibs`.
4. Rerun:
   - focused fixture test;
   - `cargo test --test idiom_gaps --quiet`;
   - a FastQC wrapper rerun or a targeted generated-crate invalid-pattern scan.

Acceptance:

- FastQC invalid-pattern file count decreases for fixed families.
- No new hardcoded non-JDK library behavior is added to `src/stdlib.rs`.
- Fixture names describe the generalized Java idiom, not the specific corpus
  file.

## Priority 3: VCF Shared Support Enum Assembly

VCF remains blocked by generated syntax around shared support enum/display
assembly, especially `VCFCompoundHeaderLineSupportedHeaderLineType`.

Actions:

1. Minimize the generated output from the VCF shared support enum failure.
2. Add a fixture for the shared enum/display/support type pattern.
3. Fix enum support item assembly so generated Rust parses before typecheck.
4. Rerun the VCF focused corpus.

Acceptance:

- VCF generated crate gets past Rust parsing.
- Any remaining VCF failures are ordinary typecheck diagnostics or explicit
  unsupported idioms.

## Priority 4: Spark And DISQ Boundary

Spark and DISQ distributed execution should not be mechanically translated.
The Java translator should preserve a facade/handoff boundary for noop Spark
configuration and future external execution.

Actions:

1. Keep Hadoop configuration, Spark, and Google Cloud Storage as noop facades
   for now.
2. Fix interface/delegation shape issues around `Reference*SparkSource.getBases`.
3. Add a fixture proving trait impl signatures match the planned method shape.
4. Avoid modeling Spark distributed semantics inside this translator.

Acceptance:

- Focused Spark/DISQ corpus no longer fails on missing interface impls caused
  by signature mismatch.
- Remaining Spark/DISQ failures are explicitly classified as facade/handoff
  requirements, not ordinary Java lowering bugs.

## Priority 5: GenomicsDB FFI Handoff

GenomicsDB should not be a semantic `javalibs` facade. It should be represented
as an FFI handoff boundary whose signatures can be provided by the future C/C++
transpiler or hand-authored ABI facts.

Actions:

1. Keep GenomicsDB out of `javalibs` as a behavioral facade.
2. Add FFI handoff fixtures covering:
   - overloaded native call names;
   - generated signature names;
   - Java wrapper calls that forward to external native state.
3. Fix duplicate generated item names for overloaded `query` style APIs.
4. Record any remaining native behavior as waiting for external FFI signatures.

Acceptance:

- Focused GenomicsDB errors are reduced below the current overload/name conflict
  class.
- The translator emits stable handoff shapes rather than attempting to
  reimplement native GenomicsDB behavior.

## Priority 6: Performance Follow-Up

The current speed is good enough to proceed with correctness work, but FastQC
still spends most time in signature fixpoint.

Actions:

1. Keep profile/release builds as the normal corpus mode.
2. Preserve the accepted scope-kind skip optimization.
3. Avoid algorithm changes that block or weaken future global semantic analysis.
4. Prefer low-risk improvements:
   - reduce unnecessary cloning;
   - add indices for repeated lookup paths;
   - improve progress reporting for long global phases.

Acceptance:

- Correctness work remains unblocked by runtime.
- Any future solver optimization has equivalence tests for cross-method
  nullability and borrowed/owned propagation.

## Suggested Work Order

1. Add cross-corpus fixtures for the shared generated-Rust families:
   array creation placeholders, switch scaffolding, Java file/logging facade
   calls, string equality/coercion, trait/interface assembly, collection method
   spelling, guarded casts, concrete `None` assignments, and remaining
   source-super variants beyond simple constructor field inlining.
2. Implement the smallest translator changes needed for those fixtures.
3. Rerun FastQC and update `docs/current-corpus-issue-audit.md`.
4. Fix VCF enum assembly.
5. Fix Spark/DISQ interface bridge shape.
6. Add GenomicsDB FFI handoff fixtures and resolve overload naming.
7. Rerun all tracked corpuses and refresh the audit.

## Do Not Do

- Do not revive the rejected signature worklist solver without equivalence
  tests; it caused semantic drift in nullable parameter propagation.
- Do not model true Spark/DISQ distributed execution in this Java translator.
- Do not put GenomicsDB native behavior into `javalibs`.
- Do not add non-JDK package/type/method tables to `src/stdlib.rs`.
- Do not treat lower invalid-pattern counts as correctness proof without
  fixture and corpus verification.
