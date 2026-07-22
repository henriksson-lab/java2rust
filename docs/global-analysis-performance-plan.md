# Global Analysis Performance Plan

## Goal

Reduce focused-corpus translation time now that batch emission is deterministic
and default-10 parallel emission is no longer the main bottleneck.

The guiding constraint is explicit: algorithmic performance changes must not get
in the way of global semantic analysis. The translator still needs one coherent
analysis envelope for resolver facts, source summaries, semantic facts,
representation plans, signature plans, inherited members, and external/facade
facts. Optimization should remove unnecessary cloning, repeated scans, and
avoidable recomputation inside that global analysis, not replace it with
per-file partial analysis.

## Current Evidence

- VarScan default-10 batch run:
  `.tmp/focused-batch-stability-varscan-default10/parallel/phase-events.tsv`
  reports about 93.2s in `batch_global_analysis` and 1.7s in
  `batch_emission`.
- Serial and default-10 VarScan batch artifacts match with `compare_exit=0`.
- More emission threads will not materially improve this case. The next gains
  should come from reducing global-analysis work.
- Fixing unneeded cloning likely takes us a long way. Adding indexes for hot
  fact, usage, resolver, and signature lookups should be the second large gain.

## Immediate 1-4 Plan

This is the near-term execution order. The important constraint is that none of
these changes should get in the way of global semantic analysis. We should keep
one shared batch-level analysis context and make that context cheaper by
removing unnecessary cloning, repeated scans, and avoidable lookup work.

1. Measure the remaining hot loops inside global analysis.
   - Keep `analysis-profile.tsv` as the stable entry point.
   - Add counters/timers only where they explain repeated work: signature
     propagation arms, fact lookups, resolver hierarchy lookups, and source
     summary body cost.
   - Do not introduce file-local semantic shortcuts as a performance fix.

2. Remove unneeded cloning in the signature solver.
   - Continue reducing whole-state clone/rollback churn before changing the
     propagation algorithm.
   - Preserve the current whole-program saturation semantics until a worklist
     version has deterministic equivalence tests.
   - Use corpus stability gates to prove generated artifacts and diagnostics
     remain unchanged.

3. Add indexes for repeated global-analysis lookups.
   - Start with signature constraint internals: method params, locals,
     constructor locals, and duplicate constraint membership.
   - Then add immutable indexed views for facts and names after all source,
     external-pack, javalibs, and user facts have been merged.
   - Keep existing precedence order exact; indexes are an implementation detail,
     not a new matching policy.

4. Investigate safe parallelism without fragmenting semantics.
   - Parallelize read-only collection, parsing, profiling, and emission where
     the merged global context is already fixed.
   - For resolver, semantic, representation, and signature phases, parallelize
     only around immutable snapshots or deterministic reduce steps.
   - Avoid designs that make future whole-program facts harder to compute, such
     as independent per-file semantic inference with later best-effort merging.

## Non-Goals

- Do not make global semantic analysis optional.
- Do not return to independent per-file workers for semantic inference.
- Do not parallelize mutable resolver, semantic, representation, or signature
  state until deterministic merge semantics are designed and tested.
- Do not change Java meaning to make an optimization easier.

## Plan

### 1. Add Fine-Grained Global Timings

Split the single `batch_global_analysis` bucket in `src/lib.rs` into stable
subphases:

- global resolver context
- semantic analysis
- representation planning
- signature constraint collection
- signature dependency graph and SCC analysis
- signature fixpoint
- signature plan build

Emit these into `phase-events.tsv` and add a machine-readable
`analysis-profile.tsv` with counters:

- primary files, types, methods, constructors, fields
- expression/call-site counts
- signature slots, unique constraints, graph nodes/edges, SCC count, max SCC
  size
- fixpoint iterations, propagation sweeps, whole-state clones

Acceptance:

- VarScan and JAligner runs show which global subphase dominates.
- Existing stability artifacts remain byte/hash stable.

Implementation status:

- Initial batch-level profiling is implemented. `translate-batch` now records
  resolver, semantic, representation, and signature subphase timings in
  `BatchTranslationTimings`.
- Signature solving now has a profiled entrypoint that records constraint
  collection, dependency graph construction, SCC analysis, pass planning,
  fixpoint saturation, plan build time, and core signature graph/fixpoint
  counters.
