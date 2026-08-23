# 0047 — `FacadeSlot`'s algebra is nine hand-copied recursions, and the two that matter already disagree

Status: **fixed** (borrow predicates now live on `FacadeSlot`; the call-site/stub
`FacadeSlot` → `RustSlotShape` mapping now has one implementation in
`shape_projection`; Rust-type → slot parsing now has one implementation in
`facts`; explicit facade ABI slots now win over conflicting argument-effect
facts on both stub and call-site paths)

Codex follow-up 2026-08-21: independently validated with a scope caveat. "Nine"
is accurate for the four facade-slot questions enumerated here, not for every
`FacadeSlot` match in the tree.

Codex follow-up 2026-08-21, later pass: fixed the two highest-risk
duplications. `FacadeSlot::contains_shared_borrow` and
`FacadeSlot::contains_mut_borrow` replaced the module-local recursive
predicates in `main`, `call_plan`, `usage`, `signature`, and `stdlib`.
`shape_projection::rust_slot_shape_from_facade_arg_slot` now owns the
`FacadeSlot` → `RustSlotShape` mapping used by both call planning and facade
stub rendering, including optional/slice unwrapping and effective-fact storage
projection. Direct tests cover the optional/slice and owned-dyn-trait arms.

Codex follow-up 2026-08-21, parser pass: the two Rust-type-to-`FacadeSlot`
parsers in `main.rs` and `external_packs.rs` were collapsed into
`facts::facade_slot_for_rust_type`; the structural guard now asserts the old
parser functions are absent.

Codex follow-up 2026-08-22, precedence pass: `call_plan` now gives explicit
`FacadeAbiFact.args` the same precedence that `stdlib` already used for emitted
facade signatures. `facade_abi_argument_plan_prefers_abi_slot_over_argument_effect`
guards the previous split, where call-site coercion could follow
`ExternalApiArgumentEffectFact` while the emitted stub followed the ABI slot.

Severity: medium — latent today, but the divergence is between the **emitted
facade signature** and the **call-site coercion for that same signature**, so
when it fires the output does not compile
Kind: invariant maintained by hand — one closed enum, one meaning per variant,
nine independent hand-written interpretations

Recorded 2026-08-21.

## Symptom

`FacadeSlot` (`src/facts.rs`) is a closed, seven-variant description of how one
Rust facade parameter or return value is passed: `Owned`, `Borrowed`,
`MutBorrowed`, `Slice(_)`, `Optional(_)`, `Index`, `DynTrait(_)`,
`CrateHandle(_)`. It is the single fact that says what a facade's ABI *is*.

Nine functions in six modules interpret it, each a separate `match` over all
variants, each free to answer differently:

| what it decides | copies |
|---|---|
| "does this slot want a mutable borrow?" | `src/main.rs:1504` `facade_slot_contains_mut_borrow`; `src/call_plan.rs:5278` `facade_slot_contains_mut_borrow`; `src/stdlib.rs:15626` `external_facade_abi_slot_contains_mut_borrow`; `src/usage.rs:2468` `facade_slot_requires_mut_borrow`; `src/signature.rs:11164` `facade_slot_requires_mutable_binding` — **five** |
| "does this slot borrow?" | `src/main.rs:1489` `facade_slot_contains_shared_borrow`; `src/call_plan.rs:1325` `external_return_slot_contains_shared_borrow` — **two** |
| Rust type spelling → slot | originally `src/main.rs:1459` `facade_slot_for_rust_type`; `src/external_packs.rs:4114` `facade_slot_for_manifest_rust_type` — **two**; now `facts::facade_slot_for_rust_type` |
| slot → `RustSlotShape` | `src/call_plan.rs:5222` `rust_slot_shape_from_facade_arg_slot`; `src/stdlib.rs:15582` `external_facade_argument_shape_from_abi_slot` — **two** |

The first three groups are currently **identical** — checked line by line, and
reported here as clean. The fourth is not.

## Verified evidence — the two shape mappings disagree

`src/call_plan.rs:5222` decides how to lower the **argument expression at the
call site**. `src/stdlib.rs:15582` decides the **parameter type printed in the
emitted facade stub** that the call site is calling
(`external_facade_args_with_fact_prefix`, `src/stdlib.rs:15347`, which formats
`_arg{index}: {arg_ty}` into `pub fn … { unimplemented!(…) }` at
`src/stdlib.rs:14161`). They read the same `abi.args[index]`. They must agree or
the generated crate does not compile.

Arm by arm:

| `FacadeSlot` arm | `call_plan.rs:5222` | `stdlib.rs:15582` | agree? |
|---|---|---|---|
| `Borrowed` | `conservative` + string/array/other projection (`:5242`) | identical (`:15585`) | yes |
| `MutBorrowed` | `conservative`, `MutableBorrow` + `Mutable` (`:5253`) | identical (`:15598`) | yes |
| `Optional(inner)`, `java_ty` **not** already `Optional` | build `inner` from `Optional<T>`, set `java_ty := Optional<T>` (`:5234`) | build `inner` from `Optional<T>` (`:15615`) | yes |
| `Optional(inner)`, `java_ty` **already** `Optional<E>` | strips: build `inner` from `E`, set `java_ty := Optional<E>` (`:5228`) | wraps again: build `inner` from `Optional<Optional<E>>` — `optional_java_type` (`src/types.rs:595`) is unconditional | **no** |
| `Slice(inner)` | strips an outer `Optional` first: `optional_element_type(&java_ty).unwrap_or(java_ty)` (`:5261`) | does not strip (`:15607`) | **no** when the type is optional |
| `Owned` / `Index` / `DynTrait` / `CrateHandle` | `storage_arg_shape_with_effective_facts` (`:5273` → `:4754`): `for_storage` ⇒ `Mutable`, plus dyn-trait override, object-safe-trait ⇒ `BoxedDynTrait`, erased-object storage | `RustSlotShape::conservative` (`:15619`): `Immutable`, `Concrete`, **no facts consulted** | **no** — differs in `mutability` on *every* owned argument, and in `dispatch`/`view` whenever a representation fact applies |

