# 0061 — Id-less compatibility paths can still rejoin local binding plans by name

Status: **open, compatibility-only residue**

Current status 2026-08-30:
the original ordinary-source same-name-local failure described by the historical
title is no longer valid.
`LocalPlan` and `LocalConstraintSet` carry `BindingId`, resolver-assigned ids
are used for parameters, declarations, enhanced-for bindings, and catches, and
bound source declarations fail closed instead of falling back to name lookup.
The remaining live surface is narrower compatibility residue: synthetic or
parser-only IR that lacks resolver ids can still use deliberate name fallback
through the usage/signature compatibility paths. Treat future fixes here as
id-less-boundary cleanup, not as evidence that normal Java source locals are
still rejoined by name.

Codex follow-up 2026-09-14: the id-less borrowed-return compatibility scanner
now tracks lexical shadowing while checking exact returned names and assignment
targets. A same-source-name local declared in an inner block can mutate without
being mistaken for the id-less parameter, while a same-name local returned from
the method no longer counts as returning the parameter. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_shadowed_local_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_shadowed_local_return`.
Codex follow-up 2026-09-14 later: the same id-less scanner now treats catch
parameters as lexical shadows too. A `catch (Exception value)` assignment no
longer rejects borrowed-return policy for an outer id-less `value` parameter,
and a return from inside that catch body no longer proves the outer parameter is
returned. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_shadowed_catch_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_shadowed_catch_return`.
Codex follow-up 2026-09-14 later: try-with-resources declarations now shadow
the id-less binding in the try body as well. The resource initializer itself is
still scanned under the outer binding, but body assignments and returns to the
same source-name resource are not attributed to the outer parameter. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_shadowed_resource_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_shadowed_resource_return`.
Codex follow-up 2026-09-14 later: switch statement groups now carry that same
lexical shadowing state forward across later case groups. A same-source-name
declaration in one case prevents later case assignments or exact returns from
being attributed to the outer id-less parameter. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_switch_shadowed_case_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_switch_shadowed_case_return`.
Codex follow-up 2026-09-14 later: positive `instanceof` pattern bindings in an
`if` condition now shadow the id-less parameter while scanning the `then` arm.
A same-source-name pattern binding assignment is no longer treated as mutating
the outer returned parameter, and a returned pattern binding no longer proves
the outer parameter was returned. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_pattern_binding_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_pattern_binding_return`.
Codex follow-up 2026-09-14 later: negated `instanceof` pattern bindings now
shadow the id-less parameter while scanning the `else` arm of an `if`. A
same-source-name pattern binding reached through `if (!(... instanceof T value))
{ ... } else { ... }` is no longer mistaken for the outer id-less `value`
parameter when checking assignments or exact returns. The branch-scope helper
also recurses through negation, so double-negated pattern conditions do not fall
back to the outer id-less binding either. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_negated_pattern_else_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_negated_pattern_else_return`,
plus
`slot_policy::tests::idless_borrowed_return_policy_rejects_double_negated_pattern_return`.
Codex follow-up 2026-09-14 later: the same positive pattern-binding shadow now
applies to `while` loop bodies. A same-source-name pattern variable assigned in
the loop body is not attributed to the outer id-less parameter, and a return
from that loop body no longer proves the outer parameter was returned.
Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_while_pattern_binding_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_while_pattern_binding_return`.
Codex follow-up 2026-09-14 later: `for` loop conditions now get the same
positive pattern-binding shadowing. `for (; input instanceof String value; )`
no longer treats assignments to the pattern variable as mutations of the outer
id-less parameter, and returning that pattern variable from the loop body no
longer proves the parameter was returned. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_for_pattern_binding_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_for_pattern_binding_return`.

Codex follow-up 2026-09-14 later: id-less signature local lookup now validates
the `local_index_by_name` entry before using it. A stale compatibility index
such as `value -> 0` no longer returns `locals[0]` when that local's stored name
is `other`; the helper falls back to the unique real name scan or fails closed.
Regression:
`signature::tests::idless_method_local_lookup_rejects_stale_name_index`.
Codex follow-up 2026-09-14 later: the id-less borrowed-return mutation scanner
now inspects evaluated constructor surfaces instead of treating them as
harmless. `this(value = "x")`/`super(value = "x")` constructor invocation
arguments and qualified inner construction receivers such as
`(value = "x").new Inner()` now reject borrowed-return policy before a later
`return value`. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_rejects_constructor_invocation_arg_mutation`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_qualified_new_receiver_mutation`.
Codex follow-up 2026-09-14 later: the same scanner is now regression-pinned for
`do { ... } while (...)` conditions. A condition assignment such as
`while (value = "next")` is evaluated before the later `return value`, so the
id-less borrowed-return policy must reject it just like `while`/`for` conditions
and update expressions. Regression:
`slot_policy::tests::idless_borrowed_return_policy_rejects_do_while_condition_mutation`.

Follow-up cleanup renamed the private name-keyed local constraint readers on
`MethodConstraintSet` and `ConstructorConstraintSet` to
`idless_compatibility_local*`. Resolver-bound propagation continues to use
`local_by_binding`/`local_mut_by_binding`; the remaining source-name path is now
grep-visible as compatibility fallback instead of hiding behind generic
`local(name)` helpers. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up 2026-09-09: `MethodUsageFacts` no longer exposes the boolean
`binding_for_declaration_with_name_fallback` footgun. Exact binding-id lookup is
now `binding_facts_for_declaration_id`, and the remaining legacy source-name
path is spelled `idless_compatibility_binding_facts_for_source_name`.
`binding_facts_for_param` remains the single parameter helper that chooses
between those two paths based on whether resolver binding identity exists. This
preserves current compatibility behavior while making the id-less boundary
grep-visible. Guard:
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

Follow-up 2026-09-07: compatibility name-map entries now have to agree on the
stored `BindingUseFacts.name` as well as any attached `BindingId`. A stale entry
stored under `bindings["value"]` with payload name `other` can no longer be
treated as current, recovered to an authoritative binding id, snapshotted across
a scope, aggregated, or used to collect reads/returns by lookup key. Regressions:
`usage::tests::name_map_payload_name_must_match_the_lookup_key` and
`usage::tests::bound_name_map_payload_name_must_match_the_lookup_key`.
The same guard now applies at the scope-restore boundary: restoring a saved
visible binding under `value` rejects a payload whose stored name is `other`,
instead of repopulating the compatibility map with a known-stale entry.
Regression:
`usage::tests::restore_visible_binding_rejects_mismatched_saved_payload_name`.
The ID-keyed compatibility table now has the matching guard too: a
`bindings_by_id[value#1]` payload whose stored name drifted to `other` is not
returned by binding-id lookup, visible-name recovery, or aggregate readers.
Regression:
`usage::tests::binding_id_payload_name_must_match_the_binding_key`.
Lowering's current-callable use-count/fact helpers now consume that same
validated ID lookup through `MethodUsageFacts::binding_facts_for_id` instead of
reading `bindings_by_id` directly. Regression:
`usage::tests::binding_facts_for_id_rejects_stale_payload_name`; layering guard:
`layering::lowering_binding_fact_queries_stay_binding_id_keyed`.
Signature and slot-policy parameter consumers now query id-less compatibility
facts with the Java `Param::source_name`, not the escaped Rust `Param::name`.
A keyword-renamed synthetic parameter such as `source_name: "type"` /
`name: "type__"` therefore finds the same usage facts that analysis records and
uses the same spelling for exact-return/assignment scans. Regressions:
`slot_policy::tests::idless_borrowed_return_param_policy_uses_source_name_not_escaped_name`
and
`signature::tests::method_param_boundary_policy_uses_source_name_for_keyword_param_usage`.
Constructor parameter nullability now uses the same source-name key when reading
body-derived source nullability constraints, so a keyword-renamed constructor
parameter no longer loses nullable storage evidence. Regression:
`signature::tests::constructor_param_nullability_constraints_use_source_name`.
The raw-collection return-type refinement path now follows the same rule:
`method_declared_binding_types` keys parameters, locals, and enhanced-for
variables by Java source spelling because returned `Expr::Var` names are source
names. This prevents keyword-renamed declarations from losing their generic
return element type during raw `List`/`Map` refinement. Regressions:
`signature::tests::method_return_type_for_signature_uses_param_source_name`,
`signature::tests::method_return_type_for_signature_uses_local_source_name`,
and
`signature::tests::method_return_type_for_signature_uses_enhanced_for_source_name`.
Lambda compatibility scoping now uses `LambdaParam::source_name` too. A lambda
parameter whose emitted Rust name is escaped, such as Java `self`, now hides the
outer source-name binding while analyzing the lambda body instead of attributing
the lambda body reads to the outer binding. Regression:
`usage::tests::lambda_keyword_param_scope_uses_source_name`.
Liveness `try/catch` handling also no longer removes catch parameters from
catch live sets by source name alone. The filter now removes only the catch
binding declared by that `try`, preserving any distinct same-name binding
already present in the set. Regression:
`liveness::tests::catch_binding_filter_preserves_distinct_same_name_bindings`.
Follow-up cleanup narrowed the `liveness` module itself to crate-internal use;
the production liveness questions remain available to lowering, while the two
raw set-projection accessors used only by same-module regression tests are now
`#[cfg(test)]` private helpers. Guard:
`layering::declaration_context_usage_lookups_use_java_callable_keys`.
Follow-up cleanup also narrowed `LivenessAnalysis`, its production query
methods, and the internal `Binding`/`OUTER_DECL` representation to crate or
same-module visibility. Lowering and signature analysis still consume the
same liveness relation, but the binding-identity carrier is no longer exposed
as public crate surface.

Follow-up 2026-09-06, updated 2026-09-10: the centralized `MethodUsageFacts`
compatibility readers remain available to internal analysis/lowering modules,
but the mutable maps are no longer public or crate-local APIs.
`binding_for_declaration_with_name_fallback` is gone; the visible-name helpers
and `name_binding_is_current` are crate-local. `all_binding_facts()` remains as
the public read-only aggregate view for integration tests and production passes
that need all validated facts without reaching into the legacy maps.
`expr_shape::idless_compatibility_local_plan_for_declared_type` is also
crate-local now and no longer hangs off `ShapeEnv`; the only production entry remains
`lower::idless_compatibility_local_plan_for_declaration`.
`lower::local_plan_for_declaration` now takes `&ShapeEnv` rather than
`&mut ShapeEnv`, matching the read-only compatibility lookup boundary and
preventing declaration-plan selection from implying cursor-style mutation.
The fallback shape builder used by that compatibility path,
`fallback_local_shape_from_declared_type`, is crate-local as well; the layering
guard pins both boundaries.
`UsageFacts::source_method_usage_for_owner` is private now; internal
cross-module callers use the narrower resolved-owner wrapper instead of a broad
owner-explicit Java-key lookup.
Follow-up cleanup narrowed the source binding/field Rust-name allocation helpers
in `java_naming` to crate-local. They remain the centralized internal policy for
escaped source names, but integration tests now guard the boundary instead of
requiring those allocators as public crate API.

Follow-up 2026-09-09: direct name-based lookup is no longer an API.
`MethodUsageFacts::binding_facts_for_name` and the unused public name-presence
helper `MethodUsageFacts::contains_binding_name` are gone. Follow-up cleanup
also migrated the remaining integration tests off
`MethodUsageFacts::unique_binding_facts_for_name` and deleted that API. The
external regression tests now rely on declaration `BindingId`s or existing
field-use assertions, while the layering guard rejects reintroducing any of the
three name-based `MethodUsageFacts` lookup helpers.

Follow-up 2026-09-09: the redundant public exact-id predicate
`MethodUsageFacts::contains_binding_id` is gone too. Integration tests now call
`binding_facts_for_id(...).is_some()` directly when they need to assert exact
resolver binding coverage, leaving only the accessor that returns the validated
fact payload.

Follow-up 2026-09-09: the parameter binding lookup policy is now owned by
`MethodUsageFacts::binding_facts_for_param`. Signature and slot-policy code no
longer carry their own `binding_usage_for_param` copies; they ask the usage
facts owner to choose between resolver `BindingId` lookup and id-less
source-name compatibility fallback. Regression:
`usage::tests::binding_facts_for_param_uses_source_name_and_binding_id`; layering
guard:
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

