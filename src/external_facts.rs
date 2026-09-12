use crate::diagnostics::DiagnosticBag;
use crate::facts::{
    AccessorInlinePolicy, Capture, CaptureOrigin, CaptureOwner, CaptureOwnerKind, CaptureSource,
    CatchFact, ClosedHierarchyFact, ControlFlowFact, ControlFlowFactKind, DispatchRequirementFact,
    DispatchRequirementKind, ExternalApiArgumentEffectFact, ExternalApiEffectFact, ExternalApiRole,
    ExternalArgumentAliasPolicy, ExternalConstructorFact, ExternalCrateApiMappingFact,
    ExternalCrateConstructorMappingFact, ExternalFieldFact, ExternalFieldKind, FactProvenance,
    FactSet, FieldKey, FieldNullabilityFact, FieldNullabilityProvenance, HierarchyOverride,
    HierarchyStrategy, MethodEffectFact, MethodKey, NodeId, NullabilityState, ObjectSafetyFact,
    ObjectSafetyViolation, ReceiverMutabilityFact, RecursionFact, RecursionKind,
    TrivialAccessorFact, TrivialAccessorKind, TypeCapabilityOverride, TypePattern,
    UnsupportedExternalApiFact, UnsupportedExternalConstructorFact, UnsupportedExternalFieldFact,
};
use crate::java_ir::JavaType;
use crate::parser::parse_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFactsParse {
    facts: FactSet,
    diagnostics: DiagnosticBag,
}

impl ExternalFactsParse {
    pub fn new(facts: FactSet, diagnostics: DiagnosticBag) -> Self {
        Self { facts, diagnostics }
    }

    pub fn facts(&self) -> &FactSet {
        &self.facts
    }

    pub fn diagnostics(&self) -> &DiagnosticBag {
        &self.diagnostics
    }

