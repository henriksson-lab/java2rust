# 0059 — String concatenation erases reference values; ordered collections lose their contract

Recorded 2026-08-23.
Status: **open, concrete repros mitigated**.
Severity: **medium/high** — the original broad concat, collection-order,
collection-mutation, enum-body, modeled display, and internal collection-carrier
divergences are now covered by differential or generated-crate fixtures. The
remaining semantic risk is narrower but still real: an arbitrary external value
erased behind `Object` cannot recover Java dynamic `toString()` unless source
downcast evidence, platform support, or an explicit external display-capability
fact is available.
Kind: behavioural (differential), JDK facades / collections / enums.

Follow-up 2026-09-09 focused triage: rechecked this audit together with 0056.
The collection-order, queue, map-view, enum, source-object, platform-object, and
modeled-external display repros are now covered by focused fixtures. The
remaining code area is `src/stdlib.rs::java_erased_any_to_string`'s
unknown-erased fallback/model boundary. The source/platform/external downcast
collector arms are centralized through `erased_display_to_string_downcast_arms`
and guarded for parity; the open path is what to do when no source/platform/fact
evidence exists. Current guards intentionally keep that case loud rather than
guessing. A low-risk next patch would need a concrete missing carrier fixture;
otherwise the real remaining fix is a typed-erasure/display-capability model,
not another broad fallback.

Follow-up 2026-09-09 compile-harness cleanup: three compile-only generated-crate
stubs for `java_erased_object_to_string` now panic loudly instead of returning
`String::new()`. This does not solve arbitrary erased external display, but it
keeps package-identity compile fixtures from carrying a plausible empty-string
placeholder for the exact display defect this audit is tracking.

Follow-up 2026-09-09 queue capability centralization: the native collection
planner no longer asks separate boolean questions for "is queue", "is priority
queue", "is array blocking queue", and "does offer always insert". Those paths
now share `NativeQueueReceiverKind`, so `offer`, queue head reads/removals,
bounded-queue unsupported lowering, and comparator-backed `PriorityQueue`
constructor detection consume the same receiver classification. The planner also
now emits `NativeCollectionMethod::PriorityQueueAdd` directly, so lowering no
longer re-derives receiver type identity to decide whether insertion must
restore natural order. Guard:
`call_plan::tests::native_queue_receiver_kind_is_the_single_queue_capability_answer`.

Codex follow-up 2026-09-13 `ArrayBlockingQueue.put` bounded-carrier pass:
`put(value)` no longer reuses generic `List.add`/`push` lowering and no longer
emits a hard `compile_error!`. The planner routes it to a dedicated native
collection method. Lowering uses the bounded `JavaArrayBlockingQueue<T>`
carrier: a non-null value is pushed while `len() < capacity()`, literal null
raises `NullPointerException`, and a full queue returns a catchable
`InterruptedException` rather than blocking the translated single-threaded
runtime. This is still not a full Java blocking-queue implementation, but it
keeps the checked-exception surface usable and preserves the already-modeled
`add`/`offer` capacity behavior. Guards:
`call_plan::tests::native_queue_receiver_kind_is_the_single_queue_capability_answer`
and
`idiom_gaps::array_blocking_queue_put_pushes_or_returns_interrupted_exception`.
Differential fixture `tests/differential/ArrayBlockingQueuePut.java` covers the
ordinary non-blocking success path against the Java oracle.

Follow-up 2026-09-07 `String.join` null delimiter pass: the focused
String.join null-display oracle now covers both null elements and a null
delimiter. The delimiter keeps Java's required non-null contract and raises
catchable `NullPointerException` instead of becoming an empty Rust delimiter.
Differential fixture: `tests/differential/StringJoinNullDelimiter.java`.

Follow-up 2026-09-09 `ArrayDeque` queue-alias pass: queue-like
`ArrayDeque.offer(value)` now reuses the existing native add path instead of
falling through to an unmodelled facade call, and `getFirst()`, `removeFirst()`,
and `pop()` reuse the existing head-read/head-removal paths. Differential
fixtures: `tests/differential/ArrayDequeOffer.java` (`true,true:ab:true`) and
`tests/differential/ArrayDequeFirstPopCatch.java` (`aab:caught`).

Follow-up 2026-09-09 `LinkedList` queue-alias pass: `LinkedList` now shares
the queue receiver classification already used for `Deque`, `ArrayDeque`, and
`PriorityQueue`, so `offer(value)` and its head read/removal methods do not fall
through to unmodelled facade calls. Differential fixture:
`tests/differential/LinkedListQueueHeadCatch.java` (`true,true:aabb:caught`).

Follow-up 2026-09-11 bounded `ArrayBlockingQueue` carrier: `ArrayBlockingQueue`
slots now render as `JavaArrayBlockingQueue<T>` rather than plain `Vec<T>`, and
the capacity constructor stores the Java capacity separately from Rust's
allocator capacity. `offer(value)` inserts while `len() < capacity()` and
returns `false` when full; `add(value)` uses the same bounded check and raises
`IllegalStateException` when full. The non-throwing `peek()` queue-head method
now lowers to `get(0).cloned()`, preserving Java's null-on-empty contract.
Focused generated-crate fixtures:
`idiom_gaps::array_blocking_queue_offer_uses_bounded_carrier_and_returns_false_when_full`
and
`idiom_gaps::array_blocking_queue_add_uses_bounded_carrier_and_throws_when_full`.
Differential fixtures promoted:
`tests/differential/ArrayBlockingQueueOffer.java` (`true,false,a`) and
`tests/differential/ArrayBlockingQueueAdd.java`
(`true,a,IllegalStateException`). Custom-comparator `PriorityQueue` instances
still need a real priority-queue carrier.

Follow-up 2026-09-09 comparator-backed priority queue constructor correction:
`new PriorityQueue<>(comparator)` now fails loudly with `compile_error!` instead
of letting the one-argument native collection constructor fallback return the
comparator expression as if it were the queue carrier. Focused generated-crate
fixture:
`idiom_gaps::priority_queue_comparator_constructor_requires_comparator_carrier`.
Known-gap differential fixture:
`tests/differential/known-gaps/PriorityQueueComparatorConstructor.java`
(`3,2,1`).
Comparator-backed queue ordering still needs a real carrier before it can be
lowered correctly; the checked-in anonymous-comparator shape also reports
`unsupported_anonymous_class`, so promoting that fixture needs both anonymous
comparator object lowering and a priority-queue comparator carrier. Guard:
`idiom_gaps::priority_queue_anonymous_comparator_constructor_reports_both_missing_carriers`.
Follow-up fixture split 2026-09-12:
`tests/differential/known-gaps/PriorityQueueNamedComparatorConstructor.java`
uses a named `Comparator` implementation and expects the same Java result and
the same loud `unsupported_priority_queue_comparator_constructor` diagnostic,
without depending on anonymous-class lowering. This isolates the carrier work
from the older combined fixture. The constructor planner now recognizes both a
literal `Comparator` argument type and source types that reach `Comparator`
through hierarchy facts, so named comparator implementations no longer fall
through to the natural-order `Vec` constructor path.

Follow-up 2026-09-09 `PriorityQueue.add`/`offer` natural-order pass: the native
`Vec<T>` carrier now sorts after `add(value)` and `offer(value)`, so `element()`
and `remove()` see the Java natural-order head instead of FIFO insertion order
for comparable values. Differential fixtures:
`tests/differential/PriorityQueueHeadOrder.java` and
`tests/differential/PriorityQueueOfferHeadOrder.java` (`true:true:1:1:5`).

Codex follow-up 2026-09-12 queue `poll()` pass: queue-like native collection
receivers now classify `poll()`/`pollFirst()` separately from throwing
`remove()`. Lowering emits `None` for an empty receiver and `Some(remove(0))`
otherwise, preserving Java's null-on-empty contract while still mutating the
queue. Focused generated-crate fixture:
`idiom_gaps::queue_poll_removes_head_and_returns_null_on_empty`. The companion
fixture `idiom_gaps::queue_poll_aliases_remove_head_and_return_null_on_empty`
pins the same lowering for `ArrayDeque.pollFirst()` and `ArrayBlockingQueue.poll()`.
Differential fixtures: `tests/differential/QueuePollNull.java` (`a:null:true`)
and `tests/differential/QueuePollAliases.java` (`a:null:b:null:c:null`).
Comparator-backed `PriorityQueue` remains open: the checked-in known-gap fixture
also needs anonymous comparator lowering and a real comparator carrier.

Follow-up 2026-09-09 focused map-view contract oracle: the dedicated
`map_view_contract_fixtures_match_java_output` loop now groups existing
map-view, entry-set, and map-entry display fixtures
so the promoted 0059 collection contracts can be checked without running the
entire differential suite. This is a measurement-floor change only; the fixtures
already existed in the broad suite.
Recheck correction: the focused compile recheck exposed two existing generated
Rust type errors before this loop could be trusted: set `addAll` initialized a
`HashSet` slot from a `Vec`, and entry-set `contains` with a parameter-side
`SimpleEntry` rendered a facade placeholder instead of borrowing the tuple entry
carrier. Both are now fixed, and `CollectionAddAllReturn` plus
`MapEntrySetContainsParam` are included in the focused loop.
Follow-up: the same focused loop now also covers bulk live-view contracts for
`Map.keySet().containsAll/retainAll`, `Map.values().containsAll/retainAll`, and
`Map.entrySet().containsAll/retainAll/removeAll` through
`tests/differential/MapKeySetBulkContract.java`,
`tests/differential/MapValuesBulkContract.java`, and
`tests/differential/MapEntrySetBulkContract.java`. These were already
unit-compiled paths; the differential fixtures pin the Java-visible map
mutation contract.

Follow-up 2026-09-09 fixed-size view `clear` pass: `Arrays.asList(...).clear()`
is now classified with the rest of the fixed-size structural mutations and
raises `UnsupportedOperationException` instead of clearing the Rust carrier.
The existing `UnmodifiableCollectionMutationCatch` differential fixture now
covers this alongside fixed-size `add`, unmodifiable collection/list/set/map
mutation, and aliased unmodifiable-list mutation.

Follow-up 2026-09-09 immutable collection factory pass:
`Collections.empty*`, `Collections.singleton*`, and `Collections.nCopies`
locals/direct receivers now keep immutable-view provenance for structural
mutation lowering. Actual structural changes raise `UnsupportedOperationException`;
empty-view `clear()` remains the Java no-op.

Follow-up 2026-09-07 erased native collection display pass: erased values
holding the concrete collection carriers emitted for `ArrayList<String>`,
`LinkedHashSet<String>`, and `LinkedHashMap<String,Integer>` now display through
the same Java collection/map formatting used by typed concat paths. Promoted
differential fixtures: `tests/differential/ErasedArrayListDisplay.java`,
`tests/differential/ErasedLinkedHashSetDisplay.java`, and
`tests/differential/ErasedLinkedHashMapDisplay.java`. The broad unknown
external-erased display risk remains open.

Follow-up 2026-09-09 nullable erased `ArrayList<String>` display pass:
`Vec<Option<String>>` erased-object values now render present elements normally
and `None` as Java `null`. Pinned by
`tests/differential/ErasedNullableArrayListDisplay.java` in the focused
`erased_display_fixtures_match_java_output` loop. This is a concrete collection
carrier arm, not a broad unknown-erased fallback.

Follow-up 2026-09-07 erased `BitSet` display pass: `BitSet` was already a
modeled platform facade for `get`/`set`, but erased display did not include it.
`BitSet` now formats as Java does (`{2, 5}` for set bits), and
`Object value = bits; "" + value` plus `value.toString()` are differential
covered by `tests/differential/ErasedBitSetDisplay.java`.
The focused erased-platform differential loop now includes the promoted
BitSet/Class/Locale/Pattern fixtures instead of leaving them to ad hoc
single-fixture runs.

Follow-up 2026-09-07 erased `Class` display pass: `Class` literals erased
behind `Object` now recover Java display through generated `JavaClass<T>`
downcast arms for the concrete token types present in the program. This covers
both string concatenation and direct erased `value.toString()` for
`Object value = String.class`, while genuinely unknown erased display remains
loud. Promoted differential fixture:
`tests/differential/ErasedClassDisplay.java`. The same pass made class-token
support detection scan static initializer blocks, so a `String.class` used only
inside `static { ... }` no longer emits `JavaClass::<String>` without the
`JavaClass<T>` support item.

Follow-up 2026-09-07 display-capability cleanup: the external facade
display-capability set had a no-op simple-name branch that reinserted the same
aliases already added for non-package-qualified receiver facts. It was deleted
rather than inverted: package-qualified `toString` facts must keep package peers
distinct and must not mark the bare simple peer display-capable. Existing
regressions:
`stdlib::tests::package_qualified_to_string_fact_does_not_mark_simple_peer_display_capable`
and
`stdlib::tests::external_display_capability_query_keeps_package_peers_distinct`.

Follow-up 2026-09-07 nullable display-capability cleanup: a modeled external
`toString(): String` fact now enables `Display` only when its return nullability
is `DefinitelyNonNull`. Nullable external `toString` facts still generate
nullable facade methods when called directly, but erased/concat display stays
loud until there is a non-null display contract. Regression:
`slot_shape_design::nullable_external_to_string_fact_does_not_mark_facade_display_capable`.

Follow-up 2026-09-06: `StringBuilder.append(map.get(k))` no longer fails on the
`Option<&&String>` support shape from borrowed map reads. The generated runtime
support now accepts `&&T` where `T: JavaAppendValue + Clone`, so optional
borrowed values can flow through the existing `Option<T>` append contract.
Regression:
`tests/idiom_gaps.rs::string_builder_append_map_get_borrowed_string_value_typechecks`.

Follow-up 2026-09-06 stale-baseline pass: four collection/view rows were still
listed as idiom-gap failures even though current lowering compiles them. The
assertions were updated to the current `Result`/`Ok` rendering and direct native
projections, then the release baseline was recaptured at `707/109`. Removed
from `tests/baselines/idiom-gaps-failures.txt`:
`list_remove_object_removes_first_equal_element_and_returns_boolean`,
`map_key_set_size_lowers_through_native_len`,
`map_values_returning_collection_collects_vec`, and
`map_values_returning_set_collects_hashset_for_set_slot`. The adjacent
`map_values_returning_hashset_collects_hashset_for_concrete_set_slot` was then
closed with a slot-aware constructor path: `new LinkedHashSet<String>(
values.values())` keeps its insertion-ordered carrier in ordinary value
contexts, but lowers directly to a `HashSet` snapshot when the surrounding slot
asks for `HashSet<String>`.

