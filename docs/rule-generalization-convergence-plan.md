# Rule Generalization Convergence Plan

Date: 2026-07-25

## Goal

Reduce the number of local lowering rules by turning repeated Rust ABI,
optional-carrier, clone-recovery, and facade-boundary decisions into shared
facts, plans, or helpers. A temporary corpus error-count increase is acceptable
when it exposes missing facts through one shared path instead of preserving many
contradictory special cases.

This plan extends the design priority in `docs/corpus-fix-next-plan.md`.

## Principles

- Facts beat rendered-string checks. Lowering should not decide semantics from
  `starts_with("&")`, `ends_with(".clone()")`, or Rust type-name substrings
  when the same decision can be represented as a slot, facade ABI, or lowered
  expression shape.
- `CallableKey` remains the identity for external and facade behavior. Partial
  matching is allowed only as an explicit fallback with provenance.
- `FacadeAbiFact` and Rust signature facts should own receiver, argument,
  return, mutability, fallibility, and borrow-source choices for Rust-facing
  APIs.
- `LoweredType` plus `SlotConversionPlan` should own conversion of Java
  semantic values into Rust slots. Local helpers may construct carriers, but
  should not choose final borrow/clone/unwrap spelling.
- Java semantic rules stay in the translator. Package-specific library behavior
  and shim ABI details stay in `javalibs` manifests/facades.
- If a tailored rule only reproduces a Rust signature's receiver, argument, or
  return ABI, replace it with a signature fact. Keep tailored rules only for
  Java syntax, Java semantics, or migration bridges with a removal note.

## Hotspots Found

### Optional Carriers

`src/lower.rs` still contains several local optional adapters:

- optional array/slice argument paths;
- map `get` optional target paths;
- field getter and constructor recovery paths;
- nullable ternary and branch construction;
- stream argument paths that rebuild borrowed string or owned argument logic.

Some of these already route through `LoweredType::OptionalElementBorrow` and
`plan_lowered_slot_conversion`; the remaining problem is that carrier creation
is inconsistent. The translator still locally chooses between `Some(value)`,
`Some(&value)`, `.as_ref()`, `.as_deref()`, `.as_mut()`, `.cloned()`, and
null-receiver unwraps.

Replacement direction:

- add a shared `OptionalProjectionPlan`/`NullableCarrierPlan` or extend
  `LoweredType` with explicit
  carrier origins such as `OptionValue`, `OptionElementBorrow`, `MapGet`,
  `FieldStorage`, `ArrayAccess`, `FacadeReturn`, and `Conditional`;
- make every nullable path return `LoweredExpr { expr, actual }` before slot
  adaptation;
- make `SlotConversionPlan` choose the final borrow, unwrap, clone, or
  ownership recovery spelling;
- keep raw rendered optional detection only as a legacy migration fallback.

### Borrow And Clone Recovery

Repeated code checks rendered Rust such as `.clone()`, `.cloned()`,
`.clone_box()`, `&mut name`, `.as_mut_slice()`, and `.as_ref().expect(...)`.
This is useful as a guard, but it should not be the source of truth.

Replacement direction:

- introduce `CloneRecoveryDecision` or equivalent, taking source expression,
  actual shape, target shape, and use context, returning `None`, `Clone`,
  `CloneBox`, `ToVec`, `Copy`, or `Unsupported`;
- add a `MoveUseFact`/`MoveUseContext` summary for later reads,
  current-statement repeated reads, catch future use, and same-call alias
  conflicts, so clone recovery does not rescan every local rule site;
- introduce a structured expression-shape tag, for example `RustExprShape`, for
  `Borrowed`, `MutBorrowed`, `OwnedRecovery`, `OptionalProjected`,
  `SliceView`, `StringView`, and `ResultPropagated`;
- keep the existing rendered helpers as compatibility shims that attach or
  recover a shape tag when legacy `RustExpr::Raw` is unavoidable;
- expand `OwnedRecoveryCapability` into the authoritative check for
  clone/copy/to_vec/clone_box recovery, keyed by Java type and Rust facade type;
- require clone insertion sites to go through one helper that returns both the
  expression and a reason/provenance.

### Rust Facade And Stdlib ABI

