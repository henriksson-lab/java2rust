use crate::diagnostics::DiagnosticBag;
use crate::facts::{
    ClosedHierarchyFact, FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy,
    ObjectSafetyFact, ObjectSafetyViolation, TypeCapabilityOverride,
};
use crate::java_ir::JavaType;
use crate::parser::parse_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFactsParse {
    pub facts: FactSet,
    pub diagnostics: DiagnosticBag,
}

pub fn parse_external_facts(source: &str) -> ExternalFactsParse {
    let mut facts = FactSet::default();
    let mut diagnostics = DiagnosticBag::default();

    for (line_index, raw_line) in source.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let parts = line.split_whitespace().collect::<Vec<_>>();
        match parts.as_slice() {
            ["closed", type_name] => facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
                type_name: (*type_name).to_string(),
                closed: true,
                provenance: FactProvenance::UserOverride,
            }),
            ["closed", type_name, closed] => {
                if let Some(closed) = parse_bool(closed) {
                    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
                        type_name: (*type_name).to_string(),
                        closed,
                        provenance: FactProvenance::UserOverride,
                    });
                } else {
                    push_external_fact_error(&mut diagnostics, line_index, "invalid closed value");
                }
            }
            ["closed-enum", type_name, enum_name, variants @ ..] if !variants.is_empty() => {
                facts.hierarchy_overrides.push(HierarchyOverride {
                    type_name: (*type_name).to_string(),
                    strategy: HierarchyStrategy::ClosedEnum {
                        enum_name: (*enum_name).to_string(),
                        variants: variants
                            .iter()
                            .map(|variant| (*variant).to_string())
                            .collect(),
                    },
                    provenance: FactProvenance::UserOverride,
                });
            }
            ["dyn-trait", type_name, trait_name] => {
                facts.hierarchy_overrides.push(HierarchyOverride {
                    type_name: (*type_name).to_string(),
                    strategy: HierarchyStrategy::DynTrait {
                        trait_name: (*trait_name).to_string(),
                    },
                    provenance: FactProvenance::UserOverride,
                })
            }
            ["boxed-dyn-trait", type_name, trait_name] => {
                facts.hierarchy_overrides.push(HierarchyOverride {
                    type_name: (*type_name).to_string(),
                    strategy: HierarchyStrategy::BoxedDynTrait {
                        trait_name: (*trait_name).to_string(),
                    },
                    provenance: FactProvenance::UserOverride,
                })
            }
            ["erased-object", type_name] => facts.hierarchy_overrides.push(HierarchyOverride {
                type_name: (*type_name).to_string(),
                strategy: HierarchyStrategy::ErasedObject,
                provenance: FactProvenance::UserOverride,
            }),
            ["concrete", type_name] => facts.hierarchy_overrides.push(HierarchyOverride {
                type_name: (*type_name).to_string(),
                strategy: HierarchyStrategy::ConcreteStruct,
                provenance: FactProvenance::UserOverride,
            }),
            ["copy", type_name] => facts
                .type_capability_overrides
                .push(bool_capability_override(
                    type_name,
                    CapabilityBoolField::Copy,
                    true,
                )),
            ["copy", type_name, value] => {
                push_bool_capability_override(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    CapabilityBoolField::Copy,
                    value,
                );
            }
            ["clone", type_name] => facts
                .type_capability_overrides
                .push(bool_capability_override(
                    type_name,
                    CapabilityBoolField::Clone,
                    true,
                )),
            ["clone", type_name, value] => {
                push_bool_capability_override(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    CapabilityBoolField::Clone,
                    value,
                );
            }
            ["string-like", type_name] => {
                facts
                    .type_capability_overrides
                    .push(bool_capability_override(
                        type_name,
                        CapabilityBoolField::StringLike,
                        true,
                    ));
            }
            ["string-like", type_name, value] => {
                push_bool_capability_override(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    CapabilityBoolField::StringLike,
                    value,
                );
            }
            ["final", type_name] => facts
                .type_capability_overrides
                .push(bool_capability_override(
                    type_name,
                    CapabilityBoolField::FinalType,
                    true,
                )),
            ["final", type_name, value] => {
                push_bool_capability_override(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    CapabilityBoolField::FinalType,
                    value,
                );
            }
            ["object-safe-trait", type_name] => {
                facts
                    .type_capability_overrides
                    .push(bool_capability_override(
                        type_name,
                        CapabilityBoolField::ObjectSafeTrait,
                        true,
                    ));
            }
            ["object-safe-trait", type_name, value] => {
                push_bool_capability_override(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    CapabilityBoolField::ObjectSafeTrait,
                    value,
                );
            }
            [
                "object-safety-violation",
                type_name,
                "static-method",
                method_name,
            ] => {
                push_object_safety_violation(
                    &mut facts,
                    type_name,
                    ObjectSafetyViolation::StaticMethod {
                        method_name: (*method_name).to_string(),
                    },
                );
            }
            [
                "object-safety-violation",
                type_name,
                "self-return",
                method_name,
            ] => {
                push_object_safety_violation(
                    &mut facts,
                    type_name,
                    ObjectSafetyViolation::SelfReferentialReturn {
                        method_name: (*method_name).to_string(),
                    },
                );
            }
            [
                "object-safety-violation",
                type_name,
                "nested-self-return",
                method_name,
            ] => {
                push_object_safety_violation(
                    &mut facts,
                    type_name,
                    ObjectSafetyViolation::NestedSelfReferentialReturn {
                        method_name: (*method_name).to_string(),
                    },
                );
            }
            [
                "object-safety-violation",
                type_name,
                "self-param",
                method_name,
                param_name,
            ] => {
                push_object_safety_violation(
                    &mut facts,
                    type_name,
                    ObjectSafetyViolation::SelfReferentialParameter {
                        method_name: (*method_name).to_string(),
                        param_name: (*param_name).to_string(),
                    },
                );
            }
            [
                "object-safety-violation",
                type_name,
                "nested-self-param",
                method_name,
                param_name,
            ] => {
                push_object_safety_violation(
                    &mut facts,
                    type_name,
                    ObjectSafetyViolation::NestedSelfReferentialParameter {
                        method_name: (*method_name).to_string(),
                        param_name: (*param_name).to_string(),
                    },
                );
            }
            ["object-safety-violation", type_name, "field", field_name] => {
                push_object_safety_violation(
                    &mut facts,
                    type_name,
                    ObjectSafetyViolation::FieldMember {
                        field_name: (*field_name).to_string(),
                    },
                );
            }
            ["object-safety-violation", type_name, "constructor"] => {
                push_object_safety_violation(
                    &mut facts,
                    type_name,
                    ObjectSafetyViolation::ConstructorMember,
                );
            }
            ["object-safety-violation", type_name, "unsupported-member"] => {
                push_object_safety_violation(
                    &mut facts,
                    type_name,
                    ObjectSafetyViolation::UnsupportedMember,
                );
            }
            ["sequence-like", type_name, element_type] => {
                facts
                    .type_capability_overrides
                    .push(sequence_capability_override(
                        type_name,
                        parse_type(element_type),
                    ));
            }
            ["extends", type_name, super_type] => {
                facts
                    .type_capability_overrides
                    .push(extends_capability_override(
                        type_name,
                        parse_type(super_type),
                    ));
            }
            ["implements", type_name, implemented_types @ ..] if !implemented_types.is_empty() => {
                facts
                    .type_capability_overrides
                    .push(implements_capability_override(
                        type_name,
                        implemented_types
                            .iter()
                            .map(|implemented_type| parse_type(implemented_type))
                            .collect(),
                    ));
            }
            _ => push_external_fact_error(&mut diagnostics, line_index, "unrecognized fact line"),
        }
    }

    ExternalFactsParse { facts, diagnostics }
}

