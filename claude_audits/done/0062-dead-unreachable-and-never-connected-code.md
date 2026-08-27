# 0062 — Dead, unreachable and never-connected code in `src/`

Status: **fixed**. The original scan was grep + construction/pattern
classification rather than `rustc` dead-code output; later cleanup passes
re-ran the scans, removed the remaining accidental dead surface, and documented
the few intentionally retained public/identity helpers.

Updated 2026-08-24 final pass: the production zero-caller scan no longer has
accidental dead public wrappers after excluding documented public inspection
surface, and the unread-field scan is down to `BindingId.occurrence`. That field
is intentionally retained because [0061](../0061-a-local-binding-has-no-identity-so-the-plan-is-rejoined-by-name-four-times.md)
uses it as the planned binding identity key; deleting it would preserve today's
name-rejoin bug rather than fix dead code. Two remaining descriptor echo fields
in `shape_projection` (`EscapingRoleDispatchPolicy.source_dispatch` and
`DispatchRequirementProjectionPolicy.source_dispatch`) were removed in this pass.

Updated 2026-08-24: another test-only default wrapper was retired:
`access_lowering::lower_field_access` had no production caller and only four
test callers beside the live fact-aware `lower_field_access_with_facts` entry
point. The tests now pass `&FactSet::default()` explicitly, and the wrapper is
gone. This is one more item removed from the historical §7 wrapper inventory; do
not treat the original wrapper count as a current count.

Updated 2026-08-24: retired four more test-only default-facts item assembly
wrappers: `struct_item_for_type`, `constructor_function_shell`,
`constructor_default_field_value`, and `method_function_shell`. The surviving
tests now call the facts-aware entry points with an explicit
`FactSet::default()`. Re-running the current wrapper scan reports 11 test-only
base wrappers: `default_expr_for_slot` (two fact-aware siblings),
`emit_stream_lambda_body`, `external_api_facade_support_items`,
`indexed_assignment_shape`, `infer_assignment_target_shape`,
`infer_single_input_method_reference_output_shape`, `plan_object_model`,
`render_stream_plan`, and the intentionally-retained `translate_source`.

Updated 2026-08-24: retired the test-only
`shape_projection::indexed_assignment_shape` wrapper as well. Its behavioral
test now calls `indexed_assignment_shape_with_facts(..., &FactSet::default())`
directly, and `shape_projection.rs` no longer imports the raw
`sequence_element_type` helper for that default path. The current wrapper scan is
down to 9 test-only base wrappers: `default_expr_for_slot` (two fact-aware
siblings), `emit_stream_lambda_body`, `external_api_facade_support_items`,
`infer_assignment_target_shape`, `infer_single_input_method_reference_output_shape`,
`plan_object_model`, `render_stream_plan`, and the intentionally-retained
`translate_source`.

Updated 2026-08-24: retired the test-only
`expr_shape::infer_assignment_target_shape` wrapper. The two tests that used it
now call `infer_assignment_target_shape_with_context(...,
ExprShapeContext::default())`, making the no-context configuration explicit. The
current wrapper scan is down to 8 test-only base wrappers:
`default_expr_for_slot` (two fact-aware siblings), `emit_stream_lambda_body`,
`external_api_facade_support_items`,
`infer_single_input_method_reference_output_shape`, `plan_object_model`,
`render_stream_plan`, and the intentionally-retained `translate_source`.

Updated 2026-08-24: retired the test-only
`method_reference_shape::infer_single_input_method_reference_output_shape`
wrapper. Tests now call
`infer_single_input_method_reference_output_shape_with_context(...,
&TypeEnv::new(), &FactSet::default())`, and the source-layout guard tracks only
the context-aware entry point. The current wrapper scan is down to 7 test-only
base wrappers: `default_expr_for_slot` (two fact-aware siblings),
`emit_stream_lambda_body`, `external_api_facade_support_items`,
`plan_object_model`, `render_stream_plan`, and the intentionally-retained
`translate_source`.

Updated 2026-08-24: retired the test-only
`stream_lowering::emit_stream_lambda_body` wrapper. Tests now call
`emit_stream_lambda_body_with_context(..., StreamRenderContext::empty())`
directly for the empty-context case. That exposed one stale assertion:
`value.trim()` currently lowers to `value.trim().to_string()`, preserving Java
`String.trim()` as an owned string result. The current wrapper scan is down to 6
test-only base wrappers: `default_expr_for_slot` (two fact-aware siblings),
`external_api_facade_support_items`, `plan_object_model`, `render_stream_plan`,
and the intentionally-retained `translate_source`.

Updated 2026-08-24: retired the test-only
`stdlib::external_api_facade_support_items` wrapper. Tests now call
`external_api_facade_support_items_with_local_type_program` directly, with a
small test helper where that avoids repeating the default local-program,
summary-program, and representation arguments. The current wrapper scan is down
to 5 test-only base wrappers: `default_expr_for_slot` (two fact-aware siblings),
`plan_object_model`, `render_stream_plan`, and the intentionally-retained
`translate_source`.

Updated 2026-08-24: retired the test-only `slot_render::default_expr_for_slot`
wrapper. Tests now call `default_expr_for_slot_with_facts` directly with an
explicit `FactSet::default()` or the existing local default facts. The current
wrapper scan is down to 3 test-only base wrappers: `plan_object_model`,
`render_stream_plan`, and the intentionally-retained `translate_source`.

Updated 2026-08-24: retired the test-only `object_model::plan_object_model`
wrapper. Object-model tests now use a local helper that constructs
`EffectiveFacts` and calls `plan_object_model_with_effective_facts`; the
semantic-facts regression does the same inline. The current wrapper scan is down
to 2 test-only base wrappers: `render_stream_plan` and the
intentionally-retained `translate_source`.

