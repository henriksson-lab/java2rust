# Use-site borrow analysis — scoping plan (2026-06-21)

The single root behind two open threads: the **clone-reduction audit** (TODO §4.1,
the active lever) and **in-place idiomatic IO** (`docs/in-place-io-prototype.md`,
parked behind exactly this). Grounded in `SEMANTICS.md §6`.

## 1. The model (SEMANTICS §6)

The translator inserts an owning `.clone()` at every "move position" (return /
assignment-RHS / var-init / by-value non-`Copy` arg — `emit_moved_value`,
`dump.rs:2179`). But **most of those positions only READ the value**, and a read
needs a *borrow*, not ownership. The correct rule:

> **Own only at a genuine move** (store into an owned slot, return owned, pass to a
> by-value param). **Borrow at every read** (format/println args, comparison
> operands, `if`/`while` conditions, read-only method receivers, index bases).

A value used N times, all reads, needs **zero** owned copies. The eager-clone
strategy over-owns; under-borrowing now also *forces* later clones. Fixing the
borrow aggressiveness is the root; the marked clones (~10.9k, see TODO §1) and the
§7.2 enum borrow seams are symptoms of the same "wrong borrow shape at a use."

## 2. Current state — what's landed

`use_is_read_borrow(e)` (`dump.rs:2297`) is the central **use-site classifier**.
Today it returns true for exactly ONE case: `is_readonly_method_receiver` (a
conservative whitelist `is_readonly_java_method`, `dump.rs:8159`). It's consulted at
three emit sites — the NAME read (`~3212`), the non-Copy name read (`~3331/3342`),
and `~5089` — each choosing `.as_ref().unwrap()` (borrow, `&T`, zero clones) over
`.clone().unwrap()`. Landed slices (errors −12, clones −629, zero regression):
- (1) read-only-method receiver at the NAME site
- (a) the same at `this.field` / inherited-field sites
- (e) the LazyLock-const receiver + logging-method whitelist

Ordering at a read: **last-use move > as_ref borrow > clone** (`is_movable_last_use`
`dump.rs:2199` already moves an owned local at its final read).

## 3. The frontier insight (why this is hard, from the slice-(b) NO-GO)

Slice (b) — index-base `x.clone().unwrap()[i]` → `.as_ref().unwrap()[i]` — gives a
**huge** clone win (−510, incl. jhlabs −237, jts −141) but **regresses errors**
(jhlabs +3, jts +4). Root cause: for a *non-Copy element struct* read in a
*numeric-coercion context* (`pts[i].x - 1000` on `Vec<Point>`), borrowing the base
through `&Vec` reshuffles the translator's (already-buggy) f32/f64 coercions into
new errors. Only ~7 of ~500 field-reads cascade, and **they can't be separated from
the good ones by a local predicate** — the discriminator is *element-Copy-ness ×
numeric-context*, which is **type information not available at the use site today**.

**Conclusion:** a purely syntactic `use_is_read_borrow` has hit its ceiling. The
next tier needs the **resolver's `Ty` threaded to the use site** (element type,
Copy-ness, coercion context). This is the central design decision of this work.

## 4. Architecture — a typed use-site classifier

Replace the boolean `use_is_read_borrow` with a richer verdict that has the type
context it needs:

```
enum UseKind { Move, ReadBorrow, MutBorrow }
fn classify_use(&self, e: NodeId) -> UseKind   // e = a value-read node
```

`classify_use` walks `parent(e)` (as the existing helpers do) and consults
`self.ty(e)` / element types where the borrow safety depends on Copy-ness or
coercion context. Verdict table by parent context:

| Parent context | Verdict | Notes |
|---|---|---|
| format/println/write arg | ReadBorrow | macro auto-refs; clone always spurious (§6) |
| comparison operand, `if`/`while`/`?:` cond | ReadBorrow | slice (c); watch `&T == T` slice-compares |
| read-only method receiver (whitelist) | ReadBorrow | **landed** |
| index base `x[i]` | ReadBorrow **iff** element is Copy/scalar | slice (b) — gated on element type (the missing piece) |
| assignment-target index `x[i] = …` | MutBorrow (`.as_mut()`) | a **real correctness bug** today: mutation lost to a discarded clone |
| `&`-by-ref argument (callee param `by_ref`) | ReadBorrow, suppress the leading `&` | slice (d); `&` + `as_ref().unwrap()` = `&&T` — coordinate in `print_one_default_argument` |
| foreach iterable | ReadBorrow only for last-use/owned-temporary | slice (f); general `for v in &it` ripples into the body — do NOT |
| `Map.get(k).cloned().unwrap()` read-context | ReadBorrow → `.get(&k).unwrap()`; `.copied()` if Copy | slice (g) |
| store into owned slot / return owned / by-value param | Move | genuine move — keep the clone/move |

