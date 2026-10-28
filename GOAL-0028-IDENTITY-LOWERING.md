# Goal: implement 0028 identity lowering for shared handles

Recorded 2026-08-07. Successor to `NEXT-ISSUES-HANDOFF.md`, which listed this as
priority 2 while it was still blocked. It is no longer blocked: `0002b` step 3
landed, so source objects are behind handles and identity is now expressible.

## 1. Prove the base first (60–90 min)

Run `scripts/gate.sh --all` **before touching code**. Expected:

| suite | expected |
|---|---|
| `idiom_gaps` | 538 / 146 |
| `slot_shape_design` | 516 / 80 |
| `differential` | 1 / 0 |
| `--lib` | 178 / 0 |

Verified clean at `aa6da29` with no uncommitted changes. Anything else is a
dirty-base problem, not part of 0028 — stop and reconcile.

## 2. The actual starting point

`tests/differential/known-gaps/Identity.java` translates to this **today**:

```rust
let a: std::rc::Rc<IdentityKey> = std::rc::Rc::new(IdentityKey::new(5));
let b: std::rc::Rc<IdentityKey> = a.clone();
let c: std::rc::Rc<IdentityKey> = std::rc::Rc::new(IdentityKey::new(5));
return format!("{},{},{},{}",
    (b.get()).to_string(), (a == b).to_string(), (a == c).to_string(), (a == c).to_string());
```

Two things to read off it:

- **The handles are there.** 0002b did its half; `a` and `b` really are one
  object. Nothing about storage needs changing.
- **`==` on `Rc<T>` is structural.** `Rc`'s `PartialEq` delegates to `T`, so
  `a == c` is `true` where Java says `false`.
- **`.equals()` already collapsed to `==`** — the third and fourth format
  arguments are the *same expression*. Both halves of 0028 are therefore one
  fix, not two.

Oracle `5,true,false,false`; current `5,true,true,true`.

## 3. Where to hook it

**Not `src/lower.rs`.** The equality decision lives in
`src/operator_lowering.rs`:

- `plan_equality_comparison_with_facts(left, right, facts)` — already takes a
  `FactSet`, so the placement is reachable without new plumbing.
- `EqualityComparisonPlan` — add the identity arm here, not at the render site.
- `lower_equality_comparison_expr_for_op` renders the plan.

Three existing pieces to build on rather than around:

- **`custom_equals_method(java_ty, facts)`** (same file) already answers "does
  this type override `equals`", via `facts.method_effect_for(name, "equals")`.
  That is the guardrail-4 answer for *"explicit custom `.equals()` stays a normal
  method call"* — do not re-derive it from names.
- **`facts.storage_placement_for(name)`** with `.shared && .is_implemented()` is
  the predicate for "this is behind a handle". Same one the wrap uses, so the two
  cannot drift.
- **`placement.handle_path()`** returns `std::rc::Rc` or `std::sync::Arc`.
  **Do not hardcode `Rc`** — a type reachable from a Java `static` is `Arc`, and
  `Arc::ptr_eq` is the matching call. `StoragePlacement` already decides.

### Preserve the symmetry invariant

`plan_equality_comparison_with_facts` canonicalises operand order
(`equality_operand_rank`) and mirrors the answer (`mirrored()`), specifically so
an asymmetric arm has nowhere to be written. `equality_symmetry_tests` asserts
the involution. A new identity arm must go through the same path — if you find
yourself adding a `Left*`/`Right*` pair, that is the signal you have bypassed it.

`Rc::ptr_eq` takes `&Rc<T>`, so the arm needs to know each operand's ownership;
the plan already carries both `RustSlotShape`s.

## 4. Acceptance

`tests/differential/known-gaps/Identity.java` promoted out of `known-gaps/` once
it matches the oracle. It already covers all four required cases — aliases
compare `true`, distinct equal-valued objects compare `false`, inherited
`Object.equals` follows identity, and its doc comment explains why the fixture
cannot validate `SharedImmutable` on its own.

Add a sibling fixture for the case `Identity.java` deliberately does not have:
a class that **does** override `equals`, where the call must stay a method call
and structural semantics must survive.

Keep structural equality for primitives, `String`, enums, and non-shared value
classes. Enums matter: they were 10 of 32 failures in an earlier 0002b pass
precisely because identity/equality assumptions broke `match` and derived
equality.

## 5. Scope

**Do not start `SharedLocal` / `SharedSync`.** The `RefCell`/`Mutex` rungs carry
the never-hold-a-borrow-across-a-call discipline and are a separate design. If
identity lowering surfaces a mutable shared case, write it into
`claude_audits/0028-...md` and leave it for the mutable-tier pass.

## 6. Fallback if 0028 stalls

Lowering-time metric coverage for no-op facade **method** call sites under 0018 —
constructor call sites are counted, methods are not, so `noop_facade_call_sites`
is a lower bound. Emit a marker when call lowering resolves a target to a no-op
facade method and count that structured signal rather than scanning rendered
Rust. Start at `src/output_markers.rs`, `src/stdlib.rs`,
`tests/output_markers.rs`.

## 7. Guardrails

- Focused test after each small change, then `scripts/gate.sh --all`.
- For `idiom_gaps`, **compare failure names, not counts** — diff against
  `tests/baselines/idiom-gaps-failures.txt`.
- No string-based equality special cases where a shape or type predicate exists.
- Do not update baselines unless the only movement is newly added passing tests
  or an intentionally promoted known-gap.

### One lesson worth carrying over

0002b step 3 was implemented and reverted **eight times** before landing. What
broke the cycle was not a better idea; it was
[0041](claude_audits/done/0041-slot-shape-design-had-no-generated-crate-checks.md)
— adding generated-crate compilation checks so failures named a *type error*
instead of an assertion drift. Those checks now exist (99 in
`slot_shape_design`, 296 in `idiom_gaps`). Use them: if a change looks like an
expectation drift, confirm the generated crate still compiles before editing the
expectation.

## 8. Known-stale thing to fix in passing

`report_storage_disagreements` still says a shared type "**is lowered by
value**, so an alias becomes a copy". That was true before step 3 and is not
now — `Identity.Key` is behind an `Rc`. The message should distinguish "shared
and now handled" from "shared but still by value" (the `!is_implemented()`
tiers). Small, and it currently misreports the thing 0028 is built on.
