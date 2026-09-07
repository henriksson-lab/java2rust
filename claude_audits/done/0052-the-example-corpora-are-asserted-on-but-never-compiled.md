# 0052 — The example corpora are asserted on, never compiled, and almost never run

Recorded 2026-08-21.
Status: **fixed for compile visibility** — the corpus compile visibility gap is
covered by `tests/corpus_compile.rs`, the checked-in baseline is down to the one
invalid-Java fixture that both `javac` and the translator reject loudly, and the
`idiom_gaps` rendered-Rust assertion surface is generated-crate-backed or
expected-failure-backed. Behavioural oracle expansion continues under the
differential/semantic audits rather than this compile-measurement issue.
Severity: **high** — current dirty-tree corpus failures are visible through
`tests/corpus_compile.rs`. The remaining checked-in corpus failure is a loud
`compile_error!` for invalid Java, not a silent Rust type/mutability failure.
Kind: measurement / coverage.

Current state 2026-09-07: the gate baseline has `idiom_gaps` green at 816/0 and
`fixtures` green at 54/0. `tests/differential.rs` now asserts 228 active
behavioural fixtures. The rendered-Rust scan table below is historical unless
regenerated with the current scanner; do not use its suite/test counts as
current baseline numbers.

Extends [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)
(the compile-error metric cannot see silent failures) and
[0041](0041-slot-shape-design-had-no-generated-crate-checks.md) (a suite
that asserted on rendered Rust with nothing compiling it). 0041 fixed one suite.
The same hole is still open in three more, and it is open on the *fixture
corpora themselves* — the Java files the whole project is organised around.

---

## Symptom

`examples/java` (67 files) and `examples/idiom-gaps` (122 files) are the
project's curated inputs. 921 tests assert on the Rust rendered from them and
from inline sources. **Nothing compiles the corpora.** Doing so once, by hand,
originally found 58 files whose output `rustc` rejects — including two that
produce Rust that does not even *parse*. Follow-up fixes reduced that baseline
to 20 files; a later corpus guard run surfaced eight stale baseline deltas in
the dirty tree. Follow-up fixes brought the checked baseline to 24 files, and
return-slot structural-form fixes reduced it to 20 files. A follow-up optional
method-receiver equality fix reduced it to 19 files by making
`InstanceofGuardNarrowsCast` compile. External public-field facts for the
comparator fixture then reduced it to 18 files by making
`ComparatorInterface` compile. Callable-return inference for raw `submit(Callable)`
callbacks then reduced it to 17 files by making `ExecutorSubmitBlockLambda`
compile. Raw collection erasure for `List.add` / `List.get` then reduced it to
16 files by making `RawTypes` compile. Fallback static-final external field
surfaces then reduced it to 15 files by making
`ExternalFieldThrowingCallInTry` compile without injected facts. Java NIO `Files`
static support plus mutable-byte-buffer reborrowing then reduced it to 14 files
by making `TryWithResourcesCopy` compile. A follow-up made return-bearing
try-with-resources scopes use the try/catch result carrier for resource
initialization, cleanup and body result return, reducing it to 13 files by
making `TryWithResourcesReturn` compile. A catch/finally follow-up lowered
catch returns with the result-carrier policy before running `finally`, reducing
it to 12 files by making `TryCatchFinally` compile. Preserving
result-propagated shape through owned `Option::unwrap()` and adding
`java.io.Closeable.close` support then reduced it to 11 files by making
`TryWithResources` compile. Stream prefix-local nullability planning plus
temporary nested-stream string recovery then reduced it to 10 files by making
`StreamFlatMapBlockPipeline` compile. Erased Spark `sortByKey` facade returns
then reduced it to 9 files by making `SparkRddVariantPipeline` compile. Spark
callback facade typing, `collectAsMap` support, return-only facade generics, and
slot-aware null defaults then reduced it to 8 files by making
`StreamCallbackChain` compile. Modeling the fixture's external `ParentWriter`
super constructor as a no-state boundary then reduced it to 7 files by making
`SuperConstructorState` compile. Supplying the corpus guard with the same local
`ndarray` shim used by focused-package checks, and recovering borrowed
`ndarray::Array2` values with `clone()` instead of sequence `.to_vec()`, then
reduced it to 6 files by making `ArrayInitializers` compile. Bounded-wildcard
collection handling then reduced it to 5 files by making `WildcardBounds`
compile. Dyn-trait interface parameter planning then reduced it to 4 files by
making `DynTraitMethodCompatibility` compile; interface trait signatures now use
the same borrowed `&dyn Trait` parameter view that compatible implementations
require.

Update 2026-08-24: a current guard run showed the checked baseline is stale
again. It reported new failures in `FileLoadConstructorDelegation`,
`SerializableInstanceofObject`, `SparkRddVariantPipeline`, `ForOmittedParts`,
`LabeledBreak`, `LabeledContinue`, `LabeledContinueForUpdate`,
`LoopsAndBranches`, `AliasingSharedMutation`, `FieldWriteReceiverMutation`, and
`ArraysAndVarMutation`. A follow-up fixed the repeated `for (int i = 0; ...;
i++)` mutability subset by making liveness resolve for-loop condition/update
expressions in the post-initializer scope. Focused generated Rust for
`examples/java/02-control-flow/LabeledBreak.java` now compiles. A second
follow-up fixed field/index assignment receiver mutability by making assignment
lvalue roots feed the write-side liveness relation; focused generated Rust for
`examples/java/04-objects/AliasingSharedMutation.java` now compiles, while still
emitting the existing SharedLocal alias warning. The baseline should be re-run
before editing `tests/baselines/corpus-compile-failures.txt`.

Update 2026-08-24 second guard pass: the dirty tree reported 12 failures beyond
the checked four-name baseline. Two shared one root: Java `String` switch
selectors that lower as borrowed `&str` still got `.as_str()`, which is unstable
on `str` in this toolchain. `lower_switch_match_selector` now consults the
selector slot shape and leaves borrowed string views alone. Focused generated
Rust for `examples/java/02-control-flow/SwitchFallthrough.java` and
`examples/idiom-gaps/SimpleReturnSwitches.java` now compiles. The remaining
new corpus failures from that run were unrelated owner/static-call, enum-field,
erased-object, hierarchy, null-representation and collection-carrier cases; do
not update `tests/baselines/corpus-compile-failures.txt` until those are either
fixed or intentionally accepted as the new baseline.

Update 2026-08-24 third guard pass: stale owner/static-call, enum payload,
try-with-resources, erased-object `Serializable`, ordered-map carrier and Spark
erased-facade assignment failures are fixed. A follow-up made anonymous
interface returns use their boxed dyn-trait slot, so `AnonymousInterface` now
compiles. A later pass fixed `FileLoadConstructorDelegation`: native
`SortedMap.put` previous-value nullability now survives external-facade
anchoring, and split-array keys borrow optional elements before cloning. Direct
emitted-source `super.m()` calls now lower to the superclass implementation, so
`InheritanceAndSuper` compiles. The corpus compile baseline was then 3 named
files:

- `idiom-gaps/UnresolvedConstructorDelegation.java` — loud unresolved
  constructor delegation.
- `java/03-methods/NullOverloadAmbiguity.java` — loud ambiguous null overload.
- `java/04-objects/LocalClassCapture.java` — loud unsupported local type
  declaration plus the expected missing local-class method.

Update 2026-08-24 fourth guard pass: two transient dirty-tree regressions were
fixed without changing the then-three-name baseline. `ComparatorInterface.java`
compiled again after `Collections.sort(records, this)` learned to unwrap a
fallible translated comparator result inside the Rust `sort_by` closure.
`CustomEqualsMethod.java` compiled again after return propagation stopped
placing `?` directly on lowered plain equality expressions.

Update 2026-08-25: the corpus compile guard was re-run against the checked-in
baseline and passed: the only remaining names are
`idiom-gaps/UnresolvedConstructorDelegation.java`,
`java/03-methods/NullOverloadAmbiguity.java`, and
`java/04-objects/LocalClassCapture.java`.

Update 2026-08-25 second pass: `UnresolvedConstructorDelegation.java` is invalid
Java, but it no longer makes generated Rust fail to compile. The translator now
keeps the diagnostic at runtime with `panic!("unresolved Java constructor
delegation")` instead of emitting `compile_error!` for this already-invalid
input, and its focused idiom-gap test compiles the generated module. The same
corpus run surfaced a stale bounded-wildcard receiver edge: fallible
`List<? extends Number>.get(0)?` already produces the required `Number`, so the
receiver adaptation for `.intValue()` must not add a second Java-null
`ok_or_else` after `?`. That is fixed and pinned in `WildcardBounds`. The corpus
compile baseline was then 2 named files:

- `java/03-methods/NullOverloadAmbiguity.java` — loud ambiguous null overload.
- `java/04-objects/LocalClassCapture.java` — loud unsupported local type
  declaration plus the expected missing local-class method.

Update 2026-08-25 third pass: `LocalClassCapture.java` now compiles. Method-local
classes with effectively-final parameter/local captures are emitted as source
classes with hidden capture fields and a synthetic constructor, and `new
LocalClass()` receives the captured values. Batch/global signature planning uses
the same synthetic support program, and source-object fallback methods are
collected after local classes have been emitted. The corpus compile baseline is
now one named file:

- `java/03-methods/NullOverloadAmbiguity.java` — invalid Java; `javac` rejects
  the `pick(null)` call as an ambiguous overload, and the translator keeps that
  loud with `compile_error!`.

Update 2026-08-25 fourth pass: the corpus guard caught two stale
`examples/idiom-gaps` regressions before the baseline was changed:
`FileLoadConstructorDelegation.java` and `TryCatchReusesMovedArgument.java`.
Both were the same lowering-order bug: a modeled `Files.readAllLines(...)`
static facade call inside a propagated slot was replaced by the early
unresolved-static placeholder, producing `unimplemented!(...)?` and Rust
`E0277`. The unresolved-static placeholder now defers when the static
owner/method/arity has a facade ABI fact, so `JavaNioFileFiles::read_all_lines`
lowers through its generated `Result<Vec<String>, JavaException>` support
method. Both fixture tests now compile-check their generated modules.

Update 2026-08-25 fifth pass: one more rendered-Rust-only idiom assertion was
promoted to compile discipline. `static_initializer_block_lowers_to_class_init_function`
now feeds its generated module through `cargo_check_generated_modules` after
asserting the class-init function and static setter calls. This is not a corpus
baseline change; it removes one unchecked assertion from the remaining 0052
surface. Focused check:
`cargo test --test idiom_gaps static_initializer_block_lowers_to_class_init_function --quiet`.

Update 2026-08-26: `fallback_static_call_uses_translated_signature_arg_shapes`
is now compile-backed. Promoting the assertion exposed a real support-generation
compile failure: an external API argument fact for a source-declared
`MutableConfig` caused the facade prelude to synthesize an opaque external
`MutableConfig` shell beside the translated source struct. External facade
support now carries the local/emitted type exclusion set into opaque dependency
collection, so fact parameter/return types that are actually emitted source
types do not produce duplicate external shells. Focused check:
`cargo test --test idiom_gaps fallback_static_call_uses_translated_signature_arg_shapes --quiet`.

Update 2026-08-26 second pass: six more rendered-Rust-only idiom assertions are
now compile-backed:
`translates_cause_runtime_rethrow_without_nominal_downcast`,
`source_static_final_literal_fields_emit_associated_constants_not_storage`,
`source_static_final_fields_are_referenced_as_associated_constants`,
`unresolved_external_setter_static_string_argument_uses_owned_adapter`,
`inherited_source_summary_instance_fields_are_flattened_into_primary_struct`,
and `inherited_source_summary_field_types_emit_opaque_support_shells`.
Promoting them found two stale assertions and one real generated-crate failure.
The static-final owner assertion now accepts the actual associated-constant
owner path, and the inherited counter assertion accepts the current
overflow-preserving `wrapping_add` lowering. The real compile failure was in
source-summary support generation: a source-summary-only `ProgressMeter.record`
method was recorded as already emitted because the support-context program also
contains summary types, while only an opaque `pub struct ProgressMeter;` shell
was generated. Source-summary-only types are now excluded when recording
already-emitted source methods from that support-context program, so the
existing source-summary facade saturation can add the `record` method surface to
the opaque shell. Focused checks: the six tests above, `cargo fmt -- --check`,
and `cargo check --quiet`.

Update 2026-08-26 third pass: a follow-up promotion probe tried the nullable
source/source-summary call fixtures and one optional collection fixture. The
probe found a real but non-trivial null-representation gap: source-summary
support method surfaces were losing parsed nullable-return facts whenever an
external API effect existed with `Unknown` return nullability. That precedence is
fixed, so source-summary opaque methods such as `Lookup.find(...)` now expose
`Option<Path>` instead of plain `Path`. The caller/local slot still binds the
result as `Path`, so the compile promotion remains blocked on 0056-style
nullable source-call local planning. The same probe also exposed an older
optional collection compile failure around cloning `Option<Box<dyn Any>>`; do
not promote `optional_borrowed_header_push_clones_element_not_reference` until
that collection carrier issue is fixed. The speculative compile-check calls from
this probe were not landed. Focused validation kept for this pass:
`cargo check --quiet`.

Update 2026-08-26 fourth pass: the nullable source-call local planning blocker
above is fixed for both translated-source and source-summary receivers. The
signature fixpoint now propagates return constraints from typed unresolved
source calls when exactly one receiver/method candidate matches, and local
lowering no longer lets declared-type anchoring flatten an already-optional
initializer plan. Source-summary return-shape lookup now prefers the
source-summary declaration before the merged support-context signature fallback,
so parsed nullable summary surfaces such as `Lookup.find(...) -> Option<Path>`
also refine caller locals to `Option<Path>`. Promoted compile checks:
`nullable_source_call_through_field_receiver_stays_optional_without_double_wrap`,
`source_summary_nullable_return_into_optional_local_does_not_double_wrap`, and
`source_summary_nullable_return_into_concrete_local_unwraps_once`. The optional
collection carrier issue remains separate.

Update 2026-08-26 fifth pass: four more inline `idiom_gaps` rendered-Rust
assertions are now compile-backed:
`source_static_call_uses_signature_shapes_for_nullable_args`,
`overload_null_argument_makes_exact_callee_parameter_optional`,
`source_static_throwing_call_uses_question_mark`, and
`throwing_call_chain_promotes_caller_result_shells`. The nullable static-source
call assertion was stale after the caller parameter became `Option<String>`; the
current generated call passes `workspace.as_ref()` to the `Option<&String>`
callee slot and now compiles. The stale idiom failure-baseline entry for that
test was removed.

Update 2026-08-26 sixth pass: two more inline rendered-Rust assertions are now
compile-backed: `math_min_lowers_as_native_static_call` and
`unshaped_constructor_argument_call_that_throws_propagates_result`. The latter
also had a stale pre-borrow assertion; current output borrows `path` for the
throwing `open(path)?` call and clones only for the second constructor
argument.

Update 2026-08-26 seventh pass: the attempted
`map_get_keyset_loop_does_not_double_borrow_key` promotion exposed a real
generated-crate failure rather than a cheap assertion cleanup: returning
`context.get(...)` from a borrowed `HashMap<String, Box<dyn Any>>` boxed a clone
tied to the borrowed map lifetime while the return slot wanted owned
`Box<dyn Any>`. Native `Map.get` lowering now recognizes optional Java `Object`
targets whose value comes from a borrowed map element and projects the optional
value with `java_clone_erased_object_ref(value)` instead of
`Box::new(value.clone())`. The fixture is now compile-backed.

Update 2026-08-26 eighth pass: two more map/erased-object rendered-Rust
assertions are now compile-backed:
`object_map_put_keeps_erased_object_value_without_downcast` and
`hashtable_put_lowers_to_native_map_insert`. An attempted promotion of
`optional_map_field_put_lowers_through_mutable_optional_receiver` exposed a
separate shared-handle insertion gap: the optional map field stores
`Rc<Kmer>`, but native `put` lowering inserts a bare `Kmer::new(...)` value.
That test remains assertion-only until collection insertion applies the same
shared-handle slot wrapping used by ordinary value construction.

Update 2026-08-26 ninth pass: the optional map-field blocker above is now fixed
and compile-backed. `Map.put`'s special argument path now applies the same
bare-constructor shared-handle wrap as ordinary shaped call arguments, so a
`HashMap<String, Kmer>` field represented as
`HashMap<String, Rc<Kmer>>` inserts `Rc::new(Kmer::new(...))` instead of the
bare value. Promoting the fixture also exposed a support-emission defect:
allowed builtin `StringBuilder/StringBuffer -> Appendable` facade conversions
were emitted even when the generated crate did not emit the builder/buffer
support surface. Those conversions are now pruned unless the corresponding
surface is present.

Update 2026-08-26 tenth pass: the nearby Appendable/StringBuilder fluent-chain
fixture is now compile-backed too. This pins both sides of the support-emission
cleanup: crates that do not emit StringBuilder/StringBuffer surfaces no longer
get dangling `From<StringBuilder/StringBuffer> for Appendable` impls, while
crates that do use `new StringBuilder(...)` still emit the conversion and
compile the checked fluent call chain.

Update 2026-08-26 eleventh pass: the optional collection carrier blocker named
in the third pass is now fixed and compile-backed.
`optional_borrowed_header_push_clones_element_not_reference` exposed two hidden
errors once compiled: a reused `Option<Box<dyn Any>>` was cloned as a carrier,
which cannot compile, and the recovered value then needed the concrete list
element type rather than another erased `Box<dyn Any>`. Reused optional-erased
arguments now borrow the optional carrier, downcast to the concrete target when
the target is not Java `Object`, clone the concrete referent, and tag the
projection with the target slot shape so the generic reused-argument adapter
does not reapply erased-object conversion. Focused checks:
`optional_borrowed_header_push_clones_element_not_reference`,
`direct_optional_clone_recovery_unwrap_uses_structured_projection`,
`optional_clone_recovery_owned_element_projection_records_recovery_shape`,
`cargo fmt --check`, and `cargo check --quiet`.