Each verdict maps to an emission: ReadBorrow → `.as_ref().unwrap()` (or drop a
spurious `&`/clone); MutBorrow → `.as_mut().unwrap()`; Move → today's behavior.

## 5. Incremental slices (each its own measured KEEP; ordered confidence × leverage)

Discipline (TODO §2): build, re-translate, measure clones over `/tmp/audit-<c>`
ONLY (never a build dir — §1 GOTCHA) **and** all-12 errors; KEEP only if clones down
**and** zero per-corpus error regression. One measurement job at a time.

1. **(c) comparison/condition operands** (~200) — ❌ **first attempt NO-GO (measured
   2026-06-21): vcf +13, jts +15** (bjalign −1). A purely local `use_is_read_borrow`
   extension (borrow any `==`/`!=` operand) breaks: borrowing ONE operand emits
   `&T == <owned T>`, which doesn't compile unless the OTHER operand also borrows.
   The predicate is per-node and can't coordinate both sides. **What it actually
   needs:** `visit_binary` must emit a *consistent* borrow shape for both `==`/`!=`
   operands (emit `&a == &b`, or deref the borrow to a value), driven by the typed
   `classify_use`. (Condition `if`/`while` positions were `bool`/Copy no-ops.) So
   (c) is NOT type-info-free as first assumed — it's the first concrete case that
   *requires* the operand-coordination this plan's §4 classifier owns. Reverted;
   NO-GO comment at `use_is_read_borrow`. **Re-do (c) together with the binary-op
   emission change, not before it.**
   → ✅ **DONE the right way (2026-06-21): −116 clone markers, errors flat (zero
   regression — vcf & jts both back to baseline).** Added a `cmp_borrow` flag +
   `is_borrowable_nullable_read` + `emit_cmp_operand_borrowed` (`dump.rs`): in
   `visit_binary`, when an `==`/`!=` operand is a borrowable nullable read, BOTH
   sides emit as `&T` — the nullable side via `.as_ref().unwrap()` (forced by
   `cmp_borrow`, priority over the last-use move), the other wrapped in `&(..)` —
   so it's `&T == &T` (same `T: PartialEq` requirement as today). Fires only when a
   nullable non-Copy NameExpr is an `==`/`!=` operand; non-nullable comparisons
   unchanged. This is the **binary-op operand-coordination** the §4 classifier needs,
   and the template for the deferred `&V` halves of (g)/below.
2. **(g) `Map.get` read-context** — ✅ **Copy-value half DONE (2026-06-21):
   −232 clone markers, errors flat (zero regression).** `Map.get(k)` for a Copy
   (`Type::Prim`) value type now emits `.get(&k).copied().unwrap()` instead of
   `.cloned()…` — a free copy, not a marked clone (`dump.rs` `("get",1)` is_map arm).
   Bigger than the ~76 estimate. Gates green. **The OTHER half — `.get(&k).unwrap()`
   (`&V`) for a NON-Copy value in a read-context — ✅ also DONE (2026-06-21): −16
   clone markers + −1 error (vcf), zero regression.** A non-Copy `Map.get(k)` whose
   get-call is a read-only method receiver (`map.get(k).equals(..)`/`.length()`/… —
   `use_is_read_borrow` on `parent(recv)`) emits `.get(&k).unwrap()` (`&V`, the use
   autorefs); a move position (stored/returned/passed) keeps the clone. Smaller than
   the Copy half (most get-results are move positions). The `==`-operand case is
   already covered by the slice-(c) coordination (which wraps the get-call in `&(..)`).
   The remaining `Map.get` clones are genuine move positions (stored/returned/passed).