    pub fn into_parts(self) -> (FactSet, DiagnosticBag) {
        (self.facts, self.diagnostics)
    }
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
                    strategy: HierarchyStrategy::DynTrait {
                        trait_name: (*trait_name).to_string(),
                    },
                    provenance: FactProvenance::UserOverride,
                });
                facts
                    .dispatch_requirement_facts
                    .push(DispatchRequirementFact {
                        type_name: (*type_name).to_string(),
                        requirement: DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
                        provenance: FactProvenance::UserOverride,
                    });
            }
            ["dispatch-requirement", type_name, requirement] => {
                let Some(requirement) = parse_dispatch_requirement(requirement) else {
                    push_external_fact_error(
                        &mut diagnostics,
                        line_index,
                        "invalid dispatch requirement",
                    );
                    continue;
                };
                facts
                    .dispatch_requirement_facts
                    .push(DispatchRequirementFact {
                        type_name: (*type_name).to_string(),
                        requirement,
                        provenance: FactProvenance::UserOverride,
                    });
            }
            [
                "method-effect",
                type_name,
                method_name,
                receiver_mutability,
                return_nullability,
                metadata @ ..,
            ] => {
                push_method_effect_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    method_name,
                    receiver_mutability,
                    return_nullability,
                    metadata,
                );
            }
            [
                "capture",
                "method",
                origin_type,
                origin_method,
                owner_id,
                owner_kind,
                owner_name,
                target_type,
                capture_name,
                capture_source,
                capture_type,
                effectively_final,
                mutable_after_capture,
            ] => {
                push_capture_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    CaptureOriginInput::Method {
                        type_name: origin_type,
                        method_name: origin_method,
                    },
                    CaptureFactInput {
                        owner_id,
                        owner_kind,
                        owner_name,
                        target_type,
                        capture_name,
                        capture_source,
                        capture_type,
                        effectively_final,
                        mutable_after_capture,
                    },
                );
            }
            [
                "capture",
                "constructor",
                origin_type,
                owner_id,
                owner_kind,
                owner_name,
                target_type,
                capture_name,
                capture_source,
                capture_type,
                effectively_final,
                mutable_after_capture,
            ] => {
                push_capture_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    CaptureOriginInput::Constructor {
                        type_name: origin_type,
                    },
                    CaptureFactInput {
                        owner_id,
                        owner_kind,
                        owner_name,
                        target_type,
                        capture_name,
                        capture_source,
                        capture_type,
                        effectively_final,
                        mutable_after_capture,
                    },
                );
            }
            [
                "capture",
                "type",
                origin_type,
                owner_id,
                owner_kind,
                owner_name,
                target_type,
                capture_name,
                capture_source,
                capture_type,
                effectively_final,
                mutable_after_capture,
            ] => {
                push_capture_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    CaptureOriginInput::Type {
                        type_name: origin_type,
                    },
                    CaptureFactInput {
                        owner_id,
                        owner_kind,
                        owner_name,
                        target_type,
                        capture_name,
                        capture_source,
                        capture_type,
                        effectively_final,
                        mutable_after_capture,
                    },
                );
            }
            [
                "trivial-getter",
                type_name,
                method_name,
                field_name,
                metadata @ ..,
            ] => {
                push_trivial_getter_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    method_name,
                    field_name,
                    metadata,
                );
            }
            [
                "trivial-setter",
                type_name,
                method_name,
                field_name,
                param_name,
                metadata @ ..,
            ] => {
                push_trivial_setter_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    method_name,
                    field_name,
                    param_name,
                    metadata,
                );
            }
            [
                "recursion",
                type_name,
                method_name,
                calls_type,
                calls_method,
                kind,
            ] => {
                push_recursion_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    method_name,
                    calls_type,
                    calls_method,
                    kind,
                );
            }
            ["control-flow", type_name, method_name, kind] => {
                push_control_flow_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    method_name,
                    kind,
                );
            }
            [
                "catch",
                type_name,
                method_name,
                clause_index,
                catch_types,
                metadata @ ..,
            ] => {
                push_catch_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    method_name,
                    clause_index,
                    catch_types,
                    metadata,
                );
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
            [
                "external-api",
                method_name,
                receiver_type,
                role,
                receiver_mutability,
            ] => {
                push_external_api_effect_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    method_name,
                    receiver_type,
                    role,
                    receiver_mutability,
                    "unknown",
                    &[],
                );
            }
            [
                "external-api",
                method_name,
                receiver_type,
                role,
                receiver_mutability,
                return_nullability,
                metadata @ ..,
            ] => {
                push_external_api_effect_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    method_name,
                    receiver_type,
                    role,
                    receiver_mutability,
                    return_nullability,
                    metadata,
                );
            }
            [
                "external-argument",
                method_name,
                receiver_type,
                parameter_index,
                parameter_nullability,
                argument_mutability,
                alias_policy,
                metadata @ ..,
            ] => {
                push_external_api_argument_effect_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    method_name,
                    receiver_type,
                    parameter_index,
                    parameter_nullability,
                    argument_mutability,
                    alias_policy,
                    metadata,
                );
            }
            [
                "external-crate-api",
                receiver_type,
                method_name,
                arg_count,
                rust_name,
            ] => {
                push_external_crate_api_mapping_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    receiver_type,
                    method_name,
                    arg_count,
                    rust_name,
                );
            }
            [
                "external-crate-constructor",
                type_name,
                arg_count,
                rust_type_name,
                rust_name,
            ] => {
                push_external_crate_constructor_mapping_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    arg_count,
                    rust_type_name,
                    rust_name,
                );
            }
            ["external-constructor", type_name, arg_types @ ..] => {
                facts
                    .external_constructor_facts
                    .push(ExternalConstructorFact {
                        type_name: (*type_name).to_string(),
                        arg_types: arg_types
                            .iter()
                            .map(|arg_type| parse_type(arg_type))
                            .collect(),
                        provenance: FactProvenance::UserOverride,
                    });
            }
            [
                "unsupported-api",
                pack_name,
                receiver_type,
                method_name,
                reason @ ..,
            ] if !reason.is_empty() => {
                facts
                    .unsupported_external_api_facts
                    .push(UnsupportedExternalApiFact {
                        pack_name: (*pack_name).to_string(),
                        receiver_type: (*receiver_type).to_string(),
                        method_name: (*method_name).to_string(),
                        reason: reason.join(" "),
                        provenance: FactProvenance::UserOverride,
                    });
            }
            [
                "unsupported-constructor",
                pack_name,
                type_name,
                arg_count,
                reason @ ..,
            ] if !reason.is_empty() => {
                push_unsupported_external_constructor_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    pack_name,
                    type_name,
                    arg_count,
                    reason,
                );
            }
            [
                "unsupported-field",
                pack_name,
                type_name,
                field_name,
                reason @ ..,
            ] if !reason.is_empty() => {
                facts
                    .unsupported_external_field_facts
                    .push(UnsupportedExternalFieldFact {
                        pack_name: (*pack_name).to_string(),
                        field: FieldKey::named(*type_name, *field_name),
                        reason: reason.join(" "),
                        provenance: FactProvenance::UserOverride,
                    });
            }
            ["field", type_name, field_name, field_type] => {
                push_external_field_fact(&mut facts, type_name, field_name, field_type);
            }
            ["field", field_path, field_type] => {
                if let Some((type_name, field_name)) = field_path.split_once('.') {
                    push_external_field_fact(&mut facts, type_name, field_name, field_type);
                } else {
                    push_external_fact_error(
                        &mut diagnostics,
                        line_index,
                        "field path must be Type.field",
                    );
                }
            }
            ["field-nullability", type_name, field_name, state] => {
                push_field_nullability_fact(
                    &mut facts,
                    &mut diagnostics,
                    line_index,
                    type_name,
                    field_name,
                    state,
                );
            }
            ["field-nullability", field_path, state] => {
                if let Some((type_name, field_name)) = field_path.split_once('.') {
                    push_field_nullability_fact(
                        &mut facts,
                        &mut diagnostics,
                        line_index,
                        type_name,
                        field_name,
                        state,
                    );
                } else {
                    push_external_fact_error(
                        &mut diagnostics,
                        line_index,
                        "field-nullability path must be Type.field",
                    );
                }
            }
            _ => push_external_fact_error(&mut diagnostics, line_index, "unrecognized fact line"),
        }
    }

    ExternalFactsParse::new(facts, diagnostics)
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

