# 0057 — Java exceptions still leak as panics, and general `finally` completion is not represented

Recorded 2026-08-23.
Status: **done for the audited actual-error scope**.

Codex follow-up 2026-09-17 fallibility traversal completion: the fallibility
planner now descends into both operands of array access and the qualifier of a
qualified constructor expression. Those positions previously could hide an
integer division/remainder, `Math.toIntExact`, or nullable receiver dereference
from the enclosing `Result` decision. Guards:
`exception_plan::tests::{integer_div_rem_scan_sees_array_index,
integer_div_rem_scan_sees_qualified_new_receiver,
math_to_int_exact_scan_sees_qualified_new_receiver,
nullable_receiver_deref_scan_sees_qualified_new_receiver}`. The related
non-throwing interface override group also remains explicitly in fallible-free
state via `non_throwing_interface_override_group_stays_infallible`.

Codex follow-up 2026-09-16 rectangular primitive-array carrier pass:
representation-aware slot rendering now selects `ndarray::Array2<T>` for
primitive `T[][]` before its generic `Vec` fallback. This closes a generated-
crate regression where catchable 2D creation/read lowering correctly emitted
`Array2` operations but a local declaration still said `Vec<Vec<T>>`. The
negative-size, row-read, and element-read catch paths now compile and execute;
guards: `idiom_gaps::{array2_creation_negative_dimension_in_try_lowers_to_java_exception_result,
array2_index_read_in_try_lowers_to_java_exception_result,
array2_row_read_in_try_lowers_to_java_exception_result}` and
`slot_shape_design::representation_aware_type_rendering_keeps_rectangular_primitive_arrays_as_array2`.

Closure recheck 2026-09-13: two independent current-state audits found no
remaining named 0057 fixture or TODO2 item that still reproduces as an uncovered
semantic error. After rebuilding the translator from a clean `target/`, the
focused gates passed:

- `cargo test --test idiom_gaps finally -- --nocapture`: 24/24
- `cargo test --test differential finally_completion_fixtures_match_java_output -- --nocapture`
- `cargo test --test differential list_exception_fixtures_match_java_output -- --nocapture`

The historical notes below still document why a fully general Java completion
carrier would be cleaner than the accumulated bounded fast paths. That broader
representation work is design debt, but this audit no longer tracks a known
current semantic failure.

Current checkpoint 2026-09-09: checked-in release `idiom_gaps` baseline is
`816 passed; 0 failed`, and the focused exception/finally oracle loops cover
the promoted exception, collection-exception, parse-exception, and completion
fixtures named in this file. The audit remains open for the general completion
carrier and remaining panic-only Java exception surfaces, not for the
already-promoted fixture set.

Codex recheck 2026-09-12 actual-error focus: the current pinned gates for this
audit pass. `cargo test --test idiom_gaps finally -- --nocapture` passed 23/23,
`cargo test --test differential finally_completion_fixtures_match_java_output
-- --nocapture` passed, and `cargo test --test differential
list_exception_fixtures_match_java_output -- --nocapture` passed. A scan for
tests still accepting `unsupported Java statement: try/catch/finally` found
only negative guards. The remaining 0057 work is therefore not an already-pinned
fixture failure; it is the broader completion-carrier design and future
concrete panic-only helper surfaces as they are found.

Codex follow-up 2026-09-12 thrown-fact drift pass: the built-in facade model for
`Random.nextInt(int)` already lowered through `Random::next_int_result` and the
differential `RandomNextIntCatch` fixture catches `IllegalArgumentException`,
but the parallel external-pack effect fact still advertised no thrown
exception. `external_packs::java_platform_api_effect_for_receiver_with_arity`
now reports `IllegalArgumentException` for both `Random.nextInt` spellings, and
`tests/semantic_facts.rs::random_next_int_fact_advertises_illegal_argument_exception`
pins the arity-sensitive fact lookup.

Codex follow-up 2026-09-12 Pattern.compile catchability pass:
`Pattern.compile(String)` now has a bounded `Pattern::compile_result` path and
advertises `PatternSyntaxException` from the built-in external-pack effect
fact. Propagated Java `try` contexts call the result helper, so malformed
patterns such as `"["` are catchable instead of silently constructing a
`Pattern`; ordinary callers still use the panic-adapting `Pattern::compile`.
`PatternSyntaxException` is also linked under `IllegalArgumentException` in the
generated exception hierarchy. Covered by
`tests/idiom_gaps.rs::pattern_compile_invalid_regex_is_catchable` and
`tests/semantic_facts.rs::pattern_compile_fact_advertises_pattern_syntax_exception`.

Codex follow-up 2026-09-12 actual-error pass: three more compile/catchability
surfaces are fixed and pinned in `tests/idiom_gaps.rs`. `ArrayDeque.getLast()`
and `removeLast()` now share the catchable queue-tail projections instead of
rendering missing `Vec::get_last`/`Vec::remove_last` methods
(`array_deque_last_aliases_are_catchable_and_compile`). `Integer.decode` and
`Integer.valueOf(String)`, including the string-concat operand path, now lower
through catchable `NumberFormatException` parse projections instead of
`Integer::decode` or `<Integer>::value_of("...")`
(`integer_decode_and_value_of_string_are_catchable`). A nested
try/finally-return carrier now wraps `Ok(Some(value))` through block
expressions and keeps an impossible `Ok(None) => unreachable!()` arm, avoiding
both `Result<String>`/`()` fallthrough and non-exhaustive generated matches
(`nested_try_finally_return_wraps_optional_carrier_value`). Verified with
`cargo check --tests` and the three focused generated-crate tests.

Codex follow-up 2026-09-12 helper-surface pin: `StringBuffer`/`StringBuilder`
helper methods that keep panic wrappers for ordinary uncaught calls are
verified catchable in propagated `try` contexts. `StringBuffer.{charAt,
setLength,delete,setCharAt,deleteCharAt,replace}` and
`StringBuilder.{insert,substring,setCharAt,deleteCharAt,replace}` lower through
`*_result` helpers with `?`, so Java `catch
(StringIndexOutOfBoundsException)` observes the failure instead of a Rust
panic. Covered by
`tests/idiom_gaps.rs::string_buffer_mutating_helpers_in_try_are_catchable` and
`tests/idiom_gaps.rs::string_builder_insert_and_substring_helpers_in_try_are_catchable`.
The broader audit remains open for helper surfaces that still lack a propagated
result path and for the general completion carrier.

Codex follow-up 2026-09-12 `setCharAt` actual-error pass:
`StringBuffer.setCharAt` and `StringBuilder.setCharAt` now share the same
result-helper path. Invalid indices propagate `StringIndexOutOfBoundsException`
through `*_result()?` inside Java `try`, while valid calls replace by Java
character index instead of byte offset.

Codex follow-up 2026-09-12 `replace`/`deleteCharAt` actual-error pass:
`StringBuffer.replace`, `StringBuilder.replace`, `StringBuffer.deleteCharAt`,
and `StringBuilder.deleteCharAt` no longer emit missing Rust methods in Java
`try` blocks. They now use catchable result helpers, preserve fluent mutation
on success, clamp `replace` end to the buffer length, and reject invalid start
or character indices with `StringIndexOutOfBoundsException`.

Codex follow-up 2026-09-12 list-iterator compile/catchability pass:
`List.listIterator()` no longer lowers to a missing `Vec::list_iterator()` call
or a non-emitted `ListIterator<T>` facade type. It now renders a
`JavaListIterator<T>` runtime cursor, includes that support block when
`java.util.ListIterator` is imported or declared, and routes
`ListIterator.previous()` through `previous_result()` so exhaustion is a
catchable `NoSuchElementException` in propagated `try` contexts. Covered by
`tests/idiom_gaps.rs::java_list_iterator_previous_is_catchable`, with the
existing `java_list_iterator_cursor_uses_platform_facade` guarding ordinary
`Iterator` behavior.

Codex follow-up 2026-09-12 indexed list-iterator actual-error pass:
`List.listIterator(int)` no longer ignores its index argument. Propagated Java
`try` contexts now lower the call through `JavaListIterator::new_at_result`, so
negative or too-large indices raise catchable `IndexOutOfBoundsException`
instead of constructing a cursor at zero and later misclassifying the failure
as `NoSuchElementException`. Covered by
`tests/idiom_gaps.rs::java_list_iterator_index_is_catchable`.

Codex follow-up 2026-09-12 radix-parse compile-error pass:
integral radix parse helpers now pass a borrowed string view to
`from_str_radix`. `Integer.parseInt(String,int)`,
`Long.parseLong(String,int)`, `Short.parseShort(String,int)`, and
`Byte.parseByte(String,int)` no longer emit `from_str_radix(String, ...)`,
which failed to compile before Java `NumberFormatException` handling could run.
Covered by
`tests/idiom_gaps.rs::integer_radix_parse_string_argument_is_catchable`.

Codex follow-up 2026-09-12 string/class catchability pass:
`Class.forName(String)` now lowers through `JavaClass::for_name_result` in
propagated Java `try` contexts, so a missing class becomes catchable
`ClassNotFoundException` instead of an invalid `?` on an infallible
`JavaClass`. `String.getBytes(String)` is now distinct from zero-argument
`getBytes()`: the charset overload lowers through `java_string_get_bytes_result`
with a catchable `UnsupportedEncodingException`, while zero-argument `getBytes`
keeps the direct byte-vector helper. Covered by
`tests/idiom_gaps.rs::class_for_name_missing_class_is_catchable`,
`tests/idiom_gaps.rs::java_string_get_bytes_charset_is_catchable`, and the
existing `java_string_get_bytes_and_index_of_use_native_helpers` generated
crate check.

Codex follow-up 2026-09-12 copy/getChars actual-error pass:
`System.arraycopy` is no longer an emitted no-op. The runtime facade now copies
between source and destination slices, and the optional-local destination
fixture runs a generated crate assertion that proves bytes are copied.
`String.getChars` now lowers as a free helper instead of an impossible method
on `&str`, while `StringBuilder.getChars`/`StringBuffer.getChars` have runtime
facade methods. Their destination `char[]` argument is treated as mutating, so
locals passed as the destination render as `mut`, and invalid ranges propagate
`StringIndexOutOfBoundsException` in Java `try` contexts. Covered by
`tests/idiom_gaps.rs::system_arraycopy_mutable_optional_local_dest_does_not_clone_slice`
and
`tests/idiom_gaps.rs::java_string_get_chars_helpers_are_catchable_and_copy_chars`.

Codex follow-up 2026-09-12 iterator-mutator actual-error pass:
`Iterator.remove()` and `ListIterator.set(value)` no longer fall through to
missing runtime methods or panic-only call sites. The current iterator runtime
is snapshot-backed, so these mutators are modeled explicitly as catchable
`UnsupportedOperationException` rather than pretending to update the backing
collection. Covered by
`tests/idiom_gaps.rs::iterator_snapshot_mutators_are_catchable_unsupported_operations`.
Full Java backing-collection iterator mutation remains out of scope for this
bounded fix and would require changing the iterator representation.

Codex follow-up 2026-09-12 arraycopy bounds catchability pass:
`System.arraycopy` keeps the plain panic-adapting helper for uncaught calls but
now also emits `System::arraycopy_result` for propagated Java `try` contexts.
Out-of-range source or destination slices return catchable
`IndexOutOfBoundsException` instead of panicking inside Rust slice indexing.
Covered by
`tests/idiom_gaps.rs::system_arraycopy_bounds_failure_is_catchable`.

Codex follow-up 2026-09-12 erased-object compile-error pass: borrowed facade
recovery into erased `Object` slots no longer renders
`Box::new(value.clone()) as Box<dyn Any>` for erased values. It now uses the
existing `java_clone_erased_object_ref` helper, preserving concrete clone/box
recovery only for non-erased borrowed values. Return-shape selection also keeps
solved `Option<Object>` return carriers instead of collapsing them back to a
declared bare `Object`, and fallible optional receiver projection stops applying
an optional-erased facade map after `ok_or_else(...)?` has already produced the
borrowed element. Verified with the focused generated-crate fixtures for
constructor fallible cast recovery, cross-source null-check helpers,
erased-object `getClass().{getName,isArray}`, raw-list `instanceof`, plus
`cargo check --tests` and `cargo test --test idiom_gaps catchable`.

Codex follow-up 2026-09-12 nullable parse sequence-element pass:
`Integer.parseInt(values.get(i))` where `values` is a nullable `List<String>`
no longer emits `Option<String>.parse::<i32>()`. Propagated native `List.get`
now records the actual element storage shape: true nullable elements keep their
owned nullable carrier, while ordinary `String` elements stay bare so nullable
local/return slots wrap them through the usual slot conversion. The integral
parse null-policy recovery also recognizes a fallible cloned sequence-get
receiver that has already been coerced to `.as_str()`. The generated Rust maps
the Java null input to `"null"`, so `NumberFormatException` remains catchable
instead of producing invalid Rust or a synthetic `NullPointerException`.
Verified with
`tests/idiom_gaps.rs::integer_parse_int_nullable_sequence_element_is_number_format_exception`,
`tests/idiom_gaps.rs::sequence_get_in_try_slot_coercion_is_catchable`,
`tests/idiom_gaps.rs::catch_and_finally_locals_receive_signature_plans`,
`lower::tests::integral_parse_shaped_optional_string_arg_maps_null_to_number_format_input`,
`cargo test --test idiom_gaps catchable`, `cargo test --test idiom_gaps finally`,
and `cargo check --tests`.

Codex follow-up 2026-09-12 source exception constructor-state compile pass:
source-defined classes whose names end in `Exception` no longer collapse to the
`JavaException` carrier in ordinary source type positions. The carrier predicate
now uses the modeled JDK/runtime throwable table instead of a suffix rule, so a
source constructor parameter such as `CauseStateException owner` remains the
source struct while JDK exception supertypes still render as `JavaException`.
Self-dependent constructor field assignments also apply the shared-handle fresh
construction wrap on the deferred assignment path, and source structs used as
those constructor placeholders derive `Default` only when their fields support
it. The
`external_standard_exception_super_with_trailing_body_lowers_subclass_state`
fixture now cargo-checks instead of being pinned as an expected
`ExceptionContext: Default` failure. Verified with that focused generated-crate
test, `slot_render::tests::exception_like_java_types_render_to_java_exception_carrier`,
and `cargo test --test idiom_gaps exception`.

Codex follow-up 2026-09-12 enhanced-for continue fail-fast pass: the bounded
enhanced-for-over-mutated-local lowering now handles a loop-body `continue`
instead of falling back to a compile-time placeholder. The synthesized
index-based loop rewrites `continue` sites to run the same
`ConcurrentModificationException` length check before advancing the synthetic
index and continuing; a mutation still throws before the back edge, while a
non-mutating continue proceeds to the next element. Nested-loop continues remain
outside this bounded lowering. Regression:
`enhanced_for_mutating_iterated_collection_with_continue_throws_concurrent_modification`.
Verified with the two focused enhanced-for generated runtime tests.

Codex follow-up 2026-09-12 erased varargs compile pass: source `Object...`
forwarding no longer relies on `Clone` for `Box<dyn Any>` or
`Vec<Option<Box<dyn Any>>>`. Explicit erased varargs arrays and appended erased
varargs tails now use `java_clone_optional_erased_object_vec_ref`, which accepts
both `Vec` and slice callers, while reused scalar `Object` arguments use
`java_clone_erased_object_ref`. Receiver-mutability propagation also recognizes
resolved source mutating calls through a self field when the exact resolved
varargs key misses. `source_object_varargs_forward_and_append_erased_arguments`
now cargo-checks.

Codex follow-up 2026-09-12 source-context boundary compile pass: non-emitted
context sources now contribute opaque boundary types, used constructors, and
used methods to the external facade support surface. Source-summary varargs
method arity matching accepts fixed-prefix calls, facade stubs render declared
argument types instead of unconstrained generics, inherited context receiver
projections borrow optional shared fields instead of moving them, inherited
source-super constructor fields pass through the shared-handle assignment plan,
and reused erased `Object` values downcast through
`java_clone_erased_object_ref` instead of cloning `Box<dyn Any>`. The four
source-super/context fixtures now cargo-check:
`source_super_constructor_preserves_trailing_method_result_call`,
`source_super_constructor_matches_varargs_super_arity`,
`implicit_source_super_constructor_initializes_inherited_fields`, and
`inherited_context_trailing_call_uses_context_field_and_shared_varargs_plan`.

Codex follow-up 2026-09-12 erased object array clone pass: borrowed or
catchable `List<Object[]>.get(...)` results no longer recover ownership with
Rust `.to_vec()` / `.cloned()`, which requires `Box<dyn Any>: Clone`. Optional
erased object arrays now route through `java_clone_optional_erased_object_vec_ref`
in both the ordinary optional-wrap path and the fallible list-get projection.
`erased_object_array_clone_and_format_use_explicit_helpers` now cargo-checks
instead of pinning the old clone-trait failure.

Codex follow-up 2026-09-13 exception-supertype boundary pass: source exception
classes passed as `this` to a `Throwable` slot no longer emit the generic class
hierarchy `compile_error!`. Because modeled exception supertypes already lower
to the `JavaException` carrier, current-self exception upcasts now construct a
`JavaException` with the source class name and display message at that boundary.
Covered by
`tests/idiom_gaps.rs::exception_self_as_throwable_context_argument_uses_exception_carrier`
and
`tests/idiom_gaps.rs::explicit_cast_exception_self_as_throwable_context_argument_uses_exception_carrier`.

Fixture-count numbers below are chronological checkpoints, not live inventory.
For the current differential inventory, use
`tests/differential.rs::EXPECTED_DIFFERENTIAL_FIXTURES` rather than prose counts
recorded during earlier promotion passes.

Follow-up 2026-09-09 source-exception parent pass: generated `JavaException`
parent lookup now keeps exact package-qualified source exception edges before
simple-name compatibility. The source parent table emits full child/parent names
and only emits a simple child alias when that simple source exception name is
unambiguous, so `a.Problem` and `b.Problem` no longer collapse into one
`"Problem"` parent arm. Regression:
`exception_lowering::tests::java_exception_support_keeps_package_qualified_source_exception_parent_edges`.

Follow-up 2026-09-07 `List.subList` exception-class pass: catchable
`List.subList(from, to)` lowering now preserves Java's split exception contract.
`from > to` raises `IllegalArgumentException`, while negative/out-of-size bounds
continue to raise `IndexOutOfBoundsException`. Regression:
`idiom_gaps::list_sub_list_invalid_range_is_catchable_as_illegal_argument`.

Codex follow-up 2026-09-12 enhanced-for mutation guard, superseded later the
same day: enhanced-for over a local collection that is structurally mutated in
the loop body first became loud instead of emitting borrow-checker-invalid Rust
such as `for s in xs.iter() { xs.push(...) }`. The later focused fail-fast pass
below replaced that loud placeholder for the bounded local/result-producing
shape with a catchable `ConcurrentModificationException`.

