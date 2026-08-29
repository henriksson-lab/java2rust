# 0064 — The Java→Rust name map is a pure, non-injective function of one string

Status: **open, mostly mitigated** — the 7 historical repros were validated
with `javac`/`java` oracles and `rustc` output, but most concrete collision
paths now have local fixes. The remaining defect is the broader identity model:
package/type ownership is still not first-class across the analysis keys.
Severity: high — three of the seven produce a *silently wrong program*
(wrong class constructed, wrong package's constant read, `unimplemented!` for a
class the translator is itself translating); the rest are `E0428`/`E0592`
Kind: identity modelling — the same failure family as
[0009](../../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md),
one level below it

Current tree update:

* Generic source type-bound lookup no longer accepts the first emitted Rust
  alias match when multiple Java owners collapse to the same Rust name. Exact
  Java-owner lookup still wins, but the Rust-alias tier now requires uniqueness
  like the simple-name tier, so an emitted alias such as `AB_C` cannot silently
  select whichever of `a.B.C` or `AB.C` appears first. Regressions:
  `item_assembly::tests::source_type_bound_decl_refuses_ambiguous_simple_package_peer`
  and
  `item_assembly::tests::source_type_param_into_supertype_bounds_refuses_ambiguous_simple_package_peer`.
* FactSet lookup fallbacks for field nullability, trivial accessors, external
  static field values, and external constructors now refuse ambiguous
  package-simple owner aliases, matching the earlier `external_field_for`
  behavior. Exact/direct owners still resolve, but querying `Item` no longer
  selects the last matching fact when both `a.Item` and `b.Item` publish the
  same member/constructor shape. Regression:
  `semantic_facts::fact_lookup_refuses_ambiguous_simple_package_owner_aliases`.
* Resolved anonymous interface constructors now resolve the declaring interface
  owner through the local import/static-alias table and combined context lookup
  before rendering the trait object constructor. `import a.I; new I() { ... }`
  no longer falls back to `<dyn I>::anonymous()` when `a.I`/`b.I` peers are
  both visible; it renders the declaration-backed trait name such as
  `<dyn AI>::anonymous()`. Regression:
  `lower::tests::resolved_anonymous_interface_constructor_uses_import_alias_owner`.
* §4 and §5 are fixed for declarations in one generated `impl`: after the
  normal overload name is computed, declared methods are allocated in the Rust
  inherent namespace with constructor names pre-occupied. Regressions:
  `source_methods_with_colliding_snake_case_names_get_distinct_rust_names` and
  `source_overloads_with_colliding_type_suffixes_get_distinct_rust_names`.
* §6 is fixed by teaching the nested-name qualifier descendant aliases such as
  `P.Counter -> W.P.Counter`; regression:
  `partially_qualified_nested_type_reference_uses_declared_source_type`.
* §7 is stale in this tree: `boxed_dyn_trait_projection` now calls
  `normalize_emitted_java_type_name`, and `NestedInterfaceDispatch.java` covers
  the boxed nested-interface path.
* §2 is fixed for nested source types. Type-to-Rust mangling now preserves
  adjacent type path boundaries (`K.AB -> K_AB`, `K.A.B -> K_A_B`) while keeping
  package-like lowercase segments compact (`java.io.InputStream ->
  JavaIoInputStream`). Nested convenience aliases are emitted only when unique
  in the generated file's flat Rust namespace. Regressions:
  `non_injective_nested_type_mangling_distinguishes_path_boundaries`,
  `duplicate_nested_simple_type_aliases_are_suppressed`, plus the unit-level
  `nested_type_name_mangling_preserves_type_path_boundaries`.
* §3b's import-specific static owner drift is fixed for the local alias table:
  explicit imports are seeded authoritatively, while support/context imports and
  context type aliases no longer overwrite them. Regression:
  `explicit_import_aliases_survive_duplicate_context_simple_names`. Broader
  package identity remains open at the model level even though the known batch
  and co-translated-context same-simple-name regressions are now covered below.
* §3a's explicit package-qualified source references are fixed for ordinary
  source AST type positions and constructor targets. `a.Item ai = new a.Item()`
  and `b.Item bi = new b.Item()` now lower to `AItem`/`BItem` slots and
  constructors, while `a.Item.KIND`/`b.Item.KIND` continue to resolve to the
  package-specific static owners. Regression:
  `package_qualified_source_type_references_keep_package_identity`.
* §3a's package-qualified source class literals now keep package identity in
  both the Java class name and token type. `a.Item.class` lowers as
  `JavaClass::<AItem>::literal("a.Item")` and `b.Item.class` lowers as
  `JavaClass::<BItem>::literal("b.Item")`, including the merged-batch case
  where the first same-simple-name source type is stored under the simple key.
  Regressions: `parses_package_qualified_class_literal_type_identity`,
  `class_literal_tokens_are_planned_before_lowering`, and
  `package_qualified_class_literals_keep_package_identity`.
* Same-package simple class literals now keep the Java package name without
  forcing the Rust token type through the package-qualified mangler.
  `package a; class Item { Item.class.getName(); }` lowers as
  `JavaClass::<Item>::literal("a.Item")`, and the generated fallback
  `get_class()` returns the same Java name. Regression:
  `same_package_simple_class_literal_keeps_java_package_name`.
* §3a is also fixed for same-simple-name package peers emitted in a batch:
  duplicate top-level primary type declarations are qualified before the shared
  batch context is built, ambiguous simple static-owner aliases are suppressed,
  and package-qualified source constants resolve through the qualified owner
  instead of falling back to the first simple-name match. The strengthened
  `package_qualified_source_type_references_keep_package_identity` regression
  now checks both declaration sites (`pub struct AItem` / `pub struct BItem`)
  and use sites (`AItem::new`, `BItem::new`, `AItem::KIND`, `BItem::KIND`).
* The same duplicate-program pre-pass now includes co-translated context source
  programs before they are merged into the single-source context. Regression:
  `cotranslated_same_simple_name_package_peers_keep_package_identity`.
* Context type lookup now refuses ambiguous simple-name matches instead of
  returning the first package peer. Exact/package-qualified and emitted Rust-name
  matches still win; a unique simple-name fallback still works. Regression:
  `context_type_lookup_refuses_ambiguous_simple_package_peer`.
* Emitted-context type lookup now follows the same rule. It also refuses
  ambiguous simple-name package peers after exact emitted-name/package-qualified
  matches have had a chance to win. Regression:
  `emitted_context_type_lookup_refuses_ambiguous_simple_package_peer`.
* Update 2026-08-25: the emitted-context ambiguity check now spans all three
  programs visible to lowering (`program`, `support_local_type_program`, and
  `source_summary_type_program`). A unique simple-name fallback is still
  accepted, but same-simple package peers split across those programs no longer
  return whichever program was searched first. Regression:
  `emitted_context_type_lookup_refuses_cross_program_ambiguous_simple_package_peer`.
* Imported source enum `valueOf(String)` calls now keep package owner identity
  when same-simple-name package peers are present. The static `valueOf` lowering
  path resolves the owner through the existing source/context alias lookup
  before rendering the Rust associated-call owner, instead of using the raw
  receiver spelling. The same regression also caught an adjacent defect in the
  duplicate-type pre-pass: it renamed `TypeDecl.name` from `Mode` to `a.Mode`
  without rewriting the declarations and expressions inside that type, so enum
  constants still had type `Mode` and the emitted `AMode` enum had no variants.
  The pre-pass now rewrites internal type references under the renamed type.
  Compile-backed regression:
  `imported_source_enum_value_of_keeps_package_owner_identity`.
* Instance-method overload retargeting no longer resolves a bare same-simple
  owner by searching `program`, then `support_local_type_program`, then
  `source_summary_type_program` and accepting the first match. The helper now
  uses the cross-program ambiguity-aware context lookup, so `Item.work()` fails
  closed when `a.Item` and `b.Item` are both visible, while exact owners
  (`a.Item`, `AItem`, `b.Item`, `BItem`) still resolve. Regression:
  `lower::tests::source_declared_instance_method_lookup_refuses_cross_program_ambiguous_owner`.
* Source object `toString()` fallback now uses the same cross-program
  ambiguity-aware context lookup instead of searching emitted/support context
  before summary context. A bare `Item.toString()` fallback no longer picks
  whichever same-simple package peer appears first; it declines the erased
  fallback when `a.Item` and `b.Item` are both visible. Regression:
  `lower::tests::object_to_string_fallback_refuses_cross_program_ambiguous_simple_package_peer`.
* Static-storage accessor spellings no longer re-derive a snake-case name after
  the static field allocator has selected an emitted Rust field name. The
  accessor projection now treats its input as the already allocated member name,
  and `static_storage_accessors_for_field` no longer emits snake-case alias
  getters/setters beside the allocated pair. Regressions:
  `rendered_string_heuristics_have_centralization_guardrails`,
  `lower::tests::static_storage_reference_uses_resolved_context_owner_identity`,
  and `source_static_final_byte_constants_preserve_declared_slot_type`.
  Follow-up 2026-08-28: the byte-constant regression was still listed in the
  `idiom_gaps` expected-failure baseline even though the current lowering
  preserves `i8` constants and emits owner-qualified constant reads. The stale
  `Self::` assertion was updated to `ByteConstantOwner::`, and the test was
  removed from the baseline after the focused check passed.
* Exact source static field names now outrank alias fallback in the static
  semantic/access-name/owner maps. The previous maps let an earlier alias claim
  a later exact field spelling, so `static int self; static int self__;` could
  emit distinct accessors but still assign both writes through
  `__set_static_self___`. The owner lookup also now prefers the current type's
  typed static-storage table before consulting the flat owner map, because two
  different impls can legitimately both have an emitted `__static_self__`
  accessor. Regressions:
  `qualified_static_storage_keyword_fields_do_not_collapse`,
  `bare_static_storage_keyword_alias_does_not_overwrite_exact_field`,
  `nested_static_storage_exact_owner_beats_enclosing_alias`, and
  `qualified_static_final_keyword_consts_do_not_collapse`.
