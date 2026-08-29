use std::collections::{BTreeSet, HashMap};

use new_java2rust::effective_facts::EffectiveFacts;
use new_java2rust::facts::{
    DispatchRequirementFact, DispatchRequirementKind, FactProvenance, HierarchyOverride,
    HierarchyStrategy,
};
use new_java2rust::java_ir::JavaType;
use new_java2rust::object_model::{
    HierarchyEdgeDiagnosticOutcome, ObjectHierarchyEdge, ObjectHierarchyEdgeDiagnosticInput,
    ObjectHierarchyEdgeKind, ObjectModelInput, ObjectRepresentation, ObjectRepresentationRequest,
    ObjectSafetyPolicy, RepresentationLookupFallback, dispatch_requirements_from_usage,
    object_hierarchy_edge_diagnostic_policy, object_hierarchy_edge_representation_policy,
    object_representation_lookup_policy, object_representation_policy,
    plan_object_model_from_input, plan_object_model_with_effective_facts,
};
use new_java2rust::parser::parse_java_source;
use new_java2rust::representation::{plan_representations, representation_dispatch_policy};
use new_java2rust::resolve::resolve_program;
use new_java2rust::shape_projection::{
    DispatchRequirementProjectionRole, dispatch_requirement_projection_policy,
};
use new_java2rust::usage::{
    BindingUse, BindingUseFacts, CallArgumentBoundary, MethodUsageFacts, UsageFacts, UsageMethodKey,
};
use new_java2rust::usage::{analyze_usage, analyze_usage_with_facts};
use new_java2rust::{
    facts::{FactSet, TypeCapabilityOverride},
    slot_shape::DispatchShape,
};

fn plan_object_model_for_test(
    program: &new_java2rust::java_ir::Program,
    facts: &FactSet,
) -> new_java2rust::object_model::ObjectModelPlan {
    let effective_facts = EffectiveFacts::new(facts);
    plan_object_model_with_effective_facts(program, &effective_facts)
}

#[test]
fn object_model_owns_initial_interface_strategy_and_object_safety_fallback() {
    let parsed = parse_java_source(
        r#"
interface Named {
    String name();
}

interface Factory {
    Factory next();
}

interface PredicateLike {
    PredicateLike and(PredicateLike other);
    PredicateLike or(PredicateLike other);
    PredicateLike negate();
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let object_plan = plan_object_model_for_test(&resolved.program, &resolved.facts);

    let named = object_plan.strategy_for("Named").unwrap();
    let factory = object_plan.strategy_for("Factory").unwrap();
    let predicate = object_plan.strategy_for("PredicateLike").unwrap();

    assert_eq!(
        named.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "Named".to_string(),
        }
    );
    assert_eq!(named.primary_provenance(), FactProvenance::ParsedSource);
    assert_eq!(named.provenance, vec![FactProvenance::ParsedSource]);
    assert_eq!(factory.strategy, ObjectRepresentation::ErasedObject);
    assert_eq!(
        predicate.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "PredicateLike".to_string(),
        }
    );
    assert!(predicate.diagnostics.is_empty());
    assert!(factory.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "non_object_safe_trait_fallback"
            && diagnostic.message.contains("interface Factory")
            && diagnostic
                .message
                .contains("method next returns the interface type")
    }));

    let representations = plan_representations(&resolved.program, &resolved.facts);
    assert_eq!(
        representations.dispatch_for(&JavaType::Named {
            name: "Named".to_string(),
            args: Vec::new(),
        }),
        DispatchShape::DynTrait {
            trait_name: "Named".to_string(),
        }
    );
    assert_eq!(
        representations.dispatch_for(&JavaType::Named {
            name: "Factory".to_string(),
            args: Vec::new(),
        }),
        DispatchShape::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&JavaType::Named {
            name: "PredicateLike".to_string(),
            args: Vec::new(),
        }),
        DispatchShape::DynTrait {
            trait_name: "PredicateLike".to_string(),
        }
    );
}

