//! The Java→Rust symbol map: shared schema for the `gen-symbols` producer and
//! the translator's consumer (`--link`).
//!
//! The map is keyed by Java FQN and carries the *current* Rust shape of a
//! previously-translated dependency (rust_path, nullability, ownership), so a
//! new translation can link against it. It is produced from a translated crate
//! (see `bin/gen_symbols.rs`) and consumed here to resolve referenced types to
//! their real Rust paths.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SymbolMap {
    pub types: BTreeMap<String, TypeSym>,
    /// Simple type names that appear as an `instanceof`/cast *target* anywhere in
    /// the project — i.e. hierarchies actually dispatched dynamically. R4 enum
    /// synthesis activates only for these (a storage-only hierarchy gains nothing
    /// from an enum and only risks regressions).
    #[serde(default)]
    pub dispatched: std::collections::BTreeSet<String>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct TypeSym {
    pub rust_path: String,
    pub kind: String,
    /// FQN of the `extends` superclass, if any (for inherited-member resolution).
    #[serde(default)]
    pub parent: Option<String>,
    /// FQNs of implemented interfaces (for generating `impl Trait for Type`).
    #[serde(default)]
    pub interfaces: Vec<String>,
    /// True if the type has type parameters (a generic trait can't be a plain
    /// `dyn` object without its args, so polymorphism skips it).
    #[serde(default)]
    pub generic: bool,
    /// The type's own type-parameter names (e.g. `["E"]`), for emitting correct
    /// generic arity in generated `impl` headers.
    #[serde(default)]
    pub generic_params: Vec<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldSym>,
    /// Static fields (kept separate from instance `fields`: a static is an
    /// associated `const`, resolved as `Type::NAME`, not `self.base.name`).
    #[serde(default)]
    pub static_fields: BTreeMap<String, FieldSym>,
    #[serde(default)]
    pub methods: BTreeMap<String, MethodSym>,
    /// Can a synthesized `impl PartialEq`/`Eq` be emitted for this struct (every
    /// field, incl. the `base` chain, is `==`-comparable)? Computed by a fixpoint
    /// in `crate_layout`; lets a type that can't `#[derive]` (a subtype, or a
    /// map/set-bearing value type) still work in `==`.
    #[serde(default)]
    pub partial_eq_capable: bool,
    /// Like `partial_eq_capable`, but for `Eq + Hash` (so the type can key a
    /// `HashSet`/`HashMap`): every field is hashable, where a *top-level*
    /// `Map`/`Set` field is hashed by an order-independent fold (it isn't `Hash`
    /// itself) and a nested one is not hashable.
    #[serde(default)]
    pub eq_hash_capable: bool,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct FieldSym {
    pub rust: String,
    #[serde(rename = "type")]
    pub rust_type: String,
    pub nullable: bool,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct MethodSym {
    pub rust: String,
    pub rust_path: String,
    pub receiver: String,
    pub ret: Option<String>,
    pub ret_nullable: bool,
    /// The Java method declares `throws`, so it returns `Result<_, String>` and
    /// call sites must unwrap (or `?`) to reach the underlying value.
    #[serde(default)]
    pub throws: bool,
    #[serde(default)]
    pub params: Vec<ParamSym>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct ParamSym {
    /// The param's *numeric* Rust type (`f32`/`i64`/…), or empty — drives
    /// numeric argument widening at call sites.
    #[serde(rename = "type")]
    pub rust_type: String,
    /// The param's full Java type (`Double`, `Map<K, V>`, `String[]`), or empty
    /// — used to infer a stub call's return type from the argument position it
    /// flows into. Kept separate from `rust_type` so the numeric-widening path
    /// (which expects only numeric Rust types) is unaffected.
    #[serde(default)]
    pub java_type: String,
    pub by_ref: bool,
    pub mutable: bool,
    pub nullable: bool,
}

/// One or more loaded dependency maps, merged and indexed for lookup during a
/// translation.
#[derive(Default)]
pub struct LinkIndex {
    map: SymbolMap,
    /// Lazily-built index for the step-5 nested-type fallback: simple name →
    /// the FQNs `pkg.Outer.Simple` whose enclosing prefix `pkg.Outer` is itself
    /// a known type (i.e. genuine nested types). Without this, step 5 scanned
    /// *every* map key on each resolve miss — O(map) per call, which made a
    /// large project (Bio-Formats) pathologically slow. Built once on first use.
    nested_index: std::sync::OnceLock<std::collections::HashMap<String, Vec<String>>>,
    /// The synthesized-enum routing map (`variant rust_path → (enumKind path,
    /// variant name, is_root)`), built once from the (project-global) symbol map.
    /// Lives here, not per-dumper, because it depends only on `LinkIndex` +
    /// crate-mode and was otherwise recomputed for *every file* — O(roots·types·
    /// depth) per file, the dominant translate cost on a large multi-file crate
    /// (jts). The dumper supplies the builder (it needs `crate_relativize`).
    enum_info: std::sync::OnceLock<std::collections::HashMap<String, (String, String, bool)>>,
}

impl LinkIndex {
    /// The shared, lazily-built enum-routing cache (populated by the dumper's
    /// `enum_info_map` on first access; see the field doc).
    pub fn enum_info_cache(
        &self,
    ) -> &std::sync::OnceLock<std::collections::HashMap<String, (String, String, bool)>> {
        &self.enum_info
    }

    pub fn is_empty(&self) -> bool {
        self.map.types.is_empty()
    }

    /// Merge another map in. Later entries win on FQN collision.
    pub fn merge(&mut self, other: SymbolMap) {
        for (fqn, t) in other.types {
            self.map.types.insert(fqn, t);
        }
        self.map.dispatched.extend(other.dispatched);
    }

    /// Is `simple` an `instanceof`/cast target somewhere in the project (i.e. a
    /// dynamically-dispatched type)? Gates R4 enum activation.
    pub fn is_dispatched(&self, simple: &str) -> bool {
        self.map.dispatched.contains(simple)
    }

    /// Parse a map from JSON text and merge it.
    pub fn merge_json(&mut self, json: &str) -> Result<(), String> {
        let map: SymbolMap = serde_json::from_str(json).map_err(|e| e.to_string())?;
        self.merge(map);
        Ok(())
    }

    /// Load a map JSON file and merge it.
    pub fn load(&mut self, path: &std::path::Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.merge_json(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Exact FQN lookup.
    pub fn lookup(&self, fqn: &str) -> Option<&TypeSym> {
        self.map.types.get(fqn)
    }

    /// Iterate all (FQN, type) entries.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &TypeSym)> {
        self.map.types.iter()
    }

    /// Resolve a referenced Java type to a known dependency type, given the
    /// importing file's imports and package. Tries, in order: an explicit
    /// import of `name`, the same package, each wildcard import, and the name as
    /// a bare FQN. There is deliberately no unqualified simple-name fallback: a
    /// bare name with no import must NOT silently bind to an unrelated dependency
    /// type that merely shares its simple name (e.g. project `Feature` vs a dep's
    /// `…UnsupportedZipFeatureException.Feature`).
    pub fn resolve(
        &self,
        name: &str,
        explicit_imports: &[String],
        wildcard_pkgs: &[String],
        package: Option<&str>,
    ) -> Option<&TypeSym> {
        if self.map.types.is_empty() {
            return None;
        }
        let simple = name.rsplit('.').next().unwrap_or(name);

        // 1. explicit `import a.b.Simple;`
        let suffix = format!(".{simple}");
        for imp in explicit_imports {
            if imp.ends_with(&suffix) || imp == simple {
                if let Some(t) = self.map.types.get(imp) {
                    return Some(t);
                }
            }
        }
        // 2. same package
        if let Some(pkg) = package {
            if let Some(t) = self.map.types.get(&format!("{pkg}.{simple}")) {
                return Some(t);
            }
        }
        // 3. wildcard imports `import a.b.*;`
        for pkg in wildcard_pkgs {
            if let Some(t) = self.map.types.get(&format!("{pkg}.{simple}")) {
                return Some(t);
            }
        }
        // 4. the name itself is already an FQN we know
        if let Some(t) = self.map.types.get(name) {
            return Some(t);
        }
        // 5. A *nested* type referenced by simple name from within the same
        //    package (e.g. inside `Alignments`, a bare `ProfileProfileAlignerType`
        //    means `pkg.Alignments.ProfileProfileAlignerType`). The map keys
        //    nested types under `pkg.Outer.Simple`, but steps 2/3 only try
        //    `pkg.Simple`, so the defining file falls through to a stub while
        //    other files (which qualify it) resolve it — the inconsistency that
        //    produced `expected X, found alignments::X`. Match a known type whose
        //    FQN is `pkg.Outer.Simple` where the enclosing prefix `pkg.Outer` is
        //    *itself a known type*: this distinguishes a nested class from a
        //    sub-package type and from an unrelated dependency that merely shares
        //    the simple name (the caveat above). Bail on ambiguity (two same-
        //    package nested types sharing a simple name) — safer to stub than to
        //    bind the wrong one.
        if let Some(pkg) = package {
            // The index already restricts to nested types (`pkg.Outer.Simple`
            // with `pkg.Outer` a known type); here we only filter to the
            // referencing file's package and bail on ambiguity.
            if let Some(cands) = self.nested_index().get(simple) {
                let pkg_prefix = format!("{pkg}.");
                let mut found: Option<&TypeSym> = None;
                let mut ambiguous = false;
                for fqn in cands {
                    if fqn.starts_with(&pkg_prefix) && fqn.len() - suffix.len() > pkg.len() {
                        if found.is_some() {
                            ambiguous = true;
                            break;
                        }
                        found = self.map.types.get(fqn);
                    }
                }
                if !ambiguous {
                    if let Some(t) = found {
                        return Some(t);
                    }
                }
            }
        }
        None
    }

    /// Build (once) the simple-name → nested-FQN index used by `resolve` step 5.
    /// A nested type's FQN is `pkg.Outer.Simple` where the prefix `pkg.Outer`
    /// (everything before the last segment) is itself a known type.
    fn nested_index(&self) -> &std::collections::HashMap<String, Vec<String>> {
        self.nested_index.get_or_init(|| {
            let mut idx: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();
            for fqn in self.map.types.keys() {
                if let Some(dot) = fqn.rfind('.') {
                    let outer = &fqn[..dot];
                    if self.map.types.contains_key(outer) {
                        idx.entry(fqn[dot + 1..].to_string()).or_default().push(fqn.clone());
                    }
                }
            }
            idx
        })
    }
}
