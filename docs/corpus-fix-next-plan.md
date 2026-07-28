# Corpus Fix Next Plan

Date: 2026-07-22

This plan starts from `docs/current-corpus-issue-audit.md`. The current local
gates are green, so the next work should preserve that baseline while reducing
real corpus failures one minimized idiom at a time.

Companion design-convergence plan: `docs/rule-generalization-convergence-plan.md`
tracks how to reduce the current rule count by replacing repeated borrow,
clone, optional-carrier, stream, and facade ABI special cases with shared facts,
plans, or helpers. Use that plan when choosing between a new local idiom fix and
a broader fact/policy extraction.

## Batched Execution Mode

The current implementation mode is batched. Prefer coordinated slices that move
several related rendered Rust spellings or local compatibility rules into shared
facts, slot plans, or projection helpers before running the expensive full
corpus gate. Inside a batch, use focused unit/idiom tests and cheap compile
checks to catch local drift; run the full `idiom_gaps` suite and corpus reruns
at batch boundaries.

Batching should not weaken the semantic direction of this plan. The default
tradeoff is to accept a temporary increase in local error count when it removes
specialized rules, makes Rust ABI/borrow behavior fact-driven, or positions the
translator for global semantic analysis. Do not make algorithmic shortcuts that
would block later whole-program/source-summary analysis; prefer indices,
memoization, and reusable plans over narrowing analysis scope.

Current default batch size: group roughly 3-5 low-risk projection/fact cleanups,
or one larger semantic refactor, then run a full gate. Use up to 10 parallel
workers for independent investigation or corpus triage, but keep final edits in
this checkout when they touch shared translator or `javalibs` behavior.

Checkpoint policy: the user has granted permission to create git commits in
their name at good checkpoints. Use this after coherent batches with meaningful
verification, so larger edits have clean rollback points. Commit messages should
name the completed slice and avoid bundling unrelated working-tree changes.
This permission is explicit for the current batched mode: commit after green
checkpoints when that helps make larger refactors tractable, while keeping
unrelated user or generated files out of the commit.
2026-07-27 confirmation: this permission remains active for the current goal;
prefer committing each verified shared-policy slice before taking the next
larger batched risk.

## Top Design Priority: Infer Rust Facade ABI Slots

Repeated corpus failures around missing `&`, `&mut`, clones, slices, optional
handoffs, and `Box<dyn Any>` fallbacks should be treated as evidence that the
translator needs a general Rust facade ABI inference layer, not more
call-site-specific fixes.

Rust stdlib/core/alloc are too large to mirror with bespoke lowering rules or a
comprehensive handwritten model. Whenever a Rust signature fact gives the same
receiver, argument, return, and fallibility shape as a tailored rule, prefer the
signature fact and keep the tailored rule only as a Java-semantic rule or a
temporary compatibility bridge. This should cover mechanical ABI choices such as
`&T`, `&mut T`, slices, owned returns, `Option`, `Result`, and
iterator/collection method receiver style, while leaving Java-specific behavior
in explicit Java facts. The standard-library signature work is not a goal to
model all of `std`; it is an equivalence-and-replacement path for special cases
that generated code already hits.

Default triage rule: when a new generated-Rust error looks like a mechanical
Rust API ABI problem, first ask whether a Rust std/core/alloc signature fact
would produce the same lowering as the proposed special case. If yes, add or
extract the signature fact and route the call through the shared slot planner;
do not add a new bespoke stdlib branch.

Landed receiver-ABI slot slice: `facade_receiver_slot_plan` maps
`ReceiverAbi::{BorrowedSelf, MutBorrowedSelf, OwnedSelf, AssociatedFunction}` to
shared receiver slots, and non-optional external facade receivers now pass
through `plan_lowered_slot_conversion` before method-call emission. Pure
`&self`/`&mut self` receiver requirements are still emitted as ordinary Rust
method syntax when method-call autoref satisfies the fact; casts, owned moves,
optional projections, and non-autoref-compatible conversions remain explicit
slot conversions. Optional receiver projection remains on the older unwrap path
and should be migrated only when optional receiver carriers are represented by
the same slot plan. The current focused coverage is
`facade_receiver_abi_maps_to_receiver_slot_shapes`,
`facade_borrowed_self_receiver_uses_receiver_slot_conversion`,
`class_literal_get_class_loader_uses_platform_facade`,
`cargo test facade_abi --test slot_shape_design --quiet`, and
`cargo test receiver --test idiom_gaps --quiet`.

Landed fallible facade-support slice: external facade support return types now
wrap `Result<..., JavaException>` using both user `FactSet` effects and builtin
external-pack effects after ABI/custom return type selection. This keeps
try-with-resources cleanup (`input.close()?`, `output.close()?`) and JDK
`Files.newInputStream/newOutputStream` support aligned with the selected
callable facts instead of re-deriving fallibility from a partial fact source.
Focused coverage: `documents_try_with_resources_copy_gap`.

Highest-priority expansion: make optional wrapper construction a first-class
slot conversion/fact result, not a local `Some(...)` rewrite. The current
`FacadeAbiFact` path can express borrowed optional returns and borrowed
arguments, but several nullable/Optional construction paths still decide
whether to emit `Some(value)`, `Some(&value)`, `.as_ref()`, `.as_deref()`, or a
clone in `lower.rs` helpers. Those paths should be replaced by a shared
`OptionalSlotConstructionPlan` or equivalent conversion-plan variant that
consumes the actual expression shape and target slot shape. This prevents the
same simple borrow mismatch from reappearing whenever a Java nullable value,
optional facade return, field getter, map lookup, or optional constructor feeds
a borrowed Rust ABI slot.

Landed first step: non-null ternary branches feeding an optional borrowed
argument now construct optional branch values without borrowing the wrapper
itself. The focused regression
`non_null_ternary_into_optional_borrowed_arg_wraps_planned_borrow` rejects
`&Some(...)`/`&if ... Some(...)` drift and proves borrowed array elements remain
borrowed as `Some(&parts[i])`.

Follow-up cleanup: `OptionalSlotConstructionPlan` now tracks owned wrapper
construction separately from element ownership, and
`SlotConversionPlan::OptionalConstruction` renders `Some(element)` after
applying only the element coercion plan. Common `Some(...)`/ternary cases are
detected structurally from `RustExpr` when preventing accidental wrapper
borrows. A raw-expression fallback remains only for legacy raw optional
constructions; retire that once optional construction is represented end-to-end
as a lowered expression marker.

Latest optional-construction slice: non-null optional branches now build the
construction plan from the actual expression shape and the target optional slot,
including borrowed optional targets. Indexed array reads normalize the syntactic
place expression separately from the semantic borrowed-projection shape, so
`Some(&parts[i])` is produced by `SlotConversionPlan::OptionalConstruction`
instead of a local raw `Some(&...)` rewrite. Remaining cleanup should apply the
same actual-shape construction path to map lookups, field getters, facade
optional returns, and any remaining nullable helper that still manually chooses
between `Some(value)`, `Some(&value)`, `.as_ref()`, or `.as_deref()`.

Optional assignment rewrap follow-up: assignment values that arrive as
already-rendered `Some(inner)` now unwrap the element and re-enter
`SlotConversionPlan::OptionalConstruction` instead of formatting a new
`Some(...)` wrapper in `lower.rs`. The helper has a preconverted-element mode
for compatibility paths that already performed raw/generic element adaptation,
so optional list ternaries and `FileInputStream -> InputStream` facade upcasts
avoid double coercion while wrapper construction is still centralized. Verified
with `optional_assignment_rewrap_uses_optional_construction_plan`,
`file_input_stream_constructor_upcasts_into_optional_input_stream_slot`,
`optional_ternary_list_assignment_wraps_once_without_runtime_downcast`, and
`cargo test optional_ --test idiom_gaps --quiet`.
Present-value optional construction cleanup: `lower_present_expr_for_optional_target`
now delegates both ternary and single-expression present branches to
`construct_optional_value_for_target` after lowering the element into the
planned construction element slot. This removes the remaining local
`LoweredSlotExprConversionPlan { OptionalConstruction(...) }` assembly from the
present-value helper while preserving indexed-borrow and ternary element-slot
selection.
Array-access optional construction cleanup: `lower_array_access_expr_for_slot`
now uses the same `construct_optional_value_for_target` helper when an indexed
read feeds an optional borrowed target, preserving the conservative indexed
element actual shape while keeping optional wrapper construction on the shared
path.
Optional construction boundary cleanup: the concrete
`LoweredSlotExprConversionPlan { SlotConversionPlan::OptionalConstruction(..) }`
assembly now lives in `conversion_lowering.rs` rather than `lower.rs`.
Expression lowering may still infer actual element shape and choose branch
slots, but optional wrapper application is a conversion-lowering concern.
Optional assignment adaptation cleanup: `wrap_optional_assignment_value_if_needed`
now delegates already-optional result unwrapping, legacy rendered `Some(...)`
compatibility, `None` idempotence, and fallback slot conversion to
`conversion_lowering::adapt_optional_assignment_value_for_target`. Lowering
still chooses the actual expression shape from source/fact context, but it no
longer parses optional wrapper renderings locally.
Optional constructed-value detection cleanup: argument borrowing now calls
`coercion::rust_expr_is_constructed_optional_value` when avoiding accidental
borrows of already-constructed optional wrappers. The raw `Some(...)`
compatibility fallback remains centralized beside `unwrap_rendered_some_expr`
and optional string-view normalization instead of living in `lower.rs`.
Remaining cleanup should continue replacing nullable call sites that choose
`.as_ref()`, `.as_deref()`, `Some(...)`, or clone recovery locally with calls
through these shared conversion helpers or direct slot plans.

Optional borrowed facade return cleanup: `Option<&T>` returns from Rust facade
ABI facts now convert to optional erased-object slots through a shared slot
conversion plan, rendering `.map(|value| Box::new(value.clone()) as
Box<dyn std::any::Any>)` instead of first collapsing to `Option<T>`. Declared
`Object` locals preserve optional erased carriers when nullable facade getters
return concrete borrowed values, and optional erased-object carriers are treated
as erased boundaries by the shared shape predicate.
Optional borrowed-return recovery follow-up: optional borrowed facade returns
now carry the same explicit borrowed-return recovery policy as required borrowed
returns, so manifests can choose `clone` versus `to-path-buf` for
`Option<&T>` without relying on rendered Rust type-name fallback checks.

2026-07-27 checkpoint: external facade result metadata now takes precedence over
Java `throws` when rendering support return types, so explicit Rust ABI facts can
say `rust-result=false` without being rewrapped from Java exception facts. The
same slice tightened external/facade argument precedence: external argument
effects are preferred over facade ABI slots when present, nullable borrowed
argument facts can request optional borrowed carriers, and static support
signature fallback no longer overwrites an already-shaped fact plan. HTSJDK
`IntervalList.add(Interval)` is now expressed as a built-in samtools-util
argument fact and mutator effect rather than a lowering special case, which
keeps `IntervalList` locals concrete and passes constructed `Interval` values
without `Box<dyn Any>` handoff. Focused verification covered
`rust_result_metadata_separates_facade_fallibility_from_java_throws`,
`external_borrowed_literal_argument_is_hoisted_when_call_result_is_stored`,
`external_byte_array_return_recovers_java_signed_bytes_from_rust_unsigned_bytes`,
`borrowed_external_facade_return_recovers_owned_value_before_erasure`,
`pack_enabled_final_domain_facade_local_stays_concrete_not_erased`,
`domain_collection_facade_constructor_is_not_native_collection_constructor`,
`builtin_htsjdk_interval_facade_support_matches_overloaded_constructor_calls`,
and `array_plan_and_lowering_own_java_array_shape_before_orchestrator`.
`cargo check`, `cargo fmt --check`, and `git diff --check` were clean. A full
`slot_shape_design` run was restarted and progressed past the earlier argument
and return-shape regressions, then reached the pre-existing representation and
signature baseline failure cluster; continue treating that cluster as separate
goal work unless a new focused change moves one of those fixtures.

Rendered compatibility centralization follow-up: same-call alias snapshots still
decide in `lower.rs` because they need argument aliasing and slot-shape context,
but the rendered replacement table now lives in `coercion.rs` as
`snapshot_rendered_shared_borrow_argument`. This keeps the compatibility
spellings for `&name`, `Some(&name)`, `.as_slice()`, and `.as_str()` snapshots
beside clone/borrow rendering helpers instead of inside argument lowering.
Verified with `rendered_string_heuristics_have_centralization_guardrails`,
`cargo test same_call --test idiom_gaps --quiet`, and
`cargo test alias --test idiom_gaps --quiet`.

Direct clone idempotence follow-up: `lower.rs` no longer owns a local
`.clone()`/`.clone_box()` suffix table for direct clone idempotence; it calls
`rendered_expr_has_direct_clone_recovery` from `coercion.rs`, beside the broader
clone-recovery idempotence helper. Verified with
`rendered_string_heuristics_have_centralization_guardrails`,
`cargo test clone_recovery --test slot_shape_design --quiet`, and
`cargo test clone --test idiom_gaps --quiet`.
Audited clone projection follow-up: self-cast facade boundaries and owned
receiver-field recoveries now call `coercion::apply_audited_clone_recovery`
instead of formatting `self.clone()/* TODO(...) */` or
`expr.clone()/* TODO(...) */` in `lower.rs`. Lowering still decides when a Java
self/field value crosses an owned facade boundary, but the Rust clone spelling
and audit marker live with the shared clone recovery policy.
Class-literal projection follow-up: `call_lowering.rs` no longer formats
`JavaClass::<T>::literal` directly. The spelling lives in
`coercion::java_class_literal_path_projection`, beside the other shared
call/member path projections, while class-literal planning still selects the
Java name and Rust token type from facts.
Result-propagation projection follow-up: `lower.rs` no longer imports
fallible-call `?` wrapping from call lowering. The generic rendered Rust
projection now lives in `coercion::propagate_result_projection`, keeping
result-shape spelling with slot/projection helpers while call planning remains
responsible for deciding when propagation is required.
Erased-object constructor follow-up: constructor lowering now uses
`shape_is_erased_object_boundary(&target.owner_shape)` instead of comparing the
rendered owner type to `Box<dyn std::any::Any>`. `call_lowering.rs` still
selects the constructor projection, but the erased-object decision is based on
the shared slot/shape fact boundary.
Erased-object type spelling follow-up: `slot_render::erased_object_rust_type`
now owns the canonical `Box<dyn std::any::Any>` type, including wildcard,
erased-object, generic type-variable, and fallback map element policies.
Lowering renders typed `None` and iterator fallback element types through that
slot-render helper instead of carrying the Rust ABI spelling locally.
External dyn-trait signature follow-up: `EffectiveFacts` now shares the
canonical external type alias matcher, and same-provenance object
representation facts prefer explicit non-concrete strategies over concrete
fallbacks. Source and constructor parameters consume object-representation facts
directly, so external interface arguments render as crate-qualified
`&impl facade::Trait` without method-local string matching. Nullable source
returns declared as external dyn-trait/object-safe types now keep an owned
erased carrier (`Option<Box<dyn Any>>`) unless a future precise facade return
ABI fact proves a better concrete carrier. Method return rendering now trusts
the solved slot shape before falling back to direct crate-mapped Java type
spelling.
Storage/capture/String projection follow-up: static storage setter/getter call
paths, qualified static storage paths, captured outer-this field access, and
char-array-to-`String` collection now use coercion projection helpers. Lowering
still decides when Java static storage, non-static inner capture, or a
`new String(char[])` special Java semantic applies, but the emitted Rust
spellings live beside the other shared projections.
Erased-object comparison follow-up: `getClass()` type-predicate inequality and
`this == Object` identity inequality now use the shared logical-not projection
instead of formatting local `!(...)` wrappers in `lower.rs`. The erased-object
runtime predicate itself is still selected by Java semantic lowering.
String equality projection follow-up: operator equality planning still selects
direct, optional, string-view, and custom-equals comparison shapes, but the
Rust `.as_str()` and optional string-view projection spellings now live in
`coercion.rs`. This keeps recurring borrowed string comparison mechanics beside
the other shared string/facade view helpers.
Static-storage support-body follow-up: generated getter/setter function bodies
for mutable Java static fields now use coercion projections for the
thread-local `RefCell<Option<T>>` read/write spellings. Source lowering still
owns deciding which Java fields need storage accessors and what initial value
slot to use.
Constructor self-field projection follow-up: deferred constructor field
assignments and delegated-constructor trailing field assignments now use a
shared `__java_self.field` projection helper. Constructor lowering still owns
sequencing, default seeding, and self-dependent expression recovery.
Emitted source-call signature consistency follow-up: resolved static calls into
types emitted from the current source program now rebuild their argument plan
from the exact `required_method_signature(TypeDecl, MethodDecl)` used by item
assembly. This prevents the alias/base-name lookup path from weakening a
source method parameter to a borrowed slot while the emitted Rust function still
takes the owned slot. The first focused proof is
`current_self_concrete_static_argument_clones_without_downcast`, while
`resolved_static_helper_call_uses_fallback_signature_arg_shapes` and
`local_passed_to_resolved_static_mutable_source_param_is_declared_mutable`
remain green, preserving the separate non-emitted source-summary borrow
fallback.
Unsupported/abstract fallback projection follow-up: abstract method stubs,
abstract interface stubs, unsupported inner-class outer capture placeholders,
and unsupported constructor-call handoffs now use named coercion projections for
their Rust `panic!` spellings. Lowering still owns when those Java fallback
paths are selected.
Optional receiver borrowed-carrier follow-up: call-target receiver adaptation no
longer projects already-borrowed optional receivers inline. The shared optional
receiver slot-plan adapter now handles `OptionalElementBorrow` receivers,
projects the carrier, attaches the element shape, and runs the normal slot
conversion/autoref gate.

Landed optional-carrier follow-up: raw-erasure casts into optional targets now
route through `lower_present_expr_for_optional_target`, so typed optional
collection slots preserve the target element shape instead of falling back to
runtime downcasts. Present ternaries feeding optional targets lower the branch
value once and apply `SlotConversionPlan::OptionalConstruction` around the
whole conditional, producing `Some(if ... { element } else { element })`
rather than branch-local optional wrappers. Unknown or already-optional branch
actuals inherit the target element shape before construction, which keeps
nullable list and borrowed-array cases on the shared slot path.

Map lookup follow-up: `Map.get(...)` lowering now handles optional target slots
as an optional borrowed carrier (`OptionalElementBorrow`) and routes adaptation
through `plan_lowered_slot_conversion`, preserving Rust `HashMap::get` missing
key semantics instead of forcing a Java null-receiver unwrap. The call-argument
optional shared-borrow adapter now asks the call planner whether an argument is
already an optional borrowed native collection result before adding `.as_ref()`,
so `values.get(key.as_str())` can flow directly into `Option<&T>` parameters.
Map-owned recovery follow-up: `Map.get(...).toArray(...)` now treats the
lookup result as `LoweredType::OptionalElementBorrow` and recovers the owned
collection through `plan_lowered_slot_conversion` before final target-slot
adaptation, so clone/null unwrap policy comes from borrowed-element facts rather
than a local `.expect(...).clone()` rendering. Remaining map work is optional
element view changes that still need to be proven by slot facts rather than by
rendered-expression cleanup.

Latest map/optional argument slice: borrowed optional call targets now use a
shared `lower_optional_argument_for_borrowed_optional_target` helper before the
legacy optional-array adapter. The helper preserves native collection carriers
that already match `Option<&T>` and only asks the slot converter for projection
when the actual optional storage needs a borrowed-element view. This fixed the
recurring `values.get(...).as_ref()` over-borrow pattern while still allowing
`Option<String>` to feed `Option<&str>` through the centralized `as_deref()`
normalization.

Optional array/slice follow-up: optional array arguments and expression slots
that target borrowed slice views now enter the shared `OptionalElementBorrow`
conversion path, so `Option<Vec<T>>`, `Option<&mut [T]>`, and related nullable
array carriers project through the borrowed-element slot policy instead of
local `.as_ref()`/`.as_deref()` formatting. Legacy argument adapters now treat
already-planned optional slice projections as idempotent; the remaining cleanup
is to retire those adapters once all nullable array call paths preserve the
carrier expression until slot conversion.

Field getter follow-up: optional self-field recovery for borrowed optional
return/argument slots now delegates to `plan_slot_conversion` instead of
rendering `.as_ref()` directly. This keeps `Option<T>` field storage to
`Option<&T>` getter views on the same slot-coercion path used by calls and
arguments. Latest field-recovery slice: owned element recovery from optional
self-fields now first projects the Rust storage carrier to `Option<&T>` and
then applies `plan_lowered_slot_conversion` as an `OptionalElementBorrow`,
so constructor arguments and returns that need `T` recover with the shared
borrowed-element clone/unwrap policy instead of local
`.as_ref().expect(...).clone()` formatting. Whole-optional self-field storage
copies now use the same carrier plan (`Option<T>` storage projected to
`Option<&T>`, then recovered by `BorrowedElement` policy), and optional
recoverability follows the element recovery capability so `Option<String>` has
proven clone recovery. Non-self optional receiver field element recovery now
follows the same split: unwrap the nullable receiver, treat optional field
storage as an `OptionalElementBorrow` carrier, and let
`plan_lowered_slot_conversion` choose `.as_ref()`, `.cloned()`, and
null-receiver unwraps. Latest field-recovery cleanup: reused optional locals
that came from nullable fields now re-enter the same lowered
`OptionalElementBorrow` carrier path before owned return/argument recovery,
instead of formatting `.as_ref().unwrap().clone()` locally. Clone-recovery
idempotence also recognizes the owned value produced by
`.cloned().expect("Java null receiver")`, preventing later argument reuse from
adding a second audited clone. Follow-up cleanup: shared self-field and
borrowed receiver-field owned recovery now use the shared audited clone
coercion helper, while optional field element recovery feeds the field carrier
through `apply_optional_field_borrowed_carrier_to_slot`. Reused owned values
wrapped back into optional targets now call `construct_optional_value_for_target`
after clone recovery instead of formatting `Some(value.clone())` locally.
Reused optional values in string/display slots now call
`JavaDisplayPlan::OptionalValue` instead of formatting the optional `"null"`
display map in `lower.rs`, keeping nullable display policy centralized in the
coercion layer. Clone recovery in lowering now calls the expression-level
`apply_audited_clone_recovery` helper instead of the render-only
`render_audited_clone_recovery` API, including varargs, external-into child
arguments, callback preservation, and deferred constructor field assignments.
Borrowed owned string-literal temporary hoisting now asks the coercion layer for
the temporary projection plan, so `lower.rs` no longer parses
`Some(&"...".to_string())` or `&"...".to_string()` directly when preserving
borrowed literal lifetimes across stored call results.
Remaining field work should convert any non-element optional receiver views to
explicit carrier/slot plans so clone, unwrap, and view choices are selected by
facts rather than by local string construction.

Typed external field support follow-up: shared support merging now keeps
associated constants from alias/target facade pairs type-correct. When an
associated constant such as `Color::BLACK` is promoted onto the canonical
`JavaAwtColor` impl, the merger rewrites an alias-as-constructor initializer
from `pub const BLACK: Color = Color;` to
`pub const BLACK: Color = JavaAwtColor;` and drops duplicate constants with the
same name. This preserves the Java-facing alias while avoiding Rust value
construction from a type alias. Verified with
`shared_support_merger_preserves_associated_constants` and
`static_owner_name_does_not_collapse_to_same_named_instance_field`.

JDK collection static projection cleanup: `Arrays.asList`, `Arrays.equals`, and
`Collections.{emptyList,emptySortedSet,singleton,singletonList,singletonMap,nCopies}`
lowering now call `coercion` projection helpers. Call lowering still selects the
native Java collection method and supplies conservative arity fallbacks, while
coercion owns the Rust expression shapes for vector literals, equality,
empty collections, singleton sets/lists/maps, and repeated vectors.

Buffer discard projection cleanup: `Buffer.put(...)` lowering now delegates the
discard-block Rust spelling to `coercion::discard_value_projection`, keeping the
instance-method dispatcher focused on Java method selection and malformed-arity
fallbacks.

Erased object boxing cleanup: `new Object(...)` lowering and borrowed facade
returns now call `coercion::erased_object_box_projection` for the
`Box<dyn std::any::Any>` handoff shape. This keeps call lowering from owning the
Rust erased-object ABI spelling directly and gives later facade ABI facts a
single projection point to replace or specialize.
Follow-up: `new Object(...)` now delegates the zero/one/many argument carrier
selection to `coercion::erased_object_constructor_projection` as well. Call
lowering still decides when Java `Object` construction is selected, but the
unit/default payload, tuple payload, and final erased box projection are in the
coercion layer.

Inherited source-context import follow-up: inherited method bodies now seed
static type aliases from support and source-summary context imports before
lowering. This prevents copied superclass methods from losing static facade
owners imported by their declaring source, as in FastQC `AbstractQCModule`
methods inherited by `AdapterContent` lowering `ImageIO.write(...)` to
`image_i_o::write(...)`. Verified with
`inherited_context_method_body_sees_declaring_static_imports` and the existing
`class_loader_static_resource_call_uses_platform_facade`.

Java platform constructor follow-up: `java.io.File` support now exposes
overload-compatible facade constructors for both resolver-derived arity names
and the idiomatic single-argument ABI. `File::new_1(path)` delegates to
`File::new(path)`, while `File::new_2(parent, child)` preserves Java
`new File(parent, child)` path-join semantics through `JavaPathArg`. This
removes the generated `File::new_1`/`File::new_2` missing-method family without
canonicalizing distinct Java overloads to an arity-wrong Rust call. Verified
with `jdk_file_constructor_overload_facade_supports_resolved_arity_names` and
`java_platform_facade_support_covers_path_uri_and_file_boundaries`.

String byte-array ABI follow-up: Java `String.getBytes(charset)` now uses the
same native string target as `getBytes()` and carries a selected stdlib
`FacadeAbiFact` for the borrowed charset argument. This prevents resolved
overload names such as `get_bytes_1` from escaping into generated Rust while
keeping the mechanical argument slot in the shared ABI fact path. Verified with
`java_string_get_bytes_and_index_of_use_native_helpers` and
`stdlib_owns_known_jdk_content_method_facts`.

String byte-array constructor follow-up: Java `new String(byte[])`,
`new String(byte[], charset)`, `new String(byte[], offset, len)`, and
`new String(byte[], offset, len, charset)` now route through explicit Java
byte-decoding helpers instead of Rust's zero-argument `String::new`. The
charset overloads currently share the lossy byte decoding path used by the
existing range helper, while the charset argument itself remains a mechanical
ABI concern handled by the call/constructor argument planner. Verified with
`byte_array_local_with_spaced_dimensions_feeds_string_range_constructor`.

Static string map-key follow-up: native `Map.get(...)` key normalization now
keeps inferred shared-borrow shape facts for associated/static string constants
through the final borrow projection gate. This prevents
`HashMap<String, _>.get(CONST_STR)` from becoming `get(&CONST_STR)` and
selecting the invalid `String: Borrow<&str>` lookup shape. Verified with
`map_get_static_string_constant_key_does_not_double_borrow_str` and
`.tmp/corpus-rerun-htsjdk-vcf-after-static-string-map-key-r1`, where the VCF
focused corpus drops from 658 to 652 generated-Rust errors and the
`Borrow<&str>` family is gone.

Corpus cleanup slice: abstract classes that implement interfaces now emit the
trait impl even when some interface methods are abstract/missing; the generated
trait method uses the existing panic fallback while the inherent abstract stub
keeps direct calls compilable. Qualified nested `Builder` types keep their
owner-qualified Rust struct names and no longer emit an ambiguous top-level
`pub type Builder` alias. Spark noop boundary tests now accept the explicit
erased handoff shape while still requiring external-boundary diagnostics and
rejecting stream-local lowering drift.

Operational rule: do not build a broad handwritten Rust stdlib model in
`src/stdlib.rs`. Add std/core/alloc coverage as small signature-fact clusters
driven by generated-code failures, and require each replacement to prove that
the signature fact produces the same lowering as the old tailored rule. If a
tailored rule remains, document whether it is preserving Java semantics,
bridging a migration gap, or covering a Rust construct that cannot be recovered
from the signature alone.

The Rust standard library is deliberately not a translation table backlog. Use
signature facts as an equivalence mechanism: when a Rust std/core/alloc
signature explains the same receiver style, argument slot, borrow/mut-borrow,
slice handoff, return wrapper, or fallibility as a tailored rule, the fact wins
and the special case becomes migration debt unless it encodes Java semantics
that the Rust signature cannot express.

Preferred direction:

1. Parse or otherwise extract Rust-facing shim/facade signatures from
   `javalibs` and generated support code.
   Also investigate ingesting selected Rust stdlib/core/alloc signatures into
   the same ABI fact model, because `std` is too large to mirror with tailored
   call-site rules. Prefer signature facts whenever they produce the same
   receiver/argument/return slot as a handwritten rule.
   Start with std/core/alloc surfaces already emitted or targeted by the
   translator, not the entire Rust standard library:
   - `String`, `str`, `Vec`, slices, arrays, `Option`, `Result`;
   - `HashMap`, `HashSet`, `BTreeMap`, iterator adaptors, and collection
     constructors/accessors;
   - `Path`, `PathBuf`, `File`, basic `Read`/`Write`/`Seek`;
   - formatting/display boundaries where the signature can prove an argument
     requires `Display` or `AsRef<str>`.
2. Convert those signatures into explicit `FacadeAbiFact`/slot facts keyed by
   stable callable identities:
   - receiver ABI: free function, associated function, `&self`, `&mut self`;
   - argument ABI: owned, `&T`, `&mut T`, slice, optional, dyn trait, erased
     handoff, FFI handle;
   - return ABI: owned, borrowed, optional, result-wrapped, erased, FFI handoff.
3. Make ordinary calls, stream lowering, method references, and fallback facade
   calls consume those facts through the same argument/return slot conversion
   path.
4. Use hand-written external argument effects only as migration shims or for
   Java semantic facts that are not visible in Rust signatures.
5. Audit and retire older specialized inference rules that become redundant or
   contradictory once facade ABI facts are authoritative. This includes
   call-name-specific borrow insertion, rendered Rust type-string inspection,
   generic facade preservation branches, stream-local call/member guesses, and
   external argument-effect rows that merely duplicate Rust shim signatures.
   Include std/core/alloc-specific lowering branches in this audit; if they
   only reproduce receiver/argument/return slots visible in a Rust signature,
   move them behind `FacadeAbiFact` and delete the special case.

Acceptance for this design priority:

- a facade method such as `InputStream::read(&buffer)` is selected because the
  Rust ABI says parameter 0 is borrowed/mut-borrowed, not because a lowering
  branch recognizes `read`;
- generated Rust no longer relies on string heuristics such as checking whether
  a rendered Rust type starts with `&`;
- Rust stdlib behavior uses ABI/signature facts when that is equivalent to a
  tailored lowering rule; handwritten stdlib rules remain only for Java
  semantics, syntax forms, or cases where a Rust signature alone is insufficient.
- adding or changing a `javalibs` facade signature updates Java call lowering
  without adding a parallel Rust-code table in `src/stdlib.rs`.
- Rust stdlib/core/alloc coverage grows by signature clusters, not by a giant
  bespoke model. Start from Rust APIs that generated code already targets, then
  let the shared slot planner derive receiver, argument, borrow, optional,
  result, and return ownership choices. Add a tailored rule only when the Java
  operation is not equivalent to one Rust signature, such as overload
  resolution, Java null/display semantics, class tokens, synthetic array
  length, or control-flow rewrites.
- every new std/core/alloc tailored rule must first answer whether an existing
  or easily extracted Rust signature fact would produce the same ABI plan. If
  yes, add the signature fact and test the equivalence instead of adding the
  special case.
- the audit identifies which legacy inference rules are kept as Java semantic
  fallbacks, which are deleted, and which are temporarily bridged to
  `FacadeAbiFact` compatibility shims.

Immediate std/core/alloc signature-fact migration queue:

1. Add equivalence fixtures for generated-code failures before adding any new
   Rust stdlib special case. Each fixture should assert both the emitted Rust
   and the selected `FacadeAbiFact`/slot plan when the ABI fact is expected to
   replace a tailored rule.
2. Grow std/core/alloc coverage in small clusters driven by real corpus rows:
   `Vec`/slice element access and mutation, `Option::as_ref`/`as_mut`/
   `as_deref`/`as_deref_mut`, `Result` propagation boundaries, iterator
   adapters used by stream lowering, and string/formatting boundaries.
3. Treat parsed or curated Rust signatures as the authority for mechanical ABI
   shape: receiver borrow, argument borrow or mutable borrow, slice handoff,
   optional wrappers, result wrappers, and owned/borrowed returns.
4. Keep a tailored lowering rule only when the Rust signature does not encode
   the Java behavior being preserved, such as overload selection, Java null
   exceptions, class literals, array `.length`, switch/control-flow rewrites,
   primitive numeric promotion, or Java collection semantic adaptation.
5. Audit existing std/core/alloc branches after each cluster lands. If a branch
   only re-states a Rust function signature, replace it with the fact-backed
   slot plan and delete or mark the branch as a temporary compatibility bridge.

Current remaining refactor priority from the parallel audit:

1. Extend the new call-boundary receiver ABI plan to optional receivers.
   Non-optional `ExternalFacadeReceiver` lowering now carries
   `ReceiverAbi::{BorrowedSelf, MutBorrowedSelf, OwnedSelf, FreeFunction,
   AssociatedFunction}` through `facade_receiver_slot_plan` and shared slot
   conversion. Optional call receiver adaptation now passes explicit
   `OptionalReceiverProjection` variants to `coercion::project_optional_receiver`
   instead of formatting `.as_ref()/.as_mut()` from string access names. The
   legacy call-receiver path still uses unchecked `.unwrap()` projection
   variants to avoid broad output churn; remaining receiver work is to represent
   these optional receiver carriers directly in the same typed slot plan as
   facade receiver ABI. Native collection optional receivers now use
   `coercion::OptionalReceiverProjection::{SharedUnchecked, MutableUnchecked}`
   rather than formatting local `"as_ref"`/`"as_mut"` access names before
   unwrap.
2. Move optional shared/mutable projection decisions out of local rendered
   suffix checks in `lower.rs`. The next target functions are
   same-call mutable alias checks and any remaining optional receiver/view
   branches. Map-key borrowing now delegates projection policy to
   `coercion::normalize_map_key_borrow_projection`; the replacement pattern
   should continue to be typed optional-element projection plans consumed by
   slot conversion.
   Latest progress: `LoweredType::optional_element_borrow_from_option_shape`
   now owns optional-element carrier construction in `conversion_plan.rs`;
   `lower.rs` routes optional argument, field, and reused-optional carriers
   through that API instead of constructing the enum directly. The older
   `adapt_optional_shared_borrow_argument`/`adapt_optional_mut_borrow_argument`
   bridge has also been removed. Same-call mutable alias handling now has an
   explicit local `SameCallArgumentAliasPlan` that separates AST/use-order
   detection from applying coercion-owned snapshot rendering; keep this local
   until global semantic analysis models call argument sequencing.
3. Keep literal/clone/display compatibility parsing centralized in
   `coercion.rs` while raw `RustExpr` remains common. The latest slice moved
   borrowed owned string-literal temporary projection there. Literal slot
   behavior for borrowed string-view arguments/returns and Java numeric
   char-code literals now uses `coercion::LiteralSlotPlan`; lowering only
   extracts literal expressions and asks for the shared plan.