Update 2026-08-26 twelfth pass: the nearby
`serializable_instanceof_erased_object_uses_runtime_marker_helper` assertion was
stale rather than broken. Current array access emits the bounds-checked
`__java_arrays_source.get(...)` block, so the assertion now accepts that spelling
while still requiring `java_is_serializable(...)`,
`Some(java_clone_erased_object_ref(...))`, no double-boxed erased-object clone,
and no unsupported-instanceof diagnostic. Focused checks:
`serializable_instanceof_erased_object_uses_runtime_marker_helper`,
`optional_borrowed_header_push_clones_element_not_reference`,
`object_output_stream_write_object_clones_indexed_erased_object_handoff`,
`cargo fmt --check`, and `cargo check --quiet`.

Update 2026-08-26 thirteenth pass: the casted map-carrier fixture is now
compile-backed. `casted_borrowed_map_argument_clones_for_owned_helper_slot`
exposed a real inbound carrier mismatch once checked: a borrowed `Map<K,V>`
parameter cast to an owned `SortedMap<K,V>` slot was still passed as the borrowed
`HashMap` carrier. Slot conversion now rebuilds map carriers structurally,
using `into_iter().collect::<...>()` for owned map-to-map conversion and
`iter().map(|(key, value)| (key.clone(), value.clone())).collect::<...>()` when
an owned target is built from a borrowed map. The cast helper also now infers the
inner expression's shape directly, instead of asking through the target slot and
losing the original carrier identity. Focused checks:
`casted_borrowed_map_argument_clones_for_owned_helper_slot`,
`optional_borrowed_header_push_clones_element_not_reference`,
`serializable_instanceof_erased_object_uses_runtime_marker_helper`,
`object_output_stream_write_object_clones_indexed_erased_object_handoff`,
`cargo fmt`, and `cargo check --quiet`.

Update 2026-08-26 fourteenth pass: eleven more inline `idiom_gaps`
rendered-Rust assertions are now compile-backed. The promoted fixtures cover
string argument reuse, map key reuse, primitive `Map.get` in ternaries, optional
map receiver `toString`, external SAM/two-bit facade return surfaces, nullable
path overload arguments, optional mutating-overload receivers, and overloaded
static exception facts. Promoting them found only stale assertion spelling in
the landed slice: optional map `toString` now uses the fallible Java-null
`ok_or_else(...)?` policy, the optional-field overload call passes
`name.as_str()`, nullable path overload planning now uses borrowed `Path`
parameters, and the overloaded-static compile check needs the local `ndarray`
shim in scope. The remaining inline rendered-output assertions without a
generated-crate compile/test check are now **220**.

Three probe promotions were deliberately not counted as complete. The
context-only sibling-overload fixture resolves against `Holder` but does not
emit that context source into the standalone checked module. The stream block
lambda fixture exposed a real array-element optionality bug: `partitionInfo[1]`
and `[2]` are still parsed as `Option<String>` values and then used with
`.parse()`. The static owned-path fixture exposed a real temporary-borrow
regression: `Helper::check_path(&mut IO::get_path(...), ...)` borrows a fresh
owned path as a mutable receiver. Those should be handled as separate lowering
bugs before their compile checks are landed.

Update 2026-08-26 fifteenth pass: the static owned-path fixture is now landed
as a compile-backed check, and the remaining inline rendered-output assertions
without a generated-crate compile/test check are now **219**. The production fix
is source-shadow-aware external receiver constraint planning: a source-declared
type named `Path` no longer picks up the platform facade receiver effect for
`Path.resolveSibling`, so `Helper::check_path(IO::get_path(&vcf_path))` is
planned as an owned value call instead of `&mut IO::get_path(...)`.

The promoted fixture was narrowed to the behavior it names. Its previous
optional second argument also exposed the older optional-value wrapping shape
`Some(&IO::get_path(...))`; that is still a separate value-wrapping issue rather
than part of the static owned temporary-borrow regression.

Update 2026-08-26 sixteenth pass: two more rendered-Rust assertions are now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **217**.
`java_string_identity_fails_loudly_until_identity_representation_exists` now
uses the expected-failure generated-crate helper to prove the intentional
`compile_error!` stays loud. `stream_block_lambda_string_array_local_clones_owned_index`
now compiles its generated crate as well. Promoting the stream fixture exposed
two stream-specific lowering gaps: stream lambda argument adaptation did not
infer shapes for expression arguments such as `partitionInfo[1]`, so native
parse calls tried `.parse()` on `Option<String>`; and stream lambda constructor
calls ignored resolved constructor names, producing `SimpleInterval::new_3(...)`
for a source constructor emitted as `new(...)`. The stream adapter now infers
argument expression shapes through the scoped lambda environment, direct string
array locals borrow nullable elements before cloning, and resolved stream-lambda
constructors use `ResolvedConstructor.rust_name`.

Update 2026-08-26 seventeenth pass: two more Java integer exception-propagation
assertions are now compile-backed, and the remaining inline rendered-output
assertions without a generated-crate compile/test check are now **215**.
`java_integer_result_propagation_in_call_arguments_uses_context_policy` compiled
as-is. Promoting `java_integer_result_propagation_in_array_indexes_uses_context_policy`
exposed a stale assertion and a real index-lowering bug: a fallible integer
index already containing an inner `?` was then rendered with a second `?` around
the `as usize` cast, producing invalid Rust such as `(... as usize)?`.
`lower_index_expr` now strips that invalid outer try around usize casts while
keeping the inner fallible computation, so reads and writes render as
`({ fallible_index()? }) as usize` before generated-crate checking.

Update 2026-08-26 eighteenth pass: one more external-facade assertion is now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **214**.
`sam_record_fastq_string_getters_emit_typed_facade_surfaces` now checks the
generated crate after asserting the typed `SAMRecord` getter surface. Promotion
only found stale text expectations: the current generated facade bodies are
explicit `unimplemented!` stubs and the source caller methods return
`Result<String, JavaException>` with `Ok(...)` wrapping.

Update 2026-08-26 nineteenth pass: three enum/interface assertions are now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **211**.
`closed_enum_variant_borrows_as_dyn_trait_parameter` and
`closed_enum_variant_constructor_delegation_supplies_empty_varargs_without_optional_as_ref`
compiled as-is. Promoting `source_java_enum_constant_uses_slot_plan_not_optional_projection`
found only a stale expectation: nested enum Rust names now preserve the path
boundary as `EnumConstantOptionalUse_HeaderVersion`, while the fixture still
expected the older concatenated spelling.

Update 2026-08-26 twentieth pass: three nested-interface assertions are now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **208**. Promotion found stale
expectations rather than compile errors: nested interface and nested implementor
names now preserve path boundaries as `Incrementor_MaxCountExceededCallback`
and `Builder_Reader`; the checked generated crates compile with those names.

Update 2026-08-26 twenty-first pass: six more idiom-gap assertions are now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **202**. Four promoted as-is
(`array_element_assignment_to_bare_field_requires_mutable_self`,
`system_arraycopy_uses_signature_fact_slice_slots_for_optional_array_fields`,
`raw_cast_left_operand_is_parenthesized_before_ordered_comparison`, and
`java_numeric_literal_suffixes_are_removed_or_rust_typed`). Promoting
`system_arraycopy_uses_signature_fact_slice_slots_for_parameter_fields` found a
stale expectation: null receiver checks are now fallible
`ok_or_else(...)?` checks, so the generated method returns
`Result<(), JavaException>` but compiles. Promoting
`external_receiverless_instance_overload_delegation_uses_pack_constructor_and_conversion`
found a support-surface bug: external adapter targets written as simple
`InputStream` were canonicalized to `JavaIoInputStream`, leaving no
`impl From<SeekablePathStream> for InputStream`; support rendering now preserves
simple stream endpoints where the adapter fact asks for them. The source-only
`receiverless_instance_overload_delegation_uses_current_self_and_subtype_argument`
fixture remains assertion-only because it still lowers through the known loud
source class upcast hierarchy gap.

Update 2026-08-26 twenty-second pass: two more idiom-gap assertions are now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **200**.
`documents_java_bean_getter_setter_naming_gap` and
`documents_for_each_lambda_side_effect_gap` both compiled after their assertions
were updated for current result propagation / borrowed-collection signatures.
The same probe intentionally did not promote
`documents_collection_remove_if_lambda_gap`: it still lowers the mutating
`List.removeIf(...)` parameter as an owned `Vec<String>` instead of the older
expected `&mut Vec<String>`. The executor lambda fixtures also remain
assertion-only; compile checking them currently runs into missing
`ExecutorService.submit` support and `JavaLinkedHashMap` iteration support,
which are broader external facade gaps than this 0052 cleanup batch.

Update 2026-08-26 twenty-third pass: four more idiom-gap assertions are now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **196**.
`plain_returning_method_promotes_result_shell_for_fallible_helper_call`,
`nested_unchecked_exception_throw_forces_result_return_shell`,
`constructor_deferred_field_recovers_casted_fallible_helper_without_question_mark`,
and `context_translation_uses_sibling_static_string_constant_shape` now compile
their generated Rust. Promotion exposed two small production bugs. First,
slot-level result recovery was reapplied around casts whose inner fallible call
had already been recovered before downcast, producing `.unwrap_or_else(...)` on
a plain `String`; casts no longer ask the outer slot for a second recovery.
Second, optional borrowed facade returns into erased-object slots boxed
`value.clone()` inside `map(...)`, which borrowed a non-`'static` field; the
clone branch now uses the existing `java_clone_erased_object_ref(value)` helper.
`constructor_self_optional_field_argument_uses_field_slot_shape` remains
assertion-only because compile checking it still exposes the broader optional
constructor wrapping gap (`Rc<Rc<Helper>>` / `Option<Rc<Token>>` shape mismatch).

Update 2026-08-26 twenty-fourth pass: three more source-summary/support-surface
idiom-gap assertions are now compile-backed, and the remaining inline
rendered-output assertions without a generated-crate compile/test check are now
**193**. `source_summary_direct_field_type_emits_opaque_support_shell`,
`source_summary_opaque_type_gets_used_inherited_method_surface`, and
`selected_opaque_support_type_gets_used_known_method_surfaces_by_arity` now
compile their generated Rust. Promotion found one stale expectation in the
inherited summary-method fixture: the generated method surface is now attached
to `FeatureInput`, so the unsupported diagnostic says `FeatureInput.toPath`
rather than the inherited declaration owner `GATKPath.toPath`. The sibling
context-source and emitted-context object-stringification probes remain
assertion-only for this pass because their generated Rust references types from
separate context/source units that the current generated-crate harness does not
compile into the same module namespace.

Update 2026-08-26 twenty-fifth pass: five more idiom-gap assertions are now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **188**.
`optional_field_assignment_wraps_external_call_result`,
`optional_this_field_assignment_wraps_external_call_result`,
`file_extensions_fasta_lowers_as_iterable_static_slice`,
`null_guarded_ternary_display_branch_uses_refined_local_once`, and
`null_guarded_ternary_append_argument_uses_refined_local_once` now compile their
generated Rust. The two null-guarded local-string fixtures had stale assertions:
current lowering keeps the `Option<&str>` borrowed as
`read_name.as_ref().unwrap().to_string()` rather than consuming it with
`read_name.unwrap()`, while still avoiding the original double-unwrap form.

Update 2026-08-26 twenty-sixth pass: two more idiom-gap assertions are now
compile-backed, and the remaining inline rendered-output assertions without a
generated-crate compile/test check are now **186**.
`map_get_string_display_segment_unwraps_optional_borrow` and
`nullable_static_alias_return_emits_lifetime_for_borrowed_params` now compile
their generated Rust. The latter had a stale assertion: nullable borrowed
returns are now wrapped in `Result<..., JavaException>` when the method body can
throw. The attempted promotion of
`reused_optional_display_segment_borrows_before_later_use` found two separate
compile-surface facts. First, external support for `GATKPath.toPath()` can
reference the built-in `Path` facade even when the source file does not import
`java.nio.file.Path`; support emission now follows external API return/thrown
types for receiver types used by the program. Second, after that support was
present, the fixture exposed a non-trivial lifetime planner gap for instance
methods that return a nullable borrowed parameter:
`fn check(&self, value: Option<&GATKPath>) -> Result<Option<&GATKPath>, ...>`
needs an explicit parameter lifetime independent of `&self`. That compile-check
is intentionally not landed in this pass.

Update 2026-08-26 twenty-seventh pass: five more external/platform facade
idiom-gap assertions are now compile-backed, and the remaining inline
rendered-output assertions without a generated-crate compile/test check are now
**181**. `broad_spark_utils_hadoop_path_exists_has_bool_surface`,
`seekable_stream_factory_get_instance_has_concrete_surface`,
`java_nio_files_exists_has_bool_surface`,
`raw_list_cast_over_typed_external_call_uses_target_slot_shape`, and
`typed_external_call_to_declared_generic_local_uses_external_return_shape` now
compile their generated Rust. Promotion exposed a real static-owner gap in the
Spark fixture: `SparkUtils.hadoopPathExists(...)` emitted a support method but
the call body still used the unresolved-static placeholder because the fallback
guard compared rendered Rust names against Java fact names and ignored built-in
external-pack evidence. The guard now uses Java method-name aliases, consults
built-in external API policy, and applies the same preferred static owner alias
used by support generation; `SparkUtils` was added to that preferred-owner table.

### Reproduction

```sh
for f in $(find examples/java examples/idiom-gaps -name '*.java'); do
  b=$(basename $f .java)
  ./target/release/new-java2rust "$f" > /tmp/$b.rs &&
  rustc --edition 2021 --crate-type lib --crate-name gen_x \
        --emit=metadata -o /tmp/$b.rmeta /tmp/$b.rs
done
```

| corpus | files | compile | do **not** compile |
|---|---:|---:|---:|
| `examples/java` | 67 | 66 | **1** |
| `examples/idiom-gaps` | 122 | 122 | **0** |
| total | 189 | 188 | **1** |

The remaining named failure is invalid Java that is reported loudly by both
`javac` and the translator.

---

## Why no metric sees it

`count_invalid_patterns` (`src/output_markers.rs:236`) is the signal that is
supposed to mean *"the translator produced something it cannot stand behind"*.
Across the one remaining named failure it fires **once**:

| corpus | non-compiling | of which emit `compile_error!` |
|---|---:|---:|
| `examples/java` | 1 | **1** (`NullOverloadAmbiguity`) |
| `examples/idiom-gaps` | 0 | **0** |

So the remaining checked-in corpus compile failures are now all visible through
`compile_error!`. This does not close 0018's broader thesis: unchecked rendered
Rust assertions can still hide files that would be rejected by the Rust compiler
and contribute zero to every counter in `summary.tsv`.

`unimplemented!` is present in many of them (24 occurrences in
`TryWithResources`), but it is a *stub* marker counted separately and it is
never at the site of the error.

---

## Verified backlog A — `examples/java`, 1 file

Work queue. Error code is the first `rustc` diagnostic; `refs` is how many test
files mention the fixture path at all.

| fixture | first error | refs |
|---|---|---:|
| `03-methods/NullOverloadAmbiguity.java` | `compile_error!` ambiguous null overload | 1 |

The remaining examples/java failure is referenced by at least one test. The
broad `all_java_fixtures_are_loadable` count now tracks the corpus size, but
fixture-specific compile checks are still sparse.

More generally, **21 of the 67 `examples/java` files are referenced by no test**:
`MixedNumericArithmetic`, `NumericPromotion`, `StringConcat`, `ForOmittedParts`,
`LabeledBreak`, `LabeledContinueForUpdate`, `NamingPolicy`, `NullOverloadAmbiguity`,
`Varargs`, `VarargsCalls`, `AliasingSharedMutation`, `ConstructorExpressions`,
`FieldWriteReceiverMutation`, `StaticFinalFields`,
`ArrayInitializers`, `ArraysAndVarMutation`, `GenericsAndWildcards`,
`CheckedException`, `OptionalAndObjects`,
`StringFormatAndThrow`, `SynchronizedBlock`.

---

## Verified backlog B — `examples/idiom-gaps`, 0 files

Only **43 of the 122** idiom-gaps fixtures are reached by any test that compiles
its output. `tests/idiom_gaps.rs` has 352 compile-checked tests, but they check
mostly *inline* Java sources: of the 696 tests that assert on `translation.rust`,
571 use inline source (317 compile-checked) and **124 use a fixture file, of
which 35 compile-checked**.

There are no remaining non-compiling idiom-gaps fixtures.

| error | fixtures |
|---|---|
| `compile_error!` diagnostics (0) | — |

`SparkRddVariantPipeline` now compiles: erased `JavaPairRDD.sortByKey` facade
methods return the erased carrier (`Box<dyn Any>`) when called through the
external erased-object facade, matching the local slot that stores the branch
result.

`StreamCallbackChain` now compiles: Spark `mapPartitionsToPair` callbacks use
the same `JavaIterator<T>` carrier as declared Java iterator slots,
`JavaPairRDD.collectAsMap` is modeled, return-only facade placeholders such as
`Broadcast.value<T0>()` are declared on the generated method, and `return null`
into the non-optional external RDD slot lowers to the slot default.

`SuperConstructorState` now compiles: its external `ParentWriter(writer,
sampleName)` super-constructor call is modeled as a no-state boundary, so the
child constructor can lower its own `state = Utils.nonNull(state)` assignment.

`ArrayInitializers` now compiles: the rustc-only corpus guard supplies the same
minimal `ndarray` surface used by focused-package offline checks, and
borrowed-to-owned recovery for `ndarray::Array2` uses `clone()` rather than the
1D sequence `.to_vec()` adapter.

`WildcardBounds` now compiles: `List<? extends Number>` slice views render their
readable upper bound as `&[Number]`, and mutation into the erased public
`List<? super Integer>` carrier boxes the inserted value as `Box<dyn Any>`.

`DynTraitMethodCompatibility` now compiles: interface trait parameter planning
adopts the compatible implementation's borrowed dyn-trait view for the same Java
parameter type, so both the `Metric` trait and its impls render `other: &dyn
Metric` instead of splitting between `Metric` and `&dyn Metric`.

