# In-place idiomatic IO translation — prototype findings (2026-06-21)

Goal (user preference, memory `in-place-translation-preference`): translate Java IO
**in place** to idiomatic `std::io`/`std::fs`, instead of routing to the
runtime-carrier structs (`JavaReader`/`JavaInputStream`, `src/runtime/io_read.rs`)
that the dumper currently emits.

## What the carrier emits today (jaligner `SequenceParser.parse`)

```rust
let mut reader: Option<crate::java_runtime::JavaReader> = None;
reader = Some(java_buffered_reader(java_input_stream_reader(java_file_input_stream(&file))));
let line = reader.clone().unwrap().read_line();              // Option<String>
while let Some(line) = reader.clone().unwrap().read_line() { ... }
reader.clone().unwrap().close();
```

## The idiomatic target — PROVEN to compile (`/tmp/io_spike.rs`)

```rust
use std::io::BufRead;
fn read_line(r: &mut dyn BufRead) -> Option<String> {     // one unavoidable helper
    let mut s = String::new();
    if r.read_line(&mut s).unwrap_or(0) == 0 { return None; }
    while s.ends_with('\n') || s.ends_with('\r') { s.pop(); }
    Some(s)
}
let mut reader: Option<Box<dyn BufRead>> =
    Some(Box::new(std::io::BufReader::new(std::fs::File::open(file).unwrap())));
let first = read_line(reader.as_mut().unwrap());          // &mut, NOT .clone()
while let Some(line) = read_line(reader.as_mut().unwrap()) { buffer.push_str(&line); }
drop(reader.take());                                       // close() -> drop
```

This is genuinely more Rust: std traits/types, no bespoke carrier struct.

## Why it isn't a localized translator change — two walls

1. **Type erasure → `Box<dyn std::io::BufRead>`.** A Java `Reader`-typed var takes
   any reader subtype and is reassigned; Rust has no subtyping, so the var needs one
   type. `Box<dyn BufRead>` is the std analogue of the `JavaReader` carrier. (Tractable:
   `map_type_name` Reader-family → `Box<dyn BufRead>`; ctors → idiomatic std builders.)

2. **Access must be `&mut`-borrow, not `.clone()` — the blocker.** `JavaReader` is
   `Rc<RefCell<Box<dyn BufRead>>>`: Clone + shared-mutable. That is *precisely why* the
   dumper can emit `reader.clone().unwrap().read_line()` under its value/clone access
   model. A `Box<dyn BufRead>` is **not Clone**, and `read_line` mutates the cursor, so
   every read site must become `reader.as_mut().unwrap()`. Switching IO locals from
   clone-access to borrow-access is the use-site-borrow frontier (SEMANTICS §6 /
   clone-reduction audit) — not a per-ctor edit. A partial change (type+ctors only,
   leaving clone-access) regresses hard: `Box<dyn BufRead>` won't `.clone()`.

   Note: the carrier `JavaReader` ≈ inline `Rc<RefCell<Box<dyn BufRead>>>` + a `read_line`
   helper. Under the current clone model it is already close to minimal; the *only*
   un-idiomatic part is that it's a named struct. Fully idiomatic IO (`?`-propagation,
   `&mut` access, concrete `BufReader<File>`) additionally needs **Result-propagation**
   for the `throws IOException` methods.

## Verdict

In-place idiomatic IO is **feasible and the output compiles**, but it is gated on
two frontier capabilities, not a stdlib-table edit:
- IO locals accessed by `&mut` borrow (use-site-borrow analysis), and
- (for the cleanest form) `Result`/`?` propagation for `throws` methods.

Recommended sequencing: land **borrow-mode access for a marked set of local types**
first (reusable beyond IO), then flip the Reader family `map_type_name` →
`Box<dyn BufRead>` + idiomatic ctors + the `read_line` helper, and measure. Until the
borrow frontier exists, the carrier is the pragmatic emission. The spike
(`/tmp/io_spike.rs`) is the compile-verified target to translate toward.

## Alternative direction — stubs + a separate inlining pass (proposed, TO DISCUSS)

User's proposal (2026-06-21), not yet decided: rather than translate IO **directly**
to idiomatic `std::io` (the carrier or the in-place flip above), **separate the two
concerns**:

1. **Stubs first.** Let the IO types fall back to the existing opaque-stub mechanism
   (`stub_<pkg>.rs`: `pub struct X{}` + `unimplemented!()` methods). They COMPILE
   immediately with no carrier machinery — "make it type-check" is decoupled from
   "make it do real work."
2. **A separate stub-inlining mechanism.** A distinct pass that replaces a stub
   type/method with an idiomatic inline Rust implementation — general, NOT IO-specific,
   reusable across the whole stdlib surface (ties into memory `stdlib-stub-implementation`
   and the [[in-place-translation-preference]]).

**Why this may be better:** the carrier approach couples *what type* with *how it's
implemented*; stubs + inlining decouples them. The translation core only ever emits
stubs (uniform, simple); idiomatic-ness becomes an orthogonal, incrementally-applied
substitution pass. It also sidesteps the borrow-frontier blocker for *getting it to
compile* (stubs need no `&mut`/`?` discipline) and lets the idiomatic work proceed
independently. **Open questions for the discussion:** how the inlining pass binds a
stub call to its idiomatic impl (name/signature mapping); whether inlined IO still
needs the borrow-mode access + `Result` propagation (likely yes, but now isolated to
the inlining pass, not the core); and how this relates to the existing runtime-carrier
types already shipped (`io_read.rs`/`io_write.rs` — keep, or migrate to inlined stubs?).
