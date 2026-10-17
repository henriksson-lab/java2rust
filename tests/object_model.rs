use std::collections::BTreeSet;

use new_java2rust::binding_id::BindingId;
use new_java2rust::effective_facts::EffectiveFacts;
use new_java2rust::facts::{
    DispatchRequirementFact, DispatchRequirementKind, FactProvenance, HierarchyOverride,
    HierarchyStrategy,
};
use new_java2rust::java_ir::{JavaType, Member, MethodDecl, Program, TypeDecl};
use new_java2rust::object_model::{
    HierarchyEdgeDiagnosticOutcome, ObjectHierarchyEdge, ObjectHierarchyEdgeDiagnosticInput,
    ObjectHierarchyEdgeKind, ObjectModelInput, ObjectRepresentation, ObjectRepresentationRequest,
    ObjectSafetyPolicy, RepresentationLookupFallback, dispatch_requirements_from_usage,
    object_hierarchy_edge_diagnostic_policy, object_hierarchy_edge_representation_policy,
    object_representation_lookup_policy, object_representation_policy_with_effective_facts,
    plan_object_model_from_input, plan_object_model_with_effective_facts,
};
use new_java2rust::parser::parse_java_source;
use new_java2rust::representation::{plan_representations, representation_dispatch_policy};
use new_java2rust::resolve::resolve_program;
use new_java2rust::shape_projection::{
    DispatchRequirementProjectionRole, dispatch_requirement_projection_policy,
};
use new_java2rust::usage::{
    BindingUse, BindingUseFacts, CallArgumentBoundary, MethodUsageFacts, UsageFacts,
};
use new_java2rust::usage::{analyze_usage, analyze_usage_with_facts};
use new_java2rust::{
    facts::{FactSet, TypeCapabilityOverride},
    slot_shape::DispatchShape,
};

fn test_binding_facts_for_id<'a>(
    usage: &'a MethodUsageFacts,
    binding: &BindingId,
) -> Option<&'a BindingUseFacts> {
    usage
        .all_binding_facts()
        .into_iter()
        .find(|facts| facts.binding() == Some(binding))
}

fn test_type_decl<'a>(program: &'a Program, name: &str) -> &'a TypeDecl {
    program
        .types
        .iter()
        .find(|ty| ty.name == name)
        .unwrap_or_else(|| panic!("missing test type {name}"))
}

fn test_method_decl_by_java_name<'a>(ty: &'a TypeDecl, name: &str) -> &'a MethodDecl {
    ty.members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == name => Some(method),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing method {name} on {}", ty.name))
}

fn test_method_usage_by_java_name<'a>(
    usage: &'a UsageFacts,
    program: &'a Program,
    type_name: &str,
    method_name: &str,
) -> &'a MethodUsageFacts {
    let ty = test_type_decl(program, type_name);
    let method = test_method_decl_by_java_name(ty, method_name);
    usage
        .source_method_usage(ty, method)
        .unwrap_or_else(|| panic!("missing usage for {type_name}.{method_name}"))
}

fn plan_object_model_for_test(
    program: &new_java2rust::java_ir::Program,
    facts: &FactSet,
) -> new_java2rust::object_model::ObjectModelPlan {
    let effective_facts = EffectiveFacts::new(facts);
    plan_object_model_with_effective_facts(program, &effective_facts)
}

#[test]
fn object_model_owns_initial_interface_reference_strategy() {
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
        named.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "Named".to_string(),
        }
    );
    assert_eq!(named.primary_provenance(), FactProvenance::ParsedSource);
    assert_eq!(named.provenance(), vec![FactProvenance::ParsedSource]);
    assert_eq!(
        factory.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "Factory".to_string()
        }
    );
    assert_eq!(
        predicate.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "PredicateLike".to_string(),
        }
    );
    assert!(predicate.diagnostics().is_empty());
    assert!(factory.diagnostics().is_empty());

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
        DispatchShape::DynTrait {
            trait_name: "Factory".to_string()
        }
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
        read_like.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "ReadLike".to_string(),
        }
    );
    assert!(read_like.diagnostics().is_empty());
    assert_eq!(
        resolved.facts.object_safe_trait_fact_for_name("ReadLike"),
        Some(true)
    );
}

