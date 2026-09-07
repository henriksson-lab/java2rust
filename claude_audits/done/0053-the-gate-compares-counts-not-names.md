# 0053 — The gate compares counts, not names, and three metric readers fall back to zero

Recorded 2026-08-21.
Status: **fixed** — the gate now compares failure names for every red target,
and metric readers fail loudly instead of treating missing signals as zero.
Severity: **high (process)** — it is the mechanism that decides whether any of the
other audits' fixes are believed.
Kind: measurement.

Current state 2026-09-07: `tests/differential.rs` asserts 228 active
differential fixtures. `tests/baselines/suite-results.tsv` records 3141 passed
/ 99 failed / 3240 total; the only red target is `slot_shape_design` at 480
passed / 99 failed. The older 23-fixture and 264-failure sections below are
retained as the pre-fix snapshot, not current status.

Codex follow-up 2026-08-23: measurement hardening slices landed. The differential
harness no longer returns `ok` when the translator binary or required tools are
missing; it panics instead, and it asserts the active fixture count. That count
was 23 when this note was recorded; the current guard in `tests/differential.rs`
is 228 fixtures. Verified both directions:
`cargo test --release --test differential` runs the fixtures and passes, while
`env PATH=/nonexistent ./target/release/deps/differential-...` now fails on
missing `javac` instead of reporting a green skip. `scripts/gate.sh`
also now extracts failed test names and compares them when a per-target
failure-name baseline exists, so the existing `idiom_gaps` name baseline is no
longer dead prose. `scripts/translate-focused-package.sh` now fails loudly when
required idiom-sweep signals, analysis-profile metrics, or status columns are
missing instead of converting absence to `0`; the wrapper has a regression that
shadows the idiom sweep and confirms a missing `stubbed_constants` row fails the
summary. The focused wrapper also no longer has a hand-copied invalid-marker
scan: non-batch translation calls the registry-backed `count-invalid-patterns`
CLI, and `invalid-patterns.tsv` is aggregated from the status rows. The final
measurement slice recorded failure-name baselines for every red target, made
missing red-target name baselines a gate failure, and verified the full gate
with `scripts/gate.sh --all`.

Companion to [0052](0052-the-example-corpora-are-asserted-on-but-never-compiled.md)
(what the suites cannot see) and to
[0018](0018-the-compile-error-metric-cannot-see-silent-failures.md), whose
second-pass conclusion was *"a metric whose detectors can silently stop matching
is worse than no metric, because it reports success."* This file finds four
places where that is still true, one level up from the detectors: in the thing
that reads them.

---

## 1. 264 failing tests are baselined as two integers

`scripts/gate.sh` is the project's gate. For each target it extracts
`N passed; M failed` and diffs the pair against
`tests/baselines/suite-results.tsv` (`scripts/gate.sh:57-60,92-107`). Nothing
else is compared.

At recording time, the baseline accepted **264 failing tests out of 2617**:

| target | passed | failed |
|---|---:|---:|
| `idiom_gaps` | 552 | **138** |
| `slot_shape_design` | 527 | **73** |
| `fixtures` | 21 | **20** |
| `dependency_mode` | 49 | **16** |
| `resolve_calls` | 280 | **10** |
| `semantic_facts` | 166 | **4** |
| `external_packs` | 165 | **2** |
| `parser_tree_sitter` | 43 | **1** |
| (12 other targets) | 550 | 0 |
| **total** | **2353** | **264** |

Because the comparison is a count, **any change that breaks one baselined-green
test while fixing one baselined-red test in the same target passes the gate.**
With 264 red tests spread across eight targets, that is not a hypothetical
budget — it is the largest single blind spot in the project's verification, and
it is the one that decides whether every other audit's fix is accepted.

A name baseline exists for exactly one target,
`tests/baselines/idiom-gaps-failures.txt` (138 names). **No script reads it.**
`grep -rn idiom-gaps-failures scripts/ tests/` returns nothing; the only
instructions to diff it are prose in `NEXT-ISSUES-HANDOFF.md:433`. The remaining
126 baselined failures, in seven targets, have no name record anywhere.

### The names, so they are a work queue rather than a number

Obtained by running the prebuilt test binaries directly
(`./target/release/deps/<target>-<hash>`), which needs no cargo and no build lock.