fn parse_external_api_role(value: &str) -> Option<ExternalApiRole> {
    match value {
        "getter" => Some(ExternalApiRole::Getter),
        "boolean-getter" => Some(ExternalApiRole::BooleanGetter),
        "setter" => Some(ExternalApiRole::Setter),
        "mutator" => Some(ExternalApiRole::Mutator),
        "fluent-mutator" => Some(ExternalApiRole::FluentMutator),
        "side-effect" => Some(ExternalApiRole::SideEffect),
        "indexed-getter" => Some(ExternalApiRole::IndexedGetter),
        "indexed-setter" => Some(ExternalApiRole::IndexedSetter),
        "unknown" => Some(ExternalApiRole::Unknown),
        _ => None,
    }
}

fn parse_receiver_mutability(value: &str) -> Option<ReceiverMutabilityFact> {
    match value {
        "read-only" => Some(ReceiverMutabilityFact::ReadOnly),
        "mutates-receiver" => Some(ReceiverMutabilityFact::MutatesReceiver),
        "unknown" => Some(ReceiverMutabilityFact::Unknown),
        _ => None,
    }
}

fn parse_external_argument_alias_policy(value: &str) -> Option<ExternalArgumentAliasPolicy> {
    match value {
        "unknown" => Some(ExternalArgumentAliasPolicy::Unknown),
        "no-alias" => Some(ExternalArgumentAliasPolicy::NoAlias),
        "borrowed" => Some(ExternalArgumentAliasPolicy::Borrowed),
        "escapes" => Some(ExternalArgumentAliasPolicy::Escapes),
        _ => None,
    }
}

fn parse_receiver_type(value: &str) -> Option<String> {
    match value {
        "*" | "any" | "global" => None,
        _ => Some(value.to_string()),
    }
}

fn parse_dispatch_requirement(value: &str) -> Option<DispatchRequirementKind> {
    match value {
        "needs-subtype-dispatch" => Some(DispatchRequirementKind::NeedsSubtypeDispatch),
        "needs-closed-runtime-case-split" => {
            Some(DispatchRequirementKind::NeedsClosedRuntimeCaseSplit)
        }
        "needs-instanceof" => Some(DispatchRequirementKind::NeedsInstanceof),
        "needs-downcast" => Some(DispatchRequirementKind::NeedsDowncast),
        "needs-owned-polymorphic-storage" => {
            Some(DispatchRequirementKind::NeedsOwnedPolymorphicStorage)
        }
        "needs-borrowed-trait-view" => Some(DispatchRequirementKind::NeedsBorrowedTraitView),
        "needs-public-extensible-api" => Some(DispatchRequirementKind::NeedsPublicExtensibleApi),
        "reject-trait-object" => Some(DispatchRequirementKind::RejectTraitObject),
        "needs-unknown-runtime-object" => Some(DispatchRequirementKind::NeedsUnknownRuntimeObject),
        _ => None,
    }
}

