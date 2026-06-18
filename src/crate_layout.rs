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
use crate::dump::{escape_rust_keyword, sanitize_path_segments};
use std::collections::HashSet;

use crate::naming::camel_to_snake_case;
use crate::symbol_map::{FieldSym, MethodSym, SymbolMap, TypeSym};

/// A type as collected from source, before its parent FQN is resolved.
struct RawType {
    fqn: String,
    rust_path: String,
    kind: String,
    generic: bool,
    generic_params: Vec<String>,
    /// `extends` superclass simple name (classes only).
    parent_simple: Option<String>,
    /// `implements` interface simple names.
    interface_simples: Vec<String>,
    /// (java name, rust name, java simple type, nullable) for instance fields.
    fields: Vec<(String, String, String, bool)>,
    /// (java, rust, java simple type, nullable) for static fields (assoc consts).
    static_fields: Vec<(String, String, String, bool)>,
    /// (symbol-map key, rust name, param signatures, throws, recv-mut,
    /// numeric-ret) for instance methods. The key is the bare Java name, or
    /// `name#arity` for non-base overloads.
    methods: Vec<(String, String, Vec<crate::symbol_map::ParamSym>, bool, bool, Option<String>, bool)>,
    /// The defining file's import/package context, for resolving `parent_simple`.
    explicit_imports: Vec<String>,
    wildcard_pkgs: Vec<String>,
    package: String,
}

/// Build a symbol map of every type defined under `input_root`: crate-path
/// `rust_path`, resolved `parent` FQN, and own instance fields/methods (for
/// inherited-member resolution during codegen).
pub fn build_project_map(input_root: &Path) -> SymbolMap {
    let base = input_root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut raw = Vec::new();
    let mut dispatched = BTreeSet::new();
    if input_root.is_dir() {
        walk_sources(input_root, &[base], &mut raw, &mut dispatched);
    } else {
        collect_file(input_root, &[], &mut raw, &mut dispatched);
    }

    let defined: HashSet<&String> = raw.iter().map(|r| &r.fqn).collect();
    let mut map = SymbolMap::default();
    map.dispatched = dispatched;
    for r in &raw {
        let parent = r.parent_simple.as_ref().and_then(|p| {
            resolve_parent(p, &r.explicit_imports, &r.wildcard_pkgs, &r.package, &r.fqn, &defined)
        });
        let interfaces = r
            .interface_simples
            .iter()
            .filter_map(|s| {
                resolve_parent(s, &r.explicit_imports, &r.wildcard_pkgs, &r.package, &r.fqn, &defined)
            })
            .collect();
        let mut t = TypeSym {
            rust_path: r.rust_path.clone(),
            kind: r.kind.clone(),
            parent,
            interfaces,
            generic: r.generic,
            generic_params: r.generic_params.clone(),
            fields: Default::default(),
            static_fields: Default::default(),
            methods: Default::default(),
            partial_eq_capable: false,
            eq_hash_capable: false,
        };
        // `rust_type` holds the field's *Java simple type name* (e.g. `Map`),
        // used by the dumper to resolve a cross-class static field's receiver
        // category (`Constraints.aaMap.get(k)`).
        for (java, rust, jty, nn) in &r.fields {
            t.fields.insert(
                java.clone(),
                FieldSym { rust: rust.clone(), rust_type: jty.clone(), nullable: *nn },
            );
        }
        for (java, rust, jty, nn) in &r.static_fields {
            t.static_fields.insert(
                java.clone(),
                FieldSym { rust: rust.clone(), rust_type: jty.clone(), nullable: *nn },
            );
        }
        for (java, rust, params, throws, recv_mut, ret, ret_nullable) in &r.methods {
            t.methods.insert(
                java.clone(),
                MethodSym {
                    rust: rust.clone(),
                    params: params.clone(),
                    throws: *throws,
                    ret: ret.clone(),
                    ret_nullable: *ret_nullable,
                    // A `&mut self` project method records `refmut` so callers
                    // borrow the receiver variable mutably (see
                    // `collect_mut_borrow_params`).
                    receiver: if *recv_mut { "refmut".to_string() } else { String::new() },
                    ..Default::default()
                },
            );
        }
        map.types.insert(r.fqn.clone(), t);
    }
    break_parent_cycles(&mut map);
    compute_eq_capability(&mut map);
    map
}