**`fixtures` — 20 failures** (this is `examples/java`, see
[0052](0052-the-example-corpora-are-asserted-on-but-never-compiled.md)):

| test | failure |
|---|---|
| `all_java_fixtures_are_loadable` | `left: 67  right: 65` — fixture count never updated |
| `collection_capacity_constructor_uses_with_capacity` | stale expectation |
| `first_slice_generated_rust_compiles` | `E0277` `dyn Metric` not `Sized` |
| `inherited_overload_does_not_replace_child_override_body` | — |
| `nested_static_builder_call_uses_resolved_type_owner` | stale expectation |
| `stream_block_lambda_static_owner_uses_type_owner` | stale expectation |
| `stream_flat_map_lambda_generated_rust_preserves_behavior` | `E0308`, **behavioural** |
| `stream_flat_map_projection_generated_rust_preserves_behavior` | **behavioural** |
| `switch_fallthrough_generated_rust_preserves_behavior` | `E0308` in the *harness*, **behavioural** |
| `translates_expression_precedence_shape` | `let x: i32 = a + b * c;` not produced |
| `translates_methods_with_overload_suffixes` | `format_string` not produced |
| `translates_same_arity_overload_call_sites` | `choose_string` not produced |
| `translates_simple_stream_sum_to_iterator_chain` | `positive_sum` signature |
| `translates_stream_collect_to_list_to_iterator_chain` | pipeline shape |
| `translates_stream_find_first_and_any_match_through_slot_coercion` | `first_long` signature |
| `translates_stream_flat_map_block_pipeline` | — |
| `translates_stream_non_null_filter_method_reference` | — |
| `translates_stream_nullable_map_then_non_null_filter` | — |
| `translates_switch_fallthrough_without_compile_error` | `'java_switch: loop` not produced |
| `translates_throw_only_method_to_result` | `Err(JavaException::new(…))` not produced |

Three of those 20 are the project's *behavioural* tests. Three of ten
run-the-Rust checks are dark and the gate reports the same `21/20` either way.

Codex follow-up 2026-08-23: implemented for all current red targets. The gate
now extracts top-level failed test names from `---- test stdout ----` headers
(falling back to the final `failures:` block only when no headers are present),
records them for every failing target, and treats a missing failure-name
baseline as a gate failure. `scripts/gate.sh --record` wrote baselines for
`dependency_mode` (16), `external_packs` (2), `fixtures` (21), `idiom_gaps`
(190), `parser_tree_sitter` (1), `resolve_calls` (11), `semantic_facts` (4),
`slot_shape_design` (77), and `--lib` (5). `scripts/gate.sh --all` then passed
with matching counts and names across all 18 targets.

Codex follow-up 2026-08-24: the `--lib` target is no longer red. The five
recorded lib failures were borrow-projection idempotence/parenthesization
regressions; `cargo test --lib -- --nocapture` now passes 327/0, so
`tests/baselines/suite-results.tsv` records `--lib 327 0` and
`tests/baselines/failure-names/--lib.txt` has no failure names.

Follow-up: `parser_tree_sitter` is also no longer red. Method-invocation
receivers now use the same identifier normalization as ordinary identifiers, so
`Ints.toArray(values)` parses its receiver as `Expr::Var("ints")` and leaves
static-owner interpretation to later resolver/lowering policy. `cargo test
--test parser_tree_sitter -- --nocapture` passes 47/0, and the parser-tree
failure-name baseline is empty.

Follow-up: `external_packs` is also no longer red. Constructor argument-effect
coverage now counts manifest constructor entries, and source receiver method
calls no longer seed conservative external fallbacks for source-owned types
without external superclasses. `cargo test --test external_packs -- --nocapture`
passes 169/0, and the external-pack failure-name baseline is empty.

Follow-up: `semantic_facts` is also no longer red. Package-private trivial
getter calls on non-self receivers now lower to the backing field instead of
falling back to the source accessor call, while public accessors still preserve
the API wrapper. Borrowed string external argument facts keep their `&str` view
through signature normalization, readonly external receiver usage accepts the
more precise `KnownContentRead` boundary for borrow planning, and the VCF field
expectations now match the qualified nested-type identities stored in the
external pack. `cargo test --test semantic_facts -- --nocapture` passes 174/0,
and the semantic failure-name baseline is empty.

