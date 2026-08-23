# 0044 — The external-fact *owner key* is computed twelve times, and the copies disagree

Status: **fixed** — the demonstrated owner-key disagreement classes now route
through shared helpers or a private source-context owner-key wrapper.

Codex follow-up 2026-08-21: independently validated. Wording caveat: the
inventory is twelve private producer functions across nine modules, not exactly
"one per module". The typed-key direction is still the right fix, but the
constructor cannot be only `for_receiver(ty)`: `Optional.get` needs the
container owner while operations on a contained value need the element owner, so
the query mode or method context must be explicit.

Codex follow-up 2026-08-21, holding fix: the signature receiver constraint path
now receives a `TypeEnv` for method/constructor parameters and keys bounded type
parameters through their bound, matching the call-planning path. The same path
also uses method-aware Optional receiver keying: `Optional.get`,
`Optional.isPresent`, and `Optional.orElse` key as `Optional`, while nullable
storage calls still unwrap to the element owner. Regressions:
`type_param_bound_external_receiver_fact_shapes_signature_mutability` and
`java_optional_get_does_not_use_element_owner_external_fact`. The audit remains
open because the owner-key producers are still duplicated and untyped.

Codex follow-up 2026-08-22: the narrow field-style producer for
`Named`/`Unknown` receivers is now centralized as
`stdlib::external_named_receiver_owner_key`, with direct unit coverage for its
boundary (`Named`/`Unknown` canonicalize; `String`, arrays, primitives, void and
wildcards do not). The simple copies in `semantic_analysis` and `resolve` now
call it. The audit remains open for the richer API receiver modes: Optional
method ownership, type-parameter bound substitution, array/string policy, and
the still-untyped `Option<&str>` lookup APIs.

Codex follow-up 2026-08-22, parallel audit: still valid. The next tractable
slice is a typed API-owner helper in `stdlib`, with one value-owner mode and one
method-aware mode: `Optional.get`/`isPresent`/`orElse` should key as `Optional`,
nullable stored values should still use the element owner, and type parameters
should substitute their bound through `TypeEnv`. The first callers to move are
the private producers in `call_plan`, `signature`, and `semantic_analysis`,
where existing tests already cover the Optional and bounded-type-parameter
regressions.

Codex follow-up 2026-08-22, API-owner slice: the richer API receiver owner-key
policy now lives in `stdlib::external_api_receiver_owner_key` and
`stdlib::external_api_method_receiver_owner_key`. The shared helper owns
Optional value unwrapping, Optional container method keying, `TypeEnv`
type-parameter-bound substitution, String/Array policy, and Rust-path
simple-name canonicalisation. `call_plan`, `signature`, and `semantic_analysis`
now delegate to it for the first high-risk API-owner callers. The audit remains
open for the remaining field-style/simple wrappers, `usage`, `aliasing`,
method-reference lowering/static-owner corners, and the still-untyped public
`Option<&str>` lookup APIs.

Codex follow-up 2026-08-22, method-reference slice: the parsed type-target
fallback for method references now calls the method-aware helper too, so
`A::getBaseString` with `A extends Allele` records `Allele.getBaseString`, not
`A.getBaseString`. The parallel read-only audit found the remaining wrappers in
`usage` and `aliasing` are not safe one-line substitutions: usage needs
analysis-side `TypeEnv` threading for modeled call boundaries, and aliasing
currently preserves array spellings that the API-owner helper intentionally
normalizes to `Array`. The public `Option<&str>` lookup APIs remain the broader
type-safety hole.

Codex follow-up 2026-08-22, usage boundary slice: usage analysis now builds a
per-method/constructor `TypeEnv` for type-parameter bounds and threads it to
modeled receiver boundary lookup. The private `usage::external_receiver_type_key`
copy was deleted; modeled receiver boundaries now call
`stdlib::external_api_receiver_owner_key`, so `A extends ExternalHandle` uses the
`ExternalHandle` fact when deciding whether the call is content-read-only.
Remaining wrappers at that point were the static-owner expression classifiers, the
array-spelling-sensitive aliasing lookup, `stdlib`'s private primitive-aware key
for known built-ins, and the still-untyped public `Option<&str>` lookup APIs.

