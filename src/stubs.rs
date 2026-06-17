//! Stub generation for unresolved external symbols.
//!
//! When the translator references a type/method/constructor/function it cannot
//! resolve (not a primitive, not stdlib-mapped, not a linked dependency, not a
//! type defined elsewhere in the same translated tree), it records a best-effort
//! signature here. `main` aggregates these across the whole tree and writes a
//! single `stubs.rs` — opaque structs + `impl` blocks + free functions with
//! inferred signatures and `unimplemented!()` bodies — so an LLM (or human) can
//! fill in the bodies and wire the module up.
//!
//! Stubs carry `/// @java <fqn>` provenance markers, so once the real
//! dependency is translated and `gen-symbols`'d, you can `--link` it and drop
//! the stub. Stub generation is the inverse of, and fallback for, linking.

use std::collections::{BTreeMap, BTreeSet};

use crate::symbol_map::{MethodSym, SymbolMap, TypeSym};

/// Placeholder type emitted where a Rust type could not be inferred.
pub const UNKNOWN: &str = "Unknown";

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Receiver {
    #[default]
    None,
    Ref,
    RefMut,
}

#[derive(Default, Clone)]
pub struct StubSig {
    pub receiver: Receiver,
    /// Rust type per parameter, best-effort (`Unknown` when not inferable).
    pub params: Vec<String>,
    /// Rust return type; `None` => unit / not inferred.
    pub ret: Option<String>,
}

impl StubSig {
    /// How much concrete (non-`Unknown`) type info this signature carries — used
    /// to keep the most informative variant when the same symbol is seen at
    /// several call sites.
    fn info(&self) -> usize {
        let p = self.params.iter().filter(|t| *t != UNKNOWN).count();
        let r = self.ret.as_deref().map(|t| t != UNKNOWN).unwrap_or(false) as usize;
        p + r
    }
}

#[derive(Default, Clone)]
pub struct StubType {
    pub rust_name: String,
    /// All Java FQN guesses seen for this struct (recorded as provenance). More
    /// than one when the same simple name was referenced from several packages
    /// without an import to pin it (e.g. `Map.Entry`).
    pub java_fqns: BTreeSet<String>,
    pub fields: BTreeSet<String>,
    pub methods: BTreeMap<String, StubSig>,
    pub statics: BTreeMap<String, StubSig>,
    /// Static-final constant names accessed as `Type::NAME` (e.g.
    /// `TimeUnit.MILLISECONDS`), emitted as `pub const NAME: Unknown = ()`.
    pub static_consts: BTreeSet<String>,
    /// Constructors keyed by arity — Java overloads by parameter count, which
    /// Rust can't, so each arity becomes a distinct `new`/`new_<arity>` fn.
    pub ctors: BTreeMap<usize, StubSig>,
}

#[derive(Default)]
pub struct StubCollector {
    /// Keyed by Rust struct name (so the rendered file has no duplicate
    /// definitions); each carries its Java FQN guess(es) as provenance.
    pub types: BTreeMap<String, StubType>,
    pub free_fns: BTreeMap<String, StubSig>,
}

fn keep_better(slot: &mut StubSig, new: StubSig) {
    if new.info() > slot.info() || (slot.params.is_empty() && !new.params.is_empty()) {
        *slot = new;
    }
}

impl StubCollector {
    pub fn is_empty(&self) -> bool {
        self.types.is_empty() && self.free_fns.is_empty()
    }

    fn type_entry(&mut self, fqn: &str, rust_name: &str) -> &mut StubType {
        let e = self.types.entry(rust_name.to_string()).or_default();
        if e.rust_name.is_empty() {
            e.rust_name = rust_name.to_string();
        }
        e.java_fqns.insert(fqn.to_string());
        e
    }

    pub fn note_type(&mut self, fqn: &str, rust_name: &str) {
        self.type_entry(fqn, rust_name);
    }

