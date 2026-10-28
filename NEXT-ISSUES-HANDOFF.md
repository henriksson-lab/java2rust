# Next Issues Handoff

Recorded: 2026-08-06

This is a synthesis of the current audit state plus three parallel read-only
worker passes. The tree was reported gate-clean after the latest fixes, with the
checked-in suite baseline still showing the known red suites rather than new
regressions.

## Current Baseline

Source of truth:

- `tests/baselines/suite-results.tsv`
- `tests/baselines/idiom-gaps-failures.txt`
- `docs/claude-audit-refactoring-plan.md`
- `claude_audits/0002b-shared-representation-design.md`
- `claude_audits/done/0018-the-compile-error-metric-cannot-see-silent-failures.md`
- `claude_audits/0007-rust-ir-is-mostly-opaque-strings.md`

Latest checked-in suite counts:

| Suite | Passed | Failed |
| --- | ---: | ---: |
| `idiom_gaps` | 693 | 117 |
| `slot_shape_design` | 485 | 120 |
| `fixtures` | 54 | 0 |
| `dependency_mode` | 65 | 0 |
| `resolve_calls` | 291 | 0 |
| `semantic_facts` | 174 | 0 |
| `external_packs` | 169 | 0 |
| `batch_translation` | 16 | 0 |
| `parser_tree_sitter` | 47 | 0 |
| `--lib` | 669 | 0 |
| `output_markers` | 23 | 0 |
| `idiom_sweep_script` | 48 | 0 |

Use `scripts/gate.sh --all` for the full check. For `idiom_gaps`, diff failure
names against `tests/baselines/idiom-gaps-failures.txt`; the count alone is not
enough.

Update 2026-08-30: focused stale-row checks showed the checked-in `fixtures`
and `slot_shape_design` rows were not describing the current dirty tree. The
first full correction found `fixtures` at `45/9` and `slot_shape_design` at
`473/136`. The slot-shape movement is a baseline correction from the current
full target output, not evidence that the current small 0007 cleanup caused
those failures.

Update 2026-08-30 later pass: four stale fixture rows were repaired and then
verified by a full `fixtures` run. The fixture baseline moved to `49/5`.
Two rows only needed nullable string-display expectations updated to the
current Java-null-preserving lowering. The two stream `findFirst` rows exposed a
real optional-borrow carrier bug: borrowed stream `next()` results are already
`Option<&T>`, so the terminal expression is now tagged `OptionalProjected`
instead of being sent through another `.as_ref()` projection before clone
recovery.

Update 2026-08-30 stream/static/TWR batch: the remaining five `fixtures`
failures were resolved and a full target run is now green at `54/0`. Two static
owner rows were stale expectations after resolved-owner fixes. Two stream
`flatMap` rows were real optional-sequence bugs: lambda collection sources now
unwrap optional sequence returns before iteration, and nested flatMap pipelines
reuse the lambda scoped environment so prefix locals already materialized as
non-optional are not unwrapped again. The TWR row was a real external model bug:
`Files.newBufferedReader` now returns `BufferedReader`, preserving concrete
`readLine` and `close` lowering for resources.

Update 2026-08-30 stream-plan/0064 batch: focused `stream_plan` expectations
were stale after owned Java `String` stream results and package-qualified static
owner fixes; the filter is now green at `21/0`. The remaining source-summary
static overload cluster in `0064` is also green: resolved source static calls
now preserve the resolver-selected overload name (`pick_int`/`pick_string`)
instead of re-planning from normalized receiver spelling and falling back to
bare `pick`. The slot-shape baseline moved to `482/127`.

Update 2026-08-30 external-constructor 0056 pass: external constructor
argument facts now feed the same owned-native borrow adapter used by ordinary
method calls. `new IntVector("id", allocator)` with a borrowed
`RootAllocator` parameter now emits `IntVector::new("id", &allocator)`, and
unknown external constructor argument nullability stays non-optional. The
slot-shape baseline moved to `484/125`.

