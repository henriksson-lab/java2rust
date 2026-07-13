use std::fs;
use std::path::Path;

use new_java2rust::facts::{
    ControlFlowFactKind, DispatchRequirementKind, ExceptionKind, ExternalApiEffectFact,
    ExternalApiRole, FactProvenance, NullabilityState, ReceiverMutabilityFact, RecursionKind,
    TrivialAccessorKind,
};
use new_java2rust::java_ir::JavaType;
use new_java2rust::object_model::plan_object_model;
use new_java2rust::rewrite::{
    AssignmentConditionNormalizedShape, ControlFlowRewritePlan, NullTestKind,
    TailRecursiveNormalizedShape,
};
use new_java2rust::slot_shape::OwnershipShape;
use new_java2rust::types::optional_java_type;
use new_java2rust::usage::{FieldUse, UsageMethodKey};
use new_java2rust::{
    emit, exception_plan, external_facts, lower, parser, representation, resolve,
    semantic_analysis, signature, translate_source, translate_source_with_facts,
};

fn idiom_gap(path: &str) -> String {
    fs::read_to_string(Path::new("examples/idiom-gaps").join(path)).unwrap()
}

fn java_fixture(path: &str) -> String {
    fs::read_to_string(Path::new("examples/java").join(path)).unwrap()
}

fn analyze(source: &str) -> semantic_analysis::SemanticFacts {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    )
}

#[test]
fn records_dispatch_requirements_for_instanceof_and_casts() {
    let source = r#"
interface Animal {}
class Dog implements Animal {}
class Use {
    boolean isDog(Animal animal) {
        return animal instanceof Dog;
    }
    Dog asDog(Animal animal) {
        return (Dog) animal;
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    assert_eq!(
        semantic.facts.dispatch_requirements_for("Dog"),
        vec![
            DispatchRequirementKind::NeedsInstanceof,
            DispatchRequirementKind::NeedsDowncast,
        ]
    );
    assert_eq!(
        semantic.facts.dispatch_requirement_facts_for("Dog").len(),
        2
    );

    let object_plan = plan_object_model(&resolved.program, &semantic.facts);
    let dog = object_plan.strategy_for("Dog").unwrap();
    assert_eq!(
        dog.requirements,
        vec![
            DispatchRequirementKind::NeedsInstanceof,
            DispatchRequirementKind::NeedsDowncast,
        ]
    );
}

#[test]
fn dispatch_requirement_producer_ignores_primitive_cast_targets() {
    let source = r#"
class Use {
    int asInt(long value) {
        return (int) value;
    }
}
"#;
    let semantic = analyze(source);

    assert!(semantic.facts.dispatch_requirement_facts.is_empty());
}

fn analyze_with_config(
    source: &str,
    config: semantic_analysis::SemanticAnalysisConfig,
) -> semantic_analysis::SemanticFacts {
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    semantic_analysis::analyze_program_with_config(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
        config,
    )
}

#[test]
fn semantic_analysis_shell_preserves_current_translation_output() {
    let source = idiom_gap("NonNullFieldFromConstructors.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let lowered = lower::lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &semantic.facts,
        &semantic.usage,
        &representations,
        &signatures,
    );

    assert_eq!(
        translate_source(&source).rust,
        emit::emit_file(&lowered.file)
    );
}

#[test]
fn records_non_null_field_from_all_constructors() {
    let semantic = analyze(&idiom_gap("NonNullFieldFromConstructors.java"));

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("NonNullFieldFromConstructors", "name"),
        Some(NullabilityState::DefinitelyNonNull)
    );
}

#[test]
fn field_nullability_external_override_beats_constructor_proof() {
    let source = idiom_gap("NonNullFieldFromConstructors.java");
    let parsed_external = external_facts::parse_external_facts(
        "field-nullability NonNullFieldFromConstructors.name maybe-null\n",
    );
    assert!(
        parsed_external.diagnostics.is_empty(),
        "{:?}",
        parsed_external.diagnostics
    );

    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &parsed_external.facts,
    );

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("NonNullFieldFromConstructors", "name"),
        Some(NullabilityState::MaybeNull)
    );

    let translation = translate_source_with_facts(&source, &parsed_external.facts);
    assert!(
        translation.rust.contains("name: Option<String>"),
        "{}",
        translation.rust
    );
}