Updated 2026-08-24: retired the test-only `stream_lowering::render_stream_plan`
wrapper. Tests now call
`render_stream_plan_with_context(..., StreamRenderContext::empty())` directly.
The pass exposed stale stream expectations around owned Java `String.trim()`
results and method-chain grouping; those tests now assert the current structured
output. The current wrapper scan is down to one test-only base wrapper:
`translate_source`, which is intentionally retained as the public convenience API
for integration tests.

Updated 2026-08-24: re-ran the zero-caller public function scan. It reported 29
current candidates; three clearly superseded wrappers were deleted:
`object_strategy_for_type_with_effective_facts`,
`external_static_string_constant_support_items`, and
`external_static_string_constant_support_items_with_local_type_program_and_declared_owners`
(plus the intermediate local-type wrapper made dead by deleting the base). The
live static-constant support entry point remains
`external_static_string_constant_support_items_with_context_programs_and_declared_owners`,
and the stale docs reference now names it. The current zero-caller scan reports
26 candidates.

Recorded 2026-08-24. Companion: [0063](0063-test-facing-dead-weight.md)
covered the test-side half and is now fixed.

## Why grep and not `rustc`

The crate is a `[lib]` + `[bin]` pair. Almost everything in `src/` is `pub`, and
`tests/` is an external consumer of the lib, so `dead_code` fires for essentially
nothing: a `pub fn` that no production code calls but one test calls is *live* to
rustc and *dead* to the compiler being built. That is the single reason this
report exists — **rustc cannot see the deadness that matters here.** Where a
finding could have been produced by `rustc` I say so.

Convention used throughout:

- **production** = `src/` with `#[cfg(test)]` blocks blanked out.
- **test-only reader counts as no reader.** Where a test is the only consumer it
  is stated explicitly, because that is the interesting case, not an exemption.

---

## Summary

| # | what | kind | lines |
|---|---|---:|---:|
| 1 | dyn-clone-box adapter recovery: unreachable from the one variant nobody builds | fixed | **226** |
| 2 | `plan_handle_adaptation` is inert by construction (`Wrap` never built) | fixed | 34 |
| 3 | external-pack object-safety declarations: 0 of 61 packs use them | fixed | 80 |
| 4 | `lattice.rs` descriptor layer: 0 production readers | superseded | **229 of 326** |
| 5 | `FactKind` (28 variants) and `RewriteNeed` (4 variants): 0 references anywhere | fixed | 37 |
| 6 | 39 `pub fn` with **no caller in `src/` or `tests/`** | fixed/retained | **472** |
| 7 | 74 `X(..)` shims whose only callers are tests, beside a live `X_with_facts(..)` | superseded | 561 |
| 8 | 6 further unreachable match arms (variant matched, never constructed) | fixed | ~40 |
| 9 | 44 struct fields with no `.field` read in `src/` **or** `tests/` | fixed/retained | 44 |
| 10 | `src/fixtures.rs`: a test helper shipped inside the production library | misplaced | 33 |
| 11 | one stale `#[allow(dead_code)]`, one hiding two unread fields | — | 3 |

Rough deletable total: **~1 700 lines**, without changing a single translation.