fn parse_recursion_kind(value: &str) -> Option<RecursionKind> {
    match value {
        "direct" => Some(RecursionKind::Direct),
        "mutual" => Some(RecursionKind::Mutual),
        _ => None,
    }
}

fn parse_control_flow_kind(value: &str) -> Option<ControlFlowFactKind> {
    match value {
        "assignment-in-condition" => Some(ControlFlowFactKind::AssignmentInCondition),
        "tail-recursive-self-call" => Some(ControlFlowFactKind::TailRecursiveSelfCall),
        _ => None,
    }
}

fn parse_accessor_inline_policy(value: &str) -> Option<AccessorInlinePolicy> {
    match value {
        "preserve-api" => Some(AccessorInlinePolicy::PreserveApi),
        "inline-candidate" => Some(AccessorInlinePolicy::InlineCandidate),
        "analyze-only" => Some(AccessorInlinePolicy::AnalyzeOnly),
        _ => None,
    }
}

fn parse_capture_owner_kind(value: &str) -> Option<CaptureOwnerKind> {
    match value {
        "lambda" => Some(CaptureOwnerKind::Lambda),
        "local-class" => Some(CaptureOwnerKind::LocalClass),
        "anonymous-class" => Some(CaptureOwnerKind::AnonymousClass),
        "inner-class" => Some(CaptureOwnerKind::InnerClass),
        _ => None,
    }
}

fn parse_capture_source(value: &str) -> Option<CaptureSource> {
    match value {
        "local" => Some(CaptureSource::Local),
        "parameter" => Some(CaptureSource::Parameter),
        "this-field" => Some(CaptureSource::ThisField),
        "outer-this" => Some(CaptureSource::OuterThis),
        _ => None,
    }
}

fn parse_nullability_state(value: &str) -> Option<NullabilityState> {
    match value {
        "definitely-non-null" | "non-null" => Some(NullabilityState::DefinitelyNonNull),
        "maybe-null" | "nullable" => Some(NullabilityState::MaybeNull),
        "definitely-null" | "null" => Some(NullabilityState::DefinitelyNull),
        "unknown" => Some(NullabilityState::Unknown),
        _ => None,
    }
}

fn parse_comma_type_list(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    value: &str,
    context: &str,
) -> Option<Vec<JavaType>> {
    if value.is_empty() {
        push_external_fact_error(diagnostics, line_index, context);
        return None;
    }

    let mut types = Vec::new();
    for type_name in value.split(',').filter(|ty| !ty.is_empty()) {
        let ty = parse_type(type_name);
        if !types.contains(&ty) {
            types.push(ty);
        }
    }
    if types.is_empty() {
        push_external_fact_error(diagnostics, line_index, context);
        return None;
    }
    Some(types)
}

enum CaptureOriginInput<'a> {
    Method {
        type_name: &'a str,
        method_name: &'a str,
    },
    Constructor {
        type_name: &'a str,
    },
    Type {
        type_name: &'a str,
    },
}

struct CaptureFactInput<'a> {
    owner_id: &'a str,
    owner_kind: &'a str,
    owner_name: &'a str,
    target_type: &'a str,
    capture_name: &'a str,
    capture_source: &'a str,
    capture_type: &'a str,
    effectively_final: &'a str,
    mutable_after_capture: &'a str,
}

fn parse_optional_name(value: &str) -> Option<String> {
    match value {
        "*" | "-" | "none" => None,
        _ => Some(value.to_string()),
    }
}

fn parse_optional_type(value: &str) -> Option<JavaType> {
    match value {
        "*" | "-" | "none" => None,
        _ => Some(parse_type(value)),
    }
}