4. Stream lowering still has a few borrowed-string/rendered fallback helpers.
   Retire those only after ordinary calls expose the same receiver/argument
   slot plan to stream lambdas and method references. Latest cleanup:
   stream borrowed-string argument fallback now uses
   `coercion::BorrowedStringViewProjection` for already-view detection and
   local `.as_str()` rendering, leaving stream lowering responsible only for
   lambda/local binding context. Stream source iteration now maps
   `StreamSourceIteration` into `coercion::SequenceIterationProjection` and
   calls `render_sequence_iteration_source`, so optional sequence receiver
   unwrapping plus `.iter()`/`.into_iter()` spelling is no longer owned by
   `stream_lowering.rs`. Nested stream subpipeline owned-source iteration now
   uses the same shared renderer as top-level stream sources. Optional
   non-null stream filtering now calls
   `coercion::render_optional_non_null_filter_map`, and lambda collection
   flat-map source iteration reuses `render_sequence_iteration_source` instead
   of selecting `.iter()`/`.into_iter()` locally. Borrowed collection
   method-reference flat-map rendering now calls
   `coercion::render_borrowed_collection_flat_map`, keeping the `.iter()`
   spelling outside `stream_lowering.rs`. Stream lambda parameter dereference
   spelling (`*item`, `**item`, and receiver-parenthesized forms) now goes
   through `coercion::StreamLambdaParamProjection`, leaving stream lowering to
   choose the semantic lambda value shape.

Current implementation note, 2026-07-23:

- Latest full FastQC evidence, 2026-07-24:
  `.tmp/corpus-batch-current-fastqc-after-syntax-fixes-r1` removed the old
  generated-Rust syntax blockers from shared support (`pub in`) and batch
  imports (`Localizable:`), leaving 306 generated-Rust errors. The follow-up
  `.tmp/corpus-batch-current-fastqc-after-facade-keyword-and-strip-r1` removed
  the `CountDownLatch.await` facade keyword method blocker and same-module
  duplicate `Localizable` trait stub, but now reports 1401 generated-Rust
  errors because cargo reaches broader real representation/facade failures.
  Invalid-pattern counts remain 39 files. Treat the leading FastQC families as
  representation/trait-object recovery, AWT/Swing facade surfaces, unsupported
  constructor/inheritance paths, switch/array/class-literal/instanceof
  placeholders, and crate assembly performance. The full rerun also shows
  crate assembly at 1,130,913 ms, so repeated FastQC sweeps should first index
  source-stub stripping/import assembly or use smaller focused subsets.
- 2026-07-24 corpus sweep update: Picard program groups, HTSJDK FastQ, and
  VarScan now translate and cargo-check cleanly in focused package mode with
  `--jobs 10`. Evidence:
  `.tmp/corpus-sweep-20260724-picard-programgroups-after-orphan-trait`
  has 11/11 translated, 0 invalid patterns, `cargo_check=ok`;
  `.tmp/corpus-sweep-20260724-htsjdk-fastq-after-erased-facade-merge`
  has 8/8 translated, 0 invalid patterns, `cargo_check=ok`;
  `.tmp/corpus-sweep-20260724-varscan-after-map-insert-reuse-fallback`
  has 18/18 translated, 0 invalid patterns, `cargo_check=ok`.
- The FastQ fix was shared-support ABI assembly, not an HTSJDK table: duplicate
  `JavaErasedObjectFacade` declarations/impls are merged by trait method
  signature and complex impl target (`Box<dyn std::any::Any>`), while orphan
  simple-target impls such as `impl Localizable for LocalizedFormats` are
  removed when the target declaration is absent.
- The VarScan fixes reinforce the Rust-signature direction. Static numeric
  wrapper `toString` now keeps a Rust-facing display argument signature and
  body (`impl Display -> String`) instead of selecting an opaque `&self`
  fallback, and the shared-support merger prefers that static conversion
  variant when `Integer::to_string(value)` is used. Owned map insert arguments
  now use a conservative callable use-count fallback when suffix-read analysis
  misses a later key/value read inside nested branch lowering.
- JAligner source `static final byte` constants now preserve their declared
  source slot type (`i8`) instead of collapsing to `i32`, so constants such as
  `Directions.STOP/LEFT/DIAGONAL/UP` agree with byte arrays and byte-valued
  switch labels. This is a source-field fact/slot fix, not a JAligner table.
  Verified with
  `source_static_final_byte_constants_preserve_declared_slot_type` plus the
  existing source-constant tests. The same slice also prunes generated support
  `impl Trait for Type` rows when the concrete support type is absent, matching
  the batch shared-support orphan-impl invariant.
- Source/default field slot rendering now distinguishes integer, `float`, and
  `double` primitive zero values, so empty constructors use `0f32`/`0f64` for
  planned floating-point fields instead of integer `0`. Verified with
  `empty_constructor_uses_typed_float_defaults_for_primitive_fields`,
  `slot_render_owns_default_expressions_for_planned_views`, and focused
  JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-float-default-r1`, which reduced
  generated-Rust errors from 128 to 125.
- `StringBuilder`/`StringBuffer.append(...)` generated support now uses an
  unconstrained generic handoff for Java append overload width instead of a
  hard `Display` bound or a borrowed-string-only fact. Java-semantic
  stringification still stays in call lowering where available, for example
  erased object append still flows through `java_erased_object_to_string`.
  Verified with `string_buffer_append_support_accepts_char_arrays_and_object_values`,
  `direct_string_builder_append_is_modeled_as_fluent_jdk_call`,
  `non_static_inner_class_outer_fields_lower_through_outer_capture_handle`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-generic-append-r1`, which reduced
  generated-Rust errors from 125 to 112.
- Emitted source classes/records that do not declare `toString` now get an
  inherent `to_string(&self) -> String` fallback backed by
  `java_erased_object_to_string(self)`. This preserves Java `Object.toString`
  semantics for source and source-context classes without relying on Rust
  `Display`/`ToString` trait resolution, while explicit source `toString`
  methods and JDK facades such as `StringBuffer.toString()` still win. Verified
  with source, source-summary, emitted-context, concat, explicit-source, and
  `StringBuffer` fixtures plus focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-source-to-string-surface-r1`,
  which removed the 12 `Alignment: Display` errors and reduced generated-Rust
  errors from 112 to 100.
- Same-class/static source calls now feed `Self::...` and fallback-self targets
  through the current type's solved signature slots when selecting argument
  shapes. For primitive rectangular `T[][]` values lowered as
  `ndarray::Array2<T>`, borrowed two-dimensional array parameters now borrow
  the native matrix itself (`&scores`, `&mut pointers`) instead of passing the
  owned matrix or trying to treat it as a one-dimensional slice. Verified with
  `same_class_static_calls_borrow_ndarray_arguments_from_signature_slots`,
  `two_dimensional_numeric_arrays_lower_to_ndarray_array2`,
  `static_final_two_dimensional_numeric_tables_can_feed_readonly_slice_helpers`,
  `two_dimensional_row_replacement_lowers_to_ndarray_row_mut_assign`,
  `cargo fmt --check`, and `cargo check --quiet`.
- Source receiver fallback signatures now use solved source signature slots
  instead of recomputing Java-declaration defaults for array parameters. This
  keeps public storage setters such as `setSequence1(char[])` owned
  (`Vec<char>`) while true helper array parameters can remain borrowed slices.
  The same slice propagates resolved source `&mut self` receiver facts back to
  local binding mutability and clones optional array fields read through shared
  source receivers when an owned target slot requires it. Verified with
  `source_array_setter_arguments_follow_owned_storage_signature`,
  `array_arguments_use_borrowed_slice_views`,
  `final_sorted_map_receiver_mutation_declares_rust_local_mutable`,
  `local_passed_to_resolved_static_mutable_source_param_is_declared_mutable`,
  `cargo fmt --check`, and `cargo check --quiet`. Focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-source-array-setter-r1` reduced
  generated-Rust errors from 103 to 88.
- Java `new String(char[])` now lowers as a Java semantic constructor overload
  by coercing the argument through the shared array/slice slot machinery and
  collecting `char` slices into `String`. This deliberately stays out of Rust
  stdlib signature facts because Rust `String::new` has no such ABI; getter
  return shapes are consulted before raw inference so source getters returning
  `char[]` work too. Verified with
  `char_array_feeds_java_string_constructor`, `cargo fmt --check`,
  `cargo check --quiet`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-string-getter-char-constructor-r1`,
  which reduced generated-Rust errors from 88 to 85.
- Remaining tracked failure batch from the current rerun: JAligner at
  `.tmp/corpus-sweep-20260724-jaligner-after-string-getter-char-constructor-r1`
  has 29 files, 2 invalid-pattern files, and 85 generated-Rust errors, down
  from 156 in
  `.tmp/corpus-sweep-20260724-jaligner-after-erased-facade-merge`. Top idioms
  are still mismatched types, Java class/file facade gaps (`getClassLoader`,
  `File.list`, `isFile`, `canRead`), invalid downcasts, `Matrix::new_2`
  constructor support, `Vec<String>.iterator()`, lowercase static-owner
  recovery (`system`, `random`, `float`, `commons`), string comparison helpers
  (`compareTo`, `compareToIgnoreCase`, `equalsIgnoreCase`, `charAt`), missing
  inherited/interface setters on `Format` subclasses, temporary borrowed string
  lifetimes, and support assembly gaps such as missing `JavaUtilLoggingLevel`.
  This remains the next idiom batch; prefer ABI/signature facts for mechanical
  receiver/argument/return fixes and keep Java semantics separate.

- Follow-up ClassLoader slice: `JavaClass<T>`/`ClassLoader` support was extended
  locally so class-literal loader calls have a typed facade surface, and the
  focused fixture `class_literal_get_class_loader_uses_platform_facade` verifies
  both `get_class_loader()` and concrete `InputStream` return unwrapping. The
  focused corpus wrapper also had a stale hardcoded `JavaClass` shared-support
  template; it now matches the Rust stdlib support item. Verification:
  `cargo fmt --check`, `cargo check --quiet`,
  `class_literal_get_class_loader_uses_platform_facade`,
  `class_loader_static_resource_call_uses_platform_facade`,
  `source_array_setter_arguments_follow_owned_storage_signature`, and
  `char_array_feeds_java_string_constructor`.
- The JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-classloader-template-r1` is not a
  net improvement yet: it has 29 files, 2 invalid-pattern files, and 98
  generated-Rust errors. The previous `get_class_loader` missing-method family
  is gone, but newly exposed/failing adjacent rows include `String.as_deref()`
  on concrete Java exception messages, missing `URL.get_file`/
  `URL.open_connection`, missing `InputStream.read_0`, and nullable
  `getResourceAsStream(...)` assignment/return shape mismatches. Treat this as
  the next JDK/platform-facade ABI batch and prefer signature/facade facts over
  more call-site branches.
- Java exception message ABI now matches the nullable Java fact:
  `JavaException::get_message()` and `get_localized_message()` return
  `Option<String>` in both translator support and the focused corpus wrapper.
  This keeps `e.getMessage()` display lowering fact-driven instead of adding a
  special case for concrete Rust `String`. Verified with
  `caught_exception_message_display_uses_nullable_facade_signature`,
  `exception_lowering_policy_owns_result_support_ok_and_err_shapes`,
  `cargo fmt --check`, `cargo check --quiet`, and JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-exception-message-option-r1`,
  which removed the 11 `String.as_deref()` generated-Rust errors and reduced
  the focused corpus from 98 to 87 errors.
- JDK `InputStream.read()` no-arg overload now has a facade ABI fact, builtin
  external effect row, and shared support method `read_0() ->
  Result<i32, JavaException>` in both translator support and the focused corpus
  wrapper. Verified with
  `input_stream_no_arg_read_uses_result_facade_overload`, the same local gates,
  and JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-inputstream-read0-r1`, which
  removes the `InputStream.read_0` missing-method row and leaves 86
  generated-Rust errors.
- JDK URL/JAR resource loading now uses shared facade ABI facts and typed
  platform support rather than generated `()` stubs: `URL.getFile`,
  `URL.openConnection`, `JarURLConnection.getJarFile`, `JarFile.entries`,
  `JarFile.getManifest`, `Manifest.getMainAttributes`, and
  `Attributes.getValue`. `URL.openConnection` is exposed as a generic
  result-shaped Rust facade method so Java cast/assignment context can infer
  `JarURLConnection` without an erased downcast. `Enumeration.nextElement`
  remains generic at the support boundary because zip and jar callers need
  different concrete element types. The same slice marks
  `Base64.getEncoder()` as a static javalibs facade method to avoid duplicate
  static/instance support shims. Verified with
  `casted_factory_result_into_concrete_optional_slot_uses_slot_shape`,
  `jdk_facade_signature_facts_type_io_zip_xml_awt_and_base64_calls`,
  `cargo fmt --check`, `cargo check --quiet`, and JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-url-jar-facade-r1`, which removes
  the URL missing-method rows, drops runtime invalid-cast idiom count to 0, and
  reduces generated-Rust errors from 86 to 65.
- Current remaining JAligner top rows are source visibility/support issues
  (`Commons`, `MatrixLoader`, `SmithWatermanGotoh`, `Format`), lowercase static
  owner recovery (`system`, `instance`, `random`), source/interface method
  propagation (`Format` subclass `set_id`), numeric wrapper constructor/compare
  semantics such as `new Integer(String).compareTo(...)`, `Matrix::new_2`,
  trait/type confusion around `Localizable`, invalid `()` numeric casts, two
  temporary-borrow rows, and residual switch state-machine scaffolding.

- selected Rust stdlib ABI facts are now available through
  `stdlib::rust_stdlib_signature_facts()` for Java `String`/Rust `str`
  receiver calls such as `length`, `trim`, `contains`, `startsWith`,
  `endsWith`, `charAt`, and casing conversions. `FacadeSlot::Index` now lets
  ABI facts request the existing shared `Slot::Index` coercion for Rust
  `usize`/Java index arguments, so `charAt` gets its `i32 -> usize` conversion
  from the facade argument planner before native lowering emits
  `.chars().nth(...)`;
- Rust stdlib coverage should continue through signature-fact clusters, not a
  broad hand-written model. For every generated-code failure that looks like
  "missing `&`", "needs `&mut`", "needs slice", "needs `usize`", "needs
  `Option`/`Result` adaptation", or "wrong owned/borrowed return", first ask
  whether a Rust std/core/alloc signature would produce the same ABI plan as a
  tailored rule. If yes, add or extract the signature fact and retire the
  special case. Keep tailored rules only for Java semantics or syntax that a
  Rust signature cannot express.
- the next stdlib signature cluster covers helper-backed Java string regex and
  split APIs whose Rust support signatures accept `AsRef<str>`:
  `matches`, `replaceFirst`, `replaceAll`, `split`, and `indexOf`. Their
  regex/pattern/needle arguments now flow through `FacadeAbiFact` borrowed
  string slots, while the existing runtime helpers still preserve Java
  regex/split/index semantics. Borrowed string literals now lower generically
  as `&str` literals for borrowed string slots instead of depending on a
  string-pattern-specific lowering branch; `indexOf` rendering also avoids
  appending `.as_str()` to an expression that is already an `&str` literal.
  Emitted lowering now prefers the shared ABI slot plan for the older
  `contains`, `startsWith`, `endsWith`, and literal `replace` cases too, with
  the previous string-pattern branch kept only as an unshaped fallback.
- static numeric parsing is also fact-backed for its mechanical argument ABI:
  `Integer.parseInt(String)` and `Double.parseDouble(String)` are modeled as
  associated/static `FacadeAbiFact` rows with borrowed string arguments. Native
  lowering still owns Java parse semantics and return primitive types, but
  argument lowering now comes from the shared ABI slot planner.
- Java string comparison helper calls are now fact-backed for their mechanical
  argument ABI: `String.compareTo`, `String.compareToIgnoreCase`, and
  `String.equalsIgnoreCase` route through explicit `FacadeAbiFact` rows with
  borrowed string slots, and the generated runtime helpers accept `AsRef<str>`
  so temporaries from `substring`, casing, and other string-producing calls can
  be passed without broad receiver string heuristics. `StringBuffer`/
  `StringBuilder.length()` and `Random.nextInt(int)` also have typed facade
  facts rather than falling through to erased-object support. Focused evidence:
  `java_string_comparison_methods_use_signature_helpers`,
  `jdk_mutable_buffer_and_random_numeric_methods_have_typed_facades`,
  `java_string_get_bytes_and_index_of_use_native_helpers`,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-chained-string-compare-r1`, which
  translates 29/29 files with 0 invalid-pattern files and 60 generated-Rust
  errors. The old broad string comparison rows are gone; the remaining
  `compare_to` row is numeric-wrapper Java semantics from
  `new Integer(String).compareTo(new Integer(String))`, so it should not be
  solved by widening the Rust string rule.
- Boxed primitive constructors now preserve Java numeric-wrapper semantics for
  string inputs before Rust ABI rendering: `new Integer(text)` lowers through
  `java_parse_i32(...)`, and boxed/primitive `compareTo(...)` lowers through
  `java_primitive_compare_to(...)` instead of leaving a Rust method call such
  as `.compare_to(...)`. Java string helper receivers now borrow through the
  shared `AsRef<str>` helper ABI, avoiding moves from owned `String` locals
  while still accepting `&str`, `String`, `&String`, and temporary substring
  results. Focused evidence:
  `boxed_primitive_constructors_lower_to_values`,
  `java_string_comparison_methods_use_signature_helpers`,
  `java_string_get_bytes_and_index_of_use_native_helpers`,
  `cargo fmt --check`, `cargo check --quiet`,
  `bash -n scripts/translate-focused-package.sh`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-boxed-primitive-asref-len-fix-r1`.
  That rerun translates 29/29 files with 0 invalid-pattern files and reduces
  generated-Rust errors from 60 to 57. The `compare_to`, `str_as_str`,
  `&usize` cast, and `s1`/`s2` moved-value regressions from intermediate
  attempts are absent in the final top rows.
- JDK collection argument ABI is now fact-backed through the same callable-key
  path for `List`/`ArrayList`/`LinkedList`, `Set`/`HashSet`, and
  `Map`/`HashMap`/`LinkedHashMap` methods such as `add`, `addAll`, `get`,
  `put`, and `containsKey`, plus zero-argument collection views and queries
  such as `size`, `isEmpty`, `toArray`, `keySet`, and `entrySet`. These facts
  drive mechanical slots such as owned values, borrowed string keys, borrowed
  slices, index coercions, and no-argument receiver ABI. Native collection
  target and return planning still run ahead of broad facade call rendering so
  Java semantics remain precise: `Map.get` keeps Java null/optional handling,
  `List.addAll` keeps Java collection-extension behavior, and receiver
  generics still provide the semantic Java argument type for optional unwraps
  and dyn-trait collection elements. The native collection renderer now
  delegates `size`, `keySet`, `keySet`-to-`Set`, `toArray`, and
  `iterator` projection spelling to `coercion::{len_i32_projection,
  owned_map_keys_vec_projection, owned_map_keys_set_projection,
  owned_collection_to_vec_projection, java_iterator_from_collection_projection}`,
  keeping `call_lowering.rs`/`lower.rs` focused on selecting the Java collection
  operation and target collection shape rather than formatting cloned Rust
  views. Focused evidence now asserts both the
  selected `CallableKey`/`FacadeAbiFact` rows and that generated idioms use
  borrowed `HashMap` string keys like `key.as_str()` without double-borrowing.
- Commons Math `Localizable` is now represented by built-in external-pack
  representation facts: `Localizable` is an object-safe dyn-trait facade, and
  `LocalizedFormats` implements it through type-capability facts. Semantic
  analysis imports built-in representation facts generically, so external enum
  constants such as `LocalizedFormats.OVERFLOW_IN_ADDITION` lower to
  `&LocalizedFormats::OVERFLOW_IN_ADDITION as &dyn Localizable` when passed to a
  `Localizable` slot. Focused tests:
  `commons_math_localized_formats_implements_localizable_pack_trait` and
  `commons_math_localized_formats_constants_borrow_as_localizable_pack_trait`.
  The FastQC math-util rerun at
  `.tmp/corpus-fastqc-math-util-after-localizable-pack-trait-r1` confirms the
  original `expected Localizable, found LocalizedFormats` rows are gone and
  generated-Rust errors dropped from 7 to 6.
- Landed baseline: focused package shared-support assembly now keeps dyn-trait
  facade support narrowly. The merger promotes crate-local trait dependencies
  only when needed by promoted trait impls, keeps unrelated erased-object
  support local, and the focused wrapper preserves `impl dyn Trait` inherent
  support such as anonymous constructor handoffs. Nested interface aliases now
  render as `type Alias = dyn OwnerAlias`, and anonymous
  `new Interface() { ... }` expressions passed to boxed dyn slots lower through
  `<dyn Trait>::anonymous()` instead of invalid `&dyn Trait::anonymous()` or
  redundant `.clone_box()` calls. Focused tests:
  `shared_support_merger_promotes_trait_dependency_for_shared_facade_impl`,
  `focused_package_shared_support_stripper_keeps_dyn_trait_inherent_impls`,
  and `anonymous_nested_interface_constructor_lowers_to_boxed_dyn_handoff`.
  FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-anonymous-box-no-clone-r1` has 10/10
  translated, 0 invalid-pattern files, and generated-Rust errors down to 3.
- Landed baseline: FastQC math-util now cargo-checks cleanly as a focused
  corpus. The remaining `ResizableDoubleArray` `internal_array` rows were fixed
  through shared signature/slot behavior rather than FastQC-specific lowering:
  bare Java field array element writes mark source methods as mutating `self`;
  `System.arraycopy` is a JDK ABI fact with shared source slice, mutable
  destination slice, and index slots; `FacadeSlot::Slice` unwraps optional Java
  storage to a required slice unless the ABI explicitly asks for an optional
  slot; optional array arguments to optional source slots lower through
  `as_deref()`/`as_deref_mut()`; and same-call alias snapshots now handle
  slotted calls containing index arguments plus mutable field projections.
  Focused evidence:
  `.tmp/corpus-fastqc-math-util-after-same-call-index-r1` has 10/10 translated,
  0 invalid-pattern files, `cargo_check=ok`, and 0 generated-Rust errors.
- Java `Optional<T>` now has a small Rust `Option<T>` ABI cluster:
  `isPresent`, `get`, and `orElse` are keyed `FacadeAbiFact` rows and lower
  through native Optional semantics (`is_some`, cloned unwrap with Java null
  receiver policy, and `unwrap_or`). The planner deliberately recognizes these
  methods from the declared Java receiver type, not from nullable internal
  `Option<T>` shapes, so unrelated nullable receivers such as
  `Future<T> future; future.get()` keep their original facade/instance
  behavior. Focused evidence asserts the Optional callable keys, argument
  slots, emitted Rust for `isPresent`/`get`/`orElse`, and the Future regression
  guard.
- `java.util.Objects.nonNull/isNull` now has a fact-backed Rust `Option`
  predicate cluster. The stdlib ABI facts model the mechanical Rust signature
  as optional-carrier predicates (`Option::is_some`/`Option::is_none`), while
  ordinary expression lowering keeps Java null semantics shape-aware:
  translated `Option<T>` values emit `.is_some()`/`.is_none()`, and concrete
  non-optional Rust values emit constant `true`/`false`. Callback and method
  reference lowering for concrete elements now avoids `Objects::non_null(...)`
  helper placeholders.
- `java.io.File.exists()` is now covered by a keyed JDK platform ABI fact for
  Rust `Path::exists`, with no arguments, borrowed receiver ABI, and owned bool
  return. This is intentionally only the mechanical Rust ABI: native Java File
  target rendering still has higher precedence so Java `File` values are
  projected through their path representation before calling Rust path
  existence checks.
- `java.nio.file.Files.exists(Path)` is now covered by the same JDK platform
  ABI/signature-fact direction for its borrowed path argument and owned bool
  return. Static Java utility owner selection still has precedence over broad
  facade ABI retargeting: the fact shapes the argument slot, while emitted Rust
  keeps the Java facade support call `JavaNioFileFiles::exists(path)` rather
  than rewriting the static call as `Path::exists(path)`.
- The same `java.nio.file.Files` support-signature cluster now covers
  `newInputStream(Path)`, `newOutputStream(Path)`,
  `newBufferedReader(Path)`, and `readAllLines(Path)`. These keyed
  `FacadeAbiFact` rows mark the path parameter as borrowed, the return as
  owned, and the support call as `Result`-wrapped. Focused evidence asserts the
  callable keys and proves `Files.readAllLines(path)` gets a borrowed path slot
  without bypassing the Java static facade target
  `JavaNioFileFiles::read_all_lines(path)?`.
- stream method-reference rendering now goes through
  `method_reference_plan::plan_method_reference` plus
  `method_reference_lowering::render_single_input_method_reference_body`.
  Stream lowering no longer owns a separate switch for `Integer::intValue`,
  `Object::toString`, getter-shaped references, `this::method`, or tuple
  accessor calls. The shared helper preserves the existing one-input closure
  body output while making method-reference rendering planner-owned.
- stream method-reference output-shape inference now goes through
  `method_reference_shape::infer_single_input_method_reference_output_shape`
  and `infer_method_reference_output_shape_for_plan`, so `stream.rs` no longer
  owns a raw method-reference result table. The remaining Java-specific cases
  are explicit in the method-reference shape helper and should be replaced by
  callable/result facts when those facts can prove the same result shape.
- common Java math/static numeric calls now have the same fact-backed argument
  ABI treatment: `Math.log`, `Math.exp`, `Math.toIntExact`, `Double.isNaN`,
  and `Float.isNaN` are modeled as associated/static `FacadeAbiFact` rows with
  owned/copy primitive arguments. Native lowering still owns Java-specific
  rendering, overload behavior, and primitive return semantics, but call
  argument shaping no longer depends on these method names being handled only
  in lowering.
- stream lambda receiver calls now ask the shared
  `plan_call_arguments_with_facts_and_args` path for shaped and explicit-slot
  argument plans before falling back to stream-local argument effects. The
  stream path also carries lambda parameter types into that planner, so
  fact-backed calls such as `String.charAt(index)` can select
  `FacadeSlot::Index`/`Slot::Index` through the same callable-key lookup as
  ordinary calls. Native `String.charAt` rendering enforces the final Rust
  `usize` handoff at the call boundary.
- stream lambda result-shape inference no longer owns a stream-only table for
  native Java calls such as `Integer.parseInt`, `Double.parseDouble`,
  `Math.toIntExact`, `String.trim`, `String.toLowerCase`, and `String.split`.
  Those result shapes now flow through a shared `call_plan`
  `native_java_call_lowered_type` projection, leaving stream planning to
  consume the same call target semantics as ordinary expression lowering.
- ordinary expression shape inference also asks that shared native-Java
  lowered-type projection before falling back to legacy borrowed-string view
  recognition or source signatures. This keeps non-stream and stream call
  result shapes aligned around `CallTargetPlan` rather than parallel
  method-name checks.
- Rust/JDK stdlib call behavior should be carried by ABI/signature projections
  whenever that gives the same result as a hand-written lowering branch. The
  current string-helper slice makes Java `String.replace`, `replaceFirst`,
  `replaceAll`, `split`, `matches`, and related runtime methods expose a shared
  return-type projection, so chained unresolved helpers such as
  `value.replaceAll(...).split(...)` can retarget through `java_string_split`
  without a split-only rewrite.
- Rust `std`/`core`/`alloc` are too large for broad hand-written modeling.
  Before adding a new Rust stdlib special case, write down the Rust signature
  that would imply the same receiver style, argument slots, borrow/mutability,
  slice/index shape, and return `Option`/`Result` behavior. If that signature
  can be represented as a `FacadeAbiFact` or related slot fact, implement the
  fact-backed path and keep any tailored rule only as a Java-semantic override
  or temporary migration bridge.
- The Rust-stdlib fact rule is equivalence-driven, not enumeration-driven:
  add small corpus-backed signature clusters only for APIs that generated code
  actually reaches. Missing `&`, `&mut`, slice, iterator, optional/result, or
  owned-versus-borrowed behavior should trigger a signature-fact investigation
  first, because those are ABI properties rather than reasons to grow another
  method-name table.
- The stdlib projection is deliberately lower precedence than precise semantic
  shape rules. Borrowed string-view handling, notably `String.trim()` where the
  Rust result can remain a borrowed view, must run before broad nominal
  return-signature facts to avoid semantic drift and unnecessary allocation.
- Java `char` primitive conversions are now handled in all shared scalar
  conversion paths rather than with site-specific casts. Explicit Java casts,
  return/argument slot scalar casts, and primitive widening coercions use
  `char::from_u32((value as u32) & 0xffff).unwrap_or('\0')` for numeric-to-char
  and a `char -> u32 -> target` path for char-to-number. This removes the
  invalid Rust `i32 as char` and `char as f64` rows from the FastQC Utilities
  `QualityCount` subset while keeping Java UTF-16 narrowing explicit at the
  conversion boundary.
- native map-held collection mutation now has a shared lowering path: Java
  `map.get(key).add(value)`/set insert/extend shapes lower the map lookup as
  `get_mut(key).expect("Java null receiver")` before applying the existing
  native collection mutation. This fixes the FastQC Utilities
  `fileBases.get(baseName).add(file)` rows without changing read-side
  `Map.get`, which still lowers as an optional shared borrow. Focused evidence:
  `.tmp/corpus-fastqc-utilities-after-map-get-mut` keeps 10/10 files
  translated and drops generated Rust errors from 20 to 18; the remaining rows
  are the matrix row assignment family and the known
  `MultiMemberGZIPInputStream` object-graph/native-input-stream gap.
- 2D Java arrays now split ndarray eligibility from generic two-dimensional
  array identity. Primitive rectangular `T[][]` remains an `ndarray::Array2<T>`
  candidate, while object/ragged arrays such as FastQC `File[][]` grouping stay
  as `Vec<Vec<File>>`. Slot-aware creation handles `new File[n][]` as
  `vec![Vec::new(); n]`, and `map.get(key).toArray(...)` can fill a row by
  cloning the map-held vector through a borrowed `HashMap::get` key. Focused
  evidence: `.tmp/corpus-fastqc-utilities-after-ragged-object-arrays-r2` keeps
  10/10 files translated and drops generated Rust errors from 18 to 12; no
  `Array2<File>`/`row_mut` rows remain. The remaining focused Utilities errors
  are now all in `MultiMemberGZIPInputStream` object-graph/native input-stream
  modeling.
- JDK stream wrapper protected state now comes from typed stdlib/platform facts
  instead of ad hoc member-access lowering. `GZIPInputStream` exposes inherited
  `in`, `inf`, `buf`, and `len` field shapes, directly self-recursive source
  fields are boxed as `Option<Box<T>>`, and `Inflater.getRemaining()` is a
  modeled no-throw JDK facade API. Shadowing is guarded so a local source class
  named `FilterInputStream` is not mistaken for `java.io.FilterInputStream`.
  Verified with
  `jdk_stream_wrapper_inherits_typed_protected_stream_fields`,
  `source_self_referential_instance_fields_use_boxed_optional_storage`, and
  `lowercase_keyword_receiver_is_not_guessed_as_static_owner`. Focused
  evidence:
  `.tmp/corpus-fastqc-utilities-after-jdk-stream-inflater-facts-r1` keeps 10/10
  files translated, 1 invalid-pattern file, and drops generated Rust errors
  from 12 to 10. Remaining rows are still intentionally concentrated in
  `MultiMemberGZIPInputStream`: unsupported self-linking constructors, erased
  `PushbackInputStream` cast/unread, missing `?` on `read_3`, and clone recovery
  blocked by the unresolved object-identity model.
- JDK IO result ABI facts now cover `InputStream.read`/`read_3` as
  result-wrapped facade methods, and the early external-call lowering path
  propagates `?` before slot conversion so optional wrappers such as
  `BufferedReader.readLine()` keep the existing `Some(reader.read_line()?)`
  shape. Verified with
  `jdk_stream_wrapper_inherits_typed_protected_stream_fields` plus the
  `buffered_reader_read_line_*` regression fixtures. Focused evidence:
  `.tmp/corpus-fastqc-utilities-after-jdk-io-result-abi-r1` keeps 10/10 files
  translated, 1 invalid-pattern file, and drops generated Rust errors from 10
  to 9. The former missing-`?` `read_3` row is gone; remaining rows are the two
  unsupported self-linking constructors, their return-type cascades, erased
  `PushbackInputStream` cast/unread, and clone recovery blocked by unresolved
  object identity.
- Unsupported constructor bodies now keep the precise `compile_error!`
  diagnostic but add a diverging `panic!` tail, so intentionally unsupported
  non-unit constructors no longer create extra Rust return-type mismatch
  cascades. Verified with the unsupported constructor design fixture and
  `jdk_stream_wrapper_super_with_self_linking_trailing_mutation_is_precise_gap`.
  Focused evidence:
  `.tmp/corpus-fastqc-utilities-after-unsupported-constructor-diverge-r1` keeps
  10/10 files translated, 1 invalid-pattern file, and drops generated Rust
  errors from 9 to 7. The former constructor `E0308` rows are gone; remaining
  rows are the two deliberate object-graph `compile_error!` diagnostics, erased
  `PushbackInputStream` cast/unread, and clone recovery blocked by unresolved
  object identity.
- Explicit external facade cast receivers now preserve the cast target during
  receiver lowering. `PushbackInputStream.unread(byte[], int, int)` is modeled
  as a small JDK facade ABI fact with a borrowed byte-buffer slot, and
  `((PushbackInputStream)this.in).unread(...)` lowers through a visible facade
  cast boundary instead of teaching `InputStream` a false `unread` method.
  Verified with
  `explicit_pushback_input_stream_cast_receiver_keeps_facade_target`. Focused
  evidence:
  `.tmp/corpus-fastqc-utilities-after-explicit-facade-cast-r1` keeps 10/10
  files translated, 1 invalid-pattern file, and drops generated Rust errors
  from 7 to 5. The former `InputStream.unread` rows are gone; remaining rows
  are the two deliberate object-graph `compile_error!` diagnostics and the two
  `self.clone()` cascades from attempting to call those unsupported private
  self-linking constructors.
- Unsupported source constructor calls now stop at the unsupported constructor
  boundary when the target constructor body is already classified as
  `super(...)` plus unsupported object graph mutation. This removes the
  `self.clone()` recovery cascade from `new MultiMemberGZIPInputStream(this)`
  without pretending the self-linking object graph is mechanically supported.
  JDK stream-wrapper `super.read(byte[], int, int)` calls now lower through the
  inherited `InputStream.read_3` facade ABI and typed `in` field instead of
  producing `Default::default()?`. Verified with
  `jdk_stream_wrapper_super_with_self_linking_trailing_mutation_is_precise_gap`
  and the extended `jdk_stream_wrapper_inherits_typed_protected_stream_fields`
  fixture. Focused evidence:
  `.tmp/corpus-fastqc-utilities-after-super-read-boundary-r1` keeps 10/10
  files translated, 1 invalid-pattern file, and drops generated Rust errors
  from 5 to 3. The top errors are now only the two deliberate
  `unsupported Java constructor: super(...) followed by unsupported object graph mutation`
  diagnostics plus the cargo summary row.
- Full FastQC refresh, 2026-07-24:
  `.tmp/corpus-batch-current-fastqc-refresh-20260724-r1` translates 156/156
  files with 0 translation failures, 28 invalid-pattern files, and 1427
  generated-Rust cargo errors. Runtime was 1,577,967 ms total: translation
  1,132,882 ms, crate assembly 432,084 ms, and cargo check 12,135 ms. The main
  compile families are still broad mismatched types, fallible `?` propagation,
  erased `Box<dyn Any>` GUI/module calls, switch/state scaffold names, and
  shared-support facade identity problems around Base64/OutputStream.
- Shared-support merger follow-up: module-local aliases whose name is already
  registered as a shared-support type are now removal-only, and existing bad
  aliases in the shared-support input are stripped before appending merged
  items. The merger also synthesizes JDK `OutputStream` upcasts in the same
  shared type-universe pass used for `InputStream`, and source-owned nested
  types that collide with existing shared facades are removed instead of
  promoting their stateful impl bodies onto opaque facade structs. This targets
  the full FastQC `pub type OutputStream = Base64OutputStream`,
  `Base64OutputStream: From<ZipOutputStream>`, and missing Base64 state-field
  families without translating Base64 internals as core Java semantics.
- Base64 output-stream constructor follow-up: Apache Commons Codec
  `Base64OutputStream(OutputStream)` and iHarder
  `Base64.OutputStream(OutputStream)` are now modeled as external-pack
  constructor boundaries. Constructor lowering can build the concrete external
  stream and then apply the existing external `Into` adapter into
  `OutputStream`, so the focused regression no longer emits erased
  `invalid Java cast` downcasts for `new Base64OutputStream(out)` assigned to
  an `OutputStream` slot. The no-later-read constructor assignment now moves
  `out` directly, while later-read cases still keep the audited clone. This was
  fixed by trusting block-level later-read facts over coarse whole-callable use
  counts when statement suffix data is available.
