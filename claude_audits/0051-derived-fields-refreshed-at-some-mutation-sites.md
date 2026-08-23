# 0051 — Three more derived fields refreshed at only some of their mutation sites, and a repair pass that fixes one of two cases

Status: **verified by census** (every mutation site of every source field
enumerated and classified); live-vs-latent stated per instance
Severity: **medium-high** — the first instance can emit a Rust `enum` that is
missing variants; the other two are latent
Kind: derived state that is stored and can go stale

Recorded 2026-08-21.

[0046](0046-slot-shape-type-identity-is-a-hand-refreshed-cache.md) documents
this pattern for `RustSlotShape.type_identity` (7 of 20 mutation sites refresh;
I reproduced that census independently and get the same 7/13/20 split). It is
not the only instance. Three more structs store a field that is a pure function
of a sibling field and refresh it at some assignment sites and not others.

Two of them differ from 0046 in a way worth naming: the repo already contains
an **ad-hoc repair pass** that re-derives the field after the mutations — and
the pass handles one of the two representations that need it. A half-written
repair is the strongest evidence that the drift was noticed, patched at the
point where it hurt, and left in place everywhere else.

---

## Instance 1 — `TypeRepresentation`'s variant list vs `direct_subtypes` (LIVE: emits an enum with missing variants)

`src/representation.rs:48 representation::TypeRepresentation`

```rust
pub struct TypeRepresentation {
    pub java_name: String,
    pub representation: ObjectRepresentation,  // ClosedEnum{variants} / SourceHierarchyCarrier{variants}
    pub provenance: RepresentationProvenance,
    pub direct_subtypes: Vec<String>,
}
```

`variants` is a pure function of `direct_subtypes`:
`src/object_model.rs:1776 closed_enum_variant_names_for_subtypes` (used at
`object_model.rs:1158` and `object_model.rs:1742`) and
`src/object_model.rs:1315 source_hierarchy_carrier_variant_names`.

### The ordering that creates the staleness

`src/representation.rs:95 representation::plan_representations`:

```rust
for strategy in object_plan.strategies.into_values() {
    let representation = representation_for_strategy_with_external_final_facade_policy(…);
    plan.types.insert(strategy.root.clone(), TypeRepresentation {
        representation,                        // ← variants baked in HERE
        direct_subtypes: strategy.direct_subtypes,
        …
    });
}
for edge in object_plan.hierarchy_edges {
    record_hierarchy_edge(&mut plan, edge);    // ← direct_subtypes GROWS here
}
keep_source_platform_facade_subclasses_concrete(program, &mut plan);
expand_source_hierarchy_carrier_variants(&mut plan);   // ← repairs ONE variant
```

`src/representation.rs:1234 add_direct_subtype` (reached from
`src/representation.rs:1223 record_hierarchy_edge`) is the only mutator:

```rust
if !entry.direct_subtypes.iter().any(|existing| existing == subtype_name) {
    entry.direct_subtypes.push(subtype_name.to_string());
}
```

It never touches `entry.representation`.

### The repair pass covers one of the two cases

`src/representation.rs:170 expand_source_hierarchy_carrier_variants` exists
*only* to undo this — it re-derives variants after the pushes:

```rust
let roots = plan.types.iter().filter_map(|(name, representation)| {
    matches!(representation.representation,
             ObjectRepresentation::SourceHierarchyCarrier { .. })
        .then_some(name.clone())
}).collect::<Vec<_>>();
for root in roots {
    let variants = source_hierarchy_carrier_variants_for_root(plan, &root);
    …*existing = variants;
}
```

`ObjectRepresentation::ClosedEnum { variants }` is not in the filter and gets no
repair.

### Why this one is live

The variant list is not advisory — it *is* the emitted enum body.
`src/representation_lowering.rs:139` destructures
`ObjectRepresentation::ClosedEnum { enum_name, variants }` and passes `variants`
to `closed_enum_item`, which at `representation_lowering.rs:103-134` emits:

- the variant declarations (`:103 variant_items`)
- the `values()` array (`:108`)
- the `value_of` match arms (`:113`)
- the `name()` match arms (`:118`)
- the `ordinal()` match arms (`:123`)
- the `Default` impl, from `variants.first()` (`:129`)
- the payload accessor methods (`:134`)

A subtype that entered `direct_subtypes` via `add_direct_subtype` *after* the
representation was built is **absent from all seven**. Constructing or matching
that subtype in the generated crate does not compile; `values()`/`ordinal()`
silently return a short list.