`FacadeAbiFact` now handles important pieces such as iterator mutability and
borrowed facade returns, but several std/core/alloc and runtime helper surfaces
are still encoded through call-name-specific lowering branches.

Replacement direction:

- add a small `RustSignatureFact` extraction path for generated support,
  `javalibs` facades, and selected std/core/alloc APIs already hit by corpuses;
- lower extracted signatures into `FacadeAbiFact` records;
- cover `String`/`str`, `Vec`/slices, `Option`, `Result`, `HashMap`,
  `HashSet`, iterators, `Path`/`PathBuf`, `File`, and basic I/O incrementally;
- delete a tailored rule once a focused test proves the signature fact produces
  the same receiver/argument/return slot.

Do not attempt a comprehensive Rust stdlib database. The stdlib work is a
replacement mechanism for observed special cases.

### Borrow And View Projection

Borrow insertion appears in ordinary calls, map keys, string helper arguments,
slice arguments, mutable slice arguments, and display contexts. Those sites
currently encode different idempotence checks and parenthesization rules.

Replacement direction:

- add `ensure_borrow_projection(expr, actual, target, BorrowUse)` where
  `BorrowUse` covers normal argument, map key, string view, slice view, mutable
  slice, display, and method receiver;
- build borrow/view operations as structured `RustExpr` nodes or tagged
  lowered expressions, using rendered text only for final emission;
- migrate callers gradually, keeping the current rendered-string checks as
  compatibility fallback until parity tests prove each use mode.

### External JDK And Pack Facts

`src/external_packs.rs` has a key-based index for external API, constructor,
and field facts, but it still keeps fallback lookup maps by simple receiver,
method, arity, and aliases. `src/stdlib.rs` still has large return-type and
field-type tables for platform APIs.

Replacement direction:

- unify external API role, return type, nullability, exception, receiver
  mutability, argument effects, and facade ABI into a single keyed signature
  fact or a small family of facts that share a `CallableKey`;
- keep simple-name alias fallback explicit and diagnostic-visible;
- migrate non-JDK package rows out of `src/stdlib.rs` into `javalibs`;
- reduce procedural field typing by requiring typed manifest field facts for
  constants and public data fields.

### Stream And Method Reference Lowering

`src/stream_lowering.rs` still adapts arguments locally for borrowed strings,
owned values, index casts, and receiverless/source calls. That duplicates the
ordinary call planner and slot converter.

Replacement direction:

- make stream lambda calls produce ordinary `LoweredExpr` values and run them
  through `CallTargetPlan`, `MemberAccessPlan`, and `SlotConversionPlan`;
- make method references resolve to the same `CallableKey` lookup as ordinary
  calls;
- remove stream-only mappings for string helpers, parse helpers, getter
  guesses, and borrowed/owned argument adaptation once focused parity tests are
  green.

### Facade Borrowed Returns

Borrowed facade return recovery no longer decides path-like recovery by
matching rendered Rust type names such as `Path` and `PathBuf`.
`FacadeAbiFact.borrowed_return_recovery` selects the recovery policy, and
missing facts default to `Clone` so non-clone/path-specific behavior remains a
manifest or facade ABI gap instead of a Rust type-string heuristic.

Remaining direction:

- model recovery as `Clone`, `ToOwned`, `ToPathBuf`, `ToVec`, `CloneBox`, or
  `Unsupported` instead of the current narrower `Clone`/`ToPathBuf` set;
- report the selected `borrow_source` and recovery policy in diagnostics.

### Java Display Semantics

String concatenation, `String.valueOf`, `String.format`, optional display, and
erased-object display still duplicate Java display rules in multiple modules.

Replacement direction:

- introduce `JavaDisplayPlan` for null printing as `"null"`, borrowed `str`
  rendering, primitive `Display`, optional mapping, and erased object display;
- make arithmetic/display cleanup a property of the display plan instead of
  stripping `.to_string()` from rendered Rust;
- route string concat, format helpers, `String.valueOf`, logging, and optional
  display branches through the same plan.

### Rendered-String Compatibility Layer

Rendered-expression inspection will not disappear in one slice because many
legacy paths still use `RustExpr::Raw`. Treat it as a compatibility layer, not a
semantic API.

Replacement direction:

- centralize rendered idempotence helpers in one module;
- every helper should have a structured-shape replacement target and a removal
  condition;