* Signature-plan simple owner lookup now fails closed before method filtering
  when a bare owner query is ambiguous across package peers. A query such as
  `Item.who()` no longer resolves to `b.Item` merely because `a.Item` lacks
  `who`; exact `a.Item`/`b.Item` queries still work. Regression:
  `signature::tests::method_signature_presence_fails_closed_for_ambiguous_simple_package_owner`.
* Receiverless null-overload ambiguity detection now uses the same combined
  context lookup for the current source type instead of searching program,
  support and summary contexts in priority order. A bare current type `Item`
  split across `a.Item` and `b.Item` no longer reports an ambiguous `m(null)`
  call from whichever context appears first; exact `a.Item` still reports the
  real overload ambiguity. Regression:
  `lower::tests::current_null_overload_check_refuses_ambiguous_context_owner`.
* Receiverless source-summary inherited return-shape lookup now resolves the
  current type through the same combined context lookup before walking inherited
  summary methods. Bare `Child.value()` no longer recovers a return shape from
  the first `Child` package peer when another context peer exists; exact
  `a.Child` still recovers the inherited shape. Regression:
  `lower::tests::receiverless_summary_return_shape_refuses_ambiguous_current_owner`.
* Static singleton initializer lowering now resolves the current type through
  the same combined context lookup instead of a program/support priority chain.
  A bare `Holder.INSTANCE` no longer lowers to `Config::new()` from whichever
  context peer is searched first when `a.Holder` and `b.Holder` are both
  visible; exact `a.Holder` remains accepted. Regression:
  `lower::tests::static_singleton_initializer_refuses_ambiguous_current_owner`.
* Unsupported source-constructor call handoff now resolves the constructed
  source type through the combined context lookup. A bare `new Item()` no longer
  emits the unsupported-super-constructor handoff from `a.Item` when another
  context peer `b.Item` is also visible; exact `new a.Item()` still emits the
  handoff. Regression:
  `lower::tests::unsupported_source_constructor_handoff_refuses_ambiguous_context_owner`.
* Nested enum static-field owner candidates now resolve the current owner
  through the combined context lookup before adding nested enum aliases. A bare
  ambiguous `Holder` no longer contributes `a.Holder.Mode` as a candidate for
  `mode`, while exact `a.Holder` still does. Regression:
  `lower::tests::enum_static_field_owner_candidates_refuse_ambiguous_current_owner`.
* Source constructor lowering for a concrete destination slot now uses the
  combined context lookup for the constructed type guard. `new Item()` fails
  closed when `Item` is ambiguous across `a.Item` and `b.Item`, while exact
  `new a.Item()` from support context still lowers to `AItem::new()`.
  Regression:
  `lower::tests::source_constructor_concrete_slot_refuses_ambiguous_context_owner`.
* Current-type field shape helpers no longer search program/support/summary
  contexts in priority order. Both ordinary current field shape recovery and
  static-final const-array field type recovery now use the combined
  ambiguity-aware context lookup, so a bare current type `Item` split across
  package peers fails closed instead of borrowing the first peer's field shape.
  Regressions:
  `lower::tests::current_type_field_shape_refuses_ambiguous_context_owner` and
  `lower::tests::static_final_const_array_field_type_refuses_ambiguous_current_owner`.
* Source-summary receiver method return-shape recovery no longer checks
  `source_summary_type_program` directly before the combined context lookup. A
  `this.value()` call under a bare ambiguous current type no longer borrows the
  summary peer's method shape just because the support/program peers do not
  carry that method; exact package-qualified owners still recover the summary
  shape. Regression:
  `lower::tests::receiver_summary_return_shape_refuses_ambiguous_current_owner`.
* Source Java enum type-declaration lookup no longer walks
  program/support/summary and accepts the first simple-name or rendered-alias
  match. Current-type enum lookup and Java-type enum lookup now both use the
  combined context key lookup before requiring `TypeKind::Enum`, so bare
  same-simple package peers fail closed while exact and unique emitted aliases
  still resolve. Regression:
  `lower::tests::source_enum_type_decl_lookup_refuses_cross_program_ambiguous_simple_owner`.
* Signature-plan required method/field fallbacks no longer cross distinct
  qualified owners after an exact lookup misses. The compatibility fallback for
  a unique bare `Item` owner remains, but `a.Item::who` and `a.Item::value` no
  longer borrow the sole `b.Item` method or field plan merely because the final
  simple segment matches. Regressions:
  `signature::tests::required_method_signature_does_not_borrow_distinct_package_peer`
  and
  `signature::tests::required_field_shape_does_not_borrow_distinct_package_peer`.
* This does not close the broader package-identity item: generated support
  stubs for separately emitted context peers are still not compiled as part of
  the single-source helper, and type declaration identity still needs a
  first-class package-aware key instead of the current duplicate pre-pass.

Line numbers are from the working tree at audit time; prefer the symbol names.
Every repro in this file is staged under `repro/` next to it.


[0009](../../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md)
closed on *methods*: `JavaCallableSlotKey` now carries `(java_name,
erased_param_types, kind, staticness)` and the override/inherit graph runs on it.
That work assumed the surrounding identity — **which type** a slot belongs to,
and **what Rust identifier** the slot is finally spelled as — was already sound.
It is not. This file is about that surrounding identity, and its companion
[0065](0065-the-overload-suffix-is-scoped-to-one-file-and-one-level-of-inheritance.md)
is about the one piece of 0009's own scheme that is still scoped wrongly.

---

## 1. The shape of the problem

Java identity for a nameable entity is

```
(package, enclosing-type chain, simple name, [parameter types], kind)
```

Rust identity for an item is `(module, identifier)`; for an inherent method it is
`(type, identifier)`. Going from the first to the second is a *function*, and for
the translation to be correct that function must be

* **total** — defined for every Java entity, including one referred to by a
  partially-qualified spelling;
* **injective** — two distinct Java entities never land on one Rust identifier;
* **single-valued** — the *declaration site* and every *reference site* compute
  the same answer for the same entity.

Today it is none of the three, and the reason is structural: the map is
implemented as a handful of **pure `&str -> String` functions** that see only the
spelling of the name in front of them. They have no access to the package, no
access to the enclosing-type chain, and no access to what identifiers are already
taken. Injectivity is not something such a function can have.

### What exists today, per entity kind

| Entity | Function | Sees | Injective? |
|---|---|---|---|
| local binding | `java_naming::source_binding_rust_names` → `allocate_rust_ident` | all names in scope, **occupancy set** | **yes** |
| instance field | `java_naming::source_instance_field_rust_names` | all fields of the type, **occupancy set** | **yes** (within a type) |
| static field | `java_naming::source_static_field_rust_names` | all static fields of the type, **occupancy set** | **yes** (within a type) |
| method | `parser::to_snake_case` + `parser::type_suffix` suffix (`resolve::resolve_method`) | one name string; param type *spellings* | **no** (§3, §4) |
| type | `java_naming::rust_name_for_java_named_type` | one dotted string, **package excluded** | **no** (§2, §5) |

The two entity kinds that already have a collision-avoiding allocator are exactly
the two that never collide in practice. The two that carry the program's
structure — types and methods — are the pure functions.

`allocate_rust_ident` (`src/java_naming.rs:69`) is already the right primitive:
it takes a base name and an occupancy set and appends `_` until free. It is
called only from the field/local paths.

### The key that would fix it exists, and is not the key anything is stored under

`src/java_signature.rs` defines `JavaCallableKey { owner, java_name,
erased_param_types, kind, staticness }`. Meanwhile the two maps that lowering
actually indexes are:

```rust
// src/facts.rs:922
pub struct MethodKey { pub type_name: String, pub method_name: String }   // Java name
// src/signature.rs:57
pub struct MethodKey { pub type_name: String, pub rust_name: String }     // Rust name
```

Both key on a **bare `type_name` string with no package**, and they disagree with
each other about which spelling of the method name they hold. `usage::CallableKey::Method`
(`src/usage.rs:27`) is a third, with `type_name` + `method_name`. `src/lower.rs`
still has 23 `.name ==` comparisons.

---

## 2. Type mangling path-boundary collisions — fixed in the current tree

This section's original repro said type mangling was effectively
`join(path_parts, "")`, so `K.AB` and `K.A.B` both became `KAB`. That is now
historical. `src/java_naming.rs` preserves adjacent type-looking path
boundaries, so those examples render as `K_AB` and `K_A_B`.

The simple-alias half is also guarded: duplicate nested simple aliases are
suppressed instead of emitting multiple `pub type Inner = ...` items into the
same Rust namespace.

Regressions:

* `java_naming::tests::nested_type_name_mangling_preserves_type_path_boundaries`
* `non_injective_nested_type_mangling_distinguishes_path_boundaries`
* `duplicate_nested_simple_type_aliases_are_suppressed`

The audit remains open for the broader package/type identity model, not for
this nested path-boundary collision.

---

## 3. The package is not part of a source type's identity at all

`rust_name_for_java_named_type` never sees a package, because a source
`TypeDecl.name` never contains one. Two classes with the same simple name in
different packages are one Rust type.

### 3a. Both emit `pub struct Item`

```java
// a/Item.java        package a; public class Item { public static final int KIND = 1; public int who() { return 1; } }
// b/Item.java        package b; public class Item { public static final int KIND = 2; public int who() { return 2; } }
// Main.java
public class Main { public static void main(String[] a) {
    a.Item ai = new a.Item();  b.Item bi = new b.Item();
    System.out.println(ai.who() + "," + bi.who() + "," + a.Item.KIND + "," + b.Item.KIND);
} }
```