Follow-up 2026-09-10: the remaining signature unit-test fixture that needed a
deliberately stale split between `bindings_by_id` and the id-less compatibility
name map now builds that state through test-only `MethodUsageFacts` helpers
instead of mutating both private maps from `signature.rs`. The slot-shape
integration test extension also dropped the old `binding_facts_for_name`,
`unique_binding_facts_for_name`, and `contains_binding_name` vocabulary in
favor of explicitly named id-less source-name compatibility helpers, and its
exact-id assertions call `binding_facts_for_id(...).is_some()` directly instead
of keeping a local `contains_binding_id` shim. Guard:
`layering::legacy_usage_binding_name_map_is_owned_by_usage_module`.
Follow-up cleanup: the broad
`MethodUsageFacts::from_visible_binding_fixture_facts` constructor is private to
`usage.rs` tests. The one external unit test that needed a deliberately visible
same-name split now seeds exact id facts and the compatibility visible-name row
with the narrower test insertors instead of importing the combined constructor.
Guard: `layering::idless_local_binding_compatibility_surface_stays_explicit`.
Codex follow-up 2026-09-10: `MethodUsageFacts::binding_facts_for_id` is
crate-local now. Integration tests that need exact binding-id assertions go
through the public validated `all_binding_facts()` view with test-local helpers,
so downstream users cannot depend on the direct exact-id compatibility reader.
Guard: `layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.
Codex follow-up 2026-09-12 narrowed four `ShapeEnv` local side-fact helpers to
crate-local: `insert_simple_immutable_map_entry_local`,
`simple_map_entry_local_is_immutable`, `insert_nested_array2_local`, and
`local_uses_nested_array2_slice`. Lowering still uses those binding-aware local
side tables internally, but they are no longer public API for name-keyed local
fact mutation/inspection. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up 2026-09-10: the generic mixed `MethodUsageFacts::from_binding_facts`
constructor is gone. Fixtures and internal tests now spell whether they are
building exact resolver-bound facts (`from_exact_binding_facts`), id-less
compatibility facts (`from_idless_compatibility_facts`), or the narrow
test-only visible-name fixture map (`from_visible_binding_fixture_facts`). This
removes one remaining broad construction path that silently populated the
legacy name map from resolver-bound facts. Regression:
`usage::tests::binding_fact_constructors_separate_exact_and_idless_surfaces`;
the layering guard rejects reintroducing the generic constructor name.

Follow-up 2026-09-10: the `MethodUsageFacts` backing maps for exact
`BindingId` facts and id-less compatibility name facts are now private to the
usage module. External production code was already using the owner helpers; the
visibility fence makes direct crate-wide map reads or mutation impossible while
preserving the remaining compatibility surface inside `usage.rs`. Guard:
`layering::callable_analysis_shell_reaches_lowering_context`.
Follow-up cleanup also narrowed the arbitrary `UsageFacts::insert_method_usage`
helper to test-only crate-local visibility; external callers keep the named
current Rust-name insertion helpers instead of constructing raw compatibility
keys.

Follow-up 2026-09-10 signature propagation cleanup: resolved constructor
arguments are no longer all classified as `InternalSameType`; the constructor
argument boundary now stays internal only when the resolved constructor owner is
the current type. Cross-type construction therefore keeps the unknown-boundary
pressure that forces aliased array parameters to owned/native when they leave
the current class. The parameter constraint normalizer now lets such owned
requirements replace borrowed-slice defaults instead of rejecting them after an
alias has already supplied shared/slice constraints. Regressions:
`slot_shape_design::signature_fixpoint_propagates_new_argument_unknown_boundaries_through_aliases`
and `signature::tests::owned_param_constraint_replaces_borrowed_slice_default`.
The neighboring constructor-internal-call fixture was renamed to assert the
current read-only borrowed-slice path for a resolved source receiver call,
rather than the old unknown-boundary expectation.

Follow-up 2026-09-07 guardrail cleanup: the layering guard for legacy
`MethodUsageFacts.bindings` access now scans every production `src/*.rs` module
that mentions `MethodUsageFacts`, excluding only `usage.rs` where the
compatibility map is owned. It rejects direct `.bindings` and `.bindings_by_id`
get/mutate calls outside the owner module, so new consumers such as callable
analysis or storage strategy cannot reopen a name-map join without going through
validated accessors or `all_binding_facts()`. Regression:
`layering::legacy_usage_binding_name_map_is_owned_by_usage_module`.

Follow-up 2026-09-07, updated 2026-09-10: the raw
`MethodUsageFacts.bindings` and `bindings_by_id` maps are private to
`usage.rs`, not public or crate-local APIs. Integration tests were moved off
direct map access onto validated read helpers instead of reaching into the
maps. The conversion exposed one stale split:
`Stmt::VarDecl` initializer call-result usage was recorded through the visible
name helper after the declaration was inserted, leaving the binding-id fact with
a different boundary than the legacy name-map payload. Declared initializer
call-result usage now records against the declaration `BindingId` when present
and mirrors the current compatibility map, so validated fact readers see the
same answer.

Partial progress:

- Display-hint synthesis now gives untyped lambda parameters a scoped binding
  when the lambda is assigned to a declared functional-interface local such as
  `Consumer<T>`/`Function<T, R>`/`Predicate<T>`. That keeps a lambda parameter
  named like an outer field from rejoining to the field while collecting facade
  display return hints. Regression:
  `stdlib::display_hint_tests::inferred_lambda_param_shadows_outer_binding_for_display_hint_owner`.
- The signature solver's own constraint-to-plan join now carries explicit
  `BindingId`s on `LocalConstraintSet` and `LocalPlan`. Resolver-assigned ids on
  `VarDecl` and `EnhancedFor` are used when present; otherwise the signature
  traversal assigns compatibility ids for parsed-only or synthetic inputs.
  `collect_local_plans_from_stmts` looks up constraints by that id rather than
  by a `(name, type)` occurrence cursor. Regressions:
  `signature::tests::local_constraints_are_selected_by_binding_id`,
  `same_name_same_type_locals_use_their_own_signature_constraints`, and
  `same_name_disjoint_locals_do_not_share_mutability`.
- Java IR now carries optional resolver-assigned `BindingId`s on `Param`,
  `Stmt::VarDecl`, `Stmt::EnhancedFor`, and `CatchClause`. The resolver assigns
  ids per callable in source traversal order, and ordinary `VarDecl` lowering
  plus delegated-constructor trailing `VarDecl` shape learning select
  `LocalPlan`s by declaration id. Bound source declarations now fail closed to a
  conservative shape if their id has no solved plan, instead of falling back to
  the old occurrence cursor or name lookup; unassigned synthetic paths keep the
  compatibility fallback. Regressions:
  `resolver_assigns_binding_ids_to_local_declarations`,
  `lower::tests::local_plan_for_bound_declaration_does_not_fall_back_by_name`,
  `expr_shape::tests::bound_var_decl_shape_learning_does_not_fall_back_by_name`,
  and
  `expr_shape::tests::bound_enhanced_for_shape_learning_does_not_fall_back_by_name`.
- The concrete `FeatureReader` symptom that exposed the same-name path is fixed:
  concrete typed-erased facade returns render as their named facade type, and an
  anonymous external erased-facade implementation lowers to the concrete slot
  default instead of `Box<dyn Any>` when the target slot is concrete/native.
  Regressions:
  `anonymous_external_facade_return_keeps_declared_concrete_shape_for_internal_method`
  and
  `duplicate_local_names_keep_selected_concrete_shape_before_map_insert_conversion`.
- Signature local mutability no longer comes from the whole-method mutated-name
  set for source declarations. Local constraint collection now asks the shared
  liveness write-after relation about the specific declaration statement, while
  preserving mutability proven by usage/external-argument facts. Regression:
  `same_name_disjoint_locals_do_not_share_mutability`.
- For-loop header update/condition expressions now resolve names in the
  post-initializer loop scope, not the pre-loop scope. This fixed the repeated
  generated-Rust shape `let i: i32 = 0; ... i = i + 1` for ordinary
  `for (int i = 0; ...; i++)` loops. Regression:
  `liveness::tests::for_update_mutates_initializer_binding`; focused generated
  Rust for `examples/java/02-control-flow/LabeledBreak.java` now compiles.
- Assignment lvalue roots now feed the same write-after relation for field and
  index targets. `first.value = ...` and `a[i] = ...` mark the receiver/root
  binding mutable without treating ordinary reads of `first.value` as writes.
  Regression: `liveness::tests::field_assignment_mutates_receiver_binding`;
  focused generated Rust for `examples/java/04-objects/AliasingSharedMutation.java`
  now compiles. Its remaining alias warning is still the open SharedLocal/0002
  semantic gap, not a Rust compile failure.
- The remaining lowering-side occurrence cursor no longer saturates to the last
  same-name plan when traversal overruns the solved local list. It now fails
  closed (`None`), so a traversal mismatch degrades to conservative local
  lowering instead of silently reusing another declaration's answer. It also no
  longer falls back from a declared-type mismatch to a same-name plan or shape;
  id-less compatibility requires the same erased declared type identity, either
  from the stored local type or from the solved concrete shape for erased-carrier
  facade locals. Id-less declarations without initializers also no longer consume
  the occurrence cursor, so a scoped placeholder declaration cannot steal the
  plan for a later initialized declaration.
  Regressions:
  `expr_shape::tests::local_plan_occurrence_overrun_does_not_reuse_the_last_plan`,
  `expr_shape::tests::local_plan_type_mismatch_does_not_fall_back_by_name`, and
  `expr_shape::tests::local_shape_type_mismatch_does_not_fall_back_by_name`.
- Delegated-constructor trailing `VarDecl` statements no longer consume the
  local-plan occurrence cursor twice. The trailing statement lowerer now updates
  the same constructor-trailing environment used by later statements, and the
  separate shape-learning pass is skipped for statements that have already been
  lowered. Regression:
  `lower::tests::delegated_constructor_trailing_vardecl_consumes_local_plan_once`.
- `ShapeEnv` insertion paths now preserve resolver-assigned binding ids instead
  of synthesizing fresh ids in the lowering environment. This covers source
  parameters, ordinary local declarations, enhanced-for variables, and catch
  parameters; the unbound synthetic paths still use the compatibility
  insertors. Regressions:
  `expr_shape::tests::bound_local_env_insert_preserves_resolver_binding_id`,
  `expr_shape::tests::bound_param_env_insert_preserves_resolver_binding_id`,
  `expr_shape::tests::bound_loop_variable_env_insert_preserves_resolver_binding_id`,
  `expr_shape::tests::bound_catch_param_env_insert_preserves_resolver_binding_id`,
  plus the existing `bound_` shape-learning tests.
- The scoped type-parameter/source-class collision regression now asserts the
  current optional/exception lowering shape and explicitly rejects handles on
  the scoped type-parameter slots. The generated crate still compiles, so the
  concrete class `ELEM` and generic parameter `ELEM` remain distinct at the
  rendered Rust boundary.
- Source nullability's name-keyed compatibility map is now fenced to id-less
  declarations. After collecting the legacy by-name state, resolver-bound
  parameters, ordinary locals, enhanced-for variables, and catch parameters are
  removed from that map; bound declarations keep using `BindingId` lookup.
  Regression:
  `signature::tests::bound_nullability_constraints_do_not_fall_back_by_name`.

Update 2026-08-26: the lowering occurrence cursor described in the original
finding is gone in the current tree.
`expr_shape::idless_compatibility_local_plan_for_declared_type` is a
module-level helper rather than a `ShapeEnv` cursor method; it accepts only a
unique exact `(name, declared type)` compatibility match, with a narrow
erased-object boundary special case, and otherwise fails closed. The one
production caller is explicitly named
`idless_compatibility_local_plan_for_declaration`, and `tests/layering.rs` now
guards both the single caller and the absence of the old cursor symbols. The
  audit remains open for the remaining compatibility surface: the legacy
  `MethodUsageFacts.bindings` map is still keyed by name, and a few id-less or
  synthetic paths still accept name fallback by explicit policy.
- Follow-up 2026-08-26, current names refreshed 2026-09-10: signature's
  remaining local-usage helper no longer reaches directly into
  `MethodUsageFacts.bindings` to implement the id-less fallback. Exact
  declaration-id lookup is centralized on
  `MethodUsageFacts::binding_facts_for_declaration_id`, while parameter callers
  use `MethodUsageFacts::binding_facts_for_param` for the explicit id-less
  source-name compatibility path. Focused regressions:
  `usage::tests::binding_facts_for_declaration_id_does_not_fall_back_by_name`,
  `signature::tests::binding_usage_for_bound_local_does_not_fall_back_by_name`,
  `layering::idless_local_binding_compatibility_surface_stays_explicit`, and
  `layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.
- Follow-up 2026-08-26: `expr_shape`'s id-less shape-learning helper now shares
  the same unique-match policy as
  `expr_shape::idless_compatibility_local_plan_for_declared_type`.
  Duplicate same-name/same-type compatibility plans fail closed instead of
  returning the first shape. Regression:
  `expr_shape::tests::ambiguous_same_name_same_type_idless_local_shape_fails_closed`.
- Follow-up 2026-08-26: delegated-constructor trailing `VarDecl` shape learning
  now preserves the resolver binding id when it refreshes the constructor
  trailing environment. The path already selected a binding-aware `LocalPlan`,
  but reinserted the shape with `insert_local_shape`, synthesizing a fresh
  name-keyed binding. Regression:
  `lower::tests::delegated_constructor_trailing_vardecl_shape_preserves_binding_id`.
- Follow-up 2026-08-26: assignment and branch-assignment shape refinement now
  preserve the current environment binding id when replacing a local's shape
  with a more precise generic value shape. Regressions:
  `lower::tests::assigned_local_shape_refinement_preserves_binding_id` and
  `lower::tests::branch_local_shape_refinement_preserves_binding_id`.
- Follow-up 2026-08-26: `usage`'s remaining visible-name-to-binding-id recovery
  is centralized on `MethodUsageFacts::visible_binding_id_for_name`. Assignment
  aliasing and `record_use` no longer open-code `.bindings.get(name)` when they
  need the current scoped binding id. Regressions:
  `usage::tests::visible_binding_id_for_name_centralizes_compatibility_lookup`
  and
  `usage::tests::usage_name_binding_id_recovery_goes_through_visible_binding_helper`.
- Follow-up 2026-08-26: that centralized visible-name recovery now treats the
  binding-id table as authoritative. A name-map entry that claims a binding id
  absent from `bindings_by_id` fails closed instead of propagating a phantom
  target id into assignment aliases or duplicated use records. Regression:
  `usage::tests::visible_binding_id_for_name_rejects_stale_name_map_binding`.
- Follow-up 2026-08-26: the stale-name guard is now applied to the consumers
  that still expose compatibility facts. `record_use` no longer appends uses to
  a name-map entry whose claimed binding id is absent from `bindings_by_id`, and
  `all_binding_facts` no longer reports that phantom entry as a fallback fact.
  Regressions:
  `usage::tests::visible_binding_id_for_name_rejects_stale_name_map_binding`
  and `usage::tests::record_use_rejects_stale_name_map_binding`.
- Follow-up 2026-08-26: field-shadow detection now uses the same stale-name
  policy. A compatibility name-map entry whose binding id is absent from
  `bindings_by_id` no longer shadows a same-named field when usage analysis
  decides whether a bare name can be a field. Regression:
  `usage::tests::stale_name_map_binding_does_not_shadow_field`.
- Follow-up 2026-08-28: the remaining name-set local-mutability helper is no
  longer a public `signature` API named like a general shape primitive. It is
  private and explicitly named
  `legacy_name_set_local_mutability_constraints`, leaving the public surface on
  the binding/liveness-aware path and keeping the old set-of-names fallback
  visibly fenced inside signature's compatibility implementation.
- Follow-up 2026-08-28: two stale unit expectations that still said id-less
  compatibility should fall back by name were revised to match the current
  fail-closed policy. A name-only fact remains usable only when there is no
  same-name resolver-bound history and no stale missing binding id.
- Follow-up 2026-08-28: the same scoped identity hole was visible in clone
  recovery for a method or class type parameter whose source name collides with
  a concrete source class. `scoped_type_param_does_not_collide_with_same_named_source_class`
  now rejects `Rc<ELEM>`/`Arc<ELEM>` and unnecessary `.clone()` recovery for the
  scoped type-parameter slots. Class type-parameter impl bounds are also no
  longer added just because a field contains `T` and some callable mentions
  `T`; `T: Clone` is kept for concrete owned recovery from collection/map
  fields, preserving the `Map<String,T>.get(...).cloned()` case.
- Follow-up 2026-08-29: source-super/`this(...)` constructor compatibility
  substitution no longer looks like a general local-binding primitive. The
  remaining by-name substitution helpers are private and explicitly named
  `substitute_source_constructor_param_*_by_name_compat`, and
  `tests/layering.rs::constructor_delegation_name_substitution_stays_fenced`
  guards that every use remains inside the constructor-delegation section. This
  preserves existing constructor-flattening behavior while making the name-based
  residue visible and bounded.

Dimension: phase architecture / information lost between passes.

## Symptom

Historical status: the concrete same-name local symptoms in this section are
now fixed and guarded. Keep this section as origin evidence for the audit, not
as a current failure report. The live complaint is the narrower one stated at
the top of this file: deliberately retained id-less/synthetic compatibility
paths can still use name fallback, while resolver-bound source declarations use
`BindingId`-keyed plans/facts and fail closed instead of rejoining by name.

The signature solver computes a `LocalPlan` (Rust slot shape + binding
mutability) for every local declaration in a method. Lowering then has to work
out *which* `LocalPlan` belongs to *which* declaration. Nothing in the plan
records that, so each phase re-derives the correspondence from the local's
*name*, and the derivations do not agree.

Consequences visible in output:

- Adding a declaration in a disjoint block changes the generated Rust for an
  unrelated local of the same name earlier in the method.
- A local can acquire `mut` because a *different* local elsewhere is mutated,
  or lose the `mut` the analysis gave it because *some* other local shares its
  name.
- When a declaration has no plan at all (see `0060`), lowering does not report
  a missing plan — it silently returns the plan of a different variable.

The signature solver already knows the correspondence exactly: it built the
`Vec<LocalPlan>` by walking the body in source order. That order is the only
thing carrying the identity, and it is carried by an ad-hoc mutable cursor in a
different module.

## Verified evidence

### The identity is now partial, not absent

`src/signature.rs`:

```rust
pub struct LocalPlan {
    pub binding: BindingId,
    pub name: String,
    pub java_ty: JavaType,
    pub shape: RustSlotShape,
    pub binding_mutable: bool,
}
```

`src/usage.rs`:

```rust
pub struct MethodUsageFacts {
    bindings: HashMap<String, BindingUseFacts>,   // keyed by *name*
    bindings_by_id: HashMap<BindingId, BindingUseFacts>,
    ...
}
```

The original finding that neither type carried declaration identity is stale:
`BindingId` is now plumbed through `java_ir`, `resolve`, `usage`, `signature`,
`expr_shape`, and `lower`. The remaining defect is the compatibility surface:
`LocalPlan` still carries a separate `name`, the internal
`MethodUsageFacts.bindings` compatibility map remains name-keyed, and some
id-less/synthetic paths can still rejoin by `(name, type)` occurrence. The maps
are private now; the risk is retained compatibility state, not public mutable
evidence.

The identity type is still the right primitive:

```rust
pub struct BindingId { pub kind: BindingKind, pub name: String, pub occurrence: usize }
```

### The four re-derivations

1. **`usage` used to merge only by name.** `MethodUsageFacts.bindings`
   remains as the compatibility map, but `MethodUsageFacts.bindings_by_id`
   now records resolver-assigned `BindingId` entries for parameters,
   `VarDecl`s and enhanced-for variables. Local signature constraints prefer
   the binding-keyed entry before falling back to the name map. Several older
   propagation consumers still iterate the name map, so this is no longer the
   root data loss for local constraints, but it is not yet a complete removal
   of name-based usage.

2. **`signature` used to detect the collision and discard the facts.**
   This is historical now. Local signature constraints and plans are
   binding-keyed for ordinary resolver-bound declarations, so duplicate
   source names no longer disable all usage-derived local constraints. The
   remaining duplicate-name check is the explicit source-nullability
   compatibility view: `source_binding_nullability_constraints` still fails
   closed when an id-less name lookup would be ambiguous.

3. **`signature` used to rejoin its own two walkers by `.find()`.**
   This is historical too: the old `local_constraints_for_declared_type`
   helper is gone, and source declarations with `BindingId`s query their own
   binding-keyed constraints. The remaining name/type matching lives in the
   id-less compatibility surfaces documented below, not in normal local plan
   selection.

4. **Lowering now has an explicit id-less compatibility helper.**
   The original shared mutable cursor has been removed. Current
   `expr_shape::idless_compatibility_local_plan_for_declared_type` filters the
   plan slice by `(name, declared type)` and returns a plan only when the match is
   unique, or when the erased-object boundary special case finds one
   boundary-shaped plan. It no longer has `local_plan_occurrences` or
   `consume_local_plan_occurrence`. The remaining concern is not cursor drift
   but the deliberately retained id-less compatibility surface:
   `lower::idless_compatibility_local_plan_for_declaration` is the sole
   production caller, and it must stay fenced to synthetic/unassigned
   declarations.

### Observable divergence — a shadow in a disjoint block changes unrelated output

`F.java`, where `three` is `one` plus a trailing `if (b) { ... }` block that
redeclares `a`, `sb` and `c`:

```java
public String one(List<String> xs, Map<String,String> m, boolean b) {
    String a = m.get("k");
    StringBuilder sb = new StringBuilder();
    sb.append(a);
    List<String> c = new ArrayList<>();
    c.add(a);
    return sb.toString();
}
public String three(List<String> xs, Map<String,String> m, boolean b) {
    /* identical five statements */
    if (b) { String a = "z"; StringBuilder sb = new StringBuilder();
             List<String> c = new ArrayList<>(); sb.append(a); c.add(a); }
    return sb.toString();
}
```

emits

```rust
pub fn one(...)   { ... let mut sb: StringBuilder = StringBuilder::new_0(); sb.append(a.as_ref()); ... }
pub fn three(...) { ... let     sb: StringBuilder = StringBuilder::new_0(); sb.append(a.as_ref()); ... }
```

The outer `sb` loses `mut` in `three` — mechanism 2 above: `sb` is a duplicate
name, so usage is `None`, so `MutableBinding` is never derived. Harmless only
because the emitted `StringBuilder::append` happens to take `&self`; the
mechanism is not type-specific.

The reverse direction, `I.java`:

```java
public String leak3(Map<String,String> m, boolean f) {
    Map<String,String> t = new HashMap<>(m);
    if (f) { List<String> t = new ArrayList<>(); t.add("q"); System.out.println(t.size()); }
    return t.get("k");
}
public String noshadow(Map<String,String> m, boolean f) { /* same, inner local named u */ }
```

emits `let mut t: HashMap<String, String>` in `leak3` and `let t: HashMap<...>`
in `noshadow`. The outer map is never mutated; it becomes `mut` because
`local_shape_constraints` (`src/signature.rs:10422-10428`) tests
`mutated: &HashSet<String>` — a set of *names* — and the inner `Vec` named `t`
is mutated.

### The plan is treated as a hint, not a decision

After the lookup, `src/lower.rs:7916-7950` applies five corrections to the
solved plan before using it:

```rust
local_plan = refine_local_plan_from_initializer(...);          // 7923
local_plan = own_uninitialized_optional_borrowed_local(...);   // 7925
if let Some(shape) = exact_erased_object_cast_initializer_local_shape(...) { local_plan.shape = shape; }   // 7927
if declared_typed_initializer_should_anchor_local_type(...) { local_plan.shape = ...; }                    // 7934
if mutated_later { local_plan.binding_mutable = true; }                                                    // 7948
```

with the comment at `:7942`:

> `// A local that is written later must be declared mut. Ask the analysis about`
> `// this statement's binding rather than a name set.`