#[test]
fn object_model_allows_clone_like_self_returning_interface_methods() {
    let parsed = parse_java_source(
        r#"
interface ReadLike {
    String name();
    ReadLike copy();
    ReadLike deepCopy();
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let object_plan = plan_object_model_for_test(&resolved.program, &resolved.facts);
    let read_like = object_plan.strategy_for("ReadLike").unwrap();

    assert_eq!(
        read_like.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "ReadLike".to_string(),
        }
    );
    assert!(read_like.diagnostics.is_empty());
    assert_eq!(
        resolved.facts.object_safe_trait_fact_for_name("ReadLike"),
        Some(true)
    );
}

#[test]
fn object_model_keeps_general_self_returning_interface_methods_non_object_safe() {
    let parsed = parse_java_source(
        r#"
interface Factory {
    Factory next();
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let object_plan = plan_object_model_for_test(&resolved.program, &resolved.facts);
    let factory = object_plan.strategy_for("Factory").unwrap();

    assert_eq!(factory.strategy, ObjectRepresentation::ErasedObject);
    assert!(factory.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "non_object_safe_trait_fallback"
            && diagnostic
                .message
                .contains("method next returns the interface type")
    }));
}

#[test]
fn object_model_allows_combinator_self_returning_interface_methods() {
    let parsed = parse_java_source(
        r#"
interface FilterLike {
    boolean test(String value);
    FilterLike and(FilterLike other);
    FilterLike or(FilterLike other);
    FilterLike negate();
}

interface TransformerLike {
    String apply(String value);
    TransformerLike andThen(TransformerLike after);
    TransformerLike compose(TransformerLike before);
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let object_plan = plan_object_model_for_test(&resolved.program, &resolved.facts);
    let filter = object_plan.strategy_for("FilterLike").unwrap();
    let transformer = object_plan.strategy_for("TransformerLike").unwrap();

    assert_eq!(
        filter.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "FilterLike".to_string(),
        }
    );
    assert!(filter.diagnostics.is_empty());
    assert_eq!(
        transformer.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "TransformerLike".to_string(),
        }
    );
    assert!(transformer.diagnostics.is_empty());
    assert_eq!(
        resolved.facts.object_safe_trait_fact_for_name("FilterLike"),
        Some(true)
    );
    assert_eq!(
        resolved
            .facts
            .object_safe_trait_fact_for_name("TransformerLike"),
        Some(true)
    );
}

#[test]
fn object_model_input_is_the_strategy_solver_entrypoint() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
final class Dog implements Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let effective_facts = EffectiveFacts::new(&resolved.facts);
    let from_input = plan_object_model_from_input(ObjectModelInput {
        program: &resolved.program,
        facts: &resolved.facts,
        effective_facts: &effective_facts,
        usage: None,
    });
    let from_effective_api = plan_object_model_for_test(&resolved.program, &resolved.facts);

    assert_eq!(from_input, from_effective_api);
    assert!(from_input.strategy_for("Animal").is_some());
    assert!(from_input.strategy_for("Dog").is_some());
}