Codex follow-up 2026-08-22, field-owner cleanup slice: the remaining
field-style/simple wrappers in `call_plan`, `member_access_plan`,
`semantic_analysis`, and `resolve` were removed or inlined. External field
owner lookup now calls `stdlib::external_named_receiver_owner_key` directly at
those sites, and a source guard pins the absence of the local helper copies.
Remaining scope at that point was the divergent aliasing lookup, static-owner
expression classifiers, method-reference lowering, `stdlib`'s private
primitive-aware key for known built-ins, and the still-untyped public
`Option<&str>` lookup APIs.

Codex follow-up 2026-08-22, method-reference lowering slice:
`method_reference_lowering` no longer has its own `JavaType` -> owner-key helper.
The call-plan candidate path now asks
`stdlib::external_api_method_receiver_owner_key`, so method references use the
same method-aware Optional/array/String/key policy as call planning and
signature solving. The existing semantic bounded-type-parameter fixture remains
the behavior guard, and the slot-shape source guard now forbids the old local
`java_type_callable_owner_key` copy. Remaining scope at that point was the
divergent aliasing lookup, static-owner expression classifiers, `stdlib`'s
private primitive-aware key for known built-ins, and the still-untyped public
`Option<&str>` lookup APIs.

Codex follow-up 2026-08-22, known-builtin and aliasing slices:
`stdlib`'s private primitive-aware owner-key helper is now explicitly scoped as
`external_known_builtin_receiver_owner_key`, used by known built-in method-return
and field-constant support, and guarded against regressing to a generic
`external_receiver_type_key` copy. Observable-alias external mutator detection
now builds a per-method/constructor `TypeEnv` and calls
`stdlib::external_api_receiver_owner_key`, so bounded type parameters and arrays
use the same owner keys as usage/signature/call planning. Regression coverage
pins both `T extends Record` resolving to `Record.setFlag` and array receivers
resolving to `Array.setFlag`; the source guard forbids
`aliasing::external_receiver_type_name`. Remaining scope is now the static-owner
expression classifiers and the still-untyped public `Option<&str>` lookup APIs.

Codex follow-up 2026-08-22, static external-call collector slice: the imported
static-owner collector no longer has a separate
`external_pack_static_owner_for_call` classifier. External static-call owner
facts now pass through one `external_static_call_owner_key` helper for
canonicalization, lowercase static-owner filtering, arity-specific fact lookup,
and explicit `receiver_type` override handling. A source guard pins the
collector to that helper. Remaining static-owner scope is now the qualified
owner expression walkers and `lower.rs`'s source-context/static-retarget
classifiers; those were left separate because they answer emitted source-type
questions rather than external fact owner-key lookup.

Codex follow-up 2026-08-22, qualified owner walker slice:
`stdlib::qualified_static_owner_java_name` is now `pub(crate)` and call planning
uses it for qualified static-call argument owner lookup. The duplicate
`call_plan::qualified_static_owner_java_key`, `static_owner_root_segment`, and
`static_owner_java_name` walkers were deleted, with a source guard preventing the
copy from returning. The remaining `lower.rs` static-owner helpers are still
intentionally separate source-context/static-retarget classifiers, and the
untyped public `Option<&str>` fact lookup APIs remain the next larger 0044
cleanup.

Codex follow-up 2026-08-22, source-context owner wrapper slice: the
`lower.rs` source-context/static-retarget classifiers now pass owner candidates
through a private `SourceContextOwnerKey` wrapper instead of raw `Vec<String>`
candidate lists. The wrapper owns canonical/simple-name context matching, Rust
owner aliases, nested-owner candidate expansion, and string extraction at the
public boundary. A source guard pins `fallback_receiver_type_name_candidates`
and the source-context static fallback away from raw owner-candidate strings.
The remaining public `Option<&str>` fact lookup APIs are still a hardening
opportunity, but the current production callers for the audited disagreement
axes now compute keys through the shared typed policy before crossing that
boundary.