- FastQC focused proof: after rebuilding the release translator,
  `.tmp/corpus-fastqc-abstract-qcmodule-after-base64-qualified-new-r2`
  translates 8/8 AbstractQCModule subset files, reports 0 invalid-pattern
  files, and drops generated-Rust errors from 12 to 9. `ImageToBase64` now
  emits `Base64OutputStream::new(os.clone()/* TODO(translation): validate added clone */).into()`
  with no `invalid Java cast` downcast. The remaining focused compile family is
  `cannot move out of index of Vec<String>`, so the next slice should fixture
  indexed owned `String` reads that flow into owned slots and clone/recover via
  the shared slot policy. 2026-07-27 recheck: the focused
  `indexed_string_array_reads_clone_only_for_owned_slots` idiom test is green
  and compiles generated Rust, so treat future recurrence as corpus evidence
  drift to rerun rather than as an unfixtureed translator gap.
- the old emitted `lower_string_pattern_args` path has been retired. The only
  remaining no-facts compatibility path for `contains`/`startsWith`/`endsWith`
  and literal `replace` now builds an explicit borrowed-string
  `ArgumentLoweringPlan` and then uses the normal `lower_args_with_plan`
  machinery. In normal translation, stdlib `FacadeAbiFact` rows should be the
  preferred source of those borrowed pattern slots.
- normal single-file and batch translation include those facts as low-precedence
  defaults before user/source-summary/external facts are extended;
- call argument planning now consumes `FacadeAbiFact.args` to derive borrowed,
  mutable-borrowed, slice, optional, owned, and explicit index argument slots
  before falling back to older external argument-effect rows;
- parsed facade signatures and manifest Rust return metadata now preserve
  mutable slice ABI as `FacadeSlot::Slice(MutBorrowed)`, and the shared
  argument planner turns that slot into a mutable borrowed array/slice shape.
  The facade ingestion path also derives borrowed-return `BorrowSource` from
  the parsed return slot instead of re-checking whether the raw Rust return
  payload starts with `&`. The remaining raw Rust-type string checks are
  boundary parsing/classification helpers and should keep shrinking as more
  parsed Rust signatures become structured facts.
- JDK facade signatures for the exposed FastQC IO/XML/ZIP/AWT/Base64/ImageIO
  rows now come from platform facts/support metadata instead of generic
  `R: Default` or `()` fallback methods. The late opaque-support pass now also
  honors the shared static-facade classification, so static signatures such as
  `RepaintManager.currentManager(...)` stay associated functions;
- a narrow source singleton idiom now treats `private static T instance = new
  T(); public static T getInstance() { return instance; }` as an owned
  initialized source value, avoiding nonexistent Rust static storage.
- source structs with `Vec<Box<dyn Trait>>` fields can derive `Clone` when the
  generated trait-object clone adapter exists, so source report/archive values
  reused by value no longer fail on dyn-trait container fields;
- local Java array declarators now preserve `[]` dimensions written between the
  type and variable name, including `byte [] array = new byte[128]`, while
  ignoring array markers that belong to generic arguments such as
  `ArrayList<String[]> group`;
- Java `String(byte[], int, int)` construction now lowers through the shared
  runtime helper `java_string_from_byte_range(&bytes, offset, len)` instead of
  emitting an invalid `String::new(...)` overload;
- JDK `InputStream.read(byte[], int, int)` and `OutputStream.write(byte[], int,
  int)` now use external argument/ABI slot facts so reused byte-array buffers
  lower as slices rather than moved owned values. `InputStream.read`/`read_3`
  uses a `FacadeSlot::Slice(MutBorrowed)` first argument, while
  `OutputStream.write`/`write_3` uses `FacadeSlot::Slice(Borrowed)`. Same-call
  alias reads that appear after a mutable argument, such as
  `input.read(buffer, 0, buffer.length)`, are evaluated into a call-local
  temporary before the mutable borrow so generated Rust satisfies borrow
  checking without an IO-specific lowering rule. Source signature and local
  mutability inference now also consume mutable facade argument slots, so
  values passed to `&mut` facade arguments become `&mut [T]` parameters or
  `let mut` locals from facts rather than from stream-wrapper-specific rules.
- internal source-call signature propagation no longer lets a caller-owned
  array local force an otherwise borrowed callee parameter back to owned
  `Vec<T>` storage. Read-only nullable source array parameters can now remain
  `Option<&[T]>`/borrowed slices while output parameters keep `&mut [T]`.
- borrowed Java array slots now normalize to Rust slice views when the ABI asks
  for a borrow, including nullable array parameters. Optional mutable arrays
  reborrow through `as_deref()`/`as_deref_mut()` rather than move-based
  `Option::map(...)`, so a caller can pass `Option<&mut [T]>` to a read-only
  helper and still mutate the original optional slice afterward.
- caller-local mutable binding requirements no longer override a callee
  parameter that the callee body proves is read-only. This fixes same-array
  alias calls such as `splitMult(xs, xs, out)` by planning shared borrows for
  read-only inputs and a mutable borrow only for the output slot. The remaining
  FastQC math-util alias family is now a read/write overlap on the same output
  binding, not read-only input aliases.
- same-call read/write aliases now snapshot the shared-read argument when a
  later argument mutably borrows the same local. This preserves Java's
  pre-call read semantics for calls such as `splitAdd(result, temps, result)`
  and avoids Rust's overlapping immutable/mutable borrow error without adding a
  package-specific `splitAdd` rule.
- lowercase JDK static-owner recovery now includes `StrictMath`, and
  `StrictMath.IEEEremainder(double, double)` is represented as a typed JDK
  facade/static support method. This removes the FastQC math-util
  `strict_math.i_e_e_eremainder(...)` unresolved receiver row without adding a
  corpus-specific rule.
- focused crate assembly now registers `JavaPathArg` as a shared support trait
  and strips module-local impl blocks for shared support traits. This avoids
  both duplicate root re-exports and conflicting trait implementations when a
  generated source module contains fallback path/URI/File shims that have
  already been promoted to crate-level support.
- static-final 2D primitive tables are kept as `'static` nested slices for
  cheap constant storage, but calls or returns that need an owned/borrowed
  matrix slot hand off through `ndarray::Array2::from_rows(...)`. Static row
  reads that flow into owned Java array slots recover with `.to_vec()`. The
  local focused-crate `ndarray` shim now provides `Array2::from_rows(...)` and
  row iteration, so offline corpus checks exercise the real generated Rust
  surface without downloading crates.
- enhanced-for loops over primitive sequence elements now preserve Java value
  semantics at the iterator boundary by using `.iter().copied()` and installing
  an owned primitive loop-variable shape. This keeps by-value source helper
  calls such as `format(double)` aligned with the emitted Rust signature instead
  of relying on call-site-specific dereference rules.
- source method effect facts are now also published under emitted overload
  Rust names when those names differ from the Java declaration name, and
  exception plans are keyed by emitted Rust method identity with a Java-name
  fallback. This keeps resolved overload call targets and `Result` shell
  planning attached to the same source effect. Focused FastQC math-util
  evidence:
  `.tmp/corpus-fastqc-math-util-after-overload-exception-key-r1` translates
  10/10 files with 0 translation failures, 0 invalid-pattern files, and 20
  generated-Rust errors, down from 23 after primitive enhanced-for copying.
  The `ResizableDoubleArray.copy(ResizableDoubleArray, ResizableDoubleArray)`
  static helper now returns `Result<(), JavaException>`. Remaining fallibility
  rows are overloaded `FastMathCalc.printarray(...)` callers, non-void
  `Precision.round_float_int_int(...)`, and the instance `ResizableDoubleArray
  copy()` caller that invokes the now-fallible static helper. These should be
  fixed by extending source-call effect projection over resolved/current-type
  calls, not by adding method-name tables.
- Rust expression emission now parenthesizes compound operands for explicit
  primitive casts, so Java casts such as `(int)(bits >> 52)` lower as
  `((bits >> 52) as i32)` rather than `bits >> 52 as i32`. This is a central
  cast-rendering fix, not a FastMath-specific rewrite. Focused FastQC
  math-util follow-up: internal erased-`Object` helper parameters now keep
  erased-object representation constraints local instead of erasing precise
  same-type caller bindings, and public source parameters with field or
  array-element mutations drop the owned public-boundary default before slot
  solving. The focused run
  `.tmp/corpus-fastqc-math-util-after-public-mutated-param-r1` translates
  10/10 files with 0 translation failures, 0 invalid-pattern files, and 17
  generated-Rust errors; `ResizableDoubleArray.copy(..., dest)` now emits
  `dest: &mut ResizableDoubleArray`. Remaining rows include source moves
  through erased `Object` null-check calls and `System.arraycopy` movement from
  optional fields, so the next slice should use ABI/signature facts or a
  structural null-check assertion model rather than a `MathUtils` name table.
- resolved source calls now propagate only mutable-binding requirements from
  callee mutable-borrow parameters back to caller bindings. This keeps local
  out values mutable when passed as `&mut` to resolved public static source
  helpers without reviving broad source-call ownership propagation across
  unknown boundaries. Focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-source-mut-local-abi-infra-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 14 generated-Rust errors; `ResizableDoubleArray.copy()` now declares
  `let mut result`.
- a trial `System.arraycopy` ABI fact was rejected and backed out. Borrowed
  source/mutable-destination/index slots made local fixtures compile but
  worsened the focused FastQC math-util subset to 19 generated-Rust errors
  because Java argument order can read `dest.internalArray.length` after a
  mutable borrow of `dest.internalArray` has already been planned. Before
  `arraycopy` becomes a simple signature fact, calls need call-level temporary
  staging for source/destination alias reads.
- source method-effect key matching now normalizes both stored keys and lookup
  keys between Java spelling and emitted Rust spelling. Together with the
  callgraph caller key using emitted Rust overload names, this lets thrown
  effects propagate from `checkLen`/`roundUnscaled`/fallible static helpers to
  overloaded or current-type callers without adding method-name tables.
  Focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-symmetric-method-effect-key-r2`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 8 generated-Rust errors, down from the stale 14-error rerun. The previous
  `?`-inside-non-`Result` rows for `FastMathCalc.printarray(...)`,
  `Precision.round_float_int_int(...)`, and `ResizableDoubleArray.copy()` are
  gone.
- exception constructor message arguments now lower through display slots
  except for the explicit second-argument cause path. This lets erased Java
  `Object` and `Object...` arguments use `java_erased_object_to_string(...)`
  before entering `format!`, and erased-object runtime support detection now
  includes Java `Object`/`Object[]` parameter and return types. Focused
  FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-exception-display-slot-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 7 generated-Rust errors. The former
  ``Vec<Box<dyn Any>>` doesn't implement Display` row is gone.
- remaining FastQC math-util rows after that run are: external/support-shell
  enum-to-interface coercion (`LocalizedFormats` passed to `Localizable`),
  trait-object alias and anonymous callback construction in `Incrementor`, and
  two `ResizableDoubleArray` `internal_array` mutable/move rows. The
  `LocalizedFormats`/`Localizable` case should be addressed via external or
  source-summary hierarchy/capability facts, not an Apache-method-name rule.
  planning for later read arguments that alias an earlier mutable slot.
  math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-explicit-cast-parentheses-r1` translates
  10/10 files with 0 translation failures, 0 invalid-pattern files, and 17
  generated-Rust errors, down from 20 after overload exception keying. The
  three `i64 -> i32` bit-extraction rows in `FastMath` are gone. Remaining
  families are 5 mismatched types, 4 fallible calls in non-`Result` methods,
  trait-object alias/spelling, mutable receiver drift, erased-object vector
  display, and one value-position `if` without an `else`.
- abstract source methods without bodies now emit a typed diverging placeholder
  instead of an empty Rust function body. This preserves abstract/template
  method semantics while keeping generated Rust compileable for non-void
  abstract methods such as `ContinuedFraction.getA/getB`. Focused FastQC
  math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-abstract-method-placeholders-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 15 generated-Rust errors, down from 17 after explicit cast
  parenthesizing. The two `expected f64, found ()` rows from abstract
  `ContinuedFraction` hooks are gone. Remaining families are 4 fallible calls
  in non-`Result` methods, 3 mismatched types, trait-object alias/spelling,
  mutable receiver drift, erased-object vector display, and one value-position
  `if` without an `else`.
- Trait implementation forwarding now compares the solved trait-method slot
  shape with the solved inherent implementation slot shape before rendering the
  delegated call. When a trait method receives a borrowed slice view but the
  concrete implementation stores/owns the Java array, the forwarding argument
  recovers with `.to_vec()` instead of passing `&[T]` into a `Vec<T>` slot.
  This is a shared slot-shape forwarding rule, not a `DoubleArray` or
  FastQC-specific mapping. Focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-trait-forwarding-slice-recovery-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 14 generated-Rust errors, down from 15 after abstract method
  placeholders. The generated `DoubleArray for ResizableDoubleArray`
  forwarding body now emits
  `ResizableDoubleArray::add_elements(self, values.to_vec());`. Remaining
  families are 4 fallible calls in non-`Result` methods, 2 mismatched types,
  trait-object alias/spelling, mutable receiver drift, erased-object vector
  display, and one value-position `if` without an `else`.
- Java `synchronized (...) { ... }` statements are now preserved by the parser
  as ordinary block statements instead of being skipped during block
  collection. This intentionally ignores monitor locking for now, but keeps the
  Java body visible to semantic analysis and lowering. Focused FastQC
  math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-synchronized-block-body-r1` translates
  10/10 files with 0 translation failures, 1 invalid-pattern file, and 16
  generated-Rust errors. The previous tail `if`/missing-`else` boolean row is
  gone because `ResizableDoubleArray.equals(...)` now retains the synchronized
  body after the guard returns. This exposes real follow-up rows that were
  previously hidden: Java cast/downcast handling for `(ResizableDoubleArray)
  object`, `Arrays.equals(...)` static facade lowering, optional array
  `.len()`/arraycopy access inside synchronized copy blocks, and receiver
  mutability for synchronized static copy destinations.
- Rejected `instanceof` guards now feed the same exact-erased-object refinement
  path as existing `getClass()` guards. After
  `if (object instanceof T == false) return false;`, a later `(T) object`
  cast lowers to a borrowed `downcast_ref::<T>()` view instead of moving out of
  `Box<dyn Any>` through `downcast::<T>()`. JDK `Arrays.equals(...)` is also
  modeled as a small typed platform/static method and lowers to Rust equality
  rather than an unresolved lowercase `arrays` receiver. Focused FastQC
  math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-arrays-equals-r1` translates 10/10 files
  with 0 translation failures, 0 invalid-pattern files, and 15 generated-Rust
  errors. The invalid Java cast scan is clean again, `ResizableDoubleArray`
  now binds `other` as `&ResizableDoubleArray`, and the previous
  `cannot find value arrays` row is gone.
- latest focused FastQC AbstractQCModule evidence:
  `.tmp/corpus-fastqc-abstract-qcmodule-after-io-borrow-slots-r1` translates
  8/8 files, reports 0 invalid-pattern files, and drops generated-Rust errors
  to 2. The byte-buffer move row is gone. The remaining compile error is
  `SequenceFile.name()` returning `()` in the primary-only focused subset; a
  broad full FastQC source-summary run proves the real interface method returns
  `String`, but that summary root currently introduces duplicate/conflicting
  symbols into the focused crate. Do not fix this with a corpus-specific
  `name()` table; the intended fix is a typed source/context/facade return
  slot fact for omitted context interfaces.
- omitted context receiver methods used directly in display/string sinks now
  get a fallback `String` support return hint keyed by receiver type, method,
  and arity. This only replaces weak support returns such as `()` and is skipped
  when JDK/external-pack facts already provide a concrete return type, so
  precise facts like `TableModel.getValueAt(...) -> Object` remain
  authoritative. Latest focused FastQC AbstractQCModule evidence:
  `.tmp/corpus-fastqc-abstract-qcmodule-after-display-context-return-r2`
  translates 8/8 files, reports 0 invalid-pattern files, and offline
  `cargo check` passes with 0 generated-Rust errors.
- full FastQC rerun after the display-context return fix:
  `.tmp/corpus-batch-current-fastqc-after-display-context-return-r1`
  translates 156/156 files with 0 translation failures. It improves the prior
  full FastQC baseline from 20 invalid-pattern files and 2733 generated-Rust
  errors to 18 invalid-pattern files and 2143 generated-Rust errors. Remaining
  top families are optional array/vector indexing, weak GUI/AWT erased return
  surfaces, class literals/tokens, missing facade constants, array creation,
  switch scaffolding, and source object/constructor representation gaps.
  Runtime remains dominated by global signature propagation:
  `global_signature_ms=525467`, `signature_fixpoint_ms=516779`, and
  `signature_propagation_sweep_ms=502527` out of `total_elapsed_ms=924533`.
  This reinforces the need for indexed signature propagation alongside broader
  ABI-fact coverage.
- optional Java sequence receivers now unwrap at read/index/length use sites
  instead of rendering invalid Rust such as `Option<Vec<T>>[...]` or
  `Option<Vec<T>>.length`. The fix routes Java `.length` on optional
  array-like storage through the shared `MemberAccessPlan` and uses explicit
  read-side `as_ref().expect("Java null receiver")` or write-side
  `as_mut().expect("Java null receiver")` for array assignment targets. Focused
  regressions cover nullable `String[]` fields, nullable `double[][]` fields,
  and the existing erased-object varargs row-push case without reintroducing
  clone recovery. This is still not the full FastQC local out-parameter answer:
  reused mutable arrays passed through facade/source calls should move next to
  ABI facts with `&mut [T]`/owned recovery rather than ad hoc array-name rules.
- HTSJDK `SAMTag.CO`/`NM` are now typed external field facts with
  `ExternalFieldKind::EnumVariant`. Static constant support treats enum-variant
  facts as owner-typed, avoiding the old simple-name fallback conflict where
  `SAMTag` could be emitted both as an alias and as an opaque support struct.
- Optional source fields copied through guarded nullable copy-constructor
  receivers now use the receiver owner's field shape before optional wrapping,
  so `Option<T>` fields copy directly instead of becoming `Some(Option<T>)`.
  Latest focused FASTQ evidence:
  `.tmp/corpus-batch-current-htsjdk-fastq-after-optional-copy-samtag`
  translates 8/8 files, reports 0 invalid-pattern files, and offline
  `cargo check` passes with 0 generated-Rust errors.
- Parameter array declarators now preserve dimensions written after the
  parameter name, such as `final double result[]`. Focused regressions prove
  read-only parameters lower as shared slices and written out-parameters lower
  as mutable slices instead of scalar `f64`.
- Nullable sequence out-parameters that are guarded by `param != null` now
  render writes through `param.as_mut().expect("Java null receiver")` and reads
  through `param.as_ref().expect("Java null receiver")` without leaving stale
  `.unwrap().as_mut().expect(...)` chains. Method parameter rendering marks
  optional sequence out-parameters as `mut` when the Rust binding itself must
  be mutable for `Option::as_mut()`.
- Current local gates are green after this batch:
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 17 passed,
  `cargo test --test idiom_gaps --quiet` with 396 passed, and
  `cargo build --release --quiet --bin new-java2rust`.
- Latest focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-param-array-and-refined-write-r2`
  translates 10/10 files with 0 translation failures, 1 invalid-pattern file,
  and 351 generated-Rust errors. The previous `cannot index into f64` family
  and stale optional receiver `.unwrap().as_mut().expect(...)` family are gone.
  Remaining math-util families are missing source/facade constants and helper
  types (`FastMath`/`FastMathCalc` tables, `LocalizedFormats`, `ExpIntTable`,
  `LnMant`), reserved identifier/raw-name leakage around `in`/`as`, local
  mutable binding gaps such as `reduce_results`, and math facade constants for
  `BigDecimal`/`BigInteger`.
- Internal source-call signature propagation now carries `MutableBinding` back
  to caller locals when the callee parameter is a mutable-borrow slot. This
  fixes local Java arrays lowered as `Vec<T>` and passed to source out-array
  methods via `.as_mut_slice()`. Focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-internal-mut-borrow-local-r1` translates
  10/10 files and drops generated-Rust errors to 347; the
  `reduce_results`/`r#as` mutable-binding rows are gone.
- Literal Java `while (true)` now lowers as Rust `loop`, preserving definite
  assignment for locals initialized in the loop before `break`. Focused FastQC
  math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-while-true-loop-r1` translates 10/10
  files and drops generated-Rust errors to 341; the `rem_a`/`rem_b`
  possibly-uninitialized rows are gone.
- Source `static final` primitive expression constants now emit associated
  constants and resolve normalized references back to original Java constant
  names. Source `static final` primitive/String array literal constants now
  emit associated static slices, including old-style field declarators such as
  `double FACT[]` and nested `double[][]` tables. Static const table reads use
  nested slice indexing, static table `.length` uses `.len()`, shared slice
  arguments do not append `.as_slice()` to already-borrowed associated slices,
  and array initializer parsing ignores comment nodes. Focused FastQC
  math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-static-array-consts-r3` translates 10/10
  files with 0 translation failures, 1 invalid-pattern file, and 196
  generated-Rust errors. The previous scalar constant rows, missing literal
  table rows, `.nrows()` on static slices, `.as_slice()` on static slices, and
  missing `FastMathCalc::FACT` rows are gone. Remaining top families are
  mismatched types, omitted/context dependency types such as `LocalizedFormats`,
  reserved identifier leakage around `in`, helper table classes
  (`ExpIntTable`, `ExpFracTable`, `LnMant`), `Float.isNaN` facade casing,
  fallible calls in non-`Result` methods, and math facade constants for
  `BigDecimal`/`BigInteger`.
- `Float.isNaN(...)` now shares the native floating-point static-call plan with
  `Double.isNaN(...)`, so generated Rust uses primitive `.is_nan()` and no
  longer relies on missing `Float::is_na_n` support methods. Focused FastQC
  math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-float-isnan-r1` translates 10/10 files
  with 0 translation failures, 1 invalid-pattern file, and 189 generated-Rust
  errors. Remaining top families are mismatched types, omitted/context
  dependency types such as `LocalizedFormats`, reserved identifier leakage
  around `in`, helper table classes (`ExpIntTable`, `ExpFracTable`, `LnMant`),
  fallible calls in non-`Result` methods, receiverless same-class static calls
  such as `round(...)`, and math facade constants for `BigDecimal`/`BigInteger`.
- Receiverless same-class static overload calls now use current source
  signatures and partial known argument types to retarget to
  `Self::<overload>(...)`. Exact primitive matches outrank widening matches
  when unknown arguments such as missing static constants cannot fully type the
  call. Focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-static-self-overload-r3` translates
  10/10 files with 0 translation failures, 1 invalid-pattern file, and 187
  generated-Rust errors. The previous free `round(...)` rows are gone; generated
  `Precision` calls now use `Self::round_double_int_int(...)` and
  `Self::round_float_int_int(...)`. Remaining top families are mismatched
  types, omitted/context dependency types such as `LocalizedFormats`, reserved
  identifier leakage around `in`, helper table classes (`ExpIntTable`,
  `ExpFracTable`, `LnMant`), fallible calls in non-`Result` methods, and math
  facade constants for `BigDecimal`/`BigInteger`.
- JDK numeric facade constants and wrapper value APIs now use a small
  platform fact/support cluster instead of weak ad hoc fallback stubs:
  `BigDecimal` rounding constants and `BigInteger.ZERO`/`ONE` are typed
  static-final facts; `BigDecimal.setScale(...).doubleValue()` keeps a
  `BigDecimal -> f64` chain; `new Float(x)`/`new Double(x)` erase to primitive
  values; and boxed primitive `hashCode()` lowers through native primitive
  hash-code expressions. Support dedupe now counts only function parameter
  lists and avoids re-emitting value-object static-final constants through the
  enum-like surface path while preserving true external enum constants.
  Verified with `jdk_decimal_and_integer_constants_are_typed_facade_facts`,
  `boxed_primitive_constructors_lower_to_values`, `cargo fmt --check`,
  `cargo check --quiet`, `cargo test --test parser_gaps --quiet` with 17
  passed, `cargo test --test idiom_gaps --quiet` with 398 passed, and
  `cargo build --release --quiet --bin new-java2rust`.
  Latest focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-wrapper-hashcode-r1` translates 10/10
  files with 0 translation failures, 1 invalid-pattern file, and 159
  generated-Rust errors. The previous missing `BigDecimal::ROUND_*`,
  missing `BigInteger::ZERO`/`ONE`, `f32::new`/`f64::new`,
  `set_scale(...) -> ()`, and primitive `.hash_code()` rows are gone.
  Remaining top families are mismatched types, omitted/context dependency
  types such as `LocalizedFormats`, reserved identifier leakage around `in`,
  helper table classes (`ExpIntTable`, `ExpFracTable`, `LnMant`), fallible
  calls in non-`Result` methods, and remaining source/facade constants such as
  `MathUtils::TWO_PI` and `Precision::SGN_MASK_FLOAT`.
- Source `static final` constants can now reference other source constants by
  owner and Java hex int sign-bit literals preserve Java two's-complement
  `int` semantics. Focused FastQC math-util evidence:
  `.tmp/corpus-fastqc-math-util-after-cross-source-consts-r1` translates 10/10
  files with 0 translation failures, 1 invalid-pattern file, and 155
  generated-Rust errors. The previous missing `MathUtils::TWO_PI` and
  `Precision::SGN_MASK_FLOAT` rows are gone.
- Nested static source helper classes now get simple-name type aliases when
  Rust flattens them to `OuterInner`, and `static final` primitive/String array
  fields without inline initializers emit conservative associated empty-slice
  constants for static-block-populated table reads. Verified with
  `nested_static_table_classes_get_simple_aliases_and_default_array_consts`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 400 passing tests,
  `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
  math-util rerun
  `.tmp/corpus-fastqc-math-util-after-nested-static-table-alias-r1`. The subset
  remains 10/10 translated with 0 translation failures and 1 invalid-pattern
  file, while generated-Rust errors drop to 146. The missing helper-type rows
  for `ExpIntTable`, `ExpFracTable`, and `LnMant` are gone; the remaining top
  families are mismatched types, omitted/context dependency constants such as
  `LocalizedFormats`, reserved identifier leakage around `in`,
  receiverless/local static helper calls such as `fast_math_calc`, fallible
  calls in non-`Result` methods, and remaining scalar/type coercion rows.
- Apache Commons Math `LocalizedFormats` enum constants used by the focused
  math-util subset are now typed external-pack enum-variant field facts, and
  enum variants are emitted from the used static-constant support path rather
  than duplicated by broad external field surface emission. Verified with
  `commons_math_localized_formats_constants_are_pack_enum_fields`,
  `builtin_external_member_lookup_uses_cached_pack_facts`,
  `sam_tag_enum_variant_constants_emit_typed_facade_surfaces`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 401 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. A full
  `cargo test --test external_packs --quiet` now passes after fixing the SAM
  header indexed-getter role, keeping Broad Hellbender constructors in the
  Broad manifest instead of Tribble index, removing the ambiguous iHarder
  `Base64OutputStream` simple alias that collided with Apache Commons Codec,
  and deduping source-facing exception aliases in catch facts. The ambiguous
  bare `Option`/`Attribute` regressions from an attempted broad field-alias
  change remain gone. Focused FastQC math-util rerun
  `.tmp/corpus-fastqc-math-util-after-localized-formats-pack-r1` translates
  10/10 files with 0 translation failures, 0 invalid-pattern files, and 107
  generated-Rust errors. The previous 42 missing `LocalizedFormats` rows and
  the two invalid Java cast downcast rows are gone.
- Rust function parameter rendering now raw-ident escapes names after applying
  the existing mutable-parameter convention, so optional mutable Java array
  parameters named Rust keywords render consistently between the signature and
  body, e.g. `mut r#in` instead of invalid `mut in` plus `r#in` references.
  Verified with
  `mutable_optional_array_parameter_named_rust_keyword_uses_raw_binding`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 402 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC
  math-util rerun `.tmp/corpus-fastqc-math-util-after-keyword-param-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 91 generated-Rust errors. The previous 15 missing `r#in` rows and
  invalid `mut in` parse row are gone. Remaining top families are 58 mismatched
  types, 10 missing receiverless `fast_math_calc` references, 5 fallible calls
  in non-`Result` methods, plus smaller moved-value, ndarray/static-slice
  indexing, mutable receiver, `strict_math`, keyword, and scalar coercion rows.
- Same-source static-owner aliases are now seeded from primary/support/summary
  source type declarations, including parser-normalized snake-case aliases, and
  fallback overload names can refine `StaticOwner` call plans monotonically
  without degrading already-specific names such as `check_path` back to
  `check`. Verified with
  `same_source_static_owner_receiver_keeps_type_path_after_name_normalization`,
  `emitted_context_static_call_uses_solved_param_shape`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 403 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC
  math-util rerun `.tmp/corpus-fastqc-math-util-after-static-owner-overload-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 85 generated-Rust errors. The previous unresolved lowercase
  `fast_math_calc` owner rows are gone, and 6 of the 10 `FastMathCalc`
  overload-name rows now use the emitted suffixed overloads. The remaining 4
  `FastMathCalc::printarray` rows are the table-constant calls where static
  table field argument types are still too weak for overload disambiguation.
  Other top families are 58 mismatched types, 5 fallible calls in non-`Result`
  methods, plus smaller moved-value, ndarray/static-slice indexing, mutable
  receiver, `strict_math`, keyword, and scalar coercion rows.
- Nested static table aliases and source field facts now participate in static
  overload selection, so calls such as `FastMathCalc.printarray(...,
  ExpIntTable.EXP_INT_TABLE_A)` use the emitted suffixed overload instead of a
  generic static method name. Static associated array constants are also
  recognized as slice-backed during array-access lowering, so they keep nested
  slice indexing while ordinary mutable 2D arrays continue to lower to
  `ndarray::Array2`. Complex bitwise/shift index expressions now cast the whole
  expression to `usize`, e.g. `((bits & MASK) >> shift) as usize`, rather than
  casting only the shift operand. Verified with
  `nested_static_table_field_type_guides_static_overload_selection`,
  `nested_static_table_classes_get_simple_aliases_and_default_array_consts`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 404 passing tests,
  `cargo test --test type_coercion --quiet` with 86 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC
  math-util rerun `.tmp/corpus-fastqc-math-util-after-static-table-index-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 81 generated-Rust errors. The remaining `LN_MANT` row has moved from an
  index-cast problem to slice-to-`Vec` recovery (`clone()` on `&[f64]` should
  become `to_vec()`), and the `LN_MANT` 2D printarray call still needs a
  slice-backed 2D argument shape or array2 adapter policy.
- Borrowed sequence ownership recovery is now explicit in the shared coercion
  plan: a borrowed Java sequence recovered into an owned Java sequence emits
  `.to_vec()` instead of generic `.clone()`. This is the general shape behind
  static-table row reads such as `LnMant.LN_MANT[index]` assigned to
  `double[]`, and it gives future Rust signature/ABI facts a direct
  `&[T] -> Vec<T>` recovery path rather than another call-site rule. Verified
  with `borrowed_sequence_ownership_recovery_uses_to_vec`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 404 passing tests,
  `cargo test --test type_coercion --quiet` with 87 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC
  math-util rerun `.tmp/corpus-fastqc-math-util-after-sequence-to-vec-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 80 generated-Rust errors. The `LN_MANT` slice-to-`Vec` row is gone;
  remaining static-table work includes the 2D slice-vs-`Array2` printarray
  shape and source getters returning static slice constants as owned arrays.
- Primitive widening casts now parenthesize only genuinely whole-expression
  parentheses before appending `as`, so Java expressions such as
  `(value >> 12) / SCALE` render as `((value >> 12) as f64) / SCALE` instead of
  shifting by a floating-point distance, and parenthesized integer sums render
  as `(((a << n) + b) as f64) / SCALE` instead of casting only one child of the
  sum. Verified with
  `java_shift_expression_casts_as_whole_expression_when_promoted`,
  the widened-compound assertion in `type_coercion`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 405 passing tests,
  `cargo test --test type_coercion --quiet` with 87 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC
  math-util rerun `.tmp/corpus-fastqc-math-util-after-whole-widen-cast-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 80 generated-Rust errors. The previous `i64 >> f64` and `f64 + i64`
  Payne-Hanek rows are gone. One scalar `f64 * i64` row remains and should be
  handled as another expression-boundary numeric promotion case, not as a
  FastMath-specific rewrite.
- Optional array/index expression type inference now unwraps
  `Optional<Array<T>>` before deriving the indexed element type, so Java
  numeric promotion sees nullable sequence elements as their real element type
  instead of `Unknown`. This removes the remaining `f64 * i64` scalar
  promotion row for `hiPrec[0] * HEX_40000000` by casting the integer constant
  operand before assignment fallback. Verified with
  `type_inference_owns_optional_array_access_element_type`,
  `optional_array_element_participates_in_numeric_promotion`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 406 passing tests,
  `cargo test --test type_coercion --quiet` with 88 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC
  math-util rerun
  `.tmp/corpus-fastqc-math-util-after-optional-array-numeric-promotion-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 79 generated-Rust errors. The remaining top families are 57 mismatched
  types, 5 fallible calls in non-`Result` methods, two moved `hi_prec` rows,
  static slice/`Array2` shape drift, mutable receiver, missing `strict_math`,
  moved `x2`, type annotations, display support, duplicate `JavaPathArg`, and
  raw keyword support.
- Fresh array locals no longer adopt optional borrowed out-parameter storage
  merely because they are passed to a nullable source parameter. Array
  creation/initializer locals stay owned `Vec<T>` storage, and the call
  boundary wraps mutable slice arguments as `Some(values.as_mut_slice())`.
  Nullable mutable out-parameters that are passed onward and then reused now
  reborrow with `as_deref_mut()` instead of moving the `Option<&mut [T]>`.
  Verified with
  `local_array_argument_to_nullable_out_parameter_stays_owned_vec`,
  `nullable_out_parameter_reborrows_for_recursive_call_before_reuse`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 408 passing tests,
  `cargo test --test type_coercion --quiet` with 88 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC
  math-util rerun
  `.tmp/corpus-fastqc-math-util-after-nullable-out-param-reborrow-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 72 generated-Rust errors. The previous `Option<&mut Vec<f64>>` to
  `Option<&mut [f64]>` rows and moved `hi_prec` rows are gone. Remaining top
  families are 52 mismatched types, 5 fallible calls in non-`Result` methods,
  static slice/`Array2` shape drift, mutable receiver, missing `strict_math`,
  moved `x2`, type annotations, display support, duplicate `JavaPathArg`, and
  raw keyword support.
