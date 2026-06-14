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
java2rust-rs -d <path_file.java | path_directory> [-o output] [-i] [-v 2] [-cp]
```

