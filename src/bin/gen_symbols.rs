//! gen-symbols — extract a Java→Rust symbol map from a (possibly LLM-edited)
//! translated Rust crate.
//!
//! It parses the crate's `.rs` files with `syn`, finds items carrying a
//! `/// @java <fqn>` provenance marker (emitted by the translator), and reads
//! their *current* Rust signatures — so the map reflects whatever the code looks
//! like now, including hand/LLM edits (Option, &mut, renames, …).
//!
//! Usage: gen-symbols <crate-dir-or-file> [-o map.json]
//!
//! Output: JSON keyed by Java FQN (see `java2rust_rs::symbol_map`). Types carry
//! their fields/methods; each method records receiver kind, params (with
//! by_ref/mutable/nullable) and return (with nullable).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use quote::ToTokens;

use java2rust_rs::symbol_map::{FieldSym, MethodSym, ParamSym, SymbolMap, TypeSym};

#[derive(Default)]
struct Collector {
    /// java type FQN -> TypeSym
    types: BTreeMap<String, TypeSym>,
    /// members seen before their type: java FQN with '#' -> field/method
    orphan_fields: BTreeMap<String, FieldSym>,
    orphan_methods: BTreeMap<String, MethodSym>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = None;
    let mut out: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                out = args.get(i).cloned();
            }
            other => input = Some(other.to_string()),
        }
        i += 1;
    }
    let Some(input) = input else {
        eprintln!("usage: gen-symbols <crate-dir-or-file> [-o map.json]");
        std::process::exit(2);
    };

    let root = PathBuf::from(&input);
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    files.sort();

    let mut c = Collector::default();
    for f in &files {
        let module = module_path_of(&root, f);
        if let Ok(src) = fs::read_to_string(f) {
            if let Ok(file) = syn::parse_file(&src) {
                process_items(&file.items, &module, &mut c);
            } else {
                eprintln!("warning: could not parse {}", f.display());
            }
        }
    }

    // Attach orphan members to their types by FQN prefix.
    for (fqn, fld) in std::mem::take(&mut c.orphan_fields) {
        if let Some((ty, mem)) = fqn.split_once('#') {
            if let Some(t) = c.types.get_mut(ty) {
                t.fields.insert(mem.to_string(), fld);
            }
        }
    }
    for (fqn, m) in std::mem::take(&mut c.orphan_methods) {
        if let Some((ty, mem)) = fqn.split_once('#') {
            if let Some(t) = c.types.get_mut(ty) {
                t.methods.insert(mem.to_string(), m);
            }
        }
    }

    let map = SymbolMap { types: c.types };
    let json = serde_json::to_string_pretty(&map).expect("serialize map");
    match out {
        Some(path) => {
            fs::write(&path, json).expect("write map");
            eprintln!("wrote {path}");
        }
        None => println!("{json}"),
    }
}

fn collect_rs(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_file() {
        if p.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(p.to_path_buf());
        }
        return;
    }
    if let Ok(rd) = fs::read_dir(p) {
        for e in rd.flatten() {
            collect_rs(&e.path(), out);
        }
    }
}

/// Module path derived from the file's location relative to the crate root
/// (e.g. `org/broadinstitute/Foo.rs` -> `org::broadinstitute`). `lib`/`main`/
/// `mod` stems contribute nothing.
fn module_path_of(root: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let mut parts: Vec<String> = Vec::new();
    for comp in rel.components() {
        parts.push(comp.as_os_str().to_string_lossy().into_owned());
    }
    if let Some(last) = parts.last_mut() {
        *last = last.trim_end_matches(".rs").to_string();
    }
    parts
        .into_iter()
        .filter(|s| !matches!(s.as_str(), "lib" | "main" | "mod" | ""))
        .collect::<Vec<_>>()
        .join("::")
}