**`dependency_mode` — 16 failures:**
`external_fact_closed_enum_override_cleans_multilevel_hierarchy_shape`,
`external_fact_hierarchy_override_cleans_breakpoint_evidence_shape`,
`nested_source_serializer_subclass_of_qualified_kryo_serializer_uses_external_boundary`,
`primary_extends_source_summary_base_is_erased_boundary_not_inheritance_pressure`,
`primary_root_resolves_imported_nested_graph_types_without_external_guesses`,
`primary_source_hierarchy_override_cleans_breakpoint_evidence_shape`,
`resolver_benchmark_can_disable_builtin_packs_and_readd_them_as_providers`,
`resolver_benchmark_classifies_snake_case_project_field_receiver_as_project`,
`resolver_benchmark_does_not_treat_lambda_params_as_static_pack_receivers`,
`resolver_benchmark_java_sql_pack_composes_with_sqlite_configuration_workflow`,
`resolver_benchmark_propagates_project_field_guess_ownership_through_static_field_chains`,
`source_summary_field_contexts_do_not_treat_lambda_params_as_static_receivers`,
`source_summary_neutral_envelope_resolves_core_declaration_shapes`,
`source_summary_root_resolves_fermilite_shaped_assembler_graph_boundary`,
`source_summary_root_resolves_imported_nested_graph_types`,
`source_summary_roots_resolve_project_helper_fields_and_nested_exception_constructors`

**`resolve_calls` — 10 failures:**
`resolver_benchmark_does_not_import_incidental_nested_package_types`,
`resolver_deduplicates_substituted_generic_override_candidates`,
`resolver_matches_generic_sibling_nested_constructor_with_current_receiver_argument`,
`resolver_prefers_lexically_scoped_nested_constructor_when_simple_names_collide`,
`resolver_prefers_sibling_nested_constructor_visible_from_nested_class`,
`resolver_treats_source_enum_types_as_assignable_to_enum_api_overloads`,
`resolver_uses_enclosing_source_field_type_for_nested_class_overload_arguments`,
`resolver_uses_enclosing_static_method_return_type_in_nested_constructor_delegation`,
`resolver_uses_external_domain_constants_for_primitive_and_string_overloads`,
`resolver_uses_implicit_no_arg_constructor_for_source_nested_class`

**`semantic_facts` — fixed:**
formerly `external_argument_effects_shape_method_params_in_signature_solver`,
`external_field_provenance_uses_builtin_model_for_known_field_types`, and
`readonly_external_receiver_usage_can_borrow_public_helper_param`; now 174/0.

**`external_packs` — fixed:**
formerly `builtin_pack_manifests_explicitly_classify_modeled_member_facts` and
`java_sql_pack_does_not_claim_project_connection_or_reflection_class_names`;
now 169/0.

**`parser_tree_sitter` — fixed:**
formerly `parses_static_utility_method_receiver_as_identifier`; now 47/0.

`slot_shape_design`'s 73 are not enumerated here: that suite runs 104 nested
`cargo` invocations and would contend with a concurrent build. It is the largest
un-named block and should be captured the same way.

---

## 2. The only behavioural suite reports success when it runs nothing

`tests/differential.rs:20-27` returns early — passing — if the translator binary
is absent, or if any of `javac`, `java`, `rustc` is missing. Verified:

```
$ env PATH=/nonexistent ./target/release/deps/differential-779e8d2e6b3b2ed8
running 1 test
test translated_output_matches_java_output ... ok
test result: ok. 1 passed; 0 failed; … finished in 0.00s

$ ./target/release/deps/differential-779e8d2e6b3b2ed8      # real run
test result: ok. 1 passed; 0 failed; … finished in 55.14s
```

The two results are byte-identical in everything the gate reads, and both match
the baseline row `differential 1 0`. The entire behavioural oracle — 23 fixtures,
1241 lines of Java, the technique 0018 credits with finding a defect in every
batch — reports the same success whether it compared 23 programs or zero.

