# 0046 — `RustSlotShape.type_identity` is a stored derivation of `java_ty`, refreshed by hand at 7 of 20 mutation sites

Status: **fixed** — `RustSlotShape.type_identity` is no longer stored. It is
derived by `RustSlotShape::type_identity()` from `java_ty`, and
`has_more_generic_detail_than` computes erasure directly from `java_ty`.

Codex follow-up 2026-08-21: independently validated against the current tree,
then deleted the stored field. The old class of stale-cache bugs is now
unrepresentable: the seven refresh writes and thirteen missed refresh sites are
gone as a category, and `PartialEq` no longer compares derived metadata.

Severity: medium — it cannot produce wrong Rust on its own, but it can silently
suppress a shape refinement, and it is a live trap for the next reader of the
field
Kind: invariant maintained by hand — a field that is a pure function of a
sibling field, stored, and re-derived at some assignment sites but not others

Recorded 2026-08-21.

## Symptom

`RustSlotShape` (`src/slot_shape.rs:5`) carries six fields. Five are decisions.
The sixth, `type_identity`, is not a decision — it is a **pure function of
`java_ty`**, computed by `TypeIdentity::from_java_type`
(`src/slot_shape.rs:118`):

| `TypeIdentity` field | derivation from `java_ty` |
|---|---|
| `declared_java_ty` | literally `java_ty` |
| `generic_args` | `args` of a `Named`, else empty |
| `runtime_bounds` | the bound of a bounded `Wildcard`, else empty |
| `erasure` | `java_type_erasure(&java_ty)` (`src/slot_shape.rs:138`) |

Shapes are constructed once and then **edited in place** — `let mut s =
other.clone(); s.java_ty = …` is the standard idiom in lowering. Nothing makes
the derived field follow. So a shape can claim to be a `Foo` while its
`type_identity` still describes the `Optional<Foo>` it was cloned from.

Concretely, `optional_element_slot_shape_with_facts`
(`src/conversion_plan.rs:1696`) is handed the shape of an `Optional<Foo>` slot
and returns the shape of its element:

```rust
let mut element_shape = option_shape.clone();          // :1701
…
} else {
    element_shape.java_ty = element_ty;                // :1729   <- no refresh
}
```

For `Optional<Foo>` the result is

```
java_ty                        = Foo
type_identity.declared_java_ty = Optional<Foo>     // wrong
type_identity.generic_args     = [Foo]             // wrong
type_identity.erasure          = Some(Optional)    // wrong
```

and therefore `element_shape.erased_java_type()` (`src/slot_shape.rs:208`)
answers **`Optional`** for a shape whose type is `Foo`.

## Verified evidence

### 1. Every write of `type_identity` is the same derivation

Non-test writes, exhaustive (`grep -rn "type_identity" src/`):

| site | value written |
|---|---|
| `src/slot_shape.rs:74` (`RustSlotShape::conservative`) | `from_java_type(java_ty)` |
| `src/slot_shape.rs:85` (`RustSlotShape::for_storage`) | `from_java_type(java_ty)` |
| `src/slot_render.rs:149` | `from_java_type(element_shape.java_ty)` |
| `src/slot_render.rs:202` | `from_java_type(element_shape.java_ty)` |
| `src/signature.rs:7870` | `from_java_type(shape.java_ty)` |
| `src/lower.rs:10220` | `from_java_type(declared_ty)`, and `java_ty` is set to the same value on the line above |
| `src/conversion_plan.rs:1707` | `projected.type_identity`, where `java_ty` is set to `projected.java_ty` on `:1704` |
| `src/conversion_plan.rs:1717` | `from_java_type(element_shape.java_ty)` |
| `src/conversion_plan.rs:1727` | `from_java_type(element_shape.java_ty)` |

(`src/operator_lowering.rs:1200` and `:1498` also write it; both are inside
`#[cfg(test)]` modules and also write `from_java_type(java_ty)`.)

**There is not one site where `type_identity` is intentionally something other
than `TypeIdentity::from_java_type(self.java_ty)`.** The field is a cache with
no independent meaning.

### 2. 13 of 20 `java_ty` mutations leave the cache stale

All 20 `.java_ty = …` assignments in `src/`, with whether `type_identity` is
refreshed alongside:

