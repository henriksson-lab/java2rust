# Type semantics of the Java→Rust translator

A semi-formal specification of how the translator assigns, propagates, and lowers
types. The goal is not full rigor but a *shared model* precise enough to (a) say
what each pass is responsible for, (b) state the invariants every type-dependent
rewrite must preserve, and (c) make the remaining work fall out as "what the model
requires but we don't yet compute."

Status legend in the text: **[have]** implemented; **[partial]** implemented for a
subset; **[gap]** required by the model, not yet built.

---

## 0. Orientation

The translator is **not** a type checker that rejects programs; it is a
*best-effort lowering* of already-valid Java into Rust that should compile. Every
type decision is therefore governed by one meta-rule:

> **M0 (best-effort + capable bottom).** When a type cannot be determined, resolve
> it to `Unknown` — a *capable* placeholder (it implements `Clone`, `Default`,
> `PartialEq/Eq/Hash/Ord`, `Display`, `Iterator`, and arithmetic ops, and coerces
> widely). Callers fall back to their existing behaviour rather than guess. Being
> capable, `Unknown` rarely *causes* an error; replacing it with a less-capable
> concrete type can *introduce* errors. (Empirically: collapsing toward `Unknown`
> is safe; splitting it into distinct bare types regressed — see §11 *Lessons*.)

---

## 1. Domains

- `J` — Java source types (syntactic: `int`, `String`, `List<Foo>`, `Foo[]`,
  `Map<K,V>`, type variables, `Object`, nested/qualified names).
- `R` — Rust surface types (`i32`, `String`, `Vec<T>`, `HashMap<K,V>`,
  `Box<dyn Tr>`, `Option<T>`, user paths, `crate::…::Unknown`).
- `Ty` — the internal type IR (`src/types.rs::Type`), the common currency between
  resolution and codegen:

```
Ty ::= Prim(i8|i16|i32|i64|usize|f32|f64|bool|char)
     | Str
     | Vec(Ty) | Set(Ty) | Map(Ty,Ty) | Opt(Ty)
     | Named { path, args:[Ty] }     -- a user/stub struct or enum
     | TraitObj(name)                -- Box<dyn Trait>
     | Param(name)                   -- a generic type parameter
     | Var(u32)                      -- a unification variable (Tier 2)
     | Unknown                       -- bottom (M0)
```

`Ty` deliberately omits two things, handled as **orthogonal overlays** (§5, §6):
nullability (`Option`-ness) and ownership (`&`/`&mut`). `Ty` is the *underlying*
type; the overlays decide how it is wrapped at a given site.

**Lattice.** `Unknown` is bottom. There is no general subtyping order on `Named`
(Rust has no struct subtyping — this single fact drives §7). `Var(α)` is an
unresolved leaf that the Tier-2 solver replaces with a ground `Ty` or `Unknown`.

---

## 2. Pipeline & who decides what

For each compilation unit (`src/lib.rs::translate`):

```
parse → id_tracker::run → type_tracker::run → nullability::analyze → [borrow] → dump
```

and, in `--crate` mode, a project-wide pre-pass `crate_layout::build_project_map`
runs first, producing the **symbol map** that all units link against (§8).

| pass | decides |
|---|---|
| `parse` | AST (`Arena`) |
| `id_tracker` | name resolution, declarations, imports/package |
| `type_tracker` | declared types of locals/fields (the `Γ` environment) |
| `nullability` | the `N(·)` overlay: which declarations are nullable (§5) |
| `borrow` | the ownership overlay: which params are `&`/`&mut` (§6) |
| `crate_layout` | the symbol map: per-type `rust_path`, `parent`, `interfaces`, fields, methods, the `dispatched` set, eq/hash capability (§8, §9) |
| `dump` + `TypeResolver` | ground typing `Γ ⊢ e : Ty` (§4) and lowering to `R` (§3) |

**Tiers.** Type determination is two-tiered:
- **Tier 1 — deterministic ground resolution** `[have]`: `TypeResolver::type_of`
  computes a ground `Ty` from declared types + literals + a fixed set of stdlib/
  method-return rules. No guessing; unknowns are `Unknown`.
- **Tier 2 — unification** `[partial]`: type *variables* for positions Tier 1
  leaves unknown (collection elements, stub returns), equality constraints from
  dataflow, union-find solve. The substrate exists; see §10.

---

## 3. Ground translation `⟦·⟧ : J → R`