fn process_items(items: &[syn::Item], module: &str, c: &mut Collector) {
    for item in items {
        match item {
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    let sub = join_mod(module, &m.ident.to_string());
                    process_items(inner, &sub, c);
                }
            }
            syn::Item::Struct(s) => {
                record_type(c, &s.attrs, module, &s.ident.to_string(), "struct");
                if let Some(java_ty) = doc_java(&s.attrs) {
                    for f in &s.fields {
                        if let Some(name) = &f.ident {
                            if let Some(fmem) = doc_java(&f.attrs) {
                                let fld = FieldSym {
                                    rust: name.to_string(),
                                    rust_type: type_str(&f.ty),
                                    nullable: is_option(&f.ty),
                                };
                                insert_field(c, &java_ty, &fmem, fld);
                            }
                        }
                    }
                }
            }
            syn::Item::Enum(e) => {
                record_type(c, &e.attrs, module, &e.ident.to_string(), "enum");
            }
            syn::Item::Trait(t) => {
                record_type(c, &t.attrs, module, &t.ident.to_string(), "trait");
                for ti in &t.items {
                    if let syn::TraitItem::Fn(f) = ti {
                        if let Some(jm) = doc_java(&f.attrs) {
                            let m = method_sym(&f.sig, module, "Self");
                            insert_method(c, &jm, m);
                        }
                    }
                }
            }
            syn::Item::Impl(im) => {
                let self_ty = last_path_ident(&im.self_ty).unwrap_or_else(|| "Self".into());
                for ii in &im.items {
                    if let syn::ImplItem::Fn(f) = ii {
                        if let Some(jm) = doc_java(&f.attrs) {
                            let m = method_sym(&f.sig, module, &self_ty);
                            insert_method(c, &jm, m);
                        }
                    }
                }
            }
            syn::Item::Fn(f) => {
                if let Some(jm) = doc_java(&f.attrs) {
                    let m = method_sym(&f.sig, module, "");
                    insert_method(c, &jm, m);
                }
            }
            _ => {}
        }
    }
}

fn record_type(c: &mut Collector, attrs: &[syn::Attribute], module: &str, ident: &str, kind: &str) {
    if let Some(java) = doc_java(attrs) {
        let entry = c.types.entry(java).or_default();
        entry.rust_path = join_mod(module, ident);
        entry.kind = kind.to_string();
    }
}

fn insert_field(c: &mut Collector, java_ty: &str, member_fqn: &str, fld: FieldSym) {
    // member_fqn is like "pkg.Type#field"
    if let Some(t) = c.types.get_mut(java_ty) {
        if let Some((_, mem)) = member_fqn.split_once('#') {
            t.fields.insert(mem.to_string(), fld);
            return;
        }
    }
    c.orphan_fields.insert(member_fqn.to_string(), fld);
}

fn insert_method(c: &mut Collector, member_fqn: &str, m: MethodSym) {
    if let Some((ty, mem)) = member_fqn.split_once('#') {
        if let Some(t) = c.types.get_mut(ty) {
            t.methods.insert(mem.to_string(), m);
            return;
        }
    }
    c.orphan_methods.insert(member_fqn.to_string(), m);
}

fn method_sym(sig: &syn::Signature, module: &str, self_ty: &str) -> MethodSym {
    let mut m = MethodSym {
        rust: sig.ident.to_string(),
        receiver: "none".to_string(),
        ..Default::default()
    };
    let base = if self_ty.is_empty() {
        module.to_string()
    } else {
        join_mod(module, self_ty)
    };
    m.rust_path = join_mod(&base, &m.rust);
    for input in &sig.inputs {
        match input {
            syn::FnArg::Receiver(r) => {
                m.receiver = if r.reference.is_some() {
                    if r.mutability.is_some() { "refmut" } else { "ref" }
                } else {
                    "value"
                }
                .to_string();
            }
            syn::FnArg::Typed(pt) => {
                let (by_ref, mutable, inner) = deref(&pt.ty);
                m.params.push(ParamSym {
                    rust_type: type_str(&pt.ty),
                    by_ref,
                    mutable,
                    nullable: is_option(inner),
                });
            }
        }
    }
    if let syn::ReturnType::Type(_, ty) = &sig.output {
        m.ret = Some(type_str(ty));
        m.ret_nullable = is_option(ty);
    }
    m
}

/// Returns (is_reference, is_mut_reference, inner-or-self type).
fn deref(ty: &syn::Type) -> (bool, bool, &syn::Type) {
    if let syn::Type::Reference(r) = ty {
        (true, r.mutability.is_some(), &r.elem)
    } else {
        (false, false, ty)
    }
}

fn is_option(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            return seg.ident == "Option";
        }
    }
    false
}

fn type_str(ty: &syn::Type) -> String {
    let s = ty.to_token_stream().to_string();
    // Tidy the most common token spacing.
    s.replace(" < ", "<")
        .replace(" > ", ">")
        .replace(" >", ">")
        .replace("< ", "<")
        .replace(" ::", "::")
        .replace(":: ", "::")
        .replace("& ", "&")
}

fn last_path_ident(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(p) = ty {
        return p.path.segments.last().map(|s| s.ident.to_string());
    }
    None
}

/// Extract the FQN from a `/// @java <fqn>` doc attribute, if present.
fn doc_java(attrs: &[syn::Attribute]) -> Option<String> {
    for a in attrs {
        if !a.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(nv) = &a.meta {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                let v = s.value();
                let v = v.trim();
                if let Some(rest) = v.strip_prefix("@java ") {
                    return Some(rest.trim().to_string());
                }
            }
        }
    }
    None
}

fn join_mod(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        format!("{a}::{b}")
    }
}
