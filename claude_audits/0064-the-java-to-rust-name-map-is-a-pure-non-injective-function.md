# 0064 — The Java→Rust name map is a pure, non-injective function of one string

Status: **open, model-level** — the 7 historical repros were validated with
`javac`/`java` oracles and `rustc` output, but the current tree has local fixes
or ambiguity guards for the concrete paths named below. The remaining defect is
the broader identity model: package/type ownership is still not first-class
across the analysis keys.
Severity: medium-high while mitigated — the original repros included three
*silently wrong programs* (wrong class constructed, wrong package's constant
read, `unimplemented!` for a class the translator is itself translating), but
the live risk is recurrence at remaining string-owner surfaces rather than a
known still-failing repro in this file.
Kind: identity modelling — the same failure family as
[0009](../../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md),
one level below it

Current tree update:

* Follow-up 2026-09-07: `representation_lowering::source_class_bound_trait_name`
  is now crate-local. This does not change the remaining first-class owner/type
  identity work, but it removes one rendered-name helper from the public API and
  keeps the compatibility surface fenced by `layering`.
* Follow-up 2026-09-07: `slot_render::rust_name_for_java_named_type` is now
  crate-local, and the CLI path that still needs name projection calls the
  canonical `java_naming::rust_name_for_java_named_type` API directly. This
  does not solve package-aware identity, but it removes a duplicate public
  string-only type-name projection surface. Regression:
  `layering::declaration_context_usage_lookups_use_java_callable_keys`.
* Follow-up 2026-09-10: `java_accessor_method_reference_fallback_name` is
  crate-local. Method-reference shape inference still uses the centralized
  Java-naming policy for accessor fallback compatibility, but that predicate is
  no longer part of the public crate API.
  Follow-up cleanup also narrowed the Java getter/alias helpers
  (`java_getter_method_name`, `java_getter_field_name`, and
  `java_method_name_aliases`) to crate-local callers. These remain compatibility
  helpers over non-injective Java/Rust spelling, but external crate users cannot
  treat them as an identity API.
  `parser::to_snake_case` and `parser::to_snake_case_unescaped` are crate-local
  now too, as are the centralized `java_naming` keyword-policy helpers; they
  remain internal compatibility primitives while the first-class name-allocation
  model remains open.
  Follow-up cleanup narrowed
  `object_model::rust_enum_variant_name_from_parsed_constant` to crate-local as
  well; direct enum-constant spelling assertions now live beside `object_model`.
* Follow-up 2026-09-07: lowerer's remaining legacy Rust-name usage fallback now
  routes through `CallableAnalysis` instead of iterating
  `UsageFacts.methods_by_rust_name` directly. The compatibility lookup still
  exists for current-method contexts that lack a `MethodDecl`, but the
  string-keyed overload-family scan is fenced at the callable-analysis boundary
  and pinned by `layering::callable_analysis_shell_reaches_lowering_context`.
  The same boundary now hides the backing `usage`, `call_graph`, `signatures`,
  `exceptions`, and `fallibility` fields behind query methods, so downstream
  code cannot bypass the shell and reach the legacy maps directly. The
  Rust-name-family accessors are crate-local, and the unused stored `call_graph`
  field was deleted rather than kept as a dead public handle.
* Follow-up 2026-09-10: `UsageFacts.methods_by_java_key` is now private to
  the usage module.
  The broad `source_method_usage_by_java_key` reader is private too; non-owner
  code uses source declarations, resolved-owner helpers, or narrower test-only
  predicates instead of arbitrary Java-key lookup. The same-simple-name
  package-peer regression asserts usage identity through that declaration
  boundary instead of directly iterating the backing map. This does not replace
  owner strings with a first-class key, but it removes one more direct Java-key
  table access outside the owning module. Regression:
  `layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`
  and `batch_analysis_keys_keep_same_simple_name_package_peers_distinct`.
  Follow-up cleanup briefly moved that package-peer regression's usage
  assertion through `CallableAnalysis::method_usage_by_java_key`, then removed
  that broad test-only shell method as well. The fixture now checks usage through
  `UsageFacts::source_method_usage(ty, method)` on the recovered source
  declarations; production and tests use declaration/resolved-owner helpers
  instead of exposing an arbitrary Java-key usage lookup through the shell.
  Follow-up cleanup also made the arbitrary `source_method_usage_for_owner`
  helper private to `usage.rs`; cross-module resolved-owner queries now use the
  narrower `resolved_source_method_usage_for_owner(owner, method)` wrapper.
  `CallableAnalysis` no longer stores `SignaturePlan` either; the only
  remaining package-peer signature assertion now queries the signature plan
  directly, and the callable shell is limited to usage, exception, and
  fallibility questions.
  Follow-up cleanup confined the package-peer fixture's remaining raw
  `JavaCallableKey` owner/name predicates to named test helpers, so the test
  body states the Java-owner-space invariant instead of open-coding alias
  checks at each assertion site.
  Codex follow-up 2026-09-12: `facts::external_type_name_matches` is now
  crate-local. This does not replace normalized external type-name matching
  with a first-class identity key, but it removes another public ad hoc
  equivalence hook. Guard:
  `layering::declaration_context_usage_lookups_use_java_callable_keys`.
  Follow-up cleanup pushed the negative alias check behind
  `UsageFacts::source_method_usage_has_collapsed_owner_alias`, deleting the
  test-only raw `source_method_usage_java_keys()` iterator. The fixture still
  checks for `Item`/`AItem`/`BItem` collapse, but it no longer iterates the
  backing Java-key map outside `usage.rs`.
  The broad `method_usage`, `constructor_usage`, `method_usage_by_key`, and
  `constructor_usage_by_key` wrappers are private implementation details now;
  callers use named current/source/resolved usage questions instead of reaching
  for arbitrary usage keys. Current-Rust-name compatibility remains explicit as
  `UsageFacts::current_rust_method_usage` and
  `UsageFacts::current_rust_constructor_usage`.
  The remaining result/exception `CallableAnalysis` questions are crate-local
  too, matching their actual lowerer-only caller set instead of making the shell
  a public API.
  Follow-up cleanup made the `CallableAnalysis` shell type and constructor
  crate-local as well. Lowering still receives the same analysis handle, but
  external callers cannot treat the compatibility shell as public crate API.
  Test-only usage seeding now goes through
  `UsageFacts::insert_source_method_usage` instead of mutating the backing map
  from `signature.rs` and `slot_policy.rs`.
  The fallibility-invariant test helper also now uses
  `CallableKey::method_identity` instead of a direct `CallableKey::Method`
  literal, and the raw-key guard covers that test module while still allowing
  legitimate production pattern matches.
  Follow-up cleanup narrowed `signature::MethodKey::type_name` and
  `signature::ConstructorKey::{type_name,rust_name}` to crate-local callers and
  deleted the unused `MethodKey::rust_name` accessor. External tests that need
  to assert signature dependency identity now compare whole keys built through
  `rust_identity(...)` instead of inspecting the owner/name tuple.
  Signature field-shape queries also now expose declaration-shaped public
  helpers, `field_shape_for_declared_field` and
  `required_declared_field_shape`; raw `(type_name, field_name)` lookup is
  crate-local compatibility surface for lowerers that still operate on rendered
  owner/name pairs.
* Follow-up 2026-09-10: the same-simple-name package-peer regression now also
  asserts emitted source-item cardinality for the distinct `AItem` and `BItem`
  owners. This closes the test-surface hole where the fixture proved the names
  appeared and compiled together but did not explicitly prove that each peer was
  emitted exactly once under its own owner-derived Rust name. Regression:
  `batch_translation::package_qualified_source_type_references_keep_package_identity`.
* Follow-up 2026-09-10: `JavaCallableKey` and its ownerless
  `JavaCallableSlotKey` projection no longer expose raw identity fields
  publicly. Construction still goes through `source_method_for_type`,
  `source_method_for_owner`, `slot()`, and resolved-source helpers; current
  internal readers use read-only `owner()`/`java_name()` accessors where they
  still need compatibility owner/name pairs. This is not the full first-class
  owner-key migration, but it closes the public raw-field bypass that could
  recreate source callable identity from arbitrary strings.
  Follow-up cleanup narrowed `JavaCallableKey::source_method_for_type` to
  crate-local as well; external callers no longer get a public constructor for
  source callable keys.
  Guard:
  `layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.
  Follow-up cleanup narrowed `facts::CallableKey::with_owner_type` to
  crate-local too. It remains available to builtin external-pack alias expansion
  but is no longer a public owner-rewriting API over raw strings.
  Codex follow-up 2026-09-11 narrowed the inherited-member result carriers as
  well: source inherited field/method refs and external inherited field/method
  records now keep their owner/name payloads private behind read-only
  accessors. This does not replace the remaining string owner model, but it
  prevents downstream callers from treating inherited-member discovery results
  as mutable identity structs.
* Follow-up 2026-09-10: `resolve::ConstantDeclaration` no longer exposes its
  package-qualified owner, package, or field payload publicly. Constant folding
  still constructs declarations centrally from `TypeDecl` traversal and keeps
  the owner/package distinction needed for nested types, but downstream callers
  cannot fabricate or mutate constant owner facts by assigning raw strings into
  the declaration record. Guard:
  `layering::declaration_context_usage_lookups_use_java_callable_keys`.
* Follow-up 2026-09-10: `signature::MethodKey`,
  `signature::ConstructorKey`, and `signature::FieldKey` now have named
  constructors and read-only accessors, and their raw owner/name strings are
  private. Source field collection uses the declaration-aware constructor when
  it has `TypeDecl`/`FieldDecl` context, while compatibility lookups use the
  explicit Rust-identity constructor. This fences the signature tables without
  claiming the broader first-class owner key migration is complete. Guard:
  `layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.
  Codex follow-up 2026-09-10 narrowed `signature::FieldKey::type_name()` to
  crate-local and deleted the unused field-name accessor because field-key
  owner/name inspection has no public graph API caller. `signature::MethodKey`
  and `signature::ConstructorKey` accessors remain public for the public
  signature-dependency graph node surface; replacing that graph-facing string
  inspection needs a separate API pass.
* Follow-up 2026-09-10: `usage::UsageMethodKey` and
  `usage::UsageConstructorKey` now have the same fence: named constructors plus
  read-only accessors, with raw owner/name fields private. The remaining
  signature bridge reads these keys through accessors instead of depending on
  public struct payloads. This still preserves the existing string identity, but
  prevents non-owner code from fabricating usage keys through field assignment.
  Guard:
  `layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.
  Follow-up cleanup made the primary `UsageFacts.methods` and
  `UsageFacts.constructors` maps private as well. Solver code that still needs
  full traversal now uses owner-provided entry/value iterators, direct lookups go
  through key query methods, and fixture seeding goes through
  `insert_method_usage` instead of replacing the backing map.
  The legacy Rust-name compatibility map is private too. The remaining
  Rust-name family binding lookup is now owned by
  `UsageFacts::binding_facts_for_rust_method_family`, so `callable_analysis`
  asks the narrow binding question instead of iterating the non-injective map.
  Follow-up cleanup narrowed the current-Rust-name usage-key constructors to
  crate-local and added `UsageFacts` owner-level lookup/insert helpers for the
  compatibility cases that still seed or query by emitted Rust names. Integration
  tests no longer construct those raw compatibility keys directly.
  Codex follow-up 2026-09-10 narrowed the read-only
  `UsageMethodKey`/`UsageConstructorKey` string accessors to crate-local as
  well, so external tests and downstream users cannot treat those compatibility
  strings as public semantic identity. Guard:
  `layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.
  Codex follow-up 2026-09-12 narrowed
  `usage::CallableKey::{resolved_method,resolved_constructor,constructor_identity,external_constructor}`
  to crate-local constructors. Public callers still get the source-method and
  external-method identity surfaces needed by the integration call-graph tests,
  plus read-only `*_parts` inspection, but downstream code can no longer build
  constructor or resolved-call graph keys from arbitrary owner/name strings.
  Guard: `layering::declaration_context_usage_lookups_use_java_callable_keys`.
  Follow-up cleanup also narrowed `CallGraph::from_edges` and
  `CallGraphEdge::new` to cfg-test crate-local callers. Public users can still
  inspect nodes/edges and build node-only synthetic graphs through `from_nodes`,
  but arbitrary edge construction over `CallableKey` identity is no longer a
  production or public crate API. Guard:
  `layering::declaration_context_usage_lookups_use_java_callable_keys`.
  Codex follow-up 2026-09-12 replaced remaining direct
  `usage::CallableKey::{Method,Constructor,ExternalMethod}` field
  destructuring in `semantic_analysis` and `exception_plan` with the named
  `method_identity_parts`, `constructor_identity_parts`, and
  `external_method_parts` query methods. The callable-key layering guard now
  covers those production modules too, so non-owner code cannot casually reopen
  the raw owner/name enum payload while traversing call graph edges or seeding
  fallibility.
* Follow-up 2026-09-09: usage collection and signature planning no longer build
  declaration-backed source method/constructor keys with local struct literals.
  The canonical source-method key, source-constructor key, and legacy Java-name
  compatibility method key now have named constructors on
  `UsageMethodKey`/`UsageConstructorKey` and `signature::MethodKey`/
  `signature::ConstructorKey`, so the Java-name compatibility insertion is
  visible at the API boundary instead of looking like another ordinary usage
  key. Lowerer's declaration-less current-method/current-constructor fallback
  now goes through `CallableAnalysis::current_rust_method_usage` and
  `CallableAnalysis::current_rust_constructor_usage`; those helpers use
  explicitly named Rust-name constructors instead of local struct literals.
  Follow-up cleanup moved source-constructor usage lookups behind
  `UsageFacts::source_constructor_usage`, matching the existing source-method
  lookup surface so callers with a real constructor declaration do not reach
  into the constructor map. Guards/tests:
  `usage::tests::usage_source_key_constructors_name_canonical_and_legacy_paths`,
  `signature::tests::signature_source_key_constructors_use_resolved_rust_names`,
  and `layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.
  Follow-up cleanup added `UsageFacts::resolved_source_method_usage` and a
  shared resolved-source key helper, so aliasing no longer rebuilds
  `JavaCallableKey::resolved_source_method` or reads `methods_by_java_key`
  directly to decide receiver mutation; the self-mutability propagation fixpoint
  uses the same helper for membership checks.
* Follow-up 2026-09-09: signature dependency construction no longer lets a
  Java-name `UsageMethodKey` pick the first planned overload whose Rust name has
  the same prefix. `method_key_for_usage` still accepts an exact match and a
  unique legacy prefix match, but multiple overload-prefix matches now fail
  closed instead of depending on `HashMap` iteration order. Regression:
  `signature::tests::usage_method_key_prefix_fallback_requires_unique_planned_method`.
* Follow-up 2026-09-09: lowerer's source-context static signature fallbacks by
  Rust name and by base name/argument types now fail closed when both owner
  candidates match. The compatibility paths still support a single
  current/source owner match, but no longer choose the first string owner
  candidate when both expose the matching static method. Regressions:
  `lower::tests::source_context_static_signature_by_rust_name_refuses_ambiguous_context_owner`
  and
  `lower::tests::source_context_static_signature_by_name_and_args_refuses_ambiguous_context_owner`.
* Follow-up 2026-09-09: emitted-source argument planning no longer lets
  `method_signature_for_owner_aliases` choose the first canonical/Rust-projected
  owner alias when the exact owner key misses. Exact owner lookup still wins,
  but fallback aliases must identify one unique owner signature. Regression:
  `lower::tests::method_signature_for_owner_aliases_refuses_ambiguous_fallback_owner_alias`.
* Follow-up 2026-09-09: receiver-method signature fallbacks now keep
  compatibility aliases only when they resolve to one unique `(owner, method)`
  target. Duplicate spellings of the same resolved owner are ignored, but a
  second distinct owner makes the fallback fail closed. Source-context receiver
  fallback also pairs argument shapes with the overload selected by argument
  types instead of borrowing `matching_methods[0]`. Regression:
  `lower::tests::source_context_receiver_fallback_pairs_selected_signature_with_selected_overload`.

  Codex follow-up 2026-09-12: the source-receiver recovery path no longer
  unwraps `SourceContextOwnerKey` through `source_receiver_type_name(...)`.
  Output/input stream special cases, source declared overload retargeting,
  Object `toString` fallback, and trivial-accessor return-shape lookup now keep
  the `SourceContextOwnerKey` wrapper through the exact signature/type lookup
  boundary instead of eagerly downgrading to a raw string. This is still a
  string-backed compatibility wrapper, not first-class package-aware owner
  identity. Guard: `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.
  The same pass removed two sorted-owner fallbacks that could erase resolver
  priority: optional current-method display shape lookup and returned-field
  accessor shape lookup now try the resolved declaring owner before the
  receiver/current owner. Regressions:
  `lower::tests::optional_current_method_display_shape_refuses_ambiguous_current_owner`
  and
  `lower::tests::source_trivial_accessor_returned_field_shape_prefers_resolved_owner`.
  Follow-up cleanup: the immediate receiver fallback candidate helper now keeps
  `SourceContextOwnerKey` values through lookup callers instead of downgrading
  to plain `String` at the API boundary. Guard:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.
  Follow-up cleanup: nested receiver-owner candidates, receiver fallback
  ambiguity resolution, source-context receiver fallback lookup, and
  source-call owner lookup now ask `SourceContextOwnerKey` for nested owners,
  resolved owner identity, or context type lookup instead of open-coding
  `find_context_type_by_key_in_lower_ctx(ctx, owner.as_str())` /
  `resolved_context_owner_key(ctx, ...)` beside the wrapper. This still does not
  make the key first-class or package-aware, but it narrows another raw-string
  escape hatch at the fallback boundary. Guards:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup` and
  `lower::tests::nested_owner_candidates_refuse_ambiguous_current_owner`.
  Follow-up cleanup: the source returned-field lookup helper used by
  receiverless calls and trivial accessor return-shape recovery now accepts a
  `SourceContextOwnerKey` and asks it for context type lookup, rather than
  taking an already-downgraded `&str` owner. The signature-field lookup still
  consumes the legacy string key, so this is another wrapper-boundary cleanup
  rather than first-class owner identity. Guard:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.
  Follow-up cleanup: resolved static receiver matching now asks
  `SourceContextOwnerKey::resolves_to_owner(...)` for both parsed static-owner
  candidates and static import aliases instead of routing through a free
  `context_type_key_resolves_to_owner(ctx, type_key, owner)` helper over raw
  strings. This keeps the comparison at the wrapper boundary; the owner payload
  is still string-backed. Guards:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup` and
  `lower::tests::resolved_static_fallback_uses_resolved_context_owner_not_declaring_spelling`.
  Follow-up cleanup: `SourceContextOwnerKey::resolves_to_owner` now takes the
  target owner as `&SourceContextOwnerKey` too, so the resolved-owner comparison
  no longer reopens a raw owner string lookup inside the wrapper method.
  Follow-up cleanup: static-owner emitted-context checks now keep
  `SourceContextOwnerKey` candidates through the emitted/source-summary
  predicate too. `SourceContextOwnerKey::{is_static_context_emitted,
  is_non_program_emitted_context,is_emitted_program_context}` replace the
  free `static_owner_key_is_non_program_emitted_context(owner, ctx)` helper for
  the static-owner receiver and fallback-signature paths. Guard:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.
  Follow-up cleanup: those three predicates now share
  `SourceContextOwnerKey::find_emitted_context_type_and_program(...)`, keeping
  the raw emitted-context lookup at one owner-key boundary.
  Follow-up cleanup: the remaining static emitted-owner helpers for
  source-program alias checks now delegate to
  `SourceContextOwnerKey::{is_static_context_emitted,is_emitted_program_context}`
  instead of calling the raw emitted-context lookup directly. Guard:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.
  Follow-up cleanup: the receiverless current static source-context fallback now
  wraps `ctx.current_type` as a `SourceContextOwnerKey` before resolving the
  current declaration, instead of reopening
  `find_context_type_by_key_in_lower_ctx(ctx, current_type)` inside the fallback
  helper. The signature-plan probes still use legacy string owners, so this is
  a boundary cleanup, not first-class owner identity. Guard:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.
  Follow-up cleanup: nested receiver-owner recovery now wraps
  `ctx.current_type` as `SourceContextOwnerKey` before resolving the current
  declaration. This keeps the nested-type owner helper on the same wrapper
  boundary as receiver-owner lookup instead of reopening
  `find_context_type_by_key_in_lower_ctx(ctx, current_type)` locally. Guard:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`;
  behavior probe:
  `lower::tests::nested_owner_candidates_refuse_ambiguous_current_owner`.
  Follow-up cleanup: receiverless summary return-shape lookup now accepts
  `&SourceContextOwnerKey` for the current owner and resolves through
  `current_owner.find_context_type(ctx)?`. Both production callers wrap
  `ctx.current_type` before crossing the helper boundary, and the ambiguity
  behavior remains covered by
  `lower::tests::receiverless_summary_return_shape_refuses_ambiguous_current_owner`.
  Follow-up cleanup: the remaining direct tuple-struct construction sites for
  `SourceContextOwnerKey` now go through `SourceContextOwnerKey::new(...)`.
  This does not change the string-backed model, but it keeps all owner-key
  construction behind the named wrapper boundary. Guard:
  `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.