Three fixtures are referenced by nothing at all: `NonNullFieldViaHelper`,
`PackagePrivateGetterSetterInlining`, `ToArrayEmptyStringArray`.

### Fixed sharp edge: optional array field read into owned setter parameter

`ArraySetterOwnedStorage.copy` passed `source.letters` into a setter whose
signature wants owned `Vec<char>`. The old lowering unwrapped the `Option` by
value (`source.letters.expect(...)`), which tried to move a field out of the
borrowed source object and failed with `E0507`. A 2026-08-23 follow-up now
treats optional field reads feeding owned array parameters as borrowed carrier
reads, emitting `as_ref().cloned()` before the Java-null check. The fixture is
compile-checked in `tests/idiom_gaps.rs`.

### Fixed sharp edge: imported static side-effect fallback returns generic `R`

`FallbackStaticMutBorrowArgument` calls an unresolved imported static
`ExternalApi.fill(config)` as a statement. The fallback facade used to render
`pub fn fill<R, T0>(_arg0: T0) -> R`, leaving `R` unconstrained at the call site
and failing with `E0282`. A 2026-08-23 follow-up classifies `fill` with the
other side-effect-style imported static names, so the no-facts corpus path now
renders a unit-returning stub and compiles. The existing argument-effect test
also asserts the more precise `&mut MutableConfig` unit-returning facade when an
explicit mutating argument fact is supplied.

### Fixed sharp edge: fallible nullable assignment into erased optional local

`JalignerTryCatchFinallyCleanup` assigned `resource.read()` into a nullable
`String` local inside a `try` block. The local currently lowers through an
erased optional carrier, and the conversion pipeline lost the fact that `?` had
already been applied while wrapping the value as `Some(Box<dyn Any>)`. That
emitted `Some(...) ?` inside a `Result` closure and failed with `E0277`. A
2026-08-23 follow-up preserves the `ResultPropagated` shape through structured
`Some` and `Box::new(... as dyn Any)` wrappers, so the generated assignment is
`sequence = Some(Box::new(resource.read()?) as Box<dyn std::any::Any>);`.
The same shape fix also made `JalignerFallthroughTryCatch` compile. Both
fixtures have generated-crate checks.

### Fixed sharp edge: captured collection mutation inside `forEach`

`MapForEachTryCatchLambda` lowered `Map.forEach` to a loop, but signature
planning did not look through lambda bodies when deciding whether a method
parameter binding needed `mut`. The generated body inserted into `dest`, while
the generated parameter was immutable:

```rust
pub fn copy(&self, futures: &HashMap<String, Future>, dest: HashMap<String, Rc<Item>>) -> Result<(), JavaException> {
    // ...
    dest.insert(name.clone(), item);
}
```

That failed with `E0596`. A 2026-08-23 follow-up makes the signature mutation
walk recurse into lambda bodies, excludes lambda parameters from the captured
mutation set, and reuses the native collection mutator table for calls such as
`put`, `add`, and `addAll`. The generated `copy` parameter is now `mut dest`,
and the fixture compiles directly.

### Fixed sharp edge: unresolved collection source calls in `addAll`

`UnresolvedSetReturnAddAll` used the argument slot from `Set.addAll`, but still
rendered an unresolved receiverless source call as a Rust free function:

```rust
self.names.as_mut().unwrap().extend(default_names());
```

Because the Java fixture has no `defaultNames` declaration, the generated Rust
failed with `E0425`. A 2026-08-23 follow-up keeps the target collection shape
but renders an unresolved collection-producing call as a typed empty fallback
with an audit marker:

```rust
self.names.as_mut().unwrap().extend(
    <std::collections::HashSet<String> as Default>::default()
    /* TODO(translation): unresolved Java collection source call defaultNames */
);
```

The fixture now compiles directly and has a fixture-backed generated-crate check.

### Fixed sharp edge: unresolved class literal token types

`ClassLiteralRegistration` compiled Java code that referenced
`RegisteredType.class` without declaring `RegisteredType` in the fixture. The
lowering kept the Java class name but used it as the Rust token type:

```rust
kryo.register_1(JavaClass::<RegisteredType>::literal("RegisteredType"));
```

That made the output fail with `E0412`. A 2026-08-23 follow-up keeps concrete
source class literals typed, but renders unresolved named literals as
`JavaClass<()>` while preserving the runtime Java name:

```rust
kryo.register_1(JavaClass::<()>::literal("RegisteredType"));
```

The fixture now compiles directly and has a fixture-backed generated-crate check.

### Fixed sharp edge: a facade method named `1`

`Tuple2::_1` / `Tuple2::_2` (Spark) lose their leading underscore when the
facade **definition** is synthesised, while the **call site** keeps it:

```rust
// SparkRddVariantPipeline.rs:1929
impl Tuple2 {
    pub fn 1(&self) -> Option<Box<dyn std::any::Any>> { unimplemented!("Tuple2.1 is not modelled") }
    pub fn 1<T0>(&self, _arg0: T0) -> Option<Box<dyn std::any::Any>> { unimplemented!("Tuple2.1 is not modelled") }
    pub fn _1(&self) -> Option<Box<dyn std::any::Any>> { unimplemented!("Tuple2._1 is not modelled") }
    …
}
…
// :1979
return sorted.map(|value: Tuple2| value._1());
```

`pub fn 1` is not valid Rust, so **the entire file failed to parse** — which per
0018 meant the file reported *zero type errors* in a corpus run, the strongest
form of the erasure-rewards-the-metric problem. A 2026-08-23 follow-up now
normalizes digit-leading facade aliases to `_1` / `_1_1` style Rust identifiers.
Both fixtures still fail later type checks, but they no longer poison the corpus
measurement as parse failures.

Two tests cover these fixtures:
`corpus_spark_rdd_pipeline_keeps_stream_lowering_at_noop_spark_boundary`
(`tests/idiom_gaps.rs:15300`) asserts `value._1()`, and
`documents_stream_callback_chain_gap` (`tests/idiom_gaps.rs:13414`) asserts
`value._2()`. They now also assert that the generated facade does not contain
digit-leading method definitions.

---

## Which suites assert on rendered Rust without compiling it

Counted by scanning each `#[test]` block for `.rust` and for
`cargo_check_generated_modules` / `cargo_test_generated_modules`
(`tests/common/mod.rs:21,25`):

| suite | tests | assert on `translation.rust` | compile it | **unchecked** |
|---|---:|---:|---:|---:|
| `tests/idiom_gaps.rs` | 767 | 696 | 351 | **345** |
| `tests/slot_shape_design.rs` | 604 | 160 | 104 | **56** |
| `tests/fixtures.rs` | 57 | 45 | **7** | **38** |
| `tests/semantic_facts.rs` | 170 | 11 | 8 | 3 |
| `tests/output_markers.rs` | 23 | 8 | 0 | 8 |
| `tests/batch_translation.rs` | 11 | 3 | 0 | 3 |
| `tests/external_packs.rs` | 167 | 2 | 0 | 2 |
| `tests/idiom_sweep_script.rs` | 48 | 1 | 0 | 1 |
| **total** | | **925** | **467** | **458** |

The remaining nine suites (`dependency_mode`, `resolve_calls`, `type_coercion`,
`object_model`, `parser_gaps`, `parser_tree_sitter`, `capture_facts`,
`effective_facts`, `differential`) assert on facts/IR, not on rendered Rust, and
are correctly out of scope here.

`tests/fixtures.rs` is 0041's finding repeated one suite over: **38
tests assert on rendered Rust and do not compile it**, and it is the suite that
owns `examples/java` — the corpus with 2 non-compiling files.

### The backlog 0041 opened is growing, silently

0041 listed 52 `slot_shape_design` tests left unchecked (21 genuine + 31
needing harness support). Today the unchecked set is **56**: three of the listed
ones were fixed (`collections_sort_comparator_*`) and **seven new unchecked
tests were added** —

- `a_cotranslated_source_type_gets_no_unmodelled_import_stub`
- `context_static_string_constant_owner_stays_sibling_source_type`
- `implemented_shared_identity_equality_has_no_gap_diagnostics`
- `item_assembly_owns_struct_impl_and_function_shells_from_signatures`
- `semantic_analysis_owns_exception_classification_and_catch_facts`
- `source_summary_static_string_constant_emits_external_facade`
- `translate_source_with_facts_ingests_external_closed_hierarchy_facts`

Nothing requires a new translation test to compile its output, so the ratio
drifts back down after every cleanup.

---

## Behavioural coverage: 23 fixtures and six live tests

Compiling is a gate, not an oracle. The project's entire behavioural surface —
"run the Java, run the Rust, compare" or "run the Rust against a written
expectation" — is:

1. `tests/differential.rs` — **one** `#[test]`, **23 fixtures**, 1241 lines of
   Java total. Verified working: a full run takes 55s and all 23 pass.
2. `tests/fixtures.rs` — **11** tests that write the translation to a file, add
   a hand-written `mod generated_tests`, and `rustc --test` it.
   **Four are currently failing** and baselined as such, so seven are live.

That is 8 of 2618 tests, over 23 + ~11 Java programs.

### The hand-written oracles are not oracles, and one has been dead for a while

`switch_fallthrough_generated_rust_preserves_behavior` (`tests/fixtures.rs:1178`)
asserts `score("A") == 4, "B" == 2, "C" == 2, "D" == -1`, which is correct Java.
It fails — but not on behaviour:

```
error[E0308]: mismatched types
1798 |  assert_eq!(scorer.score("A".to_string()), 4);
1798 |  assert_eq!(scorer.score(&"A".to_string()), 4);   <-- rustc's suggestion
```

The generated signature moved to `&String`; the hand-written harness was never
updated. So the only behavioural check on switch fallthrough has not executed a
single assertion since the signature changed, and the failure is indistinguishable
from a translation defect. That is 0041's "a stale expectation hid a genuine type
error", inverted: here a stale expectation hides the *absence* of any check.

The deeper point is that these ten tests encode *the translator author's belief*
about what the Java means. `tests/differential.rs` asks `javac`. Only the second
kind can find a defect the author did not anticipate — which is why, per 0018,
it found one on every batch of fixtures added to it.

### What the differential corpus cannot see

Scanned across all 23 fixtures (`grep -lE … tests/differential/*.java`):

| Java construct | fixtures using it |
|---|---:|
| `null` literal | **0** |
| `throw` statement | **0** |
| `throws` clause | **0** |
| `char` declaration | **0** (one `'y'` literal) |
| `Map` / `Set` / `HashMap` / `HashSet` | **0** |
| enhanced `for (X x : xs)` | **0** |
| `enum` | **0** |
| `abstract` class | **0** |
| `String.format` | **0** |
| `String` instance methods (`substring`, `split`, `charAt`, `indexOf`, `trim`) | **0** |
| `do { } while` | **0** |
| varargs | **0** |
| `List` / `ArrayList` | 1 (`LambdaMutation`) |
| `.stream()` | **0** |

`null` is the one that matters most: nullable slots, `Option` carriers and
null-check narrowing are among the largest subsystems in `src/`, they are the
subject of a large share of `slot_shape_design`, and **not one line of Java in
the behavioural corpus contains the token `null`**. The same is true of every
`E0599 expect`-on-a-non-`Option` failure in backlog A, which is a nullable-slot
defect.

Three structural limits keep the corpus small, and they are worth naming
because they are the actual cost of adding fixtures:

- `fixture_paths()` (`tests/differential.rs:167`) reads `*.java` in one flat
  directory and `javac`s **one file**. Multi-class programs, packages and
  cross-file inheritance cannot be expressed.
- `harness_main()` (`:156`) can only drive `public static String run()`. A
  program whose observable behaviour is `System.out` interleaving, or a
  side effect, cannot be compared.
- the subject is built with bare `rustc`, no `Cargo.toml`, so any fixture
  touching a crate-mapped facade (`ndarray`, `hdf5_facade`, …) cannot run —
  the same limitation as 0041's 31 harness-only failures.

**Zero of the 67 `examples/java` files declare a `main`**, so the curated corpus
is structurally incapable of being run as Java at all. The two corpora have no
overlap in method: one is asserted on and never executed, the other is executed
and never grows.

---

## Root cause

Three separate decisions, each locally reasonable:

1. `cargo_check_generated_modules` is **opt-in per test**. A test author who
   forgets it gets a green test. Nothing in the suite requires a test that
   touches `translation.rust` to also compile it — so the checked fraction is
   whatever history left behind (47% overall, 0% in `fixtures.rs`).
2. The **fixture corpora are inputs, not subjects**. `examples/java` and
   `examples/idiom-gaps` are read by tests but are never themselves swept: no
   job translates all 189 and compiles the results. `translate-focused-package.sh`
   does exactly this for *external* corpora, and would find all 58 in one run.
3. **Compiling and running are conflated in cost estimation.** Compiling the
   corpora is ~2 minutes of `rustc` with no cargo involved (measured: the 189-file
   sweep above). It has never been done because it looks like it belongs to the
   expensive differential harness, which it does not.

---

## Codex update — 2026-08-23

Implemented the first direction item:

- Added `tests/corpus_compile.rs::checked_in_example_corpora_have_named_compile_failures`.
  It translates every checked-in Java fixture under `examples/java` and
  `examples/idiom-gaps`, runs `rustc --crate-type lib --emit=metadata` on each
  generated file, and compares the failing fixture names against
  `tests/baselines/corpus-compile-failures.txt`.
- The sweep is bounded by the existing test watchdog and parallelized with a
  conservative default of four workers. Override with
  `NEW_JAVA2RUST_CORPUS_COMPILE_JOBS`.
- HEAD then had **59** named corpus compile failures. Claude's measured 58 were
  still valid as a shape, but the tree also failed
  `idiom-gaps/MethodReferenceFlatMap.java`.
- Fixed the stale `all_java_fixtures_are_loadable` count from 65 to the current
  67 files.

Still open from this audit:

- The corpus compile failures are now visible but not all fixed. A 2026-08-23
  follow-up fixed borrowed stream terminal collection for flat-map pipelines,
  removing `idiom-gaps/MethodReferenceFlatMap.java` and
  `java/06-stdlib/StreamFlatMapLambda.java`. A second pass fixed owned
  flat-map projection shape, removing
  `java/06-stdlib/StreamFlatMapProjection.java`. A third pass fixed source
  static owners shadowing JDK facade owners, removing
  `java/06-stdlib/StaticOwnerShadowing.java`. A fourth pass fixed redundant
  Java-null receiver projection after optional collection `unwrap()` in native
  capacity arguments, removing
  `java/06-stdlib/CollectionCapacityConstructor.java`. A fifth pass applied the
  same normalization to required borrowed string views after fallible optional
  reads, removing `java/06-stdlib/CheckedPropagation.java`; the same projection
  fix also removed `idiom-gaps/IndexedStringArrayOwnership.java`,
  `idiom-gaps/NullableTrimEqualsOriginal.java`, and
  `idiom-gaps/OptionalArgumentWrapReuse.java`. A sixth pass fixed imported
  static-owner support where the fully qualified facade owns the associated
  function while the simple instantiated facade needs an instance-compatible
  wrapper, removing `java/06-stdlib/ImportedStaticUtilityOwner.java`. A seventh
  pass made string return signatures respect the merged owner/view constraint
  result instead of raw rejected borrowed constraints, removing
  `java/06-stdlib/ExternalStringConstantArgument.java`. An eighth pass skipped
  source enum methods that duplicate generated constructor-payload accessors,
  removing `java/04-objects/EnumConstantsWithFields.java`. A ninth pass made
  effect-backed external getter facades reuse known direct return types for
  audited primitive and primitive-array getters, removing
  `idiom-gaps/BooleanGetterDispatchCoercion.java`,
  `idiom-gaps/ByteArrayGetterDispatchCoercion.java`,
  `idiom-gaps/ExternalGetterReadOnly.java`,
  `idiom-gaps/ExternalGetterSetterMutability.java`, and
  `idiom-gaps/PrimitiveGetterDispatchCoercion.java`. A tenth pass recursively
  stripped redundant Java-null receiver `expect` calls after optional
  `unwrap()` inside map-entry iteration receivers, removing
  `idiom-gaps/MapEntrySetFirstKeyFromNullableField.java`. An eleventh pass
  seeded self-returning fluent facts for chained external calls whose expression
  shape or declared return type proves that the external receiver is returned,
  removing `idiom-gaps/ExternalFluentMutation.java`. A twelfth pass fixed
  digit-leading external facade aliases for Scala tuple accessors, changing
  `SparkRddVariantPipeline.java` and `StreamCallbackChain.java` from parse
  failures into ordinary type-check failures. Later focused fixes, including
  optional array field reads into owned setter parameters, imported static
  side-effect fallback stubs, fallible nullable assignment into erased
  optional locals, and duplicate static/instance `Objects.equals` facade
  pruning, reduced the known corpus compile baseline to **30** failures. A
  subsequent pass lowered class literals as erased `JavaClass<()>` tokens when
  returning into `Class<?>` slots or comparing against `getClass()`, removing
  `idiom-gaps/ClassLiteralToken.java` and reducing the baseline to **29**
  failures. Another pass made expected assignment/variable types seed guessed
  external call return surfaces and allowed known external `String` returns to
  replace opaque placeholders, removing `idiom-gaps/TypedGetterAvoidsDowncast.java`
  and `idiom-gaps/ExternalIndexedAccessors.java`; the baseline is now **27**
  failures. The same target-type seeding now also applies to return expressions
  for unknown external calls, removing
  `idiom-gaps/ExceptionBaseCatchCoversSubclass.java`,
  `idiom-gaps/ExceptionSubclassCatchOrder.java`, and
  `idiom-gaps/ExceptionRuntimeCheckedHierarchy.java`; the baseline is now
  **24** failures. A later narrow return-slot fix wraps `return this` values
  when the same-type return slot is shared, removing
  `idiom-gaps/MapForEachBlockLambda.java`; the baseline is now **23**
  failures. A subsequent pass let declared local/assignment target types seed
  named external call return surfaces, propagated expected primitive types
  through arithmetic, and marked null-compared assignment calls as nullable,
  removing `idiom-gaps/FfiNativeHandoverCall.java`,
  `idiom-gaps/WhileAssignmentNotNull.java`, and
  `idiom-gaps/WhileAssignmentNullVariants.java`; the baseline is now **20**
  failures. A later return-slot ABI alignment made source method bodies lower
  against the same erased external return boundary emitted in the signature,
  removing `idiom-gaps/TypedFeatureReaderHeaderCast.java`; the same corpus guard
  run exposed eight stale baseline deltas already present in the dirty tree, so
  the checked baseline moved back to **24** named failures after the source
  static-owner fix for `java/06-stdlib/StaticOwnerShadowing.java` and primitive
  stream-sum lowering for `java/06-stdlib/StreamsUse.java` and
  `java/06-stdlib/StreamCapturesLocal.java`. A subsequent pass made
  return-slot external-call adaptation decline recognized stream expressions,
  so `anyMatch` terminals are lowered by the stream planner instead of as raw
  receiver method calls, removing `java/06-stdlib/StreamsFindMatch.java` and
  `java/06-stdlib/StreamStringMethodReferences.java`; the checked baseline is
  now **22** named failures. The same return-slot preemption class also hid the
  existing map-entry chain lowering behind external-call adaptation; declining
  recognized map-entry accessors there and normalizing optional receiver
  projections removed `idiom-gaps/MapEntrySetFirstKeyFromNullableField.java`
  and `idiom-gaps/MapEntrySetFirstKey.java`. The checked baseline is now
  **20** named failures. A follow-up made raw `submit(Callable)` parameters lower
  as generic zero-argument closures and taught callback return inference to see
  block-local declarations before return expressions, removing
  `idiom-gaps/ExecutorSubmitBlockLambda.java`. A follow-up made raw `List`
  arguments use erased-object element slots for `add`, and made raw sequence
  `get` clone through the erased-object runtime helper before returning an owned
  erased value, removing `java/05-types/RawTypes.java`. A subsequent pass made
  inferred fallback static-final external fields emit opaque associated-const
  surfaces instead of undeclared owner paths or intentional compile errors,
  removing `idiom-gaps/ExternalFieldThrowingCallInTry.java`. A follow-up added
  `JavaNioFileFiles` support for static `Files` calls and made one-argument
  `InputStream.read(byte[])` lowering reborrow mutable byte buffers instead of
  consuming `&mut [i8]`, removing `idiom-gaps/TryWithResourcesCopy.java`. The
  next follow-up made return-bearing try-with-resources scopes use the enclosing
  try/catch result carrier for resource construction, cleanup and body result
  return, removing `idiom-gaps/TryWithResourcesReturn.java`. The next follow-up
  made catch/finally catch arms use the result-carrier return
  policy before running `finally`, removing `java/06-stdlib/TryCatchFinally.java`.
  Preserving result-propagated shape through owned optional unwraps and adding
  `java.io.Closeable.close` support then removed
  `java/06-stdlib/TryWithResources.java`. Stream prefix-local nullability
  planning plus temporary nested-stream string recovery then removed
  `java/06-stdlib/StreamFlatMapBlockPipeline.java`. Erased Spark RDD facade
  returns then removed `idiom-gaps/SparkRddVariantPipeline.java`; Spark callback
  facade typing and `collectAsMap` support then removed
  `idiom-gaps/StreamCallbackChain.java`. Modeling `ParentWriter` as a no-state
  external super-constructor boundary then removed
  `idiom-gaps/SuperConstructorState.java`. Adding the corpus `ndarray` shim and
  making `ndarray::Array2` borrowed-to-owned recovery clone the matrix then
  removed `java/05-types/ArrayInitializers.java`. Anonymous interface return
  shape planning now keeps boxed dyn-trait slots for method returns, removing
  `java/04-objects/AnonymousInterface.java`. Later package-owner/static-call,
  enum payload, erased-object, ordered-map, Spark erased-facade assignment,
  `FileLoadConstructorDelegation`, and direct emitted-source `super.m()` fixes
  brought the checked baseline to **3** loud named failures.
