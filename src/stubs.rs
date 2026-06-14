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
    pub ctor: Option<StubSig>,
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
        match &mut t.ctor {
            Some(existing) => keep_better(existing, sig),
            None => t.ctor = Some(sig),
        }
    }

    pub fn add_field(&mut self, fqn: &str, rust_name: &str, field: &str) {
        self.type_entry(fqn, rust_name).fields.insert(field.to_string());
    }

    pub fn add_free_fn(&mut self, rust_name: &str, sig: StubSig) {
        match self.free_fns.get_mut(rust_name) {
            Some(existing) => keep_better(existing, sig),
            None => {
                self.free_fns.insert(rust_name.to_string(), sig);
            }
        }
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
            if let Some(c) = t.ctor {
                self.add_ctor(&fqn, &t.rust_name, c);
            }
            for f in t.fields {
                self.add_field(&fqn, &t.rust_name, &f);
            }
        }
        for (name, sig) in other.free_fns {
            self.add_free_fn(&name, sig);
        }
    }

    /// Render all stubs as a single Rust source file.
    pub fn render(&self) -> String {
        let mut s = file_header();
        for t in self.types.values() {
            s.push_str(&render_type(t));
        }
        for (name, sig) in &self.free_fns {
            s.push_str(&render_fn(name, sig, None));
            s.push('\n');
        }
        s
    }

    /// Render stubs split by originating package (a proxy for the dependency
    /// JAR), as a map of filename -> contents: `stub_<package>.rs` per package,
    /// and `stubs.rs` for free functions and package-less types. Group a whole
    /// translation's stubs so each dependency can be filled in independently.
    pub fn render_grouped(&self) -> BTreeMap<String, String> {
        // package -> rendered type blocks
        let mut groups: BTreeMap<String, String> = BTreeMap::new();
        for t in self.types.values() {
            let pkg = t
                .java_fqns
                .iter()
                .next()
                .and_then(|f| f.rsplit_once('.').map(|(p, _)| p.to_string()))
                .unwrap_or_default();
            groups.entry(pkg).or_default().push_str(&render_type(t));
        }
        // Free functions have no package; put them in the base file.
        if !self.free_fns.is_empty() {
            let base = groups.entry(String::new()).or_default();
            for (name, sig) in &self.free_fns {
                base.push_str(&render_fn(name, sig, None));
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
     /// Placeholder for a type that could not be inferred.\n\
     pub type Unknown = ();\n\n"
        .to_string()
}

fn render_type(t: &StubType) -> String {
    let mut s = String::new();
    for fqn in &t.java_fqns {
        s.push_str(&format!("/// @java {fqn}\n"));
    }
    s.push_str("#[derive(Clone, Default)]\n");
    if t.fields.is_empty() {
        s.push_str(&format!("pub struct {} {{}}\n", t.rust_name));
    } else {
        s.push_str(&format!("pub struct {} {{\n", t.rust_name));
        for f in &t.fields {
            s.push_str(&format!("    pub {f}: Unknown,\n"));
        }
        s.push_str("}\n");
    }
    let has_impl = t.ctor.is_some() || !t.methods.is_empty() || !t.statics.is_empty();
    if has_impl {
        s.push_str(&format!("impl {} {{\n", t.rust_name));
        if let Some(c) = &t.ctor {
            s.push_str(&format!("    {}\n", render_fn("new", c, Some(&t.rust_name))));
        }
        for (name, sig) in &t.statics {
            s.push_str(&format!("    {}\n", render_fn(name, sig, None)));
        }
        for (name, sig) in &t.methods {
            s.push_str(&format!("    {}\n", render_fn(name, sig, None)));
        }
        s.push_str("}\n");
    }
    s.push('\n');
    s
}

/// Render a single function/method as one line. `ctor_ret` forces the return
/// type (the type name) for constructors.
fn render_fn(name: &str, sig: &StubSig, ctor_ret: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    match sig.receiver {
        Receiver::Ref => parts.push("&self".to_string()),
        Receiver::RefMut => parts.push("&mut self".to_string()),
        Receiver::None => {}
    }
    for (i, p) in sig.params.iter().enumerate() {
        parts.push(format!("a{i}: {p}"));
    }
    let ret = match ctor_ret {
        Some(t) => format!(" -> {t}"),
        None => match &sig.ret {
            Some(t) => format!(" -> {t}"),
            None => String::new(),
        },
    };
    format!("pub fn {name}({}){ret} {{ unimplemented!() }}", parts.join(", "))
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