#[test]
fn object_model_input_usage_adds_requirements_without_rechoosing_stable_dyn_shape() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
final class Dog implements Animal {}
class Use {
    Animal id(Animal animal) { return animal; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let effective_facts = EffectiveFacts::new(&resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let with_usage = plan_object_model_from_input(ObjectModelInput {
        program: &resolved.program,
        facts: &resolved.facts,
        effective_facts: &effective_facts,
        usage: Some(&usage),
    });
    let without_usage = plan_object_model_from_input(ObjectModelInput {
        program: &resolved.program,
        facts: &resolved.facts,
        effective_facts: &effective_facts,
        usage: None,
    });

    assert_eq!(
        with_usage.strategy_for("Animal").unwrap().strategy,
        without_usage.strategy_for("Animal").unwrap().strategy
    );
    assert_eq!(
        with_usage.strategy_for("Animal").unwrap().requirements,
        vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage]
    );
    assert!(
        without_usage
            .strategy_for("Animal")
            .unwrap()
            .requirements
            .is_empty()
    );
}

#[test]
fn object_model_input_usage_records_instanceof_dispatch_requirement() {
    let parsed = parse_java_source(
        r#"
class HeaderLine {}
class InfoHeaderLine extends HeaderLine {}
class Use {
    boolean isInfo(HeaderLine line) {
        return line instanceof InfoHeaderLine;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let effective_facts = EffectiveFacts::new(&resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let method_usage = usage
        .methods
        .get(&UsageMethodKey {
            type_name: "Use".to_string(),
            method_name: "isInfo".to_string(),
        })
        .expect("usage should include Use.isInfo");
    assert!(
        method_usage
            .bindings
            .get("line")
            .expect("line binding should be tracked")
            .uses
            .iter()
            .any(
                |usage| matches!(usage, BindingUse::InstanceOfTarget { target_ty }
                if matches!(target_ty, JavaType::Named { name, args } if name == "InfoHeaderLine" && args.is_empty()))
            ),
        "instanceof should be represented as a dispatch requirement source"
    );

    let object_plan = plan_object_model_from_input(ObjectModelInput {
        program: &resolved.program,
        facts: &resolved.facts,
        effective_facts: &effective_facts,
        usage: Some(&usage),
    });

    assert_eq!(
        object_plan.strategy_for("HeaderLine").unwrap().requirements,
        vec![DispatchRequirementKind::NeedsInstanceof]
    );
}

#[test]
fn object_model_input_usage_records_enhanced_for_instanceof_dispatch_requirement() {
    let parsed = parse_java_source(
        r#"
sealed class HeaderLine permits InfoHeaderLine {}
final class InfoHeaderLine extends HeaderLine {}
class Use {
    void repairAll(java.util.Set<HeaderLine> lines) {
        for (HeaderLine line : lines) {
            if (line instanceof InfoHeaderLine) {
            }
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let effective_facts = EffectiveFacts::new(&resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let method_usage = usage
        .methods
        .get(&UsageMethodKey {
            type_name: "Use".to_string(),
            method_name: "repairAll".to_string(),
        })
        .expect("usage should include Use.repairAll");
    assert!(
        method_usage
            .bindings
            .get("line")
            .expect("enhanced-for line binding should be tracked")
            .uses
            .iter()
            .any(
                |usage| matches!(usage, BindingUse::InstanceOfTarget { target_ty }
                if matches!(target_ty, JavaType::Named { name, args } if name == "InfoHeaderLine" && args.is_empty()))
            ),
        "enhanced-for instanceof should be represented as a dispatch requirement source"
    );

    let object_plan = plan_object_model_from_input(ObjectModelInput {
        program: &resolved.program,
        facts: &resolved.facts,
        effective_facts: &effective_facts,
        usage: Some(&usage),
    });

    assert_eq!(
        object_plan.strategy_for("HeaderLine").unwrap().strategy,
        ObjectRepresentation::ClosedEnum {
            enum_name: "HeaderLineValue".to_string(),
            variants: vec!["InfoHeaderLine".to_string()],
        }
    );
}

#[test]
fn object_model_policy_keeps_effective_object_safety_decision_separate() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "UnsafeApi".to_string(),
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

    let policy = object_representation_policy(
        ObjectRepresentationRequest::DynTraitOverride {
            type_name: "UnsafeApi".to_string(),
            trait_name: "UnsafeApi".to_string(),
        },
        &facts,
    );

    assert_eq!(
        policy.object_safety,
        ObjectSafetyPolicy::RejectedNonObjectSafe
    );
    assert_eq!(policy.representation, ObjectRepresentation::ErasedObject);
    assert!(
        policy
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "non_object_safe_trait_fallback")
    );
}

#[test]
fn object_model_owns_hierarchy_edge_representation_support_policy() {
    assert!(
        !object_hierarchy_edge_representation_policy(ObjectRepresentation::ConcreteStruct)
            .represented
    );
    assert!(
        object_hierarchy_edge_representation_policy(ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        })
        .represented
    );
    assert!(
        object_hierarchy_edge_representation_policy(ObjectRepresentation::ClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variants: vec!["Dog".to_string()],
        })
        .represented
    );
    assert!(
        object_hierarchy_edge_representation_policy(ObjectRepresentation::ErasedObject).represented
    );
}

#[test]
fn object_model_owns_hierarchy_edge_diagnostic_policy() {
    let policy = object_hierarchy_edge_diagnostic_policy(ObjectHierarchyEdgeDiagnosticInput {
        type_name: "Poodle".to_string(),
        declared_extends: Some("Dog".to_string()),
        declared_implements: vec!["Pet".to_string()],
        edges: vec![
            ObjectHierarchyEdge {
                super_name: "Dog".to_string(),
                sub_name: "Poodle".to_string(),
                kind: ObjectHierarchyEdgeKind::Extends,
            },
            ObjectHierarchyEdge {
                super_name: "Pet".to_string(),
                sub_name: "Poodle".to_string(),
                kind: ObjectHierarchyEdgeKind::Implements,
            },
        ],
        represented_super_names: BTreeSet::from(["Pet".to_string()]),
    });

    assert_eq!(policy.type_name, "Poodle");
    assert_eq!(
        policy.outcomes,
        vec![HierarchyEdgeDiagnosticOutcome::UnsupportedInheritance]
    );
    assert!(policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "unsupported_inheritance" && diagnostic.message.contains("Poodle")
    }));

    let declared_only =
        object_hierarchy_edge_diagnostic_policy(ObjectHierarchyEdgeDiagnosticInput {
            type_name: "GhostDog".to_string(),
            declared_extends: None,
            declared_implements: vec!["Marker".to_string()],
            edges: Vec::new(),
            represented_super_names: BTreeSet::new(),
        });

    assert_eq!(
        declared_only.outcomes,
        vec![HierarchyEdgeDiagnosticOutcome::UnsupportedInterfaceImpl {
            interfaces: vec!["Marker".to_string()],
        }]
    );
    assert!(declared_only.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "unsupported_interface_impl"
            && diagnostic.message.contains("GhostDog")
            && diagnostic.message.contains("Marker")
    }));
}

#[test]
fn object_model_owns_unplanned_representation_lookup_fallbacks() {
    let planned = object_representation_lookup_policy(
        &JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        },
        Some(ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        }),
    );
    let missing_named = object_representation_lookup_policy(
        &JavaType::Named {
            name: "ExternalHandle".to_string(),
            args: Vec::new(),
        },
        None,
    );
    let unknown =
        object_representation_lookup_policy(&JavaType::Unknown("Object".to_string()), None);
    let wildcard = object_representation_lookup_policy(&JavaType::Wildcard(None), None);
    let primitive = object_representation_lookup_policy(
        &JavaType::Primitive(new_java2rust::java_ir::PrimitiveType::Int),
        None,
    );