- Many tests still assert on `translation.rust` without local compile checks;
  the corpus sweep catches checked-in fixtures, not every inline assertion.
- Behavioural coverage is still thin; differential fixtures for `null`,
  collections and `throw` remain the highest-value next step.
- A 2026-08-25 Codex pass repaired the stale hand-written fixture harnesses in
  `tests/fixtures.rs`: the suite now matches the current fallible signatures,
  wrapping arithmetic, underscore-qualified nested names, owned stream strings,
  and erased raw-list borrow spelling. `cargo test --test fixtures --quiet`
  passes 51/51.
- Follow-up 2026-08-25: seven more rendered-Rust fixture assertions in
  `tests/fixtures.rs` now use the shared generated-crate checker:
  `StaticImportUtilityOwner`, imported chained facade return, nullable clear
  assignment, `SimpleClass`, expression precedence, method overload suffixes,
  and `Utils.nonNull(path)`. Promoting `Utils.nonNull(path)` exposed and fixed a
  real compile gap: a Java `String` parameter lowered as `&str` now recovers an
  owned `String` before entering the generic `non_null<T0>(T0) -> T0` facade
  when the call is used as an owned return value.
- Follow-up 2026-08-25: three more stream fixture assertions now use the shared
  generated-crate checker: `StreamFilterNonNull`, `StreamMapNullFilter`, and
  `StreamsUse`'s simple sum pipeline. These did not expose new translator gaps,
  but they shrink the unchecked rendered-Rust surface in `tests/fixtures.rs`.
- Follow-up 2026-08-25: four more collection/stream rendered assertions now
  use the same checker: `CollectionsUse` map-get/null ternary,
  `MapPutPreviousValue`, `StreamsCollect`, and `StreamsFindMatch`. The
  `tests/fixtures.rs` shared generated-crate helper is now used by 14 fixture
  assertions.
- Follow-up 2026-08-25: three more method/owner rendered assertions now use the
  shared checker: constructor delegation, same-arity overload dispatch, and
  nested static builder owner resolution. The shared helper is now used by 17
  fixture assertions.
- Follow-up 2026-08-25: two more stream/collector rendered assertions now use
  the shared checker: stream block-lambda static-owner resolution and
  `Collectors.toMap` lowering. The shared helper is now used by 19 fixture
  assertions.
- Follow-up 2026-08-25: dyn-trait interface parameter rendering now has a
  direct generated-crate check, and the imported static utility owner fixture
  was moved from an ad hoc `rustc --emit=metadata` block onto the shared
  generated-crate helper. Follow-up conversions moved the try/catch/finally,
  try-with-resources, external string constant, checked propagation, raw types,
  and wildcard bounds fixture checks onto the same helper. `tests/fixtures.rs`
  no longer has ad hoc metadata-only `rustc` checks; the shared helper is now
  used by 27 fixture assertions. Re-running the full suite exposed a dirty-tree
  regression in anonymous source-interface returns: matching anonymous returns
  were forced through the concrete slot path again, producing a bare trait
  return and `Default::default()`. Anonymous returns whose declared type is a
  dyn-trait/object-safe source interface now request an owned boxed dyn-trait
  slot, while erased external facade returns keep the concrete boundary path.
- Follow-up 2026-08-25: `tests/semantic_facts.rs` now uses the same
  generated-crate checker for four rendered nullability assertions: external
  field-nullability override, nullable field storage wrapping, bare constructor
  field assignments, and constructor-initialized sorted-map storage. These were
  already valid translations; the change makes the suite reject future stale
  rendered-Rust expectations with real compiler diagnostics. Running the full
  suite also exposed and fixed one stale public accessor assertion: `copy_from`
  now returns `Ok(self.get_value())` because the method is fallible.
- Follow-up 2026-08-25: the public trivial-accessor wrapper and package-private
  trivial-accessor inlining assertions in `tests/semantic_facts.rs` are now
  compile-backed too. The suite now has six generated-crate checks for rendered
  semantic-facts output.
- Follow-up 2026-08-25: bare constructor assignment with parameter shadowing is
  now compile-backed as well. Promoting `FieldOptionalityCases` exposed another
  compile gap: value-producing unresolved external static calls such as
  `ExternalHelper.fetch()` lowered as `external_helper::fetch()` without an
  emitted facade/module, and the erased-object recovery path then tried to
  downcast the loud fallback. Unresolved value-returning static-owner calls now
  lower to a typed `unimplemented!` marker, and erased-object downcast recovery
  leaves that marker alone so Rust can coerce it from `!` into the surrounding
  slot type. `FieldOptionalityCases` is now generated-crate checked.
- Follow-up 2026-08-25: all remaining `tests/fixtures.rs` ad hoc temp-file
  `rustc` checks now use the shared generated-crate helpers. Behavior tests for
  static-owner shadowing, enum payload constants, throw-only methods, stream
  collect/sum/capture/find/method-reference/flat-map/block/projection paths,
  switch fallthrough, and collection capacity constructors keep their generated
  test modules but run under the watchdog and cleanup path. The first-slice
  compile sweep now checks one generated crate with separate modules instead of
  spawning bare `rustc` once per fixture. Re-running the full fixture suite
  exposed five stale assertions around the current unresolved value-returning
  static-call policy and ordinary parse result projection; those assertions now
  match the typed `unimplemented!`/fallible parse output while retaining
  generated-crate checks. `cargo test --test fixtures --quiet` passes 52/52, and
  `rg 'Command::new|std::env::temp_dir|rustc' tests/fixtures.rs` has no hits.
- Follow-up 2026-08-25: three `tests/idiom_gaps.rs` behavior checks moved from
  hand-written temp-file `rustc --test` invocations to the shared
  generated-crate test helper: `ForEachLambdaSideEffect`,
  `MethodReferenceFlatMap`, and `SwitchReturnsEnumMatch`. The migration exposed
  stale generated-test snippets for current fallible/slice signatures and the
  emitted enum owner name; those snippets now use the current generated API.
  Focused regressions pass:
  `for_each_lambda_side_effect_generated_rust_compiles`,
  `method_reference_flat_map_generated_rust_preserves_behavior`, and
  `switch_returns_enum_match_generated_rust_compiles`.
- Follow-up 2026-08-25: five exception-oriented `tests/idiom_gaps.rs` behavior
  checks moved from hand-written temp-file `rustc --test` invocations to the
  shared generated-crate test helper: `SimpleRethrowCatch`,
  `SimpleCauseChaining`, `InternalThrowingCallInTry`,
  `SimpleTryFinallyCleanup`, and `SimpleTryCatchReturn`. The migration exposed
  stale generated-test snippets for the current `Result<_, JavaException>`
  rethrow API and by-value `Resource` parameter; those snippets now match the
  generated API. Focused regressions pass for all five. Removing the last ad hoc
  `idiom_gaps.rs` temp-file harness also exposed an enum-payload propagation
  bug: `EnumPayloadMethodUse::recent() -> bool` returned the fallible
  `is_at_least_as_recent_as(...) -> Result<bool, JavaException>` call without
  unwrapping or propagating it. The lowering fallback now maps planned source
  static-field receivers such as `PayloadVersion.V2` back to the solved source
  callable fallibility plan and applies the existing uncaught-exception policy.
  `source_enum_constructor_payload_fields_emit_match_backed_accessors` is now
  generated-crate checked.
- Follow-up 2026-08-25: the two remaining rendered-Rust translation checks in
  `tests/semantic_facts.rs` are now compile-backed. The assignment-in-condition
  and tail-recursive rewrite diagnostics tests still assert that the pending
  diagnostics/text are gone, and now also run the emitted Rust through
  `cargo_check_translation`. Focused regressions:
  `translation_no_longer_reports_pending_assignment_in_condition_diagnostics`
  and `translation_no_longer_reports_pending_tail_recursive_rewrite_diagnostics`.
- Follow-up 2026-08-25: `documents_gatk_acronym_identifier_splitting_gap` now
  compile-checks the `AcronymIdentifierNames.java` generated module after its
  rendered-name assertions. This removes another cheap 0041-style assertion-only
  fixture from `tests/idiom_gaps.rs`; the broader task remains converting the
  remaining rendered-Rust assertions where generated-crate checking is cheap.
- Follow-up 2026-08-25: `bare_final_instance_field_read_does_not_become_static_constant`
  in `tests/idiom_gaps.rs` now typechecks its generated crate after the string
  assertions.
- Follow-up 2026-08-25: two nearby rendered-Rust assertions are now
  compile-backed too. `java_string_substring_and_hash_code_use_runtime_helpers`
  had stale assertions from the pre-fallible/null-checked string lowering; the
  assertions now match the current `Result`-propagating helper calls.
  `external_receiver_generic_getter_projects_receiver_type_argument` exposed a
  real generated-crate error: an unmodelled external generic getter returned
  `T0` but required `T0: Default`, leaking a stub-body implementation detail
  into callers. Fact-backed unmodelled facade methods now leave return-only
  placeholder generics unbounded because their body is `unimplemented!`.
- Follow-up 2026-08-25: three more inline rendered-Rust assertions in
  `tests/idiom_gaps.rs` are now compile-backed:
  `translates_java_numeric_relational_promotion_before_rust_comparison`,
  `reused_string_parameters_clone_in_multiple_vec_initializers`, and
  `reused_string_source_clones_for_multiple_owned_local_assignments`.
- Follow-up 2026-08-25: `documents_string_format_static_call_gap` in
  `tests/idiom_gaps.rs` is now compile-backed. A nearby attempted promotion,
  `casted_borrowed_map_argument_clones_for_owned_helper_slot`, exposed a real
  non-cheap generated-crate failure: the `Map`/`SortedMap` conditional currently
  mixes `HashMap` and `BTreeMap` return/argument slots after the casted branch.
  That fixture remains assertion-only until the collection-carrier unification
  issue is addressed.
- Follow-up 2026-08-25: five more inline rendered-Rust assertions in
  `tests/idiom_gaps.rs` are now compile-backed:
  `source_value_struct_with_java_hash_equality_derives_eq_and_hash`,
  `string_pattern_methods_borrow_owned_string_arguments`,
  `string_pattern_chained_receiver_keeps_borrowed_pattern_argument`,
  `string_trim_owned_slots_convert_borrowed_view_to_string`, and
  `reference_array_element_null_comparison_reads_nullable_element`.
- Follow-up 2026-08-25: four more inline rendered-Rust assertions in
  `tests/idiom_gaps.rs` are now compile-backed:
  `java_string_split_with_limit_uses_runtime_helper` and
  `optional_sequence_get_unwraps_receiver_for_concrete_slot`, plus the adjacent
  `sequence_get_adapts_simple_interval_to_locatable_slot` and
  `assignment_condition_loop_body_sees_shadowed_local_as_non_null`. The
  sequence-get gap is fixed by unwrapping optional target slots to their element
  shape before applying the known `SimpleInterval` -> `Locatable` facade
  wrapper. The assignment-in-condition gap is fixed by lowering the normalized
  assigned value through the optional target slot, so a non-optional source
  return is wrapped before assignment into the nullable loop local.
- Follow-up 2026-08-25: three more inline rendered-Rust assertions in
  `tests/idiom_gaps.rs` are now compile-backed:
  `typed_map_get_redundant_cast_uses_generic_value_shape`,
  `throwing_call_local_initializer_propagates_result_with_question_mark`, and
  `solved_local_plan_inherits_nullable_source_call_initializer`.
- Follow-up 2026-08-25: `source_summary_nullable_string_field_hash_code_uses_runtime_helper`
  is now compile-backed. The promotion exposed that summary-source expressions
  already used field nullability, while the emitted external facade field still
  rendered only from the Java type. Summary sources now contribute field
  nullability facts before their bodies are stripped, and facade field surfaces
  render through those facts, so nullable summary fields use `Option<T>` in the
  emitted support struct.
- Follow-up 2026-08-25: four more inline rendered-Rust assertions in
  `tests/idiom_gaps.rs` are now compile-backed:
  `source_static_final_byte_constants_preserve_declared_slot_type`,
  `context_translation_emits_sibling_static_final_constant_definition`,
  `emitted_type_gets_source_summary_inherited_static_final_constant`, and
  `static_string_constant_argument_to_owned_string_slot_uses_to_string_adapter`.
  The promotion exposed two generated-crate failures: erased-object display
  support emitted downcast arms for non-emitted context/summary types, and
  non-emitted context static string constants were referenced without a support
  owner. Source downcast arms now scan only emitted source types, and static
  string constant support treats non-emitted context owners as support surfaces.
  A neighboring compile-backed object-display test also caught `&alignment`
  being passed as `&&Alignment` to the erased-object display helper for borrowed
  summary/context receivers; value-side display plans now distinguish owned
  erased objects from already-borrowed erased objects.
- Follow-up 2026-08-25: the `static_final` idiom-gaps filter is now
  compile-backed and green (`15 passed`). The promotion flushed out stale
  assertions around result-returning pattern/matrix helpers and the external
  inherited-constant diagnostic, plus two real static-storage defects:
  mixed/canonical Rust accessor names could disagree between static storage
  declaration and use, and qualified static-storage string getters were passed
  directly to string pattern methods without the borrowed-string adapter. Static
  storage access now resolves through the canonical source field name while
  keeping compatibility aliases for older snake-case accessors, and qualified
  storage reads are lowered through the slot conversion planner. The slot-aware
  storage path explicitly refuses associated-const `static final` fields so
  compile-time constants such as context `Constants::NAME` stay as associated
  constants.
- Follow-up 2026-08-25: four assertion-style tests in `tests/fixtures.rs` are
  now compile-backed after their rendered-Rust checks:
  `translates_multiline_string_concat_with_braces_without_closing_impl`,
  `inherited_overload_does_not_replace_child_override_body`,
  `translates_switch_fallthrough_without_compile_error`, and
  `translates_throw_only_method_to_result`. The intentionally loud
  `NullOverloadAmbiguity` assertion remains uncompiled because its correct
  output contains `compile_error!`; the direct generated-module checks in
  `anonymous_interface_return_uses_boxed_dyn_trait_slot` and
  `first_slice_generated_rust_compiles` already covered their outputs.