Codex follow-up 2026-08-22, residual audit: a parallel read-only audit found no
remaining demonstrated 0044 correctness defect. Public fact lookup APIs still
accept untyped `Option<&str>` owner keys in `facts`, `external_packs` and
`stdlib`, but current call planning, signature solving, semantic analysis,
usage, aliasing, field lookup, method-reference lowering, static owner facts and
`lower.rs` source-context fallbacks no longer compute those owner strings ad hoc
for the original hard cases. A future typed `ExternalOwnerKey` wrapper at the
public lookup boundary would make future drift harder, but that is cleanup
rather than a residual live fix for this audit.

Severity: **high** — silently changes parameter mutability, receiver mutability
and inferred return types, and does so *inconsistently within a single method*
Kind: duplicated code path — one concept, twelve private near-duplicates

Found by asking the calibration question: *how many places compute a Rust-side
name from a Java one, and do they agree?*

## The concept

Every external-fact table in the project is keyed by an **owner string**:

```
FactSet::external_api_effect_for_receiver{,_with_arity,_or_rust_name}   facts.rs:1811,1836,1860
FactSet::external_field_for / external_field_for_key                    facts.rs:2676,2712
FactSet::external_ffi_receiver_handover_for_receiver                    facts.rs:2255
FactSet::external_library_pack_for_type                                 facts.rs:2776
external_packs::external_api_effect_for_receiver{,_with_arity}          external_packs.rs:29767,29790
external_packs::external_field_for / external_field_for_key             external_packs.rs:31398,31445
external_packs::external_library_pack_for_type                          external_packs.rs:31742
```

All of them take `receiver_type: Option<&str>`. So the question *"which owner
does this `JavaType` belong to?"* has to be answered before every lookup, and
because the parameter is a bare `Option<&str>` nothing constrains **how**.

The shared *tail* of the answer, `stdlib::canonical_external_receiver_type_key`
(`src/stdlib.rs:4566`), is `pub` and has a test
(`tests/semantic_facts.rs:7447`). The twelve *heads* — the functions that decide
which `JavaType` shapes even reach that tail — are all private, all untested,
and all different.

## Verified inventory

Twelve private functions across nine modules, all `&JavaType -> Option<String>`
or the direct equivalent, all feeding the tables above:

| # | anchor | `String` | `Array(E)` | `Primitive(p)` | unwraps `Optional<T>` | substitutes type-param bound | canonicalised |
|---|---|---|---|---|---|---|---|
| 1 | `aliasing.rs:617` `external_receiver_type_name` | `"String"` | `"E[]"` | – | no | no | **no — raw name** |
| 2 | `call_plan.rs:2074` `external_field_receiver_name` | – | – | – | no | no | yes |
| 3 | `call_plan.rs:2093` `external_receiver_type_key` | `"String"` | `"Array"` | – | **yes** | **yes** | yes + `rust_path_simple_type_name` |
| 4 | `member_access_plan.rs:121` `external_field_receiver_name` | – | – | – | no | no | yes |
| 5 | `method_reference_lowering.rs:317` `java_type_callable_owner_key` | `"String"` | – | – | no | no | yes |
| 6 | `resolve.rs:4546` `external_receiver_type_name` | – | – | – | no | no | yes |
| 7 | `semantic_analysis.rs:2142` `external_field_receiver_name` | – | – | – | no | no | yes |
| 8 | `semantic_analysis.rs:2193` `external_api_receiver_type_key_with_env` | `"String"` | `"Array"` | – | **yes** | **yes** | yes |
| 9 | `semantic_analysis.rs:2206` `external_api_receiver_type_key` | `"String"` | `"Array"` | – | no | no | yes |
| 10 | `signature.rs:11585` `external_api_receiver_type_key` | `"String"` | `"Array"` | – | **yes** | no | yes |
| 11 | `stdlib.rs:4435` `external_receiver_type_key` | `"String"` | `"Array"` | **`format!("{p:?}")`** | no | no | yes |
| 12 | `usage.rs:2835` `external_receiver_type_key` | – | – | – | no | no | yes |

