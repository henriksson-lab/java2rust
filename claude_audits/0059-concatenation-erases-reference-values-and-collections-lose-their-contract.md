# 0059 — String concatenation erases reference values; ordered collections lose their contract

Recorded 2026-08-23.
Status: **open, concrete repros mitigated**.
Severity: **medium/high** — the original broad concat, collection-order,
collection-mutation, enum-body, and modeled display divergences are now covered
by differential fixtures. The remaining semantic risk is narrower but still real:
an arbitrary external value erased behind `Object` cannot recover Java dynamic
`toString()` unless source downcast evidence, platform support, or an explicit
external display-capability fact is available.
Kind: behavioural (differential), JDK facades / collections / enums.

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
oracle (`null,1,false`). The audit stays open for ordered collection, set
construction, and enum-body sections below.

Updated 2026-08-24 fourth pass: §4's `new HashSet<>()` diamond repro is
promoted to `tests/differential/SetConstruction.java` and now passes the Java
oracle (`1,1,1`). Raw/diamond set constructor owners no longer default their
missing element type to erased `Object` for the hash-free carrier decision, so
they keep the native `HashSet` carrier instead of falling into the `Vec` carrier
branch. The audit stays open for ordered collection and enum-body sections
below.

Updated 2026-08-24 fifth pass: §5's enum constant body repro is promoted to
`tests/differential/EnumConstantBodies.java` and now passes the Java oracle
(`5,6`). Enum constant class bodies are preserved as `Expr::New { body: ... }`
initializers, and abstract enum methods with one-return same-signature constant
overrides lower to `match self` dispatch instead of the abstract-method panic.
The audit stays open for the ordered collection discipline in §2.

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
is promoted and covers both concatenation and direct `Object.toString()` over
autoboxed `int`/`char` values (`i=7,c=A,direct=7/A`). The differential suite is
green at 102 fixtures.

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
residue and live map-view mutation such as `keySet().remove(...)`.

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
enhanced-for `Map.Entry.setValue` live-view carrier remains open because current
`entrySet` iteration still uses borrowed tuple reads.

Follow-up 2026-08-27 values-view contains pass: aliased and direct
`Map.values()` receivers now route `contains(value)` through the backing map
instead of asking a materialized values snapshot. The lowering mirrors
`Map.containsValue`: it evaluates the searched value once, iterates
`map.values()`, and compares each live backing value against that temporary.
Covered by `lower::tests::map_values_alias_contains_uses_backing_map` and
`tests/differential/MapValuesContains.java` (`false,true,true`). The full
differential oracle passed with 164 fixtures.