| site | refreshed? | what goes stale |
|---|---|---|
| `src/slot_render.rs:148` | yes | — |
| `src/slot_render.rs:201` | yes | — |
| `src/signature.rs:7869` | yes | — |
| `src/lower.rs:10219` | yes | — |
| `src/conversion_plan.rs:1704` | yes | — |
| `src/conversion_plan.rs:1713` | yes | — |
| `src/conversion_plan.rs:1721` | yes | — |
| `src/conversion_plan.rs:1729` | **no** | element of an `Optional<E>` keeps `erasure = Some(Optional)` |
| `src/conversion_plan.rs:672` | **no** | same shape, built by `optional_slot_construction_plan_with_facts` |
| `src/stream.rs:588` | **no** | wraps to `Optional<T>` but keeps `T`'s identity ⇒ `erasure = None` |
| `src/stream.rs:666` | **no** | unwraps `Optional<E>` to `E`, keeps `Optional`'s identity |
| `src/method_reference_shape.rs:253` | **no** | `java_ty := Unknown(method)`, identity of the input shape |
| `src/shape_projection.rs:405` | **no** | unwraps `Optional<E>`, keeps `Optional`'s identity |
| `src/call_plan.rs:5231` | **no** | inner shape built from `E`, then `java_ty := Optional<E>` |
| `src/call_plan.rs:5239` | **no** | stale whenever `inner` is `Slice` or a nested `Optional` (both strip/rewrap the type before `conservative` sees it) |
| `src/lower.rs:23944` | **no** | element of an `Optional` target |
| `src/lower.rs:26576` | **no** | `Unknown(_)` shape retyped to a concrete `element_ty` ⇒ keeps `erasure = None` |
| `src/lower.rs:26601` | **no** | element of an `Optional` actual |
| `src/lower.rs:32293` | **no** | element of an `Optional` target |
| `src/lower.rs:32556` | **no** | element of an `Optional` actual |

**7 refresh, 13 do not.** The brief's suspicion is confirmed, and the count is
exactly 13.

The decisive one is `optional_element_slot_shape_with_facts`
(`src/conversion_plan.rs:1696`): it has **four** branches that assign `java_ty`,
refreshes in three (`:1707`, `:1717`, `:1727`) and forgets in the fourth
(`:1729`) — inside one 40-line function. When the discipline fails within a
single function body, it is not a discipline.

### 3. The field is read, and the codebase already does not trust it

Non-test readers of `type_identity`, exhaustive:

- `RustSlotShape::erased_java_type` (`src/slot_shape.rs:208`) —
  `self.type_identity.erasure.as_ref().unwrap_or(&self.java_ty)`. Its only
  caller is `has_more_generic_detail_than` (`:190`), whose first act is
  `if self.erased_java_type() != other.erased_java_type() { return false; }`.
  Five decision sites depend on it: `src/lower.rs:10233`, `:10322`, `:10426`,
  `:10449`, `:32061`.
- the **derived `PartialEq`** on `RustSlotShape` (`src/slot_shape.rs:4`), used
  as whole-struct equality at `src/lower.rs:23855`
  (`actual_shape == target_shape`, inside `reused_owned_argument_needs_clone` —
  a decision about whether to emit `.clone()`).

`declared_java_ty`, `generic_args` and `runtime_bounds` have **no reader in
`src/` at all** — only `tests/slot_shape_design.rs:686–714`. They are written,
carried through every clone, allowed to go stale, and never consulted.

That the derived field poisons `PartialEq` is already known in practice, though
not stated anywhere. Three comparison helpers hand-list the fields to compare so
that `type_identity` is excluded:

- `optional_element_slot_shape_matches` (`src/conversion_plan.rs:1992`) —
  compares `java_ty`, `ownership`, `view`, `dispatch` and stops;
- `optional_element_borrow_target_projection` (`src/conversion_plan.rs:1987`) —
  same three axes;
- `tests/type_coercion.rs:3369-3373` asserts on
  `construction.element_shape.java_ty` and `.ownership` field by field, while
  the neighbouring `assert_eq!(construction.element_shape, actual_owned)` at
  `:3378` can use whole-struct equality only because that shape is passed
  through unmodified.

So the codebase maintains, by hand, an exclusion list for a field that a derive
silently includes.

### 4. The same relation is defined three more times, from `java_ty`

`erased_java_type()` reads the cache. Four call sites of
`has_more_generic_detail_than` *also* guard with a relation recomputed from
`java_ty` on the spot:

- `java_types_have_same_erased_identity` (`src/lower.rs:10325`), used at
  `:10321`, `:10425`, `:10448` and (via
  `lowered_type_slot_has_more_generic_information`) `:32060`;
- `java_types_have_same_erased_cast_identity` (`src/conversion_plan.rs:3277`) —
  **byte-identical** to the above, different name, different module;
- `java_type_erasure` (`src/slot_shape.rs:138`) — a third, structurally
  different, formulation feeding the cache.

`src/lower.rs:10321-10322` is the clearest picture of the problem:

```rust
java_types_have_same_erased_identity(&initializer.java_ty, &local.java_ty)
    && initializer.has_more_generic_detail_than(local)
//                 ^ re-asks "same erased type?" from the cached field
```

Two notions of "same erased type" in one boolean. With fresh caches they agree
and the second is redundant. With a stale cache they disagree, and the only
possible effect of the disagreement is to turn a `true` into a `false`.

