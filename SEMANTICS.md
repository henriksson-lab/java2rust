# Type semantics of the Java→Rust translator (formal)

A formal model of how types are assigned, propagated, and lowered. It exists to make
three things checkable: what each pass computes, the invariants every type-dependent
rewrite must preserve, and which open work is *forced* by the model.

Status tags: **[have]** built · **[partial]** subset · **[gap]** required, not built.
Section numbers and invariant labels (M0, N1–N3, R1/R2, U1/U2, P1) are stable — code
comments cite them.

---

## 0. Meta-rule

The translator is a *total, best-effort lowering* of valid Java to compilable Rust,
**not** a type checker — it never rejects. Hence:

> **M0 (capable bottom).** Undetermined types resolve to `Unknown = ⊥`, a *capable*
> placeholder (impls `Clone, Default, PartialEq/Eq/Hash/Ord, Display, Iterator`,
> arithmetic; coerces widely). `⊥` rarely *causes* an error; replacing it with a
> less-capable concrete type can *introduce* one. So the resolver is **monotone
> upward from ⊥ only on unambiguous evidence** (§9 U2); collapsing toward `⊥` is
> safe, splitting it is not (measured, §11).

---

## 1. Domains & lattice

```
J  ::=  Java source types  (int, String, List<Foo>, Foo[], Map<K,V>, X, Object, pkg.Outer.Inner)
R  ::=  Rust surface types (i32, String, Vec<T>, HashMap<K,V>, Box<dyn Tr>, Option<T>, paths, …::Unknown)
τ ∈ Ty ::=  Prim(i8|i16|i32|i64|usize|f32|f64|bool|char)
        |  Str | Vec(τ) | Set(τ) | Map(τ,τ) | Opt(τ)
        |  Named{path, args:[τ]}      -- user/stub struct or enum
        |  TraitObj(name)             -- Box<dyn Trait>
        |  Param(name)                -- generic parameter
        |  Var(α)                     -- Tier-2 unification variable
        |  Unknown                    -- ⊥
```

`Ty` is the common currency of resolution and codegen (`src/types.rs`). It is the
**underlying** type: it deliberately omits the two **overlays** — nullability
(`Option`-ness, §5) and ownership (`&`/`&mut`, §6) — which are decided per *site*,
not stored in the type (invariant **N3**).

**Lattice (Ty, ⊑).** `⊥ = Unknown`; `Var(α)` is a free leaf the solver (§9) maps to a
ground `τ` or `⊥`. There is **no subtyping order on `Named`** — Rust has no struct
subtyping, the single fact that forces §7. Join `⊔` exists only along the
`parent`/`interfaces` hierarchy (LUB, §9).

---

## 2. Pipeline

Per compilation unit (`src/lib.rs::translate`); `--crate` mode runs a project-wide
symbol-map pass first (§8).

```
build_project_map?           -- crate mode: the symbol map M (§8)
parse        : Source → AST
id_tracker   : AST → Names    -- resolution, imports, package
type_tracker : AST → Γ        -- Γ : Var ⇀ Ty  (declared types of locals/fields)
nullability  : AST → N        -- N : Decl → {nullable, nonnull}   (fixpoint, §5)
borrow       : AST → B0        -- param &/&mut facts                (§6)
dump         : (AST,Γ,N,B0,M) → Rust     -- typing (§4) + the render eqn (§10)
```

| pass | computes | §|
|---|---|---|
| `id_tracker` | name/decl/import resolution ||
| `type_tracker` | `Γ` (declared slot types) ||
| `nullability` | overlay `N` | 5 |
| `borrow` | param ownership facts `B0` | 6 |
| `crate_layout` | symbol map `M`: `rust_path, kind, parent, interfaces, fields, methods(ret,throws,nullable), dispatched, capability` | 8 |
| `dump`+`TypeResolver` | `Γ ⊢ e : τ` and the lowering | 4,10 |

