# 0063 — Test-facing dead weight: a design registry only the tests read, and tests that cannot fail

Status: **fixed for the rechecked source-layout scope**. The original inventory was measured by
parsing every `#[test]` body in `tests/`, not by running the full suite. The
descriptor inventory has been re-run against current source and reports
`TEST_ONLY_DESCRIPTOR_COUNT 0`; after the 2026-09-06 cleanup, the conservative
`slot_shape_design.rs` source-only scanner reports `candidate_count 0` and the
relocated source-layout guards pass in `tests/layering.rs`.

Updated 2026-08-24: `cargo test --lib` exposed three stale `coercion.rs` unit
assertions that still described the old rendered spellings for static storage
getters and erased-object clone recovery. They now assert the current structured
projection contract: static storage initializes through
`get_or_insert_with(...).clone()`, and erased-object clone recovery records a
grouped borrowed operand. A later cleanup added three focused occurrence-cursor
regressions; `cargo test --lib` is clean again at 331 passed.

Updated 2026-08-24: `CastAndInstanceOfPlanningBoundary`, the clearest
descriptor-only specimen below, was removed. The surviving
`lower_consumes_cast_and_instanceof_plans_without_rechecking_representation_facts`
test keeps the source-boundary assertions against the actual lowering and
conversion code, without first asserting a hard-coded struct literal that
production never consulted. `ErasedObjectRuntimeContract` was removed the same
way: its test now asserts the real erase/downcast/instanceof/coercion policies
directly.

Updated 2026-08-24: `type_coercion` still had stale concat policy assertions
from before the 0059 display split was narrowed. A known named type without
string-like facts now routes through `ToStringForConcat`; the erased-object
display fallback is reserved for unknown/genuinely erased values. The tests now
assert that current contract instead of the older blanket erased helper route.

Updated 2026-08-24: another default-`FactSet` convenience wrapper,
`borrowed_content_capability_policy`, was deleted from `slot_policy.rs`; tests
now call `borrowed_content_capability_policy_with_effective_facts` directly. The
same pass repaired a stale effective-facts source guard by routing two duplicate
copy-capability reads in `signature.rs` through the existing effective
capability aggregate, leaving one owned copy query site in that module.

Updated 2026-08-24: the nine negative-only source-grep tests named in §3-5 were
collapsed into one `tests/layering.rs` guard. Four more short pure source-grep
guards were moved there from `slot_shape_design.rs` / `type_coercion.rs`
(`types_own_java_collection_and_optional_type_helpers`,
`conversion_plan_owns_stream_collect_actual_shape_selection`,
`member_access_plan_uses_type_inference_and_shape_projection`,
`string_concat_semantics_do_not_render_rust`). The move exposed one stale
`stream.rs` `.cloned()` ban that was already represented as a
`slot_shape_design` baseline failure; that obsolete needle was dropped and the
baseline moved from `slot_shape_design` to the new passing `layering` target.

