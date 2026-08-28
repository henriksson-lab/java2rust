# 0057 — Java exceptions still leak as panics, and general `finally` completion is not represented

Recorded 2026-08-23.
Status: **open**.

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
and `StringReplaceChar.java` pass the Java oracle as part of the 47-fixture
differential suite.

Updated 2026-08-24 again: `StringBuilder.append(intLocal)` now keeps primitive
locals in their copy-value shape. The local-signature shortcut that selects a
concrete borrowed value is no longer eligible for primitive/copy types, so the
conflict winner (`CopyValue`/native) agrees with the rendered local type.
`AppendIntLocal.java` is promoted and passes the Java oracle as part of the
48-fixture differential suite.

Updated 2026-08-24 third pass: `Character.isDigit`, `isLetter`,
`isWhitespace`, `toUpperCase`, and `toLowerCase` are planned as native Java
static calls. Lowering now emits the corresponding Rust `char` methods, and
`CharacterStatics.java` is promoted into the 49-fixture differential suite.

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
coverage into `tests/differential/`. The then-59-fixture differential suite gates
`null instanceof String == false` and a non-null `String` object matching the same
check, rather than relying on this file's prose inventory.

Updated 2026-08-24 ninth pass: three more verified-correct control/null shapes
are promoted from repro-only coverage: `DoWhile.java`, `ShortCircuitLocal.java`,
and `TernaryNull.java`. The then-current differential suite gated 62 fixtures, including
do/while execution, observable `&&`/`||` short-circuiting, null-guarded member
access, ternary null propagation, and guarded division.

Updated 2026-08-24 tenth pass: four verified-correct char/string fixtures are
promoted from repro-only coverage: `CharOps.java`, `CharArray.java`,
`StringMethods.java`, and `StringSplit.java`. The differential suite now gates
66 fixtures, including char arithmetic/casts/comparison, `toCharArray`,
char compound assignment/increment, `String.valueOf(char)`, string search and
case methods, and regex-style `String.split`.

Updated 2026-08-24 eleventh pass: three verified-correct control-flow fixtures
are promoted from repro-only coverage: `EnhancedForArray.java`,
`EnhancedForCollection.java`, and `LabeledBreakOnly.java`. The differential
suite now gates 69 fixtures, including enhanced-for over primitive arrays,
reference arrays and `List<String>`, plus a labeled break out of nested loops.

Updated 2026-08-24 twelfth pass: four verified-correct exception fixtures are
promoted from repro-only coverage: `ExceptionsCustom.java`,
`CatchSupertype.java`, `CatchException.java`, and
`TryCatchReturnFinally.java`. The then-current differential suite gated 73 fixtures,
including user exception construction/message flow, catching a user subclass
through `RuntimeException`, catching a built-in unchecked exception through
`Exception`, exception crossing a call boundary, and `finally` after return in
both `try` and `catch`.

Updated 2026-08-24 thirteenth pass: `SwitchChar.java` is promoted from
repro-only coverage. The then-current differential suite gated 74 fixtures, including
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

**30 areas probed for this audit's exception/control-flow surface, 29 verified
correct.**

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
`tests/differential/StaticStringBuilder.java` pinned it in the then-53-fixture
differential suite.

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
   time. The current suite gates 108 fixtures, including `IntegralParseRadix.java`,
   `FinallyReturnOverride.java`, and `CatchFinallyReturnOverride.java`; the
   repro inventory remains useful, but it should not be treated as a gate until
   each file is promoted.

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
`tests/differential/OptionalFactoryGetCatch.java` is promoted and verifies
`Optional.empty().get()` reaches `catch (NoSuchElementException)` while
`Optional.of` and `ofNullable(null)` preserve presence semantics. The
differential suite is now at 113 fixtures.

Update 2026-08-25: `TreeMap.firstKey()` and `TreeMap.lastKey()` are no longer
panic-only inside propagated `try` scopes. The normal native collection lowering
uses the same `NoSuchElementException` result projection for ordinary callers
and unwraps it through the shared uncaught-Java-exception panic adapter, while
propagated calls use `keys().next()/next_back().ok_or_else(...).map(|value|
value.clone())?` and raise `NoSuchElementException` for empty maps.
`tests/differential/TreeMapFirstLastKeyCatch.java` verifies both empty-map
catches plus the non-empty `a`/`b` key results. The differential suite is now at
114 fixtures.

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
oracle; the differential suite is now at 110 fixtures.

Update 2026-08-25: `StringBuilder.setLength(-1)` and
`StringBuffer.setLength(-1)` no longer silently clamp to zero. The support layer
now exposes `set_length_result`, the built-in and external-pack facts mark
`setLength`/`set_length` as mutating and throwing
`StringIndexOutOfBoundsException`, and the facade ABI routes propagated
`try`/`catch` calls through the fallible path. Ordinary non-fallible callers
unwrap through the shared uncaught-Java-exception panic adapter.
`tests/differential/StringBuilderSetLengthCatch.java` verifies normal
grow/truncate behavior plus caught negative-length failures against the Java
oracle; the differential suite is now at 111 fixtures.