- The focused wrapper writes `analysis-profile.tsv` and includes its path in
  `summary.tsv` when present.
- Smoke evidence:
  `scripts/translate-focused-package.sh --skip-resolver-bench --primary
  examples/java/01-expressions --out .tmp/global-analysis-profile-smoke
  --offline-cargo` writes the expected profile and detailed global phase rows.
- The stability comparator now compares non-timing `analysis-profile.tsv`
  counters when both serial and parallel outputs contain the profile.

### 2. Remove Signature Solver Whole-State Churn

The highest-risk/highest-value target is `execute_signature_solver_pass_plan` in
`src/signature.rs`. It currently clones the whole `SignatureConstraintState`,
runs a whole-program propagation sweep per SCC, then rolls back changes outside
the active component.

Replace this incrementally:

- First count clone time, sweep count, component count, rollback time, and
  constraints added per sweep.
- Then remove avoidable full-state equality/cloning where possible.
- Then move from whole-program sweep-per-component toward component-local or
  worklist propagation over dependency nodes.

Acceptance:

- Final `SignaturePlan` and generated batch manifests match before/after on
  VarScan, JAligner, HTSJDK FASTQ, Picard programgroups, GenomicsDB primary-only,
  and Spark/DISQ primary-only.
- Fixpoint diagnostics and saturation reports remain equivalent.

Implementation status:

- Measurement groundwork is in place. The signature profile now records
  component sweep count, changed component sweep count, whole-state clone count,
  clone time, propagation sweep time, component rollback time, and per-arm
  propagation sweep/change/timing counters.
- VarScan profile before optimization:
  `.tmp/global-analysis-profile-varscan-equality-timed/analysis-profile.tsv`
  showed about 95.9s global analysis, 94.8s signature solving, 93.3s signature
  fixpoint, 11,666 SCC components, 1,637 mutable components, and 35,062
  component sweeps.
- The first solver optimization is implemented: `execute_signature_solver_pass_plan`
  skips SCCs that contain no state-bearing signature slot. These components
  are still represented in the dependency graph, but the existing
  sweep/rollback solver cannot retain changes for them.
- VarScan profile after the mutable-component skip:
  `.tmp/global-analysis-profile-varscan-mutable-components/analysis-profile.tsv`
  shows global analysis dropping to about 19.3s, signature solving to about
  18.0s, signature fixpoint to about 16.1s, and component sweeps dropping from
  35,062 to 4,975. Generated Rust hashes, diagnostics, invalid-pattern counts,
  and top compile errors match the pre-optimization run.
- VarScan profile after hierarchy fact indexing:
  `.tmp/global-analysis-profile-varscan-hierarchy-fact-index-r2/analysis-profile.tsv`
  shows about 21.0s global analysis, 91ms resolver, 1.4s semantic analysis,
  19.5s signature solving, 17.7s signature fixpoint, and 4,976 component
  sweeps. This confirms resolver hierarchy work is no longer material for this
  corpus; signature propagation remains the dominant path.
- Current VarScan serial/default-10 stability artifacts pass the adjusted
  comparator at
  `.tmp/global-analysis-profile-varscan-mutable-components-stability/stability-report.tsv`.
- JAligner profile after the same optimization:
  `.tmp/global-analysis-profile-jaligner-mutable-components/analysis-profile.tsv`
  shows about 4.6s global analysis, 4.1s signature solving, 3.7s signature
  fixpoint, 3,033 SCC components, 751 mutable components, and 1,563 component
  sweeps.
- JAligner profile after hierarchy fact indexing:
  `.tmp/global-analysis-profile-jaligner-hierarchy-fact-index-r2/analysis-profile.tsv`
  shows about 4.5s global analysis, 45ms resolver, 506ms semantic analysis,
  3.9s signature solving, 3.5s signature fixpoint, and 1,563 component sweeps.
- Remaining solver work should target the full propagation sweep itself:
  VarScan still spends about 5.5s of measured time in propagation sweeps and
  JAligner about 1.6s. These changes must preserve whole-program saturation
  semantics; prefer removing repeated scans/clones inside the sweep before
  replacing it with a new algorithm.