/// Compute, per project struct, whether a synthesized `impl PartialEq`/`Eq+Hash`
/// can be emitted — a monotone fixpoint over the map. A type that can't
/// `#[derive]` (a subtype, via its `base` field; or a `Map`/`Set`-bearing value
/// type) can still get hand-written impls iff every field (incl. the `base`
/// chain) is comparable/hashable. The dumper hashes a *top-level* `Map`/`Set`
/// field by an order-independent fold; a nested one (`Vec<Map>`) is not hashable.
fn compute_eq_capability(map: &mut SymbolMap) {
    use std::collections::HashMap;
    // simple name -> FQNs, for resolving a field that names a project type.
    let mut by_simple: HashMap<String, Vec<String>> = HashMap::new();
    for fqn in map.types.keys() {
        let s = fqn.rsplit('.').next().unwrap_or(fqn).to_string();
        by_simple.entry(s).or_default().push(fqn.clone());
    }
    let fqns: Vec<String> = map.types.keys().cloned().collect();
    let mut pe: HashMap<String, bool> = fqns.iter().map(|f| (f.clone(), true)).collect();
    let mut eh = pe.clone();
    loop {
        let mut changed = false;
        for fqn in &fqns {
            let t = &map.types[fqn];
            let mut p = !t.generic;
            let mut e = !t.generic;
            if p {
                for fs in t.fields.values() {
                    let (fp, fe) = field_cap(&fs.rust_type, map, &by_simple, &pe, &eh, &t.generic_params);
                    p &= fp;
                    e &= fe;
                }
                if let Some(par) = &t.parent {
                    // Unknown (external) base: stub derives all — treat as capable.
                    p &= pe.get(par).copied().unwrap_or(true);
                    e &= eh.get(par).copied().unwrap_or(true);
                }
            }
            if pe[fqn] != p {
                pe.insert(fqn.clone(), p);
                changed = true;
            }
            if eh[fqn] != e {
                eh.insert(fqn.clone(), e);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for fqn in &fqns {
        let t = map.types.get_mut(fqn).unwrap();
        t.partial_eq_capable = pe[fqn];
        t.eq_hash_capable = eh[fqn];
    }
}

/// `(partial_eq, eq_hash)` capability of a field given its *Java* type string
/// (`type_java_simple`, with generics). `eq_hash` here means "the dumper can
/// hash this field": a top-level `Map`/`Set` is hashable via fold (iff elements
/// are directly hashable); everything else must be directly hashable.
fn field_cap(
    jty: &str,
    map: &SymbolMap,
    by_simple: &std::collections::HashMap<String, Vec<String>>,
    pe: &std::collections::HashMap<String, bool>,
    eh: &std::collections::HashMap<String, bool>,
    gparams: &[String],
) -> (bool, bool) {
    let s = jty.trim();
    if let Some(elem) = s.strip_suffix("[]") {
        // array -> Vec: PartialEq if elem; Hash only if elem is DIRECTLY hashable.
        let (p, _) = field_cap(elem, map, by_simple, pe, eh, gparams);
        return (p, field_hash_direct(elem, map, by_simple, eh, gparams));
    }
    if let (Some(lt), true) = (s.find('<'), s.ends_with('>')) {
        let outer = &s[..lt];
        let args = split_top_commas(&s[lt + 1..s.len() - 1]);
        let pall = args.iter().all(|a| field_cap(a, map, by_simple, pe, eh, gparams).0);
        match outer {
            "Map" | "HashMap" | "LinkedHashMap" | "TreeMap" | "SortedMap" | "NavigableMap"
            | "Hashtable" | "Set" | "HashSet" | "LinkedHashSet" | "TreeSet" | "SortedSet"
            | "NavigableSet" | "EnumSet" => {
                // top-level map/set: folded -> hashable iff elements directly hashable.
                let eall = args.iter().all(|a| field_hash_direct(a, map, by_simple, eh, gparams));
                (pall, eall)
            }
            "List" | "ArrayList" | "LinkedList" | "Vector" | "Collection" | "Queue" | "Deque"
            | "Iterable" | "Stack" | "Optional" => {
                let eall = args.iter().all(|a| field_hash_direct(a, map, by_simple, eh, gparams));
                (pall, eall)
            }
            _ => {
                // a generic project/external type — classify by its outer name.
                field_cap(outer, map, by_simple, pe, eh, gparams)
            }
        }
    } else {
        let p = field_pe_bare(s, map, by_simple, pe, gparams);
        let e = field_hash_direct(s, map, by_simple, eh, gparams);
        (p, e)
    }
}

/// Does the field's Rust type implement `Hash` *directly* (derive or synthesized
/// impl — no fold)? Maps/sets are NOT directly hashable.
fn field_hash_direct(
    jty: &str,
    map: &SymbolMap,
    by_simple: &std::collections::HashMap<String, Vec<String>>,
    eh: &std::collections::HashMap<String, bool>,
    gparams: &[String],
) -> bool {
    let s = jty.trim();
    if let Some(elem) = s.strip_suffix("[]") {
        return field_hash_direct(elem, map, by_simple, eh, gparams);
    }
    if let (Some(lt), true) = (s.find('<'), s.ends_with('>')) {
        let outer = &s[..lt];
        let args = split_top_commas(&s[lt + 1..s.len() - 1]);
        return match outer {
            "Map" | "HashMap" | "LinkedHashMap" | "TreeMap" | "SortedMap" | "NavigableMap"
            | "Hashtable" | "Set" | "HashSet" | "LinkedHashSet" | "TreeSet" | "SortedSet"
            | "NavigableSet" | "EnumSet" => false, // not directly Hash
            "List" | "ArrayList" | "LinkedList" | "Vector" | "Collection" | "Queue" | "Deque"
            | "Iterable" | "Stack" | "Optional" => {
                args.iter().all(|a| field_hash_direct(a, map, by_simple, eh, gparams))
            }
            _ => field_hash_direct(outer, map, by_simple, eh, gparams),
        };
    }
    match s {
        "int" | "long" | "short" | "byte" | "char" | "boolean" | "Integer" | "Long" | "Short"
        | "Byte" | "Character" | "Boolean" | "String" | "CharSequence" | "Object" => true,
        "float" | "double" | "Float" | "Double" => false,
        _ => {
            if gparams.iter().any(|g| g == s) {
                return false;
            }
            match resolve_simple(s, map, by_simple) {
                Some(t) => match t.kind.as_str() {
                    "trait" => false,
                    "enum" => true,
                    _ => eh.get_fqn(map, by_simple, s),
                },
                None => true, // external -> stub derives Hash
            }
        }
    }
}

fn field_pe_bare(
    s: &str,
    map: &SymbolMap,
    by_simple: &std::collections::HashMap<String, Vec<String>>,
    pe: &std::collections::HashMap<String, bool>,
    gparams: &[String],
) -> bool {
    match s {
        "int" | "long" | "short" | "byte" | "char" | "boolean" | "Integer" | "Long" | "Short"
        | "Byte" | "Character" | "Boolean" | "String" | "CharSequence" | "Object" | "float"
        | "double" | "Float" | "Double" => true,
        _ => {
            if gparams.iter().any(|g| g == s) {
                return false;
            }
            match resolve_simple(s, map, by_simple) {
                Some(t) => match t.kind.as_str() {
                    "trait" => false,
                    "enum" => true,
                    _ => pe.get_fqn(map, by_simple, s),
                },
                None => true,
            }
        }
    }
}

/// Resolve a simple type name to its `TypeSym` (conservatively: only if it maps
/// to project types; ambiguity is handled by the caller AND-ing all candidates).
fn resolve_simple<'a>(
    simple: &str,
    map: &'a SymbolMap,
    by_simple: &std::collections::HashMap<String, Vec<String>>,
) -> Option<&'a crate::symbol_map::TypeSym> {
    let fqns = by_simple.get(simple)?;
    map.types.get(fqns.first()?)
}

/// Helper trait: read a capability for a simple project-type name, AND-ing over
/// all FQN candidates (conservative — if any matching type isn't capable, the
/// field isn't, so a synthesized impl is never emitted for an uncertain case).
trait CapLookup {
    fn get_fqn(
        &self,
        map: &SymbolMap,
        by_simple: &std::collections::HashMap<String, Vec<String>>,
        simple: &str,
    ) -> bool;
}
impl CapLookup for std::collections::HashMap<String, bool> {
    fn get_fqn(
        &self,
        _map: &SymbolMap,
        by_simple: &std::collections::HashMap<String, Vec<String>>,
        simple: &str,
    ) -> bool {
        match by_simple.get(simple) {
            Some(fqns) => fqns.iter().all(|f| self.get(f).copied().unwrap_or(false)),
            None => true, // external
        }
    }
}

/// Split on top-level commas (not inside nested `<...>`).
fn split_top_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim();
    if !last.is_empty() {
        out.push(last.to_string());
    }
    out
}