— an in-source admission that the solver's mutability came from a name set and
is not trusted. Local mutability is therefore decided in three places with
three different notions of identity: `signature`'s `HashSet<String>` (name),
`ctx.liveness.binding_is_mutated_after(stmt, name)` (statement + name), and
`conservative_lambda_local_plan` (`src/lower.rs:10457-10470`, hardcoded
`true`). `declared_typed_initializer_should_anchor_local_type` exists twice
under the same name with different parameter lists —
`src/signature.rs:11735` (takes `&FactSet`) and `src/lower.rs:10700` (takes
`&ShapeEnv, LowerCtx`) — the same decision computed from different inputs in
two phases.

## Root cause

Historical status: this root cause no longer describes ordinary parsed Java
source in the current tree. `Param`, `Stmt::VarDecl`, `Stmt::EnhancedFor`, and
`CatchClause` now carry `binding: Option<BindingId>`, and the bound source path
looks up local plans by `BindingId`. The remaining root cause is compatibility
only: some synthetic/parser-only IR can still arrive without a resolver id, so
those callers must stay explicitly fenced and fail closed on ambiguity.

Ordinary parsed Java declarations now do have resolver identity: `Param`,
`Stmt::VarDecl`, `Stmt::EnhancedFor`, and `CatchClause` carry
`binding: Option<BindingId>`. The remaining lossy surface is the explicit
id-less compatibility path for synthetic/parser-only declarations whose
`binding` is still `None`. Callers on that path must treat the source name as
ambiguous unless the compatibility helper proves a unique `(name, declared
type)` match.

Because the key is lossy, each phase repairs the lossiness in its own way:
`usage` merges, `signature` gives up (mechanism 2) or takes the first
(mechanism 3), and lowering assumes traversal-order lockstep (mechanism 4).
Each repair is locally defensible and they compose into different answers.

The `Option`/absence question is the same bug in a second form: a `LocalPlan`
lookup returning `Some(plan)` cannot distinguish "this is the plan for this
declaration" from "this is some plan for some declaration that shares this
name". `idless_compatibility_local_plan_for_declared_type` returns
`Option<&LocalPlan>` where
the meaningful states are *three*: planned, not-planned, and
plan-belongs-to-a-different-binding — and the third is silently reported as the
first.

## Why it went unnoticed

- Shadowing is legal but uncommon in small fixtures, and reusing a loop
  variable name (`for (String s : a) ... for (String s : b) ...`) usually
  yields the *same* plan, so the collision is invisible.
- The five repair steps in lowering mask most shape divergences — the plan is
  overwritten from the initializer before it can be wrong on screen. That is
  why the surviving symptom is mostly `mut`, which no repair covers in the
  losing direction.
- `binding_mutable` errors are `unused_mut` (a warning) in one direction and
  only an error in the other for types without interior mutability, so a
  compile-error count under-reports it.
- `SignaturePlan::required_local_shape` (`src/signature.rs:5665`) carries the
  message *"signature solver must plan every lowered local"* — the invariant is
  written down, and the name-based `.find()` beneath it (`:5643-5652`) is what
  keeps it from ever firing when the invariant is actually violated.

## Concrete redesign

Historical status: most of this redesign has already landed for source locals.
The current small remaining design target is not "add identity to the IR"; it is
to keep shrinking or deleting the explicitly named id-less compatibility
helpers, especially callers that still cannot provide a declaration
`BindingId`.

**Give the declaration an identity in the IR and key every plan on it.**

1. `java_ir::Stmt::VarDecl`, `Stmt::EnhancedFor`, `CatchClause` and `Param`
   gain a `binding: BindingId` field, assigned once by the resolver
   (`resolve.rs`) in a single per-callable numbering pass —
   `binding_id::BindingId` already has the right shape and needs no change.
   This is the one place `occurrence` is computed.

2. Re-key the plans:

   ```rust
   pub struct MethodSignaturePlan { pub locals: HashMap<BindingId, LocalPlan>, ... }
   pub struct MethodUsageFacts    { pub bindings: HashMap<BindingId, BindingUseFacts>, ... }
   pub struct LocalPlan           { pub binding: BindingId, ... }   // name derivable from it
   ```

   `LocalPlan.name` becomes `binding.name()`; the field is deleted so the two
   cannot disagree.

3. Delete the four re-derivations. `collect_duplicate_declared_local_names`
   and the `let usage = if duplicate_names.contains(name) { None }` guards go
   away entirely — shadowed locals become ordinary distinct bindings.
   `local_constraints_for_declared_type` becomes a `HashMap` index.
   `expr_shape::idless_compatibility_local_plan_for_declared_type`,
   `consume_local_plan_occurrence` and the
   `Rc<RefCell<HashMap<(String,String),usize>>>` cursor go away, replaced by
   `plan.locals.get(&stmt.binding)`.