- new lowering code should not add fresh rendered-string tests unless it also
  records the matching fact/shape gap.

## Implementation Phases

Landed first slice: `BorrowProjectionUse` and `ensure_borrow_projection` now
centralize basic borrow/view insertion in `src/coercion.rs`. Java string helper
arguments, owned-native argument borrowing, and map-key borrowing use this
shared entry point. The helper still keeps legacy rendered idempotence checks
for raw expressions, but new call sites should route through this API rather
than adding local `&...` prefix logic.

Local-shape preservation follow-up: initializer refinement no longer replaces a
solver-selected borrowed local with an optional owned initializer shape. This
keeps `Map.get` assignments such as `ExternalOwner.Payload payload =
payloads.get(name)` on the required `&Payload` slot and lets the map-get
lowerer perform the null check and borrow recovery in the shared slot path.

Java-display slice: `JavaDisplayPlan` and `render_java_display_operand` now own
the existing Java display/null and arithmetic-format cleanup policies used by
slot coercion, string concatenation, and `DecimalFormat.format`. This is still a
small rendering policy rather than a complete display fact model, but new
display call sites should use it instead of adding another local
`.to_string()`/`"null"`/arithmetic cleanup rule.

Null-guarded optional display follow-up: proven-present optional branches now
display the unwrapped element through `JavaDisplayPlan::ConcatValue` in both
ternary and if/else paths. This avoids routing a refined optional value through
erased-object fallback and keeps the "unwrap once, then display" behavior in
one shared policy. Remaining display work is to carry this as structured
`LoweredType`/display-plan state instead of matching rendered optional unwrap
forms.

Optional argument borrowed-target slice: optional arguments targeting optional
borrowed element slots now enter
`lower_optional_argument_for_borrowed_optional_target`, which represents the
argument as `OptionalElementBorrow` before `plan_lowered_slot_conversion`.
The older array-specific optional slice path remains as a fallback for required
slice views and migration coverage.

Clone-recovery rendering slice: `coercion.rs` now exposes shared audited clone
recovery helpers for idempotence and rendering. `lower.rs` no longer owns its
own duplicate `rendered_has_*clone*` and `render_audited_clone_recovery`
helpers, and immediate render-then-clone sites route through the expression
helper. The remaining work is the semantic `CloneRecoveryDecision` layer that
chooses whether clone/copy/to_vec/clone_box recovery is needed from source,
actual shape, target shape, and move-use context.

Clone-recovery decision slice: `lower.rs` now has a named
`CloneRecoveryDecision` for reused arguments and assignment/value slots. The
first implementation keeps the existing predicate behavior, but separates
"should recover ownership" from "how to render/idempotently apply the clone".
Next cleanup should move the move-use inputs (`later_read_names`,
`current_stmt_read_counts`, catch future uses, and exclusive branch reads) into
a compact `MoveUseContext`, then let the decision choose clone/copy/to_vec or
unsupported recovery from facts rather than only `AuditedClone`.

Move-use context follow-up: the reused argument/value clone predicates now read
later reads, repeated current-statement reads, catch future uses, exclusive
terminal branch reads, and binding use counts through `MoveUseContext`. This is
still local to `lower.rs`, but it gives the future clone/copy/to_vec decision a
single movement summary instead of reaching through `LowerCtx` at every rule
site.

Clone-recovery application follow-up: `CloneRecoveryDecision` now carries a
`CloneRecoveryPolicy` and applies ownership recovery through the shared
`Coercion::CloneRecovery` operation instead of formatting a clone expression in
`lower.rs`. Reused-move recovery remains `AuditRequired` because the audit is
about move safety at the use site, not whether the Java type is clone-capable.
The next step is to add structured recovery variants for copy, to-vec,
clone-box, and unsupported recovery when the source expression shape actually
requires those modes.

Clone-recovery coercion follow-up: `CloneRecoveryDecision` now stores the
planned recovery `Coercion` directly rather than storing only a
`CloneRecoveryPolicy` and rebuilding `Coercion::CloneRecovery` at application
time. Current behavior is still clone-only, but the decision boundary can now
grow to `CopyBorrowedValue`, `ToOwnedSequence`, clone-box, or unsupported
ownership coercions without adding new rendering branches in `lower.rs`.
Unshaped argument clone recovery now routes through the same
`apply_clone_recovery_decision` path instead of calling the audited clone
renderer directly. External-facade preserving argument lowering also delegates
its first-argument clone recovery to the same decision application helper.
These migrations reduce compatibility edges before broader copy/to-vec/clone-box
decisions are introduced.