**Not a finding — checked and clean:** every CLI flag in `src/main.rs`. See
[Appendix A](#appendix-a-the-cli-is-clean). This was the dimension the brief
flagged as highest-risk (`--no-builtin-external-packs` was a verified no-op); it
was fixed by [0050](../claude_audits/done/0050-builtin-pack-facts-and-name-aliases-rebuilt-per-call.md)
and I re-verified it empirically rather than take the note on trust.

---

## The distinction that matters

Two of these categories look identical in a grep and need opposite treatment.

**Dead because superseded** — a second implementation won and this one was left
behind. Deleting it is pure profit. Items 4, 7, 10, and most of 6.

**Dead because a feature was never finished** — the *consumer* exists, sometimes
with a full match over every case, and the *producer* was never written. Deleting
it silently discards a design and the only surviving record of a known gap. These
are missing features wearing dead code's clothes: items 1, 2, 3, 5, and parts of
8. Each one below says which, and what the missing producer would have to build.

---

## 1. The dyn-clone-box adapter recovery is unreachable — nobody builds the variant that enters it

Status: **fixed for this sub-item.** The production path had already moved to a
reachable `DispatchCoercionPolicy::CloneBoxDynTrait` / `Coercion::CloneBoxDynTrait`
boundary that renders `.clone_box()` for borrowed-dyn to owned-boxed-dyn slots.
The obsolete deferred fallback design was therefore removed instead of wired:
`DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn`,
`BorrowedDynToOwnedBoxedDynRecoveryPolicy`, the `DynCloneBoxAdapter*`
descriptor enums/functions, and the duplicate conversion/coercion diagnostic
matches are gone. The two `tests/type_coercion.rs` tests now assert only the live
planning/lowering path; the direct construction of unavailable adapter states was
deleted. `docs/rust-slot-shape-and-object-representation.md` now describes the
current `clone_box()` coercion instead of the removed deferred-generation model.

Historical evidence follows.

**Never finished.** Also the largest single duplication in the tree: the same
36-line decision is written twice, in two files, and both copies are unreachable.

`DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn` (`src/conversion_plan.rs:395`)
is **never constructed**, in `src/` or in `tests/`:

```
$ grep -rn 'DispatchCoercionFallbackPolicy::' src/ tests/ | grep BorrowedDynToOwnedBoxedDyn
src/conversion_plan.rs:2461:        DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
src/coercion.rs:641:        DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
```

Both hits are `match` arms. The four sites that *do* construct the enum
(`conversion_plan.rs:374, 377, 2164-2244, 2312`) build only `ErasedObjectToDynTrait`,
`ErasedObjectToBoxedDynTrait`, `MissingRuntimeTarget` and `CompileError`.

Everything reachable only through that variant is therefore dead:

| item | `src/conversion_plan.rs` | lines |
|---|---|---:|
| `BorrowedDynToOwnedBoxedDyn` variant | 394-398 | 5 |
| `enum BorrowedDynToOwnedBoxedDynRecoveryPolicy` | 407-411 | 5 |
| `enum DynCloneBoxAdapterRequirement` | 413-425 | 13 |
| `enum DynCloneBoxAdapterAvailability` | 427-431 | 5 |
| `enum DynCloneBoxAdapterGenerationPolicy` | 433-446 | 14 |
| `enum DynCloneBoxAdapterDeferralReason` | 448-452 | 5 |
| `dyn_clone_box_adapter_requirement` | 454-463 | 10 |
| `dyn_clone_box_adapter_requirement_with_availability` | 465-488 | 24 |
| `dyn_clone_box_adapter_generation_policy` | 490-514 | 25 |
| `dyn_clone_box_adapter_generation_policy_with_availability` | 516-526 | 11 |
| `dyn_clone_box_adapter_deferral_message` | 528-552 | 25 |
| match arm | 2461-2464 | 4 |
| `borrowed_dyn_to_owned_boxed_dyn_diagnostic` | 2483-2514 | 32 |
| **`src/coercion.rs`** | | |
| match arm | 641-644 | 4 |
| `borrowed_dyn_to_owned_boxed_dyn_message_policy` | 664-698 | 35 |
| | **total** | **~217** |

`borrowed_dyn_to_owned_boxed_dyn_diagnostic` and
`borrowed_dyn_to_owned_boxed_dyn_message_policy` are the same three-arm match over
the same enum, differing only in whether they build a `ConversionDiagnostic` or an
`UnsupportedCoercionMessagePolicy` — with the format strings copied verbatim
between them. This is the same species as
[0047](../claude_audits/done/0047-facade-slot-algebra-is-nine-hand-copied-recursions.md),
except neither copy runs.

Two arms are dead **twice over**: `DynCloneBoxAdapterAvailability::Available` is
only ever read by `matches!` at `conversion_plan.rs:470` and never built in `src/`,
and `dyn_clone_box_adapter_generation_policy` maps `RequiresGeneratedAdapter` to
`DeferredUnsupported` unconditionally — so
`DynCloneBoxAdapterGenerationPolicy::GenerateAdapter` cannot be produced either,
and its two arms (`coercion.rs:678`, `conversion_plan.rs:2498`) are unreachable
even from inside dead code.

### Why it survived

`DynCloneBoxAdapterDeferralReason::GenerationNotImplemented` says out loud that
the generation step was deferred. What was *also* deferred is the producer: the
site that decides "this is a `&dyn T` reaching an owned `Box<dyn T>` slot" and
emits `BorrowedDynToOwnedBoxedDyn`. Tests construct the recovery policy directly
(`tests/type_coercion.rs:5846, 5872, 5948, 5964`), which made the pipeline look
exercised while its entry point was never wired.

### Treatment

Completed by recording the current producer (`CloneBoxDynTrait`) and removing
the superseded unreachable deferred-adapter design. The remaining live risk is
whether every generated dyn trait actually has a valid `clone_box()` surface;
that is a runtime-support/representation contract issue, not this dead-code item.

---

## 2. `plan_handle_adaptation` cannot return anything but `None` — and says so

Status: **fixed for this sub-item.** The inert hook was deleted:
`HandleAdaptation`, `SlotConversionPlan::Handled`, `plan_handle_adaptation`, and
`apply_handle_adaptation` are gone. Shared-handle wrapping now remains in the
representation-aware type/value rendering paths described in 0002b, not in a
slot-conversion wrapper that always returned `None`.

**Never finished**, and unusually honest about it.
`src/conversion_plan.rs:3322-3352`:

```rust
    // Inert for now. Handle wrapping currently lives in rendering decisions,
    // not in `RustSlotShape`, ...
    let _ = (placement_of(actual), target.and_then(placement_of));
    HandleAdaptation::None
```

The function computes both placements and discards them. Consequences:

- `HandleAdaptation::Wrap` (`conversion_plan.rs:3054`) is never constructed —
  `grep -rn 'HandleAdaptation' src/` returns 9 lines, none a construction of `Wrap`.
- therefore `SlotConversionPlan::Handled` is never constructed either
  (`conversion_plan.rs:3372` is inside `match ... { None => inner, handle => Handled {..} }`,
  whose non-`None` arm is unreachable),
- therefore three consumer arms are unreachable: `conversion_plan.rs:2994`,
  `coercion.rs:1148`, and `apply_handle_adaptation`'s `Wrap` arm at `coercion.rs:1118`.

**Treatment:** completed by deleting the stale in-code hook and preserving the
failure history in this audit/0002b instead.

---

## 3. External-pack object-safety declarations: a complete feature, used by zero packs

Status: **fixed for this sub-item.** Runtime external facts/manifests now declare
object-safety facts directly through the parsed `ObjectSafetyFact` path
(`push_external_pack_object_safety_fact` /
`parse_external_pack_object_safety_violation` in `external_packs.rs`), and
dependency-mode tests cover configured provider object-safety facts. The
compile-time `ExternalPackObjectSafety` wrapper, its violation enum, the empty
`NO_OBJECT_SAFETY` built-in-pack field, and the manifest coverage comparison
against that static field were deleted. Do not treat the historical evidence
below as evidence that object-safety cannot be declared by external metadata.

**Never finished.** `ExternalPackObjectSafety` (`src/external_packs.rs:125-129`)
and `ExternalPackObjectSafetyViolation` (9 variants, `132-154`) exist, are read by
`push_facts` → `as_object_safety_violation` (`1689-1732`, a 44-line total mapping
onto `ObjectSafetyViolation`), and are reachable from
`ExternalPack::object_safety`. Nothing populates them:

```
$ grep -c 'object_safety: NO_OBJECT_SAFETY' src/external_packs.rs
61
$ grep -n 'object_safety:' src/external_packs.rs | grep -v NO_OBJECT_SAFETY
101:    pub object_safety: &'static [ExternalPackObjectSafety],     # the field decl
```

All 61 built-in packs pass the empty slice. The fields are `&'static str`, so a
runtime-parsed manifest **cannot** construct one either — the only possible
producer is a compile-time pack table, and none does it. The single construction
in the whole repo is `tests/external_packs.rs:1509`.

Dead: ~30 lines of type + 44 lines of mapper + the two readers at
`external_packs.rs:360` and `1057` ≈ **80 lines**.

Former sibling with the same shape: `ExternalPackObjectStrategyKind::ClosedEnum`
used to be matched but not constructed by the const built-in pack factories.
That part is now stale: parsed external-pack manifests construct closed-enum
hierarchy strategies through `external-pack-object ... closed-enum ...`, covered
by `tests/external_packs.rs`.

**Treatment:** completed by deleting the empty compile-time pack API and keeping
the parsed metadata route as the single external-pack object-safety producer.

---

## 4. `lattice.rs`: descriptor-only layer had no production reader

Status: **fixed for this sub-item.** The descriptor-only lattice layer
(`LatticeDescriptor`, unused value-domain enums, value arrays, descriptor
constructors, `dispatch_lattice_value`, and unused rank helpers) was deleted.
The live ranked join/rank functions and `field_nullability_join` remain.

**Superseded**, and the tree already admits it. `src/storage_strategy.rs:73-76`:

```rust
/// Ordered: a later variant subsumes an earlier one.
```

Cross-referencing every `pub` item in `lattice.rs` against `src/` (excluding
`lattice.rs` itself) and `tests/`:

| item | production readers | test readers |
|---|---|---|
| `join_ranked_lattice`, `ownership_lattice_rank`, `view_lattice_rank`, `dispatch_shape_lattice_rank`, `storage_nullability_lattice_rank`, `field_nullability_join`, `RankedLatticeJoin` | `signature.rs`, `semantic_analysis.rs` | yes |
| `LatticeDescriptor`, `RankedLatticeJoin`'s descriptor role | **none** | none |
| `DispatchLatticeValue`, `EscapeLatticeValue`, `InitializationLatticeValue` | **none** (only a doc comment) | `slot_shape_design.rs` |
| 8 × `*_VALUES` consts | **none** | **none** |
| 7 × `*_lattice()` descriptor constructors | **none** | `slot_shape_design.rs` |
| `dispatch_lattice_value` | **none** | `slot_shape_design.rs` |
| `receiver_mutability_lattice_rank`, `escape_lattice_rank`, `initialization_lattice_rank` | **none** | **none** |

Reproduce:

```
for n in LatticeDescriptor DISPATCH_VALUES escape_lattice initialization_lattice_rank; do
  echo "$n: $(grep -rlw "$n" src/*.rs | grep -v lattice.rs | tr '\n' ' ')"
done
```

Dead lines at audit time: 5-16, 24-52, 54-111, 113-217, 302-326 = **229 of
326**. That descriptor layer is gone. The remaining module is intentionally
small and live: ranked slot-shape joins are used by `signature.rs`, and
`field_nullability_join` is used by both `signature.rs` and
`semantic_analysis.rs`, so there is no current follow-up to delete `lattice.rs`
or move it into one caller.

The 8 `*_VALUES` constants are the sharpest case: they have **no reader at all**,
not even a test. They are a hand-maintained mirror of each enum's variant list,
which means they can silently drift out of sync with the enum they describe and
nothing will notice.

---

## 5. `FactKind` and `RewriteNeed`: declared, never referenced

Status: **fixed for this sub-item.** `FactKind` and the unused top-level
`RewriteNeed` enum were deleted. The live concrete rewrite plan types in
`rewrite.rs` remain.

**Never connected.** Two `pub enum`s that nothing in the repo mentions outside
their own declaration.

```
$ grep -rn '\bFactKind\b'   src/ tests/ scripts/ docs/ | wc -l
2        # src/facts.rs:765 (the decl) + this line's own doc reference
$ grep -rn '\bRewriteNeed\b' src/ tests/ scripts/ docs/ | wc -l
1        # src/rewrite.rs:2 (the decl)
```

- `FactKind` — `src/facts.rs:764-793`, **28 variants**, 30 lines. It enumerates
  every fact category the analyser produces. Nothing tags, dispatches on, or
  counts by it. Note the shape: `FactSet` has ~28 parallel `Vec<...>` fields and
  this enum is the abstraction over them that was never adopted.
- `RewriteNeed` — `src/rewrite.rs:1-7`, 4 variants, 7 lines
  (`LoopContinueUpdatePath`, `BorrowConflict`, `EscapingInnerCapture`,
  `ComplexFinally`). `rewrite.rs` went on to implement two concrete rewrites
  (`ControlFlowRewritePlan::{AssignmentInCondition, TailRecursiveLoop}`) and never
  looked back at the classification.

`rustc` *could* have caught both had they not been `pub` in a lib crate. This is
the "over-wide visibility hides deadness" case in its purest form.

**Treatment:** `RewriteNeed`'s four names are a to-do list for control-flow
rewrites — worth keeping as prose, not as an enum. `FactKind` is safe to delete.

---

## 6. 39 `pub fn` with no caller in `src/` **or** `tests/` — 472 lines

Status: **partially fixed for this sub-item.** Six verified zero-caller
  functions were deleted after re-checking the current tree and confirming
  `cargo check --release` still passes:
`legacy_java_exception_support_item`, `benchmark_java_file`,
`program_uses_floating_point`, `external_crate_constructor_rust_type_name_for`,
`external_call_throws`, `object_strategy_for_type`, and
`observably_aliased_locals`. The latter was initially left as a possible 0002
entry point, but the current representation path consumes the richer
`observable_aliases_with_facts` / `report_observable_aliases` API instead. The
production `src/fixtures.rs` test helper from item 10 was also moved to
`tests/common` and removed from the public library surface. A follow-up
removed eight verified-unused `ShapeEnv` insertion wrappers after confirming
they had no remaining `src` or `tests` references:
`insert_field_shape_with_java_type`,
`insert_type_static_storage_field_shape`,
`insert_type_static_storage_field_shape_with_java_type`,
`insert_static_field_shape_with_java_type`,
`insert_static_field_shape_for_owner`,
`insert_static_storage_field_shape`,
`insert_static_storage_field_shape_owned_by`, and
`insert_static_storage_field_shape_with_java_type`. The live precise insertion
forms, including the `*_with_access_name*` callers used by lowering, remain.

Produced by: parse every `fn` outside `#[cfg(test)]` and outside raw-string
literals, tokenise all of `src/` (production) and all of `tests/` + `src/`'s
`#[cfg(test)]` blocks, and keep the names whose production occurrence count is 1
(the definition) and whose test occurrence count is 0. Each was then confirmed by
hand with `grep -rn "\bNAME\b" src/ tests/ examples/ scripts/ docs/`.

**False-positive classes excluded:** trait-impl methods (all 39 are inherent or
free functions), items named inside `r#"..."#` runtime-support templates (this
removed 16 candidates in `javalibs.rs` and `stdlib.rs` — e.g. `set_clip`,
`draw_oval`, `append_path_to_dir` are text emitted into generated crates, not Rust
items), re-exports (`src/lib.rs` has none for these), and macro-generated calls
(the crate uses no proc macros).

Largest, superseded:

| lines | item |
|---:|---|
| 77 | `exception_lowering.rs:147` `legacy_java_exception_support_item` — **deleted**; `java_exception_support_item` is the live facts-aware replacement |
| 43 | `resolver_benchmark.rs:1040` `benchmark_java_file` — **deleted**; `benchmark_java_resolver_dir`/`_config` are what `main.rs` and the tests use |
| 23 | `stdlib.rs:5182` `program_uses_floating_point` — **deleted** |
| 20 | `facts.rs:2665` `external_crate_constructor_rust_type_name_for` — **deleted**; `external_crate_rust_type_name_for` already includes constructor mapping fallback |
| 19 | `call_plan.rs:1481` `external_call_throws` — **deleted**; callers use `external_call_returns_rust_result*` / effect-specific queries |

A cluster worth its own line: **`expr_shape.rs` had dead `insert_*_shape*`
builders** (`:247, :328, :382, :462, :491, :525, :562, :636` in the original
inventory). These no-access/no-access-name wrappers have now been removed after
current `rg` confirmation. `ShapeEnv` still has several insertion variants
because production uses the precise access-name/java-type forms from lowering
and stream lowering; re-check any future candidate individually.

Two more with a story:

- `object_model.rs:510` `object_strategy_for_type` — the `FactSet` half of an
  `_with_effective_facts` pair, and unlike the other 73 (item 7) not even a test
  calls it. **Deleted.** A later current-tree pass deleted the remaining
  zero-caller `object_strategy_for_type_with_effective_facts` wrapper; the solver
  now uses the requirements-aware internal helper directly.
- `aliasing.rs:1051` `observably_aliased_locals` — an aliasing query with no
  consumer, in the module [0002](../claude_audits/0002-object-aliasing-has-no-representation.md)
  says has no representation. **Deleted** after confirming the current tree uses
  `observable_aliases_with_facts` / `report_observable_aliases` instead.

Current zero-caller scan after the 2026-08-24 cleanup: **4 retained public
helpers remain**. This pass deleted the remaining mechanical wrappers and
orphaned query helpers from `expr_shape`, `unsupported`, `output_markers`,
`item_assembly`, `storage_strategy`, `external_packs`, `stdlib`,
`conversion_plan`, `statement_render`, `signature`, `facts`, and
`callable_analysis`, plus the private `constructor_key_matches` helper orphaned
by that removal.

The remaining four are intentionally not deleted in this pass:

| item | reason retained |
|---|---|
| `lower.rs:285` `lower_program` | top-level convenience API; analogous to the retained `translate_source` wrapper |
| `coercion.rs:8691` `expr_is_slice_view` | semantic inspection helper paired with `slice_view_expr`; tests document the non-string-matching boundary |
| `aliasing.rs:76` `observable_aliases` | public no-facts inspection API beside the diagnostics entry point |
| `liveness.rs:182` `mutated_after_for` | public inspection API called out by the liveness audit notes |

Full list with line counts: run the scan described above, or see the table in the
"39 zero-caller functions" section of the working notes.

---

## 7. 74 `X(..)` entry points that only tests call, beside a live `X_with_facts(..)`

**Superseded.** The tree has a systematic pair pattern: `foo(a, b)` delegating to
`foo_with_facts(a, b, &FactSet::default())` or to
`foo_with_effective_facts(a, b, &EffectiveFacts::new(facts))`.

| suffix | pairs | base never called in production |
|---|---:|---:|
| `_with_facts` | 76 | **42** |
| `_with_effective_facts` | 56 | **32** |
| `_with_context` | 11 | 5 |
| `_with_local_type_program` | 2 | 2 |
| | **145** | **74 distinct** (561 lines) |

Current-tree update: `plan_shape_coercion` has been removed from the production
library surface. Its test callers now call
`plan_shape_coercion_with_facts(..., &FactSet::default())` directly, making the
no-facts test configuration explicit at each use site. `plan_call_target` was
also removed; its test callers now use `plan_call_target_with_facts` with an
explicit default `FactSet` and `TypeEnv`. `shape_coercion_plan` was also
removed; tests now call `shape_coercion_plan_with_facts(...,
&FactSet::default())` directly. `solve_signature_shape` and
`conservative_boundary_constraints` were removed as well; tests now call
`solve_signature_shape_with_facts` or
`conservative_boundary_constraints_with_effective_facts` with explicit facts.
`apply_expr_to_slot` was removed after the same check; tests now call
`apply_expr_to_slot_with_facts(..., &FactSet::default())` directly. The
zero-caller `apply_expr_to_shape` wrapper was also removed; only the facts-aware
entry point remains. The same test-only no-facts wrapper batch was then retired
for `plan_coercion`, `syntax_coercion_plan`, `slot_conversion_plan`, and
`lowered_slot_conversion_plan`; their tests now name the default facts argument
at each use site. `plan_instanceof_check` and the test-facing
`apply_instanceof_check` wrapper were retired in the same style; tests now plan
with `plan_instanceof_check_with_facts(..., &FactSet::default())` and pass the
result to `instanceof_check_projection`. The test-only `plan_member_access`
wrapper was removed as well; tests now call `plan_member_access_with_facts`
with explicit default facts. `apply_lowered_expr_to_slot` was removed after the
same caller check; tests now call `apply_lowered_expr_to_slot_with_facts(...,
&FactSet::default())` directly. Three more test-only default-facts wrappers
were then retired: `optional_slot_construction_plan`, `plan_new_target`, and
`native_collection_arg_shapes`. Their surviving tests now call the facts-aware
entry points with an explicit `FactSet::default()` and, for constructor planning,
the arity argument. The same cleanup then removed the test-only wrappers
`indexed_read_shape`, `infer_stream_lambda_body_shape`, and
`infer_stream_source_element_shape`; tests now call the fact-aware stream and
projection APIs directly. A further test-facing batch retired
`borrowed_return_shape_from_param`, `java_exception_support_item`,
`apply_new_target`, `local_let_stmt`, and `plan_equality_comparison`; their
surviving tests now call the corresponding facts-aware entry points.
`type_emission_for` and `trait_impl_emissions_for_type` were retired the same
way from `representation_lowering`; tests now use the facts-aware representation
emission helpers directly. A later pass retired the same species from
`item_assembly.rs`: `struct_item_for_type`, `constructor_function_shell`,
`constructor_default_field_value`, and `method_function_shell` are gone, and the
tests now pass an explicit `FactSet::default()` to the `_with_facts` entry
points. `indexed_assignment_shape` was retired in the same style from
`shape_projection.rs`; tests now call `indexed_assignment_shape_with_facts`
directly. `infer_assignment_target_shape` was also retired from `expr_shape.rs`;
tests now pass `ExprShapeContext::default()` to
`infer_assignment_target_shape_with_context`. The method-reference shape wrapper
was retired next; tests now call
`infer_single_input_method_reference_output_shape_with_context` directly.
`emit_stream_lambda_body` was retired next; tests now pass
`StreamRenderContext::empty()` to `emit_stream_lambda_body_with_context`.
`external_api_facade_support_items` was retired after that; tests now call
`external_api_facade_support_items_with_local_type_program` directly.
`default_expr_for_slot` was retired next; tests now call
`default_expr_for_slot_with_facts` directly.
`plan_object_model` was retired next; tests now call
`plan_object_model_with_effective_facts` directly or through a local helper.
`render_stream_plan` was retired next; tests now pass
`StreamRenderContext::empty()` to `render_stream_plan_with_context`.

Two different problems hide here, and they are worth separating:

- The `_with_effective_facts` bases (32) just re-wrap `FactSet` → `EffectiveFacts`.
  Harmless duplication; delete and let tests call the real entry point.
- The **`_with_facts` bases default the facts to `FactSet::default()`**. Before
  the update above, tests calling `plan_shape_coercion(a, b)` exercised a
  **no-facts** configuration the binary never runs in — every real translation
  arrives with a populated `FactSet`. That wrapper is now gone. Continue
  retiring the same species one at a time where tests can name the facts-aware
  entry point directly. Same species as
  [0018](../claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md).

Special case: `translate_source` (`lib.rs:176`) is in this list with **792 test
callers** and 0 production callers — `main.rs` uses `translate_source_with_facts`.
That one is a genuine convenience API and should stay; it is listed only so the
count is honest.

---

## 8. Six more unreachable match arms

Status: **mostly fixed for this sub-item.** Current-tree checks confirmed the
following variants still had zero construction sites, so their pattern arms were
deleted: `SlotValueRustTypePolicy::NativeDispatch`,
`OwnershipCoercionFallbackPolicy::CompileError`,
`BorrowedElementRecoveryFallbackPolicy::CompileError`,
`BorrowProjectionUse::{SliceView, Display}`, and
`CallableKind::{FieldGetter, FieldSetter}`. The remaining entries below are the
two marked "never finished", where consumers exist and the missing piece is a
producer, not a safe deletion.

Method: classify every `Enum::Variant` occurrence in production as *pattern*
(followed by `=>`/`|`/`if`, or inside `matches!(`) or *construction*, then keep
variants with 0 constructions and ≥1 pattern. 33 raw candidates; hand-checking
removed the ones constructed via `Self::V` or a `use Enum::*` bare name
(`BorrowedContentPolicy::ContentView`, `WhileStmtPlan::{Infinite,Plain}`,
`MethodReferencePlan::BoundReceiver`, `RustItem::Comment`,
`FunctionalTypeSource::{TypeArgument,Primitive}`,
`RustBorrowedFacadeReturnRecovery::ToPathBuf`; the former
`ExternalPackObjectSafetyViolation` static-pack enum is now deleted). Survivors,
beyond items 1-3:

| variant | arm site(s) | why unreachable |
|---|---|---|
| `SlotValueRustTypePolicy::NativeDispatch` (`slot_render.rs:68`) | `slot_render.rs:390-392` | **deleted** after current-tree confirmation; `slot_value_rust_type_policy_with_effective_facts` builds `ConcreteDispatch` for every native case |
| `OwnershipCoercionFallbackPolicy::CompileError` (`conversion_plan.rs:162`) | `conversion_plan.rs:2393`, `coercion.rs:483` | **deleted** after current-tree confirmation; the three construction sites build only the three `BorrowedTo*` variants |
| `BorrowedElementRecoveryFallbackPolicy::CompileError` (`conversion_plan.rs:792`) | `conversion_plan.rs:832`, `coercion.rs:10561` | **deleted** after current-tree confirmation; both construction sites build `NonCloneableElement` |
| `BorrowProjectionUse::{SliceView, Display}` (`coercion.rs:34, 36`) | `coercion.rs:8527`, `8539` | **deleted** after current-tree confirmation; the construction sites build only `Argument`, `MutableSliceView`, `StringView`, `MapKey`, `MethodReceiver` |
| `CallableKind::{FieldGetter, FieldSetter}` (`facts.rs:810-811`) | `facts.rs:2841`, `external_packs.rs:31489` | **deleted** after current-tree confirmation; constructions build `Method`, `Constructor`, `StaticField`, `InstanceField` only |
| `TypePattern::TypeParam(String)` (`facts.rs:825`) | `facts.rs:3457, 3479`; `stdlib.rs:16144, 16163` | **fixed for this sub-item**; `external-pack-api ... params=type-param:T` and user `external-api ... params=type-param:T` now construct the pattern explicitly |
| `FacadeSlot::CrateHandle(String)` (`facts.rs:1171`) | 5 sites incl. `facts.rs:1185, 1197`, `call_plan.rs:1358`, `stdlib.rs:14273`, `shape_projection.rs:226` | **fixed for this sub-item**; `facade_slot_for_rust_type` now classifies path-like non-stdlib Rust types such as `dep_facade::ReaderHandle` as crate handles, and `rust-return=dep_facade::ReaderHandle` reaches `ExternalRustReturnTypeFact.return_slot` as `CrateHandle` |

`TypePattern::TypeParam` is now wired through both external metadata parsers.
Focused regressions: `external_api_params_can_declare_type_parameter_patterns`
and `external_fact_api_params_can_declare_type_parameter_patterns`. The
`FacadeSlot::CrateHandle` half is also wired through the shared Rust-type parser;
focused regressions:
`shared_facade_slot_parser_recognizes_crate_handles_and_dyn_traits` and
`external_pack_rust_crate_return_declares_crate_handle_facade_slot`.

This is the same shape the brief cited (the unreachable `"String"` arm): a match on
a value whose construction sites are few and enumerable.

---

## 9. 44 struct fields with no `.field` read anywhere

Method: extract `pub`/private fields of every non-`#[cfg(test)]` struct; count
`\.field\b` in production and in tests. **A field name that collides with another
struct's field is credited with the other struct's reads**, so this is a strict
*under*-count: 44 is a floor, not a total. 82 fields have zero production reads;
**44 have zero reads in `src/` and `tests/` both**.

Dominant pattern — 30 of the 44 are fields of `*Policy` / `*Boundary` / `*Contract`
descriptor structs whose constructor is itself only called by tests:

- `conversion_plan.rs`: `CastAndInstanceOfPlanningBoundary` (4/4 fields unread,
  **deleted under 0063**), `ErasedObjectRuntimeContract` (2,
  **deleted under 0063**), `PrimitiveWideningPolicy` (`from_rank`/`to_rank`
  now deleted), `PrimitiveRankPolicy` (now deleted),
  `Optional/Vec/SetElementTargetMatchPolicy` echo fields (now deleted),
  `RustScalarSlotCastPolicy` echo fields (now deleted),
  `RustBorrowedFacadeReturnPolicy` echo fields (now deleted),
  `RustUnsignedByteArrayReturnPolicy` echo fields (now deleted),
  `SyntaxCoercionPlan.content` (now deleted)
- `object_model.rs`: `BoxedDynObjectRepresentationPolicy` (**4/4**,
  now deleted),
  `ClosedEnumFactPolicy` (now deleted), `ObjectRepresentationPolicy.request`
  (now deleted)
- `signature.rs`: `SignatureBoundaryRolePolicy` (now deleted), `SignatureSlotBaselinePolicy`
  echo fields (now deleted), `SignatureNativeBoundaryPolicy` echo fields (now deleted),
  `SignatureSlotDispatchRequirementPolicy` echo fields (now deleted),
  `FieldNullabilityShapePolicy` echo fields (now deleted),
  `SignatureOwnedRecoveryCapabilityPolicy.java_ty` (now deleted),
  `SignatureFlagMerge.changed` (now deleted),
  `ConstructorParamBoundaryPolicy.constructor_name` (now deleted)
- `shape_projection.rs` (`ShapeProjectionCapabilityPolicy.java_ty` and
  `SequenceElementProjectionPolicy.java_ty` now deleted), `slot_render.rs`
  (`SlotContentCapabilityPolicy.java_ty` now deleted), `stream.rs`
  (`StreamElement.value_category`/`StreamValueCategory` now deleted), `stream_shape.rs`
  (`StreamSourceCapabilityPolicy.source_ty` now deleted),
  `facts.rs` (`HierarchySubtypePolicy` echo fields now deleted),
  `coercion.rs` (`RustPrimitiveTypeNamePolicy.primitive` now deleted),
  `binding_id.rs` (`BindingKind::Unknown` now deleted),
  `semantic_analysis.rs` (`SemanticAnalysisIterations.{external_api_effects, control_flow}`
  now deleted), `external_packs.rs` (`ExternalTemplateMethodHook.inherited_method`
  now deleted)

A field nobody reads is a computation nobody checks. The two named progress
fields above are now gone; keep applying this check one descriptor at a time
rather than trusting the original line-count inventory.

---

## 10. `src/fixtures.rs` is a test helper inside the production library

Status: **fixed for this sub-item.** `Fixture` and `load_java_fixtures` now live
in `tests/common/mod.rs`, the integration tests import them through `mod common`,
`pub mod fixtures` was removed from `src/lib.rs`, and `src/fixtures.rs` was
deleted.

33 lines. `load_java_fixtures` + `collect_java_files` walk a directory for `.java`
files.

Original evidence:

```
$ grep -rn 'fixtures::' src/          # nothing
$ grep -rn 'fixtures::' tests/
tests/corpus_compile.rs:13:use new_java2rust::fixtures::load_java_fixtures;
tests/fixtures.rs:6:use new_java2rust::fixtures::load_java_fixtures;
```

Current expected evidence:

```
$ grep -rn 'new_java2rust::fixtures\|pub mod fixtures' src/ tests/
# no matches
```

It is the only module in `src/` with **zero** references from any other production
module. It belongs in `tests/common/mod.rs`, where
[0041](../claude_audits/done/0041-slot-shape-design-had-no-generated-crate-checks.md)
already put `cargo_check_generated_modules`.

---

## 11. Stale `#[allow(dead_code)]`, and what changed

Status: **fixed for production.** Current `rg -n "#\[allow\(dead_code\)\]" src`
has no matches. `tests/common/mod.rs` still has a module-level allow because the
shared integration-test helper module is compiled separately into many test
targets, and each target intentionally uses a different subset of helpers;
removing the allow creates target-local dead-code warnings without finding
production dead code.

- Status: **fixed for one sub-item.** The stale
  `#[allow(dead_code)]` on `find_raw_pattern_outside_literals` was removed; the
  helper is now explicitly `#[cfg(test)]` because its only current caller is the
  same-outer inner-constructor rewrite regression helper inside `lower.rs`'s test
  module.
- Status: **fixed for the second sub-item.** The production parser still validates
  `generic_suffix` so `new::<T>` is accepted and other suffixes are rejected, but
  only the test-only raw rewrite helper needs to preserve `.generic_suffix` and
  `.whitespace_before_paren` in the returned struct. Those two fields are now
  explicitly `#[cfg(test)]`, and the stale production `#[allow(dead_code)]` is
  gone.

Replacing the obsolete attribute with `#[cfg(test)]` lets `rustc` police the
production surface without warning on a test-only helper; there is no longer a
field deletion here.

---

## Appendix A — the CLI is clean

The brief singled this out, so it was checked exhaustively rather than by reading.
`src/main.rs` has 4 subcommands and 15 flags. Every one reaches a reader that
changes behaviour.

Traced by hand: `--facts`, `--external-pack-manifest`, `--builtin-external-pack`,
`--no-builtin-external-packs` → `load_external_inputs` (`main.rs:1112-1150`) /
`load_external_fact_providers` (`:1153-1195`); `--context-dir`,
`--context-source-summary-dir`, `--context-dir-package`, `--context-dir-emitted` →
`load_context_sources` (`:427-450`) → `translate_source_with_context_sources_and_summary_sources`;
`--primary`, `--out-dir`, `--generated-dir`, `--jobs`, `--phase-events`,
`--worker-events` → `run_translate_batch` (`:469-730`); `--source-root`,
`--source-root-package` → `DependencyModeConfig`.

Verified by running the prebuilt binary:

```
$ ./target/release/new-java2rust Vc.java > base.rs
$ ./target/release/new-java2rust --no-builtin-external-packs Vc.java > a.rs
$ diff base.rs a.rs | wc -l          # 26
$ ./target/release/new-java2rust --builtin-external-pack htsjdk.variant.variantcontext Vc.java > b.rs
$ diff base.rs b.rs | wc -l          # 77
$ diff a.rs <(... --no-builtin-external-packs --builtin-external-pack htsjdk.variant.variantcontext ...) | wc -l   # 101
```

(`Vc.java` = a 5-line class calling `VariantContext.getContig()`.) Batch mode:
`--generated-dir` relocates output, `--phase-events` wrote 22 rows,
`--worker-events` wrote 38, `--jobs 1` vs `--jobs 10` produced identical generated
Rust and differing timings — all four confirmed by running them.

`disable_builtin_external_packs` is read at **24 sites** across `stdlib.rs`,
`call_plan.rs`, `member_access_plan.rs`, `inherited_members.rs` and `lib.rs`. The
verified no-op recorded in the brief was closed by
[0050](../claude_audits/done/0050-builtin-pack-facts-and-name-aliases-rebuilt-per-call.md);
this is an independent re-confirmation.

**One asymmetry worth a note, not a defect:** `--builtin-external-pack X` changes
output by 77 lines even when built-in packs are *already enabled by default*. So
the default built-in fact set is not the union of the individually-nameable packs.
That is a fact-coverage question, out of scope here.

## Appendix B — how to re-run these scans

All findings came from four passes over the tree, none of which need `cargo`:

1. **zero-caller functions** — tokenise `src/` (production) and `tests/` +
   `#[cfg(test)]` separately; a `fn NAME` whose production token count is 1 is
   uncalled. Filter out raw-string regions (`r#"..."#`) or you will get 16 false
   positives from emitted runtime templates.
2. **unread fields** — count `\.field\b`. Under-counts on name collisions.
3. **unreachable variants** — classify each `Enum::Variant` occurrence as pattern
   vs construction by the token that follows its balanced payload. Must be checked
   by hand for `Self::V` and `use Enum::*` constructions; roughly half the raw
   candidates were false positives.
4. **overload pairs** — for each `fn X_with_facts`, check whether `fn X` exists and
   whether `X` has any production caller.