    assert_eq!(planned.fallback, RepresentationLookupFallback::PlannedType);
    assert_eq!(
        planned.representation,
        ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        missing_named.fallback,
        RepresentationLookupFallback::ConservativeConcrete
    );
    assert_eq!(
        missing_named.representation,
        ObjectRepresentation::ConcreteStruct
    );
    assert_eq!(
        unknown.fallback,
        RepresentationLookupFallback::ConservativeErasedObject
    );
    assert_eq!(unknown.representation, ObjectRepresentation::ErasedObject);
    assert_eq!(
        wildcard.fallback,
        RepresentationLookupFallback::ConservativeErasedObject
    );
    assert_eq!(wildcard.representation, ObjectRepresentation::ErasedObject);
    assert_eq!(
        primitive.fallback,
        RepresentationLookupFallback::ConservativeConcrete
    );
    assert_eq!(
        primitive.representation,
        ObjectRepresentation::ConcreteStruct
    );
}

#[test]
fn object_model_consumes_finite_dispatch_requirement_facts() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "Animal".to_string(),
            requirement: DispatchRequirementKind::NeedsSubtypeDispatch,
            provenance: FactProvenance::ParsedSource,
        });
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "Animal".to_string(),
            requirement: DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
            provenance: FactProvenance::CorpusAnalysis,
        });
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "Animal".to_string(),
            requirement: DispatchRequirementKind::NeedsSubtypeDispatch,
            provenance: FactProvenance::UserOverride,
        });

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let animal = object_plan.strategy_for("Animal").unwrap();

    assert_eq!(
        facts.dispatch_requirements_for("Animal"),
        vec![
            DispatchRequirementKind::NeedsSubtypeDispatch,
            DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
        ]
    );
    assert_eq!(
        animal.requirements,
        vec![
            DispatchRequirementKind::NeedsSubtypeDispatch,
            DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
        ]
    );
    assert_eq!(facts.dispatch_requirement_facts_for("Animal").len(), 3);
}