Update 2026-08-25: `List.get(index)` is no longer routed through the ordinary
`Option` lookup/coercion path when the call is inside a propagated
`try`/`Result` scope. The catchable branch now lowers directly to
`get(index).cloned().ok_or_else(|| JavaException::new(
"IndexOutOfBoundsException", ...))?`, after first checking the signed Java index
for `< 0` before casting to `usize`; ordinary callers keep the existing
panic-preserving behavior. `tests/differential/ListGetCatch.java` verifies both
high and negative out-of-range `ArrayList.get` calls reach
`catch (IndexOutOfBoundsException)`, and the full differential suite now passes
115 fixtures.

Update 2026-08-25: `List.set(index, value)` now has the same catchable
propagated path. The result projection checks the signed Java index before the
`usize` cast, returns the old element through `Ok(old)`, and assigns through a
fallible `get_mut(...).ok_or_else(...)` so both negative and high indexes raise
`IndexOutOfBoundsException` instead of panicking. `tests/differential/
ListSetCatch.java` verifies the normal return value, mutation, and both caught
out-of-range cases; the full differential suite now passes 116 fixtures.

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
`catch (NegativeArraySizeException)` for `new BitSet(-1)`; the full
differential suite now passes 117 fixtures.

Update 2026-08-25: `new String(bytes, offset, len)` no longer silently clamps
invalid byte ranges. The support helper now has a `java_string_from_byte_range_result`
variant that returns `IndexOutOfBoundsException`, the ordinary helper unwraps it
through the shared uncaught-Java-exception panic adapter, and propagated
three-argument `String` constructor lowering applies `?` to the result helper.
`tests/differential/StringByteRangeCtorCatch.java` verifies the valid slice plus
high and negative out-of-range catches; the full differential suite now passes
118 fixtures.

Update 2026-08-25: `new StringBuilder(capacity)` and
`new StringBuffer(capacity)` no longer silently clamp negative signed capacities
to zero. The shared support now checks signed capacity inputs and reports
`NegativeArraySizeException`; ordinary `new_1` calls unwrap through the existing
uncaught-Java-exception panic adapter, while propagated constructor lowering
routes integral one-argument `StringBuilder`/`StringBuffer` construction through
`new_capacity_result(...) ?`. `tests/differential/StringBuilderCapacityCatch.java`
verifies normal capacity construction plus both negative-capacity catches; the
full differential suite now passes 119 fixtures.

Update 2026-08-25: `Math.toIntExact(long)` no longer lowers to truncating
`as i32`. The native static lowering now uses `i32::try_from(...)` and maps
overflow to `JavaException::new("ArithmeticException", ...)`; propagated
`try`/`catch` scopes route the call through the result projection, while
ordinary callers keep the existing uncaught-exception panic policy. The facade
ABI is result-wrapped and still marked as an associated/static call. The
external-call propagation planner now treats result-wrapped facade ABI as a
catchable result source, not only checked thrown-effect facts. `tests/
differential/MathToIntExactCatch.java` verifies normal values plus caught
overflow against the Java oracle; the full rebuilt differential suite now
passes 122 fixtures.

Update 2026-08-25: `Optional.of(null)` now raises `NullPointerException` through
the same catchable Java-exception path used for required nullable arguments.
The slot-aware `Optional.of` lowering unwraps the nullable argument before
constructing the returned optional, so a caught null no longer becomes
`Some(None)` and bypasses `catch (NullPointerException)`.
`tests/differential/OptionalOfNullCatch.java` verifies the Java oracle; with the
new fixture the differential suite is expected at 123 fixtures after rebuilding
the release translator.

Update 2026-08-25: nested no-catch/no-resource `try/finally` inside the
simple loop-body finally fast path no longer emits `continue` inside a closure.
The loop fast-path predicate now admits nested `try/finally` statements whose
`finally` block falls through, and the existing abrupt-rewrite pass sees loop
abruptions through that nested body. `FinallyNestedContinue.java` moved out of
`tests/differential/known-gaps/` into the main differential suite and verifies
Java's `0ioio2io` oracle. The differential suite now passes 124 fixtures, and
the known-gap set is down to 6 fixtures.

Update 2026-08-25: `Thread.sleep(-1)` no longer casts the negative Java `long`
to a huge Rust `u64` duration in catchable contexts. Propagated `Thread.sleep`
calls now route through a result projection that returns
`IllegalArgumentException` for negative timeouts before sleeping; ordinary calls
keep the existing panic/blocking lowering policy. `tests/differential/
ThreadSleepCatch.java` verifies the Java oracle, and the differential suite now
passes 125 fixtures.

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
(`iae`) against the translated Rust output; the differential suite is now
expected at 195 fixtures.

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
`tests/differential/StringTokenizerCharAtBoundsCatch.java`.

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
rebuilt differential suite is now 191 fixtures. Suppressed exception attachment,
throwing `finally`, and the general nested/labeled completion carrier remain
open.

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
the full differential suite passes 200 fixtures. Broader panic-only exception
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
differential suite passes 202 fixtures. Broader panic-only exception surfaces
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