`java Main` → `1,2,1,2`. `translate-batch` emits `pub struct Item` in *both*
`00002_Item.rs` and `00003_Item.rs`, and in `00001_Main.rs`:

```rust
let ai: Item = Item::new();
let bi: Item = Item::new();
println!("{}", format!("{},{},{},{}", (ai.who()).to_string(), (bi.who()).to_string(),
                       (a.Item.k_i_n_d).to_string(), (b.Item.k_i_n_d).to_string()));
```

Three separate collapses in three lines: the two distinct types became one name;
`a.Item.KIND` was emitted as raw dotted Java (`E0425: cannot find value a`); and
its field was snake-cased to `k_i_n_d` while the declaration site emits `pub
const KIND: i32 = 1;`.

Also worth recording: the class literal loses the package too —
`JavaClass::<Cfg>::literal("Cfg")` where `a.Cfg.class.getName()` is `"a.Cfg"`.

Note the asymmetry with *external* types, which **do** get their package folded
in: the same output contains `pub type OrgDisqBioDisqBaiWriteOption =
BaiWriteOption;` and `pub struct ACfg;`. So the translator has a
package-qualified naming function; it just does not apply it to types whose
source it is reading.

### 3b. A same-simple-name lookup ignores the import and takes the first match

This one is the silent-wrong-answer version.

```java
// a/Cfg.java   package a; public class Cfg { public static final int N = 1; public static int who() { return 1; } }
// b/Cfg.java   package b; public class Cfg { public static final int N = 2; public static int who() { return 2; } }
// UseA.java    import a.Cfg;  public class UseA { public static int v() { return Cfg.N + Cfg.who(); } }
// UseB.java    import b.Cfg;  public class UseB { public static int v() { return Cfg.N + Cfg.who(); } }
// Main2.java   prints UseA.v() + "," + UseB.v()
```

`java Main2` → `2,4`.

```rust
// 00002_UseA.rs   -- the file that imports a.Cfg
pub type Cfg = ACfg;
...
pub fn v() -> i32 { return (BCfg::N).wrapping_add(Cfg::who()); }

// 00003_UseB.rs   -- the file that imports b.Cfg
pub fn v() -> i32 { return (BCfg::N).wrapping_add(Cfg::who()); }
```

Both files emit **`BCfg::N`**. Swapping the two `import` lines changes nothing —
re-run with `UseA` importing `b.Cfg` and `UseB` importing `a.Cfg` and both still
emit `BCfg::N`, while the `pub type Cfg = ACfg;` alias moves to the other file.
So within one generated file, the type alias respects the import and the constant
read does not, and neither is asked which package the reference is in.

Root cause anchor:

```rust
// src/lower.rs:32186
fn context_type_key_matches(candidate: &str, query: &str) -> bool {
    if rust_name_for_java_named_type(candidate) == query { return true; }
    ...
    let candidate_simple = candidate.rsplit(['.', '$']).find(|s| !s.is_empty()).unwrap_or(candidate);
    let query_simple     = query.rsplit(['.', '$']).find(|s| !s.is_empty()).unwrap_or(query);
    canonical_external_receiver_type_key(candidate_simple) == canonical_external_receiver_type_key(query)
        || canonical_external_receiver_type_key(candidate_simple) == canonical_external_receiver_type_key(query_simple)
}
```

`find_context_type_by_key` (`src/lower.rs:32050`) uses `find_map` over the
program's types, so the **first** simple-name match wins; there is no ambiguity
signal and no notion of the referring file's imports or package.

This is worth contrasting with `source_constant_values::ConstantScope`
(`src/source_constant_values.rs:154`), which was fixed for exactly this problem
and gets it right: fully-qualified wins, then package-relative, then
unambiguous-or-refuse. That discipline exists in one module and governs constant
*folding*; the *emission* path above never consults it.

---

## 4. Two Java method names that snake_case to one Rust name

```rust
// src/parser.rs:3418
pub fn to_snake_case_unescaped(name: &str) -> String {
    ...
    } else if ch.is_ascii_uppercase() {
        if idx > 0 && !out.ends_with('_') && !out.ends_with('.') { out.push('_'); }
        out.push(ch.to_ascii_lowercase());
    } else { out.push(ch); }
}
```

An existing `_` suppresses the inserted one, so `xY` and `x_Y` both become `x_y`.

```java
public class A {
    public int xY(int a)  { return a + 1; }
    public int x_Y(int a) { return a + 2; }
    public static void main(String[] args) {
        A a = new A();
        System.out.println(a.xY(1) + "," + a.x_Y(1));
    }
}
```

`java A` → `2,3`. Generated:

```rust
impl A {
    pub fn x_y(&self, a: i32) -> i32 { return (a).wrapping_add(1); }
    pub fn x_y(&self, a: i32) -> i32 { return (a).wrapping_add(2); }
    pub fn main(args: Vec<String>) {
        let a: A = A::new();
        println!("{}", format!("{},{}", (a.x_y(1)).to_string(), (a.x_y(1)).to_string()));
    }
}
```

`error[E0592]: duplicate definitions with name x_y`. (`getURL` vs `getUrl` do
*not* collide — `get_u_r_l` vs `get_url` — the collision is specifically between
a camel boundary and an explicit underscore, which is the common style clash in a
codebase that mixes conventions.)

Current tree status: fixed for methods declared in the same source type. Method
lowering still starts with the existing Java-name/overload-name scheme, but
`allocate_unique_declared_method_rust_names` now allocates the final names inside
the Rust inherent namespace, so `xY` keeps `x_y` and `x_Y` becomes `x_y_`; call
resolution sees the assigned names through `ProgramSymbols`.

---

## 5. The overload suffix is built from *type spelling*, and is also non-injective

```rust
// src/parser.rs:3436
pub fn type_suffix(ty: &JavaType, is_varargs: bool) -> String {
    match ty {
        JavaType::Array(inner) => format!("{}_array", type_suffix(inner, false)),
        JavaType::Named { name, .. } => to_snake_case(&java_type_suffix_name(name)),
        ...
```

so `int[]` → `int_array` and a class named `IntArray` → `int_array`.

```java
class IntArray { int n = 9; }
public class Z2 {
    static int g(int[] a)     { return a.length; }
    static int g(IntArray a)  { return a.n; }
    public static void main(String[] args) {
        System.out.println(g(new int[3]) + "," + g(new IntArray()));
    }
}
```

`java Z2` → `3,9`. Generated:

```rust
impl Z2 {
    pub fn g_int_array(a: &IntArray) -> i32 { return a.length; }   // was g(int[])
    pub fn g_int_array(a: &IntArray) -> i32 { return a.n; }
    pub fn main(args: Vec<String>) {
        println!("{}", format!("{},{}",
            (Self::g_int_array(vec![0; 3 as usize].as_slice())).to_string(),
            (Self::g_int_array(IntArray::new())).to_string()));
    }
}
```

`error[E0592]: duplicate definitions with name g_int_array` — and note the array
overload's **parameter type** was also rewritten to `&IntArray` (`E0609: no field
length on type &IntArray`), so the collision is not confined to the identifier:
one slot's signature overwrote the other's.

`java_type_suffix_name` (`src/parser.rs:3452`) maps `.`, `$`, `<`, `>`, `,`, `?`
and whitespace all to `_`, so `f(Outer.Inner)` and `f(OuterInner)` are the same
suffix by the same mechanism.

Current tree status: fixed for same-type overload declarations. The suffixes can
still collide, but the post-pass allocator now gives the second declared method a
distinct Rust name (`g_int_array_`) and call resolution uses that assigned name.
This is a local fix, not the full type-identity allocator proposed below.

---

## 6. A partially-qualified nested reference is never qualified, and becomes an "unmodelled import"

`P.Inner` inside `Outer` is how ordinary Java spells a doubly-nested type. The
qualifier pass does not handle it:

```rust
// src/resolve.rs:285  qualify_nested_type_names_for_type
let direct_nested = ty.members.iter().filter_map(|m| match m {
    Member::Type(nested) => Some((nested.name.clone(),                    // <- SIMPLE name only
                                  format!("{}.{}", ty.name, nested.name.rsplit('.').next()...))),
    _ => None }).collect::<HashMap<_,_>>();
```

The map's keys are single identifiers (`P` → `N2.P`, and inside `P`'s own scope
`Inner` → `N2.P.Inner`). A reference written `P.Inner` matches no key, stays
unqualified, and downstream is mangled as a top-level name `PInner` — a *different
Rust type* from the real `N2PInner`, and one the translator then decides it knows
nothing about.

### Repro, and it compiles

```java
public class W {
    static int total = 0;
    static class P { static class Counter { void bump() { total += 10; } } }
    public static void main(String[] args) {
        P.Counter c = new P.Counter();
        c.bump(); c.bump();
        System.out.println(total);
    }
}
```

`java W` → `20`. Generated:

```rust
impl PCounter {                                  // synthesised import facade
    pub fn bump(&self) -> () { unimplemented!("P.Counter.bump is not modelled") }
}
...
pub struct WPCounter { }                         // the real class
impl WPCounter {
    pub fn bump(&self) { W::__set_static_total((W::__static_total()).wrapping_add(10)); }
}
pub type Counter = WPCounter;                    // and `pub type Counter = PCounter;` above