Inventory/guardrail slice: `rendered_string_heuristics_have_centralization_guardrails`
now records the current compatibility-layer footprint for rendered-string
checks in `lower.rs`, `call_lowering.rs`, and `stream_lowering.rs`, while
asserting that borrow projection, clone recovery, Java display rendering, and
move-use summaries have shared owners. The ceilings are intentionally
non-zero; they prevent new local rendered checks from appearing while the
remaining legacy paths are migrated.

Non-JDK stdlib guardrail: `stdlib_does_not_grow_non_jdk_domain_package_knowledge`
keeps HTSJDK/GATK/HDF5/SQLite/Arrow/Spark/GenomicsDB/DISQ/Picard/Barclay and
related package strings out of `src/stdlib.rs`. The test records the two legacy
Cloud Storage identifier normalizations that still need migration, but blocks
new package/domain rows from entering the core stdlib module.

Stream borrowed-string projection slice: non-null stream lambda arguments that
target borrowed Java string content now go through
`ensure_borrow_projection_rendered(..., BorrowProjectionUse::StringView)`
instead of formatting `.as_str()` in `stream_lowering.rs`. Nullable self-field
strings still keep their explicit Java null unwrap until nullable carriers are
represented end-to-end. Stream lambda shape inference also seeds untyped
receiver calls from the stream element shape before invoking the shared
`CallTargetPlan`, preserving the `String.trim()` borrowed-view priority over
broader string-return signature facts.

Stream borrowed-string slot-conversion slice: stream lambda arguments targeting
borrowed Java string content now first build a `LoweredExpr` and invoke
`plan_lowered_slot_conversion(..., Slot::Argument(...))`, sharing ordinary
argument conversion with non-stream calls before falling back to stream-local
borrow projection. Owned stream capture/cloning remains stream-local until it can
consume the same move-use context as ordinary expression lowering.

Stream receiver-call fallback slice: stream block-lambda instance calls now route
their arguments through `emit_stream_lambda_call_args_for_receiver` instead of
formatting raw lambda arguments directly. When no callable/slot fact is available,
the fallback still recovers the impossible `*param` move for borrowed Java
`String` stream elements as `param.clone()`, while leaving primitive dereferences
available for value arguments.

Stream static method-reference slice: single-input stream method references for
known Java static calls now build a synthetic lambda argument, ask
`CallTargetPlan` and `ArgumentLoweringPlan` for the same native target and slot
adaptation used by ordinary calls, then render through `apply_call_target`.
`Integer::parseInt`/`Double::parseDouble` therefore lower to
`value.as_str().parse::<...>().unwrap()` instead of stream-local
`Integer::parse_int(...)`/`Double::parse_double(...)` fallback calls. Java
null-predicate method references such as `Objects::nonNull` remain on their
dedicated stream filter path until optional stream elements are represented as
structured nullable carriers.

Stream unbound String method-reference slice: `String::length`, `String::trim`,
and `String::isEmpty` style method references now seed the lambda receiver type
and ask `CallTargetPlan` for the same zero-argument native instance target used
by ordinary calls. Method-reference output-shape inference also has a
context-aware call-plan path, so borrowed string-view results such as
`String::trim` collect through the shared borrowed-element recovery policy
instead of falling back to `.cloned()` or a `String::trim(...)` support call.
Boxed primitive method references such as `Integer::intValue`,
`Double::doubleValue`, and `Float::floatValue` now use the same call-plan probe
for rendering and output-shape inference. Stream rendering dereferences the
borrowed iterator item before handing it to the primitive-number call target,
so the stream path no longer needs the old stream-local `Integer::intValue`
fallback. Primitive number-value call targets now retain the known source
primitive when resolver/type facts can prove it, so no-op unboxing such as
`Integer.intValue()` over an `i32` value returns the receiver directly while
real conversions such as `Float.doubleValue()` and `Double.floatValue()` still
emit casts. This keeps wrapper unboxing/no-op cast selection in the shared call
target instead of reintroducing stream-only rendering rules.