**Two tiers of type determination.** Tier 1 (§4) is deterministic ground resolution
(`TypeResolver::type_of`); unknowns are `⊥`. Tier 2 (§9) introduces `Var(α)` for what
Tier 1 leaves open and solves by union-find + LUB.

### 2.1 Rewrite phases (where reformulation happens)

A rewrite changes the program; there are exactly two kinds, distinguished by *what*
they change and therefore *which semantics you reason in*:

| | **local / type rewrite** | **structural rewrite** |
|---|---|---|
| changes | one expression's type *representation* | the *statement structure* (introduce a binding, reorder, split, change a signature) |
| examples | `Option`-wrap, `Route` enum, `&T` borrow, `s.length()`→`.len()`, N2 fold | hoist `new B(new R(f))` into `let`s; try-with-resources → drop; `?`-propagation |
| mechanism | the render equation §10, syntax-directed at emit | a program-to-program transform with its own correctness criterion |
| reason in | `Ty` + the overlay composition (this doc) | *source* semantics (if Java-side) or *target* semantics (if Rust-side) |
| status | **[have]** | **[gap]** — no phase exists |

**Current architecture.** Emission is a single syntax-directed pass that prints Rust
**text** over the Java AST (`dump` + `Printer`); there is no Java-AST normalization pass
and no Rust AST. Hence **only local rewrites are expressible**: the dumper emits an
expression *in place* and cannot inject a preceding statement. This is exactly what §10
models — and all it models.

**Structural rewrites: pick the phase by its proof obligation.**
- **Java→Java normalization, BEFORE the dumper [preferred].** The canonical structural
  rewrite is **ANF/let-hoisting**: replace `outer(inner)` with `let t = inner; outer(t)`.
  It is *source-semantics-preserving* (naming a subexpression) **and universally valid in
  Rust** — a `let` binding owns `t`, so `outer(t)` moves it or `outer(&t)` borrows it; the
  intermediate always has a stable owner. So ANF-hoisting **categorically dissolves the
  stacked-constructor ownership problem** (`new BufferedReader(new FileReader(f))`) before
  ownership is even considered, and keeps the dumper + the render equation untouched —
  structural and type concerns stay orthogonal. (Whether a hoist is *needed* is target-
  side, so do it everywhere-or-by-idiom-table; always-hoist is correct but verbose.)
- **Rust-AST post-pass, AFTER the dumper [avoid].** Has the target types (knows exactly
  when to hoist) but needs a Rust AST (we emit strings) and ownership reasoning to decide
  hoisting ≈ reimplementing the borrow checker. Only justified once a Rust AST exists.
