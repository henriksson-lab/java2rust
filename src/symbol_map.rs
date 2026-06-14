//! The Java→Rust symbol map: shared schema for the `gen-symbols` producer and
//! the translator's consumer (`--link`).
//!
//! The map is keyed by Java FQN and carries the *current* Rust shape of a
//! previously-translated dependency (rust_path, nullability, ownership), so a
//! new translation can link against it. It is produced from a translated crate
//! (see `bin/gen_symbols.rs`) and consumed here to resolve referenced types to
//! their real Rust paths.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SymbolMap {
    pub types: BTreeMap<String, TypeSym>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct TypeSym {
    pub rust_path: String,
    pub kind: String,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldSym>,
    #[serde(default)]
    pub methods: BTreeMap<String, MethodSym>,
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
    #[serde(default)]
    pub params: Vec<ParamSym>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct ParamSym {
    #[serde(rename = "type")]
    pub rust_type: String,
    pub by_ref: bool,
    pub mutable: bool,
    pub nullable: bool,
}

/// One or more loaded dependency maps, merged and indexed for lookup during a
/// translation.
#[derive(Default)]
pub struct LinkIndex {
    map: SymbolMap,
    /// Simple Rust/Java type name → FQNs that carry it (for the unqualified
    /// fallback when imports don't pin a name).
    by_simple: HashMap<String, Vec<String>>,
}

impl LinkIndex {
    pub fn is_empty(&self) -> bool {
        self.map.types.is_empty()
    }

    /// Merge another map in. Later entries win on FQN collision.
    pub fn merge(&mut self, other: SymbolMap) {
        for (fqn, t) in other.types {
            let simple = fqn.rsplit('.').next().unwrap_or(&fqn).to_string();
            self.by_simple.entry(simple).or_default().push(fqn.clone());
            self.map.types.insert(fqn, t);
        }
    }

    /// Load a map JSON file and merge it.
    pub fn load(&mut self, path: &std::path::Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let map: SymbolMap =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.merge(map);
        Ok(())
    }

    /// Exact FQN lookup.
    pub fn lookup(&self, fqn: &str) -> Option<&TypeSym> {
        self.map.types.get(fqn)
    }

    /// Resolve a referenced Java type to a known dependency type, given the
    /// importing file's imports and package. Tries, in order: an explicit
    /// import of `name`, the same package, each wildcard import, the name as a
    /// bare FQN, and finally a unique simple-name match.
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
        // 5. last resort: a globally-unique simple name
        if let Some(fqns) = self.by_simple.get(simple) {
            if fqns.len() == 1 {
                return self.map.types.get(&fqns[0]);
            }
        }
        None
    }
}
