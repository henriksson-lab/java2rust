//! Exact source ownership for the files emitted by a surrounding batch.
//!
//! Analysis also contains context and declaration-only summaries. Neither is
//! evidence that a Rust definition will be present in the assembled crate.
use std::collections::{BTreeMap, BTreeSet};

use crate::java_ir::{Member, Program, TypeDecl};

#[derive(Debug, Clone, Default)]
pub(crate) struct SourceEmissionPlan {
    owners: BTreeMap<String, String>,
    aliases: BTreeMap<String, BTreeSet<String>>,
}

impl SourceEmissionPlan {
    #[cfg(test)]
    pub(crate) fn empty_shared() -> &'static std::sync::Arc<Self> {
        static EMPTY: std::sync::LazyLock<std::sync::Arc<SourceEmissionPlan>> =
            std::sync::LazyLock::new(|| std::sync::Arc::new(SourceEmissionPlan::default()));
        &EMPTY
    }

    pub(crate) fn empty() -> &'static Self {
        static EMPTY: SourceEmissionPlan = SourceEmissionPlan {
            owners: BTreeMap::new(),
            aliases: BTreeMap::new(),
        };
        &EMPTY
    }

    pub(crate) fn add_program(&mut self, program: &Program) {
        for ty in &program.types {
            self.add_type(ty, program.package.as_deref(), None);
        }
    }

    fn add_type(&mut self, ty: &TypeDecl, package: Option<&str>, enclosing: Option<&str>) {
        let declared = crate::java_naming::source_type_name_in_owner(&ty.name, enclosing);
        let owner = match package.filter(|package| !package.is_empty()) {
            Some(package) if !declared.starts_with(&format!("{package}.")) => {
                format!("{package}.{declared}")
            }
            _ => declared.clone(),
        };
        let rust_name = crate::java_naming::rust_name_for_java_named_type(&declared);
        self.owners.insert(owner.clone(), rust_name.clone());
        for alias in [
            declared.clone(),
            rust_name,
            crate::java_naming::rust_name_for_java_named_type(&owner),
        ] {
            self.aliases.entry(alias).or_default().insert(owner.clone());
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.add_type(nested, package, Some(&declared));
            }
        }
    }

    pub(crate) fn rust_owner(&self, owner: &str) -> Option<&str> {
        if let Some(rust_name) = self.owners.get(owner) {
            return Some(rust_name);
        }
        let candidates = self.aliases.get(owner)?;
        if candidates.len() != 1 {
            return None;
        }
        self.owners.get(candidates.first()?).map(String::as_str)
    }

    pub(crate) fn facade_names(&self) -> BTreeSet<String> {
        self.owners
            .iter()
            .flat_map(|(owner, rust_name)| {
                [
                    rust_name.clone(),
                    crate::java_naming::rust_name_for_java_named_type(owner),
                ]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_provenance_uses_the_emission_inventory_without_leaf_name_guessing() {
        let mut plan = SourceEmissionPlan::default();
        plan.add_program(
            &crate::parser::parse_java_source("package neutral; interface Bucket<T> {}").program,
        );
        let mut env = crate::types::TypeEnv::new();
        env.set_source_emission(std::sync::Arc::new(plan.clone()));
        env.insert_static_type_alias("Bucket", "neutral.Bucket");
        let facts = crate::facts::FactSet::default();
        assert!(crate::call_plan::java_type_owner_is_parsed_source(
            "Bucket", &env, &facts
        ));
        env.insert_static_type_alias("Bucket", "foreign.Bucket");
        assert!(!crate::call_plan::java_type_owner_is_parsed_source(
            "Bucket", &env, &facts
        ));
        assert!(!crate::call_plan::java_type_owner_is_parsed_source(
            "foreign.Bucket",
            &env,
            &facts
        ));

        plan.add_program(
            &crate::parser::parse_java_source("package other; interface Bucket<T> {}").program,
        );
        let mut ambiguous = crate::types::TypeEnv::new();
        ambiguous.set_source_emission(std::sync::Arc::new(plan));
        assert!(!crate::call_plan::java_type_owner_is_parsed_source(
            "Bucket", &ambiguous, &facts
        ));
        assert!(crate::call_plan::java_type_owner_is_parsed_source(
            "neutral.Bucket",
            &ambiguous,
            &facts
        ));
    }

    #[test]
    fn emission_ownership_is_package_exact_and_uses_shared_rust_naming() {
        let mut plan = SourceEmissionPlan::default();
        plan.add_program(
            &crate::parser::parse_java_source(
                "package first; class Owner { static class Nested {} }",
            )
            .program,
        );
        assert_eq!(plan.rust_owner("first.Owner"), Some("Owner"));
        assert_eq!(plan.rust_owner("FirstOwner"), Some("Owner"));
        assert_eq!(plan.rust_owner("first.Owner.Nested"), Some("Owner_Nested"));
        assert_eq!(plan.rust_owner("other.Owner"), None);
    }
}
