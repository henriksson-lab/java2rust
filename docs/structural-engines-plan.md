# Structural engines plan

The next big steps after the unified type-system work (`src/types.rs`) and the
stub-return-inference contexts. These are **build-then-payoff** structural
engines, not incremental fixes: effort accrues and the error reduction lands at
each phase's exit.

## Shared discipline (every phase)

Measure all 8 corpora (`tools/*_check.sh`) + 42 golden (`cargo run --example
check`) + 110 compile-check (`tools/compilecheck.sh`) + the cargo tests after
**each** change. Revert any net-negative step. Land "all-sites-at-once" engines
(nullability, trait objects) as one unit — a partial change *adds* errors. Every
phase has an explicit exit criterion and a stop point.

## Priority order (ROI ÷ risk)

1. **Synthetic stub-return types** — continues the stub-inference engine, attacks
   a concrete cluster, self-contained.
2. **Whole-program nullability** — largest error share (E0308 mass); proven
   template (array-element nullability).
3. **Trait-object support** — big bjalign E0277, but capped by an unsolvable
   sub-problem (`Default` for a bare `Box<dyn T>` field).
4. **`Box<dyn Any>` / Object modeling** — vcf-specific, hardest, lowest leverage.
   Defer.

---

## Engine 1 — Synthetic stub-return types (`stub.foo().bar()`) — **NO-GO (tried, reverted)**

**Outcome:** implemented and measured — **net +6** (bjaaprop +5, vcf +3, trim −2),
reverted. The coupling was *not* the problem (the single-source-of-truth name
derivation worked). The problem is that the shared `Unknown` is a **capable**
placeholder — it impls `Iterator`, arithmetic, `Display`, and all derives — so
replacing it with a distinct bare stub struct fixes `no method bar()` but breaks
every result that was iterated / formatted / used in arithmetic, and those
outnumber the method-resolution wins. **`Unknown`'s broad capabilities are worth
more than distinct typing.** Would only become viable if synthetic types
replicated all of `Unknown`'s impls — not worth the machinery. Do not re-attempt
in this form.

**Original problem statement (for reference):** an un-inferrable stub return is
the shared `Unknown`, so a method called on it (`.bar()`) fails. Idea was:
`foo()` returns a *distinct* stub type that accrues `bar()`.

**Fragile coupling, solved by construction.** `foo`'s recorded return path and
`bar`'s recording location must agree. Route **both** through one helper:

```
fn synthetic_return(&self, inner_call) -> Option<SynthRet>
//  SynthRet { fqn, simple, rust_path } — derived once from (owner_fqn, method, arity)
```

Give the synthetic type a real FQN in a dedicated package (`__ret.<Owner>_<method>`)
so `missing_type_key` / the stub renderer place it deterministically and
`rust_path` is computed by the *same* machinery the renderer uses. One source of
truth ⇒ the two paths cannot diverge.

- **Phase 1.1** — `synthetic_return` helper + `SynthRet`. Gate: inner call's
  receiver resolves to a stub (`missing_type_key` succeeds), return not otherwise
  inferable, chain depth ≤ 2. Unit-test name/path derivation. No wiring.
- **Phase 1.2** — `infer_call_ret_type`: call-as-receiver-of-unknown-method →
  `synthetic_return(..).rust_path`. `callee_recv_type`: `MethodCallExpr` receiver
  → `synthetic_return(..).simple` so `record_missing_call` attaches `bar`.
  Register the synthetic type so it renders. Measure.
- **Risk/exit:** synthetic-type proliferation + trait satisfaction (synthetic
  stub gets the standard derives + `Iterator`/arithmetic impls). Exit:
  receiver-chain `Unknown` cluster down, no regressions. Stop after 1.2.

---

## Engine 2 — Whole-program nullability (`Option<T> ↔ T`) — **LANDED (−59)**

**Outcome:** 2178 → 2119 (vcf −30, jaligner −10, bjalign −9, trim −5, jahmm −3,
fastq −2), no regressions. The gap was concrete: `expr_nullable` didn't handle
`FieldAccessExpr`, so nullable `this.field` targets/reads weren't recognized
(`self.version = version` never `Some`-wrapped → "expected Option, found X", ×100).
Fix: an `expr_nullable` arm for `this.field` + a nullable-field read-unwrap in
`visit_field_access`. **Key subtlety:** resolve the field against the class's
*fields directly* (`this_field_nullable`), NOT general name resolution — a
same-named param (`this.version = version`) shadows it and gives the wrong
nullability (this caused a +6 trim regression until fixed). The "3× regression"
history was avoided by all-sites + measure + shadow-safe resolution.