Stream `Object::toString` method references now also probe the ordinary native
instance call planner, using the stream element's inferred Java type as the
synthetic receiver. This preserves the previous borrowed-item spelling such as
`(*value).to_string()` for `List<String>` streams, but selection no longer
depends on the legacy method-reference fallback when the element type proves a
native Java receiver call. Source-object `toString` fallbacks remain available
for cases that still require source hierarchy facts.

Static optional-construction helper slice: static storage initializer wrapping
now calls `construct_optional_value_for_target`, which builds an
`OptionalSlotConstructionPlan` and applies `SlotConversionPlan::OptionalConstruction`
instead of formatting `Some(...)` in the initializer path. The focused
structural guard proves this path no longer owns raw wrapper construction;
plain non-null static `String` storage remains a lazy default value because the
thread-local cell uses `Option<T>` for initialization state separately from
Java nullable field semantics.

Optional erased-handoff slice: optional generic/erased facade argument handoff
now uses `construct_optional_value_for_target` instead of formatting
`Some(...)` directly. This lets erased-object element conversion and optional
wrapper construction stay on the same slot-conversion path used by nullable
object locals and returns. Verified by the structural
`optional_erased_handoff_arguments_use_optional_construction_plan` guard and
existing nullable object erasure regressions.

Optional element carrier helper slice: `lower.rs` now constructs
`LoweredType::OptionalElementBorrow` through
`optional_element_borrow_lowered_type`. Optional array arguments, source field
getter return shapes, optional borrowed slot normalization, and optional field
borrowed carriers all share the same carrier representation before
`SlotConversionPlan` chooses unwrap, borrow, clone, or view projection.
Follow-up: the actual carrier enum construction now lives in
`LoweredType::optional_element_borrow_from_option_shape` in
`conversion_plan.rs`; `lower.rs` may destructure or pass the carrier, but no
longer constructs `LoweredType::OptionalElementBorrow` directly.

Optional element expression helper slice: optional self-field recovery now uses
`optional_element_borrow_lowered_expr` to build the Rust option-carrier
expression and matching `LoweredType` together before calling
`plan_lowered_slot_conversion`. This removes the local `.as_ref()` construction
from `apply_optional_field_borrowed_carrier_to_slot` without changing optional
array paths that intentionally pass the original option carrier into the
borrowed-element adapter. Remaining nullable argument adapters still need a
larger carrier-origin model before their `.as_deref()`/`.as_mut()` rendering can
be deleted safely.

Optional assignment rewrap slice: `wrap_optional_assignment_value_if_needed`
now rewraps already-lowered `Some(inner)` values through
`SlotConversionPlan::OptionalConstruction` instead of formatting `Some(...)`
locally. A preconverted-element construction helper preserves the existing
assignment semantics for cases where the element has already passed through a
raw/generic compatibility path, such as optional list ternaries and facade
upcasts, while still centralizing wrapper construction.

Optional string-view normalization slice: the legacy rendered cleanup that
turns `Some(&value.as_str())` into `Some(value.as_str())` now lives in
`coercion.rs` as `normalize_optional_constructed_string_view_expr`. Lowering
still calls it at compatibility boundaries, but new optional construction paths
should prefer `OptionalSlotConstructionPlan` and structured string-view facts
over adding local normalization helpers.

Same-call alias snapshot slice: the same-call shared-read-before-mut-borrow
decision remains in `lower.rs`, where argument aliasing and slot shapes are
known, but the rendered compatibility rewrite now lives in `coercion.rs` as
`snapshot_rendered_shared_borrow_argument`. This keeps snapshot spellings for
`&name`, `Some(&name)`, slice views, and string views in the shared
clone/borrow rendering layer instead of embedding another local replacement
table in argument lowering.

Direct clone idempotence slice: direct clone-recovery idempotence now uses
`coercion::rendered_expr_has_direct_clone_recovery`, alongside the broader
`rendered_expr_has_clone_recovery`. `lower.rs` still decides whether a
particular argument or value needs clone recovery, but suffix matching for
`.clone()`, `.clone_box()`, and audited direct clone spellings is centralized in
the coercion compatibility layer.

