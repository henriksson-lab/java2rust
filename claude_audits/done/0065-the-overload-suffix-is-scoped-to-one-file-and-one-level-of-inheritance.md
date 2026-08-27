# 0065 — The overload suffix is decided from a name *count* scoped to one file and one level of `extends`

Status: **fixed** — hierarchy-wide overload counts now use source symbols across
context files, transitive superclasses and implemented interfaces; lowered
source-overload retargeting uses typed arguments before arity fallback
Fixed: 2026-08-24
Verified: `cargo test --release --test idiom_gaps cross_file_overloaded_override_uses_context_hierarchy_names`;
`cargo test --release --test idiom_gaps interface_default_overloads_count_for_implementor_override_names`;
`cargo test --release --test idiom_gaps transitive_superclass_overloads_count_for_grandchild_override_names`;
`cargo check --release`

Original status: **validated** — 3 minimal repros; one crashes the translator, one panics
at runtime against a `java` oracle, one diverges silently
Severity: **highest of this round** — the cross-file case is a hard
`panic!` in `new-java2rust` itself, on a two-file, six-line Java input, in the
single most common Java shape there is (one public class per file, a subclass
overriding one of several inherited overloads)
Kind: identity modelling — reopens the *scope* of
[0009](../../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md)'s
own fix, not its design

Line numbers are from the working tree at audit time; prefer the symbol names.
Every repro in this file is staged under `repro/` next to it.


---

## Symptom

0009's closing update states that the audited collapse is fixed and that reopening
should require *"a concrete reproduced path"*. Here are three, all of the same
root: the fix decides a method's Rust name from a **count of how many methods
share that Java name**, and it computes that count over a set that is too small
in three independent directions.

```rust
// src/resolve.rs:815
fn resolve_method(method: &mut MethodDecl, counts: &HashMap<String, usize>, direct_superclass: Option<&str>) {
    let base = crate::parser::to_snake_case(&method.name);
    ...
    if counts.get(&method.name).copied().unwrap_or(0) <= 1 {
        method.rust_name = Some(base);            // bare name
        return;
    }
    if method.params.is_empty() { method.rust_name = Some(base); return; }
    let suffix = method.params.iter().map(|p| type_suffix(&p.ty, p.is_varargs)).collect::<Vec<_>>().join("_");
    method.rust_name = Some(format!("{base}_{suffix}"));
}
```

The `counts` are built by `method_counts_including_supertypes`
(`src/resolve.rs:666`) — the function 0009 added to make an override agree with
what it overrides:

```rust
let mut seen = HashSet::new();
let mut pending = ty.extends.clone().into_iter().collect::<Vec<_>>();
while let Some(super_name) = pending.pop() {
    ...
    for key in [super_name.as_str(), simple.as_str()] {
        if let Some(super_counts) = program_counts.get(key) { ... }
    }
}
```

Three gaps are visible in those seven lines:

1. `pending` is seeded from `ty.extends` and **never pushed to**, so only the
   *direct* superclass is counted. A grandparent's overloads are invisible.
2. `ty.implements` is **never consulted**, so an interface's overloads are
   invisible.
3. `program_counts` is built in `resolve_program_with_source_symbols_and_facts`
   (`src/resolve.rs:200-204`) from `program.types` only — the **primary
   compilation unit**. A superclass in another file contributes nothing.

Gap 3 is the one that matters: Java's one-public-class-per-file convention means
that in a real corpus the supertype is *always* in another file, so the
supertype-aware counting 0009 added never runs on anything but nested classes.
That is why the fix looked green.

---

## 1. Cross-file: the translator panics

Two files, six lines.

```java
// Base.java
public class Base {
    public int f(int a)     { return a + 1; }
    public int f(String s)  { return s.length() + 100; }
}
// Child.java
public class Child extends Base {
    @Override public int f(int a) { return a + 1000; }
}
```

```
$ target/release/new-java2rust --context-dir /tmp/idaudit/xf2/src /tmp/idaudit/xf2/src/Child.java
thread 'main' panicked at src/signature.rs:5267:13:
signature solver must plan every lowered method: missing Child::f_string,
available for type: [f, f_int], nearby types: [Child]
```

`--context-dir-emitted` panics identically. `translate-batch` over the directory
takes the worker down:

```
thread '<unnamed>' panicked at src/signature.rs:5267:13:
signature solver must plan every lowered method: missing Child::f,
available for type: [f_int, f_string, g], nearby types: [Child]
thread 'main' panicked at src/lib.rs:906:41:
batch emission worker panicked: Any { .. }
```

The *same* Java program with `Base` and `Child` in one file translates cleanly to
`f_int` / `f_string`. The only variable is the file boundary.

### What the two disagreeing namers are

`available for type: [f, f_int]` is the interesting half. Three answers coexist
for one method set on one type:

* `assign_type_rust_names` on the primary program sees `Child` declaring one `f`
  → count 1 → **`f`**;
* the source-summary path (`resolve::assign_rust_names_to_context_program`,
  `src/lib.rs:253`/`:380`) resolves `Base` in its own program, where `f` has
  count 2 → **`f_int`** / **`f_string`**, and inherited-member materialisation
  copies those into `Child`;