4. Make the third state explicit. Lowering's lookup returns
   `Option<&LocalPlan>` on a `BindingId` key, where `None` now genuinely means
   "the solver did not plan this declaration" and can be asserted or reported
   (`required_local_plan`'s existing `expect` becomes real). It can no longer
   mean "here is a different variable's plan".

5. Collapse mutability to one producer. With a `BindingId`, the liveness
   answer (`binding_is_mutated_after`) and the signature answer
   (`mutated: HashSet<String>`) are queries on the same key; keep the liveness
   one, feed it into the constraint collector, and delete
   `local_shape_constraints`'s name-set test and the `mutated_later` patch at
   `src/lower.rs:7944-7950`. Similarly, keep one
   `declared_typed_initializer_should_anchor_local_type` (the `&FactSet` form)
   and have lowering call it rather than reimplement it.

**What becomes unrepresentable.** Two distinct Java declarations can no longer
share a plan entry, so "the shape of `v`" stops being a question about a
string. The `local_plan_occurrences` cursor — a piece of hidden mutable state
whose correctness is "lowering visits declarations in the same order the solver
did" — has no expressible form once the key is the binding itself. And the
silent third state disappears: a missing plan is a `None` you must handle, not
another variable's `Some`.

**Migration order.** Step 1 + step 2 for *locals only* (leaving params and
fields on names) is the smallest change that removes mechanisms 2, 3 and 4 at
once; `LocalPlan` is constructed in exactly two places (`signature::local_plan`
at `:11695` and `conservative_lambda_local_plan` at `lower.rs:10457`) and read
through the `SignaturePlan` accessors at `signature.rs:5643-5676` plus the two
`VarDecl` arms in `lower.rs`, so the blast radius is bounded. Regression
fixtures: `F.java` (`one` and `three` must emit an identical first five
statements), `I.java` (`leak3` and `noshadow` must emit an identical first
statement), and the `0060` catch repro `K.java`, which this change turns from
"wrong variable's shape" into an honest `None`.

## Related

- `0060` (this round) — the dropped `catch`/`finally` sub-trees whose damage is
  amplified into wrong-variable typing by the name-only fallback described
  here.
- `0009` (open) — `MethodKey { type_name, rust_name }`: the same "identity
  collapsed onto a name" shape one level up, for overloads.
- `0006` (done) — move analysis was five approximations of one relation; this
  is the same pattern for the declaration↔plan relation.

## Codex parallel audit 2026-08-25

The complaint is still valid, but several rows above are now mitigation history
rather than the live failure. Current code has source-order occurrence cursors
in the signature path, binding-scoped liveness for local mutability, loop header
scope fixes, lvalue-root mutation fixes, and delegated-constructor occurrence
repairs. Those changes reduce known symptoms, and ordinary resolver-bound
declarations now carry identity. The remaining risk is the id-less compatibility
surface that still has to bridge synthetic, unresolved, or legacy name-only
paths.

The remaining name rejoin points are:

- `java_ir::Stmt::VarDecl`, `Stmt::EnhancedFor`, `Param`, and `CatchClause`
  carry optional resolver-assigned `BindingId`s, but many consumers still accept
  name-only compatibility paths.
- `signature::MethodSignaturePlan.locals` is still a vector of `LocalPlan`, but
  `LocalPlan` now carries an internally assigned `BindingId` as well as `name`.
- `usage::MethodUsageFacts` is now dual-keyed: `bindings_by_id` records
  resolver-assigned parameter/local/enhanced-for/catch bindings and local
  signature constraints consume it, but `bindings` is still keyed by `String`
  for older compatibility consumers and for synthetic/unresolved paths.
- `usage::analyze_stmt` now scopes direct block/case/loop/try/catch/finally
  locals in the compatibility name map, while keeping all declaration-id facts
  in `bindings_by_id`.
- `usage::MethodUsageFacts::all_binding_facts()` exposes a deduplicated
  all-declaration view. Aggregate consumers that do not write back by local name
  have started using it (`storage_strategy` unknown-boundary escape collection
  and the returned-borrow-safe alias check).
- `signature::SignatureDependencyNode::{MethodLocal, ConstructorLocal}` now
  carries an optional `BindingId`, and constraint sets have binding indexes for
  method/constructor locals. Binding-keyed local constraint insertion exists,
  and resolved-source mutable-argument propagation is the first write-side
  propagation pass moved to the all-declaration view. Mutable-receiver, call-
  result-to-binding, field-storage, and method-return-to-returned-binding
  propagation are also on the binding-fact insertion path. Signature propagation
  readers and dependency graph construction now consume `all_binding_facts()`.
- `usage::BindingUse::AssignedToLocal` carries the target local binding when the
  target declaration is known, so alias propagation no longer has to write a
  source binding's constraints back through a local name.
- Direct parameter consumers in signature and slot-policy now use the same
  binding-id lookup. `MethodUsageFacts::binding_facts_for_declaration_id` treats
  a known resolver id as authoritative: if the id has no entry, it returns
  `None` instead of silently falling back to the same-name compatibility map.
  `MethodUsageFacts::binding_facts_for_param` remains the explicit parameter
  helper that can use the id-less source-name compatibility path when a
  declaration has no resolver id. Regression:
  `usage::tests::binding_facts_for_param_uses_source_name_and_binding_id`.
- Lowering move/clone use counts now resolve the visible name through
  `ShapeEnv::binding_id` and query `bindings_by_id`. The helper no longer
  manufactures a compatibility local id or reads `method.bindings.get(name)` /
  `constructor.bindings.get(name)`, so same-name compatibility facts cannot make
  reused-argument clone recovery fire for the wrong declaration. Source guard:
  `callable_analysis_shell_reaches_lowering_context`.
- Lowering returned/clone-needed binding fact queries now take a `BindingId`
  directly and also read `bindings_by_id`. The remaining optional-display,
  optional-wrap, and uninitialized-optional-local decisions no longer ask the
  compatibility name map whether a same-name binding elsewhere was returned or
  moved. The same source guard rejects `.bindings.get(` and
  `bindings.contains_key(` inside this helper.
- `signature::collect_duplicate_declared_local_names` still detects duplicate
  local names, but local signature constraints now treat a known binding id as
  authoritative: `binding_usage_for_local` reads `bindings_by_id` for bound
  locals and no longer falls through to `usage.bindings.get(name)` when that id
  misses. The legacy name fallback is kept only for callers that have no binding
  id, and is still disabled for duplicate names.
- `expr_shape::idless_compatibility_local_plan_for_declared_type` remains for
  id-less synthetic compatibility paths, but lowering no longer falls through to
  it when a source declaration has a `BindingId` and the binding-keyed plan lookup
  misses, and the compatibility lookup no longer reuses a same-name local plan
  with a different erased declared type identity. Shape learning for bound
  `VarDecl` and `EnhancedFor` declarations now has the same rule: binding-keyed
  shape or conservative fallback, never same-name rejoin; id-less shape learning
  likewise requires the same erased declared type identity, with erased-carrier
  locals allowed to match through their solved concrete shape.
- `SignaturePlan` now has binding-based local plan/shape accessors, and the
  older local name-only accessors are no longer part of its public surface.
  Remaining name/type matching lives in `expr_shape`/lowering id-less
  compatibility and source-nullability compatibility views, not in normal
  `SignaturePlan` local lookup APIs.
- The `F.java` symptom described above is now pinned at generated-Rust level by
  `duplicate_inner_locals_do_not_change_outer_generated_mutability`: the outer
  `StringBuilder sb` in `three()` remains mutable before the inner duplicate
  block, and the translation must not contain an immutable
  `let sb: StringBuilder = StringBuilder::new_0();` regression.
- The `leak3`/`noshadow` shape described above is now pinned by
  `mutated_inner_list_does_not_make_outer_map_local_mutable`: an inner mutated
  `List t` cannot make the outer `HashMap<String, String> t` render as
  `let mut`.

This pass also surfaced an adjacent erased-facade return bug while tightening the
id-less cursor tests: anonymous external facade construction may be represented
as an erased boundary internally, but a source method declared to return that
facade still needs a concrete declared return slot for callers. The
`anonymous_declared_return_constraints` path now keeps those concrete
constraints instead of rejecting every erased-object representation. Regressions:
`anonymous_external_facade_return_keeps_declared_concrete_shape_for_internal_method`
and `duplicate_local_names_keep_selected_concrete_shape_before_map_insert_conversion`.

Follow-up 2026-09-09: the remaining legacy name-set local-mutability fallback in
signature planning is now gated at the helper boundary by `binding_id.is_none()`.
Resolver-bound locals no longer receive `MutableBinding` from the name set just
because a same-name declaration appears there; id-less compatibility callers
retain the explicit fallback. The stale occurrence-worded map-insert regression
was renamed to describe the current selected-binding behavior.

Follow-up 2026-09-09: the Rust-name overload-family compatibility lookup in
`CallableAnalysis` now also fails closed on ambiguity. The previous fact lookup
returned the first matching `methods_by_rust_name` entry by hash iteration, while
the use-count lookup took the maximum across matching overloads. Both now share
one unique-or-none helper, so a repeated `Param value#0` across overloads cannot
rejoin the current lowering context to an arbitrary family member. Regression:
`callable_analysis::tests::rust_method_family_binding_lookup_refuses_ambiguous_overloads`.

Follow-up 2026-09-09: signature local binding usage no longer hides the
bound-vs-idless compatibility decision behind `binding_usage_for_local(...,
allow_name_fallback: bool)`. Parameter call sites route through
`binding_usage_for_param`, resolver-bound locals route through
`binding_usage_for_bound_local`, and id-less compatibility callers route through
`idless_compatibility_binding_usage_for_local`. The behavior is unchanged, but
the remaining name fallback is now named at the helper boundary and guarded by
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.
The same pass moved slot-policy's two borrowed-content parameter lookups behind
its own `binding_usage_for_param` helper, so that module also has one named
id-less compatibility boundary instead of two open-coded fallback calls.
Usage receiver type recovery now similarly names its expression-name fallback as
`idless_compatibility_binding_java_type_for_name`, keeping the visible-binding
fast path separate from the id-less compatibility lookup guarded by
`usage_receiver_type_fallback_does_not_scan_all_binding_history_by_name`.

The method field-storage alias-chain residual is fixed:
`signature_fixpoint_propagates_method_field_storage_constraints` now carries the
storage requirement back through `second -> first -> input`. The repair had two
parts: field-storage writes use binding facts, and storage-slot requirements now
override the borrowed-slice parameter default instead of being rejected by it.

Follow-up 2026-08-25: alias analysis no longer asks the legacy
`MethodUsageFacts.bindings` name map for a receiver handle's Java type.
`receiver_java_type_for_handle` now consumes `all_binding_facts()` and returns a
type only when every same-name binding fact agrees. Same-name declarations with
different Java types therefore fail closed instead of applying an external
receiver mutability fact to whichever scoped binding happened to occupy the
compatibility map. Regression:
`aliasing::tests::receiver_java_type_for_handle_rejects_same_name_type_disagreement`.
Source guard:
`layering::aliasing_uses_binding_fact_view_instead_of_legacy_name_map`.

Follow-up 2026-08-25: object-model dispatch requirements no longer iterate only
the legacy `MethodUsageFacts.bindings` name map. `dispatch_requirements_from_usage`
now uses `all_binding_facts()`, so a same-name binding preserved only in
`bindings_by_id` can still request owned polymorphic storage, borrowed trait
view, or `instanceof` support. Regression:
`object_model::tests::dispatch_requirements_include_shadowed_binding_id_facts`.

Follow-up 2026-08-25: the id-less synthetic local-plan lookup no longer reuses
the first same-name erased-object plan forever.
`expr_shape::idless_compatibility_local_plan_for_declared_type` now consumes the
`(name, declared-type)` occurrence before selecting an erased-boundary match, so
repeated compatibility lookups advance to the next matching plan and overrun
fails closed. Regression:
`expr_shape::tests::erased_object_local_plan_lookup_consumes_occurrences`.

Follow-up 2026-08-25: signature local constraint lookup by bare name now fails
closed when more than one local has that name. `index_local_constraints` no
longer indexes ambiguous names, and `MethodConstraintSet::local_index` plus
`ConstructorConstraintSet::local_index` reject duplicate-name vector scans even
if a stale/manual compatibility map exists. Binding-keyed lookup still reaches
the exact local. Regressions:
`signature::tests::ambiguous_same_name_method_local_lookup_fails_closed_but_binding_lookup_still_works`
and
`signature::tests::ambiguous_same_name_constructor_local_lookup_fails_closed_but_binding_lookup_still_works`.

Follow-up 2026-08-25: source nullability inference no longer lets one
same-name local declaration feed another through the compatibility map.
`collect_binding_nullability_state_map` removes duplicate declared local names
after collecting the legacy name-keyed states, so local and return nullability
queries fail closed until this path is fully binding-keyed. Regressions:
`source_binding_nullability_constraints_fail_closed_for_duplicate_local_names`
and `source_return_nullability_does_not_reuse_shadowed_local_name_state`.

Follow-up 2026-08-25: the local side of that source-nullability path is now
binding-keyed for resolver-bound declarations. The collector keeps the old
fail-closed name view for compatibility callers, but also records nullability
states by `BindingId` under lexical scopes. Signature local constraint
collection asks that binding view for source `VarDecl`/enhanced-for declarations
with ids, while id-less synthetic declarations keep the documented name fallback.
The duplicate-name regression now also checks the solved local shapes: an inner
`Object value = null` becomes optional, while the later outer
`Object value = new Object()` keeps its own non-optional shape. Return
nullability uses a scoped binding lookup too, with a separate return-only
binding state for construction expressions; the same shadowed fixture now proves
the returned outer binding as definitely non-null without pushing that
constructor fact into local slot constraints.

Follow-up 2026-08-25, updated 2026-09-10: method and constructor parameter
nullability now use the same binding-keyed source-nullability map. The
crate-local `source_binding_nullability_constraints(block, name)` helper
remains the explicit compatibility/name view and still fails closed when
duplicate local names make the name ambiguous, but parameter shaping calls the
declaration helper with the parameter's resolver `BindingId` and the callable's
parameter scope. Regression:
`source_param_nullability_uses_binding_when_duplicate_locals_hide_name_view`
proves both method and constructor parameters keep their nullable constraint even
when duplicate same-name locals hide the compatibility name view.

Follow-up 2026-08-25: usage receiver-type inference for call boundaries no
longer trusts whichever same-name binding happens to occupy the legacy
compatibility map. `call_receiver_java_type` and `call_result_receiver_type`
now read the deduplicated all-binding view and return a Java type only when all
same-name binding facts agree. Static-owner receiver fallback remains for
id-less owner calls. Regressions:
`usage::tests::call_receiver_type_fails_closed_for_same_name_type_disagreement`
and `usage::tests::call_receiver_type_uses_legacy_name_when_unambiguous`.

Follow-up 2026-08-25: facade ABI mut-borrow receiver matching no longer treats
the mere presence of a local receiver name as matching every ABI owner with the
same method/arity. `facade_abi_matches_call_receiver` now compares the
deduplicated local receiver Java type to the ABI owner through the canonical
external receiver key; only id-less static-owner fallbacks still compare the
receiver spelling directly. Regressions:
`usage::tests::facade_abi_receiver_match_rejects_wrong_local_receiver_type` and
`usage::tests::facade_abi_receiver_match_accepts_matching_local_receiver_type`.

Follow-up 2026-08-25: the proposed `bindings_by_id` check for
`is_unshadowed_field_name` was audited and rejected for this predicate:
`bindings_by_id` is whole-method history, while field shadowing needs the
current lexical-scope map. A block-scoped local named like a field must not keep
shadowing that field after the block ends. Regression:
`usage::tests::block_scoped_local_does_not_shadow_field_after_scope`.

Follow-up 2026-08-25: the remaining lowering-side
`idless_compatibility_local_plan_for_declared_type` call is now fenced behind an explicitly
named `idless_compatibility_local_plan_for_declaration` helper. Bound source
declarations still return only the binding-keyed plan lookup result and do not
fall through to the compatibility cursor when that lookup misses. Source guard:
`slot_shape_design::callable_analysis_shell_reaches_lowering_context`.

Follow-up 2026-08-25: source null-assertion statement deletion no longer
reinterprets a local or field receiver as a source static owner. The unresolved
source-method lookup used by `source_null_assertion_call_expr_stmt_is_noop`
now passes through `ShapeEnv` and rejects `Expr::Var` owners that are visible
locals or fields before consulting context source types. Regression:
`lower::tests::source_call_method_lookup_rejects_static_owner_shadowed_by_local`.

The next implementation step is finishing the remaining compatibility surface:
narrowly fence any remaining id-less synthetic fallback surface and production
name-map lookup that is not explicitly id-less compatibility.

Explorer follow-up 2026-08-25: the two concrete compatibility risks identified
by the latest explorer pass are now fixed.

- Fixed: id-less initialized declarations no longer select a local plan by
  occurrence cursor. `expr_shape::idless_compatibility_local_plan_for_declared_type` now accepts a
  unique declared-type match, but same-name/same-type ambiguity fails closed,
  including erased-object boundary matches. The lowering helper therefore no
  longer depends on resolver-missed lowering and signature planning traversing
  declarations identically. Regressions:
  `expr_shape::tests::unique_idless_local_plan_match_is_still_accepted`,
  `expr_shape::tests::ambiguous_same_name_same_type_idless_local_plan_fails_closed`,
  `expr_shape::tests::ambiguous_erased_object_idless_local_plan_lookup_fails_closed`,
  and
  `lower::tests::id_less_ambiguous_same_type_declaration_does_not_take_local_plan_by_cursor`.
- Fixed: assignment propagation no longer reads every same-name local constraint
  when `AssignedToLocal { local_binding: None }` appears. Bound aliases still
  propagate through `local_slot_requirements_for_assignment`, but id-less
  assignment aliases now fail closed for this local-slot constraint transfer.
  Regression:
  `signature::tests::idless_assignment_local_requirements_do_not_merge_same_name_locals`.

Focused guard audit 2026-08-25:

- `same_name_same_type_locals_use_their_own_signature_constraints` already
  proves same-name/same-type locals keep distinct binding ids and distinct
  signature constraints; the later initialized binding keeps its storage
  constraint, while the earlier uninitialized shadow does not inherit it.
- `same_name_disjoint_locals_do_not_share_mutability` already proves a mutated
  inner `List t` does not make a later `Map t` mutable.
- `duplicate_inner_locals_do_not_change_outer_generated_mutability` pins the
  generated-Rust outer `StringBuilder sb` mutability case.
- `mutated_inner_list_does_not_make_outer_map_local_mutable` pins the
  generated-Rust outer-map case.
- Added source guard
  `layering::idless_local_binding_compatibility_surface_stays_explicit`: the
  only production `idless_compatibility_local_plan_for_declared_type` call must
  stay behind the named `idless_compatibility_local_plan_for_declaration` helper, and
  `collect_duplicate_declared_local_names(block)` must remain limited to the
  one source-nullability fail-closed call site plus its guard.

Follow-up 2026-08-25: the duplicate-name guard was removed from local signature
constraint collection. That path always supplies a declaration binding id now,
and `binding_usage_for_local` treats an id as authoritative, so the
`duplicate_names.contains(...)` gate was obsolete and made the binding-keyed
path look less complete than it is. `collect_duplicate_declared_local_names`
remains only for the legacy name-keyed source-nullability view, where duplicate
names still deliberately fail closed until the compatibility view is deleted.
The layering guard now requires exactly that narrower surface.

Follow-up 2026-08-25: the borrowed-return parameter policy no longer lets the
legacy name-only assignment scan veto resolver-bound parameter facts. The policy
already reads the exact parameter through `bindings_by_id`; plain assignment to
that exact parameter is rejected by the binding's `Mutated` fact, while the old
`legacy_idless_method_assigns_to_binding(method, name)` scan remains only for id-less
compatibility inputs. Regression:
`slot_policy::tests::borrowed_return_param_policy_uses_binding_facts_not_name_assignment_scan`.

Follow-up 2026-08-25: internal returned-object alias constraints now make the
same distinction. The shared-borrow return constraint is selected by iterating
declared parameters and resolving each through `binding_facts_for_param`,
instead of scanning `all_binding_facts()` for any same-name binding and then
vetoing with `legacy_idless_method_assigns_to_binding(method, name)`.
Resolver-bound params
therefore cannot be rejected by a same-name shadow local; mutation of the exact
parameter binding still rejects through `BindingUse::Mutated`. Regression:
`signature::tests::returned_object_alias_constraints_use_param_binding_not_shadow_name`.

Follow-up 2026-08-26: a stale `slot_shape_design` assertion was corrected to
match the narrowed source-nullability compatibility surface. Resolver-bound local
declarations are removed from the crate-local name-only nullability view and
must be shaped through binding-keyed signature collection; the no-params
crate-local helper still exposes method parameters by name because it has no
parameter identity input. Regression:
`slot_shape_design::source_nullability_constraints_shape_locals_params_and_returns`.

Follow-up 2026-08-26: display-sink hint collection no longer walks typed lambda
bodies with only the outer binding-name context. Typed lambda parameters now
enter the hint context before the lambda body is scanned, so a lambda parameter
named like an outer field does not synthesize external display capability for
the wrong owner. This is still a local type context, not the full binding-id
model, but it closes a concrete same-name rejoin in the display hint path.
Regression:
`stdlib::display_hint_tests::typed_lambda_param_shadows_outer_binding_for_display_hint_owner`.

Follow-up 2026-08-26: call receiver type recovery now treats the visible
name-map binding as authoritative when that entry points at a live
`bindings_by_id` fact. A same-name, out-of-scope binding with a different Java
type no longer makes `call_receiver_java_type` or `call_result_receiver_type`
fail closed for the current receiver. Stale visible entries whose binding id is
missing from `bindings_by_id` still fail closed. Regressions:
`usage::tests::call_receiver_type_uses_current_visible_binding_before_same_name_history`
and
`usage::tests::call_receiver_type_rejects_stale_visible_name_binding`.

Follow-up 2026-08-26: source return-nullability collection now scopes
`for`-initializer bindings before walking returns in the loop body. The
binding-state collector already saw those initializers, but the return-state
walker grouped `for` with `while` and analyzed the body under an empty loop
scope. A return of the initializer local now resolves through its binding id
instead of falling back to `Unknown`. Regression:
`signature::tests::source_return_nullability_scopes_for_initializer_binding`.

Follow-up 2026-08-26: source return-nullability field lookup no longer uses a
whole-method shadowed-name set. It checks active lexical binding scopes, so a
block-local declaration named like a field stops shadowing that field when the
block exits. Regression:
`signature::tests::source_return_nullability_field_shadow_expires_after_block`.

Follow-up 2026-08-26: display-sink hint collection now inserts catch parameters
into the catch-body type context. The display hint walker already scoped method
params, locals, enhanced-for variables, and typed lambda params; catch bodies
were the remaining cheap omission in this compatibility-only name map. This is
not a full binding-id migration, but it prevents an erased-display hint from
missing the caught variable owner entirely. Regression:
`stdlib::display_hint_tests::catch_param_scopes_display_hint_owner`.

Follow-up 2026-08-26: usage receiver-type recovery now treats a live visible
binding id as authoritative for the Java type as well as for staleness. The
compatibility name-map entry can point at the current binding id but still carry
drifted Java type text; `agreed_binding_java_type` now reads
`bindings_by_id[binding_id].java_ty` first. Regression:
`usage::tests::call_receiver_type_reads_authoritative_binding_id_fact`.

Follow-up 2026-09-06: the remaining receiver-type helper no longer repeats the
visible-name-to-authoritative-binding lookup inline. `agreed_binding_java_type`
now asks `MethodUsageFacts::visible_binding_java_type_for_name` first, then
keeps the explicit id-less declaration fallback through
`idless_compatibility_binding_facts_for_source_name`. Focused coverage:
`usage::tests::call_receiver_type_*` and
`layering::usage_receiver_type_fallback_does_not_scan_all_binding_history_by_name`.

Follow-up 2026-09-06: the stale
`ShapeEnv::next_local_plan_for_declared_type` name is gone. The helper no longer
advanced a cursor; it now exposes its real boundary as
`ShapeEnv::idless_compatibility_local_plan_for_declared_type`, with the single
lowering caller still fenced behind
`idless_compatibility_local_plan_for_declaration`. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up 2026-08-26: aggregate binding readers now deduplicate by the
`bindings_by_id` map key, not by the optional embedded `BindingUseFacts.binding`
field. If the authoritative id table contains a fact whose embedded binding id
is absent or drifted, the compatibility name-map entry for that same key no
longer leaks through as a second fact. Regression:
`usage::tests::all_binding_facts_deduplicates_by_binding_id_key`.

Follow-up 2026-08-26: the id-less declaration compatibility fallback now uses
the same stale-entry rule as the other name-map readers. A legacy name-only fact
is still accepted, but a name-map entry that points at a binding id absent from
`bindings_by_id` fails closed instead of returning stale declaration facts.
Regression:
`usage::tests::visible_binding_id_for_name_rejects_stale_name_map_binding`.

Follow-up 2026-08-26: scoped binding restoration now follows the same rule.
Restoring a saved name-map fact can no longer recreate a missing binding id in
`bindings_by_id`; id-less legacy compatibility facts still restore. Regressions:
`usage::tests::restore_binding_use_facts_rejects_stale_saved_binding_id` and
`usage::tests::restore_binding_use_facts_keeps_legacy_idless_binding`.

Follow-up 2026-08-26: the signature-side regression for bound local usage now
matches the stale-name policy above. An id-less caller no longer accepts a
compatibility name-map fact whose embedded binding id is absent from
`bindings_by_id`; only a genuinely id-less legacy fact keeps the fallback.
Regression:
`signature::tests::binding_usage_for_bound_local_does_not_fall_back_by_name`.

Remaining scope: this audit is no longer blocked on a simple known same-name
local bug. The broader cleanup is replacing the id-less compatibility surface
with binding-keyed callers end to end; until then, the compatibility helpers
are intentionally fenced and regression-tested rather than deleted.

Follow-up 2026-08-26: exact erased-object cast lowering no longer asks
`current_local_shapes` for a local by bare name when deciding whether the
erased carrier is optional. That fallback could borrow optionality from a stale
same-name declaration plan when the scoped `ShapeEnv` had no visible local
shape. The helper now fails closed from the scoped environment only. Regression:
`lower::tests::exact_erased_object_cast_optional_carrier_does_not_use_stale_name_plan`.

Follow-up 2026-08-27: usage scope shadowing no longer open-codes the legacy
visible-name compatibility map. Enhanced-for variables, catch parameters,
lambda parameters, and direct scoped local declarations now snapshot/hide/restore
through `MethodUsageFacts::{snapshot,hide,restore}_visible_binding_for_scope`.
Those helpers reuse the same stale-entry policy as the other name-map readers:
a visible entry whose `BindingId` is missing from `bindings_by_id` is dropped
instead of being saved and later resurrected. Regressions:
`usage::tests::visible_binding_scope_helpers_rejects_stale_saved_binding_id`
and `usage::tests::usage_scope_shadowing_goes_through_visible_binding_helpers`.

Follow-up 2026-08-27: `record_use` is now a thin wrapper over
`MethodUsageFacts::record_visible_binding_use`. The compatibility policy for
updating both the visible name-map fact and the authoritative binding-id fact
now lives with the rest of `MethodUsageFacts` instead of being open-coded by the
free helper. Regression coverage is folded into
`usage::tests::usage_name_binding_id_recovery_goes_through_visible_binding_helper`.

Follow-up 2026-09-06: the scope/name-map maintenance helpers
`snapshot_visible_binding_for_scope`, `hide_visible_binding_for_scope`,
`restore_visible_binding_for_scope`, and `record_visible_binding_use` are
crate-local. They remain the centralized implementation for scoped
compatibility-name maintenance, but are no longer public crate API.

Follow-up 2026-08-26: id-less declaration compatibility matches no longer fork
binding identity when the selected `LocalPlan` is inserted into `ShapeEnv`.
Resolver-provided declaration bindings still win, but an id-less local,
enhanced-for variable, ordinary lowered local, or delegated-constructor trailing
local that uniquely matched a compatibility plan now preserves that plan's
`LocalPlan.binding` instead of synthesizing a fresh same-name binding. This keeps
the compatibility path aligned with the binding-keyed facts it selected, without
claiming the broader id-less fallback surface is gone. Regressions:
`expr_shape::tests::idless_var_decl_shape_learning_preserves_selected_compatibility_binding`,
`expr_shape::tests::idless_enhanced_for_shape_learning_preserves_selected_compatibility_binding`,
`lower::tests::idless_local_env_insert_preserves_selected_compatibility_plan_binding`,
and
`lower::tests::delegated_constructor_trailing_idless_vardecl_preserves_selected_plan_binding`.

Follow-up 2026-08-26: usage receiver-type recovery no longer falls back to
scanning every historical binding fact by visible name when the scoped/current
binding checks fail. The remaining accepted compatibility path is the existing
id-less name-map entry, fenced by the same stale-entry rule as the declaration
helpers. This closes the last narrow same-name-history branch reported in this
file without claiming the full id-less compatibility surface is gone.
Regressions:
`usage::tests::call_receiver_type_rejects_same_name_history_without_visible_binding_even_when_types_agree`,
`usage::tests::facade_abi_receiver_match_rejects_same_name_history_without_visible_binding`,
and
`layering::usage_receiver_type_fallback_does_not_scan_all_binding_history_by_name`.

Follow-up 2026-08-26: id-less declaration name fallback now also fails closed
when `bindings_by_id` contains any current fact for the same visible source
name. A genuinely legacy id-less name-only fact remains usable, and a name-map
entry that points at a live binding id still resolves through that id, but
`binding: None` no longer claims to identify `value` once resolver-bound
`value` facts exist. This prevents `call_receiver_java_type` and declaration
lookup callers from accepting an ambiguous stale id-less name-map entry.
Regression:
`usage::tests::id_less_name_fallback_rejects_same_name_bound_history`.

Follow-up 2026-08-26: the same ambiguous id-less rule now gates the other
compatibility readers too. `name_binding_is_current` no longer treats
`binding: None` as current when id-backed facts for the same name exist, so
`record_use` will not append uses to that name-map entry and field-shadow
analysis will not let it hide a same-name field. `all_binding_facts` also skips
the ambiguous id-less fact instead of reporting it beside the authoritative
id-backed facts. Regression:
`usage::tests::ambiguous_id_less_name_map_entry_is_not_current_or_aggregated`.

Follow-up 2026-08-28, updated 2026-09-10: the block/name signature helper is
now explicitly local-only and crate-local. `source_binding_nullability_constraints(block, name)`
fails closed unless the block declares that local name itself, so resolver-bound
parameters can no longer be accidentally queried through the compatibility
surface with `params = []`. The declaration-aware helper is crate-local for
callers that have the actual `Binding`; focused regressions cover parameter
nullability through
`source_binding_nullability_constraints_for_declaration` and the old helper's
empty result for resolver-bound parameters. This is a fence, not a deletion of
the remaining id-less compatibility surface.

Follow-up 2026-08-28: the local-only fence is now direct-block-only, not
recursive. A nested `Object value = null` inside an `if`/loop/try body no longer
enables `source_binding_nullability_constraints(outer_body, "value")` for the
outer block/name query. The duplicate-name scanner remains recursive where it is
used for whole-block local-plan disambiguation, but the crate-local compatibility
helper now uses a narrower direct declaration predicate. Regression:
`slot_shape_design::source_binding_nullability_constraints_do_not_use_nested_only_local_name`.

Follow-up 2026-09-10 cleanup: the direct block/name helper is no longer public;
it is crate-local and its exact compatibility-scope assertions live in
`signature.rs` unit tests. The method return nullability helper is crate-local,
and integration tests now assert through solved signature/shape behavior instead
of calling either compatibility helper directly.

Follow-up 2026-09-10 cleanup: `stdlib::source_summary_method_return_nullability`
is crate-local too. Source-summary and lowering paths can still share the
fallback return-nullability rule, but external callers cannot bypass the
signature/nullability planners through that helper.

Follow-up 2026-08-28: facade ABI receiver matching now routes its id-less
fallback through the same static-owner pseudo-receiver policy as receiver-type
recovery. A missing binding can still match an explicit static owner such as
`StringUtils`, but an arbitrary local-looking spelling such as `string_utils`
is no longer promoted to the ABI owner by a broad canonical name comparison.
This fences one compatibility edge without deleting the remaining static-owner
escape hatch. Regression:
`usage::tests::facade_abi_receiver_match_restricts_idless_fallback_to_static_owners`.

Follow-up 2026-08-28: the crate-local block/name nullability compatibility
helper no longer treats enhanced-for variables as direct declarations of the
enclosing block. The loop variable remains scoped to the enhanced-for body, so
`source_binding_nullability_constraints(outer_body, "item")` cannot borrow
nullability state from `for (Object item : values) { item = null; }`. Direct
id-less locals in the same block remain available through the compatibility
path. Regression:
`slot_shape_design::source_binding_nullability_constraints_do_not_expose_enhanced_for_variable`.

Follow-up 2026-08-28: imported static-owner support discovery no longer uses a
whole-method shadow set. The collector now walks statements in lexical order:
local initializers are scanned before their declared name is added, nested block
shadows do not leak outward, enhanced-for/catch names are scoped to their
bodies, and `for` initializers scope only to that `for` statement. This prevents
a later `Object StaticUtility = ...` from suppressing an earlier
`StaticUtility.readValue(...)` owner-support requirement. Regressions:
`stdlib::external_facade_support_tests::imported_static_owner_call_before_shadowing_local_is_collected`,
`stdlib::external_facade_support_tests::imported_static_owner_shadow_does_not_escape_nested_block`,
and the existing imported-static collector/source guards.

Follow-up 2026-08-28: compatibility name-map entries that point at a live
`BindingId` now also have to agree on the queried source name. Previously a
stale/manual `bindings["value"]` entry could point at an existing
`BindingId(Local, "other", ...)` and still be treated as current because the id
existed. `visible_binding_id_for_name`,
`idless_compatibility_binding_facts_for_source_name`, and
`binding_use_facts_are_current` now require `binding_id.name() == name`, so the
remaining name-map compatibility path cannot rejoin one visible name to an
unrelated live binding id. Regression:
`usage::tests::name_map_binding_id_must_match_the_queried_name`.

Follow-up 2026-08-28: source return-nullability scoping now binds enhanced-for
loop variables before walking the loop body. The return walker previously opened
a child scope for `for (T item : values)` but grouped it with `while`/`do`
loops, so `return item` inside the body could miss the resolver `BindingId` and
fall back to name-based or field-based evidence. The enhanced-for arm now
inserts `var_name -> BindingId` into that child scope, matching the binding
nullability collector. Regression:
`signature::tests::source_return_nullability_scopes_enhanced_for_binding`.

Follow-up 2026-08-28: `ShapeEnv` access-name aliases for resolver-bound locals,
parameters, and enhanced-for variables now preserve the same `BindingId` as the
source declaration name. The previous bound insertors stored the id under
`name` but left a renamed emitted spelling such as `self__`, `r#type`, or
`crate__` id-less, reopening the compatibility surface after keyword escaping
or other access-name rewrites. Regressions:
`expr_shape::tests::bound_local_env_insert_preserves_resolver_binding_id_for_access_name`,
`expr_shape::tests::bound_param_env_insert_preserves_resolver_binding_id_for_access_name`,
and
`expr_shape::tests::bound_loop_variable_env_insert_preserves_resolver_binding_id_for_access_name`.

Follow-up 2026-08-28: catch parameters now carry the same source/emitted-name
split as params, locals, and enhanced-for variables. `CatchClause` records an
unescaped `source_name` beside the rendered Rust `name`; semantic, usage,
resolve, liveness, signature, stdlib-support, and benchmark scopes use the
source identity, while lowering registers a catch-param access-name alias for
the emitted binding. A Rust-keyword catch variable such as
`catch (RuntimeException crate)` therefore resolves body reads through `crate`
without rejoining by name, while the generated Rust pattern still uses
`crate__`. Regressions:
`parser_tree_sitter::catch_params_preserve_unescaped_source_name` and
`expr_shape::tests::bound_catch_param_env_insert_preserves_resolver_binding_id_for_access_name`.

Follow-up 2026-08-28: call-site carrier refinement no longer lets a bound
local's missing exact constraint fall back to a same-name local constraint when
computing the actual Java type. `effective_method_binding_java_type` now uses
`local_by_binding` for resolver-bound facts and reserves `local(name)` for
id-less compatibility facts. A stale `BindingId(Local, "value", 0)` constraint
therefore cannot override usage for `BindingId(Local, "value", 1)`.
Regression:
`signature::tests::effective_method_binding_java_type_for_bound_local_does_not_fall_back_by_name`.

Follow-up 2026-08-29: the reusable `SignaturePlan` local lookup surface is now
binding-keyed. The public name-only helpers for local shape, required local
shape/plan, and binding mutability were removed; tests that need by-name
inspection now use test-local unique-name helpers instead of production APIs.
Duplicate same-name locals remain individually queryable through exact
`BindingId`s. Regressions:
`signature::tests::duplicate_local_name_signature_plan_lookup_uses_binding_id`
and `layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up 2026-08-29: `ShapeEnv`'s collection-view side facts are now bound to
local identity when resolver `BindingId`s are available. The `keySet()`,
`values()`, `entrySet()`, mutable entry-value receiver, and `SimpleEntry`
constructor-value tables still keep a name-keyed compatibility path for id-less
synthetic inputs, but bound lookups no longer fall back from a current
same-name declaration to an earlier view fact. Regression:
`expr_shape::tests::bound_map_view_facts_do_not_fall_back_to_same_name_local`.

Follow-up 2026-08-29: direct production access to the legacy
`MethodUsageFacts.bindings` name map is now fenced to `usage.rs`, the module
that owns the scoped compatibility policy. Main consumers (`aliasing`,
`lower`, `object_model`, `signature`, and `slot_policy`) must use
`MethodUsageFacts` accessors, `bindings_by_id`, or aggregate binding views
instead of reaching into `.bindings` directly. Regression:
`layering::legacy_usage_binding_name_map_is_owned_by_usage_module`.

Follow-up 2026-08-29: aliasing's unresolved handle receiver type lookup now
asks `MethodUsageFacts::receiver_handle_java_type` for the current visible
binding instead of scanning whole-method same-name binding history with
`all_binding_facts()`. The generic `visible_binding_java_type_for_name` helper
is private to `usage.rs`, so cross-module callers must ask the semantic receiver
question instead of treating the compatibility name map as a general type
environment. A hidden earlier `String handle` can no longer make the current
`Record handle` look ambiguous and suppress receiver-specific external mutation
facts. Regression:
`aliasing::tests::receiver_java_type_for_handle_uses_visible_binding_over_same_name_history`.

Follow-up 2026-08-29: self-mutability propagation now uses exact
`JavaCallableKey` identity for resolved self-calls instead of rejoining every
resolved call to a `(declaring_type, name)` pair. The compatibility name path is
still present for unresolved lambda-body calls, but a read-only overload no
longer inherits `&mut self` pressure just because another overload with the same
Java name mutates the receiver. Regression:
`semantic_facts::usage_self_mutability_propagation_uses_resolved_overload_identity`.

Follow-up 2026-08-29: pre-lowering map-view shape learning now records
`keySet()`, `values()`, and `entrySet()` receiver facts under the declaration's
registered access-name alias as well as the source/name spellings. A Java local
whose source spelling is rewritten for Rust access can therefore still find the
same live-view side fact later by emitted binding name. Regression:
`expr_shape::tests::map_view_shape_learning_preserves_access_name_alias`.

Follow-up 2026-08-29: pre-lowering `Map.Entry` shape learning now records the
same binding-aware side facts as lowering for entries produced by
`map.entrySet().iterator().next()` or by `new AbstractMap.SimpleEntry(k, v)`.
The entry-value receiver and simple-entry key/value tables are stored under the
resolver binding when available, and under the source/access-name spellings for
compatibility. Regression:
`expr_shape::tests::map_entry_shape_learning_preserves_access_name_alias`.

Follow-up 2026-08-29, current names refreshed 2026-09-10: the broad
`MethodUsageFacts::binding_for_declaration` compatibility wrapper is gone.
Signature and slot-policy parameter paths now call
`MethodUsageFacts::binding_facts_for_param`, which keeps resolver-bound
parameters exact and reserves the remaining source-name fallback for id-less
compatibility parameters. Regression:
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

Follow-up 2026-08-29: id-less compatibility lookups that find a live
`BindingId` in the legacy name map now return the authoritative
`bindings_by_id` fact instead of the name-map payload. The name map can still
admit genuine id-less facts behind the existing ambiguity gate, but a stale
name entry can no longer carry a different Java type or use list after it has
validated against a live binding id. Regression:
`usage::tests::binding_name_fallback_returns_authoritative_live_id_fact`.

Follow-up 2026-08-29: the signature heuristic for "returns a local initialized
from a call" no longer stores a flat set of call-derived local names. It now
tracks a scoped visible alias map, so an inner block declaration named like an
outer local cannot leak a call-result fact into the outer return, and assigning a
non-call value back into the visible local clears the alias. This leaves the
remaining name-only compatibility surface explicit while removing one more
shadowing-sensitive silent signature input. Regressions:
`signature::tests::call_result_alias_for_visible_local_still_detected`,
`signature::tests::call_result_alias_from_inner_shadow_does_not_leak_by_name`,
and
`signature::tests::call_result_alias_is_cleared_by_visible_reassignment`.

Follow-up 2026-08-29: the returned-local source-call nullability helper no
longer keeps a stale call-derived state after a visible declaration or
assignment changes the local to a non-source-call value. Declarations and
assignments now replace the current visible state, or clear it when the new
value is not a same-type source call, so `value = "literal"; return value;`
cannot inherit the nullability of an earlier `value = helper()` by name.
Regression:
`signature::tests::returned_local_initializer_call_nullability_clears_visible_reassignment`.

Follow-up 2026-08-29: borrowed-return parameter content policy no longer lets a
resolver-bound parameter satisfy the return precondition through
`method_exactly_returns_param`, which only scans returned names. Bound
parameters must now carry an exact `BindingUse::Returned`; the name-only return
scan is reserved for id-less compatibility. Regression:
`slot_policy::tests::borrowed_return_param_content_requires_exact_returned_binding_when_param_bound`.

Follow-up 2026-08-29: local shape mutability collection no longer falls back to
the whole-method mutated-name set when a resolver-bound declaration lacks exact
liveness evidence for the statement. The legacy name set remains available only
for id-less compatibility declarations; bound locals require the liveness answer
for their own declaration/binding path. Regression:
`signature::tests::bound_local_mutability_does_not_fall_back_to_name_set_when_liveness_is_missing`.

Follow-up 2026-08-30: visible-use recording no longer writes the same
`BindingUse` through both the compatibility name map and the authoritative
`bindings_by_id` entry when a live resolver `BindingId` is visible. Bound
bindings now collect each use once in `bindings_by_id`; only id-less
compatibility entries collect uses in the name map. Regressions:
`usage::tests::record_use_records_live_binding_id_once` and the full
`usage::tests` module.

Follow-up 2026-08-30: bound local shape learning now preserves Rust keyword
access-name aliases for ordinary declarations, not only for later map-view side
facts. `Stmt::VarDecl { source_name: "self", name: "self__", binding: ... }`
is inserted through the same access-name-aware `ShapeEnv` path used by main
lowering, so expressions parsed under the source spelling can still find the
resolver binding and solved shape. Delegated-constructor trailing statement
shape learning uses the same policy. Regressions:
`expr_shape::tests::bound_var_decl_shape_learning_preserves_keyword_access_name_binding`
and
`lower::tests::delegated_constructor_trailing_vardecl_shape_preserves_keyword_access_name_binding`.

Follow-up 2026-08-30: usage scopes now insert resolver-bound params, local
declarations, enhanced-for locals, and scoped direct declarations under their
Java `source_name` rather than the escaped emitted Rust name. A bound
declaration such as `source_name: "self", name: "self__"` therefore remains
visible to later `Expr::Var("self")` use recording, while the emitted name still
appears in the `BindingUse::AssignedToLocal` payload. Regression:
`usage::tests::keyword_renamed_bound_local_records_uses_by_source_binding`.

Follow-up 2026-08-30: the remaining crate-local block/name source-nullability
compatibility collector no longer crosses nested callable boundaries. Local type
member bodies, anonymous-class method bodies, and lambda bodies can contain
parameters or locals with the same source name as an outer local; those nested
facts no longer leak into `source_binding_nullability_constraints(outer,
"value")`. Regressions:
`slot_shape_design::source_binding_nullability_constraints_do_not_cross_anonymous_method_body`
and
`slot_shape_design::source_binding_nullability_constraints_do_not_cross_lambda_body`.

Follow-up 2026-08-30: declaration-aware source-nullability lookup now has one
`SourceNullabilityMap::constraints_for_declaration` binding/name compatibility
join. The crate-local declaration helper plus local and enhanced-for constraint
collection all route through it instead of hand-copying the same
`binding.map(...).unwrap_or_else(name)` fallback. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`; behavior
covered by the `source_binding_nullability_constraints` slot-shape filter.

Follow-up 2026-08-30: source-return nullability scopes now insert resolver-bound
parameters, local declarations, and enhanced-for variables under their Java
`source_name`, matching the spelling used by `Expr::Var` in source IR. A
keyword-renamed binding such as `source_name: "self", name: "self__"` can now
collect assignment/nullability facts and be returned through its exact
`BindingId`. The same pass made `SourceNullabilityMap::state_for_var` fail
closed when a scoped binding is known but has no binding-specific state, so a
same-name field or legacy by-name entry cannot fill in for an in-scope local.
Regressions:
`signature::tests::source_return_nullability_param_scope_uses_source_name`,
`signature::tests::source_return_nullability_local_scope_uses_source_name`,
`signature::tests::source_return_nullability_enhanced_for_scope_uses_source_name`,
and
`signature::tests::source_return_nullability_in_scope_binding_does_not_fall_back_to_field`.

Follow-up 2026-08-30: the remaining name-only assignment scan used by
borrowed-return content policy is now named and scoped as
`legacy_idless_method_assigns_to_binding`. The two production callers already
sit behind `param.binding.is_none()` compatibility gates; the rename plus
layering guard prevent it from looking like a general binding-identity API.
Follow-up 2026-09-07 strengthened that guard to assert the id-less predicate at
both call sites, not just the helper name/reference count.
Regressions:
`slot_policy::tests::borrowed_return_param_policy_uses_binding_facts_not_name_assignment_scan`,
`slot_policy::tests::borrowed_return_param_content_requires_exact_returned_binding_when_param_bound`,
`signature::tests::returned_object_alias_constraints_use_param_binding_not_shadow_name`,
and `layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

Follow-up 2026-08-30: that same id-less assignment scan now sees expression-side
mutations instead of only `Stmt::Assign { target: Expr::Var(..) }`. The retained
compatibility path rejects borrowed-return content when an id-less parameter is
mutated through `++`/`--`, assignment expressions, `for` update expressions, or
other nested expression positions before being returned. Resolver-bound
parameters still use exact `BindingUse` facts, so this only narrows the legacy
id-less fallback. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_rejects_update_expr_mutation`,
`slot_policy::tests::idless_borrowed_return_policy_rejects_for_update_mutation`,
and `layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

Follow-up 2026-09-07: the remaining facade mut-borrow and string borrowed-view
signature helpers now use Java source spellings at source-IR lookup points.
`collect_facade_mut_borrow_argument_names` seeds parameter, local, and
enhanced-for receiver types by `source_name`, so keyword-escaped declarations
such as Java `self` / Rust `self__` still match later `Expr::Var("self")`
calls. `method_param_shape_constraints_with_declared_source_types` compares
those facade-mutated names against `Param::source_name` before adding
`MutableBorrow`/`MutableBinding`. The borrowed string return guard now matches
returned variables to `Param::source_name`, while still reading solved parameter
constraints from the emitted-name-indexed constraint table. Regressions:
`signature::tests::facade_mut_borrow_param_policy_uses_source_name`,
`signature::tests::facade_mut_borrow_local_receiver_type_uses_source_name`,
`signature::tests::facade_mut_borrow_enhanced_for_receiver_type_uses_source_name`,
and `signature::tests::string_return_borrowed_view_uses_param_source_name`.

Follow-up 2026-09-07, tightened 2026-09-12: the explicit id-less local-plan
compatibility lookup is read-only at the type level.
`expr_shape::idless_compatibility_local_plan_for_declared_type` is a module-level
compatibility primitive rather than a `ShapeEnv` method, and lower's wrapper no
longer passes a shape environment into the id-less-only lookup. The remaining
compatibility path is visibly a lookup rather than another occurrence cursor or
shape-mutating fallback.

Follow-up 2026-09-07: several source-IR signature collectors now compare
keyword-escaped parameters by Java `source_name`, not emitted Rust `name`.
Field-receiver mutation, array-element mutation, map-view receiver mutation,
direct local mutation, field-receiver reads, typed source return-nullability
map-get detection, local-constraint parameter type seeding, and input-stream
read lambda shadow filtering now all use the same spelling that appears in
`Expr::Var`. Regressions:
`signature::tests::method_keyword_param_field_receiver_mutation_uses_source_name`,
`signature::tests::method_keyword_param_array_element_mutation_uses_source_name`,
`signature::tests::typed_source_return_nullability_map_get_param_uses_source_name`,
and `signature::tests::input_stream_read_lambda_keyword_param_shadows_source_name`.

Follow-up 2026-09-07: method-body assigned-parameter shadow insertion now
queries assignment names by Java `source_name`, while still emitting the Rust
shadow with the escaped parameter `name`. The same assignment collector now
removes local and enhanced-for declarations by source spelling, so keyword-
escaped local declarations do not accidentally leave stale outer assignment
facts behind. Regression:
`idiom_gaps::non_rawable_rust_keyword_reassigned_param_gets_shadow`.

Follow-up 2026-09-07: lowerer's read-name/read-count compatibility collectors
now remove lambda parameters, local declarations, and enhanced-for variables by
Java source spelling too. This keeps keyword-escaped local shadows from leaking
as reads of an outer same-name binding. Regression:
`lower::tests::source_name_shadowing_removes_keyword_locals_from_read_sets`.

Follow-up 2026-09-07: the remaining lambda shadow filters now clear both the
Java source spelling and escaped Rust spelling. `collect_mutated_local_names`
no longer reports a lambda keyword parameter receiver mutation as an outer
binding mutation, and lower's expression read collector no longer reports a
keyword lambda parameter body read as an outer read. Regressions:
`signature::tests::mutated_lambda_keyword_param_shadows_source_name` and
`lower::tests::source_name_shadowing_removes_keyword_locals_from_read_sets`.

Follow-up 2026-09-07: the declaration-aware source-binding nullability helper is
crate-local. `source_binding_nullability_constraints_for_declaration` remains
the internal bridge for signature planning to use resolver-bound parameter
identity, but integration tests now assert the public parameter/return signature
effects instead of importing the compatibility helper directly. Regression:
`signature::tests::declaration_nullability_constraints_use_resolver_bound_parameter_identity`.

Follow-up 2026-09-10: normal test-only `MethodUsageFacts` binding fixtures
outside `usage.rs` now use the explicit fixture constructors
`MethodUsageFacts::from_exact_binding_facts`,
`MethodUsageFacts::from_idless_compatibility_facts`, or narrower test insertors
instead of manually writing both `bindings` and `bindings_by_id`. The broad
visible-binding fixture constructor is private to `usage.rs`; the shadowed
binding fixture in `aliasing` now spells the exact-id rows and visible-name row
separately through owner-provided test helpers. The layering guard pins those
fixture boundaries too.
Follow-up cleanup moved the deliberately malformed stale-map fixture in
`signature::binding_usage_for_bound_local_does_not_fall_back_by_name` behind
test-only `MethodUsageFacts` helpers, so the inconsistency remains explicit
without reopening direct map mutation outside `usage.rs`.

Follow-up 2026-09-10: `MethodUsageFacts::all_binding_facts()` is now the public
read-only aggregate view for integration tests that still need legacy
name-oriented assertions while production code stays off the mutable
`bindings`/`bindings_by_id` maps. `slot_shape_design.rs` uses a local test
extension backed by that view rather than reopening production
`binding_facts_for_name` helpers.
Follow-up cleanup renamed that local slot-shape extension from
`unique_idless_compatibility_binding_facts_for_source_name` to
`unique_binding_facts_for_source_name`. The helper filters the aggregate
`all_binding_facts()` view by source name and can see exact-id facts, so the old
name overstated an idless-only lookup that no longer exists at that boundary.
The stale same-name local assertion now expects this aggregate lookup to fail
closed and checks the two exact binding ids for the real facts.

Follow-up 2026-09-10 guard cleanup: the layering guard for test-only direct
binding-map mutations now counts the actual `.bindings_by_id.insert(` operation
instead of the inert `.bindings_by_id\n` substring. Current non-`usage.rs`
fixture setup goes through `MethodUsageFacts` constructors or test helpers
rather than mutating `bindings`/`bindings_by_id` directly.

Follow-up 2026-09-10 API fence: `BindingId::compatibility_local` is now
crate-local. It remains available to the internal id-less compatibility bridge
and same-crate tests, but external callers can no longer manufacture the
compatibility sentinel as public binding identity. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.
Follow-up cleanup private-backed `BindingId`'s `kind`, `name`, and `occurrence`
payload too. Callers still construct ids through `BindingId::new(...)` and
inspect them through `kind()`, `name()`, and `occurrence()`, but cannot mutate
local identity by assigning raw fields after construction.

Follow-up 2026-09-10 API fence: `MethodUsageFacts::from_idless_compatibility_facts`
is test-only crate-local too. The one integration test that used it only needed
synthetic dispatch-requirement inputs, so it now seeds exact binding facts
instead of keeping an external or production id-less compatibility constructor
available. Same-crate tests continue to cover the compatibility path directly.
Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up 2026-09-10 API fence: `MethodUsageFacts::idless_compatibility_binding_facts_for_source_name`
is now private to `usage.rs`. Production compatibility callers in `usage.rs`
still share the single helper, and same-module tests continue to pin stale-name
failure cases, but `signature.rs` no longer reaches across the module boundary
to ask id-less lookup questions directly. Guards:
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback` and
`signature::tests::binding_usage_for_bound_local_does_not_fall_back_by_name`.

Follow-up 2026-09-10 API fence: the old block/name
`source_binding_nullability_constraints(block, name)` compatibility helper is
test-only and private now. Production callers use the declaration-aware
`source_binding_nullability_constraints_for_declaration` path, while same-module
tests keep the direct stale-name/nullability assertions. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up 2026-09-10 API fence: the non-binding `MethodUsageFacts` collections
and flags are private-backed too. Call-site, constructor-call, returned-call,
field-use, and `mutates_self` reads now go through slice/bool accessors. A later
cleanup deleted the temporary synthetic field-use constructor after it proved to
have no callers, so field-use fixture construction no longer has a standalone
helper. Guard: `layering::usage_owns_call_site_boundary_facts`.

Follow-up 2026-09-10 API fence: `MethodUsageFacts` visible-binding scope
maintenance is private to `usage.rs`. `snapshot_visible_binding_for_scope`,
`hide_visible_binding_for_scope`, `restore_visible_binding_for_scope`,
`record_visible_binding_use`, `record_declared_binding_use`,
`visible_binding_id_for_name`, and `name_binding_is_current` no longer expose
crate-local hooks for mutating or querying the compatibility name map directly;
cross-module users keep the narrower typed readers. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up 2026-09-11 API fence: constructor-side rust-name compatibility usage
helpers are gone. `UsageFacts::insert_constructor_usage_for_current_rust_name`
was unused and deleted; integration tests that previously queried
`constructor_usage_by_current_rust_name` now resolve the source constructor
declaration and call `source_constructor_usage(ty, ctor)`. A later same-day pass
removed the method-side siblings too: integration tests now resolve source
method declarations and call `source_method_usage(ty, method)`, and the two
synthetic rust-name method fixture builders were replaced by tiny source
programs whose bodies produce the same usage facts through `analyze_usage`.
Guard:
`layering::usage_keys_hide_raw_owner_and_name_fields`.

Follow-up cleanup narrowed `LocalPlan::source_name` to crate-local and deleted
the unused `LocalPlan::access_name` accessor. The plan still carries explicit
binding and emitted access-name data, but external tests and users no longer get
a public convenience API that encourages treating local-plan identity as a pair
of names instead of the resolver `BindingId`. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up cleanup also narrowed `MethodUsageFacts::from_exact_binding_facts` to
test-only crate-local callers. Exact binding fixtures can still construct
method-usage facts directly, but production code and external consumers keep the
validated read-only `all_binding_facts()` view instead of a public constructor
for binding fact maps. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up cleanup deleted the sibling `MethodUsageFacts::from_field_uses`
constructor after narrowing showed it had no callers. Public consumers still get
the read-only `field_uses()` accessor, while synthetic field-use map
construction no longer has a standalone helper. Guard:
`layering::usage_owns_call_site_boundary_facts`.

Follow-up guard cleanup made the intentional public usage boundary explicit.
`UsageFacts::source_method_usage` and `source_constructor_usage` remain public
because they take resolved source declarations, while the Java-key, owner-key,
and resolved-source compatibility helpers stay crate-local. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up cleanup narrowed the usage call-boundary policy carriers. The public
factory functions still return `CallResultBoundaryPolicy`,
`ConstructorInvocationBoundaryPolicy`, `CallArgumentBoundaryPolicy`, and
`MethodApiBoundaryPolicy`, but those records no longer expose their raw
name/count/type/boundary fields for external construction or mutation. Tests and
callers assert through accessors instead. Guard:
`layering::usage_owns_call_site_boundary_facts`.

Follow-up cleanup removed the stale receiver-type argument from
`resolved_receiver_call_boundary`. The boundary decision had already stopped
using static receiver type to upgrade resolved source instance calls, so the
extra parameter only preserved an id-less compatibility-looking hook. Guard:
`usage::tests::resolved_receiver_boundary_keeps_cross_object_source_calls_unknown`.

Follow-up cleanup narrowed the stored call-site usage records too.
`CallSiteUse`, `ConstructorCallSiteUse`, and `ReturnedCallResultUse` still expose
read-only query methods, but their raw name/count/boundary/resolved payloads are
private-backed. Signature propagation and integration tests now read those facts
through accessors instead of reaching into the usage record layout. Guard:
`layering::usage_owns_call_site_boundary_facts`.

Follow-up cleanup private-backed `BindingUseFacts` as well. The usage walker
still owns mutation of binding-use payloads internally, while callers and tests
read binding source name, optional `BindingId`, Java type, and recorded uses
through accessors. A later cleanup narrowed `BindingUseFacts::new` to test-only
crate-local fixtures and moved synthetic policy assertions beside
`signature.rs`, so integration tests no longer get a public constructor for
id-less binding fact payloads. This keeps local-binding facts from exposing a
mutable name/id/type tuple as public identity. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Follow-up cleanup narrowed signature binding-boundary helpers that are only
used inside the signature solver. `binding_boundary_policy`,
`external_argument_binding_constraints`,
`external_call_return_binding_constraints`, and
`external_receiver_binding_constraints` are crate-local now; public callers keep
the higher-level method/constructor/field/signature-policy entry points. The
native collection receiver mutation assertion moved beside the helper in
`signature.rs`, while the integration fixture keeps the end-to-end signature
shape assertion. Guards:
`signature::tests::external_receiver_constraints_keep_native_collection_mutable_borrow`,
`slot_shape_design::native_collection_add_all_argument_boundary_borrows_source_content`,
and `layering::signature_boundary_policy_is_the_single_boundary_constraint_entrypoint`.

Codex follow-up 2026-09-12: the remaining exported slot-policy helper no longer
asks the broad name-only question "does this method assign to this binding?".
Callers now use
`idless_compatibility_exact_returned_param_is_assignment_safe(...)`, which
combines the exact-return and no-assignment checks behind the id-less
compatibility boundary; the raw assignment scan is private to `slot_policy.rs`.
Guard: `layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

Codex follow-up 2026-09-12: signature's method and constructor constraint sets
no longer open-code separate copies of the id-less local-name uniqueness check.
Both wrappers still name the compatibility boundary, but they delegate the
"duplicate visible name fails closed" policy to one private helper:
`idless_compatibility_local_index_for_constraints(...)`. This is a fence
tightening step only; the audit remains open for the broader id-less
compatibility surface. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Codex follow-up 2026-09-12: the exact `BindingId` local-constraint lookup beside
that compatibility path is centralized too. `MethodConstraintSet` and
`ConstructorConstraintSet` still expose separate internal read/mutate wrappers,
but both route through `local_index_for_binding_constraints(...)`, which also
validates indexed entries before falling back to the local list. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Codex follow-up 2026-09-12: method/constructor parameter-name lookup now follows
the same pattern. The two constraint-set wrappers delegate to
`param_index_for_constraints(...)`, so the indexed fast path is validated once
before falling back to the parameter list instead of keeping parallel name
lookup snippets beside the id-less compatibility code. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Codex follow-up cleanup: the id-less local-name read and mutable-read wrappers
now share `idless_compatibility_local_for_constraints(...)` and
`idless_compatibility_local_mut_for_constraints(...)` instead of duplicating the
index-to-local lookup in both method and constructor constraint sets. The
exact-`BindingId` siblings now share `local_for_binding_constraints(...)` and
`local_mut_for_binding_constraints(...)` too. This keeps the compatibility and
exact-binding boundaries named on each wrapper while moving the shared behavior
into private helpers. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Codex follow-up cleanup: declared visible-binding insertion now goes through the
private owner-module helper `MethodUsageFacts::insert_visible_binding_for_scope`.
The free `insert_binding_use_facts` wrapper delegates to that method instead of
mutating `bindings`/`bindings_by_id` directly, matching the existing
snapshot/restore/record helper boundary for the legacy visible-name map. Guard:
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

Codex follow-up cleanup: the test-only visible-binding fixture constructor now
delegates to the same private owner-module insertion policy through
`MethodUsageFacts::insert_visible_binding_facts_for_scope(...)` instead of
open-coding the two-map update. The helper accepts a pre-built `BindingUseFacts`
so fixture payloads keep their existing recorded uses while the map-maintenance
rule stays in one place. Guard:
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.

Codex follow-up cleanup: current visible-name validation is now centralized in
the private owner-module helper
`MethodUsageFacts::current_visible_binding_facts_for_name(...)`. The id-less
compatibility lookup, visible binding-id lookup, scope snapshot, and current
name check share the same stale-map rejection path, while receiver-handle type
lookup still requires a resolver binding id and does not treat id-less
compatibility entries as a general type environment. Guards:
`usage::tests::visible_binding_id_for_name_centralizes_compatibility_lookup` and
`layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.
The same pass moved mutable visible-name updates behind the private
`current_visible_name_binding_mut_for_scope(...)` helper, so record-use paths
share the same current-name-map validation before mutating the compatibility
entry.

Codex follow-up cleanup: source-constructor compatibility substitution now
matches Java IR reads against `Param::source_name` rather than the rendered Rust
parameter name. This closes the keyword-escaped residue in delegated
`this(...)`/`super(...)` constructor composition. The same source-name rule now
feeds enum payload constructor detection in both lowering and representation
emission, and the hand-rendered enum payload accessor path emits raw identifiers
for rawable Rust keywords. Guards:
`idiom_gaps::source_super_constructor_composes_delegated_parameter_bindings` and
`idiom_gaps::source_enum_constructor_payload_fields_emit_match_backed_accessors`.

Codex follow-up cleanup: `ShapeEnv`'s map-view and simple-map-entry side-table
helpers are crate-local now. Lowering and shape inference still share the
binding-aware tables internally, but external callers cannot mutate or inspect
those name-keyed compatibility side facts directly. Guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Codex follow-up cleanup: delegated-null constructor field constraints in
signature analysis now compare target constructor assignments against
`Param::source_name` rather than the emitted Rust parameter name. A delegated
constructor target that initializes a field from a Java parameter such as
`self` can now still mark `this(..., null)` fields nullable after Rust keyword
renaming. Guard:
`idiom_gaps::source_super_constructor_delegated_null_marks_flattened_field_optional`.

Codex follow-up cleanup: `ShapeEnv` access-name insertion helpers are
crate-local now. Lowering and stream lowering can still attach rendered access
names to exact binding/source-name facts, and field/static-storage lowering can
still attach access names to side facts, but downstream callers cannot mutate
the name/access-name compatibility maps through public `ShapeEnv` methods.
Guard: `layering::idless_local_binding_compatibility_surface_stays_explicit`.

Codex follow-up cleanup: the unused id-less loop-variable and catch-parameter
compatibility insertion helpers were deleted. Shape collection still seeds
legacy compatibility bindings through the binding-aware insertion helpers when
no resolver `BindingId` exists, but integration tests and downstream callers no
longer have a separate API for writing those binding-kind side facts. The
catch-parameter insertion helper that lacks an access-name boundary is private to
`expr_shape` now; cross-module lowering uses the exact source/emitted-name
wrapper instead. Boundary guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Codex follow-up cleanup: the intermediate field/static-field/static-storage
access-name insertion steps inside `ShapeEnv` are private unless a lowerer
actually needs the cross-module owner-aware boundary. That keeps ordinary field
and storage side-table mutation owned by `expr_shape`, while lowering still gets
crate-local entry points for owner-qualified static field and type static-storage
facts. Boundary guard:
`layering::idless_local_binding_compatibility_surface_stays_explicit`.

Codex follow-up 2026-09-14 later: expression-level pattern-binding scopes now
feed the same id-less borrowed-return assignment scanner used by statement
branches. A pattern binding introduced by the left side of `&&`, by the negated
left side of `||`, or by a ternary condition shadows the same-source-name
id-less parameter while scanning the reachable right/arm expression. Assigning
to that pattern variable no longer rejects borrowed-return policy for the outer
parameter. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_ternary_pattern_binding_assignment`,
`slot_policy::tests::idless_borrowed_return_policy_ignores_and_pattern_binding_assignment`,
and
`slot_policy::tests::idless_borrowed_return_policy_ignores_or_negated_pattern_binding_assignment`.

Codex follow-up 2026-09-14 later: switch case labels that introduce a pattern
binding now shadow the id-less outer binding while scanning that case's
statements. A pattern case assignment no longer looks like a mutation of the
returned parameter, and a return from the pattern binding no longer proves the
outer parameter is returned. Regressions:
`slot_policy::tests::idless_borrowed_return_policy_ignores_switch_pattern_case_assignment`
and
`slot_policy::tests::idless_borrowed_return_policy_rejects_switch_pattern_case_return`.
The same label shadow is applied to switch expression arm values for the
assignment scanner; exact-return scanning does not currently descend into switch
expression values. Regression:
`slot_policy::tests::idless_borrowed_return_policy_ignores_switch_expression_pattern_case_assignment`.

Codex follow-up 2026-09-14 later: liveness scope recording now carries
`instanceof` pattern bindings into the true/false branch scopes that can see
them, and kills those branch-local bindings at the branch boundary so they do
not become live before the condition that creates them. Reads and writes of a
same-source-name pattern binding therefore no longer keep or mutate the outer
binding in liveness queries. Regressions:
`liveness::tests::if_pattern_binding_read_does_not_keep_outer_same_name_live`
and
`liveness::tests::if_pattern_binding_write_does_not_mutate_outer_same_name`.
Additional coverage pins negated-pattern `else` scopes, `while` pattern scopes,
and switch pattern-label scopes:
`liveness::tests::negated_else_pattern_binding_read_does_not_keep_outer_same_name_live`,
`liveness::tests::while_pattern_binding_write_does_not_mutate_outer_same_name`,
and
`liveness::tests::switch_pattern_label_read_does_not_keep_outer_same_name_live`.

Codex follow-up 2026-09-14 later: usage analysis now gives reachable
`instanceof` pattern variables their own temporary visible binding while
analyzing the branch/case body that can see them. Same-source-name pattern
receiver reads therefore no longer attach receiver-call facts to an outer
parameter or local merely because the pattern variable has no resolver
`BindingId`. The temporary pattern binding is intentionally id-less: it prevents
wrong attribution without widening the broader id-less receiver-type fallback.
Regressions:
`usage::tests::instanceof_pattern_binding_read_does_not_use_outer_same_name_binding`
and
`usage::tests::negated_instanceof_else_pattern_binding_read_does_not_use_outer_same_name_binding`.

Codex follow-up 2026-09-14 later: signature source-nullability scopes now
represent id-less pattern-variable shadows separately from resolver-bound
locals. A pattern binding can therefore block same-name field fallback and
same-name outer binding updates without inventing a fake `BindingId`. This fixes
two wrong-answer paths: returning a pattern variable no longer borrows a
same-name field's nullability fact, and assigning `null` to a same-name pattern
variable no longer marks the outer parameter nullable. Regressions:
`signature::tests::source_return_nullability_pattern_binding_does_not_fall_back_to_field`
and
`signature::tests::source_return_nullability_pattern_assignment_does_not_update_outer_param`.

Codex follow-up 2026-09-14 later: signature source-nullability shadow scopes now
also apply inside expression-level control flow. Ternary arms inherit the
true/false pattern-variable scope from the condition, and short-circuit `&&`/`||`
right-hand sides inherit the corresponding reachable pattern scope from the
left-hand side. This closes the same-name rejoin for expression-only returns and
assignments such as `(input instanceof String value) ? value : fallback` and
`input instanceof String value && (value = null) == null`. Regressions:
`signature::tests::source_return_nullability_ternary_pattern_binding_does_not_fall_back_to_field`,
`signature::tests::source_return_nullability_ternary_pattern_assignment_does_not_update_outer_param`,
and
`signature::tests::source_return_nullability_short_circuit_pattern_assignment_does_not_update_outer_param`.

Codex follow-up 2026-09-14 later: the remaining name-only source-nullability
compatibility map now shares the same pattern-variable shadowing rules as the
binding-aware source-nullability passes. An id-less outer parameter no longer
inherits nullable-storage evidence from assignments or null comparisons against
a same-source-name pattern variable in statement branches, switch cases, ternary
arms, or short-circuit right-hand sides. Regressions:
`signature::tests::idless_source_nullability_pattern_assignment_does_not_update_outer_param`
and
`signature::tests::idless_source_nullability_expression_pattern_assignment_does_not_update_outer_param`.

Codex follow-up 2026-09-14 later: usage analysis now applies pattern-variable
visible-binding shadows inside expression-level control flow too. Short-circuit
`&&`/`||` right-hand sides, ternary arms, and switch-expression case values no
longer charge receiver-call/content-read facts to an outer resolver-bound
same-name parameter when the read is from an id-less pattern variable.
Regressions:
`usage::tests::and_pattern_binding_read_does_not_use_outer_same_name_binding`,
`usage::tests::or_negated_pattern_binding_read_does_not_use_outer_same_name_binding`,
`usage::tests::ternary_pattern_binding_read_does_not_use_outer_same_name_binding`,
and
`usage::tests::switch_expression_pattern_binding_read_does_not_use_outer_same_name_binding`.
