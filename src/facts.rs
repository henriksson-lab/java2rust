use std::collections::HashSet;

use crate::fact_saturation::{FiniteSaturationPolicy, SaturationStep};
use crate::java_ir::JavaType;
use crate::slot_shape::TypeCapabilities;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactSet {
    pub notes: Vec<String>,
    pub captures: CaptureFacts,
    pub hierarchy_overrides: Vec<HierarchyOverride>,
    pub closed_hierarchy_facts: Vec<ClosedHierarchyFact>,
    pub object_safety_facts: Vec<ObjectSafetyFact>,
    pub type_capability_overrides: Vec<TypeCapabilityOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyOverride {
    pub type_name: String,
    pub strategy: HierarchyStrategy,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HierarchyStrategy {
    ConcreteStruct,
    ClosedEnum {
        enum_name: String,
        variants: Vec<String>,
    },
    DynTrait {
        trait_name: String,
    },
    BoxedDynTrait {
        trait_name: String,
    },
    ErasedObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCapabilityOverride {
    pub type_name: String,
    pub copy: Option<bool>,
    pub clone: Option<bool>,
    pub string_like: Option<bool>,
    pub sequence_like: Option<JavaType>,
    pub implements: Vec<JavaType>,
    pub extends: Option<JavaType>,
    pub final_type: Option<bool>,
    pub object_safe_trait: Option<bool>,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyEdgeFact {
    pub super_name: String,
    pub sub_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedHierarchyFact {
    pub type_name: String,
    pub closed: bool,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSafetyFact {
    pub type_name: String,
    pub object_safe: bool,
    pub violations: Vec<ObjectSafetyViolation>,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectSafetyViolation {
    ExplicitNonObjectSafeFact {
        provenance: FactProvenance,
    },
    StaticMethod {
        method_name: String,
    },
    SelfReferentialReturn {
        method_name: String,
    },
    NestedSelfReferentialReturn {
        method_name: String,
    },
    SelfReferentialParameter {
        method_name: String,
        param_name: String,
    },
    NestedSelfReferentialParameter {
        method_name: String,
        param_name: String,
    },
    FieldMember {
        field_name: String,
    },
    ConstructorMember,
    UnsupportedMember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactProvenance {
    ParsedSource,
    CorpusAnalysis,
    UserOverride,
    ConservativeFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedRecoveryCapability {
    Copy,
    ExplicitClone { java_ty: JavaType },
    ConservativeClone,
    NonCloneable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureFacts {
    pub owners: Vec<CaptureOwner>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOwner {
    pub id: NodeId,
    pub kind: CaptureOwnerKind,
    pub name: Option<String>,
    pub target_type: Option<JavaType>,
    pub origin: CaptureOrigin,
    pub captures: Vec<Capture>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureOwnerKind {
    Lambda,
    LocalClass,
    AnonymousClass,
    InnerClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOrigin {
    Method {
        type_name: String,
        method_name: String,
    },
    Constructor {
        type_name: String,
    },
    Type {
        type_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub name: String,
    pub source: CaptureSource,
    pub ty: JavaType,
    pub effectively_final: bool,
    pub mutable_after_capture: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureSource {
    Local,
    Parameter,
    ThisField,
    OuterThis,
}

impl FactSet {
    pub fn extend_from(&mut self, other: FactSet) {
        self.notes.extend(other.notes);
        self.captures.owners.extend(other.captures.owners);
        self.hierarchy_overrides.extend(other.hierarchy_overrides);
        self.closed_hierarchy_facts
            .extend(other.closed_hierarchy_facts);
        self.object_safety_facts.extend(other.object_safety_facts);
        self.type_capability_overrides
            .extend(other.type_capability_overrides);
    }

    pub fn type_capabilities_for(&self, java_ty: &JavaType) -> TypeCapabilities {
        let mut capabilities = TypeCapabilities::conservative(java_ty);
        for override_fact in self.type_capability_overrides_for(java_ty) {
            apply_type_capability_override(&mut capabilities, override_fact);
        }
        capabilities
    }

    pub fn sequence_element_type_for(&self, java_ty: &JavaType) -> Option<JavaType> {
        self.type_capabilities_for(java_ty).sequence_like
    }

    pub fn is_copy_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).copy
    }

    pub fn is_clone_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).clone
    }

    pub fn explicit_clone_capability_for(&self, java_ty: &JavaType) -> Option<bool> {
        let type_name = capability_type_name(java_ty);
        self.type_capability_overrides
            .iter()
            .rev()
            .filter(|override_fact| Some(override_fact.type_name.as_str()) == type_name)
            .filter_map(|override_fact| override_fact.clone)
            .next()
    }

    pub fn is_string_like_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).string_like
    }

    pub fn can_recover_owned_from_borrowed(&self, java_ty: &JavaType) -> bool {
        !matches!(
            self.owned_recovery_capability_for(java_ty),
            OwnedRecoveryCapability::NonCloneable
        )
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

    pub fn object_safe_trait_fact_for_name(&self, type_name: &str) -> Option<bool> {
        self.type_capability_overrides
            .iter()
            .rev()
            .filter(|override_fact| override_fact.type_name == type_name)
            .filter_map(|override_fact| override_fact.object_safe_trait)
            .next()
            .or_else(|| {
                self.object_safety_facts
                    .iter()
                    .rev()
                    .find(|fact| fact.type_name == type_name)
                    .map(|fact| fact.object_safe)
            })
    }

    pub fn object_safe_trait_fact_for(&self, java_ty: &JavaType) -> Option<bool> {
        capability_type_name(java_ty)
            .and_then(|type_name| self.object_safe_trait_fact_for_name(type_name))
    }

    pub fn object_safety_fact_for_name(&self, type_name: &str) -> Option<&ObjectSafetyFact> {
        self.object_safety_facts
            .iter()
            .rev()
            .find(|fact| fact.type_name == type_name)
    }

    pub fn object_safety_violation_for_name(
        &self,
        type_name: &str,
    ) -> Option<ObjectSafetyViolation> {
        self.object_safety_fact_for_name(type_name)
            .and_then(|fact| fact.violations.first().cloned())
            .or_else(|| {
                self.type_capability_overrides
                    .iter()
                    .rev()
                    .find(|override_fact| {
                        override_fact.type_name == type_name
                            && override_fact.object_safe_trait == Some(false)
                    })
                    .map(
                        |override_fact| ObjectSafetyViolation::ExplicitNonObjectSafeFact {
                            provenance: override_fact.provenance,
                        },
                    )
            })
    }

    pub fn has_object_representation_facts(&self, java_ty: &JavaType) -> bool {
        self.object_safe_trait_fact_for(java_ty).is_some()
            || capability_type_name(java_ty).is_some_and(|type_name| {
                self.hierarchy_overrides
                    .iter()
                    .any(|override_fact| override_fact.type_name == type_name)
                    || self.closed_hierarchy_fact_for_name(type_name).is_some()
            })
    }

    pub fn extends_type_for(&self, java_ty: &JavaType) -> Option<JavaType> {
        self.type_capabilities_for(java_ty).extends
    }

    pub fn implemented_types_for(&self, java_ty: &JavaType) -> Vec<JavaType> {
        self.type_capabilities_for(java_ty).implements
    }

    pub fn is_final_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).final_type
    }

    pub fn hierarchy_edges_for_type(&self, java_ty: &JavaType) -> Vec<HierarchyEdgeFact> {
        self.direct_hierarchy_edges_for_type(java_ty)
    }

    pub fn reachable_hierarchy_edges_for_type(&self, java_ty: &JavaType) -> Vec<HierarchyEdgeFact> {
        let Some(sub_name) = java_type_name(java_ty).map(str::to_string) else {
            return Vec::new();
        };
        let mut state = self.direct_hierarchy_edges_for_type(java_ty);
        let domain_size = self.hierarchy_type_names(&sub_name).len();
        let saturation_policy = FiniteSaturationPolicy::new(domain_size, domain_size);
        let result = saturation_policy.saturate(
            state,
            |edges| {
                let mut changed = false;
                let snapshot = edges.clone();
                for edge in snapshot {
                    let super_ty = JavaType::Named {
                        name: edge.super_name.clone(),
                        args: Vec::new(),
                    };
                    for next_edge in self.direct_hierarchy_edges_for_type(&super_ty) {
                        if next_edge.super_name == sub_name {
                            continue;
                        }
                        let reachable_edge = HierarchyEdgeFact {
                            super_name: next_edge.super_name,
                            sub_name: sub_name.clone(),
                        };
                        if !edges.contains(&reachable_edge) {
                            edges.push(reachable_edge);
                            changed = true;
                        }
                    }
                }
                SaturationStep::from(changed)
            },
            |_| (),
        );
        state = result.state;
        state.sort_by(|left, right| {
            left.super_name
                .cmp(&right.super_name)
                .then_with(|| left.sub_name.cmp(&right.sub_name))
        });
        state
    }

    fn direct_hierarchy_edges_for_type(&self, java_ty: &JavaType) -> Vec<HierarchyEdgeFact> {
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

    fn hierarchy_type_names(&self, seed_type_name: &str) -> HashSet<String> {
        let mut names = HashSet::from([seed_type_name.to_string()]);
        for override_fact in &self.type_capability_overrides {
            names.insert(override_fact.type_name.clone());
            if let Some(extends) = &override_fact.extends
                && let Some(name) = java_type_name(extends)
            {
                names.insert(name.to_string());
            }
            for implemented in &override_fact.implements {
                if let Some(name) = java_type_name(implemented) {
                    names.insert(name.to_string());
                }
            }
        }
        names
    }

    pub fn closed_hierarchy_fact_for_name(&self, type_name: &str) -> Option<bool> {
        self.closed_hierarchy_facts
            .iter()
            .rev()
            .filter(|fact| fact.type_name == type_name)
            .map(|fact| fact.closed)
            .next()
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

    pub fn type_capability_overrides_for(
        &self,
        java_ty: &JavaType,
    ) -> impl Iterator<Item = &TypeCapabilityOverride> {
        let type_name = capability_type_name(java_ty);
        self.type_capability_overrides
            .iter()
            .filter(move |override_fact| Some(override_fact.type_name.as_str()) == type_name)
    }

    pub fn diagnostics(&self) -> Vec<FactDiagnostic> {
        type_capability_override_conflicts(&self.type_capability_overrides)
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

fn type_capability_override_conflicts(overrides: &[TypeCapabilityOverride]) -> Vec<FactDiagnostic> {
    let mut diagnostics = Vec::new();
    for user_override in overrides
        .iter()
        .filter(|fact| fact.provenance == FactProvenance::UserOverride)
    {
        for source_fact in overrides.iter().filter(|fact| {
            fact.provenance == FactProvenance::ParsedSource
                && fact.type_name == user_override.type_name
        }) {
            collect_type_capability_conflicts(source_fact, user_override, &mut diagnostics);
        }
    }
    diagnostics
}

fn collect_type_capability_conflicts(
    source_fact: &TypeCapabilityOverride,
    user_override: &TypeCapabilityOverride,
    diagnostics: &mut Vec<FactDiagnostic>,
) {
    push_bool_conflict(
        "copy",
        source_fact.copy,
        user_override.copy,
        user_override,
        diagnostics,
    );
    push_bool_conflict(
        "clone",
        source_fact.clone,
        user_override.clone,
        user_override,
        diagnostics,
    );
    push_bool_conflict(
        "string_like",
        source_fact.string_like,
        user_override.string_like,
        user_override,
        diagnostics,
    );
    push_bool_conflict(
        "final_type",
        source_fact.final_type,
        user_override.final_type,
        user_override,
        diagnostics,
    );
    push_bool_conflict(
        "object_safe_trait",
        source_fact.object_safe_trait,
        user_override.object_safe_trait,
        user_override,
        diagnostics,
    );
    push_java_type_conflict(
        "sequence_like",
        source_fact.sequence_like.as_ref(),
        user_override.sequence_like.as_ref(),
        user_override,
        diagnostics,
    );
    push_java_type_conflict(
        "extends",
        source_fact.extends.as_ref(),
        user_override.extends.as_ref(),
        user_override,
        diagnostics,
    );
    if !source_fact.implements.is_empty()
        && !user_override.implements.is_empty()
        && source_fact.implements != user_override.implements
    {
        diagnostics.push(FactDiagnostic {
            code: "type_capability_override_conflict",
            message: format!(
                "type capability override for {} conflicts with source-proven implements facts",
                user_override.type_name
            ),
        });
    }
}

fn push_bool_conflict(
    field: &str,
    source_value: Option<bool>,
    override_value: Option<bool>,
    override_fact: &TypeCapabilityOverride,
    diagnostics: &mut Vec<FactDiagnostic>,
) {
    if let (Some(source_value), Some(override_value)) = (source_value, override_value)
        && source_value != override_value
    {
        diagnostics.push(FactDiagnostic {
            code: "type_capability_override_conflict",
            message: format!(
                "type capability override for {} sets {field}={override_value}, conflicting with source-proven {field}={source_value}",
                override_fact.type_name
            ),
        });
    }
}

fn push_java_type_conflict(
    field: &str,
    source_value: Option<&JavaType>,
    override_value: Option<&JavaType>,
    override_fact: &TypeCapabilityOverride,
    diagnostics: &mut Vec<FactDiagnostic>,
) {
    if let (Some(source_value), Some(override_value)) = (source_value, override_value)
        && source_value != override_value
    {
        diagnostics.push(FactDiagnostic {
            code: "type_capability_override_conflict",
            message: format!(
                "type capability override for {} sets {field}={override_value:?}, conflicting with source-proven {field}={source_value:?}",
                override_fact.type_name
            ),
        });
    }
}