impl W { pub fn main(args: Vec<String>) {
    let c: PCounter = PCounter::new();           // <- the facade, not the class
    c.bump(); c.bump();
    println!("{}", Self::__static_total());
} }
```

Only one `rustc` error, the duplicate `pub type Counter`. Delete that single
alias line and the crate builds; running it:

```
thread 'main' panicked at W2.rs:1785:32:
not implemented: P.Counter.bump is not modelled
```

The translator concluded that a class **declared in the file it is translating**
is an unmodelled external import, purely because the source spelled it the legal
short way. Writing `W.P.Counter` instead makes the same program translate
correctly — same Java entity, two Rust identities chosen by spelling.

Current tree status: fixed. `qualify_nested_type_names_for_type` now indexes
descendant aliases while visiting the enclosing type, so `P.Counter` inside `W`
is rewritten to `W.P.Counter` before lowering. The regression compiles and runs
the generated crate and asserts that the real source class updates `W.total`.

---

## 7. `dyn` positions: three of four normalise, one does not

Current tree status: stale. The current `boxed_dyn_trait_projection` goes through
`normalize_emitted_java_type_name`, matching the other dyn projection sites; keep
the historical notes below as evidence of the original recurrence, not as an
open symptom.

0009's 2026-08-03 update introduced `normalize_emitted_java_type_name`
(`src/coercion.rs:11188`) with the comment *"Every position that emits a Java
type name as Rust goes through this."* Three do:

```rust
borrow_as_dyn_trait            (src/coercion.rs:1970)  -> normalize_emitted_java_type_name
borrow_boxed_as_dyn_trait      (src/coercion.rs:1981)  -> normalize_emitted_java_type_name
anonymous_dyn_trait_projection (src/coercion.rs:2006)  -> normalize_emitted_java_type_name
```

`boxed_dyn_trait_projection` (`src/coercion.rs:7447`) does not:

```rust
pub fn boxed_dyn_trait_projection(value: RustExpr, trait_name: &str) -> RustExpr {
    boxed_cast_projection(value, format!("Box<dyn {trait_name}>"))
}
```

Reached from `src/lower.rs:20633`. Output, from the `D.java` repro in
[0065](0065-the-overload-suffix-is-scoped-to-one-file-and-one-level-of-inheritance.md):

```rust
let i: Box<dyn DI> = Box::new(DImpl::new()) as Box<dyn D.I>;
```

```
error: expected one of `,` or `>`, found `.`
error[E0747]: constant provided when a type was expected
```

The declared slot got `DI`, the cast target got `D.I`. This is the fourth
instance of the same fix being applied per-symptom — 0013 fixed `downcast::<T>`,
0009 fixed `&dyn`, and `Box<dyn>` was missed.

---

## Why nothing noticed

* **The unit of compilation hides it.** `translate-focused-package.sh` emits one
  `pub mod generated_<stem>` per Java file (`scripts/translate-focused-package.sh:2758`).
  Two packages' `Item` land in two Rust modules, so the `E0428` that §3 would
  otherwise produce never fires; what is left is a *wrong reference*, which
  compiles or fails for an unrelated-looking reason. §2 and §5 only collide
  because both entities are in one file — which corresponds to a corpus using
  nested classes, and the corpus fixtures largely do not.
* **The compile-error metric is a count.** Every §-item here that does produce
  errors produces them at the *use* site, attributed to `E0308`/`E0609`/`E0425`,
  not to naming. [0018](../../claude_audits/done/0018-the-compile-error-metric-cannot-see-silent-failures.md)
  and [0053](../../claude_audits/done/0053-the-gate-compares-counts-not-names.md)
  already make this argument.
* **No test asserts injectivity.** There is no check anywhere that the set of
  emitted item names in a module has the same cardinality as the set of Java
  entities that produced it. Such a check is cheap and would have caught §2, §4
  and §5 mechanically, without anyone having to think of the collision.
* **The right key was built and not adopted.** 0009 built
  `JavaCallableKey`; the surrounding type identity was never given the same
  treatment, so the key's `owner: String` is itself a package-free simple name.

---

## Direction

The three properties at the top are the specification. In dependency order:

1. **Give a Java type an identity that carries its package.** A source
   `TypeDecl.name` should be the binary name (`a.Item`, `W.P.Counter`), and every
   lookup key that today holds a bare `type_name: String` — `facts::MethodKey`,
   `signature::MethodKey`, `usage::CallableKey`, `JavaCallableKey::owner` — should
   hold that. §3 and §6 both dissolve; §2 does not, but becomes detectable.

2. **Replace the pure mangling functions with one allocation pass.**
   `allocate_rust_ident` already exists and already does the right thing. Run it
   once, over the whole program, in a deterministic order, producing a
   `HashMap<JavaEntityId, String>` that is thereafter the *only* answer to "what
   is this called in Rust". Emission and every reference site read that map; no
   site is permitted to call `to_snake_case`, `type_suffix` or
   `rust_name_for_java_named_type` on its own. That makes §2, §4, §5 and §7
   unrepresentable rather than fixed. It is the same move 0009 made for method
   *slots* — decide names late, as a total function of the resolved entity set —
   applied to the naming itself.

3. **Assert injectivity in the gate.** For each generated module, the number of
   distinct emitted item names must equal the number of Java entities emitted
   into it. This is a one-line invariant that subsumes every collision in this
   file and every future one.

4. Until (2), a **resolution that finds two candidates by simple name must
   refuse**, as `ConstantScope::lookup_path` already does. `context_type_key_matches`
   silently taking the first is what turns §3b from a compile error into a wrong
   number.

### Regression cases worth pinning

```java
// injectivity of the type map
class K { static class AB {} static class A { static class B {} } }            // KAB twice
class N { static class P { static class Inner {} } static class Q { static class Inner {} } }

// injectivity of the method map
class A { int xY(int a){return 1;} int x_Y(int a){return 2;} }                  // x_y twice
class Z { static int g(int[] a){return 1;} static int g(IntArray a){return 2;} } // g_int_array twice

// package identity — values, not spellings
// a.Cfg.N == 1, b.Cfg.N == 2; UseA (import a.Cfg) must read 1, UseB must read 2

// partially-qualified nested reference
class W { static class P { static class Counter { void bump(){...} } }
          void m(){ P.Counter c = new P.Counter(); c.bump(); } }               // must not be an import facade
```

## Codex parallel audit 2026-08-25

The current tree has local fixes for most concrete repros in this file:
adjacent nested type path boundaries are preserved, declared method names are
allocated against occupied constructor/method names, descendant aliases are
qualified for nested type references, and the boxed dyn-trait nested-name route
uses `normalize_emitted_java_type_name`. A 2026-08-25 follow-up also pinned the
batch analysis maps directly: `batch_analysis_keys_keep_same_simple_name_package_peers_distinct`
asserts that `UsageFacts::methods_by_java_key` and `SignaturePlan` both keep
`a.Item::who` and `b.Item::who` under Java-owner keys, not `Item` or emitted
`AItem`/`BItem`. The remaining live problem is broader: there is still no
first-class package-aware Java type identity object carried through the whole
pipeline.

Follow-up 2026-08-25: the explicit static receiver check no longer asks the raw
`SourceContextOwnerKey::matches_context_type` predicate before the
ambiguity-aware context lookup. A bare `Item.m()` receiver in a context with both
`a.Item` and `b.Item` now fails closed instead of matching whichever resolved
owner is being checked; exact emitted (`AItem`) and package-qualified
(`b.Item`) receivers still match. Regression:
`lower::tests::explicit_static_receiver_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-25: context static type alias seeding now computes simple-name
ambiguity across the support-local and source-summary programs together before
inserting aliases. A support `a.Item` and summary `b.Item` therefore no longer
leave a live `Item`/`item` alias pointing at whichever context was seeded first;
exact dotted aliases remain available. Regression:
`lower::tests::context_alias_seeding_refuses_cross_program_ambiguous_simple_name`.
This is still a local ambiguity guard, not the first-class package-aware type
identity model described below.

Representative remaining string-owner surfaces:

- `java_naming::rust_name_for_java_named_type` still maps from name text to Rust
  spelling.
- `resolve::{resolve_method, resolve_constructor}` still combine string owners
  and local context rather than consulting one resolved entity map.
- `usage::UsageMethodKey`, `signature::MethodKey`, and many call-site helpers
  still accept owner strings. The batch duplicate pre-pass currently keeps
  same-simple-name package peers distinct for the pinned Java-key path, but the
  type system cannot express the invariant yet.

Smallest implementation slice: introduce a package-aware `JavaTypeKey` (or one
canonical owner helper returning such a key) and migrate the remaining
Rust-name/usage owner strings first: `usage::UsageMethodKey`,
`signature::MethodKey`, and construction sites that currently pass
`ty.name.clone()`. Keep emitted Rust names unchanged in that slice. The goal is
to make the already-pinned Java owner invariant explicit instead of relying on
pre-pass string rewrites.

Focused tests for that slice:

- Extend `batch_analysis_keys_keep_same_simple_name_package_peers_distinct`
  when `UsageMethodKey`/`signature::MethodKey` gain a typed owner, so it checks
  those maps in addition to the Java-key lookup.
- A context ambiguity unit extending `find_context_type_by_key` for exact and
  package-qualified owner lookups. Follow-up 2026-08-25: the lookup guard is now
  symmetric for package peers: `a.Item`/`AItem` resolve to `a.Item`,
  `b.Item`/`BItem` resolve to `b.Item`, and bare `Item` fails closed both within
  one program and across the combined lowering context. The emitted-context
  companion guards check the same four owner spellings. Regression filters:
  `lower::tests::context_type_lookup_refuses_ambiguous_simple_package_peer` and
  `lower::tests::context_type_lookup_refuses_cross_program_ambiguous_simple_package_peer`.

Follow-up 2026-08-25: the batch package-peer regression now compiles the
generated source items for `a.Item`, `b.Item`, and `Main` together with minimal
shared support stubs. This keeps the rendered-name assertions but also catches
duplicate emitted item names and wrong cross-file references for the
same-simple-name package-peer shape. Regression:
`batch_translation::package_qualified_source_type_references_keep_package_identity`.