* Follow-up 2026-09-09: fact-level method and constructor keys now have named
  constructors. Parsed-source semantic/resolve facts use
  `facts::MethodKey::source_method`, external/user override facts use
  `facts::MethodKey::named`, and constructor recursion facts use
  `facts::ConstructorKey::named`. This preserves the existing stored strings but
  removes another set of local owner/name struct literals from fact
  construction.
* Codex follow-up 2026-09-10: the remaining direct `field: FieldKey { ... }`
  test and signature dependency literals were migrated to `FieldKey::named(...)`.
  This is still string-keyed compatibility, not a first-class owner model, but
  it removes another raw owner/name construction spelling from the graph and
  external-pack test surfaces. Guard:
  `layering::declaration_context_usage_lookups_use_java_callable_keys`.
  Follow-up accessor pass: `facts::FieldKey` now has read accessors, and
  `inherited_members.rs`, `member_access_plan.rs`, `external_packs.rs`,
  `resolver_benchmark.rs`, and the external-field facade/static-constant helpers
  in `stdlib.rs` use them instead of reaching into `.field.type_name` /
  `.field.field_name` directly. The `facts::FieldKey` payload fields are now
  private, so non-owner code and integration tests must use the named
  constructors/accessors while the broader model migration remains open.
  Follow-up cleanup applied the same fence to fact-level `MethodKey` and
  `ConstructorKey`: their stored owner/member strings are private, call sites
  use accessors, and the one intentional method-name rewrite goes through
  `MethodKey::with_method_name(...)` instead of mutating a raw field.
* Follow-up 2026-09-09: signature dependency and propagation code no longer
  open-codes `MethodKey`/`ConstructorKey` bridges for resolved calls,
  resolved constructors, or usage constructor/method keys. These now use
  `MethodKey::resolved_call`, `MethodKey::from_usage_key`,
	  `ConstructorKey::resolved_constructor`, and `ConstructorKey::from_usage_key`,
	  making the declaring-owner plus resolved Rust-name policy explicit at the key
	  boundary instead of repeating the two-field copy throughout the solver.
	  Follow-up cleanup added `SignaturePlan::resolved_source_method_signature`
	  and moved the resolved-source `JavaCallableKey` construction out of call
	  argument planning; `call_plan` now asks the signature table by semantic
	  question instead of rebuilding owner/name/argument keys locally.
	  `SignaturePlan::resolved_source_method_return_shape` applies the same
	  boundary to expression-shape return recovery. The old unused public
	  `SignaturePlan::method_return_shape_by_java_key` shortcut was deleted, so
	  resolved return-shape callers use the semantic resolved-source helper
	  instead of a broad Java-key accessor.
	  Follow-up cleanup moved lowerer's declaration-scoped binding and signature
	  lookups off local `JavaCallableKey::source_method_for_*` construction:
	  current-method binding usage now goes through
	  `CallableAnalysis::source_method_usage_for_resolved_owner`, and emitted
	  source method signatures use `SignaturePlan::method_signature` with the
	  resolved `TypeDecl`/`MethodDecl`.
	  The remaining exact Java-key signature lookup is test-only/crate-local too;
	  internal regression checks can still assert the table contents without
	  leaving it as public crate API or normal production surface.