### (original plan below)

Template: array-element nullability (orthogonal fact + all-sites-at-once +
revert-on-regression) — sidesteps the 3× local-tweak regressions.

- **Phase 2.1 (analysis)** — `nullability` already produces the `nullable` decl
  set + fixpoint. Audit completeness (field/local/param/return). No emission
  change yet. Measure how many E0308 are `Option<T>↔T` and which decls drive them.
- **Phase 2.2 (emission, ALL sites at once)** — the whole risk is here:
  - declared type → `Option<T>` (field/local/param/return)
  - value into a nullable slot → `Some(..)`/`None` (`emit_into_option`)
  - read of a nullable value in a plain position → `.clone().unwrap()`
  - `== null`/`!= null` → `.is_none()`/`.is_some()`
  New work is **coverage**, not new mechanism. Land as one commit; measure;
  revert if net-negative.
- **Risk/exit:** read-context coverage (closures, borrows). Highest payoff *and*
  highest risk; all-at-once + immediate revert is non-negotiable.

---

## Engine 2.3 — inherited-field nullability — **LANDED (−6)**

Follow-on to Engine 2: populated `FieldSym.nullable` (was hardcoded `false`) in
`crate_layout`, and added `inherited_field_nullable` so a nullable *superclass*
field (`self.base.version`) is `Some`-wrapped on assign and unwrapped on read,
via the symbol map. vcf −6, no regressions. Further nullability follow-ons exist
(method-return `ret_nullable` propagation, ~3 vcf; non-`this` object fields) but
are smaller / cross-class.

## Engine 3 — Trait objects (`Box<dyn T>`) — **assessed LOW-ROI/capped, deferred**

Measurement after Engine 2: bjalign's 140 `E0277` is dominated by type-param
`S: Default` (39) + `C: Default` (39) = 78 — the generic-`Default`-construction
**dead-end** (adding those bounds cascades; proven). Only ~15 are
`dyn T: Clone/Default` (what `clone_box` targets), and the structs holding those
`Box<dyn>` fields *also* fail `S/C: Default`, so `clone_box` would net ≈0 there.
Engine 3.1 (`Box::new`) is done (−1). Engine 3.2 (`clone_box`) is not worth the
high-effort generic-`Clone`-for-`Box` machinery for ≈0 net — deferred.

### (original Engine 3 plan below)

- **Phase 3.1 — `Box::new` coercion** beyond the declarator (done) to
  return/assignment/argument positions. Bounded, independent, low-risk.
- **Phase 3.2 — `clone_box`**: interface→`trait` emission gains
  `fn clone_box(&self) -> Box<dyn T>` + `impl Clone for Box<dyn T>`; each
  `impl T for Struct` forwards to derived `Clone`. Spans trait + all implementors
  + generic `impl<…> Clone for Box<dyn T<…>>`.
- **Hard boundary (document, don't chase):** a *bare* `Box<dyn T>` field needs
  `Default`, which a trait object can't provide — caps the win.
- **Exit:** bjalign `Box<dyn>: Clone` E0277 + `Box::new` E0308 down; accept the
  `Default`-capped residual.

---

## PLAN STATUS (worked through)

- Engine 1 (synthetic stub returns): **NO-GO** — `Unknown`'s capabilities dominate.
- Engine 2 + 2.3 (whole-program nullability, incl. inherited fields): **LANDED, −65** — the headline win.
- Engine 3.1 (`Box::new` return coercion): landed (−1). Engine 3.2 (`clone_box`): **capped/deferred** — bjalign's mass is the `S/C: Default` dead-end, holding structs fail `Default` regardless.
- Engine 4 (subtype/Object): **deferred** — see below; confirmed it's the nature of the remaining "Option" mismatches (`Option<VariantContext>` vs `Option<Feature>` = a stub-to-stub upcast).

Remaining error mass across the 8 corpora is now dominated by: subtype upcasts between stub types (no trait hierarchy ⇒ no clean Rust coercion), the generic-`Default`-construction dead-end, `Box<dyn Any>`/Object downcasts, and stub-return `Unknown` chains. These are research-grade (modeling Java subtyping/Object among stubs), not bounded incremental fixes.

## Engine 4 — `Box<dyn Any>` / Object modeling — DEFERRED

vcf-specific (`(Type) objectValue` → `as ConcreteType`, illegal). Needs an
`Any`-downcast model (`.downcast_ref()`). Lower leverage; plan only after 1–3.

## Sequencing

Execute 1 → 2 → 3, each its own measured mini-project; #2 must land atomically.
Stop points after every phase to reassess.
