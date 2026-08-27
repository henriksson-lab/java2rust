# 0061 — A local binding has no identity, so the plan is rejoined to the declaration by name, four times, differently

Status: **open, compatibility-only residue**

Partial progress:

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
  `duplicate_local_names_use_scoped_occurrence_shape_before_map_insert_conversion`.
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
finding is gone in the current tree. `ShapeEnv::next_local_plan_for_declared_type`
no longer advances `local_plan_occurrences` or saturates/overruns through a
shared mutable counter; it accepts only a unique exact `(name, declared type)`
compatibility match, with a narrow erased-object boundary special case, and
otherwise fails closed. The one production caller is explicitly named
`idless_compatibility_local_plan_for_declaration`, and `tests/layering.rs` now
guards both the single caller and the absence of the old cursor symbols. The
  audit remains open for the remaining compatibility surface: the legacy
  `MethodUsageFacts.bindings` map is still keyed by name, and a few id-less or
  synthetic paths still accept name fallback by explicit policy.
- Follow-up 2026-08-26: signature's remaining local-usage helper no longer
  reaches directly into `MethodUsageFacts.bindings` to implement the id-less
  fallback. The policy is centralized on
  `MethodUsageFacts::binding_for_declaration_with_name_fallback`, so bound
  declarations, id-less compatibility callers, and duplicate-name fail-closed
  callers all use the same gate. Focused regressions:
  `usage::tests::binding_for_declaration_does_not_fall_back_by_name_when_id_is_known`,
  `signature::tests::binding_usage_for_bound_local_does_not_fall_back_by_name`,
  `layering::idless_local_binding_compatibility_surface_stays_explicit`, and
  `layering::signature_uses_usage_binding_policy_for_legacy_name_fallback`.
- Follow-up 2026-08-26: `expr_shape`'s id-less shape-learning helper now shares
  the same unique-match policy as `ShapeEnv::next_local_plan_for_declared_type`.
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

Dimension: phase architecture / information lost between passes.

## Symptom

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

`src/signature.rs:97-103`:

```rust
pub struct LocalPlan {
    pub binding: BindingId,
    pub name: String,
    pub java_ty: JavaType,
    pub shape: RustSlotShape,
    pub binding_mutable: bool,
}
```

`src/usage.rs:117-124`:

```rust
pub struct MethodUsageFacts {
    pub bindings: HashMap<String, BindingUseFacts>,   // keyed by *name*
    pub bindings_by_id: HashMap<BindingId, BindingUseFacts>,
    ...
}
```

The original finding that neither type carried declaration identity is stale:
`BindingId` is now plumbed through `java_ir`, `resolve`, `usage`, `signature`,
`expr_shape`, and `lower`. The remaining defect is the compatibility surface:
`LocalPlan` still carries a separate `name`, `MethodUsageFacts.bindings` remains
as a name-keyed map, and some id-less/synthetic paths can still rejoin by
`(name, type)` occurrence.

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

2. **`signature` detects the collision and discards the facts.**
   `collect_duplicate_declared_local_names` (`src/signature.rs:9917-9924`)
   counts declarations by name; `collect_local_constraint_sets_from_stmts`
   then does, at `src/signature.rs:10004-10008` and again at `:10068-10072`:

   ```rust
   let usage = if duplicate_names.contains(name) { None } else { usage };
   ```

   The name is enough to disable *all* usage-derived constraints for *every*
   local with that name — external receiver mutability, argument boundary,
   native-collection receiver, map-get value shape, and `MutableBinding`.

3. **`signature` rejoins its own two walkers by `.find()`.**
   `collect_local_constraint_sets_from_stmts` (`:9984`) and
   `collect_local_plans_from_stmts` (`:10248`) walk the same body in the same
   order, but the second does not use position: for each declaration it calls
   `local_constraints_for_declared_type` (`:10410`), which is
   `constraints.iter().find(|c| c.name == name && c.java_ty == *ty)` with a
   name-only `.find()` fallback (`:10399`). N locals sharing a name *and* a
   declared type all receive the **first** one's constraint set, discarding the
   per-declaration work done a few hundred lines earlier
   (`local_initializer_requires_owned_storage`,
   `initializer_proves_declared_non_null`, capture constraints).

4. **Lowering now has an explicit id-less compatibility helper.**
   The original shared mutable cursor has been removed. Current
   `expr_shape::ShapeEnv::next_local_plan_for_declared_type` filters the plan
   slice by `(name, declared type)` and returns a plan only when the match is
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