/// Sever cyclic `parent` chains in the project map. `resolve_parent`'s
/// nested-type fallback (longest-shared-prefix match) can pick a candidate that
/// makes a class transitively extend itself — e.g. Bio-Formats produced a cycle
/// that sent every parent-chain walk (inherited method/field resolution) into an
/// infinite loop. A type can never legitimately extend itself, so detect each
/// cycle and drop the back-edge.
fn break_parent_cycles(map: &mut SymbolMap) {
    let fqns: Vec<String> = map.types.keys().cloned().collect();
    let mut to_clear: Vec<String> = Vec::new();
    for start in &fqns {
        let mut seen = HashSet::new();
        let mut cur = start.clone();
        loop {
            if !seen.insert(cur.clone()) {
                // Revisited `cur` ⇒ it sits on a cycle; clearing its parent
                // breaks the loop while leaving the acyclic prefix intact.
                to_clear.push(cur);
                break;
            }
            match map.types.get(&cur).and_then(|t| t.parent.clone()) {
                Some(p) if map.types.contains_key(&p) => cur = p,
                _ => break,
            }
        }
    }
    for f in to_clear {
        if let Some(t) = map.types.get_mut(&f) {
            t.parent = None;
        }
    }
}

/// Resolve an `extends` simple name to a defined FQN, via the file's imports and
/// package (returns `None` for an external/unknown parent).
fn resolve_parent(
    simple: &str,
    explicit: &[String],
    wildcard: &[String],
    package: &str,
    self_fqn: &str,
    defined: &HashSet<&String>,
) -> Option<String> {
    let suffix = format!(".{simple}");
    for imp in explicit {
        if imp.ends_with(&suffix) && defined.contains(imp) {
            return Some(imp.clone());
        }
    }
    if !package.is_empty() {
        let fqn = format!("{package}.{simple}");
        if defined.contains(&fqn) {
            return Some(fqn);
        }
    }
    for pkg in wildcard {
        let fqn = format!("{pkg}.{simple}");
        if defined.contains(&fqn) {
            return Some(fqn);
        }
    }
    if defined.contains(&simple.to_string()) {
        return Some(simple.to_string());
    }
    // Nested parent: a class `extends`/`implements` an *inner* class by simple
    // name, but that type's FQN carries the enclosing-class segment(s) (e.g.
    // `…Trimmer.IlluminaClippingSeq`), so the lookups above miss it. Fall back to
    // a defined type whose FQN ends with `.simple`, preferring the candidate
    // sharing the longest prefix with `self_fqn` (a sibling inner class of the
    // same outer). Deterministic: ties broken by FQN order.
    let mut candidates: Vec<&String> =
        defined.iter().copied().filter(|f| f.ends_with(&suffix)).collect();
    candidates.sort();
    candidates
        .into_iter()
        .max_by_key(|f| shared_prefix_len(self_fqn, f))
        .cloned()
}

/// Number of leading bytes two strings share.
fn shared_prefix_len(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

fn walk_sources(dir: &Path, prefix: &[String], raw: &mut Vec<RawType>, dispatched: &mut BTreeSet<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let mut sub = prefix.to_vec();
            sub.push(name);
            walk_sources(&path, &sub, raw, dispatched);
        } else if path.extension().map(|e| e == "java").unwrap_or(false) {
            collect_file(&path, prefix, raw, dispatched);
        }
    }
}

/// Collect simple type names used as an `instanceof`/cast target (a flat scan of
/// all nodes — every node id is `0..node_count()`).
fn collect_dispatched(arena: &Arena, dispatched: &mut BTreeSet<String>) {
    for i in 0..arena.node_count() {
        let id = NodeId(i as u32);
        let typ = match arena.kind(id) {
            Node::InstanceOfExpr { typ, .. } | Node::CastExpr { typ, .. } => *typ,
            _ => continue,
        };
        let simple = type_java_simple(arena, typ);
        let simple = simple.split(['<', '[']).next().unwrap_or(&simple).trim();
        if !simple.is_empty() {
            dispatched.insert(simple.to_string());
        }
    }
}

fn collect_file(path: &Path, prefix: &[String], raw: &mut Vec<RawType>, dispatched: &mut BTreeSet<String>) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let Some((arena, root)) = crate::parse::create_compilation_unit(&text) else { return };
    collect_dispatched(&arena, dispatched);
    let mut id = crate::id_tracker::IdTracker::new();
    crate::id_tracker::run(&arena, root, &mut id);
    crate::type_tracker::run(&arena, root, &mut id);
    let nullable = crate::nullability::analyze(&arena, root, &id);
    let package = id.package_name.clone().unwrap_or_default();
    let mut explicit = Vec::new();
    let mut wildcard = Vec::new();
    for imp in &id.imports {
        if imp.static_import {
            continue;
        }
        if imp.wildcard_import {
            wildcard.push(imp.import_string.clone());
        } else {
            explicit.push(imp.import_string.clone());
        }
    }

    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let module = camel_to_snake_case(&stem);
    let mut segs = prefix.to_vec();
    segs.push(module);
    let escaped: Vec<String> = segs.iter().map(|s| escape_rust_keyword(s.clone())).collect();
    let mod_path = format!("crate::{}", escaped.join("::"));

    collect_types(&arena, root, &package, "", &mod_path, &explicit, &wildcard, &nullable, &id, raw);
}