The same shape exists one layer down on `src/object_model.rs:27
ObjectStrategyPlan`: `src/object_model.rs:1123 add_direct_subtype_to_strategy`
pushes to `strategy.direct_subtypes`, and `apply_closed_enum_strategy_selection`
(`src/object_model.rs:1127`) returns early for an already-`ClosedEnum`
representation, so the stale list survives that pass too. The *counter*-example
is right beside it: `src/object_model.rs:1394
apply_hierarchy_overrides_to_strategy_plan` sets `strategy.strategy` and
`strategy.direct_subtypes` together, which is what all of them should do.

**Live-vs-latent: live.** The reachability condition is precise — an entry whose
representation is `ClosedEnum` at insertion time and whose `direct_subtypes`
gains a member from `object_plan.hierarchy_edges` afterwards. That is exactly
the situation `expand_source_hierarchy_carrier_variants` was written to handle
for the sibling representation, so it is known to occur.

---

## Instance 2 — `ResolvedConstructor.params` vs `constructed_type` (one of two sites omits the refresh)

`src/java_ir.rs:243 java_ir::ResolvedConstructor` carries both
`constructed_type` and `params`. `src/resolve.rs:2567
constructor_params_for_constructed_type` substitutes `constructed_type`'s
generic arguments into `params`, so `params` is derived from
`constructed_type`.

Two sites assign `constructed_type` and then feed `params` to
`apply_constructor_param_targets`, which propagates each param type onto the
corresponding argument expression (`src/resolve.rs:2556`). They differ by
exactly one statement.

`src/resolve.rs:2545` — correct:

```rust
resolved.constructed_type = inferred_constructed_type(ty, Some(target_ty), …);
resolved.params = constructor_params_for_constructed_type(resolved, symbols);   // refresh
apply_constructor_param_targets(args, resolved, symbols, external_facts);
```

`src/resolve.rs:3060` — missing the middle line:

```rust
constructor.constructed_type = inferred_constructed_type(
    ty, None, &constructor.declaring_type, symbols, external_facts,
);
                                                                    // no refresh
apply_constructor_param_targets(args, &constructor, symbols, external_facts);
```

At the second site `constructor.params` is still `selected.params.clone()` — the
*declared* parameter list, with the type variables (`T`, `E`) unsubstituted — so
the target type propagated onto each argument is the type variable rather than
the inferred type argument. Readers of the stale field are
`src/resolve.rs:2562` (the propagation above) and `src/lower.rs:6047`, which
matches a source `ConstructorDecl` against `resolved.params` **by param type**.

**Live-vs-latent: latent as far as I could push it.** I tried
`new Box<String>("hello")` and `new Box<>(s)` against a generic nested class and
the emitted Rust was correct — `GenBox::new("hello".to_string())` — because the
argument coercion is recovered on a later path. The defect is in the code, not
(yet) in the output. The claim here is structural: two sites that perform the
same three-step sequence must not differ by whether one of them remembers to
re-derive.

Worth noting one step further out: `ResolvedConstructor.arg_coercions` is
derived from `params`
(`src/resolve.rs:4931 java_coercion_evidence_for_params(&selected.params, …)`)
and is refreshed at **neither** site — including the "good" one at
`resolve.rs:2552`.

---

## Instance 3 — `HierarchyFactLookupIndex`: a memo keyed on a name, valued on a whole `FactSet`

`src/facts.rs:79`:

```rust
#[derive(Debug, Clone, Default)]
pub struct HierarchyFactLookupIndex {
    reachable_by_sub_name: HashMap<String, HierarchyReachabilityResult>,
}
```

`src/facts.rs:589 HierarchyFactLookupIndex::new(facts: &FactSet)` fills it by
running `facts.reachable_hierarchy_edges_for_type_with_stats(&java_ty)` — a
worklist saturation over `type_capability_overrides` and `hierarchy_overrides`
— for every name in `facts.hierarchy_fact_type_names()`. **The key is a type
name. The value depends on the entire `FactSet`.**

The query signature states the mismatch out loud (`src/facts.rs:606`):

```rust
pub fn reachable_hierarchy_edges_for_type_with_stats(
    &self,
    facts: &FactSet,          // <- taken, and ignored on a hit
    java_ty: &JavaType,
) -> HierarchyReachabilityResult {
    …
    self.reachable_by_sub_name
        .get(sub_name)
        .cloned()
        .unwrap_or_else(|| facts.reachable_hierarchy_edges_for_type_with_stats(java_ty))
}
```

It falls back to a live recompute on a *missing key*, never on a *changed fact
set*. And `src/semantic_analysis.rs:137` holds an index and a `&mut FactSet`
alive across two calls:

```rust
let hierarchy_index = facts.hierarchy_lookup_index();
collect_exception_facts(program, &mut facts, &hierarchy_index, &mut diagnostics);
collect_catch_facts(program, &mut facts, &hierarchy_index);
```

