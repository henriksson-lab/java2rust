# 0056 — Nullable representation repaired for the audited surface

Closed 2026-09-21. All named repros from this audit are permanent differential
or generated-crate regressions, the differential known-gap inventory is empty,
and nullable values now remain explicit until Java's actual dereference or
unboxing boundary. Those boundaries raise catchable Java exceptions in
propagating contexts. Unknown external runtime display without source,
platform, or explicit external `toString` capability evidence remains a loud
unsupported boundary; fabricating a value there would reintroduce the silent
semantic corruption this audit was opened to remove.

Closure re-audit 2026-09-21: the superseded 2026-09-02 regressions are all
represented by current Java-oracle gates, including nullable `Map.get` locals,
idempotent nullable receiver projection, nullable array length, enum values,
nested map reads, and borrowed map-entry primitive use. The focused
`null_representation_fixtures_match_java_output`,
`nullable_map_get_fixtures_match_java_output`, and
`borrowed_element_discharge_fixtures_match_java_output` gates cover those
boundaries; the unknown external erased-display case remains deliberately loud
and has its own negative generated-crate guard. No Java fixture remains in the
executable known-gap directory.

The fresh closure run also found and repaired two current adapter gaps rather
than relying on the historical status: tracked maps promoted for live-entry
identity now acquire read/write guards before `entrySet` contains/remove view
operations, and `List<Integer>.get(...)` reaches the slot conversion plan
before the generic nullable-boxed-primitive shortcut, so assignment to an
`int` consumes `RecoverOwnedCopyValue`. The map-view, borrowed-element, and
nullable-map differential gates pass with those repairs.

Codex follow-up 2026-09-21 interface-map parameter boundary:
`InterfaceMapEntryParameterMutation.java` is promoted. A source helper taking
`Map<String, Integer>` now receives a generic tracked-map handle, so HashMap,
LinkedHashMap, and TreeMap callers retain live entry identity through the
interface slot. The same oracle covers nullable hash/linked keys and values and
a nullable TreeMap value; the helper's Java-facing `Map.Entry` contract remains
unchanged.
`InterfaceMapFieldEntryMutation.java` extends the same nullable carrier to
instance `Map`/`SortedMap` fields and a process-wide static `Map` field. Null
keys/values remain explicit while entry mutation, backing-map reads, and entry
reads observe the same tracked node.
`InterfaceMapLocalEntryMutation.java` covers nullable `Map` and `SortedMap`
locals as well; interface declaration no longer prevents promotion to the
tracked hash/sorted carrier.

The representation trigger is now derived from the native entry operation
itself: mutated/aliased entry locals, supported map parameters and fields, and
direct `entrySet().iterator().next()` arguments request the live value carrier.
The parameter and inline differential fixtures no longer contain unused
source-defined `Map.Entry` classes merely to activate that representation.
Mutable inline arguments are emitted as `&mut JavaMapEntryValue`, preserving
Java `setValue` behavior through the source helper.
Read-only native entry locals remain lightweight owned tuple snapshots. Their
accessors now clone from the owned tuple shape before nullable-to-primitive
conversion, so a null map value raises the catchable `NullPointerException`
without generating a spurious dereference of the already-owned primitive.
`NullableMapGetSwitchCatch.java` closes another non-argument coercion: both
boxed-integer and String values returned by `Map.get` are unwrapped at a Java
`switch` selector boundary. A missing key now raises a catchable
`NullPointerException`; present values select the same arms as Java. General
statement switches now share the selector-lowering path already used by switch
expressions and return/assignment switches.
`NullableMapGetPrimitiveContexts.java` extends the same copied-value unwrap to
Boolean conditions, primitive returns, primitive source-method arguments,
primitive local assignments, and array indices. The String receiver case is
covered alongside them. Missing keys raise catchable Java NPEs in every
context, while present values retain their normal primitive/string behavior;
borrowed `&T` map values no longer leak into primitive Rust slots.
The array-index case deliberately indexes an inline `new int[] {...}` value.
Fallible primitive array reads now copy the checked element inside the block
that owns or borrows the array source, rather than returning a borrow and
dereferencing it after an inline array temporary has expired.
`InlineArrayTemporaryAccess.java` extends this to reference arrays. Typed inline
array initializers now use nullable `Option<T>` element storage even without an
enclosing declaration slot, all-null initializers carry an explicit Rust type,
and a checked read consumes a disposable inline array with `into_iter().nth()`.
String, null-String/NPE, and source-object elements therefore survive the
temporary-array boundary without an invalid borrow or a non-null carrier.
The same oracle uses a side effect in an inline String-array initializer.
One-argument `substring(begin)` now binds its receiver before deriving the
implicit end index, so the Java receiver expression is evaluated exactly once.

Codex follow-up 2026-09-21: `NullableMapEntryParameterMutation.java` is
promoted to `tests/differential/`. Live native-map source parameters are now
generic over the map's Rust key and value storage, so the same Java
`HashMap<String, Integer>` helper accepts tracked maps storing nullable or
present-only keys and values. Its Java-facing live entry remains
`Map.Entry<String, Integer>` in both cases; Java and Rust print
`null:7,null:8,null:9:t` after each `setValue` returns the prior null value.
This closes the pinned nullable
map-value parameter boundary for HashMap, LinkedHashMap, and TreeMap, not
arbitrary unresolved/external map carriers.

Codex follow-up 2026-09-21 live nullable map-field boundary: HashMap,
LinkedHashMap, and TreeMap fields now retain live entry identity, and
literal-null key/value evidence shapes the tracked field carrier's generic
arguments as optional storage. `tests/differential/NullableMapFieldEntryMutation.java`
is promoted; Java and Rust print `null:7,null:8,null:9:a` while the entry's
Java-facing contract remains `Map.Entry<String, Integer>`.

Codex follow-up 2026-09-21: `NullableHashMapValueConcat.java` is promoted to
`tests/differential/` and checks nullable boxed-integer values read from
`HashMap`, `LinkedHashMap`, and `TreeMap`. A `Map.get` over an optional value
carrier now flattens the borrowed outer lookup result before Java display;
null renders as `null`, and present integers render normally.

Codex follow-up 2026-09-21: the promoted
`MixedSourceAndTreeMapEntryAlias.java` oracle now also checks a nullable value
in a live `TreeMap` entry. Assigning its `getValue()` to primitive `int`
raises a catchable Java `NullPointerException`, matching the native hash-map
and source-entry unboxing paths. This is a verified sorted-map entry slice,
not general null-coercion closure.
The adjacent `TreeMapEntryParameterMutation.java` helper returns
`entry.setValue(7)` through a boxed `Integer` return. Return analysis now
recognizes a top-level local's declared `Map.Entry` type and keeps the nullable
previous value across that source-method boundary; Java and Rust both print
`1:7:a` for the present value.

Codex follow-up 2026-09-21: a nullable `Map.Entry<String, Integer>.getValue()`
assigned to primitive `int` now unwraps through a catchable Java
`NullPointerException`. `NullableHashMapEntryUnboxingCatch.java` is promoted to
`tests/differential/` and matches Java (`caught:true`). This closes the direct
local-unboxing slice for both the owned native-entry snapshot and live
entry-value carrier; broader null
coercions and unknown erased display remain open.

The adjacent `NullableMapEntryPrimitiveReturnCatch.java` oracle now checks
nullable source `Map.Entry` values crossing primitive return, argument,
assignment, arithmetic, ordered-comparison, and numeric `switch` selector contexts. Each null case is
caught as Java `NullPointerException`; present values still produce
`four:4:4:4:5:true` on the present selector/value paths. This closes
those explicit source-entry conversions, not the wider null-coercion surface.

`NullableMapEntryStringSwitchCatch.java` checks the corresponding nullable
`String` selector: null is catchable, while `"ok"` and an unmatched present
string select their Java arms. Source-entry `getValue()` is unwrapped before
borrowing it as `&str` for the generated Rust `match`.

`NullableMapEntryBooleanConditionCatch.java` extends the same source-entry
carrier to boxed `Boolean` in `if` conditions, logical negation, and
short-circuit `&&`/`||`. Null unboxing is catchable only when that operand is
evaluated; skipped short-circuit operands do not throw. The fixture also checks
both present Boolean values.

Recorded 2026-08-23.
Status: **fixed/fenced for the audited surface: explicit nullable carriers,
catchable null failures, and loud evidence-free external boundaries**.
Severity: **medium-high, narrowed** — the original everyday null-bearing shapes
are mostly fixture-backed as explicit nullable carriers, catchable
`NullPointerException`s, or loud unsupported paths. The remaining live risk is
unknown erased-object display without source/platform/external `toString`
evidence, plus broader nullable representation work outside the already-pinned
fixtures.
Kind: semantics / lowering. **Closed.**

Codex follow-up 2026-09-21: `SourceMapEntryNullComparison.java` is promoted to
`tests/differential/` and matches Java at `true:true:true:true:true:true`. Direct
`Map.Entry` carrier calls to `getKey`, `getValue`, and `setValue` now keep their
single-layer optional result when compared to `null`, including source-defined
entry implementations. The same rule covers live native ordered-map entries;
tuple snapshots stay on their existing path. The mixed oracle additionally
checks null comparisons and catchable empty `entrySet` iteration.

The mixed oracle now sends a live native entry through a source helper that
returns `entry.setValue(10)`. A null previous value crosses the helper as
`Option<i32>`, not a bare `i32`; return-nullability planning recognizes the
`Map.Entry` accessor on an interface-typed parameter. Both Java-oracle and
signature-constraint tests pin this source-method boundary.

Codex follow-up 2026-09-21: the promoted
`tests/differential/MixedSourceAndNativeMapEntry.java` now exercises a native
entry with both null key and null value while a source `Map.Entry`
implementation is in the same program. The local ordered-map lowering path
constructs the live entry carrier without nesting optional key/value layers;
`getKey`, `getValue`, their direct comparisons to `null`, `setValue`,
`toString`, and `entrySet().contains` match Java. Broader
nullable receiver/escape shapes remain open.

Codex follow-up 2026-09-21: the native ordered-map live-entry runtime adapter
now implements `JavaMapEntry<K,V>` for bare or optional map keys and values.
Generated-crate tests verify that a null key/value crosses `getKey`, `getValue`,
and `setValue(null)` with one nullable layer, and that a detached entry keeps
its last value. This was runtime groundwork for the bounded mixed-entry
lowering path recorded above; native entry locals outside that path can still
use tuple snapshots.

Codex follow-up 2026-09-21: `NullableMapKeyConstructor.java` is promoted to
`tests/differential/`. A nullable key passed into a source constructor now
reaches its key parameter and the constructor's mutable `Map` parameter; the
caller keeps the same optional-key ordered-map carrier after construction.
`containsKey(null)`, `get(null)`, and key-set iteration agree with Java. This
extends the bounded nullable-key path through constructor calls; broader
unmodeled nullable expressions and unknown external erased display remain open.

Codex follow-up 2026-09-21: `NullableMapKeyParameter.java` is promoted to the
permanent differential suite. A nullable key local passed into a source method
now makes its key parameter nullable; `Map.put` inside that method makes the
map parameter's key carrier nullable, and the callee's requirement propagates
back to caller map locals. The same method also accepts a present string key.
The test covers variable-null, present, and literal-null keys through source
methods, including `put`, lookup, and `keySet().iterator().next()`. This is a
bounded source-method carrier path, not closure
of all nullable argument expressions or external erased display.

Codex follow-up 2026-09-21: `NullableMapKeyVariable.java` is promoted to
`tests/differential/` and matches Java at `true:false:1:null`. The map-key
constraint now follows the exact binding passed to `Map.put`, so a nullable
variable key selects `Option<String>` just as a literal-null key does. The
`keySet().iterator().next()` path preserves that optional key and converts
iterator exhaustion through the existing Java exception projection. This is a
bounded local-binding/map-view fix; nullable keys arriving through other
unmodeled argument expressions remain part of the open representation work.

Codex follow-up 2026-09-21: `NullableMapEntryValueCatch.java` is promoted to
`tests/differential/` and matches Java at
`npe:a:7:npe-key:2:1:true:2:9:1:2:0:true:true:true:true:0`. A map entry local
whose backing map stores `Option<Integer>` now keeps that optional value until
`getValue().intValue()` actually dereferences it, raising a catchable NPE.
Literal-null map keys now get their own nullable-key signature constraint,
independent of nullable map values; `getKey().length()` likewise raises a
catchable NPE instead of observing a fabricated empty string. The present
boxed-value control still reads `7`, and the nullable-key map continues to
support present/null lookup, `getOrDefault`, `containsKey`, key-set and
entry-set contains/remove, and direct removal. This closes these bounded
map-entry paths, not general null representation.

Codex follow-up 2026-09-21: source `Map.Entry` implementations now retain
nullable key/value/old-value method slots and nullable boxed locals receiving
`getKey`, `getValue`, or `setValue`. The generated-crate and Java-oracle pair
`tests/differential/SourceMapEntryNullableValue.java` checks both null-to-value
and value-to-null mutation through an interface reference, while the concrete
alias sees the same stored value. This is a bounded entry carrier, not closure
of broader null representation or unknown erased display.

Codex follow-up 2026-09-20: a nullable boxed-primitive instance field
returned by a source getter no longer becomes `Some(Option<i32>)`. The field
and return slots were both already `Option<i32>`; shared-self-field recovery
mistakenly treated the stored optional as a bare element and wrapped it again.
That branch now applies only when the actual field value is non-optional.
`NullableBoxedFieldGetter.java` checks Java/Rust `null:2` through null and
present getter calls. The focused oracle, the full
`null_representation_fixtures_match_java_output` group (319.49 s), 1,102
library tests, `cargo check --tests`, `cargo fmt --check`, and `git diff --check`
pass at this checkpoint.

Codex follow-up 2026-09-20: the same boxed-field oracle now exercises a
`replace(Integer)` method that returns the previous field value through a
local binding, including `null -> 1` and `2 -> null` transitions. The return
signature previously narrowed to `i32` and called `.expect("Java null
receiver")` on the null old value. The binding-keyed return-nullability pass
now seeds a local initialized or assigned from a typed source field using the
field's nullability fact, preserving `Option<i32>` across the method return and
its callers. Java and translated Rust both produce
`null:2:null:1:2:null`; unit guard
`signature::tests::source_return_nullability_tracks_field_value_through_returned_local`
pins the binding-level fact. The 1,103 library tests, full null-representation
Java/Rust group (319.02 s), nearby typed-map-entry and erased-source-argument
oracles, `cargo check --tests`, `cargo fmt --check`, and `git diff --check` pass
at this checkpoint.

Codex follow-up 2026-09-20: a typed `SimpleEntry<String,String>` boxed as
`Object` now retains literal-null key and value components as typed `None`
values, rather than emitting untyped `Default::default()` tuple elements that
fail to compile. The `ErasedMapEntryTypedDisplay` Java/Rust oracle checks
`null=null` through a separate `show(Object)` call. This is a bounded
constructor/erased-display case, not general nullable `Map.Entry` storage.
Adjacent mixed-element and method-return `Object[]` failures are now fixed.
An owned boxed String literal is no longer rewritten into a borrowed element;
`ErasedObjectArrayElements` covers String, integer, and null display. An
`Object[]` method return now uses an owned vector rather than an unsized slice,
and borrowed erased elements clone through the runtime helper rather than
`to_vec()`. `ErasedObjectArrayRoundTrip` checks the returned length, String
element, and null element against Java. This does not establish array alias
identity across the method boundary.