- **Target-driven structural changes** (e.g. `?`-propagation, which must flip the
  enclosing fn's return to `Result`) cannot be Java-side; they need a small target-aware
  step — the natural host for `docs/in-place-io-prototype.md`'s stubs-then-inline pass.

**The carrier escape [have].** The IO carriers (`Rc<RefCell<Box<dyn Read>>>`, §6/§3) are
the current *avoidance* of structural rewrites: by making every wrapper **own** its inner,
stacked construction composes without any separate binding — at the cost of non-idiomatic
output. "Carrier types" and "no structural-rewrite phase" are one decision seen twice.

---

## 3. Type lowering `⟦·⟧ : J → R`  (slot types: fields/params/returns/locals/args)

Compositional; reads `M` for user types.

```
⟦byte|short|int|long⟧ = i8|i16|i32|i64        ⟦float|double⟧ = f32|f64
⟦boolean⟧ = bool   ⟦char⟧ = char              ⟦String|CharSequence⟧ = String   (param: &str — §6)
⟦T[]⟧ = Vec<⟦T⟧>                              ⟦List<T>|…⟧ = Vec<⟦T⟧>
⟦Set<T>|…⟧ = HashSet<⟦T⟧>                     ⟦Map<K,V>|…⟧ = HashMap<⟦K⟧,⟦V⟧>
⟦Optional<T>⟧ = Option<⟦T⟧>                   ⟦Iterator<T>⟧ = JavaIter<⟦T⟧>
⟦Object|Class⟧ = Unknown        (M0; real-Object gap §7.4)
⟦interface I⟧ = Box<dyn I>  (owned)  |  &dyn I  (behind &)        [object-safety permitting]
⟦class C⟧ = M[C].rust_path   (project/dep)   |   stub Named / Unknown   (unresolved, §8.3)
⟦X⟧ = Param(X)
⟦raw List|Map⟧ = Vec|HashMap with element ← Tier-2 (§9), else E0107   [gap if unsolved]
```

**Raw-generic rule [have].** A raw *project* generic (`HuffmanTree` for `HuffmanTree<T>`)
is filled with `()` to its recorded arity. A raw *stdlib* collection has no arity source
→ element from Tier 2 or bare (`E0107`). Defaulting the element to `⊥` is **forbidden**:
`Vec<Unknown>` cascades (every concrete element use clashes) — §11.

A single renderer `ρ : Ty → R` (`ty_to_rust_string`, `dump.rs`) backs every "type to
string" need; do not hand-roll parallel derivers (invariant **P1**).

---

## 4. Resolve / Tier-1 typing  `Γ ⊢ e : τ`

`TypeResolver::type_of`, memoized. Inference rules (all unlisted forms ⊢ `⊥`, M0):

```
─────────────   ──────────────   ─────────────   ──────────────
⊢ intLit : I32   ⊢ strLit : Str   ⊢ boolLit:Bool   ⊢ charLit:Char     (…longLit,dblLit)

 x:τ ∈ Γ          ⊢ e : Vec(τ)∨Set(τ)        ⊢ e1:τ1  ⊢ e2:τ2  τ1,τ2 numeric
────────         ──────────────────         ───────────────────────────────
⊢ x : τ           ⊢ e[i] : τ                  ⊢ e1 op e2 : promote(τ1,τ2)

──────────────      ──────────────
⊢ (T)e : ⟦T⟧         ⊢ new C(..) : Named(C)

         ⊢ scope : τr      m,args
        ────────────────────────────
         ⊢ scope.m(args) : mret(τr,m,args)
```

`mret` (= `method_call_type`) tries, in order: (1) fixed stdlib reach
(`Map.get→V`, `List.get→elem`, `size→i32`, `charAt→char`, …); (2) project/linked
return via `M`, walking `parent`; (3) self/inherited; (4) the **return tables**
`StdRule.ret` + `runtime_method_ret(javaType,name,arity)` (keyed on the *Java* name,
e.g. `Random.nextInt→i32`). Receivers are typed **recursively**, so chains
`a.f().g()` resolve once the return facts exist.

```
mret(τr, m, args):
  for rule in [stdlib_reach, project_ret(M), self_ret, return_tables]:
     if (t = rule(τr,m,args)) ≠ ⊥: return t
  return ⊥
```

**Member fields.** `x:τ∈Γ` covers locals/params only. A bare/`this.`/inherited
*field* is not a lexical name → resolved against `M` by walking `currentClass`+`parent`
(`resolve_self_field_type`); a nullable field resolves to `Opt(τ)` so the overlays see
the truth (never the bare `τ` — would misfold N2). Closing this `⊥`-gap was −151.

**Overload resolution [have].** Key `name#arity`; on shared arity pick by
argument-type score (collection-vs-scalar shape must match); unique best wins, ties →
base overload. (Unconditional base-overload was a bug, §11.)

**P1 — one resolver, consulted not re-derived.** The shallow dispatch helpers
(`recv_type_name`, `callee_recv_type`) must route through `TypeResolver` for cases
they can't handle (a `MethodCallExpr` receiver), not spawn a third inference path.
Caveat: routing a `Named` result flips `receiver_is_user_type` and changes the
stdlib-vs-user rewrite split → keep `recv_type_name`'s fallback **String-only**;
`callee_recv_type` may use full `Named` (it only shapes a signature).

---

## 5. Overlay `Option^{N}`  (nullability)

`N : Decl → {nullable, nonnull}` is a fixpoint over fields/locals/params/returns/elements.
As an operator on the rendered type, indexed by the slot's `N`:

```
Option^{nonnull}(τ)  = τ
Option^{nullable}(τ) = Option<τ>
```

Site lowering (ι = inject, π = project):

```
write v into nullable slot      ⟼  Some(v) / None          (ι: emit_into_option)
read nullable in plain position ⟼  v.{as_ref()|clone()}.unwrap()   (π, borrow form per §6)
x ==|!= null,  x:Opt            ⟼  x.is_none() / x.is_some()
x ==|!= null,  x:concrete       ⟼  false / true            (N2 fold)
```

**N1 (all-sites duality).** `π ∘ ι = id`; the wrap and the unwrap must be applied
**together over all sites of a decl**. A one-sided application (Some-wrap a write, read
unprojected — or vice versa) makes a site where static-type ≠ expected-type → `E0308`.
Field nullability is resolved shadow-safe (a same-named param must not be consulted);
inherited via `parent`.

**N2 (null-compare fold) [have, −98].** `x ==|!= null` with `⊢ other : τ` concrete
(`τ ∉ {Opt, ⊥}`) folds to `true`/`false` (the value can't be null) instead of emitting
`.is_some()` on a non-`Option` (`E0599`). Sound because a genuinely-nullable operand
resolves to `Opt`/`⊥` and keeps the check — *iff* member fields resolve nullable→`Opt`
(§4).

**N3 (overlays ∉ Ty).** `type_of` returns the *underlying* τ — identical for a nullable
local `String x` (emitted `Option<String>`) and a non-null one. So `τ`-concreteness is a
sound gate **only where being wrong stays well-typed** (N2: a folded bool always type-
checks). It is **not** sound for deciding the nullable-read projection (`.unwrap()` /
`.as_ref().unwrap()`): that fires on every nullable read incl. locals, where
`τ`-concrete ≠ "not Option-wrapped". The only signal for "emitted as `Option`?" is `N`
itself. *Measured:* gating the unwrap on `τ`-concrete **+2503** (NO-GO). The residual
`unwrap`/`is_some`-on-concrete cluster (~32) is exactly `N`-says-nullable-but-emitted-
concrete; fix at the source (make `N` ⇔ emission, §12-item-7), never a per-read `τ` gate.

---

## 6. Overlay `Borrow`  (ownership)

A **fixed** strategy (not inferred lifetimes). `Borrow : Ty × Pos → R`:

```
Copy/scalar               ⟼ by value
class | array | String    ⟼ &T  (param position)        String param ⟼ &str/&String
mutated param             ⟼ &mut T    (from B0)
```

`print_arguments_linked` shapes each argument to the callee's `ParamSym(by_ref,
mutable, nullable)`. **No structural rewrites to satisfy the borrow checker** — residual
borrow seams are accepted as errors (§12), not chased.

### 6.1 Use-site borrow analysis (clone reduction)  [partial — landed slices]

`.clone()` is the symptom of **under-borrowing**: the translator owns at "move
positions" (`emit_moved_value`: return / assign-RHS / var-init / by-value arg), but
most positions only *read* — and a read needs a borrow. The principle:

> **own only at a genuine move; borrow at every read.** A value used N times, all
> reads, needs zero owned copies.

Model the decision as a per-use classifier:

```
classify : Use → { Move, ReadBorrow, MutBorrow }
emit(Move)       = owned value     (clone iff non-Copy read out of a borrow)
emit(ReadBorrow) = &T              (.as_ref().unwrap() for a nullable read; &expr otherwise)
emit(MutBorrow)  = &mut T          (.as_mut())
```

Verdict by parent context (those marked ✓ landed; each gated + measured):

| use context | verdict | note |
|---|---|---|
| format!/println! arg | ReadBorrow | macro auto-refs; a clone is always spurious |
| read-only `&self` method receiver | ReadBorrow ✓ | whitelist `is_readonly_java_method` |
| `==`/`!=` operand | ReadBorrow ✓ | **coordination law** below |
| index base `arr[i]` **read**, element Copy | ReadBorrow ✓ | non-Copy element NO-GO (coercion cascade) → keep clone |
| `Map.get(k)`, Copy value | (`.copied()`) ✓ | a free copy, not a marked clone |
| `Map.get(k)`, non-Copy, read-only receiver | ReadBorrow ✓ | `.get(&k).unwrap()` → `&V` |
| foreach iterable: owned-temp or last-use local | Move-without-clone ✓ | consume directly; field/param keeps clone |
| index/foreach **write** target | MutBorrow [gap] | `.as_mut()`; today clones (lost-mutation bug) |
| store-owned / return-owned / by-value arg | Move | genuine move — keep |

**Coordination law (k-ary operators).** When emitting an operator whose operands are
compared/combined by value (`==`, `!=`), **all operands must render at the same borrow
depth**. Borrowing one operand to `&T` while the other is owned `T` is `&T == T` →
ill-typed. Emission: if any operand is a borrowable nullable read, render *both* as
`&T` (nullable side `.as_ref().unwrap()`; other side `&(expr)`), giving `&T == &T`
(needs only `T: PartialEq`, same as owned `==`). *Measured:* the uncoordinated form
regressed +28; the coordinated form is flat.

The classifier is currently a set of context predicates (`use_is_read_borrow`,
`is_copy_index_base`, `cmp_borrow`, …); unifying them into one `classify` is the open
refactor (§12). **Borrowed returns are CLOSED** (§6.2).

### 6.2 Borrowed returns / lifetimes — CLOSED (measured NO-GO for clones)

A getter `&self → &T` (elided lifetime) avoids the callee clone, but moves it to *every
caller that consumes the result owned*. Probe: getter→`&T` produced **0** new borrow-
checker errors and **−48 errors**, but clones **+316 jts / +23 jsoup** (callers mostly
consume by value). Net clone reduction needs whole-program **caller-read-dominance**
analysis — the global ownership inference the fixed strategy avoids. *Applyable only if
errors, not clones, are the target.* Do not reopen for clone reduction.

---

## 7. Inheritance & polymorphism  (no `Named` subtyping → two lowerings)

### 7.1 Composition `[have]`
`C extends P` → struct with `base: ⟦P⟧`; `c.f` (inherited) → `c.base.…f`; methods
resolve up the `parent` chain. Correct for member access but **erases dynamic type**
(a `P`-slot holds only `base`), so `instanceof`/downcast fail — §7.2 fixes that for
dispatched hierarchies.

### 7.2 Overlay `Route` — sealed-hierarchy enums (R4) `[have, dispatched]`
A root `P` is **dispatched** iff some member is an `instanceof`/cast target in-project
(`M.dispatched`). For each, synthesize:

```
enum PKind { C1(C1), …, P(P) }   impl Deref{Target=P}   impl Default,Clone,Eq,Hash (§8.4)
```

`Route` is the overlay selecting the enum at polymorphic slots:

```
Route(τ at slot) = Named(PKind)   if decl-type(slot) is a dispatched-hierarchy supertype with ≥1 project subtype
                 = τ              otherwise   (leaf stays concrete)
```

Keyed by *declared* type (`slot_enum_name`), consulted at render (`visit_class_type`)
**and** in `type_of` (so `coll.get()` yields `PKind`). Site lowering (ι/π):

```
ι  construct v:C into routed slot  ⟼ PKind::C(v)     [have: return, decl, const, assign, add/put, arg, arr-elem]
π  read already-routed value       ⟼ as-is           (DO NOT re-wrap → double-wrap)   [have]
   instanceof C                    ⟼ matches!(v, PKind::C(_)|…descendants)            [have]
   (C)v  downcast                  ⟼ match v { PKind::C(x)=>x, _=>unreachable!() }     [have]
   method m on PKind              ⟼ base→Deref; intermediate→per-variant match        [partial]
```

**R1 (no seam).** A leak (`expected C, found PKind` or reverse) occurs *exactly* at a
boundary between a routed and an un-routed position. Cure = **universal routing**: every
position of a dispatched type, incl. `C[]` → `Vec<PKind>`, must route. Universal enough
for *contained* hierarchies (vcf); a *pervasive value type* (`Coordinate`, also in
`Coordinate[]`) has residual seams `[partial]` (§12).
**R2 (monotone wrap).** Never render `PKind` at a slot without wrapping its writes and
ensuring its reads are `PKind`. (Same shape as N1.)

### 7.3 Why enums, not trait objects
Closed (all subtypes in-project) + value semantics (`Hash`/`Eq` for `HashSet` keys),
which a struct-enum gives and `Box<dyn>` does not. Trait objects remain for *open*
supertypes `[partial]`.

### 7.4 `java.lang.Object` `[gap]`
`⟦Object⟧ = ⊥` today. Heterogeneous `Map<String,Object>` with `instanceof` dispatch
wants a *synthesized* tagged enum of observed variants (§7.2 over a collected, not
declared, hierarchy); open-world fallback `Box<dyn Any>` + downcast.

---

## 8. Symbol map `M`  (cross-module resolution)

`build_project_map` links the project to itself; dependency maps merge.
`resolve(name,imports,wildcards,pkg)`: explicit import → same package → wildcard → FQN
→ nested-by-simple-name within package (gated).

- **8.1 `TypeSym`** carries `rust_path, kind∈{struct,trait,enum}, parent, interfaces,
  generics, fields, statics, methods(ret,throws,nullable), capability`.
- **8.2 Acyclicity.** `break_parent_cycles` severs cyclic `parent` at build; every
  chain-walk is additionally cycle-guarded. (A cycle once hung the translator.)
- **8.3 Stubs.** A type absent from `M` → stub: capability-free aliases to `⊥`;
  method/field-bearing → named stub struct with `⊥` members.
- **8.4 Eq/Hash capability fixpoint [have].** Monotone fixpoint marks each type
  `partial_eq_capable`/`eq_hash_capable` iff every field (incl. `base`) is. `Map`/`Set`
  fields hash via an order-independent fold (std maps aren't `Hash`). Enables the §7.2
  enum to be `HashSet`-keyable.

---

## 9. Tier-2 unification  `[partial — substrate done]`

For positions Tier 1 leaves open (raw-collection elements, stub returns):

```
1. Var:        each unknown slot ← Var(α)
2. Constrain:  coll.add(x)|get|foreach ⟹ α = ⊢x        a=b (colls) ⟹ elem(a)=elem(b)
               arg/return            ⟹ α = elem(param/ret)   new ArrayList<Foo> ⟹ α=Foo
3. Solve:      union-find; class → ground member, joined by LUB over parent/interfaces
               (.add(Polygon)+.add(LineString) ⟹ Geometry, not a sample); else ⊥
4. Apply:      one shared  slot→Ty  map, read by BOTH renderer and resolver
```

**U1 (render/resolver agreement).** The inferred element MUST be applied at the render
*and* in `type_of` of every use. Guaranteed by the shared map; a per-declaration attempt
lacking it cascaded (render `Vec<Shape>`, `.get()` still `⊥`).
**U2 (monotone).** Resolve a `Var` to ground only on unambiguous single-LUB evidence;
conflict/none ⟹ stay `⊥`/bare. (Shipped: leaf-element locals only — §12.)

**Fusion R4×Tier-2 [gap].** When an element LUB is a dispatched root `P`, the element is
`PKind` → `Vec<PKind>`, `.add` wraps, `.get` reads the enum. Unifies §7.2 and §9; blocked
only by §7.2's non-universal routing (R1).

---

## 10. The render equation  (the central object)

A slot's Rust type is the **composition of the four overlays over the ground type**:

```
render(slot)  =  Borrow ∘ Option^{N(slot)} ∘ Route ∘ Resolve  (decl-type(slot))
                 └──§6──┘ └─────§5─────┘   └─§7.2─┘ └§4⊕§9┘
```

The overlays are **orthogonal endofunctions** (each = identity unless its condition
holds) applied in this fixed order; each obeys its own all-sites invariant.

**Composition is not branching [have].** Where two overlays co-occur they must *nest*:
a concrete member into a nullable routed slot is `Some(PKind::C(v))` =
`(Option^N ∘ Route)(v)`, via `emit_into_option_enum` — **not** "Option *or* Route".
Treating co-occurring overlays as mutually-exclusive branches is itself a seam.

---

## 11. Invariants & lessons

**The contract** (every type rewrite must keep; each is a face of one law —
*a representation change must be applied uniformly across all sites of a value*):

| id | statement |
|---|---|
| M0 | unknown ⟹ `⊥` (capable); never downgrade `⊥` to a less-capable type |
| N1,R2,U2 | **all-sites monotonicity**: update writes *and* reads together, or don't fire |
| U1 | render(slot) and `type_of`(its uses) agree — via the shared map, never two guesses |
| N3 | overlays ∉ `Ty`: gate on `type_of`-shape only where being wrong stays well-typed |
| P1 | consult the one resolver; don't spawn a parallel derivation |
| §6 | no structural rewrites for borrowing; coordination law for k-ary ops |
| §8.2 | acyclic `parent` + cycle-guarded walks |

**Measured NO-GOs** (each violated all-sites or N3; kept as a fence):

| change | result | why |
|---|---|---|
| unwrap gated on `type_of`-concrete | **+2503** | N3: `τ`-concrete ≠ not-Option for locals |
| N2 fold gated on `expr_nullable` not `τ` | +68 | surfaces `N`≠emission inconsistency as `E0599` |
| borrowed returns (for clones) | clones +339 | callers consume owned; needs global analysis (§6.2) |
| uncoordinated `==` operand borrow | +28 | `&T == T` (coordination law) |
| unconditional index-base borrow | +7 | non-Copy element coercion cascade (→ Copy-gate) |
| `Vec<Unknown>` default element | cascades | every concrete element use clashes (§3) |
| split `⊥` into bare distinct types | regressed | M0 |

**Measured wins** (the lever each pulled): member-field `type_of` (−151) · method-return
tables (−170) · N2 fold (−98) · over-wrap fix (−1586) · overload-by-arg-score (−361) ·
use-site borrow slices (clones −708, this frontier).

---

## 12. Roadmap (gaps = what the model demands but we don't compute)

In dependency order:

1. **Universal routing (R1)** `[partial]` — route *every* position of a dispatched type
   (incl. `C[]`→`Vec<CKind>`, borrow seams `&CKind`). Closes R4's pervasive-value leaks;
   precondition for the fusion.
2. **R4×Tier-2 fusion (§9)** `[gap]` — `List<Geometry>` → `Vec<GeometryKind>`. Unblocks
   the JTS `E0107` mass.
3. **Full Tier-2 solve (§9)** `[partial]` — cross-slot union-find (field↔param↔return↔
   local) + LUB beyond the leaf-local slice.
4. **`N` ⇔ emission consistency (§5 N3)** `[gap]` — a flagged-nullable value always
   emitted `Option<T>` (or dropped from `N`). Closes the ~32 unwrap-on-concrete cluster
   at the source. *Tractable, high-leverage.*
5. **Unified `classify` (§6.1)** `[partial]` — one use-site classifier + the MutBorrow
   write-target form (`.as_mut()`, a real lost-mutation bug). Unblocks in-place IO
   (a `Box<dyn BufRead>` reader is a MutBorrow local — see `docs/in-place-io-prototype.md`).
6. **Intermediate-method delegation (§7.2)** `[partial]` — enum methods for supertype
   APIs not covered by `Deref`.
7. **Real `Object` (§7.4)** `[gap]` · **generic trait objects** `[gap]` (`Box<dyn Tr<S,C>>`).

**Non-goals** (the model forbids): infer borrows, reject programs, downgrade `⊥`. The
single through-line: *make every type decision consistent across all sites of a value* —
N1/R1/R2/U1/U2 are five faces of it; universal routing + Tier-2 unification is its general
form.