- Primitive numeric equality and inequality now use the same numeric promotion
  planner as arithmetic and ordered comparisons before falling back to generic
  equality lowering. This covers `double == 0`, `double != 0`, `double == long`,
  and `float == double` without FastMath-specific rewrites. Verified with
  `translates_java_numeric_relational_promotion_before_rust_comparison`,
  `cargo fmt --check`, `cargo check --quiet`,
  `cargo test --test parser_gaps --quiet` with 18 passing tests,
  `cargo test --test idiom_gaps --quiet` with 408 passing tests,
  `cargo test --test type_coercion --quiet` with 88 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC
  math-util rerun
  `.tmp/corpus-fastqc-math-util-after-numeric-equality-promotion-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 45 generated-Rust errors. The equality/coercion rows for raw integer
  comparisons against floating operands are gone, and mismatched-type rows drop
  from 52 to 25. Remaining top families are 25 mismatched types, 5 fallible
  calls in non-`Result` methods, static slice/`Array2` shape drift, mutable
  receiver, missing `strict_math`, moved `x2`, type annotations, display
  support, duplicate `JavaPathArg`, and raw keyword/support rows.
- Builtin numeric facade facts now type immutable `BigInteger` instance methods
  used by Apache Commons Math: `multiply(...)`, `pow(...)`, `shiftRight(...)`,
  and `testBit(...)`. The generated support methods now return `BigInteger` or
  `bool` instead of weak `()`, following the existing fact-backed numeric
  facade path rather than call-site rewrites. Verified with the expanded
  `jdk_decimal_and_integer_constants_are_typed_facade_facts`, `cargo fmt
  --check`, `cargo check --quiet`, `cargo test --test parser_gaps --quiet`
  with 18 passing tests, `cargo test --test idiom_gaps --quiet` with 408
  passing tests, `cargo test --test type_coercion --quiet` with 88 passing
  tests, and `cargo build --release --quiet --bin new-java2rust`. Focused
  FastQC math-util rerun
  `.tmp/corpus-fastqc-math-util-after-biginteger-method-facts-r1` translates
  10/10 files with 0 translation failures, 0 invalid-pattern files, and 38
  generated-Rust errors. The previous `BigInteger` unit-return rows are gone.
- Plain array assignment expressions in value position now preserve Java's
  assigned-value semantics by emitting an assignment block that reads the target
  back through the existing array-access planner. This covers chained writes
  such as `ys[0] = ys[1] = 0.0` without changing statement-side assignment
  lowering. Verified with
  `chained_array_assignment_expression_returns_assigned_value`, `cargo fmt
  --check`, `cargo check --quiet`, `cargo test --test idiom_gaps --quiet` with
  409 passing tests, `cargo test --test parser_gaps --quiet` with 18 passing
  tests, `cargo test --test type_coercion --quiet` with 88 passing tests, and
  `cargo build --release --quiet --bin new-java2rust`. Focused FastQC math-util
  rerun `.tmp/corpus-fastqc-math-util-after-chained-array-assign-r1`
  translates 10/10 files with 0 translation failures, 0 invalid-pattern files,
  and 41 generated-Rust errors. The assignment-expression unit rows are gone,
  and the latest top families expose the next source-array slot gap: read-only
  source array parameters still lower as owned `Option<Vec<T>>` instead of
  borrowed optional slices, producing repeated move rows for `xs` and `facts`.

Audit todo before and during this work:

- Treat improved automatic `&`/borrow inference as a signal to clean up older
  inference rules, not as an isolated addition. The likely drift risk is that a
  previous method/package/string-shape rule continues to fire after the shared
  source/facade ABI path can infer the same slot more accurately.
- Treat each new general inference mechanism as a trigger to audit previous
  inference rules for over-specialization. In particular, check whether older
  rules were compensating for missing ABI/slot facts and would now cause
  semantic drift if they remain preferred over the general path.
- Inventory specialized inference rules that currently insert borrows, clones,
  optional unwraps, type preservation, or facade argument handling from Java
  method names, rendered Rust type strings, package names, or stream-local
  guesses.
- For each rule, classify it as one of:
  - keep as Java semantic knowledge, because it describes Java behavior not
    visible in a Rust signature;
  - replace with Rust facade ABI facts inferred from `javalibs` or generated
    support signatures;
  - bridge temporarily through compatibility facts while older manifests are
    migrated;
  - delete once the shared slot planner covers the same behavior.
- Add regression tests for rules that are deleted or bridged so future
  call-site-specific inference does not creep back in.
- Add a targeted semantic-drift audit for old inference rules that prove too
  specialized after general source/facade ABI facts land. These rules should not
  stay silently preferred over the shared planner; classify them as lowered
  precedence, explicit compatibility facts, or deletion targets.
- Re-run focused corpuses after each retirement pass, because making ABI facts
  authoritative can expose older specialized rules that were previously masking
  semantic drift.
- Latest audit example: source receiver call return-shape lowering must prefer
  emitted source `SignaturePlan` ABI for primary source methods, but must keep
  source-summary nullability authoritative for summary-only owners. A blanket
  source-summary-first or signature-first rule causes semantic drift in one of
  those two cases.
- Latest name-resolution audit example: bare local receivers must shadow
  same-named fields and outer fields when deciding whether to add optional
  receiver adapters. Otherwise a concrete constructor local such as
  `ZipFile zipFile = new ZipFile(file)` can incorrectly inherit the nullable
  shape of an instance field named `zipFile`.

## Goal

Make the focused Tier 1/Tier 2 corpus checks useful again by fixing the
remaining generated-Rust blockers before running larger scheduled corpuses.

Success for this slice means:

- `cargo fmt --check`, `cargo check --quiet`, and
  `cargo test --test idiom_gaps --quiet` stay green.
- `picard-programgroups` remains clean.
- `htsjdk-fastq` typechecks.
- `fastqc` no longer panics in signature solving.
- focused generated crates that emit `ndarray::` can be checked offline without
  a crates.io dependency-cache failure.
- `docs/current-corpus-issue-audit.md` is updated after the rerun.

## Order Of Work

### 1. Fix FASTQ Optional Getter Shape Drift

Status: done.

Evidence before fix:

- `.tmp/corpus-batch-current-htsjdk-fastq-final5`
- Generated Rust still contains `Some(record.get_read_name())`,
  `Some(record.get_read_string())`, `Some(record.get_base_quality_header())`,
  and `Some(record.get_base_quality_string())`.
- Those getters already return `Option<String>`.

Plan:

1. Add a minimized fixture from `FastqEncoder.write`.
2. Assert the local variable initializers are direct optional values, not
   `Some(Option<T>)`.
3. Trace which lowering branch emits the wrapping. The attempted external-call
   guard did not affect corpus output, so inspect var-decl initializer lowering,
   `lower_expr_for_slot_with_refinements`, and any call-specific fast paths that
   run before `planned_method_call_return_shape`.
4. Fix at the planner/shape level, not by rendered-string surgery.
5. Rerun:
   - `cargo test <new fastq fixture test> --test idiom_gaps --quiet`
   - `cargo test --test idiom_gaps --quiet`
   - focused `htsjdk-fastq`

Acceptance:

- No generated `Some(record.get_...())` when `record.get_...()` already returns
  `Option<T>`.

Verified:

- `cargo test source_nullable_getter_local_initializer_stays_optional --test idiom_gaps --quiet`
- `cargo test --test idiom_gaps --quiet`
- focused `htsjdk-fastq` in
  `.tmp/corpus-batch-current-htsjdk-fastq-after-signature-field-shape`

### 2. Fix FASTQ Nullable Copy Constructor Field Access

Status: done.

Evidence before fix:

- FASTQ copy constructor emits field reads like `other.readName` where `other`
  is `Option<FastqRecord>`.

Plan:

1. Add a minimized copy-constructor fixture:
   - constructor parameter is nullable;
   - null guard throws;
   - later field copies read from the now-proven non-null parameter.
2. Teach refinements from early null-throw guards to apply in constructor field
   initializers, or route copy-constructor field assignments through the same
   non-null refinement machinery used in method bodies.
3. Prefer preserving Java semantics:
   - guarded nullable input becomes non-null after the throw branch;
   - unguarded nullable input still requires explicit unwrap or optional field
     handling.

Acceptance:

- Copy constructors do not emit direct field access on `Option<T>`.
- The fixture distinguishes guarded from unguarded nullable input.
- Optional source fields copied through nullable receivers are not wrapped as
  `Some(Option<T>)`.

Verified:

- `cargo test guarded_nullable_copy_constructor_reads_source_fields_after_unwrap --test idiom_gaps --quiet`
- `cargo test guarded_nullable_copy_constructor_preserves_optional_source_fields --test idiom_gaps --quiet`
- `cargo test sam_tag_enum_variant_constants_emit_typed_facade_surfaces --test idiom_gaps --quiet`
- `cargo test --test idiom_gaps --quiet` with 389 passing tests
- focused `htsjdk-fastq` in
  `.tmp/corpus-batch-current-htsjdk-fastq-after-optional-copy-samtag`

### 3. Isolate And Fix FastQC Signature Solver Panic

Evidence:

- `.tmp/corpus-batch-current-fastqc`
- Panic:
  - `signature solver must plan every lowered method`
  - `signature solver must plan every lowered field`

Plan:

1. Rerun FastQC with smaller subsets until the panic file is known.
2. Add the smallest fixture that reproduces the unplanned lowered method/field.
3. Fix the signature solver invariant so every lowered member has either:
   - a real method/field signature plan;
   - an explicit unsupported diagnostic path;
   - or a deliberate generated facade/support member plan.
4. Avoid weakening the invariant globally. A panic here usually means an earlier
   ownership/representation fact was skipped.

Acceptance:

- FastQC focused run completes to a normal summary instead of panicking.
- Any remaining FastQC failures are generated Rust errors or invalid-pattern
  rows, not translator crashes.

Isolation notes:

- Previous incomplete emitters were
  `test/unit/Modules/BasicStatsTest.java` and
  `uk/ac/babraham/FastQC/Statistics/NormalDistribution.java`.
- Single-file scratch primaries for those files do not reproduce the
  signature-solver panic; they lower to ordinary generated-Rust errors without
  the full source context.
- The minimized repro is duplicate simple owner names across packages:
  - `test/ui/Modules/BasicStatsTest.java` plus
    `test/unit/Modules/BasicStatsTest.java` reproduced the missing-method
    invariant for `BasicStatsTest::test_format_length_dot`.
  - `org/apache/commons/math3/distribution/NormalDistribution.java` plus
    `uk/ac/babraham/FastQC/Statistics/NormalDistribution.java` reproduced the
    missing-field invariant for `NormalDistribution::stdev`.
- Fixed batch source merging so same-simple-name classes are not discarded, and
  added a narrow signature fallback for the unique package-qualified duplicate
  owner. Minimized duplicate-owner subsets now complete to normal generated
  Rust errors instead of panicking:
  - `.tmp/corpus-fastqc-subset-duplicate-basicstats-test-after-key-fix`
  - `.tmp/corpus-fastqc-subset-duplicate-normal-distribution-after-key-fix`
- A fresh full FastQC rerun in
  `.tmp/corpus-batch-current-fastqc-after-duplicate-key-fix` was stopped while
  still in global parse/emit setup. The next step is either inner phase
  reporting for batch translation or a package-partitioned FastQC rerun that
  reaches emission quickly enough to prove the full corpus no longer panics.
- Inner batch and signature-solver phase reporting is implemented and covered by
  `cargo test --test batch_translation --quiet`. Full FastQC evidence from
  `.tmp/corpus-batch-current-fastqc-after-live-progress-safe-solver` shows:
  - semantic analysis completes in about 29.6s;
  - signature constraint collection completes in about 6.2s;
  - dependency graph, SCC analysis, and pass planning complete in under 1s;
  - signature fixpoint reaches pass 4 before the 900s timeout, with pass 1 at
    about 263s, pass 2 at about 520s, and pass 3 at about 782s;
  - no duplicate-owner signature panic occurs before the timeout.
- A worklist signature-solver experiment was rejected and removed because it
  changed existing semantics. The failing fixture was
  `reused_optional_borrowed_source_argument_wraps_by_reference`, where a
  nullable callee parameter stopped propagating back to the caller parameter.
  Any future worklist/indexed solver must have equivalence tests against the
  current saturation semantics.
- The accepted runtime fix is narrower: scoped propagation arms now return
  immediately when the active SCC has no slot kind that the arm can mutate.
  This preserves SCC-local saturation semantics and avoids repeated full usage
  scans for impossible target components. Representative evidence:
- VarScan `.tmp/global-analysis-profile-varscan-scope-kind-skip` drops
  signature fixpoint to about 5.4s from the scoped-mutation baseline around
  10.1s, with generated Rust matching the immediately preceding prefilter
  run.
  - JAligner `.tmp/global-analysis-profile-jaligner-scope-kind-skip` drops
    signature fixpoint to about 1.5s from the scoped-mutation baseline around
    2.8s.
  - FastQC `.tmp/corpus-batch-current-fastqc-after-scope-kind-skip` reaches
    `batch_translate_end` in about 719.7s with 156/156 files translated and no
    duplicate-owner panic. Signature fixpoint completes in about 622.3s across
    5 iterations, compared with the previous run timing out in pass 4.

Current status:

- The duplicate-owner panic is fixed for minimized repros.
- Full FastQC translation reaches emission, crate assembly, cargo, invalid
  scan, and idiom sweep in
  `.tmp/corpus-batch-current-fastqc-after-scope-kind-skip-long`.
- The longer run translated 156/156 files with 0 translation failures and no
  duplicate-owner panic. Signature fixpoint completed in 573014 ms, crate
  assembly in 203601 ms, and total wrapper time was 883330 ms.
- FastQC invalid-pattern scan reports 45 affected files:
  - 88 `compile_error!` placeholders across 36 files;
  - 40 Java cast downcasts across 14 files;
  - 10 `.put` calls across 5 files;
  - 1 concrete `None` assignment.
- The next action is to fixture and fix the top invalid-pattern families. The
  `.put` family, one generic-erased source `super(...)` subfamily, the
  closed-enum-to-interface coercion subfamily, and the nested Java
  interface-name rendering bug are now fixed locally. Fresh FastQC evidence in
  `.tmp/corpus-batch-current-fastqc-after-nested-interface-names` translates
  156/156 files, removes `.put` from the invalid-pattern list, reduces
  invalid-pattern files from 45 to 30, and lets offline cargo reach generated
  Rust diagnostics instead of the nested-name parse blocker.
- Latest FastQC invalid-pattern scan:
  - 58 `compile_error!` placeholders across 24 files;
  - 40 Java cast downcasts across 14 files;
  - 0 `.put` calls;
  - 1 concrete `None` assignment.
- Latest FastQC cargo check fails as generated Rust with 3236 errors. The next
  top parse/type blockers include keyword receivers such as `in::read`, Java
  block-comment artifacts in conditions, missing cast/comparison parentheses,
  remaining `super(...)` placeholders, interface stubs, optional array
  indexing/length, and facade/static constant gaps.
- Current local gate is green after the keyword receiver, cast-comparison, and
  numeric literal parser slice:
  - `cargo fmt --check`
  - `cargo check --quiet`
  - `cargo test --test parser_gaps --quiet`
  - `cargo test --test idiom_gaps --quiet` with 311 passing tests
  - `cargo test --test batch_translation --quiet`
  - `cargo test --test type_coercion --quiet`
  - `cargo build --release --quiet --bin new-java2rust`
- Partial FastQC rerun
  `.tmp/corpus-batch-current-fastqc-after-keyword-cast-literals` completed
  translation and crate assembly: 156/156 translated, 0 failed files, global
  analysis 614130 ms, signature fixpoint 577545 ms, emission 66962 ms, crate
  assembly 207014 ms. The wrapper was interrupted after crate assembly
  during/after cargo reporting, so no final `summary.tsv` exists.
- Targeted scans on that partial rerun show the `in::read` static-owner blocker
  and raw `as i32 <` cast-comparison blocker are gone. Remaining immediate
  parse blockers included one Java block-comment condition artifact, two Java
  hex floating literals, and 24 Java `0x8000000000000000L` long-mask spellings.
  These now have local/focused fixes:
  - `parses_parenthesized_condition_after_block_comment`;
  - `parses_hex_long_sign_bit_as_twos_complement_literal`;
  - `parses_java_hex_floating_point_literals`;
  - focused Base64 translation emits `self.position >= self.num_sig_bytes`;
  - focused FastMath/BitsStreamGenerator translations contain zero Java hex
    long suffixes and zero Java hex floating suffixes.
- Fresh FastQC rerun
  `.tmp/corpus-batch-current-fastqc-after-comment-hex-literals` completed
  translation for 156/156 files with 0 failed files and no translator panic.
  The wrapper hung before final summary/compile reporting, but generated-source
  scans prove the targeted parse blockers are gone across FastQC:
  - 0 `/*!` block-comment condition artifacts;
  - 0 Java hex floating literal suffixes;
  - 0 Java hex long suffix masks;
  - 0 `in::read` static-owner renderings;
  - 0 raw `as i32 <` cast-comparison renderings.
  Invalid-pattern counts remain 58 `compile_error!` placeholders across 24
  files, 40 Java cast downcasts across 14 files, 0 `.put` calls, and 30
  invalid-pattern files total.
- JDK stream-wrapper super constructors now model `GZIPInputStream`,
  `FilterInputStream`, and `FilterOutputStream` as no-state external
  superclasses when trailing constructor statements are already supported.
  Verified locally with
  `jdk_stream_wrapper_super_constructors_are_no_state_facades`. The remaining
  private FastQC `MultiMemberGZIPInputStream(parent)` constructors are now
  classified as the more precise object-graph/self-linking gap rather than the
  broad inheritance-lowering gap:
  - fixture:
    `jdk_stream_wrapper_super_with_self_linking_trailing_mutation_is_precise_gap`;
  - focused file:
    `/tmp/new_java2rust_multimember_gzip_check.rs`;
  - scan result: 2
    `unsupported Java constructor: super(...) followed by unsupported object graph mutation`
    diagnostics and 0
    `unsupported Java constructor: super(...) requires inheritance lowering`
    diagnostics for `MultiMemberGZIPInputStream`.
  This should not be lowered as normal Rust ownership until the translator has
  an object identity/handle model for `this.parent.child = this`.
  Focused Base64 translation has 0 stream-super placeholders, and focused
  MultiMemberGZIPInputStream translation clears the public `super(input, ...)`
  constructors.
- Java `String.join(delimiter, value...)` varargs now lower through the
  existing native `StringJoin` hook by packing the values into a `Vec<String>`
  and calling `.join(delimiter)`. Verified with
  `java_string_join_varargs_lowers_to_vec_join`,
  `java_string_join_static_call_renders_collection_join`, and focused FastQC
  `test/integration/cli/Cli.java` translation in
  `/tmp/new_java2rust_fastqc_cli_join_check.rs`, where the classpath join now
  emits
  `vec!["bin".to_string(), ".".to_string(), "jbzip2-0.9.jar".to_string(), "cisd-jhdf5.jar".to_string(), "htsjdk.jar".to_string()].join(sep.as_str())`
  and the old `String.join varargs are not supported yet` placeholder is gone.
- Source-backed `super(...)` flattening now preserves supported trailing
  expression statements such as method-result calls after superclass
  construction. Verified with
  `source_super_constructor_preserves_trailing_method_result_call`. A focused
  FastQC Commons-Math exception subset in
  `.tmp/corpus-fastqc-exception-subset-after-trailing-expr` translates 9/9 and
  shows `TooManyEvaluationsException` now emits
  `__java_self.get_context().add_message(...)` instead of a broad
  `super(...) requires inheritance lowering` placeholder. The same subset still
  has 2 broad `super(...)` placeholders in the base exception classes, now
  isolated to the next constructor subfamily: source superclass constructors
  that delegate through `this(...)` and constructors that call external
  standard exception superclasses such as `super(cause)`.
- Standard Java exception superclass calls with supported trailing subclass
  work now lower as no-state external exception construction. Bare constructor
  field assignments such as `context = ...` and `argument = ...` are also
  recognized as constructor field initialization, and source-super lookup now
  matches varargs superclass constructors by fixed arity. Verified with
  `external_standard_exception_super_with_trailing_body_lowers_subclass_state`
  and `source_super_constructor_matches_varargs_super_arity`. The focused
  FastQC exception subset in
  `.tmp/corpus-fastqc-exception-subset-after-varargs-super-matching` still
  fails as generated Rust, but broad
  `unsupported Java constructor: super(...) requires inheritance lowering`
  placeholders are now 0 in the subset. Invalid patterns shift to 2
  `instanceof` placeholders in `ExceptionContext` and 6 downcast rows across 4
  files, including the remaining object-model issue where `this` is passed
  through a superclass context constructor.
- JDK `Serializable` marker checks now lower through an explicit
  `java_is_serializable(...)` runtime helper instead of the generic erased
  object `instanceof` fallback. Verified with
  `serializable_instanceof_erased_object_uses_runtime_marker_helper`. The
  focused FastQC exception subset in
  `.tmp/corpus-fastqc-exception-subset-after-serializable-helper-import`
  translates 9/9 files, has 0 `compile_error!` placeholders, 0 unsupported
  array-creation placeholders, and still has 6 invalid Java downcast rows
  across 4 files. Generated cargo errors move from 51 to 49; the remaining
  invalid rows are now all object-model/downcast issues.
- Current self passed to a known Java exception supertype slot, such as
  `new ExceptionContext(this)` where the constructor expects `Throwable`, now
  lowers as an explicit facade boundary instead of an invalid erased-object
  downcast. `HashMap<String, Object>.put(key, value)` also keeps Java `Object`
  values as erased-object carriers instead of downcasting to a synthetic
  `Object` facade. Verified with
  `exception_self_as_throwable_context_argument_uses_facade_boundary` and
  `object_map_put_keeps_erased_object_value_without_downcast`. The focused
  FastQC exception subset in
  `.tmp/corpus-fastqc-exception-subset-after-object-map-erasure` translates
  9/9 files with 0 invalid-pattern files: 0 `compile_error!` placeholders, 0
  invalid Java cast downcasts, 0 `.put` calls, and 0 unsupported array-creation
  placeholders. Generated cargo still fails with 43 errors, now dominated by
  ordinary generated-Rust type/interface issues such as `ExceptionContext`
  cross-module visibility, `Localizable` trait recovery, `StringBuilder`
  surface gaps, missing constants, and collection/string method gaps.

Additional progress:

- Source-backed `super(args)` constructors now lower when the direct source
  superclass constructor is available and its body can be represented as simple
  field initialization plus supported trailing statements. The superclass
  constructor parameter values are substituted from the child `super(...)`
  arguments into the flattened child struct fields.
- Source-backed `super(args)` constructor lookup also resolves generic-erased
  superclass spellings, e.g. `extends BaseSolver<FUNC>`, to a unique source
  declaration before flattening inherited fields.
- JDK `FileReader` superclass constructors are modeled as no-state external
  superclass construction for generated corpus checks.
- Closed enum values now borrow as dyn-trait parameters when hierarchy facts
  prove that the enum implements the target interface. This covers
  `LocalizedFormats` values passed to `Localizable` slots while keeping
  unrelated closed-enum/trait coercions unsupported.
- Nested Java interface/type names now render as valid Rust identifiers in
  trait items, dyn-trait slot types, and overload suffixes, covering
  `Incrementor.MaxCountExceededCallback`.
- Source interface collection storage and indexed access now use boxed dyn
  trait carriers through the shared slot conversion plan. `List<Localizable>`
  renders as `Vec<Box<dyn Localizable>>`, `add(pattern)` lowers through
  `pattern.clone_box()`, generated source traits include the `as_any`/`clone_box`
  ABI, and indexed `get(i)` clones the boxed trait object instead of boxing
  `&Box<dyn Localizable>`.
- Verified with:
  - `source_super_constructor_with_args_initializes_flattened_inherited_fields`
  - `generic_source_super_constructor_initializes_flattened_inherited_fields`
  - `jdk_file_reader_super_constructor_is_no_state_facade`
  - `closed_enum_variant_borrows_as_dyn_trait_parameter`
  - `nested_interface_type_names_are_valid_rust_identifiers`
  - `source_interface_collection_elements_use_boxed_dyn_trait`
  - `cargo test super_constructor --test idiom_gaps --quiet`
- Focused FastQC exception-subset rerun:
  `.tmp/corpus-fastqc-exception-subset-after-indexed-localizable-box`
  translates 9/9 files, reports 0 invalid-pattern files, and reduces generated
  cargo errors from the current 19-error baseline to 15. The next dominant
  family is erased `Object` carrier policy: `Box<dyn Any>` /
  `Vec<Box<dyn Any>>` clone-display-serialization and deferred
  `ObjectInputStream.readObject()` typing.
- VarScan rerun:
  `.tmp/corpus-batch-current-varscan-after-jdk-file-reader-super` translates
  18/18, reports 0 invalid-pattern files, and moves the prior
  `SmartFileReader` `super(...)` placeholders out of the invalid-pattern list.
  Offline `cargo check` still fails as `generated_rust` with 344 errors.
- Raw Rust identifier emission now covers Java-derived identifiers that collide
  with Rust keywords in ordinary emitted identifier positions. The scanner also
  recognizes raw optional locals as optional storage. Verified with
  `rust_keyword_local_names_render_as_raw_identifiers` and
  `cargo test --test idiom_gaps --quiet`.
- Latest VarScan rerun:
  `.tmp/corpus-batch-current-varscan-after-rust-keyword-ident2` translates
  18/18, reports 0 invalid-pattern files, and has no keyword parse errors for
  Java local name `in`. Offline `cargo check` still fails as `generated_rust`
  with 369 errors, now concentrated in type/facade families such as mismatched
  types, temporary-reference returns, `Err` namespace collisions, missing
  `Out`/`System.err`/`OutputStream`, Java exception/string methods, and index
  coercions.
- Output and exception logging progress:
  - `System.out`/`System.err` statement-side `print`/`println` now lowers to
    Rust output macros instead of unresolved `Out::`/`Err::` pseudo-types.
  - Nonzero-argument `PrintStream` support methods use overload-safe Rust names.
  - `JavaException` runtime support includes `get_localized_message`, the
    focused wrapper emits the same method, and statement-side `printStackTrace`
    lowers to an explicit no-op.
  - Verified with
    `system_out_and_err_print_stream_statements_lower_to_rust_macros`,
    `java_exception_logging_surface_compiles_without_system_err_value`, and
    `cargo test --test idiom_gaps --quiet`.
- Latest VarScan rerun after output/exception logging:
  `.tmp/corpus-batch-current-varscan-after-exception-runtime-wrapper` translates
  18/18, reports 0 invalid-pattern files, and offline `cargo check` fails as
  `generated_rust` with 267 errors. The prior `Out::`/`Err::`,
  `System.err.println`, and `printStackTrace(System.err)` failures are gone.

### 4. Minimize VCF Shared-Support Enum Assembly

Evidence:

- `.tmp/corpus-batch-current-htsjdk-vcf`
- Generated crate has an unclosed delimiter around shared support enum/display
  assembly.

Plan:

1. Minimize from `VCFCompoundHeaderLineSupportedHeaderLineType`.
2. Add a fixture for nested/shared enum support generation with `Display`,
   `Hash`, and imported static references if needed.
3. Fix item assembly so generated impl blocks are balanced even when support
   types are emitted from shared context.

Acceptance:

- VCF gets past parsing/generated-crate syntax and reaches ordinary `cargo
  check` diagnostics.

### 5. Fix Spark/DISQ Interface Bridge Shape

Evidence:

- `.tmp/corpus-batch-current-gatk-disq-spark-datasources`
- Missing `Reference*SparkSource.getBases` implementations and borrowed/owned
  delegate mismatches.

Plan:

1. Add a small fixture for a Java interface with an implementation delegating to
   a field-backed source.
2. Include a method returning bases/sequence-like data so slot ownership is
   exercised.
3. Ensure trait impl generation uses the same method signature plan as the
   source method, including borrowed/owned argument slots.
4. Keep Spark as a noop facade boundary; do not model distributed execution in
   this translator.

Acceptance:

- The fixture emits a complete trait impl without missing methods.
- Spark/DISQ focused invalid-pattern count decreases or generated Rust errors
  move to a different family.

### 6. GenomicsDB Overload And Handoff Cleanup

Evidence:

- `.tmp/corpus-batch-current-gatk-genomicsdb`
- Duplicate `query` definitions and one mismatched type.

Plan:

1. Minimize the duplicate `query` overload into an overload-naming fixture.
2. Confirm whether the mismatched type is Java-side translation or FFI handoff
   boundary shape.
3. Keep GenomicsDB out of `javalibs` as a semantic facade. The Java translator
   should only lower calls to handover state/signatures produced by the future
   C/C++ transpiler.

Acceptance:

- Duplicate Rust item names are eliminated for the minimized overload.
- GenomicsDB focused errors drop below the current 3 generated Rust errors.

### 7. Unblock Ndarray-Dependent Corpus Typechecks

Evidence:

- BioJava, JAligner, VarScan, JAHMM, and Trimmomatic reach generated crates but
  cannot typecheck because `ndarray` is not available in the current cache.

Plan:

1. Generate a local path dependency shim named `ndarray` for focused generated
   crates that contain `ndarray::`.
2. Keep the shim narrow: it only needs the surface currently emitted by the
   translator (`Array2`, `arr2`, `row`, `row_mut().assign()`, and
   `ArrayView1::from()`).
3. Rerun:
   - `biojava-aaprop`
   - `biojava-alignment`
   - `jaligner`
   - `varscan`
   - `jahmm`
   - `trimmomatic`

Acceptance:

- Dependency-cache failures are replaced by generated Rust diagnostics or clean
  checks.

Current status:

- Done for the wrapper dependency gate. `scripts/translate-focused-package.sh`
  now emits `ndarray = { path = "ndarray-shim" }` when generated code uses
  `ndarray::`.
- Verified by
  `cargo test focused_package_offline_cargo_uses_local_ndarray_shim --test batch_translation --quiet`.
- `varscan` rerun:
  `.tmp/corpus-batch-current-varscan-after-jdk-file-reader-super` translates
  18/18, reports 0 invalid-pattern files, then offline `cargo check` fails as
  `generated_rust` with 344 errors instead of `dependency_cache`.
- `jaligner` rerun:
  `.tmp/corpus-batch-current-jaligner-after-ndarray-shim` translates 29/29,
  then offline `cargo check` fails as `generated_rust` with 629 errors instead
  of `dependency_cache`.
- BioJava AAPROP rerun:
  `.tmp/corpus-batch-current-biojava-aaprop-after-ndarray-shim` translates
  33/33, then offline `cargo check` fails as `generated_rust` with 740 errors.
- BioJava alignment rerun:
  `.tmp/corpus-batch-current-biojava-alignment-after-ndarray-shim` translates
  53/53, then offline `cargo check` fails as `generated_rust` with 188 errors.
- JAHMM rerun:
  `.tmp/corpus-batch-current-jahmm-after-ndarray-shim` translates 102/102,
  then offline `cargo check` fails as `generated_rust` with 120 errors.
- Trimmomatic rerun:
  `.tmp/corpus-batch-current-trimmomatic-after-ndarray-shim` translates 68/68,
  then offline `cargo check` fails as `generated_rust` with 856 errors.
- Later VarScan progress:
  - `System.out`/`System.err` statement output and Java exception logging
    lowered away the old `Out::`/`Err::`, `System.err.println`, and
    `printStackTrace(System.err)` generated-Rust families.
  - Java string API normalization now covers `toUpperCase`, `toCharArray`,
    literal `replace`, `replaceFirst`, `replaceAll`, and `matches`, with
    shared runtime helpers also injected by `scripts/translate-focused-package.sh`.
  - Verified by
    `cargo test java_string_uppercase_char_array_and_replace_use_rust_string_apis --test idiom_gaps --quiet`,
    `cargo test java_string_regex_helpers_use_runtime_surfaces --test idiom_gaps --quiet`,
    `cargo test --test idiom_gaps --quiet` with 280 passing tests,
    `cargo test --test batch_translation --quiet`, `cargo check --quiet`,
    `cargo fmt --check`, `bash -n scripts/translate-focused-package.sh`, and
    `cargo build --release --quiet --bin new-java2rust`.
  - Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-string-regex-support-wrapper`
    translates 18/18, reports 0 invalid-pattern files, and fails offline
    `cargo check` as `generated_rust` with 229 errors. The prior
    `to_upper_case`, `to_char_array`, `String: Pattern`, `replace_first`, and
    `replace_all` rows are gone.
- Constructed string return and Java string equality progress:
  - Return-slot planning now keeps locally constructed Java `String` values
    owned unless a method exactly returns a borrow-safe parameter alias. This
    removes generated `format!(...).as_str()` and `"".to_string().as_str()`
    return sites from VarScan.
  - Java string equality now compares non-optional string slots through string
    views, including borrowed parameters and `Vec<String>` index expressions.
  - Verified with
    `constructed_string_return_stays_owned_not_borrowed_alias`,
    `java_string_equality_normalizes_owned_and_borrowed_slots`,
    `call_return_shape_drives_return_slot_coercion`,
    `signature_fixpoint_propagates_private_internal_call_return_shape`,
    `cargo test --test idiom_gaps --quiet` with 282 passing tests,
    `cargo test --test batch_translation --quiet`, `cargo check --quiet`,
    `cargo fmt --check`, `bash -n scripts/translate-focused-package.sh`, and
    `cargo build --release --quiet --bin new-java2rust`.
  - Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-string-equality-all-strings`
    translates 18/18, reports 0 invalid-pattern files, and fails offline
    `cargo check` as `generated_rust` with 151 errors. The temporary-reference
    return and `String`/`&String` comparison rows are gone.
  - JDK facade batch progress:
    `Math.log`, `Math.exp`, `Double.isNaN`, `DecimalFormat.format`,
    `File.exists`, reader `ready`, `System.exit`, and `BitSet.get` now plan
    through explicit native JDK call targets and return facts. Verified with
    the focused JDK fixtures, `cargo test --test idiom_gaps --quiet` with 285
    passing tests, `cargo test --test batch_translation --quiet`,
    `cargo check --quiet`, `cargo fmt --check`, and
    `cargo build --release --quiet --bin new-java2rust`. Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-jdk-facades-bitset` translates
    18/18, reports 0 invalid-pattern files, and fails offline `cargo check` as
    `generated_rust` with 119 errors. The JDK numeric/display/static-arity,
    file-exists, reader-ready, system-exit, and BitSet boolean-void rows are
    gone.
  - Collection `toArray` progress:
    direct `List.toArray(new String[]{})` and casted
    `map.keySet().toArray(new String[0])` now lower through native collection
    conversion to owned `Vec<_>` rather than a missing `.to_array(...)` method.
    Verified with `collection_to_array_empty_string_array_lowers_to_owned_vec`,
    `cargo test --test idiom_gaps --quiet` with 285 passing tests,
    `cargo test --test batch_translation --quiet`, `cargo check --quiet`,
    `cargo fmt --check`, and
    `cargo build --release --quiet --bin new-java2rust`. Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-keyset-to-array-cast` translates
    18/18, reports 0 invalid-pattern files, and fails offline `cargo check` as
    `generated_rust` with 111 errors. The `to_array` rows are gone.
  - Compound index-cast progress:
    Java integer arithmetic used as a Rust array index now casts the whole
    expression to `usize`, e.g. `(a + b) as usize`, instead of casting only the
    right-hand operand. Verified with
    `java_int_arithmetic_array_index_casts_whole_expression_to_usize`,
    `cargo test --test idiom_gaps --quiet` with 287 passing tests,
    `cargo test --test type_coercion --quiet`,
    `cargo test --test batch_translation --quiet`, `cargo check --quiet`,
    `cargo fmt --check`, and
    `cargo build --release --quiet --bin new-java2rust`. Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-compound-index-cast` translates
    18/18, reports 0 invalid-pattern files, and fails offline `cargo check` as
    `generated_rust` with 84 errors. The `i32 + usize`, `i32 - usize`, and
    indexed-by-`i32` rows are gone.
  - Primitive map-get arithmetic and assignment/update expression progress:
    primitive-valued `Map.get` results in arithmetic now unwrap/copy optional
    borrowed values before numeric lowering, Java assignment expressions in
    value position return the assigned local value, and prefix update arguments
    such as `++a`/`--b` lower to block expressions that return the updated
    value. Verified with
    `map_get_primitive_value_in_arithmetic_unboxes_optional_borrow`,
    `compound_assignment_expression_argument_returns_assigned_value`,
    `prefix_update_expression_argument_returns_updated_value`,
    `buffered_reader_read_line_assignment_condition_wraps_result_value`,
    `cargo test --test idiom_gaps --quiet` with 291 passing tests,
    `cargo test --test type_coercion --quiet`,
    `cargo test --test batch_translation --quiet`, `cargo check --quiet`,
    `cargo fmt --check`, and
    `cargo build --release --quiet --bin new-java2rust`. Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-readline-assignment-condition` translates
    18/18, reports 0 invalid-pattern files, and fails offline `cargo check` as
    `generated_rust` with 47 errors. The option arithmetic, prefix update
    argument, immutable parameter reassignment, and fallible `readLine`
    assignment-condition rows from this family are gone.
  - Inline `ndarray` row replacement and remaining JDK system/thread/output
    progress: assigning an inline Java array to a 2D row now borrows the
    generated vector through `ArrayView1::from(&vec![...])`; `Thread.sleep`
    lowers to `std::thread::sleep(std::time::Duration::from_millis(...))`;
    lowercase parsed `thread` resolves to the `Thread` facade identity;
    `System.in` lowers to `System::r#in` with generated `InputStream` support;
    and typed constructor argument support now emits facade types referenced
    only through constructor argument facts, covering `PrintStream(OutputStream)`.
    Verified with
    `two_dimensional_row_replacement_lowers_to_ndarray_row_mut_assign`,
    `system_in_static_field_and_thread_sleep_use_jdk_facades`,
    `print_stream_output_stream_constructor_has_typed_support_arg`,
    `cargo test --test idiom_gaps --quiet` with 292 passing tests,
    `cargo test --test type_coercion --quiet`,
    `cargo test --test batch_translation --quiet`, `cargo check --quiet`,
    `cargo fmt --check`, and
    `cargo build --release --quiet --bin new-java2rust`. Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-jdk-system-thread-outputstream`
    translates 18/18, reports 0 invalid-pattern files, and fails offline
    `cargo check` as `generated_rust` with 29 errors. The ndarray row
    replacement input, unresolved `thread`, lower-case `system.in`, and missing
    `OutputStream` rows are gone.
  - Optional `Map.get` display progress: `Map.get(...)` values used in
    display/string-concat slots now lower through the optional carrier with
    `map(|value| value.to_string()).unwrap_or_else(|| "null".to_string())`
    instead of passing `Option<&String>` directly to `format!`. Verified with
    `map_get_string_display_segment_unwraps_optional_borrow`,
    `cargo test --test idiom_gaps --quiet` with 293 passing tests,
    `cargo test --test type_coercion --quiet`,
    `cargo test --test batch_translation --quiet`, `cargo check --quiet`,
    `cargo fmt --check`, and
    `cargo build --release --quiet --bin new-java2rust`. Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-map-get-display` translates 18/18,
    reports 0 invalid-pattern files, and fails offline `cargo check` as
    `generated_rust` with 27 errors. The `Option<&String>` display bucket is
    gone.
  - Reused owned-string and numeric/display progress: reusable owned Java
    `String` locals now clone at repeated owned consuming slots, including
    vector initializers, local declarations, assignments, and later reads.
    Explicit primitive numeric casts now survive binary lowering, native
    formatter/string-concat output no longer inserts `.to_string()` inside
    arithmetic operands, and primitive array fill assertions now accept typed
    fill literals. Verified with
    `reused_string_parameters_clone_in_multiple_vec_initializers`,
    `reused_string_source_clones_for_multiple_owned_local_assignments`,
    `explicit_primitive_casts_survive_numeric_binary_lowering`,
    `decimal_format_argument_keeps_numeric_expression_before_string_concat`,
    `cargo test --test idiom_gaps --quiet` with 297 passing tests,
    `cargo test --test type_coercion --quiet`,
    `cargo test --test batch_translation --quiet`, `cargo check --quiet`,
    `cargo fmt --check`, and
    `cargo build --release --quiet --bin new-java2rust`. Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-numeric-format-fixes` translates
    18/18, reports 0 invalid-pattern files, and fails offline `cargo check` as
    `generated_rust` with 10 errors. The moved-string and mixed numeric/display
    buckets are gone.
  - VarScan generated-Rust cleanup: statement-wrapper value returns inside
    fallible try/catch wrappers now use `Result<Option<T>, JavaException>`, raw
    Java collection returns refine to a concrete generic collection type when
    every explicit return returns the same typed local, statement-side non-void
    `super(...)` boundary calls lower to an explicit typed no-op propagation,
    JDK `File` one-argument constructors have a typed `String` argument slot so
    indexed borrowed `String[]` values clone instead of moving, and reassigned
    Java `String` enhanced-for variables iterate owned cloned elements with a
    mutable loop binding. Verified with
    `plain_fallthrough_try_with_value_returns_uses_optional_result_carrier`,
    `raw_map_return_refines_to_consistent_returned_local_generic_type`,
    `source_summary_super_calls_lower_to_boundary_defaults_not_raw_super_receiver`,
    `jdk_file_constructor_clones_borrowed_string_array_element`,
    `reassigned_string_enhanced_for_variable_iterates_owned_mutable_values`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 301 passing tests,
    `cargo test --test batch_translation --quiet`,
    `cargo test --test type_coercion --quiet`, and
    `cargo build --release --quiet --bin new-java2rust`. Latest VarScan rerun
    `.tmp/corpus-batch-current-varscan-after-loop-owned-elements` translates
    18/18, reports 0 invalid-pattern files, and passes offline `cargo check`.
  - Lowercase JDK static-owner recovery: aliases such as `math`, `double`,
    `float`, `integer`, and `system` now recover to canonical JDK static owners
    only when a known static method/arity proves the owner. The negative rule
    for ordinary lowercase receivers is preserved, and generated support emits
    matching static JDK surfaces while avoiding duplicate weak instance stubs
    for `Math`, `Double`, `Float`, `Integer`, `System`, and `Objects`.
    Verified with
    `lowercase_jdk_static_owner_aliases_recover_canonical_static_calls`,
    `call_plan_recovers_normalized_static_utility_owner_receivers`,
    `jdk_numeric_and_decimal_format_facades_keep_precise_return_shapes`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 351 passing tests,
    `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
    statistics rerun
    `.tmp/corpus-fastqc-statistics-after-lowercase-jdk-static-support`.
    The focused subset translates 2/2 files with 0 invalid-pattern files;
    generated sources contain `Math::sqrt`/`Math::pow` plus emitted `Math`
    support and no lowercase `math.` rows.
  - Compound primitive-array allocation lengths now cast to `usize` after the
    whole length expression is rendered. FastQC-style `new float[data.length -
    offset]` now emits a valid repeat count instead of `(len as i32) - offset
    as usize`. Verified with
    `primitive_array_allocation_casts_compound_length_as_whole_expression`,
    `documents_primitive_array_allocation_gap`, `cargo fmt --check`,
    `cargo check --quiet`, `cargo test --test idiom_gaps --quiet` with 352
    passing tests, `cargo build --release --quiet --bin new-java2rust`, and
    `.tmp/corpus-fastqc-statistics-after-array-length-cast`, which translates
    2/2 files with 0 invalid-pattern files and offline `cargo check` ok.
  - JDK `File` instance facade progress: `getName`, `getParent`,
    `getAbsoluteFile`, `isDirectory`, and `mkdir` now have generated platform
    support, and the focused wrapper's shared support block is synchronized
    with the translator support. Verified with
    `jdk_file_common_instance_facades_compile`, `cargo check --quiet`, and
    focused FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-file-facades-r3`, where the
    previous missing File method rows are gone. The subset now translates 8/8
    with 0 invalid-pattern files and reaches generated Rust diagnostics headed
    by non-static inner-class outer capture, optional vector indexing, and
    smaller JDK/GUI facade gaps.
  - Collection-get array indexing and string facade progress:
    `group.get(0)[n]` now unwraps the collection lookup before applying the
    nested array index, and Java `String.getBytes()`/`String.indexOf(...)` now
    lower through native string plans plus generated runtime helpers. Verified
    with `collection_get_result_unwraps_before_array_indexing`,
    `erased_object_sequence_push_moves_when_no_later_read`,
    `java_string_get_bytes_and_index_of_use_native_helpers`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 355 passing tests,
    `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
    AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-string-facades-r2`. The subset
    remains 8/8 translated with 0 invalid-pattern files and generated-Rust
    errors drop to 190; the optional `Option<&Vec<_>>` indexing rows and
    `String.get_bytes`/`String.index_of` rows are gone.
  - Receiverless `getClass().getResourceAsStream(...)` and AWT graphics
    facade progress: current-source `getClass()` no longer emits an
    unresolved free `get_class()` call, `BufferedImage.TYPE_INT_RGB` is a typed
    JDK constant, `createGraphics()` returns `Graphics`, and explicit
    `Graphics g = image.createGraphics()` locals stay typed even when passed
    through `JPanel.print(g)`. Verified with the receiverless getClass fixture,
    four buffered-image/graphics fixtures, full `idiom_gaps` with 360 passing
    tests, batch/parser/type-coercion/idiom-sweep checks, wrapper syntax,
    release build, and focused FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-getclass-awt-facades-r7`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 186; the `get_class`,
    `BufferedImage::TYPE_INT_RGB`, erased `Graphics.dispose`, and invalid
    `Graphics` downcast rows are gone.
  - Resource `InputStream` facade progress: `InputStream.read(byte[])` now
    plans the byte buffer as a mutable borrowed slice through shared external
    argument facts and slot coercion, and `InputStream.close()` has concrete
    generated support in both the translator and focused wrapper. Conventional
    external `VERSION` static constants now type as string constants, so
    imported owners such as `FastQCApplication.VERSION` no longer fall into
    invalid string downcasts. Verified with
    `resource_input_stream_read_and_close_have_typed_facade_support`,
    `nullable_receiver_external_argument_facts_shape_borrowed_stream_read_args`,
    `platform_mutable_external_argument_marks_local_binding_mutable`,
    `conventional_external_version_constant_append_does_not_downcast`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 361 passing tests,
    `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
    AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-inputstream-resource-facade-r3`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 174; `InputStream.read`/`close` and
    `FastQCApplication.VERSION` invalid-cast rows are gone.
  - Non-static inner-class outer capture now emits an explicit
    `Rc<RefCell<Outer>>` carrier and lowers proven outer-field reads through
    `self.__outer.borrow().field`. Verified with
    `non_static_inner_class_outer_fields_lower_through_outer_capture_handle`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 362 passing tests, release
    build, and focused FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-inner-outer-capture-r2`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 154; bare unresolved `sb`, `c_width`,
    `c_height`, and `g` rows are gone.
  - `StringBuilder`/`StringBuffer.append(...)` support now accepts displayable
    JDK append overload values instead of only borrowed strings, and the
    outer-capture fixture typechecks the generated support. Verified with
    `non_static_inner_class_outer_fields_lower_through_outer_capture_handle`,
    `direct_string_builder_append_is_modeled_as_fluent_jdk_call`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 362 passing tests, release
    build, and focused FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-display-append-r1`. The subset
    remains 8/8 translated with 0 invalid-pattern files and generated-Rust
    errors drop to 63.
  - Nullable constructor field slots now preserve nullable static/JDK facade
    results instead of double-wrapping `Option<T>`, including the
    `System.getProperty(...)`/`System::get_property(...)` spelling bridge.
    Verified with
    `static_external_nullable_constructor_field_initializer_does_not_double_wrap`,
    the existing nullable-field/local optional regressions, `cargo fmt --check`,
    `cargo check --quiet`, `cargo test --test idiom_gaps --quiet` with 363
    passing tests, release build, and focused FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-nullable-constructor-slot-r1`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 62; the `line_separator:
    Some(System::get_property(...))` row is gone.
  - Constructor field assignments that depend on constructor-local
    declarations now use the `__java_self` body path rather than struct-literal
    compaction, so required locals are declared before deferred field
    assignment. `BufferedImage.getGraphics()` is modeled like
    `createGraphics()` as a typed JDK AWT facade return, focused-wrapper
    `BufferedImage`/`Graphics` support is synchronized, and modeled external
    returns assigned to same-typed fields now provide concrete native
    field-shape evidence instead of leaving unknown-boundary erasure in
    control. Verified with
    `constructor_field_assignment_from_local_keeps_local_and_typed_field`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 365 passing tests,
    `cargo build --release --quiet --bin new-java2rust`, wrapper syntax, and
    focused FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-local-graphics-r4`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 55; `SVGGenerator::new` now keeps
    `let b: BufferedImage = ...`, assigns `__java_self.g =
    Some(b.get_graphics())`, and stores `g` as `Option<Graphics>` rather than
    `Option<Box<dyn std::any::Any>>`.
  - Stable JDK GUI facade getters now have typed return facts for
    `Component.getWidth/getHeight`, `Color.getRed/getGreen/getBlue`,
    `Font.getSize/isBold`, and Swing `TableModel` count/name/value accessors.
    `String.valueOf(Object)` and Java append/display slots now share the
    erased-object display conversion path where needed, so
    `TableModel.getValueAt(...)` display uses lower through
    `java_erased_object_to_string(&table.get_value_at(...))` instead of invalid
    `downcast::<String>()`. The erased-object runtime helper support is emitted
    only for source patterns that need it, so ordinary string-array and
    `StringBuilder.append(String)` idioms remain stable. Verified with
    `platform_gui_facade_getters_have_precise_return_shapes`,
    `direct_string_builder_append_is_modeled_as_fluent_jdk_call`,
    `documents_string_array_literal_argument_gap`, `cargo fmt --check`,
    `cargo check --quiet`, `cargo test --test idiom_gaps --quiet` with 366
    passing tests, `cargo build --release --quiet --bin new-java2rust`, and
    focused FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-gui-erased-display-r2`. The
    subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 39.
  - Source borrowed getter local recovery and local-shadowed JDK `ZipFile`
    receiver/facade support are fixed in focused FastQC AbstractQCModule runs
    through `.tmp/corpus-fastqc-abstract-qcmodule-after-zipfile-facade-r1`.
    The subset remains 8/8 translated with 0 invalid-pattern files and 34
    generated-Rust errors; the `ZipFile.as_ref` family is gone.
  - Constructor field assignments whose RHS reads another receiverless instance
    field now defer until `__java_self` exists, and UpperCamel static-owner
    field access no longer collapses to same-named lowerCamel fields. Typed JDK
    constants for `Color.BLACK` and `Font.PLAIN` now lower as static constants
    rather than optional receiver field reads. Verified with
    `constructor_assigned_field_read_defers_until_self_temporary_exists`,
    `static_owner_name_does_not_collapse_to_same_named_instance_field`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 370 passing tests,
    `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
    AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-static-owner-r1`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 32.
  - Deferred constructor field assignments now lower self-dependent RHS
    expressions with a temporary self receiver before replacing `self` with
    `__java_self`, so nested source-constructor arguments can recover owned
    fields as `__java_self.field.clone()` or optional fields as
    `__java_self.field.as_ref().expect(...).clone()` instead of cloning the
    whole partially built struct. Verified with
    `constructor_nested_arg_reads_assigned_field_without_cloning_self`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 371 passing tests,
    `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
    AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-constructor-self-field-slot-r1`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 31.
  - JDK constructor argument planning now has an actual-argument-aware fact path
    ahead of arity-only constructor facts. This preserves the existing
    `File(String)` slot recovery while distinguishing `FileOutputStream(File)`
    from `FileOutputStream(String)`: nullable `File` fields are recovered as
    `.as_ref().expect(...).clone()`, while string paths stay string/path slots.
    Verified with
    `file_output_stream_constructor_selects_file_and_string_overload_slots`,
    `jdk_file_constructor_clones_borrowed_string_array_element`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 372 passing tests,
    `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
    AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-fileoutputstream-overload-slots-r1`.
    The subset remains 8/8 translated with 0 invalid-pattern files and 31
    generated-Rust errors; the `FileOutputStream` line now unwraps
    `__java_self.zip_file`, and the remaining adjacent constructor-argument row
    is `ZipOutputStream`/`OutputStream` facade-inheritance slot drift.
  - Java platform conversion adapter facts now cover
    `ZipOutputStream -> OutputStream`, generated support emits the matching
    `From` surface, and reused owned values that need both clone recovery and
    external `Into` conversion clone before `.into()` rather than after it.
    Verified with `zip_output_stream_converts_to_output_stream_slot`,
    `java_platform_zip_output_stream_converts_to_output_stream_slot`,
    `appendable_field_conversion_clones_before_into`, `cargo fmt --check`,
    `cargo check --quiet`, `cargo test --test idiom_gaps --quiet` with 373
    passing tests, `cargo test --test type_coercion --quiet` with 86 passing
    tests, `cargo build --release --quiet --bin new-java2rust`, and focused
    FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-zip-outputstream-adapter-r2`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 30; the remaining stream mismatch is
    the FastQC iHarder Base64 stream wrapper rather than core JDK knowledge.
  - External-pack conversion facts now cover both Apache Commons Codec
    `Base64OutputStream -> OutputStream` and the actual FastQC iHarder
    `net.sourceforge.iharder.base64.Base64.OutputStream -> OutputStream`
    nested stream shape. Verified with
    `apache_commons_codec_pack_declares_base64_output_stream_conversion`,
    `iharder_base64_pack_declares_nested_output_stream_conversion`,
    `base64_output_stream_converts_to_output_stream_slot`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
    AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-iharder-base64-outputstream-adapter-r1`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 29; generated `ImageToBase64` now emits
    `Base64OutputStream::new(os.clone()/* TODO(translation): validate added clone */).into()`.
  - Source struct assembly now preserves Java `public` instance field
    visibility, while private/package fields remain private. Slot lowering also
    clones owned field values read through borrowed source receivers, so public
    `String` fields do not move out of `&T`. Verified with
    `public_source_instance_fields_remain_public_in_generated_struct`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 375 passing tests,
    `cargo test --test type_coercion --quiet` with 86 passing tests,
    `cargo build --release --quiet --bin new-java2rust`, and focused FastQC
    AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-public-field-visibility-r1`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 27; `FastQCConfig.line_separator` and
    `FastQCConfig.svg_output` now emit as public fields.
  - Source `static final` non-literal facade fields now emit associated default
    facade values instead of instance storage, and constructor planning no
    longer initializes those static fields as if they were instance fields.
    JDK regex facade facts now type `Pattern.compile`, `Pattern.matcher`,
    `Matcher.matches`, and `Matcher.group`. Verified with
    `source_static_final_facade_fields_emit_associated_defaults_not_storage`,
    `cargo fmt --check`, `cargo check --quiet`,
    `cargo test --test idiom_gaps --quiet` with 376 passing tests,
    `cargo test --test type_coercion --quiet` with 86 passing tests,
    `TMPDIR=/data/henriksson/github/claude/new_java2rust/.tmp cargo build --release --quiet --bin new-java2rust`,
    and focused FastQC AbstractQCModule rerun
    `.tmp/corpus-fastqc-abstract-qcmodule-after-static-final-pattern-r1`.
    The subset remains 8/8 translated with 0 invalid-pattern files and
    generated-Rust errors drop to 25; `SVGImageSaver::LINE_PATTERN` and
    `SVGImageSaver::RECT_PATTERN` are now emitted as associated `Pattern`
    values.