Update 2026-08-30 native Map.get / external getter 0056 pass: native
`Map.get` into a declared final external facade local now preserves the
borrowed map element shape through local refinement, and borrowed-element
conversion no longer lets an early optional projection suppress the required
null unwrap. External return nullability facts also apply to read-only modeled
calls as well as unknown external boundaries, so `VariantContext.getAttribute`
feeds `MaybeNull` into the assigned `Object` local. The slot-shape baseline
moved to `482/123`.

Update 2026-08-30 lib cleanup batch: full `cargo test --release --lib` is green
at `669/0`. The seven dirty-tree failures were a mix of stale structured-IR
assertions, one malformed raw result-recovery bug, a `this` receiver summary
lookup that did not fail closed on ambiguous current owners, stale usage
guardrail variable names, and storage tests whose helper callee had become
modelled rather than unknown.

## Suggested Work Order

1. Low-risk visibility fixes: no-op facade call-site diagnostics and stale metric
   handling.
2. Localized resolver/source-summary fixes: nested type lookup and source-summary
   facade completeness.
3. Optional/nullable slot adaptation: high leverage for `idiom_gaps`, moderate
   risk.
4. Representation/identity (`0002b`, `0028`, then `0023`): highest semantic
   value, highest blast radius.
5. Remaining architectural guardrails: plan ownership, raw Rust string removal,
   and shared support sweep coverage.

## Priority Issues

### 0. Completed: Optional Erased-String Raw-Leaf Cleanup

`0007` cleanup in this pass: `rewrite_optional_erased_borrowed_string_view_call_args`
now enters through structured `RustExpr` traversal. The legacy
outside-literals/comments replacement is still present, but only behind the
`RustExpr::Raw` compatibility arm. Verified with
`rendered_string_heuristics_have_centralization_guardrails` and the
`optional_erased_string` lib-test filter.

### 0b. Completed: Exception Lowering Layering Guard

`0057` cleanup in this pass: source exception parent lookup moved out of
`exception_lowering`'s Java-type construction path and behind
`FactSet::extends_type_name_for`. The exception renderer now stays on facts and
Rust IR in production code. Verified with the focused layering guard, full
`tests/layering.rs`, and the `semantic_facts` exception filter.

### 0c. Completed: Optional Carrier Projection Guardrail

`0007`/`0056` cleanup in this pass: optional receiver field access now routes
through the central optional receiver slot/projection plan instead of calling
`project_optional_receiver` directly from field helpers. `Map.get` into an
optional boxed-primitive slot also recognizes boxed Java primitives as primitive
storage and emits `.copied()` on the borrowed optional carrier. Verified with
`slot_shape_design::optional_element_borrow_carrier_construction_is_centralized`
and `fixtures::translates_map_get_null_ternary_with_borrowed_optional_carrier`.

### 0d. Completed: Borrowed Stream `findFirst` Optional Carrier

`0007`/`0056` cleanup in this pass: `findFirst()` over a borrowed stream now
marks the lowered `.next()` terminal as an already-projected optional carrier.
That prevents the later optional conversion path from adding a redundant
`.as_ref()` and producing `Option<&String>` where the slot needs
`Option<String>`. Verified with
`fixtures::stream_find_match_generated_rust_preserves_behavior`,
`fixtures::translates_stream_find_first_and_any_match_through_slot_coercion`,
and a full `fixtures` target run.

### 0e. Completed: Fixture Baseline Cleanup

The `fixtures` target is now green at `54/0`. This pass fixed real stream/TWR
lowering defects and cleaned up stale static-owner expectations:
`stream_flat_map_projection_generated_rust_preserves_behavior`,
`translates_stream_flat_map_block_pipeline`,
`try_with_resources_nullable_read_and_closeable_cleanup_compiles`,
`nested_static_builder_call_uses_resolved_type_owner`, and
`translates_static_import_and_qualified_static_utility_owner`.

### 0f. Completed: Raw Deref String-View Parser Removal