#[allow(clippy::too_many_arguments)]
fn collect_types(
    arena: &Arena,
    node: NodeId,
    pkg: &str,
    prefix: &str,
    mod_path: &str,
    explicit: &[String],
    wildcard: &[String],
    nullable: &std::collections::HashSet<NodeId>,
    id: &crate::id_tracker::IdTracker,
    raw: &mut Vec<RawType>,
) {
    for c in arena.children(node) {
        let (name, is_class, kind, generic, generic_params) = match arena.kind(c) {
            Node::ClassOrInterfaceDeclaration { name, is_interface, type_parameters, .. } => {
                let params: Vec<String> = type_parameters
                    .iter()
                    .filter_map(|&p| match arena.kind(p) {
                        Node::TypeParameter { name, .. } => Some(name.clone()),
                        _ => None,
                    })
                    .collect();
                (
                    Some(name.clone()),
                    !is_interface,
                    if *is_interface { "trait" } else { "struct" },
                    !type_parameters.is_empty(),
                    params,
                )
            }
            Node::EnumDeclaration { name, .. } => {
                (Some(name.clone()), false, "enum", false, Vec::new())
            }
            _ => (None, false, "", false, Vec::new()),
        };
        if let Some(name) = name {
            let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}.{name}") };
            let fqn = if pkg.is_empty() { path.clone() } else { format!("{pkg}.{path}") };
            let rust_name = name.replace('$', "_");
            let (parent_simple, interface_simples, fields, static_fields, methods) = if is_class {
                type_members(arena, c, false, nullable, id)
            } else if kind == "enum" {
                // Enum variants are recorded as `static_fields` (same `Enum::Name`
                // access), so a `switch` on the enum can qualify its case labels.
                (None, Vec::new(), Vec::new(), enum_variants(arena, c), Vec::new())
            } else {
                // An interface: its fields are implicitly `static final` constants.
                type_members(arena, c, true, nullable, id)
            };
            raw.push(RawType {
                fqn,
                rust_path: format!("{mod_path}::{rust_name}"),
                kind: kind.to_string(),
                generic,
                generic_params,
                parent_simple,
                interface_simples,
                fields,
                static_fields,
                methods,
                explicit_imports: explicit.to_vec(),
                wildcard_pkgs: wildcard.to_vec(),
                package: pkg.to_string(),
            });
            collect_types(arena, c, pkg, &path, mod_path, explicit, wildcard, nullable, id, raw);
        } else {
            collect_types(arena, c, pkg, prefix, mod_path, explicit, wildcard, nullable, id, raw);
        }
    }
}

/// Extract a class's `extends` parent simple name and its own instance
/// field/method (java, rust) names.
#[allow(clippy::type_complexity)]
fn type_members(
    arena: &Arena,
    decl: NodeId,
    is_interface: bool,
    nullable: &std::collections::HashSet<NodeId>,
    id: &crate::id_tracker::IdTracker,
) -> (
    Option<String>,
    Vec<String>,
    // (java name, rust name, java simple type, nullable) for fields / statics.
    Vec<(String, String, String, bool)>,
    Vec<(String, String, String, bool)>,
    Vec<(String, String, Vec<crate::symbol_map::ParamSym>, bool, bool, Option<String>, bool)>,
) {
    use crate::modifiers;
    let Node::ClassOrInterfaceDeclaration { extends, implements, members, .. } = arena.kind(decl)
    else {
        return (None, Vec::new(), Vec::new(), Vec::new(), Vec::new());
    };
    let simple_of = |e: NodeId| match arena.kind(e) {
        Node::ClassOrInterfaceType { name, .. } => {
            Some(name.rsplit('.').next().unwrap_or(name).to_string())
        }
        _ => None,
    };
    let parent = extends.first().and_then(|&e| simple_of(e));
    let interfaces: Vec<String> = implements.iter().filter_map(|&i| simple_of(i)).collect();
    let mut fields = Vec::new();
    let mut static_fields = Vec::new();
    // Instance methods and constructors in declaration order, as
    // (map-key, rust-base, param-sigs, throws, recv-mut), so overload mangling
    // matches `RustDumpVisitor::compute_overloads` exactly (which groups
    // constructors under the base name `new`).
    let mut method_decls: Vec<(
        String,
        String,
        Vec<crate::symbol_map::ParamSym>,
        bool,
        bool,
        Option<String>,
        bool,
    )> = Vec::new();
    // `&mut self` set for the whole class (includes self-call propagation), so a
    // method's recorded receiver matches what the dumper emits.
    let mut_methods = crate::borrow::class_mut_methods(arena, id, decl);
    for &m in members {
        match arena.kind(m) {
            Node::FieldDeclaration { modifiers, variables, typ, .. } => {
                // Interface fields are implicitly `static final` constants.
                let target = if is_interface || modifiers::is_static(*modifiers) {
                    &mut static_fields
                } else {
                    &mut fields
                };
                let jty = type_java_simple(arena, *typ);
                for &v in variables {
                    if let Node::VariableDeclarator { id: vid, .. } = arena.kind(v) {
                        if let Node::VariableDeclaratorId { name } = arena.kind(*vid) {
                            let nn = nullable.contains(vid);
                            target.push((name.clone(), rust_member_name(name), jty.clone(), nn));
                        }
                    }
                }
            }
            // Both instance and static methods are recorded: the call site picks
            // the `::`/`.` separator from the receiver, so the symbol-map lookup
            // (by name + arity) is the same for either.
            Node::MethodDeclaration { name, parameters, throws, .. } => {
                let mb = crate::borrow::analyze_method(arena, id, m);
                let params = param_syms(arena, parameters, nullable, &mb);
                method_decls.push((
                    name.clone(),
                    rust_member_name(name),
                    params,
                    !throws.is_empty(),
                    mut_methods.contains(name),
                    method_ret_type(arena, m),
                    nullable.contains(&m),
                ));
            }
            Node::ConstructorDeclaration { parameters, throws, .. } => {
                let mb = crate::borrow::analyze_method(arena, id, m);
                let params = param_syms(arena, parameters, nullable, &mb);
                // A constructor has no `self` receiver, so `recv_mut` is unused.
                method_decls.push((
                    "new".to_string(),
                    "new".to_string(),
                    params,
                    !throws.is_empty(),
                    false,
                    None,
                    false,
                ));
            }
            _ => {}
        }
    }
    let methods = mangle_overloads(&method_decls);
    (parent, interfaces, fields, static_fields, methods)
}