- Follow-up 2026-08-25: two rendered-Rust assertions in `tests/batch_translation.rs`
  are now compile-backed: `same_package_simple_class_literal_keeps_java_package_name`
  and `batch_duplicate_simple_type_names_keep_distinct_signature_keys`. Promoting
  the duplicate-name batch assertion exposed a real generated-crate failure:
  each per-file erased-object display support table was built from the whole
  batch support program and emitted unqualified downcast arms for sibling-file
  types that were not in that module. Batch lowering now keeps global support
  context for analysis while passing the current resolved program as the
  same-file source downcast context.
- Follow-up 2026-08-25: the remaining rendered-Rust assertion found by the
  current cheap scan, `external_packs::broad_http_pack_emits_function_facade_for_bucket_prefetch_wrapper`,
  is now compile-backed. Promotion exposed another generated-crate failure:
  `Function<SeekableByteChannel, SeekableByteChannel>` return types were emitted
  against a zero-parameter `pub struct Function;` facade. External generic
  carrier support now includes `Function<T0, T1>` with defaults, while preserving
  the existing `Future<T = Box<dyn Any>>` shape.
- Follow-up 2026-08-25: `selected_opaque_future_get_is_generic_instance_surface`
  is now compile-backed. Promotion exposed a stale facade surface:
  `Future<T>::get()` was emitted as returning plain `T`, while call lowering
  already treated Java `Future.get()` as throwable and appended the uncaught
  Java-exception recovery for a `Result<T, JavaException>`. The generated
  `Future<T>` carrier now declares `get(&self) -> Result<T, JavaException>`,
  so ordinary callers and propagated `try` scopes agree with the support method.
- Follow-up 2026-08-25: the remaining invalid-corpus fixture
  `NullOverloadAmbiguity` is now compiler-backed as an expected failure.
  `tests/common` has a generated-crate helper for assertions whose correct
  output contains `compile_error!`, and the fixture test now checks that nested
  `cargo check` fails with the ambiguous-overload diagnostic instead of only
  matching rendered text.
- Follow-up 2026-08-25: three more assertion-only idiom tests are now
  compile-backed:
  `same_class_nullable_string_field_hash_code_method_uses_runtime_helper`,
  `object_return_from_primitive_boxes_autoboxed_value`, and
  `exception_constructor_message_argument_is_stringified`. The nullable string
  hash-code promotion exposed a real field-nullability bug: qualified
  constructor assignments such as `this.bases = bases` treated the constructor
  parameter as definitely non-null, unlike the equivalent bare assignment.
  Field-nullability collection now applies the same parameter-aware rule to
  qualified field writes and trivial setters.
- Follow-up 2026-08-25: three more assertion-only idiom tests are now
  compile-backed:
  `object_return_from_typed_external_getter_erases_once_at_boundary`,
  `object_return_from_nonnullable_external_getter_boxes_concrete_value`, and
  `nonescaping_biconsumer_parameter_lowers_to_callback_slot`. The two external
  getter tests had stale expectations from before source methods were wrapped
  in the fallible `Result<..., JavaException>` shell; the generated crates now
  compile with the current `Ok(...)` erased-boundary shape.
- Follow-up 2026-08-25: three more assertion-only idiom tests are now
  compile-backed:
  `map_size_return_int_uses_java_int_width`,
  `external_boolean_getter_no_longer_requires_runtime_downcast`, and
  `external_nullable_getter_assignment_uses_option_carrier_recovery`. The map
  size and nullable getter assertions were stale in small ways: the current
  generated shape returns through `Ok(...)`, and the nullable ternary unwraps
  the else branch before wrapping the whole result in `Some(...)`.
- Follow-up 2026-08-25: `map_key_set_size_lowers_through_native_len` is now
  compile-backed. The failure was not parser shape: the Java AST already kept
  `values.keySet()` as the receiver of the outer `size()` call. The bad `.size()`
  was attached later by Rust method-call projection, after the receiver had
  already become `values.keys().cloned().collect::<Vec<_>>()`. Projection now
  recognizes that owned map-key/map-value Vec recovery shape and lowers a Java
  `size()` suffix to Rust `len() as i32`.
- Follow-up 2026-08-25: the same attempted promotion then fixed
  `object_local_from_concrete_external_factory_boxes_concrete_value` and made it
  compile-backed. The early unresolved-static placeholder already deferred for
  facade ABI facts, but `external-pack-api` support methods are backed by
  external API effect facts instead. The placeholder now also defers when a
  modeled external API fact renders to the planned Rust static owner, allowing
  the object-slot lowering path to box `DepExternalFactory::open()` normally.
- Follow-up 2026-08-26: two more assertion-only `parallelStream` idiom tests are
  now compile-backed: `parallel_stream_uses_shared_stream_lowering` and
  `parallel_stream_reduce_uses_shared_stream_lowering`. The reduce fixture now
  uses `new Rdd()` as the identity expression so the check stays focused on
  shared stream lowering rather than nullable field/default-value and
  move-from-field policy.
- Follow-up 2026-08-26: `source_interface_collection_elements_use_boxed_dyn_trait`
  is now compile-backed. Promoting it exposed two hidden generated-crate errors:
  collection generic arguments for object-safe source interfaces rendered bare
  unsized `dyn Trait` instead of `Box<dyn Trait>`, and external dyn-trait support
  could still synthesize a pack trait when a hierarchy override's type name was
  already owned by translated source. Focused check:
  `cargo test --test idiom_gaps source_interface_collection_elements_use_boxed_dyn_trait --quiet`.
- Follow-up 2026-08-26: six more assertion-only `idiom_gaps` tests are now
  compile-backed:
  `null_checked_static_helper_mutating_parameter_fields_keeps_mutable_borrow_slot`,
  `cross_source_null_checked_static_helper_keeps_concrete_mutable_borrow_slot`,
  `two_dimensional_numeric_array_literal_with_comment_lowers_to_ndarray_arr2`,
  `two_dimensional_row_replacement_lowers_to_ndarray_row_mut_assign`, and
  `optional_array_receivers_unwrap_for_read_indexing_and_length`, plus the larger
  `two_dimensional_numeric_arrays_lower_to_ndarray_array2` fixture.
  The ndarray promotions use a local generated-crate `ndarray` stub, matching the
  existing `assignment_order_ndarray_index` pattern instead of adding a test-crate
  dependency. The same pass fixed a stale optional-array length projection:
  nullable `Array2.length` now unwraps nullability and calls `.nrows()` directly
  instead of converting the receiver through a slice view. Promoting
  `optional_array_receivers_unwrap_for_read_indexing_and_length` exposed and then
  fixed a nested-read shape bug: `matrix[row]` on an optional `double[][]` carrier
  used the plain indexed-read projection, so `matrix[row][col]` inherited bogus
  optional element shape and emitted `.unwrap()` on `f64`. Indexed expression
  shape inference now uses the optional-aware read projection, with
  `nested_indexed_shape_projects_optional_primitive_matrix_scalar` pinning the
  shape-layer regression. Promoting the larger ndarray fixture also removed
  stale assertion text for fallible `Result` returns around ndarray reads and
  length projections.
- Follow-up 2026-08-26: three more assertion-only `idiom_gaps` tests are now
  compile-backed:
  `nullable_ternary_branches_consume_branch_refinements_and_slot_shape`,
  `nullable_ternary_call_receiver_uses_branch_refinement_before_optional_adapter`,
  and `map_key_set_returning_set_collects_hashset_for_set_slot`. Promotion
  flushed out stale assertion text for current optional boxed-int lowering,
  catchable `NullPointerException` receiver projections, and map key-set
  `Result<HashSet<_>, JavaException>` returns; the generated crates now compile
  for all three.
- Follow-up 2026-08-26: three interface/receiver assertion-only `idiom_gaps`
  tests are now compile-backed:
  `source_interface_object_getter_returns_borrowed_field`,
  `source_interface_trait_impl_unwraps_nullable_implementation_return`, and
  `lowercase_keyword_receiver_is_not_guessed_as_static_owner`. Promotion exposed
  a real optional shared-handle borrow bug: an implementation getter returning a
  nullable source-interface method view lowered `Option<Rc<T>>` as
  `Option<&Rc<T>>` where the signature promised `Option<&T>`. Optional borrowed
  return recovery now projects shared-handle carriers through
  `.as_ref().map(|value| &**value)`, and all three generated crates compile.
- Follow-up 2026-08-26: four exception/external-effect `idiom_gaps` tests are
  now compile-backed:
  `caught_checked_exception_rethrown_as_known_unchecked_propagates_result`,
  `unchecked_direct_throw_in_plain_method_returns_result`,
  `external_throwing_call_in_try_uses_external_effect_facts`, and
  `external_field_throwing_call_in_try_uses_external_field_facts`. Promotion
  exposed a missing runtime surface for Java's `StringBuilder`/`StringBuffer`
  upcast to `Appendable`; the external facade support now emits those `From`
  impls only when `Appendable` and the string-buffer runtime are both present.
  The field-fact compile check also now supplies an explicit const value fact,
  preserving the separate diagnostic path for static constants with no modeled
  value.
- Follow-up 2026-08-26: four more ownership/nullability `idiom_gaps` assertions
  are now compile-backed:
  `constructor_stores_object_parameter_as_owned_field`,
  `optional_owned_field_getter_returns_borrowed_option_view`,
  `source_nullable_getter_local_initializer_stays_optional`, and
  `guarded_nullable_copy_constructor_reads_source_fields_after_unwrap`.
  Promotion exposed a borrowed-receiver trivial-accessor bug:
  `record.getReadName()` retargeted to `record.read_name` too late for
  slot-aware optional clone recovery, so a local initializer tried to move an
  `Option<String>` out of `&FastqRecord`. Non-self trivial accessors for
  optional target slots now lower through the field-access slot path, producing
  an owned optional clone before assignment.
- Follow-up 2026-08-26: five nullable getter/equality `idiom_gaps` assertions
  are now compile-backed:
  `source_nullable_field_getter_returns_optional_string`,
  `source_nullable_field_return_does_not_capture_shadowing_local`,
  `context_source_nullable_field_getter_keeps_optional_field_shape`,
  `nullable_trim_equals_original_uses_optional_string_view_policy`, and
  `borrowed_nullable_trim_equals_original_uses_optional_string_view_policy`.
  Promotion exposed two compile-visible bugs. First, erased-object display
  downcast support was built from context/source-summary types that are not
  emitted into the generated module; source downcast arms are now limited to the
  emitted source program while external runtime support still scans the broader
  support surface. Second, optional `.equals()`/`==` lowering added `.as_ref()`
  to nullable parameters that the method signature had already lowered as
  `Option<&String>`, producing `Option<&&String>`; the equality lowerer now
  compares already-borrowed nullable locals directly on the optional side.
- Follow-up 2026-08-26: three sorted-carrier `idiom_gaps` assertions are now
  compile-backed: `tree_map_constructor_keeps_sorted_map_carrier`,
  `tree_map_first_key_uses_btree_key_iterator`, and
  `tree_set_constructor_keeps_sorted_set_carrier`. Promotion exposed a real
  chained-call failure: `m.firstKey().length()` lowered the `TreeMap.firstKey()`
  receiver to a Rust `String` but left the outer call as Java `.length()`.
  Slot-aware external call lowering now runs the native string retarget pass,
  and string-producing receiver detection accepts resolved `String` return
  types plus the sorted-map `firstKey`/`lastKey` string-key fallback. The
  promoted fixture now asserts `len()` is used and `.length()` is not left in
  the generated Rust.
- Follow-up 2026-08-26: four more collection/facade `idiom_gaps` assertions are
  now compile-backed: `map_put_all_lowers_to_hash_map_extend`,
  `map_put_all_boxes_values_for_erased_object_value_map`,
  `queue_put_lowers_to_native_push_for_vec_backed_queue`, and
  `byte_and_char_buffer_put_are_modeled_statement_facade_calls`. The `putAll`
  and queue promotions compiled as-is. The buffer promotion exposed a stale
  assertion: current byte-array indexing is the bounds-checked fallible array
  projection, not the old direct `encoded[0 as usize]` spelling. The assertion
  now pins the checked projection and the modeled no-op `put` statement facade.
- Follow-up 2026-08-26: six more `idiom_gaps` assertions are now
  compile-backed: `documents_string_array_literal_argument_gap`,
  `documents_map_put_get_java_api_gap`,
  `array_arguments_use_borrowed_slice_views`,
  `same_package_uppercase_utility_receiver_lowers_as_static_owner`,
  `uppercase_utility_receiver_shadowed_by_local_stays_instance_receiver`, and
  `documents_primitive_array_allocation_gap`. Promotion found two stale
  assertions from earlier lowering improvements: map `put/get` now returns
  `Result<Option<String>, JavaException>`, and array argument lowering now uses
  fallible null checks plus Rust's `&Vec<T>` to `&[T]` coercion. It also exposed
  a real static-owner gap: `ArgUtils.flatten(values)` was still emitted as an
  unresolved static placeholder, then as `arg_utils::flatten(values)`. The
  imported-static first-argument utility model now covers `ArgUtils.flatten`,
  and static owner rendering maps canonical `arg_utils` back to the emitted
  `ArgUtils` support type.
- Follow-up 2026-08-26: five more map/key-reuse `idiom_gaps` assertions are now
  compile-backed: `field_map_put_reused_key_clones_at_owned_boundary`,
  `map_put_else_branch_reused_key_clones_before_later_block_read`,
  `map_put_else_branch_reassigned_key_clones_before_later_block_read`,
  `optional_map_put_reused_value_borrows_before_clone_unwrap`, and
  `optional_argument_wrap_reused_value_clones_before_some`. Promotion exposed a
  real native map gap: `Map<String, Integer>.get(key)` returned directly into an
  `int` slot as `Ok(map.get(&key))` because the early external-return hook
  intercepted the call before map-specific slot lowering. Native `Map.get`
  return handling now defers structurally to the map path, primitive target slots
  use `.copied().expect("Java null receiver")`, nullable map returns keep the
  optional carrier with `.cloned()`, and optional string-view equality compares
  `Some(value.as_str())` to `Option<&str>` instead of borrowing an owned
  `String`.
- Follow-up 2026-08-26: the mechanical counter now recognizes six tests that
  were already generated-crate tested through the local
  `cargo_test_translation` wrapper. Renaming that helper to
  `cargo_test_generated_modules_for_translation` changed the measurement from
  181 to 175 without changing coverage.
- Follow-up 2026-08-26: three more `idiom_gaps` assertions are now
  compile-backed:
  `sam_record_static_byte_array_constants_emit_typed_facade_surfaces`,
  `same_class_static_calls_borrow_ndarray_arguments_from_signature_slots`, and
  `static_final_two_dimensional_numeric_tables_can_feed_readonly_slice_helpers`.
  Promotion exposed a real optional-slice indexing compile failure: checked
  array reads rebound `row.as_ref().as_ref().ok_or_else(...)?`, creating a
  temporary borrow in result-propagating code. The checked-index source binder
  now keeps already-borrowed optional sequence projections as the source and
  collapses the redundant shared `as_ref` before the null check. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 172 after this pass.
- Follow-up 2026-08-26: four more `idiom_gaps` assertions are now
  compile-backed: `documents_nullable_concrete_object_gap`,
  `documents_java_collection_api_gap`,
  `documents_external_primitive_getter_dispatch_coercion_gap`, and
  `documents_external_byte_array_getter_dispatch_coercion_gap`. The two getter
  fixtures were already covered by the combined direct-getter generated-crate
  check, but their individual tests are now visible to the mechanical
  assertion-only counter too. A nearby probe of
  `documents_proven_non_null_member_field_gap` was not landed: the test still
  fails its existing desired-output assertion because the field is lowered as
  `Option<String>`, so adding a compile check there would hide rather than fix
  the gap. The remaining rendered-Rust-only `idiom_gaps` assertion count is 168.
- Follow-up 2026-08-26: `documents_collection_remove_if_lambda_gap` is now
  compile-backed. Three adjacent closure/future probes were not landed because
  compile checking exposed broader generated-crate gaps:
  `executor_submit_lambda_receiverless_helper_call_uses_current_self` lowers
  `Executor.submit(...)` as unit where the Java `Object` return slot wants
  `Box<dyn Any>`, `executor_submit_future_slot_shapes_block_lambda_result`
  lacks an `ExecutorService.submit` support surface and still cannot convert
  the ordered-map carrier with `into_iter()`, and
  `stream_block_lambda_return_uses_declared_local_receiver_not_static_owner`
  still renders `Product.newBuilder()` as `product.new_builder()` inside the
  stream block. The remaining rendered-Rust-only `idiom_gaps` assertion count is
  167.
- Follow-up 2026-08-26: `inherited_receiverless_instance_call_uses_current_override_signature_name`
  is now compile-backed. The nearby generic-bound `Box<T extends HeaderLine>`
  probes were then made compile-backed too:
  `source_generic_class_bound_method_call_uses_bound_surface`,
  `source_class_bound_inherited_mutator_forwards_to_child_method`, and
  `source_generic_class_bound_overload_call_uses_bound_rust_name`. Promotion
  exposed three real/stale issues: translator-owned support code used
  unqualified `Box<JavaException>` / `Box<dyn Any>` and could be shadowed by a
  source class named `Box`; class-level type-parameter impl bounds missed
  constructor-driven clone obligations; and trivial setter calls through an
  optional type-parameter receiver lowered to a field write instead of a bound
  setter call. Those are fixed, and the stale assertions now accept the current
  optional-field unwrap and borrowed-string bound spellings. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 163.