    pub fn add_method(&mut self, fqn: &str, rust_name: &str, method: &str, sig: StubSig, is_static: bool) {
        let t = self.type_entry(fqn, rust_name);
        let map = if is_static { &mut t.statics } else { &mut t.methods };
        match map.get_mut(method) {
            Some(existing) => keep_better(existing, sig),
            None => {
                map.insert(method.to_string(), sig);
            }
        }
    }

    pub fn add_ctor(&mut self, fqn: &str, rust_name: &str, sig: StubSig) {
        let t = self.type_entry(fqn, rust_name);
        let arity = sig.params.len();
        match t.ctors.get_mut(&arity) {
            Some(existing) => keep_better(existing, sig),
            None => {
                t.ctors.insert(arity, sig);
            }
        }
    }

    pub fn add_field(&mut self, fqn: &str, rust_name: &str, field: &str) {
        self.type_entry(fqn, rust_name).fields.insert(field.to_string());
    }

    pub fn add_static_const(&mut self, fqn: &str, rust_name: &str, name: &str) {
        self.type_entry(fqn, rust_name).static_consts.insert(name.to_string());
    }

    pub fn add_free_fn(&mut self, rust_name: &str, sig: StubSig) {
        match self.free_fns.get_mut(rust_name) {
            Some(existing) => keep_better(existing, sig),
            None => {
                self.free_fns.insert(rust_name.to_string(), sig);
            }
        }
    }

    /// A symbol map from each stub type's Java FQN(s) to its `crate::stub_…::Name`
    /// path, matching the per-package files [`render_grouped`] emits. Merged into
    /// the link index (crate mode, pass 2) so references to stubbed externals
    /// resolve instead of staying bare.
    pub fn crate_symbol_map(&self) -> SymbolMap {
        let mut map = SymbolMap::default();
        for t in self.types.values() {
            let pkg = t
                .java_fqns
                .iter()
                .next()
                .and_then(|f| f.rsplit_once('.').map(|(p, _)| p.to_string()))
                .unwrap_or_default();
            let module = if pkg.is_empty() {
                "stubs".to_string()
            } else {
                format!("stub_{}", pkg.replace('.', "_"))
            };
            let rust_path = format!("crate::{module}::{}", t.rust_name);
            // Expose the per-arity constructors so an object creation resolves to
            // the right `new`/`new_<arity>` (see `resolve_ctor`).
            let mut methods = BTreeMap::new();
            for (i, (arity, _)) in t.ctors.iter().enumerate() {
                let (rust, key) = ctor_names(i, *arity);
                methods.insert(key, MethodSym { rust, ..Default::default() });
            }
            let entry = TypeSym {
                rust_path: rust_path.clone(),
                kind: "struct".to_string(),
                parent: None,
                interfaces: Vec::new(),
                generic: false,
                generic_params: Vec::new(),
                fields: Default::default(),
                static_fields: Default::default(),
                methods,
            };
            for fqn in &t.java_fqns {
                map.types.insert(fqn.clone(), entry.clone());
            }
            // Also key by bare simple name, so a reference resolves regardless of
            // how the file imports it. Safe: only stubs add simple-name keys, and
            // project/dep types resolve via their FQN (import/package) first.
            map.types.entry(t.rust_name.clone()).or_insert(entry);
        }
        map
    }

    pub fn merge(&mut self, other: StubCollector) {
        for (_rust, t) in other.types {
            let fqn = t.java_fqns.iter().next().cloned().unwrap_or_default();
            for f in &t.java_fqns {
                self.note_type(f, &t.rust_name);
            }
            for (m, sig) in t.methods {
                self.add_method(&fqn, &t.rust_name, &m, sig, false);
            }
            for (m, sig) in t.statics {
                self.add_method(&fqn, &t.rust_name, &m, sig, true);
            }
            for (_arity, c) in t.ctors {
                self.add_ctor(&fqn, &t.rust_name, c);
            }
            for f in t.fields {
                self.add_field(&fqn, &t.rust_name, &f);
            }
            for c in t.static_consts {
                self.add_static_const(&fqn, &t.rust_name, &c);
            }
        }
        for (name, sig) in other.free_fns {
            self.add_free_fn(&name, sig);
        }
    }