There is no `BindingId` on the IR node. A `Stmt::VarDecl`
(`src/java_ir.rs`) carries `ty`, `name`, `source_name`, `initializer` and no
identity, so every phase that wants to say something *about a particular
declaration* has to key it by something the IR does expose. Name is the only
candidate, and name is not unique within a method body.

Because the key is lossy, each phase repairs the lossiness in its own way:
`usage` merges, `signature` gives up (mechanism 2) or takes the first
(mechanism 3), and lowering assumes traversal-order lockstep (mechanism 4).
Each repair is locally defensible and they compose into different answers.

The `Option`/absence question is the same bug in a second form: a `LocalPlan`
lookup returning `Some(plan)` cannot distinguish "this is the plan for this
declaration" from "this is some plan for some declaration that shares this
name". `next_local_plan_for_declared_type` returns `Option<&LocalPlan>` where
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
   `expr_shape::next_local_plan_for_declared_type`,
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
repairs. Those changes reduce known symptoms; they do not give local
declarations identity.

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
  binding-id lookup. `MethodUsageFacts::binding_for_declaration` treats a known
  resolver id as authoritative: if the id has no entry, it returns `None`
  instead of silently falling back to the same-name compatibility map. Id-less
  callers still keep the legacy fallback. Regression:
  `usage::tests::binding_for_declaration_does_not_fall_back_by_name_when_id_is_known`.
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
- `expr_shape::ShapeEnv::next_local_plan_for_declared_type` still exists for
  id-less synthetic compatibility paths, but lowering no longer falls through to
  it when a source declaration has a `BindingId` and the binding-keyed plan lookup
  misses, and the compatibility lookup no longer reuses a same-name local plan
  with a different erased declared type identity. Shape learning for bound
  `VarDecl` and `EnhancedFor` declarations now has the same rule: binding-keyed
  shape or conservative fallback, never same-name rejoin; id-less shape learning
  likewise requires the same erased declared type identity, with erased-carrier
  locals allowed to match through their solved concrete shape.
- `SignaturePlan` now has binding-based local plan/shape accessors, but the
  older name-based compatibility accessors are still used by callers that do not
  yet have declaration ids.
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
and `duplicate_local_names_use_scoped_occurrence_shape_before_map_insert_conversion`.

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

Follow-up 2026-08-25: the id-less synthetic local-plan cursor no longer reuses
the first same-name erased-object plan forever. `ShapeEnv::
next_local_plan_for_declared_type` now consumes the `(name, declared-type)`
occurrence before selecting an erased-boundary match, so repeated compatibility
lookups advance to the next matching plan and overrun fails closed. Regression:
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

Follow-up 2026-08-25: method and constructor parameter nullability now use the
same binding-keyed source-nullability map. The public
`source_binding_nullability_constraints(block, name)` helper remains the
explicit compatibility/name view and still fails closed when duplicate local
names make the name ambiguous, but parameter shaping calls the declaration
helper with the parameter's resolver `BindingId` and the callable's parameter
scope. Regression:
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
`next_local_plan_for_declared_type` call is now fenced behind an explicitly
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
  occurrence cursor. `ShapeEnv::next_local_plan_for_declared_type` now accepts a
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
  only production `next_local_plan_for_declared_type` call must stay behind the
  named `idless_compatibility_local_plan_for_declaration` helper, and
  `collect_duplicate_declared_local_names(block)` must remain limited to the
  two current fail-closed/guard call sites.

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
`method_assigns_to_binding(method, name)` scan remains only for id-less
compatibility inputs. Regression:
`slot_policy::tests::borrowed_return_param_policy_uses_binding_facts_not_name_assignment_scan`.

Follow-up 2026-08-25: internal returned-object alias constraints now make the
same distinction. The shared-borrow return constraint is selected by iterating
declared parameters and resolving each through `binding_for_declaration`,
instead of scanning `all_binding_facts()` for any same-name binding and then
vetoing with `method_assigns_to_binding(method, name)`. Resolver-bound params
therefore cannot be rejected by a same-name shadow local; mutation of the exact
parameter binding still rejects through `BindingUse::Mutated`. Regression:
`signature::tests::returned_object_alias_constraints_use_param_binding_not_shadow_name`.

Follow-up 2026-08-26: a stale `slot_shape_design` assertion was corrected to
match the narrowed source-nullability compatibility surface. Resolver-bound local
declarations are removed from the public name-only nullability view and must be
shaped through binding-keyed signature collection; the no-params public helper
still exposes method parameters by name because it has no parameter identity
input. Regression:
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
