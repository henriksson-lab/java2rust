use crate::facts::{
    DispatchRequirementKind, FactProvenance, FactSet, HierarchyEdgeFact, HierarchyOverride,
    ObjectSafetyFact, ObjectSafetyViolation, OwnedRecoveryCapability, TypeCapabilityOverride,
};
use crate::java_ir::JavaType;
use crate::slot_shape::TypeCapabilities;

#[derive(Debug, Clone, Copy)]
pub struct EffectiveFacts<'a> {
    facts: &'a FactSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveFact<T> {
    pub value: T,
    pub provenance: FactProvenance,
}

impl<'a> EffectiveFacts<'a> {
    pub fn new(facts: &'a FactSet) -> Self {
        Self { facts }
    }

    pub fn hierarchy_overrides(&self) -> &'a [HierarchyOverride] {
        &self.facts.hierarchy_overrides
    }

    pub fn dispatch_requirements_for(&self, type_name: &str) -> Vec<DispatchRequirementKind> {
        self.facts.dispatch_requirements_for(type_name)
    }

    pub fn hierarchy_edges_for_type(&self, java_ty: &JavaType) -> Vec<HierarchyEdgeFact> {
        let Some(sub_name) = java_type_name(java_ty).map(str::to_string) else {
            return Vec::new();
        };
        let mut edges = Vec::new();
        if let Some(super_name) = self
            .extends_type_for(java_ty)
            .as_ref()
            .and_then(java_type_name)
        {
            edges.push(HierarchyEdgeFact {
                super_name: super_name.to_string(),
                sub_name: sub_name.clone(),
            });
        }
        edges.extend(
            self.implemented_types_for(java_ty)
                .iter()
                .filter_map(java_type_name)
                .map(|super_name| HierarchyEdgeFact {
                    super_name: super_name.to_string(),
                    sub_name: sub_name.clone(),
                }),
        );
        edges
    }

    pub fn extends_type_for(&self, java_ty: &JavaType) -> Option<JavaType> {
        self.type_capabilities_for(java_ty).extends
    }

    pub fn implemented_types_for(&self, java_ty: &JavaType) -> Vec<JavaType> {
        self.type_capabilities_for(java_ty).implements
    }

    pub fn object_safe_trait_fact_for_name(&self, type_name: &str) -> Option<bool> {
        self.object_safe_trait_fact_for_name_with_provenance(type_name)
            .map(|fact| fact.value)
    }

    pub fn object_safe_trait_fact_for_name_with_provenance(
        &self,
        type_name: &str,
    ) -> Option<EffectiveFact<bool>> {
        let override_fact = self
            .facts
            .type_capability_overrides
            .iter()
            .filter(|override_fact| override_fact.type_name == type_name)
            .filter_map(|override_fact| {
                override_fact.object_safe_trait.map(|value| EffectiveFact {
                    value,
                    provenance: override_fact.provenance,
                })
            })
            .max_by_key(effective_fact_precedence_key);
        let source_fact = self
            .facts
            .object_safety_facts
            .iter()
            .filter(|fact| fact.type_name == type_name)
            .map(|fact| EffectiveFact {
                value: fact.object_safe,
                provenance: fact.provenance,
            })
            .max_by_key(effective_fact_precedence_key);
        effective_fact_max_by_precedence(override_fact, source_fact)
    }

    fn object_safety_fact_for_name_with_effective_precedence(
        &self,
        type_name: &str,
    ) -> Option<&'a ObjectSafetyFact> {
        self.facts
            .object_safety_facts
            .iter()
            .filter(|fact| fact.type_name == type_name)
            .max_by_key(|fact| fact_precedence_key(fact.provenance))
    }

    pub fn object_safety_violation_for_name(
        &self,
        type_name: &str,
    ) -> Option<ObjectSafetyViolation> {
        let object_safe = self.object_safe_trait_fact_for_name_with_provenance(type_name)?;
        if object_safe.value {
            return None;
        }
        self.object_safety_fact_for_name_with_effective_precedence(type_name)
            .filter(|fact| fact.provenance == object_safe.provenance)
            .and_then(|fact| fact.violations.first().cloned())
            .or(Some(ObjectSafetyViolation::ExplicitNonObjectSafeFact {
                provenance: object_safe.provenance,
            }))
    }

    pub fn object_safe_trait_fact_for(&self, java_ty: &JavaType) -> Option<bool> {
        capability_type_name(java_ty)
            .and_then(|type_name| self.object_safe_trait_fact_for_name(type_name))
    }

    pub fn object_safety_fact_for_name(&self, type_name: &str) -> Option<&'a ObjectSafetyFact> {
        self.object_safety_fact_for_name_with_effective_precedence(type_name)
    }

    pub fn closed_hierarchy_fact_for_name(&self, type_name: &str) -> Option<bool> {
        self.closed_hierarchy_fact_for_name_with_provenance(type_name)
            .map(|fact| fact.value)
    }

    pub fn closed_hierarchy_fact_for_name_with_provenance(
        &self,
        type_name: &str,
    ) -> Option<EffectiveFact<bool>> {
        self.facts
            .closed_hierarchy_facts
            .iter()
            .filter(|fact| fact.type_name == type_name)
            .map(|fact| EffectiveFact {
                value: fact.closed,
                provenance: fact.provenance,
            })
            .max_by_key(effective_fact_precedence_key)
    }

    pub fn closed_hierarchy_variants_for(
        &self,
        type_name: &str,
        direct_subtypes: &[String],
    ) -> Option<Vec<String>> {
        if self.closed_hierarchy_fact_for_name(type_name) != Some(true) {
            return None;
        }
        if direct_subtypes.is_empty() {
            return None;
        }
        let all_variants_are_final = direct_subtypes.iter().all(|subtype| {
            self.is_final_type(&JavaType::Named {
                name: subtype.clone(),
                args: Vec::new(),
            })
        });
        all_variants_are_final.then(|| direct_subtypes.to_vec())
    }

    pub fn is_final_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).final_type
    }

    pub fn has_object_representation_facts(&self, java_ty: &JavaType) -> bool {
        self.object_safe_trait_fact_for(java_ty).is_some()
            || capability_type_name(java_ty).is_some_and(|type_name| {
                self.facts
                    .hierarchy_overrides
                    .iter()
                    .any(|override_fact| override_fact.type_name == type_name)
                    || self.closed_hierarchy_fact_for_name(type_name).is_some()
            })
    }

    pub fn owned_recovery_capability_for(&self, java_ty: &JavaType) -> OwnedRecoveryCapability {
        if self.is_copy_type(java_ty) {
            return OwnedRecoveryCapability::Copy;
        }
        if self.explicit_clone_capability_for(java_ty) == Some(true) {
            return OwnedRecoveryCapability::ExplicitClone {
                java_ty: java_ty.clone(),
            };
        }
        if self.is_clone_type(java_ty) {
            return OwnedRecoveryCapability::ConservativeClone;
        }
        OwnedRecoveryCapability::NonCloneable
    }

    pub fn is_string_like_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).string_like
    }

    pub fn is_copy_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).copy
    }

    pub fn is_clone_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).clone
    }

    pub fn explicit_clone_capability_for(&self, java_ty: &JavaType) -> Option<bool> {
        self.explicit_clone_capability_for_with_provenance(java_ty)
            .map(|fact| fact.value)
    }

    pub fn explicit_clone_capability_for_with_provenance(
        &self,
        java_ty: &JavaType,
    ) -> Option<EffectiveFact<bool>> {
        let type_name = capability_type_name(java_ty);
        self.facts
            .type_capability_overrides
            .iter()
            .filter(|override_fact| Some(override_fact.type_name.as_str()) == type_name)
            .filter_map(|override_fact| {
                override_fact.clone.map(|value| EffectiveFact {
                    value,
                    provenance: override_fact.provenance,
                })
            })
            .max_by_key(effective_fact_precedence_key)
    }

    pub fn can_recover_owned_from_borrowed(&self, java_ty: &JavaType) -> bool {
        !matches!(
            self.owned_recovery_capability_for(java_ty),
            OwnedRecoveryCapability::NonCloneable
        )
    }

    pub fn sequence_element_type_for(&self, java_ty: &JavaType) -> Option<JavaType> {
        self.type_capabilities_for(java_ty).sequence_like
    }

    pub fn type_capabilities_for(&self, java_ty: &JavaType) -> TypeCapabilities {
        let mut capabilities = TypeCapabilities::conservative(java_ty);
        let mut override_facts = self
            .type_capability_overrides_for(java_ty)
            .collect::<Vec<_>>();
        override_facts.sort_by_key(|override_fact| fact_precedence_key(override_fact.provenance));
        for override_fact in override_facts {
            apply_type_capability_override(&mut capabilities, override_fact);
        }
        capabilities
    }

    pub fn type_capability_overrides_for(
        &self,
        java_ty: &JavaType,
    ) -> impl Iterator<Item = &'a TypeCapabilityOverride> {
        let type_name = capability_type_name(java_ty);
        self.facts
            .type_capability_overrides
            .iter()
            .filter(move |override_fact| Some(override_fact.type_name.as_str()) == type_name)
    }
}