#[test]
fn records_nullable_field_fallback() {
    let semantic = analyze(&java_fixture("05-types/Nullability.java"));

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("Nullability", "cached"),
        Some(NullabilityState::MaybeNull)
    );
}

#[test]
fn records_non_null_field_assignment_through_private_helper() {
    let semantic = analyze(&idiom_gap("NonNullFieldViaHelper.java"));

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("NonNullFieldViaHelper", "name"),
        Some(NullabilityState::DefinitelyNonNull)
    );
}

#[test]
fn records_non_null_field_assignment_through_constructor_delegation() {
    let semantic = analyze(&java_fixture("03-methods/ConstructorOverloads.java"));

    assert_eq!(
        semantic
            .facts
            .field_nullability_for("ConstructorOverloads", "label"),
        Some(NullabilityState::DefinitelyNonNull)
    );
    assert!(semantic.iterations.field_nullability >= 2);
}

#[test]
fn nullable_field_storage_uses_option_shape_and_assignment_wrapping() {
    let translation = translate_source(&java_fixture("05-types/Nullability.java"));

    assert!(
        translation.rust.contains("cached: Option<String>"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.cached = Some(value);"),
        "{}",
        translation.rust
    );
}

#[test]
fn records_trivial_getter_and_setter_facts() {
    let semantic = analyze(&idiom_gap("JavaBeanGetterSetterNames.java"));

    let getter = semantic
        .facts
        .trivial_accessor_for("JavaBeanGetterSetterNames", "getValue")
        .unwrap();
    assert!(matches!(
        &getter.kind,
        TrivialAccessorKind::Getter { field }
            if field.type_name == "JavaBeanGetterSetterNames" && field.field_name == "value"
    ));

    let setter = semantic
        .facts
        .trivial_accessor_for("JavaBeanGetterSetterNames", "setValue")
        .unwrap();
    assert!(matches!(
        &setter.kind,
        TrivialAccessorKind::Setter {
            field,
            param_name,
            fluent: false,
        } if field.type_name == "JavaBeanGetterSetterNames"
            && field.field_name == "value"
            && param_name == "value"
    ));
}

#[test]
fn lowers_trivial_accessor_calls_as_field_access_without_deleting_api_wrappers() {
    let translation = translate_source(&idiom_gap("JavaBeanGetterSetterNames.java"));

    assert!(
        translation.rust.contains("pub fn get_value"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn set_value"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn copy_from(&mut self"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.value = other.value"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return self.value;"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("self.get_value()")
            && !translation.rust.contains("self.set_value("),
        "{}",
        translation.rust
    );
}

#[test]
fn usage_treats_trivial_accessor_calls_as_backing_field_uses() {
    let semantic = analyze(&idiom_gap("JavaBeanGetterSetterNames.java"));
    let copy_from = semantic
        .usage
        .methods
        .get(&UsageMethodKey {
            type_name: "JavaBeanGetterSetterNames".to_string(),
            method_name: "copyFrom".to_string(),
        })
        .unwrap();

    assert!(copy_from.mutates_self);
    assert!(
        copy_from.field_uses.iter().any(|field_use| matches!(
            field_use,
            FieldUse::Returned { field_name } if field_name == "value"
        )),
        "{:?}",
        copy_from.field_uses
    );
}

#[test]
fn records_external_getter_and_setter_effect_guesses() {
    let semantic = analyze(&idiom_gap("ExternalGetterSetterMutability.java"));

    let getter = semantic.facts.external_api_effect_for("getValue").unwrap();
    assert_eq!(getter.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    let setter = semantic.facts.external_api_effect_for("setValue").unwrap();
    assert_eq!(
        setter.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn semantic_analysis_reports_external_api_budget_diagnostic() {
    let semantic = analyze_with_config(
        &idiom_gap("ExternalGetterSetterMutability.java"),
        semantic_analysis::SemanticAnalysisConfig {
            max_external_guess_iterations: Some(1),
            enable_external_api_guesses: true,
            enable_control_flow_facts: true,
        },
    );

    assert!(
        semantic.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "semantic_external_api_effect_budget_exhausted"
        })
    );
}

#[test]
fn external_setter_effect_requires_mutable_receiver_binding_in_signature() {
    let source = idiom_gap("ExternalGetterSetterMutability.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "update" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_eq!(param_shape.ownership, OwnershipShape::MutableBorrow);
}

#[test]
fn external_api_user_override_beats_later_name_guess() {
    let source = idiom_gap("ExternalGetterSetterMutability.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut external_facts = new_java2rust::facts::FactSet::default();
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "setValue".to_string(),
            receiver_type: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            provenance: FactProvenance::UserOverride,
        });
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external_facts);

    let effect = semantic.facts.external_api_effect_for("setValue").unwrap();
    assert_eq!(effect.provenance, FactProvenance::UserOverride);
    assert_eq!(effect.receiver_mutability, ReceiverMutabilityFact::ReadOnly);

    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "update" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_ne!(param_shape.ownership, OwnershipShape::MutableBorrow);
}

#[test]
fn external_api_receiver_specific_override_beats_global_override_in_signature() {
    let source = idiom_gap("ExternalGetterSetterMutability.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let mut external_facts = new_java2rust::facts::FactSet::default();
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "setValue".to_string(),
            receiver_type: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            provenance: FactProvenance::UserOverride,
        });
    external_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "setValue".to_string(),
            receiver_type: Some("ExternalCounter".to_string()),
            role: ExternalApiRole::Setter,
            receiver_mutability: ReceiverMutabilityFact::MutatesReceiver,
            provenance: FactProvenance::UserOverride,
        });
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &external_facts);

    let effect = semantic
        .facts
        .external_api_effect_for_receiver(Some("ExternalCounter"), "setValue")
        .unwrap();
    assert_eq!(effect.receiver_type.as_deref(), Some("ExternalCounter"));
    assert_eq!(
        effect.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );

    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "update" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_eq!(param_shape.ownership, OwnershipShape::MutableBorrow);
}

