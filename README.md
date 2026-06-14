# java2rust

This tool aims to translate Java code to Rust. The output code needs further work
to become idiomatic Rust, and due to the stricting ownership rules of Rust, might
not compile at all after a first pass. 

The tool is derived from [cguz/java-to-rust](https://github.com/cguz/java-to-rust), which was used
for inspiration. Due to differences in parsing, this code was not translated
but the semantic mapping decisions where kept. Further rules have been introduced
by testing the tool on large codebases relevant for bioinformatics.

**Note: Just because the output compiles does not mean that the translation is correct!
Be sure that you are aware of the pitfalls of translation before you commit to it, and
unless you know it yourself, ensure you have access to someone with good amount of Rust
programming experience.**

Read more about our work on Rust translation here: [henriksson-lab/rustification](https://github.com/henriksson-lab/rustification)

## License

This crate, Java2rust, is made using LLM. Be careful with reusing code as we cannot guarantee that code has not been copied from somewhere.

The original code license is hard to understand:
* The code is inspired by [cguz/java-to-rust](https://github.com/cguz/java-to-rust), which is stated to be GPL3. 
* But cguz/java-to-rust is in turn derived from [aschoerk/converter-page](https://github.com/aschoerk/converter-page), which is under the Apache License v2
So the legal state of this code is uncertain

Thus, consider this crate to be public domain by default, but possibly tained with GPL3 and Apache license v2


## Using this crate (guide for LLMs)

This section is written for an LLM agent driving or post-editing this tool.

### What it is / is not
- It does a **mechanical** Java→Rust translation, file by file. Output is
  syntactically valid Rust but **frequently will not compile** as-is: external
  deps, cross-file references, and Rust's ownership rules need follow-up edits.
- **Compiling ≠ correct.** Treat the output as a starting point to be reviewed,
  not a finished port.

### Build & binaries
```
cargo build --release
```
Produces three binaries:
- `java2rust-rs` — the translator (Java → Rust).
- `gen-symbols` — extracts a Java→Rust **symbol map** from a translated crate.
- `jar-to-symbols` — extracts the same symbol map from a dependency **`.jar`**
  (ground-truth signatures + annotation nullability); warns about uncovered types.

### Translate
```
java2rust-rs -d <file.java | dir> -o <outdir> [-i] [-v <n>] [-cp] [-l map.json]...
```
- `-d` input file or directory (directories recurse; the last `-d` wins).
- `-o` output directory (default `output`). Filenames are snake_cased; the input
  directory tree (package layout) is mirrored under `-o`.
- `-i` skip files already present; `-v` verbosity; `-cp` copy non-`.java` files.
- `-l/--link <map.json>` link against a dependency's symbol map (repeatable) —
  see **Linking** below.

### Output conventions you must preserve
Every emitted type, method, constructor, and struct field carries a **provenance
marker** doc comment recording its Java origin:
```rust
/// @java org.example.Point          // on the struct/enum/trait
/// @java org.example.Point#x        // on a field
/// @java org.example.Point#<init>   // on a constructor (-> fn new)
/// @java org.example.Point#getX     // on a method
```
**Do not delete these markers.** `gen-symbols` reads them to re-derive the symbol
map from the *current* (possibly hand/LLM-edited) source. You may freely change
the Rust around them — rename, add `Option`, switch to `&mut self`, etc.

### Nullability → `Option`
A dedicated pass marks only declarations that can actually hold `null` as
`Option<T>` (seeded from `= null` / `return null` / `x != null` / `null` passed
as an argument, then a cross-method fixpoint). For those: `null`→`None`, values
into the slot →`Some(v)`, reads →`.unwrap()`, `x != null`→`x.is_some()`.
Everything else stays a plain `T`. So an `Option` in the output is a real
nullability signal, not noise.

### Linking a translation to an already-translated dependency
Goal: when translating crate B that depends on crate A (already translated to
Rust), make B reference A's **real Rust paths** instead of bare Java names.

Workflow:
```
# 1. translate the dependency
java2rust-rs -d A/src -o A_rust
# 2. extract its symbol map (run this AFTER any LLM edits to A_rust, so the map
#    reflects the current code — Option, &mut, renames included)
gen-symbols A_rust -o A_map.json
# 3. translate the dependent, linking against the map
java2rust-rs -d B/src -o B_rust --link A_map.json
```
With `--link`, two things happen:

1. **Type references** — a referenced type `Point` (resolved via the file's
   `import`s + package to the FQN `org.example.Point`) is emitted as its mapped
   `rust_path` (e.g. `point::Point`) in field types, return types, locals, and
   `new` calls. Resolution order: explicit import → same package → wildcard
   import → bare FQN → unique simple-name match. Unknown types fall back to the
   built-in stdlib mapping.

2. **Call sites** — when a method call's receiver resolves to a linked type
   (a typed local/param/field, a `new X()`, or a static `Type.m()` reference),
   the call is shaped to the callee's *recorded* signature:
   - the **exact current Rust method name** is used, so a dependency method the
     LLM renamed (`lookup` → `find`) is called correctly;
   - **argument borrowing** matches the params — `&` / `&mut` for by-reference
     params, `Some(..)` for nullable-by-value params, a `.clone()` for non-Copy
     by-value names;
   - a **nullable (`Option`) return** read as a plain value gets `.unwrap()`.

   Example: `s.lookup(k)` against a dep whose `lookup` was edited to
   `fn find(&self, key: &String) -> Option<String>` becomes `s.find(&k).unwrap()`.

3. **Caller `&mut` upgrade** — a value used as the receiver of a linked
   `&mut self` call is made mutable so the call type-checks: a *parameter*
   becomes `&mut T` (e.g. `s: &mut store::Store`), and a *local* gets `let mut`.

   Caveat (left for a later LLM pass): nullability is not propagated back into a
   caller's own *parameter types* (a caller param passed only into nullable dep
   slots stays `T`, not `Option<T>`). In practice the eager `.unwrap()` on
   nullable returns and `Some(..)`-wrapping into nullable params keep the output
   compiling; deeper cross-method signature inference is left to a follow-up.

Key property: the map is generated **from** the translated crate, not frozen at
translation time. Edit the Rust to make it idiomatic/performant, re-run
`gen-symbols`, and the map tracks your edits. The linker links to "whatever the
code looks like now"; remaining mismatches are left for a later LLM pass.

### Linking a dependency you do *not* translate — from its JAR
For a third-party dependency you won't translate (or haven't yet), generate the
same `--link` map directly from its compiled `.jar` with `jar-to-symbols`:
```
jar-to-symbols dep1.jar dep2.jar ... -o deps_map.json
java2rust-rs -d src -o out --link deps_map.json [--stubs]
```
`.class` bytecode carries **ground-truth signatures** (exact parameter/return
types, static-ness → `receiver`, `throws`), **generics** (read from the
`Signature` attribute, not the erased descriptor — `List<String>` → `Vec<String>`,
`Map<String,Integer>` → `HashMap<String, i32>`, type variables and `? extends`
bounds preserved), and — when the library ships nullability annotations
(`@Nullable`/`@CheckForNull`, which have CLASS retention and survive in the
bytecode) — **real nullability**, so a `@Nullable` return becomes an `Option` and
yields `.unwrap()` at the call site. This is strictly more precise than `--stubs`'
call-site guessing; prefer a JAR when you have one.

**Provide every JAR.** `jar-to-symbols` **warns** about each type referenced in a
signature that none of the supplied JARs define (JDK types excepted), grouped by
package — add those JARs so the map (and the translation) is precise. Pass all of
a project's dependency JARs in one invocation.

Limits: bytecode has no Rust ownership info, so `&`/`&mut` stay heuristic (`&` for
non-primitive params, no `mut`); unbounded wildcards (`?`) render as `_`; and
overloads (same name, different params) collapse to one entry (the richer
signature wins), since the map keys methods by bare Java name. An LLM resolves
the rest.

The three map sources are interchangeable (same JSON, same `--link`):
`gen-symbols` (from translated Rust, tracks LLM edits) · `jar-to-symbols` (from a
dependency JAR, ground truth) · `--stubs` (last-resort guess for whatever neither
covers).

### Symbol map schema (`A_map.json`)
JSON keyed by Java FQN (`serde`-serialized; see `src/symbol_map.rs`):
```jsonc
{
  "types": {
    "org.example.Point": {
      "rust_path": "point::Point",          // how to name this type in Rust
      "kind": "struct",                      // struct | enum | trait
      "fields": {
        "x": { "rust": "x", "type": "i32", "nullable": false }
      },
      "methods": {
        "getX": {
          "rust": "get_x", "rust_path": "point::Point::get_x",
          "receiver": "ref",                 // none | value | ref | refmut
          "ret": "i32", "ret_nullable": false,
          "params": [ { "type": "i32", "by_ref": false,
                        "mutable": false, "nullable": false } ]
        }
      }
    }
  }
}
```

### Gotchas
- Files are translated in **isolation**: references to other project classes or
  external libraries won't resolve unless you `--link` a map that covers them, or
  generate stubs for them (`--stubs`, below).
- Swing/AWT and other external libraries are out of scope (see below).
- `rust_path`s come from the dependency crate's module layout at `gen-symbols`
  time; if you restructure that crate's modules, regenerate the map.

### Generating stubs for unresolved symbols (`--stubs`)
The inverse of, and fallback for, linking: for every symbol the translation
*can't* resolve (not a primitive, not stdlib-mapped, not `--link`ed, not defined
elsewhere in the same tree), record a best-effort signature — opaque structs +
`impl` blocks (methods, constructors `-> Self`, statics) + free functions, each
with `/// @java` provenance and an `unimplemented!()` body.

Output is split **one file per originating package** (a proxy for the dependency
JAR), so each dependency can be filled in independently:
`<output>/stub_<package>.rs` (e.g. `stub_org_json.rs`,
`stub_org_apache_commons_jexl2.rs`), with free functions and package-less types
in `<output>/stubs.rs`. Each file is self-contained.

```
java2rust-rs -d <src> -o <out> --stubs            # stubs for everything missing
java2rust-rs -d <src> -o <out> --link dep.json --stubs   # stubs for what dep.json doesn't cover
```

Signatures are inferred from call sites: parameter types from argument
expressions (literals + typed locals/params), return types from the call's usage
context (assigned into a typed local, or returned). Where a type can't be
inferred it is the placeholder `Unknown` (`= ()`); the untranslatable
`java.lang.Object` is also mapped to `Unknown`. An external type referenced from
several packages without an import (e.g. `Map.Entry`) yields a single struct
keyed by its Rust name, carrying every `@java` guess as provenance.

This is a **scaffold for an LLM/human to fill in**, not a finished artifact: the
stub *signatures* are the value (they record how each symbol is used). `stubs.rs`
compiles standalone only when its signatures reference solely primitives/`String`
/other stubs; in general it is meant to be dropped into the translated crate,
where project and JDK types are in scope. Once you translate the real dependency
and `gen-symbols` it, `--link` that map and the stub disappears.

Example (`org.json:json` is an unmapped dependency):
```rust
/// @java org.json.JSONObject
#[derive(Clone, Default)]
pub struct JSONObject {}
impl JSONObject {
    pub fn new() -> JSONObject { unimplemented!() }
    pub fn get_string(&self, a0: String) -> String { unimplemented!() }
    pub fn put(&self, a0: String, a1: Unknown) { unimplemented!() }
}
```

On real corpora the stub set is exactly the external libraries: e.g. htsjdk's
`src/main` stubs resolve to `org.json`, `org.apache.commons.{jexl2,compress}`,
`org.xerial.snappy`, `com.fulcrumgenomics.jlibdeflate`, plus unmapped JDK
corners (`java.util.concurrent`, `javax.script`, `java.awt`, `Map.Entry`, …).

## Goal (below is for LLM)

The original tool is now treated as **inspiration, not a spec**: we use its
Java→Rust mapping approach but aim to emit **Rust that actually compiles**, fixing
the original's quirks (it deliberately produces "unrusty", non-compiling output).

`tools/compilecheck.sh` runs a corpus of self-contained Java snippets through the
converter and checks each with `rustc --crate-type lib`. Fixes landed so far:
struct fields emit as `name: Type,` (not `let …;`), static fields → associated
`const`, methods that mutate fields take `&mut self`, principled `&`-borrows
(AST-derived, not reflection), `do`/`while` lowering, and dropped the spurious
`&` on method-call arguments. Java-isms are lowered: `println!`, `panic!` for
`throw`, bare blocks for `synchronized`/`try`, `assert!`, proper `++`/`--`.

**Stdlib mapping (partial):** collection/boxed types (`List`/`ArrayList`→`Vec`,
`Map`→`std::collections::HashMap`, `Set`→`HashSet`, `Integer`→`i32`, …), their
constructors (`new ArrayList<>()`→`Vec::new()`), `Math.*`→receiver methods
(`Math.max(a,b)`→`(a).max(b)`, `Math.sqrt(x)`→`(x).sqrt()`), and Rust-keyword
identifiers escaped as raw identifiers (`box`→`r#box`). Common collection/String
methods: `.size()`/`.length()`→`(x.len() as i32)`, `.isEmpty()`→`.is_empty()`,
`.equals(y)`→`(x == y)`, `.add(z)`→`.push(z)` (or `.insert` for a `Set`),
`.get(i)`→`x[(i) as usize].clone()` for a list / `x.get(&(k)).cloned().unwrap()`
for a map, `.put(k,v)`→`.insert(k,v)`, `.contains(x)`→`.contains(&(x))`,
`.containsKey`→`.contains_key`, and `String` ops (`toLowerCase`/`toUpperCase`/
`trim`/`charAt`/`substring`), and `String.format("%d…%s", …)`→`format!("{}…{}", …)`
(`%`-specifiers converted). Enhanced-for `for (T x : coll)`→`for x in coll.clone()`
(by-value, matching Java). **Streams + lambdas:** Java lambdas → Rust closures
(`x -> e` → `|x| e`), `.stream()`→`.iter().cloned()`, `.collect(...)`→
`.collect::<Vec<_>>()`, `.count()`→`(… as i32)`, `.forEach`→`.for_each`,
`.toArray`→`collect`, `.mapToInt`/etc→`.map`. Borrowing predicate combinators
(`.filter`/`.anyMatch`→`.any`/`.allMatch`→`.all`) clone-shadow the item so the
lambda body sees `T` not `&T`: `.filter(|x| { let x = x.clone(); x > 0 })`. So
`xs.stream().filter(x->x>0).map(x->x*2).collect(...)` →
`xs.iter().cloned().filter(|x| { let x = x.clone(); x > 0 }).map(|x| x * 2).collect::<Vec<_>>()`.
Also `.findFirst`/`.findAny`→`.next()`, `.limit`/`.skip`→`.take`/`.skip(… as usize)`,
`.sum`→`.sum::<i32>()`, `IntStream.range(a,b)`→`((a)..(b))`. More `String` ops:
`.split`→`split(...).map(to_string).collect`, `.replace`, `.indexOf`→`.find(...).map().unwrap_or(-1)`,
`.startsWith`/`.endsWith`, receiver-aware `.contains`. Static calls on a class
(`Collections.x`) use `::`; chained/instance calls use `.`.
A variable gets `let mut` when a mutating method (`add`/`put`/`remove`/…) is
called on it.

**Ownership (partial):** structs `#[derive(Clone)]`; non-Copy values read in a
move position (return / assignment RHS / var init) get `.clone()`; array indices
are cast `as usize`; `char c = 65` → `65 as u8 as char`. Self-contained compile
corpus: **39/39** (`tools/compilecheck.sh`).

**Nullability** is inferred by a dedicated pass (`nullability.rs`, run after
`IdTracker`): it finds which declarations can hold `null` (seeded from
`= null` / `return null` / `x != null` / passing `null` as an argument, then a
cross-method fixpoint), and only those become `Option<T>` — `null`→`None`,
values into a nullable slot →`Some(v)`, reads →`.unwrap()`, `x != null`
→`x.is_some()`. Everything else stays a plain `T`.

The golden fixtures are re-baselined to this tool's own output (regression lock),
not the jar's. Earlier history (below) targeted byte-parity with the jar.

## Out of scope

**GUI frameworks — Swing/AWT (`javax.swing.*`, `java.awt.*`) — are permanently
out of scope.** They have no Rust equivalent, so code using them will not
translate to anything that resolves or compiles, and we do not attempt to map
them. More broadly, any **external dependency** (other libraries, and a Java
project's own other classes when files are converted in isolation) is out of
scope for the per-file `rustc` check: such files emit syntactically valid Rust
but fail to compile with `cannot find type/value` until those types exist.

Real bioinformatics codebases are used as compile tests (none is expected to
fully compile — all have heavy external dependencies). They are cloned under
`testdata/`. For each, the converter emits syntactically valid Rust for **every**
file (0 panics, 0 syntax errors); the files that don't compile fail only on
external/cross-file references (`cannot find type/value`) — the out-of-scope
boundary, not translation bugs. Convert a tree with
`cargo run --release --example batch -- <java-src-dir> <out-dir>`.

- **FastQC** (`s-andrews/FastQC`, 156 files) — 0 syntax errors; 15 compile
  standalone, the rest fail only on external refs (much of it Swing).
- **GATK** (`broadinstitute/gatk`, 1595 files) — 0 syntax errors; ~93 compile
  standalone, the rest on cross-file/external refs.
- **Picard** (`broadinstitute/picard`) — additional corpus.
- **htsjdk** — byte-parity reference for the original mapping (historical).

`tools/compilecheck.sh` runs the small self-contained snippet corpus through
`rustc` (75/75 compiling).

---

Earlier goal was **behavioral equivalence** with `java2rust.jar` (same output for
same input). Internals are idiomatic Rust and use tree-sitter for parsing.

## Architecture

The original Java tool runs a four-stage pipeline (see `JavaConverter.convert`):

1. **Parse** Java → AST (originally JavaParser 2.5.1).
2. **IdTrackerVisitor → IdTracker** — scope / identifier tracking.
3. **TypeTrackerVisitor** — type inference over tracked identifiers.
4. **RustDumpVisitor** — the code generator (81 visit methods, ~2200 LOC) + a
   `SourcePrinter`. Produces the Rust string.

We reproduce the same pipeline. The key decision is the **parse layer**:

> **tree-sitter parses → an adapter builds a JavaParser-shaped typed AST → the
> three passes are ported ~1:1 against that typed AST → the dumper emits a string.**

tree-sitter does the robust parsing; an adapter reconstructs a typed AST that
mirrors the JavaParser node types the original code is written against. This
isolates every tree-sitter quirk in one adapter layer and lets the intricate
codegen port mechanically — the lowest-risk route to matching output.


## Usage (target)

```
java2rust-rs -d <path_file.java | path_directory> [-o output] [-i] [-v 2] [-cp] [-l map.json]...
```

See **Using this crate (guide for LLMs)** above for the `-l/--link` linking
workflow and the `gen-symbols` map extractor.