#[test]
fn object_model_allows_general_interface_return_values() {
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

    assert_eq!(
        factory.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "Factory".to_string()
        }
    );
    assert!(factory.diagnostics().is_empty());
    assert_eq!(
        resolved.facts.object_safe_trait_fact_for_name("Factory"),
        Some(true)
    );
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
        filter.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "FilterLike".to_string(),
        }
    );
    assert!(filter.diagnostics().is_empty());
    assert_eq!(
        transformer.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "TransformerLike".to_string(),
        }
    );
    assert!(transformer.diagnostics().is_empty());
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
    let from_input = plan_object_model_from_input(ObjectModelInput::new(
        &resolved.program,
        &resolved.facts,
        &effective_facts,
        None,
    ));
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
    let with_usage = plan_object_model_from_input(ObjectModelInput::new(
        &resolved.program,
        &resolved.facts,
        &effective_facts,
        Some(&usage),
    ));
    let without_usage = plan_object_model_from_input(ObjectModelInput::new(
        &resolved.program,
        &resolved.facts,
        &effective_facts,
        None,
    ));

    assert_eq!(
        with_usage.strategy_for("Animal").unwrap().strategy(),
        without_usage.strategy_for("Animal").unwrap().strategy()
    );
    assert_eq!(
        with_usage.strategy_for("Animal").unwrap().requirements(),
        vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage]
    );
    assert!(
        without_usage
            .strategy_for("Animal")
            .unwrap()
            .requirements()
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
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Use")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "isInfo" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let method_usage = test_method_usage_by_java_name(&usage, &resolved.program, "Use", "isInfo");
    assert!(
        test_binding_facts_for_id(method_usage, method.params[0].binding.as_ref().unwrap())
            .expect("line binding should be tracked")
            .uses()
            .iter()
            .any(
                |usage| matches!(usage, BindingUse::InstanceOfTarget { target_ty }
                if matches!(target_ty, JavaType::Named { name, args } if name == "InfoHeaderLine" && args.is_empty()))
            ),
        "instanceof should be represented as a dispatch requirement source"
    );

    let object_plan = plan_object_model_from_input(ObjectModelInput::new(
        &resolved.program,
        &resolved.facts,
        &effective_facts,
        Some(&usage),
    ));

    assert_eq!(
        object_plan
            .strategy_for("HeaderLine")
            .unwrap()
            .requirements(),
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
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Use")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "repairAll" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let loop_binding = method
        .body
        .as_ref()
        .and_then(|body| body.statements.first())
        .and_then(|stmt| match stmt {
            new_java2rust::java_ir::Stmt::EnhancedFor { binding, .. } => binding.as_ref(),
            _ => None,
        })
        .expect("enhanced-for binding should be assigned");
    let method_usage =
        test_method_usage_by_java_name(&usage, &resolved.program, "Use", "repairAll");
    assert!(
        test_binding_facts_for_id(method_usage, loop_binding)
            .expect("enhanced-for line binding should be tracked")
            .uses()
            .iter()
            .any(
                |usage| matches!(usage, BindingUse::InstanceOfTarget { target_ty }
                if matches!(target_ty, JavaType::Named { name, args } if name == "InfoHeaderLine" && args.is_empty()))
            ),
        "enhanced-for instanceof should be represented as a dispatch requirement source"
    );

    let object_plan = plan_object_model_from_input(ObjectModelInput::new(
        &resolved.program,
        &resolved.facts,
        &effective_facts,
        Some(&usage),
    ));

    assert_eq!(
        object_plan.strategy_for("HeaderLine").unwrap().strategy(),
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

    let effective_facts = EffectiveFacts::new(&facts);
    let policy = object_representation_policy_with_effective_facts(
        ObjectRepresentationRequest::DynTraitOverride {
            type_name: "UnsafeApi".to_string(),
            trait_name: "UnsafeApi".to_string(),
        },
        &effective_facts,
    );

    assert_eq!(
        policy.object_safety(),
        ObjectSafetyPolicy::RejectedNonObjectSafe
    );
    assert_eq!(policy.representation(), &ObjectRepresentation::ErasedObject);
    assert!(
        policy
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "non_object_safe_trait_fallback")
    );
}

#[test]
fn object_model_owns_hierarchy_edge_representation_support_policy() {
    assert!(
        !object_hierarchy_edge_representation_policy(ObjectRepresentation::ConcreteStruct)
            .represented()
    );
    assert!(
        object_hierarchy_edge_representation_policy(ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        })
        .represented()
    );
    assert!(
        object_hierarchy_edge_representation_policy(ObjectRepresentation::ClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variants: vec!["Dog".to_string()],
        })
        .represented()
    );
    assert!(
        object_hierarchy_edge_representation_policy(ObjectRepresentation::ErasedObject)
            .represented()
    );
}