#[test]
fn object_model_input_usage_contributes_dispatch_requirements() {
    let parsed = parse_java_source(
        r#"
interface Stored {}
interface Borrowed {}
interface UnknownArg {}
interface RejectedReceiver {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let facts = FactSet::default();
    let effective_facts = EffectiveFacts::new(&facts);
    let usage = UsageFacts {
        methods_by_rust_name: HashMap::new(),
        methods_by_java_key: HashMap::new(),
        methods: HashMap::from([(
            UsageMethodKey {
                type_name: "Use".to_string(),
                method_name: "run".to_string(),
            },
            MethodUsageFacts {
                bindings: HashMap::from([
                    (
                        "stored".to_string(),
                        BindingUseFacts {
                            name: "stored".to_string(),
                            binding: None,
                            java_ty: JavaType::Named {
                                name: "Stored".to_string(),
                                args: Vec::new(),
                            },
                            uses: vec![BindingUse::StoredInField {
                                field_name: "value".to_string(),
                            }],
                        },
                    ),
                    (
                        "borrowed".to_string(),
                        BindingUseFacts {
                            name: "borrowed".to_string(),
                            binding: None,
                            java_ty: JavaType::Named {
                                name: "Borrowed".to_string(),
                                args: Vec::new(),
                            },
                            uses: vec![BindingUse::PassedToCall {
                                name: "read".to_string(),
                                receiver_type: None,
                                arg_count: 1,
                                arg_index: 0,
                                boundary: CallArgumentBoundary::InternalSameType,
                            }],
                        },
                    ),
                    (
                        "unknown".to_string(),
                        BindingUseFacts {
                            name: "unknown".to_string(),
                            binding: None,
                            java_ty: JavaType::Named {
                                name: "UnknownArg".to_string(),
                                args: Vec::new(),
                            },
                            uses: vec![BindingUse::PassedToCall {
                                name: "external".to_string(),
                                receiver_type: None,
                                arg_count: 1,
                                arg_index: 0,
                                boundary: CallArgumentBoundary::Unknown,
                            }],
                        },
                    ),
                    (
                        "receiver".to_string(),
                        BindingUseFacts {
                            name: "receiver".to_string(),
                            binding: None,
                            java_ty: JavaType::Named {
                                name: "RejectedReceiver".to_string(),
                                args: Vec::new(),
                            },
                            uses: vec![BindingUse::ReceiverForCall {
                                name: "external".to_string(),
                                boundary: CallArgumentBoundary::Unknown,
                                has_literal_null_arg: false,
                            }],
                        },
                    ),
                ]),
                ..MethodUsageFacts::default()
            },
        )]),
        constructors: HashMap::new(),
    };

    let usage_requirements = dispatch_requirements_from_usage(&usage);
    assert_eq!(
        usage_requirements.get("Stored").unwrap(),
        &vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage]
    );
    assert_eq!(
        usage_requirements.get("Borrowed").unwrap(),
        &vec![DispatchRequirementKind::NeedsBorrowedTraitView]
    );
    assert_eq!(
        usage_requirements.get("UnknownArg").unwrap(),
        &vec![DispatchRequirementKind::NeedsUnknownRuntimeObject]
    );
    assert_eq!(
        usage_requirements.get("RejectedReceiver").unwrap(),
        &vec![DispatchRequirementKind::RejectTraitObject]
    );

    let object_plan = plan_object_model_from_input(ObjectModelInput {
        program: &resolved.program,
        facts: &facts,
        effective_facts: &effective_facts,
        usage: Some(&usage),
    });

    assert_eq!(
        object_plan.strategy_for("Stored").unwrap().requirements,
        vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage]
    );
    let stored_dispatch = representation_dispatch_policy(
        object_plan.strategy_for("Stored").unwrap().strategy.clone(),
    )
    .dispatch;
    assert_eq!(
        dispatch_requirement_projection_policy(
            DispatchRequirementProjectionRole::Storage,
            stored_dispatch,
            DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
        )
        .projected_dispatch,
        Some(DispatchShape::BoxedDynTrait {
            trait_name: "Stored".to_string(),
        })
    );
    assert_eq!(
        object_plan.strategy_for("Borrowed").unwrap().requirements,
        vec![DispatchRequirementKind::NeedsBorrowedTraitView]
    );
    let borrowed_dispatch = representation_dispatch_policy(
        object_plan
            .strategy_for("Borrowed")
            .unwrap()
            .strategy
            .clone(),
    )
    .dispatch;
    assert_eq!(
        dispatch_requirement_projection_policy(
            DispatchRequirementProjectionRole::Param,
            borrowed_dispatch,
            DispatchRequirementKind::NeedsBorrowedTraitView,
        )
        .projected_dispatch,
        Some(DispatchShape::DynTrait {
            trait_name: "Borrowed".to_string(),
        })
    );
    assert_eq!(
        object_plan.strategy_for("UnknownArg").unwrap().strategy,
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        object_plan
            .strategy_for("RejectedReceiver")
            .unwrap()
            .strategy,
        ObjectRepresentation::ErasedObject
    );
}