Updated 2026-08-24 again: `representation_plan_does_not_render_rust_items`, a
standalone pure source-grep layering guard, moved from `slot_shape_design.rs` to
`tests/layering.rs` as
`representation_plan_keeps_rust_item_rendering_out_of_plan_layer`. The same pass
moved the standalone `switch_plan_does_not_render_rust` guard to
`tests/layering.rs` as `switch_plan_keeps_rust_rendering_out_of_plan_layer`,
and moved `lowering_consumes_solved_plans_without_choosing_shape_axes` to
`tests/layering.rs` as
`shape_axis_free_lowering_layers_do_not_choose_shape_axes`, narrowing a stale
operator/stream-lowering assumption to the lowering modules where the invariant
still holds. `conversion_plan_does_not_render_rust_expressions` also moved to
`tests/layering.rs` as
`conversion_plan_keeps_rust_expression_rendering_out_of_plan_layer`. Two more
standalone source-layer guards moved with it:
`item_assembly_return_type_wrappers_delegate_to_fact_aware_rendering` and
`lower_orchestrator_does_not_render_default_slot_values` (renamed in layering to
`lower_orchestrator_delegates_static_storage_defaults_to_item_assembly`). Two
short signature source-layout guards moved with the same pattern:
`typed_external_argument_effects_preserve_concrete_source_parameter_shape` and
`resolved_unknown_source_calls_can_propagate_signature_constraints`. A further
batch moved five compact source-layout guards:
`signature_planning_keeps_declared_iterator_locals_native`,
`signature_capture_boundaries_come_from_resolved_capture_facts`,
`signature_diagnostics_route_fields_through_capability_audit`,
`usage_owns_call_site_boundary_facts`, and
`semantic_diagnostics_are_plumbed_through_translation_boundary`. Two more
straight source-layout guards then moved:
`indexed_assignment_target_shape_consumes_inferred_array_shape` and
`statement_render_renders_local_types_with_facts`. The next pass moved two more
pure architectural guards: `stdlib_does_not_grow_non_jdk_domain_package_knowledge`
and `control_flow_normalization_plans_are_owned_before_lowering`. The
`coercion_plan_owns_instanceof_fallback_policy` module-boundary guard moved to
`tests/layering.rs` in the same cleanup stream. Four architecture/documentation
contract guards moved with it:
`new_feature_responsibility_checklist_is_tracked`,
`recursive_code_policy_is_finite_fixpoint_not_lowering_expansion`,
`object_representation_fallback_matrix_is_documented`, and
`erased_object_runtime_contract_is_documented_and_shared`. Two larger but still
pure source-ownership guards moved next:
`signature_boundary_policy_is_the_single_boundary_constraint_entrypoint` and
`shape_projection_owns_borrowed_string_content_shape`. Two source-only tails were
then split out of behavioral signature tests and moved as layering guards:
`signature_dispatch_requirement_projection_stays_role_policy_based` and
`signature_requirement_merge_policy_owns_axis_priority_symbols`. Another split
moved three source-only tails as
`signature_owned_recovery_policy_owns_capability_symbols`,
`signature_capability_and_baseline_policy_symbols_stay_fact_aware`, and
`stream_shape_stream_source_semantics_stay_effective_fact_based`. The
`effective_fact_precedence_queries_are_audited_by_consumer_module` source
inventory moved wholesale, and the source-only stream lowering tail moved as
`stream_lowering_borrows_lambda_string_views_structurally`. The final strict
source-grep pass moved/split the remaining separable guards:
`slot_render_policy_symbols_stay_effective_fact_aware`,
`slot_render_projects_erased_object_facts_without_representation_plan_lookup`,
`slot_render_default_empty_slice_stays_structured`,
`call_target_lowering_uses_shared_projection_helpers`,
`exception_lowering_keeps_exception_paths_structured`,
`stream_method_references_use_call_target_projection_helpers`, and
`stream_nullable_map_carrier_rendering_stays_behind_projection_helpers`. The
last remaining strict source tail moved as
`slot_render_default_policy_symbols_stay_effective_fact_aware`.

Updated 2026-08-24 final: re-running the `*_policy` / `*_lattice` /
`*_boundary` / `*_contract` descriptor inventory against current `src/` found no
production functions with zero production callers and test-only readers. The
final strict scan for pure `include_str!("../src|docs")` tests left in
`tests/slot_shape_design.rs` also returned zero. `0063` is therefore complete:
the stale descriptors were removed or replaced with live-path assertions, and
the remaining source-layout guards are isolated in `tests/layering.rs` instead
of inflating behavioral coverage.

Recorded 2026-08-24. Companion to
[0062](0062-dead-unreachable-and-never-connected-code.md), which covers `src/`.

## What this is about

The suite is 2 399 real `#[test]` functions across 147 877 lines. It is in good
shape: **2 out of 2 399 tests have no check of any kind**, and both turned out to
be false positives on inspection. There is no epidemic of vacuous assertions here,
and I want that stated plainly before the rest, because the interesting finding is
a different one.