3. **(f) foreach last-use/owned-temporary subset** — ✅ **owned-temporary half DONE
   (2026-06-21): −103 clone markers, errors flat (zero regression).** A foreach over a
   fresh `new`/method-call result (`MethodCallExpr`/`ObjectCreationExpr` iterable) drops
   the `.clone()` — the loop consumes the temporary directly (`dump.rs` `ForeachStmt`
   arm). A name/field/param iterable keeps the clone (moving out of a binding or
   `&self`/`&` borrow is illegal). Gates green. **The last-use-LOCAL half — ✅ also DONE
   (2026-06-21): −48 clone markers, errors flat (zero regression).** Added
   `foreach_iterable_movable_local` (mirrors `is_movable_last_use` but tests the
   *foreach* for an enclosing loop, since the iterable's own parent IS the foreach): a
   local read exactly once (this foreach), whose foreach isn't nested in an outer loop,
   is MOVED into the loop (`for s in xs`) instead of cloned. Multi-read or outer-loop
   cases keep the clone. The general `for v in &it` borrow form stays OUT (rebinds `v` to `&T` →
   ripples into the body).
4. **(d) `&`-borrow argument** — ✅ **DONE (2026-06-22): errors −5 (vcf −3, jsoup −1,
   jts −1), zero regression + clone reduction.** A nullable non-Copy name passed where a
   `&T` is expected was `&x.clone().unwrap()` (borrow of a CLONED temporary). Added an
   `arg_borrow` flag (mirrors `cmp_borrow`): `print_one_default_argument` detects a
   `is_borrowable_nullable_read` arg, suppresses the leading `&`, and sets `arg_borrow` so
   the nullable name path emits `x.as_ref().unwrap()` — which IS the `&T` (no `&&T`, no
   clone). `arg_borrow` takes priority over the last-use move (the borrow shape the caller
   arranged for; also keeps `x` alive vs `&x.unwrap()` consuming it). Errors went DOWN, not
   just flat — some `&x.clone().unwrap()` sites had a Clone-bound/borrow failure the
   `as_ref` form avoids. Applied to BOTH the unlinked default-arg path
   (`print_one_default_argument`, −5) AND the linked `print_arguments_linked` `by_ref`
   plain-`&T` arm (non-mut/non-nullable/non-enum, −16: trim −2, jahmm −1, bjalign −1, jts
   −12). **Combined: errors 10807 → 10786 (−21), zero regression**; jts clones 4409→4090
   (−319 cumulative with the as_mut slice). Gates: golden 42/42, compilecheck 110/110, 145
   tests, 0 warnings.
5. **(b) index-base, REVIVED with element type** — ✅ **DONE (2026-06-21): −193 clone
   markers, errors flat (zero regression).** Added `is_copy_index_base` (wired into
   `use_is_read_borrow`): the base of a `arr[i]` **READ** whose element is a Copy scalar
   (`Type::Prim` — `int[]`/`float[]`/`char[]`) borrows (`arr.as_ref().unwrap()[i]`,
   copies the Copy element out of `&Vec`); NON-Copy elements keep the clone (the prior
   unconditional attempt's jhlabs +3 / jts +4 came from non-Copy structs in
   numeric-coercion contexts — now excluded). Write targets (`arr[i] = x`, `arr[i]++`)
   excluded — they need `&mut`. **Biggest single slice** (Copy pixel/coordinate arrays).
6. **(MutBorrow) write-target `.as_mut()`** — ✅ **DONE (2026-06-22): a CORRECTNESS fix,
   ZERO net errors, zero regression, 91+ silent lost-mutation bugs fixed** (jts 36, jhlabs
   53, jahmm 2; more across all 12). A nullable array WRITE target (`arr[i] = …`,
   `arr[i] += …`, `arr[i]++`) emitted `arr.clone().unwrap()[i] = …` — the store hit a
   DISCARDED clone, so the mutation was silently lost (e.g. `holes[i] = ring` left `holes`
   all-default). New `is_mut_borrow_index_base` (`dump.rs`) → the nullable name path emits
   `.as_mut().unwrap()` (MutBorrow), placed BEFORE the last-use move (`.unwrap()` would move
   the `Vec` out and write to a temporary, also losing it). `.as_mut()` needs a mutable
   binding, which surfaced 4 E0596s; cleared by: (a) locals already get `let mut` via the
   change-tracker (the array-access base is recorded under `in_assign_target`); (b) **nullable
   by-value `Option<…>` params** mutated through an element write now get a `mut` binding
   (`visit_parameter`: `nullable && mut_borrow_params.contains` — the `&mut`-ref path only
   covered element-nullable arrays). NB caller-visible mutation still isn't preserved (Java
   arrays are by-ref → a separate pre-existing `&mut`-param gap); this fixes the IN-function
   store, which is the lost-mutation bug. Gates: golden 42/42, compilecheck 110/110, 145
   tests, 0 warnings.