- VarScan propagation-arm profile with nanosecond accumulation:
  `.tmp/global-analysis-profile-varscan-propagation-arm-profile-r2/analysis-profile.tsv`
  shows `signature_propagation_sweep_ms` at 5,481ms over 4,976 sweeps. The
  largest measured arms were `constructor_call_result_to_binding` at 2,661ms,
  `call_result_to_binding` at 1,109ms, `constructor_local_alias` at 792ms,
  `constructor_local_alias_source` at 761ms, `method_local_alias` at 651ms,
  `method_local_alias_source` at 598ms, and `method_return_binding` at 529ms.
  Several arms ran all 4,976 sweeps with zero changes on this corpus, so the
  next high-leverage solver optimization is likely reducing redundant
  whole-sweep scheduling or moving to an equivalence-tested worklist, not
  adding broad usage maps blindly.
- Rejected experiment: replacing SCC-local rollback with one global propagation
  sweep per saturation iteration is fast but not safe as a drop-in change.
  VarScan dropped to about 3.5s global analysis and 1.9s signature solving at
  `.tmp/global-analysis-profile-varscan-global-sweep-solver/analysis-profile.tsv`,
  and JAligner dropped to about 964ms global analysis and 370ms signature
  solving at
  `.tmp/global-analysis-profile-jaligner-global-sweep-solver/analysis-profile.tsv`.
  However, targeted signature tests object to the semantic change: public
  method boundary constraints can flow into internal callees, unknown call
  result constraints can reach return slots that were intentionally isolated,
  and mutual recursive ownership flows can gain conflicts. Do not keep this
  replacement without an explicit redesign of boundary-aware global propagation
  and equivalence tests.
- Rejected experiment: a per-sweep `PassedToCall` usage index was not kept. It
  preserved generated output but made VarScan propagation materially slower
  because the index was rebuilt for every component sweep. If usage adjacency is
  revisited, build it once per solver run or per immutable usage snapshot rather
  than inside `apply_signature_propagation_sweep`.
- Rejected experiment: a once-per-solver-run `PassedToCall` usage adjacency
  index was also tested and removed. It compiled and preserved the focused
  regression suite, but did not improve the representative profiles:
  `.tmp/global-analysis-profile-varscan-usage-propagation-index-once/analysis-profile.tsv`
  showed VarScan `global_signature_ms` rising from 19,487ms to 19,864ms, and
  `.tmp/global-analysis-profile-jaligner-usage-propagation-index-once/analysis-profile.tsv`
  showed JAligner rising from 3,929ms to 4,041ms. The next signature work should
  avoid broad adjacency maps unless counters first prove the repeated scan is
  hotter than key construction and lookup overhead.

### 3. Index Signature Constraints And Usage

Reduce repeated linear work inside signature propagation.

Targets:

- Index method params and locals by name instead of repeated `.position()` and
  `.iter().find()`.
- Avoid repeated `Vec::contains` over constraint lists by using an internal
  canonical set or per-slot membership cache while preserving deterministic
  output order.
- Build a `UsageFacts` adjacency view for:
  - call result to binding/field/return destinations
  - binding name to local/field/return uses
  - call target to callee param and return constraints

Acceptance:

- Propagation counters show fewer visited bindings/uses per sweep.
- Generated output remains stable under the existing stability gate.

Implementation status:

- Method and constructor signature constraint sets now carry derived
  `param_index_by_name` and `local_index_by_name` maps.
- The hot mutation and dependency-node lookup paths use these maps instead of
  repeated param/local scans. Defensive linear fallbacks remain for manually
  constructed test states that do not populate the derived maps.
- The derived maps are excluded from `PartialEq`, so fixpoint change detection
  still compares semantic constraint state rather than cache contents.
- Local mutation preserves the old first-match behavior, while local
  requirement reads still scan all same-name local slots as before.
- Verification:
  - `cargo fmt --check`
  - `cargo check --quiet`
  - `cargo test --test batch_translation --quiet`, which now asserts
    `analysis-profile.tsv` includes propagation-arm sweep counters.
  - `scripts/translate-focused-package.sh --skip-resolver-bench --primary
    examples/java/01-expressions --out .tmp/global-analysis-index-smoke
    --offline-cargo`
- Known verification gap: `cargo test signature_ --test slot_shape_design
  --quiet` still has pre-existing signature semantic expectation failures, so
  the broad signature-filtered suite is not yet usable as a clean gate for this
  optimization.

### 4. Add Fact And Name Lookup Indexes