* lowering then asks for a slot by Rust name and finds it absent.

The panic site is the reason a mismatch is fatal rather than merely wrong:

```rust
// src/signature.rs:5254
pub fn required_method_signature(&self, ty: &TypeDecl, method: &MethodDecl) -> &MethodSignaturePlan {
    let key = MethodKey { type_name: ty.name.clone(), rust_name: method_rust_name(method) };
    self.methods.get(&key)
        .or_else(|| self.unique_method_signature_for_simple_owner(&key))
        .unwrap_or_else(|| panic!("signature solver must plan every lowered method: missing {}::{}", ...))
}
```

`signature::MethodKey` is `{ type_name: String, rust_name: String }`
(`src/signature.rs:57`) — the plan store is indexed on the **emitted Rust name**,
which is exactly what 0009 says identity must not be. The assertion is correct
and load-bearing; it is catching a real disagreement. But it converts a naming
disagreement into a crash of the whole translation, which is why this is the
highest-severity item here: the FastQC-shaped input from 0009 §"Validated
instance" no longer mistranslates — it fails to translate at all.

---

## 2. `implements` is not counted: a `panic!` stub over a method that exists

```java
public class D {
    interface I {
        default int f(int a)    { return a + 1; }
        default int f(String s) { return s.length() + 100; }
    }
    static class Impl implements I {
        @Override public int f(int a) { return a + 1000; }
    }
    public static void main(String[] args) {
        I i = new Impl();
        System.out.println(i.f(1) + "," + i.f("ab"));
    }
}
```

`java D` → `1001,102`. Generated:

```rust
pub trait DI {
    fn f_int(&self, a: i32) -> i32 { ... }
    fn f_string(&self, s: &String) -> i32 { ... }
}

impl DImpl {
    pub fn f(&self, a: i32) -> i32 { return (a).wrapping_add(1000); }   // <- count 1, bare name
    pub fn f_string(&self, s: &String) -> i32 { ... }                   // <- inherited, present!
}

impl DI for DImpl {
    fn f_int(&self, a: i32) -> i32 { DImpl::f(self, a) }
    fn f_string(&self, s: &String) -> i32 {
        panic!("incompatible Java interface method implementation: D.Impl.f parameter types do not match interface");
    }
}
```

`DImpl::f_string` is right there, four lines above the `panic!` that replaces it.
The slot matcher looked for a member whose Rust name is `f_string`, found
`DImpl::f` (bare, because the count said 1) as the only candidate for *both*
slots, took it for `f_int`, and stubbed the other.

Patching only the unrelated `Box<dyn D.I>` parse error (0064 §7) and adding a
`fn main` shim:

```
$ ./D2.bin
thread 'main' panicked at D2.rs:1836:45:
incompatible Java interface method implementation: D.Impl.f parameter types do not match interface
```

Java: `1001,102`. Rust: aborts. Declaring **both** overrides in `Impl` makes the
same program emit `f_int`/`f_string` correctly and the stub disappears — which
isolates the cause to the count, not to the slot matcher.

This is the same `panic!` text 0009 §"Root cause" reported from FastQC and asked
to be replaced with a translator diagnostic. It is still a runtime `panic!`, and
it is still reachable — 0009's interface-candidate ordering fix (2026-08-03,
second pass) orders candidates by `JavaCallableSlotKey`, which cannot help when
the candidate set has one element.

---

## 3. Only the direct superclass is counted

```java
public class B {
    static class Base  { int f(int a) { return a + 1; } int f(String s) { return s.length() + 100; } }
    static class Mid extends Base { }
    static class Child extends Mid { @Override int f(int a) { return a + 1000; } }
    ...
}
```

`java B` → `1001,102`. Generated:

```rust
impl BBase  { pub fn f_int(&self, a: i32) -> i32 {...}  pub fn f_string(&self, s: &String) -> i32 {...} }
impl BMid   { pub fn f_int(&self, a: i32) -> i32 {...}  pub fn f_string(&self, s: &String) -> i32 {...} }
impl BChild { pub fn f(&self, a: i32) -> i32 {...}      pub fn f_string(&self, s: &String) -> i32 {...} }
```

`Mid` declares nothing, so `program_counts["Mid"]` is empty and `Child`'s count
for `f` is 1. `BChild::f` and `BBase::f_int` are the same Java method under two
Rust names — precisely the condition 0009's 2026-08-02 update was written to
remove, surviving one extra level of inheritance. It does not produce an error
here only because inheritance is materialised by copy-down, so `BChild` got its
own copy of every caller. It breaks as soon as dispatch is virtual, which is §2.

### The mirror case, for completeness

A subclass that *adds* an overload diverges the other way — `Base` declares one
`f` and keeps the bare name while `Child` declares two and gets `f_int`:

```rust
impl HBase  { pub fn f(&self, a: i32) -> i32 { ... }  pub fn g(&self) -> i32 { return self.f(5); } }
impl HChild { pub fn f_int(&self, a: i32) -> i32 { ... } pub fn f_string(...) {...}
              pub fn g(&self) -> i32 { return self.f_int(5); } }
```