`0007` cleanup in this pass: `raw_deref_string_view_projection` no longer keeps
a bespoke `*identifier` text parser. Raw compatibility now goes through the
shared rendered-expression recovery path and requires structured unary evidence
before composing `.as_str()`. The leftover borrowed-item string-view helpers are
`#[cfg(test)]`, so integration-target lib builds no longer warn about dead
production helpers. Verified with
`coercion::tests::raw_deref_string_view_recovery_accepts_only_plain_identifier_source`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

### 0g. Completed: Source-Name Nullability And Id-Less Assignment Fence

`0061` cleanup in this pass: source-return nullability scopes now insert bound
params, locals, and enhanced-for variables under Java `source_name`, not escaped
Rust access names. Known scoped bindings without a binding-specific nullability
state now fail closed instead of falling through to same-name field/name
fallbacks. The remaining name-only assignment scan in borrowed-return policy is
renamed to `legacy_idless_method_assigns_to_binding` and guarded as an explicit
id-less compatibility surface. Verified with the `source_return_nullability_`
lib-test filter, `borrowed_return_param`, `returned_object_alias_constraints`,
and `layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

### 0h. Completed: Map-View Bulk Argument Helper Cleanup

`0007` cleanup in this pass: `lower_map_view_bulk_collection_arg` no longer
accepts an unused debug/temp-name parameter. The map-view arms still own their
exact temporary names; the shared helper now only handles argument shape
inference and slot lowering. Verified with the `alias_` lib-test filter, which
covers keySet/values/entrySet alias bulk lowering.

### 0i. Completed: Stream Lambda Access-Name Ownership

`0007` cleanup in this pass: stream-lambda parameter access-name insertion now
lives only in `stream_lambda_scoped_env`. The nullable lambda and prefix-stmt
paths still insert prefix-local access names, but they no longer reinsert the
same param mapping. Verified with
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`,
`slot_shape_design stream_plan`, and
`fixtures::stream_flat_map_lambda_generated_rust_preserves_behavior`.

### 0j. Completed: Stream Source Capability Predicate

