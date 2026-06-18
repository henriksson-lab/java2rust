# Research plan: Java reference subtyping & Object among stubs

The dominant remaining error mass is **Java subtyping between stub types** — a
value of stub type `X` flowing into a slot of stub type `Y` where Java has
`X <: Y` (extends/implements), e.g. `Option<VariantContext>` returned where
`Option<Feature>` is expected. Rust has no struct subtyping, and both types are
*stubs* (external, no source — we don't know the hierarchy a priori).

## The governing insight (from the Engine-1 NO-GO)

Engine 1 (splitting the shared `Unknown` into distinct per-method stub types)
**regressed**: a distinct bare stub struct lacks `Unknown`'s capabilities
(`Iterator`, arithmetic, `Display`, all derives) — those uses outnumbered the
method-resolution wins. The lesson: **`Unknown` is a *capable* universal type;
distinct named types give method resolution but lose coercion/capability.**

That frames the whole space as one trade-off:

| representation | gives | costs |
|---|---|---|
| shared `Unknown` | coerces everywhere, all capabilities | no real methods |
| distinct stub struct | its own methods | no subtyping, no extra capabilities |
| **trait** (`dyn Y`) | supertype methods **and** coercion `X -> dyn Y` | object-safety limits; impls needed |

So the research is: **for each stub supertype, pick the representation that
minimizes net errors** — collapse to `Unknown` when its methods aren't used (free
coercion), model as a `trait` when they are (coercion + methods).

## Constraint collection (shared substrate for all experiments)

Reuse the type resolver (`type_of`) + the symbol map. Collect a relation over
stub types from every value-into-slot site (assignment, return, argument, ctor
arg, `Option`/collection elements):

> `flows(X, Y)` when a value the resolver types as stub `X` flows into a slot
> the context types as stub `Y`, with `X != Y`.

`flows(X, Y)` ⇒ `X <: Y` (Java upcast). Build the directed graph; `Y` is a
**supertype** (appears as a target). Also record, per stub type `Y`, the set of
methods invoked on `Y`-typed values (`methods_used(Y)`) — the resolver already
locates these.

## R0 — RESULTS (measured; redirected the research)

Mined `expected/found` conflicts across all 8 corpora. The premise (subtyping
dominates) was **wrong** — the breakdown:

- **~68 = nested-type resolution inconsistency (a BUG, not subtyping).** A nested
  `public static enum` like `Alignments.ProfileProfileAlignerType` (×20),
  `AlignerHelper.Last` (×12), `IProfeatProperties.{ATTRIBUTE,TRANSITION,...}`
  (×24), `PairwiseSequence{Aligner,Scorer}Type` (×12) is resolved to a **stub**
  in its own/defining file but to the **project path** (`alignments::X`) from
  other files → `expected X, found alignments::X`. These are project types being
  stubbed inconsistently. **Highest-ROI, and it's a fixable bug — do this first,
  before any subtype research.**
- **~40 = `Unknown` stub-return chains** (`Last`/`String`/`Vec<String>` ↔
  `Unknown`) — the Tier-2 stub-return-inference tail.
- **~12 = nullability** (`String` ↔ `Option<String>`) — non-`this`/param cases.
- **~10 = true subtype upcasts** (`Reader`/`InputStreamReader`,
  `Writer`/`OutputStreamWriter`) — the actual R2 target, smaller than expected.
- **~6 = `java.lang.Object`** (`Box<dyn Any>` ↔ `String`) — R3.

**New priority (pre-empts R1/R2/R3): fix nested-type resolution** so a nested
project enum/class is consistently recognized as a project type (never stubbed)
from every file. Root: `Alignments$ProfileProfileAlignerType` is in the project
map (resolves from outside) but the defining file falls back to the stub —
`resolve_type_sym`/`missing_type_key` don't recognize the nested type uniformly.
~68 errors, a bug not research.

## R0a — RESULTS: nested-type resolution fix — **LANDED (−89)**

2104 → 2015 (bjalign −63, bjaaprop −25, jahmm −9, trim −1; **vcf +9**, the only
regressor). Root cause confirmed: a nested type's map key is `pkg.Outer.Simple`,
but `SymbolMap::resolve` only tried `pkg.Simple` (step 2) — so the *defining*
file (bare reference) fell through to a stub while files that qualify it
(`Alignments.X`, resolved to the project FQN) bound the real type → `expected X,
found alignments::X`. `crate_layout::resolve_parent` already had this nested
fallback, but only for `extends`/`implements`; `resolve` (which drives the
stubbing decision) lacked it.