fn effective_fact_max_by_precedence<T>(
    left: Option<EffectiveFact<T>>,
    right: Option<EffectiveFact<T>>,
) -> Option<EffectiveFact<T>> {
    match (left, right) {
        (Some(left), Some(right)) => {
            if fact_precedence_key(right.provenance) > fact_precedence_key(left.provenance) {
                Some(right)
            } else {
                Some(left)
            }
        }
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn effective_fact_precedence_key<T>(fact: &EffectiveFact<T>) -> u8 {
    fact_precedence_key(fact.provenance)
}

fn fact_precedence_key(provenance: FactProvenance) -> u8 {
    match provenance {
        FactProvenance::ConservativeFallback => 0,
        FactProvenance::CorpusAnalysis => 1,
        FactProvenance::BuiltinExternalModel | FactProvenance::ExternalLibraryPack => 2,
        FactProvenance::ParsedSource => 3,
        FactProvenance::UserOverride => 4,
    }
}

fn apply_type_capability_override(
    capabilities: &mut TypeCapabilities,
    override_fact: &TypeCapabilityOverride,
) {
    if let Some(copy) = override_fact.copy {
        capabilities.copy = copy;
    }
    if let Some(clone) = override_fact.clone {
        capabilities.clone = clone;
    }
    if let Some(string_like) = override_fact.string_like {
        capabilities.string_like = string_like;
    }
    if let Some(sequence_like) = &override_fact.sequence_like {
        capabilities.sequence_like = Some(sequence_like.clone());
    }
    if !override_fact.implements.is_empty() {
        capabilities.implements = override_fact.implements.clone();
    }
    if let Some(extends) = &override_fact.extends {
        capabilities.extends = Some(extends.clone());
    }
    if let Some(final_type) = override_fact.final_type {
        capabilities.final_type = final_type;
    }
    if let Some(object_safe_trait) = override_fact.object_safe_trait {
        capabilities.object_safe_trait = object_safe_trait;
    }
}

fn capability_type_name(java_ty: &JavaType) -> Option<&str> {
    java_type_name(java_ty)
}

fn java_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::String => Some("String"),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}
