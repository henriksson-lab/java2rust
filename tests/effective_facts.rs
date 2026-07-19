use new_java2rust::effective_facts::{EffectiveFact, EffectiveFacts};
use new_java2rust::facts::{
    ClosedHierarchyFact, FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy,
    ObjectSafetyFact, ObjectSafetyViolation, OwnedRecoveryCapability, TypeCapabilityOverride,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType};

#[test]
fn effective_facts_own_object_safety_override_precedence() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Api".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: Some(false),
            provenance: FactProvenance::UserOverride,
        });
    facts.object_safety_facts.push(ObjectSafetyFact {
        type_name: "Api".to_string(),
        object_safe: true,
        violations: Vec::new(),
        provenance: FactProvenance::ParsedSource,
    });

    let effective = EffectiveFacts::new(&facts);

    assert_eq!(
        effective.object_safe_trait_fact_for_name("Api"),
        Some(false)
    );
    assert_eq!(
        effective.object_safety_violation_for_name("Api"),
        Some(ObjectSafetyViolation::ExplicitNonObjectSafeFact {
            provenance: FactProvenance::UserOverride,
        })
    );
    assert_eq!(
        effective.object_safe_trait_fact_for_name_with_provenance("Api"),
        Some(EffectiveFact {
            value: false,
            provenance: FactProvenance::UserOverride,
        })
    );
}

#[test]
fn effective_facts_own_closed_hierarchy_variant_resolution() {
    let mut facts = FactSet::default();
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "Expr".to_string(),
        closed: false,
        provenance: FactProvenance::ParsedSource,
    });
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "Expr".to_string(),
        closed: true,
        provenance: FactProvenance::UserOverride,
    });
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "Expr".to_string(),
        closed: false,
        provenance: FactProvenance::ParsedSource,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Add".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Open".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(false),
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });

    let effective = EffectiveFacts::new(&facts);

    assert!(effective.is_final_type(&JavaType::Named {
        name: "Add".to_string(),
        args: Vec::new(),
    }));
    assert_eq!(
        effective.closed_hierarchy_fact_for_name_with_provenance("Expr"),
        Some(EffectiveFact {
            value: true,
            provenance: FactProvenance::UserOverride,
        })
    );
    assert_eq!(
        effective.closed_hierarchy_variants_for("Expr", &["Add".to_string()]),
        Some(vec!["Add".to_string()])
    );
    assert_eq!(
        effective.closed_hierarchy_variants_for("Expr", &["Add".to_string(), "Open".to_string()]),
        None
    );
}

#[test]
fn effective_facts_own_hierarchy_capability_resolution() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Dog".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: vec![
                JavaType::Named {
                    name: "Pet".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Named".to_string(),
                    args: Vec::new(),
                },
            ],
            extends: Some(JavaType::Named {
                name: "Animal".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Dog".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "LowerPriorityPet".to_string(),
                args: Vec::new(),
            }],
            extends: Some(JavaType::Named {
                name: "LowerPriorityAnimal".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    let dog = JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    };

    let effective = EffectiveFacts::new(&facts);

    assert_eq!(
        effective.extends_type_for(&dog),
        Some(JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        effective.implemented_types_for(&dog),
        vec![
            JavaType::Named {
                name: "Pet".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "Named".to_string(),
                args: Vec::new(),
            },
        ]
    );
    assert_eq!(effective.hierarchy_edges_for_type(&dog).len(), 3);
}

#[test]
fn effective_facts_own_object_representation_fact_presence() {
    let mut facts = FactSet::default();
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "Expr".to_string(),
        closed: true,
        provenance: FactProvenance::ParsedSource,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Animal".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::UserOverride,
    });

    let effective = EffectiveFacts::new(&facts);

    assert!(effective.has_object_representation_facts(&JavaType::Named {
        name: "Expr".to_string(),
        args: Vec::new(),
    }));
    assert!(effective.has_object_representation_facts(&JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    }));
    assert!(
        !effective.has_object_representation_facts(&JavaType::Named {
            name: "Plain".to_string(),
            args: Vec::new(),
        })
    );
}

#[test]
fn effective_facts_own_owned_recovery_capability_resolution() {
    let mut facts = FactSet::default();
    let handle = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalHandle".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let effective = EffectiveFacts::new(&facts);

    assert_eq!(
        effective.owned_recovery_capability_for(&JavaType::Primitive(PrimitiveType::Int)),
        OwnedRecoveryCapability::Copy
    );
    assert_eq!(
        effective.owned_recovery_capability_for(&JavaType::String),
        OwnedRecoveryCapability::ExplicitClone {
            java_ty: JavaType::String,
        }
    );
    assert_eq!(
        effective.owned_recovery_capability_for(&handle),
        OwnedRecoveryCapability::NonCloneable
    );
}

#[test]
fn effective_facts_own_string_and_sequence_capability_resolution() {
    let mut facts = FactSet::default();
    let label = JavaType::Named {
        name: "ExternalLabel".to_string(),
        args: Vec::new(),
    };
    let ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalLabel".to_string(),
            copy: Some(false),
            clone: None,
            string_like: Some(true),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalLabel".to_string(),
            copy: Some(true),
            clone: None,
            string_like: Some(false),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(id.clone()),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let effective = EffectiveFacts::new(&facts);

    assert!(effective.is_string_like_type(&label));
    assert!(!effective.is_copy_type(&label));
    assert!(!effective.is_string_like_type(&ids));
    assert_eq!(effective.sequence_element_type_for(&ids), Some(id));
    assert_eq!(effective.sequence_element_type_for(&label), None);
}