Build immutable indexed views after facts have been assembled for the current
analysis envelope.

Targets:

- `FactSet::external_api_effect_for_receiver_with_arity`
- `FactSet::external_field_for`
- `FactSet::external_library_pack_for_type`
- `EffectiveFacts::type_capabilities_for`
- builtin pack helpers such as `builtin_external_pack_models_api` and
  `external_api_effects_for_method_arity`
- repeated type/method/field alias construction and snake-case conversion

Keep precedence exact: user facts, explicit pack facts, wildcard facts,
receiverless fallback, direct type matches, aliases, and simple-name fallback
must keep their current order.

Acceptance:

- Add focused tests for override precedence, receiverless fallback, exact-vs-
  alias matching, static/enum external fields, and overloaded external method
  names.
- Corpus stability gates stay unchanged.

Implementation status:

- Added `ExternalFactLookupIndex<'a>` in `src/facts.rs` as an explicit
  immutable indexed view over an assembled `FactSet`.
- The index currently supports:
  - external API effect lookup by method alias while preserving user override,
    receiverless fallback, arity-specific, return-typed, and broad fallback
    ordering;
  - external field lookup by field-name alias while preserving direct type match
    before broader alias matching;
  - external library-pack lookup with the same direct ownership, package
    ownership, and member-alias fallback order.
- The index deliberately is not a hidden cache inside `FactSet`, because
  `FactSet` is public and frequently mutated by tests and semantic collection.
  Callers should build the index after facts are assembled for a read-only
  analysis phase.
- Added focused parity tests in `tests/semantic_facts.rs`:
  - `indexed_external_api_lookup_preserves_override_and_arity_precedence`
  - `indexed_external_api_lookup_preserves_global_reverse_order_across_alias_buckets`
  - `indexed_external_field_lookup_preserves_direct_then_alias_precedence`
  - `indexed_external_pack_lookup_preserves_member_alias_fallback`
- `src/call_plan.rs` exposes indexed call-planning entrypoints while keeping
  the existing public compatibility APIs.
- `src/lower.rs` now builds one `ExternalFactLookupIndex` in `LowerCtx` per
  lowered program and uses it for call-expression planning and external
  call-result type inference, avoiding repeated index construction in the hot
  expression-lowering path.
- `src/stream_lowering.rs` now accepts the same optional indexed view through
  `StreamRenderContext`; stream lambda member-call planning uses it when stream
  rendering is invoked from normal lowering.
- External call result/exception propagation now has indexed `call_plan`
  entrypoints. `src/lower.rs` routes result propagation checks and call
  exception planning through `LowerCtx.external_index`; only compatibility
  wrappers rebuild an index for legacy public callers and tests.
- Indexed call planning covers selected read-only hot lookups:
  - external API effect lookup for crate/static call planning;
  - external field lookup during weak receiver-type recovery;
  - external library-pack lookup for Rust crate receiver/static call planning.
- Added `HierarchyFactLookupIndex` as an owned immutable snapshot of
  transitive hierarchy reachability over effective type-capability facts. The
  legacy worklist path remains the fallback for query roots outside the indexed
  fact domain.
- Semantic exception classification and catch assignability now share one
  hierarchy reachability snapshot for the stable post-dispatch-fact phase,
  avoiding repeated worklist saturation for the same exception hierarchy roots.
- Verification:
  - `cargo fmt --check`
  - `cargo check --quiet`
  - `cargo test indexed_external_ --test semantic_facts --quiet`
  - `cargo test indexed_hierarchy_reachability_matches_legacy_worklist --test
    semantic_facts --quiet`
  - `cargo test known_unchecked_exception_fact_beats_parsed_unknown_exception_fact
    --test semantic_facts --quiet`
  - `cargo test --test semantic_facts --quiet`
  - `cargo test --test batch_translation --quiet`
  - `cargo test source_summary_nullable --test idiom_gaps --quiet`
  - `scripts/translate-focused-package.sh --skip-resolver-bench --primary
    examples/java/01-expressions --out
    .tmp/global-analysis-exception-index-smoke --offline-cargo`
  - `scripts/translate-focused-package.sh --skip-resolver-bench --primary
    examples/java/01-expressions --out
    .tmp/global-analysis-hierarchy-fact-index-smoke --offline-cargo`