Codex follow-up 2026-09-12 loop-body `try/finally` carrier guard, superseded
later the same day: after the known simple-loop fast paths declined a
`try/finally`, a loop-body shape whose body could `break` and whose `finally`
could conditionally return first emitted the existing unsupported
`try/catch/finally` placeholder instead of closure-crossing invalid Rust. The
later focused loop-finally pass below now lowers this bounded shape correctly.

Follow-up 2026-09-08 `BitSet.clear`/`flip` exception pass: the modeled
`java.util.BitSet` facade now carries mutating, result-returning ABI/effect
facts for `clear(int)` and `flip(int)`, and the runtime facade preserves Java's
negative-index `IndexOutOfBoundsException` contract. Differential fixture:
`tests/differential/BitSetClearFlipCatch.java` (`true,true:clear,flip`).

Follow-up 2026-09-09 `ArrayDeque` head-operation alias pass: zero-argument
`getFirst()` now lowers through the same catchable head-read path as
`element()`, while `removeFirst()` and `pop()` lower through the same catchable
head-removal path as `remove()`. The shared native collection method
classification now covers dispatch plus argument-shape and ABI argument-type
planning. Differential fixture:
`tests/differential/ArrayDequeFirstPopCatch.java` (`aab:caught`).

Follow-up 2026-09-09 `LinkedList` queue-head alias pass: the call planner now
treats `LinkedList` as a queue receiver in the same places as the stdlib/native
type facts, so `element()`, `remove()`, `getFirst()`, `removeFirst()`, and
`pop()` use the existing catchable head-read/head-removal projections.
Differential fixture: `tests/differential/LinkedListQueueHeadCatch.java`
(`true,true:aabb:caught`).

Follow-up 2026-09-10 focused oracle expansion: the cheap exception loop now
also covers the promoted comparator-throw, exception hierarchy/custom, and
null-argument catch fixtures. The cheap finally loop now also covers the
promoted catch/finally break-continue, return-override, labeled-continue,
ordinary finally path, and try/catch/finally completion fixtures. This is
coverage hygiene for already-promoted fixtures; the general completion carrier
remains open.

Follow-up 2026-09-07: the exception-support predicate is crate-local. Integration
tests now assert the public `plan_java_exception_support` result instead of
keeping `program_uses_java_exception_with_facts` exported as a standalone API.
The same pass removed two remaining lowering-side source exception probes:
fallible receiver projection and summary-only `super` fallback wrapping now ask
the solved method/callable exception plans instead of reading `method.throws` or
`facts.method_has_thrown_exceptions` directly.
`plan_call_exception_propagation_with_external_index` is also crate-local now:
the public boundary remains the no-index wrapper for tests and the
external-only propagation planner used by lowering.
Follow-up cleanup narrowed that external-only propagation planner as well:
`call_plan::plan_external_call_exception_propagation` is now crate-local. The
public exception-planning API remains `plan_call_exception_propagation`, while
lowering keeps using the no-source-probe external path internally after the
solved exception/fallibility plan has answered source-call questions. Guard:
`slot_shape_design::exception_lowering_policy_owns_result_support_ok_and_err_shapes`.
Follow-up cleanup 2026-09-12: `fallibility_invariant` is crate-internal too.
Lowering still runs the invariant reporter before emission, but the diagnostic
pass and `report_fallibility_violations` entry point are no longer advertised
as public crate API. Guard:
`layering::declaration_context_usage_lookups_use_java_callable_keys`.

Follow-up 2026-09-06: erased-object downcast failures inside propagating
`try/catch` lowering now become catchable `ClassCastException`s instead of
panicking through `.expect("invalid Java cast")`. The rewrite is limited to the
existing propagated runtime-expect pass: `downcast::<T>()` uses `map_err(...)?`,
and optional/reference downcast chains use `ok_or_else(...)?`. Regression:
`idiom_gaps::erased_object_class_cast_exception_is_catchable`.

Follow-up 2026-09-07 stale-baseline cleanup: the Java exception logging surface
now compiles without treating `System.err` as a value. `println` lowers to
`eprintln!` through the optional localized-message display path, and both
`printStackTrace` calls are omitted by the translation runtime marker. Removed
`idiom_gaps::java_exception_logging_surface_compiles_without_system_err_value`
from the failure baseline after focused generated-crate verification.
At that intermediate checkpoint, release `idiom_gaps` was `743/73`; later
2026-09-07 passes in this file supersede that baseline.

Follow-up 2026-09-06 stale-baseline pass: two try/finally rows were still
listed as idiom-gap failures after current generated code compiled. The
standalone `SimpleTryFinallyCleanup` generated test now passes the borrowed
`Resource` parameter the method actually expects, and the try-with-resources
assertion now checks the cleanup-result combiner rather than the older direct
`input.close()?` tail. Verified:
`idiom_gaps::simple_try_finally_cleanup_generated_rust_compiles` and
`idiom_gaps::translates_exception_propagating_try_with_resources_without_catch`.
The adjacent JAligner try/catch/finally row was then closed by making optional
erased-object assignment wrapping use the non-optional concrete RHS shape before
the erased-object element conversion. `Object sequence = null; sequence =
resource.read();` now stores `Some(Box::new(resource.read()?) as Box<dyn Any>)`
instead of boxing `Some(...)` as the erased object. Verified:
`idiom_gaps::translates_jaligner_try_catch_finally_cleanup_without_placeholder`.

Follow-up 2026-09-06: direct structural mutation of a freshly returned
`Collections.unmodifiableList`/`unmodifiableSet`/`unmodifiableMap` view, and
direct structural mutation of a fixed-size `Arrays.asList(...)` view, now lower
to `UnsupportedOperationException` instead of mutating the wrapped Rust
collection through the identity projection. The fix is intentionally limited to
the direct expression shape, for example
`Collections.unmodifiableList(values).add(value)` or
`Arrays.asList(value).add(other)`. `Arrays.asList(...).set(i, v)` is not
treated as unsupported, matching Java's fixed-size-list contract. Regressions:
`lower::tests::direct_unmodifiable_list_mutation_throws_unsupported_operation_exception`
and
`lower::tests::direct_arrays_as_list_add_throws_unsupported_operation_exception`.

Follow-up 2026-09-06: local aliases of those direct views now carry explicit
view provenance in `ShapeEnv`, keyed by binding id where available and cleared
on redeclaration. Structural mutation through an aliased
`Collections.unmodifiable*` or `Arrays.asList(...)` local now lowers to
`UnsupportedOperationException` too, without re-evaluating the original view
initializer arguments at the mutation site. Regressions:
`expr_shape::tests::bound_collection_view_facts_do_not_survive_same_name_redeclaration`,
`lower::tests::aliased_unmodifiable_list_mutation_throws_unsupported_operation_exception`,
`lower::tests::aliased_arrays_as_list_add_throws_unsupported_operation_exception`,
and
`lower::tests::collection_view_var_decl_facts_feed_later_mutation_lowering`.
Follow-up 2026-09-09: direct and aliased unmodifiable/fixed-size list mutation
is also pinned by the Java/Rust oracle, including the allowed
`Arrays.asList(...).set(i, value)` path, direct/aliased unmodifiable `clear()`,
unmodifiable `Collection.remove`, unmodifiable `Set.remove`, unmodifiable
`Map.put`, immutable factory structural mutations from `empty*`, `singleton*`,
and `nCopies`, and fixed-size `Arrays.asList(...).clear()`. The immutable
factory handling preserves Java's empty-view `clear()` no-op while still
throwing for actual structural changes. Differential fixture:
`tests/differential/UnmodifiableCollectionMutationCatch.java`
(`direct,collectionRemove,alias,aliasClear,setRemove,setClear,mapPut,mapClear,emptyList,emptySet,singleton,copies,fixed,clear:y:Y`).

Follow-up 2026-09-09 focused exception oracle expansion: the dedicated
`list_exception_fixtures_match_java_output` loop now includes already-promoted
exception fixtures for `Arrays.copyOf`, `BitSet` constructors/get/set,
iterators, map-entry iterators, `Math.toIntExact`, optional factories,
StringBuilder/StringBuffer bounds and capacity, string byte-range constructors,
StringTokenizer exhaustion, and TreeMap first/last-key emptiness. This is a
measurement-floor change only; the fixtures already existed in the broad
differential suite.
Follow-up 2026-09-09 second oracle expansion: the same focused loop now also
covers existing array checked read/write fixtures, catch hierarchy/order
fixtures, `Collections.nCopies`, nullable receiver/value exceptions, map-get
arithmetic null, `Math.floorDiv`/`floorMod`, queue empty head operations,
general `String` bounds, `StringBuilder.delete`/`insert`, and `Thread.sleep`
argument checks. Verified with
`cargo test --test differential list_exception_fixtures_match_java_output -- --nocapture`.

Follow-up 2026-09-10 focused exception/finally oracle expansion: the same
exception loop now also includes already-promoted parse-null/radix fixtures
(`IntegerParseNull`, `LongParseNull`, `NarrowParseNull`, `FloatingParseNull`,
and `IntegralParseRadix`) and map-view unsupported-operation fixtures
(`MapKeySetAddUnsupported`, `MapKeySetAddAllUnsupported`,
`MapValuesAddUnsupported`, `MapEntrySetAddUnsupported`, and
`MapEntrySetAddAllUnsupported`). The finally-focused loop now includes the
suppressed-exception try-with-resources fixtures
`TryWithResourcesMultipleCloseSuppressed` and
`TryWithResourcesSuppressedPrimary`. This is coverage hygiene for already
promoted fixtures; the general completion carrier remains open.

Follow-up 2026-09-09 negative array-size oracle: direct array construction with
a negative runtime length is now pinned by the Java/Rust oracle rather than only
by generated-crate assertions. `tests/differential/ArrayNegativeSizeCatch.java`
checks both the catchable `NegativeArraySizeException` path and the ordinary
positive-length path (`caught,len:2`).

Follow-up 2026-09-09 ragged negative array-size oracle: `new int[n][]` is now
also promoted from an inline generated-crate assertion to the Java/Rust oracle.
`tests/differential/RaggedArrayNegativeSizeCatch.java` checks the catchable
negative row-count path and the ordinary ragged row assignment/read path
(`caught,rows:2,cols:2`).

Follow-up 2026-09-09 Arrays range oracle: `Arrays.copyOfRange` and ranged
`Arrays.binarySearch` catchability are now pinned by Java/Rust differential
fixtures rather than only generated-crate assertions.
`tests/differential/ArraysCopyOfRangeCatch.java` checks order, bounds, and
ordinary padded-copy behavior (`order,bounds,ok:3:2:0`);
`tests/differential/ArraysBinarySearchRangeCatch.java` checks order, bounds,
and successful ranged search (`order,bounds,2`).

Follow-up 2026-09-09 Arrays compare/mismatch oracle: ranged `Arrays.equals`,
`Arrays.compare`, and `Arrays.mismatch` exception paths are now promoted from an
inline generated-crate assertion to `tests/differential/
ArraysCompareMismatchRangeCatch.java`. The fixture checks order errors, bounds
errors, and the ordinary successful range result
(`equals-order,equals-bounds,compare-order,compare-bounds,mismatch-order,mismatch-bounds,false:-1:1`).

Follow-up 2026-09-09 Arrays sort/fill range oracle: ranged `Arrays.sort` and
`Arrays.fill` catchability is now promoted from generated-crate compile checks
to `tests/differential/ArraysRangeCatch.java`, covering `fromIndex > toIndex`
and negative range bounds (`sort-order,fill-bounds`).

Follow-up 2026-09-09 Arrays stream range oracle/fix: ranged
`Arrays.stream(...).sum` catchability is now promoted from a generated-crate
compile check to `tests/differential/ArraysStreamRangeCatch.java`. The oracle
exposed that Java reports `fromIndex > toIndex` as
`ArrayIndexOutOfBoundsException` for `Arrays.stream`, unlike `Arrays.sort` and
`Arrays.fill`; stream range lowering now uses a stream-specific guard and the
fixture covers normal sum, reversed bounds, negative start, and end past length
(`5,-2,-2,-2`).

Follow-up 2026-08-30 layering cleanup: exception-support lowering no longer
imports or constructs `JavaType` while building source exception parent arms.
The Java-type-specific parent lookup is now hidden behind
`FactSet::extends_type_name_for`, keeping `exception_lowering` on facts plus
structured Rust IR rather than re-entering Java semantic types from the render
layer. The layering guard now checks the production prefix before test fixtures.
Regressions: `layering::exception_lowering_keeps_exception_paths_structured`,
the full `tests/layering.rs` suite, and the `semantic_facts` exception filter.
Companion unit fixtures now import `JavaType` inside the `#[cfg(test)]` module
only, preserving the production layering boundary while keeping
`cargo test --lib` compile-clean.

Follow-up 2026-08-30: `Arrays.stream(values, from, to)` range checks now use
the result-returning Java-exception carrier when the stream is lowered inside a
propagating `try/catch` context. The source-slice projection mirrors the
existing array range mutator split: ordinary value contexts keep the uncaught
panic wrapper, while propagated contexts return `Err(JavaException::new(...))`
for `fromIndex > toIndex` and out-of-bounds ranges, then apply `?` before
building the iterator. Regression:
`idiom_gaps::java_arrays_stream_range_in_try_lowers_to_java_exception_results`;
the existing structured stream-range test now pins both panic and result
rendering paths. The audit remains open for the broader panic-only exception
surface and general `finally` completion carrier.

Codex correction 2026-09-12: the stream-specific guard now matches the
documented oracle above for both panic and result carriers:
`fromIndex > toIndex` is `ArrayIndexOutOfBoundsException` for
`Arrays.stream(values, from, to)`, while the generic sort/fill range helpers
continue to use `IllegalArgumentException`. The focused generated-crate
regression and `tests/differential/ArraysStreamRangeCatch.java` now agree at
`5,-2,-2,-2`.

Follow-up 2026-08-29 baseline pass: six exception/finally fixtures were still
listed in `tests/baselines/idiom-gaps-failures.txt` even though current
lowering covers them. Two assertions were stale after newer result/cleanup
rendering: the plain fallthrough `try` value carrier now emits braced match
arms, and try-with-resources return lowering now uses the cleanup-result
combiner with suppression handling instead of the older direct `reader.close()?`
tail. Verified and removed from the baseline:
`corpus_genomicsdb_file_load_constructor_delegation_exposes_exception_type_gap`,
`external_standard_exception_super_with_trailing_body_lowers_subclass_state`,
`plain_fallthrough_try_with_value_returns_uses_optional_result_carrier`,
`simple_rethrow_catch_generated_rust_compiles`,
`translates_return_bearing_try_with_resources_without_placeholder`, and
`try_with_resources_external_type_gets_implicit_close_surface`. The audit
remains open for the general completion carrier and remaining panic-only
exception surfaces.

Follow-up 2026-08-29 baseline pass: three more 0057 fallibility rows were
verified passing and removed from `tests/baselines/idiom-gaps-failures.txt`.
Optional `PrintStream.println` through a throwing argument now keeps the arity
specific support method and catchable null propagation, overloaded static calls
carry exception facts on the emitted Rust name, and source-interface trait
surfaces use fallible implementation signatures. Verified:
`optional_print_stream_field_println_with_throwing_arg_keeps_arity_name`,
`overloaded_static_caller_gets_exception_fact_on_emitted_rust_name`, and
`source_interface_trait_uses_fallible_implementation_signature`. The audit
remains open for the general completion carrier and remaining panic-only
exception surfaces.

Follow-up 2026-08-28: `make_java_null_expect_catchable` now handles the raw
expression compatibility surface that was left behind after structured
expression and raw-statement rewrites. Parseable raw expressions are recovered,
then recursively rewritten, so embedded Java-null receiver unwraps can become
catchable `NullPointerException` propagation. Opaque raw text is preserved
through the centralized raw-leaf constructor instead of open-coded
`RustExpr::Raw` construction.
Regression:
`lower::tests::make_java_null_expect_catchable_recovers_parseable_raw_expression`.
Severity: **high** — no silent wrong answers in this group (that is 0056's
half). The original `StringBuilder.append(intLocal)` and several built-in JDK
exception cases are fixed; the active risk is the remaining panic-based
exception surface and the general/nested/labeled `finally` lowering model.
Kind: semantics / lowering.

Method and evidence discipline as 0056: `javac`/`java` oracle vs. compiled
translation, per `tests/differential.rs`. Repros in `repro/`.

Follow-up 2026-08-28: `Collections.sort(values, comparator)` can now carry
catchable comparator exceptions when the comparator is a throwing source method
reference or a fallible comparator object. The lowering detects method-reference
fallibility through the same callable result table used by ordinary calls,
routes fallible sort bodies through `java_collection_sort_by_result`, annotates
method-reference callback parameters from the target collection element type,
and emits the runtime helper for native comparator-sort sites after the facade
call is lowered away. `tests/differential/CollectionsSortComparatorThrowCatch.java`
pins the Java oracle, and
`slot_shape_design::collections_sort_throwing_method_reference_comparator_propagates_result`
checks the generated Rust compiles without falling back to `.cmp(&0)` on a
`Result`.

Follow-up 2026-08-27: one-dimensional array element writes in propagated
`try/catch` contexts now use a checked mutable slot instead of direct Rust
indexing. The lowered statement binds the mutable target and original Java
index, obtains the element with
`get_mut(__java_arrays_index as usize).ok_or_else(...)?`, then assigns through
that slot, so high and negative indexes cross the translated catch boundary as
`ArrayIndexOutOfBoundsException`. `tests/differential/ArrayWriteException.java`
pins both cases. Compound assignment expressions and the broader completion
carrier remain outside this narrow slice.

Follow-up 2026-08-26: the catchable `NullPointerException` rewrite now descends
into additional structured Rust IR positions: call function operands,
unsupported Java array-creation dimensions, and nested expressions inside
`expr?`. In a result-propagating context, a Java-null unwrap in those positions
is rewritten through `ok_or_else(...)?` instead of staying as
`.expect("Java null receiver")`; a direct null-expect under `expr?` collapses to
one propagated expression rather than double-`?`. Regressions:
`lower::tests::make_java_null_expect_catchable_recurses_into_call_function`,
`lower::tests::make_java_null_expect_catchable_recurses_into_unsupported_array_dimensions`,
`lower::tests::make_java_null_expect_catchable_recurses_inside_try_expression`,
and
`lower::tests::make_java_null_expect_catchable_collapses_direct_try_null_expect`.

Follow-up 2026-08-26: `StringBuilder.charAt`/`StringBuffer.charAt` now report
`StringIndexOutOfBoundsException`, matching Java's precise runtime class,
instead of the broader `IndexOutOfBoundsException`. The generated exception
hierarchy already treats it as a subtype of `IndexOutOfBoundsException`, so
existing broad catches still work while precise catches are now possible.
`tests/differential/StringBuilderCharAtCatch.java` now catches the precise class.