## Root cause

`RustSlotShape` (`src/slot_shape.rs:5`) stores a projection of one of its own
fields. `java_ty` is `pub`, so every module can move the shape into a state its
constructors could never produce. The two constructors that *do* establish the
invariant — `RustSlotShape::conservative` (`:68`) and `::for_storage` (`:79`) —
have no monopoly on it: `clone()` plus a field write bypasses them, and that is
the dominant idiom in `lower.rs`, `conversion_plan.rs`, `stream.rs` and
`call_plan.rs`.

`TypeIdentity` reads like a record of *declared* type information that might
legitimately outlive a rewritten `java_ty` — the field name `declared_java_ty`
invites that reading. Evidence item 1 shows it is not: every writer sets it from
the current `java_ty`. The type promises a distinction it never makes.

## Why it has not bitten harder

Three accidents, all of which are one commit away from ending:

1. **Three of the four `TypeIdentity` fields have no reader.** Whatever they
   say, stale or not, nothing acts on it. The first consumer of
   `declared_java_ty` or `generic_args` inherits 13 latent bugs on day one.
2. **The one live reader fails closed.** A stale `erasure` makes
   `has_more_generic_detail_than` return `false`, so lowering keeps the
   *less* detailed shape. The result is a raw `Vec<…>` where a `Vec<String>`
   was available — under-refinement, not miscompilation. It leaves no
   `rustc` error and no diagnostic, so [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)'s
   metric cannot see it.
3. **The one whole-struct `==` is heavily pre-filtered.** `src/lower.rs:23855`
   is reached only when the target is an owned `Optional`, and the other five
   axes must already match, so the window in which the stale sixth field is the
   deciding vote is narrow. It is not closed: `src/call_plan.rs:5231` produces
   exactly an owned-`Optional` argument-target shape with a stale identity, and
   argument-target shapes are what that comparison receives.

None of the three is a property anyone chose. They are why a defect of this
shape stays invisible until it suddenly is not.

## Redesign — make staleness unrepresentable

**Delete the field.** It has one non-test reader; the migration is a few lines.

```rust
pub struct RustSlotShape {
    pub java_ty: JavaType,
    pub ownership: OwnershipShape,
    pub view: ViewShape,
    pub dispatch: DispatchShape,
    pub mutability: MutabilityShape,
}

impl RustSlotShape {
    pub fn type_identity(&self) -> TypeIdentity {
        TypeIdentity::from_java_type(self.java_ty.clone())
    }
    pub fn erased_java_type(&self) -> Cow<'_, JavaType> { … }
}
```

What this buys, in the order the maintainer's philosophy asks for:

1. **The 13 sites stop existing as a category.** There is no cache, so there is
   nothing to forget to refresh. `s.java_ty = t` is total again.
2. **`PartialEq` becomes the right relation for free.** With the derived field
   gone, `#[derive(PartialEq)]` is exactly the five-axis comparison that
   `optional_element_slot_shape_matches` (`src/conversion_plan.rs:1992`) and
   `optional_element_borrow_target_projection` (`:1987`) spell out by hand.
   Both collapse to `==`, and the hand-maintained exclusion list — itself a
   thing that can drift when a seventh field is added — disappears with them.
3. **One definition of erasure.** `erased_java_type()` becomes
   `java_type_erasure(&self.java_ty)`, so `has_more_generic_detail_than`'s guard
   and `java_types_have_same_erased_identity` are the same predicate rather than
   two that happen to agree. Fold `java_types_have_same_erased_cast_identity`
   (`src/conversion_plan.rs:3277`, a byte-identical copy) into it and delete
   both local definitions in favour of one in `src/types.rs`.
   Note the two are *not* currently equivalent for arrays:
   `java_types_have_same_erased_identity(String[]<…>, …)` falls through to full
   equality while `java_type_erasure` descends into `Array`. Unifying them
   settles that question once instead of leaving two answers in the tree.

**If a cache is wanted for cost** (it is not — `from_java_type` is three matches
and a clone, on a struct that is cloned wholesale everywhere anyway), the
representation that still forbids the bad state is: make `java_ty` private and
give `RustSlotShape` a `set_java_ty(&mut self, ty: JavaType)` that recomputes.
Then the 13 sites become compile errors today rather than a table in an audit.

**Do not** fix this by refreshing the 13 sites. That leaves the 14th assignment
— the one written next month — exactly as free to forget, and leaves
`PartialEq` still comparing a field nobody means to compare.

## Cheapest thing that would have caught it

A law test in the spirit of `mirrored().mirrored() == identity`:

```rust
// for any shape produced by any planner
assert_eq!(shape.type_identity,
           TypeIdentity::from_java_type(shape.java_ty.clone()));
```

asserted over the shapes reachable from a corpus translation. Under the redesign
the assertion is a tautology and the test is unnecessary — which is the point.