- Known verification gap: `cargo test stream_ --test slot_shape_design
  --quiet` currently fails on existing stream snapshot expectations around
  explicit lambda parameter annotations. The failures are not an index
  precedence mismatch, but the broad stream-filtered suite is not yet a clean
  gate.
- Remaining work: migrate remaining direct fact lookups in helper code and
  resolver phases where profiles still show repeated lookup cost. Keep new
  indexes as explicit read-only snapshots over the assembled global fact set.

### 5. Reuse Resolver Symbols And Hierarchy Lookups

Avoid rebuilding resolver context from scratch within one batch analysis.

Targets:

- Cache `SourceSymbols` for the global source program.
- Attach external facts by reference or indexed view instead of cloning large
  fact sets during resolve.
- Cache method/field hierarchy lookups by `(type, method, arity)` and
  `(type, field)`.
- Memoize subtype distance and hierarchy reachability.

Acceptance:

- Resolver subphase timing decreases or is proven not dominant.
- Overload, inherited-member, nested-type, and source-summary tests remain
  unchanged.

Implementation status:

- `BatchTranslationContext` now stores the once-built
  `resolve::SourceSymbols` for the global batch source program instead of
  keeping the raw source program only for per-file resolution.
- `build_batch_translation_context` constructs `SourceSymbols` once and uses
  `resolve_program_with_source_symbols_and_facts` for the global resolve pass.
- Per-file batch emission now resolves each parsed primary source against the
  shared source-symbol view via
  `resolve_program_with_source_symbols_and_facts`, avoiding repeated
  `source_symbols_from_program` work while preserving the single global
  semantic analysis envelope.
- `ProgramSymbols` now builds immutable method/field hierarchy indexes after
  source-symbol collection. `methods_for_hierarchy` and
  `fields_for_hierarchy` read from these precomputed views and fall back to
  the original traversal only for symbols created without rebuilt indexes.
- `resolve_program_with_source_symbols_and_facts` no longer clones the whole
  `ProgramSymbols` just to attach external facts. External hierarchy facts are
  passed by reference through constructor inference, overload scoring,
  assignability, common-type inference, and stream method-reference inference,
  which preserves fact-aware subtype behavior without copying the global symbol
  tables or precomputed hierarchy indexes per resolve call.
- Subtype-distance resolution now memoizes the fact-independent source
  hierarchy walk per canonical type in a thread-safe cache on `ProgramSymbols`.
  The cache stores only source traversal order and distances; external
  hierarchy facts are still applied by reference at lookup time so different
  fact sets cannot reuse stale fact-dependent subtype answers.
- Verification:
  - `cargo fmt --check`
  - `cargo check --quiet`
  - `cargo test --test batch_translation --quiet`
  - `cargo test resolver_benchmark_keeps_cross_file_source_symbols_package_scoped
    --test resolve_calls --quiet`
  - `cargo test resolver_benchmark_reports_hierarchy_worklist_phase_counters
    --test resolve_calls --quiet`
  - `cargo test htsjdk_tribble_feature_pack_declares_feature_boundary --test
    external_packs --quiet`
  - `cargo test disq_write_pack_declares_temp_parts_option_boundary --test
    external_packs --quiet`
  - `scripts/translate-focused-package.sh --skip-resolver-bench --primary
    examples/java/01-expressions --out
    .tmp/global-analysis-source-symbol-smoke --offline-cargo`
  - `scripts/translate-focused-package.sh --skip-resolver-bench --primary
    examples/java/01-expressions --out
    .tmp/global-analysis-hierarchy-index-smoke --offline-cargo`
  - `scripts/translate-focused-package.sh --skip-resolver-bench --primary
    examples/java/01-expressions --out
    .tmp/global-analysis-no-symbol-fact-clone-smoke --offline-cargo`
  - `scripts/translate-focused-package.sh --skip-resolver-bench --primary
    examples/java/01-expressions --out
    .tmp/global-analysis-subtype-walk-cache-smoke --offline-cargo`
- Remaining work: rerun representative corpus profiles to measure whether
  resolver hierarchy lookups remain material on HTSJDK, Picard, GenomicsDB,
  and Spark/DISQ. VarScan and JAligner now show resolver time below 100ms, so
  the next broad win is still signature propagation. Any follow-on resolver
  optimization should stay compatible with the planned global semantic analysis
  model rather than forcing file-local analysis.