Follow-up 2026-08-26: a bounded `try-with-resources` + `catch` + fall-through
`finally` shape now lowers instead of being pre-marked unsupported. The
accepted slice is resource declarations only, no loop-abrupt exits in the
resource body/catches/finally, and a fall-through `finally`. The implementation
composes the existing resource-cleanup scope inside the existing catch/finally
scope, and the duplicated unsupported-statement gate was updated to recognize
the new `WithResourcesCatchFinally` plan. `TryWithResourcesCatchFinally.java`
pins Java's ordering as `trcfd`, with planner and generated-crate compile
coverage.

Follow-up 2026-08-26: `BitSet.get(-1)` is now catchable. The support facade
keeps ordinary `get` as the uncaught wrapper, adds `get_result` for propagated
contexts, and advertises `IndexOutOfBoundsException` in the JDK effect facts.
`tests/differential/BitSetGetCatch.java` covers negative-index catchability
while preserving the ordinary missing-bit `false` contract.

Follow-up 2026-08-26: the simple-loop `try/finally` fast-path detector now
scans nested `try` catch and `finally` blocks for loop-abrupt exits instead of
only checking the nested try body. This closes a narrow detection hole where a
`continue`/`break` in an inner `finally` or catch could escape the pre-finally
rewrite. Regression:
`lower::tests::loop_abrupt_detection_scans_try_catches_and_finally`.

Follow-up 2026-08-26: `Queue.element()` is now planned separately from the
zero-argument collection fallback and uses the same `NoSuchElementException`
carrier as queue-head `remove`. Ordinary value contexts unwrap through the
uncaught Java-exception panic helper; propagated contexts return
`ok_or_else(...)?` so translated `catch (NoSuchElementException e)` can observe
the exception. `tests/differential/QueueElementCatch.java` covers the non-empty
peek contract and the empty-queue catch path.

Follow-up 2026-08-26: `Collections.nCopies(-1, value)` now raises
`IllegalArgumentException` instead of casting `-1` to a huge `usize` in
`vec![value; count as usize]`. The ordinary projection unwraps a shared
result-returning helper through the uncaught Java-exception adapter; propagated
contexts emit `...?` so Java `catch (IllegalArgumentException e)` can observe
the failure. `tests/differential/CollectionsNCopiesCatch.java` covers the
positive size contract and the negative-count catch path.

Updated 2026-08-24: two small `String` compile breaks are fixed and promoted.
`String.trim()` now lowers as an owned `String` result, so `return s.trim();`
compiles and matches Java. `String.replace(char, char)` now adapts the
replacement char literal to Rust's `&str` replacement argument. `TrimInReturn.java`
and `StringReplaceChar.java` passed the Java oracle at that checkpoint; the
current fixture count is owned by `tests/differential.rs`.

Updated 2026-08-24 again: `StringBuilder.append(intLocal)` now keeps primitive
locals in their copy-value shape. The local-signature shortcut that selects a
concrete borrowed value is no longer eligible for primitive/copy types, so the
conflict winner (`CopyValue`/native) agrees with the rendered local type.
`AppendIntLocal.java` was promoted and passed the Java oracle at that
checkpoint; the current fixture count is owned by `tests/differential.rs`.

Updated 2026-08-24 third pass: `Character.isDigit`, `isLetter`,
`isWhitespace`, `toUpperCase`, and `toLowerCase` are planned as native Java
static calls. Lowering now emits the corresponding Rust `char` methods, and
`CharacterStatics.java` was promoted at that checkpoint; the current fixture
count is owned by `tests/differential.rs`.

Updated 2026-08-24 fourth pass: `new String(String)` no longer resolves to the
byte-array constructor. The new-target plan records a native String constructor
variant from source argument types, constructor argument lowering keeps the
source `String` as a String slot, and `NewStringCtor.java` is promoted into the
50-fixture differential suite. `StringEqualsVsEq.java` is not an exception or
constructor gap; its `String ==` behaviour is the documented String-identity
policy boundary in
[0028](done/0028-reference-equality-lowered-as-structural-equality.md) and is
covered as an executable known gap in `tests/differential/known-gaps/`.

Updated 2026-08-24 fifth pass: `Integer.parseInt` in a fallible context now
lowers to `parse::<i32>().map_err(|err| JavaException::from_display(
"NumberFormatException", err))?` instead of `.unwrap()`. The generated
`JavaException::is_instance` also follows the built-in unchecked exception
parent chain, so `catch (IllegalArgumentException)` catches
`NumberFormatException`. `ExceptionHierarchy.java` is promoted into the
51-fixture differential suite.

Updated 2026-08-24 sixth pass: array reads in fallible lowering contexts now use
checked `.get(index).ok_or_else(|| JavaException::new(
"ArrayIndexOutOfBoundsException", ...))?` instead of direct Rust indexing.
Primitive element reads dereference the checked borrow. `ArrayIndexException.java`
is promoted into the 52-fixture differential suite.

Updated 2026-08-24 seventh pass: this audit's remaining active scope was
re-triaged against the current tree. `StringBuilder.append(intLocal)`,
`String.trim`, `String.replace(char,char)`, `new String(String)`, selected
`Character.*` statics, `Integer.parseInt` in fallible contexts, checked array
reads in fallible contexts, and simple loop `finally` break/continue cases are
no longer the live failures. Keep the audit open for general/nested/labeled
`finally` completion semantics and panic-based Java exceptions that still do not
cross catch boundaries as `JavaException`.

Updated 2026-08-24 eighth pass: `NullInstanceof.java` moved from repro-only
coverage into `tests/differential/`. The differential suite gated
`null instanceof String == false` and a non-null `String` object matching the same
check at that checkpoint, rather than relying on this file's prose inventory.

Updated 2026-08-24 ninth pass: three more verified-correct control/null shapes
are promoted from repro-only coverage: `DoWhile.java`, `ShortCircuitLocal.java`,
and `TernaryNull.java`. At that checkpoint the differential suite gated
do/while execution, observable `&&`/`||` short-circuiting, null-guarded member
access, ternary null propagation, and guarded division.

Updated 2026-08-24 tenth pass: four verified-correct char/string fixtures are
promoted from repro-only coverage: `CharOps.java`, `CharArray.java`,
`StringMethods.java`, and `StringSplit.java`. At that checkpoint the
differential suite gated 66 fixtures, including char arithmetic/casts/comparison,
`toCharArray`, char compound assignment/increment, `String.valueOf(char)`,
string search and case methods, and regex-style `String.split`.

Updated 2026-08-24 eleventh pass: three verified-correct control-flow fixtures
are promoted from repro-only coverage: `EnhancedForArray.java`,
`EnhancedForCollection.java`, and `LabeledBreakOnly.java`. At that checkpoint
the differential suite gated 69 fixtures, including enhanced-for over primitive
arrays, reference arrays and `List<String>`, plus a labeled break out of nested
loops.

Updated 2026-08-24 twelfth pass: four verified-correct exception fixtures are
promoted from repro-only coverage: `ExceptionsCustom.java`,
`CatchSupertype.java`, `CatchException.java`, and
`TryCatchReturnFinally.java`. The differential suite gated these cases at that
checkpoint,
including user exception construction/message flow, catching a user subclass
through `RuntimeException`, catching a built-in unchecked exception through
`Exception`, exception crossing a call boundary, and `finally` after return in
both `try` and `catch`.

Updated 2026-08-24 thirteenth pass: `SwitchChar.java` is promoted from
repro-only coverage. The differential suite gated, at that checkpoint,
`switch` on `char`, `switch` on `String`, grouped switch labels, and
fallthrough.

Updated 2026-08-24 fourteenth pass: the remaining repro-only entries in this
file's "Correct — do not spend time here" table were stale. `FinallyPaths.java`
and `FinallyOnBreak.java` were already in `tests/differential/`; the table now
points at the maintained oracle fixtures for every row in that section.

Updated 2026-08-24 fifteenth pass: nullable receiver dereference in a fallible
context now raises catchable `JavaException::new("NullPointerException", ...)`
instead of panicking through `unwrap`/`expect`. `tests/differential/NullDeref.java`
pins the Java oracle (`npe`) and moves this specific NPE receiver path out of
the active panic surface. Non-receiver null coercions and broader `finally`
completion semantics remain open.

Updated 2026-08-24 sixteenth pass: nullable reference-parameter receiver
dereferences now seed `NullPointerException` fallibility when not locally
caught. This lets a caller's translated `try/catch` catch a null dereference
that happens inside a source callee instead of seeing a Rust `.unwrap()` panic.
`tests/differential/NullArgCatch.java` is promoted and matches Java's `npe`.
General/nested/labeled `finally` and the remaining panic-based Java exception
surfaces remain open.

Updated 2026-08-24 seventeenth pass: non-receiver argument-slot null coercions
now use the catchable NPE carrier in propagated blocks. `NullCallArgCatch.java`
passes a nullable source-call result through a required `String` argument slot
inside `try/catch`; the generated Rust now uses `ok_or_else(...)?` instead of
`.expect("Java null receiver")`, and the fixture matches Java's `npe`.
General/nested/labeled `finally` and other panic-based Java exception surfaces
remain open.

Updated 2026-08-24 eighteenth pass: the simple loop-body `try/finally` fast
path now handles labeled loop exits as well as unlabeled exits. A labeled
`continue outer` inside an inner-loop `try` body now gets the `finally` body
inserted before the continue, then the existing classic-for labeled-continue
rewrite inserts the target loop's update at the outer-loop level. `tests/
differential/FinallyLabeledContinue.java` is promoted and matches Java
(`00f01f02ff20f21f22f`). The general nested/labeled completion carrier remains
open; this only covers the already-special-cased no-catch/no-resource loop-body
shape with a fall-through `finally`.

Updated 2026-08-25: `java.util.Random.nextInt(int)` with an invalid bound no
longer panics past a Java catch boundary. The built-in facade facts now mark
`nextInt`/`next_int` as `Result`-wrapped and throwing
`IllegalArgumentException`; generated `Random::next_int_result` returns
`Err(JavaException::new("IllegalArgumentException", ...))` when `bound <= 0`;
and generic expression statements now apply the same result-propagation policy
as expression slots, so a side-effect-only result call inside `try` emits `?`
instead of discarding the `Result`. `tests/differential/RandomNextIntCatch.java`
is promoted and the 121-fixture differential oracle matches Java.

Updated 2026-08-24 nineteenth pass: the catchable Java-null expectation rewrite
now recurses through structured `match` arm bodies and block statements instead
of only the match scrutinee and block tail. This closes a narrow panic leak in
the existing argument/value coercion mitigation; the general exception and
completion-carrier work remains open.

Updated 2026-08-24 twentieth pass: plain and infinite `while` loop bodies now
use the same body-aware lowering path as classic `for` loop bodies, so the
simple no-catch/no-resource `try/finally` fast path runs for `continue`/`break`
inside a `while`. `tests/differential/FinallyWhileContinue.java` pins the
previously missed `continue` shape; the focused translated output matches
Java's `1ff3f`. General nested/labeled completion semantics remain open.

Updated 2026-08-24 twenty-first pass, extended 2026-08-25: nullable integral
parse arguments now preserve Java's odd null contract. `Byte.parseByte(null)`,
`Short.parseShort(null)`, `Integer.parseInt(null)`, and `Long.parseLong(null)`
throw `NumberFormatException`, not `NullPointerException`, so the integral parse
lowering maps the existing nullable-string unwrap projection to `"null"` before
parsing.
`tests/differential/IntegerParseNull.java` pins `nfe,nfe,ok` against Java, and
`tests/differential/LongParseNull.java` pins `nfe,nfe,ok:9223372036854775807`;
`tests/differential/NarrowParseNull.java` pins the same contract for `byte` and
`short`.
The same pass now covers the radix overloads for `Byte`, `Short`, `Integer`,
and `Long`: `tests/differential/IntegralParseRadix.java` proves that the radix
argument is not silently ignored, null still maps to `NumberFormatException`,
and invalid radix values become catchable `NumberFormatException` instead of
Rust's internal `from_str_radix` panic.
`Float.parseFloat(null)` and `Double.parseDouble(null)` still follow their Java
`NullPointerException` behavior.

Updated 2026-08-25 parse receiver cleanup: the rendered parse receiver recovery
path now recognizes Java-null receiver unwrap spellings with method arguments,
not only dotted zero-argument chains. Raw
`.expect("Java null receiver")` and
`.ok_or_else(|| JavaException::new("NullPointerException", ...))?` receivers are
rebuilt into structured Rust IR first, then the existing redundant-expect
stripper decides whether an extra unwrap/expect layer is removable. This keeps
the primitive parse family aligned with the broader catchable-null rewrite
without adding another rendered parse-call spelling.

Updated 2026-08-24 twenty-second pass: string bounds failures now cross
translated `try/catch` as `StringIndexOutOfBoundsException`. In propagated
contexts, `String.charAt` uses `.ok_or_else(...)?` instead of `.expect(...)`,
and `String.substring` calls a `java_string_substring_result` helper instead of
the old clamping helper. The non-fallible substring helper now panics through an
uncaught `JavaException` on invalid bounds rather than silently returning the
wrong slice. `tests/differential/StringBoundsException.java` pins Java's
`char,sub` result.

Updated 2026-08-24 twenty-third pass: the floating parse-null row is now gated.
`Float.parseFloat(null)` and `Double.parseDouble(null)` throw
`NullPointerException`, while bad non-null input still throws
`NumberFormatException`; `tests/differential/FloatingParseNull.java` pins
`npe,nfe,ok;npe,nfe,ok` against Java. The same differential run exposed a
regression in string concatenation of fallible source `.equals()` results:
custom `.equals()` lowers as `Result<bool, JavaException>`, so concat segments
must apply `?` only for that actual source-call shape, not for every expression
whose subtree can throw. `tests/differential/IdentityCustomEquals.java` now
compiles and matches Java again. The 92-fixture differential suite passes.

Updated 2026-08-25: the catchable Java-null expectation rewrite now recurses
through structured closure bodies, not just match arms and block statements.
This closes another narrow panic leak where a generated closure could keep
`.expect("Java null receiver")` inside a propagated exception context. Covered
by `lower::tests::make_java_null_expect_catchable_recurses_into_closure_bodies`.

Updated 2026-08-25 again: escaping Java `throw` statements now seed callable
fallibility even for unchecked exception classes. This lets direct
`UnsupportedOperationException`, `IllegalArgumentException`, and known nested
unchecked exceptions cross source method boundaries as `Result<_, JavaException>`
instead of lowering to an immediate Rust panic. Local catches still suppress the
fallibility seed for the guarded body, and lambda/anonymous-class declaration
bodies are not charged to the enclosing expression. Covered by the renamed
focused idiom tests
`unchecked_direct_throw_in_plain_method_returns_result`,
`nested_unchecked_exception_throw_forces_result_return_shell`, and
`caught_checked_exception_rethrown_as_known_unchecked_propagates_result`.

Updated 2026-08-25 indexed `List.remove`: `List.remove(int)` is now planned as
a separate native collection method from `List.remove(Object)`, using the actual
argument expression type to disambiguate the Java overload. In propagated
contexts it checks negative and high indexes before calling `Vec::remove`, so
`IndexOutOfBoundsException` crosses translated `try/catch` as `JavaException`
instead of a Rust panic. `tests/differential/ListRemoveIndexCatch.java` pins
successful removal, high index, and negative index against Java.

Updated 2026-08-26 bounded follow-up triage, superseded later the same day:
simple loop-body `try/finally` break/continue paths were already covered, and
the then-open `try/catch/finally` catch-body loop exits were later fixed by the
direct catch-arm loop-body lowering recorded near the end of this file.
`tests/differential/CatchFinallyBreak.java`, `CatchFinallyContinue.java`, and
`CatchFinallyLabeledContinue.java` now verify those Java oracles. Do not spend a
new pass on this exact E0267 claim unless one of those fixtures regresses.

---

## Summary of areas probed

### Correct — do not spend time here

Every one of these was verified byte-for-byte against the Java oracle.

| area | fixture |
|---|---|
| `char` arithmetic (`c - 32`, `c + 1`), `char`↔`int`, `char` comparison | `tests/differential/CharOps.java` |
| `charAt`, `charAt` in an accumulating loop, `s.length()` | `tests/differential/CharOps.java` |
| `switch` on `char`, on `String`, empty-case grouping, fallthrough | `tests/differential/SwitchChar.java` |
| `toCharArray`, `char +=`, `char++`, `String.valueOf(char)` | `tests/differential/CharArray.java` |
| `substring(a)`, `substring(a,b)`, `indexOf(String)`, `indexOf(char)`, `lastIndexOf`, `replace(String,String)`, `toUpperCase`, `toLowerCase`, `equals`, `startsWith`, `endsWith`, `compareTo`, `contains`, `length`, `isEmpty` | `tests/differential/StringMethods.java` |
| `split(",")` incl. empty fields, `split("\\s+")` | `tests/differential/StringSplit.java` |
| `do`/`while`, incl. a body that runs once with a false guard | `tests/differential/DoWhile.java` |
| enhanced-`for` over `int[]` and `String[]`, with `break` and `continue` | `tests/differential/EnhancedForArray.java` |
| enhanced-`for` over `List<String>`, with `continue` | `tests/differential/EnhancedForCollection.java` |
| labelled `break`/`continue` out of a nested loop | `tests/differential/LabeledBreakOnly.java` |
| `&&`/`||` short-circuit with observable side effects | `tests/differential/ShortCircuitLocal.java` |
| ternary short-circuit (guarded division, guarded index) | `tests/differential/TernaryNull.java` |
| custom exception class extending `RuntimeException`, `throw`, `getMessage()` | `tests/differential/ExceptionsCustom.java` |
| catching a user subclass via `catch (RuntimeException e)` | `tests/differential/CatchSupertype.java` |
| catching via `catch (Exception e)` | `tests/differential/CatchException.java` |
| exception crossing a call boundary | `tests/differential/ExceptionsCustom.java` |
| catch ordering across exact, widened, array-index, arithmetic, and no-exception paths | `tests/differential/CatchOrder.java` |
| `try`/`catch`/`finally` with `return` in **both** `try` and `catch` | `tests/differential/TryCatchReturnFinally.java` |
| nested `try`/`finally` statement body, return-value finally, uncaught finally then outer catch | `tests/differential/FinallyPaths.java` |
| simple `break`, `continue`, and labeled `continue` out of `try`/`finally` inside a `for` body | `tests/differential/FinallyOnBreak.java`, `tests/differential/FinallyOnContinue.java`, `tests/differential/FinallyLabeledContinue.java` |
| `ArithmeticException` on `/ 0` (a real `JavaException`) | `tests/differential/Exceptions.java` |
| `instanceof` on `null` | `tests/differential/NullInstanceof.java` |
| `Integer.parseInt(null)` throws `NumberFormatException`, not `NullPointerException` | `tests/differential/IntegerParseNull.java` |
| `Long.parseLong(null)` throws `NumberFormatException`, not `NullPointerException` | `tests/differential/LongParseNull.java` |
| `Byte.parseByte(null)` / `Short.parseShort(null)` throw `NumberFormatException`, not `NullPointerException` | `tests/differential/NarrowParseNull.java` |
| `Float.parseFloat(null)` / `Double.parseDouble(null)` throw `NullPointerException`, not `NumberFormatException` | `tests/differential/FloatingParseNull.java` |
| `String.charAt`/`substring` invalid bounds throw `StringIndexOutOfBoundsException` | `tests/differential/StringBoundsException.java` |

The remaining divergent `String ==` policy case is tracked by
[0028](done/0028-reference-equality-lowered-as-structural-equality.md), not this
exception/finally audit. It is still executable coverage via
`tests/differential/known-gaps/StringEqualsVsEq.java`, where the Java oracle and
current `java_string_identity` warning are both checked.

Historical checkpoint: **30 areas probed for this audit's exception/control-flow
surface, 29 verified correct.** This count predates the later promoted
exception/finally differential loops and is no longer the current fixture
total; use the active fixture lists in `tests/differential.rs` and the README's
current suite count for present coverage.

---

## §1 — `StringBuilder.append(intLocal)` types the *local* as `&i32`

The smallest program that shows it, `repro/AppendIntLocal.java`:

```java
public class AppendIntLocal {
    public static String run() {
        StringBuilder out = new StringBuilder();
        int i = 7;
        out.append(i);
        return out.toString();
    }
}
```

Java: `7`. Emitted:

```rust
let mut out: StringBuilder = StringBuilder::new_0();
let i: &i32 = 7;        // E0308: expected `&i32`, found integer
out.append(i);          // E0277: the trait bound `&i32: JavaAppendValue` is not satisfied
return out.to_string();
```

The `&` migrated from the *call site* to the *binding*. For a `String`
argument the same machinery correctly emits `out.append(&"x".to_string())` with
the local left as `String`; for a `Copy` primitive it instead rewrites the
local's declared type and leaves the call bare, so both ends are wrong.

The translator sees the conflict and reports resolving it the right way — but
then emits the rejected shape:

```
Warning signature_constraint_conflict: AppendIntLocal::run local i has conflicting Owner
constraints; kept RequiredOwnership(CopyValue), rejected RequiredOwnership(SharedBorrow);
Owner join kept RequiredOwnership(CopyValue) because it requires copy value
```

This is now fixed and covered by `tests/differential/AppendIntLocal.java`.
`kept CopyValue` and the rendered type now agree:

```rust
let i: i32 = 7;
out.append(i);
```

The `SharedBorrow`/`Str` constraint arrives from
`src/stdlib.rs:13379 display_sink_method`, which lists `append` (with `write`,
`print`, `println`, …) as a display sink and hints its argument toward a
borrowed string view. That hint is right for `String` and wrong for every
primitive — even though the generated `StringBuilder::append` is
`pub fn append<T: JavaAppendValue>(&self, value: T)` and `JavaAppendValue` is
implemented for `bool`, `char`, `i8`, `i16`, `i32`, `i64`, … by value.

**Blast radius.** `for (int i = …) sb.append(i)` is how a large fraction of Java
builds a string. Every nested-loop-plus-`StringBuilder` program in the probe set
failed on this and nothing else. It is also why `tests/differential/`'s own
`ControlFlow.java` survives: it appends only `String`s.

The display-sink hint still reaches the constraint set, so the diagnostic can
report the rejected borrowed view. The local-shape shortcut no longer overrides
the normal copy-value result for primitive/copy Java types.

---

## §2 — `try`/`finally` is lowered to a closure, which breaks two ways

### 2a. a `void` try body got the enclosing method's return type — fixed

`repro/FinallyPaths.java`:

```java
static String uncaughtWithFinally() {
    StringBuilder sb = new StringBuilder();
    try {
        try { sb.append("t"); throwing(); }
        finally { sb.append("f"); }
    } catch (RuntimeException e) { sb.append("outer"); }
    return sb.toString();
}
```

Java: `tfouter`. Previously emitted (line broken for readability):

```rust
{ let __java_try_result = (|| -> Result<String, JavaException> {
        sb.append(&"t".to_string()); Self::throwing()?; return Ok(());  // E0308: Ok(()) is not Ok(String)
    })();
  sb.append(&"f".to_string());
  __java_try_result?; };