#[test]
fn resolved_constructor_interface_flow_keeps_dyn_trait_storage() {
    let parsed = parse_java_source(
        r#"
interface Writer {
}

class BasicWriter implements Writer {
}

class AsyncWriter implements Writer {
    Writer delegate;

    AsyncWriter(Writer writer) {
        this.delegate = writer;
    }
}

class InterfaceWriterFactory {
    Writer newWriter(boolean async) {
        Writer writer = new BasicWriter();
        if (async) {
            return new AsyncWriter(writer);
        }
        return writer;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);
    let usage_requirements = dispatch_requirements_from_usage(&usage);
    let writer_requirements = usage_requirements.get("Writer").unwrap();

    assert!(
        writer_requirements.contains(&DispatchRequirementKind::NeedsOwnedPolymorphicStorage),
        "{writer_requirements:?}"
    );
    assert!(
        !writer_requirements.contains(&DispatchRequirementKind::NeedsUnknownRuntimeObject),
        "{writer_requirements:?}"
    );
    assert!(
        !writer_requirements.contains(&DispatchRequirementKind::RejectTraitObject),
        "{writer_requirements:?}"
    );

    let object_plan = plan_object_model_for_test(&resolved.program, &resolved.facts);
    assert_eq!(
        object_plan.strategy_for("Writer").unwrap().strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "Writer".to_string(),
        }
    );
}

#[test]
fn object_model_selects_closed_enum_for_proven_closed_hierarchy() {
    let parsed = parse_java_source(
        r#"
sealed interface Expr permits Add, Num {}
final class Add implements Expr {}
final class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);

    let object_plan = plan_object_model_for_test(&resolved.program, &resolved.facts);
    let expr = object_plan.strategy_for("Expr").unwrap();

    assert_eq!(
        expr.direct_subtypes,
        vec!["Add".to_string(), "Num".to_string()]
    );
    assert_eq!(
        expr.strategy,
        ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        }
    );
    assert_eq!(expr.primary_provenance(), FactProvenance::CorpusAnalysis);
    assert_eq!(
        expr.provenance,
        vec![FactProvenance::ParsedSource, FactProvenance::CorpusAnalysis,]
    );
    assert!(expr.rejected.is_empty());
    assert_eq!(
        object_plan.hierarchy_edges,
        vec![
            ObjectHierarchyEdge {
                super_name: "Expr".to_string(),
                sub_name: "Add".to_string(),
                kind: ObjectHierarchyEdgeKind::Implements,
            },
            ObjectHierarchyEdge {
                super_name: "Expr".to_string(),
                sub_name: "Num".to_string(),
                kind: ObjectHierarchyEdgeKind::Implements,
            },
        ]
    );
}

#[test]
fn object_model_selects_closed_enum_for_proven_closed_class_runtime_split() {
    let parsed = parse_java_source(
        r#"
sealed class HeaderLine permits FormatLine, InfoLine {}
final class FormatLine extends HeaderLine {}
final class InfoLine extends HeaderLine {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "HeaderLine".to_string(),
            requirement: DispatchRequirementKind::NeedsInstanceof,
            provenance: FactProvenance::ParsedSource,
        });

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let header_line = object_plan.strategy_for("HeaderLine").unwrap();

    assert_eq!(
        header_line.strategy,
        ObjectRepresentation::ClosedEnum {
            enum_name: "HeaderLineValue".to_string(),
            variants: vec!["FormatLine".to_string(), "InfoLine".to_string()],
        }
    );
    assert!(header_line.rejected.is_empty());
}