    /// `rust_name -> crate path` for every stub type, so a stub method's return
    /// that names another stub type can be emitted as a resolvable path.
    fn stub_type_paths(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        for t in self.types.values() {
            let pkg = t
                .java_fqns
                .iter()
                .next()
                .and_then(|f| f.rsplit_once('.').map(|(p, _)| p.to_string()))
                .unwrap_or_default();
            let module = if pkg.is_empty() {
                "stubs".to_string()
            } else {
                format!("stub_{}", pkg.replace('.', "_"))
            };
            m.insert(t.rust_name.clone(), format!("crate::{module}::{}", t.rust_name));
        }
        m
    }

    /// Render all stubs as a single Rust source file.
    pub fn render(&self) -> String {
        let paths = self.stub_type_paths();
        let mut s = file_header();
        for t in self.types.values() {
            s.push_str(&render_type(t, &paths));
        }
        for (name, sig) in &self.free_fns {
            s.push_str(&render_fn(name, sig, None, &paths));
            s.push('\n');
        }
        s
    }

    /// Render stubs split by originating package (a proxy for the dependency
    /// JAR), as a map of filename -> contents: `stub_<package>.rs` per package,
    /// and `stubs.rs` for free functions and package-less types. Group a whole
    /// translation's stubs so each dependency can be filled in independently.
    pub fn render_grouped(&self) -> BTreeMap<String, String> {
        let paths = self.stub_type_paths();
        // package -> rendered type blocks
        let mut groups: BTreeMap<String, String> = BTreeMap::new();
        for t in self.types.values() {
            let pkg = t
                .java_fqns
                .iter()
                .next()
                .and_then(|f| f.rsplit_once('.').map(|(p, _)| p.to_string()))
                .unwrap_or_default();
            groups.entry(pkg).or_default().push_str(&render_type(t, &paths));
        }
        // Free functions have no package; put them in the base file.
        if !self.free_fns.is_empty() {
            let base = groups.entry(String::new()).or_default();
            for (name, sig) in &self.free_fns {
                base.push_str(&render_fn(name, sig, None, &paths));
                base.push('\n');
            }
        }
        groups
            .into_iter()
            .map(|(pkg, body)| {
                let filename = if pkg.is_empty() {
                    "stubs.rs".to_string()
                } else {
                    format!("stub_{}.rs", pkg.replace('.', "_"))
                };
                (filename, format!("{}{}", file_header(), body))
            })
            .collect()
    }
}

fn file_header() -> String {
    "//! Auto-generated stubs for unresolved external symbols.\n\
     //! Signatures are best-effort (inferred from call sites); fill in the\n\
     //! bodies and replace `Unknown` placeholders, or translate the real\n\
     //! dependency, run `gen-symbols` on it, and `--link` its map instead.\n\
     #![allow(dead_code, unused_variables, non_snake_case)]\n\n\
     /// Placeholder for a type that could not be inferred. A real struct (not a\n\
     /// `()` alias) so it can implement the traits stubbed values are used with\n\
     /// (`Display` in `format!`, `Hash`/`Eq`/`Ord` as map keys, etc.).\n\
     #[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]\n\
     pub struct Unknown;\n\
     impl std::fmt::Display for Unknown {\n\
         fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }\n\
     }\n\
     // A stubbed iterable/stream resolves to `Unknown`; a degenerate empty\n\
     // iterator lets `for x in unknown {}` and `unknown.collect()` compile.\n\
     impl Iterator for Unknown {\n\
         type Item = Unknown;\n\
         fn next(&mut self) -> Option<Unknown> { None }\n\
     }\n\
     // Arithmetic on a stubbed numeric value (with `Unknown` on the left): keep\n\
     // it `Unknown` so the surrounding expression still type-checks. (`prim op\n\
     // Unknown` can't be covered — the orphan rule forbids it.)\n\
     macro_rules! __unknown_op { ($t:ident, $m:ident) => {\n\
         impl<T> std::ops::$t<T> for Unknown { type Output = Unknown;\n\
             fn $m(self, _: T) -> Unknown { Unknown } }\n\
     }; }\n\
     __unknown_op!(Add, add); __unknown_op!(Sub, sub); __unknown_op!(Mul, mul);\n\
     __unknown_op!(Div, div); __unknown_op!(Rem, rem);\n\n"
        .to_string()
}