```

The carrier closure is annotated `Result<String, JavaException>` — the *enclosing
function's* return type — while the try block is a statement block whose value is
`()`. The sibling `finally_on_throw` in the same file gets it right
(`Result<(), JavaException>`), so the type is picked from the wrong scope only
when the `try`/`finally` is nested inside another `try`.

Fixed 2026-08-24. `TryStmtPlan::FinallyOnly` now mirrors the existing
`CatchFinally` rule and uses `Result<(), JavaException>` when the guarded body
falls through as a statement. `tests/differential/FinallyPaths.java` pins all
three paths and prints `tcfa,t,tfouter` on both Java and Rust.

### 2b. simple `break`/`continue` no longer cross the closure boundary

`repro/FinallyOnBreak.java` — six lines, entirely ordinary:

```java
for (int i = 0; i < 3; i++) {
    try { if (i == 1) break; sb.append("b"); }
    finally { sb.append("f"); }
}
```

Java: `bff`. Previously emitted:

```rust
while i < 3 {
    { let __java_try_result = (|| -> Result<String, JavaException> {
            if i == 1 { break; }              // E0267: cannot `break` inside of a closure
            sb.append(&"b".to_string()); return Ok(());
        })();
      sb.append(&"f".to_string());
      if let Err(ex) = __java_try_result { panic!("uncaught Java exception: {:?}", ex); } };
    …
}
```

JLS 14.20.2 makes `finally` run on *every* abrupt completion of the block —
`throw`, `return`, `break`, `continue`. The closure carrier can only model
`throw` (via `Result`) and `return` (via `return Ok(..)`); `break` and `continue`
have no encoding at all and are emitted verbatim into a closure body, which is a
hard `rustc` error.

Fixed for the simple loop-body shape 2026-08-24. A `try/finally` with no
resources/catches and a fall-through `finally` in a `for` body is lowered without
the `Result` closure when the guarded body contains a loop exit. The lowering
inserts the `finally` body before `break`; for an unlabeled `continue`, it
inserts `finally`, then the current `for` update expressions, then `continue`.
For a labeled `continue`, it inserts only `finally` at the inner site and lets
the existing labeled-continue rewrite place the target loop's update at the
target loop level. The break fixture prints `bff`;
`tests/differential/FinallyOnContinue.java` pins the sibling continue ordering,
and `tests/differential/FinallyLabeledContinue.java` pins the labeled
outer-continue ordering.

Note also the `panic!("uncaught Java exception: …")` in the non-fallible variant:
a `finally` inside a method the analysis decided cannot throw will abort rather
than propagate. Same class of problem as §3.

**Remaining design note.** The closure carrier is still the wrong general shape
for arbitrary `finally`. A labelled block plus an explicit outcome enum (`Normal
| Break(label) | Continue(label) | Return(v) | Throw(e)`), or a scope-guard
type, is still the full answer for nested loops, labelled exits and exception
paths. The checked-in fix deliberately covers the common simple loop-exit shape
without claiming that full carrier.

---

## §3 — four JDK exceptions are process aborts, not `JavaException`s

`ArithmeticException` is correct (`tests/differential/Exceptions.java` covers
it, and the lowering emits a real `Err(JavaException::new("ArithmeticException",
"/ by zero"))`). The rest are not.

### `ArrayIndexOutOfBoundsException` — `repro/ArrayIndexException.java`

```java
int[] a = new int[2];
try { return "got:" + a[5]; }
catch (ArrayIndexOutOfBoundsException e) { return "aioobe"; }
```

Java: `aioobe`. Fixed translation:

```rust
*a.get(5 as usize).ok_or_else(|| JavaException::new(
    "ArrayIndexOutOfBoundsException",
    "Java index out of bounds".to_string(),
))?
```

The checked read is used only in fallible lowering contexts, so ordinary reads
keep direct indexing while reads inside `try` can be caught. Covered by
`tests/differential/ArrayIndexException.java`.

### `NumberFormatException` — `repro/ExceptionHierarchy.java`

```java
try { return "v:" + Integer.parseInt(s); }
catch (IllegalArgumentException e) { return "iae"; }
```

Java: `v:12,iae`. Fixed translation:

```rust
return Ok(format!("v:{}", s.parse::<i32>()
    .map_err(|err| JavaException::from_display("NumberFormatException", err))?));
```

The native parse arm now emits a `JavaException` in fallible lowering contexts,
and generated `JavaException::is_instance` follows the built-in unchecked
exception parent chain. `tests/differential/ExceptionHierarchy.java` covers
`NumberFormatException <: IllegalArgumentException`.

### `NullPointerException` — see 0056 §4.

`tests/differential/CatchOrder.java` now covers the combined catch-order case:
an exact `IllegalArgumentException` catch, a widened `RuntimeException` catch
for `IllegalStateException`, a catchable array-index exception, a catchable
arithmetic exception, and the no-exception path. The array-index branch now
formats the Java-style `Index {index} out of bounds for length {len}` message
instead of the old placeholder.

One incidental observation from that file: the class-name set for
`catch (RuntimeException e)` is a fixed enumeration that includes project
specific names —

```rust
Err(e) if e.is_instance(&["IllegalArgumentException", "IllegalStateException",
    "InternalCodecException", "RuntimeException", "SAMException", "VariantContextException",
    "htsjdk.samtools.SAMException", "htsjdk.tribble.TribbleException.InternalCodecException",
    "htsjdk.variant.variantcontext.VariantContextException", "java.lang.RuntimeException"])
```

It happened to include the user's own subclass in `tests/differential/CatchSupertype.java`, so
that case passes; but a hand-enumerated subtype closure is a latent
false-negative for any `RuntimeException` subclass the enumerator misses.

### `Character.*` — `repro/CharacterStatics.java`

```java
return Character.isDigit('7') + "," + Character.isLetter('a') + ","
     + Character.toUpperCase('q') + "," + Character.toLowerCase('Q') + ","
     + Character.isWhitespace(' ');