The lowering of a *type* (used for slots: fields, params, returns, locals, type
args). Defined compositionally; `⟦·⟧` reads the symbol map for user types.

```
⟦byte|short|int|long⟧      = i8|i16|i32|i64
⟦float|double⟧             = f32|f64
⟦boolean⟧ = bool   ⟦char⟧ = char
⟦String|CharSequence⟧      = String           (param position may use &str — §6)
⟦T[]⟧                      = Vec<⟦T⟧>          [have]
⟦List<T>|ArrayList<T>|…⟧   = Vec<⟦T⟧>
⟦Set<T>|…⟧                 = HashSet<⟦T⟧>      (LinkedHashSet → also HashSet)
⟦Map<K,V>|…⟧               = HashMap<⟦K⟧,⟦V⟧>
⟦Optional<T>⟧              = Option<⟦T⟧>
⟦Iterator<T>⟧              = crate::java_runtime::JavaIter<⟦T⟧>
⟦Object|Class⟧             = Unknown            (M0; see §7.4 for the real-Object gap)
⟦interface I⟧ (owned pos)  = Box<dyn I>         [have, object-safety permitting]
⟦interface I⟧ (behind &)   = &dyn I
⟦class C⟧ (project/dep)    = the symbol map's rust_path for C
⟦class C⟧ (unresolved)     = a stub Named, or Unknown (§8.3)
⟦type var X⟧               = X (Param)
⟦raw List / Map (no args)⟧ = Vec / HashMap with **missing** element → `E0107`
                             unless an element is inferred (Tier 2, §10) [gap]
```

**Raw-generic rule [have].** A *project* generic type used raw (`HuffmanTree` for
`HuffmanTree<T>`) is filled with `()` placeholders to the recorded arity. A *stdlib*
collection used raw has no arity source → bare `Vec`/`HashMap` (`E0107`) unless
Tier 2 supplies the element. Defaulting the element to `Unknown` is **forbidden**:
`Vec<Unknown>` cascades (every concrete element use clashes) — see §11.

---

## 4. Expression typing (Tier 1): `Γ ⊢ e : Ty`

`TypeResolver::type_of` (memoized per node). Selected rules (the rest bottom out at
`Unknown` per M0):

```
literals:  intLit:I32  longLit:I64  dblLit:F64  charLit:Char  boolLit:Bool  strLit:Str
name x:    Γ(x)                              (declared type via type_tracker)
e[i]:      Ty where Γ⊢e:Vec(Ty)∨Set(Ty)     (array/list element)
cast (T)e: ⟦T⟧                              (the cast *target*)
new C(..): Named(C)
e1 op e2 (arith): promote(Ty1,Ty2)           (both numeric ⇒ wider; else Unknown)
e.m(args): method_call_type(recv, m, args)   (below)
```

`method_call_type` resolves, in order: (1) a fixed stdlib reach
(`Map.get→V`, `List.get→elem`, `Optional.get→T`, `size→i32`, `charAt→char`,
String-returning String methods, boxed-number unboxing, `Math` float fns…);
(2) a **project/linked** method's recorded return type when the receiver is
`Named` (via the symbol map, walking the `parent` chain — §8); (3) a self/inherited
call. Else `Unknown`.

**Overload resolution [have].** A scoped call resolves to the receiver type's
method keyed `name#arity`; when several overloads share an arity, pick by
*argument-type score* over the candidates (collection-vs-scalar shape must match),
unique best wins, ties fall back to the base overload. *(Resolving by base overload
unconditionally was a bug — it mis-targeted every shared-arity overload.)*

The `Ty` query methods (`numeric_rust`, `category`, `is_char`, `elem`,
`map_value`, …) are how codegen consults a resolved type; they are the single
source of truth replacing the old scattered ad-hoc derivers.

---

## 5. Nullability overlay `N(·)` (orthogonal to `Ty`)

`nullability::analyze` computes a fixpoint `N(d) ∈ {nullable, non-null}` over
declarations (fields, locals, params, returns, array elements). Lowering:

```
slot of underlying ⟦T⟧ with N=nullable     ⟼  Option<⟦T⟧>
value v into a nullable slot                ⟼  Some(v) / None        (emit_into_option)
read of a nullable value in a plain pos.    ⟼  v.clone().unwrap()
x == null / x != null                       ⟼  x.is_none() / x.is_some()
```

