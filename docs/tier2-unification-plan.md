# Tier-2: global type-variable unification

The incremental/structural frontier is exhausted; three measured NO-GOs
(`Vec<Unknown>`, per-decl element inference, stub-return splitting) all proved the
same thing: the dominant remaining clusters are **whole-dataflow facts**, not
per-site facts. A type guessed correctly at one site still clashes at every other
site that wasn't unified to it. The fix is the **Tier-2 unification** the codebase
reserved `Type::Var(u32)` for from the start: infer an unknown type **once** and
propagate it **consistently** to every read/write/pass.

## The core idea
- Assign a **type variable** `Var(id)` to each currently-unknown type position.
- Generate **equality constraints** between type-expressions from the dataflow.
- **Union-find** solve: each class resolves to its (unique) ground member, or to
  `Unknown` (conflict, or no ground evidence).
- **Apply consistently**: a global map `slot → resolved Type` is consulted by BOTH
  the renderer (`visit_class_type`) AND the resolver (`TypeResolver::type_of`), so
  a declaration and all its uses agree. *This consistency is the whole point — the
  per-decl NO-GO failed precisely because the render said `Vec<Shape>` while the
  resolver still saw raw `List` at `.get()`.*

## Conservatism (non-negotiable, per the NO-GOs)
A variable resolves to a ground type ONLY on unambiguous evidence. Conflict or
no-evidence → `Unknown` (= today's behaviour). Tier-2 must be **monotone**: it can
only turn an `Unknown`/bare slot into a *consistently-applied* concrete type; it
must never introduce a type that some site disagrees with. Measure every phase
across all 12 corpora; revert any net-positive.

## Architecture
- **Var table** (`src/types.rs` or a new `unify.rs`): a union-find over `u32` var
  ids; each id carries an optional resolved ground `Type`. Keyed by **slot
  identity** — start with: a collection-typed declaration's *element* (field FQN /
  local decl NodeId), a stub method's *return*, later generic-param instances.
- **Constraint collection**: a pass over the project (reuse `crate_layout`'s
  all-files parse + the resolver) emitting `unify(a, b)` where a/b are vars or
  ground types. Sources per phase below.
- **Solve**: run after collection; freeze the `slot → Type` map.
- **Application**: the map is threaded into the dumper (like `SymbolMap.dispatched`
  /the eq-capability flags) and consulted at render + in `type_of`.

## Phases (each its own measured unit; the order is by payoff ÷ containment)

**Phase 0 — substrate (zero behaviour change).** Union-find + var table + the
`slot → Type` plumbing into the dumper and resolver, with an empty map. Verify all
12 corpora unchanged.

### Phase 0 + per-decl Phase 1 — RESULT: substrate PROVEN, per-decl solve is a dead end

A fork built Phase 0 and a *per-declaration* Phase 1. **Phase 0 substrate works and is
correct** (kept as reference: `docs/tier2-substrate-reference.diff`): a shared
`Rc<HashMap<NodeId, Type>>` keyed by the declaration's type-node id, consulted by BOTH
`visit_class_type` (render) and `TypeResolver` (new `coll_elem` field + `with_coll_elem`,
in `type_of_node`). This delivers render/resolver **agreement** — a raw `List` field and
its `.get()`/iteration type to the same element. That mechanism is the reusable foundation.

**The per-decl solve is conclusively dead** (JTS `E0107` 1038→877 but TOTAL 7490→**7609,
+119**) for two reasons that define what the real solve must do:
1. **Cross-declaration flows** — a field inferred `Vec<T>` flows into params/returns/other
   raw collections that stayed bare → `expected Vec<T>, found Vec<_>` (the dominant new
   errors). Decl-site agreement isn't enough; the field must be unified with the *slots it
   flows to*.
2. **Element over-narrowing** — `.add(x)` is one *sample*, not the declared type. JTS
   `.add(somePolygon)` into a `List<Geometry>` infers `Vec<Polygon>` (×228-ish clashes),
   even entangling with the R4 `GeometryKind` enum. Must infer the **least upper bound**
   (common supertype over the `parent`/`interfaces` hierarchy), not a sample.

**So real Phase 1 = (a) union-find over ALL collection slots** (field ↔ param ↔ return ↔
local ↔ the methods they flow through — one element per flow, or bare) **+ (b) LUB joining
over the Java type hierarchy.** Build on the proven substrate; replace the per-decl policy.

### Phase 0 + Phase 1 (leaf-local slice) — LANDED (JTS −18; substrate on main)

Shipped the substrate + a deliberately *monotone* Phase-1 slice, gated three ways:
**leaf elements only** (skip any element with project subtypes or that is R4-enum'd —
because a concrete subtype element cascades under Rust's lack of struct subtype
covariance), **locals only** (skip fields/params — contains cross-flow), **exclude
`Object`/`Class`** (they map to `Unknown`). JTS 7490 → **7472 (−18)**, all 11 others exact
baseline, all green. The substrate (`coll_elem` in the resolver + the shared
`slot→Type` map at the renderer) is now on `main` — the foundation for the rest.

### The real remaining unlock: R4 × Tier-2 FUSION