Follow-up 2026-09-06 second stale-baseline pass: six more 0059 rows were
verified passing and removed from `tests/baselines/idiom-gaps-failures.txt`.
The assertions now accept current nullable/checked array projections,
`&mut Vec<String>` `removeIf` lowering, narrowed `Map.get(...).add(...)`
negative checks, and `Result`-wrapped explicit source `toString()` returns:
`arrays_as_list_single_string_array_spreads_for_add_all`,
`documents_collection_remove_if_lambda_gap`,
`map_get_collection_mutation_uses_mutable_lookup`,
`explicit_source_to_string_method_wins_over_object_fallback`,
`nested_indexed_string_array_append_borrows_inner_vec_element`, and
`string_buffer_append_ternary_argument_stays_owned_value`. At that checkpoint,
release `idiom_gaps` was `718/98`; the current checked baseline is `816/0`.
The remaining live 0059 rows then included the source-object `toString`
fallback pair and enum-values set carrier mismatch.

Follow-up 2026-09-07 source-object fallback pass: current-program source types
without a declared `toString()` now lower direct calls through their generated
Java `Object.toString()` fallback method instead of detouring through erased
display. The path stays scoped to `ctx.program`; support/emitted-context and
source-summary types still use erased display because their concrete fallback
type may not exist in the generated crate. Removed from the idiom-gap baseline:
`source_object_to_string_uses_java_object_stringification_fallback` and
`source_object_to_string_fallback_applies_across_source_classes`. At that
checkpoint, release `idiom_gaps` was `720/96`.

Follow-up 2026-09-07 easy-row pass: eight more checked-in 0059 rows are closed.
Two were stale assertions for current `Result` and nested-enum spelling
(`documents_class_literal_token_gap`, `enum_compare_to_uses_ordinal_order`).
Four Java array helper rows only needed generated-test fixtures to use nullable
`String[]` leaves; the adjacent `Arrays.toString(String[])` row also needed the
runtime helper to format nullable object elements as `null`. The remaining real
lowering fix was `new LinkedHashSet<Mode>(Arrays.asList(Mode.values()))` in a
`Set<Mode>` slot, which now snapshots straight to the requested native
`HashSet`. Removed from the idiom-gap baseline:
`java_arrays_hash_code_lowers_as_static_platform_hash_projection`,
`java_arrays_to_string_lowers_as_static_platform_format_projection`,
`java_arrays_deep_to_string_lowers_nested_arrays`,
`java_arrays_deep_equals_lowers_nested_arrays`,
`java_arrays_deep_hash_code_lowers_nested_arrays`, and
`source_enum_values_spread_into_arrays_as_list_and_compare_borrowed_loop_items`.
At that checkpoint, release `idiom_gaps` was `728/88`.

Follow-up 2026-09-07 equality/assertion cleanup: `!list.get(i).equals(value)`
now respects the non-null receiver produced by catchable `List.get` lowering
instead of treating the propagated receiver as `Option<String>` and emitting an
ambiguous `.as_ref().map(...) != Some(value)`. Six more rows were removed from
the idiom-gap baseline after focused verification: numeric char literal slots,
char-array `String` construction, whole-expression array indices, shift
promotion constant paths, nested enum type access spelling, and static array
length constants.
At that checkpoint, release `idiom_gaps` was `735/81`.

Follow-up 2026-09-07 stale assertion cleanup: two more rows were verified as
current generated-crate passes after assertion updates. Optional array field
length now renders through the checked nullable array receiver and `.as_slice()`
before `len()`, and static primitive expression constants use explicit owner
paths in generated static methods. Removed
`optional_array_field_length_unwraps_on_non_self_receiver` and
`source_static_final_primitive_expression_fields_emit_associated_constants` from
the idiom-gap baseline. The adjacent cross-owner static constant row also proved
stale and now accepts explicit owner paths in static method bodies.
At that checkpoint, release `idiom_gaps` was `738/78`.

Follow-up 2026-09-07 tokenizer/static-key stale cleanup: two more assertion-only
rows now match current generated Rust. `StringTokenizer.nextToken().charAt(0)`
uses the owned `String` returned by `next_token_result()` directly, and
`Map.get(ID)` on a static string constant uses the explicit owner path without
introducing an extra borrow. Removed
`java_string_tokenizer_uses_facade_signature_facts` and
`map_get_static_string_constant_key_does_not_double_borrow_str` from the
idiom-gap baseline.
At that checkpoint, release `idiom_gaps` was `740/76`.

Follow-up 2026-09-07 equals/Objects stale cleanup: `EqualsAndObjects` now
renders direct string equality as `Ok(left == right)` for the fallible method
and `left == right` for the null-safe static helper. The old `.as_str()`
assertion was stale; the generated crate passes. Removed
`documents_equals_and_objects_gap` from the idiom-gap baseline.
At that checkpoint, release `idiom_gaps` was `742/74` after the adjacent stale
`ZipOutputStream -> OutputStream` guardrail row was converted to a positive
generated-crate check under 0058.

Follow-up 2026-09-06 API-surface cleanup: collection `removeIf` and `addAll`
lowering helpers in `call_lowering` are crate-local, and the `firstKey`,
`lastKey`, `remove`, and `element` projection helpers that are only used inside
that module are private. This does not change collection semantics; it fences
the remaining collection lowering surface behind the call-target lowering API.
The catchable-result projections for list, queue, and map endpoint operations
are crate-local as well.

Follow-up 2026-08-30 entry-set `removeAll` pass: `Map.entrySet().removeAll(...)`
now mutates the live backing map instead of falling through to an unmodelled
collection call. The lowering mirrors the existing `retainAll` backing-map
path with the predicate inverted, and handles self-aliases such as
`entries.removeAll(entries)` by clearing the backing map and returning whether
it was non-empty. Regressions:
`lower::tests::map_entry_set_alias_remove_all_mutates_backing_map_entries`,
`lower::tests::map_entry_set_alias_remove_all_self_alias_clears_backing_map`,
and `tests/idiom_gaps.rs::map_entry_set_bulk_methods_use_backing_map_contract`.

Follow-up 2026-08-30 map-view `toArray` pass: `Map.values().toArray(...)` now
uses the backing-map value projection directly instead of relying on the
`keySet()`-only collection-view classifier and falling through to generic
method lowering. The existing `keySet().toArray(...)` path was moved to the
same direct backing-map projection, so both key and value views preserve the
Java array carrier for reference elements without materializing a view and then
iterating it again. `Map.entrySet().toArray()` now has a slot-aware erased
array path for `Object[]` targets, boxing each fresh `(key, value)` tuple as
`Box<dyn Any>` while preserving the typed `Map.Entry[]` expression path. This
also guards against the invalid `values.iter().iter()` shape. Regressions:
`idiom_gaps::collection_to_array_empty_string_array_lowers_to_owned_vec` and
`lower::tests::map_entry_set_to_array_uses_backing_map_without_double_iterating_view`.

Follow-up 2026-08-29 entry-set unsupported mutation pass:
`Map.entrySet().add(...)` and `Map.entrySet().addAll(...)` now follow Java's
live-view contract by raising `UnsupportedOperationException` instead of
falling through to an unmodelled collection call. The lowering mirrors the
existing `keySet`/`values` unsupported mutation paths and evaluates the backing
map plus arguments before returning the Java exception result. Promoted
differential fixtures:
`tests/differential/MapEntrySetAddUnsupported.java` and
`tests/differential/MapEntrySetAddAllUnsupported.java`.

Follow-up 2026-08-29 baseline pass: six collection/view/value-carrier rows were
verified passing and removed from `tests/baselines/idiom-gaps-failures.txt`.
The current tree now covers borrowed array slice argument views, `Map.put/get`
API shape, optional sequence `get` unwrapping into concrete slots,
`SimpleInterval` list element adaptation to `Locatable`, boxed dyn trait
collection elements, and `TwoBitReference` reference-sequence getter shape.
Verified:
`array_arguments_use_borrowed_slice_views`,
`documents_map_put_get_java_api_gap`,
`optional_sequence_get_unwraps_receiver_for_concrete_slot`,
`sequence_get_adapts_simple_interval_to_locatable_slot`,
`source_interface_collection_elements_use_boxed_dyn_trait`, and
`two_bit_reference_get_reference_bases_returns_reference_sequence`. The audit
remains open for genuinely unknown erased external display and opaque
`Map.Entry` values without recoverable key/value or backing-map identity.

Updated 2026-08-28: native map carriers now have an explicit Java display
coercion for string concatenation/display slots. `HashMap`, `BTreeMap`, and the
insertion-ordered `JavaLinkedHashMap` path render through `java_map_to_string`
over borrowed entries, producing Java map syntax (`{key=value, ...}`) without
moving the map. Regression:
`tests/idiom_gaps.rs::tree_map_string_concat_uses_java_map_display`; the helper
emission detector is covered by
`stdlib::java_string_runtime_support_tests::map_in_string_concat_emits_collection_display_support`.
Unknown erased external display remains open; later collection-view passes
closed the live-view mutation residue tracked here.

Updated 2026-08-26 again: modeled external `toString(): String` display support
is broader than the single `VCFHeader` case. VCF header-line classes and
`htsjdk.samtools.util.Interval` now use an explicit zero-arity typed
`toString` fact helper, so erased-object display can emit compilable downcast
arms and facade `Display` impls for those simple generated facades. Unknown
external erased display remains loud/open when no explicit display-capability
fact or runtime downcast evidence exists.

Updated 2026-08-26: erased platform display now includes `java.net.URI` in the
same downcast path as `File`, `StringBuilder`, and `StringBuffer`.
Regressions: `slot_shape_design::erased_uri_object_display_delegates_to_uri_display`
and `tests/differential/ErasedUriDisplay.java`.

Updated 2026-08-24: §1's concrete concat repro is promoted to
`tests/differential/ConcatReferenceValues.java` and now passes the Java oracle.
The current lowering renders the boxed primitive, enum, source object,
`List<Integer>` element, `List<String>` element, and `Map.get` result with
native `to_string`/string paths instead of `java_erased_object_to_string`.
While promoting it, the differential suite exposed two regressions outside this
audit and both were fixed: `.equals()` inside concat/display slots now keeps the
`==` versus custom-`equals` distinction, and `Iface.super.m()` resolves
lowercased/static-aliased interface owners before falling through to static
owner lowering. `cargo test --test differential -- --nocapture` now passes
25/25 fixtures.

Updated 2026-08-24 again: §3's `Map.put` previous-value repro is promoted to
`tests/differential/MapPutPrevious.java` and now passes the Java oracle
(`null,1`). Native collection calls now contribute their planned return shape to
local declaration inference, and erased precomputed local plans can inherit an
optional initializer when the optional element matches the declared Java type.
The generated locals are `Option<i32>` and keep `HashMap::insert`'s `None`
instead of unwrapping it. `cargo test --test differential -- --nocapture` now
passes 26/26 fixtures.

Updated 2026-08-24 third pass: `Map.remove(k)` is now a native collection
method, borrows the key argument, mutates the receiver, and returns `Option<V>`.
`tests/differential/MapRemovePrevious.java` is promoted and matches the Java
oracle (`null,1,false`). Later follow-ups below supersede the then-open ordered
collection, set-construction, and enum-body sections; the remaining live risk is
the unknown erased-display residue summarized at the current-status section.

Updated 2026-08-24 fourth pass: §4's `new HashSet<>()` diamond repro is
promoted to `tests/differential/SetConstruction.java` and now passes the Java
oracle (`1,1,1`). Raw/diamond set constructor owners no longer default their
missing element type to erased `Object` for the hash-free carrier decision, so
they keep the native `HashSet` carrier instead of falling into the `Vec` carrier
branch. Later follow-ups below supersede the then-open ordered collection and
enum-body sections; the remaining live risk is the unknown erased-display
residue summarized at the current-status section.

Updated 2026-08-24 fifth pass: §5's enum constant body repro is promoted to
`tests/differential/EnumConstantBodies.java` and now passes the Java oracle
(`5,6`). Enum constant class bodies are preserved as `Expr::New { body: ... }`
initializers, and abstract enum methods with one-return same-signature constant
overrides lower to `match self` dispatch instead of the abstract-method panic.
Later follow-ups below supersede the then-open ordered collection discipline in
§2; the remaining live risk is the unknown erased-display residue summarized at
the current-status section.

Updated 2026-08-24 sixth pass: the sorted half of §2 is fixed.
`TreeMap`/`SortedMap`/`NavigableMap`/`ImmutableSortedMap` now carry a sorted-map
discipline and render as `std::collections::BTreeMap`; `TreeSet`/`SortedSet`/
`NavigableSet` render as `std::collections::BTreeSet`. Local declarations such
as `Map<K,V> m = new TreeMap<>()` merge the declared generic arguments with the
concrete sorted constructor carrier, so the annotation and initializer agree.
`tests/differential/TreeMapOrder.java` is promoted and passes the Java oracle
(`a=1;b=2;c=3;`). At this point `LinkedHashMapOrder.java` remained a known gap
because insertion-order collections still needed a real support carrier.

Updated 2026-08-24 seventh pass: `TreeMap.firstKey()` is now a native sorted-map
method projection over the `BTreeMap` key iterator. `tests/differential/
TreeMapFirstKey.java` is promoted and passes the Java oracle (`a:1`). The
remaining sorted-map method-table surface should follow the same pattern.

Updated 2026-08-24 eighth pass: `Set.remove(x)` is now a native set-like method
projection over `HashSet::remove(&x)`/`BTreeSet::remove(&x)`, with a Java
`boolean` return. `tests/differential/SetRemove.java` is promoted and covers the
mutating boolean contract (`false,true,1`).

Updated 2026-08-24 ninth pass: `List.contains(x)`/`Set.contains(x)` now share a
native collection membership projection that borrows the element and returns
Java `boolean`. `tests/differential/ListContains.java` is promoted and covers
the list membership contract (`true,false`).

Updated 2026-08-24 tenth pass: `Map.containsValue(x)` is now a native map-value
membership projection over `values().any(|value| value == &x)`, with a Java
`boolean` return. `tests/differential/MapContainsValue.java` is promoted and
covers the value-membership contract (`true,false`).

Updated 2026-08-24 eleventh pass: `Map.getOrDefault(k, defaultValue)` is now a
native map lookup projection over `get(&k).cloned().unwrap_or(defaultValue)`.
`tests/differential/MapGetOrDefault.java` is promoted and covers both present
and absent keys (`1,9`).

Updated 2026-08-24 twelfth pass: `Map.putIfAbsent(k, v)` is now a native map
mutation projection that stores the key once, returns `Some(previous)` when the
key is present, and inserts/returns `None` only when absent. `tests/differential/
MapPutIfAbsent.java` is promoted and covers the Java oracle (`null,1,1`).

Updated 2026-08-25: erased platform display now covers boxed primitive and
`Character` carriers too. `java_erased_any_to_string` downcasts `char`, integer
widths, and floats before falling through to source-specific `toString` arms or
the loud unknown-erased panic. `tests/differential/ErasedBoxedPrimitiveDisplay.java`
is promoted and now included in the focused
`erased_display_fixtures_match_java_output` loop, covering both concatenation
and direct `Object.toString()` over autoboxed `int`/`char` values
(`i=7,c=A,direct=7/A`).