* Follow-up 2026-09-09: call-graph/fallibility `CallableKey` construction now
  has the same named boundary for resolved source calls and constructors.
  `CallableKey::resolved_method`, `CallableKey::resolved_constructor`,
  `CallableKey::method_identity`, and `CallableKey::constructor_identity`
  replaced the remaining production `CallableKey::Method`/`Constructor` struct
  literals in `usage.rs` and lowerer's callable-result queries; direct enum
  variants remain only in the helper definitions and pattern matches.
  Codex follow-up 2026-09-10: the public `usage::CallableKey` variants are
  `#[non_exhaustive]`. Internal analysis can still pattern-match the current
  enum while the real opaque-key model remains open, but downstream crates can
  no longer construct raw `CallableKey::Method { type_name, method_name }` or
  sibling struct-like variants directly. Public construction is limited to the
  remaining non-source surfaces such as external methods; source/resolved
  method and constructor identity constructors stay crate-local. Guard:
  `layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.
  This paragraph is about `usage::CallableKey`, the call-graph/fallibility
  identity key. It does not claim that `facts::CallableKey::resolved_method` or
  `facts::CallableKey::resolved_constructor` are crate-local; those remain
  public fact-query constructors by design, and
  `layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`
  intentionally guards that public boundary.
  Follow-up cleanup added public read-only query methods for call-graph
  assertions (`method_identity_parts`, `constructor_identity_parts`,
  `external_method_parts`, `external_constructor_parts`, `is_lambda`, and
  `anonymous_method_parts`) and moved the slot-shape integration tests off raw
  `new_java2rust::usage::CallableKey::* { ... }` field patterns. The same
  layering guard now rejects reintroducing those public raw enum-field reads in
  that integration-test surface.
  Follow-up cleanup also narrowed `CallableKey::method_identity` itself to
  crate-local. Integration tests that need source method keys now find existing
  call-graph nodes through `nodes()` plus `method_identity_parts()` instead of
  constructing owner/name identity from arbitrary strings.
  Follow-up cleanup also moved fallibility-invariant diagnostic rendering behind
  `CallableKey::diagnostic_label`, so that module no longer pattern-matches the
  public callable enum just to recover owner/name strings. The existing layering
  guard now covers that production module as well as its test fixtures.
  The same local fallibility surface now keeps `FallibilityViolation` caller and
  callee keys private-backed with read-only accessors, so downstream callers
  cannot construct or mutate violation identity by assigning raw key fields.
  Follow-up cleanup also made `usage::CallGraph`'s backing `nodes` and `edges`
  collections private. Read-only callers use `nodes()` / `edges()`, and tests
  that need synthetic graphs use `from_edges(...)` or `from_nodes(...)` instead
  of constructing the graph by assigning its raw collections.
  Follow-up cleanup extended that fence to `usage::CallGraphEdge`: caller,
  callee, and edge-kind payloads are private-backed, public callers use
  `caller()` / `callee()` / `kind()`, and synthetic tests construct edges with
  `CallGraphEdge::new(...)` instead of field literals.
  The same accessor fence now covers `signature::SignatureDependencyGraph`:
  public callers can inspect `nodes()` / `edges()`, but cannot mutate or
  fabricate the dependency graph by assigning its backing collections directly.
  `SignatureDependencyEdge` now has the matching private-backed payload:
  callers inspect `from()` / `to()` / `kind()` or construct via `new(...)`,
  rather than assigning graph endpoint fields directly.
  `SignatureDependencySccAnalysis` now follows that pattern too: components,
  successor indexes, node-to-component lookup, and stats are private backing
  state with read-only accessors instead of public mutable graph internals.
  `SignatureSolverPassPlan` now also exposes component order, cyclic/mutable
  component sets, and stats through read-only accessors rather than public
  mutable fields.
  `SignatureConstraintState` now follows the same fence for method,
  constructor, and field constraint maps: public callers can read through
  `methods()` / `constructors()` / `fields()` and build synthetic states through
  explicit insertion helpers, but cannot mutate or replace the backing maps by
  assigning raw key-indexed fields.
  `SignatureFixpointResult` also keeps that constraint state private-backed now;
  callers inspect it through `constraints()` instead of mutating the solved
  key-indexed tables through the result object.
  The result's dependency SCC analysis and solver pass plan are private-backed
  too, preserving their existing read-only accessors instead of exposing
  replaceable graph/planning internals.
* Follow-up 2026-09-09: fact-table callable keys also gained resolved-source
  helpers. `facts::CallableKey::resolved_method` now replaces the resolved-call
  owner/Rust-name reconstruction in call argument/facade planning, while
  `facts::CallableKey::resolved_constructor` is available as the constructor
  counterpart. External facade lookup keys still use the generic method factory
  because those inputs are external owner spellings, not resolver identities.
  Follow-up cleanup added `FactSet::facade_abi_for_external_method` and
  `FactSet::facade_abi_for_external_call` so straightforward external
  owner/name/arity facade lookups in call planning, method-reference shape
  planning, CLI facade extraction, and lowerer static-owner probes no longer
  rebuild temporary `CallableKey::method` values at each lookup site. Sites that
  still need a local key for source-name compatibility or primitive-fallback
  matching keep the explicit key for now.
  Method-reference lowering now also returns facade ABI candidates directly via
  `method_reference_facade_abi_candidates`, preserving candidate order while
  removing the temporary callable-key candidate list from the production helper.
  Follow-up cleanup moved storage-strategy implementor receiver-mutability
  checks from local `JavaOwnerKey::source_type(implementor)` construction to
  `UsageFacts::source_method_usage(implementor, method)`, so callers with a real
  source declaration use the owner-derived API boundary instead of rebuilding
  the Java owner key.
* Follow-up 2026-09-09: the remaining signature usage-constructor bridges and
  already-Rust-name lookups now use named constructors too.
  `ConstructorKey::from_usage_key` covers all constructor usage-key bridges in
  the solver, while `MethodKey::rust_identity` and
  `ConstructorKey::rust_identity` make by-Rust-name lookup and inherited method
  registration explicit instead of building local key literals.
* Follow-up 2026-09-09 test-surface cleanup: the remaining non-`usage.rs`
  `UsageMethodKey { ... }` fixture/setup literals now use
  `UsageMethodKey::current_rust_method_name(...)` when they intentionally seed
  Rust-name compatibility keys. `tests/layering.rs` now rejects
  `UsageMethodKey`/`UsageConstructorKey` struct literals in the lowerer,
  signature, slot-policy, callable-analysis, object-model module, and the
  integration-test files that had fixture literals, leaving raw key construction
  inside the key owner module only. Follow-up cleanup also removed direct
  `signature::MethodKey`, `signature::ConstructorKey`, and `facts::MethodKey`
  struct literals from the object-model, semantic-facts, and slot-shape
  integration-test surfaces. Those tests now use `rust_identity(...)`,
  `facts::MethodKey::named(...)`, or `facts::MethodKey::source_method(...)`, and
  the same layering test rejects reintroducing those raw literals. The
  signature module's own unit tests were also moved to those named constructors,
  leaving direct key literals in `src/signature.rs` only at the key definitions.
  A later pass did the same for direct `usage::CallableKey::Method` construction
  in the slot-shape integration tests; those call-graph/fallibility fixtures now
  inspect existing graph nodes through `method_identity_parts()` rather than
  constructing source method keys, and the layering guard rejects direct
  method-key literals on that integration-test surface. A follow-up pass moved
  `src/usage.rs`'s own source-key constructor regression off raw
  `UsageMethodKey { ... }` / `UsageConstructorKey { ... }` expected values too;
  it now compares constructor output to the named `current_*` constructors, so
  direct usage-key literals remain only inside the key constructors themselves.
* Follow-up 2026-09-07: ownerless callable-slot construction is crate-local.
  `JavaCallableSlotKey::source_method` still supports internal inherited-method
  and trait-slot comparisons, but external callers stay on owner-bearing
  `JavaCallableKey` constructors instead of manufacturing ownerless
  compatibility identities.

* Follow-up 2026-09-06: external static string constant support now suppresses
  simple compatibility type aliases that would map one Rust alias to multiple
  emitted owners. Package peers such as `a.Item.KIND` and `b.Item.KIND` still
  emit/use `AItem::KIND` and `BItem::KIND`, but no longer also emit colliding
  `pub type Item = AItem;` / `pub type Item = BItem;` aliases. Regressions:
  `batch_translation::package_qualified_source_type_references_keep_package_identity`
  and
  `batch_translation::stream_static_constant_support_keeps_package_qualified_owner_identity`.
* Follow-up 2026-09-07: inferred platform `ExecutorService`/`Future` support no
  longer emits the built-in `Future<T>` facade when the translated source
  program already declares a local `Future` type. A bare source `Executor.submit`
  fixture now keeps its source `Future` class instead of colliding with the
  compatibility JDK facade. Regression:
  `idiom_gaps::translates_executor_submit_block_lambda_as_closure`.
* Follow-up 2026-09-07: package-qualified class-literal token planning now
  prefers the emitted package-qualified source owner when that is the available
  storage fact. A merged-source `a.Item.class` with storage facts under
  `AItem`, or a simple `Item` compatibility fact for a package source, now
  plans the token as `JavaClass<AItem>` rather than drifting to `JavaClass<Item>`.
  Regression: `slot_shape_design::class_literal_tokens_are_planned_before_lowering`.
* Follow-up 2026-08-30: source-summary superclass boundary facts are still
  generated as corpus-analysis erased-boundary facts, but those facts no longer
  replace or shadow a parsed source type representation. This keeps a
  cotranslated `VCFCompoundHeaderLine` concrete with `ParsedSource` provenance
  even when source-summary metadata also names it as a boundary. Regression:
  `slot_shape_design::source_summary_boundary_erasure_does_not_replace_parsed_source_type`.
* Representation lowering now resolves unqualified `extends`, `implements`, and
  source type-parameter bound names in the declaring type's package before
  falling back to emitted-alias or unique-simple-name lookup. A package peer
  pair such as `a.Base`/`b.Base` no longer makes `b.Child extends Base` fail
  closed in representation/interface/bound analysis just because the simple
  name is globally ambiguous. Regressions:
  `representation_lowering::type_lookup_tests::source_type_extends_uses_same_package_before_ambiguous_simple_peer`,
  `representation_lowering::type_lookup_tests::implemented_interfaces_use_same_package_before_ambiguous_simple_peer`,
  and
  `representation_lowering::type_lookup_tests::method_type_param_bounds_use_owner_package_before_ambiguous_simple_peer`.
* Generic source type-bound lookup no longer accepts the first emitted Rust
  alias match when multiple Java owners collapse to the same Rust name. Exact
  Java-owner lookup still wins, but the Rust-alias tier now requires uniqueness
  like the simple-name tier, so an emitted alias such as `AB_C` cannot silently
  select whichever of `a.B.C` or `AB.C` appears first. Regressions:
  `item_assembly::tests::source_type_bound_decl_refuses_ambiguous_simple_package_peer`
  and
  `item_assembly::tests::source_type_param_into_supertype_bounds_refuses_ambiguous_simple_package_peer`.
* FactSet lookup fallbacks for field nullability, trivial accessors, external
  static field values, and external constructors now refuse ambiguous
  package-simple owner aliases, matching the earlier `external_field_for`
  behavior. Exact/direct owners still resolve, but querying `Item` no longer
  selects the last matching fact when both `a.Item` and `b.Item` publish the
  same member/constructor shape. Regression:
  `semantic_facts::fact_lookup_refuses_ambiguous_simple_package_owner_aliases`.
  Follow-up 2026-08-30: unsupported external constructor and unsupported
  external API/field facts now use the same ambiguity guard before falling
  through to broad owner matching. Exact `a.Item`/`b.Item` unsupported facts
  still resolve, while bare `Item` fails closed instead of selecting whichever
  package peer appears last. The same regression covers these fact families.
  The same pass also moved external API argument-effect facts onto a shared
  exact/direct-then-guarded-broad lookup helper, including the private indexed
  path used while matching external API parameter facts. Concrete arity still
  outranks generic arity, receiver-specific facts still outrank receiverless
  facts, and user overrides still occupy their existing precedence tier.
  External API effect facts now use the same direct-or-guarded-broad owner
  check in both ordinary receiver lookup and the parameter-aware call lookup, so
  `Item.transform(...)` no longer selects between `a.Item` and `b.Item` facts by
  insertion order while exact owners continue to resolve.
  Source equals-override facts and dispatch-requirement facts now follow the
  same rule: exact/direct owners win, unique simple-owner compatibility remains,
  and ambiguous bare package-simple owners fail closed instead of merging or
  selecting requirements from multiple package peers. The MethodKey-backed fact
  families now follow the same policy as well: method effect, control-flow,
  recursion, and catch facts resolve exact `a.Item.work`/`b.Item.work` keys, but
  a bare `Item.work` query no longer borrows whichever package peer happens to
  match the simple owner alias. Facade ABI call lookup now uses the same split:
  exact qualified callable keys still select their ABI, while a bare owner such
  as `Item.work(String)` is rejected when both `a.Item` and `b.Item` publish the
  same callable header. Exception-kind lookup also resolves exact
  `a.Problem`/`b.Problem` facts but rejects a bare `Problem` when the package
  peers disagree on checked/unchecked classification. External-library pack
  discovery from member facts now requires a direct/canonical member owner
  match, so a pack that owns `a.Item` is not inferred for `other.Item` through
  the shared simple alias `Item`.
* Resolved anonymous interface constructors now resolve the declaring interface
  owner through the local import/static-alias table and combined context lookup
  before rendering the trait object constructor. `import a.I; new I() { ... }`
  no longer falls back to `<dyn I>::anonymous()` when `a.I`/`b.I` peers are
  both visible; it renders the declaration-backed trait name such as
  `<dyn AI>::anonymous()`. Regression:
  `lower::tests::resolved_anonymous_interface_constructor_uses_import_alias_owner`.
* §4 and §5 are fixed for declarations in one generated `impl`: after the
  normal overload name is computed, declared methods are allocated in the Rust
  inherent namespace with constructor names pre-occupied. Regressions:
  `source_methods_with_colliding_snake_case_names_get_distinct_rust_names` and
  `source_overloads_with_colliding_type_suffixes_get_distinct_rust_names`.
* §6 is fixed by teaching the nested-name qualifier descendant aliases such as
  `P.Counter -> W.P.Counter`; regression:
  `partially_qualified_nested_type_reference_uses_declared_source_type`.
* §7 is stale in this tree: `boxed_dyn_trait_projection` now calls
  `normalize_emitted_java_type_name`, and `NestedInterfaceDispatch.java` covers
  the boxed nested-interface path.
* §2 is fixed for nested source types. Type-to-Rust mangling now preserves
  adjacent type path boundaries (`K.AB -> K_AB`, `K.A.B -> K_A_B`) while keeping
  package-like lowercase segments compact (`java.io.InputStream ->
  JavaIoInputStream`). Nested convenience aliases are emitted only when unique
  in the generated file's flat Rust namespace. Regressions:
  `non_injective_nested_type_mangling_distinguishes_path_boundaries`,
  `duplicate_nested_simple_type_aliases_are_suppressed`, plus the unit-level
  `nested_type_name_mangling_preserves_type_path_boundaries`.
* §3b's import-specific static owner drift is fixed for the local alias table:
  explicit imports are seeded authoritatively, while support/context imports and
  context type aliases no longer overwrite them. Regression:
  `explicit_import_aliases_survive_duplicate_context_simple_names`. Broader
  package identity remains open at the model level even though the known batch
  and co-translated-context same-simple-name regressions are now covered below.
* §3a's explicit package-qualified source references are fixed for ordinary
  source AST type positions and constructor targets. `a.Item ai = new a.Item()`
  and `b.Item bi = new b.Item()` now lower to `AItem`/`BItem` slots and
  constructors, while `a.Item.KIND`/`b.Item.KIND` continue to resolve to the
  package-specific static owners. Regression:
  `package_qualified_source_type_references_keep_package_identity`.
* §3a's package-qualified source class literals now keep package identity in
  both the Java class name and token type. `a.Item.class` lowers as
  `JavaClass::<AItem>::literal("a.Item")` and `b.Item.class` lowers as
  `JavaClass::<BItem>::literal("b.Item")`, including the merged-batch case
  where the first same-simple-name source type is stored under the simple key.
  Regressions: `parses_package_qualified_class_literal_type_identity`,
  `class_literal_tokens_are_planned_before_lowering`, and
  `package_qualified_class_literals_keep_package_identity`.
* Same-package simple class literals now keep the Java package name without
  forcing the Rust token type through the package-qualified mangler.
  `package a; class Item { Item.class.getName(); }` lowers as
  `JavaClass::<Item>::literal("a.Item")`, and the generated fallback
  `get_class()` returns the same Java name. Regression:
  `same_package_simple_class_literal_keeps_java_package_name`.
* §3a is also fixed for same-simple-name package peers emitted in a batch:
  duplicate top-level primary type declarations are qualified before the shared
  batch context is built, ambiguous simple static-owner aliases are suppressed,
  and package-qualified source constants resolve through the qualified owner
  instead of falling back to the first simple-name match. The strengthened
  `package_qualified_source_type_references_keep_package_identity` regression
  now checks both declaration sites (`pub struct AItem` / `pub struct BItem`)
  and use sites (`AItem::new`, `BItem::new`, `AItem::KIND`, `BItem::KIND`).
* The same duplicate-program pre-pass now includes co-translated context source
  programs before they are merged into the single-source context. Regression:
  `cotranslated_same_simple_name_package_peers_keep_package_identity`.
* Context type lookup now refuses ambiguous simple-name matches instead of
  returning the first package peer. Exact/package-qualified and emitted Rust-name
  matches still win; a unique simple-name fallback still works. Regression:
  `context_type_lookup_refuses_ambiguous_simple_package_peer`.
* Emitted-context type lookup now follows the same rule. It also refuses
  ambiguous simple-name package peers after exact emitted-name/package-qualified
  matches have had a chance to win. Regression:
  `emitted_context_type_lookup_refuses_ambiguous_simple_package_peer`.
  Follow-up 2026-09-07: Rust-qualified package-looking queries now also fail
  closed after exact/alias lookup misses. A query such as `a::Item` no longer
  degrades to a simple `Item` fallback that can select `b.Item` when it is the
  only visible package peer, while `Outer::Inner` nested-type compatibility is
  preserved. Regression:
  `emitted_context_type_lookup_refuses_wrong_package_path_simple_fallback`.
* Update 2026-08-25: the emitted-context ambiguity check now spans all three
  programs visible to lowering (`program`, `support_local_type_program`, and
  `source_summary_type_program`). A unique simple-name fallback is still
  accepted, but same-simple package peers split across those programs no longer
  return whichever program was searched first. Regression:
  `emitted_context_type_lookup_refuses_cross_program_ambiguous_simple_package_peer`.
* Imported source enum `valueOf(String)` calls now keep package owner identity
  when same-simple-name package peers are present. The static `valueOf` lowering
  path resolves the owner through the existing source/context alias lookup
  before rendering the Rust associated-call owner, instead of using the raw
  receiver spelling. The same regression also caught an adjacent defect in the
  duplicate-type pre-pass: it renamed `TypeDecl.name` from `Mode` to `a.Mode`
  without rewriting the declarations and expressions inside that type, so enum
  constants still had type `Mode` and the emitted `AMode` enum had no variants.
  The pre-pass now rewrites internal type references under the renamed type.
  Compile-backed regression:
  `imported_source_enum_value_of_keeps_package_owner_identity`.
* Instance-method overload retargeting no longer resolves a bare same-simple
  owner by searching `program`, then `support_local_type_program`, then
  `source_summary_type_program` and accepting the first match. The helper now
  uses the cross-program ambiguity-aware context lookup, so `Item.work()` fails
  closed when `a.Item` and `b.Item` are both visible, while exact owners
  (`a.Item`, `AItem`, `b.Item`, `BItem`) still resolve. Regression:
  `lower::tests::source_declared_instance_method_lookup_refuses_cross_program_ambiguous_owner`.
* Source object `toString()` fallback now uses the same cross-program
  ambiguity-aware context lookup instead of searching emitted/support context
  before summary context. A bare `Item.toString()` fallback no longer picks
  whichever same-simple package peer appears first; it declines the erased
  fallback when `a.Item` and `b.Item` are both visible. Regression:
  `lower::tests::object_to_string_fallback_refuses_cross_program_ambiguous_simple_package_peer`.
* Follow-up 2026-09-07: source-summary receiver return-shape recovery no longer
  bypasses the combined context lookup with a summary-program-only fallback. A
  local receiver typed as bare `Item` now fails closed when support context
  contains `a.Item` and source-summary context contains `b.Item`; exact
  `b.Item` receiver types still recover the summary return shape. Regression:
  `lower::tests::local_receiver_summary_return_shape_refuses_ambiguous_context_owner`.
* Follow-up 2026-09-07: emitted source super-boundary lowering no longer
  classifies a support superclass as source-summary just because the
  source-summary program also contains the same owner key. The summary handoff
  now requires the combined context lookup to select the summary program, so a
  support `a.Base` keeps the emitted `ABase::m(...)` path even when summary
  metadata also names `a.Base`. Regression:
  `lower::tests::emitted_source_super_boundary_call_does_not_borrow_summary_peer_for_support_super`.
* Follow-up 2026-09-07: object-array default-derive obligations keep package
  peers distinct. A `new a.Item[1]` allocation still obligates `a.Item`, but its
  recorded simple-name compatibility entry no longer marks `b.Item` when both
  package peers are present. Unique simple-name compatibility remains accepted.
  Regression:
  `array_default_obligations::tests::package_qualified_object_array_default_obligation_does_not_mark_package_peer`.
  Follow-up cleanup made the `array_default_obligations` module private, kept
  `program_allocates_object_array_of` crate-local, and made the raw
  `object_array_element_type_names` collector private. Item assembly can still
  ask the obligation question, but downstream users cannot treat the
  simple-name compatibility collector as public owner-identity API.
* Follow-up 2026-09-07: fact-backed external static constant planning now
  renders the owner from the exact external field fact instead of stripping the
  queried receiver to its simple name after lookup. Package peers such as
  `a.Item.KIND` and `b.Item.KIND` therefore plan as `AItem::KIND` and
  `BItem::KIND` instead of collapsing to `Item::KIND`. Regression:
  `type_coercion::member_access_plan_preserves_package_qualified_external_static_constant_owner`.
* Follow-up 2026-09-06: fallback Rust-name adapters in `call_plan` are
  crate-local. `apply_fallback_receiver_rust_name`,
  `apply_fallback_self_rust_name`, and `apply_fallback_static_owner_rust_name`
  remain available to lowering but are no longer public APIs; their overload
  boundary behavior moved to a `call_plan` unit test and a layering guard pins
  the boundary.
  Follow-up cleanup also narrowed `external_call_receiver_type_key` to
  crate-local; its callers are semantic-analysis internals, and public users
  should not depend on a rendered external receiver-key recovery helper.
  The adjacent `external_call_returns_rust_result_for_receiver` wrapper is
  crate-local too; lowering can ask the question internally without exporting
  another string receiver-key API.
  `external_constructor_returns_rust_result` is likewise crate-local because
  its only caller is constructor lowering.
  The overload-modeled probes `external_receiver_overload_is_modeled` and
  `external_constructor_is_modeled` are crate-local too; they remain available
  to lowering without exporting exact receiver/type-name modeling probes as
  public API.
* Static-storage accessor spellings no longer re-derive a snake-case name after
  the static field allocator has selected an emitted Rust field name. The
  accessor projection now treats its input as the already allocated member name,
  and `static_storage_accessors_for_field` no longer emits snake-case alias
  getters/setters beside the allocated pair. Regressions:
  `rendered_string_heuristics_have_centralization_guardrails`,
  `lower::tests::static_storage_reference_uses_resolved_context_owner_identity`,
  and `source_static_final_byte_constants_preserve_declared_slot_type`.
  Follow-up 2026-08-28: the byte-constant regression was still listed in the
  `idiom_gaps` expected-failure baseline even though the current lowering
  preserves `i8` constants and emits owner-qualified constant reads. The stale
  `Self::` assertion was updated to `ByteConstantOwner::`, and the test was
  removed from the baseline after the focused check passed.
* Exact source static field names now outrank alias fallback in the static
  semantic/access-name/owner maps. The previous maps let an earlier alias claim
  a later exact field spelling, so `static int self; static int self__;` could
  emit distinct accessors but still assign both writes through
  `__set_static_self___`. The owner lookup also now prefers the current type's
  typed static-storage table before consulting the flat owner map, because two
  different impls can legitimately both have an emitted `__static_self__`
  accessor. Regressions:
  `qualified_static_storage_keyword_fields_do_not_collapse`,
  `bare_static_storage_keyword_alias_does_not_overwrite_exact_field`,
  `nested_static_storage_exact_owner_beats_enclosing_alias`, and
  `qualified_static_final_keyword_consts_do_not_collapse`.
* Signature-plan simple owner lookup now fails closed before method filtering
  when a bare owner query is ambiguous across package peers. A query such as
  `Item.who()` no longer resolves to `b.Item` merely because `a.Item` lacks
  `who`; exact `a.Item`/`b.Item` queries still work. Regression:
  `signature::tests::method_signature_presence_fails_closed_for_ambiguous_simple_package_owner`.
* Receiverless null-overload ambiguity detection now uses the same combined
  context lookup for the current source type instead of searching program,
  support and summary contexts in priority order. A bare current type `Item`
  split across `a.Item` and `b.Item` no longer reports an ambiguous `m(null)`
  call from whichever context appears first; exact `a.Item` still reports the
  real overload ambiguity. Regression:
  `lower::tests::current_null_overload_check_refuses_ambiguous_context_owner`.
* Receiverless source-summary inherited return-shape lookup now resolves the
  current type through the same combined context lookup before walking inherited
  summary methods. Bare `Child.value()` no longer recovers a return shape from
  the first `Child` package peer when another context peer exists; exact
  `a.Child` still recovers the inherited shape. Regression:
  `lower::tests::receiverless_summary_return_shape_refuses_ambiguous_current_owner`.
* Static singleton initializer lowering now resolves the current type through
  the same combined context lookup instead of a program/support priority chain.
  A bare `Holder.INSTANCE` no longer lowers to `Config::new()` from whichever
  context peer is searched first when `a.Holder` and `b.Holder` are both
  visible; exact `a.Holder` remains accepted. Regression:
  `lower::tests::static_singleton_initializer_refuses_ambiguous_current_owner`.
  Follow-up cleanup: this current-owner lookup now resolves through
  `SourceContextOwnerKey` as well. Guard:
  `layering::current_owner_utility_lookups_stay_wrapped_until_context_lookup`.
* Unsupported source-constructor call handoff now resolves the constructed
  source type through the combined context lookup. A bare `new Item()` no longer
  emits the unsupported-super-constructor handoff from `a.Item` when another
  context peer `b.Item` is also visible; exact `new a.Item()` still emits the
  handoff. Regression:
  `lower::tests::unsupported_source_constructor_handoff_refuses_ambiguous_context_owner`.
* Nested enum static-field owner candidates now resolve the current owner
  through the combined context lookup before adding nested enum aliases. A bare
  ambiguous `Holder` no longer contributes `a.Holder.Mode` as a candidate for
  `mode`, while exact `a.Holder` still does. Regression:
  `lower::tests::enum_static_field_owner_candidates_refuse_ambiguous_current_owner`.
  Follow-up cleanup: the nested enum static-field and current-type enum alias
  helpers now perform that current-owner lookup through
  `SourceContextOwnerKey::new(current_type).find_context_type(ctx)` instead of
  reopening the raw `find_context_type_by_key_in_lower_ctx(ctx, current_type)`
  call locally. Guard:
  `layering::enum_current_owner_lookup_stays_wrapped_until_context_lookup`.
* Source constructor lowering for a concrete destination slot now uses the
  combined context lookup for the constructed type guard. `new Item()` fails
  closed when `Item` is ambiguous across `a.Item` and `b.Item`, while exact
  `new a.Item()` from support context still lowers to `AItem::new()`.
  Regression:
  `lower::tests::source_constructor_concrete_slot_refuses_ambiguous_context_owner`.
* Current-type field shape helpers no longer search program/support/summary
  contexts in priority order. Both ordinary current field shape recovery and
  static-final const-array field type recovery now use the combined
  ambiguity-aware context lookup, so a bare current type `Item` split across
  package peers fails closed instead of borrowing the first peer's field shape.
  Regressions:
  `lower::tests::current_type_field_shape_refuses_ambiguous_context_owner` and
  `lower::tests::static_final_const_array_field_type_refuses_ambiguous_current_owner`.
  Follow-up cleanup: both current-field helpers now route the current-owner
  context lookup through `SourceContextOwnerKey` before asking for field
  details, instead of reopening the raw current-type string lookup locally.
  Guard: `layering::current_field_owner_lookup_stays_wrapped_until_context_lookup`.
  The same wrapper-boundary cleanup was applied to current source field-name
  recovery (`current_type_source_instance_field_rust_name`), with behavior still
  covered by
  `lower::tests::current_type_source_field_name_refuses_cross_context_simple_owner`.
  Follow-up cleanup: the remaining direct `ctx.current_type?` context lookups
  in source parameter shape recovery and `this` receiver generic return-shape
  recovery now also route through `SourceContextOwnerKey::find_context_type`.
  This keeps the current owner wrapped at the lookup boundary without changing
  the broader string-owner model. Guard:
  `layering::current_owner_utility_lookups_stay_wrapped_until_context_lookup`.
* Source-summary receiver method return-shape recovery no longer checks
  `source_summary_type_program` directly before the combined context lookup. A
  `this.value()` call under a bare ambiguous current type no longer borrows the
  summary peer's method shape just because the support/program peers do not
  carry that method; exact package-qualified owners still recover the summary
  shape. Regression:
  `lower::tests::receiver_summary_return_shape_refuses_ambiguous_current_owner`.
* Source Java enum type-declaration lookup no longer walks
  program/support/summary and accepts the first simple-name or rendered-alias
  match. Current-type enum lookup and Java-type enum lookup now both use the
  combined context key lookup before requiring `TypeKind::Enum`, so bare
  same-simple package peers fail closed while exact and unique emitted aliases
  still resolve. Regression:
  `lower::tests::source_enum_type_decl_lookup_refuses_cross_program_ambiguous_simple_owner`.
  Follow-up cleanup: the current-type enum lookup helper now uses
  `SourceContextOwnerKey` for that context lookup boundary as well. Guard:
  `layering::enum_current_owner_lookup_stays_wrapped_until_context_lookup`.
* Signature-plan required method/field fallbacks no longer cross distinct
  qualified owners after an exact lookup misses. The compatibility fallback for
  a unique bare `Item` owner remains, but `a.Item::who` and `a.Item::value` no
  longer borrow the sole `b.Item` method or field plan merely because the final
  simple segment matches. Regressions:
  `signature::tests::required_method_signature_does_not_borrow_distinct_package_peer`
  and
  `signature::tests::required_field_shape_does_not_borrow_distinct_package_peer`.
* This does not close the broader package-identity item: generated support
  stubs for separately emitted context peers are still not compiled as part of
  the single-source helper, and type declaration identity still needs a
  first-class package-aware key instead of the current duplicate pre-pass.

Line numbers are from the working tree at audit time; prefer the symbol names.
Every repro in this file is staged under `repro/` next to it.


[0009](../../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md)
closed on *methods*: `JavaCallableSlotKey` now carries `(java_name,
erased_param_types, kind, staticness)` and the override/inherit graph runs on it.
That work assumed the surrounding identity — **which type** a slot belongs to,
and **what Rust identifier** the slot is finally spelled as — was already sound.
It is not. This file is about that surrounding identity, and its companion
[0065](done/0065-the-overload-suffix-is-scoped-to-one-file-and-one-level-of-inheritance.md)
is about the one piece of 0009's own scheme that is still scoped wrongly.

---

## 1. The shape of the problem

Java identity for a nameable entity is

```
(package, enclosing-type chain, simple name, [parameter types], kind)
```

Rust identity for an item is `(module, identifier)`; for an inherent method it is
`(type, identifier)`. Going from the first to the second is a *function*, and for
the translation to be correct that function must be

* **total** — defined for every Java entity, including one referred to by a
  partially-qualified spelling;
* **injective** — two distinct Java entities never land on one Rust identifier;
* **single-valued** — the *declaration site* and every *reference site* compute
  the same answer for the same entity.

Today it is none of the three, and the reason is structural: the map is
implemented as a handful of **pure `&str -> String` functions** that see only the
spelling of the name in front of them. They have no access to the package, no
access to the enclosing-type chain, and no access to what identifiers are already
taken. Injectivity is not something such a function can have.

### What exists today, per entity kind

| Entity | Function | Sees | Injective? |
|---|---|---|---|
| local binding | `java_naming::source_binding_rust_names` → `allocate_rust_ident` | all names in scope, **occupancy set** | **yes** |
| instance field | `java_naming::source_instance_field_rust_names` | all fields of the type, **occupancy set** | **yes** (within a type) |
| static field | `java_naming::source_static_field_rust_names` | all static fields of the type, **occupancy set** | **yes** (within a type) |
| method | `parser::to_snake_case` + `parser::type_suffix` suffix, then `resolve::allocate_unique_declared_method_rust_names` | declaring type namespace and occupancy set for declared methods; string-owner compatibility elsewhere | **locally guarded, not first-class** (§3, §4) |
| type | `java_naming::rust_name_for_java_named_type` plus duplicate/source-context pre-passes and ambiguity guards | dotted spelling plus local context in many lookups; no typed owner key | **locally guarded, not first-class** (§2, §5) |

The first version of this audit found that only fields and locals used a
collision-avoiding allocator. The current tree has since added a local method
allocator and many package-ambiguity guards, but the type/method identity model
is still expressed through strings at many boundaries instead of one resolved
entity map.

Codex follow-up 2026-09-12 field-name allocator cleanup: the instance/static
field rows above still expose separate crate-local entry points because callers
ask different semantic questions, but both now delegate to one private
`source_field_rust_names(...)` allocator. That keeps the keyword/user-suffix
collision policy single-valued for fields while preserving separate static and
instance namespaces. Guards:
`java_naming::tests::source_field_allocators_share_keyword_collision_policy`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.
The singular accessors used by lowering call sites are pinned by
`java_naming::tests::source_single_field_accessors_share_keyword_collision_policy`
so they cannot drift back to allocating against an empty occupancy set.

`allocate_rust_ident` (`src/java_naming.rs`) remains the right primitive: it
takes a base name and an occupancy set and appends `_` until free. The remaining
gap is where that allocation result is not yet the only source of truth.

### The key that would fix it exists, and is not the key anything is stored under

`src/java_signature.rs` defines `JavaCallableKey { owner, java_name,
erased_param_types, kind, staticness }`. Meanwhile the maps that lowering
actually indexes are still string-keyed:

```rust
// src/facts.rs
pub struct MethodKey { type_name: String, method_name: String }   // Java name
// src/signature.rs / src/usage.rs
// Method keys now hide their fields behind constructors/accessors, but still
// carry owner/name strings rather than one first-class owner key.
```

The live state is now private-backed, and helper APIs that only serve internal
renaming are crate-local, but it still keys owner identity on type-name strings
in multiple surfaces. `src/lower.rs` also still has many direct `.name ==`
comparisons. Some are harmless syntax checks, but the type system cannot
distinguish them.

---

## 2. Type mangling path-boundary collisions — fixed in the current tree

This section's original repro said type mangling was effectively
`join(path_parts, "")`, so `K.AB` and `K.A.B` both became `KAB`. That is now
historical. `src/java_naming.rs` preserves adjacent type-looking path
boundaries, so those examples render as `K_AB` and `K_A_B`.

The simple-alias half is also guarded: duplicate nested simple aliases are
suppressed instead of emitting multiple `pub type Inner = ...` items into the
same Rust namespace.

Regressions:

* `java_naming::tests::nested_type_name_mangling_preserves_type_path_boundaries`
* `non_injective_nested_type_mangling_distinguishes_path_boundaries`
* `duplicate_nested_simple_type_aliases_are_suppressed`

The audit remains open for the broader package/type identity model, not for
this nested path-boundary collision.

---

## 3. The package is not part of a source type's identity at all

`rust_name_for_java_named_type` never sees a package, because a source
`TypeDecl.name` never contains one. Two classes with the same simple name in
different packages are one Rust type.

### 3a. Both emit `pub struct Item`

```java
// a/Item.java        package a; public class Item { public static final int KIND = 1; public int who() { return 1; } }
// b/Item.java        package b; public class Item { public static final int KIND = 2; public int who() { return 2; } }
// Main.java
public class Main { public static void main(String[] a) {
    a.Item ai = new a.Item();  b.Item bi = new b.Item();
    System.out.println(ai.who() + "," + bi.who() + "," + a.Item.KIND + "," + b.Item.KIND);
} }
```

`java Main` → `1,2,1,2`. `translate-batch` emits `pub struct Item` in *both*
`00002_Item.rs` and `00003_Item.rs`, and in `00001_Main.rs`:

```rust
let ai: Item = Item::new();
let bi: Item = Item::new();
println!("{}", format!("{},{},{},{}", (ai.who()).to_string(), (bi.who()).to_string(),
                       (a.Item.k_i_n_d).to_string(), (b.Item.k_i_n_d).to_string()));