Follow-up 2026-08-25: context type lookup no longer becomes ambiguous again
when each individual context program is safe but the combined lowering context
is not. `find_context_type_by_key_in_lower_ctx` now collects matches across the
primary, support-local, and source-summary programs before choosing a type:
exact/package-qualified/emitted Rust-name matches still win, but a bare simple
name such as `Item` fails closed when `a.Item` and `b.Item` live in different
context programs. The narrow source-context/static-owner helper paths now use
that combined lookup instead of priority-chaining the three programs.
Regression:
`lower::tests::context_type_lookup_refuses_cross_program_ambiguous_simple_package_peer`.
This is another ambiguity guard; the first-class `JavaTypeKey`/typed owner
migration remains the broader open model work.

Follow-up 2026-08-25: static source-context fallback signatures no longer carry
the receiver spelling forward after resolving a context type. A unique bare
receiver such as `Util` may resolve to `a.Util`, but the returned
`ContextFallbackMethodSignature.owner` now uses the declaration owner
`ty.name`, and the receiver-plan lookup asks the same resolved owner. Regression:
`lower::tests::source_context_static_fallback_uses_resolved_owner_not_receiver_spelling`.

Follow-up 2026-08-25: receiver-method source-context fallback now follows the
same rule. A local receiver typed as bare `Util` may resolve through the
combined context lookup to support `a.Util`; the fallback signature now returns
`a.Util`, not the local type spelling, and the emitted-context decision is based
on that resolved declaration owner. Regression:
`lower::tests::source_context_receiver_fallback_uses_resolved_owner_not_receiver_type_spelling`.

Follow-up 2026-08-25: resolved static source-context fallback now also
canonicalizes through the resolved context declaration. If both the explicit
receiver spelling and `ResolvedCall.declaring_type` are bare names, they are
accepted only when the combined context lookup resolves both to the same unique
type, and the returned fallback owner is that declaration name (`a.Util`), not
the bare spelling. The existing ambiguous `Item` guard still fails closed.
Regression:
`lower::tests::resolved_static_fallback_uses_resolved_context_owner_not_declaring_spelling`;
guarded by `lower::tests::explicit_static_receiver_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-25: static primitive/string storage reference folding now
resolves explicit owners through the combined context lookup and renders from
the resolved declaration owner. `AItem.KIND` and `a.Item.KIND` therefore both
emit through `AItem`, while ambiguous bare `Item.KIND` declines the fold instead
of priority-chaining to whichever package peer appears first or falling back to
the current type. Regressions:
`lower::tests::static_storage_reference_uses_resolved_context_owner_identity`
and `lower::tests::static_storage_reference_refuses_ambiguous_simple_owner`.

Follow-up 2026-08-26: static-array element writes now use the same combined
context lookup. `Item.VALUES[i] = value` no longer priority-chains
`program`/`support_local_type_program`/`source_summary_type_program` and silently
picks the first same-simple package peer; ambiguous bare owners fail closed,
while exact package-qualified and emitted Rust owner names still resolve.
Regression:
`lower::tests::static_storage_element_assignment_refuses_ambiguous_simple_owner`.

Follow-up 2026-08-26: interface trait argument shaping now uses the combined
context owner lookup too. A bare interface owner such as `Item` no longer
priority-chains to the first of `a.Item`/`b.Item` when selecting trait parameter
shapes for lowering; ambiguous package peers fail closed before signature lookup.
Regression:
`lower::tests::interface_argument_shapes_refuse_cross_program_ambiguous_simple_owner`.

Follow-up 2026-08-26: the erased-name hierarchy helper and source enum/interface
bridges now use the combined context lookup as well. Generic spellings are
erased before lookup (`Item<T>`), exact/package-qualified and emitted owner names
still resolve, and ambiguous bare package peers fail closed instead of selecting
the first visible context program. The enum target-interface bridge now sees
summary-only interfaces and compares implemented interface names through the
canonical owner key. `lower_interface_super_default_call` also routes its current
type and named interface lookup through the combined helper. Regressions:
`lower::tests::erased_context_type_lookup_refuses_cross_program_ambiguous_simple_owner`,
`lower::tests::current_type_implements_target_refuses_ambiguous_bare_super_owner`,
`lower::tests::enum_trait_lookup_refuses_cross_program_ambiguous_simple_owner`,
and
`lower::tests::enum_target_interface_lookup_uses_summary_and_refuses_ambiguous_target`.

Follow-up 2026-08-26: source interface supertrait bounds now use the same
combined erased-name context lookup. `interface Child extends Parent` fails
closed when support and summary context provide different `a.Parent`/`b.Parent`
peers, while exact `a.Parent` and exact summary `b.Parent` still produce the
expected Rust supertrait bound. Regression:
`lower::tests::interface_supertrait_bounds_use_combined_context_lookup`.

Follow-up 2026-08-26: the `context_type_key_is_summary_only` predicate now asks
the combined visible context whether the summary declaration is uniquely
selected, and it treats support-local matches as not summary-only. This prevents
summary-boundary return-shape and super-boundary handling from being enabled for
a bare owner that is actually ambiguous across support and summary context.
Regression:
`lower::tests::context_type_key_is_summary_only_accounts_for_support_context`.

Follow-up 2026-08-26: source-summary inherited method lookup now follows one
combined context edge per superclass step instead of checking program, then
summary/support, then program again. A source type extending bare `Parent` no
longer picks the summary `b.Parent` method when support also provides
`a.Parent`; exact owner spellings still resolve. Regression:
`lower::tests::source_summary_inherited_method_lookup_refuses_ambiguous_super_owner`.

Follow-up 2026-08-26: the current-type InputStream facade predicate now uses
the combined context lookup instead of program/support/summary priority
chaining. A bare current type `Item` no longer inherits InputStream behavior from
support `a.Item` when summary `b.Item` is also visible; exact `a.Item` still
resolves. Regression:
`lower::tests::current_type_input_stream_facade_lookup_refuses_ambiguous_context_type`.

Follow-up 2026-08-26: receiverless resolved static owner retargeting now
requires the resolved declaring type to be unique in the combined context and
renders from that resolved declaration name. A bare `Item` declaring type no
longer rewrites `Self::make` to whichever same-simple context owner is found;
exact `a.Item` still retargets to `AItem::make`. Regression:
`lower::tests::resolved_static_owner_retarget_refuses_ambiguous_context_declaring_type`.

Follow-up 2026-08-26: the combined context lookup itself now separates true Java
owner identity from emitted Rust-name aliases. A real source/context type named
`AItem` wins over the alias produced for `a.Item`, even when the two live in
different visible context programs; generated aliases remain a fallback instead
of an `Exact` match that silently keeps the first scanned type. The emitted
context lookup uses the same buckets. Regressions:
`lower::tests::context_type_lookup_prefers_real_owner_name_over_emitted_alias`
and
`lower::tests::emitted_context_type_lookup_prefers_real_owner_name_over_emitted_alias`.

Follow-up 2026-08-25: method-reference planning no longer drops every owner
segment before the final `.`/`$`. Package-qualified and nested-qualified method
references now use the same Java type-name mangler as ordinary type rendering,
so `a.Mapper::tag`, `b.Mapper::tag`, `K.AB::tag`, and `K.A.B::tag` stay distinct
at the plan boundary. Regression:
`method_reference_plan::tests::method_reference_owner_preserves_package_and_nested_identity`.

Follow-up 2026-08-25: `SignaturePlan`'s boolean source-method presence probe now
uses the same unique-match path as the lookup helpers. The direct lookup helpers
already failed closed when a bare owner such as `Item` matched both `a.Item` and
`b.Item`, but `has_method_signature_by_base_name_accepting_arity` still used
`.any(...)` and could report that a method existed for the ambiguous simple
owner. Exact package-qualified owners still resolve. Regression:
`signature::tests::method_signature_presence_fails_closed_for_ambiguous_simple_package_owner`.

Follow-up 2026-08-25: batch erased-object display support no longer leaks
sibling-file source owners into a per-file generated module. The global batch
support program still drives semantic/signature analysis, but
`java_erased_any_to_string` source downcast arms are now built from the current
resolved program for batch emission. This avoids unqualified arms such as
`downcast_ref::<BDuplicateOwner>()` inside the `a.DuplicateOwner` module where
`BDuplicateOwner` is not defined. Regression:
`batch_translation::batch_duplicate_simple_type_names_keep_distinct_signature_keys`
now compile-checks both emitted files.

Follow-up 2026-08-26: source/JDK subtype reachability now uses the same
context-key lookup policy for the actual type and fails closed for ambiguous
simple target names. A unique simple package type such as `Child` can still
resolve to `a.Child`, but `Child` or `Base` no longer silently means whichever
of `a.Child`/`b.Child` or `a.Base`/`b.Base` happens to match during hierarchy
traversal. Regressions:
`lower::tests::source_subtype_reachability_uses_unique_context_simple_name` and
`lower::tests::source_subtype_reachability_refuses_ambiguous_context_simple_name`.

Follow-up 2026-08-26: the source-class upcast and erased-object source-supertype
`instanceof` hierarchy guards now use the combined context lookup as well. A
bare `Child -> Base` hierarchy check fails closed when support and summary
context expose `a.Child`/`b.Child` or `a.Base`/`b.Base`, while exact
package-qualified owners still trigger the loud hierarchy-lowering refusal.
Regressions:
`lower::tests::source_class_upcast_hierarchy_guard_refuses_ambiguous_context_owner`
and
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_context_target`.

Follow-up 2026-08-26: emitted-context static argument shaping no longer falls
back from the combined emitted-owner lookup to per-program emitted Rust-name
searches. `emitted_source_method_declared_arg_plan` and
`static_context_owner_is_emitted` now use the same combined lookup that refuses
ambiguous simple package peers, so a bare emitted owner cannot regain
program/support/summary priority order during argument-slot selection.
Regressions:
`lower::tests::static_context_owner_emitted_check_uses_combined_ambiguous_lookup`
and
`lower::tests::emitted_source_method_arg_plan_uses_combined_emitted_owner_lookup`.