Codex follow-up 2026-09-20: nullable enhanced-for iterables now raise
catchable `NullPointerException` for null array and list receivers, while
nullable array elements remain explicit loop values. The previous borrowed
`&Option<String>` binding produced invalid `.cloned()` output on a loop-value
return. A boxed element assigned to a primitive loop variable now unboxes
lazily at each iteration and raises a catchable NPE before the body instead of
silently rendering `null` in arithmetic/string expressions. The
`NullableEnhancedForCatch.java` Java oracle covers null array/list receivers,
null element dereference, null and present boxed integers, and reassignment of
a nullable string element; it is included in the null-representation group.
The same fixture now checks source-class array elements across member writes,
read-only use, loop-variable rebinding, and an alias retained outside the
array. Enhanced-for source elements contribute a storage alias fact, selecting
shared handles where mutation requires them; cloning a loop handle therefore
preserves the array's object identity while rebinding remains local to the
loop variable.

Codex follow-up 2026-09-20: `GenericObjectsToStringDefault.java` now checks
generic present/null values through `Objects.toString(T, default)`, including a
literal-null default, a nullable `String` default parameter, and a nullable
result returned across a source-method boundary. The two-argument call no
longer falls through to an unimplemented static stub in a generic method;
nullable defaults preserve `Option<String>` rather than becoming the text
`"null"`, and return signatures carry that nullability so callers can compare
the result with Java `null`. A present `Object` holding a source-defined value
also retains dynamic display through the two-argument route.

Codex follow-up 2026-09-20: `GenericNullableFormatting.java` extends the
generic nullable-field oracle through `Objects.toString(T)` and
`String.format("%s", T)`. The latter previously emitted Rust `format!("{}",
Option<T>)`, which does not compile. Format display arguments now use the same
Java-null-aware generic helper as concat and `String.valueOf`; String, null,
boxed integer, and source-defined `toString()` values match Java.

Codex follow-up 2026-09-20: `tests/differential/NullableReceiverMatrix.java`
confirms catchable NPEs for nullable source-class locals, nullable source-call
results, `String.substring`, `StringBuilder.length`, and `List.size` receivers.
The focused oracle passes; these paths are not the remaining null boundary.
`tests/differential/GenericNullableFieldDisplay.java` pins a shared 0056/0059
case: a generic `T` field must display both present values and Java `null`.
It initially failed at `Option<T>::to_string()` and also exposed a constructor
`Option<T>::clone()` without a `T: Clone` bound. Both failures are now fixed for
the fixture's single-store constructor and generic display path.

Codex follow-up 2026-09-20: the constructor transfer half is fixed for a
single-use generic parameter stored once in a nullable field. The usage facts
record both `StoredInField` and `ContentRead` for that one assignment; treating
their length as two independent reads had inserted an invalid `Option<T>` clone.
Generic Java-null-aware display now uses the erased-object runtime dispatcher
for present `T` values and prints `null` for absent values. The fixture was
promoted to the differential suite and matches Java at
`Box(a),Box(null),Box(2),Box(payload)`.

Codex follow-up 2026-09-20: `GenericNullableStringification.java` extends
that generic field contract through `String.valueOf(Object)` and
`StringBuilder.append(Object)`. Both now use the same nullable generic display
helper as concatenation, so present String/integer/source values and absent
values match Java (`x:x,null:null,2:2,payload:payload`).

Codex follow-up 2026-09-20: `NullableReceiverMatrix` now also checks null
`String.equals`, `List.get`, `Map.get`, and `Set.contains` receivers against
Java. Instance `String.equals` first requires a present receiver and raises a
catchable `NullPointerException`; static `Objects.equals` remains null-safe.
The latter exposed a separate erased-boundary bug: a declared `String` local
was solved as `Option<Box<dyn Any>>`. Local constraint normalization now pins
concrete `String` dispatch as the existing parameter normalizer does, and
`NullableObjectsEquals.java` is promoted with Java/Rust output `false`.

Codex follow-up 2026-09-20: the expanded `NullableReceiverMatrix` now checks
instance `Object.equals` separately from static `Objects.equals`. A null
`Object` receiver throws a catchable `NullPointerException`, including when
the argument is `null`; `equals(null)` on a present receiver returns `false`.
For erased `Object` values holding modeled `String` or boxed scalar carriers,
runtime equality dispatch preserves virtual String equality and boxed-type
distinctions (`Integer(2)` equals `Integer(2)`, not `Long(2)`). Unknown erased
runtime equality targets remain an explicit loud gap rather than a fabricated
Rust `==` result.

Codex follow-up 2026-09-20: `ErasedStringIdentityEquals.java` verifies that
the erased equality dispatcher recognizes the `JavaString` identity carrier:
`Object.equals` compares content while a separate `String ==` keeps reference
identity. The reused String-to-Object local handoff clones the Arc handle so
the source binding remains usable; `ErasedStringAliasDisplay.java` checks the
same handoff with the ordinary owned String carrier.

Codex follow-up 2026-09-20: a nullable erased `Object` receiver's direct
`toString()` call now raises a catchable `NullPointerException` inside a
propagating `try`, rather than panicking through `Option::expect`. The same
rewrite reaches `"x" + value.toString()` without changing ordinary
`"x" + value` null display (`xnull`).
`tests/differential/NullableErasedToStringCatch.java` runs all three paths
against Java and is registered in the null and exception fixture groups.

Codex follow-up 2026-09-20: `tests/differential/GenericErasedDisplay.java`
promotes a source generic display case shared with 0059. Java prints `xBox:Box`
for a `Box<String>` held as `Object`; previously generated Rust panicked in
`java_erased_any_to_string` because source downcast generation skipped types
with parameters. Concrete generic instantiations visible in source `new`
expressions or declared slots now gain runtime downcast arms, which call
`toString()` on the live value rather than caching a string. The Java/Rust
oracle covers explicit `Box<String>` construction and a `Box<Integer>` local
initialized with diamond syntax; both pass.

This is closed-world recovery for source-created values, not a dynamic display
contract for unknown external erased values or unresolved generic arguments.

Codex follow-up 2026-09-16 borrowed nullable return lifetime pass:
representation-aware signature rewriting now only overwrites a parameter type
when representation rendering differs from normal slot rendering. This
preserves lifetimes already added for borrowed parameters that flow into a
borrowed nullable return, so generated signatures use `&'a T` parameters
instead of returning `Option<&'a T>` from shorter-lived `&T` inputs. Guards:
`idiom_gaps::nullable_static_alias_return_emits_lifetime_for_borrowed_params`
and `differential::null_representation_fixtures_match_java_output`.

Codex follow-up 2026-09-16 path-constructor null pass: nullable reference-array
elements supplied to the modeled `File`/`Path`/`URI` constructors now unwrap at
the constructor boundary and raise the ordinary catchable
`NullPointerException`, rather than flowing through `JavaPathArg`'s historical
empty-path fallback. Guard:
`idiom_gaps::jdk_file_constructor_clones_borrowed_string_array_element`.

Codex follow-up 2026-09-14 boxed-primitive operator unbox pass:
boxed primitive calls used directly in non-argument primitive contexts now
project the nullable carrier before applying the operator. The previous lowering
planned `Integer`/`Boolean` calls as primitive for numeric/condition planning
but still emitted the raw `Option<T>` expression in `if (boxedBoolean)`,
`1 + boxedInteger`, ordered comparison, and unary `+`/`-`. In a
`try/catch (NullPointerException)` block that either failed to compile or could
not produce Java's catchable NPE. Primitive-context lowering now unwraps the
optional boxed primitive through the existing catchable Java-null projection.
Guards:
`idiom_gaps::boxed_primitive_operator_unbox_nulls_are_catchable` and
`tests/differential/BoxedPrimitiveOperatorNullCatch.java` in the focused null
representation differential loop.

Codex follow-up 2026-09-14 nullable boxed switch-selector pass:
switch selectors now share the same primitive-context nullable boxed-primitive
projection. `switch (IntegerValueReturningNull())` previously emitted
`match Option<i32>` against integer patterns, failing Rust compilation and
preventing Java's selector unboxing `NullPointerException` from reaching an
enclosing `catch`. The selector is now unboxed through the catchable
Java-null path before matching. Guards:
`idiom_gaps::boxed_primitive_switch_selector_unbox_null_is_catchable` and
`tests/differential/NullableUnboxSwitchCatch.java`.

Codex follow-up 2026-09-13 ordinary collection iterator-null receiver pass:
`ArrayList<String> values; values.add(null); String value =
values.iterator().next(); value.length()` now keeps the declared `String` local
instead of inheriting an erased-object shape from the nullable iterator element.
The value side lowers the chained collection iterator through a direct Rust
`iter().cloned().next().ok_or_else(...)` path and unwraps the nullable element
as a catchable `NullPointerException` in propagating contexts. Regression:
`tests/idiom_gaps.rs::nullable_collection_iterator_next_into_string_is_catchable`.

Codex follow-up 2026-09-13 erased Java Optional display pass:
`Optional.of(value)` and `Optional.empty()` assigned through `Object` now keep
the Java `Optional` carrier at the erased-object boundary instead of unwrapping
the present element or boxing an untyped `None`. `java_erased_any_to_string`
renders top-level erased `Option<T>` carriers as `Optional[...]` or
`Optional.empty`, including `Optional<Object>` values whose present element is
itself an erased object. Covered by `tests/differential/ErasedOptionalDisplay.java`
and `tests/differential/ErasedOptionalObjectDisplay.java` in the focused
erased-display differential loop. This does not change ordinary
nullable-reference display or the broader unknown-erased-object fallback.

Codex follow-up 2026-09-13 optional boxed-primitive display pass:
`Optional<Integer>`-shaped values in string concatenation now use the nullable
Java display chain (`Some(value)` -> value display, `None` -> `"null"`) instead
of falling through to `Option<T>.to_string()`. This fixes the concrete
`PriorityQueue<Integer>.poll()` concat path covered by
`tests/differential/PriorityQueuePollIntegerConcat.java`. The same pass also
routes optional `Float`/`Double` display through the Java float/double spelling
helpers instead of Rust `Display`, covered by
`tests/differential/PriorityQueuePollDoubleConcat.java`, and keeps
`PriorityQueue<Boolean>.poll()` on the same optional Java display path through
`tests/differential/PriorityQueuePollBooleanConcat.java`. The broader unknown
erased-display model is unchanged.

Codex follow-up 2026-09-13 erased floating-point display pass:
erased `Float`/`Double` carriers now reuse the existing Java float/double
spelling helpers in direct erased display, erased collection display, and erased
`Optional` display. This keeps `1.0` as `1.0` instead of Rust's `1` without
changing the unknown-erased fallback. Differential fixture:
`tests/differential/ErasedFloatingPointDisplay.java`.

Codex follow-up 2026-09-13 erased optional collection display pass:
`Optional.of(list)` erased through `Object` now displays the present collection
as Java does (`Optional[[a, null, c]]`) instead of falling through to the loud
unknown-erased runtime panic. The runtime support adds `Vec<T>` optional display
for already-modeled erased collection element carriers, including nullable
elements. Differential fixture:
`tests/differential/ErasedOptionalCollectionDisplay.java`.

Codex follow-up 2026-09-14 nullable erased-object local initializer pass:
`Object value = null` again stays as `None::<Box<dyn std::any::Any>>` for an
`Option<Box<dyn Any>>` local instead of being post-processed into
`Box::new(None::<Box<dyn Any>>) as Box<dyn Any>`. The latter confused Java
`Optional`-as-erased-Object boxing with ordinary nullable `Object` storage and
made `NullConcat`/`NullInstanceof` fail to compile. Guard:
`idiom_gaps::nullable_erased_object_null_initializer_stays_none_carrier`; the
focused `null_representation_fixtures_match_java_output` loop covers
`tests/differential/NullConcat.java` and
`tests/differential/NullInstanceof.java`.

Codex follow-up 2026-09-14 nullable boxed array-index pass:
array index expressions now apply Java boxed-primitive unboxing before the
`usize` cast. `Integer value = null; values[value]` therefore raises a catchable
`NullPointerException` in propagating `try` scopes instead of emitting
`Option<i32> as usize` and failing Rust compilation. Guards:
`tests/idiom_gaps.rs::boxed_primitive_array_index_unbox_null_is_catchable` and
`tests/differential/NullableUnboxArrayIndexCatch.java`.

Codex follow-up 2026-09-14 nullable boxed array-dimension pass:
array creation dimensions now use the same boxed-primitive unboxing policy while
keeping the signed Java dimension for the `NegativeArraySizeException` check.
`Integer size = null; new int[size]` therefore raises a catchable
`NullPointerException` instead of comparing/casting `Option<i32>`. Guards:
`tests/idiom_gaps.rs::boxed_primitive_array_dimension_unbox_null_is_catchable`
and `tests/differential/NullableUnboxArrayDimensionCatch.java`.

Codex follow-up 2026-09-14 nullable boxed string-index pass:
`String.charAt` index arguments now use the same index-slot lowering policy as
array indexing and facade index slots, and native `charAt(int)` no longer forces
boxed index locals into the unknown-external non-null argument default. `Integer
index = null; "abc".charAt(index)` therefore raises a catchable
`NullPointerException` before the Rust `usize` projection instead of emitting
`Option<Box<dyn Any>> as usize` or `Option<i32> as usize`. Guards:
`tests/idiom_gaps.rs::boxed_primitive_string_char_at_index_unbox_null_is_catchable`
and `tests/differential/NullableUnboxStringCharAtCatch.java`.

Follow-up 2026-09-09 focused triage: rechecked this audit together with
0059. The smallest remaining live surface is still not a safe table-arm patch:
`src/stdlib.rs::java_erased_any_to_string` only recovers carriers for values
whose concrete runtime type is known through source downcast generation,
`PLATFORM_ERASED_DISPLAY_TYPES`, or an explicit non-null external
`toString(): String` fact. Direct calls on `Object`-typed erased parameters
route through the same erased-runtime display helper; the guarding fixtures
deliberately pin that known erased receivers use the helper while missing
runtime evidence stays loud:
`tests/idiom_gaps.rs::external_object_to_string_receiver_uses_erased_runtime_display` and
`tests/slot_shape_design.rs::erased_external_object_display_without_modeled_to_string_stays_loud`.
Adding more unconditional downcast arms would not solve arbitrary external
`Object` display; the next valid fix needs a typed-erasure/display-capability
model that records which erased values can dynamically dispatch Java
`toString()`.

Follow-up 2026-09-09 compile-harness cleanup: the generated-crate harness stubs
used by three package-identity compile checks no longer implement
`java_erased_object_to_string` as `String::new()`. They now panic with the same
loud unknown-erased-display message used by runtime support, so compile-only
tests do not preserve the old silent-empty behaviour in their local scaffolding.

Follow-up cleanup 2026-09-12: `nullability_plan::TernaryNullRefinementPlan` is
crate-local now, matching its crate-local planner; the `nullability_plan`
module itself is also crate-internal. Ternary null refinement remains an
internal lowering detail rather than a public nullable-representation API.
Guard: `slot_shape_design::operator_lowering_owns_binary_and_unary_rust_spelling`.
Follow-up cleanup: `facts::external_parameter_nullability_policy` is
crate-local now. Signature, call planning, and stdlib modeling still share the
same defaulting rule for unknown external parameters, but callers no longer get
a public raw nullable-parameter policy hook separate from those planners. Guard:
`slot_shape_design::slot_policy_and_signature_solver_use_type_capability_facts`.