/// An enum's variant names as `(java, rust)` pairs (the variant is emitted with
/// its source name, so the two coincide).
fn enum_variants(arena: &Arena, decl: NodeId) -> Vec<(String, String, String, bool)> {
    let Node::EnumDeclaration { entries, .. } = arena.kind(decl) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|&e| match arena.kind(e) {
            Node::EnumConstantDeclaration { name, .. } => {
                Some((name.clone(), name.clone(), String::new(), false))
            }
            _ => None,
        })
        .collect()
}

/// The full (unqualified-but-generic) Java type string of a type node —
/// `java.util.Map<K, V>` -> `Map<K, V>`, `String[]` -> `String[]`, a primitive
/// -> its keyword. Recorded as a field's type so the resolver can recover its
/// element/value types for cross-class receivers. Empty when not determinable.
fn type_java_simple(arena: &Arena, typ: NodeId) -> String {
    match arena.kind(typ) {
        Node::ClassOrInterfaceType { name, type_args, .. } => {
            let simple = name.rsplit('.').next().unwrap_or(name);
            if type_args.is_empty() {
                simple.to_string()
            } else {
                let args: Vec<String> =
                    type_args.iter().map(|&a| type_java_simple(arena, a)).collect();
                format!("{simple}<{}>", args.join(", "))
            }
        }
        Node::ReferenceType { typ, array_count } => {
            let inner = type_java_simple(arena, *typ);
            (0..*array_count).fold(inner, |t, _| format!("{t}[]"))
        }
        Node::PrimitiveType { kind } => format!("{kind:?}").to_lowercase(),
        _ => String::new(),
    }
}

/// Compute the `(symbol-map-key, rust-name)` pairs for a type's instance
/// methods, applying the same overload mangling as
/// `RustDumpVisitor::compute_overloads`: in each same-name group the first
/// member keeps the base name, later ones are suffixed by arity (`foo`,
/// `foo_2`, …). The map key is the bare Java name for the base overload and
/// `name#arity` for the others, so a cross-file caller resolves by arity
/// (see `resolve_linked_callee`).
fn mangle_overloads(
    decls: &[(String, String, Vec<crate::symbol_map::ParamSym>, bool, bool, Option<String>, bool)],
) -> Vec<(String, String, Vec<crate::symbol_map::ParamSym>, bool, bool, Option<String>, bool)> {
    use std::collections::HashMap;
    let mut groups: HashMap<String, usize> = HashMap::new();
    // Per base name, how many overloads share each arity — an arity that recurs
    // can't disambiguate (e.g. `f(int)` vs `f(String)`), so it must NOT be keyed
    // `name#arity` (callers would resolve the wrong one); they fall back to the
    // base overload instead.
    let mut arity_counts: HashMap<(String, usize), usize> = HashMap::new();
    for (_, base, params, _, _, _, _) in decls {
        *groups.entry(base.clone()).or_insert(0) += 1;
        *arity_counts.entry((base.clone(), params.len())).or_insert(0) += 1;
    }
    let mut out = Vec::new();
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (name, base, params, throws, recv_mut, ret, ret_null) in decls {
        let arity = params.len();
        let base = base.clone();
        let overloaded = groups.get(&base).copied().unwrap_or(0) > 1;
        if !overloaded {
            out.push((name.clone(), base, params.clone(), *throws, *recv_mut, ret.clone(), *ret_null));
            continue;
        }
        let idx = *seen.entry(base.clone()).or_insert(0);
        *seen.get_mut(&base).unwrap() += 1;
        if idx == 0 {
            used.insert(base.clone());
            out.push((name.clone(), base, params.clone(), *throws, *recv_mut, ret.clone(), *ret_null));
        } else {
            let mut cand = format!("{base}_{arity}");
            let mut k = 2;
            while used.contains(&cand) {
                cand = format!("{base}_{arity}_{k}");
                k += 1;
            }
            used.insert(cand.clone());
            // Key by arity only when that arity is unique in the group; otherwise
            // key by the (unique) mangled name so it doesn't shadow a same-arity
            // sibling — same-arity calls then resolve to the base overload.
            let unique_arity = arity_counts.get(&(base.clone(), arity)).copied().unwrap_or(0) == 1;
            let key = if unique_arity { format!("{name}#{arity}") } else { cand.clone() };
            out.push((key, cand, params.clone(), *throws, *recv_mut, ret.clone(), *ret_null));
        }
    }
    out
}

/// Build `ParamSym`s for a parameter list, setting `mutable` for reference
/// parameters the borrow analysis found are mutated through (`&mut T`).
fn param_syms(
    arena: &Arena,
    parameters: &[NodeId],
    nullable: &std::collections::HashSet<NodeId>,
    mb: &crate::borrow::MethodBorrow,
) -> Vec<crate::symbol_map::ParamSym> {
    parameters
        .iter()
        .map(|&p| {
            let mut ps = param_sym(arena, p, nullable);
            if ps.by_ref {
                if let Some(name) = param_name(arena, p) {
                    if mb.mut_params.contains(&name) {
                        ps.mutable = true;
                    }
                }
            }
            ps
        })
        .collect()
}

/// The Rust primitive name of a numeric method return type (`int` -> `i32`,
/// `long` -> `i64`, …), or `None` for non-numeric/void returns. Recorded in the
/// symbol map so callers can infer the numeric type of a call result.
fn numeric_ret(arena: &Arena, decl: NodeId) -> Option<String> {
    let typ = match arena.kind(decl) {
        Node::MethodDeclaration { typ, .. } => *typ,
        _ => return None,
    };
    rust_numeric_of_type(arena, typ)
}