**Invariant N1 (all-sites).** Wrapping and unwrapping are dual and must be applied
*together* across all sites of a declaration; a partial application (`Some`-wrap a
write without unwrapping the read, or vice versa) *adds* `E0308`. Field nullability
is resolved against the class's own field set (`this.field`), **shadow-safe** (a
same-named param must not be consulted). Inherited-field and inherited-getter
nullability resolve via the symbol-map `parent` chain.

---

## 6. Ownership overlay (orthogonal to `Ty`)

A fixed, deliberate borrow strategy (NOT inferred lifetimes):

```
scalars / Copy types         ⟼  by value
classes, arrays, String      ⟼  by reference (& / &mut)   in param position
&mut                         ⟼  when the borrow analysis sees the param mutated
String param                 ⟼  &str / &String (callee-shaped)
```

`print_arguments_linked` shapes each argument to the callee's recorded `ParamSym`
(`by_ref`, `mutable`, `nullable`). **We do not rewrite program structure to satisfy
the borrow checker** — borrow seams that remain are accepted as residual errors
(§12), not chased with ownership rewrites.

**`.clone()` is a symptom of under-borrowing [gap — future refinement].** The
translator inserts an owning `.clone()` at "move positions" (`emit_moved_value`:
return / assignment-RHS / var-init / by-value arg of a non-`Copy` value). Each such
clone is marked `/* TODO(translation): validate added clone */` (README) because it
can break Java's by-reference aliasing or be wasted allocation. But many of these
positions are **not actually moves** — they only *read* the value, and a read needs
a borrow, not ownership:

- **`format!`/`println!`/`write!` arguments never move** — the formatting machinery
  takes args by shared reference (`&dyn Display`). So a format-position value needs
  *neither* a clone *nor* an explicit `&` (the macro auto-refs); `format!("{} {}",
  s, s)` is already correct. Any clone emitted for a format arg is *always* spurious.
- Comparison operands, `if`/`while` conditions, and read-only method receivers
  likewise only borrow.

So the model: **own only at a genuine move** (store into an owned slot, return
owned, pass to a by-value param); **borrow at every read.** A value used N times,
all reads, needs zero owned copies. The current eager-clone strategy over-owns; a
*use-site borrow analysis* (classify each use as read-borrow vs move) would
eliminate most marked clones (and the audit burden), and likely also closes some of
the §7.2 borrow seams (which are the same "wrong borrow shape at a use" problem).
Under-borrowing now *forces* clones later; fixing the borrow aggressiveness is the
root, the clones the symptom. **Investigate after the R1/Tier-2 routing work.**

---

## 7. Inheritance & polymorphism

Java single-inheritance + interfaces, lowered two ways.

### 7.1 Composition (the base representation) `[have]`
A subclass `C extends P` becomes a struct with a `base: ⟦P⟧` field; inherited
member access `c.f` lowers to `c.base.…f`. Method inheritance resolves up the
symbol-map `parent` chain. This is correct for member access but **erases dynamic
type**: a `P`-typed slot holds only the `base` struct, so `instanceof`/downcast on
it are impossible — which §7.2 fixes for *dispatched* hierarchies.

### 7.2 Sealed-hierarchy enums (R4) `[have, for dispatched hierarchies]`
For a hierarchy whose root `P` is **dispatched** (some member is an `instanceof`/
cast target anywhere in the project — recorded in `symbol_map.dispatched`),
synthesize one enum per root:

```
enum PKind { C1(C1), C2(C2), …, P(P) }     -- one variant per concrete member
impl Deref for PKind { Target = P; … }     -- base methods via deref
impl Default, Clone, PartialEq, Eq, Hash   -- derives gated on member capability (§8.4)
```

**Routing predicate `route(slot)`** — a slot typed as a *polymorphic supertype*
of a dispatched hierarchy renders as the enum, not the concrete:

```
route(slot) = PKind      if  decl-type(slot) is a hierarchy supertype with ≥1 project subtype
            = concrete   otherwise   (a leaf type stays concrete)
```

`route` is keyed by the *declared* type (via `slot_enum_name`), consulted at type
rendering (`visit_class_type`) **and** by the resolver (so `coll.get()` etc. yield
`PKind`). Boundary rules (the value↔slot seam):

```
construct: concrete subtype value v : C  into a routed slot   ⟼  PKind::C(v)   (wrap)
   positions [have]: return, declarator, const-field, assignment, collection add/put,
                     method argument, array-element assignment
read: a read of an already-routed value (name/field/index/method-result, thru parens)
                     is already PKind ⟼ DO NOT wrap (else double-wrap)            [have]
instanceof C:        matches!(v, PKind::C(_) | …descendants…)                     [have]
(C) v  (downcast):   match v { PKind::C(x) => x, _ => unreachable!() }            [have]
method m on PKind:   base method ⟶ Deref; intermediate-only method ⟶ per-variant
                     match-delegation (unreachable! for variants lacking it)      [partial]
```