#[test]
fn external_getter_effect_preserves_shared_receiver_binding_in_signature() {
    let source = idiom_gap("ExternalGetterReadOnly.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "read" => Some(method),
            _ => None,
        })
        .unwrap();
    let param_shape = signatures.method_param_plans(ty, method).unwrap()[0]
        .shape
        .clone();

    assert_ne!(param_shape.ownership, OwnershipShape::MutableBorrow);
}

#[test]
fn records_mutual_recursion_edges_without_unbounded_growth() {
    let semantic = analyze(&idiom_gap("MutualRecursion.java"));

    let even = semantic
        .facts
        .recursion_facts_for("MutualRecursion", "even");
    assert!(
        even.iter()
            .any(|fact| { fact.calls.method_name == "odd" && fact.kind == RecursionKind::Mutual })
    );

    let odd = semantic.facts.recursion_facts_for("MutualRecursion", "odd");
    assert!(
        odd.iter()
            .any(|fact| { fact.calls.method_name == "even" && fact.kind == RecursionKind::Mutual })
    );
}

#[test]
fn records_assignment_in_condition_marker() {
    let semantic = analyze(&idiom_gap("WhileAssignmentNotNull.java"));

    assert!(semantic.facts.has_control_flow_fact(
        "WhileAssignmentNotNull",
        "drain",
        ControlFlowFactKind::AssignmentInCondition
    ));
}