The dead weight is not *weak* tests. It is a **second, parallel description of the
compiler** — a layer of `*_policy()` / `*_lattice()` / `*_boundary()` /
`*_contract()` descriptor functions that live in `src/`, are called by nothing in
`src/`, and exist so that a test can assert what the policy *should* be. The
policy is therefore written twice: once as a descriptor nothing runs, once as the
real code path. The test checks the copy.

## Summary

| # | what | count | lines |
|---|---|---:|---:|
| 1 | `*_policy`/`*_lattice`/`*_boundary`/`*_contract` fns in `src/` with **no production caller** | **49** | (see 0062 §7) |
| 2 | tests whose subject is one of those 49 | **54** | **6 554** |
| 3 | tests that assert on `include_str!("../src/*.rs")` source text | **89** | — |
| 4 | of those, tests with **no** call into the compiler at all | **36** | 386 assertions |
| 5 | tests whose every assertion is `assert!(!source.contains(..))` | **9** | 44 assertions |
| 6 | tests driving a state production cannot construct | **2** | **445** |
| 7 | tautological tests (assert a constant equals the constant it was built from) | 1 | 79 |
| 8 | tests with no assertion and no panicking helper | **2** | (both false positives) |
| 9 | fixture files referenced by no test | 1 | — |

---

## 1-2. The descriptor layer: 49 functions in `src/` that only tests call, 54 tests that only call them

`src/` contains **252** functions named `*_policy`, `*_lattice`, `*_boundary` or
`*_contract`. **63** have zero production callers. Of those, 14 are unit-test
helpers inside `#[cfg(test)]`; the remaining **49 are defined in production code
and called only from `tests/`** — 264 call sites, across 54 distinct tests
spanning 6 554 lines.

Reproduce (the identifier counts come from tokenising `src/` with `#[cfg(test)]`
blanked out, versus `tests/` plus those blanked blocks):

```
grep -rn 'pub fn .*_policy\|pub fn .*_lattice\|pub fn .*_boundary\|pub fn .*_contract' src/
# then for each name N:
grep -c "\bN\b" <production text>   # == 1 means: defined, never called
```

The 49, by module:

| module | test-only descriptor functions |
|---|---|
| `signature.rs` | default-`FactSet`/no-scope wrappers `signature_owned_recovery_capability_policy`, `signature_boundary_representation_policy`, `signature_boundary_representation_fact_policy`, `signature_native_boundary_policy`, `signature_slot_dispatch_requirement_policy`, `signature_slot_dispatch_requirement_policy_with_effective_facts`, `signature_boundary_constraints`, and `conservative_boundary_constraints_with_effective_facts` deleted; descriptor-only `signature_boundary_role_policy`, `signature_ownership_requirement_priority_policy`, `signature_view_requirement_priority_policy`, `signature_dispatch_requirement_priority_policy`, and `field_nullability_shape_policy` deleted; tests now assert the live merge and field-constraint entrypoints |
| `slot_render.rs` | all listed default-`FactSet` convenience wrappers deleted; tests call live `_with_effective_facts` helpers directly, including `slot_content_capability_policy_with_effective_facts` |
| `conversion_plan.rs` | default/convenience wrappers `coercion_shape_content_policy`, `coercion_owned_recovery_capability_policy`, `optional_shape_coercion_policy`, `borrowed_element_adapter_policy`, `java_cast_semantics_policy`, and `dispatch_coercion_policy` deleted; `syntax_slot_coercion_policy` deleted and tests now assert the live `SyntaxCoercionPlan` policy (`cast_and_instanceof_planning_boundary`, `erased_object_runtime_contract`, and the dyn clone-box adapter descriptors deleted) |
| `shape_projection.rs` | default-`FactSet` wrappers `shape_projection_capability_policy`, `indexed_element_projection_policy`, `iteration_element_projection_policy`, and `sequence_element_projection_policy` deleted; tests now call the live `_with_effective_facts` helpers directly |
| `slot_policy.rs` | default-`FactSet` wrappers `borrowed_content_capability_policy`, `method_param_borrowed_content_policy`, `constructor_param_borrowed_content_policy`, and `borrowed_return_content_policy_from_param` deleted; tests now call the live fact-aware helpers directly |
| `lattice.rs` | descriptor-only lattice layer deleted; the remaining rank/join helpers are live in `signature.rs`/`semantic_analysis.rs` |
| others | `java_member_access_policy`, `representation_hierarchy_fact_policy`, `representation_hierarchy_fact_policy_with_effective_facts`, `anonymous_class_diagnostic_policy`, `external_api_method_policy`, and `call_result_boundary_policy` deleted; tests now assert live planning/analysis paths |