Updated 2026-08-24 thirteenth pass: `List.set(i, v)` is now a native list
mutation projection that stores the usize index once, clones and returns the
previous element, then assigns the new value at that index. `tests/differential/
ListSet.java` is promoted and covers the Java oracle (`b,a,c`).

Updated 2026-08-24 fourteenth pass: `List.remove(Object)` is now a native list
mutation projection for non-integral single arguments. It stores the needle once,
finds the first equal element with `iter().position`, removes it by index when
present, and returns Java `boolean`. `tests/differential/ListRemoveObject.java`
is promoted and covers the Java oracle (`true,false,2,a,b`).

Updated 2026-08-24 fifteenth pass: emitted source enums now include
`compare_to` implemented through their already-correct `ordinal()` method.
`tests/differential/EnumCompareTo.java` is promoted and covers declaration-order
comparison (`-1,1,0`).

Updated 2026-08-24 sixteenth pass: `JavaAppendValue` now accepts borrowed
primitive values through explicit reference impls, and nullable append inputs
through `Option<T>`, so `StringBuilder`/`StringBuffer` append can consume shapes
such as nullable borrowed list elements. Covered by
`string_builder_append_support_accepts_borrowed_primitive_values`.

Updated 2026-08-24 seventeenth pass: the generic `List<T>.get(i)` typed-local
row was rechecked and the current failure was sharper than the stale note:
lowering already recovered the borrowed element with `.clone()`, but the
method-level type parameter did not get the corresponding `T: Clone` bound, so
Rust cloned `&T` instead. `callable_type_param_needs_clone_bound` now also
recognizes locals/returns recovered from sequence `get` calls. Covered by
`generic_list_get_into_typed_local_recovers_owned_element`.

Updated 2026-08-24 eighteenth pass: the `List<? extends Number>` `.intValue()`
row was stale. `wildcard_bounds_use_read_upper_bound_and_write_erased_carrier`
already compiles the generated Rust and checks `List<? extends Number>` reads as
`&[Number]`, so the call no longer goes through `&Box<dyn Any>`. The missing
native-collection method-table rows in this section are now closed.

Updated 2026-08-24 nineteenth pass: insertion-order collections now have native
support carriers. `LinkedHashMap` renders as `JavaLinkedHashMap` and
`LinkedHashSet` renders as `JavaLinkedHashSet`; both preserve first-insertion
order using a small `Vec`-backed runtime carrier instead of falling back to a
randomized hash collection or compile-error placeholder. `LinkedHashMapOrder.java`
has been promoted to `tests/differential/LinkedHashMapOrder.java` and the
differential suite passes 40/40 fixtures. The audit remains open for the broader
concat/null display gaps recorded above.

Updated 2026-08-25: the insertion-order carrier is now gated for replacement
too. Java `LinkedHashMap.put` updates an existing key's value without moving the
key to the end of iteration order; `tests/differential/LinkedHashMapReinsertOrder.java`
covers `z, a, m, a` insertion/replacement and matches the Java oracle
(`z=1;a=22;m=3;`). The differential suite is green at 104 fixtures.

Updated 2026-08-25 again: the display-sink hint collector now follows source
field receivers one hop before deciding that an omitted external method needs a
`String` facade surface. `summaryText.append(record.sequenceFile.name())`
therefore synthesizes `SequenceFile::name() -> String` instead of leaving the
method as a weak erased/unknown facade result. This narrows the unknown external
display residue, but does not replace the broader display-capability model:
genuinely erased targets with no source field/type evidence are still loud.

Updated 2026-08-24 twentieth pass: the erased-display residue is now loud
instead of blank. `java_erased_object_to_string` panics with an explicit
unsupported-erased-display message, and generated `JavaErasedObjectFacade`
`toString` defaults panic instead of returning `Default::default()`. This does
not close the display gap: genuinely erased values still need dynamic
`toString` recovery, and statically-known values should avoid the erased helper
entirely.

Updated 2026-08-24 twenty-first pass: the stale generic-display known gap is
promoted. `tests/differential/Generics.java` now passes the Java oracle
(`5,true,false,11`); `EXPECTED_DIFFERENTIAL_FIXTURES` was 58 at that point. The
active display residue then was `ObjectToStringConcat.java` and
unknown/genuinely erased display recovery, not the old generic fixture.

Updated 2026-08-24 twenty-second pass: `ObjectToStringConcat.java` is promoted.
The erased-object runtime support now emits a source-type downcast table for
non-generic translated classes that declare `toString()`, and the
`Box<dyn Any>` erased-object facade delegates `toString()` through the same
helper. `Object o = new Point(3)` now recovers `Point.toString()` for both
implicit concat and explicit `o.toString()`; the fixture matches the Java oracle
(`aP3,bP4,P4,P3`) and the differential suite is at 83 fixtures. Unknown erased
targets remain loud.

Updated 2026-08-24 twenty-third pass: the source downcast table now uses the
effective source `toString()` surface, not just methods declared directly on the
concrete type. A child class inheriting `Base.toString()` gets its own erased
display downcast arm only when the inherited-method lowerer will emit the
child's forwarding method, avoiding the generic Object fallback recursion.
`tests/differential/InheritedObjectToStringConcat.java` is promoted and covers
both implicit concat and explicit `Object.toString()` on `Object object = new
Child(7)`. The differential suite is at 86 fixtures. Unknown erased targets
remain loud.

Updated 2026-08-24 twenty-fourth pass: generated placeholder external facades no
longer implement `Display` as `write!(f, "")`. Both ordinary opaque facades and
object-safe-trait support facades now panic with
`unsupported Java external facade display: TypeName`, matching the existing
erased-object runtime policy: placeholder display is loud until a real external
display capability exists. Covered by
`external_opaque_facades_support_equality_and_display_fallbacks`.

Updated 2026-08-24 twenty-fifth pass: `String.valueOf(Object)` over a source
object stored behind erased `Object` now uses the erased display helper instead
of coercing the object slot through `downcast::<String>()`. The fixture
`tests/differential/StringValueOfObject.java` matches the Java oracle (`P6`),
and the differential suite is at 91 fixtures. Unknown external erased display
remains loud/open.

Updated 2026-08-25: the sorted-map method-table follow-up now includes
`TreeMap.lastKey()` as the symmetric companion to `firstKey()`. It lowers over
the `BTreeMap` key iterator with `next_back()`, clones the key, and stays loud
on empty maps with a method-specific panic. `tests/differential/
TreeMapLastKey.java` is promoted and covers the Java oracle (`c:3`). The audit
remains open for the erased/unknown display capability described in §1.

Updated 2026-08-25 again: the empty-map path for `TreeMap.firstKey()` and
`TreeMap.lastKey()` is now catchable in propagated `try` scopes. Ordinary
callers still use the panic-preserving native collection projection, while
fallible lowering raises `NoSuchElementException` through `ok_or_else(...)?`.
`tests/differential/TreeMapFirstLastKeyCatch.java` covers both empty-map catches
and the non-empty first/last key values. The differential suite is at 114
fixtures.

Updated 2026-08-25 second sorted-map pass: nullable `NavigableMap` key
navigation is modeled for `lowerKey`, `floorKey`, `ceilingKey`, and `higherKey`.
The lowering uses `BTreeMap::range::<_, _>` with owned cloned bounds and returns
`Option<K>`, preserving Java's `null` result at the edges. `tests/differential/
TreeMapNavigationKeys.java` covers both matching and missing-boundary results
(`null,b,b,d,b,d,d,null`). The audit remains open for erased/unknown display.

Updated 2026-08-25 erased-platform display pass: erased `Object` values holding
known Java platform carriers no longer fall into the loud unknown-erased panic
for basic display. `java_erased_any_to_string` now recognizes `String` and
`bool` before source-class downcasts, so `Object s = "x"` and
`Object b = Boolean.TRUE` print as Java would through both implicit concat and
explicit `toString()`. `tests/differential/ErasedPlatformDisplay.java` is
promoted and the differential suite is at 101 fixtures. Unknown external erased
display remains loud/open.

Updated 2026-08-25 erased string-buffer display pass: the same erased platform
runtime now recognizes `StringBuilder` and `StringBuffer`, plus shared
`Rc`/`Arc` handles to those carriers, but only emits those downcast arms for
programs that mention the platform types. `Object builder = new
StringBuilder("ab").append(3)` and `Object buffer = new
StringBuffer("xy").append(false)` now print through both concat and erased
`toString()`. `tests/differential/ErasedStringBuilderDisplay.java` is promoted
and the differential suite is at 112 fixtures. Unknown external erased display
remains loud/open.

Updated 2026-08-25 source default-display pass: source classes without an
explicit `toString()` now use the generated Java `Object.toString` fallback for
typed receivers and for erased `Object` carriers that downcast to a translated
source class. `tests/differential/SourceObjectDefaultToString.java` is promoted
and checks both paths by Java prefix; the differential suite is green at 120
fixtures. This does not solve genuinely unknown external erased display.

Updated 2026-08-26 list-add overload pass: `List.add(E)` and
`List.add(int,E)` are no longer collapsed into one `Vec::push` projection.
The one-argument overload now lowers to a structured block that pushes and
returns Java `true`; the indexed overload lowers to `Vec::insert(index, value)`.
`tests/differential/ListAddReturnsBoolean.java` and
`tests/differential/ListAddIndex.java` cover the boolean return and insertion
order contracts. The audit remains open for the known erased/external display
residue and opaque `Map.Entry` values without extractable key/value identity.

Updated 2026-08-26 map-view mutation pass: `Map.keySet().remove(k)` no longer
mutates a snapshot of the key set. Calls whose receiver is a zero-argument
`keySet()`/`key_set()` view now lower to removal from the backing map and return
`remove(&key).is_some()`, preserving Java's boolean contract even when the
present key maps to `null`. `tests/differential/MapKeySetRemove.java` covers the
returned booleans and backing-map mutation; the differential suite passes 140
fixtures. Broader live view APIs, especially iterator removal and value/entry
view mutations, remain outside this narrow fix.

Updated 2026-08-26 map-values remove guardrail: `Map.values().remove(v)` now
borrows the lowered backing-map receiver into one temporary and uses that
temporary for both the value search and the backing `remove`. The previous
lowering cloned the lowered receiver expression, so a non-trivial receiver
could be evaluated once while searching and again while mutating. Regression:
`lower::tests::map_values_remove_uses_one_map_receiver_temporary`.

Updated 2026-08-26 map-key-set live-view follow-up: key-set view aliases now
route `clear`, `size`, `isEmpty`/`is_empty`, and `contains` through the backing
map instead of mutating or reading the materialized snapshot. The direct
`values.keySet().clear()`/alias `keys.clear()` shape clears the map, and
`keys.size()` observes the backing map length after mutation. Regressions:
`lower::tests::map_key_set_alias_clear_and_size_use_backing_map` and
`tests/differential/MapKeySetClear.java`.

Found by differential testing along the JLS collections/generics/enums axis.
The original probe covered **34 shapes and found 14 divergences**. After the
promotions above, the checked collection contract gaps are closed: insertion
order, sorted-map endpoints/navigation, `Map.put`/`remove`/`putIfAbsent`,
`List.add`/`set`/`remove`, set/list membership, enum constant bodies, and live
`keySet()`/`values()` remove are all differential-gated. The remaining semantic
risk in this audit is the broader erased/null display surface for genuinely
unknown external runtime values.

Repros: `claude_audits/repro/ConcatReferenceValues.java`,
`LinkedHashMapOrder.java`, `MapPutPrevious.java`, `SetConstruction.java`,
`EnumConstantBodies.java`.

---

## 1. Erased object display recovers source `toString`; unknown erased display remains open

`claude_audits/repro/ConcatReferenceValues.java`

```java
enum Col { RED, BLUE }
static class P { final int x; P(int x){this.x=x;} public String toString(){ return "P" + x; } }

public static String run() {
    Integer boxed = 5;
    Col c = Col.BLUE;
    P p = new P(9);
    List<Integer> li = new ArrayList<>(); li.add(42);
    List<String> ls = new ArrayList<>();  ls.add("s");
    Map<String, Integer> m = new HashMap<>(); m.put("k", 7);
    return boxed + "|" + c + "|" + c.name() + "|" + p + "|" + li.get(0) + "|" + ls.get(0) + "|" + m.get("k");
}
```

Historical result before the display split:

```
java:           5|BLUE|BLUE|P9|42|s|7
old translation: ||BLUE|||s|7               (compiled cleanly, exit 0)
```

Four of seven segments were wiped. The historical generated line:

```rust
return format!("{}|{}|{}|{}|{}|{}|{}",
    java_erased_object_to_string(&boxed),                                  // Integer  -> ""
    java_erased_object_to_string(&c),                                      // enum     -> ""
    c.name(),                                                              // &str     -> ok
    java_erased_object_to_string(&p),                                      // user P   -> ""
    java_erased_object_to_string(&li.get(0 as usize).expect("...")),       // &i32     -> ""
    ls.get(0 as usize).expect("..."),                                      // &String  -> ok
    m.get(&"k".to_string()).map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()));
```

against the old helper body

```rust
pub fn java_erased_object_to_string<T: ?Sized>(_value: &T) -> String {
    Default::default()
}
```

Current code no longer sends these concrete `JavaType::Named` operands through
the erased helper; `tests/differential/ConcatReferenceValues.java` and
`tests/differential/Generics.java` are promoted. Source values erased behind
`Box<dyn Any>` can also recover `toString()` when their translated concrete type
declares or inherits a non-generic zero-arg `toString()` method;
`ObjectToStringConcat.java` and `InheritedObjectToStringConcat.java` are
promoted for those paths. Plain `new Object().toString()` is also promoted via
a `JavaPlainObject` erased marker. The remaining gap is genuinely
unknown/external erased display, where no generated source downcast arm or
modeled external display capability exists.

The wiped operands are `i32`, a `#[derive(Debug, Clone, Copy)]` enum with a
hand-written `impl Display`, a user struct whose `to_string()` is generated
correctly one line above, and `&i32`. **Every one of them already implements
`Display` in the emitted crate.** Nothing about them is erased — they take the
erased path because of their *Java* type, not their Rust type.

This was not the generic known gap. `tests/differential/Generics.java` is now
promoted and matches Java output (`5,true,false,11`). The historical concrete
case above was wider: `Integer`, `Col`, `P` and `List<Integer>` are all fully
concrete. The current residual is unknown erased display without a source
downcast or explicit display capability, not that generic fixture.

`tests/differential/ObjectToStringFallback.java` now covers plain
`Object.toString()`: Java answers `true` for the stable default-object-string
predicate, and the translation now does too. This does not make unknown
external erased values printable; those stay loud until there is a real display
capability model.

**Historical root cause** — `src/conversion_plan.rs:java_type_needs_java_object_display_fallback`:

```rust
JavaType::Named { name, .. } | JavaType::Unknown(name) => {
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(name);
    let receiver_simple = receiver_key.rsplit('.').next().unwrap_or(&receiver_key);
    !matches!(receiver_simple,
        "String" | "StringBuilder" | "StringBuffer" | "CharSequence"
        | "Appendable" | "Class" | "Path" | "File" | "URI")
    && !crate::stdlib::is_java_platform_facade_type_name(&receiver_key)
}
```