Follow-up 2026-09-10 compile-harness cleanup: the same loud erased-display stub
was applied to the co-translated same-simple-name package-peer slot-shape
compile harness. This was a stale test scaffold only; runtime support already
panicked loudly for unknown erased display.

Follow-up 2026-09-07 `String.join` null delimiter pass: the delimiter argument
now keeps Java's required non-null contract instead of flowing through borrowed
string-content lowering as `Default::default()`. In propagating `try/catch`
contexts, `String.join(null, "a", "b")` now raises a catchable
`NullPointerException`; null join elements still use the existing Java display
policy. Differential fixture: `tests/differential/StringJoinNullDelimiter.java`.

Follow-up 2026-09-07 erased native collection display pass: erased `Object`
display now recovers the concrete native collection carriers emitted for
`ArrayList<String>`, `LinkedHashSet<String>`, and
`LinkedHashMap<String,Integer>` instead of panicking at runtime. Promoted
differential fixtures: `tests/differential/ErasedArrayListDisplay.java`,
`tests/differential/ErasedLinkedHashSetDisplay.java`, and
`tests/differential/ErasedLinkedHashMapDisplay.java`; the focused erased-display
oracle now covers them alongside the platform display fixtures. Arbitrary
unknown erased-object display remains loud and open.

Follow-up 2026-09-07 erased `BitSet` display pass: the modeled
`java.util.BitSet` runtime facade now implements Java-like `Display` output for
set bits, and erased `Object` display includes `BitSet` downcast arms when the
program mentions the platform type. Promoted differential fixture:
`tests/differential/ErasedBitSetDisplay.java`; focused generated-crate
regression:
`slot_shape_design::erased_bitset_object_display_delegates_to_bitset_display`.
The focused erased-platform differential loop now includes this promoted fixture
and the companion Class/Locale/Pattern erased-display fixtures, so the
one-command erased-display check matches the audit's claimed coverage.

Follow-up 2026-09-07 erased `Class` display pass: erased `Object` values
holding `JavaClass<T>` now downcast through the concrete class-literal token
types present in the translated programs, not only `JavaClass<()>`. `JavaClass`
also implements Java `Class.toString()` display, and `String.class` now carries
the Java name `java.lang.String`. Promoted differential fixture:
`tests/differential/ErasedClassDisplay.java`.

Follow-up 2026-09-07 nullable external display guard: external
`toString(): String` facts now mark a facade display-capable only when the fact's
return nullability is `DefinitelyNonNull`. When a nullable `toString` facade
method is emitted, its return stays nullable, but `Display` stays on the loud
unsupported facade path instead of assuming the display contract is non-null.
Regression:
`slot_shape_design::nullable_external_to_string_fact_does_not_mark_facade_display_capable`.

Codex follow-up 2026-09-15 stale-erased-display assertion cleanup:
`idiom_gaps::exception_constructor_message_argument_is_stringified` no longer
accepts the old `java_erased_object_to_string(&e)` exception-message fallback.
The fixture now requires the current direct `e.to_string()` lowering and
explicitly rejects the stale erased-display helper spelling, so this assertion
cannot mask a regression back to the older erased-display path.

Follow-up 2026-08-30 current-tree triage:
the original broad `null` complaint is mostly stale. Bare `null`, nullable
arrays, catchable null receivers, null comparisons, and display fallbacks now
either lower through explicit optional/null carriers or fail loud where the
translator lacks a sound model. The remaining live surface in this audit is
unknown erased-object display without source/platform/external `toString`
evidence: current lowering routes it through the erased-object display helper,
which handles known downcasts and otherwise panics instead of silently returning
`""`. That is a bounded fact/platform expansion problem for known types, and a
larger runtime representation question for arbitrary unknown erased objects.
Guard: `idiom_gaps::external_object_to_string_receiver_uses_erased_runtime_display`.

Codex follow-up 2026-09-14 triage: rechecked the adjacent erased-display guards
after the direct `Object.toString()` path was tightened. The remaining
checked-in 0056/0059 display surface is still
`slot_shape_design::erased_external_object_display_without_modeled_to_string_stays_loud`:
an erased value with no source downcast, platform carrier, or modeled non-null
external `toString` evidence must stay loud. The positive neighboring
`idiom_gaps` `external_object*` guards cover direct `Object.toString()` and
modeled external display. Treat the next real fix as typed-erasure/display
capability work, not as another unconditional downcast-table arm.

Follow-up 2026-09-06 stale-baseline pass: the map-get string display segment
row was still listed as an idiom-gap failure even though current lowering uses
the borrowed optional display path and compiles:
`values.get(key).as_ref().map(|value| value.to_string()).unwrap_or_else(...)`.
The assertion now accepts that current shape. Verified:
`idiom_gaps::map_get_string_display_segment_unwraps_optional_borrow`.

Follow-up 2026-08-30 plain-object support emission pass:
`new Object()` now marks the generated crate as needing erased-object runtime
support even when the value is only stored and never displayed. The support
detector now scans field declared types and constructor target types, so
`Box::new(JavaPlainObject::new()) as Box<dyn Any>` cannot be emitted without
also emitting the `JavaPlainObject` support item. Regression:
`tests/idiom_gaps.rs::plain_object_constructor_emits_erased_runtime_support_without_display_use`.

Follow-up 2026-08-30 external-constructor borrowed argument pass:
shaped constructor arguments now reuse the method-call owned-native borrow
adapter. A Rust-crate-mapped external constructor whose manifest marks a
reference parameter as borrowed now emits `&allocator` instead of passing the
owned facade local, and `unknown` constructor-argument nullability still plans
as non-optional. Regressions:
`tests/slot_shape_design.rs::external_constructor_argument_fact_borrows_facade_parameter`
and
`tests/slot_shape_design.rs::external_constructor_argument_unknown_nullability_defaults_to_non_null`;
both were removed from the slot-shape expected-failure baseline after the
generated crate fixture gained a minimal `arrow_facade` compile stub. Broader
non-argument null representation and unknown erased display remain open.

Follow-up 2026-08-30 native map/external getter pass:
`Map.get` into a declared final external facade local now keeps the borrowed
map element shape instead of inheriting an owned `Option<T>` initializer shape;
the borrowed-element conversion also no longer treats a pre-projected
`Option<&T>::as_ref()` as sufficient when the carrier still needs a required
null unwrap. External return-nullability evidence now survives read-only
modeled-call boundary classification, so `VariantContext.getAttribute` feeds
`MaybeNull` into the assigned `Object` local. Regressions:
`tests/slot_shape_design.rs::native_map_get_into_final_facade_local_keeps_value_shape`
and
`tests/slot_shape_design.rs::variantcontext_nullable_attribute_getter_flows_into_option_slot_planning`;
both were removed from the slot-shape expected-failure baseline.

Follow-up 2026-08-29 nullable array/out-parameter pass:
nullable array argument fixtures now match the checked-array/`Result` lowering
instead of stale unchecked expectations, and the two remaining known gaps in
this cluster were removed from the `idiom-gaps` failure baseline. Static
mutating-argument analysis now treats `System.arraycopy` argument 2 as the
destination write through the same shared predicate used for `Arrays` and
`Collections`; signature analysis no longer has its narrower `Arrays.sort`
duplicate. Fallible array indexing also recognizes nullable sequence receivers
through shaped/grouped/`?` wrappers, avoiding the invalid
`&input.as_ref().as_ref().ok_or_else(...)?` borrow for `Option<&[T]>`.
Regressions:
`tests/idiom_gaps.rs::local_array_argument_to_nullable_out_parameter_stays_owned_vec`,
`tests/idiom_gaps.rs::nullable_mutable_array_helper_return_count_keeps_caller_storage_owned`,
`tests/idiom_gaps.rs::nullable_read_only_array_parameters_lower_as_optional_borrowed_slices`,
`tests/idiom_gaps.rs::nullable_out_parameter_reborrows_for_recursive_call_before_reuse`,
and
`lower::tests::fallible_array_index_read_collapses_redundant_catchable_optional_sequence_as_ref`.
The audit remains open for broader non-argument null representation and unknown
erased display.

Correction 2026-09-07: `nullable_borrowed_array_param_passes_to_slice_helper_without_extra_as_slice`
was no longer a release `idiom_gaps` failure and has been removed from the
checked-in failure baseline.

Follow-up 2026-08-29 write-side nullable array receiver pass:
optional mutable sequence receivers now use the same catchable Java-null
projection as optional read receivers when lowering inside a propagating
`try/catch` context. A write such as `values[0] = 1` where `values` is a null
array now emits `values.as_mut().ok_or_else(...NullPointerException...)?`
instead of panicking through `.expect("Java null receiver")`, while
non-propagating contexts keep the existing local unwrap. Regression:
`tests/idiom_gaps.rs::nullable_array_write_null_receiver_in_try_is_catchable`.

Follow-up 2026-08-29 source receiver mutability in while-assignment pass:
signature planning now applies parsed source-method receiver mutability to
parameters used as method receivers, not only external receiver effects. A
parameter `Cursor cursor` passed to `cursor.read()`/`cursor.advance()` now
becomes `&mut Cursor` when those source methods mutate `self`, including when
the receiver call is nested inside a while-condition assignment. This removes
the generated `cannot borrow *cursor as mutable, as it is behind a & reference`
failure from `tests/differential/WhileAssignmentNullVariantsRuntime.java`.
Regressions:
`tests/slot_shape_design.rs::signature_plan_refines_assignment_null_loop_return_to_non_optional`
and `tests/idiom_gaps.rs::documents_while_assignment_null_variants_gap`; the
runtime fixture was also regenerated and compiled as a Rust library.

Follow-up 2026-08-29 primitive parse nullable receiver pass:
primitive parse lowering now distinguishes Java's two null contracts. Integral
`Byte`/`Short`/`Integer`/`Long` parse calls route nullable string receivers
through the Java `"null"` input spelling so null is caught as
`NumberFormatException`; `Float`/`Double` parse calls keep Java's
`NullPointerException` behavior by lowering optional receivers through a
catchable `ok_or_else(...NullPointerException...)?` in propagating contexts.
The nullable integral rewrite was consolidated in `coercion` and reused by both
native-call lowering paths, removing the previous `lower.rs` duplicate.
Regressions:
`call_lowering::tests::integral_parse_native_fallback_maps_optional_string_arg_to_number_format_input`,
`coercion::tests::primitive_parse_projection_owns_parse_call_spelling`, and the
full `tests/differential.rs::translated_output_matches_java_output` suite
covering `IntegerParseNull`, `LongParseNull`, `NarrowParseNull`,
`FloatingParseNull`, and `IntegralParseRadix`.

Follow-up 2026-08-29 `Objects.nonNull`/`Objects.isNull` pass:
native `java.util.Objects` null predicates now avoid the nullable-argument ABI
when the source argument already has a Rust `Option` carrier. Already-lowered
`Some(value)` arguments fold to constants, shaped `value.as_ref()` arguments are
normalized back to `value.is_some()`/`value.is_none()`, and definitely non-null
borrowed parameters fold to `true`/`false`. Regression:
`tests/idiom_gaps.rs::java_objects_null_predicates_lower_from_rust_option_shape`.
Broader non-argument catchable null coercions and unknown erased display remain
open.

Follow-up 2026-08-29 primitive `Map.get` required-slot pass:
primitive values read from optional borrowed map entries now use the same
catchable null rewrite in propagating ternary/required-slot contexts as the
arithmetic path. `values.containsKey(k) ? values.get(k) : 0` inside a
`try/catch (NullPointerException)` lowers the `.copied().expect("Java null
receiver")` branch through `ok_or_else(...NullPointerException...)?`, so the
catch block can observe the Java null unboxing failure. Regressions:
`tests/idiom_gaps.rs::map_get_in_ternary_primitive_slot_unwraps_and_copies_value`,
`tests/idiom_gaps.rs::map_get_in_ternary_primitive_slot_null_unwrap_is_catchable`,
and
`tests/idiom_gaps.rs::map_get_primitive_value_in_arithmetic_unboxes_optional_borrow`.

Follow-up 2026-08-28 nullable OutputStream receiver pass:
nullable JDK facade receivers for result-returning calls now use the fallible
optional receiver projection when the current context can propagate
`JavaException`. `OutputStream out = null; out.write(decoded);` in a throwing
method now lowers through `ok_or_else(|| JavaException::new("NullPointerException", ...))?`
before calling the selected byte-slice overload, instead of panicking through
`.expect("Java null receiver")`. Regression:
`tests/idiom_gaps.rs::optional_output_stream_write_byte_array_uses_slice_overload`.
Broader non-argument catchable null coercions remain open.

Codex follow-up 2026-09-14 direct boxed-unbox pass: owned optional unwraps now
carry the same `"Java null receiver"` marker as borrowed optional unwraps.
`Integer value = null; try { int primitive = value; ... } catch
(NullPointerException ex) { ... }` no longer lowers to `value.unwrap()`, which
panicked before the Java catch machinery could observe it. In propagating
contexts the existing catchable-null rewrite now turns the direct local unbox
into `ok_or_else(...NullPointerException...)?`. Regressions:
`tests/idiom_gaps.rs::boxed_primitive_local_unbox_null_is_catchable` and
`tests/differential/NullableUnboxCatch.java`.

Follow-up 2026-08-28 source-class-bound return pass:
`T extends SourceClass` trait surfaces no longer force declared reference
returns back to non-null. The class-bound trait declaration now widens its
return shape from valid implementations, forwarding impls skip the old
`.expect("Java null receiver")` when the trait return is already optional, and
type-parameter-bound call shape/nullability lookup now trusts the solved bound
method signature instead of injecting a definitely-non-null policy. Regression:
`tests/idiom_gaps.rs::source_generic_class_bound_method_call_uses_bound_surface`;
the nearby display path remains covered by
`tests/idiom_gaps.rs::optional_type_param_bound_call_display_uses_bound_return_shape`.
This closes the concrete source-bound trait unwrap found by the 0056 audit;
broader non-argument catchable null coercions and unknown erased display remain
open.

Follow-up 2026-08-28 erased `Path` display pass: platform erased-object display
support now includes the built-in `java.nio.file.Path` facade. The support
emitter also treats `File` mentions as sufficient to include the `Path` downcast
because `File.toPath()` can produce an erased `Path` without a declared `Path`
slot. Covered by `tests/differential/ErasedPathDisplay.java` (`xabc`). Unknown
external erased display without source/platform/downcast evidence remains open.

Follow-up 2026-08-28: assignment clone recovery now includes nullable owned
values whose optional element is clone-recoverable. A constructor can assign a
nullable `String` parameter into one field and still read/assign it later
without moving the `Option<String>` out of the parameter. Regression:
`tests/idiom_gaps.rs::source_nullable_field_getter_returns_optional_string`.
This narrows a compile failure in nullable constructor/body lowering; it does
not close the broader null representation or unknown erased-display model work.

Follow-up 2026-08-29: the matching context-source nullable getter fixture now
passes with the same optional field shape and no longer belongs in the
`idiom-gaps` failure baseline. Removed the stale
`context_source_nullable_field_getter_keeps_optional_field_shape` baseline entry
after verifying it alongside
`source_nullable_field_getter_returns_optional_string`. This is baseline
hygiene for the existing nullable getter mitigation; broader non-argument
catchable null coercions and unknown erased display remain open.