fn strip_comment(line: &str) -> &str {
    line.split_once('#')
        .map(|(before_comment, _)| before_comment)
        .unwrap_or(line)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "yes" | "closed" => Some(true),
        "false" | "no" | "open" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapabilityBoolField {
    Copy,
    Clone,
    StringLike,
    FinalType,
    ObjectSafeTrait,
}

fn push_bool_capability_override(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    field: CapabilityBoolField,
    value: &str,
) {
    if let Some(value) = parse_bool(value) {
        facts
            .type_capability_overrides
            .push(bool_capability_override(type_name, field, value));
    } else {
        push_external_fact_error(diagnostics, line_index, "invalid capability bool value");
    }
}

fn bool_capability_override(
    type_name: &str,
    field: CapabilityBoolField,
    value: bool,
) -> TypeCapabilityOverride {
    let mut override_fact = empty_type_capability_override(type_name);
    match field {
        CapabilityBoolField::Copy => override_fact.copy = Some(value),
        CapabilityBoolField::Clone => override_fact.clone = Some(value),
        CapabilityBoolField::StringLike => override_fact.string_like = Some(value),
        CapabilityBoolField::FinalType => override_fact.final_type = Some(value),
        CapabilityBoolField::ObjectSafeTrait => override_fact.object_safe_trait = Some(value),
    }
    override_fact
}

fn push_object_safety_violation(
    facts: &mut FactSet,
    type_name: &str,
    violation: ObjectSafetyViolation,
) {
    if let Some(fact) =
        facts.object_safety_facts.iter_mut().rev().find(|fact| {
            fact.type_name == type_name && fact.provenance == FactProvenance::UserOverride
        })
    {
        fact.object_safe = false;
        fact.violations.push(violation);
    } else {
        facts.object_safety_facts.push(ObjectSafetyFact {
            type_name: type_name.to_string(),
            object_safe: false,
            violations: vec![violation],
            provenance: FactProvenance::UserOverride,
        });
    }
    facts
        .type_capability_overrides
        .push(bool_capability_override(
            type_name,
            CapabilityBoolField::ObjectSafeTrait,
            false,
        ));
}

fn sequence_capability_override(type_name: &str, element_type: JavaType) -> TypeCapabilityOverride {
    let mut override_fact = empty_type_capability_override(type_name);
    override_fact.sequence_like = Some(element_type);
    override_fact
}

fn extends_capability_override(type_name: &str, super_type: JavaType) -> TypeCapabilityOverride {
    let mut override_fact = empty_type_capability_override(type_name);
    override_fact.extends = Some(super_type);
    override_fact
}

fn implements_capability_override(
    type_name: &str,
    implemented_types: Vec<JavaType>,
) -> TypeCapabilityOverride {
    let mut override_fact = empty_type_capability_override(type_name);
    override_fact.implements = implemented_types;
    override_fact
}

fn empty_type_capability_override(type_name: &str) -> TypeCapabilityOverride {
    TypeCapabilityOverride {
        type_name: type_name.to_string(),
        copy: None,
        clone: None,
        string_like: None,
        sequence_like: None,
        implements: Vec::new(),
        extends: None,
        final_type: None,
        object_safe_trait: None,
        provenance: FactProvenance::UserOverride,
    }
}

fn push_external_fact_error(diagnostics: &mut DiagnosticBag, line_index: usize, message: &str) {
    diagnostics.error(
        "external_fact_parse_error",
        format!("external facts line {}: {message}", line_index + 1),
    );
}