**Live-vs-latent: latent, and I checked why.** `collect_exception_facts`
(`src/semantic_analysis.rs:4150`) pushes only `facts.exception_facts`;
`collect_catch_facts` (`src/semantic_analysis.rs:4281`) only
`facts.catch_facts`. Neither is read by `hierarchy_fact_type_names`
(`src/facts.rs:3233`, which reads `type_capability_overrides` only) or by the
reachability saturation. The index is correct today by accident of what those
two collectors happen to push — nothing in the types says so.

The counter-example is 15 lines above it in the same file: `src/facts.rs:64
ExternalFactLookupIndex<'a>` stores `facts: &'a FactSet` and caches *indices*
into the fact vectors rather than resolved answers, so the borrow checker pins
it to the exact `FactSet` it indexed and the matching predicate is re-evaluated
per lookup. `HierarchyFactLookupIndex` is the same idea written without the
lifetime — and with a `Default` derive, so a default-constructed one answers
"no hierarchy edges" for every type without anyone noticing.

---

## Why it went unnoticed

- **The derived field is `pub` in all three cases.** There is no setter to put
  the re-derivation behind, so "remember to refresh" is a convention enforced by
  reading the surrounding lines.
- **The repair pass hides the survivor.** Because
  `expand_source_hierarchy_carrier_variants` makes the `SourceHierarchyCarrier`
  case correct, a test written against a source hierarchy passes, and the
  `ClosedEnum` case never gets one. The existence of the pass makes the area
  *look* handled.
- **Staleness is not a crash.** A short `ClosedEnum` variant list fails in the
  generated crate, at a use site that may be in another file, and reads as an
  unrelated resolution gap; a stale `params` propagates a type variable that a
  later inference pass usually re-derives; a stale hierarchy index would just
  report a missing supertype edge.
- **No invariant is asserted anywhere.** Nothing checks
  `variants == closed_enum_variant_names_for_subtypes(&direct_subtypes)`, or
  `params == constructor_params_for_constructed_type(self, symbols)`. All three
  structs derive `PartialEq` over the derived field, so two otherwise-identical
  values can compare unequal purely because one was refreshed and one was not.

## Direction that removes the possibility of staleness

Ranked by how completely each removes the failure mode rather than patching it.

1. **`add_direct_subtype` is already the single mutation point** for
   `direct_subtypes` (`src/representation.rs:1234`). Make it re-derive the
   variant list for `ClosedEnum` as well as `SourceHierarchyCarrier`, and then
   **delete** `expand_source_hierarchy_carrier_variants` rather than extend it —
   deleting the repair pass is what proves the invariant is now maintained at the
   mutation, not after it. Same for
   `src/object_model.rs:1123 add_direct_subtype_to_strategy`.
   The stronger form: make `variants` a method
   (`fn variants(&self) -> Vec<String>` over `direct_subtypes`) rather than a
   payload of the `ObjectRepresentation` enum, so there is nothing to refresh.
2. **`ResolvedConstructor.params` should not be a field.** Replace it with
   `fn params(&self, symbols: &ProgramSymbols) -> Vec<Param>` calling
   `constructor_params_for_constructed_type`. The substitution then happens
   unconditionally and `resolve.rs:3060` becomes correct for free — as does
   `arg_coercions`, which is derived one step further and refreshed nowhere.
3. **Pin the memo to what it memoized.** Give `HierarchyFactLookupIndex` a
   `facts: &'a FactSet` field, drop the `facts` parameter from its three query
   methods, and drop the `Default` derive — the `ExternalFactLookupIndex<'a>`
   pattern already present in the same file. The borrow checker then rejects
   `semantic_analysis.rs:137`'s index-alive-across-`&mut facts` window outright,
   which is a stronger guarantee than the current "the collectors happen not to
   touch hierarchy facts".

## Reproduction

The `direct_subtypes` / variants ordering is visible by reading
`src/representation.rs:95-133` top to bottom: insert (variants frozen) →
`record_hierarchy_edge` loop (subtypes grow) → repair pass filtered to one
variant. The `ResolvedConstructor` divergence is a three-line diff between
`src/resolve.rs:2545-2553` and `src/resolve.rs:3060-3072`.

The `type_identity` census in
[0046](0046-slot-shape-type-identity-is-a-hand-refreshed-cache.md) reproduces
mechanically:

```
grep -rn "\.java_ty = " src/ | while IFS=: read -r f l rest; do
  after=$(sed -n "$((l+1)),$((l+7))p" "$f")
  if echo "$after" | grep -q "type_identity"; then st=REFRESH; else st="stale  "; fi
  printf "%s  %s:%s\n" "$st" "$f" "$l"
done
```
→ 7 `REFRESH`, 13 `stale`, 20 total. (Note that a 4-line lookahead
mis-classifies `conversion_plan.rs:1721`, whose refresh is 5 lines below;
7 lines is the correct window.)