```

Java: `true,true,Q,q,true`. Fixed: the five calls now lower through
`NativeJavaStaticMethod` to `c.is_ascii_digit()`, `c.is_alphabetic()`,
`c.is_whitespace()`, `c.to_ascii_uppercase()`, and `c.to_ascii_lowercase()`.
`tests/differential/CharacterStatics.java` covers the oracle result.

---

## §4 — four small compile breaks in the everyday String path

### 4a. `return s.trim();` — `repro/TrimInReturn.java`

```rust
pub fn run() -> String {
    let s: String = "  padded  ".to_string();
    return s.trim().to_string();
}
```

This is now fixed and covered by `tests/differential/TrimInReturn.java`.
`trim()` is treated as Java's owned `String` result rather than a borrowed Rust
view, so return position no longer needs a special coercion.

### 4b. `s.replace(char, char)` — `repro/StringReplaceChar.java`

```rust
return t.replace(',', "-");
```

Rust's `str::replace<P: Pattern>(&self, from: P, to: &str)` takes a pattern on
the left but a `&str` on the right. The `(String, String)` overload is handled
correctly (`repro/StringMethods.java`); the `(char, char)` literal overload is
now fixed and covered by `tests/differential/StringReplaceChar.java`.

### 4c. `new String("x")` constructor fixed; String identity belongs to 0028 — `repro/NewStringCtor.java`, `repro/StringEqualsVsEq.java`

```java
String a = new String("hello");
return a + "," + a.length();
```

```rust
let a: String = "hello".to_string().clone();
return format!("{},{}", a, (a.len() as i32).to_string());
```

The constructor half is fixed and covered by
`tests/differential/NewStringCtor.java`: `new String(String)` is selected from
the source argument type and lowers as a String clone instead of the byte-array
constructor.

`repro/StringEqualsVsEq.java` remains outside this audit. Java `String`
reference `==` now lowers to a loud `compile_error!` instead of Rust value
equality; the `java_string_identity` diagnostic marks that 0028 scope boundary.

### 4d. `static StringBuilder` field — `repro/StaticStringBuilder.java`

```java
static StringBuilder log = new StringBuilder();
```

Fixed 2026-08-24. `StringBuilder`/`StringBuffer` support now uses
`Arc<Mutex<String>>`, which is valid behind the global static carrier, and the
static getter installs the lazy initializer with `get_or_insert_with` before
cloning it. The generated repro compiles and prints `ab`, and
`tests/differential/StaticStringBuilder.java` pinned it in the differential
suite at that checkpoint.

---

## Why nothing noticed

`tests/differential/Exceptions.java` covers exactly one throw path
(`ArithmeticException`) and one `finally` (on the `return` path of a method
whose try body is an expression). It contains no `throw`, no custom exception
class, no `break` in a `try`, and no `void` try body — so every shape in §2 and
§3 is outside it. 0052 already recorded that the corpus has zero uses of
`throw`, `char`, and String instance methods; §1 and §4 are the price of that.

§1 in particular is invisible to every existing signal: the translator *emits a
warning* naming the exact local, and the warning says it resolved the conflict
correctly. Nothing compares the warning's stated resolution to the type actually
rendered.

---

## Direction

Updated 2026-08-25: ordinary no-catch/no-resource `try`/`finally` now accepts a
terminating `finally` block. Because the lowerer emits the finally body before
replaying the saved try result, `finally { return ... }` correctly overrides a
pending `try` return. `tests/differential/FinallyReturnOverride.java` gates this
shape (`finally`).

Updated 2026-08-25 again: the same override is now accepted for the narrow
`try`/`catch`/`finally` shape where the guarded body and all catch arms already
terminate with value/exception boundaries. The existing catch+finally lowering
saves the try/catch result, emits the finally body, and then replays the saved
result; when `finally { return ... }` terminates, normal Rust control flow skips
the replay just like Java. `tests/differential/CatchFinallyReturnOverride.java`
gates both the throwing and non-throwing paths (`finally,finally`). This does
not close the broader nested/labeled completion-carrier work.

1. Replace the general `try`/`finally` closure carrier with a construct that can
   encode all Java abrupt completions: `return`, `throw`, plain and labeled
   `break`, and plain and labeled `continue`. Simple loop `break`/`continue`
   cases and one labeled outer-`continue` loop-body shape are now gated, but
   nested/labeled completion remains this audit's main open design point.
2. Stop representing Java exceptions as Rust panics outside the few checked
   lowerings already converted to `JavaException`. Built-in exceptions that can
   cross call boundaries still need the same catchable carrier as arithmetic,
   parse, and fallible array-read paths.
3. Keep promoting repro-only "correct" cases into `tests/differential/` one at a
   time. At this checkpoint the suite gated 108 fixtures, including
   `IntegralParseRadix.java`, `FinallyReturnOverride.java`, and
   `CatchFinallyReturnOverride.java`; later follow-ups in this file supersede
   that count. The repro inventory remains useful, but it should not be treated
   as a gate until each file is promoted.

## Codex parallel audit 2026-08-25

The remaining complaint is still valid, but it is now narrower than the original
file. Catchable array bounds, parse exceptions, null receiver/argument
coercions, string bounds, catch hierarchy, and several simple `finally` paths
are covered by promoted differential fixtures (`CatchOrder`,
`ArrayIndexException`, `StringBoundsException`, `FinallyPaths`,
`FinallyOnBreak`, `FinallyOnContinue`, `FinallyLabeledContinue`,
`FinallyWhileContinue`, `TryCatchReturnFinally`, `FinallyReturnOverride`, and
`CatchFinallyReturnOverride` among them). A follow-up narrowed the classic-for
fast path again: `finally` bodies that themselves perform a simple unlabeled
`continue` now lower through the loop-body updater, so Java's for-update runs
before the `continue` just as it would for a `continue` in the guarded body.
`tests/differential/FinallyContinueOverridesBreak.java` gates the case where
`finally { continue; }` overrides a pending `break`.

What remains is not a low-risk local patch:

- `exception_lowering::try_finally_scope_stmt` and
  `try_catch_finally_scope_stmt` still use a `Result` closure carrier. That
  cannot generally represent Java's competing abrupt completions:
  `return`, `throw`, plain/labeled `break`, and plain/labeled `continue`.
  `try_with_resources_finally_scope_stmt`, `try_finally_scope_stmt`, and
  `try_catch_finally_scope_stmt` use the same carrier family but are crate-local
  now; their exact emitted compatibility-shape assertions live beside
  `exception_lowering` instead of keeping those helpers public.
  The try-with-resources resource-name/declaration helpers are crate-local too;
  their direct assertions live beside `exception_plan`, while integration tests
  exercise them through `plan_try_stmt` and lowering.
  Follow-up cleanup applied the same boundary to `try_catch_stmt`,
  `try_catch_value_fallthrough_stmt`, and `try_with_resources_scope_stmt`. Their
  exact rendering assertions now live beside `exception_lowering`; integration
  tests keep exercising the planner/lowering behavior instead of importing these
  rendering helpers as public crate API.
- `lower::lower_simple_loop_try_finally_stmt` is intentionally a narrow fast
  path. It now handles nested no-catch/no-resource `try/finally` statements that
  fall through and simple loop-abrupt `finally` bodies, but nested loops,
  switch, and throw-heavy bodies remain outside it.
- Some Java-library exception surfaces still panic in helpers rather than
  returning catchable `JavaException` values.

The next implementation step should still be a real completion carrier such as
`Normal | Break(label) | Continue(label) | Return(v) | Throw(e)`, plus a
separate pass converting selected panic-based JDK helpers to catchable
fallible APIs. The promoted `FinallyNestedContinue.java` fixture covers one
formerly known nested `finally` case; broader nested/labeled completion shapes
still need the carrier design.

Update 2026-08-25: `StringTokenizer.nextToken()` is no longer one of the
panic-only JDK helper surfaces. Built-in StringTokenizer facts now mark
`nextToken`/`next_token` as mutating and throwing `NoSuchElementException`, the
facade ABI routes through `StringTokenizer::next_token_result`, and the support
result helper returns `Result<String, JavaException>`. Normal fallible callers
propagate with `?`; ordinary `next_token()` now unwraps through the shared
uncaught-Java-exception panic adapter instead of a direct `.expect(...)`.
`tests/differential/StringTokenizerNextCatch.java` verifies a surrounding
`catch (NoSuchElementException)` observes the Java exception and matches the
Java oracle (`a:caught`). The general completion-carrier work and other
panic-based helpers remain open.

Follow-up 2026-08-25: `StringTokenizer`'s `Enumeration` aliases are covered by
the same catchable path. `hasMoreElements()` now routes to `has_more_tokens`,
and `nextElement()`/`next_element()` are marked mutating, throwing
`NoSuchElementException`, and result-wrapped through
`StringTokenizer::next_element_result`. `tests/differential/
StringTokenizerNextElementCatch.java` verifies normal token return, empty-state
checking, and caught exhaustion against Java (`a:false:caught`). Ordinary
`nextElement()` inherits the same uncaught adapter through `next_token()`.

Update 2026-08-25: `StringBuilder.charAt()` and `StringBuffer.charAt()` are
also no longer panic-only at facade call sites. Built-in platform facts now mark
the methods as throwing `IndexOutOfBoundsException`, route the facade ABI
through `char_at_result`, and keep the panic-preserving `char_at` helper for
non-fallible callers. The ordinary helper now unwraps through the shared
uncaught-Java-exception panic adapter. `tests/differential/
StringBuilderCharAtCatch.java` verifies a surrounding catch matches the Java
oracle (`a:caught`).

Update 2026-08-25: `Iterator.next()` is no longer one of the panic-only helper
surfaces at facade call sites. Built-in iterator facts now mark
`next`/`next_element` as mutating and throwing `NoSuchElementException`, the
facade ABI routes through `JavaIterator::next_result`, and the support helper
returns `Result<T, JavaException>`. Ordinary `JavaIterator::next()` now unwraps
through the shared uncaught-Java-exception panic adapter instead of a direct
`.expect(...)`. `tests/differential/IteratorNextCatch.java` verifies exhaustion
is catchable and matches the Java oracle (`a:caught`).

Update 2026-08-25: `BitSet.set(int)` is no longer panic-only at facade call
sites. Built-in BitSet facts now mark `set` as mutating and throwing
`IndexOutOfBoundsException`, route the facade ABI through `BitSet::set_result`,
and keep `BitSet::set` as the uncaught-Java-exception adapter for non-fallible
callers. `tests/differential/BitSetSetCatch.java` verifies a negative index is
catchable and preserves the prior bit state, matching the Java oracle
(`set:caught`).

Update 2026-08-25: `Optional.get()` is no longer panic-only when the call is
inside a fallible/catchable context. The built-in Optional fact now records the
generic element return and `NoSuchElementException`, the facade ABI is
`Result`-wrapped, and native Optional receiver lowering uses
`ok_or_else(...)?` instead of `.expect("Java null receiver")` when result
propagation is active. `tests/idiom_gaps.rs::
optional_get_in_catch_lowers_to_result_projection` checks that a
`try/catch (NoSuchElementException)` over an `Optional<String>` parameter
generates the catchable result path and typechecks. I did not promote a
differential fixture here because `Optional.of`/`Optional.empty` still lower via
the external static facade surface; that is a separate Java library factory
mapping gap, not part of the `get` panic surface.

Update 2026-08-25: the Optional factory blocker is fixed too.
`Optional.empty()`, `Optional.of(value)`, and `Optional.ofNullable(value)` now
plan as native Java static calls rather than pack-backed placeholder facade
methods. `java.util.Optional`/`JavaUtilOptional` canonicalize to the same native
owner, the return type is inferred as `Optional<T>`, and the value projections
are `None`, `Some(value)`, and the nullable argument carrier respectively.
`tests/differential/OptionalFactoryGetCatch.java` was promoted and verifies
`Optional.empty().get()` reaches `catch (NoSuchElementException)` while
`Optional.of` and `ofNullable(null)` preserve presence semantics. At that
checkpoint the differential suite gated 113 fixtures.

Update 2026-08-25: `TreeMap.firstKey()` and `TreeMap.lastKey()` are no longer
panic-only inside propagated `try` scopes. The normal native collection lowering
uses the same `NoSuchElementException` result projection for ordinary callers
and unwraps it through the shared uncaught-Java-exception panic adapter, while
propagated calls use `keys().next()/next_back().ok_or_else(...).map(|value|
value.clone())?` and raise `NoSuchElementException` for empty maps.
`tests/differential/TreeMapFirstLastKeyCatch.java` verifies both empty-map
catches plus the non-empty `a`/`b` key results. At that checkpoint the
differential suite gated 114 fixtures.

Update 2026-08-25: `Arrays.sort(values, from, to)` and
`Arrays.fill(values, from, to, value)` now have catchable range-check lowering
when the statement is inside a propagated `try`/`Result` scope. The existing
ordinary non-fallible helpers now panic through the shared uncaught-Java-
exception adapter, while the fallible variants return
`Err(JavaException::new("IllegalArgumentException", ...))` for `from > to` and
`Err(JavaException::new(
"ArrayIndexOutOfBoundsException", ...))` for negative/out-of-bounds ranges
before applying `?` at the statement site. The same pass fixed
`Arrays.fill(String[], ...)` to lower the fill value through the array element
assignment slot, so nullable object arrays receive `Some(value)` instead of a
bare `String`. `tests/idiom_gaps.rs::
java_arrays_range_calls_in_try_lower_to_java_exception_results`,
`java_arrays_sort_lowers_whole_and_range_mutations`, and
`java_arrays_fill_lowers_whole_and_range_mutations` pin the catchable path and
the uncaught-Java-exception panic adapter used by ordinary non-fallible callers.

Update 2026-08-25: `Arrays.copyOfRange(values, from, to)` now has the same
catchable range-check split in propagated `try`/`Result` scopes. The ordinary
projection now panics through the uncaught-Java-exception adapter for
non-fallible callers, while the fallible projection returns
`IllegalArgumentException` for `from > to` and
`ArrayIndexOutOfBoundsException` for invalid `from` before applying `?` at the
expression site. `tests/idiom_gaps.rs::
java_arrays_copy_of_range_in_try_lowers_to_java_exception_result` covers the
catchable path, and `java_arrays_copy_lowers_as_static_platform_copy_projection`
continues to pin the non-fallible panic behavior. The existing copy test's
`String[]` harness was also updated to match nullable object-array storage.

Update 2026-08-27: `Arrays.copyOf(values, len)` now has the corresponding
catchable negative-length path in propagated `try`/`Result` scopes. Ordinary
callers still panic for `len < 0`, but catchable callers return
`Err(JavaException::new("NegativeArraySizeException", ...))` before applying
`?` at the expression site. `tests/idiom_gaps.rs::
java_arrays_copy_of_in_try_lowers_to_java_exception_result` pins the generated
shape, and `tests/differential/ArraysCopyOfCatch.java` verifies the Java oracle
for both a padded normal copy and the caught negative-length branch.

Update 2026-08-25: `Arrays.binarySearch(values, from, to, key)` now has a
catchable range-check variant in propagated `try`/`Result` scopes too. The
ordinary projection now panics through the uncaught-Java-exception adapter for
non-fallible callers, while the result projection returns `IllegalArgumentException` for
`from > to` and `ArrayIndexOutOfBoundsException` for invalid bounds before
wrapping the Java insertion-point result in `Ok(...)`. `tests/idiom_gaps.rs::
java_arrays_binary_search_range_in_try_lowers_to_java_exception_result` covers
the catchable path, and
`java_arrays_binary_search_lowers_as_static_platform_search_projection` keeps
the non-fallible behavior pinned.

Update 2026-08-25: `Arrays.compare(left, from, to, right, from, to)` and
`Arrays.mismatch(left, from, to, right, from, to)` now use the same catchable
dual-range guard pattern in propagated `try`/`Result` scopes. The ordinary
range projections now panic through the uncaught-Java-exception adapter for
non-fallible callers, while the result projections return `IllegalArgumentException` for reversed ranges and
`ArrayIndexOutOfBoundsException` for invalid bounds before wrapping the
lexicographic/mismatch result in `Ok(...)`. `tests/idiom_gaps.rs::
java_arrays_compare_and_mismatch_ranges_in_try_lower_to_java_exception_results`
covers both APIs; the existing compare/mismatch projection tests continue to
pin the non-fallible behavior.

Update 2026-08-25: `StringBuilder.delete`, `StringBuilder.insert`, and their
`StringBuffer` twins are no longer silent index-clamping helpers. The built-in
effect facts now mark them as throwing fluent mutators, their facade ABI routes
through `delete_result`/`insert_result`, and the support helpers return
`StringIndexOutOfBoundsException` for invalid Java indexes while ordinary
non-fallible callers unwrap through the shared uncaught-Java-exception panic
adapter. `tests/differential/StringBuilderDeleteInsertCatch.java` verifies
normal mutation plus caught delete/insert index failures against the Java
oracle. At that checkpoint the differential suite gated 110 fixtures.

Update 2026-08-25: `StringBuilder.setLength(-1)` and
`StringBuffer.setLength(-1)` no longer silently clamp to zero. The support layer
now exposes `set_length_result`, the built-in and external-pack facts mark
`setLength`/`set_length` as mutating and throwing
`StringIndexOutOfBoundsException`, and the facade ABI routes propagated
`try`/`catch` calls through the fallible path. Ordinary non-fallible callers
unwrap through the shared uncaught-Java-exception panic adapter.
`tests/differential/StringBuilderSetLengthCatch.java` verifies normal
grow/truncate behavior plus caught negative-length failures against the Java
oracle. At that checkpoint the differential suite gated 111 fixtures.

Update 2026-08-25: `List.get(index)` is no longer routed through the ordinary
`Option` lookup/coercion path when the call is inside a propagated
`try`/`Result` scope. The catchable branch now lowers directly to
`get(index).cloned().ok_or_else(|| JavaException::new(
"IndexOutOfBoundsException", ...))?`, after first checking the signed Java index
for `< 0` before casting to `usize`; ordinary callers keep the existing
panic-preserving behavior. `tests/differential/ListGetCatch.java` verifies both
high and negative out-of-range `ArrayList.get` calls reach
`catch (IndexOutOfBoundsException)`. At that checkpoint the full differential
suite passed 115 fixtures.

Update 2026-08-25: `List.set(index, value)` now has the same catchable
propagated path. The result projection checks the signed Java index before the
`usize` cast, returns the old element through `Ok(old)`, and assigns through a
fallible `get_mut(...).ok_or_else(...)` so both negative and high indexes raise
`IndexOutOfBoundsException` instead of panicking. `tests/differential/
ListSetCatch.java` verifies the normal return value, mutation, and both caught
out-of-range cases. At that checkpoint the full differential suite passed 116
fixtures.

Update 2026-08-27: `List.add(index, value)` now has the same catchable
propagated path. The result projection evaluates the signed Java index once,
rejects negative values before the `usize` cast, rejects indexes greater than
`len()`, and only then performs `insert`, returning `Ok(())` for the statement
path. `tests/differential/ListAddIndexCatch.java` verifies the normal middle
insert plus caught high and negative out-of-range calls.

Update 2026-08-25: `new BitSet(size)` no longer silently clamps negative sizes
to an empty set in catchable contexts. The support layer now exposes
`BitSet::new_1_result`, keeps `new_1` as the uncaught-Java-exception adapter
for ordinary non-fallible construction, and propagated constructor lowering
routes one-argument `BitSet` construction through the result helper so
`NegativeArraySizeException` can be caught. `tests/differential/
BitSetCtorCatch.java` verifies the normal sized constructor plus
`catch (NegativeArraySizeException)` for `new BitSet(-1)`. At that checkpoint
the full differential suite passed 117 fixtures.

Update 2026-08-25: `new String(bytes, offset, len)` no longer silently clamps
invalid byte ranges. The support helper now has a `java_string_from_byte_range_result`
variant that returns `IndexOutOfBoundsException`, the ordinary helper unwraps it
through the shared uncaught-Java-exception panic adapter, and propagated
three-argument `String` constructor lowering applies `?` to the result helper.
`tests/differential/StringByteRangeCtorCatch.java` verifies the valid slice plus
high and negative out-of-range catches. At that checkpoint the full differential
suite passed 118 fixtures.

Update 2026-08-25: `new StringBuilder(capacity)` and
`new StringBuffer(capacity)` no longer silently clamp negative signed capacities
to zero. The shared support now checks signed capacity inputs and reports
`NegativeArraySizeException`; ordinary `new_1` calls unwrap through the existing
uncaught-Java-exception panic adapter, while propagated constructor lowering
routes integral one-argument `StringBuilder`/`StringBuffer` construction through
`new_capacity_result(...) ?`. `tests/differential/StringBuilderCapacityCatch.java`
verifies normal capacity construction plus both negative-capacity catches; the
full differential suite passed 119 fixtures at that checkpoint.

Update 2026-08-25: `Math.toIntExact(long)` no longer lowers to truncating
`as i32`. The native static lowering now uses `i32::try_from(...)` and maps
overflow to `JavaException::new("ArithmeticException", ...)`; propagated
`try`/`catch` scopes route the call through the result projection, while
ordinary callers keep the existing uncaught-exception panic policy. The facade
ABI is result-wrapped and still marked as an associated/static call. The
external-call propagation planner now treats result-wrapped facade ABI as a
catchable result source, not only checked thrown-effect facts. `tests/
differential/MathToIntExactCatch.java` verifies normal values plus caught
overflow against the Java oracle. At that checkpoint the full rebuilt
differential suite passed 122 fixtures.

Update 2026-08-25: `Optional.of(null)` now raises `NullPointerException` through
the same catchable Java-exception path used for required nullable arguments.
The slot-aware `Optional.of` lowering unwraps the nullable argument before
constructing the returned optional, so a caught null no longer becomes
`Some(None)` and bypasses `catch (NullPointerException)`.
`tests/differential/OptionalOfNullCatch.java` verifies the Java oracle. At that
checkpoint, the fixture was expected to bring the differential suite to 123
fixtures after rebuilding the release translator.

Update 2026-08-25: nested no-catch/no-resource `try/finally` inside the
simple loop-body finally fast path no longer emits `continue` inside a closure.
The loop fast-path predicate now admits nested `try/finally` statements whose
`finally` block falls through, and the existing abrupt-rewrite pass sees loop
abruptions through that nested body. `FinallyNestedContinue.java` moved out of
`tests/differential/known-gaps/` into the main differential suite and verifies
Java's `0ioio2io` oracle. At that checkpoint the differential suite passed 124
fixtures, and the known-gap set had dropped to 6 fixtures.

Update 2026-08-25: `Thread.sleep(-1)` no longer casts the negative Java `long`
to a huge Rust `u64` duration in catchable contexts. Propagated `Thread.sleep`
calls now route through a result projection that returns
`IllegalArgumentException` for negative timeouts before sleeping; ordinary calls
keep the existing panic/blocking lowering policy. `tests/differential/
ThreadSleepCatch.java` verifies the Java oracle. At that checkpoint the
differential suite passed 125 fixtures.

Follow-up 2026-08-25: the ordinary `Thread.sleep` projection now shares that
same signed timeout check and unwraps the result through the existing uncaught
Java-exception panic adapter. The remaining unchecked behavior is now the policy
for an uncaught `IllegalArgumentException`, not the old `millis as u64` cast that
could turn `-1` into a huge sleep. The propagated helper still calls one
internal unchecked sleep leaf after validating the signed value.

Follow-up 2026-08-28: the two-argument `Thread.sleep(long millis, int nanos)`
path now preserves and validates the nanos argument. Both ordinary and
propagated lowering route through the same result projection, which raises
`IllegalArgumentException` when `nanos < 0 || nanos > 999999` before sleeping.
`tests/differential/ThreadSleepNanosCatch.java` verifies the Java oracle
(`iae`) against the translated Rust output. At that checkpoint the differential
suite gated 218 fixtures.

Follow-up 2026-08-25: ordinary primitive parse projections now share the
`NumberFormatException` result chain and then apply the existing uncaught
Java-exception panic adapter, instead of spelling `.parse::<T>().unwrap()`
directly. Propagated parse paths continue to use `?`. The cleanup also exposed
two adjacent composition bugs that are now fixed: ordinary `List.get(index)`
casts the Java index through the central index projection before calling Rust
`get`, and the catchable null-rewrite no longer turns a required `expr?` value
into `expr?.as_ref().ok_or_else(...)`.

Follow-up 2026-08-25: `StringBuilder.substring(...)` and
`StringBuffer.substring(...)` now use result-returning support helpers in
catchable contexts, so `StringIndexOutOfBoundsException` is catchable instead of
falling through to a missing or panic-only method. The JDK facade ABI covers both
one- and two-argument overloads, including the `JavaLangStringBuffer`/
`JavaLangStringBuilder` owner spellings used by generated external facade
shells. The support facade now takes the Rust method name from the ABI rust path
leaf, which keeps unmodelled facade shells from emitting invalid Rust overloads
named `substring` for multiple arities. `tests/differential/
StringBuilderSubstringCatch.java` verifies valid `StringBuilder` and
`StringBuffer` substrings plus a caught out-of-bounds call. Ordinary
non-fallible substring helpers unwrap through the shared uncaught-Java-exception
panic adapter.

Follow-up 2026-08-25: boxed primitive string-constructor helpers no longer use
raw Rust parse panics. `java_parse_i8`/`i16`/`i32`/`i64`/`f32`/`f64`, used by
lowering such as `new Integer(String).compareTo(...)`, now map parse failures
to `JavaException::from_display("NumberFormatException", err)` and then apply
the same uncaught-Java-exception panic adapter as ordinary non-fallible
`parseInt`/`parseDouble` projections. The existing
`boxed_primitive_constructors_lower_to_values` generated-crate check now also
guards against the old `expect("Java number format")` helper body.

Follow-up 2026-08-25: the opaque `Future<T>` facade now agrees with call
lowering that Java `Future.get()` is throwable. The generated carrier method is
`get(&self) -> Result<T, JavaException>` rather than plain `T`, which fixes the
old invalid composition where ordinary callers appended the uncaught-exception
adapter to a non-`Result` value. Covered by the newly compile-backed
`selected_opaque_future_get_is_generic_instance_surface` test.

Update 2026-08-26: catch-body loop exits in `try/catch/finally` no longer cross
the internal result-closure boundary for the simple loop-body shape. A new
loop-body fast path keeps the guarded try body in the existing catchable
`Result` scope, but lowers catch arms as direct match blocks and injects the
`finally` body before catch-body `break`/`continue`; unlabeled `continue` still
gets the classic-for update after `finally`. The existing labeled-continue
update rewrite now recurses through structured expression `match`/block bodies,
so a catch-body `continue outer` also runs the target loop update. `tests/
differential/CatchFinallyBreak.java`, `CatchFinallyContinue.java`, and
`CatchFinallyLabeledContinue.java` verify the Java oracle, and the rebuilt
differential suite passes 137 fixtures.

Follow-up 2026-08-26: the same loop-body shape now covers try-with-resources
with catch-body loop exits. Resource declarations are lowered into the existing
resource environment, the guarded TWR body still runs inside a catchable
`Result` closure, resources are closed before the catch match executes, and
catch arms are lowered as direct loop-body blocks so `break`/`continue` do not
cross a closure boundary. Unlabeled `continue` gets the classic-for update
injected before continuing. `tests/differential/TryWithResourcesCatchBreak.java`
and `TryWithResourcesCatchContinue.java` verify close-before-catch ordering and
loop-exit behavior; the rebuilt differential suite passes 139 fixtures.

Follow-up 2026-08-26: the simple loop-body fast path also covers guarded-body
`break`/`continue` when the guarded body is syntactically non-throwing, so the
catch arm is unreachable and the only Java completion rule to preserve is
finally/resource cleanup before the loop exit. `TryCatchFinallyBodyBreak.java`
now lowers without putting `break` inside the internal exception closure;
`TryWithResourcesBodyBreak.java` and `TryWithResourcesBodyContinue.java` inject
resource cleanup before the loop exit, with classic-for updates still running
before an unlabeled `continue`. The promoted differential suite passes 146
fixtures. Broader throwing guarded bodies, nested/labeled completion carriers,
and panic-only library surfaces remain open.

Follow-up 2026-08-26: ordinary non-propagating `Optional.get()`,
`TreeMap.firstKey()`, `TreeMap.lastKey()`, and native source `String.charAt()`
lowering now use the same result projections as catchable contexts and apply
the shared uncaught-Java-exception panic adapter, instead of using direct
`.expect(...)` paths. `Optional` and TreeMap use `NoSuchElementException`;
`charAt` uses `StringIndexOutOfBoundsException`. The propagated paths still emit
`?` and are pinned by `optional_get_in_catch_lowers_to_result_projection` plus
the existing TreeMap/String bounds catch fixtures; the native fallbacks are
pinned by `optional_get_native_fallback_uses_uncaught_java_exception_adapter`
and `tree_map_last_key_native_fallback_uses_uncaught_java_exception_adapter`
plus `string_char_at_native_fallback_uses_uncaught_java_exception_adapter`.
Broader completion-carrier work and other panic-only helper surfaces remain open.

Follow-up 2026-08-26: ordinary sequence-get helper lowering no longer emits
`.expect("Java index out of bounds")`. `sequence_get_expect_projection` now
builds `get(index).ok_or_else(|| JavaException::new("IndexOutOfBoundsException",
...))` and applies the shared uncaught-Java-exception panic adapter for
non-fallible callers. Focused generated-crate checks covering optional sequence
slots, array indexing after collection get, source-to-facade slot adaptation,
raw erased-list elements, and map-key loop indexing pass with the new shape.
This is still the ordinary helper path; broader catchable `List.get` lowering
is covered separately.

Follow-up 2026-08-26: the external facade alias for `String.charAt`
(`str::chars::nth`) now uses `string_char_at_result_projection` instead of the
older direct `.expect("Java string index out of bounds")` projection. In
propagating contexts it emits `...?`, so a surrounding Java catch can observe
`StringIndexOutOfBoundsException`; ordinary contexts still apply the shared
uncaught Java-exception panic adapter. Focused checks:
`call_lowering::tests::string_char_at_native_fallback_uses_uncaught_java_exception_adapter`,
`idiom_gaps::java_string_tokenizer_uses_facade_signature_facts`, and
`tests/differential/StringTokenizerCharAtBoundsCatch.java`; follow-up
2026-09-09 added that fixture to the focused exception oracle loop, so this
composition is checked outside the broad differential sweep.

Follow-up 2026-08-26: the bounded no-catch try-with-resources/finally shape now
lowers instead of tripping the stale unsupported pre-pass. `TryStmtPlan` has a
`WithResourcesFinally` variant for declaration resources, no catches,
fall-through `finally`, and no `break`/`continue` in the guarded body or
finally body. Lowering runs resource initializers, captures the try body result,
closes resources right-to-left, runs `finally`, then replays the result, matching
JLS extended try-with-resources ordering for the accepted subset. The
unsupported-statement pre-pass now recognizes the new plan variant. Regressions:
`slot_shape_design::semantic_analysis_owns_exception_classification_and_catch_facts`,
`fixtures::try_with_resources_finally_order_fixture_lowers_without_placeholder`,
and `tests/differential/TryWithResourcesFinally.java`; the rebuilt differential
suite passes 149 fixtures. Catch+resources+finally, abrupt `finally`, loop
completion in this generic closure path, and suppressed-exception semantics
remain open.

Follow-up 2026-08-26: the simple loop-body fast path now also covers
try-with-resources/finally when the guarded body exits with `break` or
`continue`. The accepted shape is still narrow: declaration resources, no
catches, fall-through `finally`, a simple syntactically non-throwing guarded
body, and no loop completion inside the `finally` body itself. Lowering keeps
resource declarations outside the exception-result closure, injects reverse
resource cleanup followed by `finally` before the loop exit, and still runs
classic-for updates before an unlabeled `continue`. Regressions:
`tests/differential/TryWithResourcesFinallyBodyBreak.java` and
`tests/differential/TryWithResourcesFinallyBodyContinue.java`; the rebuilt
differential suite passes 151 fixtures. Throwing guarded bodies,
catch+resources+finally, abrupt `finally`, and suppressed-exception semantics
remain open.

Follow-up 2026-08-26: zero-argument `Queue.remove()`/`ArrayDeque.remove()` is
no longer misplanned as Rust `Vec::remove()` with no index. Native collection
planning now gives queue-head removal its own method, ordinary callers remove
index `0` through the shared uncaught-Java-exception adapter, and propagated
`try`/`catch` scopes lower empty queues to `NoSuchElementException`.
`tests/differential/QueueRemoveCatch.java` verifies the Java oracle
(`a:caught`); the rebuilt differential suite passes 153 fixtures. Broader
completion-carrier work and other panic-only helper surfaces remain open.

Follow-up 2026-08-26: the simple loop-body fast path now covers the definite
`finally { break; }`/`finally { continue; }` shape overriding a pending guarded
body `return`. The accepted subset requires a simple syntactically non-throwing
return expression, no guarded-body loop abrupt completion, and a finally block
whose last statement definitely completes with loop `break`/`continue`.
Lowering evaluates the pending return value for side effects, discards the
return completion, then emits the finally loop completion directly in the loop
body rather than across a closure boundary. Covered by
`lower::tests::finally_break_overrides_loop_body_return_translation` and
`tests/differential/FinallyBreakOverridesReturn.java` (`after`). Conditional
finally completion, throwing return expressions, and the full nested/labeled
completion carrier remain open.

Follow-up 2026-08-28: the same loop-body fast path now covers the adjacent
`try/catch/finally` shape where `finally { break; }` overrides a pending catch
`return`. The accepted subset is deliberately narrow: no resources, a
catch/finally plan, no value return in the try body, no loop-abrupt completion
inside catch bodies, simple finally body, and catch return values whose
expressions can be evaluated then discarded. The discard helper also unwraps
fallible `return Ok(value)` lowering back to `value;`, avoiding an ambiguous
discarded `Ok(...)` expression. Covered by
`lower::tests::finally_break_overrides_catch_return_translation` and
`tests/differential/FinallyBreakOverridesCatchReturn.java` (`after`); the full
differential suite passes 189 fixtures. Catch/resource/finally combinations,
conditional finally completion, throwing return expressions, and the full
nested/labeled completion carrier remain open.

Follow-up 2026-08-28: `Math.floorDiv`/`Math.floorMod` now have catchable
`ArithmeticException` result helpers for zero divisors. Ordinary callers keep
the existing value-returning API through the shared uncaught-Java-exception
panic adapter; propagating `try`/`catch` scopes use the `*_result` helpers and
`?`. The same projection path covers `int`, `long`, and the Java
`floorMod(long,int): int` narrowing overload. This also exposed a native-owner
alias bug: `StrictMath` could be treated as a Rust-form alias shadowing native
`Math`, so `StrictMath.floorDiv`/`floorMod` fell back to unresolved external
calls. Native static-owner matching now canonicalizes aliases before the shadow
check. Covered by `tests/differential/MathFloorDivModCatch.java`,
`coercion::tests::jdk_math_and_objects_projections_own_expression_shapes`,
`idiom_gaps::jdk_numeric_and_decimal_format_facades_keep_precise_return_shapes`,
and `idiom_gaps::strict_math_common_calls_use_native_math_alias`. Broader
panic-only helper surfaces and the general finally completion carrier remain
open.

Follow-up 2026-08-28: the no-catch try-with-resources/finally scope now stores
resource cleanup in its own `Result<(), JavaException>` scope before running
the user `finally` body. A throwing `close()` can no longer skip a fall-through
user `finally`; if cleanup fails, that failure is propagated only after the
finally body runs. Covered by
`tests/differential/TryWithResourcesCloseThrowFinally.java` (`tcf:close`); the
rebuilt differential suite gated 191 fixtures at that checkpoint. Suppressed
exception attachment, throwing `finally`, and the general nested/labeled
completion carrier remain open.

Follow-up 2026-08-28: resource stream reads inside while-assignment conditions
now compile through the typed `InputStream.read(byte[])` facade path. The
specialized source `InputStream.read` byte-array lowering adapts nullable
receivers before calling `read_3`, so an `InputStream` proven non-null by an
earlier guard is not left as `Option<InputStream>`. Local assignment expression
blocks also preserve the existing `ResultPropagated` marker when the assigned
value already contains `?`, preventing the while condition from applying `?` to
the plain assigned `i32` tail. Covered by
`tests/idiom_gaps.rs::resource_input_stream_read_and_close_have_typed_facade_support`,
which is promoted out of the idiom-gaps failure baseline. The adjacent source
subclass bridge is also pinned: one-argument `read(byte[])` on a source
`GZIPInputStream` subclass now preempts external return-call lowering and
routes through `read_3(buffer, 0, buffer.len())` instead of the inherited
one-argument facade stub. Covered by
`tests/idiom_gaps.rs::input_stream_read_byte_array_uses_three_arg_overload_with_full_buffer`.
Broader panic-only helper surfaces and the general finally completion carrier
remain open.

Follow-up 2026-08-28: try-with-resources cleanup no longer replaces an already
thrown body exception with a later throwing `close()`. TWR lowering now captures
cleanup in its own `Result<(), JavaException>` and combines
`(__java_twr_result, __java_twr_cleanup_result)` so body `Err` remains primary;
cleanup `Err` is propagated only when the body completed normally. The
no-catch/finally path still runs user `finally` before replaying that combined
result, so fall-through `finally` order is preserved. This does not yet attach
suppressed exceptions. Covered by
`tests/differential/TryWithResourcesPrimaryExceptionCatchFinally.java`
(`trc:body:f`) plus the existing try-with-resources fixture checks.

Follow-up 2026-08-28: try-with-resources cleanup exceptions are now attached
as suppressed exceptions when the guarded body has already thrown. The shared
`JavaException` carrier stores suppressed exceptions, exposes
`get_suppressed()` with the nullable Java array carrier
`Vec<Option<JavaException>>`, and the TWR result combiner mutates the primary
exception before replaying it. This also fixed a resource-body result-type
conflation: a body that only throws terminates, but it does not terminate with
a value, so TWR body result scopes now use the existing value-return predicate
instead of treating every terminating body as `Ok<T>`. Covered by
`tests/differential/TryWithResourcesSuppressedPrimary.java` (`body:1:close`)
plus `slot_render::tests::exception_like_java_types_render_to_java_exception_carrier`;
the full differential suite has 218 fixtures. Broader panic-only exception
surfaces and the general nested/labeled `finally` completion carrier remain
open.

Follow-up 2026-08-28: one-dimensional primitive array compound assignments in
propagating `try` scopes now use the same checked `get_mut(...).ok_or_else`
write path as plain array stores. The checked slot is bound first, the old
element value is read from that slot, and only then is the computed value
assigned, so both high and negative indices raise catchable
`ArrayIndexOutOfBoundsException` instead of panicking. Covered by
`lower::tests::fallible_array_index_compound_write_reads_checked_slot_before_assigning`
and `tests/differential/ArrayCompoundWriteException.java`; the full
differential suite has 218 fixtures. Broader panic-only exception surfaces
and the general nested/labeled `finally` completion carrier remain open.

Follow-up 2026-08-28: simple loop bodies now have a bounded fast path for
try-with-resources with both `catch` and fall-through `finally` when a catch
body exits the loop with `break` or `continue`. The lowering captures the TWR
body result, runs resource cleanup, dispatches catches, prepends the user
`finally` body before catch-owned loop exits, and runs the same `finally` body
after fall-through catch/normal completion. Covered by
`tests/idiom_gaps.rs::try_with_resources_catch_finally_break_runs_cleanup_and_finally_before_break`
(`trcfd`). Broader throwing-`finally`, nested/labeled completion, and
panic-only Java exception surfaces remain open.

Follow-up 2026-08-28 `Arrays.equals` range result pass:
six-argument `Arrays.equals(left, from, to, right, from, to)` now uses the same
result-propagating dual-range helper as `Arrays.compare` and
`Arrays.mismatch` when lowered inside a catchable/result-propagating context.
Bad range order returns `Err(JavaException::new("IllegalArgumentException", ...))`
and bad bounds returns
`Err(JavaException::new("ArrayIndexOutOfBoundsException", ...))` instead of
panicking before translated Java catches can observe the exception. Covered by
the extended generated-crate test
`idiom_gaps::java_arrays_compare_and_mismatch_ranges_in_try_lower_to_java_exception_results`.

Follow-up 2026-08-28 `ObjectInputStream.readObject()` downcast pass:
direct assignment of `readObject()` into a concrete local no longer composes a
second result propagation around the erased-object downcast. The lowered value
keeps `read_object()?` at the deserialization call, then downcasts the payload
without applying `?` to the recovered concrete value. The downcast diagnostic
rewrite now walks unary and `Try` wrappers, so deserialization casts consistently
report `Java deserialization type mismatch`. Exception-like Java runtime
targets (`Throwable`, `Exception`, `*Exception`, `*Error`) now downcast to the
`JavaException` carrier, matching slot rendering and field storage. Covered by
`tests/idiom_gaps.rs::object_input_stream_read_object_recovers_string_target_slot`
and the existing `object_input_stream_read_object_*` generated-crate cluster.

Follow-up 2026-08-29 try-with-resources catch/finally guarded-body pass:
the simple loop-body fast path now also covers try-with-resources with one or
more catches and a fall-through `finally` when the guarded try body exits the
loop with `break` or `continue`. The lowering keeps resource declarations in
the same scope, emits resource cleanup followed by the user `finally` before
the loop exit, and still injects classic-for updates before `continue`. This is
deliberately still a loop-local fast path; the general `TryStmtPlan` abrupt
completion model remains open. Covered by
`try_with_resources_catch_finally_body_break_runs_cleanup_and_finally_before_break`,
`try_with_resources_catch_finally_body_continue_runs_cleanup_finally_and_update`,
and the existing catch-body try-with-resources/finally loop-exit fixture.

Follow-up 2026-08-29 nullable stream source pass:
owned iteration over a nullable stream source now uses the result-propagating
Java null receiver projection inside catchable contexts. A `try` around
`maybe().stream().map(...).collect(...)` therefore receives
`NullPointerException` through `ok_or_else(...)?` instead of panicking through
`unwrap()` or iterating the `Option` carrier. Covered by
`nullable_stream_source_call_in_try_lowers_to_catchable_npe`.

Follow-up 2026-08-29 negative array size pass:
array creation inside a result-propagating/catchable context now guards every
Java dimension before casting to `usize`. Negative lengths return
`Err(JavaException::new("NegativeArraySizeException", ...))`, so
`try { int[] a = new int[n]; } catch (NegativeArraySizeException ex) { ... }`
and the corresponding multi-dimensional cases reach the translated catch
instead of falling into a Rust allocation panic. Non-catch contexts still use
the existing panic-oriented lowering. Covered by
`array_creation_negative_size_in_try_lowers_to_java_exception_result` and
`array2_creation_negative_dimension_in_try_lowers_to_java_exception_result`.

Follow-up 2026-08-29 try-with-resources finally-abrupt pass:
the loop-local try-with-resources/catch/finally fast path now covers the narrow
shape where a definitely abrupt `finally { break; }` or
`finally { continue; }` overrides a pending catch return. Lowering evaluates
the try/catch side effects, closes declared resources, discards the catch return
value, then runs the user finally body directly in the loop so the `break` or
`continue` is legal Rust; classic-for updates still run before an unlabeled
continue. Covered by
`try_with_resources_finally_break_overrides_catch_return_after_cleanup` and
`try_with_resources_finally_continue_overrides_catch_return_and_runs_update`.
Throwing cleanup/finally and the full nested/labeled completion carrier remain
open.

Follow-up 2026-08-29 map-entry iterator-next pass:
direct synthetic `Map.Entry` iterator projections now use
`Iterator.next().ok_or_else(...)?` in result-propagating/catchable contexts
instead of always rendering `iter().next().expect("Java iterator exhausted")`.
Empty `entrySet().iterator().next().getKey()` can therefore be caught as
`NoSuchElementException` when lowered through the map-entry shortcut. Ordinary
non-catch contexts keep the existing direct `expect` projection. Covered by
`tests/differential/MapEntryIteratorNextCatch.java`. General panic-only helper
surfaces and the full nested/labeled completion carrier remain open.

Follow-up 2026-08-29 two-dimensional array read pass:
ndarray-backed `values[row][col]` element reads and `values[row]` row reads now
emit explicit bounds checks in result-propagating/catchable contexts before
indexing or projecting the row. Bad positive or negative indices return
`ArrayIndexOutOfBoundsException` through the translated Java catch instead of
panicking through Rust's `Index<[usize; 2]>` or ndarray row slicing. Non-catch
contexts keep the direct indexing/projection path. Covered by
`tests/idiom_gaps.rs::array2_index_read_in_try_lowers_to_java_exception_result`
and
`tests/idiom_gaps.rs::array2_row_read_in_try_lowers_to_java_exception_result`.
Follow-up 2026-08-29 ragged primitive array creation pass:
`new int[n][]` inside a `try`/`catch (NegativeArraySizeException)` now keeps the
ragged nested-`Vec` carrier instead of trying to initialize an
`ndarray::Array2<i32>` slot. The local declaration records a binding-aware
nested-array side fact, the initializer uses the same negative-size
`Err(JavaException::new("NegativeArraySizeException", ...))` guard as ordinary
array creation, and later `values.length` lowers as `values.len()` rather than
`values.nrows()`. Covered by
`tests/idiom_gaps.rs::ragged_array_creation_negative_size_in_try_lowers_to_java_exception_result`.
The broader remaining work is still the real exception/completion carrier for
the remaining panic paths and general/nested/labeled `finally` completion
semantics.

Follow-up 2026-08-29 source exception parent support pass:
generated `JavaException::is_instance` now includes parent links for source
exception classes whose `extends` relation is present in the fact set. The JDK
parent table remains the baseline, but source/custom exception hierarchy facts
are no longer represented only by expanding every catch arm's class-name list.
Covered by
`exception_lowering::tests::java_exception_support_includes_source_exception_parent_edges`.

Follow-up 2026-08-29 fallthrough source-call propagation pass:
the `try_return_with_fallthrough_catch_uses_outer_fallback_return` fixture no
longer fails for the unrelated local `URL`/platform `URL` name collision.
At one 2026-09-07 release baseline recheck, the fixture still belonged in
`tests/baselines/idiom-gaps-failures.txt`: the catch arm computed a fallback
message but did not feed that value into a real fallthrough completion carrier,
and the generated code fell through to `Default::default()` instead. The
explicit-null return pass below supersedes this intermediate state. The general
completion carrier and remaining panic paths are still open.

Follow-up 2026-09-07 explicit-null return pass:
source methods with an explicit non-primitive `return null` no longer let
concrete/native returned-call facts strip the nullable return carrier. The
fallthrough try/catch row now keeps `Result<Option<String>, JavaException>`
inside the try value carrier and returns `None` for the outer fallback. The
adjacent `StreamCallbackChain.computeScores` assertion was updated from the old
`Default::default()` placeholder to the current `return None;` nullable return.
Focused generated-crate checks pass for both rows, and release `idiom_gaps` is
now `816 passed; 0 failed`. The audit remains open for the broader completion
carrier and remaining panic-based exception surfaces.

Follow-up 2026-08-29 loop-finally block-expression pass:
the simple loop-body `try/finally` fast path now rewrites unlabeled
`continue`/`break` statements inside lowered statement-level block expressions.
That closes the shape where Java `{ continue; }` inside a try body ran before
the `finally` body and skipped the classic-for update in emitted Rust. The
rewrite still does not recurse into nested Rust loop bodies, because those
abrupt exits target the nested loop. Covered by
`lower::tests::prepend_finally_to_loop_abrupts_recurses_into_block_expr_stmt`
and `tests/differential/FinallyBlockContinue.java` (`tftf`). The broader
completion carrier and remaining panic paths remain open.

Follow-up 2026-08-29 do/while loop-finally continue pass:
`do/while` bodies now use the same loop-body lowering hook as `while`/`for`, so
a `continue` inside a simple `try/finally` no longer lands inside the old
closure-based exception carrier. Because Java `continue` in a do/while loop
must still evaluate the trailing condition, lowered do/while bodies also rewrite
continues that target the current loop to run the condition break-check before
continuing. Covered by
`lower::tests::do_while_finally_continue_runs_finally_then_condition_check` and
`tests/differential/FinallyDoWhileContinue.java` (`0f1xf2xf`). The general
completion carrier remains open.

Follow-up 2026-08-29 do/while catch-dispatch match-arm pass:
the same do/while trailing-condition replay now recurses into statement-level
`match` expressions whose arms contain lowered block bodies. This closes the
bounded shape where a simple `try/catch/finally` catch dispatch lowered to a
`RustExpr::Match`, leaving an arm-local `continue` without the do/while
condition break-check. Covered by
`lower::tests::do_while_condition_replay_recurses_into_match_arm_blocks`. The
general nested/labeled completion carrier remains open.

Follow-up 2026-08-29 nested-block return-discard pass:
the simple loop-body `try/finally` fast path now rewrites pending returns inside
lowered statement-level block expressions before appending an overriding
`finally { break; }` / `finally { continue; }` abrupt completion. The detector
already accepted source `Stmt::Block`; the rewrite now sees the matching
`RustExpr::Block` carrier. Covered by
`lower::tests::finally_break_overrides_loop_body_return_translation`. The
general nested/labeled completion carrier remains open.

Follow-up 2026-08-29 multi-resource cleanup suppression pass:
try-with-resources cleanup closures now close every resource in reverse
declaration order even after an earlier close fails. The first cleanup exception
becomes the cleanup result and later close failures are attached with
`add_suppressed`, matching Java's resource-close rule instead of short-circuiting
on the first `close()?`. Covered by
`exception_lowering::tests::try_with_resources_cleanup_accumulates_multiple_close_failures`
and `tests/differential/TryWithResourcesMultipleCloseSuppressed.java`; both Java
and translated Rust print `b:1:a`.

Follow-up 2026-08-29 throwing-finally regression:
the existing non-loop `try/finally` carrier already preserves the Java rule that
a throwing `finally` overrides a pending try return. This is now pinned by
`tests/differential/FinallyThrowOverride.java`, where both Java and translated
Rust return `finally`. The exception-lowering helpers also now stop appending
saved try-result propagation after a definitely terminating lowered `finally`
body, removing the unreachable `return __java_try_result` tail from that shape.
Covered by
`exception_lowering::tests::terminating_finally_body_suppresses_post_finally_try_result_use`.

Follow-up 2026-08-29 optional PrintStream baseline pass:
the optional `PrintStream.println` fixture rows now pass in the current tree.
Both the arity-named support path and the mutating optional argument receiver
path compile with generated-crate checks, so
`optional_print_stream_field_println_uses_arity_named_support` and
`optional_print_stream_field_println_with_mutating_optional_arg_uses_mut_receiver`
were removed from `tests/baselines/idiom-gaps-failures.txt`.

Follow-up 2026-08-30 BitSet get result ABI pass:
`java.util.BitSet.get(int)` already advertised a catchable
`IndexOutOfBoundsException` effect, but the selected JDK facade facts only
registered the result ABI for `set(int)`. `BitSet.get` now has a
`BitSet::get_result` facade ABI with a borrowed receiver, owned index argument,
owned boolean result, and `Result` wrapping, so catchable lowering can rely on
the same fact surface as the runtime helper. Covered by
`slot_shape_design::stdlib_owns_known_jdk_content_method_facts`.

Follow-up 2026-08-30 fallible constructor fact pass:
`BitSet(int)`, `StringBuilder(int)`, and `StringBuffer(int)` already have
`*_result` runtime constructors that throw `NegativeArraySizeException`, and
lowering had bespoke propagated-context branches for those shapes. The selected
JDK fact surface now records builtin `ExternalConstructorRustResultFact` rows
for the simple, qualified, and canonical aliases, and constructor-result lookup
consults builtin model facts after user and external-pack facts. Catchable
constructor planning can now see the same fallibility without relying only on
the bespoke lowering branch. Covered by
`slot_shape_design::stdlib_owns_known_jdk_content_method_facts`.

Follow-up 2026-08-30 `Future.get()` call-site propagation pass:
the generic `Future<T>` facade already returned `Result<T, JavaException>`, but
ordinary slot lowering still missed `?` for lambda locals such as
`Reader reader = future.get()` inside a catchable `forEach` body. The selected
Java platform concurrent facts now model `Future.get()` as throwing checked
`InterruptedException`/`ExecutionException`, and both external-result lookup
paths consult the built-in external-pack surface instead of only stored
`FactSet` rows. This keeps the generated facade and call-site propagation in
agreement. Covered by
`tests/external_packs.rs::java_platform_concurrent_static_factory_declares_precise_return_type`
and
`tests/idiom_gaps.rs::foreach_lambda_throw_wraps_enclosing_method_in_result`.

Follow-up 2026-08-30 StringBuilder capacity-constructor precision pass:
the `StringBuilder(int)` / `StringBuffer(int)` constructor fact was keyed only
by owner and arity, so one-argument string constructors such as
`new StringBuilder("ab")` were falsely classified as result-returning. Slot
fallibility and value lowering now apply that constructor result only when the
single argument is an integral capacity. Capacity constructors render
`new_capacity_result(...)` in both catchable and ordinary contexts, using `?`
or the uncaught-Java-exception adapter as appropriate; string/char-sequence
initializers stay on plain `new_1(...)`. Covered by
`tests/idiom_gaps.rs::string_builder_capacity_constructor_uses_result_only_for_integral_capacity`.
The maintained full differential fixture count now lives in
`tests/differential.rs::EXPECTED_DIFFERENTIAL_FIXTURES`.

---

## 2026-09-02 — `unmodifiable*` / `asList`: direct mutation fixed, alias fix followed

The recheck added a silent-divergence finding: `Collections.unmodifiableList`,
`unmodifiableMap` and `Arrays.asList` erase to a plain `mut Vec`/`HashMap`, so
Java's `UnsupportedOperationException` never happens (Java `uoe` vs translated
`added:2`), and a dead `catch (UnsupportedOperationException)` arm is emitted
beside it.

The obvious fix — refuse these loudly, as 0058 now does for upcasts — was
**measured and rejected**. Every use in the repo is read-only consumption:

```
examples/idiom-gaps/CollectionAddAllAndSize.java:10   values.addAll(Arrays.asList("a", "b"))
examples/idiom-gaps/ArraysAsListSplitAddAll.java:10   fields.addAll(Arrays.asList(split))
tests/differential/CollectionAddAllReturn.java:15,19,20
tests/differential/MapKeySetRemoveAll.java:13
```

11 occurrences, all `Arrays.asList` as an argument to `addAll`/`removeAll`, and
**zero** uses of `unmodifiable*` anywhere in `examples/` or `tests/`. All of them
are correct today as a `Vec`. A blanket refusal would break six working fixtures,
including differential ones, to correct a divergence none of them exhibits.

At that point, the divergence needed *mutation through the view*, so the real
fix was to carry "this value came from an unmodifiable view" on the value and
refuse the mutating call — not to refuse the constructor. The follow-up below
records the bounded direct/local-alias repair that landed later; broader
exception/finally work remains open separately.

Follow-up 2026-09-06: the direct expression subset is now fixed without the
blanket refusal rejected above. Fresh structural calls such as
`Collections.unmodifiableList(values).add(value)` and `Arrays.asList(value).add(other)`
lower to `UnsupportedOperationException`, while read-only `Arrays.asList` uses
still lower as collection values. A later 2026-09-06 follow-up also records
local view provenance, so storing the view in a local and mutating it later is
covered by focused block-lowering tests. The broader 0057 exception/finally
surface remains open.

Follow-up 2026-09-06: `List.subList(from, to)` is now recognized as a native
list call. Ordinary lowering returns an owned `Vec` copy, and propagating
`try`/`catch` lowering checks Java range failures instead of panicking. It now
preserves the Java exception split: negative/out-of-size bounds raise
`IndexOutOfBoundsException`, while `from > to` raises `IllegalArgumentException`.
Chained
`subList(...).size()` is also retargeted through native collection lowering.
This deliberately does not implement Java's live-view aliasing or fail-fast
iterator semantics. Covered by
`tests/idiom_gaps.rs::list_sub_list_returns_owned_vec_copy` and
`tests/idiom_gaps.rs::list_sub_list_out_of_bounds_is_catchable`, with the
exception-class split covered by
`tests/idiom_gaps.rs::list_sub_list_invalid_range_is_catchable_as_illegal_argument`.

Follow-up 2026-09-07: the checked-in assertions for
`selected_opaque_future_get_is_generic_instance_surface` and
`optional_print_stream_field_println_with_mutating_optional_arg_uses_mut_receiver`
were stale against the current lowering. Both generated crates compile with the
null-safe/fallible receiver path, so those rows were removed from the idiom-gaps
baseline.

Follow-up 2026-09-07: a read-only triage suggested adding a compile-loud
boundary test for nested labeled `try/finally` loop completion. The focused
probe showed that shape is already supported: a labeled `continue outer` from
inside a nested loop runs the `finally` body before the outer loop update and
does not lower through a closure-crossing `Result` block. The regression is
now positive and compile/runtime-backed by
`tests/idiom_gaps.rs::nested_labeled_try_finally_continue_runs_finally_before_outer_update`.

Follow-up 2026-09-07 super-boundary fallibility cleanup: `CallTargetPlan` no
longer stores a `SuperBoundary.propagates_result` bit computed from
`FactSet::method_has_thrown_exceptions`. The summary-only and emitted-summary
super-call lowerers now ask `CallableAnalysis` for the solved
`CallableKey::Method` instead, matching the source-summary super-call path and
removing one more raw fact probe from lowering-visible call planning. Guarded by
`slot_shape_design::semantic_analysis_owns_exception_classification_and_catch_facts`.

Follow-up 2026-09-07 focused finally/TWR oracle: promoted completion fixtures
now have a small differential loop instead of relying only on the full fixture
sweep. `finally_completion_fixtures_match_java_output` covers return override,
catch/finally return override, continue overriding break, nested/labeled
continue, and try-with-resources break/continue shapes. This pins the bounded
cases already handled today; the broader completion-carrier design remains
open.

Follow-up 2026-09-07 focused finally/TWR oracle: that loop now also covers the
promoted block/do-while/throwing-finally cases and the promoted
try-with-resources+finally cases. The recheck remains intentionally bounded to
currently supported completion shapes; the broader nested/labeled completion
carrier is still open.

Follow-up 2026-09-09 focused finally/TWR oracle: the same loop now includes
`tests/differential/TryWithResourcesCatchFinallyBreak.java`, promoting the
previous inline generated-crate check for catch-body `break` plus cleanup plus
`finally` into a Java/Rust behavioural oracle (`trcfd`).

Follow-up 2026-09-09 focused finally/TWR oracle: the sibling
`finally { continue; }` override shape for try-with-resources/catch/finally is
now also promoted from the inline generated-crate check to the Java/Rust oracle.
`tests/differential/TryWithResourcesFinallyContinueOverridesCatchReturn.java`
pins cleanup-before-finally and classic-for update replay before the overriding
continue (`trcftrcfd`).

Follow-up 2026-09-09 focused finally/TWR oracle: the body-abrupt siblings that
were still only inline generated-crate checks are now promoted too.
`tests/differential/TryWithResourcesCatchFinallyBodyBreak.java` pins
cleanup-before-finally before a body `break` exits the loop (`trfd`), and
`tests/differential/TryWithResourcesCatchFinallyBodyContinue.java` pins the
body `continue` path including classic-for update replay (`trftrftrfd`).

Follow-up 2026-09-10 cleanup: `exception_lowering::CatchRenderArm` is
crate-local, and stale integration/layering assertions now check the
`pub(crate)` try/catch rendering boundary instead of expecting public helper
symbols. This does not change the broader Java completion-carrier gap.

Follow-up 2026-09-10 cleanup: `exception_lowering::ExternalExceptionConversion`
and `external_exception_conversions` are crate-local too. Exact conversion-record
coverage moved beside `exception_lowering`; integration coverage now reaches the
support item through public program lowering output.

Codex follow-up 2026-09-12 cleanup: `CatchRenderArm` and
`ExternalExceptionConversion` are no longer crate-local field bags. Try/catch
rendering constructs catch arms through `new(...)`, reads guard data through
accessors, and consumes catch bodies with `into_body()` at the render boundary;
external exception conversion rendering now uses `java_type()` / `rust_type()`.
This keeps the cleanup scoped to representation layout and does not change the
open completion-carrier work.

Follow-up 2026-09-10 cleanup: exception construction/rendering helpers
(`exception_result_type`, `exception_try_ok_type`, `return_stmt`, `throw_stmt`,
`new_exception_expr*`, `exception_cause_or_self_if_instance_expr`, and
`exception_expr_from_new*`) are crate-local. Exact helper assertions moved into
`exception_lowering` unit tests; integration coverage stays on public program
lowering/rendered output and planning behavior. The Java completion-carrier gap
remains open.

Follow-up 2026-09-10 cleanup: `exception_plan::exception_class_name` is
crate-local. Direct carrier-name assertions live beside `exception_plan`;
integration coverage stays on throw/catch planning rather than importing the
helper as public API.

Follow-up 2026-09-10 cleanup: `ThrowExprPlan`, `plan_throw_expr`, and
`plan_block_exception_completion` are crate-local. Direct throw-expression shape
assertions live beside `exception_plan`; integration coverage stays on public
exception support and try/catch planning.

Follow-up 2026-09-10 cleanup: `catch_match_class_names` is crate-local too. The
semantic assignability regression moved beside `exception_plan`, while
integration coverage keeps checking the public try/catch plan and generated
exception-support behavior.

Follow-up 2026-09-10 cleanup: the unused conversion-diagnostic adapter
`lowering_diagnostic_from_conversion_diagnostic` is deleted. Integration
coverage stays on the public `collect_lowering_diagnostic_plans` boundary and
generated diagnostic codes.

Follow-up 2026-09-10 exception contract: generated `InputStream::read_3`
facades now check Java's `read(byte[], off, len)` range preconditions before
returning the placeholder EOF value. Negative `off`, negative `len`, and
`off + len` beyond the buffer length now return a catchable
`IndexOutOfBoundsException` instead of silently returning `Ok(-1)`. Regression:
`tests/differential/InputStreamReadBoundsCatch.java`; focused generated-Rust
coverage lives in
`idiom_gaps::resource_input_stream_read_and_close_have_typed_facade_support`.

Follow-up 2026-09-10 known-gap oracle, superseded 2026-09-11: the
`FinallySwitchBreak` fixture temporarily had a rustc `E0267` expectation for
`break` inside the general finally closure carrier. The 2026-09-11 promotion
below moved this fixture into the ordinary Java/Rust differential oracle after
the narrow switch-inside-loop shape started lowering through the simple
loop-finally fast path.

Follow-up 2026-09-10 focused finally/TWR oracle: the matching
try-with-resources/catch/finally `finally { break; }` override shape is now
promoted from an inline generated-crate check to the Java/Rust differential
oracle. `tests/differential/TryWithResourcesFinallyBreakOverridesCatchReturn.java`
pins cleanup-before-finally and the final break overriding the catch return
(`trcfd`).

Follow-up 2026-09-10 known-gap harness guard, superseded 2026-09-11: while
`FinallySwitchBreak.java` was a known gap, its expectation also required
generated Rust fragments for the general finally closure carrier (`|| -> Result`)
and the crossing `break;`. The promoted fixture no longer uses that
known-gap-only guard. The general completion-carrier model remains open.

Follow-up 2026-09-11 focused finally/switch promotion: the narrow
switch-inside-loop `try/finally` shape is no longer a known gap.
`simple_try_finally_loop_body_candidate` now accepts conservative switch bodies:
non-throwing selector/labels, switch-local bare `break`, and no `continue` or
labeled switch-case exit that would require the general completion carrier.
That lets the existing loop-finally fast path lower
`tests/differential/FinallySwitchBreak.java` without putting the outer loop
`break` inside the result closure. The fixture is promoted to the Java/Rust
differential oracle, is included in
`finally_completion_fixtures_match_java_output`, and still prints `zfofdf`.
General nested/labeled finally completion remains open.

Follow-up 2026-09-11 native static catchability: JDK static calls that had been
planned through emitted facade owners are retargeted back to native static
operations in both the generic expression path and the slot/external-call path.
This covers static-owner calls such as `Math.floorDiv`, `Math.floorMod`,
`Math.toIntExact`, and `Optional.empty/of/ofNullable`, plus associated facade
targets such as `Thread.sleep`. The important verification detail is that the
differential harness uses `target/release/new-java2rust`; rebuild release before
trusting the oracle. After rebuilding, `list_exception_fixtures_match_java_output`
passes.

Rechecked 2026-09-11 after the current dirty-tree audit cleanup:
`cargo build --release` followed by
`TMPDIR=$PWD/.tmp/differential-focus cargo test --release --quiet --test differential list_exception_fixtures_match_java_output -- --nocapture`
passed, covering the promoted `FinallySwitchBreak` and `ArrayBlockingQueue*`
fixtures in the release translator.

Follow-up cleanup narrowed the public fallibility-record surface:
`CallableFallibility` still has public read-only `returns_result()` and
`reasons()` accessors for callers that inspect `ProgramFallibilityPlan`, but its
stored boolean and reason vector are private. The fallibility solver remains the
writer for those fields, and layering now guards against re-exposing them as
mutable public payload. Guard:
`layering::exception_lowering_keeps_exception_paths_structured`.

Follow-up cleanup applied the same read-only boundary to the exception-plan
booleans. `JavaExceptionSupportPlan`, `MethodExceptionPlan`, and
`ProgramExceptionPlan` remain public plan records, but their support/result
flags are private-backed and exposed through accessors and constructors instead
of public mutable fields. Guard:
`layering::exception_lowering_keeps_exception_paths_structured`.

Follow-up cleanup narrowed generated `JavaException` runtime support too:
`cause` and `suppressed` are now private-backed. Generated code still attaches
suppressed exceptions through `add_suppressed`, reads them through
`get_suppressed`, and cause-chain tests use the new read-only `cause()` accessor.
Guard: `layering::exception_lowering_keeps_exception_paths_structured`.

Follow-up cleanup completed that generated-support fence for the remaining
runtime payload fields. `class_name` and `message` are now private-backed, while
generated harnesses and differential runners read them through `class_name()`
and `message()` accessors. The exception helper methods still own subtype,
display, and `getMessage()` behavior internally. Guard:
`layering::exception_lowering_keeps_exception_paths_structured`; a focused
generated-support guard now also pins the runtime accessors directly:
`exception_lowering::tests::java_exception_support_exposes_read_only_runtime_accessors`.

Follow-up 2026-09-12 focused loop-finally fix: the narrow loop-body
`try/finally` shape where the try body can break and the `finally` block
conditionally returns is no longer a loud unsupported statement. The simple
loop fast path now lowers the try body, prepends the `finally` body before
loop-abrupt statements, and keeps the appended `finally` body for fall-through
paths; a `return` inside the inserted `finally` naturally overrides the pending
`break`, matching Java for this bounded non-catch/no-resource shape. Covered by
`tests/idiom_gaps.rs::try_finally_conditional_return_over_break_runs_finally_return`
(`tftf:fin`). The broader completion-carrier work remains open for general
nested/labeled/throwing combinations.

Follow-up 2026-09-12 focused enhanced-for fail-fast fix: a bare local
collection enhanced-for loop that mutates the iterated collection in a
result-producing `try` context is no longer only a loud compile error. The
bounded lowering snapshots the collection length, iterates by index over cloned
elements to avoid a Rust borrow conflict, and raises a catchable
`ConcurrentModificationException` if the collection length changes after an
iteration body. Covered by
`tests/idiom_gaps.rs::enhanced_for_mutating_iterated_collection_throws_concurrent_modification`
(`cme`). Broader Java iterator fail-fast fidelity remains open for labeled,
non-result, non-local, and non-length-changing mutation shapes.

Follow-up 2026-09-12 enhanced-for fail-fast continue pass: the focused
fail-fast lowering no longer declines simple `continue` bodies. The synthetic
`while` loop now rewrites `continue` sites to run the generated length check
and index increment before continuing, so a mutation raises catchable
`ConcurrentModificationException` and a non-mutating continue advances normally.
Nested-loop continues still stay outside the bounded lowering. Covered by
`tests/idiom_gaps.rs::enhanced_for_mutating_iterated_collection_with_continue_throws_concurrent_modification`.

Follow-up 2026-09-12 focused unmodelled I/O facade fix: known Java I/O facades
that already advertise `Result<_, JavaException>` no longer panic in their
generated fallback bodies. `java.nio.file.Files.{newInputStream,newOutputStream,
newBufferedReader,readAllLines}`, `java.io.OutputStream.write*`, and
`java.io.Closeable.close()` now return a catchable `IOException` when the
facade is otherwise unmodelled, including the external-facts/source-summary
support path. Covered by
`tests/idiom_gaps.rs::java_nio_files_unmodelled_facade_throws_catchable_io_exception`,
`tests/idiom_gaps.rs::closeable_close_unmodelled_facade_throws_catchable_io_exception`,
and
`tests/idiom_gaps.rs::output_stream_write_unmodelled_facade_throws_catchable_io_exception`.

Follow-up 2026-09-12 focused future/runtime hierarchy fix: the generic
`Future<T>::get()` fallback no longer panics despite advertising
`Result<T, JavaException>`. It now returns a catchable `ExecutionException`,
and the generated runtime subtype table includes the common checked edges
`IOException -> Exception`, `InterruptedException -> Exception`,
`ExecutionException -> Exception`, and `Exception -> Throwable`, so broad
`catch (Exception)` / `catch (Throwable)` arms can catch these modeled Java
exceptions. Covered by
`tests/idiom_gaps.rs::selected_opaque_future_get_is_generic_instance_surface`,
`tests/idiom_gaps.rs::foreach_lambda_throw_wraps_enclosing_method_in_result`,
`tests/idiom_gaps.rs::catch_throwable_covers_runtime_exception_via_generated_hierarchy`,
and the `catch (Exception)` arm in
`tests/idiom_gaps.rs::java_nio_files_unmodelled_facade_throws_catchable_io_exception`.

Follow-up 2026-09-12 focused loop-finally throw fix: a loop-body
`try/finally` where the try body breaks and the `finally` block throws is no
longer rejected as unsupported. The loop fast path now accepts finalizers that
the exception planner proves terminate and whose statements are simple apart
from `throw`; inserting that finalizer before `break`/`continue` preserves the
Java override because the generated `Err(JavaException)` makes the loop-abrupt
statement unreachable. Covered by
`tests/idiom_gaps.rs::finally_throw_overrides_loop_break_and_is_catchable`.
Conditional finalizers with mixed break/return/throw paths still require the
general completion carrier.

Follow-up 2026-09-12 focused mixed-finalizer loop fix: the same simple
loop-body `try/finally` fast path now also accepts finalizers with conditional
`throw` and `return` arms plus fall-through. The lowered finalizer is inserted
before the pending loop exit, so `throw` and `return` override the break while
ordinary fall-through still performs the original loop exit. Covered by
`tests/idiom_gaps.rs::finally_mixed_return_throw_overrides_loop_break`.
More general nested/labeled/resource/catch combinations still require the
completion carrier.

Follow-up 2026-09-12 catch/finally optional-string display fix: reused optional
`String` locals in catch/finally display slots now use the Java string null
display projection rather than the generic optional-value formatter. This
avoids generating `value.java_arrays_deep_to_string()` on `&String` in
`System.out.println(t)` after `String t = xs.get(i)` inside a `finally` block.
Covered by `tests/idiom_gaps.rs::catch_and_finally_locals_receive_signature_plans`.

Follow-up 2026-09-12 focused fact-backed facade fallback fix: unmodelled
external facade methods that already have a `Result<_, JavaException>` ABI from
external effect facts no longer emit `unimplemented!`. They now return
`Err(JavaException::new(<declared thrown type>, ...))`, falling back to
`UnsupportedOperationException` only when no thrown fact is available. Covered
by
`tests/idiom_gaps.rs::unmodelled_throwing_external_facade_returns_catchable_exception`.

Follow-up 2026-09-12 focused boxed-primitive map-get unboxing fix: a ternary
branch that unboxes a nullable `HashMap<K, Integer>.get(k)` into an `int` slot
no longer renders an invalid Rust branch such as `values.get(k).as_ref()` next
to a primitive `0`. The map-get slot lowering now recognizes the nested
`Option<&Option<i32>>` carrier, projects through the present map entry and
present boxed value, and raises a catchable `NullPointerException` if either is
missing. Covered by
`tests/idiom_gaps.rs::map_get_in_ternary_primitive_slot_null_unwrap_is_catchable`
and the broader `cargo test --test idiom_gaps catchable` subset.

Follow-up 2026-09-12 focused source-summary fallback fix: source-summary
facade methods that have already been assigned a `Result<_, JavaException>` ABI
from effect facts no longer render `unimplemented!` inside that fallible
surface. They now return a catchable
`UnsupportedOperationException`, preserving loud untranslated-summary behavior
for non-fallible methods while letting Java `catch (Exception)` handle the
fallible fallback. Covered by
`tests/idiom_gaps.rs::source_summary_throwing_method_fallback_is_catchable`.

Follow-up 2026-09-12 focused source-summary-super compile fix: inherited or
explicit `super.*` calls whose superclass lives only in source-summary context
no longer lower by constructing or statically calling the un-emitted summary
owner, e.g. `SummaryBase::label(&SummaryBase {})`. Inherited methods from
source-summary supertypes are emitted as untranslated summary surfaces, and
explicit fallible `super.*` calls return a catchable
`UnsupportedOperationException` rather than silently succeeding. Covered by
`tests/idiom_gaps.rs::source_summary_super_calls_lower_to_boundary_defaults_not_raw_super_receiver`.

Follow-up 2026-09-12 focused `List.get` slot-catchability fix: slot-specific
sequence-get lowering no longer uses the panic-only projection inside fallible
`try` lowering. `List.get` into a concrete local now uses the existing
`IndexOutOfBoundsException` result projection before slot coercion, so Java
`catch (IndexOutOfBoundsException)` can observe the failure instead of the
generated Rust aborting through `panic!("uncaught Java exception")`. Covered by
`tests/idiom_gaps.rs::sequence_get_in_try_slot_coercion_is_catchable`.

Follow-up 2026-09-12 focused source-summary-super return fix: `return
super.value()` for a throwing source-summary superclass method now returns the
fallback `Err(JavaException)` directly. The previous generic return path could
render the fallback as an already-propagated value under an outer `Ok(...)`,
which was noisy and brittle around source-summary boundary calls. Covered by
`tests/idiom_gaps.rs::source_summary_super_returning_throwing_method_is_not_double_wrapped`.

Follow-up 2026-09-12 focused pending-return/finally fixes: a simple loop-body
`try/finally` whose try body evaluates a direct `return` and whose `finally`
can conditionally break now saves the pending return value, runs the finalizer,
and returns the saved value only if the finalizer falls through. This fixes the
labeled-break shape `run(true) == "TFA"` / `run(false) == "TR"` without using
the old closure-crossing lowering. Covered by
`tests/idiom_gaps.rs::try_finally_conditional_labeled_break_over_return_preserves_fallthrough_return`.

Follow-up 2026-09-12 focused TWR catch/finally pending-return fix:
try-with-resources/catch/finally shapes whose try path falls through but whose
catch path returns now carry `Option<return>` through the catch/finally scope.
Resource cleanup and `finally` still run first, and a finalizer throw overrides
the pending catch return. This fixes an invalid generated Rust shape where the
catch arm returned `Ok(String)` inside a `Result<(), JavaException>` scope.
Covered by
`tests/idiom_gaps.rs::try_with_resources_finally_throw_overrides_catch_return_after_cleanup`.
The broader completion-carrier gap remains open for arbitrary nested,
labeled/resource/catch combinations.

Follow-up 2026-09-12 focused loop catch/finally pending-return fix: a loop-body
plain `try/catch/finally` whose catch computes a pending return and whose
`finally` conditionally breaks now uses the same `Option<return>` carrier in
loop-body context. The finalizer break wins when it runs; otherwise the pending
catch return resumes. Covered by
`tests/idiom_gaps.rs::try_catch_finally_conditional_break_over_catch_return_preserves_fallthrough_return`
(`b:r`).

Follow-up 2026-09-12 focused TWR finally-return-over-break fix:
try-with-resources/finally loop-body lowering now accepts the existing simple
finalizer predicate instead of only fall-through finalizers. This lets resource
cleanup and `finally` run before a pending body `break`, with a finalizer
`return` overriding the break and fall-through preserving it. Covered by
`tests/idiom_gaps.rs::try_with_resources_finally_return_overrides_body_break_after_cleanup`
(`R:A`). More general conditional TWR catch-return/labeled-continue carrier
shapes remain open.

Follow-up 2026-09-12 focused TWR catch-return carrier expansion: loop-body
try-with-resources/catch/finally now supports the pending catch-return carrier
when resource cleanup must run before a conditional finalizer `break`,
`continue`, labeled `continue`, or `throw`. The cleanup result is combined with
the guarded body before `finally` executes, and the saved catch return resumes
only when the finalizer falls through. Covered by
`tests/idiom_gaps.rs::try_with_resources_finally_break_overrides_catch_return_after_cleanup_when_conditional`
(`A:R`),
`tests/idiom_gaps.rs::try_with_resources_finally_throw_overrides_body_continue_after_cleanup`
(`E:A`), and
`tests/idiom_gaps.rs::try_with_resources_finally_labeled_continue_overrides_catch_return_after_cleanup`
(`R:R`).

Follow-up 2026-09-12 focused mixed plain catch/finally override fix: loop-body
plain `try/catch/finally` now handles the bounded shape where the guarded body
throws, a catch performs side effects and a pending `break`, and the finalizer
then performs side effects before always completing abruptly through a
conditional `throw` or fall-through `return`. The catch break is discarded
because Java finalizer completion overrides it. Covered by
`tests/idiom_gaps.rs::try_catch_finally_mixed_throw_return_overrides_catch_break`
(`tcf:caught`).

Follow-up 2026-09-12 focused URL constructor catchability pass: `new
URL(value)` now uses the stdlib constructor-result path and lowers through
`URL::new_result` in fallible contexts, so malformed specs produce a catchable
`MalformedURLException`/`IOException` instead of constructing a value
silently. Covered by
`tests/semantic_facts.rs::url_constructor_fact_uses_result_for_malformed_url_exception`
and `tests/idiom_gaps.rs::url_constructor_malformed_spec_is_catchable`.

Follow-up 2026-09-12 focused File/String IO constructor slot pass:
`new FileInputStream(file)`, `new FileReader(file)`, and
`new FileWriter(file)` now use the same actual-argument constructor slot fact
as `FileOutputStream`, so nullable `File` fields are unwrapped through the Java
null-receiver path instead of being passed as `Option<File>` to an untyped
facade constructor. The String path remains a direct String slot rather than
being coerced through `File::new(path)`. Covered by
`tests/idiom_gaps.rs::file_input_stream_constructor_selects_file_and_string_overload_slots`,
`tests/idiom_gaps.rs::file_reader_constructor_selects_file_and_string_overload_slots`,
and `tests/idiom_gaps.rs::file_writer_constructor_selects_file_and_string_overload_slots`;
the existing `file_output_stream_constructor_selects_file_and_string_overload_slots`
fixture remains green.

Follow-up 2026-09-13 focused PrintStream constructor overload slot pass:
`new PrintStream(file)`, `new PrintStream(path)`, and
`new PrintStream(out)` now use the actual argument type to select the constructor
slot. This keeps `File` nullable-field checks catchable, keeps `String` paths
from being coerced through `File::new(path)`, and avoids the previous ambiguous
`out.into()` call when the support constructor accepts `OutputStream` directly.
Covered by
`tests/idiom_gaps.rs::print_stream_output_stream_constructor_uses_actual_overload_slot`
and
`tests/idiom_gaps.rs::print_stream_constructor_selects_file_string_and_output_stream_slots`.

Follow-up 2026-09-13 focused Appendable facade argument compile pass:
`Appendable.append(...)` is still modeled as a fluent call, but its generated
facade method takes `&self` and clones the marker value. The built-in external
fact now records that receiver as read-only in Rust receiver terms, matching
the existing `Writer.append` behavior, so a source helper accepting an
`Appendable` no longer forces callers to pass `&mut Appendable` from a shared
field. Concrete-facade borrow blocks also recover their owned marker before a
later mutable borrow projection, avoiding `&mut { ...; &Facade }` if such a
slot is genuinely requested. Covered by
`tests/idiom_gaps.rs::appendable_field_conversion_clones_before_into`,
`tests/idiom_gaps.rs::appendable_fluent_chain_keeps_receiver_and_to_string_type`,
`tests/idiom_gaps.rs::caught_checked_exception_rethrown_as_known_unchecked_propagates_result`,
and
`tests/semantic_facts.rs::external_logger_style_side_effects_do_not_require_mutable_receiver`.

Follow-up 2026-09-13 focused CountDownLatch catchability pass:
`CountDownLatch.await()` now has a built-in concurrent-platform effect fact
advertising `InterruptedException` and a result-returning facade ABI. The
existing raw-identifier lowering still emits `r#await`, but propagated Java
`try` contexts now call `latch.r#await()?` so
`catch (InterruptedException)` is reachable instead of silently treating
`await()` as infallible. Covered by
`tests/idiom_gaps.rs::external_facade_keyword_method_names_render_as_raw_identifiers`
and
`tests/external_packs.rs::java_platform_concurrent_static_factory_declares_precise_return_type`.