**Fix:** added step 5 to `SymbolMap::resolve` — match a known type whose FQN is
`pkg.Outer.Simple` where the enclosing prefix `pkg.Outer` is *itself a known
type*. That discriminator (the outer is a defined type, not a sub-package) keeps
the documented no-blind-simple-name-fallback caveat intact: it can't bind a
sub-package type or an unrelated dependency sharing the simple name. Bails on
ambiguity (two same-package nested types with one simple name → stub, don't
guess).

**Gate experiment (rejected):** gating the fallback to `kind == "enum"` (R0's
literal finding) recovered vcf to +5 but *lost* bjalign (−4), jahmm (−4), trim
(−1) → total 2020 > ungated 2015. Nested **classes** resolving to their project
type help more than vcf's regression costs, so the fallback is left ungated. The
vcf +9 is the inverse of the Engine-1 lesson (a stub is *capable*; the real
nested type sometimes is less so) but is dwarfed by the cross-corpus win.

### (original R0 plan below)

Instrument the build to dump, across the 8 corpora:
- count of `flows(X, Y)` conflict sites, and distinct supertypes `Y`;
- for each supertype `Y`: `|methods_used(Y)|` (0 ⇒ collapse-candidate);
- how many conflicts are `java.lang.Object` (→ R3) vs translated interfaces
  (→ existing trait path) vs stub-to-stub structs (→ R1/R2).

Exit: a ranked table of supertypes by (conflict count, methods-used). This
decides the split between R1 (collapse) and R2 (traits).

## R1 — RESULTS: collapse capability-free stubs to `Unknown` — **LANDED (−1, marginal)**

Implemented in `stubs.rs::render_type`: a stub with no fields/methods/statics/
ctors/static-consts emits `pub type X = Unknown;` instead of a distinct struct
(strictly more capable; method-free gate keeps it safe). Net −1 (vcf), zero
regressions. **The valuable result is negative:** the flows-graph shows R1's
premise barely holds — the one genuine stub-to-stub upcast (bjalign
`DistanceMatrix`/`BasicSymmetricalDistanceMatrix` ×4) has methods on *both* sides,
so the gate correctly excludes it → **it needs R2 (traits), not collapse.** Most
other "conflicts" aren't subtyping (Unknown chains, borrow, nullability). R2 is
the real lever for the subtype mass; R1 only fires when both sides are
capability-free, which is rare here.

## Landed alongside R1 (parallel experiment run, 2015 → 1956, −59 total)

- **Enum `impl Display` (−52; the headline).** Recovering the R0a vcf +9 led to a
  *global* fix, not a step-5 gate: every generated enum now emits
  `impl Display { write!(f, "{:?}", self) }`. R0a resolved nested enums to their
  real project type, which — unlike the capable `Unknown` stub it replaced —
  lacked `Display`/`to_string` (the Engine-1 "Unknown is capable" lesson, inverted).
  Restoring that capability recovered vcf *and* netted bjaaprop −35, bjalign −6,
  fastq −2. Lesson: when resolution replaces `Unknown` with a real type, make the
  real type carry `Unknown`'s capabilities.
- **Inherited-getter nullability (−5, vcf).** `resolve_linked_callee` now walks the
  receiver's `parent` chain (mirroring `resolve_self_callee`), so an inherited
  nullable getter (`get_id` on a base `VCFHeaderLine`, called on a subtype)
  resolves — the existing `ret_nullable` unwrap then fires. An Engine-2 follow-on.
- **NO-GO: Unknown stub-return tail.** No bounded deterministic-inference win — the
  cluster is un-modeled external JDK members (`StreamTokenizer.sval`,
  `Arrays.asList`, `BufferedReader.readLine`). Real levers are JDK-stub member
  modeling (separate broad engine) or Tier-2 unification, not a one-shot change.

### (original R1 plan below)

## R1 — collapse method-free supertypes to `Unknown` (lead experiment)