The `Owned` divergence is the dangerous one. Give an owned facade parameter a
Java type carrying `object-safe-trait T true` or `dyn-trait T TTrait`: the call
site coerces the argument to `Box<dyn TTrait>` while the stub it is calling
declares `_arg0: T`.

### The two consumers also rank their inputs in opposite order

Both sides can learn a parameter's shape from two places — the structured ABI
slot, and an `ExternalApiArgumentEffectFact`. They consult them in opposite
order:

- `stdlib::external_facade_argument_rust_type` (`src/stdlib.rs:15439`): ABI slot
  first (`:15452`), argument effect only as a fallback (`:15462-15463`).
- Originally, `call_plan` argument-slot construction (`src/call_plan.rs`) used
  argument effect first and ABI slot only if there was no effect. The
  2026-08-22 precedence pass removed that override from the ABI path.

The fixed rule is that an explicit `FacadeAbiFact` wins on both paths. If only
argument-effect facts exist, both paths still use those facts as the fallback.

### Both paths are live on the same fact

`OutputStream.write(byte[])` carries a `FacadeAbiFact` with
`args = [Slice(Borrowed)]` and `params = [Exact("byte[]")]`
(`src/stdlib.rs:1791`). Translating

```java
import java.io.OutputStream;
public class W2 {
    public void go(OutputStream o) throws Exception {
        byte[] b = new byte[4];
        o.write(b);
    }
}
```

emits the stub from `stdlib.rs:15582` and the call from `call_plan.rs:5222`:

```rust
pub fn write_1(&self, _arg0: &[i8]) -> Result<(), JavaException> { … }
…
let b: &Vec<i8> = &vec![0; 4 as usize];
o.write_1(b)?;
```

They agree here — `Slice(Borrowed)` over a non-optional `byte[]` is one of the
arms that match. The point is that both copies really are on the live path for
one and the same fact.

## Root cause

`FacadeSlot` is a data description with no operations attached. Every module
that needs to know what a slot *means* re-derives it, because the enum exposes
its variants and offers nothing else. Five modules independently needed "is this
mutable?" and five wrote it.

The two shape mappings additionally sit on opposite sides of a boundary that is
never named: `stdlib.rs` *emits* the facade, `call_plan.rs` *calls* it. That is
a produce/consume pair — the archetypal place for a shared definition — and it
is two forked copies that have already drifted in three of six arms.

## Why it went unnoticed

- Nothing in the tree says the two mappings are a pair. They have different
  names, live in different modules, and neither mentions the other.
- The three agreeing groups create a false sense of safety: `grep` shows nine
  similar-looking functions, seven of which are genuinely identical, so the two
  that are not read as more of the same.
- The divergent arms need combinations today's built-in facts do not produce: an
  already-`Optional` Java parameter type on an `Optional`/`Slice` slot, or a
  representation fact on an owned facade parameter. Built-in ABIs are declared
  over primitives, `byte[]`, `String` and `Object`; the general case arrives with
  user pack manifests.
- The mismatch would surface as a `rustc` type error in generated code, which
  [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md) records is
  only measured on a subset of outputs.

## Live vs latent

**Latent, armed.** Both consumers are live and read the same fact; the arms that
differ are unreachable together in the built-in fact set. The reachability
depends entirely on which facades exist, which is user-supplied input — so this
is a property of the current corpus, not of the design.

## Redesign — one interpretation per question, owned by the enum

1. **Move the predicates onto `FacadeSlot`.** `impl FacadeSlot { fn
   requires_mut_borrow(&self) -> bool; fn borrows(&self) -> bool; }` in
   `src/facts.rs`. Seven of the nine functions become calls. A new variant then
   breaks one `match` instead of nine, and the compiler names it.

2. **One `FacadeSlot` → `RustSlotShape` function, in one module, taking
   `&EffectiveFacts`.** The stub emitter and the call planner must ask the same
   function; that is the only way "the signature and the call agree" stops being
   a thing anyone has to remember. Where they legitimately need different
   *views* of one shape — a printed type versus a coercion target — those are
   two renderings of one shape, not two shapes.

3. **Decide the ABI-slot / argument-effect precedence once**, in that shared
   function, as an explicit merge (the codebase already has the vocabulary:
   `join_ranked_lattice`, `SignatureRequirementMerge`, `src/signature.rs:7860`).
   Precedence encoded as `if let Some(x) = a { … } else { b }` in two places is
   a rule stored twice; a merge is a rule stored once.

4. **Done 2026-08-21: collapse `facade_slot_for_rust_type`
   (`src/main.rs`) and `facade_slot_for_manifest_rust_type`
   (`src/external_packs.rs`)** into `facts::facade_slot_for_rust_type`. They
   were identical, which is exactly when merging is free. They are the *entry
   point* for the enum — if the two spellings of a Rust type ever produce
   different slots, every consumer inherits the disagreement.

The structural test that would then be worth writing is the one this design
makes possible: for every `FacadeAbiFact` in the built-in and manifest fact
sets, the parameter type printed into the stub and the target shape used at the
call site are derived from the same call. Today there is no such call to test.