/// The return type recorded in the symbol map for a method: the numeric Rust
/// type (drives argument widening) or, for a class/interface return, its simple
/// name. The latter lets a call site resolve the returned type — e.g. to coerce
/// a concrete factory return into a `Box<dyn Trait>` slot (R2). Skips
/// void/primitive non-numeric returns (no useful type to record).
fn method_ret_type(arena: &Arena, decl: NodeId) -> Option<String> {
    if let Some(n) = numeric_ret(arena, decl) {
        return Some(n);
    }
    let typ = match arena.kind(decl) {
        Node::MethodDeclaration { typ, .. } => *typ,
        _ => return None,
    };
    let s = type_java_simple(arena, typ);
    if s.is_empty()
        || matches!(
            s.as_str(),
            "void" | "int" | "long" | "double" | "float" | "boolean" | "char" | "byte" | "short"
        )
    {
        None
    } else {
        Some(s)
    }
}

fn rust_numeric_of_type(arena: &Arena, t: NodeId) -> Option<String> {
    use crate::ast::PrimitiveKind::*;
    match arena.kind(t) {
        Node::PrimitiveType { kind } => Some(
            match kind {
                Int => "i32",
                Long => "i64",
                Short => "i16",
                Byte => "i8",
                Float => "f32",
                Double => "f64",
                _ => return None,
            }
            .to_string(),
        ),
        Node::ReferenceType { typ, array_count: 0 } => rust_numeric_of_type(arena, *typ),
        // A `char[]` return, recorded so the dumper's `expr_is_char_vec` routes
        // string append/concat of the result to `.iter().collect::<String>()`.
        // Inert in numeric contexts (`expr_num_type` filters to scalar numerics).
        Node::ReferenceType { typ, array_count: 1 }
            if matches!(arena.kind(*typ), Node::PrimitiveType { kind: Char }) =>
        {
            Some("Vec<char>".to_string())
        }
        _ => None,
    }
}

/// A parameter's declared (Java) name.
fn param_name(arena: &Arena, p: NodeId) -> Option<String> {
    if let Node::Parameter { id: vid, .. } = arena.kind(p) {
        if let Node::VariableDeclaratorId { name } = arena.kind(*vid) {
            return Some(name.clone());
        }
    }
    None
}

/// Best-effort `ParamSym` for a method parameter, mirroring how
/// `RustDumpVisitor::visit_parameter` renders it: a nullable param is owned
/// `Option<T>`; a non-nullable non-primitive is borrowed `&T`; a primitive is
/// owned by value. (Varargs render as an owned `Vec<T>`.)
fn param_sym(
    arena: &Arena,
    p: NodeId,
    nullable: &std::collections::HashSet<NodeId>,
) -> crate::symbol_map::ParamSym {
    use crate::symbol_map::ParamSym;
    let (typ, vid, is_var_args) = match arena.kind(p) {
        Node::Parameter { typ, id, is_var_args, .. } => (*typ, *id, *is_var_args),
        _ => return ParamSym::default(),
    };
    let is_nullable = nullable.contains(&vid);
    let is_primitive = typ
        .map(|t| matches!(arena.kind(t), Node::PrimitiveType { .. }))
        .unwrap_or(false);
    // Record the param's numeric Rust type so call sites widen a numeric arg to
    // it (`align(o: f32)` called with an `int` -> `(o) as f32`). Only numeric
    // types are recorded — `emit_numeric_arg` ignores non-numeric `rust_type`,
    // so a blank for class/String params keeps the prior behaviour.
    let rust_type = typ.and_then(|t| rust_numeric_of_type(arena, t)).unwrap_or_default();
    // The full Java type (`Double`, `Map<K, V>`) for stub-return-from-argument
    // inference (see `infer_call_ret_type`).
    let java_type = typ.map(|t| type_java_simple(arena, t)).unwrap_or_default();
    ParamSym {
        rust_type,
        java_type,
        by_ref: !is_var_args && !is_nullable && !is_primitive,
        mutable: false,
        nullable: is_nullable,
    }
}

/// Mirror of the dumper's `to_snake_if_necessary` for member names (snake unless
/// the name starts uppercase; keyword-escaped).
fn rust_member_name(n: &str) -> String {
    // A Java method literally named `clone` collides with Rust's derived
    // `Clone::clone`; rename it (call sites resolving through the map follow,
    // while synthetic `.clone()` from value-moves still hits derived `Clone`).
    if n == "clone" {
        return "clone_java".to_string();
    }
    let s = if n.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
        camel_to_snake_case(n)
    } else {
        n.to_string()
    };
    escape_rust_keyword(s)
}

// ---- post-pass: interface impls (polymorphism) ----

/// For every class implementing an interface, append `impl <Trait> for <Class>`
/// with method signatures lifted (via `syn`) from the already-translated trait
/// — so signatures match by construction. Bodies are `unimplemented!()`: the
/// `impl` exists so the type satisfies the trait and coerces to `&dyn Trait`
/// (interface polymorphism); an LLM fills in the behaviour.
pub fn generate_interface_impls(out_root: &Path, link: &crate::symbol_map::LinkIndex) {
    use std::collections::HashMap;
    use std::fmt::Write;
    let mut sig_cache: HashMap<String, Vec<(String, String, Vec<String>)>> = HashMap::new();
    // class file -> text to append
    let mut appends: HashMap<std::path::PathBuf, String> = HashMap::new();

    for (_fqn, t) in link.iter() {
        if t.kind != "struct" || t.interfaces.is_empty() {
            continue;
        }
        let Some((class_file, class_name)) = rust_path_to_file(out_root, &t.rust_path) else {
            continue;
        };
        if !class_file.exists() {
            continue;
        }
        let mut block = String::new();
        for iface in &t.interfaces {
            let Some(it) = link.lookup(iface) else { continue };
            // Only non-generic interfaces: a generic trait needs its type args in
            // both the `impl` header and `dyn` positions (a larger feature).
            if it.kind != "trait" || it.generic {
                continue;
            }
            let sigs = sig_cache.entry(iface.clone()).or_insert_with(|| {
                rust_path_to_file(out_root, &it.rust_path)
                    .map(|(f, tn)| read_trait_sigs(&f, &tn))
                    .unwrap_or_default()
            });
            if sigs.is_empty() {
                continue;
            }
            // A generic class needs its type params in both the `impl` header and
            // the self-type: `impl<E> Trait for Class<E>`.
            let gp = if t.generic_params.is_empty() {
                String::new()
            } else {
                format!("<{}>", t.generic_params.join(", "))
            };
            let _ = writeln!(block, "impl{gp} {} for {}{gp} {{", it.rust_path, class_name);
            for (sig, _name, _params) in sigs.iter() {
                // `unimplemented!()` keeps the generated impl compiling regardless
                // of inherent-signature drift; an LLM fills in real dispatch
                // (delegating bodies were measured net-negative on type errors).
                let _ = writeln!(block, "    {sig} {{ unimplemented!() }}");
            }
            let _ = writeln!(block, "}}");
        }
        if !block.is_empty() {
            appends.entry(class_file).or_default().push_str(&block);
        }
    }

    for (file, text) in appends {
        if let Ok(mut existing) = std::fs::read_to_string(&file) {
            existing.push_str("\n// ---- generated interface impls ----\n");
            existing.push_str(&text);
            let _ = std::fs::write(&file, existing);
        }
    }
}