Hypothesis: many supertypes are used *only* for upcasting (stored/returned/
passed, no supertype-method calls). For those, aliasing to `Unknown` is **free
coercion** — and aligns with the Engine-1 finding (collapsing *toward* the
capable `Unknown`, the opposite of Engine 1's failed split).

Mechanism:
- Union-find over stub types using `flows` edges. A component that contains a
  conflict (>1 type) **and** whose members have `methods_used == 0` collapses:
  render every member as `Unknown` (e.g. `pub type X = Unknown;` in its stub
  module, suppressing the struct + its methods).
- Components with method use are left for R2 (or kept distinct if no conflict).

Risk: aliasing a type whose methods *are* used reintroduces `no method` errors
(the Engine-1 failure mode) — hence the `methods_used == 0` gate. Measure
all 8 + revert per the standard discipline. This is the cheapest experiment and
directly tests the trade-off.

## R2 — RESULTS so far: the supertypes are ALREADY traits; the gap is COERCION

Mined `expected/found` named-type upcasts across all **9** corpora (bioformats
now included). The dominant genuine upcasts are **`Box<dyn Trait> ← Concrete`**
where `Trait` is a project interface *already emitted as `dyn Trait`* and the
concrete is a project struct implementing it:
- `Box<dyn Trimmer> ← IlluminaClippingTrimmer` (trim)
- `Box<dyn VCFTextTransformer> ← VCFPassThru…/VCFPercentEncoded…` (vcf)
- `Box<dyn BlockData> ← SerializedBlock` ×2 (bjalign)
- `Box<dyn GapPenalty> ← &dyn GapPenalty`, `Box<dyn PairwiseSequenceAligner> ←
  &dyn …` (bjalign — already-trait-objects needing a re-box)

**So R2's feared hard part (synthesising traits + `impl`s) is mostly unneeded —
interface→trait emission already works.** The residual is **`Box::new`/coercion at
the flow site**, i.e. Engine 3.1 extended to the positions it doesn't yet cover.
Engine 3.1 today (`dump.rs`) boxes only `new Concrete()` in **return** (`visit`
~2029) and **declarator-init** (~3272) positions. Missing: **argument**,
**assignment**, **collection-element** positions, and coercing a **non-`new`
expression** (a variable / `&dyn T`).

**Next R2 step (bounded, low-risk):** extend the `Box::new(x)` coercion to fire
wherever a value whose resolver type is a concrete project struct flows into a
`Box<dyn Trait>` slot **and** that struct implements `Trait` (check via the
symbol map's `interfaces`/`parent`). Start non-generic + object-safe; measure all
9 + validation; revert if net-negative.

**Separable / deferred:** generic trait objects (`Opdf<S,C>`,
`PairwiseSequenceAligner<S,C>` — generic `dyn` is harder) and `Box<dyn Any> ←
String/&str/Concrete` (~13 sites; that's R3/Object, not subtyping).

## R4 (redirected) — synthesize a tagged enum for a polymorphic *project-subtype* hierarchy

**Redirect (user-approved):** R4's `Object`→enum target is narrow + reflection-heavy
in our corpora (`VCFEncoder.formatVCFField(Object)` uses `getClass().isArray()`/
`Array.get` — an enum can't model it; it's the only genuine `Object` slot in vcf).
The same "synthesize an enum with no Java equivalent" *mechanism* fits the much
larger, cleaner **project-subtype dispatch**: a supertype-typed slot holding
subtype values, dispatched by `instanceof` + downcast. Apply R4 there first.

**Canonical case — the `VCFHeaderLine` hierarchy (vcf):**
```
VCFHeaderLine (base)
├─ VCFSimpleHeaderLine → VCFContig/Filter/Sample/Meta/AltHeaderLine
└─ VCFCompoundHeaderLine (abstract) → VCFFormat/InfoHeaderLine
```
- **Current rep:** each subtype is a struct embedding `base: <parent>` (composition);
  a supertype slot (`HashSet<VCFHeaderLine>`, `HashMap<String, VCFHeaderLine>`) stores
  **only the base** → subtype identity lost → `instanceof` lowers to `if false`,
  `(Subtype) x` to illegal `x as Subtype` (the 28 `E0605` + the `instanceof`-method
  cluster).
- **Target rep:** a synthesised `enum VCFHeaderLineKind { VCFHeaderLine(VCFHeaderLine),
  VCFContigHeaderLine(VCFContigHeaderLine), … }` (one variant per concrete type in the
  closed hierarchy). `x instanceof Sub` → `matches!(x, Kind::Sub(_))`; `(Sub) x` →
  `if let Kind::Sub(v) = x`; constructing a `Sub` into a supertype slot → `Kind::Sub(..)`.

**Why an enum (not `Box<dyn>`):** the hierarchy is *closed* (all subtypes are in the
project), `instanceof`/downcast enumerate it, and a struct value (needs `Hash`/`Eq`
for the `HashSet`) suits an enum better than a trait object.

**The hard parts (why this is all-sites + staged):**
1. **Method delegation.** A supertype method called on a `Kind` value (`line.getKey()`)
   must `match` and forward to the active variant (each variant's `base` chain reaches
   the inherited method). Generate delegating methods on the enum for the supertype's
   public API.
2. **Multi-level flattening.** `Contig extends Simple extends HeaderLine` — the enum
   variants are the *concrete* leaf+base types; resolve `instanceof Simple` (an
   intermediate) to "any variant whose chain includes Simple".
3. **All-sites slot replacement.** Every `VCFHeaderLine`-typed slot (field, param,
   return, collection element) becomes `VCFHeaderLineKind`, and every construction
   flowing into such a slot wraps in the variant. Partial = more errors (the
   nullability lesson) → land atomically, measure, revert if net-negative.
4. `Hash`/`Eq`/`Ord`/`Clone` derivation across variants (the `HashSet` key).

**Staging:**
- **Phase 1 (analysis, no codegen):** detect a *polymorphic closed hierarchy* — a
  project supertype with ≥2 project subtypes that is (a) used as a slot type AND
  (b) the target of `instanceof`/downcast. Reuse the symbol map (`parent`/`interfaces`)
  + an `instanceof`/cast scan. Output the {supertype → concrete variants} map + all
  dispatch/slot/construction sites. Decide go/no-go per hierarchy (closed + bounded).
- **Phase 2 (synthesis):** emit the enum + delegating methods + derives.
- **Phase 3 (rewiring, all-sites):** slots → enum, construction → variant wrap,
  `instanceof` → `matches!`, downcast → `if let`. Measure all 9 + validation; revert
  if net-negative.

Large, high-risk, high-payoff — its own focused build (good fork/worktree candidate
once the current tree is committed). Tracked as task #35.

### R4 — feasibility result (fork, NO-GO this pass): it's a multi-day feature gated on a prerequisite

A dedicated worktree fork attempted the full `VCFHeaderLine` enum atomically and
stopped at the feasibility wall (net 0, no code shipped). Three walls:

1. **The type renderer has no slot-context.** `resolve_type_name(name)` /
   `visit_class_type` map a Java type name to Rust **by name alone** — the same call
   renders a *slot* type, the *struct's own definition*, a subtype's
   `base: VCFHeaderLine` *composition field*, and `impl … VCFHeaderLine`. Replacing
   `VCFHeaderLine`→`VCFHeaderLineKind` **only at slots** needs slot-vs-definition
   context threaded through the whole type-emission path; a blanket swap corrupts the
   base-composition inheritance model and explodes the count.
2. **It's a system of 3 enums, not 1.** Three supertypes are used as slot types:
   `VCFHeaderLine` (58 sites), `VCFSimpleHeaderLine` (36), `VCFCompoundHeaderLine` (25).
   Each needs its own variant set, its own method delegation, **plus inter-enum
   coercion** (a `VCFCompoundHeaderLine`-slot value flowing into a `VCFHeaderLine` slot
   = enum→enum convert). An `instanceof` on an intermediate matches any variant whose
   parent-chain includes it.
3. **Flow-driven construction wrapping.** 33 `new VCF*HeaderLine(...)` sites each wrap
   in the correct enum+variant *depending on the target slot's enum* — needs a flow
   analysis we lack.

No partial win exists (instanceof/downcast both need the value already an enum), so
nothing drops the count until slots+construction+delegation+dispatch land together.

**Decomposition (each its own measured step, in order) — the right path if pursuing R4:**
1. **Slot-context in type rendering** — **LANDED (zero-diff).** AST-parent-based
   `is_slot_type(type_node)` (walks `arena.parent` through `ReferenceType`/type-arg
   nesting; matches the parent's type field) + a no-op `slot_enum_name(name) -> Option`
   hook wired into `visit_class_type`. No flag threading. The `base:` field, struct-def
   name, and `impl` headers never reach `visit_class_type` (emitted directly), so
   they're excluded for free; casts/`instanceof`/`new`/throws fall to the non-slot arm.
   Verified: marker test shows substitution lands only at field/param/return/type-arg
   slots; with the hook returning `None`, output is byte-identical (1951 + 15, all green).
1.5. **(prerequisite, found by the 2nd fork) a whole-program `Eq/Hash`-derivability
   FIXPOINT.** `dump.rs:2660` denies `PartialEq/Eq/Hash` to any subtype (`extends`
   non-empty) because the synthesized `base: Parent` field is a *struct* and
   `type_derives_eq` categorically rejects struct fields (a naive local allow once
   "regressed badly"). The synthesized enum must be `Hash+Eq` (it keys
   `HashSet<VCFHeaderLine>`), so every variant struct must be — blocked by this gate.
   Correct fix = a fixpoint (a type derives `Eq/Hash` iff every field incl. the base
   chain transitively does); a real multi-file analysis with cross-corpus blast radius,
   **NOT** a scoped lift. **Measured independent payoff: ~2 errors across all 9** — so
   it's almost pure R4-unblocking cost.
2. Enum synthesis + method delegation (~10 methods) for the hierarchy (one root enum for
   all three supertypes — dissolves the inter-enum-coercion wall) + derives.
3. Construction wrapping (33 sites) + instanceof→`matches!` + cast→`if let`; **measure**.

### R4 — LANDED (activated, kept). The enum-hierarchy approach works end-to-end.

Full activation with a dispatch-gate: a `<Root>Kind` enum per hierarchy that is
`instanceof`/cast-dispatched (storage-only hierarchies stay concrete), slot routing
via `slot_enum_name`/`enum_info_map` (keyed by `rust_path`), construction-wrap at
collection ops + return + declarator, cast→variant-extraction, `instanceof`→`matches!`,
`Deref`-to-root for base methods. The dispatch set is collected by a cross-file
pre-scan in `crate_layout` (`collect_dispatched`) → `SymbolMap::dispatched`.
**Results: core 1951 → 1941 (−10): vcf −7, jaligner −3; jhlabs +4 (kept as a known
residual).** All validation green (92 tests, 42/42 golden, 110/110 compilecheck, 0
warnings). **The runtime correctness goal is met** — `instanceof`/downcast dispatch
correctly through the enum.

**Known residual (the all-sites tail): enum-leak.** A hierarchy used BOTH dispatched
AND flowing into `Box<dyn Any>`(Object)/concrete-typed positions leaks the enum where
a concrete/`Any` is expected (jhlabs `Light`/`ArrayColormap`: `expected Light, found
LightKind`; `expected Box<dyn Any>, found LightKind`). Needs enum→concrete deref +
enum-as-`Any` boxing at those sites. No cheap gate (dispatch *frequency* doesn't
separate beneficial from harmful — jaligner `Format` is cast as rarely as jhlabs
`Light` yet benefits). Next focused item.

### (prior) R4 status — foundation LANDED (dormant); activation scoped by a full probe

**Foundation (on the tree, dormant, zero-impact):** `enum_root_variants` +
`emit_hierarchy_enum` synthesize `<Root>Kind` (variants per concrete hierarchy type,
correct `.base` hop chains, `#[derive(Clone[,PartialEq,Eq,Hash])]` + manual `Default`
→ root variant + `Deref`/`DerefMut` to root). `slot_enum_name` returns `None` (gate
off) so the enum is emitted-but-unused (`#[allow(dead_code)]`). Verified: all 10
corpora at baseline, all green.

**Activation probe (full supertype activation, then reverted): vcf 469 → 489 (+20).**
With `slot_enum_name` mapping all polymorphic supertypes (root + intermediates) to the
enum and NO rewiring, the +20 breaks down (vcf error histogram):
- `Deref` covers root methods for free (getKey/toString/equals).
- ~26 `E0599` "no method for enum" + ~25 "method exists but trait bounds" + ~10 "no
  variant/assoc item" = **intermediate-method calls** (`getID` on `VCFSimpleHeaderLine`,
  `getType`/`getCount` on `VCFCompoundHeaderLine`) that `Deref`-to-root doesn't cover →
  need **delegating methods** on the enum (per-variant `match`, `unreachable!` for
  variants lacking it). *This is the hard subsystem: synthesizing correct Rust method
  signatures (params/return) from the symbol map.*
- construction `E0308`: `new Sub()`/concrete value into an enum slot → **construction-wrap**
  to `VCFHeaderLineKind::Sub(..)` (reuse the `Box::new` coercion sites).
- 28 `E0605` casts are *baseline* errors → **cast-extract** (`(Sub) x` →
  `match x { Kind::Sub(v) => v, _ => unreachable!() }`) pushes them *below* baseline.

**Remaining = the rewiring (all-sites — measures only when all land together):**
delegation (hardest) + construction-wrap + cast-extract + `instanceof`→`matches!`. A
*root-only* activation is just +6 (the fork's earlier probe) and avoids the
intermediate-method delegation entirely — a possible cheaper first cut if cast-extract +
construction-wrap on root-typed values net negative without delegation. Next focused
build (fork from the committed foundation, fresh budget for the iterate-to-correct
codegen).

### R4 status update — two prerequisites now LANDED

- **Hash prerequisite — DONE (the real one).** Synthesized `impl PartialEq`/`Eq`/`Hash`
  for project structs that can't `#[derive]` (subtypes via the `base` field; map/set-
  bearing value types). A monotone capability fixpoint in `crate_layout`
  (`compute_eq_capability`) + codegen in `dump.rs` (`emit_synth_eq_impls`) that hashes a
  *top-level* `Map`/`Set` field by an order-independent fold (mirrors Java
  `AbstractMap.hashCode`). Error-neutral (all 10 corpora at baseline), **0 `E0119`**, all
  green. **The whole `VCFHeaderLine` hierarchy is now `Hash`+`Eq`** → the enum can key a
  `HashSet`. This is the de-`Hash`-semantics-change *avoided*.
- **Bonus (not R4): hex-float / bitop-on-float fix.** `stop_history_search` now stops the
  float-context walk at bitwise/shift ops (their result is integral), so hex int masks
  (`0xff`) stop being float-coerced. jhlabs 1426→1365 (−61).

**R4 remaining = just the enum itself (steps 2–3), now genuinely unblocked:** step 1
(slot-context) ✅ + Hash ✅ done. What's left for vcf `VCFHeaderLine`: synthesize the enum
(one root enum, all supertype slots) with `Hash`/`Eq` delegating to variants, ~10-method
delegation, 33 construction wraps, instanceof→`matches!`, cast→`if let`. No architectural
walls remain — both onion layers (slot-context, Hash) are peeled.

### R4 TERRAIN MAP (after 5 forks ≈1.5M tokens — only step 1 landed)

The enum approach is sound in principle (step 1's slot-context works), but every
*real* hierarchy hits a substantial, target-specific wall:

- **vcf `VCFHeaderLine` — `Hash` wall, but SOLVABLE via manual impls (correcting the
  fork).** The enum keys `HashSet<VCFHeaderLine>`, so it needs `Hash`.
  `#[derive(Hash)]` fails because `VCFSimpleHeaderLine` has a
  `genericFields: LinkedHashMap<String,String>` and Rust's std maps don't impl
  `Hash`. The fork concluded "never Hash" — but that only tested *derive*. Java
  hashes these fine: `VCFSimpleHeaderLine.hashCode()` does
  `... + genericFields.hashCode()`, and Java `Map.hashCode()` is the
  order-INDEPENDENT sum of entry hashes (`AbstractMap`). Rust can replicate that
  with a **manual `impl Hash`** that folds the map entries order-independently.
  → **The real prerequisite is NOT a de-`Hash` semantics change** (no `HashSet`→`Vec`
  dedup loss). It's: translate the Java `equals()`/`hashCode()` these types already
  define into **manual `impl Hash`/`PartialEq`/`Eq`**, hashing/comparing a `Map`/`Set`
  field by an order-independent fold (not `.hash(state)`). Semantics-preserving,
  independently useful (any Java value-type with a map field used in a `HashSet`
  hits this today), and reopens vcf R4. The translator detects `manual_impls`
  (`dump.rs:2660`) but doesn't emit a working `impl Hash` for the map-bearing case —
  that's the gap. Delegation is small (~10 methods).
- **imagej `ImageProcessor` — MECHANICAL wall, NO prerequisites.** No `Hash`-
  requiring containers, no `equals`/`hashCode` overrides → enum needs only
  `Clone`/`Default`. But the abstract base has 239 methods, **~75 actually called**
  on `ImageProcessor`-typed values → the enum must delegate ~75 methods (a `match`
  per method) OR `Deref<Target=ImageProcessor>` for non-overridden + explicit
  dispatch for overridden ones. 33 `(ConcreteProcessor)` downcasts are the payoff.

**Conclusion:** R4 is a multi-day feature on ANY real target; the choice is which
wall to climb. imagej's is *mechanical* (generate N delegating methods — bounded
codegen, no semantics decisions), which is more landable than vcf's *architectural*
onion. **If R4 continues, imagej `ImageProcessor` is the better first target.**

**Economics (honest, after 3 forks ≈1.1M tokens):** step 1 landed (zero-diff foundation).
Remaining = step 1.5 (~0 payoff) + the step 2–3 5-subsystem atomic landing (which may wall
again). Total payoff **~35 vcf errors (~1.8% of 1951)**. The payoff/effort ratio is
unfavorable — **recommend pausing R4** with step 1 kept, unless the correctness value
(working `instanceof`/downcast at runtime) is wanted for its own sake. The design is fully
mapped here for a future deliberate build.

## Object handling: R4 is primary, R3 is the fallback (ordering: R4 → R3)

Per the user: **do R4 before R3.** R4 (synthesise a tagged enum for a
heterogeneous `Object` slot) is the *primary* representation; **R3 (`Box<dyn Any>`
+ downcast) is the fallback** for when R4 fails — i.e. the variant set can't be
collected into something small/closed (open-world Object). Later, R3 may also be
selectable under memory pressure (a tagged enum is sized to its largest variant,
so a wide/rare-large union can be heavier than a boxed pointer) — but that
size-driven switch is a *later* story, not now. So: build R4; reach for R3 only
where R4 can't produce a bounded enum.

## R2 step 2 — RESULTS: `Box<dyn Trait>` coercion at const-field init — **LANDED (−2)**

Extended the same coercion to static-field/`const` initializers
(`LazyLock::new(|| new Concrete())` into a `Box<dyn Trait>` field). vcf −2
(`VCFPercentEncoded/PassThruTextTransformer` into `Box<dyn VCFTextTransformer>`),
no regressions. **R2 total this pass: −5** (1956 → 1951).

## R2 step 3 — `Box<dyn Trait>` coercion at field-assignment — **correct, net-zero (masked)**

Extended the coercion to `self.field = <concrete>` where the field's declared
type is a project interface (rendered `Box<dyn Trait>`) — detected via
`assign_target_trait` (symbol-map `kind == "trait"`), since `assign_target_rust_type`
returns the bare name, never the `Box<dyn …>` form. It boxes correctly
(`__self.child1 = Box::new(Node::new(...))` in bjalign's `GuideTree`), **but nets
zero**: the firing sites are the *generic* `GuideTreeNode<S,C>` fields, and fixing
the trait-object E0308 just unmasks a pre-existing `Node::new` generic-argument
error on the same line (rustc reports one error/line). Kept — it's correct codegen
that completes coercion coverage uniformly (return, declarator, const-field,
assignment); it will pay off once the generic-arg errors are addressed. All
validation green.

**Boundary reached:** every easy value-into-slot position now boxes. What's left
needs the deferred hard work — generic trait-object argument plumbing and the
`&dyn → Box<dyn>` `clone_box` path.

**R2 remaining surface (harder, not done):** the genuine leftovers are now
**generic** trait objects (`PairwiseSequenceAligner<S,C>`, `Opdf<…>`,
`GuideTreeNode<S,C>`, `Centroid<L>`) and **`&dyn X → Box<dyn X>` re-box** sites
(`GapPenalty`, `ParallelCompressor` — can't move out of a reference, so they need
`clone_box`, the deferred Engine-3.2 path), plus `SerializedBlock → Box<dyn
BlockData>` (an argument/field-assign position). The easy value-into-slot
positions (return, declarator, const-field) are captured.

## R2 step 1 — RESULTS: `Box<dyn Trait>` coercion at return/declarator — **LANDED (−3)**

Extended the Engine-3.1 `Box::new` coercion beyond `new Concrete()`: it now also
fires when the flowing value resolves to a concrete project **struct that
implements the target trait** (`expr_impls_trait` → `struct_impls_trait`, walking
the symbol map's `interfaces`/`parent` chain). Two sites updated (return,
declarator-init).

**Prerequisite uncovered + fixed:** the dominant case is a **static factory call**
(`return IlluminaClippingTrimmer.makeX(...)` where `makeX` returns the concrete,
flowing into a `Box<dyn Trimmer>` method). The symbol map recorded **only numeric**
return types (`numeric_ret`), so the factory's class return was `None` and couldn't
be resolved. Broadened recording to class/interface return simple names
(`method_ret_type`). This is a global resolver input change (the resolver's
`lookup_method_ret` now resolves class-typed returns to `Named`) — measured clean:
trim −1, bjaaprop −2, **1956 → 1953**, no regressions, all validation green.

**Remaining R2 surface (not yet done):** the same coercion at **argument**,
**assignment**, and **collection-element** positions; generic trait objects
(`Opdf<S,C>`, `PairwiseSequenceAligner<S,C>`); and `Box<dyn Any> ← String/Concrete`
(that's R3/Object). Each its own measured step.

### (original R2 plan below)

## R2 — model method-used supertypes as traits

For a supertype `Y` with `methods_used(Y) > 0` and known subtypes `{X...}`:
- emit `Y` as a `trait` carrying `methods_used(Y)` (best-effort signatures);
- emit each subtype `X` as a struct with `impl Y for X { ... }` (bodies
  `unimplemented!()`), and ensure `X` carries the union of its own methods;
- at `flows(X, Y)` sites, coerce: `Box::new(x) as Box<dyn Y>` / `&x as &dyn Y`
  (owned vs borrowed by slot position — reuse the `Box::new` machinery from
  Engine 3.1).

This is the principled fix (coercion **and** methods) but the hard one:
object-safety (generic/`Self`-returning methods break `dyn`), generic
supertypes (`Aligner<S,C>`), and the bare-`Box<dyn Y>`-field `Default` cap
(documented in the engines plan). Start with the **single highest-conflict,
non-generic, object-safe** supertype from R0; measure; generalise only if it
nets positive.

## R3 — `java.lang.Object` via `Box<dyn Any>` + downcast

Java `Object` slots and `(ConcreteType) obj` downcasts (vcf-heavy) currently
become illegal `obj as ConcreteType`. Experiment: model an `Object`-typed slot
as `Box<dyn std::any::Any>` and lower a downcast to
`obj.downcast::<T>().ok()...` / `downcast_ref`. Invasive (Object is pervasive;
ownership/borrow churn) — gate to the downcast sites first, measure blast radius
before widening.

## R4 — synthesize a tagged enum for heterogeneous `Object` slots (future work)

For a genuinely-heterogeneous `Object` slot (`Map<String,Object>` VCF attribute
values, `Object val`/`Object number` — see the Object-need analysis: storage is
already `Unknown`, but `instanceof` dispatch compiles to a dead `if false`),
synthesize a **Rust enum with no Java equivalent** rather than `Box<dyn Any>`:

```
enum VcfAttrValue { Int(i64), Float(f64), Str(String), List(Vec<String>), Other(Unknown) }
```

Writes wrap into a variant; `(Integer) val` downcast → variant extraction;
`val instanceof Integer` (today `if false /* instanceof Integer */`) →
`matches!(v, VcfAttrValue::Int(_))`. This beats `Box<dyn Any>` for vcf because
the set is **small and closed** and the program's own `instanceof` chains already
enumerate it.

**The special collection mechanism (this is the work).** An `Object` slot is an
unsolved type *variable*; the enum is its inferred **sum type** — a concrete
application of the Tier-2 unification reserved as `Type::Var` in `types.rs`, and
of the cross-function equivalence-class idea.

1. **Slot identity / equivalence classes.** Give each `Object` occurrence
   (field, param, local, map-value position) a slot-id; union slot-ids that are
   the same symbol across the program (a map built in `parseInfo` and dispatched
   elsewhere is one slot). Keyed via the symbol map, fixpoint like nullability.
2. **Evidence harvest** per class — union of: RHS expr types at `put`/ctor/return
   (resolver `type_of` already types these); concrete types named by `instanceof`
   tests on reads (strongest signal); downcast targets `(X) val`.
3. **Decide.** Synthesize only when the set is small and closed; else keep
   `Unknown`. Always include an `Other(Unknown)` escape variant so an
   undetermined inflow keeps `Unknown`'s capabilities (the Engine-1 open-world
   fallback). Map Java types to Rust (`Integer→i64`, `Double→f64`, …); dedup and
   name the synthetic enum per slot deterministically.

Risk: cross-erasure slot identity, variant explosion (cap and fall back to
`Unknown`), and rewiring every read/write/dispatch site atomically (all-sites,
like nullability). Highest-value Object work, vcf-specific. Supersedes R3 for the
heterogeneous-container case (keep R3/`Box<dyn Any>` only if an open Object slot
with no collectible variant set appears).

## Method & discipline

Same as always: each experiment is one measured unit across all 8 corpora +
42 golden + 110 compile-check + the cargo tests; revert net-negative; document
the finding (every NO-GO here is itself a result — the space is a trade-off and
we're mapping it). Order: **R0 → R1 → (R2 on the top supertype) → R3**, each a
stop point.

The single most valuable output is R0's table — it tells us how much of the
mass is collapsible-for-free (R1) vs needs traits (R2) vs is Object (R3), so we
invest the hard R2 effort only where it pays.