(`–` = returns `None`, i.e. the lookup is skipped or the receiver constraint is
dropped. `semantic_analysis.rs:1024 unresolved_target_receiver_type_key` is a
thirteenth wrapper that adds bound substitution on top of #9.)

Six independent axes of disagreement. Three of them are demonstrable from the
command line today.

## Divergence 1 — the same receiver is two different owners inside one method

`X extends Widget`. One fact file, two facts, both on `Widget`:

```
external-api label    Widget getter  read-only       non-null args=0 return=String
external-api scramble Widget mutator mutates-receiver unknown  args=0
```

```java
public class M<X extends Widget> {
    public int viaBound(X w) {
        w.scramble();
        return w.label().length();
    }
}
```

Actual output:

```rust
pub fn via_bound(&self, w: X) -> i32 {
    w.scramble();
    return w.label().len() as i32;
}
```

Read that twice. On the **same binding `w`**, in the **same method**:

* `w.label()` lowered to `.len() as i32` — so *something* resolved `X` to its
  bound `Widget`, found the `label` fact and learned the return type is
  `String`. That is #3 `call_plan.rs:2093`, whose `if let JavaType::Named { name,
  args } = java_ty && args.is_empty() && let Some(bound) =
  types.type_param_bound(name)` substitutes the bound before keying.
* `w` is **not** `mut` — so the receiver-mutability decision did *not* resolve
  `X` to `Widget` and found no `scramble` fact. That is #10
  `signature.rs:11585`, reached from `external_receiver_binding_constraints`
  (`signature.rs:10638`, `let receiver_type =
  external_api_receiver_type_key(&binding.java_ty);` at `signature.rs:10653`),
  which produces `SignatureConstraint::MutableBinding`. It has no bound
  substitution.

Two controls pin both halves:

```
# unbounded X: the bound-substituting path also fails, and .length() survives
public class N<X> { ... }   ->  return w.label().length();     // not .len()

# declare the fact on the *type parameter name* instead of the bound
external-api scramble X mutator mutates-receiver unknown args=0
                            ->  pub fn via_bound(&self, mut w: X)