#[test]
fn object_model_owns_hierarchy_edge_diagnostic_policy() {
    let policy = object_hierarchy_edge_diagnostic_policy(ObjectHierarchyEdgeDiagnosticInput::new(
        "Poodle".to_string(),
        Some("Dog".to_string()),
        vec!["Pet".to_string()],
        vec![
            ObjectHierarchyEdge::new("Dog", "Poodle", ObjectHierarchyEdgeKind::Extends),
            ObjectHierarchyEdge::new("Pet", "Poodle", ObjectHierarchyEdgeKind::Implements),
        ],
        BTreeSet::from(["Pet".to_string()]),
    ));

    assert_eq!(policy.type_name(), "Poodle");
    assert_eq!(
        policy.outcomes(),
        &[HierarchyEdgeDiagnosticOutcome::UnsupportedInheritance]
    );
    assert!(policy.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "unsupported_inheritance" && diagnostic.message().contains("Poodle")
    }));

    let declared_only =
        object_hierarchy_edge_diagnostic_policy(ObjectHierarchyEdgeDiagnosticInput::new(
            "GhostDog".to_string(),
            None,
            vec!["Marker".to_string()],
            Vec::new(),
            BTreeSet::new(),
        ));

    assert_eq!(
        declared_only.outcomes(),
        &[HierarchyEdgeDiagnosticOutcome::UnsupportedInterfaceImpl {
            interfaces: vec!["Marker".to_string()],
        }]
    );
    assert!(declared_only.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "unsupported_interface_impl"
            && diagnostic.message().contains("GhostDog")
            && diagnostic.message().contains("Marker")
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

    assert_eq!(
        planned.fallback(),
        RepresentationLookupFallback::PlannedType
    );
    assert_eq!(
        planned.representation(),
        &ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        missing_named.fallback(),
        RepresentationLookupFallback::ConservativeConcrete
    );
    assert_eq!(
        missing_named.representation(),
        &ObjectRepresentation::ConcreteStruct
    );
    assert_eq!(
        unknown.fallback(),
        RepresentationLookupFallback::ConservativeErasedObject
    );
    assert_eq!(
        unknown.representation(),
        &ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        wildcard.fallback(),
        RepresentationLookupFallback::ConservativeErasedObject
    );
    assert_eq!(
        wildcard.representation(),
        &ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        primitive.fallback(),
        RepresentationLookupFallback::ConservativeConcrete
    );
    assert_eq!(
        primitive.representation(),
        &ObjectRepresentation::ConcreteStruct
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
        animal.requirements(),
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
class Use {
    Object value;
    void run(Stored stored, Borrowed borrowed, UnknownArg unknown, RejectedReceiver receiver) {
        this.value = stored;
        read(borrowed);
        external(unknown);
        receiver.external();
    }
    void read(Borrowed value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let facts = FactSet::default();
    let effective_facts = EffectiveFacts::new(&facts);
    let usage = analyze_usage(&resolved.program);

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

    let object_plan = plan_object_model_from_input(ObjectModelInput::new(
        &resolved.program,
        &facts,
        &effective_facts,
        Some(&usage),
    ));

    assert_eq!(
        object_plan.strategy_for("Stored").unwrap().requirements(),
        vec![DispatchRequirementKind::NeedsOwnedPolymorphicStorage]
    );
    let stored_dispatch = representation_dispatch_policy(
        object_plan
            .strategy_for("Stored")
            .unwrap()
            .strategy()
            .clone(),
    )
    .into_dispatch();
    assert_eq!(
        dispatch_requirement_projection_policy(
            DispatchRequirementProjectionRole::Storage,
            stored_dispatch,
            DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
        )
        .into_projected_dispatch(),
        Some(DispatchShape::BoxedDynTrait {
            trait_name: "Stored".to_string(),
        })
    );
    assert_eq!(
        object_plan.strategy_for("Borrowed").unwrap().requirements(),
        vec![DispatchRequirementKind::NeedsBorrowedTraitView]
    );
    let borrowed_dispatch = representation_dispatch_policy(
        object_plan
            .strategy_for("Borrowed")
            .unwrap()
            .strategy()
            .clone(),
    )
    .into_dispatch();
    assert_eq!(
        dispatch_requirement_projection_policy(
            DispatchRequirementProjectionRole::Param,
            borrowed_dispatch,
            DispatchRequirementKind::NeedsBorrowedTraitView,
        )
        .into_projected_dispatch(),
        Some(DispatchShape::DynTrait {
            trait_name: "Borrowed".to_string(),
        })
    );
    assert_eq!(
        object_plan.strategy_for("UnknownArg").unwrap().strategy(),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        object_plan
            .strategy_for("RejectedReceiver")
            .unwrap()
            .strategy(),
        ObjectRepresentation::ErasedObject
    );
}

#[test]
fn scoped_type_parameter_unknown_call_argument_does_not_force_erased_runtime_object() {
    let parsed = parse_java_source(
        r#"
class ExternalSink {
}

class TypeParamUnknownBoundary<T> {
    void run(T value, ExternalSink sink) {
        sink.accept(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);

    let usage_requirements = dispatch_requirements_from_usage(&usage);
    assert!(
        usage_requirements
            .get("T")
            .is_none_or(|requirements| !requirements
                .contains(&DispatchRequirementKind::NeedsUnknownRuntimeObject)),
        "{usage_requirements:?}"
    );
}

#[test]
fn real_class_sharing_type_parameter_name_still_forces_unknown_runtime_object() {
    let parsed = parse_java_source(
        r#"
class T {
}

class ExternalSink {
}

class ConcreteUnknownBoundary {
    void run(T value, ExternalSink sink) {
        sink.accept(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);

    let usage_requirements = dispatch_requirements_from_usage(&usage);
    assert_eq!(
        usage_requirements.get("T").unwrap(),
        &vec![DispatchRequirementKind::NeedsUnknownRuntimeObject]
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
        object_plan.strategy_for("Writer").unwrap().strategy(),
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
        expr.direct_subtypes(),
        vec!["Add".to_string(), "Num".to_string()]
    );
    assert_eq!(
        expr.strategy(),
        ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        }
    );
    assert_eq!(expr.primary_provenance(), FactProvenance::CorpusAnalysis);
    assert_eq!(
        expr.provenance(),
        vec![FactProvenance::ParsedSource, FactProvenance::CorpusAnalysis,]
    );
    assert!(expr.rejected().is_empty());
    assert_eq!(
        object_plan.hierarchy_edges(),
        vec![
            ObjectHierarchyEdge::new("Expr", "Add", ObjectHierarchyEdgeKind::Implements),
            ObjectHierarchyEdge::new("Expr", "Num", ObjectHierarchyEdgeKind::Implements),
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
        header_line.strategy(),
        ObjectRepresentation::ClosedEnum {
            enum_name: "HeaderLineValue".to_string(),
            variants: vec!["FormatLine".to_string(), "InfoLine".to_string()],
        }
    );
    assert!(header_line.rejected().is_empty());
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
        read_key.direct_subtypes(),
        vec![
            "ReadKey.FragmentKey".to_string(),
            "ReadKey.PairKey".to_string()
        ]
    );
    assert_eq!(
        read_key.strategy(),
        ObjectRepresentation::ClosedEnum {
            enum_name: "ReadKeyValue".to_string(),
            variants: vec![
                "ReadKey_FragmentKey".to_string(),
                "ReadKey_PairKey".to_string()
            ],
        }
    );
    assert!(read_key.rejected().is_empty());
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
        open_base.direct_subtypes(),
        vec!["OpenBase.OpenChild".to_string()]
    );
    assert_eq!(open_base.strategy(), ObjectRepresentation::ConcreteStruct);
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
        token.strategy(),
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

    assert_eq!(animal.direct_subtypes(), vec!["Dog".to_string()]);
    assert_eq!(
        animal.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert!(animal.rejected().iter().any(|rejected| {
        rejected.strategy()
            == ObjectRepresentation::ClosedEnum {
                enum_name: "AnimalValue".to_string(),
                variants: vec!["Dog".to_string()],
            }
            && rejected
                .reason()
                .contains("requires a closed hierarchy fact")
    }));
}

#[test]
fn open_fieldless_source_class_dispatch_uses_companion_carrier() {
    let parsed = parse_java_source(
        r#"
class Animal {
    String speak() { return "generic"; }
}
class Dog extends Animal {
    String speak() { return "woof"; }
}
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

    let object_plan = plan_object_model_for_test(&resolved.program, &facts);
    let animal = object_plan.strategy_for("Animal").unwrap();

    assert_eq!(
        animal.strategy(),
        ObjectRepresentation::SourceHierarchyCarrier {
            carrier_name: "AnimalValue".to_string(),
            variants: vec!["Animal".to_string(), "Dog".to_string()],
        }
    );
    assert!(animal.rejected().iter().any(|rejected| {
        rejected.strategy() == ObjectRepresentation::ErasedObject
            && rejected
                .reason()
                .contains("open subtype-dispatch/runtime case-split")
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

    assert_eq!(expr.strategy(), ObjectRepresentation::ErasedObject);
    assert_eq!(expr.primary_provenance(), FactProvenance::UserOverride);
    assert_eq!(expr.provenance(), vec![FactProvenance::UserOverride]);
    assert_eq!(
        expr.direct_subtypes(),
        vec!["Add".to_string(), "Num".to_string()]
    );
    assert!(expr.rejected().is_empty());
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

    assert_eq!(constants.strategy(), ObjectRepresentation::ConcreteStruct);
    assert_eq!(constants.primary_provenance(), FactProvenance::ParsedSource);
    assert_eq!(constants.provenance(), vec![FactProvenance::ParsedSource]);
}

#[test]
fn parsed_interface_identity_precedes_external_facade_strategies() {
    let parsed = parse_java_source("interface Port<T> { T read(); }");
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    for strategy in [
        HierarchyStrategy::ErasedObject,
        HierarchyStrategy::ConcreteStruct,
        HierarchyStrategy::DynTrait {
            trait_name: "ExternalPort".to_string(),
        },
    ] {
        for provenance in [
            FactProvenance::ExternalLibraryPack,
            FactProvenance::BuiltinExternalModel,
        ] {
            let mut facts = resolved.facts.clone();
            facts.hierarchy_overrides.push(HierarchyOverride {
                type_name: "Port".to_string(),
                strategy: strategy.clone(),
                provenance,
            });
            let plan = plan_object_model_for_test(&resolved.program, &facts);
            assert_eq!(
                EffectiveFacts::new(&facts).object_representation_override_strategy_for(
                    &JavaType::Named {
                        name: "Port".to_string(),
                        args: vec![JavaType::String]
                    }
                ),
                None,
                "slot rendering and planning must agree on source precedence"
            );
            assert_eq!(
                plan.strategy_for("Port").unwrap().strategy(),
                ObjectRepresentation::DynTrait {
                    trait_name: "Port".to_string()
                },
                "parsed interface must retain its source identity over {provenance:?} {strategy:?}"
            );
        }
    }
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Port".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::UserOverride,
    });
    assert_eq!(
        plan_object_model_for_test(&resolved.program, &facts)
            .strategy_for("Port")
            .unwrap()
            .strategy(),
        ObjectRepresentation::ErasedObject,
        "explicit user override remains authoritative"
    );
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
        expr.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "Expr".to_string(),
        }
    );
    assert_eq!(expr.primary_provenance(), FactProvenance::ParsedSource);
    assert_eq!(expr.provenance(), vec![FactProvenance::ParsedSource]);
    assert!(expr.rejected().iter().any(|rejected| {
        rejected.strategy()
            == ObjectRepresentation::ClosedEnum {
                enum_name: "ExprValue".to_string(),
                variants: vec!["Add".to_string(), "Num".to_string()],
            }
            && rejected.reason().contains("public extensible API")
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

    let object_plan = plan_object_model_from_input(ObjectModelInput::new(
        &resolved.program,
        &resolved.facts,
        &effective_facts,
        Some(&usage),
    ));
    let localizable = object_plan.strategy_for("Localizable").unwrap();

    assert_eq!(
        localizable.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "Localizable".to_string(),
        }
    );
    assert!(localizable.rejected().is_empty());
    assert_eq!(
        localizable.requirements(),
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

    let sample_usage =
        test_method_usage_by_java_name(&usage, &resolved.program, "RandomSampler", "sample");
    assert!(
        sample_usage.call_sites().iter().any(|call| {
            call.name() == "value" && call.boundary() == CallArgumentBoundary::KnownContentRead
        }),
        "interface receiver calls should be treated as source content reads: {:?}",
        sample_usage.call_sites()
    );

    let object_plan = plan_object_model_from_input(ObjectModelInput::new(
        &resolved.program,
        &resolved.facts,
        &effective_facts,
        Some(&usage),
    ));
    let distribution = object_plan.strategy_for("Distribution").unwrap();

    assert_eq!(
        distribution.strategy(),
        ObjectRepresentation::DynTrait {
            trait_name: "Distribution".to_string(),
        }
    );
    assert!(distribution.rejected().is_empty());
    assert_eq!(
        distribution.requirements(),
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

    assert_eq!(animal.strategy(), ObjectRepresentation::ErasedObject);
    assert_eq!(
        animal.primary_provenance(),
        FactProvenance::ConservativeFallback
    );
    assert_eq!(
        animal.provenance(),
        vec![
            FactProvenance::ParsedSource,
            FactProvenance::ConservativeFallback,
        ]
    );
    assert!(animal.rejected().iter().any(|rejected| {
        rejected.strategy()
            == ObjectRepresentation::DynTrait {
                trait_name: "Animal".to_string(),
            }
            && rejected.reason().contains("RejectTraitObject")
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

    assert_eq!(animal.strategy(), ObjectRepresentation::ErasedObject);
    assert!(animal.rejected().iter().any(|rejected| {
        rejected.strategy()
            == ObjectRepresentation::DynTrait {
                trait_name: "Animal".to_string(),
            }
            && rejected.reason().contains("unknown runtime object")
    }));
    assert!(animal.rejected().iter().any(|rejected| {
        matches!(rejected.strategy(), ObjectRepresentation::ClosedEnum { .. })
            && rejected
                .reason()
                .contains("requires a closed hierarchy fact")
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