/// The `(rust-name, symbol-map-key)` for the `idx`-th constructor (in ascending
/// arity): the lowest arity is the base `new`; others are `new_<arity>`, keyed
/// `new#<arity>` so a caller resolves the right overload by argument count.
fn ctor_names(idx: usize, arity: usize) -> (String, String) {
    if idx == 0 {
        ("new".to_string(), "new".to_string())
    } else {
        (format!("new_{arity}"), format!("new#{arity}"))
    }
}

fn render_type(t: &StubType, paths: &BTreeMap<String, String>) -> String {
    let mut s = String::new();
    for fqn in &t.java_fqns {
        s.push_str(&format!("/// @java {fqn}\n"));
    }
    // Every stub field is typed `Unknown` (which derives all of these), so a
    // stub can always satisfy `Eq`/`Hash`/`Ord` — needed when a stubbed value is
    // a `HashMap`/`BTreeMap` key or compared with `==` (common for external
    // enum-like constants).
    s.push_str("#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]\n");
    if t.fields.is_empty() {
        s.push_str(&format!("pub struct {} {{}}\n", t.rust_name));
    } else {
        s.push_str(&format!("pub struct {} {{\n", t.rust_name));
        for f in &t.fields {
            s.push_str(&format!("    pub {f}: Unknown,\n"));
        }
        s.push_str("}\n");
    }
    let has_impl = !t.ctors.is_empty()
        || !t.methods.is_empty()
        || !t.statics.is_empty()
        || !t.static_consts.is_empty();
    if has_impl {
        s.push_str(&format!("impl {} {{\n", t.rust_name));
        for c in &t.static_consts {
            s.push_str(&format!("    pub const {c}: Unknown = Unknown;\n"));
        }
        for (i, (arity, c)) in t.ctors.iter().enumerate() {
            let (rust, _) = ctor_names(i, *arity);
            s.push_str(&format!("    {}\n", render_fn(&rust, c, Some(&t.rust_name), paths)));
        }
        for (name, sig) in &t.statics {
            s.push_str(&format!("    {}\n", render_fn(name, sig, None, paths)));
        }
        for (name, sig) in &t.methods {
            s.push_str(&format!("    {}\n", render_fn(name, sig, None, paths)));
        }
        s.push_str("}\n");
    }
    s.push('\n');
    s
}

/// Render a single function/method as one line. `ctor_ret` forces the return
/// type (the type name) for constructors.
fn render_fn(
    name: &str,
    sig: &StubSig,
    ctor_ret: Option<&str>,
    paths: &BTreeMap<String, String>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    match sig.receiver {
        Receiver::Ref => parts.push("&self".to_string()),
        Receiver::RefMut => parts.push("&mut self".to_string()),
        Receiver::None => {}
    }
    // Each parameter is a generic type so any argument is accepted: the inferred
    // param types are guesses, and naming another stub type here would reference
    // it by bare name across self-contained stub files (E0412). Generics also
    // avoid argument-type mismatches against `Unknown`.
    let generics = if sig.params.is_empty() {
        String::new()
    } else {
        let ps: Vec<String> = (0..sig.params.len()).map(|i| format!("A{i}")).collect();
        format!("<{}>", ps.join(", "))
    };
    for i in 0..sig.params.len() {
        parts.push(format!("a{i}: A{i}"));
    }
    // The constructor returns the stub's own (same-file) type. A method return is
    // kept only when it's resolvable here (a primitive/std type or `Unknown`);
    // any other name would be a cross-file stub reference, so drop it.
    // `BufferedReader.readLine()` is the canonical nullable JDK reader method;
    // force its stub return to `Option<String>` so the read-loop lowering
    // `while let Some(line) = rdr.read_line()` typechecks (see
    // `as_readline_assign`).
    let ret = if ctor_ret.is_none() && name == "read_line" && sig.params.is_empty() {
        " -> Option<String>".to_string()
    } else {
        match ctor_ret {
        Some(t) => format!(" -> {t}"),
        None => match sig.ret.as_deref() {
            // A bare name that's another stub type -> its crate path (resolves
            // cross-file; bare names from pass 1 otherwise get dropped).
            Some(t) => {
                let resolved = paths.get(t.trim()).map(String::as_str).unwrap_or(t);
                if stub_ret_renderable(resolved) {
                    format!(" -> {resolved}")
                } else {
                    " -> Unknown".to_string()
                }
            }
            // No inferred return -> `Unknown` (a real type implementing Display/
            // Hash/etc.), so a stubbed result used in `format!`/as a key compiles.
            None => " -> Unknown".to_string(),
        },
        }
    };
    format!("pub fn {name}{generics}({}){ret} {{ unimplemented!() }}", parts.join(", "))
}