#[test]
fn object_model_infers_closed_enum_for_abstract_nested_non_public_constructor_family() {
    let parsed = parse_java_source(
        r#"
abstract class ReadKey {
    static class FragmentKey extends ReadKey {
        FragmentKey(long key) {}
    }

    static class PairKey extends ReadKey {
        PairKey(long first, long second) {}
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);

    assert_eq!(
        resolved.facts.closed_hierarchy_fact_for_name("ReadKey"),
        Some(true)
    );
    assert!(resolved.facts.is_final_type(&JavaType::Named {
        name: "ReadKey.FragmentKey".to_string(),
        args: Vec::new(),
    }));
    assert!(resolved.facts.is_final_type(&JavaType::Named {
        name: "ReadKey.PairKey".to_string(),
        args: Vec::new(),
    }));

    let object_plan = plan_object_model_for_test(&resolved.program, &resolved.facts);
    let read_key = object_plan.strategy_for("ReadKey").unwrap();

    assert_eq!(
        read_key.direct_subtypes,
        vec![
            "ReadKey.FragmentKey".to_string(),
            "ReadKey.PairKey".to_string()
        ]
    );
    assert_eq!(
        read_key.strategy,
        ObjectRepresentation::ClosedEnum {
            enum_name: "ReadKeyValue".to_string(),
            variants: vec![
                "ReadKey_FragmentKey".to_string(),
                "ReadKey_PairKey".to_string()
            ],
        }
    );
    assert!(read_key.rejected.is_empty());
}

#[test]
fn object_model_does_not_infer_closed_enum_for_public_nested_constructor_family() {
    let parsed = parse_java_source(
        r#"
abstract class OpenBase {
    static class OpenChild extends OpenBase {
        public OpenChild() {}
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);

    assert_eq!(
        resolved.facts.closed_hierarchy_fact_for_name("OpenBase"),
        None
    );
    assert!(!resolved.facts.is_final_type(&JavaType::Named {
        name: "OpenBase.OpenChild".to_string(),
        args: Vec::new(),
    }));

    let object_plan = plan_object_model_for_test(&resolved.program, &resolved.facts);
    let open_base = object_plan.strategy_for("OpenBase").unwrap();

    assert_eq!(
        open_base.direct_subtypes,
        vec!["OpenBase.OpenChild".to_string()]
    );
    assert_eq!(open_base.strategy, ObjectRepresentation::ConcreteStruct);
}

#[test]
fn external_pack_override_does_not_replace_source_strategy_after_analysis_provenance() {
    let parsed = parse_java_source(
        r#"
sealed interface Token permits Word, NumberToken {}
final class Word implements Token {}
final class NumberToken implements Token {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Token".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let token = object_plan.strategy_for("Token").unwrap();

    assert_eq!(
        token.strategy,
        ObjectRepresentation::ClosedEnum {
            enum_name: "TokenValue".to_string(),
            variants: vec!["NumberToken".to_string(), "Word".to_string()],
        }
    );
    assert!(token.has_provenance(FactProvenance::ParsedSource));
}

#[test]
fn object_model_records_rejected_closed_enum_when_runtime_case_split_lacks_proof() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "Animal".to_string(),
            requirement: DispatchRequirementKind::NeedsInstanceof,
            provenance: FactProvenance::ParsedSource,
        });

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let animal = object_plan.strategy_for("Animal").unwrap();

    assert_eq!(animal.direct_subtypes, vec!["Dog".to_string()]);
    assert_eq!(
        animal.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert!(animal.rejected.iter().any(|rejected| {
        rejected.strategy
            == ObjectRepresentation::ClosedEnum {
                enum_name: "AnimalValue".to_string(),
                variants: vec!["Dog".to_string()],
            }
            && rejected.reason.contains("requires a closed hierarchy fact")
    }));
}

#[test]
fn object_model_applies_explicit_hierarchy_override_before_inferred_closed_enum() {
    let parsed = parse_java_source(
        r#"
sealed interface Expr permits Add, Num {}
final class Add implements Expr {}
final class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::UserOverride,
    });

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let expr = object_plan.strategy_for("Expr").unwrap();

    assert_eq!(expr.strategy, ObjectRepresentation::ErasedObject);
    assert_eq!(expr.primary_provenance(), FactProvenance::UserOverride);
    assert_eq!(expr.provenance, vec![FactProvenance::UserOverride]);
    assert_eq!(
        expr.direct_subtypes,
        vec!["Add".to_string(), "Num".to_string()]
    );
    assert!(expr.rejected.is_empty());
}

#[test]
fn object_model_keeps_parsed_source_concrete_over_external_erased_override() {
    let parsed = parse_java_source(
        r#"
class FastqConstants {
    static final String SEQUENCE_HEADER = "@";
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "FastqConstants".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let constants = object_plan.strategy_for("FastqConstants").unwrap();

    assert_eq!(constants.strategy, ObjectRepresentation::ConcreteStruct);
    assert_eq!(constants.primary_provenance(), FactProvenance::ParsedSource);
    assert_eq!(constants.provenance, vec![FactProvenance::ParsedSource]);
}

#[test]
fn object_model_rejects_inferred_closed_enum_for_public_extensible_api() {
    let parsed = parse_java_source(
        r#"
sealed interface Expr permits Add, Num {}
final class Add implements Expr {}
final class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    push_dispatch_requirement(
        &mut facts,
        "Expr",
        DispatchRequirementKind::NeedsPublicExtensibleApi,
    );

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let expr = object_plan.strategy_for("Expr").unwrap();

    assert_eq!(
        expr.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "Expr".to_string(),
        }
    );
    assert_eq!(expr.primary_provenance(), FactProvenance::ParsedSource);
    assert_eq!(expr.provenance, vec![FactProvenance::ParsedSource]);
    assert!(expr.rejected.iter().any(|rejected| {
        rejected.strategy
            == ObjectRepresentation::ClosedEnum {
                enum_name: "ExprValue".to_string(),
                variants: vec!["Add".to_string(), "Num".to_string()],
            }
            && rejected.reason.contains("public extensible API")
    }));
}

#[test]
fn object_model_keeps_source_interface_dyn_trait_for_resolved_receiver_call() {
    let parsed = parse_java_source(
        r#"
interface Localizable {
    String getSourceString();
}

class UseLocalized {
    String format(Localizable pattern) {
        return pattern.getSourceString();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let effective_facts = EffectiveFacts::new(&resolved.facts);
    let usage = analyze_usage(&resolved.program);

    let object_plan = plan_object_model_from_input(ObjectModelInput {
        program: &resolved.program,
        facts: &resolved.facts,
        effective_facts: &effective_facts,
        usage: Some(&usage),
    });
    let localizable = object_plan.strategy_for("Localizable").unwrap();

    assert_eq!(
        localizable.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "Localizable".to_string(),
        }
    );
    assert!(localizable.rejected.is_empty());
    assert_eq!(
        localizable.requirements,
        vec![DispatchRequirementKind::NeedsBorrowedTraitView]
    );
}

#[test]
fn object_model_keeps_inherited_source_interface_receiver_as_dyn_trait() {
    let parsed = parse_java_source(
        r#"
interface Distribution {
    double value();
}

abstract class AbstractDistribution implements Distribution {
}

class RandomSampler {
    double sample(Distribution distribution) {
        return distribution.value();
    }
}

class ConcreteDistribution extends AbstractDistribution {
    double value() {
        return 1.0;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let effective_facts = EffectiveFacts::new(&resolved.facts);
    let usage = analyze_usage_with_facts(&resolved.program, &resolved.facts);

    let sample_usage = usage
        .methods
        .get(&UsageMethodKey {
            type_name: "RandomSampler".to_string(),
            method_name: "sample".to_string(),
        })
        .expect("sample usage should be recorded");
    assert!(
        sample_usage.call_sites.iter().any(|call| {
            call.name == "value" && call.boundary == CallArgumentBoundary::KnownContentRead
        }),
        "interface receiver calls should be treated as source content reads: {:?}",
        sample_usage.call_sites
    );

    let object_plan = plan_object_model_from_input(ObjectModelInput {
        program: &resolved.program,
        facts: &resolved.facts,
        effective_facts: &effective_facts,
        usage: Some(&usage),
    });
    let distribution = object_plan.strategy_for("Distribution").unwrap();

    assert_eq!(
        distribution.strategy,
        ObjectRepresentation::DynTrait {
            trait_name: "Distribution".to_string(),
        }
    );
    assert!(distribution.rejected.is_empty());
    assert_eq!(
        distribution.requirements,
        vec![DispatchRequirementKind::NeedsBorrowedTraitView]
    );
}

#[test]
fn object_model_uses_erased_object_when_trait_objects_are_rejected() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    push_dispatch_requirement(
        &mut facts,
        "Animal",
        DispatchRequirementKind::RejectTraitObject,
    );

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let animal = object_plan.strategy_for("Animal").unwrap();

    assert_eq!(animal.strategy, ObjectRepresentation::ErasedObject);
    assert_eq!(
        animal.primary_provenance(),
        FactProvenance::ConservativeFallback
    );
    assert_eq!(
        animal.provenance,
        vec![
            FactProvenance::ParsedSource,
            FactProvenance::ConservativeFallback,
        ]
    );
    assert!(animal.rejected.iter().any(|rejected| {
        rejected.strategy
            == ObjectRepresentation::DynTrait {
                trait_name: "Animal".to_string(),
            }
            && rejected.reason.contains("RejectTraitObject")
    }));
}

#[test]
fn object_model_uses_erased_object_for_unknown_runtime_objects() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    push_dispatch_requirement(
        &mut facts,
        "Animal",
        DispatchRequirementKind::NeedsUnknownRuntimeObject,
    );

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let animal = object_plan.strategy_for("Animal").unwrap();

    assert_eq!(animal.strategy, ObjectRepresentation::ErasedObject);
    assert!(animal.rejected.iter().any(|rejected| {
        rejected.strategy
            == ObjectRepresentation::DynTrait {
                trait_name: "Animal".to_string(),
            }
            && rejected.reason.contains("unknown runtime object")
    }));
    assert!(animal.rejected.iter().any(|rejected| {
        matches!(rejected.strategy, ObjectRepresentation::ClosedEnum { .. })
            && rejected.reason.contains("requires a closed hierarchy fact")
    }));
}

#[test]
fn factset_extend_preserves_dispatch_requirement_facts() {
    let mut base = FactSet::default();
    let mut extra = FactSet::default();
    extra
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "Expr".to_string(),
            requirement: DispatchRequirementKind::NeedsClosedRuntimeCaseSplit,
            provenance: FactProvenance::UserOverride,
        });

    base.extend_from(extra);

    assert_eq!(
        base.dispatch_requirements_for("Expr"),
        vec![DispatchRequirementKind::NeedsClosedRuntimeCaseSplit]
    );
}

fn push_dispatch_requirement(
    facts: &mut FactSet,
    type_name: &str,
    requirement: DispatchRequirementKind,
) {
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: type_name.to_string(),
            requirement,
            provenance: FactProvenance::ParsedSource,
        });
}