fn push_capture_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    origin: CaptureOriginInput<'_>,
    input: CaptureFactInput<'_>,
) {
    let Ok(owner_id) = input.owner_id.parse::<usize>() else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "capture owner id must be a non-negative integer",
        );
        return;
    };
    let Some(owner_kind) = parse_capture_owner_kind(input.owner_kind) else {
        push_external_fact_error(diagnostics, line_index, "invalid capture owner kind");
        return;
    };
    let Some(capture_source) = parse_capture_source(input.capture_source) else {
        push_external_fact_error(diagnostics, line_index, "invalid capture source");
        return;
    };
    let Some(effectively_final) = parse_bool(input.effectively_final) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid capture effectively-final value",
        );
        return;
    };
    let Some(mutable_after_capture) = parse_bool(input.mutable_after_capture) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid capture mutable-after-capture value",
        );
        return;
    };

    let origin = match origin {
        CaptureOriginInput::Method {
            type_name,
            method_name,
        } => CaptureOrigin::Method {
            type_name: type_name.to_string(),
            method_name: method_name.to_string(),
        },
        CaptureOriginInput::Constructor { type_name } => CaptureOrigin::Constructor {
            type_name: type_name.to_string(),
        },
        CaptureOriginInput::Type { type_name } => CaptureOrigin::Type {
            type_name: type_name.to_string(),
        },
    };

    facts.captures.owners.push(CaptureOwner {
        id: NodeId(owner_id),
        kind: owner_kind,
        name: parse_optional_name(input.owner_name),
        target_type: parse_optional_type(input.target_type),
        origin,
        captures: vec![Capture {
            name: input.capture_name.to_string(),
            source: capture_source,
            ty: parse_type(input.capture_type),
            effectively_final,
            mutable_after_capture,
        }],
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrivialAccessorMetadata {
    inline_policy: AccessorInlinePolicy,
    fluent: bool,
}

impl Default for TrivialAccessorMetadata {
    fn default() -> Self {
        Self {
            inline_policy: AccessorInlinePolicy::InlineCandidate,
            fluent: false,
        }
    }
}

fn parse_trivial_accessor_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    metadata: &[&str],
) -> Option<TrivialAccessorMetadata> {
    let mut parsed = TrivialAccessorMetadata::default();
    for token in metadata {
        if let Some(policy) = token.strip_prefix("policy=") {
            let Some(policy) = parse_accessor_inline_policy(policy) else {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "invalid trivial accessor inline policy",
                );
                return None;
            };
            parsed.inline_policy = policy;
        } else if let Some(fluent) = token.strip_prefix("fluent=") {
            let Some(fluent) = parse_bool(fluent) else {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "invalid trivial accessor fluent value",
                );
                return None;
            };
            parsed.fluent = fluent;
        } else {
            push_external_fact_error(
                diagnostics,
                line_index,
                "unrecognized trivial accessor metadata; expected policy=... or fluent=...",
            );
            return None;
        }
    }
    Some(parsed)
}

fn push_trivial_getter_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    method_name: &str,
    field_name: &str,
    metadata: &[&str],
) {
    let Some(metadata) = parse_trivial_accessor_metadata(diagnostics, line_index, metadata) else {
        return;
    };
    facts.trivial_accessor_facts.push(TrivialAccessorFact {
        method: MethodKey::named(type_name, method_name),
        kind: TrivialAccessorKind::Getter {
            field: FieldKey::named(type_name, field_name),
        },
        inline_policy: metadata.inline_policy,
        provenance: FactProvenance::UserOverride,
    });
}

fn push_trivial_setter_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    method_name: &str,
    field_name: &str,
    param_name: &str,
    metadata: &[&str],
) {
    let Some(metadata) = parse_trivial_accessor_metadata(diagnostics, line_index, metadata) else {
        return;
    };
    facts.trivial_accessor_facts.push(TrivialAccessorFact {
        method: MethodKey::named(type_name, method_name),
        kind: TrivialAccessorKind::Setter {
            field: FieldKey::named(type_name, field_name),
            param_name: param_name.to_string(),
            fluent: metadata.fluent,
        },
        inline_policy: metadata.inline_policy,
        provenance: FactProvenance::UserOverride,
    });
}