/// Derive the on-disk `.rs` file and the type's simple name from a crate path
/// (`crate::a::b::file::Name` -> `<out>/a/b/file.rs`, `Name`). Raw-ident `r#`
/// prefixes are stripped for the filesystem path.
fn rust_path_to_file(out_root: &Path, rust_path: &str) -> Option<(std::path::PathBuf, String)> {
    let rest = rust_path.strip_prefix("crate::")?;
    let comps: Vec<&str> = rest.split("::").collect();
    if comps.len() < 2 {
        return None;
    }
    let type_name = comps.last().unwrap().trim_start_matches("r#").to_string();
    let mut file = out_root.to_path_buf();
    for c in &comps[..comps.len() - 1] {
        file.push(c.trim_start_matches("r#"));
    }
    file.set_extension("rs");
    Some((file, type_name))
}

/// Per trait method: (rendered signature, method name, parameter idents) for the
/// named trait in `file` — enough to emit a delegating `impl` method.
fn read_trait_sigs(file: &Path, trait_name: &str) -> Vec<(String, String, Vec<String>)> {
    use quote::ToTokens;
    let Ok(src) = std::fs::read_to_string(file) else { return Vec::new() };
    let Ok(parsed) = syn::parse_file(&src) else { return Vec::new() };
    for item in parsed.items {
        if let syn::Item::Trait(t) = item {
            if t.ident == trait_name {
                return t
                    .items
                    .into_iter()
                    .filter_map(|ti| match ti {
                        syn::TraitItem::Fn(f) => {
                            let name = f.sig.ident.to_string();
                            let params = f
                                .sig
                                .inputs
                                .iter()
                                .filter_map(|a| match a {
                                    syn::FnArg::Typed(pt) => match &*pt.pat {
                                        syn::Pat::Ident(pi) => Some(pi.ident.to_string()),
                                        _ => None,
                                    },
                                    syn::FnArg::Receiver(_) => None,
                                })
                                .collect();
                            Some((f.sig.to_token_stream().to_string(), name, params))
                        }
                        _ => None,
                    })
                    .collect();
            }
        }
    }
    Vec::new()
}

// ---- post-pass: module tree + Cargo.toml ----

/// The `java_runtime` module source: a `JavaIter<T>` shim mirroring
/// `java.util.Iterator`/`ListIterator` (forward + backward reads).
const JAVA_RUNTIME: &str = "\
//! java2rust runtime support.
#![allow(dead_code)]

/// A Java external iterator over a snapshot of a collection.
pub struct JavaIter<T> {
    items: Vec<T>,
    pos: usize,
}

impl<T: Clone> JavaIter<T> {
    pub fn new<I: IntoIterator<Item = T>>(src: I) -> Self {
        JavaIter { items: src.into_iter().collect(), pos: 0 }
    }
    pub fn has_next(&self) -> bool {
        self.pos < self.items.len()
    }
    pub fn next(&mut self) -> Option<T> {
        let v = self.items.get(self.pos).cloned();
        if v.is_some() {
            self.pos += 1;
        }
        v
    }
    pub fn has_previous(&self) -> bool {
        self.pos > 0
    }
    pub fn previous(&mut self) -> Option<T> {
        if self.pos > 0 {
            self.pos -= 1;
            self.items.get(self.pos).cloned()
        } else {
            None
        }
    }
}
";