- Remaining action: group the visible non-VarScan generated-Rust errors into
  fixtures. Shared families include array creation, switch scaffolding,
  trait/interface assembly, remaining source/facade return-slot mismatches, VCF
  support enum assembly, Spark/DISQ interface bridging, GenomicsDB overload
  handoff naming, void expressions flowing into display slots,
  singleton/static field lowering, and remaining source-super variants not
  covered by the current simple-constructor inlining.
- 2026-07-24 JAligner iterator-cursor slice: Java collection `.iterator()` now
  lowers through a shared `JavaIterator<T>` platform cursor facade instead of
  emitting Rust `Iterator` trait locals or `.iterator()` calls on `Vec<T>`.
  This preserves Java `hasNext`/`next` cursor semantics while avoiding a Rust
  trait-type false match. Verified with
  `java_list_iterator_cursor_uses_platform_facade`, `cargo fmt --check`,
  `cargo check --quiet`, iterator-adjacent idiom tests, `bash -n
  scripts/translate-focused-package.sh`, `cargo build --release`, and focused
  JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-java-iterator-cursor-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from the previous 57 to 51. The previous
  `Vec<String>.iterator()` and `Iterator` trait-type rows are gone; remaining
  top rows are `E0308` mismatches, missing `system`/`instance` names,
  temporary-borrow lifetime issues, `char_at` on `()`, `Matrix::new_2`, and
  JAligner-specific facade/source surface gaps.
- Follow-up debt from the iterator slice: local mutability currently has a
  narrow Java cursor bridge that treats `receiver.next()` as advancing the
  receiver. Replace this with facade ABI receiver facts (`next(&mut self)`)
  once Rust signature facts are authoritative for local mutability inference.
- 2026-07-24 JAligner JDK facade-signature slices:
  `StringTokenizer.countTokens/hasMoreTokens/nextToken` now have explicit Java
  return facts and Rust facade ABI facts. `nextToken` is modeled as
  `ReceiverAbi::MutBorrowedSelf`, and local mutability inference now marks
  receivers mutable from facade ABI facts rather than from a hardcoded
  `next`-name rule. Verified with
  `java_string_tokenizer_uses_facade_signature_facts`,
  `stdlib_owns_known_jdk_content_method_facts`, `cargo fmt --check`,
  `cargo check --quiet`, `bash -n scripts/translate-focused-package.sh`,
  `cargo build --release`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-string-tokenizer-facts-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 51 to 43. The tokenizer `()` return rows and `char_at` on `()`
  rows are gone.
- The same JDK facade-signature direction now covers `File.list`,
  `File.isFile/is_file`, and `File.canRead/can_read`, with ABI facts and shared
  support implementations mirrored in `scripts/translate-focused-package.sh`.
  Verified with `jdk_file_common_instance_facades_compile`,
  `stdlib_owns_known_jdk_content_method_facts`, local gates, release build, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-file-facade-facts-r1`. Result:
  29/29 translated, 0 invalid-pattern files, generated-Rust errors dropped from
  43 to 40. The missing `File.list/is_file/can_read` rows are gone.
- The static utility signature slice now covers lowercase static-owner recovery
  for `Float.parseFloat/parse_float` and `System.gc`, with selected
  `FacadeAbiFact` rows and native lowering for the Java semantic calls.
  Verified with
  `jdk_numeric_and_decimal_format_facades_keep_precise_return_shapes`,
  `stdlib_owns_known_jdk_content_method_facts`, `cargo fmt --check`,
  `cargo check --quiet`, `bash -n scripts/translate-focused-package.sh`,
  `cargo build --release`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-static-utility-facts-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 40 to 37. The previous `system.gc()` and
  `float.parse_float(...)` generated rows are gone.
- The ndarray scalar assignment slice now treats nested Java array assignment
  targets as storage slots for the actual cell type, so `Array2<T>` element
  writes do not borrow scalar RHS values. Numeric parse lowering also normalizes
  fact-backed borrowed string arguments before rendering Rust `.parse()` method
  syntax, avoiding `&temporary.parse::<T>()` when a Java static parse call is
  represented by a borrowed Rust string slot. Verified with
  `two_dimensional_numeric_arrays_lower_to_ndarray_array2`, `cargo fmt --check`,
  `cargo check --quiet`, `bash -n scripts/translate-focused-package.sh`,
  `cargo build --release`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-parse-receiver-ndarray-slot-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 37 to 36. The `scores[[...]] =
  &tokenizer.next_token().parse::<f32>().unwrap()` row is gone.
- Java numeric char-code literals now adapt in char slots and switch labels
  without requiring explicit `(char)` casts. Assignment into `char[]` lowers
  `0` to `'\0'`, and char switch labels such as `9`, `10`, `13`, and `32`
  lower to Rust char literals in both fallback-switch conditions and simple
  match patterns. Verified with
  `java_numeric_char_literals_adapt_to_char_slots_and_switch_labels`,
  `java_numeric_char_casts_use_valid_rust_char_conversions`,
  `cargo fmt --check`, `cargo check --quiet`,
  `bash -n scripts/translate-focused-package.sh`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-char-code-literals-r1`. Result:
  29/29 translated, 0 invalid-pattern files, generated-Rust errors dropped from
  36 to 31. The `expected char, found u8` rows are gone.
- Source inheritance/upcast and Java `Object.getClass()` fallback slice:
  source subclasses now emit state-preserving `impl From<Sub> for Super` when
  both source types are emitted and the superclass fields have been flattened
  into the subclass. Context-only superclasses still provide inherited fields
  and methods but do not produce invalid `From` impls for missing emitted
  structs. Source classes that do not declare `getClass` now get an inherent
  `get_class(&self) -> JavaClass<Self>` fallback alongside the existing
  `to_string` fallback, and optional ternaries now wrap only the concrete branch
  when the other branch is already `Option<T>`. Verified with
  `source_subclass_constructor_flows_to_flattened_superclass_slot`,
  `no_arg_super_constructor_with_inherited_setter_trailing_call_lowers`,
  `source_object_get_class_name_uses_inherent_java_class_fallback`,
  `cargo test --test idiom_gaps ternary --quiet`, `cargo fmt --check`,
  `cargo check --quiet`, `cargo build --release`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-mixed-optional-getclass-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 31 to 27. The four source `get_class` missing-method rows are
  gone; remaining top rows are now unresolved static fields/singletons,
  `Matrix::new_2` constructor naming, borrowed temporary lifetime, logging
  `Level` alias support, and residual type mismatches.
- Resolved source constructors now keep their source constructor rust name even
  when a simple-name external facade has an overloaded constructor fact. This
  prevents local source classes such as JAligner `Matrix` from being renamed to
  external overload helpers such as `Matrix::new_2(...)`. Verified with
  `resolved_source_constructor_name_beats_external_simple_name_arity_fact`,
  existing external constructor arity tests, `cargo fmt --check`,
  `cargo check --quiet`, `cargo build --release`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-source-constructor-name-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 27 to 26, and the `Matrix::new_2` source-constructor row is gone.
- Source mutable static fields now have a distinct storage identity in
  `ShapeEnv`. Bare same-class reads lower through generated reserved accessors
  such as `Self::__static_count()`, writes lower through
  `Self::__set_static_count(...)`, and non-const `static final` fields use the
  same storage path instead of pretending an associated const exists. Generated
  storage is thread-local `RefCell<Option<T>>`, which preserves prior writes in
  the translated thread without imposing `Sync`/`Send` on Java object graphs.
  Verified with `source_static_mutable_fields_use_generated_storage_accessors`,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-static-storage-r3`. Result: 29/29
  translated, 0 invalid-pattern files, generated-Rust errors dropped from 26 to
  18. Bare unresolved static rows for `instance`, `random`, `userDirectory`,
  `fileSeparator`, `lineSeparator`, and `jnlp` are gone. Remaining static debt:
  object-valued nullable singleton storage still requires reference-style
  object handles or a source object clone/capability proof; static-object field
  writes like `commons.jnlp = ...` need static receiver field-access lowering;
  static final array `.length` in constant initializers still emits the wrong
  `CHARS::length` shape.
- Static final array `.length` is now available to the constant evaluator for
  same-class static final array fields with literal array initializers. This
  lowers JAligner-style `private static final int NUMBER_OF_CHARS =
  CHARS.length;` to a real integer constant instead of `CHARS::length`.
  Verified with
  `source_static_final_array_length_can_initialize_static_final_constant`,
  the existing static-final array fixture, `cargo fmt --check`,
  `cargo check --quiet`, `cargo build --release`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-const-array-length-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 18 to 17. The `CHARS::length` row is gone and
  `RandomSequenceGenerator::NUMBER_OF_CHARS` now emits as `20`.
- Static object field writes through same-class static holder values now lower
  to the storage accessor for the declared static field instead of treating the
  holder expression as a Rust instance value. This covers Java's permitted
  static-through-instance syntax such as `commons.jnlp = jnlp` and the
  minimized `holder.enabled = value` fixture. Same-class writes render through
  `Self::__set_static_field(...)`; cross-type writes use the resolved owner
  type. Verified with
  `lowercase_static_owner_field_write_uses_static_storage_setter`,
  `source_static_mutable_fields_use_generated_storage_accessors`,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-qualified-static-storage-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 17 to 16. The `cannot find value commons` row is gone and
  `Commons::set_jnlp` now emits `Self::__set_static_jnlp(jnlp)`.
- Public source `String` parameters that are proven to be used only as string
  content now use the borrowed string-content slot instead of the broader public
  native `String` boundary. The proof path is conservative: it accepts direct
  string-content receiver calls and content-read facade arguments, and rejects
  storage, mutation, anonymous bodies, or unclassified uses. This is a
  signature/slot fix rather than a call-site adapter, so static string constants
  can flow into source helpers without allocation or `&String` mismatches.
  Verified with
  `public_readonly_string_param_uses_str_view_for_static_string_constants`,
  related source call shape tests, `cargo fmt --check`, `cargo check --quiet`,
  `cargo build --release`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-public-string-content-param-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 16 to 14. The two `expected &String, found &str` rows for
  `SmithWatermanGotohExample::load_sample_sequence(Self::SAMPLE_SEQUENCE_...)`
  are gone; the generated helper now takes `path: &str`.
- Shared support merging now preserves Java logging `Level` constants when
  `java.util.logging.Level` is canonicalized to the root `Level` facade. The
  root support merger adds missing builtin constants for promoted `Level`
  declarations, and core support keeps `Level` constant emission out of
  duplicate per-module impls. Verified with
  `java_util_logging_level_import_uses_shared_level_facade`,
  `shared_support_merger_adds_level_constants_for_promoted_logging_facade`,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-level-constants-r3`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors are
  back to 14 after the temporary `Level` regression; the missing
  `Level::SEVERE`/`Level::WARNING` rows are gone.
- `ClassLoader.getResourceAsStream(...)` and related `Class`/`ClassLoader`
  resource lookups now have option-returning JDK facade ABI facts. Chained
  facade receiver projection can recover the receiver type through calls such
  as `SomeClass.class.getClassLoader().getResourceAsStream(path)`, and optional
  assignment wrapping avoids `Some(Option<InputStream>)`. Verified with
  `optional_class_loader_resource_stream_assignment_keeps_optional_return_shape`,
  the logging `Level` regression tests, `cargo fmt --check`,
  `cargo check --quiet`, `cargo build --release`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-optional-resource-stream-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 14 to 13. The assignment now preserves the direct optional
  resource-stream value. There is still a narrow syntax-level compatibility
  fallback for this optional shape; the desired final state is that chained
  facade ABI result projection fully proves it from facts.
- Previous JAligner target: solve the exposed `FileInputStream::new(...)` into
  `Option<InputStream>` mismatch through external conversion/upcast facts
  (`FileInputStream -> InputStream`) and slot conversion, not a call-site
  wrapper. Adjacent remaining families are statement-side generic
  `Collections.sort(...) -> ()`, `NamedInputStream.get_name()` returning `()`,
  temporary borrowed string lifetimes, `Format`/`Pair` interface shape, and a
  moved `StringBuffer.append(...)` argument.
- JDK input-stream subclass upcasts now use external conversion adapter facts
  and the shared slot conversion path. The platform adapter table covers common
  `InputStream` subclasses such as `FileInputStream`, and shared support
  assembly preserves required `impl From<SubStream> for InputStream` edges even
  when `InputStream` was pre-populated as builtin focused support before the
  merger ran. Verified with
  `java_platform_file_input_stream_converts_to_input_stream_slot`,
  `java_platform_file_input_stream_converts_before_optional_wrap`,
  `java_platform_input_stream_upcast_support_emits_from_impl`,
  `file_input_stream_constructor_upcasts_into_optional_input_stream_slot`,
  `shared_support_merger_adds_input_stream_upcast_for_promoted_jdk_facades`,
  `cargo fmt --check`, `cargo check --quiet`, and focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-inputstream-upcast-r3`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 13 to 12. The `InputStream: From<FileInputStream>` row is gone,
  `MatrixLoader` emits `Some(FileInputStream::new(...).into())`, and shared
  support emits `impl From<FileInputStream> for InputStream`.
- Statement-side `Collections.sort(...)` now lowers as a Java platform mutator
  instead of falling through to a generic `JavaUtilCollections::sort<R:
  Default>(...)` facade call. The one-argument overload emits `.sort()`, and
  the comparator overload evaluates the comparator once and emits
  `.sort_by(|a, b| comparator.compare(a.clone(), b.clone()).cmp(&0))`.
  Static `Collections.sort` argument 0 is also modeled as a mutating Java
  platform argument effect, and the lowerer carries suffix mutation facts to
  local declarations so interface-declared locals such as `List<String>
  matrices = new ArrayList<String>()` become mutable before later conditional
  sorts. Verified with the `collections_sort*` focused fixtures,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-collections-sort-r4`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 12 to 11. `MatrixLoader` now emits `let mut matrices:
  Vec<String>` plus the comparator `sort_by` block, and the previous
  `cannot borrow matrices as mutable` row is gone.
