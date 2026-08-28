# 0056 — `null` is planned away, not represented: null tests are constant-folded and null values print as `""`

Recorded 2026-08-23.
Status: **open, partially mitigated**.
Severity: **critical** — several everyday null-bearing Java shapes still fail
to compile or panic instead of reaching translated control flow, and unknown
erased-object display still has no dynamic Java `toString` recovery.
Kind: semantics / lowering.

Follow-up 2026-08-28 nullable OutputStream receiver pass:
nullable JDK facade receivers for result-returning calls now use the fallible
optional receiver projection when the current context can propagate
`JavaException`. `OutputStream out = null; out.write(decoded);` in a throwing
method now lowers through `ok_or_else(|| JavaException::new("NullPointerException", ...))?`
before calling the selected byte-slice overload, instead of panicking through
`.expect("Java null receiver")`. Regression:
`tests/idiom_gaps.rs::optional_output_stream_write_byte_array_uses_slice_overload`.
Broader non-argument catchable null coercions remain open.

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

Method: differential testing against `javac`/`java` as oracle, per the recipe in
`tests/differential.rs`. Every claim below is a *verified* Java-output vs.
compiled-Rust-output comparison, not a reading of the emitted text. Repros are
in `repro/` beside this file, named as they would be in `tests/differential/`.

Follows [0052](done/0052-the-example-corpora-are-asserted-on-but-never-compiled.md) — the original 23-fixture differential corpus
contained **zero** uses of `null`, so none of this was reachable by any test.
Extends [0034](done/0034-object-array-allocation-produces-an-empty-vec.md),
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
differential suite is at 101 fixtures, and promoted null fixtures cover null
concat, bare null concat, nullable string arguments, null default returns,
`Map.get` misses, nullable constructor arguments stored in fields, nullable
`List` elements, nullable reference-array elements, source `Object.toString()`
recovery, catchable nullable source-parameter receiver dereference, and
catchable nullable source-call values coerced into required argument slots, plus
the `Integer.parseInt(null)` `NumberFormatException` contract. The former silent
`x == null` fold for unrepresented reference operands is now a
`compile_error!`, and erased object display is loud rather than blank for
unknown targets.

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
2. **Handle nullable constructor/method parameters explicitly** instead of
   forcing `.expect` when a call site passes `null`.
3. **Finish unknown/external erased object display capabilities.**
   Source `Object o = new Point(3); "a" + o` is covered. Opaque external
   facades now delegate both direct `Display` and erased-object runtime display
   to a modeled zero-argument `toString(): String` API fact, and facades without
   that fact still fail loudly. Built-in `java.io.File` is also covered through
   its existing facade `Display` implementation. The remaining gap is
   erased/unknown runtime values where no source downcast arm or explicit
   external API fact identifies a Java `toString()` surface.
5. Promote each remaining repro as it becomes loud-or-correct. Do not add a
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

Follow-up 2026-08-28: `Object value = new Random(1); "x" + value` now follows
the same platform erased-display route. `Random` is included in the generated
platform downcast table when it appears in the source surface, and the generated
external facade `Display` fallback emits Java-object-style
`java.util.Random@0` instead of panicking. Promoted differential fixture:
`ErasedRandomDisplay`, which compares Java and translated output through the
stable `xjava.util.Random@` prefix. This remains a platform-carrier mitigation;
arbitrary unknown erased display without source/platform/fact evidence remains
open.

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
matches Java's `null` oracle. The differential suite is now at 193 fixtures.

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