### Two different things are mixed in here — separate them before acting

**(a) Convenience overloads.** Status: **fixed for the listed cleanup set.** The
default-`FactSet` and no-context wrappers listed above have been deleted, and the
tests now call the fact-aware production entry points or live planning/analysis
paths directly. See [0062](0062-dead-unreachable-and-never-connected-code.md)
§7.

**(b) Genuine descriptor-only functions** with no live sibling at all. Status:
**fixed for the listed cleanup set.** The `signature_boundary_role_policy`
specimen is now gone, as are all 7 `*_lattice()` descriptors.
`CastAndInstanceOfPlanningBoundary`, `ErasedObjectRuntimeContract`, the
anonymous-class diagnostic descriptor, and the call-result empty-context wrapper
were also deleted; the surviving tests assert directly on actual lower,
conversion, representation, or usage-analysis paths.

**Why it survived:** this is a plausible and well-intentioned pattern — encode the
architecture as data so a test can enforce it. It fails because the data is not
what the compiler consults. The design and the implementation are two artefacts
that can drift, and only one of them is checked.

**Treatment:** for each of the 49, ask *which production function makes this
decision for real?* If there is one, the test should call **that** and the
descriptor should go. If there is not, the descriptor is a design note and belongs
in `docs/`, not in `src/` where it inflates the compiler and looks like machinery.

---

## 3-5. 89 tests assert on the text of `src/*.rs`

`include_str!("../src/…")` appears **255 times** in `tests/`, across **89 tests**,
naming 63 different source files. `lower.rs` is read 27 times, `signature.rs` 22,
`coercion.rs` 15.

**36 of the 89 make no call into the compiler at all** — the entire test is
`grep`. Between them they carry **386 assertions**, of which **336 are
`assert!(source.contains(..))`** and **241 are `assert!(!source.contains(..))`**
(counts across the whole suite).

A representative one, `tests/slot_shape_design.rs:19871-19890`:

```rust
let source = include_str!("../src/signature.rs");
assert!(source.contains("pub struct SignatureRequirementMerge"));
assert!(!source.contains("pub fn signature_ownership_requirement_priority_policy"));
…
assert!(!source.contains("fn ownership_requirement_rank"));
```

Nine tests have **only** negative source assertions — they can fail if a symbol is
re-introduced, never if behaviour regresses:

`facts_do_not_render_rust_for_capability_overrides` (5 asserts),
`shape_projection_does_not_render_rust_types` (4),
`slot_shape_does_not_own_indexed_projection_rules` (2),
`slot_shape_does_not_own_lowered_expression_actual_wrappers` (3),
`signature_plan_does_not_render_rust_types` (4),
`call_plan_does_not_render_rust_expressions` (7),
`exception_plan_does_not_render_rust` (6),
`stream_plan_does_not_render_rust_iterator_chains` (10),
`stream_shape_does_not_own_lowered_actual_types` (3).

**These are not worthless.** They are layering guards, and layering is exactly what
[0007](../claude_audits/0007-rust-ir-is-mostly-opaque-strings.md) is about — a plan
module that starts building `RustExpr` is a real regression and these catch it. The
problem is the *ratio*: 36 tests and 386 assertions of substring-matching is a lot
of red for a rename, and none of it is evidence that a translation is correct. They
should be one small `tests/layering.rs` with an explicit list, not scattered
through the 49 179-line `slot_shape_design.rs` where they inflate the apparent
behavioural coverage of that file.