### 6. Treat Source Summaries As Declarations

Source-summary dependencies should inform resolution and facts without paying
full body-level semantic/signature cost when their bodies are not emitted.

Approach:

- After extracting declaration facts, produce a declaration-only summary program.
- Preserve type hierarchy, fields, method/constructor signatures, static-final
  constant initializers, and facts needed for source-summary behavior.
- Strip method/constructor bodies from summary-only dependencies before global
  semantic/signature analysis.

Acceptance:

- Source-summary nullable return, inherited member, static constant, and facade
  boundary tests still pass.
- Large dependency-backed corpus runs show reduced semantic/signature work.

Implementation status:

- Summary sources are parsed once for declaration facts before their bodies are
  stripped from the merged semantic program.
- Declaration-only summaries preserve type hierarchy, field declarations,
  method/constructor signatures, nested types, and static-final field
  initializers. Method and constructor bodies are removed, and non-static-final
  field initializers are removed, before summary-only dependencies enter global
  semantic/signature analysis.
- Method-effect collection preserves existing non-unknown summary return
  nullability when adding later receiver/thrown facts for the same stripped
  declaration. This keeps source-summary declarations from being degraded by
  body-free semantic facts.
- Source-summary facade support surfaces now consult shared method-effect facts
  for nullable returns instead of re-inferring from stripped bodies.
- Verification:
  - `cargo test source_summary_declaration_facts_preserve_nullable_method_return
    --test semantic_facts --quiet`
  - `cargo test
    source_summary_declaration_facts_survive_stripped_summary_semantic_analysis
    --test semantic_facts --quiet`
  - `cargo test source_summary_nullable --test idiom_gaps --quiet`
  - `cargo test
    summary_context_translation_resolves_sibling_symbols_without_lowering_bodies
    --test idiom_gaps --quiet`
  - `cargo test
    emitted_type_gets_source_summary_inherited_static_final_constant --test
    idiom_gaps --quiet`
  - `cargo test
    source_summary_root_does_not_report_body_object_lowering_pressure --test
    dependency_mode --quiet`
  - `cargo test --test batch_translation --quiet`
  - `scripts/translate-focused-package.sh --skip-resolver-bench --primary
    examples/java/01-expressions --out
    .tmp/global-analysis-summary-declarations-smoke --offline-cargo`

## Benchmark Protocol

Use the batch path only. Do not benchmark by translating files independently,
because that measures a different semantic model.

Representative corpuses:

- VarScan: `/home/mahogny/github/claude/java2rust/testdata/varscan/src`
- JAligner: `/home/mahogny/github/claude/java2rust/testdata/jaligner/src/jaligner`
- HTSJDK FASTQ:
  `/home/mahogny/github/claude/java2rust/testdata/htsjdk/src/main/java/htsjdk/samtools/fastq`
- Picard programgroups:
  `/home/mahogny/github/claude/java2rust/testdata/picard/src/main/java/picard/cmdline/programgroups`
- GATK GenomicsDB primary-only:
  `/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/tools/genomicsdb`
- GATK Spark/DISQ primary-only:
  `/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java/org/broadinstitute/hellbender/engine/spark/datasources`

Run release builds, batch mode, and fixed inputs. For performance comparison,
run several repetitions, discard the first warmup, and compare medians. Use
serial/default-10 stability gates for semantic safety.

Regression policy:

- Fail on changed normalized `batch-manifest.tsv`, `status.tsv`,
  `invalid-patterns.tsv`, or `top-compile-errors.tsv` unless the change is an
  intentional translation-quality improvement.
- Fail on stable structural counter growth that is not explained by new facts
  or new source coverage. Volatile solver execution counters, such as changed
  component sweeps and clone counts, belong in performance baseline comparison,
  not semantic stability comparison.
- Warn on wall-clock-only noise when counters and artifacts are stable.

## Order Of Work

1. Implement timings and `analysis-profile.tsv`.
2. Rerun VarScan and JAligner to identify the dominant global subphase.
3. Remove signature solver whole-state cloning and rollback churn if confirmed.
4. Add signature/usage indexes.
5. Add fact/name indexes.
6. Reuse resolver symbol and hierarchy indexes.
7. Strip summary-only bodies once instrumentation can prove the benefit and
   source-summary tests pin behavior.