Follow-up 2026-08-26: source-call exception/fallibility owner recovery now uses
the same combined lookup policy. `source_call_target_owner_candidates` no
longer falls back through program/support/summary emitted-name searches, and a
lowered receiver path such as `Item::...` is not treated as an arbitrary first
package peer when both `a.Item` and `b.Item` are in context. Regressions:
`lower::tests::source_call_owner_candidates_refuse_ambiguous_emitted_owner`.

Follow-up 2026-08-26: same-outer inner-constructor rewriting now uses the
combined emitted-context lookup instead of its own rendered-Rust-name
program/support/summary walk. A bare constructor owner such as `Inner` no
longer selects the first of `a.Inner`/`b.Inner` when deciding whether to inject
or replace outer handles; unique emitted aliases such as `AInner` still resolve.
The now-redundant rendered-name helper was removed. Regression:
`lower::tests::same_outer_inner_constructor_owner_refuses_ambiguous_emitted_owner`.

Follow-up 2026-08-26: the `java.util.Objects.equals` recognizer no longer has
its own current-program-only emitted-name escape hatch. A translated/support
source type named `Objects` in any context program now blocks the JDK
`Objects.equals` lowering instead of letting support/summary owners be treated
as the platform class. Regression:
`lower::tests::java_objects_equals_refuses_source_objects_owner_in_context`.

Follow-up 2026-08-26: optional receiver member field-shape lookup no longer
queries the owner's simple name after the declared owner. `a.Item::field` cannot
borrow `Item::field` from a same-simple-name package peer when deciding optional
field unboxing/cloning behavior. Regression:
`lower::tests::signature_field_shape_for_member_does_not_borrow_simple_package_peer`.

Follow-up 2026-08-26: recursive optional field type identity now compares the
unwrapped Java type name exactly. It no longer treats `a.Node`, `b.Node`, or the
rendered Rust alias `ANode` as interchangeable when deciding whether a
recursive optional field needs an extra box/unbox adjustment. Regression:
`lower::tests::java_type_names_match_after_optional_uses_exact_java_identity`.

Follow-up 2026-08-26: unresolved current-source null-overload detection no
longer falls back from a qualified `current_type` to its simple name. A missing
owner such as `missing.Item` cannot inspect `a.Item` overloads merely because
the simple segment matches. Regression:
`lower::tests::current_null_overload_check_refuses_ambiguous_context_owner`.

Follow-up 2026-08-26: source instance-field name recovery now uses the combined
context lookup instead of independently probing program/support/summary. A bare
owner/current type such as `Item` cannot borrow `a.Item`'s keyword/collision
field spelling when `b.Item` is also in context; exact `a.Item` still gets the
owner-aware spelling. Regressions:
`lower::tests::source_instance_field_name_refuses_cross_context_simple_owner`
and
`lower::tests::current_type_source_field_name_refuses_cross_context_simple_owner`.

Follow-up 2026-08-26: translator-owned support code no longer depends on an
unqualified Rust prelude `Box` when rendering recursive exception causes and
erased-object runtime/facade helpers. A translated Java class named `Box<T>` can
still own the Rust item `Box<T>`, but it no longer makes infrastructure
spellings such as `JavaException.cause` or `java_clone_erased_object_ref`
resolve to the source class. Compile-backed regressions:
`source_generic_class_bound_method_call_uses_bound_surface`,
`source_class_bound_inherited_mutator_forwards_to_child_method`, and
`source_generic_class_bound_overload_call_uses_bound_rust_name`.

Follow-up 2026-08-26: the same qualification rule now covers object-safe trait
helper surfaces. Generated `anonymous`, `Clone for dyn Trait`, and
`clone_box` forwarding support use `std::boxed::Box<dyn Trait>` and
`std::boxed::Box::new`, so a source class named `Box<T>` no longer captures the
translator-owned heap-box helper type. Regression:
`source_generic_class_preserves_type_params_and_interface_bounds`.

Follow-up 2026-08-26: erased-source static retargeting now uses the combined
context type lookup instead of current-program-only owner checks. Static
receiver classification can resolve support/summary owners, and a simple owner
such as `Item` is rejected when package peers exist across context programs
instead of retargeting through whichever program happened to be checked first.
Regressions:
`lower::tests::explicit_source_static_receiver_type_uses_combined_context_lookup`,
`lower::tests::explicit_source_static_receiver_type_refuses_cross_context_simple_owner`,
and
`lower::tests::erased_source_static_retargeting_uses_combined_context_lookup`.

Follow-up 2026-08-26: erased-source static self-call lowering now uses the
same combined context lookup before rewriting `Self::method`/fallback-self
calls to owner-qualified free helpers. This removes another current-program-only
owner check from the erased-static path. Regression:
`lower::tests::erased_source_static_self_call_uses_combined_context_lookup`.

Follow-up 2026-08-26: the remaining "emitted but not current-program" static
owner predicate is now centralized as
`static_owner_key_is_non_program_emitted_context`. The helper still deliberately
asks whether the owner is absent from `ctx.program`, but the policy is no longer
duplicated in argument-shape and fallback-signature selection. Regression:
`lower::tests::non_program_emitted_context_owner_check_is_combined_but_current_program_aware`.

Follow-up 2026-08-26: nested owner candidate recovery now resolves the current
owner once through the combined, ambiguity-aware context lookup before walking
nested declarations. A bare `Item.Inner` candidate no longer leaks out of
`a.Item` or `b.Item` when the current type is the ambiguous simple owner
`Item`; exact `a.Item` still contributes `a.Item.Inner`. Regression:
`lower::tests::nested_owner_candidates_refuse_ambiguous_current_owner`.

Follow-up 2026-08-26: source-supertype conversion impl lookup and anonymous
interface constructor trait lookup now use the combined ambiguity-aware context
lookup. They no longer search only selected context programs or accept the
first simple-name package peer when exact owner identity is unavailable.
Regressions:
`lower::tests::source_supertype_conversion_impls_use_combined_context_super_lookup`
and
`lower::tests::anonymous_interface_constructor_trait_name_uses_combined_context_lookup`.

Follow-up 2026-08-26: the batch package-identity generated-crate harness now
uses a `JavaClass<T>` support stub with the same `get_name()` surface as the
runtime support emitted by the translator. This was a test-support drift exposed
by the source `Object.toString()` fallback moving through
`self.get_class().get_name()`, not a package-identity regression. Regressions:
`batch_translation::package_qualified_source_type_references_keep_package_identity`
and
`batch_translation::imported_source_enum_value_of_keeps_package_owner_identity`.

Follow-up 2026-08-26: `static_owner_receiver_is_emitted_program_type` no
longer checks raw equality against `ctx.program.types`. It now uses the
program-local emitted-context lookup, preserving current-program-only policy
while accepting emitted/package-qualified owners and refusing ambiguous bare
simple package peers. Regression:
`lower::tests::static_owner_receiver_program_type_check_uses_emitted_owner_lookup`.

Follow-up 2026-08-26: source-subtype conversion availability and source
supertype conversion target emission now resolve owners through the same
combined unique context-key policy instead of exact program/support/summary
priority checks. This admits emitted/package-qualified aliases such as
`AChild`/`ABase`, preserves the existing context-emission gate for
support/summary targets, and fails closed for ambiguous simple owners. The
external-super path also receives the resolved actual declaration instead of
looking it up again by text.
Regression:
`lower::tests::source_subtype_conversion_uses_combined_context_identity`.

Follow-up 2026-08-26: abstract-interface method stub generation now resolves
implemented interface names through the combined erased-name context lookup
instead of searching only `support_local_type_program` with a simple-tail
fallback. Interfaces declared in the current program or summary context are
visible, and ambiguous simple package peers fail closed before a wrong abstract
stub is emitted. Regression:
`lower::tests::abstract_interface_method_stubs_use_combined_context_lookup`.

Follow-up 2026-08-26: external constructor support no longer emits a bare simple
type alias when two constructed external owners collapse to the same Rust simple
name. `new a.Foo()` plus `new b.Foo()` still emits `AFoo` and `BFoo`
placeholders, but suppresses `pub type Foo = ...` because the alias is
non-injective. Regression:
`stdlib::external_facade_support_tests::external_constructor_support_suppresses_ambiguous_simple_alias`.

Follow-up 2026-08-26: source-super-constructor flattening now resolves the
superclass through the same combined erased-name context lookup and feeds
inherited-field planning from borrowed current/support/summary context programs.
An exact current-context superclass such as `a.Parent` is selected even when a
support peer `b.Parent` exists, while a bare ambiguous `Parent` fails closed
instead of flattening through the first support match. Regressions:
`lower::tests::source_super_constructor_uses_selected_context_superclass` and
`lower::tests::source_super_constructor_refuses_ambiguous_context_superclass`.

Follow-up 2026-08-26: source hierarchy reachability callers no longer discard a
current-program hierarchy edge after resolving actual and target through the
combined context. `source_class_upcast_requires_hierarchy_lowering`,
exception-self upcast detection, and exact erased-object cast refinement now use
a lower-context reachability helper instead of asking only support-local
context. Regressions:
`lower::tests::source_class_upcast_hierarchy_guard_uses_current_program`,
`lower::tests::current_exception_supertype_guard_uses_current_program`.