#[test]
fn plans_assignment_in_condition_normalization_shape() {
    let semantic = analyze(&idiom_gap("WhileAssignmentNotNull.java"));
    let plan = semantic
        .control_flow_plans
        .iter()
        .find(|plan| {
            plan.method.type_name == "WhileAssignmentNotNull" && plan.method.method_name == "drain"
        })
        .unwrap();

    assert!(plan.rewrites.iter().any(|rewrite| matches!(
        rewrite,
        ControlFlowRewritePlan::AssignmentInCondition(assignment)
            if assignment.local_name == "row"
                && assignment.source_call == "next"
                && assignment.null_test == NullTestKind::NotNull
                && assignment.normalized_shape
                    == AssignmentConditionNormalizedShape::LoopAssignThenBreak
    )));
}

#[test]
fn records_tail_recursive_self_call_marker() {
    let semantic = analyze(&idiom_gap("TailRecursiveAccumulator.java"));

    assert!(semantic.facts.has_control_flow_fact(
        "TailRecursiveAccumulator",
        "factorialLoop",
        ControlFlowFactKind::TailRecursiveSelfCall
    ));
}

#[test]
fn plans_tail_recursive_loop_normalization_shape() {
    let semantic = analyze(&idiom_gap("TailRecursiveAccumulator.java"));
    let plan = semantic
        .control_flow_plans
        .iter()
        .find(|plan| {
            plan.method.type_name == "TailRecursiveAccumulator"
                && plan.method.method_name == "factorialLoop"
        })
        .unwrap();

    assert!(plan.rewrites.iter().any(|rewrite| matches!(
        rewrite,
        ControlFlowRewritePlan::TailRecursiveLoop(tail)
            if tail.method_name == "factorialLoop"
                && tail.arg_count == 2
                && tail.normalized_shape
                    == TailRecursiveNormalizedShape::LoopWithParameterReassignment
    )));
}

#[test]
fn translation_reports_pending_control_flow_rewrite_diagnostics() {
    let translation = translate_source(&idiom_gap("WhileAssignmentNotNull.java"));

    assert!(
        translation
            .diagnostics
            .has_code("control_flow_rewrite_pending")
    );
    assert!(translation.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("assignment-in-condition normalization")
    }));
}

#[test]
fn translation_reports_pending_tail_recursive_rewrite_diagnostics() {
    let translation = translate_source(&idiom_gap("TailRecursiveAccumulator.java"));

    assert!(
        translation
            .diagnostics
            .has_code("control_flow_rewrite_pending")
    );
    assert!(translation.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("tail-recursive loop normalization")
    }));
}

#[test]
fn records_exception_subclass_edges() {
    let semantic = analyze(&idiom_gap("ExceptionSubclassCatchOrder.java"));
    let specific_problem = JavaType::Named {
        name: "SpecificProblem".to_string(),
        args: Vec::new(),
    };
    let reachable = semantic
        .facts
        .reachable_hierarchy_edges_for_type(&specific_problem);

    assert!(
        reachable
            .iter()
            .any(|edge| { edge.sub_name == "SpecificProblem" && edge.super_name == "BaseProblem" })
    );
    assert!(
        reachable
            .iter()
            .any(|edge| { edge.sub_name == "SpecificProblem" && edge.super_name == "Exception" })
    );
}

#[test]
fn records_catch_assignability_from_reachable_exception_hierarchy() {
    let semantic = analyze(&idiom_gap("ExceptionSubclassCatchOrder.java"));
    let catches = semantic
        .facts
        .catch_facts_for("ExceptionSubclassCatchOrder", "parse");

    assert_eq!(catches.len(), 2);
    assert_eq!(
        catches[0].assignable_from,
        vec![JavaType::Named {
            name: "SpecificProblem".to_string(),
            args: Vec::new(),
        }]
    );
    assert_eq!(
        catches[1].assignable_from,
        vec![
            JavaType::Named {
                name: "BaseProblem".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "SpecificProblem".to_string(),
                args: Vec::new(),
            },
        ]
    );
}

#[test]
fn records_checked_and_unchecked_exception_kinds() {
    let semantic = analyze(&idiom_gap("ExceptionRuntimeCheckedHierarchy.java"));

    assert_eq!(
        semantic.facts.exception_kind_for("CheckedProblem"),
        Some(ExceptionKind::Checked)
    );
    assert_eq!(
        semantic.facts.exception_kind_for("UncheckedProblem"),
        Some(ExceptionKind::Unchecked)
    );
}