`scripts/gate.sh:45` does build the release binary first, so the gate itself is
currently honest; but nothing *asserts* that, the skip is silent on stdout
(`eprintln!`), and any invocation that is not `gate.sh` (a bare
`cargo test --test differential`, CI without a release build, a container
without a JDK) is green and empty.

The fix is one line of shape: make the count observable. `assert_eq!` the number
of fixtures compared against a checked-in constant, or `panic!` instead of
`return` when the toolchain is missing and let the caller opt out explicitly.
A skip must be louder than a pass, not identical to it.

---

## 3. Three metric readers return 0 when the data is missing

`scripts/translate-focused-package.sh` builds `summary.tsv` — the file every
corpus decision is read from. Three of its readers cannot distinguish "the
signal is zero" from "the signal is not there".

| helper | line | behaviour when the key/file/column is absent |
|---|---|---|
| `idiom_signal_count` | `:491` | `END { if (!found) print 0 }` |
| `analysis_profile_metric` | `:497` | `if [[ ! -f "$path" ]]; then printf '0\n'` — and the same `!found` fallback |
| `status_column_sum` | `:522` | header lookup fails ⇒ `target=0` ⇒ `END { print sum + 0 }` |

`idiom_signal_count` alone feeds **eight** rows of `summary.tsv`:
`unsupported_statements`, `unsupported_members`, `unsupported_expressions`,
`unmodelled_types`, `unmodelled_call_sites`, `noop_facade_methods`,
`unmodelled_upcast_conversions`, `stubbed_constants` (`:2831-2841`). Every one of
them is a key string in `scripts/sweep-generated-idioms.py`'s `SIGNALS` list.
Rename or drop a signal there and the corresponding row in `summary.tsv` reads
`0` — which, for a *cost* counter, is the best possible score. 0018's whole
argument is that erasure must not look like a saving; here erasing the detector
itself looks like a saving.

`status_column_sum` is worse because the coupling is invisible:
`unsupported_members` is `idiom_signal_count(...) + status_column_sum(status.tsv,
"unsupported_members")`, and `status.tsv`'s header is printed 2360 lines away at
`:462`. Change that header string and the number silently halves.

Only one of the three was ever hardened, and only for one metric:
`analysis_profile_metric_or_idiom_signal` (`:507`) exists because
`noop_facade_call_sites` had already been caught reporting a silent zero
(0018, update 2026-08-06). The same failure mode is still live for the other
eight signals and for every `analysis_profile_metric` caller.

Direction: these are `awk` one-liners. Make a missing key an error
(`END { if (!found) { print "missing signal: " signal > "/dev/stderr"; exit 1 } }`)
and give the caller an explicit `--allow-missing` for the legacy
`--no-batch-translate` path that genuinely has no profile. A metric may be
absent; it must not be absent *and* spelled `0`.

Codex follow-up 2026-08-23: implemented. `idiom_signal_count`,
`analysis_profile_metric`, and `status_column_sum` now exit non-zero on missing
inputs or keys, and each required summary assignment propagates that failure.
`analysis_profile_metric_or_idiom_signal` keeps the intended legacy fallback
only when the profile file is absent; if the file exists, a missing metric is now
an error. Regression:
`cargo test --release --test batch_translation focused_package_missing_idiom_sweep_signal_fails_summary`.

---

## 4. The marker list has a third implementation, and it still has the dead detectors

0018's second pass consolidated the marker strings into `src/output_markers.rs`
so an emitter and its detector could not drift apart, and **deleted** two clauses
that had never matched anything, because
`UnsupportedExpressionLowering::Lambda` and `::MethodReference` are unreachable —
`lower_expr` handles both before consulting the fallback.

`scripts/translate-focused-package.sh:2791-2798` still contains its own
hand-written copy of the list, as raw `grep -E` patterns, and both deleted
clauses are in it:

```sh
'unsupported_lambda	unsupported Java expression: lambda' \
'unsupported_method_reference	unsupported Java expression: method reference' \
```

`invalid-patterns.tsv` therefore carries two rows that are structurally always
`0	0`, in a report whose purpose is to rank what needs work. `grep -rn
UNSUPPORTED_JAVA_LAMBDA src/` confirms the only producers are in
`src/unsupported.rs:262,265`, behind the unreachable arms 0018 documented.