Follow-up 2026-08-26: catchable Java-null rewriting now covers more structured
expression positions that could still preserve `.expect("Java null receiver")`
inside a fallible context: the function operand of a structured call,
unsupported Java array-creation dimensions, and nested expressions inside
`expr?`. A direct `value.expect("Java null receiver")?` rewrite collapses to a
single propagated `ok_or_else(...)?` instead of producing a double try. These
are still within the existing mitigation, not a complete null model.
Regressions:
`lower::tests::make_java_null_expect_catchable_recurses_into_call_function`,
`lower::tests::make_java_null_expect_catchable_recurses_into_unsupported_array_dimensions`,
`lower::tests::make_java_null_expect_catchable_recurses_inside_try_expression`,
and
`lower::tests::make_java_null_expect_catchable_collapses_direct_try_null_expect`.

Follow-up 2026-08-26 external erased display pass: a typed external value
erased behind `Object` can now recover a display-capable external facade when
the external pack declares a zero-arity `toString(): String` fact. The VCF and
SAM interval pack entries now use a dedicated typed `toString` fact helper for
`VCFHeader`, `VCFHeaderLine`/subclasses, and `Interval`; display capability
keeps source/simple facade spellings as well as canonical aliases; and
erased-object downcast generation skips dotted fact keys so it does not emit a
Rust type that the fixture did not generate.
`erased_vcf_header_display_uses_external_to_string_downcast` and
`erased_vcf_header_line_display_uses_typed_pack_to_string_fact` check the
generated support and compile it. This is still a capability-based mitigation;
genuinely unknown erased external display remains open when no `toString` fact
or source/downcast evidence exists.

Follow-up 2026-08-26 constructor guard pass: normal constructors whose body
contains representable side-effect statements no longer collapse directly to a
field-plan struct literal. They now seed `__java_self`, lower the Java body in
source order, and return afterward, so a copy-constructor guard like
`if (other == null) throw ...` executes before copied-field reads. Constructor
fallibility propagation now also recognizes `Expr::New` and source-constructor
`CallableKey`s use the emitted Rust constructor name, closing the call-site gap
where a fallible constructor inside `try` was called without `?`. Regressions:
`guarded_nullable_copy_constructor_reads_source_fields_after_unwrap`,
`guarded_nullable_copy_constructor_preserves_optional_source_fields`, and
`guarded_nullable_copy_constructor_throw_is_catchable_at_call_site`.

Updated 2026-08-24: two silent-wrong paths are now loud or narrower. A
reference-typed `== null`/`!= null` operand with no nullable representation now
emits `compile_error!` instead of folding to `false`/`true`; primitives and
fresh construction expressions still fold. Concrete `JavaType::Named` values in
string concatenation no longer route through `java_erased_object_to_string`, so
source classes with an emitted `toString()` use that method for concat. The
audit remains open because unknown erased `Object` display still has no
capability model and the repro corpus has not been promoted wholesale.

Updated 2026-08-24 again: nullable erased-object display is now represented
separately from ordinary optional display. `Object o = null; "c" + o` lowers to
`o.as_ref().map(|value| java_erased_object_to_string(value)).unwrap_or_else(|| "null".to_string())`,
so the `None` case follows Java's `"null"` spelling without requiring
`Box<dyn Any>: Display`. `tests/differential/NullConcat.java` is promoted and
passes the Java oracle (`anull,bnull,cnull,true`) as part of the 41-fixture
differential suite. The present-erased source-object case was later promoted as
`tests/differential/ObjectToStringConcat.java`.

Updated 2026-08-24 third pass: two more null literal/argument repros are
promoted. Bare null in a display slot now has a literal-slot plan and renders as
`"null".to_string()` instead of `java_erased_object_to_string(&None)`.
Nullable string parameters used in concat now lower as `Option<&str>` and use
the same optional string display projection as owned nullable strings.
`tests/differential/NullLiteralConcat.java` and
`tests/differential/NullArgLiteral.java` both pass the Java oracle as part of
the 43-fixture differential suite.

Updated 2026-08-24 fourth pass: §5f's representation-instability repro is now
stable. `Map.get` locals in `MapGetNull2.java` both lower as `Option<String>`,
the unrelated `out` local stays `String`, and the nullable display path prints
`null` only for missing values. `tests/differential/MapGetNull2.java` is
promoted and passes the Java oracle as part of the 44-fixture differential
suite.

Updated 2026-08-24 fifth pass: §5b's borrow-planned return literal repro is
now stable. Optional string-view construction normalizes
`Some("literal".to_string().as_str())` to `Some("literal")`, including the raw
expression shape produced by this return path. `tests/differential/NullDefaultReturn.java`
is promoted and passes the Java oracle as part of the 45-fixture differential
suite.

Updated 2026-08-24 sixth pass: present-erased object display is no longer a
silent empty string. `java_erased_object_to_string` now panics with an explicit
unsupported-erased-display message, and generated `JavaErasedObjectFacade`
`toString` fallbacks do the same instead of returning `Default::default()`. This
was a mitigation; a later pass added source downcast recovery and promoted
`ObjectToStringConcat.java`.

Updated 2026-08-24 seventh pass: three verified-correct null fixtures are
promoted from repro-only coverage: `NullBasics.java`, `NullFieldDefault.java`,
and `NullObjectReturn.java`. The then-current differential suite gated 77 fixtures,
including null locals and comparisons, default-null reference fields, nullable
string concat, and nullable source-object returns. Promoting
`NullObjectReturn.java` exposed and fixed a stale optional-receiver projection:
nullable source-object field reads emitted
`as_ref().expect("Java null receiver").expect("Java null receiver").field`.
The cleanup pass now strips that redundant second null expect through field and
index projections.

Updated 2026-08-24 eighth pass: `NullFieldCtorOverload.java` is promoted from
repro-only coverage. The then-current differential suite gated 78 fixtures, including
the case where one constructor leaves a reference field at Java's default
`null` while another constructor initializes the same field.

Updated 2026-08-24 ninth pass: `NullDeref.java` is promoted into
`tests/differential/`. Optional receiver adaptation in fallible lowering
contexts now emits `ok_or_else(|| JavaException::new("NullPointerException",
...))?` instead of `.unwrap()`/`.expect("Java null receiver")`, so a null
dereference inside `try` reaches a translated `catch (NullPointerException e)`.
The then-current differential suite gated 79 fixtures.

Updated 2026-08-25: erased platform display is narrower again. The runtime
`java_erased_any_to_string` fallback now recognizes Rust primitive carriers for
Java boxed values (`char`, integer widths, and floats) in addition to `String`
and `bool`, recursing through nested `Box<dyn Any>` as before.
`tests/differential/ErasedBoxedPrimitiveDisplay.java` covers `Object i = 7`,
`Object c = 'A'`, concatenation, and direct `toString()` dispatch. The
102-fixture differential suite matches the Java oracle. The audit still stays
open for genuinely unknown erased/source-external display capability.

Updated 2026-08-25 again: erased platform display also covers
`StringBuilder`/`StringBuffer` runtime carriers, including shared `Rc`/`Arc`
handles when the source program mentions those platform types. The downcast arms
are emitted conditionally so fixtures without string-buffer support do not gain
unresolved Rust type references. `tests/differential/
ErasedStringBuilderDisplay.java` covers erased concat plus direct erased
`toString()` for both carriers, and the 112-fixture differential suite matches
the Java oracle. Unknown/source-external erased display remains open.

Updated 2026-08-24 tenth pass: `NullCtorArg.java` is promoted into
`tests/differential/`. Nullable source-method call results passed to a resolved
source constructor now mark the target constructor parameter nullable, and a
nullable constructor binding stored directly into a field marks that field
nullable. The fixture lowers `Holder(String n)` as `Option<String>` storage and
matches Java's `true,null,false,x`. The then-current differential suite gated 80
fixtures.

Updated 2026-08-24 eleventh pass: `NullInList.java` is promoted into
`tests/differential/`. A native collection local that receives a literal `null`
element now plans its element as nullable, so `List<String>` with `xs.add(null)`
lowers as `Vec<Option<String>>`. Non-null-refined borrowed optional loop
variables use `as_ref().unwrap()` instead of moving through `unwrap()`, and
`list.get(i) == null` on nullable elements now tests the element after the
bounds `Option`, not the bounds `Option` itself. The fixture matches Java's
`aN,2,true`. The then-current differential suite gated 81 fixtures.

Updated 2026-08-24 twelfth pass: `NullStringArray.java` is promoted into
`tests/differential/`. Reference-typed Java arrays now render their storage
element as `Option<T>`, array creation defaults those elements to `None`,
indexed writes construct `Some(value)`, indexed reads display `null` for empty
slots, and enhanced-for loop variables over reference arrays inherit the
nullable element shape. The fixture matches Java's `true,x,null,2`. The
then-current differential suite gated 82 fixtures.

Updated 2026-08-24 thirteenth pass: `ObjectToStringConcat.java` is promoted
into `tests/differential/`. Present source objects erased behind `Object` now
recover declared source `toString()` through generated runtime downcast arms.
The fixture matches Java's `aP3,bP4,P4,P3`. The differential suite now gates
83 fixtures.

Updated 2026-08-24 fourteenth pass: `NullArgCatch.java` is promoted into
`tests/differential/`. A source method that dereferences a nullable reference
parameter now seeds `NullPointerException` fallibility unless the dereference is
locally caught; callers can then propagate that `Result` into translated
`try/catch` instead of panicking at `.unwrap()`. The fixture matches Java's
`npe`. The then-current differential suite gated 84 fixtures.

Updated 2026-08-24 fifteenth pass: `NullCallArgCatch.java` is promoted into
`tests/differential/`. Slot coercions for call and constructor arguments now
rewrite structured `.expect("Java null receiver")` projections to
`ok_or_else(|| JavaException::new("NullPointerException", ...))?` while the
enclosing block is propagating Java exceptions, so a nullable source-call result
passed through a required argument slot can be caught by translated `try/catch`.
The fixture matches Java's `npe`. The then-current differential suite gated 87 fixtures.

Updated 2026-08-24 sixteenth pass: `IntegerParseNull.java` is promoted into
`tests/differential/`. A nullable `Integer.parseInt` argument now maps null to
the parse input `"null"`, so Java's `NumberFormatException` contract wins over
the normal required-reference `NullPointerException` coercion. The fixture
matches Java's `nfe,nfe,ok`. The then-current differential suite gated 89
fixtures.

Updated 2026-08-24 seventeenth pass: `NullInList.java` is stable again under
the full differential harness. The display-argument shortcut for erased-object
facades no longer captures null-guarded nullable-`String` ternaries before the
slot-aware ternary lowering can run. `StringBuilder.append(s == null ? "N" : s)`
now lowers both branches to owned `String` values instead of mixing `String` and
`&String` inside the Rust `if`. The 92-fixture differential suite passes.

Updated 2026-08-25: the self-referential `Node next` compile row is fixed and
promoted as `tests/differential/LinkedNode.java`. Optional receiver field reads
now unwrap the receiver before primitive field access, recursive optional-field
writes put the value inside `Box::new` before `Some`, and optional boxed field
reads can flow back into an owned optional local. The fixture still emits the
existing SharedLocal alias warning from 0002, but this case does not mutate
through the copied alias and now matches Java.

Updated 2026-08-25 again: the catchable Java-null rewrite now recurses into
structured closure bodies as well as match arms and block statements. This closes
one more narrow path where an argument/value coercion inside a generated closure
could keep `.expect("Java null receiver")` instead of becoming a propagated
`NullPointerException`.

Updated 2026-08-25 display follow-up: the shared erased/external-display
residual is narrower for display sinks. When a source field receiver feeds an
omitted external method call directly into a display sink, such as
`summaryText.append(record.sequenceFile.name())`, the display-hint collector now
uses the source field declaration to synthesize a `String` facade surface for the
external method. Unknown erased display remains open when no source/downcast or
display-sink type evidence exists.

Updated 2026-08-25 source Object fallback follow-up: translated source classes
that do not declare `toString()` no longer route explicit source-typed
`other.toString()` through `java_erased_object_to_string(&other)`, which required
an impossible borrowed `Any` lifetime and could panic. The source fallback now
lowers typed receivers to the generated inherent `to_string()` method, and the
erased runtime downcast table includes source classes with the Java
`Object.toString` fallback. `tests/differential/SourceObjectDefaultToString.java`
checks both typed and erased top-level source objects by prefix, matching Java
without depending on identity hash stability. Unknown external erased display
remains open.

Updated 2026-08-25 Optional.of follow-up: `Optional.of(null)` and
`Optional.of(nullable ? null : value)` no longer lower as present Rust options
(`Some(None)` / `Some(if ... { None } ...)`). Slot-aware lowering now treats
the `of` argument as a required Java value, so `null` becomes a
`NullPointerException`; propagated `try/catch` scopes rewrite that null check to
a catchable `JavaException`. `tests/differential/OptionalOfNullCatch.java`
matches the Java oracle, and
`optional_of_null_in_catch_lowers_to_null_pointer_exception_result` pins the
emitted shape.

Updated 2026-08-25 raw parse follow-up: nullable-string values that still arrive
at integral parse lowering as rendered Rust now use the same Java null contract
as structured nullable arguments. A raw
`...as_ref().expect("Java null receiver").as_str()` parse input is converted to
the optional string display projection, so `None` becomes the literal parse
input `"null"` and reaches `NumberFormatException` instead of panicking on the
receiver unwrap. The focused `integer_parse_int_unwraps_sequence_get_argument`
fixture and `lower::tests::integral_parse_nullable_arg_maps_raw_optional_string_expect_null_to_number_format_input`
pin the field/list-get shape.

Updated 2026-08-29 structured parse follow-up: integral parse lowering now also
looks through structured `.as_str()` wrappers around nullable string carriers.
That keeps `List<String>.get(...)` and `Map<String, String>.get(...)` parse
arguments on Java's `Integer.parseInt(null)` path: the nullable input becomes
the display string `"null"` and parsing reports `NumberFormatException`, rather
than lowering through a synthetic null-receiver `NullPointerException`.
`integer_parse_int_nullable_sequence_element_is_number_format_exception`,
`integer_parse_int_unwraps_sequence_get_argument`, and
`optional_map_string_get_flows_into_string_value_contexts` pin the fixed shapes.

Updated 2026-08-25 enum `valueOf` follow-up: source enum `valueOf` now gets
the first slot-lowering opportunity before the generic static-call fallback,
and unresolved static-owner value placeholders no longer preempt one-argument
`valueOf` calls. `Map.get(...)` flowing into nested/context enum `valueOf`
therefore lowers through the borrowed-string slot as
`...get(...).expect("Java null receiver").as_str()` instead of an unresolved
static call or an `Option<&String>: AsRef<str>` type error.
`optional_map_string_get_flows_into_source_enum_value_of_inside_try_assignment`,
`optional_map_string_get_flows_into_top_level_source_enum_value_of`, and
`documents_enum_name_value_of_java_api_gap` pin the fixed paths.

Follow-up 2026-08-29 baseline pass: five more now-passing 0056 fixtures were
removed from `tests/baselines/idiom-gaps-failures.txt`:
`guarded_nullable_copy_constructor_reads_source_fields_after_unwrap`,
`integer_parse_int_unwraps_sequence_get_argument`,
`optional_map_string_get_flows_into_source_enum_value_of_inside_try_assignment`,
`optional_map_string_get_flows_into_top_level_source_enum_value_of`, and
`same_class_nullable_string_field_hash_code_method_uses_runtime_helper`. This is
baseline hygiene for already-fixed shapes, not closure of the remaining null
representation audit.