fn push_method_effect_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    method_name: &str,
    receiver_mutability: &str,
    return_nullability: &str,
    metadata: &[&str],
) {
    let Some(receiver_mutability) = parse_receiver_mutability(receiver_mutability) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid method-effect receiver mutability",
        );
        return;
    };
    let Some(return_nullability) = parse_nullability_state(return_nullability) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid method-effect return nullability",
        );
        return;
    };

    let mut thrown = Vec::new();
    for token in metadata {
        let Some(thrown_types) = token.strip_prefix("throws=") else {
            push_external_fact_error(
                diagnostics,
                line_index,
                "unrecognized method-effect metadata; expected throws=TypeA,TypeB",
            );
            return;
        };
        let Some(parsed_thrown) = parse_comma_type_list(
            diagnostics,
            line_index,
            thrown_types,
            "method-effect throws metadata must name at least one Java type",
        ) else {
            return;
        };
        for ty in parsed_thrown {
            if !thrown.contains(&ty) {
                thrown.push(ty);
            }
        }
    }

    facts.method_effect_facts.push(MethodEffectFact {
        method: MethodKey::named(type_name, method_name),
        receiver_mutability,
        return_nullability,
        thrown,
        provenance: FactProvenance::UserOverride,
    });
}

fn push_recursion_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    method_name: &str,
    calls_type: &str,
    calls_method: &str,
    kind: &str,
) {
    let Some(kind) = parse_recursion_kind(kind) else {
        push_external_fact_error(diagnostics, line_index, "invalid recursion kind");
        return;
    };
    facts.recursion_facts.push(RecursionFact {
        method: MethodKey::named(type_name, method_name),
        calls: MethodKey::named(calls_type, calls_method),
        kind,
        provenance: FactProvenance::UserOverride,
    });
}

fn push_control_flow_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    method_name: &str,
    kind: &str,
) {
    let Some(kind) = parse_control_flow_kind(kind) else {
        push_external_fact_error(diagnostics, line_index, "invalid control-flow kind");
        return;
    };
    facts.control_flow_facts.push(ControlFlowFact {
        method: MethodKey::named(type_name, method_name),
        kind,
        provenance: FactProvenance::UserOverride,
    });
}

fn push_catch_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    method_name: &str,
    clause_index: &str,
    catch_types: &str,
    metadata: &[&str],
) {
    let Ok(clause_index) = clause_index.parse::<usize>() else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "catch clause index must be a non-negative integer",
        );
        return;
    };
    let Some(types) = parse_comma_type_list(
        diagnostics,
        line_index,
        catch_types,
        "catch types must name at least one Java type",
    ) else {
        return;
    };

    let mut assignable_from = Vec::new();
    for token in metadata {
        let Some(assignable_types) = token.strip_prefix("assignable=") else {
            push_external_fact_error(
                diagnostics,
                line_index,
                "unrecognized catch metadata; expected assignable=TypeA,TypeB",
            );
            return;
        };
        let Some(parsed_assignable) = parse_comma_type_list(
            diagnostics,
            line_index,
            assignable_types,
            "catch assignable metadata must name at least one Java type",
        ) else {
            return;
        };
        for ty in parsed_assignable {
            if !assignable_from.contains(&ty) {
                assignable_from.push(ty);
            }
        }
    }

    facts.catch_facts.push(CatchFact {
        method: MethodKey::named(type_name, method_name),
        clause_index,
        types,
        assignable_from,
        provenance: FactProvenance::UserOverride,
    });
}

fn push_external_api_effect_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    method_name: &str,
    receiver_type: &str,
    role: &str,
    receiver_mutability: &str,
    return_nullability: &str,
    metadata: &[&str],
) {
    let Some(role) = parse_external_api_role(role) else {
        push_external_fact_error(diagnostics, line_index, "invalid external API role");
        return;
    };
    let Some(receiver_mutability) = parse_receiver_mutability(receiver_mutability) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid external API receiver mutability",
        );
        return;
    };
    let Some(return_nullability) = parse_nullability_state(return_nullability) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid external API return nullability",
        );
        return;
    };
    let Some(parsed_metadata) = parse_external_api_metadata(diagnostics, line_index, metadata)
    else {
        return;
    };
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: method_name.to_string(),
        receiver_type: parse_receiver_type(receiver_type),
        arg_count: parsed_metadata.arg_count,
        parameter_patterns: parsed_metadata.parameter_patterns,
        role,
        receiver_mutability,
        return_type: parsed_metadata.return_type,
        return_nullability,
        thrown: parsed_metadata.thrown,
        provenance: FactProvenance::UserOverride,
    });
}