**Genuinely-not-avoidable (don't chase, TODO §4.1):** `Vec`-index `[i].clone()`
stored owned; `Validate::not_null(Some(x.clone()))` by-value sig; R4 cast-extract
`match &x {…=> v.clone()}`; `.iter().cloned()` into `JavaIter`; copy-ctor
`self.x = param.clone()`. **Borrowed-returns is CLOSED** (SEMANTICS §6 / §3 — clones
went UP, jts +316; do not restart).

## 6. Payoff for in-place IO

In-place IO (`docs/in-place-io-prototype.md`) is blocked because a `Box<dyn BufRead>`
reader local is **not Clone** and reads mutate the cursor, so the translator must
emit `reader.as_mut().unwrap()` instead of `reader.clone().unwrap().read_line()`.
That is exactly a **MutBorrow** verdict from `classify_use` for a reader-typed local.
Once the typed classifier exists, flipping the Reader family to `Box<dyn BufRead>` +
idiomatic ctors becomes tractable (plus `Result`/`?` propagation for `throws`).

## 7. Suggested sequencing

**Revised after the slice-(c) NO-GO:** start with **(g) `Map.get` read-context**
(genuinely local/self-contained — `.get(&k)`/`.copied()`, no operand-coupling) → then
**(f) foreach last-use subset** → introduce the binary-op operand-coordination in
`visit_binary` (consistent `&a == &b` borrow shape) and land **(c)** on top of it →
**(d) `&`-arg** → introduce the typed `classify_use` + element-`Ty` threading → revive
**(b)** gated on element-Copy + the `.as_mut()` write-target bug fix → then the in-place
IO reader flip (§6). Each slice measured; a precise NO-GO (like (b) and (c)'s first
attempts) is a valuable result. **(g) is now the lowest-risk entry point, not (c).**

## 8. Why this over tier-2 unification

Tier-2 (global type-var union-find / LUB for the jts ~1000 `E0107` cluster) is the
other open frontier and the larger single error mass, but it's more abstract and
foundational (multi-session, no incremental clone payoff). Use-site borrow has
landed prior art, an incremental measured path, a clone-metric payoff each slice,
AND unblocks the user-requested in-place IO — higher leverage to start. Tier-2
remains the right follow-on for the raw-collection error mass (see
`docs/tier2-unification-plan.md` / memory `tier2-unification-frontier`).

## 9. Cross-project note — cpp2rust has nothing to copy here (checked 2026-06-21)

The sibling `cpp2rust` deliberately takes the **opposite** stance: its `Borrow`
overlay is *declaration-driven* (`ptr_depth`/`is_ref` read straight from the C++
source — a `T&` param IS a `&T`, a `T&` return becomes owned `T`), and an explicit
**non-goal** is "never infer borrows beyond the fixed strategy ... pure lookup, no
inference" (cpp2rust `SEMANTICS.md §2.2` + TODO non-goals). C++ states ownership;
Java doesn't — so cpp2rust gets for free exactly what this plan must *infer*. There
is no use-site borrow classifier to port. Takeaways:
- The contrast **validates the thesis**: use-site borrow inference is unavoidable
  for Java precisely because the source carries no ownership signal.
- It **validates the caution** (the slice-(b) NO-GO): inference is risky, so keep
  slices conservative, type-info-gated, measured — don't broaden to a general
  "borrow everywhere" pass.
- The one transferable idea — a per-callable use-site type environment
  (`type_of(expr)` over `Γ`, cpp2rust §4) — java2rust already has in richer form
  (`TypeResolver`/`self.ty`). So the typed `classify_use` builds on existing
  machinery; nothing new is needed from cpp2rust. **The plan stands as written.**