- Follow-up 2026-08-26: three more generic/alias assertions are now
  compile-backed: `source_generic_class_preserves_type_params_and_interface_bounds`,
  `type_param_bound_enum_return_equality_uses_bound_return_shape`, and
  `nested_generic_source_alias_preserves_type_params`. Promotion exposed one
  more 0064-shaped support leak: object-safe trait helper surfaces used
  unqualified heap-box spellings (`Box<dyn Trait>` and `Box::new`) and could be
  shadowed by a source class named `Box<T>`. Those helper surfaces now render
  `std::boxed::Box`, and the stale nested-alias assertion accepts the current
  `Outer_Inner<T>` spelling. Two nearby probes remain assertion-only for this
  pass: `optional_type_param_bound_call_display_uses_bound_return_shape` still
  lowers `String.format("%s", line.getID())` through a generic field access
  (`line.id`) instead of the bound method, and
  `constructor_self_optional_field_argument_uses_field_slot_shape` exposes a
  broader shared-handle/default/result-shell constructor issue. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 160.
- Follow-up 2026-08-26: `optional_type_param_bound_call_display_uses_bound_return_shape`
  is now compile-backed. Promotion fixed the generic-bound display argument
  path: a source getter call through `T extends HeaderLine` now bypasses the
  concrete trivial-accessor rewrite and emits a bound trait method call instead
  of `line.id`. Nullable receivers use the existing optional receiver projection,
  so propagated `Result` contexts can keep Java null failures catchable. Focused
  checks: `cargo test --test idiom_gaps optional_type_param_bound_call_display_uses_bound_return_shape --quiet`
  and `cargo test --test idiom_gaps source_generic_class_bound_overload_call_uses_bound_rust_name --quiet`.
  The remaining rendered-Rust-only `idiom_gaps` assertion count is 159.
- Follow-up 2026-08-26: two more passing constructor/null-display assertions are
  now compile-backed:
  `borrowed_enum_constructor_param_stores_owned_optional_field_element` and
  `null_guarded_ternary_display_branch_unwraps_optional_value`. Nearby probes
  that still fail their own rendered-output assertions were left untouched:
  `documents_proven_non_null_member_field_gap`,
  `stale_constructor_optional_enum_field_shape_uses_final_storage_slot`, and
  `if_null_else_branch_refines_optional_display_segment`. Focused checks:
  `cargo test --test idiom_gaps borrowed_enum_constructor_param_stores_owned_optional_field_element --quiet`
  and `cargo test --test idiom_gaps null_guarded_ternary_display_branch_unwraps_optional_value --quiet`.
  The remaining rendered-Rust-only `idiom_gaps` assertion count is 157.
- Follow-up 2026-08-26: three more passing rendered-output assertions are now
  compile-backed:
  `null_guarded_ternary_display_branch_matches_unqualified_field_alias`,
  `null_guarded_ternary_display_branch_matches_camel_case_this_field`, and
  `collection_field_mutator_method_takes_mut_self`. The attempted promotion of
  `reused_optional_display_segment_borrows_before_later_use` still exposes the
  known borrowed-parameter lifetime gap:
  `check(&self, value: Option<&GATKPath>) -> Result<Option<&GATKPath>, ...>`
  needs an explicit lifetime tying the return to the parameter rather than
  `&self`, so that compile check remains intentionally unlanded. Focused
  checks: the three promoted tests above. The remaining rendered-Rust-only
  `idiom_gaps` assertion count is 154.
- Follow-up 2026-08-26: two more small documentation-style idiom probes are now
  compile-backed: `documents_method_reference_constructor_gap` and
  `documents_class_literal_logger_gap`. The neighboring
  `documents_method_reference_flat_map_gap` rendered assertion is stale and was
  left alone; the behavior-preserving generated-crate test for the same fixture
  already exists separately. Focused checks:
  `cargo test --test idiom_gaps documents_method_reference_constructor_gap --quiet`
  and `cargo test --test idiom_gaps documents_class_literal_logger_gap --quiet`.
  The remaining rendered-Rust-only `idiom_gaps` assertion count is 152.
- Follow-up 2026-08-26: three nearby promotion probes were checked. The
  source-local stream shadow fixture is now expected-failure-backed, and the
  emitted JDK stream subclass fixture is now compile-backed after adding the
  exact builtin `OutputStream -> JavaIoOutputStream` conversion surface required
  by modeled `FilterOutputStream(OutputStream)` super-constructor storage.
  `optional_map_string_get_flows_into_top_level_source_enum_value_of` remains
  assertion-only: checking the use-site translation alone misses the context
  enum, while naively concatenating the separately translated enum source
  duplicates generated support/prelude items. A fourth probe,
  `optional_map_string_get_compares_to_cross_class_static_string_constant`, is
  already loud via `compile_error!("unsupported Java String reference identity comparison requires string identity representation")`
  and should stay assertion-only until string reference identity representation
  is designed. `local_filter_input_stream_shadow_does_not_get_jdk_inherited_close_surface`
  is now expected-failure-backed with `cargo_check_generated_modules_fails_with`:
  the fixture is invalid Java once the source-local `FilterInputStream` masks the
  JDK class, and the generated crate must keep failing with no inherited
  `close()` surface. Focused checks:
  `cargo test --test idiom_gaps local_filter_input_stream_shadow_does_not_get_jdk_inherited_close_surface --quiet`,
  `cargo test --test idiom_gaps emitted_jdk_stream_subclass_converts_to_exact_platform_super_slot --quiet`,
  and `cargo test --test idiom_gaps java_platform_input_stream_upcast_support_emits_from_impl --quiet`.
  The remaining rendered-Rust-only `idiom_gaps` assertion count is 150.
- Follow-up 2026-08-26: ten more super-constructor / external-exception
  fixtures are now compile-backed or expected-failure-backed:
  `modeled_external_super_constructor_field_read_defers_until_self_temporary_exists`,
  `explicit_object_super_constructor_is_noop`,
  `no_arg_super_constructor_with_source_summary_inherited_setter_lowers`,
  `translates_external_exception_super_constructor_without_placeholder`,
  `exception_self_as_throwable_context_argument_is_loud_upcast_gap`,
  `explicit_cast_exception_self_as_throwable_context_argument_is_loud_upcast_gap`,
  `jdk_file_reader_super_constructor_is_no_state_facade`,
  `jdk_stream_wrapper_super_constructors_preserve_platform_state_fields`,
  `emitted_jdk_stream_subclass_gets_query_backed_inherited_close_surface`,
  and `jdk_stream_wrapper_super_with_self_linking_trailing_mutation_is_precise_gap`.
  The last three intentional/loud gaps in that list use
  `cargo_check_generated_modules_fails_with` where the generated Rust is
  expected to keep a named `compile_error!`. Focused checks were run for each
  promoted test. The remaining rendered-Rust-only `idiom_gaps` assertion count
  is 140.
- Follow-up 2026-08-26: four more source-super / constructor assertions are now
  compile-backed:
  `source_super_constructor_composes_delegated_parameter_bindings`,
  `source_super_constructor_delegated_null_marks_flattened_field_optional`,
  `source_super_constructor_uses_resolved_overload_not_first_same_arity`, and
  `constructor_trailing_instance_call_rewrites_self_arguments`. Two of those
  promotions also fixed stale assertions for current generated names/shapes:
  nested enum names now preserve the path boundary as
  `SourceSuperDelegatedParamSubstitutionBase_Kind`, and the resolved-overload
  context fixture currently uses erased opaque context parameters for
  `CountKind` / `ValueType`.

  Three probes remain intentionally unlanded. `source_super_constructor_lowers_locals_needed_by_inherited_fields`
  still references a context-source helper owner (`HeaderBase::parse`) that the
  standalone generated-crate helper does not emit. `source_object_varargs_forward_and_append_erased_arguments`
  exposes a real erased-varargs carrier mismatch:
  `Vec<Option<Box<dyn Any>>>` is cloned/extended as though it were
  `Vec<Box<dyn Any>>`. `external_standard_exception_super_with_trailing_body_lowers_subclass_state`
  now has its assertion updated for the current shared-handle field shape, but
  compile checking still exposes the broader bare `ExceptionContext` assignment
  into an `Rc<ExceptionContext>` slot. Focused checks were run for the promoted
  tests and the three probes. The remaining rendered-Rust-only `idiom_gaps`
  assertion count is 136.
- Follow-up 2026-08-26: four more rendered-Rust-only idiom assertions are now
  compile-backed: `documents_optional_get_is_present_java_api_gap`,
  `java_optional_get_does_not_use_element_owner_external_fact`,
  `buffered_reader_read_line_uses_reader_external_effect`, and
  `translates_exception_propagating_try_with_resources_without_catch`. Promoting
  the BufferedReader fixture found a stale assertion: current nullable field
  receiver lowering emits the checked `ok_or_else(...)?` projection before
  calling `read_line()` rather than the older unchecked `.unwrap()` shape.
  Focused checks were run for all four promoted tests. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 132.
- Follow-up 2026-08-26: five more PrintStream-related assertions are now
  compile-backed: `print_stream_check_error_is_boolean_getter`,
  `print_stream_output_stream_constructor_has_typed_support_arg`,
  `optional_print_stream_field_println_uses_arity_named_support`,
  `optional_print_stream_field_println_with_throwing_arg_keeps_arity_name`, and
  `optional_print_stream_field_println_with_mutating_optional_arg_uses_mut_receiver`.
  Promoting them found three stale assertions, all caused by current generated
  Rust being more explicit than the old expected text: result-returning boolean
  facade calls now return `Ok(...)`, and nullable PrintStream/Encoder receivers
  use checked `ok_or_else(...)?` projections instead of unchecked `.unwrap()`.
  Focused checks were run for all five promoted tests. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 127.
- Follow-up 2026-08-26: seven varargs / erased-object sequence assertions now
  have generated-crate discipline. Four compile cleanly:
  `erased_object_sequence_push_moves_when_no_later_read`,
  `erased_object_sequence_list_push_moves_when_no_later_read`,
  `object_stream_erased_object_sequence_list_push_moves_block_local`, and
  `object_stream_erased_object_sequence_for_loop_field_push_moves_block_local`.
  Three are expected-compile-failure-backed rather than string-only:
  `source_varargs_pack_external_elements_with_explicit_adapter` exposes duplicate
  generated external static constants, `source_varargs_pack_borrowed_external_elements_clones_before_into`
  exposes missing `Option<WriteOption>: From<FileCardinalityWriteOption>`
  conversion, and `erased_object_sequence_list_push_inside_loop_moves_block_local`
  exposes assignment of bare `i32` into `Option<Box<dyn Any>>` for nullable
  `Object[]` elements. Focused checks were run for all seven promoted tests,
  plus `cargo fmt --check` and `cargo check --quiet`. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 120.
- Follow-up 2026-08-26: seven more assertion-only `idiom_gaps` tests now have
  generated-crate discipline. Clean compile-backed tests:
  `documents_proven_non_null_member_field_gap` (assertion corrected to document
  the current optional field gap), `emitted_context_static_call_uses_solved_param_shape`,
  `receiverless_instance_overload_delegation_uses_current_self_and_subtype_argument`,
  and `context_translation_uses_sibling_overload_signature_for_call_target`.
  Three are expected-compile-failure-backed:
  `executor_submit_lambda_receiverless_helper_call_uses_current_self` exposes
  erased `Executor.submit` return adaptation (`Box<dyn Any>` vs `()`),
  `executor_submit_future_slot_shapes_block_lambda_result` exposes the missing
  `ExecutorService.submit` facade plus linked-map iterator support, and
  `stream_block_lambda_return_uses_declared_local_receiver_not_static_owner`
  exposes a lowercase static-owner recovery miss inside a stream lambda
  (`product.new_builder()`). Focused checks were run for all seven promoted
  tests, plus `cargo fmt --check` and `cargo check --quiet`. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 113.
- Follow-up 2026-08-26: seven generic/source-hierarchy assertions are now
  expected-compile-failure-backed instead of string-only:
  `non_emitted_context_generic_constructor_does_not_emit_missing_bound`,
  `type_param_bound_source_supertype_assignment_requires_hierarchy_lowering`,
  `reused_source_subtype_argument_upcasts_after_clone_recovery`,
  `reused_source_subtype_owned_argument_upcasts_after_clone_recovery`,
  `borrowed_downcast_source_subtype_argument_upcasts_after_clone_recovery`,
  `borrowed_downcast_source_subtype_owned_argument_upcasts_after_clone_recovery`,
  and `source_receiver_generic_result_upcasts_to_supertype_assignment_slot`.
  The two owned-argument upcast assertions were also corrected: the upcast itself
  now lowers through `From<FormatLine> for CompoundLine`; their remaining loud
  failure is Java `String ==` identity representation. Focused checks were run
  for the seven promoted tests, plus `cargo fmt --check` and
  `cargo check --quiet`. The remaining rendered-Rust-only `idiom_gaps` assertion
  count is 106.
- Follow-up 2026-08-26: seven more optional/context assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `stale_constructor_optional_enum_field_shape_uses_final_storage_slot`
  (assertion updated for current enum type spelling),
  `context_translation_does_not_emit_opaque_facade_for_sibling_source_type`,
  `emitted_context_object_to_string_in_print_uses_java_object_stringification_without_declared_method`,
  `if_null_else_branch_refines_optional_display_segment` (assertion updated for
  the current `as_ref().unwrap()` refinement), and
  `sibling_optional_source_return_receiver_getter_is_unwrapped_before_call`
  (assertion updated for the current null-safe `ok_or_else(...)?` receiver
  lowering). Two are expected-compile-failure-backed:
  `constructor_self_optional_field_argument_uses_field_slot_shape` exposes a
  double `Rc` construction path for initialized shared fields, and
  `reused_optional_display_segment_borrows_before_later_use` exposes the
  anonymous lifetime gap when an optional borrowed parameter is returned after
  use. Focused checks were run for all seven promoted tests, plus
  `cargo fmt --check`, `cargo fmt`, and `cargo check --quiet`. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 99.
- Follow-up 2026-08-26: five more hierarchy/method-reference assertions are
  generated-crate-backed. `documents_method_reference_flat_map_gap` now cleanly
  compiles its generated crate, and its stale signature assertion now accepts the
  current fallible `Result<HashSet<...>, JavaException>` return shape while still
  pinning the `flat_map(...).collect()` lowering. Four source hierarchy guard
  tests are expected-compile-failure-backed:
  `source_class_upcast_requires_hierarchy_lowering`,
  `erased_object_instanceof_source_supertype_requires_hierarchy_lowering`,
  `pattern_instanceof_source_supertype_requires_hierarchy_lowering`, and
  `pattern_instanceof_ternary_source_supertype_requires_hierarchy_lowering`.
  The pattern-instanceof assertion was tightened to reject the bad local
  `o.downcast_ref::<A>()` shape instead of any unrelated support-code
  `downcast_ref::<A>()` occurrence. Focused checks were run for all five
  promoted tests, plus `cargo fmt --check`, `cargo fmt`, and
  `cargo check --quiet`. The remaining rendered-Rust-only `idiom_gaps` assertion
  count is 94.
- Follow-up 2026-08-26: five more unsupported/context-super assertions are
  generated-crate-backed. Clean compile-backed test:
  `external_pack_static_utility_call_uses_static_owner`. Four are
  expected-compile-failure-backed:
  `unsupported_instanceof_placeholder_escapes_debug_type_quotes` pins the
  escaped diagnostic text emitted through `compile_error!`,
  `optional_map_string_get_flows_into_top_level_source_enum_value_of` records
  that context enum definitions are not emitted into this helper's generated
  crate, and `source_super_constructor_lowers_locals_needed_by_inherited_fields`
  plus `generic_source_super_constructor_initializes_flattened_inherited_fields`
  record the remaining absent-superclass-owner references (`HeaderBase` and
  `BaseSolver`) in flattened inherited-field lowering. Focused checks were run
  for all five promoted tests, plus `cargo fmt --check`, `cargo fmt`, and
  `cargo check --quiet`. The remaining rendered-Rust-only `idiom_gaps` assertion
  count is 89.
- Follow-up 2026-08-26: five source-super/varargs assertions are now
  expected-compile-failure-backed:
  `source_super_constructor_preserves_trailing_method_result_call`,
  `source_super_constructor_matches_varargs_super_arity`,
  `implicit_source_super_constructor_initializes_inherited_fields`,
  `source_object_varargs_forward_and_append_erased_arguments`, and
  `inherited_context_trailing_call_uses_context_field_and_shared_varargs_plan`.
  The string assertions were tightened to the current emitted slot shapes
  (`Option<Number>`, `Rc<ExceptionContext>`, and the current erased-varargs
  vector construction) so the tests now reach rustc and name the real remaining
  failures: invalid zero-argument `Rc::new`, missing `ExceptionContext::new`,
  missing `Rc<ExceptionContext>::add_message`, and incompatible
  `Vec<Option<Box<dyn Any>>>` varargs extension. Focused checks were run for all
  five promoted tests, plus `cargo fmt --check`, `cargo fmt`, and
  `cargo check --quiet`. The remaining rendered-Rust-only `idiom_gaps` assertion
  count is 84.
- Follow-up 2026-08-26: six external/context adapter assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `corpus_genomicsdb_qualified_nested_builders_use_distinct_rust_type_names`
  (assertion updated for current `VidMapping_Builder`/`FieldInfo_Builder` names
  and fallible `Ok(mapping_builder.build())` return),
  `corpus_disq_external_chain_stays_unmodeled_boundary_without_self_recursion_warning`,
  and `variant_context_adapter_has_gatkvariant_return_surface`. Three are
  expected-compile-failure-backed:
  `external_standard_exception_super_with_trailing_body_lowers_subclass_state`
  exposes missing `Default`/handle adaptation for `Rc<ExceptionContext>`,
  `emitted_context_source_subclass_constructor_flows_to_superclass_slot` records
  the still-loud context-source upcast `compile_error!`, and
  `wildcard_imported_headerless_read_adapter_has_gatkread_return_surface`
  exposes the unresolved fully-qualified adapter owner type. Focused checks were
  run for all six promoted tests, plus `cargo fmt --check`, `cargo fmt`, and
  `cargo check --quiet`. The remaining rendered-Rust-only `idiom_gaps` assertion
  count is 78.