fn push_external_api_argument_effect_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    method_name: &str,
    receiver_type: &str,
    parameter_index: &str,
    parameter_nullability: &str,
    argument_mutability: &str,
    alias_policy: &str,
    metadata: &[&str],
) {
    let Ok(parameter_index) = parameter_index.parse::<usize>() else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "external-argument index must be a non-negative integer",
        );
        return;
    };
    let Some(parameter_nullability) = parse_nullability_state(parameter_nullability) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid external-argument parameter nullability",
        );
        return;
    };
    let Some(argument_mutability) = parse_receiver_mutability(argument_mutability) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid external-argument mutability",
        );
        return;
    };
    let Some(alias_policy) = parse_external_argument_alias_policy(alias_policy) else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "invalid external-argument alias policy",
        );
        return;
    };
    let Some(parameter_type) = parse_external_argument_metadata(diagnostics, line_index, metadata)
    else {
        return;
    };

    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: method_name.to_string(),
            receiver_type: parse_receiver_type(receiver_type),
            arg_count: None,
            parameter_index,
            parameter_type,
            parameter_nullability,
            argument_mutability,
            alias_policy,
            provenance: FactProvenance::UserOverride,
        });
}

fn parse_optional_arg_count(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    value: &str,
    context: &str,
) -> Option<Option<usize>> {
    match value {
        "*" | "-" | "any" => Some(None),
        _ => match value.parse::<usize>() {
            Ok(count) => Some(Some(count)),
            Err(_) => {
                push_external_fact_error(diagnostics, line_index, context);
                None
            }
        },
    }
}

fn push_external_crate_api_mapping_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    receiver_type: &str,
    method_name: &str,
    arg_count: &str,
    rust_name: &str,
) {
    let Some(arg_count) = parse_optional_arg_count(
        diagnostics,
        line_index,
        arg_count,
        "external-crate-api arg count must be a non-negative integer or *",
    ) else {
        return;
    };
    facts
        .external_crate_api_mapping_facts
        .push(ExternalCrateApiMappingFact {
            receiver_type: receiver_type.to_string(),
            method_name: method_name.to_string(),
            arg_count,
            rust_name: rust_name.to_string(),
            provenance: FactProvenance::UserOverride,
        });
}

fn push_external_crate_constructor_mapping_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    arg_count: &str,
    rust_type_name: &str,
    rust_name: &str,
) {
    let Ok(arg_count) = arg_count.parse::<usize>() else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "external-crate-constructor arg count must be a non-negative integer",
        );
        return;
    };
    facts
        .external_crate_constructor_mapping_facts
        .push(ExternalCrateConstructorMappingFact {
            pack_name: None,
            type_name: type_name.to_string(),
            arg_count,
            rust_type_name: parse_optional_name(rust_type_name),
            rust_name: rust_name.to_string(),
            provenance: FactProvenance::UserOverride,
        });
}

fn push_unsupported_external_constructor_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    pack_name: &str,
    type_name: &str,
    arg_count: &str,
    reason: &[&str],
) {
    let Ok(arg_count) = arg_count.parse::<usize>() else {
        push_external_fact_error(
            diagnostics,
            line_index,
            "unsupported-constructor arg count must be a non-negative integer",
        );
        return;
    };
    facts
        .unsupported_external_constructor_facts
        .push(UnsupportedExternalConstructorFact {
            pack_name: pack_name.to_string(),
            type_name: type_name.to_string(),
            arg_count,
            reason: reason.join(" "),
            provenance: FactProvenance::UserOverride,
        });
}