#[test]
fn records_declared_throws_as_method_effect_fact() {
    let semantic = analyze(&idiom_gap("ExceptionRuntimeCheckedHierarchy.java"));
    let effect = semantic
        .facts
        .method_effect_for("ExceptionRuntimeCheckedHierarchy", "classify")
        .unwrap();

    assert_eq!(
        effect.thrown,
        vec![JavaType::Named {
            name: "CheckedProblem".to_string(),
            args: Vec::new(),
        }]
    );
}

#[test]
fn records_catch_variable_rethrow_as_method_effect_fact() {
    let semantic = analyze(&idiom_gap("ExceptionRethrowSpecific.java"));
    let effect = semantic
        .facts
        .method_effect_for("ExceptionRethrowSpecific", "validate")
        .unwrap();

    assert_eq!(
        effect.thrown,
        vec![JavaType::Named {
            name: "ValidationProblem".to_string(),
            args: Vec::new(),
        }]
    );
}

#[test]
fn records_receiver_mutability_as_method_effect_fact() {
    let source = r#"
class Counter {
    int value;

    int get() {
        return this.value;
    }

    void set(int next) {
        this.value = next;
    }
}
"#;
    let semantic = analyze(source);

    assert_eq!(
        semantic
            .facts
            .method_effect_for("Counter", "get")
            .unwrap()
            .receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        semantic
            .facts
            .method_effect_for("Counter", "set")
            .unwrap()
            .receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
}

#[test]
fn records_return_nullability_as_method_effect_fact() {
    let source = r#"
class MaybeName {
    String choose(boolean flag) {
        if (flag) {
            return "name";
        }
        return null;
    }

    String create() {
        return new String();
    }
}
"#;
    let semantic = analyze(source);

    assert_eq!(
        semantic
            .facts
            .method_effect_for("MaybeName", "choose")
            .unwrap()
            .return_nullability,
        NullabilityState::MaybeNull
    );
    assert_eq!(
        semantic
            .facts
            .method_effect_for("MaybeName", "create")
            .unwrap()
            .return_nullability,
        NullabilityState::DefinitelyNonNull
    );
}

#[test]
fn return_nullability_method_effect_shapes_nullable_returns_as_option() {
    let source = r#"
class MaybeName {
    String choose(boolean flag) {
        if (flag) {
            return "name";
        }
        return null;
    }
}
"#;
    let parsed = parser::parse_java_source(source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) if method.name == "choose" => {
                Some(method)
            }
            _ => None,
        })
        .unwrap();

    assert_eq!(
        signatures.method_return_shape(ty, method).unwrap().java_ty,
        optional_java_type(JavaType::String)
    );
}

#[test]
fn exception_support_decision_consumes_semantic_exception_facts() {
    let source = idiom_gap("ExceptionRuntimeCheckedHierarchy.java");
    let parsed = parser::parse_java_source(&source);
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );

    assert!(exception_plan::program_uses_java_exception_with_facts(
        &resolved.program,
        &semantic.facts
    ));
}

#[test]
fn records_multi_catch_types_in_source_order() {
    let semantic = analyze(&java_fixture("06-stdlib/MultiCatch.java"));
    let catches = semantic.facts.catch_facts_for("MultiCatch", "parseOrZero");

    assert_eq!(catches.len(), 1);
    assert_eq!(catches[0].clause_index, 0);
    assert_eq!(
        catches[0].types,
        vec![
            JavaType::Named {
                name: "NumberFormatException".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "NullPointerException".to_string(),
                args: Vec::new(),
            },
        ]
    );
    assert_eq!(
        semantic.facts.exception_kind_for("NumberFormatException"),
        Some(ExceptionKind::Unchecked)
    );
    assert_eq!(
        semantic.facts.exception_kind_for("NullPointerException"),
        Some(ExceptionKind::Unchecked)
    );
}
