//! Crate/module wiring (`--crate`).
//!
//! Two halves:
//! 1. A **pre-pass** that builds a [`SymbolMap`] of the project's *own* types,
//!    keyed by Java FQN with `rust_path` = `crate::<module path>::<RustName>`.
//!    Merged into the translator's `LinkIndex`, it makes cross-file references
//!    resolve to real crate paths via the existing link machinery (the project
//!    is, in effect, linked against itself).
//! 2. A **post-pass** that walks the emitted tree and writes the `mod` tree
//!    (`lib.rs` + per-directory `mod.rs`) and a `Cargo.toml`, so the files form
//!    one buildable crate.
//!
//! The module path of a file mirrors the output layout exactly: the input
//! directory's basename, then the package directories, then the snake-cased
//! file stem (each Java file is one Rust module). So the pre-pass paths and the
//! post-pass tree agree by construction.

use std::collections::BTreeSet;
use std::path::Path;

use crate::ast::{Arena, Node, NodeId};
use crate::dump::escape_rust_keyword;
use crate::naming::camel_to_snake_case;
use crate::symbol_map::{SymbolMap, TypeSym};

/// Build a symbol map of every type defined under `input_root`, with crate-path
/// `rust_path`s matching the output module layout.
pub fn build_project_map(input_root: &Path) -> SymbolMap {
    let mut map = SymbolMap::default();
    let base = input_root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if input_root.is_dir() {
        walk_sources(input_root, &[base], &mut map);
    } else {
        // Single-file input: module path is just the file stem.
        add_file(input_root, &[], &mut map);
    }
    map
}

fn walk_sources(dir: &Path, prefix: &[String], map: &mut SymbolMap) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let mut sub = prefix.to_vec();
            sub.push(name);
            walk_sources(&path, &sub, map);
        } else if path.extension().map(|e| e == "java").unwrap_or(false) {
            add_file(&path, prefix, map);
        }
    }
}

fn add_file(path: &Path, prefix: &[String], map: &mut SymbolMap) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let module = camel_to_snake_case(&stem);
    let mut segs = prefix.to_vec();
    segs.push(module);
    // Escape path segments that are Rust keywords (e.g. a Java package `ref`):
    // `crate::…::r#ref::…`. The on-disk file/dir name stays unescaped.
    let escaped: Vec<String> = segs.iter().map(|s| escape_rust_keyword(s.clone())).collect();
    let mod_path = format!("crate::{}", escaped.join("::"));

    for fqn in collect_defined_fqns(&text) {
        let rust_name = fqn.rsplit('.').next().unwrap_or(&fqn).replace('$', "_");
        map.types.insert(
            fqn,
            TypeSym {
                rust_path: format!("{mod_path}::{rust_name}"),
                kind: "struct".to_string(),
                fields: Default::default(),
                methods: Default::default(),
            },
        );
    }
}

/// Full FQNs of every type declared in a Java source (incl. nested types).
fn collect_defined_fqns(java: &str) -> Vec<String> {
    let Some((arena, root)) = crate::parse::create_compilation_unit(java) else {
        return Vec::new();
    };
    let mut id = crate::id_tracker::IdTracker::new();
    crate::id_tracker::run(&arena, root, &mut id);
    let pkg = id.package_name.clone().unwrap_or_default();
    let mut out = Vec::new();
    collect_rec(&arena, root, &pkg, "", &mut out);
    out
}

fn collect_rec(arena: &Arena, node: NodeId, pkg: &str, prefix: &str, out: &mut Vec<String>) {
    for c in arena.children(node) {
        let name = match arena.kind(c) {
            Node::ClassOrInterfaceDeclaration { name, .. } | Node::EnumDeclaration { name, .. } => {
                Some(name.clone())
            }
            _ => None,
        };
        if let Some(name) = name {
            let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}.{name}") };
            let fqn = if pkg.is_empty() { path.clone() } else { format!("{pkg}.{path}") };
            out.push(fqn);
            collect_rec(arena, c, pkg, &path, out);
        } else {
            collect_rec(arena, c, pkg, prefix, out);
        }
    }
}

// ---- post-pass: module tree + Cargo.toml ----

/// Generate the `mod` tree (`lib.rs` at the root, `mod.rs` in each subdir) and a
/// `Cargo.toml`, turning the emitted files into one crate rooted at `out_root`.
pub fn finish_crate(out_root: &Path) -> std::io::Result<()> {
    gen_mod_file(out_root, true)?;
    let name = out_root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "translated".to_string());
    let crate_name = sanitize_crate_name(&name);
    let cargo = format!(
        "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [lib]\npath = \"lib.rs\"\n\n[dependencies]\n",
    );
    std::fs::write(out_root.join("Cargo.toml"), cargo)?;
    Ok(())
}

/// Write the module file for `dir` (`lib.rs` if `is_root`, else `mod.rs`),
/// declaring each child module, and recurse into subdirectories.
fn gen_mod_file(dir: &Path, is_root: bool) -> std::io::Result<()> {
    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in &entries {
        if path.is_dir() {
            let m = path.file_name().unwrap().to_string_lossy().into_owned();
            if is_ident(&m) {
                dirs.push(m);
            }
            gen_mod_file(path, false)?;
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            if stem == "mod" || stem == "lib" {
                continue;
            }
            if is_ident(&stem) {
                files.push(stem);
            }
        }
    }
    // A file module whose name collides with a sibling directory module is
    // dropped (the directory wins) — Rust can't have both `mod x;` resolve to
    // `x.rs` and `x/mod.rs`.
    let dirset: BTreeSet<&String> = dirs.iter().collect();
    let mut mods: Vec<&String> = dirs.iter().chain(files.iter().filter(|f| !dirset.contains(f))).collect();
    mods.sort();
    mods.dedup();

    let header = "// Auto-generated module tree (java2rust --crate).\n\
                  #![allow(dead_code, unused_variables, unused_imports, non_snake_case, non_camel_case_types)]\n\n";
    let mut body = String::new();
    if is_root {
        body.push_str(header);
    }
    for m in mods {
        // `pub mod r#ref;` for a keyword dir/file name (file stays `ref.rs`).
        body.push_str(&format!("pub mod {};\n", escape_rust_keyword(m.clone())));
    }
    let file = if is_root { "lib.rs" } else { "mod.rs" };
    std::fs::write(dir.join(file), body)?;
    Ok(())
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn sanitize_crate_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    if n.is_empty() || n.chars().next().unwrap().is_ascii_digit() {
        format!("c_{n}")
    } else {
        n
    }
}