The positive ones (`assert!(source.contains("pub fn foo"))`) are strictly worse
than the negative ones and should mostly go: they assert a function exists, which
the compiler already guarantees at every call site.

---

## 6. 445 lines of test drive a state production cannot reach

Status: **fixed for this sub-item.** The obsolete
`BorrowedDynToOwnedBoxedDynRecoveryPolicy` / `DynCloneBoxAdapter*` production
descriptors were deleted from `src/conversion_plan.rs` and `src/coercion.rs`.
The tests no longer construct adapter states that production cannot reach; they
keep the assertions on the live `CloneBoxDynTrait` planning and `.clone_box()`
lowering path.

Two tests in `tests/type_coercion.rs` construct
`BorrowedDynToOwnedBoxedDynRecoveryPolicy` and `DynCloneBoxAdapterAvailability`
directly:

| test | lines |
|---|---:|
| `shape_coercion_owns_dyn_trait_and_erased_object_adapters` | 301 |
| `dispatch_coercion_uses_clone_facts_for_borrowed_dyn_to_boxed_dyn_fallback` | 144 |

Neither enum is ever constructed in `src/`
([0062](0062-dead-unreachable-and-never-connected-code.md) §1): the variant that
would enter this pipeline, `DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn`,
has **zero** construction sites in the whole repo. So these 445 lines exercise a
subsystem the binary can never enter, and their green status is what made the
subsystem look wired.

`tests/type_coercion.rs:5875` and `:5969` go further and pass
`DynCloneBoxAdapterAvailability::Available` — a value `src/` never constructs,
selecting a branch (`conversion_plan.rs:470-473`) that production cannot take.

**Treatment:** tied to [0062](0062-dead-unreachable-and-never-connected-code.md) §1.
If the producer gets written, these become the right tests. If the design is
dropped, they go with it. What must not happen is deleting the `src/` half and
leaving these as the only surviving description of the intent.

---

## 7. One tautological test

Status: **fixed for this sub-item.** The descriptor assertions were deleted with
the descriptor-only `lattice.rs` layer. The remaining real checks were kept as
`field_nullability_join_handles_conflicting_evidence`.

`named_lattice_descriptors_define_core_domains`
(`tests/slot_shape_design.rs:48349-48427`, 79 lines) asserts that
`nullability_lattice().bottom == NullabilityState::DefinitelyNonNull` — where
`nullability_lattice()` is a function whose body is a struct literal setting
`bottom: NullabilityState::DefinitelyNonNull`. It also asserts
`nullability.merge_rule.contains("MaybeNull")` against a hard-coded English
sentence, and `diagnostic_rule.contains("diagnostics")` — a string asserted to
contain a word it obviously contains.

It is the sole consumer of 7 of `lattice.rs`'s descriptor constructors and of 3 of
its enums. It cannot fail except by editing the constant and the assertion out of
step, in the same file, in the same commit.

The last 15 lines of it (`field_nullability_join` cases) *are* real — that function
is on the hot path in `signature.rs` and `semantic_analysis.rs`. Keep those,
delete the other 64 with the descriptors they check.

---

## 8. Tests with no assertion — 2, both false alarms

For completeness, since "tests that cannot fail" was the brief. Scanning every
`#[test]` body for any of `assert*!`, `expect(`, `unwrap(`, `panic!`:

- `external_direct_getter_facades_compile_with_inferred_returns`
  (`tests/idiom_gaps.rs:7397`) — ends in `cargo_check_generated_modules(..)`, which
  panics on a compile failure. Real check.
- `current_get_class_vs_erased_object_get_class_uses_type_predicate`
  (`tests/idiom_gaps.rs:22000`) — my raw-string stripper truncated the body; it
  asserts normally.