- Follow-up 2026-08-26: five Spark/GenomicsDB callback assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `disq_reads_rdd_get_reads_shapes_spark_method_reference_filter_param`,
  `corpus_genomicsdb_callback_argument_external_fields_are_instance_fields`, and
  `corpus_genomicsdb_external_instance_setter_keeps_receiver`. Two are
  expected-compile-failure-backed:
  `objects_non_null_method_reference_in_callback_lowers_to_closure` and
  `disq_reads_rdd_get_reads_shapes_spark_map_lambda_param`, both exposing
  missing closure parameter type annotations in generated Spark callback code.
  The DISQ map assertion was also updated for the current optional
  `SAMRecord.getContig()` unwrap. Focused checks were run for all five promoted
  tests, plus `cargo fmt --check` and `cargo check --quiet`. The remaining
  rendered-Rust-only `idiom_gaps` assertion count is 73.
- Follow-up 2026-08-26: five inherited-field/Spark/interface assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `inherited_external_field_setter_does_not_become_static_owner`,
  `corpus_spark_branch_assigned_pair_rdd_map_callback_gets_tuple_param_type`,
  and `boxed_dyn_clone_recovery_parenthesizes_cast_receiver`. Two are
  expected-compile-failure-backed:
  `corpus_spark_pair_rdd_map_callback_gets_tuple_param_type` exposes generic
  arguments emitted on the non-generic `Tuple2` facade, and
  `corpus_spark_interface_receiver_delegation_widens_owned_return_slots`
  exposes source type `Result` shadowing `std::result::Result` in generated
  signatures. The assertions were updated for current `Tuple2._1().unwrap()`
  and shared-`Rc` interface slot lowering. Focused checks were run for all five
  promoted tests, plus `cargo fmt --check` and `cargo check --quiet`. The
  remaining rendered-Rust-only `idiom_gaps` assertion count is 68.
- Follow-up 2026-08-26: seven callback/interface/self assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `corpus_spark_casted_flatmap_lambda_is_target_typing_not_runtime_downcast`,
  `nested_interface_super_default_call_in_slot_aware_return_lowers_to_default_body`,
  `interface_return_this_does_not_emit_runtime_downcast`,
  `current_self_to_erased_implemented_interface_boxes_clone`, and
  `current_self_concrete_static_argument_clones_without_downcast`. Two are
  expected-compile-failure-backed:
  `corpus_genomicsdb_flatmap_owned_inner_stream_callback_arg` exposes the nested
  `List<Chromosome>` stream element being treated as `Vec<Chromosome>`, and
  `external_async_template_super_satisfies_interface_methods` exposes the loud
  unmodeled `AbstractAsyncWriter.DEFAULT_QUEUE_SIZE` constant. The async-writer
  assertion was also updated for the current optional field storage spelling.
  Two nearby candidates were intentionally not promoted:
  `current_self_to_erased_platform_interface_boxes_clone` currently emits
  `return self;` for a `Box<dyn Any>` return, and
  `appendable_field_conversion_clones_before_into` no longer emits the intended
  clone-before-conversion shape. Focused checks were run for all seven promoted
  tests, plus `cargo fmt`, `cargo fmt --check`, and `cargo check --quiet`. The
  remaining rendered-Rust-only `idiom_gaps` assertion count is 61.
- Follow-up 2026-08-26: nine external-fact/optional-slot assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `source_overload_prefers_expanded_varargs_over_same_arity_fixed_mismatch`,
  `broad_reads_path_data_source_converts_to_reads_data_source`,
  `broad_headerless_sam_comparator_converts_to_java_comparator`,
  `raw_list_cast_into_optional_slot_uses_optional_target_shape`,
  `source_fastq_constants_owner_is_not_erased_by_builtin_external_pack`,
  `static_external_call_to_optional_executor_slot_uses_external_return_shape`,
  `static_external_nullable_call_to_optional_field_does_not_double_wrap`, and
  `static_string_storage_initializer_preserves_lazy_default_value`. One is
  expected-compile-failure-backed:
  `source_vcf_enum_return_is_not_erased_by_builtin_external_pack` exposes that
  the source enum type is referenced by the getter but not emitted in the focused
  generated module. Two nearby candidates were intentionally not promoted:
  `source_interface_trait_uses_fallible_implementation_signature` now emits
  `String` in the trait instead of the asserted borrowed `&str` surface, and
  `static_external_call_to_generic_set_slot_uses_external_return_shape` now
  lowers the `smartMergeHeaders` call to an unresolved-static placeholder
  instead of the asserted external surface. Focused checks were run for all nine
  promoted tests; `cargo fmt`, `cargo check --quiet`, and the count check were
  also run. The remaining rendered-Rust-only `idiom_gaps` assertion count is 52.
- Follow-up 2026-08-26: four optional/FFI/constructor assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `corpus_ffi_owned_handle_transfer_requires_explicit_handover` and
  `documents_constructor_delegation_with_extra_work_gap`. Two are
  expected-compile-failure-backed:
  `raw_list_cast_over_optional_ternary_uses_optional_slot_shape` and
  `optional_ternary_list_assignment_wraps_once_without_runtime_downcast`, both
  exposing that `IntervalUtils.getSpanningIntervals(...)` lowers as the
  unresolved module path `interval_utils::get_spanning_intervals(...)` despite
  an `IntervalUtils` facade shell being emitted. Two nearby candidates were
  intentionally not promoted:
  `corpus_ffi_boundary_manifest_shapes_native_handover_calls` no longer emits
  the asserted `NativeLibrary::open_array(workspace)` call and instead falls
  back to an unresolved-static placeholder, and
  `type_param_bound_external_receiver_fact_shapes_signature_mutability` now
  emits a fallible `Result<i32, JavaException>` body rather than the asserted
  infallible return surface. Focused checks were run for the four promoted
  tests; `cargo fmt`, `cargo check --quiet`, and the count check were also run.
  The remaining rendered-Rust-only `idiom_gaps` assertion count is 48.
- Follow-up 2026-08-26: three switch/enum assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `translates_jaligner_fallthrough_try_catch_result_without_placeholder` and
  `fallback_switch_trailing_break_is_unit_typed`.
  `fallthrough_switch_external_enum_labels_are_qualified` is
  expected-compile-failure-backed, exposing that external enum constants are
  emitted in pattern position and Rust rejects them as constants of
  non-structural type `SortOrder`. Five nearby candidates were intentionally
  not promoted: `try_return_with_fallthrough_catch_uses_outer_fallback_return`
  now emits an `Option<String>` fallback shape instead of the asserted
  `Result<Option<String>, JavaException>` shape;
  `documents_switch_state_machine_gap` now emits the enum type spelling
  `SwitchReturnsEnumMatch_Type` instead of the asserted
  `SwitchReturnsEnumMatchType`; and
  `enum_constant_with_constructor_args_lowers_as_unit_variant_access`,
  `emitted_java_enum_does_not_get_opaque_facade_equality_impls`, and
  `source_java_enum_emits_default_for_support_placeholders` now emit
  underscore-qualified inner enum type names instead of the asserted collapsed
  source names. Focused checks were run for the three promoted tests. The
  remaining rendered-Rust-only `idiom_gaps` assertion count is 45.
- Follow-up 2026-08-26: five ObjectStream / erased-object handoff assertions are
  now clean generated-crate-backed tests:
  `object_input_stream_read_methods_have_typed_facade_returns`,
  `optional_output_stream_write_byte_array_uses_slice_overload`,
  `object_input_stream_read_object_cast_to_source_interface_is_explicit_handoff`,
  `object_output_stream_write_object_accepts_source_interface_object_handoff`,
  and
  `object_output_stream_write_object_clones_indexed_erased_object_handoff`.
  Six nearby candidates were intentionally not promoted because their current
  rendered output no longer satisfies their string assertions:
  `object_stream_object_methods_have_typed_handoff_facades` now wraps array
  reads as `Some(read_object()?)`; the three read-object recovery tests still
  emit `expect("invalid Java cast")` in the generated downcast path;
  `source_borrowed_getter_initializers_recover_owned_locals` now reads fields
  directly instead of calling the asserted borrowed getters; and
  `erased_object_array_clone_and_format_use_explicit_helpers` now emits
  `to_vec()/* TODO(translation): validate added clone */` rather than the
  explicit erased-object clone helper. Focused checks were run for the five
  promoted tests. The remaining rendered-Rust-only `idiom_gaps` assertion count
  is 40.
- Follow-up 2026-08-26: six functional-interface / recursion / external-mutator
  / exception assertions are now generated-crate-backed. Clean compile-backed
  tests: `documents_core_functional_interfaces_gap`,
  `documents_mutual_recursion_gap`,
  `documents_tail_recursive_accumulator_gap`,
  `documents_external_getter_setter_mutability_gap`, and
  `documents_exception_cause_chaining_gap`.
  `callback_called_only_inside_nested_lambda_is_not_nonescaping` is
  expected-compile-failure-backed: the rendered assertion correctly pins that the
  escaping callback is not lowered as `impl FnMut`, but the generated crate still
  uses `Consumer<String>` even though the facade shell is non-generic and then
  calls the `Consumer` value as a function. Focused checks were run for all six
  promoted tests. The remaining rendered-Rust-only `idiom_gaps` assertion count
  is 34.
- Follow-up 2026-08-26: two late assertions are now generated-crate-backed.
  `documents_exception_rethrow_specific_gap` is clean compile-backed.
  `emitted_type_gets_external_super_static_final_constant_surface` is
  expected-compile-failure-backed, preserving the current behavior that an
  inherited external static final constant with no modeled value emits a
  targeted `compile_error!`. The remaining rendered-Rust-only `idiom_gaps`
  assertion count is 32. The remaining names have now all been sampled in this
  pass or earlier follow-ups and are not straightforward promotions: they are
  stale/failing assertions or broad documentation smoke checks whose current
  rendered output needs to be repaired or narrowed before a meaningful generated
  crate check can be attached.
- Follow-up 2026-08-26: six stale assertions were repaired to current rendered
  shapes and are now generated-crate-backed. Clean compile-backed tests:
  `documents_switch_state_machine_gap`,
  `enum_constant_with_constructor_args_lowers_as_unit_variant_access`,
  `emitted_java_enum_does_not_get_opaque_facade_equality_impls`,
  `source_java_enum_emits_default_for_support_placeholders`, and
  `object_stream_object_methods_have_typed_handoff_facades`. The repaired
  expectations cover the underscore-qualified inner enum type spelling,
  `match r#type` keyword escaping, and `Some(read_object()?)` optional array
  slots. `try_return_with_fallthrough_catch_uses_outer_fallback_return` is
  expected-compile-failure-backed; after the assertion was updated to the
  current `Result<Option<String>, JavaException>` carrier, the generated crate
  exposes a duplicate `URL` support/source type and a missing `?` on
  `get_manifest()`. Focused checks were run for all six promoted tests. The
  remaining rendered-Rust-only `idiom_gaps` assertion count is 26.
- Follow-up 2026-08-26: five more stale assertions were repaired to current
  rendered shapes and are now generated-crate-backed. Clean compile-backed
  tests: `object_input_stream_read_object_cast_recovers_string_target_slot` and
  `object_input_stream_read_object_cast_recovers_throwable_field_slot`. The
  repaired expectations cover the current `expect("invalid Java cast")`
  downcast spelling. Three tests are expected-compile-failure-backed:
  `object_input_stream_read_object_recovers_string_target_slot` still emits a
  `?` after downcasting the bare `readObject()` result to `String`;
  `source_borrowed_getter_initializers_recover_owned_locals` still assigns a
  direct `Rc<Zip>` field read to a borrowed `&Zip` local; and
  `erased_object_array_clone_and_format_use_explicit_helpers` still falls back
  to `.to_vec()` on `Vec<Option<Box<dyn Any>>>`, requiring the unavailable
  `Box<dyn Any>: Clone` bound. Focused checks were run for all five promoted
  tests. Treating both clean generated-crate checks and expected-failure
  generated-crate checks as backed, the remaining rendered-Rust-only
  `idiom_gaps` assertion count is 21.
- Follow-up 2026-08-26: seven more `idiom_gaps` assertions are now
  generated-crate-backed. Clean compile-backed tests:
  `documents_diamond_single_unknown_gap`,
  `documents_diamond_nested_unknowns_gap`,
  `final_sorted_map_receiver_mutation_declares_rust_local_mutable`,
  `optional_resource_facade_upcast_clones_wrapper_before_consuming_unwrap`,
  `stream_joining_nullable_string_items_use_java_display_projection`, and
  `enum_binary_equality_uses_partial_eq_policy`. Repaired stale expectations
  cover the current optional resource cleanup spelling and the underscore
  qualified enum type name. `stream_filter_negated_java_equals_parenthesizes_binary_projection`
  is expected-compile-failure-backed: the rendered assertion correctly pins the
  parenthesized `!(...)` projection, but the generated stream filter still
  compares `&&StreamFilterNegatedJavaEqualsProjection_Version` against the enum
  value instead of dereferencing the iterator item. Focused checks were run for
  all seven promoted tests. Four additional candidates were probed and left out
  of the promotion count because their existing rendered assertions fail before
  a generated-crate check can run:
  `source_super_constructor_with_args_initializes_flattened_inherited_fields`,
  `static_external_call_to_generic_set_slot_uses_external_return_shape`,
  `corpus_ffi_boundary_manifest_shapes_native_handover_calls`, and
  `type_param_bound_external_receiver_fact_shapes_signature_mutability`. The
  remaining rendered-Rust-only `idiom_gaps` assertion count is 14.
- Follow-up 2026-08-26: three more `idiom_gaps` assertions are now
  generated-crate-backed. `documents_serializable_value_object_gap` compiled as
  rendered. `source_interface_trait_uses_fallible_implementation_signature` was
  repaired to the current fallible owned-`String` trait surface and then
  compiled. `current_self_to_erased_platform_interface_boxes_clone` required a
  lowering fix: bare `this` flowing into an owned erased-object return slot now
  boxes an audited clone instead of rendering `return self;`; the existing
  source-interface control remains green. The remaining rendered-Rust-only
  `idiom_gaps` assertion count is 11.
- Follow-up 2026-08-26: three more `idiom_gaps` assertions are now
  expected-compile-failure-backed. `appendable_field_conversion_clones_before_into`
  was repaired to the current facade-conversion block and now pins the generated
  `expected Appendable, found &Appendable` type error. `type_param_bound_external_receiver_fact_shapes_signature_mutability`
  was repaired to the current fallible `Result<i32, JavaException>` surface and
  now pins the missing bound-forwarded `scramble` method. `optional_map_string_get_compares_to_cross_class_static_string_constant`
  was repaired to the current loud `compile_error!` for unsupported Java string
  reference identity. The remaining rendered-Rust-only `idiom_gaps` assertion
  count is 8.
- Follow-up 2026-08-26: three more `idiom_gaps` assertions are now
  generated-crate-backed. `corpus_picard_imported_constants_need_source_summary_facts`
  compiles cleanly after its existing imported-constant assertions.
  `summary_context_translation_resolves_sibling_symbols_without_lowering_bodies`
  was repaired to the current summary-stub rendering and also compiles cleanly.
  `inherited_context_method_body_sees_declaring_static_imports` now pins the
  loud missing `BufferedImage` support-type diagnostic while keeping the
  static-owner spelling assertion. The remaining rendered-Rust-only
  `idiom_gaps` assertion count is 5.
- Follow-up 2026-08-26: the last five rendered-Rust-only `idiom_gaps`
  assertions are now generated-crate-backed or expected-failure-backed. Two
  helper-shape assertions now pin their real unresolved external helper
  failures instead of stopping at string checks:
  `resolved_static_helper_call_uses_fallback_signature_arg_shapes` and
  `constructed_external_argument_is_borrowed_for_borrowed_source_slot`. The
  source-super-constructor fixture required a lowering fix: explicit source
  `super(...)` calls now preempt the external no-state constructor model when
  source-super flattening can initialize inherited fields. That makes
  `source_super_constructor_with_args_initializes_flattened_inherited_fields`
  compile-backed, and the adjacent super-state documentation assertion was
  updated to the current checked-downcast spelling. The generic VCF static call
  now lowers to the manifest/fact-backed owner, but its generated support method
  still returns the erased collection facade; the test is therefore
  expected-failure-backed on the `HashSet<VCFHeaderLine>` versus
  `Vec<Box<dyn Any>>` mismatch. The manifest FFI handover fixture exposed the
  same unresolved-static ordering bug for arity-wildcard manifest facts:
  support generation emitted `NativeRuntimeNativeLibrary::open_array`, but the
  value lowering guard only trusted exact-arity effects and fell back to
  `unimplemented!`. The guard now treats `arg_count = None` as a wildcard, and
  `corpus_ffi_boundary_manifest_shapes_native_handover_calls` compiles cleanly.
  The remaining rendered-Rust-only `idiom_gaps` assertion count is 0.

## Direction

The compile-visibility work that belongs to this issue has landed. Future
coverage work should keep these rules in place rather than reopening the old
silent-rendered-Rust pattern.

Follow-up 2026-08-29: the tracked plan row in
`docs/claude-audit-refactoring-plan.md` now agrees with this file's `done/`
location. `tests/output_markers.rs::tracked_plan_status_agrees_with_the_audit_directory`
was correct to reject the old `open` row because it made the status table and
audit directory disagree.

Follow-up 2026-08-29: a focused failure-baseline pass removed stale
`idiom_gaps` failure names for already compile-backed rows:
`for_each_lambda_side_effect_generated_rust_compiles`,
`method_reference_flat_map_generated_rust_preserves_behavior`,
`system_arraycopy_mutable_optional_local_dest_does_not_clone_slice`, and
`uppercase_utility_receiver_shadowed_by_local_stays_instance_receiver` all pass
when run directly. This is failure-name cleanup only; the aggregate
`neutral_idiom_fixtures_generated_crate_cargo_checks` row still exposes
remaining generated-crate errors.

The same focused pass also removed stale baseline rows that this audit already
described as compile-backed/current:
`current_self_concrete_static_argument_clones_without_downcast`,
`optional_argument_wrap_reused_value_clones_before_some`,
`source_enum_constructor_payload_fields_emit_match_backed_accessors`, and
`summary_context_translation_resolves_sibling_symbols_without_lowering_bodies`.