- Omitted-context generated support now upgrades weak opaque getter methods
  when source/import/manifest facts prove a precise return type. This fixes
  `NamedInputStream.get_name()` returning `()` in focused JAligner support and
  keeps constructor arguments such as `Matrix::new(nis.get_name(), ...)` typed
  as `String` instead of `Any`/unit fallback. Verified with
  `omitted_context_name_getter_feeds_typed_constructor_argument`,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-name-getter-r1`. The getter row
  is gone; the rerun exposed nullable `BufferedReader.readLine()` drift.
- `BufferedReader.readLine()` now carries `MaybeNull` return nullability in
  the Java platform API fact, so the facade signature is
  `Result<Option<String>, JavaException>` and initializers/assignments preserve
  the direct optional value rather than wrapping with `Some(...)`. Compound
  null-test assignment expressions such as
  `while ((line = reader.readLine()) != null && ...)` now lower to
  `{ line = reader.read_line()?; line.is_some() }`, avoiding a move of the
  optional local while still using the shared assignment slot conversion path.
  Verified with
  `buffered_reader_read_line_initializer_is_optional_when_rechecked_for_null`,
  `buffered_reader_read_line_assignment_to_initialized_optional_wraps_result_value`,
  `buffered_reader_read_line_assignment_condition_wraps_result_value`,
  `buffered_reader_read_line_assignment_condition_with_and_does_not_move_optional`,
  `translates_jaligner_fallthrough_try_catch_result_without_placeholder`,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-assignment-null-condition-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 11 to 8. The remaining top families are `Format`/`Pair`
  interface conversion, singleton optional cloning for `FormatFactory`,
  borrowed temporary string lifetimes in append/substring paths, and a moved
  `StringBuffer.append(...)` argument.
- Source-subclass-to-superclass slot conversion now works when the superclass
  is an emitted source-context type rather than part of the same lowered
  source file. The existing flattened source conversion emitter now consults
  emitted context-source hierarchy, and owned assignment slots can apply the
  same `.into()` upcast when source hierarchy reachability is proven from that
  context. Verified with
  `emitted_context_source_subclass_constructor_flows_to_superclass_slot`,
  `source_subclass_constructor_flows_to_flattened_superclass_slot`,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-source-context-upcast-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, generated-Rust errors
  dropped from 8 to 5. The three `let format: Format = Pair::new();` rows are
  gone; generated tester modules now emit `Pair::new().into()`, and `Pair`
  emits `impl From<Pair> for Format`.
- JAligner now cargo-checks cleanly in focused package mode. The append slice
  routes generic `StringBuilder`/`StringBuffer.append(...)` through shared
  inferred argument slots: reusable non-primitive arguments borrow, while
  ternary/value-producing arguments stay owned to avoid mixed branch references
  and temporary lifetime errors. Static singleton source classes with
  cloneable `HashMap` fields now derive `Clone`, and superclass fields used by
  emitted cross-module source upcast impls are emitted as `pub(crate)`.
  Verified with
  `string_buffer_append_reused_object_argument_is_borrowed`,
  `string_buffer_append_ternary_argument_stays_owned_value`,
  `private_static_singleton_with_hashmap_field_derives_clone_when_value_is_cloneable`,
  `emitted_context_source_subclass_constructor_flows_to_superclass_slot`,
  `source_subclass_constructor_flows_to_flattened_superclass_slot`,
  `cargo fmt --check`, `cargo check --quiet`, `cargo build --release`, and
  focused JAligner rerun
  `.tmp/corpus-sweep-20260724-jaligner-after-upcast-field-visibility-r1`.
  Result: 29/29 translated, 0 invalid-pattern files, and `cargo_check=ok`
  with 0 generated-Rust errors.
- 2026-07-24 indexed borrowed-place follow-up: indexed Java array/vector reads
  now materialize a Rust borrowed place when the target slot is a shared-borrow
  argument, display segment, or concat segment. This closes the recurring
  `cannot move out of index of Vec<String>` family without adding a
  string-specific append branch. Optional construction/rendering also normalizes
  redundant `Some(&value.as_str())` to `Some(value.as_str())`, and borrowed
  optional element carriers avoid adding an extra `.as_ref()` when the carrier
  already has shape `Option<&T>`. Verified with
  `nested_indexed_string_array_append_borrows_inner_vec_element`,
  `indexed_string_array_reads_clone_only_for_owned_slots`,
  `indexed_string_array_into_optional_borrowed_string_arg_borrows_element`,
  `cargo test --test type_coercion --quiet`, `cargo fmt --check`,
  `cargo check --quiet`, `cargo build --release --quiet --bin new-java2rust`,
  and focused FastQC AbstractQCModule rerun
  `.tmp/corpus-fastqc-abstract-qcmodule-after-indexed-borrow-r1`. Result:
  8/8 translated, 0 invalid-pattern files, and `cargo_check=ok` with 0
  generated-Rust errors. Broad `cargo test --test idiom_gaps --quiet` remains
  red in this dirty checkout with 31 unrelated failing in-flight idiom slices,
  so use focused regressions plus corpus evidence until that suite is
  re-baselined.
- 2026-07-26 indexed projection cleanup: array/vector access lowering now asks
  `shape_projection::indexed_read_shape_for_access_with_facts` for the element
  slot shape, including direct sequence receivers and `Optional<Array<T>>`
  receivers. The remaining borrow-place rendering still lives in `lower.rs`
  because it depends on the target slot, but type/capability/fact projection for
  indexed reads is centralized before emission.
- 2026-07-26 optional receiver slot-plan cleanup: call planning now owns
  `OptionalReceiverSlotPlan`, carrying the executable
  `coercion::OptionalReceiverProjection` selected from optional receiver
  mutability plus the current legacy unchecked policy and mapping to the same
  borrowed receiver slots used by facade receiver ABI. `lower.rs` still
  decides whether a receiver expression is actually optional from local/field
  shape context, but call receiver adaptation now projects the optional carrier,
  attaches the projected receiver to an explicit `LoweredType` element shape,
  and applies `plan_lowered_slot_conversion` against the planned receiver slot.
  A method-autoref gate preserves Rust receiver autoderef for ordinary
  `String`, `Box<dyn Trait>`, and facade object method calls, so the current
  checked/unchecked null spelling remains stable while explicit receiver
  conversions still flow through the shared slot plan instead of a
  receiver-only formatting branch. Follow-up: nullable `Map.entrySet()` lowering
  now uses the same optional receiver slot-plan adapter before rendering
  `.iter()`, and enhanced-for iterable unwrapping does the same before loop
  rendering. Optional sequence `get(...)` and optional map `get_mut(...)`
  mutation lowering now use the same adapter before indexing/mutation receiver
  rendering. Optional receiver field access, including `.length`, now uses the
  adapter before field/member rendering too. This leaves `lower.rs` with no
  direct optional receiver projection calls outside the shared adapter while
  preserving existing iterator/index/mutation/field output.
- 2026-07-26 optional receiver projection plan cleanup: `OptionalReceiverSlotPlan`
  now carries the executable `coercion::OptionalReceiverProjection` directly,
  removing the lowerer-local `optional_receiver_projection_from_plan` bridge.
  Call planning still selects the projection mode from mutability and legacy
  null-check policy, but lowering no longer translates a second planning-only
  enum before calling `project_optional_receiver`.
- 2026-07-26 parse receiver cleanup: native parse-call lowering now uses
  `coercion::render_parse_receiver` for the remaining rendered `&arg`
  compatibility cleanup before emitting `.parse::<T>()`. This keeps the
  parse-specific borrowed argument spelling beside the other coercion
  compatibility helpers instead of in `call_lowering.rs`.
- 2026-07-26 primitive parse projection follow-up: `Integer.parseInt`,
  `Float.parseFloat`, and `Double.parseDouble` now call
  `coercion::primitive_parse_projection` for the full Rust
  `.parse::<T>().unwrap()` expression. Call lowering still owns Java overload
  arity checks and primitive target selection, while coercion owns the rendered
  receiver cleanup plus parse-call spelling.
- 2026-07-26 String.join projection cleanup: Java `String.join` lowering now
  delegates vararg `.to_string()` idempotence, literal extraction, and delimiter
  borrowed-string projection to `coercion::render_string_join_vararg_value` and
  `coercion::string_join_delimiter_projection`. Call lowering still selects the
  Java join shape, but the rendered Rust compatibility spellings live with the
  other coercion helpers.
- 2026-07-26 String.join varargs follow-up: the full Rust
  `vec![...].join(delimiter)` projection now lives in
  `coercion::string_join_varargs_projection`; call lowering only groups the Java
  vararg arguments and delegates the rendered spelling.
- 2026-07-26 String.format fallback cleanup: non-literal Java `String.format`
  support calls now render fallback display arguments through
  `coercion::render_string_format_arg_value` instead of formatting
  `{}.to_string()` in `call_lowering.rs`. This preserves the current generated
  Rust while keeping string display compatibility spelling in the coercion
  layer.
- 2026-07-26 String.format projection follow-up: `String.format` now delegates
  empty-argument fallback, non-literal `java_string_format(..., vec![...])`
  runtime calls, Java-to-Rust literal format conversion, and final `format!(...)`
  emission to `coercion::string_format_projection`. Call lowering only selects
  the Java static method target.
- 2026-07-26 native extend recovery cleanup: borrowed sequence sources feeding
  native collection `extend` now render recovery iterators through
  `coercion::borrowed_sequence_extend_arg`. Call lowering still emits the
  native `extend` call, while borrowed-element recovery spellings such as
  `.iter().map(|v| v.to_string())`, `.copied()`, and `.cloned()` live with the
  shared coercion policy.
- 2026-07-26 Java string helper receiver cleanup: regex/hash/split runtime
  helper receivers now use `coercion::string_helper_receiver_projection` for
  the `.as_str()` view spelling. Call lowering still selects the Java runtime
  helper, while the rendered Rust receiver projection lives with the shared
  string coercion helpers.
- 2026-07-26 Java string-view argument cleanup: string helper arguments that
  need a borrowed string view now call `coercion::render_string_view_argument`
  instead of a private `call_lowering.rs` helper. `String.indexOf`,
  `substring`, and compare-style runtime helpers still keep Java arity/helper
  selection in call lowering, while coercion owns the borrow/idempotence policy
  for Rust string-view arguments.
- 2026-07-26 Java string index/substring projection cleanup:
  `String.indexOf(...)` and `String.substring(...)` lowering now call
  `coercion::string_index_of_projection` and
  `coercion::string_substring_projection`. Call lowering still validates Java
  arity and supplies the default substring end expression, while coercion owns
  the Rust runtime helper-call shape and string-view argument projection.
- 2026-07-26 Java string runtime-helper projection cleanup: native
  `String.hashCode`, regex replace/matches, `split`, and compare-style helpers
  now call named coercion projections such as
  `string_hash_code_projection`, `string_matches_projection`,
  `string_split_projection`, and `string_compare_to_projection`. Call lowering
  still selects Java overload/arity behavior, while coercion owns the concrete
  `java_string_*` runtime helper-call shapes.
- 2026-07-26 Java byte-array String constructor projection cleanup:
  `new String(byte[])` and byte-range constructor lowering now call
  `coercion::string_from_bytes_projection` and
  `coercion::string_from_byte_range_projection`. Constructor lowering still
  detects the Java `String::new` arity/owner shape, while coercion owns the
  shared borrow projection and `java_string_from_bytes*` helper-call shapes.
- 2026-07-26 Java string byte-view cleanup: `String.getBytes()` still lowers as
  the Java byte-array semantic helper path, but the Rust byte-view expression
  now lives in `coercion::string_get_bytes_projection` instead of being rendered
  directly in call lowering.
- 2026-07-26 boxed primitive value cleanup: boxed numeric value methods still
  select their Java source/target primitive semantics in call lowering, but the
  Rust primitive cast spelling now lives in
  `coercion::primitive_number_value_projection`.
- 2026-07-26 boxed primitive hash/compare cleanup: boxed primitive
  `hashCode()` and `compareTo(...)` lowering now calls
  `coercion::primitive_hash_code_projection` and
  `coercion::primitive_compare_to_projection`. Call lowering still selects the
  Java boxed primitive source and validates `compareTo` arity, while coercion
  owns the Rust primitive hash arithmetic and runtime compare helper spelling.
- 2026-07-26 math numeric projection cleanup: `Math.log`, `Math.exp`, and
  `Math.toIntExact` still keep Java method selection and diagnostics in call
  lowering, while their Rust numeric cast/projection spelling now goes through
  `coercion::primitive_f64_method_projection` and
  `coercion::primitive_number_value_projection`.
- 2026-07-26 JDK math/object projection cleanup: `Math.max`, `Math.min`, and
  `Objects.{equals,nonNull,isNull}` lowering now call
  `coercion::{math_max_projection, math_min_projection,
  objects_equals_projection, objects_non_null_projection,
  objects_is_null_projection}`. Call lowering still performs Java arity
  validation and method selection, while coercion owns the Rust expression
  shapes for `std::cmp` calls, equality, and optional null checks.
- 2026-07-26 Java length projection cleanup: Java string `length()` and
  substring default-end lowering now use `coercion::len_i32_projection` for the
  Rust `len() as i32` spelling. Call lowering still owns Java string method
  semantics; the integer length projection is shared.
- 2026-07-26 static numeric projection cleanup: `Double.isNaN`/`Float.isNaN`
  and `System.exit` still keep Java static-call semantics and diagnostics in
  call lowering, while Rust `is_nan()` and exit-status integer projection now
  route through coercion helpers.
- 2026-07-26 JDK platform projection cleanup: `File.exists`, `System.exit`,
  and `Thread.sleep` lowering now call
  `coercion::{file_exists_projection, system_exit_projection,
  thread_sleep_millis_projection}`. Call lowering still validates Java arity and
  selects the JDK method, while coercion owns the concrete Rust platform-call
  spelling and any primitive status projection.
- 2026-07-26 stream length projection cleanup: stream lambda field-access
  lowering for Java array/string `.length` now reuses `coercion::len_i32_projection`
  instead of rendering `len() as i32` locally. Stream lowering still owns lambda
  binding context and parenthesization.
- 2026-07-26 lower member-access length follow-up: optional sequence length,
  static-final array length, nested-slice row length, and optional receiver
  field length paths in `lower.rs` now also use `coercion::len_i32_projection`.
  Lowering still owns member-access semantics, but the Rust `len() as i32`
  spelling is shared across ordinary calls, stream lowering, and expression
  lowering.
- 2026-07-26 optional member receiver projection cleanup: member-access
  lowering still detects optional sequence/ndarray receivers that need Java
  null-receiver unwrapping, but emitted `.as_ref().expect(...)` spelling now
  comes from `coercion::project_optional_receiver`.
- 2026-07-26 ndarray length projection follow-up: `Array2` row/column length
  rendering now uses `coercion::array2_rows_i32_projection` and
  `coercion::array2_cols_i32_projection` from both `lower.rs` and
  `access_lowering.rs`, so ndarray-specific `.nrows() as i32` spelling is
  centralized with the other length projections.
- 2026-07-26 static matrix handoff cleanup: static-final two-dimensional Java
  array handoff into ndarray slots now uses
  `array_lowering::array2_from_rows_projection`, keeping
  `ndarray::Array2::from_rows(...)` spelling with array projection helpers
  instead of inside `lower.rs`.
- Follow-up: Java array creation rendering now delegates sized one-dimensional,
  sized two-dimensional ndarray, and unsupported creation fallback spellings to
  `array_lowering::{array_from_elem_projection,array2_from_elem_projection,java_array_creation_fallback_projection}`.
  `render_array_expr` still owns Java array-shape selection, but the Rust
  `vec![...]`, `ndarray::Array2::from_elem(...)`, and fallback comment
  templates are named projection helpers.
- 2026-07-26 ndarray element access cleanup: read/write Java
  `matrix[row][col]` lowering still selects ndarray vs nested-slice semantics
  in `lower.rs`, but the emitted `matrix[[row, col]]` projection now comes from
  `array_lowering::array2_index_projection`.
- 2026-07-26 ndarray row replacement cleanup: Java `matrix[row] = values`
  lowering still detects ndarray row assignment in `lower.rs`, but the emitted
  `row_mut(...).assign(&ndarray::ArrayView1::from(...))` statement now comes
  from `array_lowering::array2_row_mut_assign_stmt`.
- 2026-07-26 ragged array creation cleanup: non-ndarray two-dimensional Java
  array creation still selects ragged `Vec<Vec<_>>` semantics in `lower.rs`,
  but the emitted `vec![Vec::new(); len]` outer-vector projection now comes
  from `array_lowering::ragged_array_outer_creation_projection`.
- 2026-07-28 matrix/member-access cleanup: ordinary Java `.length` access now
  asks the same `MemberAccessPlan` used by field lowering before rendering row
  counts. Rectangular numeric `ndarray::Array2<T>` receivers emit
  `.nrows() as i32`, while nested-slice/static table receivers keep `.len()`.
  The ragged object matrix fixture also exposed map-key borrow drift; indexed
  owned `String` elements now keep an explicit borrow for `HashMap::get`, while
  borrowed local keys and `&'static str` constants still avoid double-borrowing.
  Verified with `two_dimensional_numeric_arrays_lower_to_ndarray_array2`,
  `static_final_two_dimensional_numeric_tables_can_feed_readonly_slice_helpers`,
  `ragged_object_two_dimensional_arrays_stay_nested_vecs`,
  `map_get_keyset_loop_does_not_double_borrow_key`,
  `map_get_static_string_constant_key_does_not_double_borrow_str`,
  `map_get_collection_mutation_uses_mutable_lookup`, `cargo fmt --check`, and
  `cargo check --quiet`.
- 2026-07-26 sequence get projection cleanup: Java sequence `get(index)`
  lowering still selects indexed sequence semantics and borrowed element shapes
  in `lower.rs`, but the emitted
  `.get(index).expect("Java index out of bounds")` projection now comes from
  `coercion::sequence_get_expect_projection`.
- 2026-07-26 map get projection cleanup: specialized map lookup paths in
  `lower.rs` still own Java map semantics, optional-element carriers, primitive
  unboxing, and collection mutation selection, but raw `.get(...)`,
  `.get(...).expect(...)`, `.get_mut(...).expect(...)`, and copied-primitive
  optional unwrap spellings now come from `coercion` projections.
- 2026-07-26 erased-object borrowed downcast cleanup: exact erased-object cast
  lowering still decides whether the carrier is optional and whether the target
  slot is a shared borrow, but the emitted `downcast_ref::<T>().expect(...)`
  spelling now comes from `coercion::erased_object_borrowed_downcast_ref_projection`.
- 2026-07-26 native sort projection cleanup: statement planning still selects
  `Arrays.sort`/`Collections.sort` mutator semantics in `lower.rs`, but plain
  `.sort()` and comparator-backed `.sort_by(...)` Rust spellings now come from
  `coercion::{collection_sort_projection, collection_sort_by_comparator_projection}`.
- 2026-07-26 native array fill projection cleanup: statement planning still
  selects `Arrays.fill` mutator semantics in `lower.rs`, but emitted
  `.fill(value)` Rust spelling now comes from
  `coercion::array_fill_projection`.
- 2026-07-26 owned sequence projection cleanup: ndarray row recovery and
  static const array recovery in `lower.rs` now use
  `coercion::to_owned_sequence_projection` instead of formatting `.to_vec()`
  locally. Lowering still decides when immutable static data or ndarray rows
  should become owned Java arrays, while coercion owns the Rust owned-sequence
  projection and idempotence spelling.
- 2026-07-26 map-entry iteration cleanup: enhanced-for lowering over
  `Map.entrySet()` now uses `coercion::render_sequence_iteration_source` for
  borrowed map iteration instead of formatting `.iter()` locally. Lowering still
  owns Java map-entry semantics and optional receiver unwrapping, while the
  Rust sequence iteration spelling is shared with stream/source iteration.
- 2026-07-26 map-entry next/access cleanup: `Map.Entry` accessor lowering still
  recognizes Java `entrySet().iterator().next().getKey/getValue` shapes in
  `lower.rs`, but iterator exhaustion and tuple-field Rust spellings now come
  from `coercion::{iterator_next_expect_projection, tuple_field_projection}`.
- 2026-07-26 enhanced-for iterator projection cleanup: enhanced-for lowering
  still decides when Java loop variables require cloned owned strings or copied
  primitive/static string elements, but the Rust `.iter().cloned()` /
  `.iter().copied()` spellings now come from `coercion` sequence projections.
- 2026-07-26 optional borrowed receiver cleanup: call lowering now represents
  already-borrowed optional receivers with
  `coercion::OptionalReceiverProjection::Borrowed` instead of formatting
  `.expect("Java null receiver")` locally. This keeps Java null-check receiver
  spelling beside the other optional receiver projections while preserving the
  lowering-side decision about when a native collection receiver is already an
  optional element borrow.
- 2026-07-26 erased-object display cleanup: Java `Object.toString()` fallback
  lowering and native `Object.toString` call lowering now call
  `JavaDisplayPlan::ErasedObject` instead of formatting
  `java_erased_object_to_string(&...)` directly in `lower.rs` or
  `call_lowering.rs`. The lowerer still decides when a source receiver lacks an
  overriding `toString`, while the Rust erased-display helper spelling remains
  centralized with string concat and display segment policy.
- 2026-07-26 formatted display cleanup: native `DecimalFormat.format(...)`
  lowering now calls `JavaDisplayPlan::FormatValue` instead of rendering the
  display operand and appending `.to_string()` in `call_lowering.rs`. The
  coercion layer owns the arithmetic-display cleanup plus final Rust string
  projection, while call lowering keeps only the Java overload arity check.
- 2026-07-26 string char-array projection cleanup: native
  `String.toCharArray()` lowering now calls
  `coercion::string_to_char_array_projection` instead of formatting
  `.chars().collect::<Vec<char>>()` in `call_lowering.rs`. Call lowering still
  selects Java `String.toCharArray` semantics, while coercion owns the Rust
  string-to-character-vector projection spelling.
- 2026-07-26 string char-at projection cleanup: native `String.charAt(...)`
  lowering now calls `coercion::string_char_at_projection`, which composes the
  existing `IndexToUsizeProjection::JavaStringCharAt` rule with the Rust
  `.chars().nth(...).expect(...)` spelling. Call lowering still validates Java
  arity, while coercion owns the index projection and bounds-panic expression.
- 2026-07-24 external typed-return follow-up: returned-call usage facts now
  carry arity and resolved call metadata, so signature constraints can prefer
  the resolved external owner key over an erased Rust receiver shape. When an
  external typed return fact matches the declared Java return type, the method
  return slot gets concrete dispatch while preserving nullable storage. External
  support rendering also keeps fact-proven typed erased-facade returns as named
  facade values rather than defaulting to `Box<dyn Any>`. Verified with
  `typed_feature_reader_header_cast_uses_external_pack_return_fact`,
  `object_return_from_typed_external_getter_erases_once_at_boundary`,
  `cargo fmt --check`, `cargo check --quiet`, and
  `cargo test --test type_coercion --quiet`.
- Source-summary companion fix: source-summary declared method return shapes
  now keep concrete dispatch for non-`Object` types whose external facts mark
  them as typed erased facades. This keeps late-added summary surfaces such as
  `Header.samHeader() -> Option<SAMFileHeader>` aligned with the support method
  renderer and avoids adding an invalid downcast after `unwrap()`. Verified
  with `source_summary_method_surface_can_be_added_after_external_return_selects_type`.
- External typed receiver companion fix: read-only external receiver bindings
  for non-`Object` erased-facade types now keep concrete native dispatch while
  still solving as shared-borrow parameters. This lets source methods that take
  `VCFHeader` and only call fact-backed VCF getters expose `&VCFHeader`
  instead of `&Box<dyn Any>`, while the return and argument slots still flow
  through external API facts. Verified with
  `external_pack_typed_string_list_getter_supports_indexed_sample_access`,
  `external_pack_typed_set_and_dictionary_getters_shape_facade_returns`, and
  `readonly_external_receiver_usage_can_borrow_public_helper_param`.
- 2026-07-25 rerun baseline: broad `cargo test --test idiom_gaps --quiet`
  reached 474 passed / 24 failed in 528.03s after the typed-return, typed
  receiver, BufferedReader, and nullable self-field getter slices. The nullable
  getter failures are gone from the red list.
- Static facade support dedupe: generated support now treats Rust inherent
  methods as sharing one namespace across static and instance receivers. A
  static facade method replaces an already-generated instance placeholder with
  the same Rust name and arity, fixing `Pattern::compile(...)` support for
  source `static final Pattern` fields without emitting duplicate `compile`
  definitions. Verified with
  `source_static_final_facade_fields_emit_associated_defaults_not_storage`.
- Source-summary inherited method follow-up: bodyless inherited source-summary
  methods now lower to typed boundary defaults instead of empty Rust bodies, and
  emitted child types suppress redundant support placeholders for inherited
  summary methods that are lowered into the child impl. Bodyless collection
  returns are treated as nullable when no stronger body/fact evidence exists,
  preserving `Option<HashSet<T>>` for inherited summary collection getters while
  scalar body-backed summaries keep their existing non-null surface. Verified
  with `emitted_type_extending_source_summary_super_gets_inherited_method_surface`
  and `source_summary_super_calls_lower_to_boundary_defaults_not_raw_super_receiver`.
- Borrow-fact scope note: external/facade ABI facts handle Rust API argument
  and receiver borrowing, but source-field reads inside constructors still use
  internal optional-field recovery before slot conversion. The focused
  constructor nested-argument regression now accepts the shared `.as_ref()
  .cloned().expect(...)` spelling and still rejects cloning `__java_self`.
  Constructor self-temporary compatibility checks now route through
  `coercion::rust_expr_mentions_constructor_self_temporary` and
  `coercion::constructor_self_field_replacement`, so `lower.rs` no longer
  renders expressions to search for `__java_self` or strips `.clone()` from
  self-field replacement locally.
  Follow-up: move constructor/source-field optional reads fully onto typed
  `LoweredType::OptionalElementBorrow` plus `plan_lowered_slot_conversion`, and
  audit/removal candidates for specialized borrow spelling rules once Rust
  signature facts cover the same slot shapes.
- Iterator ABI mutability slice: Java iterator cursor locals now become mutable
  because `Iterator.next()`/`JavaIterator.next()` is represented as a
  `FacadeAbiFact` with `ReceiverAbi::MutBorrowedSelf`. The local mutability scan
  now tracks declared local Java types while walking statements and checks the
  ABI owner before applying mutable-receiver facts, avoiding accidental
  name/arity-only matches for unrelated `next()` methods. Verified with
  `java_list_iterator_cursor_uses_platform_facade`.
- Throwable localized-message slice: `Throwable.getLocalizedMessage()` and the
  emitted snake-case `get_localized_message()` now share the same JDK
  string/nullable return fact as `getMessage()`. This lets exception logging
  lower through the nullable display slot as
  `e.get_localized_message().as_deref().unwrap_or("null")` and keeps generated
  support signatures type-correct. Verified with
  `java_exception_logging_surface_compiles_without_system_err_value`.
- Focused stale-assertion cleanup: the generated Rust for primitive array
  allocation, proven non-null member fields, string-array literal arguments,
  exception-constructor message stringification, `FileOutputStream` nullable
  field overloads, and Java string substring/hash-code helpers already matched
  the intended semantics under newer shared slot-conversion spelling. Their
  tests now assert the behavior rather than obsolete exact spellings or global
  support-surface substrings.
- Stream method-reference parity slice: static Java method references inside
  stream lambdas now reuse `CallTargetPlan` plus `ArgumentLoweringPlan` for
  single-input known native Java calls. `Integer::parseInt` and
  `Double::parseDouble` method references lower through the same parse helpers
  and borrowed string argument slots as ordinary calls, avoiding stream-local
  `Integer::parse_int(...)`/`Double::parse_double(...)` fallback support calls.
  Verified with `stream_static_method_references_use_call_target_plan`,
  `cargo test stream --test fixtures`, `cargo fmt --check`,
  `cargo check --quiet`, `git diff --check`,
  `cargo test corpus_spark_rdd_pipeline_keeps_stream_lowering_at_noop_spark_boundary --test idiom_gaps -- --exact`,
  `cargo test --test idiom_gaps --quiet` with 499 passing tests in 527.51s, and
  `cargo build --release --quiet --bin new-java2rust`.
- Stream unbound String method-reference follow-up: `String::length`,
  `String::trim`, and `String::isEmpty` stream method references now route
  rendering and output-shape inference through the ordinary native instance
  call planner. This keeps `String::trim` as a borrowed string-view result until
  collection recovery, where shared borrowed-element conversion emits
  `map(|v| v.to_string())` instead of invalid `.cloned()` recovery. Verified
  with `stream_string_method_references_use_call_target_plan`,
  `stream_plan_lowers_method_references_in_map_and_map_to_int`,
  `cargo test stream --test fixtures`,
  `cargo test method_reference --test slot_shape_design --quiet`,
  `cargo fmt --check`, `cargo check --quiet`, `git diff --check`,
  `cargo test method_reference --test idiom_gaps --quiet`,
  `cargo test --test idiom_gaps --quiet` with 499 passing tests in 526.48s, and
  `cargo build --release --quiet --bin new-java2rust`.
- Boxed primitive method-reference follow-up: `Integer::intValue`,
  `Double::doubleValue`, and `Float::floatValue` now participate in the same
  unbound native instance call-plan path as `String` method references for both
  stream rendering and output-shape inference. The stream receiver adapter
  dereferences borrowed iterator items before applying the primitive-number
  target, so `counts.stream().mapToInt(Integer::intValue).sum()` is no longer
  handled by the legacy method-reference fallback. The follow-up primitive
  number-value target now retains known source primitive facts, so no-op wrapper
  unboxing returns the receiver directly while real primitive conversions still
  emit casts. Verified with
  `stream_plan_lowers_method_references_in_map_and_map_to_int`,
  `cargo test method_reference --test slot_shape_design --quiet`,
  `cargo test method_reference --test idiom_gaps --quiet`,
  `boxed_primitive_constructors_lower_to_values`, `cargo test primitive --test
  idiom_gaps --quiet`, and `cargo test stream --test fixtures`.
- Stream `Object::toString` follow-up: method references such as
  `names.stream().map(Object::toString)` now probe the ordinary native instance
  call planner using the stream element's inferred Java type as the receiver.
  The rendered Rust stays compatible with the previous borrowed-item output
  (`(*value).to_string()` for borrowed `String` elements), but the stream path
  no longer needs the generic method-reference fallback when facts/types prove a
  native receiver call. Verified with
  `stream_plan_lowers_method_references_in_map_and_map_to_int`,
  `cargo test method_reference --test slot_shape_design --quiet`, and
  `cargo test stream --test fixtures`.
- Method-reference receiver-helper centralization: stream lowering now imports
  `java_type_for_unbound_method_reference_target` and
  `object_method_reference_receiver_type` from `method_reference_shape` instead
  of owning duplicate target-to-Java-type tables for `String`, boxed primitives,
  and `Object::...`. This keeps stream rendering and output-shape inference on
  one shared receiver-typing path while ordinary call planning still decides the
  actual native/static target. Verified with
  `method_reference_plan_owns_generic_function_adapter_shapes`,
  `cargo test method_reference --test idiom_gaps --quiet`, and
  `cargo test stream --test fixtures`.
- Native string method classification follow-up: `call_plan` now owns
  `native_java_string_instance_method(name, arity)`, and both typed/resolved
  native Java receiver planning and stream lambda receiver typing use that
  helper. Stream lowering no longer carries a separate
  `stream_native_string_method_requires_string_receiver` method-name table for
  `length`, `trim`, `split`, `charAt`, and adjacent string calls. Verified with
  `method_reference_plan_owns_generic_function_adapter_shapes`,
  `cargo test stream --test fixtures`,
  `cargo test method_reference --test idiom_gaps --quiet`, and
  `cargo test string --test idiom_gaps --quiet`.
- Method-reference accessor fallback boundary: the remaining unbound getter
  output-shape inference is now isolated as
  `infer_unbound_accessor_method_reference_fallback_shape`, and the guardrail
  asserts that `infer_method_reference_output_shape_from_call_plan` stays before
  the fallback path. This keeps `NameHolder::getName` compatibility explicit
  while making callable/result facts the preferred method-reference result-shape
  source. Verified with
  `method_reference_plan_owns_generic_function_adapter_shapes`,
  `cargo test method_reference --test idiom_gaps --quiet`, and
  `cargo test stream --test fixtures`.
- Batched optional rendering cleanup: optional wrapper rendering now normalizes
  string-view elements inside `coercion::render_option_some` for structured
  `SlotConversionPlan::OptionalConstruction`, while `Coercion::WrapOption`
  preserves its raw-expression ABI through `render_raw_option_some`. `lower.rs`
  no longer wraps slot conversion results in
  `normalize_optional_constructed_string_view_expr` or parses `Some(inner)`
  locally for assignment rewrap; it calls
  `unwrap_rendered_some_expr` from `coercion.rs` for the remaining legacy raw
  compatibility boundary. `OptionalShapeCoercionPolicy::MapElement` rendering
  is also isolated as `render_optional_element_projection`, keeping
  `.as_ref()`, `.as_deref()`, `.as_deref_mut()`, and `map(|value| ...)`
  spellings in the coercion layer. Focused verification:
  `rendered_string_heuristics_have_centralization_guardrails`,
  `cargo test optional --test type_coercion --quiet`,
  `cargo test optional_ --test idiom_gaps --quiet`.
- Optional argument projection cleanup: shared/mutable optional argument
  projection rendering now lives in `coercion::OptionalArgumentProjection` and
  `apply_optional_argument_projection`, so `lower.rs` decides only whether a
  target/actual slot pair needs optional element projection. The same slice
  moves borrowed method receiver parenthesizing into `coercion.rs`, reducing
  another rendered-borrow compatibility check in lowering. This is still a
  migration bridge until nullable carriers expose origins end-to-end, but new
  optional argument paths should use the shared projection helper instead of
  adding local `.as_ref()`/`.as_deref()`/`.as_mut()` formatting. Focused
  verification: `rendered_string_heuristics_have_centralization_guardrails`,
  `optional_element_borrow_carrier_construction_is_centralized`,
  `cargo test optional --test type_coercion --quiet`, and
  `cargo test optional_ --test idiom_gaps --quiet`.
- Same-call mutable alias cleanup: rendered compatibility checks for prior
  mutable aliases now live in `coercion::rendered_expr_is_mutable_alias`.
  `lower.rs` still owns the semantic same-call decision because argument order,
  source reads, and slot shapes are local to argument lowering, but it no longer
  owns the raw `&mut`, `.as_mut_slice()`, optional mutable projection, or
  mutable-slice spelling table. Reborrowed optional mutable arguments now use
  `coercion::reborrow_optional_mut_projection`, keeping `.as_deref_mut()`
  idempotence out of lowering as well. Indexed erased-object handoff clone
  recovery now uses `coercion::clone_erased_object_ref_once`, so idempotence for
  `java_clone_erased_object_ref`/`java_clone_erased_object_vec_ref` is kept with
  clone rendering helpers instead of argument lowering. Optional sequence
  receiver projection now uses `coercion::project_optional_sequence_receiver`,
  so the `.unwrap()` cleanup plus `as_ref/as_mut().expect("Java null receiver")`
  spelling is shared for read/write sequence access. Deserialization downcast
  message rewriting now uses `coercion::rewrite_downcast_expect_message`, so
  the `.downcast::<...>().expect("invalid Java cast")` rendered replacement is
  no longer a lowering-local rule. Reused cloned locals feeding borrowed
  argument slots now use `coercion::normalize_reused_clone_for_borrowed_argument`,
  keeping `name.as_str()`, `name.as_slice()`, `name.as_mut_slice()`, and
  `&name` recovery spellings beside clone-recovery idempotence helpers.
  Borrowed primitive argument dereference now uses
  `coercion::deref_borrowed_projection`, keeping `*expr` projection rendering
  with the other coercion helpers. The rendered-string
  guardrail now enforces zero `rendered.starts_with` checks and lower
  `rendered.ends_with`/`rendered.contains` ceilings in `lower.rs`. Focused verification:
  `rendered_string_heuristics_have_centralization_guardrails`,
  `cargo test alias --test idiom_gaps --quiet`,
  `cargo test optional_ --test idiom_gaps --quiet`, and
  `object_output_stream_write_object_clones_indexed_erased_object_handoff`,
  plus `cargo test sequence --test idiom_gaps --quiet` and
  `cargo test array --test idiom_gaps --quiet`, and
  `cargo test object_input_stream_read_object --test idiom_gaps --quiet`,
  `cargo test clone --test idiom_gaps --quiet`, and
  `cargo test alias --test idiom_gaps --quiet`, plus
  `cargo test primitive --test idiom_gaps --quiet`.
- Stream lambda external result-shape follow-up: stream lambda call-shape
  inference now asks the shared `external_call_lowered_type` query after native
  Java call-plan projection. This lets fact-backed external getters and facade
  returns set stream element result shapes without adding stream-local return
  tables, while native Java calls still retain precedence. Focused verification:
  `stream_shape_uses_shared_call_plan_for_lambda_call_results`,
  `cargo test stream --test fixtures`, and `cargo check --quiet`.
- Method-reference native receiver typing follow-up:
  `native_java_unbound_method_reference_receiver_type` now lives in
  `call_plan`, so stream rendering and method-reference output-shape inference
  consume the same native receiver classification instead of keeping the
  `String`/boxed-primitive whitelist in `method_reference_shape`. Focused
  verification: `method_reference_plan_owns_generic_function_adapter_shapes`,
  `cargo test method_reference --test idiom_gaps --quiet`, and
  `cargo test stream --test fixtures`.
- Stream optional string field argument follow-up: the stream path still maps
  bare Java field references to `self.field`, but optional string field
  arguments now use `plan_lowered_slot_conversion` for borrow, unwrap, and
  string-view rendering instead of formatting `.as_deref().expect(...)`
  directly. Focused verification: `cargo test stream --test fixtures` and the
  focused stream-plan fixture covering optional field arguments.
- Stream borrowed-string parameter follow-up: read-only borrowed string
  argument facts in stream lambdas now normalize borrowed lambda/local string
  parameters to string views before generic slot conversion can apply owned
  copy recovery. This keeps fact-backed calls such as
  `GenomicsDBUtils.getArrayColumnBounds(workspace, partition)` from rendering
  invalid `*partition.as_str()` while still letting optional field arguments use
  shared slot conversion for unwrap and string-view projection. Focused
  verification: `stream_plan_lowers_flat_map_block_temporary_collection_as_owned_subpipeline`,
  `cargo test stream --test fixtures`, and
  `cargo test clone --test idiom_gaps --quiet`.
- Stream joining delimiter follow-up: `Collectors.joining(delimiter)` no longer
  appends `.as_str()` directly to arbitrary rendered delimiter expressions. The
  delimiter expression is rendered once, shaped through `ShapeEnv`, and adapted
  with `plan_lowered_slot_conversion` to a borrowed string argument slot, so
  nullable string fields render as
  `self.delimiter.as_ref().expect("Java null receiver").as_str()`. Focused
  verification:
  `stream_joining_delimiter_uses_slot_conversion_for_optional_string_field`,
  `rendered_string_heuristics_have_centralization_guardrails`, and
  `cargo test stream --test fixtures`.
- Constructor/facade result follow-up: deferred constructor self-value result
  propagation now delegates the uncaught Java exception unwrap rendering to
  `coercion::unwrap_result_or_panic_uncaught_java_exception`, so `lower.rs` no
  longer owns the local `strip_suffix('?')` plus panic formatting rule.
  Rust-crate mapped external constructors now also carry their facade Rust
  result type through lowered slot conversion when used in return/assignment
  slots, and method return rendering consults explicit Rust-crate type facts
  before falling back to local Java support stubs. Stale constructor tests were
  updated for direct borrowed string literals (`"id"` instead of allocating a
  temporary string) and the explicit erased HTSJDK `IntervalList.add` boundary.
  Focused verification:
  `rendered_string_heuristics_have_centralization_guardrails`,
  `cargo test constructor --test slot_shape_design --quiet`,
  `cargo test constructor --test idiom_gaps --quiet`, `cargo check --quiet`,
  and `cargo fmt --check`.