```

So the mutability decision is genuinely keyed on the string `"X"` while the
return-type decision is keyed on `"Widget"`.

## Divergence 2 — `Optional<T>`'s own methods are attributed to `T`

#10 `signature.rs:11585` opens with

```rust
if let Some(element_ty) = crate::types::optional_element_type(java_ty) {
    return external_api_receiver_type_key(&element_ty);
}
```

so an `Optional<Widget>` binding is keyed `"Widget"` — *including for method
names that belong to `Optional` itself*. #12 `usage.rs:2835` and #6
`resolve.rs:4546` key the same binding `"Optional"`.

```java
import java.util.Optional;
public class P {
    public void f(Optional<Widget> o) { Widget w = o.get(); use(w); }
    private void use(Widget w) { }
}
```

| fact file | generated signature |
|---|---|
| *(none)* | `pub fn f(&self, o: Option<Widget>)` |
| `external-api get Optional mutator mutates-receiver unknown args=0` | `pub fn f(&self, o: Option<Widget>)` — **fact ignored** |
| `external-api get Widget   mutator mutates-receiver unknown args=0` | `pub fn f(&self, mut o: Option<Widget>)` — **fact applied to the wrong type** |

Exactly backwards. A fact about `Optional.get` cannot be expressed; a fact about
`Widget.get` silently governs every `Optional<Widget>`. The `mut o` is dead
(`o` is only read) — `unused_mut` in the generated crate at best, and a wrong
premise for the aliasing and ownership solvers at worst.

Note also that `types::optional_element_type` (`src/types.rs:392`) matches
`name == "Optional"` **literally**, before any canonicalisation, so
`java.util.Optional<Widget>` is unwrapped by nobody. The unwrap is inconsistent
with itself as well as with the other eleven copies.

## Divergence 3 — `None` means two opposite things

Five of the twelve (#2, #4, #6, #7, #12) return `None` for `JavaType::String`;
the others return `"String"`. Their callers then disagree about what `None`
means at all:

* `usage.rs:2802` — **early return**, the fact is never consulted:
  ```rust
  let Some(receiver_key) = external_receiver_type_key(receiver_type) else { return boundary; };
  ```
  A consequence is visible in the code itself: `usage.rs:2825` tests
  `matches!(receiver_simple, "Math" | ... | "String" | "Objects")`, but since
  `parser.rs:2737` maps the token `String` to `JavaType::String` (never
  `Named { name: "String" }`), that arm is unreachable for every ordinary
  `String` receiver. Only a stray `Unknown("String")` can reach it.
* `resolve.rs:4535` and `aliasing.rs:598` — **pass `None` straight through** to
  the table (`receiver_name.as_deref()`), where
  `external_optional_type_name_matches` (`facts.rs:3423`) turns it into "match
  only receiver-less facts". The receiver constraint is silently *widened*
  rather than the lookup skipped — the opposite reading of the same value.

Meanwhile #10 does key `String`, so

```
external-api scramble String mutator mutates-receiver unknown args=0
public void f(String s) { s.scramble(); }   ->   pub fn f(&self, mut s: String)
```

works through the signature solver while being invisible to the usage solver
that is supposed to corroborate it.

## Divergence 4 — one key is derived by `{:?}` instead of the canonical table (latent)

#11 `stdlib.rs:4435` is the only copy that keys primitives, and it does it with
`Some(format!("{primitive:?}"))` — the `PrimitiveType` *Debug* rendering — while
`canonical_external_receiver_type_key` is a curated table using the **boxed**
spellings. `known_numeric_constant_field_type` (`stdlib.rs:3155`) keys on
`"Byte" | "Short" | "Integer" | "Long" | "Character" | "Float" | "Double"`, so
the Debug-derived key coincidentally hits for `Byte`, `Short`, `Long`, `Float`,
`Double`, `Boolean` and silently misses for `Int` (`"Int"` ≠ `"Integer"`) and
`Char` (`"Char"` ≠ `"Character"`). This is latent — a field access on a
primitive receiver is not valid Java — but it is the purest instance of the
shape: *the same name computed by two rules that were never made to agree.*

## Root cause

`Option<&str>` is the type of the key. It carries no evidence about how it was
produced, so the compiler cannot object when a module writes its own producer,
and there is no place where all producers meet. Each of the twelve was added
next to the consumer that needed it — `external_field_receiver_name` is
literally byte-identical in three files (`call_plan.rs:2074`,
`member_access_plan.rs:121`, `semantic_analysis.rs:2142`), and the two
`Optional`-unwrapping variants and the two bound-substituting variants were each
added once and never propagated to the others. The improvements are strictly
*local*: whoever added bound substitution fixed call lowering and left the
signature solver keyed on `"X"`.

This is exactly the eight-producer pattern that 0002b describes for shared
handles, applied to a name rather than to a wrapper.

## Why it was not noticed

* All twelve are **private**. Nothing in `tests/` can call any of them; the
  greps for these names over `tests/` return only
  `canonical_external_receiver_type_key`, the shared tail. There is no test that
  could compare two of them.
* Divergences only appear on inputs where **two** analyses look at the same
  receiver and only one finds the fact. Both `M.java` and `P.java` above pass
  through the whole pipeline without a diagnostic; `P.java` even compiles.
  This is [0018](done/0018-the-compile-error-metric-cannot-see-silent-failures.md)'s
  blind spot: `mut o` on a never-mutated `Option` and a missing `mut w` are not
  compile errors of the transpiler, and the second one is only a compile error
  of the *generated* crate, which most `slot_shape_design` tests do not check
  ([0041](done/0041-slot-shape-design-had-no-generated-crate-checks.md)).
* The suite pins these paths by **source text** rather than by agreement — e.g.
  `tests/slot_shape_design.rs:28186` asserts
  `source.contains("canonical_external_receiver_type_key(name)")`. That locks a
  single call site in place; it says nothing about the eleven other sites that
  also had to make the same decision.
* Redundancy masks it. `resolve.rs`'s copy failing on `String` receivers is
  invisible whenever `call_plan.rs`'s copy succeeds, so most of the axes only
  surface when a *user-supplied* fact reaches exactly one consumer.

## Merge direction — make disagreement unrepresentable

The fix is not to add bound substitution to eleven more functions. It is to make
the key a value that only one function can construct.

1. **Newtype the key.** `pub struct ExternalOwnerKey(String);` with **no** public
   constructor other than
   ```rust
   impl ExternalOwnerKey {
       pub fn for_receiver(ty: &JavaType, env: &TypeEnv) -> Option<Self>;
   }
   ```
   `TypeEnv` is already threaded to #3 and #8 and is available at every other
   producer's call site (or can be `TypeEnv::new()` where genuinely absent — but
   passing it should be the default, so type-parameter bounds resolve
   *everywhere* rather than in two modules).
2. **Change the tables to take it.** Every signature listed at the top becomes
   `receiver: Option<&ExternalOwnerKey>`. That single change deletes all twelve
   heads: a module cannot invent a key any more, because it cannot build the
   type. The `canonical_external_receiver_type_key` tail moves inside
   `for_receiver` and stops being callable independently.
3. **Give `None` one meaning.** Today it is both "skip the lookup" and "match
   receiver-less facts". Make that explicit in the parameter type — e.g.
   `enum OwnerQuery { Owner(ExternalOwnerKey), AnyOwner }` — so `resolve.rs`'s
   accidental widening cannot be written by omission.
4. **Fix the `Optional` question once.** `for_receiver` must decide whether the
   query is for the container or the element. It cannot decide from the type
   alone — `Optional.get` and `Widget.label` need different answers — so the
   method name has to be part of the query, or the caller has to say which it
   means. Divergence 2 exists precisely because that question was answered
   silently and differently in three places.
5. Then delete `types::optional_element_type`'s literal `name == "Optional"`
   test in favour of the canonical key, so `java.util.Optional` behaves like
   `Optional`.

Step 2 is what makes this structural rather than another one-instance fix: after
it, "the outer and the inner disagree about who owns this method" is a type
error, not a test that nobody wrote.

## Reproduction

`/tmp/j2r`, against `target/release/new-java2rust`:

```
new-java2rust --facts facts7.txt M.java    # divergence 1
new-java2rust --facts facts8.txt P.java    # divergence 2, key = Widget
new-java2rust --facts facts9.txt P.java    # divergence 2, key = Optional, ignored
new-java2rust               P.java         # control
```

Fact files and Java sources are given inline above; `Widget`/`Holder` are
undeclared on purpose so they stay external.

## Adjacent things checked and found clean

* **Java-named-type → Rust-name mangling is unified.**
  `slot_render.rs:1293 rust_name_for_java_named_type` is a one-line delegation
  to `java_naming.rs:23`, and all ~185 references across 17 modules reach that
  one body. This was the second candidate for this audit and is a genuine
  counter-example — [0013](done/0013-nested-type-names-unmangled-in-downcast.md)
  appears to have consolidated it properly.
* **The `slot_render.rs` `*_with_facts` family delegates rather than diverges**
  for the plain chain: `rust_type_for_slot` → `_with_facts` → `_with_effective_facts`
  (`slot_render.rs:117,125,195`) and the same for
  `rust_type_for_java_type` (`slot_render.rs:710,714,773`). **One exception is
  worth a look but is not evidenced here**:
  `rust_type_for_slot_with_facts_and_representations_unwrapped`
  (`slot_render.rs:140`) does *not* route `DispatchShape::Concrete` through
  `slot_value_rust_type_policy_with_effective_facts` the way
  `rust_type_for_slot_with_effective_facts` (`slot_render.rs:195`) does — it
  calls `rust_type_for_java_type_with_facts` directly at `slot_render.rs:171`,
  bypassing the external-crate-mapping and erased-facade arms of
  `render_slot_value_rust_type_policy_with_effective_facts`
  (`slot_render.rs:373`). Whether those two ever disagree in practice was not
  established; it is the obvious next thing to probe in this dimension.