The bulk of JTS's 1038 `E0107` are **hierarchy-typed collections** (`List<Geometry>`,
`List<Coordinate>` — subtype-bearing), which the leaf gate correctly *skips*. They can't
be `Vec<Geometry>` (Rust has no covariance — `.add(subtype)` and `.get()→supertype`
clash; measured per-decl +119). The answer fuses the two engines: a `List<Geometry>`
where `Geometry` is an R4-dispatched hierarchy must become **`Vec<GeometryKind>`** (the R4
enum element), so `.add(subtype)` wraps into the variant and `.get()` reads the enum.
That requires: cross-slot union-find (a) + LUB to the hierarchy root (b) + **mapping the
LUB to its R4 `<Root>Kind` enum (c)**. This is the next phase — and it's why Tier-2 and
R4 are one system, exactly as the unified-`Type` design intended.

### Fusion — RESULT: NO-GO, and it identifies the true prerequisite

The fusion mechanism *works* (renders `Vec<GeometryKind>`, wraps `.add`), but it
REGRESSES (jts +84 with fields, **+22 locals-only**) — because it's gated on a
hierarchy being R4-enum'd, and the dominant one, **`Coordinate`**, is a *pervasive value
type*: used in `Coordinate[]` arrays, plain params/returns/locals all over JTS. R4's
slot-routing is **NOT universal** (arrays `T[]` especially, and various positions, aren't
routed to the enum), so a collection element of `CoordinateKind` clashes with the many
un-routed concrete `Coordinate` sites (`expected Coordinate, found CoordinateKind` ×179).

**The true prerequisite (was hidden under three layers): UNIVERSAL R4 slot-routing.**
Every position of an enum'd hierarchy type — arrays `T[]`, all field/param/return/local
slots, not just the ones R4 currently routes — must route to the `<Root>Kind` enum, so
the enum can NEVER leak into a concrete site. This is also the same root cause as R4's own
"enum-leak tail" (jhlabs `Light` → `Box<dyn Any>`/concrete). Once routing is universal:
- a *contained* hierarchy (vcf `VCFHeaderLine`) is already fine;
- a *pervasive* hierarchy (`Coordinate`) stops leaking → the collection fusion becomes
  monotone, AND R4's own gains widen.

So **universal R4 slot-routing (incl. arrays) is the next lever**, not the fusion itself.
The fusion is correct but downstream of it. Kept state: Tier-2 Phase-1 leaf-local on main
(jts 7472).

### Universal-routing investigation — RESULT: the real bug was OVER-wrapping (−1586, LANDED)

Diagnosing the leak flipped the premise: R4 wasn't *under*-routing, it was **over-wrapping**.
R4 already routes collection element slots to the enum at render (`Vec<CoordinateKind>`),
but the construction-wrap re-wrapped values that were ALREADY the enum (a for-each var,
`.get()`, a routed-return read) → `Kind::Root(x)` where `x` is itself a `Kind` →
`expected Root, found Kind`, *pervasive* in hierarchy-heavy corpora. Subtractive fix in
`enum_variant_for_expr`: skip the wrap when the value resolves to the hierarchy ROOT, is a
**read** of an already-routed place (name/field/array-elem/method-result, through parens),
and isn't a fresh `new Root(...)`. **−1586 across 12 corpora, zero regression, all green:**
jts 7472→**5964 (−1508)**, jsoup 2742→2673 (−69), vcf 462→455 (−7), jhlabs −1, trim −1.
(A broader `is_root && !new` gate nets −1593 but regresses jaligner +1; the read-gate trades
those 7 for zero regression — the disciplined choice.) This is the single largest reduction
of the arc, and it makes R4's enum machinery far healthier.

**Remaining R4-routing tail (small now):** genuine arrays `T[]` and a few non-read seams
(jaligner `FormatFactory`) are still un-routed — the original "universal routing" work, now
a minor follow-up. The Tier-2 fusion (Phase 2, task #39) is also more viable post-over-wrap-fix.

### (original) Phase 1 — raw-collection elements (JTS ~1038 `E0107`; the lead). One element
var `e_D` per raw collection declaration `D`. Constraints:
- initializer `= new ArrayList<Foo>()` → `e_D = Foo`
- `D.add(x)`/`offer`/`push` → `e_D = type_of(x)`; `D.put(k,v)` → key/val vars
- `T v = D.get(i)` / `for (T v : D)` → `e_D = T`
- `D = other` (collection assign) / param-pass / return → `e_D = e_other`
Solve; the resolved element is applied at BOTH the `Vec<…>` render AND in
`type_of(D)` so `.get`/iteration/`.next` yield the same element everywhere (the
consistency the per-decl attempt lacked). Measure JTS — must go DOWN.

**Phase 2 — stub-return chains.** A stub method's return is a var; unify it with
the slot its result flows into (assignment target, next-call receiver, return).

**Phase 3 — generic params / R4 enum-leak boundaries.** Variable-ize generic
instantiations and the enum/concrete boundary slots.

## Risk & method
This touches the core type system; a wrong global inference cascades worse than a
per-site one — hence the strict conservatism + per-phase measurement. Build
Phase 0 as pure scaffolding (provably zero-change) before any inference. The single
most important property is render/resolver **agreement** via the shared map.