```

Three separate collapses in three lines: the two distinct types became one name;
`a.Item.KIND` was emitted as raw dotted Java (`E0425: cannot find value a`); and
its field was snake-cased to `k_i_n_d` while the declaration site emits `pub
const KIND: i32 = 1;`.

Also worth recording: the class literal loses the package too —
`JavaClass::<Cfg>::literal("Cfg")` where `a.Cfg.class.getName()` is `"a.Cfg"`.

Note the asymmetry with *external* types, which **do** get their package folded
in: the same output contains `pub type OrgDisqBioDisqBaiWriteOption =
BaiWriteOption;` and `pub struct ACfg;`. So the translator has a
package-qualified naming function; it just does not apply it to types whose
source it is reading.

### 3b. A same-simple-name lookup ignores the import and takes the first match

This one is the silent-wrong-answer version.

```java
// a/Cfg.java   package a; public class Cfg { public static final int N = 1; public static int who() { return 1; } }
// b/Cfg.java   package b; public class Cfg { public static final int N = 2; public static int who() { return 2; } }
// UseA.java    import a.Cfg;  public class UseA { public static int v() { return Cfg.N + Cfg.who(); } }
// UseB.java    import b.Cfg;  public class UseB { public static int v() { return Cfg.N + Cfg.who(); } }
// Main2.java   prints UseA.v() + "," + UseB.v()
```

`java Main2` → `2,4`.

```rust
// 00002_UseA.rs   -- the file that imports a.Cfg
pub type Cfg = ACfg;
...
pub fn v() -> i32 { return (BCfg::N).wrapping_add(Cfg::who()); }