- Optional receiver projection cleanup: nullable receiver unwrap/projection now
  uses `coercion::project_optional_receiver` for shared and mutable receiver
  views, and `project_optional_sequence_receiver` delegates to the same helper.
  This removes direct `as_ref/as_mut().expect("Java null receiver")` formatting
  from `lower.rs` for array, map, sequence, enhanced-for, and field receiver
  paths while keeping the null-receiver policy centralized until optional
  receivers fully participate in typed slot plans. Focused verification:
  `rendered_string_heuristics_have_centralization_guardrails`,
  `cargo test optional_receiver --test idiom_gaps --quiet`,
  `cargo test optional_receiver --test slot_shape_design --quiet`,
  `cargo test sequence --test idiom_gaps --quiet`,
  `cargo test map --test idiom_gaps --quiet`,
  `cargo test array --test idiom_gaps --quiet`, and
  `cargo test enhanced_for --test slot_shape_design --quiet`.
- Map-key borrowing cleanup: the remaining map-key idempotence check for a
  pre-rendered `&name` borrow now uses
  `coercion::collapse_redundant_shared_borrow_projection`, so `lower.rs` keeps
  the semantic shape decision but no longer owns the rendered `&name`
  compatibility spelling. Focused verification:
  `rendered_string_heuristics_have_centralization_guardrails` and
  `cargo test map --test idiom_gaps --quiet`.
- Direct projection cleanup: forced shared borrows, enum-variant borrowed
  dyn-trait views, and optional shared-value unwraps now render through
  `coercion::force_shared_borrow_projection`,
  `coercion::borrow_as_dyn_trait`, and
  `coercion::unwrap_optional_shared_value`. The lowerer still decides when a
  place, enum value, or nullable display branch needs that projection, but it no
  longer owns the raw `&expr`, `&expr as &dyn Trait`, or
  `expr.as_ref().unwrap()` spelling. Focused verification:
  `rendered_string_heuristics_have_centralization_guardrails`,
  `closed_enum_variant_borrows_as_dyn_trait_parameter`,
  `indexed_string_array_into_optional_borrowed_string_arg_borrows_element`,
  `two_dimensional_row_replacement_lowers_to_ndarray_row_mut_assign`, and
  `cargo test optional_display --test type_coercion --quiet`.
- Call-boundary borrow cleanup: Java `String(byte[])` and byte-range
  constructor lowering now uses `coercion::force_shared_borrow_projection`
  instead of formatting `&bytes` inside `call_lowering.rs`. This keeps the JDK
  constructor special case focused on Java overload semantics while the borrow
  spelling stays in the shared coercion layer. Focused verification:
  `rendered_string_heuristics_have_centralization_guardrails` and
  `byte_array_local_with_spaced_dimensions_feeds_string_range_constructor`.

Stream lambda actual-shape follow-up: `stream_lambda_arg_actual_shape` no longer
invents `JavaType::String` for unknown lambda params or block locals. Stream
argument adaptation now threads the current lambda `param_shape` through call
argument planning, block-prefix rendering, and borrowed-string recovery, so
lambda parameters use the inferred stream element shape while locals require
`ShapeEnv` facts instead of silently assuming `.as_str()` is valid. Untyped
borrowed-string argument fallback now leaves the rendered expression alone
instead of appending `.as_str()` without an actual shape. Focused verification:
`stream_lambda_argument_shapes_do_not_invent_string_fallbacks`,
`stream_plan_lowers_flat_map_block_temporary_collection_as_owned_subpipeline`,
`parallel_stream_reduce_uses_shared_stream_lowering`, and
`cargo test stream --test fixtures`.
Follow-up: `render_stream_lambda_arg_to_slot_through_plan` now uses a default
fact set when no stream context facts are attached, so known lambda/local/field
actual shapes still enter `plan_lowered_slot_conversion` before the final
borrowed-string fallback. The stream-only optional string field argument helper
was removed; optional string fields now use the same slot-conversion helper as
other known stream lambda arguments.

Stream lambda projection cleanup: stream-specific `*item`, `**item`, `(*item)`,
and `(**item)` renderings now live behind
`render_stream_lambda_value_expr`, `render_stream_lambda_receiver_expr`, and
`rendered_stream_lambda_borrowed_item_expr`. Stream lowering still owns these
iterator-item projections because ordinary call slot facts do not yet model the
closure binding layer, but the spelling table is no longer duplicated across
lambda body rendering, method-reference fallback rendering, native instance
method-reference receivers, and borrowed-parameter move recovery. Focused
verification: `method_reference_plan_owns_generic_function_adapter_shapes`,
`stream_plan_lowers_method_references_in_map_and_map_to_int`, and
`cargo test stream --test fixtures --quiet`.
Follow-up: stream lowering still decides which lambda/local/field values must be
cloned because that depends on closure binding context, but the Rust `.clone()`
projection itself now goes through `coercion::apply_clone_projection` via
`render_stream_lambda_clone_projection`. This removes the remaining stream-local
generated-expression `format!("{rendered}.clone()")` /
`format!("{param}.clone()")` spellings without pretending ordinary call slot
facts model stream iterator bindings yet.
Follow-up: nullable stream map lambdas still detect Java block guards in
`stream_lowering.rs`, but the Rust optional carrier body now renders through
`coercion::render_nullable_stream_map_body`. This keeps the `None`/`Some(...)`
projection for fused nullable-map plus non-null-filter pipelines beside the
other optional/stream projections while stream lowering owns only guard/local
analysis.

Index projection cleanup: call and stream lowering now share
`coercion::render_index_to_usize_projection` with explicit
`IndexToUsizeProjection::{JavaStringCharAt, StreamLambdaArgument}` modes. This
keeps the different legacy parenthesization requirements for `String.charAt`
and stream lambda slot arguments visible as policy, while removing local
`render_string_char_at_index`/`render_stream_lambda_index_arg` helpers and their
duplicated `as usize` suffix checks. Focused verification:
`rendered_string_heuristics_have_centralization_guardrails`,
`syntax_slot_coercion_policy_owns_index_condition_and_concat_segments`,
`stdlib_signature_facts_drive_string_index_argument_slot`, and
`stream_lambda_string_char_at_uses_signature_index_slot`.
Follow-up: stream lambda array access now uses
`coercion::render_indexed_access_projection`, which reuses the same
`IndexToUsizeProjection::StreamLambdaArgument` policy instead of formatting
`array[index as usize]` locally. Focused verification:
`method_reference_plan_owns_generic_function_adapter_shapes`,
`stream_plan_lowers_flat_map_block_temporary_collection_as_owned_subpipeline`,
and `stream_lambda_lowers_split_length_and_integer_parse_in_nullable_map`.

Optional borrowed display cleanup: `Map.get(...)` values used in string
concat/display slots now render through
`JavaDisplayPlan::OptionalBorrowedValue`, keeping the `Option<&T>` Java-null
display spelling in `coercion.rs` instead of formatting
`.map(|value| value.to_string()).unwrap_or_else(...)` in `lower.rs`. This keeps
stored `Option<T>` display (`OptionalValue`) and borrowed map lookup display as
separate explicit policies. Focused verification:
`rendered_string_heuristics_have_centralization_guardrails`,
`optional_platform_facade_display_segments_use_display_value_policy`, and
`map_get_string_display_segment_unwraps_optional_borrow`.

Java Optional owned-value projection cleanup: call lowering still selects
`Optional.get()` and `Optional.orElse(...)` as native Java Optional methods, but
the Rust `Option<T>` owned-value spelling now lives in
`coercion::optional_get_owned_value_projection` and
`coercion::optional_or_else_owned_value_projection`. This keeps
`.as_ref().map(|value| (*value).clone()).expect(...)` / `unwrap_or(...)`
compatibility rendering beside the other optional/coercion policies instead of
inside `call_lowering.rs`.

Optional facade receiver ABI cleanup: explicit `FacadeAbiFact` receiver calls now
adapt optional receivers through `facade_receiver_slot_plan` and
`plan_lowered_slot_conversion` instead of falling back to local `.as_ref()` /
`.as_mut()` branches first. The conversion layer now has a distinct mutable
optional unwrap projection (`UnwrapOptionMutBorrowed`) and suppresses the
follow-up ownership borrow that the projection already performs, while still
allowing element view conversions. Focused verification:
`optional_owned_to_mutable_borrow_uses_mutable_optional_projection`,
`optional_external_receiver_call_uses_facade_mapping_after_unwrap`,
`optional_external_mutating_receiver_call_uses_mutable_unwrap`, and
`rendered_string_heuristics_have_centralization_guardrails`.

Optional argument projection cleanup: shaped and explicit-slot argument lowering
now use one `lower_optional_argument_for_borrowed_target` helper for
`Option<T>` feeding borrowed required targets, borrowed optional targets, and
optional array/slice targets. The old `adapt_optional_shared_borrow_argument`,
`adapt_optional_mut_borrow_argument`, `lower_optional_array_argument_for_slice_shape`,
and `OptionalArgumentProjection` render bridge were removed; optional argument
projection is now selected by `LoweredType::OptionalElementBorrow` plus
`plan_lowered_slot_conversion`/borrowed-element slot adapters. Optional
element carrier projection now delegates the actual `.as_ref()`/`.as_mut()`
spelling to `coercion::project_optional_element_borrow_carrier`, leaving
`lower.rs` responsible only for choosing the optional-element lowered type and
ownership. Focused
verification: `optional_element_borrow_carrier_construction_is_centralized`,
`rendered_string_heuristics_have_centralization_guardrails`,
`cargo test optional --test type_coercion --quiet`, and
`cargo test optional_ --test idiom_gaps --quiet`.

Map-key borrow projection cleanup: map lookup lowering now gathers only the
local variable name and inferred key shape, then delegates idempotent map-key
borrowing to `coercion::normalize_map_key_borrow_projection`. This removes the
lowering-local `avoid_double_borrowed_map_key_arg` /
`ensure_borrowed_map_key_arg` split while keeping Java collection key borrowing
as one shared projection policy.

Varargs borrow projection cleanup: expanded Java varargs packing still happens
in `lower.rs`, but borrowed varargs parameters now pass the packed vector
through `coercion::force_shared_borrow_projection` instead of formatting
`&packed_vec` locally. This keeps the lowering code responsible for Java
varargs shape and the coercion layer responsible for Rust borrow spelling.

Varargs vector projection follow-up: Java varargs arity/spread selection still
belongs to `lower.rs`, but the Rust packed-vector literal and spread-extension
block now render through `coercion::{varargs_packed_vec_projection,
varargs_spread_extend_projection}`. This keeps `Vec::new()`, `vec![...]`, and
the `__java_varargs.extend(...)` block beside the other Rust projection
spellings while preserving the existing Java varargs semantics.

Native collection constructor projection cleanup: `new ArrayList`, `new
HashSet`, and `new HashMap` lowering still uses `NativeCollectionNewTarget` for
Java constructor semantics, but Rust constructor spellings now live in
`coercion::{native_collection_empty_new_projection,
native_collection_with_capacity_projection,
native_collection_default_constructor_projection}`. This removes the local
native collection type-name table from `call_lowering.rs`.

Collection remove-if projection cleanup: Java lambda extraction and predicate
lowering for `Collection.removeIf(...)` still happen in `call_lowering.rs`, but
the Rust `retain(|item| !(predicate))` spelling now lives in
`coercion::collection_remove_if_projection`.

Boundary projection cleanup: borrowed-sequence subtype recovery, exception
self-as-supertype facade fallback, and explicit Java facade-cast placeholder
lowering still make their semantic selection decisions in `lower.rs`, but their
Rust constructor/default/cast placeholder spellings now live in `coercion`
projection helpers. This keeps another set of generated-Rust templates out of
expression lowering while preserving current conservative TODO markers.

Optional/dyn boundary projection cleanup: typed erased-object `None`, anonymous
dyn-trait construction, and nullable Java resource-stream unwraps now render
through `coercion::{typed_none_projection, anonymous_dyn_trait_projection,
unwrap_java_null_receiver_projection}`. Lowering still decides when those Java
boundary cases apply, but their Rust spelling no longer lives in local raw
`format!` branches.

Trait-object boundary projection cleanup: current-self recovery into erased
object or boxed dyn-trait slots now reuses `coercion::{erased_object_box_projection,
boxed_dyn_trait_projection}` after applying the shared audited clone recovery.
Unsupported Java deserialization interface casts now use
`coercion::unsupported_deserialization_cast_handoff_projection`, keeping the
handoff marker in the projection layer while preserving the existing explicit
serialization-facade TODO behavior.

Expression projection cleanup: native print stream macros, do-while condition
negation, Java pre/post increment expression blocks, and source-subtype
`.into()` recovery now render through `coercion::{print_macro_projection,
logical_not_projection, java_update_expression_projection, into_projection}`.
Lowering still selects the Java statement/expression semantics, but these
mechanical Rust spellings are no longer embedded in local raw `format!` arms.

Call/member path projection cleanup: ordinary call-target lowering and member
access lowering now use shared `coercion` path helpers for `Self::member`,
`Owner::member`, `JavaClass::<()>::member`, external crate functions,
external crate associated constructors, static storage accessors, and associated
constants. Planning still decides the Java owner/receiver semantics, while the
Rust path spelling now has one projection surface that later callable-key and
facade ABI work can replace or enrich. The same slice tightened member-access
planning so untyped lowercase receivers with lowercase fields no longer match
procedural external/static field guesses; static fallback now requires a
static-looking member or a receiver that is already shaped like a type name.
Follow-up: simple native Java instance methods such as string view/content
methods, `Optional.isPresent`, and `MessageFormat.format` now also render
through `coercion::receiver_method_call_projection` instead of constructing
`RustExpr::MethodCall` directly in `call_lowering.rs`. Focused verification:
`rendered_string_heuristics_have_centralization_guardrails`.
Follow-up: the generic `apply_call_target` path now wraps selected function
paths and receiver methods through
`coercion::{function_call_projection, receiver_method_call_projection_exact}`
instead of constructing `RustExpr::Call`/`RustExpr::MethodCall` locally for
the common static, receiver, self, free, and Java-class call targets. Focused
verification: `apply_call_target_uses_shared_call_projection_helpers` and
`rendered_string_heuristics_have_centralization_guardrails`.
Latest follow-up: the same generic call projection helpers now cover external
crate receiver calls, free/associated facade receiver calls, native collection
receiver methods, class-literal token calls, external crate constructors, and
ordinary associated constructors in `call_lowering.rs`. Java target selection
and facade ABI receiver policy remain in call planning; only Rust call spelling
is centralized.
Projection follow-up: `String.join` receiver calls, constructor
method-reference bodies, and delegating constructor return calls now also use
the shared function/receiver call projections. The Java varargs/method
reference/delegation semantics remain in their existing planners; this only
removes another set of local `RustExpr::Call`/`MethodCall` constructions.
Follow-up: bare source instance-field access now uses
`coercion::self_field_projection` for both structured lowering and rendered
fallbacks. `access_lowering.rs` still owns local-vs-field-vs-static-name
resolution, but the Rust `self.field` spelling is centralized with the other
member/path projections.
Follow-up: planned and fallback receiver field access now use the generic
`coercion::field_projection(receiver, field)` helper. Member-access planning
still decides whether a Java access is a Rust field, static constant, or length
projection, but `access_lowering.rs` no longer constructs `RustExpr::Field`
directly.

Method-reference projection cleanup: generic method-reference lowering now
shares the same associated-path, unbound-method, current-receiver-method, and
typed single-input callback projection helpers used by call/member cleanup.
Typed stream callback lowering no longer rebuilds `|value: T| ...` callback
forms directly in `lower.rs`; it asks `method_reference_lowering` to render the
planned method reference. The remaining semantic work is to make the
method-reference plan itself resolve through callable keys/facade ABI facts
instead of keeping Java owner/method special cases in the method-reference
shape/lowering helpers.

Lowerer path projection follow-up: delegated constructor calls, static fallback
calls, enum/static constant labels, and no-argument default constructors now
reuse the same `coercion` path projections from `lower.rs` and
`item_assembly.rs`. This keeps Rust path construction centralized while those
modules continue to own Java constructor delegation, field initialization, and
fallback selection semantics.
Follow-up: the same lowerer paths now wrap delegated constructor calls, `Ok`
result returns, no-argument default constructor expressions, URI parse
constructors, and static self fallbacks with `function_call_projection`.
Lowering still chooses those Java cases; the Rust call node construction is
shared with ordinary call lowering.
Method-call projection follow-up: lowerer trailing constructor instance calls,
context trailing calls, optional target clearing, resource stream support,
graphics creation, string switch selectors, try-resource cleanup, same-object
calls, boxed primitive parse helpers, static storage setters, and local
callback invocations now use shared function/receiver call projections.
Lowering still selects the Java-specific cases and exception propagation; the
Rust call or method-call node construction is centralized.
Optional/array receiver projection follow-up: refined optional unwraps,
`Objects.isNull`/`Objects.nonNull` optional predicates, and ndarray row access
now also use `receiver_method_call_projection_exact`. Lowering still chooses
when those Java semantics apply; receiver-call spelling is shared.

Loop/array borrow projection cleanup: assignment-in-condition loops now use the
shared optional shared-value unwrap projection when introducing the narrowed
loop-local binding, and borrowed 2D-array temporaries now enter the shared
borrow projection helper instead of formatting `&...`/`&mut ...` in
`array_lowering.rs`. This is another small step toward routing nullable and
borrowed ABI surfaces through slot/projection facts rather than local rendered
string choices.

Rendered parser helper cleanup: raw Rust generic type argument parsing moved
from `item_assembly.rs` into `types.rs`, and rendered `vec![...]` body
extraction moved from `array_lowering.rs` into `coercion.rs`. These remain
compatibility helpers while raw `RustExpr`/`RustType` strings are still common,
but their parsing behavior is no longer owned by local lowering/assembly
modules.

Static final const assembly follow-up: `const_expr_value` now uses shared
associated-path projections for `Self::CONST` and `Owner::CONST` references.
Item assembly still owns constant eligibility and evaluation, but the Rust path
spelling is no longer rebuilt locally for these constant expressions.

Expression constructor projection follow-up: `expr_render::named_constructor_expr`
now uses the shared owner-associated path and function-call projections. The
expression rendering helper still owns the basic Rust expression shell API, but
constructor path/call spelling is centralized with ordinary call cleanup.

Loop null-test projection follow-up: assignment-in-condition loop lowering now
uses the shared receiver-method-call projection for generated `is_none()` /
`is_some()` break checks. Loop lowering still owns the Java control-flow rewrite
shape and optional unwrap insertion, but receiver call assembly is shared.

Stream receiver type-hint cleanup: stream lambda lowering now asks
`call_plan::native_java_instance_receiver_type_hint` for the temporary receiver
type implied by native Java instance-call planning, instead of checking the
string-method table directly. This keeps the remaining stream fallback path
aligned with ordinary call planning while full stream lambda ABI planning is
still being migrated.

Method-reference receiver inference cleanup: unbound method-reference receiver
typing now goes through
`method_reference_shape::method_reference_receiver_type_for_call_plan`. Stream
lowering no longer composes native method-reference receiver hints with the
`Object::...` fallback locally; it asks the shared method-reference shape layer
before planning the call target.

Stream static path projection cleanup: stream lowering still owns stream-local
static owner detection for lambda/raw expressions, but the final
`Owner::member` spelling now goes through the shared associated-path projection
helper. This keeps stream static constant rendering aligned with ordinary
call/member path projection cleanup while callable-key ownership resolution is
still being migrated.

Stream lambda fallback call projection cleanup: fallback static/current-receiver
lambda calls now build `RustExpr::Call`/`MethodCall` through
`owner_associated_path_projection`, `self_associated_path_projection`, and
`current_receiver_method_call_projection`. Stream lowering still owns fallback
selection and lambda context while the emitted Rust call shape is shared.
Follow-up: stream fallback static calls now also use the generic
`function_call_projection` wrapper rather than constructing `RustExpr::Call`
directly. Stream lowering still selects resolved-static versus owner-fallback
semantics; only the final Rust call node construction is shared.
Follow-up: stream lambda constructor fallback rendering now also wraps
already-rendered stream arguments in shared owner-associated path and
function-call projections. Stream lowering still owns lambda-local argument
substitution and external constructor arity selection.
Follow-up: borrowed-string stream lambda arguments now prefer the shared
`plan_lowered_slot_conversion` argument path before using the remaining
param/local `.as_str()` compatibility fallback. This keeps existing lambda
binding recovery intact while making slot facts the first authority for
String-to-`&str` argument adaptation.
Optional borrowed receiver plan follow-up: already-borrowed optional receiver
carriers (`Option<&T>` / `Option<&mut T>`) now enter
`optional_borrowed_receiver_slot_plan` in `call_plan` before projection and slot
conversion. The shared adapter still performs receiver expression discovery, but
`lower.rs` no longer hardcodes the `OptionalReceiverProjection::Borrowed`
choice for this carrier.
Thread sleep ABI follow-up: `java.lang.Thread.sleep(long)` now has a selected
stdlib `FacadeAbiFact` with an owned/copy millisecond argument. Native lowering
still owns Java sleep semantics and renders `Duration::from_millis(...)`, but
argument shape selection no longer depends only on the `Thread.sleep` lowering
branch.
System exit ABI follow-up: `java.lang.System.exit(int)` now has a selected
stdlib `FacadeAbiFact` with an owned/copy status argument. Native lowering still
owns Java exit semantics and renders `std::process::exit(...)`, but argument
shape selection no longer depends only on the `System.exit` lowering branch.
Math static ABI follow-up: fixed-arity `java.lang.Math` statics now have
selected stdlib `FacadeAbiFact`s for `max`, `min`, `log`, `exp`, and
`toIntExact` on both simple and fully qualified owners. Native lowering still
owns Java-specific projections, but numeric argument shape selection is no
longer tied only to the native static-call branch. Vararg/string formatting
calls remain separate because their callable-key arity policy needs a deliberate
design.
Objects equality ABI follow-up: `java.util.Objects.equals(Object, Object)` now
has a selected stdlib `FacadeAbiFact` with optional borrowed operands, matching
the existing `Objects.nonNull/isNull` null-carrier facts. Native lowering still
owns the equality projection, but nullable operand borrowing is now selected
from shared slot facts.
Batch-boundary verification note: after the `System.exit`, fixed-arity `Math`,
and `Objects.equals` ABI fact slices, focused tests and `cargo check --quiet`
passed, but the full `cargo test --test slot_shape_design --quiet` gate is
still red with 454 passed / 55 failed. The failures span pre-existing broad
design-drift guards in representation, signature, stream, semantic-analysis,
and lowering ownership tests, so treat the full design suite as a separate
cleanup batch rather than a blocker for these committed ABI fact slices.
Checkpoint policy: the user explicitly granted permission to create git commits
at good checkpoints during larger batched edits. Keep commits narrow enough to
audit, record focused verification for each checkpoint, and continue ignoring
unrelated untracked local files.
Call target ownership cleanup: lower-side `CallTargetPlan::InstanceReceiver` and
`FallbackReceiver` variant construction/classification has moved behind
call-plan helpers (`non_self_trivial_accessor_target`,
`retarget_static_call_to_receiver_if`, `rust_method_name`, and
`uses_receiver_argument_lowering`). The focused design guard now uses the
arity-aware call-target planner for argument-bearing Java string methods, which
matches real lowering and keeps native Java string semantics from depending on a
zero-arity compatibility path.
Conversion-lowering ownership cleanup: `conversion_lowering.rs` no longer
depends on `FactSet` or calls slot-conversion planners while rendering optional
slot construction. Facts-aware optional construction and optional assignment
rewrap decisions now happen in lower-side planning wrappers, which pass
`OptionalSlotConstructionPlan` / `SlotExprConversionPlan` into renderer helpers.
Focused coverage includes the conversion-plan ownership guard and the optional
storage, erased handoff, and assignment rewrap construction-plan guards.
Invalid-cast policy naming cleanup: lower-side assignment slot helpers no
longer reuse the `assignment_slot_for_target` planner name, preserving the
separation where `conversion_plan` owns slot policy and invalid-cast runtime
policy while lowering only asks for a target assignment slot in current-type
context.
Cast/instanceof guard cleanup: the design guard for cast and `instanceof`
lowering now checks the actual expression-lowering match branch and accepts
`plan_cast_conversion_from_actual`, which is the current API when lowering has
already computed a precise actual shape. Broader lower-side hierarchy queries
remain tracked as semantic-facts cleanup rather than being conflated with
cast/instanceof plan rendering.
Operator lowering cleanup: numeric binary promotion now keeps structured
`RustExpr::Binary` and primitive widening uses structured `RustExpr::Cast`
instead of rendering a raw parenthesized string. String equality only uses the
string-view plan when a borrowed string view is actually present, and optional
string view projection canonicalizes to `.as_deref()`.
Signature naming cleanup: Java named-type to Rust identifier normalization now
lives in neutral `java_naming` with a compatibility wrapper in `slot_render`.
`signature.rs` no longer depends on `slot_render` for closed-enum dispatch
requirements, keeping signature planning free of Rust type rendering APIs.
Static storage default cleanup: static-storage accessor generation no longer
renders fallback slot defaults directly in `lower.rs`. Default initial values
now go through `item_assembly::static_storage_initial_fallback_value`, keeping
slot default rendering behind the same item-assembly/slot-render boundary used
by constructor default fields. The lower design guard is scoped to the static
storage initializer path while broader `slot_render` references in lowering are
handled by later ownership slices.

Native collection optional receiver cleanup: optional Java collection receiver
adaptation still verifies that the nullable payload is a known Java collection,
but mutable/shared receiver projection now delegates to
`adapt_optional_instance_receiver_with_slot_plan`. This removes the native
collection-specific optional projection table and keeps unchecked legacy unwrap
behavior behind the same typed slot-plan adapter used by ordinary optional
receivers.

Facade return text classifier cleanup: support-facade `Result<T, E>` and
`Option<T>` wrapping decisions now use shared `types.rs` raw Rust generic
classifiers instead of local `starts_with("Result<")`/
`starts_with("Option<")` checks in the facade return helpers. This is still a
boundary parser while Rust signatures are not fully structured everywhere, but
the wrapper decisions are no longer reimplemented at each facade return branch.
Follow-up cleanup extended the same classifier path to support-facade opaque
return tracking, nested `Option<T>` return normalization, and default fallback
expression generation, preserving the previous JavaException-specific
`Result<T, JavaException>` behavior while avoiding local prefix parsing.
The CLI facade-signature parser now also uses the shared raw Rust generic
helpers for `Result`, `Option`, nullable parameters, return payloads, and
`FacadeSlot::Optional` detection, including path-qualified `...::Result<T>`
without the previous broad `contains("Result<")` heuristic.
Borrowed facade return recovery follow-up: erased-object recovery from borrowed
Rust facade returns no longer guesses `ToPathBuf` by matching rendered Rust
type names like `Path`/`PathBuf`. `FacadeAbiFact.borrowed_return_recovery`
selects `Clone` vs `ToPathBuf`; missing facts now default to `Clone` so
non-clone/path-specific behavior is exposed as a manifest/facade ABI gap rather
than hidden behind a type-string heuristic.
Raw struct clone-support checks in `item_assembly.rs` now reuse the same
generic parsing helpers for `Option<T>`, `Vec<T>`, and `Box<dyn Trait>` instead
of local prefix/suffix tests. This keeps the remaining raw Rust type parsing
behind one boundary while typed Rust ABI facts continue to replace these
compatibility paths.
External-pack manifest facade slot parsing now uses the shared `Option<T>`
classifier as well, and fluent self-returning facade support checks `Result`
through the shared classifier instead of local string prefixes.
Opaque final facade return handling now uses a shared raw generic head parser
for compatibility names like `ObjectAssert<T>`, replacing another local
`split_once('<')`/suffix check while the long-term target remains structured
facade ABI facts.
Stream lambda argument adaptation now classifies target argument slots through
an explicit `StreamLambdaArgumentSlotPlan` before selecting borrowed-string
view, owned-value, or generic slot-conversion rendering. Stream lowering still
owns closure item projection semantics, but the call argument adapter no longer
branches directly on borrowed-string/owned shape predicates.
Same-call mutable alias detection still remains local until global sequencing
analysis can prove read/write ordering, but the rendered compatibility boundary
now returns a named `RenderedMutableAliasProjection` instead of exposing an
anonymous boolean over optional receiver and mutable-slice spellings.
Optional receiver slot planning now also carries and constructs the optional
element borrow carrier used for receiver adaptation. `lower.rs` still owns the
expression-shape lookup and legacy unchecked projection choice, but it consumes
`OptionalReceiverSlotPlan::optional_element_borrow_type` instead of
reconstructing the shared-vs-mutable carrier decision from the local `mutable`
flag.
Stream lambda receiver-call fallback still runs only after ordinary
`CallTargetPlan` handling declines, but the final compatibility rendering now
uses `coercion::receiver_method_call_projection` instead of locally formatting
`receiver.method(args)` in `stream_lowering.rs`.
Method-reference associated-function body compatibility cases are now isolated
behind `AssociatedFunctionMethodReferenceBodyPlan`. The plan still has legacy
Java owner/method cases for `Integer::intValue`, `Object::toString`,
`String::valueOf`, and `Objects::nonNull`, but the public renderer consumes one
named body-plan boundary that can later be replaced by callable-key/facade ABI
facts.
Follow-up: the body plan still lives in method-reference lowering because it is
the compatibility boundary for Java owner/method meaning, but the Rust body
spellings now render through
`coercion::associated_function_method_reference_body_projection`. This keeps
primitive identity, receiver `to_string`, always-true, and static function
callback bodies beside the other projection helpers while callable-key/facade
ABI work replaces the legacy cases.
Follow-up: typed method-reference callbacks now have a fact-aware rendering
entry point. Optional `Objects::nonNull` callbacks are selected from the
`Objects.nonNull` facade ABI fact and render `value.is_some()` through the
shared receiver-method projection; non-optional callbacks keep the conservative
legacy `true` body.
Latest follow-up: the same typed callback entry point now also probes
receiver-method `FacadeAbiFact` rows using the inferred method-reference
receiver type. `String::toString` and `Object::toString` over known `String`
inputs can therefore render through the selected Rust ABI method projection
instead of only through the legacy `ReceiverToString` body plan. Unsupported
cases such as generic `Object::toString`, `String::valueOf(Object)` null
semantics, and boxed primitive `intValue` remain on their existing Java semantic
paths until their ABI/fact rows can express the needed behavior precisely.
Shape follow-up: method-reference output-shape inference now uses the same
fact-backed receiver-type proof for unbound facade receiver methods and asks
`external_call_lowered_type` for facade return slot interpretation once the
ordinary call planner selects an `ExternalFacadeReceiver`. This lets external
facade references such as `FacadeValue::label` derive both callback rendering
and return shape from the same `ExternalApiEffectFact`/`FacadeAbiFact` pair.
Accessor-shape follow-up: unbound accessor-looking method references now prefer
typed zero-arg external getter facts before using the legacy `get_...` fallback.
When an `ExternalApiEffectFact` proves `ExternalRecord::getName -> String`,
method-reference output-shape inference uses `external_call_lowered_type` to
derive the precise return shape; the name-shaped `Unknown("get_name")` fallback
remains only for factless source-shaped references.
Receiver-autoref follow-up: the predicates that decide whether Rust method-call
autoref already satisfies a planned facade or optional receiver slot moved from
`lower.rs` into `conversion_plan`. Lowering still selects Java receiver context
and optional receiver shape, but it no longer matches conversion-policy internals
locally to decide whether to skip an explicit borrow/projection.
JavaBean naming follow-up: getter-name parsing (`getX`/`isX` -> field `x`) now
lives in `java_naming` instead of `lower.rs`. Lowering still chooses when a
getter-shaped fallback is semantically allowed, but the Java naming convention
is centralized for source getters, method-reference fallback, and future fact
normalization work.
Method-reference alias follow-up: method-reference callable-key probes and
output-shape receiver proofs now share `java_naming::java_method_name_aliases`
instead of keeping separate snake/camel alias lists in lowering and shape
inference. The aliases are still explicit compatibility candidates; resolved
owner/type identity remains required before facts are selected.
Accessor fallback naming follow-up: the legacy method-reference accessor shape
fallback now asks `java_naming::java_accessor_method_reference_fallback_name`
instead of matching `get`/`get_...` locally in shape inference. This preserves
the compatibility fallback while making the remaining factless accessor naming
policy explicit and auditable beside the other Java naming projections.
Stream method-reference fallback follow-up: when the stream renderer has a
lambda parameter shape, the final fallback body path now calls
`render_single_input_method_reference_body_with_facts` before legacy rendering.
This gives stream method references the same facade ABI callback support as
ordinary typed callbacks while preserving the older unshaped fallback for
callers that still lack fact/slot context.
Latest stream receiver-proof follow-up: the stream call-plan method-reference
path now asks `method_reference_receiver_type_for_call_plan_with_facts`, so
receiver identity can come from the same facade/API facts used by ordinary
method-reference output-shape inference. Focused coverage:
`stream_method_reference_uses_facade_abi_receiver_facts` and
`method_reference_plan_owns_generic_function_adapter_shapes`.
External fact alias follow-up: `ExternalFactLookupIndex` now builds and probes
method-name aliases through `java_naming::java_method_name_aliases`, so external
API effects and method-reference callable-key probes share the same explicit
snake/camel compatibility candidates.
Stream shape-axis cleanup: `stream_lowering.rs` no longer constructs effective
item ownership or Java storage shapes directly. Effective stream item shape
selection and Java coercion target storage shapes now go through
`conversion_plan`, keeping `stream.rs` free of direct ownership-axis decisions.
Simple Java string stream lambda params now leave type inference to Rust, while
named facade stream method references keep explicit parameter types. The broad
shape-axis guard now enforces renderer modules and keeps `lower.rs` as the
tracked remaining orchestrator cleanup for shape-axis decisions.
Full design rerun after the stream-shape checkpoint reached 462 passed / 47
failed. The current follow-up batch fixes three narrow drifts from that run:
typed receiver `String` field access now projects from owned field storage to
borrowed string content before lowering; the nested Java enum representation
guard now uses the qualified Java identity (`Holder.Type`) and collision-safe
Rust type name (`HolderType`); and the exception-planning guard now expects the
support-enriched program passed to `plan_program_exceptions`. The
exception-lowering unit fixture also avoids naming `RuntimeException` directly
so the design guard continues to prove that runtime exception classification
does not live in the renderer.
Stream external-argument follow-up: receiverless/resolved stream call argument
adaptation now checks resolved external argument facts before falling back to
Java resolver coercions, and resolved calls in `call_plan` also try
receiver-expression external argument facts before using resolver coercion
shapes. Ordinary Java string stream lambdas continue to omit explicit `&String`
annotations and rely on Rust inference. A shared `coercion::AsStr` projection
guard strips the simple borrowed-item spelling (`*value`) before appending
`.as_str()`, preventing invalid stream outputs such as `*partition.as_str()`
without adding string-view heuristics back to `stream_lowering.rs`.
Full design rerun after the stream external-argument batch reached 474 passed /
35 failed in 341.31s. The removed failures include the stream lambda/method
reference cluster and the stale semantic/receiver-field/enum representation
guards. Remaining clusters are closed-enum cast lowering, external byte/array
and borrowed recovery, external support/static-field/logging behavior,
dyn-trait representation lowering, and signature-plan structural ownership
guards.
Closed-enum cast follow-up: closed-enum wrapping now performs concrete subtype
ownership recovery before constructing the enum variant. This covers borrowed
method parameters such as `&Dog` flowing into `(Animal) dog`, producing
`AnimalValue::Dog(dog.clone()...)` instead of the invalid
`AnimalValue::Dog(dog)`. Downcasts from borrowed closed-enum values continue to
clone after extracting the variant payload.
Java enum dispatch follow-up: Java source enums no longer project to payload
`ClosedEnum` dispatch. They now keep concrete Rust enum dispatch, while
closed-world hierarchy enums continue to use `ClosedEnum { enum_name }` and
payload variant wrapping/downcasting. This keeps erased-object conversions for
Java enums on the ordinary `Box<dyn Any>`/downcast path and prevents bogus
constructor-like variants such as `VCFHeaderVersion::VCFHeaderVersion(...)`.
Focused VCF evidence:
`.tmp/corpus-rerun-htsjdk-vcf-after-java-enum-concrete-dispatch-r1` translated
32/32 files and reduced generated Rust errors from 794 to 747; the prior
`unsupported Java dispatch coercion: actual=ErasedObject, target=ClosedEnum`
and `ClosedEnum -> ErasedObject` rows for VCF enum types are gone.
Instanceof usage tracking follow-up: source `instanceof` expressions now record
`BindingUse::InstanceOfTarget` for local operands, `object_model` maps that to
`NeedsInstanceof`, and normal representation planning passes usage facts into
the object model. This makes runtime subtype-test pressure visible before
lowering. The current VCF concrete-class hierarchy still keeps concrete
`VCFHeaderLine` storage because converting source classes with fields/methods to
runtime enum or erased-object representations requires a larger forwarding/API
strategy; do not paper over this by rendering all concrete/concrete
`instanceof` as true or false.
Hash/equality derive follow-up: source structs that declare Java `equals` and
`hashCode` now derive `Eq` and `Hash` when all emitted fields support structural
`Eq + Hash`. This keeps ordinary value structs unchanged while allowing
HashSet-backed source values such as `VCFHeaderLine { Option<String>, ... }` to
compile. Focused VCF evidence:
`.tmp/corpus-rerun-htsjdk-vcf-after-hash-eq-derive-r1` translated 32/32 files
and reduced generated Rust errors from 747 to 722; the previous
`HashSet<VCFHeaderLine>` trait-bound family is gone, and generated
`VCFHeaderLine` derives `Clone, PartialEq, Eq, Hash`.
External/recovery cleanup: unresolved normalized singular utility receivers such
as `ExternalUtil.consume(...)` now recover to a static owner after parser
normalization (`external_util -> ExternalUtil`) when no local or field shadows
the receiver. This keeps unshaped external argument clone recovery on a valid
static call path instead of emitting value-receiver calls such as
`external_util.consume(...)`. The external byte-array, erased-object carrier,
and array-conversion tests were also updated to accept the shared audited clone
marker now emitted by recovery plans while still checking the same signed-byte,
carrier, and adapter behavior.
External field/logger cleanup: `MemberAccessPlan` now consumes typed external
instance field facts directly, so lower-camel facade fields such as
`record.totalBatchCount` lower through the same member plan as source fields and
generate support struct storage from `InstanceField` facts. Rust-crate mapped
external static constants now prefer the selected field fact owner when choosing
the associated-constant path, preserving crate-qualified owners such as
`hdf5_facade::OpenMode::CREATE`. Static logger fields are retargeted back to
the shared `logger` facade after the general field-receiver retarget, preventing
`Self::logger.info(...)` from bypassing logger facade calls.
Full design rerun after the external field/logger checkpoint reached 485 passed
/ 24 failed in 340.91s. Remaining failures are now concentrated in:
external borrowed literal hoisting, boxed-dyn interface field borrowing,
pack-enabled final facade local shape, result metadata separation, semantic
ownership guardrails, and the representation/signature structural guard cluster
for dyn traits and public/API boundary shapes. The earlier external byte-array,
array adapter, erased-object carrier, nullable owned option, support-field,
static-field, logger, closed-enum cast, and stream clusters are green in the
full run.
Fact/planner ownership follow-up: lower-side direct external API effect lookups
for resource close, planned static-owner return typing, and receiver-call return
typing moved behind `call_plan` helpers. Nested interface object-safety fact
lookup now accepts qualified/simple nested aliases through effective-fact
matching. Interface object-return borrowing no longer treats typed
sequence-like facades as borrowed object returns, preserving owned `Vec<T>`
trait returns, and explicit closed-enum casts matching an owned closed-enum slot
avoid a second slot-level ownership recovery after cast lowering has already
wrapped the value. Focused guards green: semantic fact ownership,
source-parsed object safety facts, dyn-trait signature rendering with facts, and
closed-enum local-shape lowering.
Public boundary and boxed-dyn carrier follow-up: public/protected method
signature constraints now preserve source/interface representation boundaries
instead of adding local borrowed-content or non-null return constraints. Copy
capability facts prevent cheap copy values from being forced into owned storage
shape. Optional element projection is fact-aware for object-safe dyn-trait
carriers, and current-instance field arguments compare the optional element type
while preserving the `Option<Box<dyn Trait>>` carrier for shared slot conversion.
Focused guards green: `signature_constraints_make_public_api_fallback_explicit`,
`signature_constraints_treat_protected_members_as_api_boundaries`,
`signature_baseline_constraints_use_copy_capability_facts_for_all_slots`,
`signature_plan_uses_boxed_dyn_for_interface_fields`,
`signature_public_interface_boundaries_preserve_representation_shapes`,
`optional_receiver_field_element_recovery_uses_borrowed_carrier_plan`, and
`lowering_borrows_boxed_dyn_field_for_interface_arguments`.
Signature/representation drift cleanup checkpoint: representation lookup now
has a unique simple-name fallback for source-nested object-safety facts,
abstract classes no longer emit incomplete dyn-trait impls, and synthetic
external inherited method params are shaped through the same signature solver as
ordinary slots. Signature constraints now route copy decisions through
`signature_capability_policy`, preserve call-result/alias return shapes during
fixpoint collection, and apply external unknown-argument non-null defaults only
when an unqualified or external-modeled call justifies them. Explicit
`KnownContentRead` argument boundaries still enable borrowed sequence views for
native collection idioms such as `addAll`. Full verification:
`cargo test --test slot_shape_design --quiet` passed 509/509 in 354.64s.