Current code no longer treats `JavaType::Named` as needing the fallback. Unknown
and erased types can still take the erased display path unless another
capability-specific rule models them.

The two shapes that survive do so by accident of a different code path: `String`
is on the allowlist, and `Map.get` is special-cased into
`.map(|v| v.to_string()).unwrap_or("null")` by the native-collection lowering
before this predicate ever runs.

Two more observations that narrow the fix:

- **Explicit vs implicit disagree.** `p.toString() + "|" + p` prints `P(4)|`:
  the explicit call reaches the generated `to_string`, the implicit conversion
  does not. So the target is reachable from the call site; only the concat path
  misses it.
- **The same operand differs by context.** `c.name()` renders as `c.name()`
  inside a `format!` but as `java_erased_object_to_string(&c.name())` inside
  `sb.append(...)` (`src/lower.rs:33161`). The `DisplaySegment` and
  `StringConcatSegment` slots are not agreeing with the `StringBuilder` append
  slot.

**Direction.** The old broad predicate has been narrowed. Source objects stored
as `Object`, platform carriers, plain `Object`, `java.io.File`, and external
facades with a modeled zero-argument `toString(): String` fact now have concrete
display paths. Keep unknown external erased display loud until a real capability
fact/model exists; do not reintroduce a blanket helper fallback. This follows
[0016](0016-core-jdk-facades-are-no-op-stubs.md)'s rule: the remaining residue is
visible rather than blank.

**Blast radius.** The original broad concat bug fired on many reference operands.
That concrete class is now covered by differential fixtures. The remaining blast
radius is narrower but still semantic: a value erased behind `Object` cannot
recover Java dynamic `toString()` unless the translator has source/platform
downcast evidence or an explicit external display fact.

---

## 2. HISTORICAL: `LinkedHashMap` and `TreeMap` used to become `HashMap`

`claude_audits/repro/LinkedHashMapOrder.java`

```java
Map<String, String> m = new LinkedHashMap<>();
m.put("z", "1"); m.put("a", "2"); m.put("m", "3");
for (String k : m.keySet()) out = out + k + "=" + m.get(k) + ";";
```

```
java:        z=1;a=2;m=3;
translation: compiles cleanly; one binary, five consecutive runs:
             z=1;m=3;a=2;  z=1;a=2;m=3;  a=2;m=3;z=1;  a=2;z=1;m=3;  z=1;a=2;m=3;
```

```rust
let mut m: std::collections::HashMap<String, String> = std::collections::HashMap::new();
```

`TreeMap` used to be identical (`c3a1b2`, `b2a1c3`, `a1b2c3`, … across eight
runs). Both halves are now fixed: sorted maps/sets use `BTreeMap`/`BTreeSet`,
and insertion-ordered maps/sets use `JavaLinkedHashMap`/`JavaLinkedHashSet`
support carriers. `tests/differential/LinkedHashMapOrder.java`,
`LinkedHashMapReinsertOrder.java`, and `TreeMapOrder.java` are promoted and
passing.