/// Generate the `mod` tree (`lib.rs` at the root, `mod.rs` in each subdir) and a
/// `Cargo.toml`, turning the emitted files into one crate rooted at `out_root`.
pub fn finish_crate(out_root: &Path) -> std::io::Result<()> {
    // Runtime support: a Java-shaped external iterator (`Iterator`/`ListIterator`
    // map to this). It owns a snapshot of the collection so reads never conflict
    // with the source; `next`/`previous` return `Option<T>` so the nullable
    // machinery can unwrap in raw contexts and keep `Option` in `?:`/null ones.
    std::fs::write(out_root.join("java_runtime.rs"), JAVA_RUNTIME)?;
    merge_colliding_modules(out_root)?;
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

/// A class file `D/x.rs` whose stem collides with a sibling subpackage directory
/// `D/x/` cannot coexist as Rust modules (a single `pub mod x;` finds both `x.rs`
/// and `x/mod.rs` → E0761). They map to the *same* module path (`…::x`), so merge
/// the class file's content into the subpackage's `mod.rs` and delete the file —
/// no `rust_path` changes, all references resolve unchanged. Runs before
/// `gen_mod_file`, which then emits a single `pub mod x;` for the directory.
fn merge_colliding_modules(dir: &Path) -> std::io::Result<()> {
    let entries: Vec<_> =
        std::fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    let subdirs: BTreeSet<String> = entries
        .iter()
        .filter(|p| p.is_dir())
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    for path in &entries {
        if path.is_dir() {
            merge_colliding_modules(path)?;
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            if stem == "mod" || stem == "lib" {
                continue;
            }
            if subdirs.contains(&stem) {
                let content = std::fs::read_to_string(path)?;
                let mod_rs = dir.join(&stem).join("mod.rs");
                let merged = match std::fs::read_to_string(&mod_rs) {
                    Ok(existing) if !existing.trim().is_empty() => {
                        format!("{existing}\n{content}")
                    }
                    _ => content,
                };
                std::fs::write(&mod_rs, merged)?;
                std::fs::remove_file(path)?;
            }
        }
    }
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
    let out = dir.join(file);
    // A dependency package's `mod.rs` already holds its generated types; keep that
    // content and append the submodule declarations to it.
    if let Ok(existing) = std::fs::read_to_string(&out) {
        if !existing.is_empty() {
            body.push('\n');
            body.push_str(&existing);
        }
    }
    std::fs::write(out, body)?;
    Ok(())
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Emit linked **dependency** types (recovered from jars: `rust_path` not
/// crate-/std-relative) as crate modules, so references made `crate::`-relative
/// by the translator resolve. Each type becomes a unit `struct` whose methods
/// (from the jar signatures) take **generic** parameters — any argument is
/// accepted, sidestepping argument-type and overload-arity mismatches — and
/// return the concrete sibling-dependency type when known (so builder chains
/// like `obj.put(..).put(..)` type-check), else `()`.
pub fn generate_dep_modules(out_root: &Path, link: &crate::symbol_map::LinkIndex) {
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::fmt::Write;

    // simple name -> rust_path, for every dependency type (used to qualify
    // sibling return types).
    let mut dep_simple: HashMap<String, String> = HashMap::new();
    for (_fqn, t) in link.iter() {
        if is_dep_path(&t.rust_path) {
            if let Some(name) = t.rust_path.rsplit("::").next() {
                dep_simple.entry(name.to_string()).or_insert_with(|| t.rust_path.clone());
            }
        }
    }
    if dep_simple.is_empty() {
        return;
    }

    // module path ("org::json") -> rendered file body.
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for (_fqn, t) in link.iter() {
        if !is_dep_path(&t.rust_path) {
            continue;
        }
        let segs: Vec<&str> = t.rust_path.split("::").collect();
        if segs.len() < 2 {
            continue;
        }
        // Sanitize the type name (synthetic/nested names carry `$`, keywords need
        // raw-escaping) so the `struct`/`impl` matches the references emitted via
        // `crate_relativize`.
        let type_name = escape_rust_keyword(segs[segs.len() - 1].replace('$', "_"));
        let module = segs[..segs.len() - 1].join("::");
        let body = files.entry(module).or_default();

        let _ = writeln!(body, "pub struct {type_name};");
        let _ = writeln!(body, "impl {type_name} {{");
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for m in t.methods.values() {
            if !seen.insert(m.rust.clone()) {
                continue; // collapse rust-name collisions (lost overloads)
            }
            let name = escape_rust_keyword(m.rust.replace('$', "_"));
            let generics: String = if m.params.is_empty() {
                String::new()
            } else {
                let ps: Vec<String> = (0..m.params.len()).map(|i| format!("A{i}")).collect();
                format!("<{}>", ps.join(", "))
            };
            let mut args: Vec<String> = Vec::new();
            match m.receiver.as_str() {
                "ref" => args.push("&self".to_string()),
                "refmut" => args.push("&mut self".to_string()),
                "val" => args.push("self".to_string()),
                _ => {} // "none" -> associated fn
            }
            for i in 0..m.params.len() {
                args.push(format!("a{i}: A{i}"));
            }
            let ret = match dep_return_type(m.ret.as_deref(), &dep_simple) {
                Some(r) => format!(" -> {r}"),
                None => String::new(),
            };
            let _ = writeln!(
                body,
                "    pub fn {name}{generics}({}){ret} {{ unimplemented!() }}",
                args.join(", ")
            );
        }
        let _ = writeln!(body, "}}");
    }

    for (module, body) in files {
        // Each dependency package is a directory with its types in `mod.rs`, so a
        // package that also has sub-packages doesn't collide as both `pkg.rs` and
        // `pkg/mod.rs` (E0761). `gen_mod_file` later appends the submodule
        // declarations to this content.
        let parts: Vec<&str> = module.split("::").collect();
        let mut dir = out_root.to_path_buf();
        for p in &parts {
            dir.push(p);
        }
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let path = dir.join("mod.rs");
        if path.exists() {
            continue;
        }
        let _ = std::fs::write(&path, body);
    }
}

/// A path that is neither crate-relative nor a stdlib path — i.e. a dependency
/// type recovered from a jar (`org::json::JSONObject`).
fn is_dep_path(path: &str) -> bool {
    path.contains("::")
        && !matches!(path.split("::").next().unwrap_or(""), "crate" | "std" | "core" | "alloc")
}

/// Resolve a jar method's return type to something that exists in the crate:
/// a sibling dependency type is qualified to its `crate::` path; a plain
/// primitive/std composite is kept; anything else (JDK types, `Object`, type
/// variables) collapses to no return.
fn dep_return_type(ret: Option<&str>, dep_simple: &std::collections::HashMap<String, String>) -> Option<String> {
    let r = ret?.trim().trim_start_matches('&').trim();
    if r.is_empty() || r == "()" || r == "void" {
        return None;
    }
    // Bare sibling dependency type -> concrete crate path (enables chaining).
    if let Some(path) = dep_simple.get(r) {
        return Some(format!("crate::{}", sanitize_path_segments(path)));
    }
    // Composite/primitive made only of known-safe tokens is emitted verbatim.
    const SAFE: &[&str] = &[
        "String", "str", "bool", "char", "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32",
        "u64", "u128", "usize", "isize", "f32", "f64", "Vec", "Option", "Box", "HashMap",
        "HashSet", "BTreeMap", "BTreeSet", "std", "core", "alloc", "collections",
    ];
    let tokens: Vec<&str> = r
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty() && t.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false))
        .collect();
    if !tokens.is_empty() && tokens.iter().all(|t| SAFE.contains(t)) {
        return Some(r.to_string());
    }
    None
}

fn sanitize_crate_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    if n.is_empty() || n.chars().next().unwrap().is_ascii_digit() {
        format!("c_{n}")
    } else {
        n
    }
}