Optional/string slot stabilization checkpoint: public read-only string
parameters again participate in borrowed-content policy while public API
signature guards preserve explicit boundary representation. Optional string
view projection now uses `as_ref().map(|value| value.as_str())`, which works
for both `Option<String>` and `Option<&String>` carriers. Nullable owned facade
arguments for `HtsjdkReadsRddStorage.referenceSourcePath` and
`SamReaderFactory.referenceSequence` now use `NoAlias` external-pack facts, so
their `Option<String>`/`Option<Path>` support signatures receive owned optional
payloads instead of borrowed payload views. Java string equality planning now
normalizes all non-null string operands through the shared string-view equality
plan, not only operands that already arrived borrowed. Focused guards green:
`nullable_external_fluent_argument_wraps_owned_optional_branch`,
`nullable_sam_reader_reference_sequence_argument_wraps_path_branch`,
`map_get_into_optional_borrowed_arg_uses_optional_carrier_plan`,
`non_null_ternary_into_optional_borrowed_arg_wraps_planned_borrow`,
`borrowed_nullable_trim_equals_original_uses_optional_string_view_policy`,
`nullable_trim_equals_original_uses_optional_string_view_policy`,
`public_readonly_string_param_uses_str_view_for_static_string_constants`,
`java_string_equality_normalizes_owned_and_borrowed_slots`,
`disq_write_pack_declares_temp_parts_option_boundary`,
`htsjdk_sam_reader_pack_declares_factory_option_nested_enum`, and
`htsjdk_variantcontext_pack_models_builder_and_getter_effects`.

Varargs/static-constant follow-up: typed external static constants now anchor
same-typed local declarations before erased-object preservation runs, so
`ReadsFormatWriteOption.BAM`/`FileCardinalityWriteOption.SINGLE` locals keep
their precise concrete shapes and varargs packing can select the ordinary
`Into<WriteOption>` adapter. Varargs element lowering now calls the shared
reused-owned clone decision, and tests assert the intended move-use policy:
clone before later reuse, but move when the value is consumed exactly once.
Focused guards green:
`source_varargs_pack_external_elements_with_explicit_adapter`,
`source_varargs_pack_borrowed_external_elements_clones_before_into`,
`source_overload_prefers_expanded_varargs_over_same_arity_fixed_mismatch`, and
`reused_optional_borrowed_source_argument_wraps_by_reference`.

Nullable fallthrough try follow-up: source return nullability is no longer
discarded solely because a method returns a call result. This lets explicit
nullable fallbacks such as `try { return call(); } catch (...) { ... } return
null;` select an optional return carrier, while the concrete-return shortcut
remains guarded for call-result methods. Value-producing fallthrough
try/catch lowering now maps inner `Ok(Some(value))` to `Ok(Some(value))` when
the outer method return shape is already optional. Focused guards green:
`try_return_with_fallthrough_catch_uses_outer_fallback_return`,
`plain_fallthrough_try_with_value_returns_uses_optional_result_carrier`, and
`optional_argument_wrap_reused_value_clones_before_some`.

Arithmetic rendering and source `Object...` carrier follow-up: the Rust emitter
now preserves explicit grouping for arithmetic binary children instead of
treating `+`/`*` as freely associative. This keeps translated Java numeric
evaluation order stable for constant-backed expressions while still allowing
logical/bitwise flattening. Source Java `Object...` parameters now keep the
owned native varargs carrier shape even for public methods, so source methods
that store, forward, or append erased arguments no longer borrow a temporary
slice. Focused guards green:
`source_static_final_primitive_expression_fields_emit_associated_constants`,
`source_static_final_constants_can_reference_other_source_constants_and_hex_sign_bits`,
`source_static_final_primitive_array_fields_emit_associated_slice_constants`,
`source_object_varargs_storage_carrier_uses_owned_native_param_shape`,
`source_object_varargs_forward_and_append_erased_arguments`, and
`inherited_context_trailing_call_uses_context_field_and_shared_varargs_plan`.

2026-07-27 source/external identity checkpoint: simple-name external-pack
erased-object facts no longer override a type that is known from parsed source.
This keeps unrelated fixtures free to use explicit corpus/user dispatch
requirements while preventing package-specific simple names such as `Matrix`
from erasing source classes with the same simple name. Public source methods now
retain positive nullable-return evidence even at API boundaries, so a public
method that explicitly returns `null` selects `Option<T>` rather than a concrete
return shell. Verification:
`parsed_source_concrete_type_beats_external_simple_name_erasure`,
`public_source_method_returning_null_uses_optional_concrete_return_shape`,
`cargo test --test slot_shape_design --quiet` (512 passed),
`cargo test --test idiom_gaps --quiet` (499 passed),
`cargo fmt --check`, `cargo check --quiet`, `git diff --check`,
`cargo build --release --quiet --bin new-java2rust`, and focused JAligner rerun
`.tmp/corpus-rerun-20260727-jaligner-final` with 29/29 translated,
0 invalid-pattern files, `cargo_check=ok`, and 0 generated-Rust errors.

2026-07-27 full FastQC refresh after the source/external identity checkpoint:
`.tmp/corpus-batch-current-fastqc-refresh-20260727-after-source-erasure`
translates 156/156 files with 0 translation failures, 29 invalid-pattern files,
and 1459 generated-Rust cargo errors. Runtime remains dominated by global
semantic/signature work: total 2,110,447 ms, translation 1,650,111 ms, global
analysis 1,254,644 ms, signature solve 795,894 ms, signature fixpoint
767,748 ms, semantic analysis 456,896 ms, and crate assembly 447,799 ms. Top
compile families are mismatched types, fallible `?` propagation/shell drift,
GUI/AWT erased-object receiver surfaces, switch scaffold locals, Base64/
OutputStream facade identity, JDK `File::new_1/new_2` overload support, and
source methods that mutate fields through `&self`.

Next FastQC batch candidates from that rerun:

1. Fallibility shell propagation: 126 non-Result `?` rows plus 88 `?`-on-
   non-`Try` rows suggest some source methods receive fallible facade calls
   without corresponding result-shaped method signatures.
2. GUI/AWT/Swing facade surfaces: many `Box<dyn Any>` receivers are missing
   typed component/graphics methods such as `set_color`, `set_size`,
   `set_double_buffered`, `print`, `add_notify`, `get_font_metrics_0`,
   `draw_line`, `draw_string`, and `fill_rect`.
3. Source mutability inference: rows such as `self.random_data` and `self.v`
   behind `&self` should become shared receiver-mutability facts rather than
   method-local fixes.
4. Base64 nested-class and stream state lowering: focused proof in
   `.tmp/corpus-fastqc-base64-single-after-local-type-rust-key-r1` shows the
   simple `OutputStream = Base64OutputStream` alias, duplicate source/stub
   identity, and conflicting `From<Base64OutputStream>` rows are gone. The
   remaining Base64 batch should target true nested-source semantics: qualify
   outer static members from nested classes as the outer owner, avoid
   `self.field` reads while constructing `__java_self`, model inherited
   `FilterOutputStream.out` state, and fix byte-array move/bitwise coercion
   drift. Follow-up focused proof
   `.tmp/corpus-fastqc-base64-single-after-focused-file-sync-r1` syncs the
   focused wrapper with platform `File::new_1/new_2`, `File.length()`, and
   `java_string_from_bytes`; those missing-helper rows are gone and the focused
   generated-Rust count is now 59.
   Follow-up focused proof
   `.tmp/corpus-fastqc-base64-single-after-nested-static-owner-r1` retargets
   receiverless static calls resolved to an outer declaring source type and
   owner-qualifies bare associated constants introduced by context seeding.
   The nested `Self::ENCODE`/`Self::get_decodabet` missing-owner family is
   gone and the focused generated-Rust count is now 41. The next Base64 slice
   `.tmp/corpus-fastqc-base64-single-after-modeled-super-order-r1` routes
   modeled external-super constructors through the same deferred
   `__java_self` path, orders deferred assigned fields by Java constructor
   statement order, and drops the focused generated-Rust count to 37. The
   constructor `self.buffer_length`/`self.encode` use-before-construction
   family is gone. The next Base64 slice
   `.tmp/corpus-fastqc-base64-single-after-outputstream-impl-guard-r2` models
   exact `java.io` inherited stream state for `FilterInputStream.in` and
   `FilterOutputStream.out`, lets external inherited field facts override
   weaker source-summary inherited shapes, and suppresses unsafe
   `OutputStream -> JavaIoOutputStream` support impls when the simple source
   type is later suppressed by a local nested `Base64.OutputStream`. Invalid
   downcast patterns are now 0 and the focused generated-Rust count is 34.
   The next Base64 slice
   `.tmp/corpus-fastqc-base64-single-after-shared-from-strip-r1` preserves
   explicit `java.io.*Stream` parameter names, keeps exact JDK stream adapter
   endpoints distinct from simple stream names, and updates focused
   shared-support stripping so module-local `From<T>` impls targeting shared
   support types are not removed. The previous
   `InputStream: From<JavaIoInputStream>` row is gone and the focused
   generated-Rust count is 29. The next Base64 slice
   `.tmp/corpus-fastqc-base64-single-after-byte-array-outputstream-r1` models
   exact `java.io.ByteArrayOutputStream.toByteArray()` as a non-throwing
   `Vec<i8>` getter and adds simple/exact byte-array-output-stream conversion
   facts to `java.io.OutputStream`. The previous four missing `to_byte_array`
   rows are gone and the focused generated-Rust count is 25. The latest Base64
   slice `.tmp/corpus-fastqc-base64-single-after-query-inherited-close-r1`
   materializes query-backed inherited JDK method surfaces for source classes
   extending imported/qualified platform stream supertypes and keeps local
   simple-name shadows guarded. The two `Base64InputStream.close` missing-method
   rows are gone and the focused generated-Rust count is 6. The follow-up
   `.tmp/corpus-fastqc-base64-single-after-source-super-conversion-r7`
   projects source subclasses of resolved external stream supertypes to exact
   facade conversion impls, keeps those impls module-local for source-owned
   nested types, and prevents shared-support promotion from duplicating the
   real `Base64OutputStream` export. The focused generated-Rust count remains
   6, but the `Base64OutputStream -> JavaIoOutputStream` trait-bound row is
   gone. The follow-up
   `.tmp/corpus-fastqc-base64-single-after-optional-outputstream-write-r1`
   lets typed callable/facade ABI facts match nullable `Optional<T>` arguments
   by their present element type and adds exact `JavaIo*OutputStream` receiver
   keys to the shared JDK byte-slice write facts. The focused generated-Rust
   count is now 5 and the `OutputStream.write(byte[])` exact-slot mismatch is
   gone. The optional-resource/byte-array follow-up keeps output-stream wrapper
   constructor slots on the shared argument-effect path, solves nullable
   mutated byte-array helper parameters as `Option<&mut [T]>` while caller
   locals stay owned, recovers owned `Vec<T>` from returned mutable slices with
   the shared sequence recovery policy, and makes required borrowed-element
   target-view projection idempotent when an `Option<&[T]>` unwrap already
   yields the requested slice view. Focused proof
   `.tmp/corpus-fastqc-base64-single-after-optional-borrowed-slice-r1`
   translates 1/1 files with 0 invalid-pattern files and `cargo_check=ok`.

Inherited exception-owner follow-up: inherited source method bodies now lower
with the emitted subclass as the body/field context but use the declaring type
when selecting the method exception plan. This preserves result shells for
inherited methods that declare `throws` while still allowing subclass-specific
field/import context. Focused verification:
`inherited_context_method_body_sees_declaring_static_imports`,
`inherited_context_trailing_call_uses_context_field_and_shared_varargs_plan`,
`throwing_call_chain_promotes_caller_result_shells`,
`method_effect_facts_drive_exception_result_shell_and_return_wrapping`,
`cargo fmt --check`, `cargo check --quiet`, `git diff --check`,
`cargo build --release --quiet --bin new-java2rust`, and focused FastQC
AbstractQCModule rerun
`.tmp/corpus-fastqc-abstract-qcmodule-after-inherited-exception-owner-r1`.
That rerun translates 8/8 files with 0 invalid-pattern files and no remaining
`?`/result-shell rows; the only generated-Rust errors are two
`cannot return value referencing temporary value` rows in `SVGImageSaver`.

Reassigned string-parameter follow-up: reassigned string-like parameters now
force owned/native parameter slots, and returned-alias borrow constraints reject
parameters that are assigned inside the method body. This keeps internal
helpers such as FastQC `SVGImageSaver.optimizeSvg(String svg)` from returning
references to temporaries after `svg = helper(svg)` rewrites. Focused
verification:
`reassigned_string_param_return_stays_owned_not_borrowed_alias`,
`reassigned_string_param_return_renders_owned_signature`, release
`new-java2rust`, and FastQC AbstractQCModule rerun
`.tmp/corpus-fastqc-abstract-qcmodule-after-reassigned-return-owned-r3`.
That rerun translates 8/8 files with 0 invalid-pattern files and
`cargo_check=ok`.

FastQC math-util current cleanup: a fresh focused rerun after the Base64
checkpoint,
`.tmp/corpus-fastqc-math-util-after-base64-checkpoint-r1`, translated 10/10
files with 0 invalid-pattern files and only two real generated-Rust errors:
Java boolean XOR of comparisons rendered as chained Rust comparisons in
`FastMath.nextAfter(...)`. Rust expression emission now parenthesizes
comparison operands under bitwise operators, preserving shapes such as
`(direction < d) ^ (sign == 0i64)`. Focused verification:
`bitwise_boolean_operands_parenthesize_comparisons`,
`boolean_xor_of_comparisons_parenthesizes_operands`, `cargo fmt --check`,
`cargo check --quiet`, `git diff --check`,
`cargo build --release --quiet --bin new-java2rust`, and focused math-util
rerun `.tmp/corpus-fastqc-math-util-after-comparison-xor-parens-r1`. That
rerun translates 10/10 files with 0 invalid-pattern files and `cargo_check=ok`.

HTSJDK VCF current rerun: fresh focused evidence in
`.tmp/corpus-rerun-htsjdk-vcf-current-after-math-base64-r1` translates 32/32
files, reports 20 invalid-pattern files, and fails generated Rust with 805
reported errors. The first syntax cleanup parenthesizes cast expressions before
method-call projections such as `.downcast()` and `.clone_box()`, removing the
`cast cannot be followed by a method call` rows and moving the rerun to 798
reported errors in `.tmp/corpus-rerun-htsjdk-vcf-after-cast-receiver-parens-r1`.
The adjacent nominal-target cleanup renders dispatch downcast targets through
the Rust nominal type policy, so nested Java types such as
`VCFCompoundHeaderLine.SupportedHeaderLineType` become
`VCFCompoundHeaderLineSupportedHeaderLineType` in turbofish syntax. Follow-up
rerun `.tmp/corpus-rerun-htsjdk-vcf-after-rust-nominal-target-r1` reports 794
generated-Rust errors. The next VCF batch should target representation facts
for erased-object-to-closed-enum recovery and closed-enum erasure rather than
more syntax cleanup.

Java enum constant slot/shape follow-up: source enum constants now lower as
definitely non-null concrete enum-owner values before optional slot conversion
or equality planning. `lower_java_enum_static_field_access_for_slot` hands
concrete and optional enum targets to the shared slot-conversion plan, and
`expr_shape` reports the same concrete owner shape for source enum constants so
binary comparisons do not invent optional `.as_ref()` projections. Focused
verification:
`source_java_enum_constant_uses_slot_plan_not_optional_projection`,
`closed_enum_variant_constructor_delegation_supplies_empty_varargs_without_optional_as_ref`,
`cargo fmt --check`, `cargo check --quiet`,
`cargo build --release --quiet --bin new-java2rust`, and VCF reruns
`.tmp/corpus-rerun-htsjdk-vcf-after-enum-constant-slot-plan-r1` then
`.tmp/corpus-rerun-htsjdk-vcf-after-enum-constant-shape-r1`. The final rerun
translates 32/32 files, keeps 20 invalid-pattern files, and reduces
generated-Rust errors from 722 to 698. The tracked enum projection family
(`VCFHeaderVersion::VCF4_3.unwrap()`, enum constant `.as_ref()`, and
`VCFHeaderLineCount::INTEGER.map(...)`) is gone; remaining top families are
mismatched types, type annotations, duplicate `to_string`, VCF constants/static
support, concrete/concrete `instanceof`, map API gaps, erased-object
clone/comparison, and inherited `super(...)` lowering.

Follow-up JDK collection ABI slice: `Map.putAll` now has `HashMap::extend`
facade facts and the shared native collection `Extend` plan covers optional
collection field receivers. Focused idiom and signature-fact tests pass, and
VCF rerun `.tmp/corpus-rerun-htsjdk-vcf-after-map-put-all-r3` translates 32/32
files, keeps 20 invalid-pattern files, and reduces generated-Rust errors from
698 to 691. The generated VCF `generic_fields.putAll(...)` call sites now
render as `self.generic_fields.as_mut().unwrap().extend(...)`; only generated
trait stub text still contains `put_all`. Remaining VCF map work is no longer
missing `putAll`; it is mainly raw/erased map value-shape adaptation such as
`HashMap<String, String>` flowing into `HashMap<String, Box<dyn Any>>`.

Follow-up erased-map entry adaptation: shared native collection `Extend`
lowering now maps entries when the receiver is a map with an erased/object value
slot and the source map has concrete values. Focused fixture
`map_put_all_boxes_values_for_erased_object_value_map` passes, and VCF rerun
`.tmp/corpus-rerun-htsjdk-vcf-after-map-extend-erased-values-r1` still reports
691 generated-Rust errors but removes the explicit
`HashMap<String, String> as IntoIterator<Item = (String, Box<dyn Any>)>` family.
Seven VCF `generic_fields` copy sites now box values during `extend`. Treat this
as a targeted correctness fix with no headline count movement; the next VCF map
work is broader raw `Map` owner/value support and erased-object clone/comparison
rather than `putAll` lowering.

Follow-up native collection hashCode slice: Java collection `hashCode()` is now
planned as native collection lowering for list, set, and map receivers, and
renders through shared runtime helper projections instead of a Rust
`.hash_code()` method guess. The focused fixture
`map_hash_code_lowers_to_java_runtime_helper` passes; `cargo fmt --check`,
`cargo check --quiet`, adjacent map idiom tests, `bash -n
scripts/translate-focused-package.sh`, and
`cargo build --release --quiet --bin new-java2rust` were clean. VCF rerun
`.tmp/corpus-rerun-htsjdk-vcf-after-map-hash-code-r2` translates 32/32 files,
keeps 20 invalid-pattern files, and reduces generated-Rust errors from 691 to
685. The previous native-map `.hash_code()` row is gone; remaining
`.hash_code()` call sites are source enum/header-line types such as
`VCFHeaderLineType` and `VCFHeaderLineCount`, so the next hash-related slice
should be source enum/source-class hash support rather than map ABI lowering.

Follow-up source hashCode slice: source-object `hashCode()` fallback now mirrors
the source-object `toString` exclusions for strings, collections, platform
types, external packs, and explicit facade ABI facts, then lowers hashable
source values through `java_value_hash_code(&...)`. The focused fixture
`source_enum_hash_code_lowers_to_java_value_hash_helper` passes, and adjacent
hash fixtures plus `cargo fmt --check`, `cargo check --quiet`, `bash -n
scripts/translate-focused-package.sh`, and
`cargo build --release --quiet --bin new-java2rust` were clean. VCF rerun
`.tmp/corpus-rerun-htsjdk-vcf-after-source-hash-code-r1` translates 32/32
files, keeps 20 invalid-pattern files, and reduces generated-Rust errors from
685 to 680. The generated VCF modules now have no `.hash_code(` call sites;
nine former enum/header-line source hash calls render as `java_value_hash_code`.
The next high-count VCF work should move away from hash lowering and target
raw `Map` owner/value support, source hierarchy/`instanceof`, VCF
constants/static fields, or duplicate `to_string` emission.

Follow-up native collection facade borrow cleanup: raw/wildcard Java collection
arguments that resolve to borrowed native Rust collection slots now avoid both
the concrete facade placeholder (`&Map`) and duplicate borrow spelling
(`&&map`). Native collection facade target-view adaptation preserves the real
collection value and lets the outer slot planner insert the single borrow, while
plain `Coercion::Borrow`/`BorrowMut` now pass through the idempotent borrow
projection helper. The focused fixture
`raw_map_extends_object_argument_borrows_local_not_map_type` passes, and VCF
rerun
`.tmp/corpus-rerun-htsjdk-vcf-after-idempotent-native-collection-borrow-r1`
translates 32/32 files, keeps 20 invalid-pattern files, and stays at 670
generated-Rust errors. The VCF `to_string_encoding_map(...)` call sites now
render `&map`; remaining top families are broad mismatched types, type
annotations, duplicate `to_string`, VCF constants/static support,
concrete/concrete `instanceof`, erased-object clone/comparison, inherited
`super(...)`, and map-entry iterator shape drift.

Follow-up map-entry enhanced-for cleanup: enhanced-for over
`Map.entrySet()` now uses the already-planned native map iterator directly
instead of passing that iterator back through the generic enhanced-for renderer
and producing `.iter().iter()`. The loop body records `Map.Entry<K,V>` locals in
both the local shape environment and expression refinements, so
`entry.getKey()`/`entry.getValue()` project tuple fields through the slot
planner rather than falling back to fake methods. Focused fixtures
`map_entry_enhanced_for_uses_planned_iterator_and_tuple_fields`,
`map_entry_set_iterator_next_get_key_uses_native_iterator_shape`, and
`map_entry_set_iterator_next_get_key_uses_receiver_signature_shape` pass. VCF
rerun `.tmp/corpus-rerun-htsjdk-vcf-after-map-entry-loop-r1` translates 32/32
files, keeps 20 invalid-pattern files, and reduces generated-Rust errors from
670 to 664. `VCFHeaderLine.toStringEncoding(Map<...>)` now renders
`for entry in key_values.iter()` with `entry.0`/`entry.1`; remaining top
families are broad mismatched types, type annotations, duplicate `to_string`,
VCF constants/static support, concrete/concrete `instanceof`, erased-object
clone/comparison, inherited `super(...)`, and unresolved external/source facade
methods.

Follow-up Java string-search char needle cleanup: `String.indexOf(char)` and
`indexOf(char, from)` now normalize char-literal needles into one-character
string literals inside `string_index_of_projection` before using the existing
string-view ABI. The focused fixture
`java_string_get_bytes_and_index_of_use_native_helpers` now covers `"&"`, `'='`,
and `'\t'` needles. VCF rerun
`.tmp/corpus-rerun-htsjdk-vcf-after-char-index-of-r1` translates 32/32 files,
keeps 20 invalid-pattern files, and reduces generated-Rust errors from 664 to
658. The previous `char: AsRef<str>` rows are gone; remaining top families are
broad mismatched types, type annotations, duplicate `to_string`, VCF
constants/static support, concrete/concrete `instanceof`, erased-object
clone/comparison, inherited `super(...)`, unresolved external/source facade
methods, and `String: Borrow<&str>` map/set lookup drift.

Follow-up static-final string storage cleanup: source `static final String`
fields now keep using Rust associated consts only when the initializer can be
represented as a real const. Same-owner literal concatenations fold, while
cross-owner/runtime string concatenations demote to static storage. Dependent
primitive static fields that reference demoted string constants also demote, and
simple static initializer expressions preserve string `.length()` and numeric
binary operations instead of falling back to `0`. Qualified reads of runtime
static string storage keep their owned `String` actual shape so borrowed string
slots, including `startsWith`, render through `.as_str()`. Focused fixture
`source_static_final_string_concat_constants_use_fold_or_storage` passes. VCF
rerun `.tmp/corpus-rerun-htsjdk-vcf-after-static-string-storage-r3`
translates 32/32 files, keeps 20 invalid-pattern files, and reduces
generated-Rust errors from 652 to 645. The intermediate missing
`ALT_HEADER_START`/`PEDIGREE_HEADER_START` associated items and
`String: Pattern` rows are gone. Next static work should address stable static field
identity/name drift such as `MISSING_VALUE_v4`/`PASSES_FILTERS_v4` rather than
adding more string-concat special cases.

Follow-up source static field identity cleanup: qualified source associated
constant reads now resolve the receiver owner and field declaration before
rendering, so access sites use the same associated-const identifier item
assembly emitted. This fixes mixed-case suffix constants such as
`MISSING_VALUE_v4` and `PASSES_FILTERS_v4` without adding VCF-specific aliases.
Focused fixture
`source_static_final_mixed_case_suffix_field_access_uses_emitted_const_name`
passes. VCF rerun
`.tmp/corpus-rerun-htsjdk-vcf-after-static-field-identity-r1` translates 32/32
files, keeps 20 invalid-pattern files, and reduces generated-Rust errors from
645 to 627. Remaining static/enum work is now more clearly about enum/static
support identities such as `VCFHeader.HEADER_FIELDS`, source enum static
accessors like `__static_v_c_f4_3`, and package/facade static constants such as
`TabixFormat.VCF`, not raw mixed-case source constant names.

Follow-up enum/interface boundary cleanup: resolved source-interface receiver
calls now stay as known borrowed trait-view usage instead of demoting the
interface to erased object through an unknown receiver boundary. Focused
fixtures `closed_enum_variant_borrows_as_dyn_trait_parameter`,
`commons_math_localized_formats_constants_borrow_as_localizable_pack_trait`,
and `source_nested_enum_type_access_uses_flattened_enum_helpers` pass, and the
new object-model regression
`object_model_keeps_source_interface_dyn_trait_for_resolved_receiver_call`
proves the usage requirement remains `NeedsBorrowedTraitView`.

Follow-up source-field collection return cleanup: usage analysis now carries
source-declared field types directly, so unshadowed source fields such as
`patterns: List<Localizable>` can recover the receiver type for both
`patterns.add(pattern)` and returned `patterns.get(index)` calls without
depending on external field facts. Collection storage calls on `List`, `Set`,
and `Map` use the receiver element/key/value type to classify unknown external
argument boundaries as owned polymorphic storage when the stored binding matches
the collection slot. This keeps `List<Localizable>` fields as
`Vec<Box<dyn Localizable>>`, preserves `pattern.clone_box()` on insertion, and
solves `Localizable first(...) { return patterns.get(index); }` as
`Box<dyn Localizable>` instead of `Box<dyn Any>`. Verified with
`source_interface_collection_get_return_keeps_boxed_dyn_trait_slot` and
`source_interface_collection_elements_use_boxed_dyn_trait`.

Follow-up source input-stream overload cleanup: source methods that directly
override JDK input-stream `read(byte[], int, int)` now inherit the arity-safe
Rust ABI name `read_3` even when the source class declares only that one
`read(...)` method. This prevents source subclasses such as
`GZIPInputStream` wrappers from emitting both one-argument and three-argument
`read` methods with the same Rust name, while preserving the existing
source-output-stream `write_byte_array_int_int` override path. Verified with
`input_stream_read_byte_array_uses_three_arg_overload_with_full_buffer`,
`source_filter_output_stream_write_byte_array_uses_three_arg_override`, and
`documents_try_with_resources_copy_gap`.

Follow-up source-summary static helper borrow cleanup: receiver-style and
resolved receiverless static calls into non-emitted context/source-summary
helpers now project reference-like parameters into shared-borrow call slots.
When a support/facade argument plan has already shaped the call, the context
fallback may only refine an owned reference argument to the borrowed slot proved
by the source-summary signature; it does not otherwise overwrite fact-shaped
arguments. Verified with
`resolved_static_helper_call_uses_fallback_signature_arg_shapes` and
`constructed_external_argument_is_borrowed_for_borrowed_source_slot`.

Follow-up erased-object varargs carrier cleanup: source `Object...` parameters
that are stored or forwarded now keep owned/native vector carrier constraints
instead of being normalized into borrowed slices. The erased-object clone
recovery helper is only selected when the actual value is already erased; typed
borrowed values such as `Number` recover through normal clone policy before
being boxed as `dyn Any`. Verified with
`source_object_varargs_storage_carrier_uses_owned_native_param_shape`,
`source_object_varargs_forward_and_append_erased_arguments`,
`inherited_context_trailing_call_uses_context_field_and_shared_varargs_plan`,
and `cargo test erased_object --test idiom_gaps --quiet`.

Follow-up source/context string-view ABI cleanup: internal non-escaping
reference parameters may now borrow even when the parameter is unused, and
borrowed Java `String` parameters project to string-view slots. Source-summary
and non-emitted context fallback signatures use the same projection, so ordinary
source calls, context overload calls, and nullable source-summary calls agree on
`&str`/shared-borrow ABI instead of cloning into owned `String` at each call
site. Verified with
`optional_field_mutating_overload_call_uses_signature_name_and_mut_receiver`,
`context_translation_uses_sibling_overload_signature_for_call_target`,
`nullable_source_call_through_field_receiver_stays_optional_without_double_wrap`,
`cargo test source_summary_nullable_return --test idiom_gaps --quiet`,
`cargo fmt --check`, and `cargo check --quiet`.
The same ABI change made several older expectations obsolete: custom equals
generated harness calls now borrow `Value` arguments, owned-path static helper
fixtures allow borrowed `Path` arguments when the helper only reads the value,
and getter-initializer fixtures keep borrowed locals when no owned recovery is
needed. Focused guards now pass for
`custom_equals_method_call_stays_a_method_call`,
`static_owned_path_argument_does_not_borrow_owned_temporary`, and
`source_borrowed_getter_initializers_recover_owned_locals`.

## Rerun Protocol

After each fix:

1. Run the new focused idiom test.
2. Run `cargo test --test idiom_gaps --quiet`.
3. Run `cargo fmt --check` and `cargo check --quiet`.
4. Rebuild the release binary:
   `cargo build --release --quiet --bin new-java2rust`.
5. Rerun only the affected focused corpus first.
6. Update `docs/current-corpus-issue-audit.md` with exact output directories and
   new counts.

## Parallelization

Use parallel workers for investigation, not for conflicting edits:

- Worker A: isolate FASTQ optional getter/copy-constructor fixtures.
- Worker B: isolate FastQC panic file and minimization.
- Worker C: inspect VCF enum generated output and propose fixture.
- Worker D: inspect Spark/DISQ interface bridge generated output.

Merge implementation work in this checkout only after each worker returns a
fixture-sized diagnosis. This avoids branch drift while still getting parallel
read-only analysis.

## Do Not Do

- Do not add corpus-specific method-name tables to `src/stdlib.rs`.
- Do not weaken signature-solver panics into silent defaults without a typed
  fallback plan or explicit unsupported diagnostic.
- Do not model true Spark/DISQ distributed execution or GenomicsDB native
  behavior in this Java translator.
- Do not let algorithm/performance changes block future global semantic
  analysis; prefer indices and removing unnecessary cloning when performance
  work resumes.