The same list is also *incomplete*. Compared against `OUTPUT_MARKERS`
(`src/output_markers.rs:166`, 10 entries), the shell scan covers 5:

| registry marker | in the shell scan? |
|---|---|
| `compile_error` | yes |
| `invalid_java_cast` | yes |
| `unsupported_java_array_creation` | yes |
| `untranslated_add_all` | yes |
| `untranslated_put` | yes |
| `unsupported_inner_class_outer_capture` | **no** |
| `unmodelled_java_upcast` | **no** |
| `unmodelled_static_constant` | **no** |
| `untranslated_source_summary_method` | **no** |
| `unimplemented` | **no** |

plus two dead entries and one (`concrete_none_assignment`) that exists only
there. So the artefact named `invalid-patterns.tsv` reports 8 signals of which
2 cannot fire and 5 real ones are missing.

`tests/output_markers.rs` guarantees every *registry* entry has a fixture, and
`tests/idiom_sweep_script.rs::the_python_sweep_and_the_rust_counters_agree_on_shared_signals`
forces the Python sweep to agree with the Rust counters. Neither reaches the
shell. The obvious fix is to delete the shell's scan and read
`OUTPUT_MARKERS`-derived counts from the translator, which already computes them;
if the report is wanted standalone, it should be generated *from* the registry,
not typed alongside it.

Codex follow-up 2026-08-23: implemented. The shell's third marker list is gone.
The serial fallback asks `new-java2rust count-invalid-patterns <file.rs>` for
the same `src/output_markers.rs` registry-backed count used by
`translate-batch`, and `invalid-patterns.tsv` now reports the aggregate
registry-backed count from `status.tsv`. Regression:
`cargo test --release --test batch_translation count_invalid_patterns_cli_uses_registry_counter`;
wrapper coverage:
`cargo test --release --test batch_translation focused_package`.

---

## Why all four went unnoticed

The same shape in every case: **the reporting layer is not itself reported on.**

- The gate's output is `TARGET  BASELINE  NOW  DELTA`. A green line means "the
  two integers match", and reads as "the suite is fine".
- A skipped differential run prints to `stderr`, which the gate discards
  (`2>&1 | grep '^test result'`).
- A silent-zero metric prints a number, and a number is the one thing a human
  will not question.
- The shell marker scan produces a well-formed TSV with plausible rows.

None of these is a bug in the sense of a wrong branch; each is a *default chosen
for robustness* — don't fail on a missing file, don't fail without a toolchain,
don't fail on an unknown key — applied to a component whose entire job is to
detect absence. 0018 already named this for detectors; it is equally true of
readers, and it is why item (3) is worth doing even though every individual
`awk` line looks harmless.

---

## Direction, cheapest first

1. **Done: record failure *names*, not counts, for every target.** `gate.sh --record`
   already runs each suite; capturing the `failures:` block costs nothing and
   turns the 264-test budget into eight diffable files. Start with `fixtures`
   (20, listed above) and `slot_shape_design` (73, not yet listed) — the two
   with the most behavioural and generated-crate content.
2. **Done: make `gate.sh` diff `tests/baselines/idiom-gaps-failures.txt`.** The file
   exists and has been maintained for three weeks; nothing reads it. This is the
   smallest possible step and validates the format for (1).
3. **Done: make the differential skip observable.** Assert the fixture count, or fail
   on a missing toolchain.
4. **Done: make the three `awk` readers fail on a missing key.**
5. **Done: delete the shell's marker scan** in favour of the registry-backed counters.

Items 1 and 2 are the ones that matter: until the gate compares sets, every
other audit's fix is verified by an integer that a simultaneous regression can
restore.

## Relation to other issues

- [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md) —
  items 3 and 4 are its "detectors that silently stop matching", found in the
  readers rather than the detectors.
- [0041](0041-slot-shape-design-had-no-generated-crate-checks.md) — its
  careful note that turning on 52 checks at once *"would have moved the baseline
  by 52 and made it impossible to tell which kind of problem any future change
  caused"* is precisely the property a count-based baseline lacks and a name-based
  one has for free.
- [0052](0052-the-example-corpora-are-asserted-on-but-never-compiled.md) — the
  20 named `fixtures` failures are that file's corpus, seen from the gate side.
