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
}

impl LinkIndex {
    pub fn is_empty(&self) -> bool {
        self.map.types.is_empty()
    }

    /// Merge another map in. Later entries win on FQN collision.
    pub fn merge(&mut self, other: SymbolMap) {
        for (fqn, t) in other.types {
            self.map.types.insert(fqn, t);
        }
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
        None
    }
}