Correct output here (`1001,102,1005` from both toolchains), again only because
`g` was copied down and re-lowered. The names still disagree. `counts` is
computed *downward* through `extends`, so a supertype can never learn that a
subtype overloaded its method — which is unfixable within the count scheme,
because the answer depends on the whole program.

---

## Root cause, stated once

The Rust name of a method is a function of `(java_name, param_type_spellings,
**how many methods in a set share java_name**)`. The third component is not a
property of the method; it is a property of a *set*, and every one of the three
defects above is a place where the set was computed differently. Java's own rule
has no such component: a method's identity is `(name, erased param types)`
regardless of what else exists.

Anchors:

| | |
|---|---|
| `src/resolve.rs:815` | `resolve_method` — the count-conditioned suffix |
| `src/resolve.rs:666` | `method_counts_including_supertypes` — one level, `extends` only |
| `src/resolve.rs:640` | `collect_program_method_counts` — per-`TypeDecl`, keyed on Java name |
| `src/resolve.rs:200` | `program_counts` seeded from `program.types` (primary unit only) |
| `src/resolve.rs:168` | `assign_rust_names_to_context_program` — a *second*, independently-scoped run |
| `src/signature.rs:57` | `MethodKey { type_name, rust_name }` — plan store indexed on the emitted name |
| `src/signature.rs:5254` | `required_method_signature` — where the disagreement becomes a crash |
| `src/java_signature.rs:13` | `JavaCallableSlotKey` — the key that would make the count unnecessary |

---

## Why nothing noticed

* **Every regression test for this is single-file.** `tests/differential/Overloads.java`,
  `tests/differential/InterfaceOverloads.java` and the unit tests 0009 lists all
  put the hierarchy in one compilation unit, which is the one arrangement where
  `method_counts_including_supertypes` has the data it needs. §1 needs exactly
  two files to reproduce and is not covered by anything.
* **Copy-down inheritance masks §3.** Because each subtype gets its own re-lowered
  copy of an inherited body, a divergent name is self-consistent within each
  `impl` block and produces no `rustc` error. It only surfaces through a trait,
  and only for a type that has one.
* **A translator panic does not look like a naming bug.** `signature solver must
  plan every lowered method` reads as a missing-plan bug in the signature solver.
  It is the solver correctly detecting that two namers disagreed.
* **The `panic!` in §2 is counted as emitted output.** It compiles, so no
  compile-error metric sees it; it only fails when that overload is called.
  0009 asked for this to become a translator diagnostic in its very first
  revision and it is still a runtime `panic!`, which means the one signal that
  would have surfaced §2 during a corpus run is suppressed by design.

---

## Direction

The count is the defect. It exists only to answer "does this name need
disambiguating", and that question is answerable exactly once, from the resolved
program, using the key that already exists.

1. **Delete the count.** After the override/inherit graph is resolved, build the
   set of `JavaCallableSlotKey`s reachable on each type *including all
   supertypes and interfaces, transitively, across compilation units*, and assign
   Rust names from that set in one pass. This is 0009's own fix direction §2
   (*"Assign Rust names as a late, total function of the signature set"*) — the
   part that was not done. `resolve_method` currently runs *during* resolution,
   before the graph exists, which is why it needs a heuristic count instead of
   the set.

2. **Make the name follow the slot, not the declaration.** A method's Rust name
   must be a property of the *slot* (`JavaCallableSlotKey` on the type that first
   declares it), so an override cannot be named differently from what it
   overrides and a subclass adding an overload cannot rename its parent's. Both
   §3 and its mirror follow.

3. **Key the signature plan on `JavaCallableKey`, not `MethodKey { rust_name }`.**
   That converts `src/signature.rs:5267` from a crash into an impossibility, and
   is the `usage::CallableKey` adoption 0009 lists as its standing item.

4. **Replace the interface-slot `panic!` with a translator diagnostic.** Third
   time of asking. A translator bug reported as a runtime condition in the
   generated program is invisible to every metric the project has.

### Regression cases worth pinning

```java
// (1) two files, must not panic, must dispatch to values 1001 / 102
// Base.java:  public class Base { public int f(int a){return a+1;} public int f(String s){return s.length()+100;} }
// Child.java: public class Child extends Base { @Override public int f(int a){return a+1000;} }

// (2) interface default overload, one overridden -> 1001,102, no panic! stub
interface I { default int f(int a){return a+1;} default int f(String s){return s.length()+100;} }
class Impl implements I { @Override public int f(int a){return a+1000;} }

// (3) grandparent overload -> Child's override and Base's method must share a Rust name
class Base { int f(int a){...} int f(String s){...} }  class Mid extends Base {}
class Child extends Mid { @Override int f(int a){...} }

// (4) subclass adds an overload -> Base's f must not keep the bare name
class Base2 { int f(int a){...} }
class Child2 extends Base2 { @Override int f(int a){...} int f(String s){...} }
```

Each asserts *values through a supertype-typed reference*, not spellings — 0009's
2026-08-03 update records two tests that asserted contradictory spellings of the
same construct and both passed for months.