**Invariant R1 (no seam).** A leak (`expected C, found PKind` or the reverse)
occurs exactly at a boundary between a *routed* and an *un-routed* position. The
cure is **universal routing**: every position of a dispatched type — incl. arrays
`C[]` ⟶ `Vec<PKind>` — must route, so a routed value never meets a concrete slot.
Routing is currently universal enough for *contained* hierarchies (vcf
`VCFHeaderLine`); a *pervasive value type* (`Coordinate`, also used in `Coordinate[]`
and plain slots) still has residual un-routed seams `[partial]` (§12).

**Invariant R2 (monotone wrap).** Never emit `PKind` at a slot without also wrapping
its writes and ensuring its reads are `PKind`. Partial wrapping cascades.

### 7.3 Why enums (not trait objects) for closed hierarchies
The hierarchy is closed (all subtypes in-project) and elements need value semantics
(`Hash`/`Eq` to key a `HashSet`), which a struct enum gives and a `Box<dyn>` does
not. Trait objects remain the tool for *open*/method-bearing supertypes `[partial]`.

### 7.4 `java.lang.Object` `[gap]`
Currently `⟦Object⟧ = Unknown`. The genuine heterogeneous-`Object` case (a
`Map<String,Object>` whose values are `instanceof`-dispatched) wants a *synthesized
tagged enum* of the observed variants (no Java equivalent); the open-world fallback
is `Box<dyn Any>` + downcast. This is the same enum machinery as §7.2 applied to a
collected, rather than declared, hierarchy.

---

## 8. Cross-module resolution (the symbol map)

`crate_layout::build_project_map` links the project against itself; dependency maps
merge in. `LinkIndex::resolve(name, imports, wildcards, package)` resolves a Java
name to a `TypeSym` by: explicit import → same package → wildcard → bare FQN →
**(5)** a *nested* type by simple name within the package (`pkg.Outer.Inner`), the
last gated so it can't bind an unrelated dependency.

### 8.1 `TypeSym` carries
`rust_path`, `kind` (struct|trait|enum), `parent` (FQN), `interfaces`, generics,
fields, static fields, methods (with recorded return types + `throws` + nullability),
and the capability flags (§8.4).

### 8.2 Parent chains are acyclic by construction
`break_parent_cycles` severs any cyclic `parent` link at build time (a class can't
transitively extend itself); every chain-walk (method/field/enum resolution) is
additionally cycle-guarded. *(A cyclic chain previously hung the translator.)*

### 8.3 Stubs (external/unresolved types) `[have]`
A type not in any map becomes a stub: a capability-free one is aliased to `Unknown`
(M0); a method/field-bearing one is a named stub struct with `Unknown`-typed members.