/// A stub return type is renderable in a self-contained stub file only if it's
/// `Unknown` or built from primitive/std tokens (no cross-file stub names).
fn stub_ret_renderable(t: &str) -> bool {
    let t = t.trim_start_matches('&').trim();
    if t == UNKNOWN {
        return true;
    }
    // A fully-qualified path (`crate::…`, `std::…`) resolves from inside the stub
    // file, so a project/std return type is renderable as-is.
    if t.starts_with("crate::") || t.starts_with("std::") || t.starts_with("core::") {
        return true;
    }
    // A generic container named with no type argument (`Vec`, `Option`, …) takes
    // type params -> `-> Vec` is `wrong number of generic arguments`; drop it.
    if !t.contains('<')
        && matches!(t, "Vec" | "Option" | "Box" | "HashMap" | "HashSet" | "BTreeMap")
    {
        return false;
    }
    const SAFE: &[&str] = &[
        "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "usize", "isize",
        "f32", "f64", "bool", "char", "String", "str", "Vec", "Option", "Box", "HashMap",
        "HashSet", "BTreeMap", "std", "core", "alloc", "collections", "Unknown",
    ];
    let tokens: Vec<&str> = t
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty() && s.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false))
        .collect();
    !tokens.is_empty() && tokens.iter().all(|s| SAFE.contains(s))
}

/// Walk a parsed compilation unit and collect the fully-qualified names of every
/// type it declares (including nested types), for cross-file dedup of stubs.
pub fn collect_defined_types(java: &str, out: &mut std::collections::HashSet<String>) {
    let Some((arena, root)) = crate::parse::create_compilation_unit(java) else {
        return;
    };
    let mut id = crate::id_tracker::IdTracker::new();
    crate::id_tracker::run(&arena, root, &mut id);
    let pkg = id.package_name.clone().unwrap_or_default();
    collect_types_rec(&arena, root, &pkg, "", out);
}

fn collect_types_rec(
    arena: &crate::ast::Arena,
    node: crate::ast::NodeId,
    pkg: &str,
    prefix: &str,
    out: &mut std::collections::HashSet<String>,
) {
    use crate::ast::Node;
    for c in arena.children(node) {
        let name = match arena.kind(c) {
            Node::ClassOrInterfaceDeclaration { name, .. } | Node::EnumDeclaration { name, .. } => {
                Some(name.clone())
            }
            _ => None,
        };
        if let Some(name) = name {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            let fqn = if pkg.is_empty() {
                path.clone()
            } else {
                format!("{pkg}.{path}")
            };
            out.insert(fqn);
            // Also index by simple name so nested types (`Outer.Inner`) and
            // same-package references resolve when used by their bare name.
            out.insert(name.clone());
            collect_types_rec(arena, c, pkg, &path, out);
        } else {
            collect_types_rec(arena, c, pkg, prefix, out);
        }
    }
}