fn parse_external_argument_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    metadata: &[&str],
) -> Option<Option<JavaType>> {
    let mut parameter_type = None;
    for token in metadata {
        if let Some(type_name) = token.strip_prefix("type=") {
            if type_name.is_empty() {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "external-argument type metadata must name a Java type",
                );
                return None;
            }
            parameter_type = Some(parse_type(type_name));
        } else {
            push_external_fact_error(
                diagnostics,
                line_index,
                "unrecognized external-argument metadata; expected type=Type",
            );
            return None;
        }
    }
    Some(parameter_type)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ExternalApiMetadata {
    arg_count: Option<usize>,
    parameter_patterns: Option<Vec<TypePattern>>,
    return_type: Option<JavaType>,
    thrown: Vec<JavaType>,
}

fn parse_external_api_metadata(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    metadata: &[&str],
) -> Option<ExternalApiMetadata> {
    let mut parsed = ExternalApiMetadata::default();
    for token in metadata {
        if let Some(arg_count) = token.strip_prefix("args=") {
            let Ok(arg_count) = arg_count.parse::<usize>() else {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "external-api args metadata must be a non-negative integer",
                );
                return None;
            };
            parsed.arg_count = Some(arg_count);
        } else if let Some(params) = token.strip_prefix("params=") {
            if params.is_empty() {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "external-api params metadata must name at least one Java type",
                );
                return None;
            }
            let mut patterns = Vec::new();
            for param in params.split(',').filter(|param| !param.is_empty()) {
                let Some(pattern) =
                    parse_external_api_parameter_pattern(diagnostics, line_index, param)
                else {
                    return None;
                };
                patterns.push(pattern);
            }
            if patterns.is_empty() {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "external-api params metadata must name at least one Java type",
                );
                return None;
            }
            if let Some(arg_count) = parsed.arg_count
                && arg_count != patterns.len()
            {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "external-api params metadata length must match args metadata",
                );
                return None;
            }
            parsed.arg_count = Some(patterns.len());
            parsed.parameter_patterns = Some(patterns);
        } else if let Some(return_type) = token.strip_prefix("return=") {
            if return_type.is_empty() {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "external-api return metadata must name a Java type",
                );
                return None;
            }
            parsed.return_type = Some(parse_type(return_type));
        } else if let Some(thrown) = token.strip_prefix("throws=") {
            if thrown.is_empty() {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "external-api throws metadata must name at least one Java type",
                );
                return None;
            }
            let mut parsed_any = false;
            for thrown_type in thrown.split(',').filter(|ty| !ty.is_empty()) {
                parsed_any = true;
                let ty = parse_type(thrown_type);
                if !parsed.thrown.contains(&ty) {
                    parsed.thrown.push(ty);
                }
            }
            if !parsed_any {
                push_external_fact_error(
                    diagnostics,
                    line_index,
                    "external-api throws metadata must name at least one Java type",
                );
                return None;
            }
        } else {
            push_external_fact_error(
                diagnostics,
                line_index,
                "unrecognized external-api metadata; expected args=N, params=TypeA,TypeB, return=Type or throws=TypeA,TypeB",
            );
            return None;
        }
    }
    Some(parsed)
}

fn parse_external_api_parameter_pattern(
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    param: &str,
) -> Option<TypePattern> {
    if let Some(name) = param.strip_prefix("type-param:") {
        if name.is_empty() {
            push_external_fact_error(
                diagnostics,
                line_index,
                "external-api params type-param metadata must name a type parameter",
            );
            return None;
        }
        return Some(TypePattern::TypeParam(name.to_string()));
    }
    Some(crate::facts::type_pattern_for_external_java_type(
        &parse_type(param),
    ))
}

fn push_external_field_fact(
    facts: &mut FactSet,
    type_name: &str,
    field_name: &str,
    field_type: &str,
) {
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey::named(type_name, field_name),
        field_type: Some(parse_type(field_type)),
        field_kind: ExternalFieldKind::Unknown,
        provenance: FactProvenance::UserOverride,
    });
}

fn push_field_nullability_fact(
    facts: &mut FactSet,
    diagnostics: &mut DiagnosticBag,
    line_index: usize,
    type_name: &str,
    field_name: &str,
    state: &str,
) {
    let Some(state) = parse_nullability_state(state) else {
        push_external_fact_error(diagnostics, line_index, "invalid field nullability state");
        return;
    };
    facts.field_nullability_facts.push(FieldNullabilityFact {
        field: FieldKey::named(type_name, field_name),
        state,
        provenance: FieldNullabilityProvenance::ExternalOverride,
    });
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