### 8.4 Eq/Hash capability fixpoint `[have]`
A monotone fixpoint marks each project type `partial_eq_capable` / `eq_hash_capable`:
true iff every field (incl. the `base` chain) is comparable/hashable. A `Map`/`Set`
field is hashable via an **order-independent fold** (mirrors Java `AbstractMap.
hashCode`), since std maps aren't `Hash`. This lets subtypes and map-bearing value
types get hand-written `impl Hash/Eq` (a plain derive can't), which is what makes
the §7.2 enum keyable in a `HashSet`.

---

## 9. Tier-2 unification `[partial — substrate done, solve partial]`

For positions Tier 1 leaves unknown — primarily **collection element types** of raw
declarations, and stub-method returns:

1. **Variables.** Assign `Var(α)` to each unknown slot (e.g. a raw collection's
   element).
2. **Constraints (equality).** From dataflow:
   `coll.add(x)`/`get`/for-each ⟹ `α = Ty(x)`; `a = b` between collections ⟹
   `elem(a) = elem(b)`; arg-pass/return ⟹ `α = elem(param/return)`; initializer
   `new ArrayList<Foo>()` ⟹ `α = Foo`.
3. **Solve.** Union-find; a class resolves to its ground member, joined by **LUB
   over the `parent`/`interfaces` hierarchy** when several ground samples appear
   (`.add(Polygon)`+`.add(LineString)` ⟹ `Geometry`, not a sample), else `Unknown`.
4. **Apply.** A shared `slot→Ty` map consulted by **both** the renderer and the
   resolver.

**Invariant U1 (render/resolver agreement).** The inferred element MUST be applied
at the type render *and* in `type_of` of every use (`.get`, iteration). The
substrate (a shared map) guarantees this — *the per-declaration attempt that lacked
it cascaded* (render said `Vec<Shape>` while `.get()` still typed `Unknown`).

**Invariant U2 (monotone).** Resolve a variable to ground *only* on unambiguous
single-LUB evidence; conflict / no evidence ⟹ stay `Unknown`/bare. Never emit a
contested element. (Currently shipped: leaf-element locals only — §12.)

**The fusion (R4 × Tier-2)** `[gap]`: when a collection's element LUB is a *dispatched*
hierarchy root `P`, its element type is `PKind` (§7.2), so `Vec<PKind>`, `.add`
wraps into a variant, `.get` reads the enum. This is the unification of §7.2 and §9
into one system — blocked only by §7.2's residual non-universal routing (R1).

---

## 10. The judgmental shape, summarized

A slot's final Rust type is a composition of the overlays over the ground type:

```
render(slot) = Borrow( Option^{N(slot)} ( Route( Resolve(decl-type(slot)) ) ) )
```

where `Resolve` is Tier-1 ⊕ Tier-2 (§4, §9), `Route` is the enum overlay (§7.2),
`Option^N` is nullability (§5), `Borrow` is ownership (§6). The overlays are
*orthogonal* and applied in this order; each must obey its all-sites invariant
(N1, R1/R2, U1/U2).

**Composition is not optional [have].** The overlays must *nest*, not branch: a
concrete member into a nullable routed slot is `Some(Kind::V(v))`
(`emit_into_option_enum`) — `Option^N ∘ Route`, not "Option *or* Route". Treating
two overlays as mutually-exclusive branches is itself a seam (it produced
`expected Kind, found Concrete` until composed). Any pair of overlays that can
co-occur at a slot must compose at that slot.

---

## 11. Invariants (the contract every type rewrite must keep)

- **M0** — unknown ⟹ `Unknown` (capable bottom); don't downgrade `Unknown` to a
  less-capable type without replacing *all* its capabilities.
- **N1, R2, U2 — all-sites monotonicity.** A type-representation change (Option,
  enum, inferred element) must update writes *and* reads together, or not fire.
  A one-sided change cascades into more errors than it fixes. *Every measured NO-GO
  this project hit was a violation of this.*
- **U1 — render/resolver agreement.** Whatever type a slot renders as, `type_of`
  of its uses must agree. Achieved by a shared map, never by two independent guesses.
- **Acyclicity** (§8.2) and **cycle-guarded walks** are mandatory.
- **No structural rewrites for borrowing** (§6).

---

## 12. What the model requires but we don't yet compute (⇒ the roadmap)

Reading the model top-down, the **gaps** are exactly the open work, in dependency
order:

1. **Universal routing (R1)** `[partial]` — route *every* position of a dispatched
   type, incl. arrays `C[]` ⟶ `Vec<CKind>` and the borrow seams (`&CKind` vs
   `&C`). This is the current blocker: it closes R4's residual leaks for pervasive
   value types AND is the precondition for the fusion.
2. **R4 × Tier-2 fusion (§9)** `[gap]` — once routing is universal, a
   `List<Geometry>` becomes `Vec<GeometryKind>`. Unblocks the bulk of the
   hierarchy-typed-collection errors (the JTS `E0107` mass).
3. **Full Tier-2 solve (§9)** `[partial]` — cross-slot union-find (field↔param↔
   return↔local) + LUB, beyond the shipped leaf-local slice.
4. **Intermediate-method delegation (§7.2)** `[partial]` — generate enum methods
   for supertype APIs not covered by `Deref`.
5. **Real `Object` (§7.4)** `[gap]` — synthesized variant-enum for heterogeneous
   `Map<_,Object>`; `Box<dyn Any>` fallback.
6. **Generic trait objects** `[gap]` — `Box<dyn Tr<S,C>>` object-safety/arity, the
   one area both R2 and R4 defer.

The model also explains the *non*-goals: we never infer borrows, never reject
programs, and never replace `Unknown` with something less capable. The single
through-line of the remaining work is **make every type decision consistent across
all sites of a value** — N1/R1/R2/U1/U2 are five faces of that one requirement, and
Tier-2 unification with universal routing is its general form.