Follow-up 2026-09-13 focused Thread.sleep catchability pass:
`Thread.sleep(long)` and `Thread.sleep(long,int)` already lowered through the
runtime result helper in fallible contexts, but the platform effect facts did
not advertise their checked `InterruptedException`. Both overloads now have
built-in side-effect facts, so propagated Java `try` contexts keep the helper's
`?` and make `catch (InterruptedException)` reachable instead of treating the
call as an infallible sleep. The generated facade support also normalizes the
two-argument overload to `sleep_2`, avoiding the duplicate Rust `sleep` method
that surfaced when both facts were enabled. Covered by
`tests/idiom_gaps.rs::thread_sleep_interrupted_exception_is_catchable` and the
Thread assertions in
`tests/external_packs.rs::java_platform_concurrent_static_factory_declares_precise_return_type`.

Follow-up 2026-09-13 focused ArrayBlockingQueue.put catchability pass:
`ArrayBlockingQueue.put(value)` no longer emits a compile-time unsupported
diagnostic. The native collection planner has a supported `ArrayBlockingQueuePut`
target, propagated Java `try` contexts lower through a
`Result<(), JavaException>` projection, and the bounded carrier pushes while
capacity remains. A full queue cannot block in the translated single-threaded
runtime, so it returns a catchable `InterruptedException` instead of becoming a
Rust panic or hard `compile_error!`. Covered by
`tests/idiom_gaps.rs::array_blocking_queue_put_pushes_or_returns_interrupted_exception`
and promoted Java-oracle fixture `tests/differential/ArrayBlockingQueuePut.java`
for the non-blocking success path. The adjacent `ArrayBlockingQueue` add,
offer, bad-capacity, and null-element idiom guards now run generated assertions
instead of stopping at `cargo_check`, so the catchability paths are pinned both
locally and in the differential exception loop. Planner guard:
`call_plan::tests::native_queue_receiver_kind_is_the_single_queue_capability_answer`.

Follow-up 2026-09-13 focused switch/finally fallthrough pass:
`plan_block_exception_completion` no longer treats every `switch` as a
terminating exception boundary. A switch only terminates the enclosing
try/finally body when it has a default label and every possible fallthrough path
reaches `return`, `throw`, or another terminating exception boundary before a
`break`. Plain `try/finally` bodies that can both return a value and fall
through now use the same pending optional-return carrier pattern already used by
catch/finally fallthrough cases, so a `switch` case `break` runs `finally` and
continues after the try while a default `return` preserves Java's return value.
Covered by
`tests/idiom_gaps.rs::try_finally_switch_break_falls_through_after_finally`
and Java-oracle fixture `tests/differential/FinallySwitchFallsThrough.java`.