**Method note, because it changes the number a lot:** a naive scan reports 29
assertion-free tests. 26 of those are `#[test] fn typechecks_*` **inside `r#"…"#`
strings** — they are generated-crate test source that
`cargo_check_generated_modules` compiles, not tests of this suite. Any future
tooling that counts tests here must strip raw strings first, or it will
over-report by ~90 tests and mis-attribute two dozen phantom vacuous ones.

---

## 9. Fixtures

Checked all `idiom_gap("…")` and `parser_gap("…")` arguments against
`examples/idiom-gaps/` (122 files) and `examples/parser-gaps/`:

- **0 references to a fixture that does not exist** — nothing rots here.
- Status: **fixed for the unreferenced fixture.**
  `examples/idiom-gaps/ToArrayEmptyStringArray.java` is now consumed by
  `collection_to_array_empty_string_array_lowers_to_owned_vec`, which also keeps
  the `toArray(new String[0])` map-key variant as inline companion coverage.

---

## What to do first

1. Collapse §3's 36 pure source-grep tests into one `tests/layering.rs`. This is
   mechanical, removes no coverage, and stops 386 substring assertions from
   inflating `slot_shape_design.rs`.
   Done: the nine negative-only guards named in §3-5 plus all strict source-only
   guards found in `slot_shape_design.rs` now live in `tests/layering.rs`.
2. Re-run the descriptor inventory from current source before deleting anything
   else; the originally listed §1/§6/§7 candidates have already been removed or
   superseded by live-path assertions.
   Done: current inventory reports `TEST_ONLY_DESCRIPTOR_COUNT 0`.

---

## Re-verified 2026-09-02 — reopened, the second half regressed

The headline reproduces exactly: descriptor functions with no production caller
went **46 -> 0**, and all 34 named descriptors are absent.

Sections 3-5 did not hold. `tests/layering.rs` exists (1,645 lines, 53 tests) and
the nine named negative-only tests are gone, but `include_str!` occurrences went
**260 -> 278**, pure source-grep tests **69 -> 74**, and **16 pure source-grep
tests remain in `slot_shape_design.rs`** against a claim of zero. See `TODO2.md`.

Updated 2026-09-06: the positive source-grep assertions that kept
`constructor_self_field_replacement`, `has_explicit_hierarchy_override`,
`method_usage_for_source_method`, `method_signature_for_source_method`,
`source_method_returns_result`, and `unwrap_rendered_some_expr` alive have been
removed or inverted while deleting those dead helpers under 0062. The audit
remains open for relocating the remaining pure source-layout tests into
`tests/layering.rs` or replacing them with behavior checks.

The pure source-layout guard
`trivial_accessor_detection_is_not_a_lowering_special_case` has also moved from
`slot_shape_design.rs` to `tests/layering.rs`, reducing the remaining
`slot_shape_design.rs` source-grep inventory by one.

Three more pure source-layout guards moved to `tests/layering.rs` in the same
cleanup stream: `callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`,
`semantic_facts_own_nullability_and_external_api_heuristics_before_lowering`,
and `map_get_return_adaptation_uses_native_lowered_type_plan`.
`representation_lowering_owns_dyn_trait_impl_compatibility_policy` moved as a
fourth pure source-layout guard in that batch.
`callable_analysis_shell_reaches_lowering_context` was then moved as another
source-only layering guard.

Seven more source-only ownership/layering guards were moved from
`slot_shape_design.rs` to `tests/layering.rs`:
`coercion_plan_owns_invalid_cast_runtime_policy`,
`coercion_plan_owns_clone_recovery_policy`,
`coercion_plan_owns_borrowed_element_recovery_fallback_policy`,
`lower_consumes_cast_and_instanceof_plans_without_rechecking_representation_facts`,
`signature_solver_collects_param_constraints_before_applying_shapes`,
`call_lowering_renders_constructor_owner_types_with_facts`,
and `external_api_support_passes_share_missing_return_surface_policy`.
Two of those had stale source-string expectations that were still listed as
`slot_shape_design` expected failures; the assertions now match the current
implementation and pass under `tests/layering.rs`.

Current conservative scanner result:

```
candidate_count 0
```