Follow-up 2026-08-26: erased-object source-supertype cast/`instanceof` guards
now pass the resolved target declaration name into the hierarchy scan rather
than the original target text. An emitted alias such as `ABase` can resolve to
`a.Base`, and the source subtype search must use `a.Base` or it misses the
already-resolved package identity. Regression: the emitted-alias assertion in
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_context_target`
and `lower::tests::erased_cast_hierarchy_guard_uses_resolved_target_alias`.

Follow-up 2026-08-26: loud source-class upcast diagnostics now use the resolved
actual and target owner names, not the raw actual/target text supplied to the
slot or cast path. An emitted alias pair such as `AChild`/`ABase` now reports
`a.Child to a.Base`, matching the identity used by the hierarchy guard.
Regression:
`lower::tests::source_class_upcast_loud_names_use_resolved_context_identity`.

Follow-up 2026-08-26: source callable fact keys now have an explicit
`JavaOwnerKey` construction boundary. Source-declared method usage/signature
insertion, declaration-based lookups, and already-resolved lower-context source
lookups use `source_method_for_type` or `source_method_for_owner`; the raw
`source_method(owner: String, ...)` constructor is private to
`java_signature.rs`. This does not change the stored key shape yet, but it
removes the public habit of rebuilding source owner identity from arbitrary
text at the fact boundary. Regressions:
`signature::tests::method_signature_presence_fails_closed_for_ambiguous_simple_package_owner`,
`signature::tests::required_method_signature_does_not_borrow_distinct_package_peer`,
`signature::tests::required_field_shape_does_not_borrow_distinct_package_peer`,
and `lower::tests::static_owner_receiver_program_type_check_uses_emitted_owner_lookup`.

Follow-up 2026-08-26: the remaining direct `JavaCallableKey { ... }`
construction outside `java_signature.rs` was collapsed behind
`JavaCallableKey::resolved_source_method`. Resolved-call argument-shape planning
and resolved-call return-shape lookup now share the same owner and erased
argument construction path instead of rebuilding the key field-by-field in
`call_plan.rs` and `expr_shape.rs`. Regression:
`slot_shape_design::callable_analysis_shell_reaches_lowering_context`.

Follow-up 2026-08-26: explicit source static receiver recovery now returns the
resolved declaration owner, not the raw receiver spelling that happened to
resolve. A unique simple receiver such as `Util` and an emitted alias such as
`AUtil` both recover `a.Util`, so erased-source static helper calls line up with
helper declarations named from the declaration owner. Ambiguous simple owners
still fail closed through the combined context lookup. Regression:
`lower::tests::explicit_source_static_receiver_type_uses_combined_context_lookup`.

Follow-up 2026-08-26: representation-lowering source type lookup no longer
uses first match for a bare simple name. The helper now collects exact owner,
emitted Rust alias, and simple-name candidates recursively and returns only a
unique match at the highest-precedence tier. A bare `Named` with both `a.Named`
and `b.Named` in the source program now fails closed instead of silently using
the first package peer, while `a.Named` and `ANamed` still resolve. Regression:
`representation_lowering::type_lookup_tests::find_type_decl_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-26: the co-translated context same-simple-name regression is
now compile-backed, not only text-backed. A sibling test translates the caller
with both package peers visible, translates each context peer with the other
peer visible so their emitted names match the caller, stitches the three
generated bodies into one crate, and `cargo check`s it. This proves the
single-source/context references to `AItem::new`, `BItem::new`, `AItem::KIND`,
and `BItem::KIND` resolve in a generated crate. Regression:
`slot_shape_design::cotranslated_same_simple_name_package_peers_compile_with_context_bodies`.

Follow-up 2026-08-26: storage-strategy declared-source type lookup no longer
uses the first same-simple-name match for bare type names. Storage planning now
tries exact owner text, then a same-package match inferred from the containing
owner, then a unique simple-name fallback; ambiguous simple package peers fail
closed. This prevents a `b.Holder` field declared as bare `Item` from charging
static reachability or storage escape to `a.Item` merely because `a.Item` sorts
first. Regression:
`storage_strategy::tests::storage_strategy_simple_owner_uses_same_package_before_first_simple_match`.

Follow-up 2026-08-28: the duplicate-program type pre-pass now checks the Rust
declaration names produced by its planned Java-owner rewrites, not only the
source simple names. If ordinary package qualification would still collide with
another emitted declaration, such as default-package `ABC` versus package peer
`a.b.C` projected as `ABC`, the pre-pass assigns a collision-safe internal owner
like `a.b.JavaType.C` and rewrites same-file bare references plus cross-file
fully-qualified references to match. This is still a local generated-name guard,
not the first-class package-aware owner model described above. Regression:
`lib::tests::duplicate_program_prepass_avoids_projected_rust_type_name_collisions`.

Follow-up 2026-08-28: `signature_field_shape_for_type_aliases` no longer lets a
package-qualified owner miss fall through to its simple-name peer. A lookup for
`a.Item::value` must not borrow `Item::value` just because the final segment
matches; simple-owner compatibility remains for unqualified owner text only.
Regression:
`lower::tests::signature_field_shape_for_type_aliases_does_not_borrow_simple_package_peer`.

Follow-up 2026-08-28: nested simple type aliases are now suppressed against the
Rust names of emitted method-local capture types, not only against source
program type declarations. A source nested type and a capturing local class can
both be named `Inner`; the generated module now keeps the real emitted structs
and avoids adding a colliding `pub type Inner = Outer_Inner` alias. This is a
local emitted-name collision guard, not the first-class Java owner identity
model this audit still asks for. Regression:
`slot_shape_design::nested_simple_alias_is_suppressed_by_emitted_local_capture_type`.

Follow-up 2026-08-28: the adjacent local-class shadowing constructor path is
fixed for the same `Inner`/`Outer.Inner` shape. When a method-local capturing
class shadows a nested member class, the local declaration refinement now keeps
the initializer's local-capture type instead of the pre-qualified member slot,
and the source-constructor-for-slot fast path passes capture constructor
arguments just like the ordinary `Expr::New` lowering path. The generated code
therefore emits `let local: Inner = Inner::new(base);` instead of a
`compile_error!` upcast from `Inner` to `Outer.Inner`. Regression:
`slot_shape_design::local_class_constructor_shadows_nested_member_class`.

Follow-up 2026-08-28: `stdlib` source-summary type lookup no longer returns the
first package peer for a bare simple owner. `find_source_type_decl` now tries
exact Java owner text first and only accepts simple-name/Rust-alias
compatibility when it is unique; ambiguous `a.Item`/`b.Item` queried as `Item`
returns `None`. Constructor facade and imported-static summary lookups also go
through `find_source_summary_owner_decl`, which accepts a package-qualified miss
through the owner's simple name only when the summary program is for that same
package. This fences inherited source-summary method signatures, source-summary
constructor facade methods, and imported static summary methods without
introducing the broader first-class owner key. Regressions:
`stdlib::source_summary_method_identity_tests::find_source_type_decl_refuses_ambiguous_simple_package_peer`,
`stdlib::source_summary_method_identity_tests::source_summary_constructor_lookup_does_not_borrow_other_package_simple_owner`,
and
`stdlib::source_summary_method_identity_tests::source_summary_imported_static_lookup_does_not_borrow_other_package_simple_owner`.

Follow-up 2026-08-28: the adjacent `stdlib` program-type lookup helper now
uses the same exact-or-unique policy instead of returning the first simple-name
package peer. `find_program_type_by_key` tries exact canonical owner text first,
then accepts simple-name compatibility only when there is one match. This keeps
source-summary hierarchy traversal from inheriting methods or static-final
fields through an arbitrary `a.Base`/`b.Base` peer when a child only says
`extends Base` and the owner cannot be disambiguated locally. Regressions:
`stdlib::source_summary_method_identity_tests::find_program_type_by_key_refuses_ambiguous_simple_package_peer`
and
`stdlib::source_summary_method_identity_tests::source_summary_method_hierarchy_refuses_ambiguous_simple_super_owner`.

Follow-up 2026-08-28: imported static-owner support discovery now respects
lexical shadowing instead of computing one lossy method-wide name set before
owner collection. A later local with the same spelling as an imported owner no
longer suppresses an earlier owner call, and a nested-block local no longer
suppresses a call after the block. This is still a scoped collector fix rather
than the first-class Java owner identity model, but it removes one more place
where pure spelling decided whether an external owner existed. Regressions:
`stdlib::external_facade_support_tests::imported_static_owner_call_before_shadowing_local_is_collected`
and
`stdlib::external_facade_support_tests::imported_static_owner_shadow_does_not_escape_nested_block`.

Follow-up 2026-08-28: generic source type-bound lookup in `item_assembly.rs`
no longer picks the first package peer for a bare simple bound. Type-parameter
bound projection now tries exact Java owner text, then emitted Rust alias, and
only then accepts a simple-name/Rust-simple-name match when it is unique across
the support programs. The same ambiguity-aware helper drives `Into<...>`
supertype bounds for source class bounds, so `T extends Base` does not borrow
`a.Base` or `b.Base` by support-program order when both exist. Regressions:
`item_assembly::tests::source_type_bound_decl_refuses_ambiguous_simple_package_peer`,
`item_assembly::tests::source_type_param_into_supertype_bounds_refuses_ambiguous_simple_package_peer`,
and the existing generated-crate check
`idiom_gaps::source_generic_class_bound_overload_call_uses_bound_rust_name`.

Follow-up 2026-08-28: external field fact lookup now also fails closed for an
ambiguous package-qualified simple-owner alias. Exact package owners still
resolve (`a.Item.KIND`, `b.Item.KIND`), but a bare `Item.KIND` query no longer
returns whichever package peer happens to win candidate ordering when both
`a.Item` and `b.Item` expose `KIND`. This fixes the same owner-key shape at the
fact lookup boundary used by static constant/member-access planning. Regression:
`semantic_facts::external_field_lookup_refuses_ambiguous_simple_package_owner`.

Follow-up 2026-08-28: in-scope type-parameter detection in lowering now uses
the combined context owner lookup instead of checking only the primary source
program. A support-local current type such as `a.Box<T>` is recognized when the
source program is empty, while a bare ambiguous `Box` owner still fails closed
when support and summary programs contain package peers. Regression:
`lower::tests::in_scope_type_parameter_lookup_uses_combined_context_and_refuses_ambiguous_owner`.