`AUDIT-BACKLOG.md` §B5 predicted exactly this ("a `LinkedHashMap` lowered to
`std::collections::HashMap` would be a real defect"). It was confirmed, for both
`LinkedHashMap` and `TreeMap`, and the nondeterminism is the part the prediction
did not cover.

**Original root cause** — `src/stdlib.rs`:

```rust
pub enum JavaCollectionKind { List, Set, Map }
```

Ordering discipline was erased at the type level, so no downstream site could
recover it. `java_collection_kind_for_name` folded `HashMap` / `LinkedHashMap` /
`TreeMap` / `SortedMap` / `NavigableMap` into `Map`, and
`src/coercion.rs:native_collection_type_name` mapped `Map -> "std::collections::HashMap"`
unconditionally. This is now split for sorted and insertion-ordered
collections.

**Current direction.** `LinkedHashMap` / `LinkedHashSet` have no std equivalent
and now use small support types that preserve insertion order. `HashMap` itself
is genuinely unspecified in Java and can stay as-is.

`AUDIT-BACKLOG.md` §B5 can be treated as confirmed and fixed for the tested
ordered-iteration contract.

---

## 3. `Map.put` returning the previous value panics on the first put

`claude_audits/repro/MapPutPrevious.java`

```java
Map<String, Integer> m = new HashMap<>();
Integer p1 = m.put("a", 1);
Integer p2 = m.put("a", 2);
return p1 + "," + p2;
```

```
java:        null,1
old translation: thread 'main' panicked: called `Option::unwrap()` on a `None` value
current translation: null,1
```

```rust
let p1: Box<dyn std::any::Any> = Box::new(m.insert("a".to_string(), 1).unwrap()) as Box<dyn std::any::Any>;
```

Java's `put` returns `null` for an absent key — that is the documented "was it
already there?" idiom (`if (map.put(k, v) != null) …`). `HashMap::insert`
returns exactly the right `Option`, and the lowering `unwrap()`s it. The first
put of any key panics.

Fixed for `Map.put`: the declared `Integer` local now inherits the initializer's
`Option<i32>` shape, so `HashMap::insert`'s previous-value result is preserved
instead of unwrapped and erased.

Fixed for `Map.remove`: it now lowers through the native collection planner
instead of generic method fallback, so `HashMap::remove` receives a borrowed key
and its nullable previous-value result stays `Option<V>`. The `Map.get`,
`Map.put`, and `Map.remove` nullable-result family now share the same shape
contract.

---

## 4. `new HashSet<>()` — the diamond form — lowers to `Vec::new()`

`claude_audits/repro/SetConstruction.java`

```java
Set<String>    a = new HashSet<String>();   // explicit args
HashSet<String> b = new HashSet<>();        // diamond
Set<String>    c = new HashSet<>();         // diamond
```

```rust
let mut a: std::collections::HashSet<String> = std::collections::HashSet::new();
let mut b: std::collections::HashSet<String> = std::collections::HashSet::new();
let mut c: std::collections::HashSet<String> = std::collections::HashSet::new();
```

Fixed for `HashSet`: the slot type was right, but the constructor owner had no
generic arguments and the hash-free set-carrier predicate substituted erased
`Object` as the missing element. `Object` was treated as hash-free, so the
constructor chose the list/`Vec` carrier even though the local slot rendered as
`HashSet<String>`. Missing set element type now means "unknown", not "erased
Object", for that carrier decision. `new HashSet<>()` now compiles and matches
Java for the promoted repro.

---

## 5. Enum constant bodies are discarded

`claude_audits/repro/EnumConstantBodies.java`

```java
enum Op {
    ADD { int apply(int a, int b) { return a + b; } },
    MUL { int apply(int a, int b) { return a * b; } };
    abstract int apply(int a, int b);
}
```

```
java:        5,6
old translation: panicked: abstract method EnumConstantBodies.Op.apply has no implementation
current translation: 5,6
```

Fixed for the straightforward Java enum-constant override shape: each constant
body preserves its parsed method members, and an abstract enum method whose
constants all provide a same-signature single-return override now emits a
`match self` dispatcher. More complex constant bodies still fall back to the
loud abstract-method stub rather than guessing.

---

## What is correct

The map of where **not** to look. All verified by running both sides.

**Enums — correct for the probed surfaces.** `values()`, `ordinal()`, `name()`, `valueOf`,
`values().length`, `values()[i]`, `switch` on an enum (including `default`),
`==` and `!=`, `equals`, fields with a constructor and accessors, and use as a
`HashMap` key. The generated lowering is genuinely good: a `#[derive(Debug,
Clone, Copy, PartialEq, Eq, Hash)]` Rust enum with `values`/`value_of`/`valueOf`/
`name`/`ordinal` and a real `impl Display` that forwards to `name()`. The only
missing surface found in this audit is `compareTo` (`no method named compare_to`).
**Enum `name()` and `toString()` return the right string — §1 discards it at the
call site, not in the enum.**

**Collections that work:**

| shape | result |
|---|---|
| `ArrayList` add / get / size / iterate, `List<String>` and `List<Integer>` | ✅ |
| `List` element read into a **typed local** or used in arithmetic | ✅ (the *concat* is §1) |
| explicit `Iterator` with `hasNext`/`next` | ✅ |
| `HashMap` put / get / size / `containsKey` | ✅ |
| `map.get(missingKey)` → `null`, and `v == null` | ✅ `null,true` |
| `keySet()` / `values()` / `entrySet()` enhanced-for | ✅ |
| user class with `equals`+`hashCode` as a **map key** and in `List.contains`/`indexOf` | ✅ |
| nested generics `Map<String, List<Integer>>`, including iterating `m.get(k)` | ✅ |
| `new ArrayList<>(Arrays.asList(...))` | ✅ |

**Generics:** a generic method with inferred type arguments called at two
different types (`pick(3,4,true)` / `pick("p","q",false)`) is correct.
`GenericBounds.java` in the existing corpus is correct.

---

## Collection surfaces that were missing or wrong, now closed

This table is historical. These rows used to fail loudly enough for a corpus
guard to see them; the current entries are fixed and list the regression fixture
or unit test that keeps each collection surface covered.

| Java | lowered as | result |
|---|---|---|
| `List.remove(Object)` | fixed: native first-equal `position`/`remove(index)` block | returns `boolean`; covered by `ListRemoveObject` |
| `List.set(i, v)` | fixed: native index block returning previous element | covered by `ListSet` |
| `List.contains(x)` on `Vec<String>` | fixed: native list/set `contains(&x)` | returns `boolean`; covered by `ListContains` |
| `Map.containsValue` | fixed: native `values().any(|value| value == &x)` | returns `boolean`; covered by `MapContainsValue` |
| `Map.getOrDefault` | fixed: native `get(&k).cloned().unwrap_or(default)` | covered by `MapGetOrDefault` |
| `Map.putIfAbsent` | fixed: native `contains_key`/`get().cloned()`/`insert` block | returns previous nullable value; covered by `MapPutIfAbsent` |
| `Map.remove(k)` capturing the result | fixed: native `HashMap::remove(&k)` | returns `Option<V>`; covered by `MapRemovePrevious` |
| `TreeMap.firstKey` / `lastKey` | fixed: `BTreeMap::keys().next()/next_back()` with ordinary uncaught-`JavaException` adaptation and catchable `NoSuchElementException` in propagated `try` scopes | covered by `TreeMapFirstKey`, `TreeMapLastKey`, and `TreeMapFirstLastKeyCatch` |
| `TreeMap.lowerKey` / `floorKey` / `ceilingKey` / `higherKey` | fixed: `BTreeMap::range::<_, _>(...).next()/next_back().map(|(key, _)| key.clone())` | returns nullable key; covered by `TreeMapNavigationKeys` |
| `Set.remove(x)` | fixed: native set `remove(&x)` | returns `boolean`; covered by `SetRemove` |
| `enum.compareTo` | fixed: emitted enum `compare_to` over `ordinal()` | covered by `EnumCompareTo` |
| `List<T>.get(i)` into a typed local under a **generic** owner/method | fixed: generic sequence-get recovery adds the required `T: Clone` bound | covered by `generic_list_get_into_typed_local_recovers_owned_element` |
| `sb.append(intExpr)` | fixed: `JavaAppendValue` for borrowed primitives and `Option<T>` | covered by borrowed primitive append test |
| `List<? extends Number>` element `.intValue()` | fixed: bounded wildcard reads use the upper-bound carrier | covered by `wildcard_bounds_use_read_upper_bound_and_write_erased_carrier` |

The native-method-table rows in this section are closed. `LinkedHashMap` and
`LinkedHashSet` now use insertion-ordered support carriers rather than
`HashMap`/`HashSet`/loud fallback.

Follow-up 2026-08-25: the native collection role metadata now agrees with that
lowering for sorted-map navigation. `firstKey`, `lastKey`, `lowerKey`,
`floorKey`, `ceilingKey`, and `higherKey` are reported as known getters for
`TreeMap`/`SortedMap`, while plain `Map.lowerKey` remains unknown. Regression:
`stdlib::native_collection_role_tests::
sorted_map_navigation_methods_are_known_getters_only_for_sorted_maps`.

---

## Fixture placement status

`ConcatReferenceValues.java` and `LinkedHashMapOrder.java` have been promoted.

`MapPutPrevious.java`, `SetConstruction.java`, `EnumConstantBodies.java`, and
`TreeMapOrder.java` have been promoted.

The stale `0052` concern that differential coverage had no `Map`, `Set`, or
`enum` fixtures is no longer true: the promoted fixtures now cover the concrete
collection and enum rows in this audit. The external facade part of the
erased/unknown display gap is narrower now: a modeled zero-argument
`toString(): String` API fact makes both the generated `Display` impl and the
erased-object runtime display helper delegate to the facade method, while
facades without that evidence still panic instead of silently inventing a
string. The remaining open work is arbitrary erased or unknown runtime values
without a source downcast arm or explicit external API fact.

Follow-up 2026-08-25: erased `java.io.File` values now use the existing `File`
facade `Display` implementation instead of panicking in
`java_erased_any_to_string`. Covered by
`erased_file_object_display_delegates_to_file_display` and
`tests/differential/ErasedFileDisplay.java`. The open residue remains arbitrary
unknown/external erased display without a source downcast arm or explicit
external API fact.

Follow-up 2026-08-25: `List.remove(Object)` no longer double-borrows the
lowered needle when scanning a `Vec<String>`. The object-remove lowering now
compares each iterated item with the already slot-lowered needle binding, so
`tests/differential/ListRemoveObject.java` compiles and matches Java again
(`true,false,2,a,b`).

Follow-up 2026-08-26: live map-value view mutation is now covered for
`Map.values().remove(value)`. The lowering recognizes `remove` whose receiver is
a zero-argument `values()` call on a map-like receiver, finds the first matching
backing-map entry, removes that key, and returns Java's boolean result instead
of mutating an owned values snapshot. `tests/differential/MapValuesRemove.java`
matches the Java oracle (`true,false,false,1`), and the differential suite is
green at 141 fixtures.

Follow-up 2026-08-26: aliased `Map.keySet()` locals now keep enough source-view
identity for `keys.remove(k)` to mutate the backing map. Local declaration
lowering records only the narrow `Set<K> keys = map.keySet()` view fact, and
`remove` on that local reuses the existing backing-map mutation projection
instead of removing from the cloned `HashSet` snapshot. Covered by
`tests/differential/MapKeySetAliasRemove.java` (`true,false,1`).

Follow-up 2026-08-26: two remaining external-erased display holes are now
covered. `String.valueOf(Object value)` in a string slot now treats a local
declared as `Object` as erased-display evidence instead of falling through to a
`String` downcast, and external static field owners such as `ExternalEnum.A`
count as mentions of the external owner when generating erased-display downcast
arms. Static enum-constant companion `Display` impls also reuse the modeled
zero-argument `toString(): String` fact instead of hardcoding the placeholder
external-display panic. Covered by
`display_capable_external_object_uses_erased_runtime_downcast` and
`display_capable_external_enum_constant_does_not_emit_placeholder_display`.

Follow-up 2026-08-26: external facade return rendering now preserves ordered
map carriers. `LinkedHashMap<K,V>` return facts no longer render facade methods
as `std::collections::HashMap<K,V>` or get corrupted by the generic return-type
normalizer; they render as `JavaLinkedHashMap<K,V>`, matching the native slot
policy. The same helper keeps sorted maps on `BTreeMap` and ordinary maps on
`HashMap`. Regression:
`external_facade_linked_hash_map_return_keeps_insertion_order_carrier`.

Follow-up 2026-08-26: display-sink return hints now respect typed lambda
parameter scope. The hint collector previously walked lambda bodies with the
outer binding context, so a typed lambda parameter that shared a name with an
outer field could synthesize a display facade for the outer field's owner
instead of the lambda parameter's owner. Typed lambda params now enter the
display-hint type context before the lambda body is scanned. Regression:
`stdlib::display_hint_tests::typed_lambda_param_shadows_outer_binding_for_display_hint_owner`.

Follow-up 2026-08-26: aliased `Map.values()` locals now mirror the earlier
`keySet()` view handling. Shape learning records only the narrow
`Collection<V> vals = map.values()` fact, clears it on local rebinding, and
`vals.remove(v)` reuses the backing-map mutation lowering for direct
`map.values().remove(v)` instead of removing from an owned snapshot. Covered by
`lower::tests::map_values_alias_remove_mutates_backing_map`,
`lower::tests::map_values_alias_remove_translation_mutates_backing_map`, and
`tests/differential/MapValuesAliasRemove.java` (`true,false,1`).

Follow-up 2026-08-26 typed-list display pass: string concatenation of a typed
list-like collection now uses Java bracket/comma formatting instead of generic
Rust `.to_string()` on the native `Vec<T>` carrier. The concat display plan
recognizes `List<T>`-kind Java types and lowers through
`java_collection_to_string`, sharing the existing element display semantics used
by array deep stringification. `tests/differential/ListDisplay.java` covers
`"xs=" + list` against Java's `xs=[a, b]` oracle. Map/set display remains a
separate contract because Java map entries use `key=value` and set iteration
order is carrier-dependent.

Follow-up 2026-08-26 insertion-ordered map/set display pass:
`JavaLinkedHashMap` now implements `Display` using Java map syntax over its
ordered carrier: `{key=value, next=value}`. `JavaLinkedHashSet` now also
implements `Display` through the same Java collection bracket/comma formatting
as lists. Both carriers implement `JavaArraysDeepToStringElement`, so nested
collection display can reuse the same element-display contract. The attempted
set fixture exposed a representation-aware type-renderer disagreement:
`LinkedHashSet<String>` declarations could render as
`std::collections::HashSet<String>` while constructors returned
`JavaLinkedHashSet`; that path now preserves the insertion-ordered carrier.
`tests/differential/LinkedHashMapDisplay.java` and
`tests/differential/LinkedHashSetDisplay.java` cover deterministic insertion
order display. The full differential oracle passed with 159 fixtures.

Follow-up 2026-08-26 values-view clear pass: aliased `Map.values()` locals now
handle `clear()`, `size()`, and `isEmpty()` through the backing map rather than
the materialized values snapshot. This mirrors the earlier `keySet()` live-view
handling and keeps `Collection<V> vals = map.values(); vals.clear();`
observable on `map`. Covered by
`lower::tests::map_values_alias_clear_and_size_use_backing_map`,
`lower::tests::map_values_alias_clear_translation_mutates_backing_map`, and
`tests/differential/MapValuesClear.java` (`0,0,false`). The full differential
oracle passed with 161 fixtures.

Follow-up 2026-08-26 entry-set setValue pass: the immediate
`map.entrySet().iterator().next().setValue(value)` shape now mutates the backing
map and returns the previous value instead of falling through to an unsupported
tuple-entry call. The lowering keeps the map receiver as a single mutable
temporary, clones the selected key value, performs `insert`, and unwraps the
previous value that must exist for a live entry. Covered by
`lower::tests::map_entry_set_value_mutates_backing_map_and_returns_previous_value`,
`lower::tests::map_entry_set_value_translation_mutates_backing_map`, and
`tests/differential/MapEntrySetSetValue.java` (`1,2`). The broader aliased or
enhanced-for `Map.Entry.setValue` live-view carrier was still open at this
point; the later enhanced-for pass below closes that concrete shape.

Follow-up 2026-08-27 values-view contains pass: aliased and direct
`Map.values()` receivers now route `contains(value)` through the backing map
instead of asking a materialized values snapshot. The lowering mirrors
`Map.containsValue`: it evaluates the searched value once, iterates
`map.values()`, and compares each live backing value against that temporary.
Covered by `lower::tests::map_values_alias_contains_uses_backing_map` and
`tests/differential/MapValuesContains.java` (`false,true,true`). The full
differential oracle passed with 164 fixtures.

Follow-up 2026-08-28 entry-set clear/size pass: aliased and direct
`Map.entrySet()` receivers now route `clear()`, `size()`, and `isEmpty()`
through the backing map instead of materializing an invalid or stale entry-set
snapshot. `Set<Map.Entry<K,V>>` locals use the tuple carrier already used by
enhanced-for entry iteration, and `entrySet()` set-slot initialization now
collects cloned `(key, value)` tuples when a real snapshot is needed. Covered by
`lower::tests::map_entry_set_alias_clear_and_size_use_backing_map`,
`lower::tests::map_entry_set_alias_clear_translation_mutates_backing_map`, and
`tests/differential/MapEntrySetClear.java` (`0,0,false`). Entry `remove` and
`contains` were still outside this narrow slice; the later same-map iterator and
`SimpleEntry` passes below cover the concrete entry-equality shapes found so
far.

Follow-up 2026-08-28 entry-set remove-next pass: direct
`values.entrySet().remove(values.entrySet().iterator().next())` now removes the
selected key from the backing map and returns whether the removal happened
instead of lowering to invalid `values.iter().remove(...)`. Covered by
`lower::tests::map_entry_set_remove_next_translation_mutates_backing_map` and
`tests/differential/MapEntrySetRemove.java` (`true,0,false`). This is
intentionally limited to an entry proven to come from the same map iterator;
`AbstractMap.SimpleEntry` equality and enhanced-for `entry.setValue(...)` were
still separate concrete shapes at this point and are covered by later passes.
Arbitrary entry values whose key/value cannot yet be extracted structurally
remain open.

Follow-up 2026-08-28 entry-set SimpleEntry equality pass:
`values.entrySet().contains(new AbstractMap.SimpleEntry<>(key, value))` now
checks the backing map value for `key`, and
`values.entrySet().remove(new AbstractMap.SimpleEntry<>(key, value))` removes
the key only when the stored value is equal. Both paths avoid the invalid
`values.iter().contains/remove(...)` iterator calls and preserve the backing
map's live-view semantics for constructor entries with extractable key/value
operands. Covered by
`lower::tests::map_entry_set_contains_simple_entry_uses_backing_map`,
`lower::tests::map_entry_set_remove_simple_entry_mutates_backing_map`,
`tests/differential/MapEntrySetContainsSimpleEntry.java` (`true,false,2`), and
`tests/differential/MapEntrySetRemoveSimpleEntry.java` (`false,true,1,false`).
Enhanced-for `entry.setValue(...)` is covered by the next pass. Remaining
entry-set residue is arbitrary `Map.Entry` values whose key/value cannot yet be
extracted structurally.

Follow-up 2026-08-28 entry-set enhanced-for `setValue` pass:
`for (Map.Entry<K,V> entry : values.entrySet()) { entry.setValue(v); }` now
uses the backing map's mutable iterator when the loop body syntactically
contains `entry.setValue(...)`. The loop remains the existing borrowed tuple
path for read-only entry loops, but switches to `values.iter_mut()` for the
mutating shape and lowers `setValue` to clone the old value, assign through the
mutable tuple value, and return the old value. Covered by
`lower::tests::map_entry_set_enhanced_for_set_value_uses_mutable_backing_iterator`
and `tests/differential/MapEntrySetEnhancedForSetValue.java` (`1,2`). The full
differential oracle passed with 178 fixtures. Remaining entry-set residue is
arbitrary `Map.Entry` values whose key/value cannot yet be extracted
structurally.

Follow-up 2026-08-28 entry-set contains-next pass:
`values.entrySet().contains(values.entrySet().iterator().next())` now recognizes
that the probed entry comes from the same backing map. The lowering evaluates
the map receiver once as an immutable borrow, extracts the iterator entry's key,
and checks `contains_key(&key)` instead of trying to call `contains` on a Rust
hash-map iterator. Covered by
`lower::tests::map_entry_set_contains_next_translation_reads_backing_map` and
`tests/differential/MapEntrySetContainsNext.java` (`true,1`). Remaining
entry-set residue is arbitrary `Map.Entry` values whose key/value cannot yet be
extracted structurally.

Follow-up 2026-08-28 entry-set SimpleEntry local pass:
`Map.Entry<K,V> entry = new AbstractMap.SimpleEntry<>(key, value)` is now
tracked as a decomposable local value, so
`values.entrySet().contains(entry)` and `values.entrySet().remove(entry)` use
the backing map instead of iterator collection methods. Assignment to the entry
local updates the recorded key/value pair when the new value is another
`SimpleEntry`, and clears it otherwise to avoid stale decomposition. Covered by
`lower::tests::map_entry_set_contains_simple_entry_local_uses_backing_map`,
`lower::tests::map_entry_set_remove_simple_entry_local_mutates_backing_map`,
`lower::tests::map_entry_set_contains_simple_entry_local_reassignment_updates_backing_map_args`,
`tests/differential/MapEntrySetContainsEntryLocal.java` (`true,1`), and
`tests/differential/MapEntrySetRemoveEntryLocal.java` (`true,0,false`).
Remaining entry-set residue is opaque `Map.Entry` values whose key/value cannot
yet be extracted structurally.

Follow-up 2026-08-28 entry-set iterator-entry local pass:
`Map.Entry<K,V> entry = values.entrySet().iterator().next()` is now tracked as a
local entry value tied to its backing map. Same-map
`values.entrySet().contains(entry)` checks the backing map key directly, and
same-map `values.entrySet().remove(entry)` clones the entry key before mutating
the backing map. Assignment refreshes this backing-map fact when the new value
is another map-entry iterator result and clears it otherwise. Covered by
`lower::tests::map_entry_set_contains_next_local_translation_reads_backing_map`,
`lower::tests::map_entry_set_remove_next_local_translation_mutates_backing_map`,
`tests/differential/MapEntrySetContainsNextLocal.java` (`true,1`), and
`tests/differential/MapEntrySetRemoveNextLocal.java` (`true,0,false`).
Remaining entry-set residue is opaque `Map.Entry` values whose key/value or
backing map cannot yet be extracted structurally.

Follow-up 2026-08-28 SimpleEntry display pass:
`Map.Entry<K,V> entry = new AbstractMap.SimpleEntry<>(key, value)` now reuses
the tracked constructor operands when the entry is lowered as a string concat or
display segment. That preserves Java's `key=value` entry display for the
decomposable local case instead of falling through to erased-object display,
where an external `SimpleEntry` facade has no stored key/value state. Covered by
`lower::tests::simple_map_entry_local_display_uses_tracked_key_value_pair` and
`tests/differential/MapEntryDisplay.java` (`xa=1`). Remaining entry residue is
arbitrary opaque `Map.Entry` values whose key/value or backing map cannot yet be
extracted structurally.

Follow-up 2026-09-07 erased SimpleEntry display pass:
`Object value = new AbstractMap.SimpleEntry<>(key, value)` and the matching
`SimpleImmutableEntry` form now use the constructor's own generic arguments to
lower the erased object carrier as the same tuple used by typed `Map.Entry`
slots. That lets erased display reuse the runtime `(String, String)` `key=value`
arm instead of emitting an opaque external facade whose `toString()` has no
key/value state. Covered by `tests/differential/ErasedMapEntryDisplay.java` and
`tests/differential/ErasedSimpleImmutableEntryDisplay.java` (`xa=1:a=1`).

Follow-up 2026-08-28 entry-set iterator display pass:
`values.entrySet().iterator().next()` now uses the same backing-map recognition
as the entry-set mutation/contains/remove paths when it is lowered as a string
concat or display segment. The lowering evaluates the backing map once, binds
one borrowed iterator entry, and formats `key=value` instead of displaying the
Rust tuple carrier. Covered by
`lower::tests::map_entry_next_display_uses_backing_key_value_pair` and
`tests/differential/MapEntryNextDisplay.java` (`xa=1`). Remaining entry residue
is arbitrary opaque `Map.Entry` values whose key/value or backing map cannot yet
be extracted structurally.

Follow-up 2026-08-28 Map.Entry field-carrier pass:
`Map.Entry<K,V>` fields now use the same tuple carrier consistently at
construction and accessor sites. A field initialized from
`new AbstractMap.SimpleEntry<>(key, value)` lowers the constructor into the
target entry tuple instead of the external `SimpleEntry` facade, and
`entry.getKey()` / `entry.getValue()` on a typed entry receiver project tuple
fields rather than calling missing facade methods. Covered by
`lower::tests::map_entry_field_accessors_use_tuple_fields` and
`tests/differential/MapEntryFieldDisplay.java` (`a=1`). Remaining entry residue
is arbitrary opaque `Map.Entry` values whose key/value or backing map cannot yet
be extracted structurally.

Follow-up 2026-08-28 `Collection.addAll` return pass:
`List.addAll` and set-like `addAll` now plan separately from void `putAll` /
Rust `extend`. Expression-valued `addAll` lowers to a structural block that
records the collection length, performs the extend, and returns whether the
length changed; expression statements still lower as plain mutation. The JDK
facade ABI now marks list/set `addAll` receivers as `&mut self`, and the
signature boundary rule no longer lets the default owned facade parameter shape
override a known mutating receiver call. Covered by
`tests/differential/CollectionAddAllReturn.java`
(`true,false,2,true,false,1`),
`slot_shape_design::native_collection_add_all_argument_boundary_borrows_source_content`,
and `slot_shape_design::stdlib_signature_facts_include_jdk_collection_rust_abi_slots`.
The differential suite is now at 218 fixtures.

Follow-up 2026-08-28 typed `Map.Entry` parameter pass:
`values.entrySet().contains(entry)` and `values.entrySet().remove(entry)` now
decompose a stable typed `Map.Entry<K,V>` argument by projecting its tuple
carrier key/value fields. This covers parameter and other known stable
projections with an inferred entry shape, while still leaving arbitrary opaque
entry values without a recoverable key/value carrier open. The erased-return
lowering path now gives map views the same precedence as ordinary expression
and slot lowering, avoiding the previous fallback to invalid Rust iterator
`contains`/`remove` with `&Entry`. Covered by
`lower::tests::map_entry_set_contains_typed_entry_param_uses_key_value_projection`,
`lower::tests::map_entry_set_remove_typed_entry_param_uses_key_value_projection`,
`tests/differential/MapEntrySetContainsParam.java` (`true,false,1`), and
`tests/differential/MapEntrySetRemoveParam.java` (`false,true,0,false`).
The differential suite is now at 218 fixtures.

Follow-up 2026-08-28 `SimpleImmutableEntry` pass:
`AbstractMap.SimpleImmutableEntry<K,V>` now follows the same tuple key/value
carrier as `AbstractMap.SimpleEntry<K,V>` for construction, display, and
entry-set equality checks. This covers the immutable JDK pair shape without
adding a separate opaque facade or erased-object display fallback. Covered by
`lower::tests::simple_immutable_map_entry_uses_same_key_value_pair_path` and
`tests/differential/MapEntrySimpleImmutableEntry.java` (`xa=1,true,false`).
The differential suite is now at 218 fixtures.

Follow-up 2026-08-28 internal `Map` carrier pass:
private source methods declared against broad collection interfaces can now
specialize their emitted Rust parameter carrier from unambiguous internal
call-site evidence. A local such as
`Map<String,Object> map = new LinkedHashMap<String,Object>()` records the
constructor-refined `LinkedHashMap` carrier in signature metadata, and a
callee `Map<String, ? extends Object>` parameter adopts that carrier only when
all observed source call sites agree. Public API methods stay on their declared
surface type. Covered by
`tests/idiom_gaps.rs::map_entry_enhanced_for_uses_planned_iterator_and_tuple_fields`,
which now pins `values: &JavaLinkedHashMap<...>` and generated-crate
compilation.

Follow-up 2026-08-28 aliased entry-set iteration pass:
enhanced-for over a tracked `Set<Map.Entry<K,V>>` local initialized from
`map.entrySet()` now uses the same live backing-map iterator as the direct
`for (... : map.entrySet())` form. The alias no longer falls through to the
ordinary set value, which would snapshot the entry view and miss later backing
map mutations. Covered by
`lower::tests::map_entry_set_alias_iteration_uses_live_backing_map`.
Remaining entry residue is still arbitrary opaque `Map.Entry` values whose
key/value or backing map cannot yet be extracted structurally.

Follow-up 2026-08-28 `Collection.removeAll` return pass:
`List.removeAll` and set-like `removeAll` now have an expression-valued native
collection plan instead of only a statement-lowering special case. The
expression path mirrors the existing statement semantics by binding the source
collection, retaining values not contained in it, and returning whether the
receiver length changed. Covered by
`tests/idiom_gaps.rs::collection_remove_all_return_uses_boolean_collection_contract`
with generated-crate compilation. The receiver/source self-alias shape is also
handled before argument-slot adaptation: `values.removeAll(values)` lowers to
`clear()` and expression position returns whether the collection had any
elements. Covered by
`tests/idiom_gaps.rs::collection_remove_all_self_alias_clears_and_returns_changed`.

Follow-up 2026-08-28 `Map.values().remove` mutable-borrow pass:
values-view removal now uses the same backing-map mutable borrow adapter as the
entry-set mutation paths. A backing map that is already a mutable-borrowed
local lowers through `&mut *values` instead of layering a raw `&mut values`
borrow on top of the reference. Covered by
`lower::tests::map_values_remove_borrows_mutable_backing_map_without_double_reference`.

Follow-up 2026-08-28 `HashSet<>(map.entrySet())` constructor pass:
the native collection constructor bridge now recognizes `entrySet()` when the
target constructor is set-like and lowers it through the existing owned
map-entry set projection. `new HashSet<Map.Entry<K,V>>(map.entrySet())`
therefore collects `(K, V)` tuples from the backing map instead of falling
through to an unresolved facade call. Covered by
`tests/idiom_gaps.rs::map_entry_set_constructor_collects_hashset_for_set_slot`
with generated-crate compilation.

Follow-up 2026-08-29 typed `Map.Entry` display pass:
map-entry display now reuses the stable typed entry key/value projection that
already backs `entrySet().contains(entry)` and `entrySet().remove(entry)`.
Tracked `SimpleEntry` locals still use their recorded constructor key/value
expressions, but a typed parameter or field with a recoverable tuple carrier
formats as `key=value` instead of falling back to erased-object display.
Covered by
`lower::tests::typed_map_entry_parameter_display_uses_key_value_projection`.

Follow-up 2026-08-29 non-stable `Map.Entry` display pass:
call expressions and other non-stable expressions with a recoverable
`Map.Entry<K,V>` tuple carrier now lower once into a temporary before
formatting `.0` and `.1` as `key=value`. This closes the method-return shape
that could not use the stable var/field projection path and otherwise fell
through to tuple display. Covered by
`lower::tests::map_entry_return_call_display_uses_single_tuple_projection`.
Remaining entry residue is still arbitrary opaque `Map.Entry` values whose
carrier cannot be decomposed structurally.

Follow-up 2026-08-29 map-view identity pass:
tracked `keySet()`, `values()`, `entrySet()`, `Map.Entry` value receivers, and
`SimpleEntry` constructor values are now stored by resolver `BindingId` when the
view local is bound. Id-less compatibility locals still use the legacy name map,
but a same-name redeclaration no longer inherits a stale backing map or
constructor key/value pair from an earlier local. Covered by
`expr_shape::tests::bound_map_view_facts_do_not_fall_back_to_same_name_local`.

Follow-up 2026-08-29 `Map.Entry` carrier audit:
the suspected opaque typed-entry display residue is not valid against the
current slot renderer. `java.util.Map.Entry<K,V>` is intentionally represented
as `(K,V)` both at top-level slot positions and nested generic-argument
positions, so the typed display helper's `.0`/`.1` projection is backed by the
same carrier invariant as `entrySet().contains(entry)` and `setValue` lowering.
Covered by
`slot_render::tests::map_entry_renders_as_tuple_at_slot_and_generic_positions`.
The remaining entry residue is limited to expressions that are not typed as
`Map.Entry<K,V>` or cannot otherwise expose a key/value pair.

Follow-up 2026-08-30 local `SimpleEntry.setValue` pass:
locals initialized from `new AbstractMap.SimpleEntry<>(key, value)` now lower
`entry.setValue(value)` against the tuple carrier directly: clone field `.1`,
assign the new value, and return the old value. Locals initialized from
`new AbstractMap.SimpleImmutableEntry<>(...)` keep the same tuple carrier for
display/equality, but `setValue` now raises `UnsupportedOperationException`
instead of mutating the tuple or falling through to an invalid tuple method
call. Covered by
`lower::tests::simple_map_entry_local_set_value_mutates_tuple_and_returns_old_value`
and
`lower::tests::simple_immutable_map_entry_set_value_raises_unsupported_operation`.

Follow-up 2026-08-29 platform erased-display coverage:
the platform erased-object display table already includes the modeled JDK
carriers used by current corpus fixtures. Added generated-crate checks for
`File.toPath()`/`Path`, `URL`, and `Random` erased through `Object`; together
with the existing `File`, `URI`, and `System.in`/`InputStream` checks, this
keeps the loud erased fallback limited to unsupported runtime carriers.
Covered by
`tests/slot_shape_design.rs::erased_path_object_display_delegates_to_path_display`,
`tests/slot_shape_design.rs::erased_url_object_display_delegates_to_url_display`,
and
`tests/slot_shape_design.rs::erased_random_object_display_uses_platform_identity_display`.

Follow-up 2026-08-29 plain `Object` erased boundary check:
the `Object` constructor fixture now asserts the current runtime carrier:
`new Object()` erases to `Box::new(JavaPlainObject::new())` rather than the old
unit placeholder. This keeps default object display/identity support observable
through the erased boundary. Covered by
`tests/slot_shape_design.rs::java_object_return_type_lowers_to_erased_any_boundary`.

Follow-up 2026-08-29 live `Map.values()` view mutation pass:
`Collection.removeAll` on a tracked `Map.values()` view now mutates the backing
map rather than a materialized values snapshot. The lowering stores the removal
collection once, borrows the backing map once, retains entries whose values are
not in the removal collection, and returns whether the backing map length
changed. `JavaLinkedHashMap` gained the same `retain` support method so native
and insertion-ordered map carriers share the expression shape. Covered by
`lower::tests::map_values_alias_remove_all_mutates_backing_map` and
`tests/differential/MapValuesRemoveAll.java` (`true,false,1,1,false,true` on
both Java and generated Rust).

Follow-up 2026-08-29 live `Map.values()` unsupported mutator pass:
`Collection.add` and `Collection.addAll` on a tracked `Map.values()` view now
raise `UnsupportedOperationException` instead of falling through to native
collection mutation on a materialized values snapshot. The lowering evaluates
the backing map and arguments, then emits a Java exception result in catchable
contexts or the existing uncaught-exception panic recovery outside them.
Covered by
`lower::tests::map_values_alias_add_throws_unsupported_operation_exception` and
`tests/differential/MapValuesAddUnsupported.java`
(`add-unsupported,1,false,addAll-unsupported,1,false` on both Java and
generated Rust).

Follow-up 2026-08-29 display-sink baseline cleanup:
the display-sink inferred method surface tests now both pass in the current
tree. `sequenceFile.name()` and `record.sequenceFile.name()` used as
`StringBuffer.append(...)` arguments both emit a `SequenceFile::name() ->
String` facade surface rather than a unit-returning stub. The stale
`omitted_context_receiver_method_used_in_display_sink_gets_string_stub` entry
was removed from `tests/baselines/idiom-gaps-failures.txt`; no code change was
needed for this subcase. The audit remains open for erased/unknown display
capability and opaque `Map.Entry` values without recoverable key/value identity.

Follow-up 2026-08-29 package-qualified external display pass:
modeled external `toString(): String` display capability is now scoped to the
fact owner rather than the owner's simple name. This keeps erased-object display
support for the modeled package-qualified carrier while preventing a same-simple
package peer from inheriting a downcast arm it cannot satisfy. Covered by
`tests/slot_shape_design.rs::erased_external_object_display_keeps_package_qualified_to_string_capability_distinct`.
The remaining erased-display residue is still unsupported runtime carriers with
no source/platform/model fact evidence.

Follow-up 2026-08-29 live `Map.keySet()` `removeAll` pass:
`Collection.removeAll` on a tracked `Map.keySet()` view now mutates the backing
map rather than a materialized key snapshot. The lowering stores the removal
collection once, borrows the backing map once, retains entries whose keys are
not in the removal collection, and returns whether the backing map length
changed. Covered by
`lower::tests::map_key_set_alias_remove_all_mutates_backing_map` and
`tests/differential/MapKeySetRemoveAll.java` (`true,false,1,1,false,true` on
both Java and generated Rust).

Follow-up 2026-08-29 external display capability query pass:
external facade `Display` delegation now asks
`external_type_has_display_to_string_capability(facts, ty)` directly instead of
testing membership in a pre-expanded display-capable set. The derived set still
feeds erased-object downcast generation, but both paths now share one predicate
over explicit `toString`/`to_string`, arity-0, `String`-returning facts. The
query preserves exact/package-qualified and emitted Rust owner aliases while
refusing same-simple package peers. Covered by
`stdlib::functional_interface_shape_tests::external_display_capability_query_keeps_package_peers_distinct`,
`tests/slot_shape_design.rs::erased_external_object_display_without_modeled_to_string_stays_loud`,
`tests/slot_shape_design.rs::erased_external_object_display_delegates_to_modeled_to_string_surface`,
and
`tests/slot_shape_design.rs::erased_external_object_display_keeps_package_qualified_to_string_capability_distinct`.

Follow-up 2026-08-29 platform erased-display policy cleanup:
the remaining platform erased-display table is now named as
`PLATFORM_ERASED_DISPLAY_TYPES` and queried through
`platform_type_has_erased_display_support`, with the special mention rules
(`System` implies `InputStream`, `File` implies `Path`) separated into
`platform_erased_display_supporting_type_mentions`. This preserves the current
JDK carrier behavior while making the platform exception policy explicit
instead of embedding it in the downcast loop. Covered by
`stdlib::functional_interface_shape_tests::platform_erased_display_support_is_explicit`
and the existing platform erased-display generated-crate tests.
Follow-up 2026-09-11: the optional platform-facade display policy expectation
now matches the shared Java display helper used by lowering. Nullable platform
facade values such as `URI` are still planned as
`OptionalToJavaDisplayValue`, and both owned and borrowed optional projections
render `value.java_arrays_deep_to_string()` before the Java `"null"` fallback
instead of asserting the stale direct `to_string()` spelling. Guard:
`type_coercion::optional_platform_facade_display_segments_use_display_value_policy`.

Follow-up 2026-08-29 atomic erased-display parity:
`java.util.concurrent.atomic.AtomicLong` and `AtomicBoolean` now have the same
platform facade and erased-object display path as `AtomicInteger`: imports
activate the support type, state access/update methods lower to cell-backed
facades, and erased `Object` display emits downcast arms for atomic handles.
Covered by
`tests/idiom_gaps.rs::java_atomic_long_uses_platform_facade_methods`,
`tests/idiom_gaps.rs::java_atomic_boolean_uses_platform_facade_methods`,
`tests/slot_shape_design.rs::erased_atomic_long_object_display_delegates_to_atomic_display`,
`tests/slot_shape_design.rs::erased_atomic_boolean_object_display_delegates_to_atomic_display`,
`tests/differential/ErasedAtomicLongDisplay.java` (`x4`), and
`tests/differential/ErasedAtomicBooleanDisplay.java` (`xfalse`).

Follow-up 2026-08-29 imported package-qualified display capability pass:
erased-object display downcast generation now keeps the original modeled
external fact owner while deciding whether a Java source mentions that runtime
carrier. A fact for `a.ExternalToken.toString(): String` therefore activates the
`AExternalToken` downcast when source code imports `a.ExternalToken` and refers
to `ExternalToken` by its simple name, while the capability table still does
not grant bare `ExternalToken` to every package peer. Covered by the strengthened
`tests/slot_shape_design.rs::erased_external_object_display_keeps_package_qualified_to_string_capability_distinct`.

Follow-up 2026-09-06: that downcast selection now also uses the Rust facade name
the importing source can actually spell. Package-qualified VCF facts activated
through `import htsjdk.variant.vcf.VCFHeaderLine;` emit `VCFHeaderLine`
downcasts instead of the package-mangled `HtsjdkVariantVcfVCFHeaderLine`, and
clone-needed optional erased-object locals keep the erased-display path instead
of mapping `Option<Box<dyn Any>>` through `value.to_string()`. Covered by
`tests/idiom_gaps.rs::erased_vcf_header_line_display_uses_typed_pack_to_string_fact`
and
`tests/idiom_gaps.rs::erased_vcf_header_display_uses_external_to_string_downcast`.

Follow-up 2026-08-29 live `Map.keySet()` unsupported mutator pass:
`Set.add` and `Set.addAll` on a tracked `Map.keySet()` view now raise
`UnsupportedOperationException` instead of falling through to ordinary
`HashSet` mutation on a materialized key snapshot. The lowering evaluates the
backing map and arguments for Java order/side effects, then emits the Java
exception result in catchable contexts or the existing uncaught-exception panic
outside them. Covered by
`lower::tests::map_key_set_alias_add_throws_unsupported_operation_exception`,
`tests/differential/MapKeySetAddUnsupported.java` (`unsupported,false,1`), and
`tests/differential/MapKeySetAddAllUnsupported.java` (`unsupported,false,1`).

Follow-up 2026-08-29 typed external collection getter pass:
body lowering no longer treats a native collection carrier change as an
erased-object return boundary. In particular, a fact-only `Box<dyn Any>` fallback
for a modeled external `Set<T>` getter and a representation-aware
`HashSet<T>` return now keep the body slot typed, so
`header.get_meta_data_in_input_order()` is returned as a
`std::collections::HashSet<VCFHeaderLine>` instead of being boxed. The borrowed
facade return recovery helper also now honors the same erased-target predicate:
concrete targets recover with `clone()`/`to_path_buf()`, while only erased
targets box into `Box<dyn Any>`. Covered by
`tests/idiom_gaps.rs::external_pack_typed_set_and_dictionary_getters_shape_facade_returns`
and the existing coercion erased-object unit coverage.

Follow-up 2026-08-29 nullable trivial generic getter display pass:
string concat/display lowering now prefers a storage-aware trivial accessor
shape before the declared method-return shape for display slots. This matters
for generic getters such as `Box<T>.get()`: the source return type is `T`, but
the lowered field carrier can still be `Option<T>`. The conversion path now
projects that optional carrier before Java display conversion instead of
rendering `Option<i32>.to_string()`. Covered by
`tests/type_coercion.rs::optional_element_borrow_display_projects_the_option_carrier`
and by direct generated-crate checks for `tests/differential/Generics.java`;
`tests/differential/LambdaMutation.java` was also rechecked to keep the recent
single-`.as_slice()` cleanup pinned.

Follow-up 2026-08-30 direct collection bulk-method pass:
ordinary `List`/set-like `Collection.containsAll` and `retainAll` now lower to
native Rust collection contracts instead of missing `.contains_all(...)` /
`.retain_all(...)` calls. `containsAll` evaluates the source once through
`.iter().all(|value| receiver.contains(value))`; `retainAll` snapshots the
source, retains matching receiver elements, and returns whether the receiver
length changed. Self-alias bulk calls are recognized through the existing
audited clone-recovery wrapper, so `values.retainAll(values)` returns `false`
without rebuilding the collection. Covered by
`tests/idiom_gaps.rs::collection_contains_all_uses_all_with_contains`,
`tests/idiom_gaps.rs::collection_retain_all_return_uses_boolean_collection_contract`,
and `tests/idiom_gaps.rs::collection_retain_all_self_alias_is_noop_false`.
Map-view `retainAll`/`containsAll` remains a separate live-view follow-up.

Follow-up 2026-08-30 map key/value view bulk-method pass:
tracked `Map.keySet()` and `Map.values()` live views now lower
`containsAll`/`retainAll` through the backing map instead of falling through to
snapshot collection pseudo-methods. Key views test required keys with
`contains_key` and retain map entries whose keys are present in the source
collection; values views test each required value against the backing map's
values iterator and retain entries whose values are present in the source
collection. Read-only bulk checks borrow the map temporary so later uses are
not consumed. Covered by
`lower::tests::map_key_set_alias_contains_all_reads_backing_map`,
`lower::tests::map_key_set_alias_retain_all_mutates_backing_map`,
`lower::tests::map_values_alias_contains_all_reads_backing_map_values`, and
`lower::tests::map_values_alias_retain_all_mutates_backing_map`.

Follow-up 2026-08-30 map entry-set bulk-method pass:
tracked `Map.entrySet()` live views now lower `containsAll` and `retainAll`
through the backing map's key/value pairs. `containsAll` folds the supplied
entry collection with `all`, testing each tuple against `map.get(key)` and the
stored value. `retainAll` mutates the backing map with `retain`, keeping only
entries present in the supplied entry collection and returning whether the map
length changed. `removeAll` removes the supplied entry pairs from the backing
map. Same backing-view aliases short-circuit to Java's no-op results:
`containsAll(self)` is `true`, `retainAll(self)` is `false`. The
signature and local-mutability collectors now also recognize mutating map-view
calls such as `values.entrySet().retainAll(keep)` as mutations of `values`.
The signature pass also tracks locals initialized from a map view, so
`entries.retainAll(keep)` after `entries = values.entrySet()` gives the method
an `&mut` map parameter instead of an immutable borrow. Covered by
`lower::tests::map_entry_set_alias_contains_all_reads_backing_map_entries`,
`lower::tests::map_entry_set_alias_retain_all_mutates_backing_map_entries`,
`lower::tests::map_entry_set_alias_bulk_self_aliases_are_noops`, and
`tests/idiom_gaps.rs::map_entry_set_bulk_methods_use_backing_map_contract`.
Alias signature coverage:
`tests/idiom_gaps.rs::map_entry_set_alias_retain_all_marks_backing_map_mutable`.

Follow-up 2026-08-30 erased map-entry toArray display pass:
`Map.entrySet().toArray()` still lowers its `Object[]` carrier as boxed
`(String, String)` tuples for `Map.Entry<String, String>` values. The generated
erased-display helper now recognizes that tuple carrier and renders it with the
Java `Map.Entry.toString()` shape, `key=value`, instead of falling through to
the unsupported erased-object display panic when the array element is later
concatenated or displayed. Covered by the generated runtime assertion in
`tests/idiom_gaps.rs::collection_to_array_empty_string_array_lowers_to_owned_vec`.

Follow-up 2026-08-30 detached map-entry contract pass:
locals initialized from `values.entrySet().iterator().next()` now snapshot both
key and value into the tuple carrier, so later `entrySet().contains(entry)` and
`entrySet().remove(entry)` compare the full Java `Map.Entry` contract rather
than checking only the key against the backing map. Direct synthetic
`entrySet().contains(values.entrySet().iterator().next())` and `remove(...)`
paths now also bind and compare both fields before mutating. Covered by
`lower::tests::map_entry_set_contains_and_remove_detached_local_compare_key_and_value`,
`tests/idiom_gaps.rs::detached_map_entry_local_uses_key_and_value_contract`,
and `tests/differential/MapEntryDetachedContract.java`. A full differential
rerun in this tree still stops on unrelated StringBuilder catchability failures;
the new detached-entry fixture itself compiles and matches Java through the
focused generated-crate test.

---

## Re-verified 2026-09-02 — the five regressions, re-measured

The re-check found five previously-correct areas broken. All five reproduce, and
two are now fixed. The remaining three are pinned as differential fixtures rather
than left as prose, so they cannot regress further unnoticed.

### R1 `Object b = Col.BLUE` — **fixed**

An enum constant is written like a static field read and is not one: it is a
variant. `lower_java_enum_static_field_access_with_owner` knows this, but it was
a *sibling* of `lower_static_storage_field_access` rather than a step inside it,
so which of the two claimed the access depended on context. `Col c = Col.BLUE`
reached the enum path; `Object b = Col.BLUE` reached the static-storage path and
emitted `Col::__static_BLUE()` — an accessor that is only generated for real
static fields (`E0599`). The static-storage resolver now declines a field that is
a variant of its owner, so the enum path is the only lowering that can claim one,
in every context.

Fixing that alone turned the compile error into something worse. Java's
`Enum.toString` returns the constant's name, but enums were excluded from the
erased-object downcast table — `source_type_has_java_object_stringification`
admitted `Class` and `Record` only — so `b.toString()` fell through to the
empty-string default: Java `BLUE`, Rust `""`. A **silent wrong answer**, which
0018 ranks below the compile error it replaced. The generated enum already
carried a `Display` writing `self.name()`; it is now reachable through the erased
path. Gated by `tests/differential/EnumConstantAsObject.java` (`BLUE,RED,BLUE`,
matching the oracle).

### R3 nested-map read — **fixed**, see 0056

`m.get("a").get(1)` emitted two identical `.expect("Java null receiver")` calls,
the second landing on the `Vec` the first produced. Fixed at the constructor;
gated by `tests/differential/NestedMapValueRead.java`.

### R2 `Col.values()[i]` — **fixed**

Promoted to `tests/differential/EnumValuesElementIntoObject.java` on
2026-09-06. Source enum `values()` is now treated as the synthetic non-null
`Vec<Self>` it emits: indexing `values()[i]` uses the plain enum element shape,
while storing the full Java array into a source array slot wraps the elements
into Java's nullable reference-array carrier.

### R4 entry-set `sum += e.getValue()` — **fixed**

Promoted to `tests/differential/BorrowedElementIntoPrimitiveLocal.java` on
2026-09-06. A borrowed collection element is now recovered into a primitive-value
slot when the Java element is either the primitive itself or a boxed primitive
with the same storage type. The fixture covers both shapes of the defect — a
declared `int` local from `list.get(i)`, and the compound assignment
`sum += entry.getValue()`.

Redundancy removed while measuring this: `lower_map_entry_accessor_expr_for_slot`
contained the accessor match, the tuple projection and the slot application
written out **three times**, once per way of obtaining the receiver, and the
copies had drifted — `getKey` fell back to the slot's target type when the
entry's type argument was `Unknown` and `getValue` did not, for no reason beyond
which copy was edited last. They are one question now
(`map_entry_accessor_field` + `map_entry_accessor_projection`); a fourth receiver
form only has to say how to get the entry expression.

### R5 → gate failure, fixed under 0056.

Follow-up 2026-09-07: the checked-in expectation for
`generic_list_get_into_typed_local_recovers_owned_element` predated fallible
generic `get` lowering. The generated crate compiles with
`Result<Option<T>, JavaException>`, so the row was removed from the idiom-gaps
failure baseline.

Follow-up 2026-09-07 stream string-array cleanup: the checked-in expectation for
`stream_block_lambda_string_array_local_clones_owned_index` predated the current
nullable string-array parse lowering. The generated crate compiles while still
cloning the owned `contig` element for `SimpleInterval::new`; the parse calls now
use the optional-string display path so Java `Integer.parseInt(null)` reaches
`NumberFormatException` input `"null"`. The row was removed from the
idiom-gaps failure baseline after a focused check.

Follow-up 2026-09-07 Pattern erased-display oracle: the existing
`java.util.regex.Pattern` erased-display support is now backed by a differential
fixture, `tests/differential/ErasedPatternDisplay.java`, not just slot-shape
generated-crate coverage. Adding the oracle exposed that imported
`Pattern.compile(String)` still emitted an unmodelled external-facade stub even
though the `Pattern` carrier existed; the generated facade now delegates that
one-argument static call to `Pattern::compile`. The fixture checks both
concatenation and direct `toString()` through an erased `Object`, and the
platform-display support unit now names every table entry explicitly.

Follow-up 2026-09-07 unsupported-display hygiene: external facade `Display`
impls that intentionally panic for unknown erased-display carriers now name
their formatter argument `_f`. Modeled `toString()` and platform display impls
still use `f`, while the unsupported fallback no longer floods generated-crate
checks with `unused variable: f` warnings. Guarded by
`slot_shape_design::external_opaque_facades_support_equality_and_display_fallbacks`
and
`slot_shape_design::nullable_external_to_string_fact_does_not_mark_facade_display_capable`.

Follow-up 2026-09-07 Locale erased display: `java.util.Locale` now has a
value-carrying facade for `Locale.ENGLISH`/`Locale.US`, a `Display` impl, and a
platform erased-object downcast arm. `Object value = Locale.US; "x" + value`
now matches Java as `xen_US` instead of reaching the loud unknown-erased
display path. Guarded by `tests/differential/ErasedLocaleDisplay.java` and
`slot_shape_design::erased_locale_object_display_delegates_to_locale_display`.

Follow-up 2026-09-07 optional display support: optional Java value display now
maps present values through `JavaArraysDeepToStringElement` so nested nullable
values such as `Option<&Option<String>>` print inner `null` correctly. `String.join`
is marked as needing the string runtime helpers so the support trait is present
for null-bearing join inputs. Rechecked by the differential fixtures
`StringJoinNullElement.java` and `StringJoinVarargsNullElement.java`.

Follow-up 2026-09-07 focused erased-display recheck: the promoted erased
display fixtures were run one-by-one against Java and translated Rust rather
than relying on the full differential loop. Outputs matched for
`ErasedBitSetDisplay` (`x{2, 5},direct={2, 5}`), `ErasedClassDisplay`
(`value=class java.lang.String,direct=class java.lang.String`),
`ErasedLocaleDisplay` (`xen_US,direct=en_US`), and `ErasedPatternDisplay`
(`value=a+,direct=a+`). The remaining open 0059 surface is still genuinely
unknown erased display without source/platform/fact evidence.

Follow-up 2026-09-07 direct platform `toString` recheck: the focused
differential guard now runs the platform erased-display fixtures that had not
previously asserted direct `Object.toString()` in the same pass:
`ErasedAtomicBooleanDisplay`, `ErasedAtomicIntegerDisplay`,
`ErasedAtomicLongDisplay`, `ErasedFileDisplay`, `ErasedInputStreamDisplay`,
`ErasedPathDisplay`, `ErasedRandomDisplay`, and `ErasedUrlDisplay`. The direct
call uses the same `ObjectToString` plan as concat for these platform carriers.
`Random` needed one real cleanup: generated platform facade `toString` support
must delegate to `Display`, because the erased downcast helper calls
`value.to_string()` and an inherent unmodelled stub shadows Rust's blanket
`ToString` implementation.

Follow-up 2026-09-07 borrowed-element focused oracle: the promoted discharge
fixtures now have their own small differential loop instead of only relying on
the full fixture sweep. `NestedMapValueRead`, `EnumValuesElementIntoObject`,
and `BorrowedElementIntoPrimitiveLocal` are checked by
`tests/differential.rs::borrowed_element_discharge_fixtures_match_java_output`,
covering map-value receiver reads, enum array elements erased to `Object`, and
borrowed collection values flowing into primitive locals.

Follow-up 2026-09-07 String.join null-display oracle: the promoted
`StringJoinNullElement` and `StringJoinVarargsNullElement` fixtures now have
their own focused differential loop. This pins the optional Java display path
used by null-bearing join inputs without relying only on the full fixture
sweep.

Follow-up 2026-09-10 erased vector collection display: the erased-object
runtime helper no longer carries bespoke formatting arms for `Vec<String>`,
`Vec<Option<String>>`, and `Vec<i32>`. Those and the boxed primitive siblings
now share the erased-object support item's local collection-display helper,
including nullable primitive vectors such as `Vec<Option<bool>>`,
`Vec<Option<i8>>`, `Vec<Option<i16>>`, `Vec<Option<char>>`,
`Vec<Option<f32>>`, `Vec<Option<f64>>`, `Vec<Option<i32>>`, and
`Vec<Option<i64>>`. Promoted differential fixtures
`ErasedNullableBooleanArrayListDisplay.java`,
`ErasedNullableByteArrayListDisplay.java`,
`ErasedNullableCharacterArrayListDisplay.java`,
`ErasedNullableDoubleArrayListDisplay.java`,
`ErasedNullableFloatArrayListDisplay.java`,
`ErasedNullableIntegerArrayListDisplay.java`,
`ErasedNullableLongArrayListDisplay.java`, and
`ErasedNullableShortArrayListDisplay.java` pin the nullable boxed-primitive
list case through both concatenation and direct `Object.toString()`. The broader
unknown erased-display capability model remains open.

Follow-up 2026-09-10 cleanup: `source_erased_object_to_string_downcasts`,
`platform_erased_object_to_string_downcasts`, and
`external_erased_object_to_string_downcasts` are now guarded as private `stdlib`
implementation details, with `platform_type_has_erased_display_support` guarded
as crate-local. The erased-display model gap remains the same typed evidence
problem shared with 0056.

Follow-up 2026-09-10 borrowed platform erased-display parity:
`platform_erased_object_to_string_downcasts` now emits the same borrowed
`&T` downcast arm that source and external display downcasts already emitted.
This closes the small borrowed-platform carrier mismatch without broadening the
unknown erased-display model. Guarded by
`slot_shape_design::erased_file_object_display_delegates_to_file_display`.

Follow-up 2026-09-10 external erased-display carrier parity guard: the modeled
external `toString` fixture now requires all supported erased carrier downcasts:
bare `ExternalToken`, borrowed `&ExternalToken`, `Rc<ExternalToken>`, and
`Arc<ExternalToken>`. This is still not a fallback for unknown erased values;
it only keeps the fact-backed external display path aligned with the source and
platform carrier shapes. Guard:
`slot_shape_design::erased_external_object_display_delegates_to_modeled_to_string_surface`.

Follow-up 2026-09-10 erased-display carrier arm cleanup:
`platform_erased_object_to_string_downcasts`,
`external_erased_object_to_string_downcasts`, and source erased-display downcast
collection now share one private `erased_display_to_string_downcast_arms`
template for the supported `T`, `&T`, `Rc<T>`, and `Arc<T>` carriers. This
keeps the carrier parity policy in one place without changing the intentionally
loud unknown-erased fallback. Guard:
`layering::stdlib_erased_display_downcast_collectors_stay_internal`.

Follow-up 2026-09-11 `ArrayBlockingQueue` contract tightening: the bounded
carrier now preserves the Java ordering for the small contract surface covered
by promoted fixtures. `offer` uses a result-shaped projection internally so a
literal `null` argument throws catchable `NullPointerException` before the full
queue check; `add` applies the same null-before-capacity ordering while still
throwing `IllegalStateException` when full. The catchable constructor path for
`new ArrayBlockingQueue<T>(capacity)` validates signed `capacity <= 0` before
casting to `usize`, producing `IllegalArgumentException` in `try/catch`.
Promoted differential fixtures: `ArrayBlockingQueueBadCapacity.java` and
`ArrayBlockingQueueNull.java`, alongside the existing `ArrayBlockingQueueAdd`
and `ArrayBlockingQueueOffer` fixtures. Guards:
`idiom_gaps::array_blocking_queue_capacity_constructor_is_catchable_illegal_argument`
and
`idiom_gaps::array_blocking_queue_literal_null_add_and_offer_throw_null_pointer`.
Rechecked 2026-09-11 after the current dirty-tree audit cleanup:
`cargo build --release` followed by
`TMPDIR=$PWD/.tmp/differential-focus cargo test --release --quiet --test differential list_exception_fixtures_match_java_output -- --nocapture`
passed, covering all four promoted `ArrayBlockingQueue*` fixtures in the release
translator.

Follow-up triage 2026-09-11 comparator-backed `PriorityQueue`: this is not a
simple extension of the natural-order `Vec<T>` carrier. `Collections.sort` can
use a comparator expression statement-locally, but a priority queue must store
the comparator across later `add`/`offer`/`peek`/`poll` calls. Rechecked
2026-09-12: the checked-in fixture also depends on anonymous comparator body
lowering, and current translation reports both `unsupported_anonymous_class` and
`unsupported_priority_queue_comparator_constructor`. A correct promotion
therefore needs a representation decision for comparator storage, trait derives,
and the anonymous comparator object lowering path before promoting
`known-gaps/PriorityQueueComparatorConstructor.java`. The named-comparator
fixture isolates the carrier half, but still should stay known-gap until that
stored-comparator representation exists. Keep the current loud
`PriorityQueueComparatorUnsupported` path until that design is chosen.

Follow-up cleanup narrowed `SlotContentCapabilityPolicy` to a read-only
private-backed record. String-content and sequence-element facts still flow
through `slot_content_capability_policy_with_effective_facts`, but callers use
accessors instead of public fields. This keeps the display/sequence capability
question owned by slot rendering while preserving the existing public policy
entry point. Guard:
`layering::slot_render_default_policy_symbols_stay_effective_fact_aware`.

Follow-up cleanup did the same for `NativeNamedJavaRustTypePolicy`: the policy
projection remains public, but its raw Java name and selected Rust-type policy
are read through accessors instead of public fields. The internal type renderer
consumes the policy through an owned helper, so production no longer reaches
through `.policy`. Guard:
`layering::slot_render_policy_symbols_stay_effective_fact_aware`.

Follow-up cleanup narrowed `StreamSourceCapabilityPolicy` too. Sequence-like
stream-source capability is still computed by `stream_shape` from
`EffectiveFacts`, but the one stored `capability_element` is private-backed and
read through an accessor rather than exposed as a public mutable field. Guard:
`layering::stream_shape_stream_source_semantics_stay_effective_fact_based`.

Follow-up cleanup narrowed `BorrowedContentCapabilityPolicy` and
`BorrowedContentTypePolicy` to read-only private-backed records. Borrowed
string/sequence decisions still flow through the fact-aware `slot_policy`
functions, but callers assert through accessors instead of constructing raw
policy payloads. Guards:
`slot_shape_design::slot_policy_owns_borrowed_content_param_and_return_rules`
and `slot_shape_design::slot_policy_and_signature_solver_use_type_capability_facts`.

Follow-up cleanup removed a redundant no-package branch from
`external_display_downcast_rust_names`; the same `program_mentions_java_type`
condition had already inserted the Rust name before the package split. A related
visibility audit was checked and left unchanged:
`platform_type_has_erased_display_support` is still used by `call_plan`, and
`program_mentions_java_array_blocking_queue` by `lower`. Guard:
`stdlib::functional_interface_shape_tests::external_display_capability_query_keeps_package_peers_distinct`.

Follow-up cleanup narrowed the tiny borrowed-element target-match policy records
in `conversion_plan`: optional, vec, and set element target matches now expose
only a read-only `matches()` query instead of a public boolean payload. This
keeps the borrowed-element adapter decision as a conversion-plan question rather
than a caller-owned mutable fact. Guards:
`type_coercion::borrowed_element_target_match_policies_name_carrier_and_element_matches`
and `layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup narrowed `BorrowedElementSlotAdapterPolicy` to a
private-backed conversion-plan record. The target slot, carrier slot, element
slot, carrier kind, match bit, and optional adapter are now reached through
constructor/accessor methods rather than public mutable fields. This keeps
borrowed-element slot adaptation as one policy answer owned by
`conversion_plan`, while lowering only consumes the plan. Guards:
`type_coercion::borrowed_element_slot_adapter_policy_combines_carrier_match_and_recovery`
and `layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed the nested `BorrowedElementAdapterPolicy` too.
The carrier, element slot shape, recovery policy, and unwrap bit now flow through
read-only accessors; integration tests assert against planner-produced policies
instead of constructing raw adapter payloads. Guard:
`layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup narrowed the remaining borrowed-element adapter construction
surface. `BorrowedElementSlotAdapterPolicy::new` and
`borrowed_element_adapter_policy_with_unwrap` are crate-local now; the
helper-specific recovery assertions moved into `conversion_plan` unit tests,
while integration tests use the public planner and policy accessors.

Follow-up cleanup private-backed `CoercionOwnedRecoveryCapabilityPolicy`. The
owned-recovery capability answer still remains inspectable through read-only
`java_ty()` and `recovery()` accessors, but callers can no longer construct or
mutate the raw `{ java_ty, recovery }` payload. Guard:
`layering::coercion_plan_owns_clone_recovery_policy`.

Follow-up cleanup private-backed the facade-return coercion policies too.
`RustBorrowedFacadeReturnPolicy` and `RustOwnedFacadeReturnPolicy` are still
public plan variants for inspection, but their raw target/recovery payloads are
read through accessors and constructed only inside the crate. Guard:
`layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed `RustScalarSlotCastPolicy` as well. The Rust
scalar type name and Java primitive target are now read through accessors and
constructed only by conversion planning, keeping scalar return adaptation out of
caller-owned mutable policy payloads. Guard:
`layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed `RustUnsignedByteArrayReturnPolicy`. The
optional-return bit now has a read-only accessor and is constructed only by
conversion planning, matching the other return-adaptation policy carriers.
Guard: `layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed `PrimitiveWideningPolicy`. The primitive
source, target, and widening decision remain readable through accessors, but
callers can no longer synthesize or mutate the raw `{ from, to, widens }`
payload outside conversion planning. Guard:
`layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed `CoercionContentPolicy` and
`CoercionShapeContentPolicy`. String-content classification still remains
inspectable through `java_ty()` and `string_content()`, but callers no longer
own or mutate the raw `{ java_ty, string_content }` payloads. Guard:
`layering::dispatch_coercion_plan_owns_trait_box_and_clone_recovery`.

Follow-up cleanup private-backed `AssignmentValuePlan`. Assignment slot
selection still remains public through `slot()` and owned `into_slot()`, but
lowering and tests no longer reach into or construct the raw `{ slot }` payload.
Guard: `layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed `JavaTypeConversionPlan`. Resolver/cast
conversion classification still exposes read-only `actual()`, `target()`, and
`kind()` accessors, but callers can no longer synthesize or mutate the raw
`{ actual, target, kind }` payload outside conversion planning. Guard:
`layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed `SyntaxCoercionPlan`. Syntax coercion lowering
still reads `actual()`, `slot()`, and `policy()`, but integration tests and
callers no longer construct or mutate the raw `{ actual, slot, policy }`
payload. Guard: `layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed `CastConversionPlan` and `InstanceOfPlan`.
Cast and instanceof lowering now consume `actual()`, `target()`, `semantics()`,
`coercion()`, and `check()` instead of raw public fields; same-crate fixtures use
crate-local constructors for targeted plan setup. Guards:
`layering::conversion_plan_keeps_rust_expression_rendering_out_of_plan_layer`
and `layering::coercion_plan_owns_invalid_cast_runtime_policy`.

Follow-up cleanup private-backed `HierarchyInstanceOfPolicy`. Hierarchy
instanceof planning still exposes read-only `actual()`, `target()`, and
`always_true()`, while callers can no longer construct or mutate the raw
`{ actual, target, always_true }` payload. Guard:
`layering::cast_and_instanceof_planning_owns_runtime_check_policy`.

Follow-up cleanup private-backed `OptionalSlotConstructionPlan` and
`OptionalPresentConstructionPlan`. Optional construction lowering now consumes
read-only accessors for the target, element shape, element coercion, wrapper
ownership, element handle, construction plan, and element-lowering slot instead
of public mutable payload fields. Guard:
`slot_shape_design::optional_present_construction_policy_lives_in_conversion_plan`.

Follow-up cleanup narrowed the remaining stdlib method/classification helpers
that only serve translator internals:
`is_known_receiverless_static_utility_method`,
`has_receiver_agnostic_external_return_type`,
`is_known_object_inherited_method`, `native_collection_receiver_mutates`,
`native_collection_method_name_mutates_receiver`, and
`static_collection_argument_mutates` are crate-local now. Signature, lowering,
semantic analysis, call planning, and resolve can still share the name-only
legacy predicates, but downstream callers cannot treat them as a public
collection/JDK semantics API. Guard:
`layering::stdlib_erased_display_downcast_collectors_stay_internal`.
Follow-up cleanup also narrowed `native_collection_receiver_method_known` to
crate-local. Signature planning still uses the centralized typed collection
receiver query, but it is no longer exported as public API. In the same pass,
`canonical_external_field_name` was made crate-local; external field-name
normalization remains an internal compatibility detail for stdlib/external-pack
modeling rather than a public identity helper. Guard:
`slot_shape_design::slot_shape_design_guardrails`.
The native collection constructor/receiver kind classifiers were narrowed to
crate-local too. Public callers keep the generic
`java_collection_kind_for_type` query, while the native-carrier eligibility
rules used by call planning and lowering stay inside the crate. Guard:
`slot_shape_design::slot_shape_design_guardrails`.
The argument-level content-read table, `is_content_read_argument_method`, is
crate-local now as well. The public/test-facing boundary remains
`usage::call_argument_use_boundary_policy`, so callers ask the usage policy
question instead of depending on the raw stdlib classifier. Guard:
`slot_shape_design::slot_shape_design_guardrails`.
The external API receiver-mutability and return-nullability policy helpers were
narrowed to crate-local in the same cleanup family. Call planning and signature
planning can still share the solved JDK/external-role facts, but those
mutability/nullability classifiers are no longer exported as public stdlib
policy APIs. Guard: `slot_shape_design::slot_shape_design_guardrails`.
Follow-up cleanup also narrowed `external_api_method_role` to crate-local. The
public boundary remains `external_api_method_policy_with_builtin_packs`, which
returns the full role/mutability/nullability/provenance policy, while the
lower-level role classifier is kept as an internal shared primitive. Guard:
`slot_shape_design::slot_shape_design_guardrails`.
Follow-up cleanup narrowed the constructor/field provenance builtin-pack
switches as well:
`external_constructor_provenance_with_builtin_packs` and
`external_field_provenance_with_builtin_packs` are crate-local now. Public
callers keep the ordinary `external_constructor_provenance` and
`external_field_provenance` facades; semantic analysis remains the only caller
that needs to disable builtin external-pack facts for a controlled pass. Guard:
`layering::stdlib_erased_display_downcast_collectors_stay_internal`.

Follow-up cleanup private-backed `TypeCapabilities`. The copy/clone,
string-like, sequence-like, hierarchy, final-type, and object-safety capability
payload no longer exposes public mutable fields; slot/render/signature/stream
callers use accessors, and `EffectiveFacts` remains the central writer through
crate-local setters. Guard: `slot_shape_design::slot_shape_design_guardrails`;
compile guard: `cargo check --tests`.
