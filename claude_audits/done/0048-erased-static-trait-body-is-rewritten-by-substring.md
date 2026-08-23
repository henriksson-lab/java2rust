# 0048 — The erased-static trait body is produced by substring-rewriting emitted Rust

Status: **fixed** — the erased-static companion trait body rewrite is structural
and no longer substring-edits rendered Rust.
Severity: **high** — emits Rust that does not parse; the corruption is unbounded
(it can rewrite any identifier in the function) and there is no diagnostic
Live or latent: **live** — the feature runs in the VCF corpus
(`pub trait VCFHeaderLineErasedStatic` in
`.tmp/corpus-rerun-20260731-…/generated-crate/src/generated/00004_VCFHeaderLine.rs`);
it is correct there only because that owner's prefix happens to be long
Kind: decision (here: a *rewrite*) made by inspecting rendered Rust text

Codex follow-up 2026-08-23: fixed for erased-static trait bodies. The companion
trait emitter no longer renders a free helper and substring-rewrites the emitted
Rust. It now rewrites the cloned `RustFunction` body structurally, retargeting
only `RustExpr::Call` callees whose function is exactly one of the owner helper
functions (`owner_method`) to `Self::method`. Literals, method calls,
associated paths, raw text, and unrelated helper names are left untouched.
Regressions:
`cargo test --release lower::tests::erased_static_trait_helper_rewrite_only_touches_structured_helper_calls`
and
`cargo test --release --test idiom_gaps erased_source_static_trait_body_rewrites_only_helper_callees`.
This closes the concrete 0048 erased-static rewrite defect; `0007` remains open
for other rendered-string decisions.

Extends [0007](0007-rust-ir-is-mostly-opaque-strings.md). 0007 has been worked
hard on the *expression* side — `RustExprShape`, `actual: RustSlotShape`, and
~30 producer migrations. This is the same disease one level up, at
`RustItem::Raw`, where nothing has been done and where the failure mode is worse
than a wrong idempotence answer: the program **edits its own output text**.

## Symptom

A source class that lowers as an erased-object alias gets its static methods
emitted twice: as free functions `<owner_snake>_<name>(…)`, and as a companion
trait `…ErasedStatic` whose bodies call the *same* helpers through `Self::`.

The trait bodies are produced by rendering the function to a `String` and then
running a **prefix substring search-and-replace** over that string
(`src/lower.rs:rewrite_erased_source_static_helper_calls_for_trait`, line 1309).
The prefix is `to_snake_case(owner) + "_"`. The search
(`src/lower.rs:find_raw_pattern_outside_literals`, line 3706) skips string, char
and raw-string literals — and nothing else. It is **not anchored to an
identifier boundary**, so the prefix matches in the middle of an identifier,
immediately after `.`, and immediately after `::`.

## Verified evidence

All runs are `./target/release/new-java2rust --facts facts.txt <file>.java` on
the current `master` build (`target/release/new-java2rust`, 2026-08-21).

### 1. Instance method call and qualified static path — both corrupted

`facts.txt`:

```
erased-object Cfg
```

`Cfg.java`:

```java
class Item {
    int cfgVersion() { return 3; }
    static int cfgStaticVersion() { return 4; }
}

class Cfg {
    static int a(Item it) { return it.cfgVersion(); }
    static int b() { return Item.cfgStaticVersion(); }
    static int c() { int cfg_local = 5; return cfg_local; }
    static String d() { return "cfg_x("; }
}
```

Emitted:

```rust
pub trait CfgErasedStatic {
    fn a(it: &Item) -> i32 {
        return it.Self::version();          // <-- was it.cfg_version()
    }

    fn b() -> i32 {
        return Item::Self::static_version(); // <-- was Item::cfg_static_version()
    }

    fn c() -> i32 {
        let cfg_local: i32 = 5;
        return cfg_local;                    // untouched (no `(` follows)
    }

    fn d() -> String {
        return "cfg_x(".to_string();         // untouched (literal skipping works)
    }
}
```

The free-function copies of the same bodies are correct
(`it.cfg_version()`, `Item::cfg_static_version()`), which is what makes this a
pure rewrite defect rather than a lowering defect.

`rustc` on the two corrupted forms:

```
error: expected one of `(`, `.`, `;`, `?`, `}`, or an operator, found `::`
 --> snippet.rs:6:22
  |
6 |         return h.Self::id();
  |                      ^^ expected one of `(`, `.`, `;`, `?`, `}`, or an operator
```

```
error[E0223]: ambiguous associated type
 --> snip2.rs:4:28
  |
4 |     fn b() -> i32 { return Item::Self::static_version(); }
  |                            ^^^^^^^^^^
```

The first is a **parse** error, so the whole generated module is lost, not one
function.

### 2. A short owner name corrupts support-helper calls

`facts.txt`: `erased-object A`. `A.java`:

```java
class A {
    static String describe(Object o) { return String.valueOf(o); }
}
```

Emitted:

```rust
pub trait AErasedStatic {
    fn describe(o: Box<dyn std::any::Any>) -> String {
        return javSelf::erased_object_to_string(&o).to_string();
    }
}

pub fn a_describe(o: Box<dyn std::any::Any>) -> String {
    return java_erased_object_to_string(&o).to_string();   // the correct text
}
```

The prefix `a_` matched inside `java_erased_object_to_string`, and the rewrite
spliced `Self::` into the middle of the support helper's name. Any owner whose
snake prefix is a substring of a generated identifier is exposed; `java_…`,
`__java_self`, and every `java_string_*` / `java_math_*` helper are candidates.

### 3. It is only correct in the corpus by accident

The one live instance in `.tmp` is `VCFHeaderLine`, whose prefix is long and
distinctive, so its two rewrites (`Self::escape_quotes(…)`) are right. Nothing
in the mechanism made that happen — the same file also renders
`java_string_substring(…)` and `java_erased_object_to_string(…)`, which survive
only because `vcfheader_line_` does not occur inside them.

Java naming makes case 1 the common one, not the exotic one: a class `Node`
calling `x.nodeId()`, `Cfg` calling `x.cfgVersion()`, `File` calling
`x.fileName()` are ordinary Java.

## Root cause

`src/lower.rs:erased_source_static_trait_item` (line 1283) builds the whole
trait as a `String`:

```rust
let mut rendered = crate::emit::emit_file(&RustFile {
    items: vec![RustItem::Function(trait_function)],
});
rendered = rewrite_erased_source_static_helper_calls_for_trait(owner_name.as_str(), rendered);
```

and `rewrite_erased_source_static_helper_calls_for_trait`:

```rust
let prefix = format!("{}_", crate::parser::to_snake_case(owner));
while let Some(start) = find_raw_pattern_outside_literals(&rendered, &prefix, search_start) {
    let name_start = start + prefix.len();
    let Some(paren_offset) = rendered[name_start..].find('(') else { break };
    let rust_name = &rendered[name_start..name_start + paren_offset];
    if rust_name.chars().all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
        rendered.replace_range(start..name_start, "Self::");
        …
    }
}
```

Three things are wrong and each is a direct consequence of the input being text:

1. **No left boundary.** `find_raw_pattern_outside_literals` returns any
   occurrence. A match preceded by `[A-Za-z0-9_]`, `.` or `:` is not a call to a
   helper of this owner, but is accepted.
2. **The callee is guessed from the next `(`.** `rendered[name_start..].find('(')`
   scans forward across arbitrary text; whether the characters in between happen
   to be identifier-shaped is the only test. `it.cfg_version()` passes it.
3. **`break` on a missing `(`.** If the prefix appears once with no following
   `(` anywhere, the loop abandons the rest of the function, silently leaving
   later helper calls unrewritten. (Benign today — the un-rewritten form is the
   valid free-function call — but it means the pass is not even reliably
   applied.)

### The typed fact, and whether it is reachable

**It is reachable, and it is already in the same function's arguments.**

`erased_source_static_trait_item(ty: &TypeDecl, functions: &[RustFunction])`
holds:

- the exact set of names it wants to replace — `functions.iter().map(|f| &f.name)`,
  which were minted a few lines earlier by
  `erased_source_static_free_function_name` (line 1275) from `(owner, rust_name)`;
- the **structured body** of each function, `RustFunction.body: Vec<RustStmt>`,
  before it is rendered.

So the question "is this callee one of my erased-static helpers?" is
`RustExpr::Call { function, .. }` where `function` is a `Var`/`Path` whose name
is *equal* (not prefix-matched) to a member of that known set. An IR walk
replacing the callee node with `RustExpr::path(["Self", original_name])` answers
it exactly, cannot touch a receiver, cannot touch a literal, and cannot touch an
unrelated identifier.

Better still, the identity is known even earlier:
`src/lower.rs:retarget_erased_source_static_call_to_free_function` (line 13088)
is the site that *created* the free-function callee, and it has `(owner_key,
rust_name)` in hand. A dedicated plan variant (e.g.
`CallTargetPlan::ErasedStaticHelper { owner, rust_name }`) would let both the
free-function rendering and the trait rendering pick their spelling from the same
typed fact, with no rewrite pass at all.

### Why the rewrite exists at all

Because there is no `RustItem::Trait`. `RustItem` is
`Struct | Impl | Function | Comment | Raw` (`src/rust_ir.rs:7`), so every trait,
type alias, `impl From`, and hierarchy carrier this program emits is a
`format!`-built `String` wrapped in `RustItem::Raw` (`item_assembly.rs:raw_item`,
11 call sites in `lower.rs` alone). Once the trait is a string, the only way to
adjust anything inside it is to edit the string. This is 0007's thesis exactly,
at item granularity: the escape hatch does not just lose information, it forces
the *next* transformation to be textual too.

## Why it went unnoticed

- The feature's only regression test
  (`tests/idiom_gaps.rs:erased_source_static_methods_lower_as_free_helpers`)
  uses a class named `ErasedStaticHelper`. Its prefix is
  `erased_static_helper_` — 21 characters, and the fixture body calls exactly
  one helper and nothing else. The test asserts on the *free functions* and on
  `!contains("ErasedStaticHelper::encode")`; it never asserts anything about the
  trait body it just generated.
- The corrupting inputs are all "the owner name also appears in some other
  identifier", which no fixture in the suite has.
- The trait is emitted as `RustItem::Raw`, so nothing structural (no `match` on
  `RustExpr`, no shape check, no emitter precedence logic) ever looks at it. The
  only thing that would catch it is compiling the output — and the trait is not
  covered by a `cargo_check_generated_modules` case (cf.
  [0041](done/0041-slot-shape-design-had-no-generated-crate-checks.md)).

## Proposed direction

1. **Replace the rewrite with an IR walk over `RustFunction.body`, keyed on
   whole-name equality** against `functions.iter().map(|f| &f.name)`. This is a
   small, self-contained change and removes the entire class of corruption; it
   needs no new IR.
2. **Then remove the reason for the walk**: give the erased-static call target a
   plan identity at `retarget_erased_source_static_call_to_free_function`, so the
   trait renderer selects `Self::name` and the free renderer selects
   `owner_snake_name` from the same fact.
3. **Add `RustItem::Trait`.** The erased-static trait, the interface trait
   (`raw_interface_trait_item_with_default_methods`), and
   `interface_default_helper_signature` — which does
   `signature.replacen("fn {rust_name}", …, 1)` on a rendered signature — are all
   the same shape. A trait item node retires all three string surgeries.
4. **Guard the boundary in a test**, in the shape 0007's step 5 proposes: a
   fixture whose erased owner's prefix occurs inside an unrelated identifier
   (`class A` + any `java_*` support call is enough), asserted with
   `cargo_check_generated_modules`. That single fixture fails today.
5. While the rewrite survives, the minimum hardening is: require the match to be
   at an identifier boundary, require the `(` to immediately follow the name, and
   `continue` rather than `break`. This is strictly a stopgap — it makes the
   substring search *less* wrong without making it right.

## Repro files

Kept out of the tree; each is four lines of Java plus a one-line facts file:

```
facts.txt:  erased-object Cfg
Cfg.java:   (above)
facts3.txt: erased-object A
A.java:     class A { static String describe(Object o) { return String.valueOf(o); } }
```