Borrowed facade return recovery slice: `FacadeAbiFact` can now carry an
explicit `BorrowedReturnRecovery`, which is propagated through
`LoweredType::RustBorrowed` into `RustBorrowedFacadeReturnPolicy`. `coercion.rs`
therefore renders `Clone` versus `ToPathBuf` from the selected conversion
policy instead of re-checking whether the Rust return type string looks
path-like. The legacy Rust-name classifier in `conversion_plan.rs` has been
removed; older `rust-return=&T` facts that need path-specific recovery must now
provide the recovery field explicitly. External-pack manifests can set this through
`borrowed-return-recovery=clone|to-path-buf` alongside `rust-return=&T`, so
javalibs packs do not need a Rust-code special case for path-like borrowed
returns.

1. Inventory and guardrails.
   Add or update focused tests that count representative rendered-string
   heuristics and assert no new non-JDK package tables enter `src/stdlib.rs`.
   Start with documentation-only audits if the code is moving quickly.

2. Rust signature fact ingestion.
   Add a narrow extractor or handwritten bridge for observed support/facade
   Rust signatures. Convert those to `FacadeAbiFact` records and route ordinary
   calls through them. First targets: iterator methods, throwable/string
   helpers, file/path helpers, string byte helpers, `HashMap::get`, and
   `Vec`/slice adapters.

3. Nullable carrier unification.
   Extend `LoweredType` or add `OptionalProjectionPlan`/`NullableCarrierPlan`
   so fields, map gets, array accesses, facade returns, and conditionals
   represent the same carrier information. Remove local `Some(...)`,
   `.as_ref()`, `.as_deref()`, and `.cloned()` decisions as each path moves to
   `SlotConversionPlan`.

4. Owned recovery policy.
   Centralize clone/copy/to_vec/clone_box recovery behind one
   `CloneRecoveryDecision` policy with provenance and a `MoveUseContext`.
   Replace local rendered clone guards with structured expression-shape checks,
   leaving rendered fallback only for legacy raw nodes.

5. Borrow/view projection.
   Introduce `ensure_borrow_projection` and migrate call arguments, map keys,
   string views, slice views, and mutable slice views behind it.

6. Java display plan.
   Route string concat, `String.valueOf`, optional display, erased-object
   display, and format helpers through one `JavaDisplayPlan`.

7. Stream parity.
   Convert stream lambda calls, member access, and method references to consume
   the same call/member/conversion plans as ordinary expressions. Add parity
   tests where the same Java call appears inside and outside a stream.

8. External fact consolidation.
   Merge or cross-link external API effect, argument effect, return,
   constructor, field, exception, and facade ABI facts by `CallableKey`. Keep
   old manifest syntax working by translating it into keyed facts internally.

9. Rule deletion pass.
   For each retired special case, record why it is covered by facts or why it
   remains as Java semantics. Prefer deleting duplicated stream and rendered
   string branches before tackling JDK semantic helpers.

## Tests To Add

- Optional carrier tests for field read, map get, array access, nullable
  ternary, facade optional return, and constructor argument into the same
  borrowed and owned target slots.
- ABI fact parity tests where a handwritten rule and a Rust signature fact
  produce the same lowered call, then remove the handwritten rule.
- Negative simple-name tests proving a non-imported same-simple-name type does
  not match a `javalibs` or stdlib fact.
- Stream parity tests for borrowed string arguments, optional map gets,
  `Integer.parseInt`, `String.length`, getter method references, and mutable
  iterator receivers.
- Borrowed facade return tests covering `Path` by explicit recovery fact rather
  than Rust type-name matching.
- Clone recovery tests proving `String`, `Vec<T>`, boxed trait objects, erased
  objects, and non-cloneable facades take distinct policy paths.

## Temporary Regression Policy

It is acceptable for a broad corpus run to expose more compile errors during a
phase when local rules are removed, if:

- focused tests prove the replacement fact/plan is correct for the migrated
  idiom;
- failures are typed diagnostics or localized Rust compile errors, not
  translator crashes;
- the failing cases point to missing facts or unsupported carriers in the same
  shared path;
- the plan file records the regression and the next fact/policy needed.

Do not accept regressions caused by ambiguous simple-name matches, accidental
non-JDK rules in `src/stdlib.rs`, or loss of global semantic analysis.