Follow-up 2026-08-29 baseline pass: three more nullable receiver/branch fixtures
were confirmed passing and removed from the idiom-gaps expected-failure
baseline: `map_get_optional_receiver_to_string_uses_java_null_receiver_policy`,
`null_guarded_ternary_append_argument_uses_refined_local_once`, and
`borrowed_nullable_trim_equals_original_uses_optional_string_view_policy`. This
keeps the baseline from hiding already-fixed catchable/display/nullability
paths; the broader null representation and unknown erased-display work remain
open.

Correction 2026-09-07: `borrowed_nullable_trim_equals_original_uses_optional_string_view_policy`
was still present in the checked-in failure baseline. It now passes after exact
optional borrowed arguments are left unchanged at source-call boundaries.

Updated 2026-08-25 while-assignment follow-up: the runtime form of
`while ((row = cursor.read()) == null) { ... } return row;` now compiles and
matches Java. The loop lowerer already refined `row` to non-null after the
loop, but caller-side local planning could still keep a source-call result slot
nullable after merging stale path-insensitive call-result constraints with a
definite non-null proof. Declared locals initialized by calls now discard that
stale nullable storage constraint when the collected constraints also prove
definite non-null, so the caller gets `Row` rather than `Option<Row>`.
`tests/differential/WhileAssignmentNullVariantsRuntime.java` covers both
`!= null` draining and `== null` first-present loops and matches Java's `5,7`.
The signature conflict diagnostic is still noisy for this shape; fixing that
cleanly requires making the diagnostic/nullability fact collector path-sensitive,
not changing the emitted behavior.

Updated 2026-08-25 resource-stream follow-up: direct returns of known nullable
external calls now contribute return-slot nullability instead of being forced
through the declared concrete type. In particular,
`ClassLiteral.class.getClassLoader().getResourceAsStream(path)` now lowers as
`Option<InputStream>` when returned directly. The old
`.expect("Java null receiver")` at the return boundary is gone for this path;
the nullable carrier is preserved until a later Java dereference. The sibling
`ClassLoader.getResource(path)` now uses the same direct optional lowering and
support facade shape, returning `Option<URL>` without an outer `Some(...)`
wrapper. Regression: `class_literal_get_class_loader_uses_platform_facade`.

Updated 2026-08-25 nullable-field receiver follow-up, revised 2026-08-28:
the previously rendered-only `MapEntrySetFirstKeyFromNullableField` shape is
now behaviourally covered in a catchable context, and the idiom shape itself no
longer leaves the map receiver as an uncaught Rust panic. The specialized
`entrySet().iterator().next().getKey()` lowering now runs its final accessor
projection through the existing catchable-null rewrite in propagating contexts,
so `values().entrySet()` and `holder.getValues().entrySet()` lower through
`ok_or_else(|| JavaException::new("NullPointerException", ...))?` rather than
`.expect("Java null receiver")`. Regressions:
`tests/idiom_gaps.rs::map_entry_set_iterator_next_get_key_uses_native_iterator_shape`
and
`tests/idiom_gaps.rs::map_entry_set_iterator_next_get_key_uses_receiver_signature_shape`;
the latter is promoted out of the idiom-gaps failure baseline.

Follow-up 2026-08-30 map-entry iterator optional receiver repair:
the entry-set iterator projection itself now uses the result-aware sequence
iteration helper, so nullable map receivers become catchable
`NullPointerException` projections before `iterator().next()` rather than panic
unwraps. This covers both read-only and mutating entry-set iterator lowering.
Verified:
`cargo test --release map_entry_set_iterator_next_get_key --test idiom_gaps -- --nocapture`.

Follow-up 2026-08-29: source-summary nullable method returns now keep their
optional local carrier when initialized from a summary-only receiver call.
The body lowering gives source-summary declared return nullability priority over
the concrete signature fallback, and return-slot selection no longer throws away
a solved optional return just because the declared Java return is a
source-summary type. Regressions:
`tests/idiom_gaps.rs::source_summary_nullable_return_into_optional_local_does_not_double_wrap`,
`tests/idiom_gaps.rs::source_summary_nullable_return_into_concrete_local_unwraps_once`,
`tests/idiom_gaps.rs::nullable_source_call_through_field_receiver_stays_optional_without_double_wrap`,
and
`lower::tests::source_summary_nullable_receiver_call_shape_is_optional`.

Follow-up 2026-08-30 support-shell precedence repair:
the nullable source-summary local tests above regressed again when source-summary
support shells and parsed summary declarations shared the same owner name. The
method facade still exposed `Option<Path>`, but local initializer planning read
the shell signature first and flattened `Path path = lookup.find(sample)` back
to `Path`. Receiver return-shape planning now asks the source-summary
declaration before merged support-context signatures and uses the receiver's
declared Java identity separately from its Rust storage shape. Verified:
`cargo test --release source_summary --test idiom_gaps -- --nocapture`.

Follow-up 2026-08-29 baseline pass: two more nullable tests were stale rather
than active failures. Reference-array default null elements now allocate through
`repeat_with(|| None)` rather than the old `vec![None; n]` spelling while
preserving the same nullable element semantics. Parsed source-call local
initializers with nullable return shapes now return the optional local directly
instead of unwrapping at the method boundary. Verified:
`tests/idiom_gaps.rs::reference_array_element_null_comparison_reads_nullable_element`
and
`tests/idiom_gaps.rs::solved_local_plan_inherits_nullable_source_call_initializer`;
the latter was removed from the idiom-gaps expected-failure baseline.

Correction 2026-09-07: the release `idiom_gaps` baseline confirms
`solved_local_plan_inherits_nullable_source_call_initializer` is no longer a
current failure.

Follow-up 2026-08-29 baseline pass: nullable source-interface forwarding was
also a stale assertion. The generated trait surface now widens `String getID()`
to `fn get_i_d(&self) -> Option<String>`, and the concrete impl forwards the
optional value without injecting an eager `expect("Java null receiver")` at the
trait boundary. Verified:
`tests/idiom_gaps.rs::source_interface_trait_impl_unwraps_nullable_implementation_return`.

Follow-up 2026-08-29 baseline pass: nullable source-enum switch fixtures were
also stale assertions after enum type rendering preserved the owner/member path
boundary with an underscore. The generated code still keeps nullable enum
storage and compares/matches through `Some(Enum::VALUE)`. Verified:
`tests/idiom_gaps.rs::nullable_source_enum_field_switch_keeps_concrete_enum_storage`
and
`tests/idiom_gaps.rs::nullable_source_enum_fallthrough_switch_labels_wrap_some`.

Method: differential testing against `javac`/`java` as oracle, per the recipe in
`tests/differential.rs`. Every claim below is a *verified* Java-output vs.
compiled-Rust-output comparison, not a reading of the emitted text. Repros are
in `repro/` beside this file, named as they would be in `tests/differential/`.

Follows [0052](0052-the-example-corpora-are-asserted-on-but-never-compiled.md) — the original 23-fixture differential corpus
contained **zero** uses of `null`, so none of this was reachable by any test.
Extends [0034](0034-object-array-allocation-produces-an-empty-vec.md),
whose fix traded a length bug for the value bug in §1.

---

## Summary of areas probed

| area | verdict |
|---|---|
| `null` local / `== null` / `!= null` on a local | correct (`tests/differential/NullBasics.java`) |
| `null` field with no initialiser | correct (`tests/differential/NullFieldDefault.java`) |
| `null` returned from a method into an `Option` local | correct (`tests/differential/NullObjectReturn.java`) |
| `null` in `String`/`Object` concatenation (`"a" + nullString`, `"c" + nullObject`) | correct |
| ternary yielding `null`, `instanceof` on `null` | correct |
| `Map.get` miss → `null` | correct (lowered to `Option`; `MapGetNull2.java`) |
| reference array element defaults + null test | correct (`NullStringArray.java`) |
| object in string concat (`"x" + obj`) | nullable erased object fixed; present erased source objects with declared `toString()` correct (`ObjectToStringConcat.java`) |
| `list.get(i) == null` after inserting `null` | correct (`NullInList.java`) |
| NPE on dereferencing null | correct (`tests/differential/NullDeref.java`) |
| passing `null` into a source method that dereferences the parameter under `try/catch` | correct (`NullArgCatch.java`) |
| nullable `String` *parameter* used in concat | correct (`NullArgLiteral.java`) |
| returning a literal from a borrow-planned method | correct (`NullDefaultReturn.java`) |
| nullable source call passed into constructor and stored in a field | correct (`NullCtorArg.java`) |
| `List<String>` containing `null` | correct (`NullInList.java`) |
| self-referential class field (`Node next`) | correct (`tests/differential/LinkedNode.java`; SharedLocal warning remains) |
| `"x" + null` (bare `null` literal) | correct (`NullLiteralConcat.java`) |
| catchable null rewrite through structured closure bodies | fixed by `lower::tests::make_java_null_expect_catchable_recurses_into_closure_bodies` |

**18 areas probed; several null concat/argument/return/constructor/list/array
cases are now promoted regressions, while unknown/external erased object display
remains divergent. SharedLocal object identity/mutation still belongs to 0002.**

---

## §1 — FIXED: reference arrays store nullable elements

`repro/NullStringArray.java`:

```java
public class NullStringArray {
    public static String run() {
        String[] a = new String[3];
        a[0] = "x";
        int nulls = 0;
        for (String s : a) { if (s == null) nulls++; }
        return (a[1] == null) + "," + a[0] + "," + a[1] + "," + nulls;
    }
}
```

| | output |
|---|---|
| Java (oracle) | `true,x,null,2` |
| old translation | `false,x,,0` |
| current translation | `true,x,null,2` |

This is now differential-gated by `tests/differential/NullStringArray.java`.
The generated `run()` uses a nullable element carrier:

```rust
let mut a: Vec<Option<String>> = vec![None; 3 as usize];
a[0 as usize] = Some("x".to_string());
for s in a.iter() {
    if s.is_none() {
        nulls = (nulls).wrapping_add(1);
    }
}
```

The historical generated `run()` was:

```rust
pub fn run() -> String {
    let mut a: Vec<String> = vec![String::new(); 3 as usize];   // <- Java fills with null
    a[0 as usize] = "x".to_string();
    let mut nulls: i32 = 0;
    for s in a.iter() {
        if false {                                              // <- the null test is GONE
            { let __java_old_value = nulls; nulls = (nulls).wrapping_add(1); __java_old_value };
        }
    }
    return format!("{},{},{},{}", (false).to_string(), &a[0 as usize], &a[1 as usize], (nulls).to_string());
}
```

Two independent defects composed:

* `src/array_lowering.rs:318 default_array_element_expr` mapped `JavaType::String`
  to `String::new()`, i.e. `""`. JLS 4.12.5 says a reference array is
  zero-initialised to `null`. `object_element_default_expr` (`:349`) does the
  same with `Default::default()`. Its doc comment already concedes one
  divergence ("reading an element that was never assigned yields a zero value
  where Java throws `NullPointerException`") — but the *observable* consequence
  is worse than an NPE, because of the second defect.
* `src/lower.rs:lower_null_comparison_expr_for_shape` was the load-bearing old
  defect. It still folds only primitive/fresh-construction shapes and emits a
  structured `compile_error!` for reference-typed operands with no nullable
  representation, but reference-array elements now have that nullable
  representation.

This pattern — allocate a reference array, fill it lazily, use `null` as
"not yet filled" — is one of the most common shapes in the kind of Java this
project targets (sparse tables, memo caches, parser slot arrays).

**Direction.** This subcase is no longer only mitigated: reference-array
elements are represented as `Option<T>`, so Java default initialisation and
later `null` tests agree with the oracle.

---

## §2 — FIXED FOR SOURCE TYPES: present erased source objects can dispatch `toString`

`repro/ObjectToStringConcat.java`:

```java
static class Point {
    int x;
    Point(int x) { this.x = x; }
    public String toString() { return "P" + x; }
}
public static String run() {
    Object o = new Point(3);
    Point p = new Point(4);
    return "a" + o + "," + "b" + p + "," + p.toString() + "," + o.toString();
}
```

| | output |
|---|---|
| Java (oracle) | `aP3,bP4,P4,P3` |
| old translation | `a,b,P4,` |
| current translation | `aP3,bP4,P4,P3` |

This no longer silently renders `""`, and the source-type erased case no longer
only fails loudly. The erased-object runtime support emits downcast arms for
translated non-generic source types that declare `toString()`, and
`JavaErasedObjectFacade for Box<dyn Any>` delegates explicit `o.toString()`
through the same helper. Note the third field was always *right* — an explicit
`p.toString()` on a statically-known type works — while the first and fourth
fields now recover through the erased carrier.

```rust
let o: Box<dyn std::any::Any> = Box::new(ObjectToStringConcatPoint::new(3)) as Box<dyn std::any::Any>;
let p: ObjectToStringConcatPoint = ObjectToStringConcatPoint::new(4);
return format!("a{},b{},{},{}",
    java_erased_object_to_string(&o),   // -> "P3" through the generated Point downcast
    p.to_string(),                      // -> "P4" through the concrete source method
    p.to_string(),                      // -> "P4" correct
    o.to_string());                     // -> "P3" through the erased-object facade
```

Historical root cause chain:

1. `src/conversion_plan.rs:2564 syntax_slot_coercion_policy_from_content` —
   for `Slot::StringConcatSegment` it consults
   `java_type_needs_java_object_display_fallback` (`:3724`), which returns
   `true` for **every** `JavaType::Named` that is not on a nine-name allowlist
   (`String`, `StringBuilder`, `StringBuffer`, `CharSequence`, `Appendable`,
   `Class`, `Path`, `File`, `URI`) and not a platform facade. A user class with
   a declared `toString()` in the same compilation unit is not on that list.
2. That selects `Coercion::ErasedObjectToJavaDisplayString`
   (`src/coercion.rs:10621`) → `JavaDisplayPlan::ErasedObject`
   (`src/coercion.rs:7903`) → a call to `java_erased_object_to_string`.
3. `src/stdlib.rs:6639` used to define that helper as

   ```rust
   pub fn java_erased_object_to_string<T: ?Sized>(_value: &T) -> String {
       Default::default()
   }
   ```

   — ignoring its argument and returning `""`. It now panics loudly instead,
   and erased-facade `toString` fallbacks did the same. Source-type erased
   values now recover through a generated downcast table; unknown/external
   erased values remain loud.

The same helper used to make a null `Object` print `""` instead of `"null"`
(`repro/NullConcat.java`: Java `anull,bnull,cnull,true`, old translation
`anull,bnull,c,true`). That null-only erased-object display case is now fixed by
the optional-erased display coercion and covered by
`tests/differential/NullConcat.java`. Present erased source values still render
through the erased helper, but that helper can now downcast to translated
source types with declared `toString()`.

**Direction.** Statically-known source classes no longer take the erased path
just because their Java type is named, and source classes erased behind
`Object` can recover declared `toString()` through the runtime downcast table.
The remaining work is a display-capability model for genuinely unknown or
external erased values. Placeholder external facade `Display` impls are now
loud rather than empty, so the remaining gap is semantic modeling rather than
silent blank output.

Plain `new Object().toString()` no longer sits in that unknown bucket:
`tests/differential/ObjectToStringFallback.java` is promoted and checks Java's
stable `startsWith("java.lang.Object@")` predicate. Genuinely unknown/external
erased values still need the display-capability model above.

---

## §3 — FIXED: `list.get(i) == null` asks the element-null question

`repro/NullInList.java` / `tests/differential/NullInList.java`:

```java
List<String> xs = new ArrayList<>();
xs.add("a");
xs.add(null);
for (String s : xs) { sb.append(s == null ? "N" : s); }
return sb + "," + xs.size() + "," + (xs.get(1) == null);
```

Java: `aN,2,true`. Old emitted Rust was wrong on three counts:

```rust
let mut xs: Vec<String> = Vec::new();          // element type is not Option
xs.push("a".to_string());
xs.push(None);                                 // E0308  <- the only loud failure
for s in xs.iter() {
    sb.append(if false { "N".to_string() } else { s.clone() });   // §1's fold again
}
return format!("{},{},{}", sb.to_string(), (xs.len() as i32).to_string(),
    (xs.get(1).is_none()).to_string());         // "is index 1 out of range?", not "is it null?"
```

Fixed 2026-08-24. Native collection locals that receive a literal `null`
element now plan nullable element storage. The same repro now lowers as:

```rust
let mut xs: Vec<Option<String>> = Vec::new();
xs.push(Some("a".to_string()));
xs.push(None);
for s in xs.iter() {
    sb.append(if s.is_none() { "N".to_string() } else { (s.as_ref().unwrap()).to_string() });
}
return format!("{},{},{}", sb.to_string(), (xs.len() as i32).to_string(),
    (xs.get(1 as usize).expect("Java index out of bounds").is_none()).to_string());
```

The final line intentionally distinguishes Rust's outer bounds `Option` from
Java's nullable element carrier: an absent index still raises Java index
failure, while a present `None` answers `xs.get(1) == null`.

---

## §4 — FIXED FOR FALLIBLE OPTIONAL RECEIVERS: panics where Java has a catchable exception

Dereferencing null used to abort the process instead of raising catchable
`NullPointerException`.

`repro/NullDeref.java` — Java: `npe`; old translation: `panicked at 'called
Option::unwrap() on a None value'`, exit status 101.

```rust
let s: Option<String> = None;
match (|| -> Result<String, JavaException> {
        let n: i32 = s.as_ref().unwrap().as_str().len() as i32;   // <- panics
        return Ok(format!("len:{}", (n).to_string()));
    })() {
    Ok(value) => { return value; },
    Err(e) if e.is_instance(&["NullPointerException"]) => { return "npe".to_string(); },
    …
```

Fixed 2026-08-24 for optional receiver adaptation in fallible contexts. The
translator already generated the correct catch arm for `NullPointerException`;
the receiver projection now produces `Err(JavaException::new(
"NullPointerException", ...))?` so that catch arm can fire. A related
non-receiver constructor-argument shape was fixed separately in §5c; remaining
non-receiver nullability gaps are collection/array element storage and unknown
external boundaries.

See `0057 §1` for the same problem in `ArrayIndexOutOfBoundsException`,
`ArithmeticException` (already correct) and `NumberFormatException`.

---

## §5 — historical compile failures, mostly promoted

This section records the original compile-failure probes. Most are now fixed and
covered by differential fixtures; the active null work is the narrower direction
list below, especially non-argument required-reference coercions and genuinely
unknown erased display.

Nine shapes, all reduced to a single method. Each was verified with
`rustc --edition 2021`; two have since been promoted and now pass the
differential oracle.

### 5a. nullable `String` parameter used in a concatenation — `repro/NullArgLiteral.java`

```java
static String f(String s) {
    if (s == null) { return "N"; }
    return "v" + s;
}
public static String run() { return f("a") + "," + f(null); }
```

```rust
pub fn f(s: Option<&str>) -> String {
    if s.is_none() { return "N".to_string(); }
    return format!("v{}", s.as_deref().unwrap_or("null"));
}
```

This is now fixed and covered by `tests/differential/NullArgLiteral.java`.
The parameter is planned as `Option<&str>`, so the optional string display plan
compiles and prints `"null"` for `None`.

### 5b. returning a literal from a borrow-planned method — `repro/NullDefaultReturn.java`

```java
static String describe(String s) {
    if (s == null) return "none";
    return s;
}
```

```rust
pub fn describe<'a>(s: Option<&'a str>) -> Option<&'a str> {
    if s.is_none() {
        return Some("none");
    }
    return s;
}
```

This is now fixed and covered by `tests/differential/NullDefaultReturn.java`.
The returned literal lowers as a static string view inside the optional return,
so the borrow-planned method compiles and prints `x,none`.

### 5c. FIXED: nullable source-call constructor arguments — `tests/differential/NullCtorArg.java`

```java
static class Holder { String name; Holder(String n) { this.name = n; } }
static String maybe(boolean b) { return b ? "x" : null; }
Holder h = new Holder(maybe(false));
```

```rust
pub struct NullCtorArg_Holder {
    name: Option<String>,
}

pub fn new(n: Option<String>) -> NullCtorArg_Holder {
    return NullCtorArg_Holder { name: n };
}

let h: NullCtorArg_Holder = NullCtorArg_Holder::new(Self::maybe(false));
return format!("{},{},{},{}",
    (h.name.is_none()).to_string(),
    h.name.as_deref().unwrap_or("null"),
    (g.name.is_none()).to_string(),
    g.name.as_deref().unwrap_or("null"));
```

Java: `true,null,false,x`. Translation now matches. The fix has two parts:
source-call arguments with nullable return constraints are propagated to the
resolved constructor parameter, and a nullable constructor binding stored
directly into a field propagates `MaybeNull` to that field's slot. This closes
the `maybe(false)` constructor case without changing the remaining nullable
array/collection element work.

Historical note: `repro/NullObjectReturn.java` was the object-typed version and
showed a second symptom — the `Option` unwrap was applied twice:

```rust
a.as_ref().expect("Java null receiver").expect("Java null receiver").v   // E0599
```

Fixed 2026-08-24. The redundant second null expect is stripped through field
and index projections, and `tests/differential/NullObjectReturn.java` now gates
the nullable source-object return case against the Java oracle.

### 5d. self-referential field — `repro/LinkedNode.java`

The three-line linked list:

```java
static class Node { int value; Node next; Node(int v) { this.value = v; } }
Node cur = a; while (cur != null) { total += cur.value; cur = cur.next; }
```

is now covered by `tests/differential/LinkedNode.java`. The old compile errors
were `a.next`/`cur.value` against nullable locals and `Some(b.clone())` against
a field typed `Option<Box<Node>>`. Those are fixed. The translator still warns
`storage_strategy_disagreement: LinkedNode.Node needs SharedLocal … but is
lowered by value, so an alias becomes a copy` (0002's territory), so the
representation warning remains even though this non-mutating alias fixture
matches Java.

### 5e. `"x" + null` — `repro/NullLiteralConcat.java`

A bare `null` literal in a concatenation:

```rust
format!("x{}", "null".to_string())
```

This is now fixed and covered by `tests/differential/NullLiteralConcat.java`.

### 5f. representation instability — `repro/MapGetNull2.java`

Two programs differing by one statement:

```java
// compiles, correct
String miss = m.get("b");
String out = "";
if (miss == null) { out = "miss-null"; } else { out = "miss=" + miss; }
return out;
```

```java
// now compiles and matches Java
String hit  = m.get("a");     // <- added
String miss = m.get("b");
String out = "";
if (miss == null) { out = "miss-null"; } else { out = "miss=" + miss; }
return hit + "," + out;       // <- changed
```

Previously, the *unrelated* local `out` was demoted:

```rust
let mut out: Box<dyn std::any::Any> = Box::new("".to_string()) as Box<dyn std::any::Any>;
…
E0277: `dyn Any` doesn't implement `std::fmt::Display`
```

This is now fixed and covered by `tests/differential/MapGetNull2.java`.
Follow-up 2026-09-07: `MapGetNull2` and the adjacent present-null map-value
fixture `MapPutNullValue` also have a named focused differential loop,
`differential::nullable_map_get_fixtures_match_java_output`, so this null
contract can be rechecked without the full differential sweep.
Adding a second `Option<String>` local no longer flips a third local's
representation to erased.

### 5g. `Map.put` previous value through a sorted-map field

`FileLoadConstructorDelegation` now compiles: a `SortedMap.put` result assigned
to a declared `URI` local is kept as `Option<URI>`, so `previous != null` lowers
to `previous.is_some()`. The fix also keeps `String.split(...)[0]` as a borrowed
optional element before cloning it into the owned map key, instead of moving out
of a shared `Vec<Option<String>>` element. Covered by
`sorted_map_field_put_previous_value_stays_nullable`,
`split_part_map_key_borrows_optional_index_before_clone_unwrap`, and the corpus
compile baseline.

### 5h. `this.field = parameter` does not prove non-null

Compile-backing `same_class_nullable_string_field_hash_code_method_uses_runtime_helper`
exposed another nullable-field representation edge. A field assigned through
`this.bases = bases` from a constructor parameter was marked definitely
non-null, while the equivalent bare `bases = bases` path already treated the
shadowing parameter as possibly null. A later `bases == null` therefore emitted
`compile_error!("unsupported Java null comparison...")` against a non-optional
field. Qualified field writes and trivial setters now use the same
parameter-aware assignment rule as bare field writes, so such fields stay
`Option<String>` and null comparisons lower normally. Covered by
`semantic_facts::qualified_constructor_assignment_from_parameter_keeps_field_nullable`
and the compile-backed idiom test above.

---

## Why nothing noticed

This section was true when the audit was filed, but is now historical. The
differential suite is at 277 fixtures in the current worktree, and promoted
null fixtures cover null concat, bare null concat, nullable string arguments,
null default returns,
`Map.get` misses, nullable constructor arguments stored in fields, nullable
`List` elements, nullable reference-array elements, source `Object.toString()`
recovery, catchable nullable source-parameter receiver dereference, and
catchable nullable source-call values coerced into required argument slots, plus
the `Integer.parseInt(null)` `NumberFormatException` contract. The former silent
`x == null` fold for unrepresented reference operands is now a
`compile_error!`, and erased object display is loud rather than blank for
unknown targets.

Follow-up 2026-09-09 focused null-loop coverage pass: the cheap
`null_representation_fixtures_match_java_output` oracle now includes the
already-promoted null fixtures that were previously covered only by the broad
differential run: `NullArgLiteral`, `NullDefaultReturn`,
`NullFieldCtorOverload`, `NullInList`, `NullInstanceof`,
`NullableArrayLengthCatch`, and `NullableFieldReceiverCatch`.

Follow-up 2026-09-09 erased-display fallback guard: the generated
`java_erased_any_to_string` support item now has a direct unit guard for the
remaining unknown-runtime-target path. It must panic with
`unsupported Java erased object display: runtime target is erased` and must not
regress to the historical empty/default string fallback. This does not solve
unknown external dynamic display; it pins the current loud behavior.

---

## Direction

In priority order:

1. **Finish catchable null coercions beyond optional receivers.** Nullable
   optional receivers in fallible contexts now raise `NullPointerException`,
   and source-method nullable parameter receiver dereferences can propagate
   across call boundaries. Required call/constructor argument slots now use the
   same catchable carrier when they unwrap nullable values in a propagated
   block. Other non-argument `.expect("Java null receiver")` coercions still
   need the same treatment when they are semantically Java NPEs.
   Follow-up 2026-08-26: recoverable raw statements inside a block now flow
   through the same structural rewrite before recursion, so a raw expression
   statement such as `value.expect("Java null receiver");` no longer remains a
   panic when the surrounding block is made catchable. Covered by
   `lower::tests::make_java_null_expect_catchable_recovers_raw_block_statements`.
   The earlier `getResourceAsStream(...)` example is no longer active:
   `class_literal_get_class_loader_uses_platform_facade` and
   `optional_class_loader_resource_stream_assignment_keeps_optional_return_shape`
   both compile-check that direct return and local assignment preserve
   `Option<InputStream>`/`Option<URL>` instead of unwrapping at the slot
   boundary. The remaining work is to find and replace other non-argument
   required-reference coercions that still panic before an actual Java
   dereference.
   Follow-up 2026-08-26: the concrete-slot `getResourceAsStream(...)` unwrap
   now runs through the same catchable-null rewrite when the surrounding
   lowering context propagates Java exceptions. In non-propagating contexts it
   still emits the previous `.expect("Java null receiver")`, but inside
   catchable lowering it becomes `ok_or_else(...NullPointerException...)?`
   instead of a Rust panic. Regression:
   `lower::tests::nullable_resource_stream_unwrap_is_catchable_in_propagating_context`.
   Follow-up 2026-08-26: interface trait return-shape planning now widens a
   required interface return to `Option<T>` when the selected implementation
   method has an optional return shape for the same Java type. The forwarding
   impl therefore preserves a nullable `String id()` implementation as
   `fn id(&self) -> Option<String>` instead of injecting
   `NullName::id(self).expect("Java null receiver")` inside the trait boundary.
   This keeps later dereference sites available to the existing catchable null
   machinery instead of panicking during trait forwarding. Regressions:
   `representation_lowering::trait_return_shape_tests::trait_return_shape_widens_to_optional_implementation_return`,
   `representation_lowering::trait_return_shape_tests::trait_return_shape_does_not_widen_to_unrelated_optional_return`,
   and
   `slot_shape_design::trait_impl_forwarding_keeps_nullable_implementation_return_shape`.
   Follow-up 2026-08-28: required receivers produced by nullable sequence
   storage now keep the Java null boundary after any collection bounds check.
   `List<String> values; values.add(null); values.get(0).length()` used to
   compile as `Option<String>::len()` and return `0` instead of raising a
   catchable `NullPointerException`. Nullable array `.length` in a propagating
   `try/catch` also now rewrites the receiver unwrap through `ok_or_else(...)?`
   instead of leaving `.expect("Java null receiver")`. Covered by
   `lower::tests::nullable_list_get_receiver_unwrap_is_catchable_after_bounds_check`,
   `lower::tests::nullable_array_length_unwrap_is_catchable_in_propagating_context`,
   `tests/differential/ListElementReceiverNullCatch.java` (`npe`), and
   `tests/differential/NullableArrayLengthCatch.java` (`npe`).
   Follow-up 2026-08-28: primitive arithmetic operands produced by `Map.get`
   now use the same catchable null-unboxing rewrite in propagating contexts.
   An empty map `totals.get("x") / 2` inside
   `try/catch (NullPointerException)` now raises
   `JavaException::new("NullPointerException", ...)` instead of panicking
   through `.copied().expect("Java null receiver")`. Covered by
   `tests/differential/MapGetArithmeticNullCatch.java` (`npe`).
2. **Keep nullable constructor/method parameter coverage explicit.**
   Constructor and source-method `null` arguments are now fixture-backed by
   `NullCtorArg`, `NullArgCatch`, and `NullCallArgCatch`; the remaining
   parameter-side risk is external/unknown APIs with no nullable slot evidence.
3. **Finish unknown/external erased object display capabilities.**
   Source `Object o = new Point(3); "a" + o` is covered. Opaque external
   facades now delegate both direct `Display` and erased-object runtime display
   to a modeled zero-argument `toString(): String` API fact, and facades without
   that fact still fail loudly. Built-in `java.io.File` is also covered through
   its existing facade `Display` implementation. The remaining gap is
   erased/unknown runtime values where no source downcast arm or explicit
   external API fact identifies a Java `toString()` surface.
4. Promote each remaining repro as it becomes loud-or-correct. Do not add a
   passing assertion that only checks rendered Rust text.

Follow-up 2026-08-28: `String.join` now keeps explicit null elements
represented through the join value path instead of leaving the collection or
varargs vector as mixed `String`/`None` Rust. A null-bearing literal collection
or varargs vector is normalized to `Option<String>` elements, mapped through
Java's display policy (`null` renders as `"null"`), then joined. Promoted
differential fixtures: `StringJoinNullElement` and
`StringJoinVarargsNullElement`, both checking Java and translated output produce
`a,null,b`.

Follow-up 2026-08-28: `Object value = System.in; "x" + value` now uses the
platform erased-display path instead of panicking at runtime. The built-in
`InputStream` shim has a Java-object-style `Display` fallback, and the erased
object runtime emits an `InputStream` downcast when `System`/`InputStream` is in
the source surface. Promoted differential fixture: `ErasedInputStreamDisplay`,
which checks the translated output matches Java's stable `xjava.io.` prefix.
This is still a platform-carrier mitigation; arbitrary unknown erased display
without source/platform/fact evidence remains open.

Follow-up 2026-08-28/2026-09-07: `Object value = new Random(1); "x" + value`
and direct `value.toString()` now follow the same platform erased-display
route. `Random` is included in the generated platform downcast table when it
appears in the source surface, and the generated external facade `toString`
stub now delegates to `Display` instead of emitting an unmodelled panic that
shadowed Rust's blanket `ToString` implementation. Promoted differential
fixture: `ErasedRandomDisplay`, which compares Java and translated output
through the stable `java.util.Random@` prefix for both concat and direct calls.
This remains a platform-carrier mitigation; arbitrary unknown erased display
without source/platform/fact evidence remains open.

Codex follow-up 2026-09-15: `Object value = new StringTokenizer("a b"); "x" +
value` and direct `value.toString()` now follow the platform erased-display
route too. The modeled `StringTokenizer` runtime shim implements Java
object-style `Display`, and `StringTokenizer` is included in the generated
platform downcast table when it appears in the source surface. Promoted
differential fixture: `ErasedStringTokenizerDisplay`, which compares Java and
translated output through the stable `java.util.StringTokenizer@` prefix. This
remains a platform-carrier mitigation; arbitrary unknown erased display without
source/platform/fact evidence remains open.

Codex follow-up 2026-09-15 later: `Object value = new ZipEntry("folder/file.txt");
"x" + value` and direct `value.toString()` now use the modeled `ZipEntry`
name value instead of falling into the unsupported erased-object display path.
The `ZipEntry` shim implements `Display`, and `ZipEntry` is included in the
generated platform downcast table when it appears in the source surface.
Promoted differential fixture: `ErasedZipEntryDisplay`.

Codex follow-up 2026-09-15 later still: `java.util.jar.JarEntry` now keeps the
same modeled name/directory state as `ZipEntry` for its string constructor,
`getName()`, `isDirectory()`, string concatenation after erasure to `Object`,
and direct erased `Object.toString()`. Promoted differential fixture:
`ErasedJarEntryDisplay`.

Follow-up 2026-08-28: `Object value = new URL("http://example.com/a"); "x" +
value` now uses the platform erased-display route. Both generated `URL` support
shims implement `Display` as the URL string value, and `URL` is included in the
generated platform downcast table when it appears in the source surface.
Promoted differential fixture: `ErasedUrlDisplay`, which checks Java and
translated output both produce `xhttp://example.com/a`. This remains a
platform-carrier mitigation; arbitrary unknown erased display without
source/platform/fact evidence remains open.

Follow-up 2026-08-28: `String.valueOf(Object)` now preserves Java's nullable
erased-object display when the argument is a solved nullable source parameter.
The string-slot `valueOf` path and the display-call path share the same
optional-erased predicate, so `Object value = null; String.valueOf(value)`
lowers through `as_ref().map(...).unwrap_or_else(|| "null".to_string())`
instead of calling `java_erased_object_to_string(&value)` on the `Option`
carrier. Promoted differential fixture: `StringValueOfNullObject`, which
matches Java's `null` oracle. The differential suite is now at 218 fixtures.

Follow-up 2026-08-28: the fully-qualified optional
`java.io.ByteArrayOutputStream` path is no longer baseline-listed. The generated
local remains typed as `Option<JavaIoByteArrayOutputStream>` and no longer
falls back to `Option<Box<dyn Any>>` or a downcast; nullable receiver uses of
`close()` and `toByteArray()` now go through the catchable
`NullPointerException` projection instead of `unwrap()`. Regression:
`tests/idiom_gaps.rs::fully_qualified_optional_byte_array_output_stream_stays_typed`.

Follow-up 2026-08-28: the catchable-null rewrite now recovers parseable raw
expressions before walking them. A compatibility raw expression such as
`helper(value.expect("Java null receiver"))` is recovered to structured IR and
rewritten through `ok_or_else(|| JavaException::new("NullPointerException",
...))?` instead of keeping the uncaught `.expect(...)` inside the call.
Regression:
`lower::tests::make_java_null_expect_catchable_recovers_parseable_raw_expression`.

Follow-up 2026-08-28: `String.valueOf(String)` now preserves Java's `"null"`
spelling for nullable string carriers. Planned/native `String.valueOf` calls
route through a shared valueOf display helper, and the final call lowering
recovers terminal `expect("Java null receiver")` chains before applying the
optional-string display projection. Regression:
`tests/idiom_gaps.rs::string_value_of_nullable_string_preserves_java_null_spelling`.

Follow-up 2026-08-29: stream source calls whose return shape is nullable now
carry that optional sequence evidence into stream rendering. A source such as
`maybe().stream()` where `maybe()` returns `null` no longer lowers as
`Self::maybe().into_iter()` over the `Option` carrier. Regression:
`tests/idiom_gaps.rs::nullable_stream_source_call_in_try_lowers_to_catchable_npe`.

Follow-up 2026-08-29: optional borrowed and mutable-borrowed sequence sources
now use the same catchable null receiver policy as owned optional sequence
sources when Java exception propagation is active. The propagated forms lower
through `as_ref().ok_or_else(...)? .iter()` and
`as_mut().ok_or_else(...)? .iter_mut()` instead of panicking with
`expect("Java null receiver")` inside a `try` translation. Regression:
`coercion::tests::jdk_collection_static_projections_own_expression_shapes`.

Follow-up 2026-08-29: erased `Object` display now covers the existing
`AtomicInteger` platform facade. `AtomicInteger` implements `Display` by
reading its current cell value, and the erased-object display table emits
`AtomicInteger`/`Rc<AtomicInteger>`/`Arc<AtomicInteger>` downcast arms when the
platform type appears in the source surface. Promoted differential fixture:
`ErasedAtomicIntegerDisplay`, which matches Java after mutating through
`incrementAndGet` (`x4`). This is still a platform-carrier mitigation;
arbitrary unknown erased display without source/platform/fact evidence remains
open.

Follow-up 2026-08-29: erased `Object` display now has parity for
`AtomicLong`. The platform facade mirrors `AtomicInteger` with an `i64` cell,
state-changing `incrementAndGet`/`getAndIncrement`, and `Display` over the
current value. The platform erased-display table emits `AtomicLong`,
`Rc<AtomicLong>`, and `Arc<AtomicLong>` downcast arms when the type appears in
the source surface. Promoted differential fixture: `ErasedAtomicLongDisplay`
(`x4`). This remains a platform-carrier mitigation; arbitrary unknown erased
display without source/platform/fact evidence remains open.

Follow-up 2026-08-29: erased `Object` display also has parity for
`AtomicBoolean`. The platform facade uses a `bool` cell and covers `get`,
`set`, `getAndSet`, `compareAndSet`, and `Display` over the current Java
boolean spelling. The platform erased-display table emits `AtomicBoolean`,
`Rc<AtomicBoolean>`, and `Arc<AtomicBoolean>` downcast arms when the type
appears in the source surface. Promoted differential fixture:
`ErasedAtomicBooleanDisplay` (`xfalse`). This remains a platform-carrier
mitigation; arbitrary unknown erased display without source/platform/fact
evidence remains open.

Follow-up 2026-08-29: `java.util.Objects.toString` now has native call
planning/lowering instead of falling through to unmodelled facade stubs.
The one-argument overload reuses `String.valueOf` display semantics. The
two-argument overload lowers in a shaped path so the default expression is
evaluated and nullable values choose the default instead of Java's `"null"`
spelling. Emitted `Objects.toString` facade support methods are arity-suffixed
when both overloads are present, avoiding duplicate Rust method definitions in
fixtures that import `java.util.Objects`. Promoted differential fixture:
`ObjectsToStringNullDefault`, which matches Java (`x,1,d2,2,null`). The audit
remains open for the broader null representation and unknown erased-display
cases above.

Follow-up 2026-08-29: collection `toArray` lowering now preserves nullable
reference-array carriers when the collection expression is recovered through a
map view or map lookup. `keySet().toArray(new String[0])` carries the backing
map key type into the native collection plan and wraps elements as `Some`.
`map.get(key).toArray(new File[0])` compares the map value's storage element
against the target array element, then wraps the recovered owned sequence into
`Vec<Option<_>>` when the Java array slot stores nullable references. The
map-lookup special case also lowers its key against the declared map-key slot,
so indexing a nullable `String[]` key unwraps before the map borrow instead of
moving an `Option<String>` out of a shared array read. Regression:
`tests/idiom_gaps.rs::ragged_object_two_dimensional_arrays_stay_nested_vecs`;
companion assertion:
`tests/idiom_gaps.rs::collection_to_array_empty_string_array_lowers_to_owned_vec`.

Follow-up 2026-08-29 baseline pass: six map/null collection fixtures were
confirmed passing and removed from the idiom-gaps expected-failure baseline:
`map_get_in_ternary_primitive_slot_unwraps_and_copies_value`,
`map_get_into_optional_borrowed_arg_uses_optional_carrier_plan`,
`map_get_primitive_value_in_arithmetic_unboxes_optional_borrow`,
`map_get_return_uses_nullable_signature_and_optional_carrier`,
`map_get_string_display_segment_unwraps_optional_borrow`, and
`map_key_set_int_loop_casts_len_for_java_int_comparison`. This is baseline
hygiene for already-landed nullable map handling; the broader null
representation and unknown erased-display work remain open.

Follow-up 2026-08-29 baseline pass: three nullable display/refinement fixtures
were also verified passing and removed from the idiom-gaps expected-failure
baseline: `nullable_string_format_and_concat_use_java_null_display_policy`,
`nullable_ternary_branches_consume_branch_refinements_and_slot_shape`, and
`nullable_trim_equals_original_uses_optional_string_view_policy`. The borrowed
trim companion also passes but was already absent from the baseline.

Follow-up 2026-08-29 baseline pass: two more nullable fixtures were verified
and removed from the idiom-gaps expected-failure baseline. The boolean ternary
fixture now emits the intended optional string arms; only the assertion was
stale after structured condition rendering added parentheses around the erased
boolean downcast. The outer graphics fixture now keeps its concrete optional
facade receiver shape. Verified:
`nullable_boolean_ternary_assignment_preserves_optional_string_slot` and
`nullable_outer_graphics_field_call_uses_concrete_optional_facade_receiver`.

Follow-up 2026-08-29 executor callback pass: `ExecutorService.submit` is now a
discoverable platform effect and generated external facade support emits a
concrete `ExecutorService::submit` shim when the erased-object method table has
the matching `submit/1 -> Future` fact. The shim returns `Future<T>` so a
`Future<Reader>` destination slot can infer the callback payload type instead
of falling back to `Future<Box<dyn Any>>`. `JavaLinkedHashMap` now implements
consuming `IntoIterator`, allowing already-emitted map conversion expressions
to compile. Verified and removed from the baseline:
`nullable_method_result_in_callback_local_stays_optional_for_optional_arg_slot`
and `executor_submit_future_slot_shapes_block_lambda_result`.

Follow-up 2026-08-29 baseline pass: four now-passing facade/map fixtures were
removed from the idiom-gaps expected-failure baseline:
`selected_opaque_future_get_is_generic_instance_surface`,
`selected_opaque_support_type_gets_used_known_method_surfaces_by_arity`,
`selected_opaque_support_type_gets_used_typed_void_surface`, and
`typed_map_get_redundant_cast_uses_generic_value_shape`.

Follow-up 2026-08-29 baseline pass: the inherited source-summary opaque method
surface fixture was also verified passing and removed from the idiom-gaps
expected-failure baseline:
`source_summary_opaque_type_gets_used_inherited_method_surface`.

Follow-up 2026-08-29 baseline pass: two more checked fixtures were removed
after narrow assertion updates rather than lowering changes.
`external_pack_typed_string_list_getter_supports_indexed_sample_access` now
unwraps its generated `Result<String, JavaException>` test call, matching the
fallible translated method signature. `source_static_final_facade_fields_emit_associated_defaults_not_storage`
now accepts the generated owner-qualified static access
`UsesStaticFinalPattern::LINE_PATTERN`, which is equivalent inside the static
method and compiles. The nearby
`source_summary_method_surface_can_be_added_after_external_return_selects_type`
row was left in place because it was a real failure: an external fact returning
`Header` selected a source-summary type without seeding its summary method
surfaces, then the nullable `Header.samHeader()` result was sent through
source-subtype boxing as `Optional<SAMFileHeader> -> SAMFileHeader`. External
fact return types now seed source-summary facades before the second saturation
pass, erased-return lowering only runs for slots that are actually erased, and
source-subtype conversion refuses optional carriers so the normal optional
unwrap plan handles required return slots. The fixture now cargo-checks its
generated crate and was removed from the idiom-gaps expected-failure baseline.

Follow-up 2026-08-29 baseline pass: five additional nullable/fallible-shape
fixtures now pass and were removed from the idiom-gaps expected-failure
baseline. `BufferedReader.readLine()` assignment/initializer/external-effect
cases preserve optional read-line state across null checks and fallible reader
effects; nullable `Path` overload dispatch keeps the optional argument slot
while resolving the non-optional sibling; and optional array receiver indexing
and length reads unwrap through the current nullable array path. Verified:
`buffered_reader_read_line_assignment_condition_with_and_does_not_move_optional`,
`buffered_reader_read_line_initializer_is_optional_when_rechecked_for_null`,
`buffered_reader_read_line_uses_reader_external_effect`,
`nullable_path_overload_argument_uses_option_param_and_non_optional_resolve_sibling`,
and `optional_array_receivers_unwrap_for_read_indexing_and_length`.

Correction 2026-09-07: `optional_array_receivers_unwrap_for_read_indexing_and_length`
was also still present in the checked-in failure baseline. It now passes after
optional ndarray row-count lowering unwraps the `Option` receiver without adding
the slice projection used for `Vec` lengths.

Follow-up 2026-08-29 baseline pass: two more stale nullable/string-shape rows
were removed after focused verification. `bare_final_instance_field_read_does_not_become_static_constant`
now compiles with the instance nullable field read preserved, and
`constructed_string_return_stays_owned_not_borrowed_alias` now returns the
constructed owned `String` directly without borrowing through `&str`. The latter
needed only a narrower assertion because unrelated generated support can contain
other borrowed-string spellings.

Follow-up 2026-08-29 baseline pass: four more focused idiom rows were verified
passing and removed from the expected-failure baseline. Context translation now
uses the sibling overload signature for call-target slots, constructed external
arguments borrow correctly for borrowed source slots, executor-submitted lambdas
can call receiverless helpers through the current receiver, and explicit
primitive casts now apply to the whole shift/bitwise expression while using the
current Java-integer-safe `wrapping_shr`/`wrapping_sub` spelling. The primitive
fixture only needed assertion text updated to the current lowering form.

Follow-up 2026-08-30 erased object-array display pass:
`java_erased_any_to_string` now recognizes erased object-array carriers used by
`Object...` varargs and nullable erased object arrays. Those values render with
the translator's deterministic Java array identity string,
`[Ljava.lang.Object;@0`, instead of falling through to the unsupported erased
runtime-target panic. The varargs exception-message fixture now runs generated
tests that call `java_erased_object_to_string` on both
`Vec<Box<dyn Any>>` and `Vec<Option<Box<dyn Any>>>`. Covered by
`idiom_gaps::exception_constructor_erased_object_varargs_use_java_display_helper`.

Follow-up 2026-08-30 regex Pattern erased-display pass:
`java.util.regex.Pattern` now participates in the platform erased-display
downcast table when the source imports or otherwise mentions the regex Pattern
facade. The support `Pattern` carrier exposes Java's `Pattern.toString()` shape
by returning the original pattern text, so an erased `Object` holding a Pattern
can display `a+` instead of falling into the unknown-erased panic. Covered by
`idiom_gaps::erased_pattern_display_uses_platform_regex_pattern_text`.

Follow-up 2026-08-30 optional map-get carrier pass:
`Map.get` into an optional slot now treats boxed Java primitives such as
`Integer`/`java.lang.Integer` as compatible with their primitive storage type
for the borrowed optional element projection. This lets `Map<String, Integer>`
read `Integer old = counts.get(name)` as
`counts.get(name.as_str()).copied()` instead of falling through to the generic
optional conversion path and emitting the redundant
`counts.get(name.as_str()).as_ref().copied()`. Covered by
`fixtures::translates_map_get_null_ternary_with_borrowed_optional_carrier`.

Follow-up 2026-08-30 borrowed stream findFirst carrier pass:
`findFirst()` over a borrowed stream now tags the lowered `.next()` terminal as
an already-projected optional carrier. The value is already `Option<&T>`, so the
generic optional conversion path must not add another `.as_ref()` before clone
recovery. This fixes `Option<&String>` vs `Option<String>` generated-code
failures while keeping the optional carrier decision explicit in the stream
terminal lowering. Covered by
`fixtures::stream_find_match_generated_rust_preserves_behavior` and
`fixtures::translates_stream_find_first_and_any_match_through_slot_coercion`.

Follow-up 2026-08-30 stream flatMap optional-sequence pass:
flatMap lambda collection sources now unwrap optional sequence returns before
iterating the inner collection. For `group.names().stream()`, source method
signature shape proves `group.names()` is `Option<Vec<String>>`, so the emitted
closure now iterates `group.names().unwrap().into_iter()` instead of iterating
the outer `Option` and collecting `Vec<Vec<String>>`. Nested flatMap stream
pipelines also reuse the lambda scoped environment built while lowering prefix
statements, so a prefix such as `let names =
group.names().expect("Java null receiver")` is treated as a non-optional local
inside `names.stream()`. Covered by
`fixtures::stream_flat_map_projection_generated_rust_preserves_behavior`,
`fixtures::translates_stream_flat_map_block_pipeline`,
`slot_shape_design::stream_plan_infers_flat_map_lambda_projection_stream_shape`,
and `slot_shape_design::stream_plan_lowers_flat_map_block_returning_stream_pipeline`.

Follow-up 2026-08-30 try-with-resources BufferedReader model pass:
the built-in `Files.newBufferedReader` external fact and emitted facade support
now return `BufferedReader` rather than the supertype `Reader`, matching Java's
API. Try-with-resources resource lowering therefore keeps the resource concrete
instead of erasing it to `Box<dyn Any>`, preserving `read_line()` nullability and
the concrete `close()` cleanup path. Covered by
`external_packs::files_new_buffered_reader_returns_buffered_reader_fact` and
`fixtures::try_with_resources_nullable_read_and_closeable_cleanup_compiles`.

---

## 2026-09-02 — one regression fixed, one isolated but open

### Fixed: `Map.get` into a declared local aborted at runtime

`native_map_get_required_declared_value_local_shape` (added in `6312cb8`, the most
recent commit, one caller, no test naming it) gave a `Map.get` initializer the
*element borrow* shape, discarding the optionality:

```rust
let miss: &String = m.get(&_java_borrowed_miss_0).expect("Java null receiver");
```

Its premise is wrong — `String v = map.get(k)` in Java does **not** mean the value
is present. The worst form needed no null test at all:

```java
String miss = m.get("b");
return "reached:" + miss;      // Java: reached:null
```

which **panicked** (`Java null receiver`). Removed the refinement and the dead
function (57 lines). Now `let miss: Option<String> = m.get(..).cloned();`, and the
program prints `reached:null`, matching the oracle. `MapGetNull2.java` no longer
emits `compile_error!`.

### Fixed: the receiver was projected twice

`LinkedNode.java` and `NullableArrayLengthCatch.java` failed the differential gate
with a doubled projection:

```rust
cur.as_ref().ok_or_else(..)?.ok_or_else(..)?.value      // E0599
```

The second unwrap lands on the element the first already produced. Isolated by
bisection: the same field read is correct or not depending only on its syntactic
context.

| Java | before |
|---|---|
| `return cur.value;` | single unwrap, correct |
| `int v = cur.value;` | single, correct |
| `return cur.value + 1;` | **double**, `E0599` |
| `t += cur.value;` | **double**, `E0599` |

Cause: `apply_optional_receiver_slot_projection` (`src/conversion_lowering.rs:89`)
projects the receiver, discharging the optionality, and then applies a conversion
that discharges it *again*. A slot-directed lowering took a different route and
so stayed correct, which is why the defect looked context-sensitive.

`strip_redundant_java_null_expect_after_unwrap` already existed for exactly this
— and was applied at **one of the four** call sites of that function. The other
three had no guard. Rather than adding it at the remaining three, it now runs
**inside** `apply_optional_receiver_slot_projection`, so no caller can forget it:
the same one-question-many-answers shape 0042/0044/0046 describe, closed by moving
the answer to where the question is asked.

Follow-up 2026-09-09: a cleanup attempt to delete the two remaining
non-fallible lower-side wrappers around `apply_optional_receiver_slot_projection`
was rejected by measurement. The
`slot_shape_design::optional_receiver_field_element_recovery_does_not_borrow_returned_call_result`
generated-crate check exposed a signature bug instead: a method that directly
returned another call result was inferred as `&str` while the call necessarily
returned an owned `String`. `signature.rs` now refuses the borrowed string-view
return for direct returned call results; the wrappers remain load-bearing until
the projection/conversion helper records enough shape to make the final strip
local to the helper.

Two hypotheses tried and **rejected by measurement**, recorded so they are not
retried: feeding the conversion the element shape instead of its own `actual`
(no effect — `element_shape` is `None` on that path), and disabling
`lower_optional_receiver_field_access_plain` at either of its two call sites (no
effect — the sites route to each other, so only disabling both changes anything,
and that removes the unwrap entirely and is wrong).

Verified: `differential` **2 passed, 0 failed**; `type_coercion`, `object_model`,
`semantic_facts` and `slot_shape_design` unchanged against HEAD; `--lib` green.

### Also fixed: the same double, reached through a map value

`m.get("a").get(1)` produced `.expect("Java null receiver").expect("Java null receiver")`
— the outer call adapted its own receiver, asked the *plan* whether it was
optional, and got "yes" because the plan does not record what the inner
projection already did to the expression (0007/0008).

The strip pass had grown **three** hand-written arms, each recognising the one
shape of "already discharged" that was in front of whoever added it
(`.unwrap()`, `.ok_or_else(..)?`, `.as_ref().expect(..)`), so a fourth shape —
`.get()` under the inner expect — was simply not on the list. Around 150 sites
construct this unwrap; enumerating the shapes they can produce is not a strategy
that terminates.

There is one predicate now, `java_optionality_already_discharged`, and the strip
shares it rather than carrying its own list. More importantly the null-receiver
unwrap is **idempotent at its constructor**: `method_expect_projection` returns
the expression unchanged when it already discharges, so the double cannot be
built in the first place and the strip is only a net for expressions assembled
without it.

Checked before generalising: a nullable-valued `Map<String,String>` lowered its
value slot as `String`, not `Option<String>`, so no genuine `Option<Option<T>>`
needed two identical null-receiver unwraps. That separate defect is now fixed
for literal map-value writes: receiver-call usage records the literal-null
argument indexes, so `Map.put(k, null)`/`putIfAbsent(k, null)` optionalize only
the value type and `Map.put(null, v)` does not widen the value slot. Gated by
`signature::tests::literal_null_map_value_receiver_call_optionalizes_value_type_only`
and `tests/differential/MapPutNullValue.java`.

Gated by `tests/differential/NestedMapValueRead.java`. The sequence-side twin —
`Col.values()[i]` unwrapping a non-optional element — is now covered by
`tests/differential/EnumValuesElementIntoObject.java`.

### Partly fixed: `Map.get` locals split receiver-only from value-context use

Giving a `Map.get` initializer the map value's *borrow* shape discarded the
optionality, so

```java
String miss = m.get("b");
return "reached:" + miss;      // Java prints reached:null
```

aborted the process. Removing that shape fixes it, and costs something real:

| | original borrowed fix | broad optional fix |
|---|---|---|
| binding | `let p: &Payload = m.get(k).expect(..)` | `let p: Option<Payload> = m.get(k).cloned()` |
| a miss | aborts | catchable `NullPointerException` |
| the value | borrowed | **cloned**, so the alias is broken (0002) |

The catchability is an improvement — Java throws, it does not abort. The clone is
not: a local that is only ever dereferenced wants the borrow, and the borrow was
right *for that body*. It was wrong for the one that reaches a value context, and
the shape was chosen without asking which body it was in.

Deciding by **use** is the version that costs nothing: borrow-and-unwrap when the
local is only ever a receiver, keep the `Option` when it can reach a value
context. The liveness machinery that would answer this already exists.

Follow-up 2026-09-06: the signature-side predicate now makes that use split
explicit for final erased-object facade values. A `Map.get` local only receives
the shared-borrow constraint when its post-initializer uses are receiver
dereferences; value-context uses such as `return payload;` keep the owned
optional/value shape. This removes the broad constraint that could discard
nullability for locals that are not immediately dereferenced, while preserving
the existing receiver-only optimization. Regressions:
`slot_shape_design::native_map_get_into_final_facade_local_keeps_value_shape`
and
`slot_shape_design::native_map_get_value_context_final_facade_local_keeps_option_shape`.

Follow-up 2026-09-06: lowering now respects the same split for initialized
local declarations. Receiver-only final facade `Map.get` locals lower as
`Option<&T>` without `.cloned()`, preserving aliasing while keeping the later
receiver-null projection catchable. Value-context locals still lower as owned
optionals.

Follow-up 2026-09-07 easy-row pass: four nullable/optional idiom rows were
removed from the failure baseline after focused generated-crate checks. Three
were stale assertions for current lowering:
`constructor_initialized_local_receiver_does_not_get_optional_adapter`,
`optional_field_mutating_overload_call_uses_signature_name_and_mut_receiver`,
and
`optional_map_string_get_compares_to_cross_class_static_string_constant`. The
fourth,
`null_guarded_array_parameter_writes_use_mut_optional_receiver_once`, exposed a
real Rust borrow conflict in checked array writes: the mutable slot was held
while the RHS read the same nullable receiver. Simple checked array assignments
now precompute the RHS before taking the mutable slice borrow; compound writes
keep the old slot-first ordering so they can read the previous slot value.

Follow-up 2026-09-07 nullable map-value null comparison: `Map.get(k) == null`
now treats both map absence and a present nullable value containing Java `null`
as null. The map-get null-comparison branch lowers nullable map values to
`get(k).is_none_or(|value| value.is_none())`; `!= null` uses the matching
`is_some_and(|value| value.is_some())`. This fixes the remaining behavioral
gap in `tests/differential/MapPutNullValue.java`, where Java distinguishes
neither missing keys nor present null values at the `get(k) == null` surface.
Focused verification on 2026-09-07 compiled and ran that promoted fixture on
both sides: Java produced `true:null:x`, and the translated Rust binary produced
the same output.

Follow-up 2026-09-07 focused erased-display loop: the cheap platform display
oracle now includes the promoted `ErasedStringBuilderDisplay` and
`ErasedUriDisplay` fixtures, alongside the existing atomic, class, file,
input-stream, locale, path, pattern, random, and URL carriers. This is coverage
hygiene for already-modeled carriers; unknown erased display remains open.

Follow-up 2026-09-07 focused erased-display loop: the same cheap oracle now
also includes `ErasedPlatformDisplay`, so the broad platform-display fixture is
covered by the targeted recheck instead of only by the full differential sweep.

Follow-up 2026-09-10 focused erased-display note refresh: the cheap oracle now
also covers nullable list display and map-entry display carriers, including
`ErasedNullableArrayListDisplay`, `ErasedNullableBooleanArrayListDisplay`,
`ErasedNullableByteArrayListDisplay`,
`ErasedNullableCharacterArrayListDisplay`,
`ErasedNullableDoubleArrayListDisplay`,
`ErasedNullableFloatArrayListDisplay`, `ErasedNullableIntegerArrayListDisplay`,
`ErasedNullableLongArrayListDisplay`, `ErasedNullableShortArrayListDisplay`,
`ErasedMapEntryDisplay`, `ErasedSimpleImmutableEntryDisplay`, and
`ErasedTreeSetDisplay`. Unknown erased display remains open; this only records
the current promoted-fixture coverage.
Codex follow-up 2026-09-14 null-bearing `Arrays.asList` varargs pass: when a
literal null appears in `Arrays.asList(...)`, argument lowering now chooses one
nullable element slot from the non-null peers and lowers every element through
that slot. The concrete erased fixture
`tests/differential/ErasedArraysAsListNullDisplay.java` now compiles and matches
Java (`x[a, null, c]:[a, null, c]`) under the focused
`erased_display_fixtures_match_java_output` oracle.

Follow-up 2026-09-10 cleanup: the erased-display downcast collectors in
`stdlib` are pinned as private helpers, and the platform erased-display
capability predicate is pinned crate-local. This is boundary hygiene only; it
does not add a fallback for unknown erased display without type/capability
evidence.

Follow-up 2026-09-10 erased-display carrier arm cleanup: the platform,
external, and source erased-display downcast collectors now share one private
`erased_display_to_string_downcast_arms` template for `T`, `&T`, `Rc<T>`, and
`Arc<T>`. This removes a duplicated policy table while preserving the same loud
unknown-runtime-target fallback. Guard:
`layering::stdlib_erased_display_downcast_collectors_stay_internal`.

Codex follow-up 2026-09-15 erased `PriorityQueue` display pass: modeled
`JavaPriorityQueue<T>` carriers already implement Java collection display, but
the erased-object runtime did not downcast them before the loud unknown-erased
fallback. `String`, `Integer`, `Double`, and `Boolean` priority-queue carriers
now use the existing `JavaPriorityQueue<T>::to_string()` path when such a queue
is stored as `Object`. Promoted differential fixture:
`tests/differential/ErasedPriorityQueueDisplay.java`; guard:
`cargo test --test differential erased_display_fixtures_match_java_output`.

Codex follow-up 2026-09-15 erased hash collection display pass: standard
`HashMap`/`HashSet` carriers now have the same erased-object display bridge as
the existing ordered collection wrappers for deterministic one-element
regressions. This is still carrier-specific support, not a fallback for
arbitrary unknown erased runtime targets. Promoted differential fixtures:
`tests/differential/ErasedHashMapDisplay.java` and
`tests/differential/ErasedHashSetDisplay.java`; guard:
`cargo test --test differential erased_display_fixtures_match_java_output`.