// 00003_UseB.rs   -- the file that imports b.Cfg
pub fn v() -> i32 { return (BCfg::N).wrapping_add(Cfg::who()); }
```

Both files emit **`BCfg::N`**. Swapping the two `import` lines changes nothing —
re-run with `UseA` importing `b.Cfg` and `UseB` importing `a.Cfg` and both still
emit `BCfg::N`, while the `pub type Cfg = ACfg;` alias moves to the other file.
So within one generated file, the type alias respects the import and the constant
read does not, and neither is asked which package the reference is in.

Root cause anchor:

```rust
// src/lower.rs:32186
fn context_type_key_matches(candidate: &str, query: &str) -> bool {
    if rust_name_for_java_named_type(candidate) == query { return true; }
    ...
    let candidate_simple = candidate.rsplit(['.', '$']).find(|s| !s.is_empty()).unwrap_or(candidate);
    let query_simple     = query.rsplit(['.', '$']).find(|s| !s.is_empty()).unwrap_or(query);
    canonical_external_receiver_type_key(candidate_simple) == canonical_external_receiver_type_key(query)
        || canonical_external_receiver_type_key(candidate_simple) == canonical_external_receiver_type_key(query_simple)
}
```

`find_context_type_by_key` (`src/lower.rs:32050`) uses `find_map` over the
program's types, so the **first** simple-name match wins; there is no ambiguity
signal and no notion of the referring file's imports or package.

This is worth contrasting with `source_constant_values::ConstantScope`
(`src/source_constant_values.rs:154`), which was fixed for exactly this problem
and gets it right: fully-qualified wins, then package-relative, then
unambiguous-or-refuse. That discipline exists in one module and governs constant
*folding*; the *emission* path above never consults it.

---

## 4. Two Java method names that snake_case to one Rust name

```rust
// src/parser.rs:3418
pub fn to_snake_case_unescaped(name: &str) -> String {
    ...
    } else if ch.is_ascii_uppercase() {
        if idx > 0 && !out.ends_with('_') && !out.ends_with('.') { out.push('_'); }
        out.push(ch.to_ascii_lowercase());
    } else { out.push(ch); }
}
```

An existing `_` suppresses the inserted one, so `xY` and `x_Y` both become `x_y`.

```java
public class A {
    public int xY(int a)  { return a + 1; }
    public int x_Y(int a) { return a + 2; }
    public static void main(String[] args) {
        A a = new A();
        System.out.println(a.xY(1) + "," + a.x_Y(1));
    }
}
```

`java A` → `2,3`. Generated:

```rust
impl A {
    pub fn x_y(&self, a: i32) -> i32 { return (a).wrapping_add(1); }
    pub fn x_y(&self, a: i32) -> i32 { return (a).wrapping_add(2); }
    pub fn main(args: Vec<String>) {
        let a: A = A::new();
        println!("{}", format!("{},{}", (a.x_y(1)).to_string(), (a.x_y(1)).to_string()));
    }
}
```

`error[E0592]: duplicate definitions with name x_y`. (`getURL` vs `getUrl` do
*not* collide — `get_u_r_l` vs `get_url` — the collision is specifically between
a camel boundary and an explicit underscore, which is the common style clash in a
codebase that mixes conventions.)

Current tree status: fixed for methods declared in the same source type. Method
lowering still starts with the existing Java-name/overload-name scheme, but
`allocate_unique_declared_method_rust_names` now allocates the final names inside
the Rust inherent namespace, so `xY` keeps `x_y` and `x_Y` becomes `x_y_`; call
resolution sees the assigned names through `ProgramSymbols`.

---

## 5. The overload suffix is built from *type spelling*, and is also non-injective

```rust
// src/parser.rs
pub(crate) fn type_suffix(ty: &JavaType, is_varargs: bool) -> String {
    match ty {
        JavaType::Array(inner) => format!("{}_array", type_suffix(inner, false)),
        JavaType::Named { name, .. } => to_snake_case(&java_type_suffix_name(name)),
        ...
```

so `int[]` → `int_array` and a class named `IntArray` → `int_array`.
Follow-up 2026-09-10: `type_suffix` is crate-local now. The overload suffix
model remains non-injective until names are allocated from resolved identities,
but external callers can no longer treat the spelling helper as public API.

```java
class IntArray { int n = 9; }
public class Z2 {
    static int g(int[] a)     { return a.length; }
    static int g(IntArray a)  { return a.n; }
    public static void main(String[] args) {
        System.out.println(g(new int[3]) + "," + g(new IntArray()));
    }
}
```

`java Z2` → `3,9`. Generated:

```rust
impl Z2 {
    pub fn g_int_array(a: &IntArray) -> i32 { return a.length; }   // was g(int[])
    pub fn g_int_array(a: &IntArray) -> i32 { return a.n; }
    pub fn main(args: Vec<String>) {
        println!("{}", format!("{},{}",
            (Self::g_int_array(vec![0; 3 as usize].as_slice())).to_string(),
            (Self::g_int_array(IntArray::new())).to_string()));
    }
}
```

`error[E0592]: duplicate definitions with name g_int_array` — and note the array
overload's **parameter type** was also rewritten to `&IntArray` (`E0609: no field
length on type &IntArray`), so the collision is not confined to the identifier:
one slot's signature overwrote the other's.

`java_type_suffix_name` (`src/parser.rs:3452`) maps `.`, `$`, `<`, `>`, `,`, `?`
and whitespace all to `_`, so `f(Outer.Inner)` and `f(OuterInner)` are the same
suffix by the same mechanism.

Current tree status: fixed for same-type overload declarations. The suffixes can
still collide, but the post-pass allocator now gives the second declared method a
distinct Rust name (`g_int_array_`) and call resolution uses that assigned name.
This is a local fix, not the full type-identity allocator proposed below.

---

## 6. A partially-qualified nested reference is never qualified, and becomes an "unmodelled import"

`P.Inner` inside `Outer` is how ordinary Java spells a doubly-nested type. The
qualifier pass does not handle it:

```rust
// src/resolve.rs:285  qualify_nested_type_names_for_type
let direct_nested = ty.members.iter().filter_map(|m| match m {
    Member::Type(nested) => Some((nested.name.clone(),                    // <- SIMPLE name only
                                  format!("{}.{}", ty.name, nested.name.rsplit('.').next()...))),
    _ => None }).collect::<HashMap<_,_>>();
```

The map's keys are single identifiers (`P` → `N2.P`, and inside `P`'s own scope
`Inner` → `N2.P.Inner`). A reference written `P.Inner` matches no key, stays
unqualified, and downstream is mangled as a top-level name `PInner` — a *different
Rust type* from the real `N2PInner`, and one the translator then decides it knows
nothing about.

### Repro, and it compiles

```java
public class W {
    static int total = 0;
    static class P { static class Counter { void bump() { total += 10; } } }
    public static void main(String[] args) {
        P.Counter c = new P.Counter();
        c.bump(); c.bump();
        System.out.println(total);
    }
}
```

`java W` → `20`. Generated:

```rust
impl PCounter {                                  // synthesised import facade
    pub fn bump(&self) -> () { unimplemented!("P.Counter.bump is not modelled") }
}
...
pub struct WPCounter { }                         // the real class
impl WPCounter {
    pub fn bump(&self) { W::__set_static_total((W::__static_total()).wrapping_add(10)); }
}
pub type Counter = WPCounter;                    // and `pub type Counter = PCounter;` above

impl W { pub fn main(args: Vec<String>) {
    let c: PCounter = PCounter::new();           // <- the facade, not the class
    c.bump(); c.bump();
    println!("{}", Self::__static_total());
} }
```

Only one `rustc` error, the duplicate `pub type Counter`. Delete that single
alias line and the crate builds; running it:

```
thread 'main' panicked at W2.rs:1785:32:
not implemented: P.Counter.bump is not modelled
```

The translator concluded that a class **declared in the file it is translating**
is an unmodelled external import, purely because the source spelled it the legal
short way. Writing `W.P.Counter` instead makes the same program translate
correctly — same Java entity, two Rust identities chosen by spelling.

Current tree status: fixed. `qualify_nested_type_names_for_type` now indexes
descendant aliases while visiting the enclosing type, so `P.Counter` inside `W`
is rewritten to `W.P.Counter` before lowering. The regression compiles and runs
the generated crate and asserts that the real source class updates `W.total`.

---

## 7. `dyn` positions: three of four normalise, one does not

Current tree status: stale. The current `boxed_dyn_trait_projection` goes through
`normalize_emitted_java_type_name`, matching the other dyn projection sites; keep
the historical notes below as evidence of the original recurrence, not as an
open symptom.

0009's 2026-08-03 update introduced `normalize_emitted_java_type_name` (now in
`src/coercion.rs`; line numbers in the historical notes below have drifted) with
the comment *"Every position that emits a Java type name as Rust goes through
this."* Three did at the time:

```rust
borrow_as_dyn_trait            (src/coercion.rs:1970)  -> normalize_emitted_java_type_name
borrow_boxed_as_dyn_trait      (src/coercion.rs:1981)  -> normalize_emitted_java_type_name
anonymous_dyn_trait_projection (src/coercion.rs:2006)  -> normalize_emitted_java_type_name
```

At the time, `boxed_dyn_trait_projection` did not:

```rust
pub fn boxed_dyn_trait_projection(value: RustExpr, trait_name: &str) -> RustExpr {
    boxed_cast_projection(value, format!("Box<dyn {trait_name}>"))
}
```

Reached from `src/lower.rs:20633`. Output, from the `D.java` repro in
[0065](done/0065-the-overload-suffix-is-scoped-to-one-file-and-one-level-of-inheritance.md):

```rust
let i: Box<dyn DI> = Box::new(DImpl::new()) as Box<dyn D.I>;
```

```
error: expected one of `,` or `>`, found `.`
error[E0747]: constant provided when a type was expected
```

The declared slot got `DI`, the cast target got `D.I`. This is the fourth
instance of the same fix being applied per-symptom — 0013 fixed `downcast::<T>`,
0009 fixed `&dyn`, and `Box<dyn>` was missed.

---

## Why nothing noticed

* **The unit of compilation hides it.** `translate-focused-package.sh` emits one
  `pub mod generated_<stem>` per Java file (`scripts/translate-focused-package.sh:2758`).
  Two packages' `Item` land in two Rust modules, so the `E0428` that §3 would
  otherwise produce never fires; what is left is a *wrong reference*, which
  compiles or fails for an unrelated-looking reason. §2 and §5 only collide
  because both entities are in one file — which corresponds to a corpus using
  nested classes, and the corpus fixtures largely do not.
* **The compile-error metric is a count.** Every §-item here that does produce
  errors produces them at the *use* site, attributed to `E0308`/`E0609`/`E0425`,
  not to naming. [0018](../../claude_audits/done/0018-the-compile-error-metric-cannot-see-silent-failures.md)
  and [0053](../../claude_audits/done/0053-the-gate-compares-counts-not-names.md)
  already make this argument.
* **Only some tests assert injectivity.** The package-peer batch fixture now
  asserts emitted `AItem`/`BItem` source-item cardinality and cross-owner
  absence, which covers the concrete same-simple-name source collision. The
  broader generated-module invariant is still not first-class: most tests do
  not compare the full set of emitted item identities with the Java entities
  that produced them.
* **The right key was built and not adopted.** 0009 built
  `JavaCallableKey`; the surrounding type identity was never given the same
  treatment, so the key's `owner: String` is itself a package-free simple name.

---

## Direction

The three properties at the top are the specification. In dependency order:

1. **Give a Java type an identity that carries its package.** A source
   `TypeDecl.name` should be the binary name (`a.Item`, `W.P.Counter`), and every
   lookup key that today holds a bare `type_name: String` — `facts::MethodKey`,
   `signature::MethodKey`, `usage::CallableKey`, `JavaCallableKey::owner` — should
   hold that. §3 and §6 both dissolve; §2 does not, but becomes detectable.

2. **Replace the pure mangling functions with one allocation pass.**
   `allocate_rust_ident` already exists and already does the right thing. Run it
   once, over the whole program, in a deterministic order, producing a
   `HashMap<JavaEntityId, String>` that is thereafter the *only* answer to "what
   is this called in Rust". Emission and every reference site read that map; no
   site is permitted to call `to_snake_case`, `type_suffix` or
   `rust_name_for_java_named_type` on its own. That makes §2, §4, §5 and §7
   unrepresentable rather than fixed. It is the same move 0009 made for method
   *slots* — decide names late, as a total function of the resolved entity set —
   applied to the naming itself.

3. **Assert injectivity in the gate.** For each generated module, the number of
   distinct emitted item names must equal the number of Java entities emitted
   into it. This is a one-line invariant that subsumes every collision in this
   file and every future one.

4. Until (2), a **resolution that finds two candidates by simple name must
   refuse**, as `ConstantScope::lookup_path` already does. `context_type_key_matches`
   silently taking the first is what turns §3b from a compile error into a wrong
   number.

### Regression cases worth pinning

```java
// injectivity of the type map
class K { static class AB {} static class A { static class B {} } }            // KAB twice
class N { static class P { static class Inner {} } static class Q { static class Inner {} } }

// injectivity of the method map
class A { int xY(int a){return 1;} int x_Y(int a){return 2;} }                  // x_y twice
class Z { static int g(int[] a){return 1;} static int g(IntArray a){return 2;} } // g_int_array twice

// package identity — values, not spellings
// a.Cfg.N == 1, b.Cfg.N == 2; UseA (import a.Cfg) must read 1, UseB must read 2

// partially-qualified nested reference
class W { static class P { static class Counter { void bump(){...} } }
          void m(){ P.Counter c = new P.Counter(); c.bump(); } }               // must not be an import facade
```

## Codex parallel audit 2026-08-25

The current tree has local fixes for most concrete repros in this file:
adjacent nested type path boundaries are preserved, declared method names are
allocated against occupied constructor/method names, descendant aliases are
qualified for nested type references, and the boxed dyn-trait nested-name route
uses `normalize_emitted_java_type_name`. A 2026-08-25 follow-up also pinned the
batch analysis identity boundary:
`batch_analysis_keys_keep_same_simple_name_package_peers_distinct` asserts
through `UsageFacts::source_method_usage_by_java_key` and `SignaturePlan` that
`a.Item::who` and `b.Item::who` stay under Java-owner keys, not `Item` or emitted
`AItem`/`BItem`. The remaining live problem is broader: there is still no
first-class package-aware Java type identity object carried through the whole
pipeline.

Follow-up 2026-08-25: the explicit static receiver check no longer asks the raw
`SourceContextOwnerKey::matches_context_type` predicate before the
ambiguity-aware context lookup. A bare `Item.m()` receiver in a context with both
`a.Item` and `b.Item` now fails closed instead of matching whichever resolved
owner is being checked; exact emitted (`AItem`) and package-qualified
(`b.Item`) receivers still match. Regression:
`lower::tests::explicit_static_receiver_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-25, revised 2026-09-06: context static type alias seeding now
computes convenience-alias ambiguity across the support-local and source-summary
programs together before inserting aliases. A support `a.Item` and summary
`b.Item` therefore no longer leave a live `Item`/`item` alias pointing at
whichever context was seeded first; a support `a.FooBar` and summary
`b.Foo_Bar` likewise do not leave a first-wins `foo_bar` alias after snake-case
conversion. Exact dotted aliases remain available. Regression:
`lower::tests::context_alias_seeding_refuses_cross_program_ambiguous_simple_name`.
This is still a local ambiguity guard, not the first-class package-aware type
identity model described below.

Representative remaining string-owner surfaces:

- `java_naming::rust_name_for_java_named_type` still maps from name text to Rust
  spelling.
- `resolve::{resolve_method, resolve_constructor}` still combine string owners
  and local context rather than consulting one resolved entity map.
- `usage::UsageMethodKey`, `signature::MethodKey`, and many call-site helpers
  still accept owner strings. The batch duplicate pre-pass currently keeps
  same-simple-name package peers distinct for the pinned Java-key path, but the
  type system cannot express the invariant yet.

Smallest implementation slice: introduce a package-aware `JavaTypeKey` (or one
canonical owner helper returning such a key) and migrate the remaining
Rust-name/usage owner strings first: `usage::UsageMethodKey`,
`signature::MethodKey`, and construction sites that currently pass
`ty.name.clone()`. Keep emitted Rust names unchanged in that slice. The goal is
to make the already-pinned Java owner invariant explicit instead of relying on
pre-pass string rewrites.

Focused tests for that slice:

- Extend `batch_analysis_keys_keep_same_simple_name_package_peers_distinct`
  when `UsageMethodKey`/`signature::MethodKey` gain a typed owner, so it checks
  those maps in addition to the Java-key lookup.
- A context ambiguity unit extending `find_context_type_by_key` for exact and
  package-qualified owner lookups. Follow-up 2026-08-25: the lookup guard is now
  symmetric for package peers: `a.Item`/`AItem` resolve to `a.Item`,
  `b.Item`/`BItem` resolve to `b.Item`, and bare `Item` fails closed both within
  one program and across the combined lowering context. The emitted-context
  companion guards check the same four owner spellings. Regression filters:
  `lower::tests::context_type_lookup_refuses_ambiguous_simple_package_peer` and
  `lower::tests::context_type_lookup_refuses_cross_program_ambiguous_simple_package_peer`.

Follow-up 2026-08-25: the batch package-peer regression now compiles the
generated source items for `a.Item`, `b.Item`, and `Main` together with minimal
shared support stubs. This keeps the rendered-name assertions but also catches
duplicate emitted item names and wrong cross-file references for the
same-simple-name package-peer shape. Regression:
`batch_translation::package_qualified_source_type_references_keep_package_identity`.

Follow-up 2026-08-25: context type lookup no longer becomes ambiguous again
when each individual context program is safe but the combined lowering context
is not. `find_context_type_by_key_in_lower_ctx` now collects matches across the
primary, support-local, and source-summary programs before choosing a type:
exact/package-qualified/emitted Rust-name matches still win, but a bare simple
name such as `Item` fails closed when `a.Item` and `b.Item` live in different
context programs. The narrow source-context/static-owner helper paths now use
that combined lookup instead of priority-chaining the three programs.
Regression:
`lower::tests::context_type_lookup_refuses_cross_program_ambiguous_simple_package_peer`.
This is another ambiguity guard; the first-class `JavaTypeKey`/typed owner
migration remains the broader open model work.

Follow-up 2026-08-25: static source-context fallback signatures no longer carry
the receiver spelling forward after resolving a context type. A unique bare
receiver such as `Util` may resolve to `a.Util`, but the returned
`ContextFallbackMethodSignature.owner` now uses the declaration owner
`ty.name`, and the receiver-plan lookup asks the same resolved owner. Regression:
`lower::tests::source_context_static_fallback_uses_resolved_owner_not_receiver_spelling`.

Follow-up 2026-08-25: receiver-method source-context fallback now follows the
same rule. A local receiver typed as bare `Util` may resolve through the
combined context lookup to support `a.Util`; the fallback signature now returns
`a.Util`, not the local type spelling, and the emitted-context decision is based
on that resolved declaration owner. Regression:
`lower::tests::source_context_receiver_fallback_uses_resolved_owner_not_receiver_type_spelling`.

Codex follow-up 2026-09-12: constructor field-name recovery now also enters
context lookup through `SourceContextOwnerKey`, preserving the exact-vs-bare
owner behavior while removing another raw owner lookup from
`source_instance_field_rust_name`. Guard:
`layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`;
behavior guard:
`lower::tests::source_field_name_refuses_ambiguous_context_owner`.

Follow-up 2026-08-25: resolved static source-context fallback now also
canonicalizes through the resolved context declaration. If both the explicit
receiver spelling and `ResolvedCall.declaring_type` are bare names, they are
accepted only when the combined context lookup resolves both to the same unique
type, and the returned fallback owner is that declaration name (`a.Util`), not
the bare spelling. The existing ambiguous `Item` guard still fails closed.
Regression:
`lower::tests::resolved_static_fallback_uses_resolved_context_owner_not_declaring_spelling`;
guarded by `lower::tests::explicit_static_receiver_refuses_ambiguous_simple_package_peer`.

Codex follow-up 2026-09-12: unsupported constructor handoff lookup now wraps the
named `JavaType` owner in `SourceContextOwnerKey` before resolving the context
type. The existing ambiguous-bare-owner behavior is unchanged, but the helper no
longer reopens a raw `find_context_type_by_key_in_lower_ctx(ctx, type_name)`
path. Guard:
`layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`;
behavior guard:
`lower::tests::unsupported_source_constructor_handoff_refuses_ambiguous_context_owner`.

Follow-up 2026-08-25: static primitive/string storage reference folding now
resolves explicit owners through the combined context lookup and renders from
the resolved declaration owner. `AItem.KIND` and `a.Item.KIND` therefore both
emit through `AItem`, while ambiguous bare `Item.KIND` declines the fold instead
of priority-chaining to whichever package peer appears first or falling back to
the current type. Regressions:
`lower::tests::static_storage_reference_uses_resolved_context_owner_identity`
and `lower::tests::static_storage_reference_refuses_ambiguous_simple_owner`.

Codex follow-up 2026-09-12: the static primitive/string storage folds now keep
their explicit owner query wrapped as `SourceContextOwnerKey` until the context
lookup boundary. This is behavior-preserving against the combined lookup above,
but removes two more local raw `find_context_type_by_key_in_lower_ctx(ctx,
owner)` call sites. Guard:
`layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.

Follow-up 2026-08-26: static-array element writes now use the same combined
context lookup. `Item.VALUES[i] = value` no longer priority-chains
`program`/`support_local_type_program`/`source_summary_type_program` and silently
picks the first same-simple package peer; ambiguous bare owners fail closed,
while exact package-qualified and emitted Rust owner names still resolve.
Regression:
`lower::tests::static_storage_element_assignment_refuses_ambiguous_simple_owner`.

Follow-up 2026-08-26: interface trait argument shaping now uses the combined
context owner lookup too. A bare interface owner such as `Item` no longer
priority-chains to the first of `a.Item`/`b.Item` when selecting trait parameter
shapes for lowering; ambiguous package peers fail closed before signature lookup.
Regression:
`lower::tests::interface_argument_shapes_refuse_cross_program_ambiguous_simple_owner`.

Codex follow-up 2026-09-12: enum-interface checks now also keep owner and target
owner queries wrapped as `SourceContextOwnerKey` until context lookup. This
preserves the combined lookup and ambiguous-owner behavior, but removes local
raw owner lookup calls from `source_enum_directly_implements_*` and
`matches_source_java_enum_type`. Guards:
`lower::tests::enum_trait_lookup_refuses_cross_program_ambiguous_simple_owner`,
`lower::tests::enum_target_interface_lookup_uses_summary_and_refuses_ambiguous_target`,
and `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.

Codex follow-up 2026-09-12: the same wrapper-boundary cleanup was applied to
static-storage owner recovery, source/nested type recovery, context-method
effective-name lookup, lower-context subtype recursion, and interface trait
argument shape lookup. The helpers still use the legacy string-backed owner
payload, but raw context lookup is no longer reopened locally for those owner
questions. Guards:
`lower::tests::interface_argument_shapes_refuse_cross_program_ambiguous_simple_owner`,
`lower::tests::source_class_upcast_hierarchy_guard_refuses_ambiguous_context_owner`,
`lower::tests::static_storage_element_assignment_refuses_ambiguous_simple_owner`,
and `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.

Codex follow-up 2026-09-12: collections-sort method-reference result checks,
source associated-const/nested-type access, source enum static-owner recovery,
source declared static overload retargeting, and source declared instance method
lookup now use `SourceContextOwnerKey` for their local context owner lookups as
well. Guards:
`lower::tests::source_declared_instance_method_lookup_refuses_cross_program_ambiguous_owner`,
`slot_shape_design::collections_sort_throwing_method_reference_comparator_propagates_result`,
`lower::tests::enum_trait_lookup_refuses_cross_program_ambiguous_simple_owner`,
and `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.

Codex follow-up 2026-09-12: the remaining concrete-constructor, source-class
upcast, erased-object hierarchy guard, optional current-method display,
static-field receiver method candidate, and resolved static fallback owner
queries in `lower.rs` now cross the context boundary through
`SourceContextOwnerKey` too. This clears the local
`find_context_type_by_key_in_lower_ctx(ctx, owner/type_name/target_name/...)`
pattern from production lowerer code for the 0064 wrapper-boundary pass; broader
first-class owner identity is still open. Guards:
`lower::tests::source_constructor_concrete_slot_refuses_ambiguous_context_owner`,
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_context_target`,
`lower::tests::erased_cast_hierarchy_guard_uses_resolved_target_alias`,
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_bare_super_edge`,
`lower::tests::erased_cast_hierarchy_guard_refuses_ambiguous_bare_super_edge`,
`lower::tests::optional_current_method_display_shape_refuses_ambiguous_current_owner`,
`lower::tests::resolved_static_fallback_uses_resolved_context_owner_not_declaring_spelling`,
and `layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.

Follow-up 2026-08-26: the erased-name hierarchy helper and source enum/interface
bridges now use the combined context lookup as well. Generic spellings are
erased before lookup (`Item<T>`), exact/package-qualified and emitted owner names
still resolve, and ambiguous bare package peers fail closed instead of selecting
the first visible context program. The enum target-interface bridge now sees
summary-only interfaces and compares implemented interface names through the
canonical owner key. `lower_interface_super_default_call` also routes its current
type and named interface lookup through the combined helper. Regressions:
`lower::tests::erased_context_type_lookup_refuses_cross_program_ambiguous_simple_owner`,
`lower::tests::current_type_implements_target_refuses_ambiguous_bare_super_owner`,
`lower::tests::enum_trait_lookup_refuses_cross_program_ambiguous_simple_owner`,
and
`lower::tests::enum_target_interface_lookup_uses_summary_and_refuses_ambiguous_target`.
Follow-up cleanup: `lower_interface_super_default_call` now keeps the current
owner wrapped through `SourceContextOwnerKey` until the context lookup boundary.

Follow-up 2026-08-26: source interface supertrait bounds now use the same
combined erased-name context lookup. `interface Child extends Parent` fails
closed when support and summary context provide different `a.Parent`/`b.Parent`
peers, while exact `a.Parent` and exact summary `b.Parent` still produce the
expected Rust supertrait bound. Regression:
`lower::tests::interface_supertrait_bounds_use_combined_context_lookup`.

Follow-up 2026-08-26: the `context_type_key_is_summary_only` predicate now asks
the combined visible context whether the summary declaration is uniquely
selected, and it treats support-local matches as not summary-only. This prevents
summary-boundary return-shape and super-boundary handling from being enabled for
a bare owner that is actually ambiguous across support and summary context.
Regression:
`lower::tests::context_type_key_is_summary_only_accounts_for_support_context`.

Follow-up 2026-08-26: source-summary inherited method lookup now follows one
combined context edge per superclass step instead of checking program, then
summary/support, then program again. A source type extending bare `Parent` no
longer picks the summary `b.Parent` method when support also provides
`a.Parent`; exact owner spellings still resolve. Regression:
`lower::tests::source_summary_inherited_method_lookup_refuses_ambiguous_super_owner`.

Follow-up 2026-08-26: the current-type InputStream facade predicate now uses
the combined context lookup instead of program/support/summary priority
chaining. A bare current type `Item` no longer inherits InputStream behavior from
support `a.Item` when summary `b.Item` is also visible; exact `a.Item` still
resolves. Regression:
`lower::tests::current_type_input_stream_facade_lookup_refuses_ambiguous_context_type`.

Follow-up 2026-08-26: receiverless resolved static owner retargeting now
requires the resolved declaring type to be unique in the combined context and
renders from that resolved declaration name. A bare `Item` declaring type no
longer rewrites `Self::make` to whichever same-simple context owner is found;
exact `a.Item` still retargets to `AItem::make`. Regression:
`lower::tests::resolved_static_owner_retarget_refuses_ambiguous_context_declaring_type`.

Follow-up 2026-08-26: the combined context lookup itself now separates true Java
owner identity from emitted Rust-name aliases. A real source/context type named
`AItem` wins over the alias produced for `a.Item`, even when the two live in
different visible context programs; generated aliases remain a fallback instead
of an `Exact` match that silently keeps the first scanned type. The emitted
context lookup uses the same buckets. Regressions:
`lower::tests::context_type_lookup_prefers_real_owner_name_over_emitted_alias`
and
`lower::tests::emitted_context_type_lookup_prefers_real_owner_name_over_emitted_alias`.

Follow-up 2026-08-25: method-reference planning no longer drops every owner
segment before the final `.`/`$`. Package-qualified and nested-qualified method
references now use the same Java type-name mangler as ordinary type rendering,
so `a.Mapper::tag`, `b.Mapper::tag`, `K.AB::tag`, and `K.A.B::tag` stay distinct
at the plan boundary. Regression:
`method_reference_plan::tests::method_reference_owner_preserves_package_and_nested_identity`.

Follow-up 2026-08-25, updated 2026-09-10: `SignaturePlan`'s boolean
source-method presence probe now uses the same unique-match path as the lookup
helpers. The direct lookup helpers already failed closed when a bare owner such
as `Item` matched both `a.Item` and `b.Item`; the former `.any(...)` presence
probe defect is fixed too, so ambiguous simple owners no longer report a
method. Exact package-qualified owners still resolve. Regression:
`signature::tests::method_signature_presence_fails_closed_for_ambiguous_simple_package_owner`.

Follow-up 2026-08-25: batch erased-object display support no longer leaks
sibling-file source owners into a per-file generated module. The global batch
support program still drives semantic/signature analysis, but
`java_erased_any_to_string` source downcast arms are now built from the current
resolved program for batch emission. This avoids unqualified arms such as
`downcast_ref::<BDuplicateOwner>()` inside the `a.DuplicateOwner` module where
`BDuplicateOwner` is not defined. Regression:
`batch_translation::batch_duplicate_simple_type_names_keep_distinct_signature_keys`
now compile-checks both emitted files.

Follow-up 2026-08-26: source/JDK subtype reachability now uses the same
context-key lookup policy for the actual type and fails closed for ambiguous
simple target names. A unique simple package type such as `Child` can still
resolve to `a.Child`, but `Child` or `Base` no longer silently means whichever
of `a.Child`/`b.Child` or `a.Base`/`b.Base` happens to match during hierarchy
traversal. Regressions:
`lower::tests::source_subtype_reachability_uses_unique_context_simple_name` and
`lower::tests::source_subtype_reachability_refuses_ambiguous_context_simple_name`.

Follow-up 2026-08-26: the source-class upcast and erased-object source-supertype
`instanceof` hierarchy guards now use the combined context lookup as well. A
bare `Child -> Base` hierarchy check fails closed when support and summary
context expose `a.Child`/`b.Child` or `a.Base`/`b.Base`, while exact
package-qualified owners still trigger the loud hierarchy-lowering refusal.
Regressions:
`lower::tests::source_class_upcast_hierarchy_guard_refuses_ambiguous_context_owner`
and
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_context_target`.

Follow-up 2026-08-26: emitted-context static argument shaping no longer falls
back from the combined emitted-owner lookup to per-program emitted Rust-name
searches. `emitted_source_method_declared_arg_plan` and
`static_context_owner_is_emitted` now use the same combined lookup that refuses
ambiguous simple package peers, so a bare emitted owner cannot regain
program/support/summary priority order during argument-slot selection.
Regressions:
`lower::tests::static_context_owner_emitted_check_uses_combined_ambiguous_lookup`
and
`lower::tests::emitted_source_method_arg_plan_uses_combined_emitted_owner_lookup`.

Follow-up 2026-08-26: source-call exception/fallibility owner recovery now uses
the same combined lookup policy. `source_call_target_owner_candidates` no
longer falls back through program/support/summary emitted-name searches, and a
lowered receiver path such as `Item::...` is not treated as an arbitrary first
package peer when both `a.Item` and `b.Item` are in context. Regressions:
`lower::tests::source_call_owner_candidates_refuse_ambiguous_emitted_owner`.
Follow-up cleanup: the source-call static-owner and lowered-receiver owner
recovery helpers now route emitted owner lookup through
`SourceContextOwnerKey::find_emitted_context_type_and_program(...)` instead of
calling the raw emitted-context lookup directly. Guard:
`layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.

Follow-up 2026-08-26, revised 2026-09-06: same-outer inner-constructor
rewriting uses the combined emitted-context lookup before its remaining narrow
rendered-Rust-name compatibility helper. A bare constructor owner such as
`Inner` no longer selects the first of `a.Inner`/`b.Inner` when deciding whether
to inject or replace outer handles; unique emitted aliases such as `AInner`
still resolve. The compatibility helper now shares the same duplicate-owner
policy as `unique_context_type_match`, so two context programs that both expose
the same Java owner do not fail closed just because the owner was seen twice.
Regression:
`lower::tests::same_outer_inner_constructor_owner_refuses_ambiguous_emitted_owner`.
Follow-up cleanup: the combined emitted-context half of that same-outer owner
helper now goes through `SourceContextOwnerKey`; the narrow rendered Rust-use
compatibility helper remains as the documented fallback. Guard:
`layering::receiver_fallback_owner_candidates_stay_wrapped_until_lookup`.

Follow-up 2026-08-26: the `java.util.Objects.equals` recognizer no longer has
its own current-program-only emitted-name escape hatch. A translated/support
source type named `Objects` in any context program now blocks the JDK
`Objects.equals` lowering instead of letting support/summary owners be treated
as the platform class. Regression:
`lower::tests::java_objects_equals_refuses_source_objects_owner_in_context`.

Follow-up 2026-08-26: optional receiver member field-shape lookup no longer
queries the owner's simple name after the declared owner. `a.Item::field` cannot
borrow `Item::field` from a same-simple-name package peer when deciding optional
field unboxing/cloning behavior. Regression:
`lower::tests::signature_field_shape_for_member_does_not_borrow_simple_package_peer`.

Follow-up 2026-08-26: recursive optional field type identity now compares the
unwrapped Java type name exactly. It no longer treats `a.Node`, `b.Node`, or the
rendered Rust alias `ANode` as interchangeable when deciding whether a
recursive optional field needs an extra box/unbox adjustment. Regression:
`lower::tests::java_type_names_match_after_optional_uses_exact_java_identity`.

Follow-up 2026-08-26: unresolved current-source null-overload detection no
longer falls back from a qualified `current_type` to its simple name. A missing
owner such as `missing.Item` cannot inspect `a.Item` overloads merely because
the simple segment matches. Regression:
`lower::tests::current_null_overload_check_refuses_ambiguous_context_owner`.

Follow-up 2026-08-26: source instance-field name recovery now uses the combined
context lookup instead of independently probing program/support/summary. A bare
owner/current type such as `Item` cannot borrow `a.Item`'s keyword/collision
field spelling when `b.Item` is also in context; exact `a.Item` still gets the
owner-aware spelling. Regressions:
`lower::tests::source_instance_field_name_refuses_cross_context_simple_owner`
and
`lower::tests::current_type_source_field_name_refuses_cross_context_simple_owner`.

Follow-up 2026-08-26: translator-owned support code no longer depends on an
unqualified Rust prelude `Box` when rendering recursive exception causes and
erased-object runtime/facade helpers. A translated Java class named `Box<T>` can
still own the Rust item `Box<T>`, but it no longer makes infrastructure
spellings such as `JavaException.cause` or `java_clone_erased_object_ref`
resolve to the source class. Compile-backed regressions:
`source_generic_class_bound_method_call_uses_bound_surface`,
`source_class_bound_inherited_mutator_forwards_to_child_method`, and
`source_generic_class_bound_overload_call_uses_bound_rust_name`.

Follow-up 2026-08-26: the same qualification rule now covers object-safe trait
helper surfaces. Generated `anonymous`, `Clone for dyn Trait`, and
`clone_box` forwarding support use `std::boxed::Box<dyn Trait>` and
`std::boxed::Box::new`, so a source class named `Box<T>` no longer captures the
translator-owned heap-box helper type. Regression:
`source_generic_class_preserves_type_params_and_interface_bounds`.

Follow-up 2026-08-26: erased-source static retargeting now uses the combined
context type lookup instead of current-program-only owner checks. Static
receiver classification can resolve support/summary owners, and a simple owner
such as `Item` is rejected when package peers exist across context programs
instead of retargeting through whichever program happened to be checked first.
Regressions:
`lower::tests::explicit_source_static_receiver_type_uses_combined_context_lookup`,
`lower::tests::explicit_source_static_receiver_type_refuses_cross_context_simple_owner`,
and
`lower::tests::erased_source_static_retargeting_uses_combined_context_lookup`.

Follow-up 2026-08-26: erased-source static self-call lowering now uses the
same combined context lookup before rewriting `Self::method`/fallback-self
calls to owner-qualified free helpers. This removes another current-program-only
owner check from the erased-static path. Regression:
`lower::tests::erased_source_static_self_call_uses_combined_context_lookup`.
Follow-up cleanup: that current-owner lookup now resolves through
`SourceContextOwnerKey::new(current_type).find_context_type(ctx)` rather than
reopening a raw current-type context lookup inside
`apply_call_target_with_lower_context`.

Follow-up 2026-08-26: the remaining "emitted but not current-program" static
owner predicate is now centralized as
`static_owner_key_is_non_program_emitted_context`. The helper still deliberately
asks whether the owner is absent from `ctx.program`, but the policy is no longer
duplicated in argument-shape and fallback-signature selection. Regression:
`lower::tests::non_program_emitted_context_owner_check_is_combined_but_current_program_aware`.

Follow-up 2026-08-26: nested owner candidate recovery now resolves the current
owner once through the combined, ambiguity-aware context lookup before walking
nested declarations. A bare `Item.Inner` candidate no longer leaks out of
`a.Item` or `b.Item` when the current type is the ambiguous simple owner
`Item`; exact `a.Item` still contributes `a.Item.Inner`. Regression:
`lower::tests::nested_owner_candidates_refuse_ambiguous_current_owner`.

Follow-up 2026-08-26: source-supertype conversion impl lookup and anonymous
interface constructor trait lookup now use the combined ambiguity-aware context
lookup. They no longer search only selected context programs or accept the
first simple-name package peer when exact owner identity is unavailable.
Regressions:
`lower::tests::source_supertype_conversion_impls_use_combined_context_super_lookup`
and
`lower::tests::anonymous_interface_constructor_trait_name_uses_combined_context_lookup`.

Follow-up 2026-08-26: the batch package-identity generated-crate harness now
uses a `JavaClass<T>` support stub with the same `get_name()` surface as the
runtime support emitted by the translator. This was a test-support drift exposed
by the source `Object.toString()` fallback moving through
`self.get_class().get_name()`, not a package-identity regression. Regressions:
`batch_translation::package_qualified_source_type_references_keep_package_identity`
and
`batch_translation::imported_source_enum_value_of_keeps_package_owner_identity`.

Follow-up 2026-08-26: `static_owner_receiver_is_emitted_program_type` no
longer checks raw equality against `ctx.program.types`. It now uses the
program-local emitted-context lookup, preserving current-program-only policy
while accepting emitted/package-qualified owners and refusing ambiguous bare
simple package peers. Regression:
`lower::tests::static_owner_receiver_program_type_check_uses_emitted_owner_lookup`.

Follow-up 2026-08-26: source-subtype conversion availability and source
supertype conversion target emission now resolve owners through the same
combined unique context-key policy instead of exact program/support/summary
priority checks. This admits emitted/package-qualified aliases such as
`AChild`/`ABase`, preserves the existing context-emission gate for
support/summary targets, and fails closed for ambiguous simple owners. The
external-super path also receives the resolved actual declaration instead of
looking it up again by text.
Regression:
`lower::tests::source_subtype_conversion_uses_combined_context_identity`.

Follow-up 2026-08-26: abstract-interface method stub generation now resolves
implemented interface names through the combined erased-name context lookup
instead of searching only `support_local_type_program` with a simple-tail
fallback. Interfaces declared in the current program or summary context are
visible, and ambiguous simple package peers fail closed before a wrong abstract
stub is emitted. Regression:
`lower::tests::abstract_interface_method_stubs_use_combined_context_lookup`.

Follow-up 2026-08-26: external constructor support no longer emits a bare simple
type alias when two constructed external owners collapse to the same Rust simple
name. `new a.Foo()` plus `new b.Foo()` still emits `AFoo` and `BFoo`
placeholders, but suppresses `pub type Foo = ...` because the alias is
non-injective. Regression:
`stdlib::external_facade_support_tests::external_constructor_support_suppresses_ambiguous_simple_alias`.

Follow-up 2026-08-26: source-super-constructor flattening now resolves the
superclass through the same combined erased-name context lookup and feeds
inherited-field planning from borrowed current/support/summary context programs.
An exact current-context superclass such as `a.Parent` is selected even when a
support peer `b.Parent` exists, while a bare ambiguous `Parent` fails closed
instead of flattening through the first support match. Regressions:
`lower::tests::source_super_constructor_uses_selected_context_superclass` and
`lower::tests::source_super_constructor_refuses_ambiguous_context_superclass`.

Follow-up 2026-08-26: source hierarchy reachability callers no longer discard a
current-program hierarchy edge after resolving actual and target through the
combined context. `source_class_upcast_requires_hierarchy_lowering`,
exception-self upcast detection, and exact erased-object cast refinement now use
a lower-context reachability helper instead of asking only support-local
context. Regressions:
`lower::tests::source_class_upcast_hierarchy_guard_uses_current_program`,
`lower::tests::current_exception_supertype_guard_uses_current_program`.

Follow-up 2026-08-26: erased-object source-supertype cast/`instanceof` guards
now pass the resolved target declaration name into the hierarchy scan rather
than the original target text. An emitted alias such as `ABase` can resolve to
`a.Base`, and the source subtype search must use `a.Base` or it misses the
already-resolved package identity. Regression: the emitted-alias assertion in
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_context_target`
and `lower::tests::erased_cast_hierarchy_guard_uses_resolved_target_alias`.

Follow-up 2026-08-26: loud source-class upcast diagnostics now use the resolved
actual and target owner names, not the raw actual/target text supplied to the
slot or cast path. An emitted alias pair such as `AChild`/`ABase` now reports
`a.Child to a.Base`, matching the identity used by the hierarchy guard.
Regression:
`lower::tests::source_class_upcast_loud_names_use_resolved_context_identity`.

Follow-up 2026-08-26: source callable fact keys now have an explicit
`JavaOwnerKey` construction boundary. Source-declared method usage/signature
insertion, declaration-based lookups, and already-resolved lower-context source
lookups use `source_method_for_type` or `source_method_for_owner`; the raw
`source_method(owner: String, ...)` constructor is private to
`java_signature.rs`. This does not change the stored key shape yet, but it
removes the public habit of rebuilding source owner identity from arbitrary
text at the fact boundary. Regressions:
`signature::tests::method_signature_presence_fails_closed_for_ambiguous_simple_package_owner`,
`signature::tests::required_method_signature_does_not_borrow_distinct_package_peer`,
`signature::tests::required_field_shape_does_not_borrow_distinct_package_peer`,
and `lower::tests::static_owner_receiver_program_type_check_uses_emitted_owner_lookup`.
Follow-up cleanup 2026-09-12: the `java_signature` module itself is
crate-internal. Its key constructors and overload-slot helpers remain the
central implementation boundary for usage/signature/lowering, but they are no
longer exported as public crate API while the first-class owner model remains
open.

Follow-up 2026-08-26: the remaining direct `JavaCallableKey { ... }`
construction outside `java_signature.rs` was collapsed behind
`JavaCallableKey::resolved_source_method`. Resolved-call argument-shape planning
and resolved-call return-shape lookup now share the same owner and erased
argument construction path instead of rebuilding the key field-by-field in
`call_plan.rs` and `expr_shape.rs`. Regression:
`slot_shape_design::callable_analysis_shell_reaches_lowering_context`.

Follow-up 2026-08-26: explicit source static receiver recovery now returns the
resolved declaration owner, not the raw receiver spelling that happened to
resolve. A unique simple receiver such as `Util` and an emitted alias such as
`AUtil` both recover `a.Util`, so erased-source static helper calls line up with
helper declarations named from the declaration owner. Ambiguous simple owners
still fail closed through the combined context lookup. Regression:
`lower::tests::explicit_source_static_receiver_type_uses_combined_context_lookup`.

Follow-up 2026-08-26: representation-lowering source type lookup no longer
uses first match for a bare simple name. The helper now collects exact owner,
emitted Rust alias, and simple-name candidates recursively and returns only a
unique match at the highest-precedence tier. A bare `Named` with both `a.Named`
and `b.Named` in the source program now fails closed instead of silently using
the first package peer, while `a.Named` and `ANamed` still resolve. Regression:
`representation_lowering::type_lookup_tests::find_type_decl_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-26: the co-translated context same-simple-name regression is
now compile-backed, not only text-backed. A sibling test translates the caller
with both package peers visible, translates each context peer with the other
peer visible so their emitted names match the caller, stitches the three
generated bodies into one crate, and `cargo check`s it. This proves the
single-source/context references to `AItem::new`, `BItem::new`, `AItem::KIND`,
and `BItem::KIND` resolve in a generated crate. Regression:
`slot_shape_design::cotranslated_same_simple_name_package_peers_compile_with_context_bodies`.

Follow-up 2026-08-26: storage-strategy declared-source type lookup no longer
uses the first same-simple-name match for bare type names. Storage planning now
tries exact owner text, then a same-package match inferred from the containing
owner, then a unique simple-name fallback; ambiguous simple package peers fail
closed. This prevents a `b.Holder` field declared as bare `Item` from charging
static reachability or storage escape to `a.Item` merely because `a.Item` sorts
first. Regression:
`storage_strategy::tests::storage_strategy_simple_owner_uses_same_package_before_first_simple_match`.

Follow-up 2026-08-28: the duplicate-program type pre-pass now checks the Rust
declaration names produced by its planned Java-owner rewrites, not only the
source simple names. If ordinary package qualification would still collide with
another emitted declaration, such as default-package `ABC` versus package peer
`a.b.C` projected as `ABC`, the pre-pass assigns a collision-safe internal owner
like `a.b.JavaType.C` and rewrites same-file bare references plus cross-file
fully-qualified references to match. This is still a local generated-name guard,
not the first-class package-aware owner model described above. Regression:
`lib::tests::duplicate_program_prepass_avoids_projected_rust_type_name_collisions`.

Follow-up 2026-08-28: `signature_field_shape_for_type_aliases` no longer lets a
package-qualified owner miss fall through to its simple-name peer. A lookup for
`a.Item::value` must not borrow `Item::value` just because the final segment
matches; simple-owner compatibility remains for unqualified owner text only.
Regression:
`lower::tests::signature_field_shape_for_type_aliases_does_not_borrow_simple_package_peer`.

Follow-up 2026-08-28: nested simple type aliases are now suppressed against the
Rust names of emitted method-local capture types, not only against source
program type declarations. A source nested type and a capturing local class can
both be named `Inner`; the generated module now keeps the real emitted structs
and avoids adding a colliding `pub type Inner = Outer_Inner` alias. This is a
local emitted-name collision guard, not the first-class Java owner identity
model this audit still asks for. Regression:
`slot_shape_design::nested_simple_alias_is_suppressed_by_emitted_local_capture_type`.

Follow-up 2026-08-28: the adjacent local-class shadowing constructor path is
fixed for the same `Inner`/`Outer.Inner` shape. When a method-local capturing
class shadows a nested member class, the local declaration refinement now keeps
the initializer's local-capture type instead of the pre-qualified member slot,
and the source-constructor-for-slot fast path passes capture constructor
arguments just like the ordinary `Expr::New` lowering path. The generated code
therefore emits `let local: Inner = Inner::new(base);` instead of a
`compile_error!` upcast from `Inner` to `Outer.Inner`. Regression:
`slot_shape_design::local_class_constructor_shadows_nested_member_class`.

Follow-up 2026-08-28: `stdlib` source-summary type lookup no longer returns the
first package peer for a bare simple owner. `find_source_type_decl` now tries
exact Java owner text first and only accepts simple-name/Rust-alias
compatibility when it is unique; ambiguous `a.Item`/`b.Item` queried as `Item`
returns `None`. Constructor facade and imported-static summary lookups also go
through `find_source_summary_owner_decl`, which accepts a package-qualified miss
through the owner's simple name only when the summary program is for that same
package. This fences inherited source-summary method signatures, source-summary
constructor facade methods, and imported static summary methods without
introducing the broader first-class owner key. Regressions:
`stdlib::source_summary_method_identity_tests::find_source_type_decl_refuses_ambiguous_simple_package_peer`,
`stdlib::source_summary_method_identity_tests::source_summary_constructor_lookup_does_not_borrow_other_package_simple_owner`,
and
`stdlib::source_summary_method_identity_tests::source_summary_imported_static_lookup_does_not_borrow_other_package_simple_owner`.

Follow-up 2026-08-28: the adjacent `stdlib` program-type lookup helper now
uses the same exact-or-unique policy instead of returning the first simple-name
package peer. `find_program_type_by_key` tries exact canonical owner text first,
then accepts simple-name compatibility only when there is one match. This keeps
source-summary hierarchy traversal from inheriting methods or static-final
fields through an arbitrary `a.Base`/`b.Base` peer when a child only says
`extends Base` and the owner cannot be disambiguated locally. Regressions:
`stdlib::source_summary_method_identity_tests::find_program_type_by_key_refuses_ambiguous_simple_package_peer`
and
`stdlib::source_summary_method_identity_tests::source_summary_method_hierarchy_refuses_ambiguous_simple_super_owner`.

Follow-up 2026-08-28: imported static-owner support discovery now respects
lexical shadowing instead of computing one lossy method-wide name set before
owner collection. A later local with the same spelling as an imported owner no
longer suppresses an earlier owner call, and a nested-block local no longer
suppresses a call after the block. This is still a scoped collector fix rather
than the first-class Java owner identity model, but it removes one more place
where pure spelling decided whether an external owner existed. Regressions:
`stdlib::external_facade_support_tests::imported_static_owner_call_before_shadowing_local_is_collected`
and
`stdlib::external_facade_support_tests::imported_static_owner_shadow_does_not_escape_nested_block`.

Follow-up 2026-08-28: generic source type-bound lookup in `item_assembly.rs`
no longer picks the first package peer for a bare simple bound. Type-parameter
bound projection now tries exact Java owner text, then emitted Rust alias, and
only then accepts a simple-name/Rust-simple-name match when it is unique across
the support programs. The same ambiguity-aware helper drives `Into<...>`
supertype bounds for source class bounds, so `T extends Base` does not borrow
`a.Base` or `b.Base` by support-program order when both exist. Regressions:
`item_assembly::tests::source_type_bound_decl_refuses_ambiguous_simple_package_peer`,
`item_assembly::tests::source_type_param_into_supertype_bounds_refuses_ambiguous_simple_package_peer`,
and the existing generated-crate check
`idiom_gaps::source_generic_class_bound_overload_call_uses_bound_rust_name`.

Follow-up 2026-08-28: external field fact lookup now also fails closed for an
ambiguous package-qualified simple-owner alias. Exact package owners still
resolve (`a.Item.KIND`, `b.Item.KIND`), but a bare `Item.KIND` query no longer
returns whichever package peer happens to win candidate ordering when both
`a.Item` and `b.Item` expose `KIND`. This fixes the same owner-key shape at the
fact lookup boundary used by static constant/member-access planning. Regression:
`semantic_facts::external_field_lookup_refuses_ambiguous_simple_package_owner`.

Follow-up 2026-08-28: in-scope type-parameter detection in lowering now uses
the combined context owner lookup instead of checking only the primary source
program. A support-local current type such as `a.Box<T>` is recognized when the
source program is empty, while a bare ambiguous `Box` owner still fails closed
when support and summary programs contain package peers. Regression:
`lower::tests::in_scope_type_parameter_lookup_uses_combined_context_and_refuses_ambiguous_owner`.
Follow-up cleanup: the in-scope type-parameter helper now performs that current
owner lookup through `SourceContextOwnerKey`, and the same wrapper-boundary
pattern is used for unresolved null-overload ambiguity and current-method
exception planning. Guard:
`layering::current_owner_utility_lookups_stay_wrapped_until_context_lookup`.

Follow-up 2026-08-28: derive-support checks in `item_assembly.rs` no longer
recover a source declaration by taking the first Java owner whose emitted Rust
name matches a rendered `RustType::Named`. The reverse lookup now returns a
declaration only when the emitted Rust name maps to one distinct Java owner, so
collisions like `a.B.C` and `AB.C` both rendering as `AB_C` fail closed instead
of letting traversal order decide `Default`/field-walk behavior. Regression:
`item_assembly::tests::derive_support_type_lookup_refuses_ambiguous_emitted_rust_name`.

Follow-up 2026-08-29: inherited source member lookup no longer walks support
types with first-match simple-owner aliasing. `find_type_decl` now prefers
exact Java owner text and otherwise accepts alias/simple-name matches only when
they are unique, so `extends Base` with both `a.Base` and `b.Base` visible does
not inherit whichever package peer appears first. Package-qualified `extends
b.Base` still resolves. This also fences interface default traversal because it
shares the same lookup helper. Regression:
`inherited_members::tests::inherited_source_methods_refuse_ambiguous_simple_package_super`.

Follow-up 2026-08-29: slot-policy source-method usage lookups now use the
resolver-backed Java callable key via `UsageFacts::source_method_usage(ty,
method)` instead of reconstructing `UsageMethodKey { type_name, method_name }`
from plain strings. This migrates borrowed-content parameter policy off one
more non-injective owner/method spelling path. Regressions:
`slot_policy::tests::borrowed_return_param_policy_uses_binding_facts_not_name_assignment_scan`
and `layering::slot_policy_uses_java_keyed_source_method_usage_lookup`.

Follow-up 2026-08-29: anonymous declared-return dyn-trait constraint planning
no longer reaches through `slot_render` just to render a Java owner name.
`signature.rs` now uses `java_naming::rust_name_for_java_named_type`, keeping
source analysis and signature planning independent of slot rendering for this
path. Regression:
`layering::plan_layers_keep_rust_rendering_out_of_source_analysis`.

Follow-up 2026-08-29: declaration-scoped usage lookups moved another step away
from `UsageMethodKey { type_name, method_name }` string reconstruction.
`UsageFacts` now keeps the arbitrary `source_method_usage_for_owner` helper
private; declaration callers use `source_method_usage(ty, method)` and
cross-module resolved-owner callers use
`resolved_source_method_usage_for_owner(owner, method)`. `storage_strategy`,
`semantic_analysis`, and `signature` use Java-callable keys when they already
have `TypeDecl`/`MethodDecl` context. `aliasing` now uses resolved-source usage
queries for resolved receiver-mutating calls, so overload identity, erased
argument types, and staticness decide whether the call mutates its receiver.
This fixes the live case where a read-only overload
could inherit mutability from a same-name mutating overload via the legacy
Java-name map. Regressions:
`aliasing::tests::receiver_mutation_lookup_uses_resolved_overload_identity`,
`storage_strategy::handle_forwardable_surface_tests::camel_case_mutating_interface_method_blocks_forwarding`,
and `layering::declaration_context_usage_lookups_use_java_callable_keys`.

Follow-up 2026-08-29: semantic unresolved-target external-effect guessing now
distinguishes "not a source owner" from "ambiguous source owner". The receiver
owner lookup used before synthesizing an external API effect fact accepts exact
source owners, emitted Rust owner names, and simple names only when the tier is
unique; a bare `Item` with both `a.Item` and `b.Item` visible now fails closed
instead of seeding an external `Item.transform` guess from traversal order.
Regression:
`semantic_facts::external_api_effect_guess_refuses_ambiguous_source_simple_owner`.

Follow-up 2026-08-29: signature inherited-field owner lookup no longer returns
the first support declaration whose external alias matches a rendered type
name. `find_signature_type_decl` now prefers exact Java owner text and accepts
alias/simple-name matches only when the match is unique, so a bare `Base`
cannot borrow constructor-field constraints or type parameters from whichever
of `a.Base` / `b.Base` appears first. Regression:
`signature::tests::signature_type_lookup_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-29: external display-capability facts no longer grant their
simple-name capability to every package peer. A modeled
`a.ExternalToken.toString(): String` now records `a.ExternalToken` and its Rust
surface `AExternalToken`, but not bare `ExternalToken`; erased-object
stringification can still detect the qualified source mention and emit the
`AExternalToken` downcast. Regressions:
`functional_interface_shape_tests::package_qualified_to_string_fact_does_not_mark_simple_peer_display_capable`
and
`tests/slot_shape_design.rs::erased_external_object_display_keeps_package_qualified_to_string_capability_distinct`.

Follow-up 2026-08-29: source supertype conversion field flattening now uses
the combined inherited-field lookup across support and source-summary programs
instead of asking each program separately and merging both answers. A child with
`extends Base` no longer inherits fields from both `a.Base` and `b.Base` when
the bare owner is ambiguous; package-qualified `extends b.Base` still inherits
the intended field. Regression:
`lower::tests::source_instance_field_names_for_conversion_refuses_cross_program_ambiguous_simple_super`.

Follow-up 2026-08-29: optional current-method display shape recovery no longer
consults signature return shapes with an unresolved owner spelling. The display
slot path now resolves `current_type` / resolved declaring-type candidates
through the ambiguity-aware lower-context type lookup before asking
`SignaturePlan::method_return_shape_by_rust_name`, so a stale simple-owner
signature entry for `Item.name()` is ignored when the visible context contains
both `a.Item` and `b.Item`. Exact package owners still recover optional display
shape. Regression:
`lower::tests::optional_current_method_display_shape_refuses_ambiguous_current_owner`.

Follow-up 2026-08-29: stream-lambda raw static-member recovery now resolves
`Owner.member` through the active `StreamRenderContext` before rendering the
Rust associated path. A raw compatibility leaf such as `Cfg.N` therefore sees
the same static type aliases as parsed `Expr::FieldAccess`, so an import alias
for `AB.C` emits `AB_C::N` instead of treating bare `Cfg` as already-resolved
Rust owner text. Regression:
`tests/slot_shape_design.rs::stream_pipeline_chain_uses_string_and_optional_shapes`.

Follow-up 2026-08-29: package-qualified stream static constants and external
constant support discovery now preserve the full owner key. The stream lambda
path lowers parsed and raw `a.Item.KIND` through owner `a.Item`, producing
`AItem::KIND`; the support collector asks `qualified_static_owner_java_name`
before falling back to a final field segment, so the same expression no longer
emits bogus external stubs such as `pub struct Item; pub const KIND: KIND =
KIND;`. Regressions:
`tests/slot_shape_design.rs::stream_lambda_emission_uses_structured_operator_rendering`
and
`tests/batch_translation.rs::stream_static_constant_support_keeps_package_qualified_owner_identity`.

Follow-up 2026-08-29: array-allocation defaultability now resolves the Java
element owner through the ambiguity-aware lower-context lookup before asking
item assembly whether the source type can derive `Default`. It no longer
renders `a.B.C`/`AB.C` to the shared emitted alias `AB_C` and probes each
program independently, so `new AB.C[n]` cannot borrow `a.B.C`'s derive answer
when the exact `AB.C` owner is a non-defaultable interface. Regression:
`lower::tests::array_element_default_lookup_refuses_cross_program_emitted_owner_collision`.

Follow-up 2026-08-29: erased-object source-supertype `instanceof` and cast
guards now scan subtype reachability through the combined lower context instead
of asking each visible program independently. The combined subtype walk refuses
ambiguous bare source names before applying simple-name equality, and skips the
stdlib subtype shortcut when the target is a visible source type. This closes
the case where `a.Child extends Base` could be treated as reaching `a.Base`
even though another visible `b.Base` made that bare edge ambiguous.
Regressions:
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_bare_super_edge`
and
`lower::tests::erased_cast_hierarchy_guard_refuses_ambiguous_bare_super_edge`.

Follow-up 2026-08-29: `context_type_key_is_in_source_summary` now asks the
combined lower-context owner lookup which program owns a type key, instead of
probing only the summary program and accepting a bare simple-name hit. A bare
`Item` is now rejected when support and summary programs both expose distinct
`Item` owners, while exact summary names and unambiguous emitted aliases still
classify as source-summary. Regression:
`lower::tests::context_type_key_is_in_source_summary_refuses_ambiguous_simple_owner`.

Follow-up 2026-08-29: explicit static receiver matching no longer accepts a
static type alias by asking the raw `SourceContextOwnerKey` simple-name matcher.
Aliases now resolve through the combined lower-context lookup and compare the
resolved owner declaration, so `Alias -> Item` fails closed when `a.Item` and
`b.Item` are both visible, while `Exact -> a.Item` still matches. Regression:
`lower::tests::explicit_static_receiver_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-29: static-owner emitted/current-program predicates now share
an emitted context lookup that returns both the resolved type and the owning
`Program`. `static_owner_key_is_non_program_emitted_context` and
`static_owner_receiver_is_emitted_program_type` now classify by program pointer
identity instead of composing separate emitted/current-program string lookups.
Regressions:
`lower::tests::non_program_emitted_context_owner_check_is_combined_but_current_program_aware`
and
`lower::tests::static_owner_receiver_program_type_check_uses_emitted_owner_lookup`.

Follow-up 2026-08-29, tightened 2026-09-11 and 2026-09-12: usage-side receiver
boundary classification no longer treats receiver/declaring type-name equality
as proof that an arbitrary call is content-only. The
`receiver_static_type_matches_declaring_type` shortcut was removed. Explicitly
resolved source instance calls are now classified as source content reads,
while unresolved helper calls still need modeled external facts or
source-summary capability facts to prove a read-only/content-read boundary.
Regressions:
`usage::tests::resolved_receiver_boundary_treats_source_instance_calls_as_content_reads`
and
`slot_shape_design::usage_classifies_unknown_call_results_assigned_to_constructor_locals_as_boundaries`.

Follow-up 2026-08-29: member-access static constant planning no longer treats a
bare capitalized owner as sufficient proof of a Java static owner. The fallback
static-constant branch now requires an import/static alias, a known source
type-field entry, a package-qualified owner, or a small known built-in owner
set; otherwise `Other.VALUE` fails closed instead of inventing
`Other::VALUE` with an unknown Java type. Regression:
`type_coercion::member_access_plan_does_not_invent_unknown_uppercase_static_constant_owner`.

Follow-up 2026-08-29: bare static constant reads now prefer explicit static
member imports over the flat static-field owner map. `lower_bare_name("KIND")`
therefore renders `AItem::KIND` for `import static a.Item.KIND` even when a
support/context peer has also registered `b.Item.KIND` under the same bare
field spelling. Regression:
`slot_shape_design::access_lowering_owns_bare_field_and_member_access_fallbacks`.

Follow-up 2026-08-30: bare static-storage update expressions now qualify the
setter through the same recorded-owner path as the getter. `count++` for a
static field recorded as owned by `a.Counter` now plans
`ACounter::__static_count()` and `ACounter::__set_static_count`, instead of
reading from the owner-qualified getter and writing through an unqualified
setter path. Regressions:
`lower::tests::bare_static_storage_update_uses_recorded_owner_for_getter_and_setter`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-30: signature planning's legacy source-summary fact scans no
longer borrow method mutability or return-nullability facts from a different
package peer. Qualified receiver owners such as `b.Item` now fail closed
against `a.Item` facts even though both expose the same simple `Item` alias;
bare legacy owner compatibility remains limited by the existing unique-owner
ambiguity rule. Regressions:
`signature::tests::source_receiver_binding_constraints_do_not_borrow_package_peer_mutability`
and
`signature::tests::source_summary_method_effect_return_nullability_does_not_borrow_package_peer`.

Follow-up 2026-08-30: constructor argument-effect fact lookup now follows the
same exact/direct/ambiguous-simple-owner policy as the neighboring external
constructor and field fact lookups. A bare `Item` lookup no longer silently
selects the last `ExternalConstructorArgumentEffectFact` when both `a.Item` and
`b.Item` provide an effect for the same arity and parameter index, while exact
qualified lookups still resolve. Regression:
`semantic_facts::fact_lookup_refuses_ambiguous_simple_package_owner_aliases`.

Follow-up 2026-08-30: external conversion-adapter fact lookup now applies the
same package-simple ambiguity guard to both endpoints. Exact qualified adapter
facts such as `a.Item -> Target` and `Source -> b.Item` still resolve, but bare
`Item` no longer selects one package peer for either the source or target side
of an `Into` adapter. Regression:
`semantic_facts::fact_lookup_refuses_ambiguous_simple_package_owner_aliases`.

Follow-up 2026-08-30: global platform/external-pack conversion adapter lookup
now prefers exact endpoint pairs before falling back to broad alias matching.
`ZipOutputStream -> OutputStream` therefore resolves to the simple facade
adapter, while `java.util.zip.ZipOutputStream -> OutputStream` resolves to the
qualified platform adapter instead of whichever alias-compatible fact appears
first. Regression:
`external_packs::platform_conversion_adapter_lookup_prefers_exact_endpoint_names`.

Follow-up 2026-08-30: FFI handover facts and legacy Rust result/return-type
facts now use the same exact/direct/ambiguous-simple-owner policy as the
neighboring constructor and field lookups. Bare `Item.handover(...)` or
`Item.open(...)` no longer inherits receiver, argument, result, return-slot, or
constructor-result facts from whichever of `a.Item`/`b.Item` appears last,
while exact qualified owners still resolve. Regression:
`semantic_facts::fact_lookup_refuses_ambiguous_simple_package_owner_aliases`.

Follow-up 2026-08-30: external crate API mappings, constructor mappings, and
type-name mappings now use the same fail-closed package-simple owner policy.
Direct owner matches are preferred before any broad alias fallback, and a bare
`Item` query no longer selects one of two package peers for mapped method names,
constructor names, or Rust carrier type names. Regression:
`semantic_facts::fact_lookup_refuses_ambiguous_simple_package_owner_aliases`.

Follow-up 2026-08-30: the Rust-name-to-Java-method fallback in
`external_api_effect_for_receiver_with_arity_or_rust_name` now uses the same
external crate API mapping policy. A shared Rust facade method name can still
resolve for exact `a.Item` or `b.Item` receivers, but a bare `Item` receiver no
longer picks one package peer's mapped Java method effect. Regression:
`semantic_facts::fact_lookup_refuses_ambiguous_simple_package_owner_aliases`.

Follow-up 2026-08-30: source static-type receiver detection now respects local
variable shadowing. A local named `connection` in a scope that also imports a
source-summary type `Connection` is no longer treated as the static owner
selected by the snake-case type alias; `connection.getTarget()` reaches the
instance receiver resolver instead of recording a stale unresolved/static-method
fact. Regressions:
`resolver_benchmark::tests::source_static_type_receiver_respects_local_shadowing_for_imported_nested_type`
and
`dependency_mode::source_summary_root_resolves_imported_nested_graph_types`.

Follow-up 2026-08-30: type-qualified static-field lookup now uses the same
lexical shadow guard. A local named `bounds` no longer gets typed through the
snake-case alias for source class `Bounds` when resolving `bounds.SENTINEL`;
the expression reaches only the explicit unknown-overload fallback instead of
an identity `int` coercion from the static field. Legitimate
`Outer.Bounds.SENTINEL` and source-summary nested static-field cases still
resolve. Regression:
`resolve_calls::resolver_static_field_lookup_respects_local_receiver_shadowing`.

Follow-up 2026-08-30: dependency-mode source/external field attribution now
handles the parser's split spelling for lambda parameters used as field-access
owners. In source text such as `HDF5File -> HDF5File.OpenMode.CREATE`, the
lambda binding is normalized to an emitted local name while the uppercase
field-access receiver remains source-like for static-owner parsing. The
benchmark type extractors now resolve those uppercase owner tokens through the
normalized local binding and treat synthetic `lambda_param:*` unknowns as
non-owners, so lambda parameters no longer become HDF5 pack or source-summary
field evidence. Regressions:
`dependency_mode::resolver_benchmark_does_not_treat_lambda_params_as_static_pack_receivers`
and
`dependency_mode::source_summary_field_contexts_do_not_treat_lambda_params_as_static_receivers`.

Follow-up 2026-08-30: two dependency-mode project field attribution checks had
stale normalized-owner assertions after the owner-key cleanup. The behavior was
already in the intended bucket: `BaseUtils.MAX_BASE_SCORE` and
`BaseUtils.Base.N.base` both stay as project field pressure with zero external
field guesses. The tests now assert the source owner spelling instead of the
old normalized `base_utils` string. Regressions:
`dependency_mode::resolver_benchmark_classifies_snake_case_project_field_receiver_as_project`
and
`dependency_mode::resolver_benchmark_propagates_project_field_guess_ownership_through_static_field_chains`.

Follow-up 2026-08-30: dependency-mode source-summary type maps now retain the
package-relative owner-qualified nested spelling in addition to simple and fully
qualified names. This closes the attribution hole where
`new UserException.ValidationFailure(...)` resolved successfully but could not
be counted as a source-summary constructor because the table had
`ValidationFailure` and `dep.exceptions.UserException.ValidationFailure`, but
not `UserException.ValidationFailure`. Regression:
`dependency_mode::source_summary_roots_resolve_project_helper_fields_and_nested_exception_constructors`.

Follow-up 2026-08-30: `resolve_calls`' nested/lexical owner cluster is green.
Unqualified nested constructor lookup now checks the current lexical type scope
before falling back, so sibling nested constructors such as `new Insert(...)`
inside `Owner.Element` resolve to `Owner.Insert`. The source-program builder no
longer imports every top-level type in a package for a single explicit import,
so `import demo.lib.Marker` does not make incidental `demo.lib.Container.Writer`
look source-owned. Known external enum-like receiver `values()` calls now expose
typed array returns, and overload scoring applies external type aliases to
arrays as well as direct named references. Regressions:
`resolve_calls::resolver_prefers_sibling_nested_constructor_visible_from_nested_class`,
`resolve_calls::resolver_matches_generic_sibling_nested_constructor_with_current_receiver_argument`,
`resolve_calls::resolver_uses_enclosing_static_method_return_type_in_nested_constructor_delegation`,
`resolve_calls::resolver_uses_enclosing_source_field_type_for_nested_class_overload_arguments`,
`resolve_calls::resolver_benchmark_does_not_import_incidental_nested_package_types`,
`resolve_calls::resolver_uses_external_domain_constants_for_primitive_and_string_overloads`,
and the stale known-failure entries now covered by the full
`cargo test --release --test resolve_calls` pass.

Follow-up 2026-08-30: `dependency_mode`'s hierarchy/source-summary cluster is
green. Source-summary superclass facts can now introduce corpus-derived
erased-object boundary roots into per-file representation plans without
overriding primary source classes. Hierarchy diagnostics also normalize generic
supertype spellings and simple nested subtype declarations against
owner-qualified edge keys, and closed-enum override variants such as `SplitRead`
resolve to source keys such as `BreakpointEvidence.SplitRead` during validation
and application. Regressions:
`dependency_mode::primary_extends_source_summary_base_is_erased_boundary_not_inheritance_pressure`,
`dependency_mode::primary_source_hierarchy_override_cleans_breakpoint_evidence_shape`,
`dependency_mode::external_fact_hierarchy_override_cleans_breakpoint_evidence_shape`,
`dependency_mode::external_fact_closed_enum_override_cleans_multilevel_hierarchy_shape`,
`dependency_mode::nested_source_serializer_subclass_of_qualified_kryo_serializer_uses_external_boundary`,
and the full `cargo test --release --test dependency_mode` pass.

Follow-up 2026-08-30: the source-summary static overload call-site cluster is
green. Summary facade emission already produced `pick_int`/`pick_string`, and
instance calls already used `inst_int`/`inst_string`, but static source-summary
calls whose receiver spelling had been normalized still re-planned from the
receiver and fell back to bare `pick`. Resolved source static calls now retarget
static owners to the resolver-selected `ResolvedCall.rust_name` before argument
planning, so all four `summary_source` slot-shape tests pass. Regressions:
`lower::tests::source_context_static_overload_lowers_to_selected_name`,
`slot_shape_design::summary_source_overloads_get_one_rust_name_from_every_path`,
and `slot_shape_design::a_summary_source_type_gets_no_unmodelled_import_stub`.

Follow-up 2026-09-09: the resolved source-summary static fallback now accepts
package-qualified receiver syntax such as `dep.Util.make()` through the same
`SourceContextOwnerKey` candidate path used by unresolved source calls, while
preserving the local/field shadowing guard. This closes another small owner-key
split without changing the broader string-owner model. Regressions:
`lower::tests::resolved_static_fallback_accepts_package_qualified_receiver_owner`
and
`lower::tests::resolved_static_fallback_rejects_shadowed_package_qualified_receiver_owner`.
The unresolved source-summary static fallback now uses that same candidate
builder too, so package-qualified receiver syntax is no longer a second
bare-variable-only owner path. Regressions:
`lower::tests::source_context_static_fallback_accepts_package_qualified_receiver_owner`
and
`lower::tests::source_context_static_fallback_rejects_shadowed_package_qualified_receiver_owner`.

Follow-up 2026-09-07:
`source_overloads_with_colliding_type_suffixes_get_distinct_rust_names` and
`source_generic_class_preserves_type_params_and_interface_bounds` were stale
assertion rows. The former now sees fallible array allocation in the call text;
the latter correctly returns `Option<String>` for Java `String`. Both generated
crates compile, so both rows were removed from the idiom-gaps failure baseline.

Follow-up 2026-09-07: static-owner Rust-name rendering helpers are crate-local.
`rust_static_owner_type_name` and `rust_static_owner_type_name_with_facts` still
centralize same-crate fallback owner spelling, but callers outside the crate can
no longer rebuild static owner identity from arbitrary string text through this
API.

Follow-up 2026-09-07, updated 2026-09-10: the raw source-owner key construction
surface is crate-local too. `JavaOwnerKey`, `source_method_for_owner`, and
`resolved_source_method` remain available to internal resolver/lowering paths.
The source callable-key constructors that derive owner identity from `TypeDecl`
are crate-local as well, so arbitrary external callers cannot construct source
callable identity from either owner strings or declarations.

Follow-up 2026-09-10: `SignaturePlan::has_method_signature_by_base_name_accepting_arity`
and `SignaturePlan::method_signature_by_base_name_accepting_arity` are
crate-local. Lowering can still use the unique base-name compatibility probes,
but the non-injective Rust-name/base-name lookup is no longer public API.
Follow-up cleanup narrowed the remaining base-name overload-family probes
(`method_signature_by_base_name_and_arity`,
`method_signature_by_base_name_and_arg_types`, and
`method_signature_by_base_name_and_arg_types_with_facts`) to crate-local too;
`slot_shape_design` no longer calls those compatibility probes directly.

Follow-up 2026-09-07: source method Rust-name helpers are crate-local too.
Usage, exception, inheritance, and lowering still share `source_method_rust_name`
and `source_method_overload_rust_name`, but callers outside the crate cannot
recompute source method identity from a standalone `MethodDecl` through these
helpers.

Follow-up 2026-09-07: erased Java signature utility functions are crate-local.
`erased_param_types`, `erased_type_name`, and `method_staticness` still
centralize internal callable-key construction and source-summary matching, but
external callers cannot assemble partial erased-signature identity directly.

Follow-up 2026-09-07: two package-identity regressions found by the slot-shape
failure-name audit are fixed. Erased-object display downcasts are now derived
from the modeled external fact receiver identity only; importing
`a.ExternalToken` no longer grants the same `toString` capability to the bare
`ExternalToken` carrier. Context source constructors are again suppressed as
external facades, while a zero-arg constructor fallback is emitted on static
constant support owners when the primary source constructs that owner. Focused
regressions:
`slot_shape_design::erased_external_object_display_keeps_package_qualified_to_string_capability_distinct`,
`slot_shape_design::cotranslated_same_simple_name_package_peers_keep_package_identity`,
and `slot_shape_design::context_static_string_constant_owner_stays_sibling_source_type`.

Follow-up 2026-09-07, updated 2026-09-12: the legacy Rust-name usage-family map
is private to `usage.rs`. `UsageFacts::methods_by_rust_name` is still
maintained for internal compatibility scans, but the remaining family binding
lookup lives behind `UsageFacts::binding_facts_for_rust_method_family`, and
integration tests/external callers must use Java-keyed source-method accessors
instead of depending on the non-injective map directly.

Follow-up 2026-09-10: `call_plan::external_call_has_thrown_effect_for_receiver`
is crate-local. The exact rust-name-mapped external-pack predicate is tested
beside `call_plan`; integration coverage stays on the public fallibility
fixpoint instead of reaching into the compatibility probe.

Follow-up 2026-09-10: `call_plan::external_call_returns_rust_result` is
crate-local. The direct facade-ABI fallibility assertions moved beside
`call_plan`; integration coverage now checks argument shaping, target planning,
and the public program-fallibility fixpoint instead of calling the wrapper.

Follow-up 2026-09-10: `stdlib::external_facade_receiver_call_is_noop_method` is
crate-local. Call planning still uses the facade no-op predicate internally, but
it no longer exposes another public string-keyed method classification surface.

Follow-up 2026-09-07: method-reference planning now resolves imported source
owner aliases before rendering the Rust owner path. `Mapper::tag` with
`Mapper -> a.Mapper` in the shape environment and a known context type now plans
against `a.Mapper`, producing `AMapper::tag` instead of the ambiguous bare
`Mapper::tag`. External/JDK aliases still fall through to the existing planner
when no context source type resolves. Guarded by
`lower::tests::method_reference_static_type_alias_uses_resolved_context_owner`
and the method-reference source-layout guard.

Follow-up 2026-09-07: method-reference plans no longer store rendered Rust owner
paths for constructors or associated functions. `plan_method_reference` now
preserves the Java/source owner spelling (`a.Mapper`, `K.A.B`, etc.), and
`method_reference_lowering` renders the Rust owner path only at expression
construction. The shape path canonicalizes the stored source owner for the
`Integer`/`String`/`Object` special cases. Guarded by
`method_reference_plan::tests::method_reference_owner_preserves_package_and_nested_identity`,
`method_reference_lowering::tests::method_reference_renderer_and_body_helpers_stay_structured`,
`lower::tests::method_reference_static_type_alias_uses_resolved_context_owner`,
and `slot_shape_design::method_reference_plan_owns_generic_function_adapter_shapes`.

Follow-up 2026-09-07: `SignaturePlan`'s exact Rust-name lookup helpers are now
crate-local. Lowering, call planning, and expression-shape compatibility paths
can still use `method_signature_by_rust_name`,
`constructor_signature_by_rust_name`, `method_arg_shape_by_rust_name`,
`constructor_arg_shape_by_rust_name`, and `method_return_shape_by_rust_name`,
but external integration tests now use source declarations or public base-name
queries. The former public exact Java-key signature surface is now
test-only/crate-local; resolved return-shape recovery uses the crate-local
`resolved_source_method_return_shape` helper.

Follow-up 2026-09-09: the `slot_render::rust_name_for_java_named_type`
compatibility wrapper is deleted. `slot_render` still uses the canonical
projection internally, but imports it from `java_naming`; other modules now call
`java_naming::rust_name_for_java_named_type` directly instead of routing
Java-name identity through a slot-rendering facade. The layering guard rejects
reintroducing the wrapper.

Follow-up 2026-09-09: `java_naming::rust_name_for_java_named_type` is now
crate-local. The last external caller was CLI facade-owner alias lookup, and
that path already passes the final simple Java segment; it now uses that simple
owner directly before checking manifest aliases instead of keeping the generic
string-only type projection public. This still does not replace the internal
model with a first-class Java type key, but it removes one more public way to
recompute type identity from arbitrary text.
Follow-up cleanup also narrowed the source binding and source field Rust-name
allocators in `java_naming` to crate-local, keeping source-name collision policy
centralized without exporting those string-projection helpers as crate API.
Follow-up 2026-09-12 made the `callable_analysis`, `java_naming`, and
`inherited_members` modules crate-internal as well. The same package/name
compatibility policies remain available to analysis and lowering, but they no
longer appear as public crate modules while first-class owner identity remains
open.

Follow-up 2026-09-09: context type-key lookup no longer keeps parallel
type-only and type-plus-program collector trees. Ordinary and emitted context
lookups now collect `(TypeDecl, Program)` matches once and derive type-only
answers from that result, so exact/alias/simple ambiguity handling has one
owner-aware implementation per matching policy.

Follow-up 2026-09-09: call-graph/fallibility source callable keys now use
`CallableKey::source_method_for_type` and
`CallableKey::source_constructor_for_type`. Callers that already hold a
`TypeDecl` no longer rebuild the owner string with `ty.name.clone()` before
asking for callable identity. The older string-owner constructors remain for
inherited/compatibility paths that only have a resolved owner spelling.
Follow-up cleanup narrowed those source callable constructors to crate-local:
they remain the internal way to avoid ad hoc source owner strings, but external
callers no longer get public constructors for source callable identity.

Follow-up 2026-09-09: the package-qualified static-constant batch regression now
also asserts that `Main.java` does not receive either historical ambiguous alias
spelling (`pub type Item = AItem;` or `pub type Item = BItem;`). The ambiguity
filter already suppressed those aliases; the test now pins the actual old failure
shape rather than only checking for a stray `pub struct Item;`.

Follow-up 2026-09-09: `ProgramExceptionPlan::method_plan` no longer falls back
from the canonical source-method key to `(ty.name, method.name)`. The plan's only
producer stores `(ty.name, source_method_rust_name(method))`, so the Java-name
fallback was stale and could silently select a wrong entry for renamed or
overloaded methods. Regression:
`exception_plan::tests::method_plan_uses_only_canonical_source_method_key`.

Codex follow-up 2026-09-10: `JavaCallableKey`, `JavaCallableSlotKey`, their
method/slot owner/name accessors, and the method-only callable kind/staticness
enums are crate-local now. `UsageFacts::method_usage_by_key` and
`UsageFacts::constructor_usage_by_key` were narrowed to crate-local as well, so
external users can no longer treat arbitrary string-key lookup as public usage
identity. The internal migration shell still uses these exact lookups where it
owns the key. Guard:
`layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.
Codex follow-up cleanup also narrowed `UsageFacts::method_entries`,
`constructor_entries`, `method_usages`, and `constructor_usages` to crate-local.
The last integration test that used method traversal now asks for the specific
source declaration via `source_method_usage(ty, method)`, matching the
declaration-aware boundary instead of scanning all usage records.
Codex follow-up cleanup added `facts::FieldKey::named` and
`facts::FieldKey::source_field`. Source-derived trivial-accessor field facts,
field-nullability facts, parsed-source field declaration facts, folded source
constant facts, external-coverage inferred field facts, and builtin JDK field
facts now construct field identity through that named boundary instead of local
two-string `FieldKey` literals. Later cleanup moved the remaining integration
fixtures through `FieldKey::named(...)` as well, and private `FieldKey` payload
fields now enforce that boundary.
Follow-up cleanup extended that same boundary to external facts and production
external-pack expansion: unsupported-field rows, trivial accessor rows, field
rows, field-nullability rows, modeled pack fields, modeled field values,
unsupported modeled fields, and builtin pack field-nullability seeds now all
use `FieldKey::named(...)` instead of local field-key literals. Test fixtures
now use named constructors even when the fact payload is the subject under test.
Follow-up cleanup added `facts::TypeKey::named` plus
`TypePattern::exact`/`TypePattern::erased`. Production type-pattern construction
in external API facts and JDK facade ABI seeds no longer builds local
`TypeKey { type_name: ... }` literals; the remaining literal type keys are
test/fixture payloads or inside the owner constructors themselves.
Follow-up cleanup added `facts::MemberName::named` and moved the internal
`facts::CallableKey` constructors through `TypeKey::named` and
`MemberName::named`. The callable-key constructors still preserve the existing
string payloads, but the owner/member literal construction is centralized at the
same boundary as field and type-pattern keys.
Follow-up cleanup added read accessors for `facts::TypeKey`,
`facts::MemberName`, and `facts::CallableKey`, then migrated production fact
lookups, facade ABI matching, and builtin external-pack key matching away from
direct `.owner.type_name`, `.member.0`, and callable-parameter reads. The
layering guard now rejects those read patterns in the migrated production
modules. A follow-up replaced the remaining external-pack alias-expansion owner
mutation with `CallableKey::with_owner_type`, so the migrated modules no longer
reach into callable-key owner/member/type-pattern payloads for lookup or alias
rewrites.
Follow-up cleanup made `facts::TypeKey.type_name`, `facts::CallableKey`'s raw
owner/member/kind/staticness/arity/parameter fields, and the `MemberName` tuple
payload private. Callers still construct and query callable keys through the
named factories/accessors, and construct type patterns through
`TypePattern::exact`/`erased`, but cannot fabricate fact-table callable/type
identity by assigning the raw fields directly.
Follow-up cleanup fixed the arity-only call-target fallback for normalized
static utility owners. Lowercase recovered owners such as `math.pow(...)`,
`double.isInfinite(...)`, and `system.arraycopy(...)` now render through the
canonical owner key (`Math`, `Double`, `System`) when no local/field binding
shadows them, while `foo.toString()` remains an instance fallback instead of
being promoted to a fabricated static `Foo::to_string` call. Native static
rendering remains enabled when the loaded stdlib facts know the native owner
family, so ABI-backed `Math.pow` still uses `NativeJavaStatic`. Guard:
`slot_shape_design::call_plan_recovers_normalized_static_utility_owner_receivers`.
Follow-up cleanup moved lower.rs source-method result/fallibility identity
queries behind `CallableAnalysis::source_method_identity_returns_result`.
Lowering no longer constructs `CallableKey::method_identity(...)` directly for
raw `(type_name, method_name)` pairs; the remaining bridge is centralized in
`CallableAnalysis` while the broader string-key identity model remains open.
Guard: `layering::callable_analysis_shell_reaches_lowering_context`.
Follow-up cleanup also moved the resolved-source super-call fallibility query
behind `CallableAnalysis::resolved_source_method_returns_result`. Lowering no
longer fabricates `CallableKey::resolved_method(...)` directly; resolved owner
identity still flows through the existing key model, but the lowerer now asks
the callable-analysis shell instead of constructing that key itself. Guard:
`layering::callable_analysis_shell_reaches_lowering_context`.
Follow-up cleanup moved constructor fallibility queries behind
`CallableAnalysis` too. Source constructor emission now asks
`source_constructor_returns_result`, and constructor expressions ask
`constructor_expr_returns_result` for either a resolved source constructor or an
external constructor boundary. `lower.rs` no longer constructs
`CallableKey::source_constructor_for_type`, `CallableKey::resolved_constructor`,
or `CallableKey::ExternalConstructor` directly.
Follow-up cleanup added named `usage::CallableKey::external_method` and
`external_constructor` constructors, then moved call-graph construction,
callable-analysis fallibility queries, and the external fallibility integration
fixture off raw external callable enum literals. Pattern matches that inspect
existing graph nodes still destructure the enum; key creation now has a named
boundary for the external-callable string fields.
Follow-up cleanup fixed a source/JDK native static owner collision introduced
while prioritizing native static helpers. A resolved source class named `Math`
with `static floorDiv(...)` was being planned as `java.lang.Math.floorDiv`,
changing `"42"` into a catchable divide-by-zero path. Native static planning and
the lower-side facade/static-owner retarget now fail closed when the resolved
owner or static alias points at a source-program owner; known qualified JDK
owners such as `java.lang.Math` still use the native helpers. Guards:
`slot_shape_design::resolved_source_static_owner_shadows_native_jdk_static_owner`
and
`idiom_gaps::source_math_static_owner_shadows_native_jdk_math_helpers`.
Follow-up cleanup moved direct source-declared field consumers in constructor
planning, shape-env seeding, struct assembly, and static-storage lowering from
raw `(type_name, field_name)` signature lookups to
`SignaturePlan::required_declared_field_shape(ty, field)`. Inherited and
external inherited fields still use the lowered-owner compatibility lookup
because those signatures are intentionally keyed under the child owner.
Follow-up cleanup narrowed the dirty-tree `ArrayBlockingQueue` helper
predicates in `stdlib.rs` to crate-local:
`java_type_is_array_blocking_queue` and
`program_mentions_java_array_blocking_queue`. They are implementation details
of the current native-collection lowering/slot-rendering path, not public
type-identity queries. Guard:
`layering::stdlib_erased_display_downcast_collectors_stay_internal`.
Follow-up cleanup removed the remaining public `UsageFacts` rust-name method
compatibility helpers. `method_usage_by_current_rust_name` call sites in
integration tests now resolve `TypeDecl`/`MethodDecl` and query
`source_method_usage(ty, method)`, while synthetic insertion cases were replaced
with source fixtures analyzed through the normal usage walker. Guard:
`layering::usage_keys_hide_raw_owner_and_name_fields`.
Follow-up cleanup narrowed the public external facade ABI naming boundary.
`call_plan::external_method_rust_name` and
`unresolved_constructor_rust_name_for_facts` are crate-local implementation
details now; the binary reaches them through
`external_facade_method_rust_name_for_abi` and
`external_facade_constructor_rust_name_for_abi`, which state the external-pack
ABI probing question instead of exposing arbitrary unresolved Rust-name
projection as public API. Guard:
`layering::callable_analysis_shell_reaches_lowering_context`.
Follow-up cleanup narrowed the external API effect rust-name fallback too.
`FactSet::external_api_effect_for_receiver_with_arity_or_rust_name` is
crate-local now; production callers in aliasing, usage, and exception planning
can still use the compatibility bridge, while integration tests no longer get a
public owner/Rust-name fallback query. The exact simple-owner ambiguity
assertion moved beside `facts.rs`. Guard:
`layering::usage_keys_hide_raw_owner_and_name_fields`.

Follow-up cleanup narrowed the keyed external argument-effect lookup. The only
production caller is the in-crate call-planning bridge, so
`FactSet::external_api_argument_effect_for_key` is crate-local now instead of a
public API for ad hoc `CallableKey` construction. A later cleanup removed the
remaining exact-key `FactSet` lookup helpers after integration tests moved to
call-shaped, constructor-shaped, or field-shaped public lookups; this
argument-effect helper was the first internal call-planning bridge narrowed.
Guard:
`layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.

Follow-up cleanup narrowed `SignatureDependencyEdge::new` to crate-local. The
dependency graph still exposes public read-only `nodes()`, `edges()`, and edge
accessors, but external callers can no longer construct arbitrary dependency
edges from raw callable/type identity nodes. Guard:
`layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.

Follow-up cleanup narrowed the local fallibility-invariant violation surface
too. `report_fallibility_violations` is now an internal diagnostic entry point
through the crate-internal `fallibility_invariant` module; `FallibilityViolation`,
its accessors, and the raw `fallibility_violations` query are crate-local
implementation details. Guard: `layering::usage_keys_hide_raw_owner_and_name_fields`
and `layering::declaration_context_usage_lookups_use_java_callable_keys`.

Follow-up cleanup narrowed the inherited-member result/query surface to
crate-local callers. Source inherited field/method refs and external inherited
field/method records still cross internal module boundaries, but their declaring
owner, Java member name, emitted Rust name, type/nullability, arity, and fact
payloads are private behind crate-local constructors and accessors. Constructor
planning, item assembly, lowering, signature planning, and stdlib facade/static
final emission now consume those answers through the inherited-member API
without exporting inherited owner/name identity as public crate API. Guard:
`layering::usage_keys_hide_raw_owner_and_name_fields`.
The constructor-plan helper that accepts raw inherited field refs is crate-local
too, matching the visibility of the ref carrier.

Follow-up cleanup narrowed the static-final constant folding surface. The
multi-file pooling path still uses `static_final_constant_declarations`,
`constant_value_facts_from_declarations`, and `fold_constant_declarations`
inside the crate, but the public `resolve` module no longer exposes
`ConstantDeclaration` or those raw folding helpers as downstream API. Guard:
`layering::declaration_context_usage_lookups_use_java_callable_keys`.
Follow-up cleanup also made the `source_constant_values` module private and
narrowed its `ConstantValue`, `ConstantScope`, and `fold_constant_expr` API to
crate-local callers. `resolve` and `lower` can still share the constant folding
implementation, but downstream users cannot depend on the raw constant-scope
owner/name compatibility model as public crate API.

Follow-up cleanup narrowed the member-access planning records. Public callers
still get `java_member_access_policy_with_facts` and
`plan_member_access_with_facts`, but `JavaMemberAccessPolicy` and
`MemberAccessPlan` no longer expose raw member/type/shape/op fields for direct
construction or mutation. Access-lowering, shape inference, stream lowering,
and tests now read the plan through accessors. Guards:
`layering::member_access_plan_stays_fact_and_shape_based` and
`slot_shape_design::access_lowering_owns_bare_field_and_member_access_fallbacks`.

Follow-up cleanup removed the remaining exact-key lookup helpers from `FactSet`:
`external_api_effect_for_key`, `external_constructor_for_key`, and
`external_field_for_key`. Integration tests now use call-shaped,
constructor-shaped, or field-shaped public lookups on parsed facts. A later pass
also deleted the builtin-pack exact-key probes
`external_packs::external_api_effect_for_key`,
`external_packs::external_constructor_for_key`, and
`external_packs::external_field_for_key`. Builtin-pack tests now assert through
the shaped constructor and field lookups too. Guard:
`layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.

Follow-up test-surface cleanup moved the external-pack facade ABI override test
off direct `FactSet::facade_abi_for_key` lookup and onto the shaped
`facade_abi_for_external_method(owner, method, arity)` query. The raw key lookup
still exists for production paths that already have exact resolved/source
callable identity, but this removes one integration-test dependency on ad hoc
key construction.

Follow-up cleanup moved the stdlib Optional, collection, and known JDK
content-method facade ABI integration checks off temporary
`CallableKey::method(...)` handles too. Those tests now query
`facade_abi_for_external_method(owner, method, arity)` directly for simple
owner/method/arity facts. Typed overload checks that require
`method_with_params` still use `facade_abi_for_call`.

Follow-up cleanup applied the same boundary to simple production facade ABI
queries in `stdlib` and the unresolved call-argument path in `call_plan`.
Callers that already have only `(owner, method, arity)` now ask
`facade_abi_for_external_method` instead of fabricating a `CallableKey` for
lookup. Resolved-source and typed-overload paths still use exact key/call
lookups because they carry more identity than the shaped external-method query.

Follow-up cleanup narrowed `FactSet::facade_abi_for_key` to crate-local. Public
callers keep the shaped `facade_abi_for_external_method` query and the typed
`facade_abi_for_call` overload query; only in-crate resolved/source call
planning can use the exact-key fallback now. Guard:
`layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.

Follow-up cleanup narrowed `call_plan::facade_abi_argument_plan_for_rust_path`
to crate-local too. The rendered Rust path remains an internal bridge used by
lowering after a facade target is selected; integration coverage moved beside
the planner so public callers are not encouraged to query ABI facts by emitted
path spelling. Guard:
`call_plan::tests::facade_abi_argument_plan_prefers_abi_slot_over_argument_effect`.

Follow-up cleanup narrowed `slot_render::native_named_java_rust_type_policy`
and its `NativeNamedJavaRustTypePolicy` carrier to crate-local. Public callers
keep the fact-aware `native_java_rust_type_policy_with_effective_facts` and
rendered-type APIs; tests no longer query the Java-name-to-Rust-name projection
through the raw named-type helper. Guard:
`layering::slot_render_policy_symbols_stay_effective_fact_aware`.

Follow-up cleanup private-backed `RustNominalTypeNamePolicy`. The nominal
runtime target projection is still exposed for planner inspection through
read-only `java_ty()`, `target_type()`, and owned `into_target_type()`
accessors, but callers no longer construct or mutate the raw Java-type/name
payload. Guard:
`layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup narrowed the Broad/IO facade support predicates in
`javalibs`: `is_builtin_external_facade_type`,
`broad_utils_dynamic_return_type`, `broad_static_utility_return_type`,
`imported_static_method_returns_broad_utils_first_arg`,
`is_broad_utils_type_name`, `needs_conversion_constructor_for_facade`,
`is_static_facade_method`, `is_jdk_input_stream_facade_type`,
`is_known_java_subtype`, and `is_known_exception_supertype` are crate-local now,
and the `javalibs` module itself is crate-internal. The facade construction,
return-type, static-method, subtype, exception-supertype, and stream-facade
compatibility probes are internal translator hooks. Guard:
`layering::stdlib_does_not_grow_non_jdk_domain_package_knowledge`.

Codex follow-up 2026-09-12 private-backed
`facts::HierarchyReachabilityDiagnostic`. Hierarchy reachability still reports
the same budget-exhaustion diagnostic, and semantic analysis still bridges it
into `SemanticDiagnostic`, but downstream callers can only read `code()` and
`message()`; construction stays inside the crate and the raw `{ code, message }`
payload is private at the hierarchy/facts boundary. Guard:
`layering::declaration_context_usage_lookups_use_java_callable_keys`.

Codex follow-up 2026-09-12 private-backed resolver overload applicability
reports. `resolve.rs` still owns report mutation while scoring candidates, but
`OverloadArgumentApplicability` and `OverloadApplicabilityReport` now expose
mode, score, argument conversions, and rejection reasons through accessors
instead of public record fields. This fences another overload-decision surface;
it does not claim to replace the remaining string owner model. Guard:
`layering::declaration_context_usage_lookups_use_java_callable_keys`; focused
behavior guard:
`cargo test --test resolve_calls overload_applicability_reports`.

Codex follow-up 2026-09-12 narrowed the ownerless
`FactSet::external_api_effect_for(method)` compatibility lookup to crate-local.
External callers and integration tests now use the shaped public
`external_api_effect_for_receiver(receiver, method)` query, passing `None` only
for intentionally receiverless external facts. The internal ownerless helper
still exists for compatibility paths that already know there is no receiver,
but it is no longer advertised as a public method-name-only fact API. Guard:
`layering::callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating`.

Codex follow-up 2026-09-12 moved the `external_field_value_for` lookup logic
onto `ExternalFactLookupIndex` as well. `FactSet::external_field_value_for`
remains as a shaped compatibility wrapper, but it now delegates to the index.
Static external field values therefore share the same alias-indexed
exact/direct/fallback owner matching shape as external fields instead of
carrying a separate raw scan in `FactSet`. Guards:
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`
and `semantic_facts::fact_lookup_refuses_ambiguous_simple_package_owner_aliases`.