Follow-up 2026-08-28: derive-support checks in `item_assembly.rs` no longer
recover a source declaration by taking the first Java owner whose emitted Rust
name matches a rendered `RustType::Named`. The reverse lookup now returns a
declaration only when the emitted Rust name maps to one distinct Java owner, so
collisions like `a.B.C` and `AB.C` both rendering as `AB_C` fail closed instead
of letting traversal order decide `Default`/field-walk behavior. Regression:
`item_assembly::tests::derive_support_type_lookup_refuses_ambiguous_emitted_rust_name`.

Follow-up 2026-08-29: inherited source member lookup no longer walks support
types with first-match simple-owner aliasing. `find_type_decl` now prefers
exact Java owner text and otherwise accepts alias/simple-name matches only when
they are unique, so `extends Base` with both `a.Base` and `b.Base` visible does
not inherit whichever package peer appears first. Package-qualified `extends
b.Base` still resolves. This also fences interface default traversal because it
shares the same lookup helper. Regression:
`inherited_members::tests::inherited_source_methods_refuse_ambiguous_simple_package_super`.

Follow-up 2026-08-29: slot-policy source-method usage lookups now use the
resolver-backed Java callable key via `UsageFacts::source_method_usage(ty,
method)` instead of reconstructing `UsageMethodKey { type_name, method_name }`
from plain strings. This migrates borrowed-content parameter policy off one
more non-injective owner/method spelling path. Regressions:
`slot_policy::tests::borrowed_return_param_policy_uses_binding_facts_not_name_assignment_scan`
and `layering::slot_policy_uses_java_keyed_source_method_usage_lookup`.

Follow-up 2026-08-29: anonymous declared-return dyn-trait constraint planning
no longer reaches through `slot_render` just to render a Java owner name.
`signature.rs` now uses `java_naming::rust_name_for_java_named_type`, keeping
source analysis and signature planning independent of slot rendering for this
path. Regression:
`layering::plan_layers_keep_rust_rendering_out_of_source_analysis`.

Follow-up 2026-08-29: declaration-scoped usage lookups moved another step away
from `UsageMethodKey { type_name, method_name }` string reconstruction.
`UsageFacts` now exposes `source_method_usage_for_owner(JavaOwnerKey, method)`;
`storage_strategy`, `semantic_analysis`, and `signature` use Java-callable keys
when they already have `TypeDecl`/`MethodDecl` context. `aliasing` now uses
`JavaCallableKey::resolved_source_method` for resolved receiver-mutating calls,
so overload identity, erased argument types, and staticness decide whether the
call mutates its receiver. This fixes the live case where a read-only overload
could inherit mutability from a same-name mutating overload via the legacy
Java-name map. Regressions:
`aliasing::tests::receiver_mutation_lookup_uses_resolved_overload_identity`,
`storage_strategy::handle_forwardable_surface_tests::camel_case_mutating_interface_method_blocks_forwarding`,
and `layering::declaration_context_usage_lookups_use_java_callable_keys`.

Follow-up 2026-08-29: semantic unresolved-target external-effect guessing now
distinguishes "not a source owner" from "ambiguous source owner". The receiver
owner lookup used before synthesizing an external API effect fact accepts exact
source owners, emitted Rust owner names, and simple names only when the tier is
unique; a bare `Item` with both `a.Item` and `b.Item` visible now fails closed
instead of seeding an external `Item.transform` guess from traversal order.
Regression:
`semantic_facts::external_api_effect_guess_refuses_ambiguous_source_simple_owner`.

Follow-up 2026-08-29: signature inherited-field owner lookup no longer returns
the first support declaration whose external alias matches a rendered type
name. `find_signature_type_decl` now prefers exact Java owner text and accepts
alias/simple-name matches only when the match is unique, so a bare `Base`
cannot borrow constructor-field constraints or type parameters from whichever
of `a.Base` / `b.Base` appears first. Regression:
`signature::tests::signature_type_lookup_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-29: external display-capability facts no longer grant their
simple-name capability to every package peer. A modeled
`a.ExternalToken.toString(): String` now records `a.ExternalToken` and its Rust
surface `AExternalToken`, but not bare `ExternalToken`; erased-object
stringification can still detect the qualified source mention and emit the
`AExternalToken` downcast. Regressions:
`functional_interface_shape_tests::package_qualified_to_string_fact_does_not_mark_simple_peer_display_capable`
and
`tests/slot_shape_design.rs::erased_external_object_display_keeps_package_qualified_to_string_capability_distinct`.

Follow-up 2026-08-29: source supertype conversion field flattening now uses
the combined inherited-field lookup across support and source-summary programs
instead of asking each program separately and merging both answers. A child with
`extends Base` no longer inherits fields from both `a.Base` and `b.Base` when
the bare owner is ambiguous; package-qualified `extends b.Base` still inherits
the intended field. Regression:
`lower::tests::source_instance_field_names_for_conversion_refuses_cross_program_ambiguous_simple_super`.

Follow-up 2026-08-29: optional current-method display shape recovery no longer
consults signature return shapes with an unresolved owner spelling. The display
slot path now resolves `current_type` / resolved declaring-type candidates
through the ambiguity-aware lower-context type lookup before asking
`SignaturePlan::method_return_shape_by_rust_name`, so a stale simple-owner
signature entry for `Item.name()` is ignored when the visible context contains
both `a.Item` and `b.Item`. Exact package owners still recover optional display
shape. Regression:
`lower::tests::optional_current_method_display_shape_refuses_ambiguous_current_owner`.

Follow-up 2026-08-29: stream-lambda raw static-member recovery now resolves
`Owner.member` through the active `StreamRenderContext` before rendering the
Rust associated path. A raw compatibility leaf such as `Cfg.N` therefore sees
the same static type aliases as parsed `Expr::FieldAccess`, so an import alias
for `AB.C` emits `AB_C::N` instead of treating bare `Cfg` as already-resolved
Rust owner text. Regression:
`tests/slot_shape_design.rs::stream_pipeline_chain_uses_string_and_optional_shapes`.

Follow-up 2026-08-29: package-qualified stream static constants and external
constant support discovery now preserve the full owner key. The stream lambda
path lowers parsed and raw `a.Item.KIND` through owner `a.Item`, producing
`AItem::KIND`; the support collector asks `qualified_static_owner_java_name`
before falling back to a final field segment, so the same expression no longer
emits bogus external stubs such as `pub struct Item; pub const KIND: KIND =
KIND;`. Regressions:
`tests/slot_shape_design.rs::stream_lambda_emission_uses_structured_operator_rendering`
and
`tests/batch_translation.rs::stream_static_constant_support_keeps_package_qualified_owner_identity`.

Follow-up 2026-08-29: array-allocation defaultability now resolves the Java
element owner through the ambiguity-aware lower-context lookup before asking
item assembly whether the source type can derive `Default`. It no longer
renders `a.B.C`/`AB.C` to the shared emitted alias `AB_C` and probes each
program independently, so `new AB.C[n]` cannot borrow `a.B.C`'s derive answer
when the exact `AB.C` owner is a non-defaultable interface. Regression:
`lower::tests::array_element_default_lookup_refuses_cross_program_emitted_owner_collision`.

Follow-up 2026-08-29: erased-object source-supertype `instanceof` and cast
guards now scan subtype reachability through the combined lower context instead
of asking each visible program independently. The combined subtype walk refuses
ambiguous bare source names before applying simple-name equality, and skips the
stdlib subtype shortcut when the target is a visible source type. This closes
the case where `a.Child extends Base` could be treated as reaching `a.Base`
even though another visible `b.Base` made that bare edge ambiguous.
Regressions:
`lower::tests::erased_instanceof_hierarchy_guard_refuses_ambiguous_bare_super_edge`
and
`lower::tests::erased_cast_hierarchy_guard_refuses_ambiguous_bare_super_edge`.

Follow-up 2026-08-29: `context_type_key_is_in_source_summary` now asks the
combined lower-context owner lookup which program owns a type key, instead of
probing only the summary program and accepting a bare simple-name hit. A bare
`Item` is now rejected when support and summary programs both expose distinct
`Item` owners, while exact summary names and unambiguous emitted aliases still
classify as source-summary. Regression:
`lower::tests::context_type_key_is_in_source_summary_refuses_ambiguous_simple_owner`.

Follow-up 2026-08-29: explicit static receiver matching no longer accepts a
static type alias by asking the raw `SourceContextOwnerKey` simple-name matcher.
Aliases now resolve through the combined lower-context lookup and compare the
resolved owner declaration, so `Alias -> Item` fails closed when `a.Item` and
`b.Item` are both visible, while `Exact -> a.Item` still matches. Regression:
`lower::tests::explicit_static_receiver_refuses_ambiguous_simple_package_peer`.

Follow-up 2026-08-29: static-owner emitted/current-program predicates now share
an emitted context lookup that returns both the resolved type and the owning
`Program`. `static_owner_key_is_non_program_emitted_context` and
`static_owner_receiver_is_emitted_program_type` now classify by program pointer
identity instead of composing separate emitted/current-program string lookups.
Regressions:
`lower::tests::non_program_emitted_context_owner_check_is_combined_but_current_program_aware`
and
`lower::tests::static_owner_receiver_program_type_check_uses_emitted_owner_lookup`.

Follow-up 2026-08-29: usage-side resolved receiver boundary classification no
longer treats a bare static receiver type as matching a package-qualified
resolved declaring type by simple name. `receiver_static_type_matches_declaring_type`
now accepts exact matches and legacy bare-vs-bare matches, but fails closed for
`Item` vs `a.Item` in either direction unless an upstream package-aware layer
has already resolved the owner. Regressions:
`usage::tests::receiver_static_type_match_fails_closed_for_package_qualified_simple_name`
and
`usage::tests::resolved_receiver_boundary_refuses_package_qualified_simple_name_match`.