`0007` cleanup in this pass: direct and optional stream-source shape helpers now
share `java_type_is_stream_capable` for the Java-type capability question. The
two callers still keep their different fallback behavior, but the underlying
`stream_element_type` / source-capability fact lookup is no longer duplicated.
Verified with the two focused stream-source capability fact tests,
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`,
and the broader `slot_shape_design stream_plan` filter.

### 0k. Completed: Id-Less Borrowed-Return Assignment Scan Fence

`0061` cleanup in this pass: the deliberately retained
`legacy_idless_method_assigns_to_binding` compatibility scan now recognizes
expression-side mutations, including `++`/`--`, assignment expressions, and
`for` update expressions. Resolver-bound parameters remain on exact
`BindingUse` facts; this only narrows the parser-only/id-less fallback so it
does not bless a mutated parameter as borrowed-return content. Verified with
the `slot_policy::tests` filter and
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

### 0l. Completed: Borrowed String Literal Temporary Raw Guard

`0007` cleanup in this pass: `borrowed_string_literal_temporary_plan` now uses
the shared complete-string-literal predicate before accepting raw
`...to_string()` compatibility text. Valid `"literal".to_string()` and
`Some(&"literal".to_string())` plans are unchanged, while malformed
literal-looking raw leaves fail closed instead of becoming invalid
`RustExpr::Literal` nodes. Verified with
`slot_shape_design::coercion_owns_borrowed_string_literal_temporary_projection`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

### 0m. Completed: Rendered Cast Type Balance Guard

`0007` cleanup in this pass: rendered cast recovery now requires balanced
generic type text before promoting a raw fragment to `RustExpr::Cast`. Valid
simple/path/generic casts still recover structurally, but malformed text such
as `value.clone() as Vec<T>>` remains an opaque raw compatibility leaf instead
of becoming a structured cast with an invalid type string. Verified with
`coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions` and
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

### 0n. Completed: Emitter Raw Parenthesis Comment Guard

`0007` cleanup in this pass: emitter-side raw outer-parenthesis stripping now
skips raw strings and Rust line/block comments before counting delimiters.
Delimiter characters inside comments or raw-string literal bodies therefore no
longer affect whether a whole raw expression is stripped. Verified with
`emit::tests::raw_parentheses_strip_is_balanced_and_string_aware`.

### 0o. Completed: External Constructor Borrowed Arguments

`0056` cleanup in this pass: shaped constructor arguments now run the same
owned-native borrow adapter as shaped method-call arguments. External
constructor metadata can therefore express borrowed facade parameters without
the value side leaving an owned local in argument position. The two stale
expected-failure rows
`external_constructor_argument_fact_borrows_facade_parameter` and
`external_constructor_argument_unknown_nullability_defaults_to_non_null` were
removed from the slot-shape baseline after compiling the generated fixture with
a minimal `arrow_facade` stub.

### 0p. Completed: String Literal Token Decoding

`0007` cleanup in this pass: `string_literal_source` now decodes complete Rust
string literal tokens instead of slicing off surrounding quotes. `String.join`
vararg normalization therefore preserves escaped source values such as
newlines and embedded quotes instead of treating escaped token text as data.
Verified with
`coercion::tests::string_join_projection_helpers_own_literal_and_to_string_idempotence`.

### 0q. Completed: Macro ToString Receiver Recovery

`0007` cleanup in this pass: terminal raw `.to_string()` recovery now accepts
structured macro-call receivers, so `format!("{}", name).to_string()` is
recovered as a method call on `RustExpr::MacroCall` instead of retained as
grouped raw text. Verified with
`coercion::tests::string_join_projection_helpers_own_literal_and_to_string_idempotence`.

### 0r. Completed: Source-Summary Boundary Does Not Override Parsed Source

`0064` cleanup in this pass: corpus-analysis erased-boundary facts from
source-summary superclass metadata are no longer allowed to replace or shadow a
type already proven to be parsed source. This preserves the emitted concrete
struct representation while still allowing the boundary fact for true external
summary-only roots. Verified with
`slot_shape_design::source_summary_boundary_erasure_does_not_replace_parsed_source_type`;
the row was removed from the slot-shape expected-failure baseline.

### 1. `0002b`: Optional Element-Position Wrapping

Why next: the last attempted shared-handle pass stopped on a precise value-side
gap. Type rendering already recurses through `Option<T>`, but value lowering does
not have a distinct "optional element" position. That makes these two wrong
shapes easy to produce:

```rust
Some(std::rc::Rc::new(Some(Wrapper::new(...))))
std::rc::Rc::new(Some(Wrapper::new(...)))
```

The needed shape is:

```rust
Some(std::rc::Rc::new(Wrapper::new(...)))
```

Evidence:

- `claude_audits/0002b-shared-representation-design.md`
- `src/conversion_plan.rs`
- `src/slot_render.rs`
- `tests/idiom_gaps.rs` around `constructor_nested_arg_reads_assigned_field_without_cloning_self`

Resolution idea:

Add a value-lowering concept for the element position inside an optional slot,
mirroring the type-side recursion through `Option`. Do not solve this by blanket
wrapping every `Expr::New`; prior attempts produced double wrapping and wrong
outer handle placement.

First verification:

```bash
cargo test --release --test idiom_gaps constructor_nested_arg_reads_assigned_field_without_cloning_self
cargo test --release --test slot_shape_design optional_element_borrow_carrier_construction_is_centralized
scripts/gate.sh --all
```

### 2. `0002b` + `0028`: Land `SharedImmutable` With Identity Lowering

Why next: `SharedImmutable` is the useful first representation tier, but it is
not observable unless Java identity is also lowered correctly. The audit notes
the measured split after fixes: 23 distinct sharing assignments, with 19 in
`SharedImmutable` and 4 in `SharedLocal`.

Evidence:

- `claude_audits/0002b-shared-representation-design.md`
- `claude_audits/0028-reference-equality-lowered-as-structural-equality.md`
- `tests/differential/known-gaps/Identity.java`
- `src/storage_strategy.rs`
- `src/slot_render.rs`

Resolution idea:

Flip `SharedImmutable` only, together with Java identity behavior:

- Owned source-object slots use `Rc<T>` or `Arc<T>` depending on sync/static
  reachability.
- New source-object values are wrapped at the correct target slot boundary.
- Reference `==` and inherited non-overridden `.equals()` use pointer identity
  for shared object representations.
- Structural equality remains for primitives, strings, enums, and value-only
  cases.

Keep `SharedLocal` disabled until immutable identity is stable.

First verification:

```bash
cargo test --release --test type_coercion
cargo test --release --test idiom_gaps source_interface_trait_uses_fallible_implementation_signature
# Run the differential harness for tests/differential/known-gaps/Identity.java.
scripts/gate.sh --all
```

### 3. `0002b`: Handle State Is Missing From Conversion Shapes

Why next: `plan_handle_adaptation` is effectively inert because
`RustSlotShape` cannot always say whether an expression is already a handle. This
blocks clean upcasts, erasure, downcasts, generic argument wrapping, and identity
lowering.

Evidence:

- `src/conversion_plan.rs`
- `src/slot_render.rs`
- `claude_audits/0002b-shared-representation-design.md`

Resolution idea:

Move handle placement into the shape/slot model instead of adding ad hoc
`Wrap`/`Unwrap` edits. Characterize damaged sites first, especially:

- subtype upcasts and erased facade conversions
- clone recovery
- super-constructor initialization
- self-referential `Box` storage
- borrowed getters
- facade shells
- cast/instanceof direct `RustExpr` paths

First verification:

```bash
cargo test --release --test type_coercion
cargo test --release --test idiom_gaps erased_source_super_alias_conversions_are_deduplicated_by_rust_target
cargo test --release --test idiom_gaps reused_optional_erased_object_cast_clones_with_erased_helper
```

### 4. `0023`: Inner-Class Construction Needs A Shared Receiver Story

Why next: inner-class construction remains blocked by object representation.
Shared slots alone do not solve it because methods still receive `&mut Self`,
which cannot recover an `Rc<RefCell<Self>>` outer handle.

Evidence:

- `claude_audits/0023-inner-class-construction-always-panics.md`
- `tests/batch_translation.rs::batch_translation_reports_inner_class_constructor_panics`

Resolution idea:

After handle storage exists in shapes, design shared receiver lowering. Likely
options are explicit self-handle storage or wrapper methods that receive
`self: &Rc<RefCell<Self>>`. Inner constructors can then capture the outer handle
instead of failing or cloning an outer value.

First verification:

Add a differential fixture where an inner class reads and mutates outer state,
then verify Java and Rust behavior match.

### 5. `0018`: No-Op Facade Method Call-Site Accounting Boundary

Why not next as a simple patch: constructor call sites and facade-ABI-backed
fluent receiver calls are now counted, and the call-site marker is registered in
`OUTPUT_MARKERS`. Ordinary generated facade methods whose bodies are silent
defaults but whose calls do not resolve through a facade ABI remain a lower-bound
case. That is a metadata/plan-ownership gap, not a reliable text-scanning fix.

Evidence:

- `claude_audits/done/0018-the-compile-error-metric-cannot-see-silent-failures.md`
- `src/stdlib.rs`
- `src/output_markers.rs`
- `tests/output_markers.rs`

Resolution idea:

Do not add a `.method(` scanner. Teach facade generation/call planning to share
metadata for default-body facade methods, then emit the existing
`noop_facade_call_site` marker from lowering when a call resolves to that
metadata.

First verification:

```bash
cargo test --release --test output_markers noop_facade_constructor_call_sites_are_counted_per_call
cargo test --release --test batch_translation
```

Current focused coverage:

```bash
cargo test --release --test output_markers noop_facade -- --nocapture
```

### 6. `0018`: Harden Focused Summary Metrics Against Staleness

Current status: the `noop_facade_call_sites` profile-missing branch is now
covered. If `analysis-profile.tsv` exists but lacks that metric,
`translate-focused-package.sh` fails instead of using the idiom-sweep fallback.
The broader measurement rule remains: new profile-owned metrics should fail
loudly when the profile exists but is stale.

Evidence:

- `scripts/translate-focused-package.sh`
- `scripts/run-focused-stage-gates.sh`
- `tests/batch_translation.rs`

Resolution idea:

Keep profile-owned metrics on the fail-loud path used by
`analysis_profile_metric_or_idiom_signal`; do not add silent fallback branches
for present-but-incomplete profiles.

First verification:

```bash
cargo test --release --test batch_translation
scripts/gate.sh --all
```

Focused coverage:

```bash
cargo test --release focused_package_missing_analysis_profile_metric_fails_even_with_idiom_fallback --test batch_translation -- --nocapture
```

### 7. Source-Summary Facade Completeness

Why next: recent fixes made direct source-summary static methods and
constructors use explicit source-summary surfaces instead of generic no-op
external stubs. The next gap is inherited members and richer constructor/static
surfaces.

Update 2026-08-30: the nullable-return local-planning subcase is fixed. When a
source-summary support shell and source-summary declaration share an owner name,
receiver return-shape planning now consults the summary declaration before the
merged support-context signature fallback. This keeps `Lookup.find(...)` locals
as `Option<Path>` when the parsed summary body can return `null`.

Evidence:

- `src/stdlib.rs`
- `tests/slot_shape_design.rs`
- `tests/baselines/idiom-gaps-failures.txt` entries starting with `source_`

Resolution idea:

Treat source-summary declarations as a real declaration provider keyed by the
same callable/signature identity used for primary source. Then lower inherited
field/method access and super constructor calls through resolved targets rather
than first-same-arity or external fallback.

First verification:

```bash
cargo test --release --test idiom_gaps source_super_constructor_uses_resolved_overload_not_first_same_arity
cargo test --release --test dependency_mode source_summary_root_resolves_imported_nested_graph_types
```

Completed verification for the nullable-return subcase:

```bash
cargo test --release source_summary --test idiom_gaps -- --nocapture
```

Update 2026-08-30: one nested source-summary attribution hole is fixed. The
resolver previously checked static type-owner calls before local receiver
calls, so a local named `connection` was misread as the snake-case alias for
the imported source-summary type `Connection`; `connection.getTarget()` then
left a stale unresolved/static-method fact even though `Connection.getTarget`
was in the source-summary symbol table. Static type receiver detection now
fails closed when a local with the same spelling is in scope.

Completed verification:

```bash
cargo test --release source_static_type_receiver_respects_local_shadowing_for_imported_nested_type --lib -- --nocapture
cargo test --release --test dependency_mode source_summary_root_resolves_imported_nested_graph_types -- --nocapture
```

### 7a. Completed: Local `SimpleEntry.setValue`

Update 2026-08-30: a parallel audit worker found a remaining `0059` tuple-carrier
hole where `Map.Entry<String, String> entry = new AbstractMap.SimpleEntry<>(...)`
followed by `entry.setValue(...)` emitted an invalid tuple method call. Local
mutable `SimpleEntry` values now mutate tuple field `.1` and return the old
value. Local `SimpleImmutableEntry` values keep the tuple carrier for
display/equality but raise `UnsupportedOperationException` on `setValue`.

Completed verification:

```bash
cargo test --release map_entry_set_value --lib -- --nocapture
cargo test --release simple_map_entry --lib -- --nocapture
```

### 7b. Completed: Map Entry Iterator Nullable Receiver

Update 2026-08-30: the remaining small `0056` worker finding is fixed.
`Map.entrySet().iterator().next().getKey()` now routes nullable map receivers
through the existing result-aware sequence iteration projection, producing a
catchable `NullPointerException` in result contexts instead of
`.expect("Java null receiver")`.

Completed verification:

```bash
cargo test --release map_entry_set_iterator_next_get_key --test idiom_gaps -- --nocapture
```

### 8. Completed: Nested Class And Lexical Type Resolution

The `resolve_calls` nested/lexical ownership cluster is now green. Resolver
constructor lookup now considers the current lexical type scope for unqualified
nested constructor targets, tests assert the owner-qualified AST names produced
by nested type qualification, source-program assembly no longer imports every
top-level type from a package just because one type was explicitly imported, and
known external enum-like receiver `values()` calls participate in overload
typing through the same external type alias policy used by facts.

Evidence:

- `tests/resolve_calls.rs`
- `tests/baselines/suite-results.tsv`

Resolution idea:

Centralize nested type candidate production with explicit priority:

1. current and enclosing lexical scope
2. explicit type imports
3. same-package top-level types
4. external/package candidates

Preserve owner identity separately from display name. That avoids fixing one
case by rendering `Owner.Helper` where a constructor path needs `Helper`.

Verification:

```bash
cargo test --release --test resolve_calls
```

### 9. Optional/Nullable Slot Adaptation Cluster

Why next: this is one of the densest `idiom_gaps` failure clusters:
`optional_*`, `nullable_*`, `map_get_*`, borrowed optional args, nullable source
calls, and optional resource cleanup.

Evidence:

- `tests/baselines/idiom-gaps-failures.txt`
- `claude_audits/0007-rust-ir-is-mostly-opaque-strings.md`
- `src/coercion.rs`
- `src/conversion_plan.rs`

Resolution idea:

Continue moving optional construction, borrowed element views, and clone
recovery into central slot/conversion planning. Avoid local rendered-string
checks. Each fix should prove the actual produced shape and target slot shape.

First verification:

```bash
cargo test --release --test idiom_gaps map_get_return_uses_nullable_signature_and_optional_carrier
cargo test --release --test idiom_gaps optional_map_string_get_flows_into_string_value_contexts
cargo test --release --test idiom_gaps nullable_source_call_through_field_receiver_stays_optional_without_double_wrap
```

### 10. External Pack Attribution And Manifest Coverage

Why next: the remaining `external_packs` failures are small, and related
`dependency_mode` failures point to over-eager pack ownership when names collide
or receiver provenance is weak.

Follow-up 2026-08-30: two concrete `external_packs` failures from the dirty
tree were closed. General external type aliasing now reuses the nested-type
suffix aliases that were previously exception-only, so an analyzed Avro
`Schema.Parser` constructor resolves through the existing collapsed `Parser`
pack fact. The VCF manifest table also now classifies the five
`VCFUtils.smartMergeHeaders` spellings already present in the effect table.

Follow-up 2026-08-30: dependency-mode attribution no longer counts lambda
parameter field accesses as external-pack/source-summary static-owner evidence
when the parameter shadows a type-like name. The benchmark expression type
extractors now resolve uppercase field-access owner tokens through normalized
local bindings and fail closed on synthetic `lambda_param:*` unknown markers.
The dependency-mode baseline moved from `49/16` to `51/14`.

Follow-up 2026-08-30: the snake-case project field receiver attribution check
was already behaviorally fixed and only had a stale assertion. It now expects
the source owner spelling `BaseUtils` in the project bucket, matching the
current 0064 direction. The dependency-mode baseline moved from `51/14` to
`52/13`.

Follow-up 2026-08-30: the project static-field-chain attribution check had the
same stale normalized-owner assertion. The full `BaseUtils.Base.N.base` chain is
already classified as project pressure with zero external field guesses, and
the assertion now expects source owner spelling. The dependency-mode baseline
moved from `52/13` to `53/12`.

Follow-up 2026-08-30: three dependency-mode entries were stale and already green
under the current resolver: disabling/re-adding builtin packs, the Java SQL pack
composition case, and imported nested graph source-summary resolution. Removing
those known-failure names moved the baseline from `53/12` to `56/9`.

Follow-up 2026-08-30: source-summary dependency attribution now indexes nested
types by package-relative owner-qualified names as well as simple and fully
qualified names. Resolved constructors such as
`UserException.ValidationFailure` now map back to the
`dep.exceptions.UserException.ValidationFailure` source-summary declaration
instead of disappearing from the dependency table. The baseline moved from
`56/9` to `57/8`.

Follow-up 2026-08-30: the dependency-mode hierarchy/source-summary cluster is
green. Source-summary superclass boundaries can now introduce corpus-derived
erased-object roots in per-file representation plans, hierarchy diagnostics use
normalized generic/simple-name aliases when checking represented supertypes and
subtype edges, and closed-enum user facts can name nested variants by simple
Java name while resolving to owner-qualified source subtype keys. Two manifest
attribution assertions were also updated to the current counter split for
conservative API guesses. The dependency-mode baseline moved from `57/8` to
`65/0`; `cargo test --release --test dependency_mode` passes.

Evidence:

- `tests/external_packs.rs`
- `tests/dependency_mode.rs`
- `tests/baselines/suite-results.tsv`

Resolution idea:

Require imported/qualified external provenance or manifest-declared boundary
reachability before a pack claims a receiver/member. Treat lambda params,
project static chains, and unimported same-simple-name types as project or
unknown unless proven external.

First verification:

```bash
cargo test --release --test external_packs builtin_pack_manifests_explicitly_classify_modeled_member_facts
cargo test --release --test external_packs java_sql_pack_does_not_claim_project_connection_or_reflection_class_names
cargo test --release --test dependency_mode resolver_benchmark_does_not_treat_lambda_params_as_static_pack_receivers
```

### 11. Stream And Lambda Lowering Through Shared Plans

Why next: `fixtures` still has many stream failures: nullable map/non-null
filter, flatMap lambda/block/projection, method references, `findFirst`,
`anyMatch`, and collect-to-list.

Evidence:

- `tests/fixtures.rs`
- `tests/slot_shape_design.rs`

Resolution idea:

Make stream lambda body emission use the same `ShapeEnv`, call target plan, and
slot conversion functions as normal call lowering. Start with method references
and nullable-map fusion because they also exercise optional slot correctness.

First verification:

```bash
cargo test --release --test fixtures translates_stream_nullable_map_then_non_null_filter
cargo test --release --test fixtures stream_string_method_references_use_call_target_plan
cargo test --release --test slot_shape_design stream_plan_fuses_nullable_map_with_non_null_filter_shape
```

### 12. `0007`: Finish Raw Rust String Removal Guardrails

Why next: optional projection now has stronger shape tagging, but `0007` remains
open until fragile rendered-string checks are removed or guarded by a lint.

Evidence:

- `claude_audits/0007-rust-ir-is-mostly-opaque-strings.md`
- `src/coercion.rs`
- `src/conversion_plan.rs`

Resolution idea:

Add a focused anti-regression check for new raw rendered-string predicates in
lowering/coercion modules, with a narrow allowlist for existing debt. Then
migrate one producer family at a time to explicit `RustSlotShape` facts.

First verification:

```bash
rg "contains\\(|ends_with\\(|starts_with\\(" src/coercion.rs src/conversion_plan.rs src/call_lowering.rs
cargo test --release --test slot_shape_design conversion_plan_does_not_render_rust_expressions
```

## Small, High-Value Cleanup Tasks

- `0064`: local/static shadowing has been tightened for source static method
  receivers and type-qualified static fields. The remaining residue is broader
  name identity, especially places still asking package/type questions through
  emitted simple strings rather than an owner key.
- Add generated cargo-check coverage to
  `implicit_source_super_constructor_initializes_inherited_fields`; previous
  passes found invalid Rust hidden behind assertion-only testing.
- Feed `generated-shared-support.rs` into `scripts/sweep-generated-idioms.py` so
  shared support idioms are not invisible in focused summaries.
- Add a compact source-summary constants regression cluster with same-simple-name
  constants across packages, qualified array dimensions, and facade-associated
  constants.
- Before any major `0002b` switch, run an output-inert pass migrating remaining
  facts-only type render calls to representation-aware renderers, requiring
  byte-identical output.

## Verification Discipline

For any change:

```bash
cargo test --release --test <suite> <focused_test_name>
scripts/gate.sh --all
```

For `idiom_gaps`, compare failure names with:

```bash
diff -u tests/baselines/idiom-gaps-failures.txt <new_failure_names_file>
```

Expected healthy movement:

- failure names removed: good
- same failure names and same counts: neutral
- new failure names: regression unless explicitly explained
- `idiom_gaps` hangs: regression; this suite must return