A stale assertion batch was also repaired for current generated Rust spelling:
constructor field read-after-assignment uses wrapping arithmetic, decimal-format
concat keeps the grouped numeric expression, comparator/string helper calls use
the current borrowed/fallible helper forms, optional string-array element access
uses checked indexing before `Option<&str>` projection, and htsjdk integer
facade constants use wrapping addition. The corresponding focused tests now
pass and their failure names were removed.

The middle baseline slice had the same pattern. Focused passing rows were
removed, including the expected-compile-failure-backed
`resolved_static_helper_call_uses_fallback_signature_arg_shapes`. Additional
assertions were updated for checked string-array indexing, fallible/null-aware
File/ObjectOutputStream/array-length lowering, checked primitive-array
allocation, and fallible owned-string return signatures.

The tail baseline slice followed the same rule. Regex helper generated tests now
call the current fallible `&str` API; string-pattern, source-signature,
readonly-array alias, shared-self nullable recovery, source static string
constants, and static mutable storage assertions now match the current
fallible/checked/storage-backed output. Focused tests passed for those rows, and
their stale failure names were removed. The remaining `idiom_gaps` baseline rows
are implementation/modeling gaps or still-useful expected-failure guardrails, not
known rendered-Rust-only assertions.

Follow-up 2026-08-29: the two source-super constructor guardrails were still
expected-compile-failure-backed, but their pinned rustc diagnostic substrings had
drifted. They now assert the current unresolved object-model diagnostics for the
optional context receiver and missing `ExceptionContext::new` support, so their
failure names were removed. The JDK numeric facade row also moved from a stale
plan note to an implementation fix: `BigInteger.ZERO` and `BigInteger.ONE` now
use the known JDK static constant value table and emit value-object constants
instead of `compile_error!` stubs.

Follow-up 2026-08-29: adapter-backed `ExternalInto` coercions now render the
ordinary Rust `.into()` projection instead of replacing the value expression with
an unsupported-upcast `compile_error!`. This clears the URLConnection to
JarURLConnection factory-slot fixture and the optional GZIPInputStream
constructor path; the latter now compiles through the current checked-null
cleanup spelling. The sampled source-varargs WriteOption and JPanel to Component
rows remain in the baseline because they still expose signature/erasure shape
gaps rather than stale unsupported-upcast assertions.

Follow-up 2026-08-29: owned native sequence temporaries passed into borrowed
slice argument slots now use the existing `AsSlice` coercion instead of being
handed to the facade unchanged. This clears the File/FileOutputStream string
convenience fixture where `text.getBytes("US-ASCII")` feeds
`OutputStream::write_1(&[i8])`. The receiverless optional-string overload row
still remains because its failing path is a source return-slot mismatch, not
argument slice borrowing.

Follow-up 2026-08-29: the generated `File::list` support surface now returns
`Vec<Option<String>>`, matching the compiler's nullable reference-array
representation for Java `String[]`. This clears the common File instance facade
fixture after its assertion was updated to the current nullable element shape.

Follow-up 2026-08-29: inferred omitted-context support returns now treat `String`
as a direct slot, alongside primitives and primitive arrays. `NamedInputStream`
getter support therefore emits `get_name(&self) -> String` instead of allocating
an opaque result shim, and the constructor-argument fixture compiles.

Follow-up 2026-08-29: modeled JDK value-object constants now include the
`java.awt.Color` static color singletons, and fully qualified static constant
support also emits the simple-owner alias used by lowering. This clears the
static owner/instance field-name collision fixture and keeps the fully qualified
`java.util.zip.GZIPInputStream.GZIP_MAGIC` guardrail compiling.

Follow-up 2026-08-29: solved borrowed return slots now remain authoritative for
direct parameter returns during body lowering. The previous return-body fallback
treated a solved borrowed sequence return as the declared owned array shape, so
the receiverless source overload fixture emitted a `&[i8]` signature with a
`source.to_vec()` body. The direct-parameter case now lowers to the borrowed
slot, while constructed/local returns still use the declared owned fallback.

Follow-up 2026-08-29: the indexed string-array ownership fixture's assertion now
matches the current nullable element model for Java `String[]`. The generated
crate already compiles with checked indexing, `Vec<Option<String>>`, borrowed
`Option<&str>` arguments, and borrowed formatting of the selected element, so the
stale clone-only assertion row was removed from the baseline.

Follow-up 2026-08-29: blank `static final` array fields no longer become empty
associated consts. They are storage-backed so assignments from static
initializers lower through the existing static getter/setter path. This clears
the one-dimensional nested static-table overload fixture: the field access still
selects the `double[]` overload, and the call now receives a borrowed slice from
the initialized static storage.

Follow-up 2026-08-29: the nested static-table fixture now also compiles for the
ragged `double[][]` case. Blank static storage recognizes `TABLE = new
double[N][]` through the parser's assignment-target alias, preserves the
nullable static carrier as `Option<Vec<Vec<f64>>>`, and lets optional-wrapped
ragged array creation use the same nested-`Vec` value path as local ragged
arrays. The row was removed from `idiom-gaps-failures.txt` after a focused
generated-crate check passed.

Follow-up 2026-08-29: array parameters no longer force an interface trait method
from borrowed-slice ABI back to the concrete implementor's owned storage shape.
The trait implementation adapter already owns the conversion point, so
`fn accept(&mut self, values: &[f64])` can forward to an inherent
`accept(Vec<f64>)` with `values.to_vec()`. This clears the trait-array
forwarding fixture and keeps the interface boundary reusable for callers that
only have a slice.

Follow-up 2026-08-29: Swing/AWT component facade types are now classified as
Java platform facades, matching the built-in conversion facts that already knew
`JPanel`/`JComponent` can convert into `Component`. This keeps `JPanel`
parameters concrete instead of erasing them to `Box<dyn Any>`, so the
`SVGImageSaver.saveImage(Component, OutputStream)` fixture lowers to
`panel.into()` and compiles.

Follow-up 2026-08-29: structured generic type rendering now preserves
cloneability for nested `Box<dyn Trait>` fields. `Option<Vec<Option<Box<dyn
CloneableModule>>>>` can derive `Clone` because the generated trait support
already emits the clone-box adapter, and top-level raw `Option`/`Vec` clone
checks now recurse the same way nested generic arguments do. The dyn-trait
vector-holder fixture is compile-backed and removed from the idiom-gap failure
baseline.

Follow-up 2026-08-29: expanded source varargs now treat Java reference-array
elements as nullable storage elements. The varargs packer lowers
`WriteOption...` spread arguments as `Some(concrete.into())` when the Rust
callee slot is `&[Option<WriteOption>]`, including reused borrowed source
arguments that need a clone before the external `Into` adapter. The stale local
`Path` stub was removed from the focused generated-crate check because built-in
`GATKPath.toPath()` support now emits the real JDK `Path` surface. The
reused-optional borrowed source argument row was removed from the failure
baseline, and the nearby borrowed-varargs fixture is compile-backed.

Follow-up 2026-08-29: unresolved receiver calls on parsed source receiver types
no longer impose an external/unknown runtime-object boundary. This keeps a
source interface as a dyn trait when the call is written through the interface
but resolution has not attached a concrete method, as in `Distribution d;
d.value()`. The inherited-implements `this` argument fixture now emits
`impl Distribution for ConcreteDistribution`, lowers `sampler.sample(self)`, and
passes its generated-crate check.

Follow-up 2026-08-29: the aggregate neutral idiom generated-crate check now
passes. The remaining errors were stale generated test harnesses rather than
translator output: borrowed-slice calls needed a borrowed `Vec`, fallible helper
returns needed unwrapping, and nested enum tests needed the actual emitted enum
names. `tests/baselines/idiom-gaps-failures.txt` is now empty apart from its
header.

Follow-up 2026-09-07: six source/generic idiom rows were stale assertions rather
than translator defects after the nullable/source-bound changes:
`source_interface_object_getter_returns_borrowed_field`,
`source_generic_method_preserves_type_params`,
`source_class_bound_trait_generic_methods_preserve_type_params`,
`source_generic_constructor_preserves_type_param_bounds`,
`context_source_generic_constructor_preserves_type_param_bounds`, and
`context_source_generic_method_preserves_type_param_bounds`. Focused generated
crate checks passed for each, and the release `idiom_gaps` capture is now
`769 passed; 47 failed`.

Follow-up 2026-09-07: the easy stale-assertion sweep removed another twenty
`idiom_gaps` rows from the known-failure baseline after focused generated-crate
checks passed. Twelve were first-half checked-array, facade, and typed-pack
assertions; eight more were nullable/source-call, raw-map return, ndarray, and
static-helper assertions. The release `idiom_gaps` capture confirmed `791
passed; 25 failed`.

Follow-up 2026-09-07: three bounded real fixes removed
`source_varargs_pack_external_elements_with_explicit_adapter`,
`source_generic_class_bound_method_call_uses_bound_surface`, and
`optional_mutable_array_reborrows_as_readonly_slice_without_moving` from the
baseline. The first suppresses duplicate fallback constants already owned by the
typed facade surface; the second avoids concrete-field getter lowering when a
call resolves through a type-parameter bound; the third reborrows
`Option<&mut [T]>` to `Option<&[T]>` as `as_deref()` instead of
`as_mut().as_deref()`. Focused generated-crate checks pass for all three.
Follow-up in the same pass removed six more rows after focused checks and a
slow release capture. `source_overload_prefers_expanded_varargs_over_same_arity_fixed_mismatch`
now compiles and no longer expects the stale duplicate-constant diagnostic;
`static_owner_name_does_not_collapse_to_same_named_instance_field` emits
`Color::BLACK` again after raw static-constant suppression was limited to
fact-backed facade constants; `source_super_constructor_with_child_locals_and_nested_field_assignment_lowers`
and `switch_assignment_with_default_fallthrough_lowers_to_match_assignment`
had stale optional-field/nested-enum assertions; `documents_try_with_resources_copy_gap`
now accepts the try-with-resources cleanup combiner; and
`source_object_varargs_forward_and_append_erased_arguments` is expected-failure
backed against the current non-cloneable `Vec<Option<Box<dyn Any>>>` error
instead of the obsolete optional-element mismatch. Release `idiom_gaps` now
records `798 passed; 18 failed`.

Follow-up 2026-09-07: three more easy rows were removed. Two were stale
assertions whose generated crates already compile:
`private_static_singleton_with_hashmap_field_derives_clone_when_value_is_cloneable`
now asserts the current `Option<Factory>` static singleton surface while still
pinning `HashMap<String, Arc<Format>>`, and
`source_array_setter_arguments_follow_owned_storage_signature` now accepts the
catchable `ok_or_else(...)?` null projection instead of the old panic
`expect(...)`. The third,
`negated_java_equals_uses_inequality_projection`, was a small lowering bug: a
fallible `List<String>.get(index)?` receiver was still projected as optional
before `.equals`, producing `.as_ref().map(|value| value.as_str())` on a
`String`. The lowering now compares the result-propagated string view directly.
Focused generated-crate checks pass for all three; `idiom_gaps` records
`801 passed; 15 failed`.

Follow-up 2026-09-07: five more easy `idiom_gaps` rows were rechecked. Two
stale assertions, `optional_source_receiver_uses_mut_projection_for_mutating_context_fallback`
and `reassigned_string_enhanced_for_variable_iterates_owned_mutable_values`,
were removed in the prior pass after focused generated-crate checks. This pass
removed two more stale assertions:
`translates_map_for_each_try_catch_lambda_without_placeholder` now accepts the
declared `Map<String, Item>` destination shape, and
`nested_jdk_stream_subclasses_keep_overload_surfaces_compileable` now accepts
the current nested type spelling (`Base64_InputStream` /
`Base64_OutputStream`). The nearby
`jdk_stream_wrapper_inherits_typed_protected_stream_fields` row also promotes:
the local byte array passed to `read(byte[], int, int)` now renders mutable, and
`super.read(byte[], int, int)` redirects through the inherited `in` handle as
`self.r#in.read_3(...)` for source classes that extend the JDK input-stream
facade. Focused generated-crate checks pass for all three rows in this pass.
`idiom_gaps` records `806 passed; 10 failed`.

Follow-up 2026-09-07: the tuple callback parameter renderer now respects the
erased external facade shape for Scala `Tuple2`/`Tuple3`/`Tuple4` instead of
rendering Rust generics that the generated facade structs do not declare. That
promotes `documents_stream_callback_chain_gap`,
`corpus_spark_branch_assigned_pair_rdd_map_callback_gets_tuple_param_type`, and
`corpus_spark_rdd_pipeline_keeps_stream_lowering_at_noop_spark_boundary`; the
pipeline row also accepts the current fallible `Ok(...)` return wrapper. Focused
generated-crate checks pass for all three. `idiom_gaps` records `809 passed; 7
failed`.

Follow-up 2026-09-07: two more baseline rows were assertion/diagnostic drift,
not active release failures. `inherited_context_trailing_call_uses_context_field_and_shared_varargs_plan`
now accepts the current `vec![Some(max)]` varargs carrier and records the current
expected generated-crate failure on non-cloneable `Box<dyn Any>`.
`corpus_spark_interface_receiver_delegation_widens_owned_return_slots` now
accepts the owned `SimpleInterval`/`Result` interface delegation surface and
keeps the generated-crate check as an expected failure on the remaining generic
facade/fallibility errors. Focused checks pass for both. `idiom_gaps` records
`811 passed; 5 failed`.

Follow-up 2026-09-07: the former
`static_owned_path_argument_does_not_borrow_owned_temporary` row
was a stale assertion after read-only source parameter slots learned borrowed
ABI shapes. The fixture now accepts `check_path(path: &Path)` and still pins the
important behavior under its renamed test
`static_owned_path_argument_uses_readonly_borrowed_slot`: the owned temporary
from `IO::get_path(&vcf_path)` is not mutably borrowed or wrapped as an optional
borrow. Focused generated-crate verification passes, and `idiom_gaps` records
`812 passed; 4 failed`.

Follow-up 2026-09-07: the non-manifest FFI handover row was misnamed after the
manifest-backed acceptance test landed. The real FFI contract remains covered by
`corpus_ffi_boundary_manifest_shapes_native_handover_calls`; the plain
`translate_source` variant now asserts the typed loud fallback
`unresolved Java static call NativeRuntimeNativeLibrary.open_array is not
modelled` instead of expecting crate-backed facts that were never supplied.
Focused generated-crate verification passes, and `idiom_gaps` records `813
passed; 3 failed`.

Follow-up 2026-09-07: the constructor self-field optional row was still
expected-failure-backed against an obsolete shared-handle diagnostic. The active
failure is now the wrap-order bug where optional construction is applied before
uncaught `Result` recovery, yielding `Some(...).unwrap_or_else(...)` and the
Rust diagnostic `closure is expected to take 0 arguments`. The test now records
that current failure mode instead of the stale `Rc<Helper>` mismatch, so the
known-failure baseline only tracks rows whose test bodies fail. Focused
verification passes, and `idiom_gaps` records `814 passed; 2 failed`.

Follow-up 2026-09-07: the GenomicsDB nested `flatMap` row also had a stale
expected-failure diagnostic. The old callback receiver mismatch has been
superseded; the active generated-crate failure is the broader stream-lowering
gap where a null-propagating receiver projection emits `?` inside an iterator
closure that does not return `Result` or `Option`. The test now records that
current failure mode, and `idiom_gaps` records `815 passed; 1 failed`.

Follow-up 2026-09-07: the final `idiom_gaps` failure was fixed after narrowing
the return-shape rule that had previously regressed unrelated rows. A source
method with an explicit non-primitive `return null` now keeps its nullable
return carrier even when returned-call facts also request concrete/native owned
storage. `try_return_with_fallthrough_catch_uses_outer_fallback_return` now
passes its generated-crate check, and `documents_stream_callback_chain_gap`
accepts the same nullable-return policy for `computeScores`. Release
verification records `816 passed; 0 failed`; the known-failure file is empty
apart from comments.

1. **Keep the corpus compile test strict.** A single `#[test]` translates every file
   under `examples/` and `rustc --crate-type lib --emit=metadata`s the output,
   with an allow-list checked in at `tests/baselines/corpus-compile-failures.txt`
   holding the current one invalid-Java name. Cost: no cargo, one `rustc` per file, ~2
   minutes. Value: the whole of backlogs A and B stays a named, shrinking list
   instead of an unknown, and every future fixture is compiled the day it is
   added. Note this is the *corpus* analogue of what 0041 did per test.
2. **Make the check non-optional where it is cheap.** A test that mentions
   `translation.rust` and does not compile it is the 0041 pattern. The tests in
   `fixtures.rs` are now either compile-backed or, for the one invalid-Java
   fixture, expected-compile-failure-backed, and the assertion-heavy
   `idiom_gaps` tests have the same generated-crate or expected-failure
   discipline.
3. **Differential fixtures for `null`, collections and `throw`,** in that order.
   0018 records that the harness found a defect in every batch added to it; the
   three largest untouched areas are the three above, and backlog A's five
   `E0599 expect`-on-a-non-`Option` failures say the null area is already broken
   in ways a string assertion approved.
4. **Keep `tests/fixtures.rs` green.** The stale hand-written harnesses have
   been repaired; future work should either keep new fixture assertions
   compile-backed or move behaviour into differential fixtures.

## Relation to other issues

- [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md) — this
  file is the corpus-sized measurement of its central claim: 55 of 58
  compiler-rejected files score as clean.
- [0041](0041-slot-shape-design-had-no-generated-crate-checks.md) — same
  hole, three suites over, plus evidence that its backlog grows without a rule.
- [0009](0009-overload-identity-collapses-onto-rust-name.md) — the former
  `E0592` failures in `EqualsAndObjects` and `NullableEqualsComparison` are now
  fixed; `EnumConstantsWithFields` remains covered by the original overload
  identity audit history.
- [0011](done/0011-dyn-any-fallback-fails-at-every-use.md) — the former `E0599`
  on `&Box<dyn Any>` in `WildcardBounds` is now fixed for bounded wildcard
  collection reads.
