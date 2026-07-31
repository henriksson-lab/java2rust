use std::collections::{HashMap, HashSet};

use crate::fact_saturation::{
    FiniteFactDomain, WorklistSaturationPolicy, WorklistStats, saturate_worklist,
};
use crate::java_ir::JavaType;
use crate::slot_shape::TypeCapabilities;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactSet {
    pub notes: Vec<String>,
    pub captures: CaptureFacts,
    pub hierarchy_overrides: Vec<HierarchyOverride>,
    pub closed_hierarchy_facts: Vec<ClosedHierarchyFact>,
    pub sealed_permits_facts: Vec<SealedPermitsFact>,
    pub object_safety_facts: Vec<ObjectSafetyFact>,
    pub type_capability_overrides: Vec<TypeCapabilityOverride>,
    pub owned_recovery_capability_facts: Vec<OwnedRecoveryCapabilityFact>,
    pub field_nullability_facts: Vec<FieldNullabilityFact>,
    pub trivial_accessor_facts: Vec<TrivialAccessorFact>,
    pub external_api_effect_facts: Vec<ExternalApiEffectFact>,
    pub external_api_argument_effect_facts: Vec<ExternalApiArgumentEffectFact>,
    pub external_ffi_handover_facts: Vec<ExternalFfiHandoverFact>,
    pub external_constructor_argument_effect_facts: Vec<ExternalConstructorArgumentEffectFact>,
    pub external_conversion_adapter_facts: Vec<ExternalConversionAdapterFact>,
    pub external_crate_api_mapping_facts: Vec<ExternalCrateApiMappingFact>,
    pub external_crate_type_mapping_facts: Vec<ExternalCrateTypeMappingFact>,
    pub external_crate_constructor_mapping_facts: Vec<ExternalCrateConstructorMappingFact>,
    pub facade_abi_facts: Vec<FacadeAbiFact>,
    pub external_rust_result_facts: Vec<ExternalRustResultFact>,
    pub external_rust_return_type_facts: Vec<ExternalRustReturnTypeFact>,
    pub external_constructor_rust_result_facts: Vec<ExternalConstructorRustResultFact>,
    pub external_constructor_facts: Vec<ExternalConstructorFact>,
    pub external_field_facts: Vec<ExternalFieldFact>,
    pub external_field_value_facts: Vec<ExternalFieldValueFact>,
    pub method_effect_facts: Vec<MethodEffectFact>,
    pub recursion_facts: Vec<RecursionFact>,
    pub constructor_recursion_facts: Vec<ConstructorRecursionFact>,
    pub control_flow_facts: Vec<ControlFlowFact>,
    pub unresolved_target_facts: Vec<UnresolvedTargetFact>,
    pub dispatch_requirement_facts: Vec<DispatchRequirementFact>,
    pub exception_facts: Vec<ExceptionFact>,
    pub catch_facts: Vec<CatchFact>,
    pub external_library_packs: Vec<ExternalLibraryPackFact>,
    pub unsupported_external_api_facts: Vec<UnsupportedExternalApiFact>,
    pub unsupported_external_constructor_facts: Vec<UnsupportedExternalConstructorFact>,
    pub unsupported_external_field_facts: Vec<UnsupportedExternalFieldFact>,
}

#[derive(Debug, Clone)]
pub struct ExternalFactLookupIndex<'a> {
    facts: &'a FactSet,
    external_api_by_method_alias: HashMap<String, Vec<usize>>,
    external_field_by_name_alias: HashMap<String, Vec<usize>>,
    external_pack_member_by_type_alias: HashMap<String, Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExternalApiParameterMatch {
    matches: bool,
    saw_parameter_facts: bool,
    specificity: usize,
}

#[derive(Debug, Clone, Default)]
pub struct HierarchyFactLookupIndex {
    reachable_by_sub_name: HashMap<String, HierarchyReachabilityResult>,
}

impl<'a> ExternalFactLookupIndex<'a> {
    pub fn new(facts: &'a FactSet) -> Self {
        let mut external_api_by_method_alias = HashMap::<String, Vec<usize>>::new();
        for (index, fact) in facts.external_api_effect_facts.iter().enumerate() {
            for alias in external_method_name_aliases(&fact.method_name) {
                external_api_by_method_alias
                    .entry(alias)
                    .or_default()
                    .push(index);
            }
        }

        let mut external_field_by_name_alias = HashMap::<String, Vec<usize>>::new();
        for (index, fact) in facts.external_field_facts.iter().enumerate() {
            for alias in external_field_name_aliases(&fact.field.field_name) {
                external_field_by_name_alias
                    .entry(alias)
                    .or_default()
                    .push(index);
            }
        }

        let mut external_pack_member_by_type_alias = HashMap::<String, Vec<usize>>::new();
        let pack_index_for_type = |type_name: &str| -> Option<usize> {
            facts
                .external_library_packs
                .iter()
                .rposition(|pack| external_pack_owns_type(pack, type_name))
        };
        for fact in &facts.external_api_effect_facts {
            if let Some(receiver_type) = fact.receiver_type.as_deref()
                && let Some(pack_index) = pack_index_for_type(receiver_type)
            {
                index_external_pack_member_type(
                    &mut external_pack_member_by_type_alias,
                    receiver_type,
                    pack_index,
                );
            }
        }
        for fact in &facts.external_constructor_facts {
            if let Some(pack_index) = pack_index_for_type(&fact.type_name) {
                index_external_pack_member_type(
                    &mut external_pack_member_by_type_alias,
                    &fact.type_name,
                    pack_index,
                );
            }
        }
        for fact in &facts.external_field_facts {
            if let Some(pack_index) = pack_index_for_type(&fact.field.type_name) {
                index_external_pack_member_type(
                    &mut external_pack_member_by_type_alias,
                    &fact.field.type_name,
                    pack_index,
                );
            }
        }

        Self {
            facts,
            external_api_by_method_alias,
            external_field_by_name_alias,
            external_pack_member_by_type_alias,
        }
    }

    pub fn external_api_effect_for_receiver(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
    ) -> Option<&'a ExternalApiEffectFact> {
        self.external_api_effect_for_receiver_with_arity(receiver_type, method_name, None)
    }

    pub fn external_api_effect_for_receiver_with_arity(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
    ) -> Option<&'a ExternalApiEffectFact> {
        self.find_external_api_effect(
            receiver_type,
            method_name,
            arg_count,
            Some(FactProvenance::UserOverride),
        )
        .or_else(|| {
            self.find_external_api_effect(
                None,
                method_name,
                arg_count,
                Some(FactProvenance::UserOverride),
            )
        })
        .or_else(|| self.find_external_api_effect(receiver_type, method_name, arg_count, None))
        .or_else(|| self.find_external_api_effect(None, method_name, arg_count, None))
    }

    pub fn external_api_effect_for_call(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_types: &[JavaType],
    ) -> Option<&'a ExternalApiEffectFact> {
        self.find_external_api_effect_for_call(
            receiver_type,
            method_name,
            arg_types,
            Some(FactProvenance::UserOverride),
        )
        .or_else(|| {
            self.find_external_api_effect_for_call(
                None,
                method_name,
                arg_types,
                Some(FactProvenance::UserOverride),
            )
        })
        .or_else(|| {
            self.find_external_api_effect_for_call(receiver_type, method_name, arg_types, None)
        })
        .or_else(|| self.find_external_api_effect_for_call(None, method_name, arg_types, None))
    }

    pub fn external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&'a ExternalFieldFact> {
        self.find_exact_external_field_for(type_name, field_name)
            .or_else(|| self.find_external_field_for(type_name, field_name, true))
            .or_else(|| self.find_external_field_for(type_name, field_name, false))
    }

    pub fn external_library_pack_for_type(
        &self,
        type_name: &str,
    ) -> Option<&'a ExternalLibraryPackFact> {
        self.facts
            .external_library_packs
            .iter()
            .rev()
            .find(|pack| external_pack_directly_owns_type(pack, type_name))
            .or_else(|| {
                self.facts
                    .external_library_packs
                    .iter()
                    .rev()
                    .find(|pack| external_pack_owns_type(pack, type_name))
            })
            .or_else(|| self.external_pack_with_member_for_type(type_name))
    }

    fn find_external_api_effect(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
        provenance: Option<FactProvenance>,
    ) -> Option<&'a ExternalApiEffectFact> {
        if let Some(receiver_type) = receiver_type
            && let Some(effect) = self.find_exact_receiver_external_api_effect(
                receiver_type,
                method_name,
                arg_count,
                provenance,
            )
        {
            return Some(effect);
        }
        if provenance.is_none() {
            if let Some(effect) = arg_count.and_then(|arg_count| {
                self.find_external_api_effect_candidate(method_name, |fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_optional_type_name_matches(
                            fact.receiver_type.as_deref(),
                            receiver_type,
                        )
                        && fact.arg_count == Some(arg_count)
                        && fact.return_type.is_some()
                })
            }) {
                return Some(effect);
            }
            if let Some(effect) = self.find_external_api_effect_candidate(method_name, |fact| {
                external_method_name_matches(&fact.method_name, method_name)
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
                    && (arg_count.is_none() || fact.arg_count.is_none())
                    && fact.return_type.is_some()
            }) {
                return Some(effect);
            }
        }
        arg_count
            .and_then(|arg_count| {
                self.find_external_api_effect_candidate(method_name, |fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_optional_type_name_matches(
                            fact.receiver_type.as_deref(),
                            receiver_type,
                        )
                        && fact.arg_count == Some(arg_count)
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
            .or_else(|| {
                self.find_external_api_effect_candidate(method_name, |fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_optional_type_name_matches(
                            fact.receiver_type.as_deref(),
                            receiver_type,
                        )
                        && (arg_count.is_none() || fact.arg_count.is_none())
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
    }

    fn find_external_api_effect_for_call(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_types: &[JavaType],
        provenance: Option<FactProvenance>,
    ) -> Option<&'a ExternalApiEffectFact> {
        let arg_count = arg_types.len();
        let mut saw_parameter_facts = false;
        let mut best = None::<(&'a ExternalApiEffectFact, (bool, usize, bool))>;
        for index in self.external_api_candidate_indices(method_name) {
            let fact = &self.facts.external_api_effect_facts[index];
            if !external_method_name_matches(&fact.method_name, method_name)
                || !external_optional_type_name_matches(
                    fact.receiver_type.as_deref(),
                    receiver_type,
                )
                || fact.arg_count != Some(arg_count)
                || provenance.is_some_and(|provenance| fact.provenance != provenance)
            {
                continue;
            }
            let match_result = self.external_api_effect_parameter_match(
                fact,
                receiver_type,
                method_name,
                arg_types,
            );
            saw_parameter_facts |= match_result.saw_parameter_facts;
            if match_result.matches {
                let rank = (
                    fact.provenance == FactProvenance::UserOverride,
                    match_result.specificity,
                    fact.return_type.is_some(),
                );
                if best.is_none_or(|(_, best_rank)| rank > best_rank) {
                    best = Some((fact, rank));
                }
            }
        }
        best.map(|(fact, _)| fact).or_else(|| {
            (!saw_parameter_facts)
                .then(|| {
                    self.find_external_api_effect(
                        receiver_type,
                        method_name,
                        Some(arg_count),
                        provenance,
                    )
                })
                .flatten()
        })
    }

    fn external_api_effect_parameter_match(
        &self,
        fact: &ExternalApiEffectFact,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_types: &[JavaType],
    ) -> ExternalApiParameterMatch {
        if let Some(parameter_patterns) = &fact.parameter_patterns {
            return ExternalApiParameterMatch {
                matches: callable_key_params_match_java_types(parameter_patterns, arg_types),
                saw_parameter_facts: true,
                specificity: callable_key_param_specificity(parameter_patterns),
            };
        }
        let mut saw_parameter_facts = false;
        let mut specificity = 0usize;
        for arg_index in 0..arg_types.len() {
            let Some(argument_fact) = self.find_external_api_argument_effect_for_call(
                fact.receiver_type.as_deref().or(receiver_type),
                method_name,
                Some(arg_types.len()),
                arg_index,
            ) else {
                continue;
            };
            let Some(parameter_type) = &argument_fact.parameter_type else {
                continue;
            };
            saw_parameter_facts = true;
            let pattern = external_type_pattern_for_java_type(parameter_type);
            specificity += callable_key_param_specificity(std::slice::from_ref(&pattern));
            if !type_pattern_matches_java_type(&pattern, &arg_types[arg_index]) {
                return ExternalApiParameterMatch {
                    matches: false,
                    saw_parameter_facts,
                    specificity,
                };
            }
        }
        ExternalApiParameterMatch {
            matches: true,
            saw_parameter_facts,
            specificity,
        }
    }

    fn find_external_api_argument_effect_for_call(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
        parameter_index: usize,
    ) -> Option<&'a ExternalApiArgumentEffectFact> {
        self.facts
            .external_api_argument_effect_facts
            .iter()
            .rev()
            .find(|fact| {
                external_method_name_matches(&fact.method_name, method_name)
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
                    && (arg_count.is_none()
                        || fact.arg_count == arg_count
                        || fact.arg_count.is_none())
                    && fact.parameter_index == parameter_index
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.facts
                    .external_api_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_method_name_matches(&fact.method_name, method_name)
                            && external_optional_type_name_matches(
                                fact.receiver_type.as_deref(),
                                receiver_type,
                            )
                            && (arg_count.is_none()
                                || fact.arg_count == arg_count
                                || fact.arg_count.is_none())
                            && fact.parameter_index == parameter_index
                    })
            })
    }

    fn find_exact_receiver_external_api_effect(
        &self,
        receiver_type: &str,
        method_name: &str,
        arg_count: Option<usize>,
        provenance: Option<FactProvenance>,
    ) -> Option<&'a ExternalApiEffectFact> {
        arg_count
            .and_then(|arg_count| {
                self.find_external_api_effect_candidate(method_name, |fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && fact.receiver_type.as_deref() == Some(receiver_type)
                        && fact.arg_count == Some(arg_count)
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
            .or_else(|| {
                self.find_external_api_effect_candidate(method_name, |fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && fact.receiver_type.as_deref() == Some(receiver_type)
                        && (arg_count.is_none() || fact.arg_count.is_none())
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
    }

    fn find_external_api_effect_candidate(
        &self,
        method_name: &str,
        predicate: impl Fn(&ExternalApiEffectFact) -> bool,
    ) -> Option<&'a ExternalApiEffectFact> {
        for index in self.external_api_candidate_indices(method_name) {
            let fact = &self.facts.external_api_effect_facts[index];
            if predicate(fact) {
                return Some(fact);
            }
        }
        None
    }

    fn external_api_candidate_indices(&self, method_name: &str) -> Vec<usize> {
        let mut seen = HashSet::new();
        let mut indices = Vec::new();
        for alias in external_method_name_aliases(method_name) {
            if let Some(alias_indices) = self.external_api_by_method_alias.get(&alias) {
                for index in alias_indices {
                    if seen.insert(*index) {
                        indices.push(*index);
                    }
                }
            }
        }
        indices.sort_by(|left, right| right.cmp(left));
        indices
    }

    fn find_external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
        direct_match_only: bool,
    ) -> Option<&'a ExternalFieldFact> {
        for index in self.external_field_candidate_indices(field_name) {
            let fact = &self.facts.external_field_facts[index];
            let type_matches = if direct_match_only {
                external_type_name_directly_matches(&fact.field.type_name, type_name)
            } else {
                external_type_name_matches(&fact.field.type_name, type_name)
            };
            if type_matches && external_field_name_matches(&fact.field.field_name, field_name) {
                return Some(fact);
            }
        }
        None
    }

    fn find_exact_external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&'a ExternalFieldFact> {
        for index in self.external_field_candidate_indices(field_name) {
            let fact = &self.facts.external_field_facts[index];
            if fact.field.type_name == type_name
                && external_field_name_matches(&fact.field.field_name, field_name)
            {
                return Some(fact);
            }
        }
        None
    }

    fn external_field_candidate_indices(&self, field_name: &str) -> Vec<usize> {
        let mut seen = HashSet::new();
        let mut indices = Vec::new();
        for alias in external_field_name_aliases(field_name) {
            if let Some(alias_indices) = self.external_field_by_name_alias.get(&alias) {
                for index in alias_indices {
                    if seen.insert(*index) {
                        indices.push(*index);
                    }
                }
            }
        }
        indices.sort_by(|left, right| right.cmp(left));
        indices
    }

    fn external_pack_with_member_for_type(
        &self,
        type_name: &str,
    ) -> Option<&'a ExternalLibraryPackFact> {
        if !type_name.contains('.') {
            return None;
        }
        let mut seen = HashSet::new();
        let mut indices = Vec::new();
        for alias in external_type_name_aliases(type_name) {
            if let Some(alias_indices) = self.external_pack_member_by_type_alias.get(&alias) {
                for index in alias_indices {
                    if seen.insert(*index) {
                        indices.push(*index);
                    }
                }
            }
        }
        indices.sort_by(|left, right| right.cmp(left));
        for index in indices {
            let pack = &self.facts.external_library_packs[index];
            if self
                .facts
                .external_pack_has_member_for_type(pack, type_name)
            {
                return Some(pack);
            }
        }
        None
    }
}

impl HierarchyFactLookupIndex {
    pub fn new(facts: &FactSet) -> Self {
        let mut reachable_by_sub_name = HashMap::new();
        for type_name in facts.hierarchy_fact_type_names() {
            let java_ty = JavaType::Named {
                name: type_name.clone(),
                args: Vec::new(),
            };
            reachable_by_sub_name.insert(
                type_name,
                facts.reachable_hierarchy_edges_for_type_with_stats(&java_ty),
            );
        }
        Self {
            reachable_by_sub_name,
        }
    }

    pub fn reachable_hierarchy_edges_for_type_with_stats(
        &self,
        facts: &FactSet,
        java_ty: &JavaType,
    ) -> HierarchyReachabilityResult {
        let Some(sub_name) = java_type_name(java_ty) else {
            return HierarchyReachabilityResult {
                edges: Vec::new(),
                stats: WorklistStats::default(),
                diagnostics: Vec::new(),
            };
        };
        self.reachable_by_sub_name
            .get(sub_name)
            .cloned()
            .unwrap_or_else(|| facts.reachable_hierarchy_edges_for_type_with_stats(java_ty))
    }

    pub fn reachable_hierarchy_edges_for_type(
        &self,
        facts: &FactSet,
        java_ty: &JavaType,
    ) -> Vec<HierarchyEdgeFact> {
        self.reachable_hierarchy_edges_for_type_with_stats(facts, java_ty)
            .edges
    }

    pub fn hierarchy_subtype_policy(
        &self,
        facts: &FactSet,
        subtype: &JavaType,
        supertype: &JavaType,
    ) -> HierarchySubtypePolicy {
        let result = self.reachable_hierarchy_edges_for_type_with_stats(facts, subtype);
        let reaches = match (java_type_name(subtype), java_type_name(supertype)) {
            (Some(sub_name), Some(super_name)) if sub_name == super_name => true,
            (Some(_), Some(super_name)) => result
                .edges
                .iter()
                .any(|edge| edge.super_name == super_name),
            _ => false,
        };
        HierarchySubtypePolicy {
            subtype: subtype.clone(),
            supertype: supertype.clone(),
            reaches,
            diagnostics: result.diagnostics,
        }
    }
}

fn external_method_name_aliases(method_name: &str) -> Vec<String> {
    let mut aliases = crate::java_naming::java_method_name_aliases(method_name);
    aliases.sort();
    aliases.dedup();
    aliases
}

fn index_external_pack_member_type(
    index: &mut HashMap<String, Vec<usize>>,
    type_name: &str,
    pack_index: usize,
) {
    for alias in external_type_name_aliases(type_name) {
        index.entry(alias).or_default().push(pack_index);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyReachabilityResult {
    pub edges: Vec<HierarchyEdgeFact>,
    pub stats: WorklistStats,
    pub diagnostics: Vec<HierarchyReachabilityDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchySubtypePolicy {
    pub subtype: JavaType,
    pub supertype: JavaType,
    pub reaches: bool,
    pub diagnostics: Vec<HierarchyReachabilityDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyReachabilityDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactKind {
    Capture,
    HierarchyOverride,
    ClosedHierarchy,
    SealedPermits,
    ObjectSafety,
    TypeCapability,
    FieldNullability,
    TrivialAccessor,
    MethodEffect,
    ConstructorRecursion,
    ExternalApiEffect,
    ExternalApiArgumentEffect,
    ExternalConstructorArgumentEffect,
    ExternalConversionAdapter,
    ExternalCrateApiMapping,
    ExternalCrateConstructorMapping,
    FacadeAbi,
    ExternalRustReturnType,
    ExternalConstructor,
    ExternalField,
    ControlFlow,
    UnresolvedTarget,
    DispatchRequirement,
    ExceptionHierarchy,
    ExternalLibraryPack,
    UnsupportedExternalApi,
    UnsupportedExternalConstructor,
    UnsupportedExternalField,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeKey {
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberName(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallableKind {
    Method,
    Constructor,
    StaticField,
    InstanceField,
    FieldGetter,
    FieldSetter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Staticness {
    Static,
    Instance,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypePattern {
    Exact(TypeKey),
    Erased(TypeKey),
    TypeParam(String),
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallableKey {
    pub owner: TypeKey,
    pub member: MemberName,
    pub kind: CallableKind,
    pub staticness: Staticness,
    pub arity: usize,
    pub params: Vec<TypePattern>,
    pub generic_arity: Option<usize>,
}

impl CallableKey {
    pub fn method(
        owner: impl Into<String>,
        method: impl Into<String>,
        staticness: Staticness,
        arity: usize,
    ) -> Self {
        Self {
            owner: TypeKey {
                type_name: owner.into(),
            },
            member: MemberName(method.into()),
            kind: CallableKind::Method,
            staticness,
            arity,
            params: vec![TypePattern::Any; arity],
            generic_arity: None,
        }
    }

    pub fn method_with_params(
        owner: impl Into<String>,
        method: impl Into<String>,
        staticness: Staticness,
        params: Vec<TypePattern>,
    ) -> Self {
        let arity = params.len();
        Self {
            owner: TypeKey {
                type_name: owner.into(),
            },
            member: MemberName(method.into()),
            kind: CallableKind::Method,
            staticness,
            arity,
            params,
            generic_arity: None,
        }
    }

    pub fn constructor(owner: impl Into<String>, params: Vec<TypePattern>) -> Self {
        let arity = params.len();
        Self {
            owner: TypeKey {
                type_name: owner.into(),
            },
            member: MemberName("<init>".to_string()),
            kind: CallableKind::Constructor,
            staticness: Staticness::Static,
            arity,
            params,
            generic_arity: None,
        }
    }

    pub fn field(
        owner: impl Into<String>,
        field: impl Into<String>,
        kind: CallableKind,
        staticness: Staticness,
    ) -> Self {
        Self {
            owner: TypeKey {
                type_name: owner.into(),
            },
            member: MemberName(field.into()),
            kind,
            staticness,
            arity: 0,
            params: Vec::new(),
            generic_arity: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldKey {
    pub type_name: String,
    pub field_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodKey {
    pub type_name: String,
    pub method_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstructorKey {
    pub type_name: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullabilityState {
    DefinitelyNonNull,
    MaybeNull,
    DefinitelyNull,
    Unknown,
}

pub fn external_parameter_nullability_policy(state: NullabilityState) -> NullabilityState {
    // External parameter slots are required unless a fact provides positive
    // nullable evidence. Future injected facts should enter before this policy.
    match state {
        NullabilityState::Unknown => NullabilityState::DefinitelyNonNull,
        explicit => explicit,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldNullabilityProvenance {
    ConstructorAssignments,
    FieldInitializer,
    MethodAssignment,
    ExternalOverride,
    ExternalLibraryPack,
    ConservativeFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldNullabilityFact {
    pub field: FieldKey,
    pub state: NullabilityState,
    pub provenance: FieldNullabilityProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrivialAccessorKind {
    Getter {
        field: FieldKey,
    },
    Setter {
        field: FieldKey,
        param_name: String,
        fluent: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessorInlinePolicy {
    PreserveApi,
    InlineCandidate,
    AnalyzeOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrivialAccessorFact {
    pub method: MethodKey,
    pub kind: TrivialAccessorKind,
    pub inline_policy: AccessorInlinePolicy,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverMutabilityFact {
    ReadOnly,
    MutatesReceiver,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalApiRole {
    Getter,
    BooleanGetter,
    Setter,
    Mutator,
    FluentMutator,
    SideEffect,
    IndexedGetter,
    IndexedSetter,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiEffectFact {
    pub method_name: String,
    pub receiver_type: Option<String>,
    pub arg_count: Option<usize>,
    pub parameter_patterns: Option<Vec<TypePattern>>,
    pub role: ExternalApiRole,
    pub receiver_mutability: ReceiverMutabilityFact,
    pub return_type: Option<JavaType>,
    pub return_nullability: NullabilityState,
    pub thrown: Vec<JavaType>,
    pub provenance: FactProvenance,
}

impl ExternalApiEffectFact {
    pub fn callable_key(&self) -> Option<CallableKey> {
        let arg_count = self.arg_count?;
        let params = self
            .parameter_patterns
            .clone()
            .unwrap_or_else(|| vec![TypePattern::Any; arg_count]);
        if params.len() != arg_count {
            return None;
        }
        Some(CallableKey::method_with_params(
            self.receiver_type.clone()?,
            self.method_name.clone(),
            Staticness::Unknown,
            params,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalArgumentAliasPolicy {
    Unknown,
    NoAlias,
    Borrowed,
    Escapes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiArgumentEffectFact {
    pub method_name: String,
    pub receiver_type: Option<String>,
    pub arg_count: Option<usize>,
    pub parameter_index: usize,
    pub parameter_type: Option<JavaType>,
    pub parameter_nullability: NullabilityState,
    pub argument_mutability: ReceiverMutabilityFact,
    pub alias_policy: ExternalArgumentAliasPolicy,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFfiHandoverFact {
    pub method_name: String,
    pub receiver_type: Option<String>,
    pub slot: ExternalFfiHandoverSlot,
    pub parameter_type: Option<JavaType>,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalFfiHandoverSlot {
    Receiver,
    Argument { parameter_index: usize },
}

impl ExternalApiArgumentEffectFact {
    pub fn callable_key(&self) -> Option<CallableKey> {
        Some(CallableKey::method(
            self.receiver_type.clone()?,
            self.method_name.clone(),
            Staticness::Unknown,
            self.arg_count.unwrap_or(self.parameter_index + 1),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalConstructorArgumentEffectFact {
    pub type_name: String,
    pub arg_count: usize,
    pub parameter_index: usize,
    pub parameter_type: Option<JavaType>,
    pub parameter_nullability: NullabilityState,
    pub argument_mutability: ReceiverMutabilityFact,
    pub alias_policy: ExternalArgumentAliasPolicy,
    pub provenance: FactProvenance,
}

impl ExternalConstructorArgumentEffectFact {
    pub fn callable_key(&self) -> CallableKey {
        CallableKey::constructor(
            self.type_name.clone(),
            vec![TypePattern::Any; self.arg_count],
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExternalConversionAdapterKind {
    Into,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalConversionAdapterFact {
    pub from_type: String,
    pub to_type: String,
    pub kind: ExternalConversionAdapterKind,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCrateApiMappingFact {
    pub receiver_type: String,
    pub method_name: String,
    pub arg_count: Option<usize>,
    pub rust_name: String,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCrateTypeMappingFact {
    pub type_name: String,
    pub rust_type_name: String,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCrateConstructorMappingFact {
    pub type_name: String,
    pub arg_count: usize,
    pub rust_type_name: Option<String>,
    pub rust_name: String,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverAbi {
    FreeFunction,
    AssociatedFunction,
    BorrowedSelf,
    MutBorrowedSelf,
    OwnedSelf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FacadeSlot {
    Owned,
    Borrowed,
    MutBorrowed,
    Index,
    Optional(Box<FacadeSlot>),
    Slice(Box<FacadeSlot>),
    DynTrait(String),
    CrateHandle(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultAbi {
    Plain,
    ResultWrapped,
    OptionWrapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowSource {
    Receiver,
    Argument(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedReturnRecovery {
    Clone,
    ToPathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacadeAbiFact {
    pub key: CallableKey,
    pub rust_path: String,
    pub receiver: ReceiverAbi,
    pub args: Vec<FacadeSlot>,
    pub return_slot: FacadeSlot,
    pub result: ResultAbi,
    pub borrow_source: Option<BorrowSource>,
    pub borrowed_return_recovery: Option<BorrowedReturnRecovery>,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRustResultFact {
    pub receiver_type: Option<String>,
    pub method_name: String,
    pub arg_count: Option<usize>,
    pub returns_result: bool,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRustReturnTypeFact {
    pub receiver_type: Option<String>,
    pub method_name: String,
    pub arg_count: Option<usize>,
    pub rust_type_name: String,
    pub return_slot: FacadeSlot,
    pub borrowed_return_recovery: Option<BorrowedReturnRecovery>,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalConstructorRustResultFact {
    pub type_name: String,
    pub arg_count: usize,
    pub returns_result: bool,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalLibraryMode {
    ExternalFactModel,
    RustCrateMapping,
    FfiBoundary,
    ConservativeBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalApiCoverage {
    Declared,
    Conservative,
    Semantic,
    Mapped,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLibraryPackFact {
    pub pack_name: String,
    pub package_prefix: String,
    pub mode: ExternalLibraryMode,
    pub coverage: ExternalApiCoverage,
    pub rust_target: Option<String>,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedExternalApiFact {
    pub pack_name: String,
    pub receiver_type: String,
    pub method_name: String,
    pub reason: String,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedExternalConstructorFact {
    pub pack_name: String,
    pub type_name: String,
    pub arg_count: usize,
    pub reason: String,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedExternalFieldFact {
    pub pack_name: String,
    pub field: FieldKey,
    pub reason: String,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalConstructorFact {
    pub type_name: String,
    pub arg_types: Vec<JavaType>,
    pub provenance: FactProvenance,
}

impl ExternalConstructorFact {
    pub fn callable_key(&self) -> CallableKey {
        CallableKey::constructor(
            self.type_name.clone(),
            self.arg_types
                .iter()
                .map(external_type_pattern_for_java_type)
                .collect(),
        )
    }
}

fn external_type_pattern_for_java_type(ty: &JavaType) -> TypePattern {
    match ty {
        JavaType::Named { name, .. } => TypePattern::Erased(TypeKey {
            type_name: name.clone(),
        }),
        JavaType::Primitive(primitive) => TypePattern::Exact(TypeKey {
            type_name: format!("{primitive:?}").to_ascii_lowercase(),
        }),
        JavaType::String => TypePattern::Exact(TypeKey {
            type_name: "String".to_string(),
        }),
        JavaType::Array(element) => TypePattern::Erased(TypeKey {
            type_name: format!("{}[]", external_type_pattern_key_for_java_type(element)),
        }),
        JavaType::Unknown(_) | JavaType::Wildcard(_) | JavaType::Void => TypePattern::Any,
    }
}

fn external_type_pattern_key_for_java_type(ty: &JavaType) -> String {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.clone(),
        JavaType::Primitive(primitive) => format!("{primitive:?}").to_ascii_lowercase(),
        JavaType::String => "String".to_string(),
        JavaType::Array(element) => {
            format!("{}[]", external_type_pattern_key_for_java_type(element))
        }
        JavaType::Wildcard(_) => "Object".to_string(),
        JavaType::Void => "void".to_string(),
    }
}

pub fn type_pattern_for_external_java_type(ty: &JavaType) -> TypePattern {
    external_type_pattern_for_java_type(ty)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalFieldKind {
    Unknown,
    InstanceField,
    StaticFinal,
    EnumVariant,
    NestedType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFieldFact {
    pub field: FieldKey,
    pub field_type: Option<JavaType>,
    pub field_kind: ExternalFieldKind,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFieldValueFact {
    pub field: FieldKey,
    pub rust_value: String,
    pub provenance: FactProvenance,
}

impl ExternalFieldFact {
    pub fn callable_key(&self) -> CallableKey {
        let (kind, staticness) = match self.field_kind {
            ExternalFieldKind::EnumVariant
            | ExternalFieldKind::NestedType
            | ExternalFieldKind::StaticFinal => (CallableKind::StaticField, Staticness::Static),
            ExternalFieldKind::InstanceField => (CallableKind::InstanceField, Staticness::Instance),
            ExternalFieldKind::Unknown => (CallableKind::InstanceField, Staticness::Unknown),
        };
        CallableKey::field(
            self.field.type_name.clone(),
            self.field.field_name.clone(),
            kind,
            staticness,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodEffectFact {
    pub method: MethodKey,
    pub receiver_mutability: ReceiverMutabilityFact,
    pub return_nullability: NullabilityState,
    pub thrown: Vec<JavaType>,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecursionKind {
    Direct,
    Mutual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursionFact {
    pub method: MethodKey,
    pub calls: MethodKey,
    pub kind: RecursionKind,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorRecursionFact {
    pub constructor: ConstructorKey,
    pub calls: ConstructorKey,
    pub kind: RecursionKind,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlowFactKind {
    AssignmentInCondition,
    TailRecursiveSelfCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFlowFact {
    pub method: MethodKey,
    pub kind: ControlFlowFactKind,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnresolvedTargetKind {
    MethodCall,
    ConstructorCall,
    ConstructorDelegation,
    FieldAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedTargetFact {
    pub enclosing_type: String,
    pub enclosing_member: Option<String>,
    pub kind: UnresolvedTargetKind,
    pub target_name: String,
    pub receiver_type: Option<JavaType>,
    pub arg_types: Vec<JavaType>,
    pub is_anonymous_class: bool,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatchRequirementKind {
    NeedsSubtypeDispatch,
    NeedsClosedRuntimeCaseSplit,
    NeedsInstanceof,
    NeedsDowncast,
    NeedsOwnedPolymorphicStorage,
    NeedsBorrowedTraitView,
    NeedsPublicExtensibleApi,
    RejectTraitObject,
    NeedsUnknownRuntimeObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRequirementFact {
    pub type_name: String,
    pub requirement: DispatchRequirementKind,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionKind {
    Checked,
    Unchecked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionFact {
    pub type_name: String,
    pub kind: ExceptionKind,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatchFact {
    pub method: MethodKey,
    pub clause_index: usize,
    pub types: Vec<JavaType>,
    pub assignable_from: Vec<JavaType>,
    pub provenance: FactProvenance,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
pub struct SealedPermitsFact {
    pub type_name: String,
    pub permitted_subtypes: Vec<String>,
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
    BuiltinExternalModel,
    ExternalLibraryPack,
    CorpusAnalysis,
    UserOverride,
    ConservativeFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedRecoveryCapability {
    Copy,
    ExplicitClone { java_ty: JavaType },
    CloneBoxAdapter { java_ty: JavaType },
    FactoryCopy { java_ty: JavaType },
    ImmutableSharedHandle { java_ty: JavaType },
    FacadeOwnedExternalHandle { java_ty: JavaType },
    FfiOwnedHandle { java_ty: JavaType },
    ConservativeClone,
    NonCloneable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedRecoveryCapabilityFact {
    pub type_name: String,
    pub capability: OwnedRecoveryCapability,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureFacts {
    pub owners: Vec<CaptureOwner>,
}

impl CaptureFacts {
    pub fn outer_this_capture_for_type(&self, type_name: &str) -> Option<&Capture> {
        self.owners
            .iter()
            .filter(|owner| owner.kind == CaptureOwnerKind::InnerClass)
            .filter(|owner| {
                owner.name.as_deref().is_some_and(|name| {
                    crate::stdlib::canonical_external_receiver_type_key(name)
                        == crate::stdlib::canonical_external_receiver_type_key(type_name)
                }) || owner.target_type.as_ref().is_some_and(|target| {
                    java_type_name(target).is_some_and(|target_name| {
                        crate::stdlib::canonical_external_receiver_type_key(target_name)
                            == crate::stdlib::canonical_external_receiver_type_key(type_name)
                    })
                })
            })
            .flat_map(|owner| owner.captures.iter())
            .find(|capture| capture.source == CaptureSource::OuterThis)
    }

    pub fn outer_this_type_for_type(&self, type_name: &str) -> Option<&str> {
        let capture = self.outer_this_capture_for_type(type_name)?;
        java_type_name(&capture.ty)
    }
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
    pub fn external_lookup_index(&self) -> ExternalFactLookupIndex<'_> {
        ExternalFactLookupIndex::new(self)
    }

    pub fn hierarchy_lookup_index(&self) -> HierarchyFactLookupIndex {
        HierarchyFactLookupIndex::new(self)
    }

    pub fn extend_from(&mut self, other: FactSet) {
        self.notes.extend(other.notes);
        self.captures.owners.extend(other.captures.owners);
        self.hierarchy_overrides.extend(other.hierarchy_overrides);
        self.closed_hierarchy_facts
            .extend(other.closed_hierarchy_facts);
        self.sealed_permits_facts.extend(other.sealed_permits_facts);
        self.object_safety_facts.extend(other.object_safety_facts);
        self.type_capability_overrides
            .extend(other.type_capability_overrides);
        self.owned_recovery_capability_facts
            .extend(other.owned_recovery_capability_facts);
        self.field_nullability_facts
            .extend(other.field_nullability_facts);
        self.trivial_accessor_facts
            .extend(other.trivial_accessor_facts);
        self.external_api_effect_facts
            .extend(other.external_api_effect_facts);
        self.external_api_argument_effect_facts
            .extend(other.external_api_argument_effect_facts);
        self.external_ffi_handover_facts
            .extend(other.external_ffi_handover_facts);
        self.external_constructor_argument_effect_facts
            .extend(other.external_constructor_argument_effect_facts);
        self.external_conversion_adapter_facts
            .extend(other.external_conversion_adapter_facts);
        self.external_crate_api_mapping_facts
            .extend(other.external_crate_api_mapping_facts);
        self.external_crate_type_mapping_facts
            .extend(other.external_crate_type_mapping_facts);
        self.external_crate_constructor_mapping_facts
            .extend(other.external_crate_constructor_mapping_facts);
        self.facade_abi_facts.extend(other.facade_abi_facts);
        self.external_rust_result_facts
            .extend(other.external_rust_result_facts);
        self.external_rust_return_type_facts
            .extend(other.external_rust_return_type_facts);
        self.external_constructor_rust_result_facts
            .extend(other.external_constructor_rust_result_facts);
        self.external_constructor_facts
            .extend(other.external_constructor_facts);
        self.external_field_facts.extend(other.external_field_facts);
        self.external_field_value_facts
            .extend(other.external_field_value_facts);
        self.method_effect_facts.extend(other.method_effect_facts);
        self.recursion_facts.extend(other.recursion_facts);
        self.constructor_recursion_facts
            .extend(other.constructor_recursion_facts);
        self.control_flow_facts.extend(other.control_flow_facts);
        self.unresolved_target_facts
            .extend(other.unresolved_target_facts);
        self.dispatch_requirement_facts
            .extend(other.dispatch_requirement_facts);
        self.exception_facts.extend(other.exception_facts);
        self.catch_facts.extend(other.catch_facts);
        self.external_library_packs
            .extend(other.external_library_packs);
        self.unsupported_external_api_facts
            .extend(other.unsupported_external_api_facts);
        self.unsupported_external_constructor_facts
            .extend(other.unsupported_external_constructor_facts);
        self.unsupported_external_field_facts
            .extend(other.unsupported_external_field_facts);
    }

    pub fn field_nullability_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<NullabilityState> {
        self.field_nullability_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_matches(&fact.field.type_name, type_name)
                    && external_field_name_matches(&fact.field.field_name, field_name)
                    && fact.provenance == FieldNullabilityProvenance::ExternalOverride
            })
            .or_else(|| {
                self.field_nullability_facts.iter().rev().find(|fact| {
                    external_type_name_matches(&fact.field.type_name, type_name)
                        && external_field_name_matches(&fact.field.field_name, field_name)
                })
            })
            .map(|fact| fact.state)
    }

    pub fn trivial_accessor_for(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<&TrivialAccessorFact> {
        self.trivial_accessor_facts
            .iter()
            .rev()
            .find(|fact| method_key_matches(&fact.method, type_name, method_name))
    }

    pub fn trivial_getter_field_for(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<&FieldKey> {
        self.trivial_accessor_for(type_name, method_name)
            .and_then(|fact| match &fact.kind {
                TrivialAccessorKind::Getter { field } => Some(field),
                TrivialAccessorKind::Setter { .. } => None,
            })
    }

    pub fn trivial_setter_field_for(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<&FieldKey> {
        self.trivial_accessor_for(type_name, method_name)
            .and_then(|fact| match &fact.kind {
                TrivialAccessorKind::Setter { field, .. } => Some(field),
                TrivialAccessorKind::Getter { .. } => None,
            })
    }

    pub fn external_api_effect_for(&self, method_name: &str) -> Option<&ExternalApiEffectFact> {
        self.external_api_effect_facts
            .iter()
            .rev()
            .find(|fact| {
                fact.method_name == method_name && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_api_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| fact.method_name == method_name)
            })
    }

    pub fn external_api_effect_for_receiver(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
    ) -> Option<&ExternalApiEffectFact> {
        self.external_api_effect_for_receiver_with_arity(receiver_type, method_name, None)
    }

    pub fn external_api_effect_for_key(&self, key: &CallableKey) -> Option<&ExternalApiEffectFact> {
        if key.kind != CallableKind::Method {
            return None;
        }
        self.external_api_effect_facts
            .iter()
            .rev()
            .find(|fact| fact.callable_key().as_ref() == Some(key))
            .or_else(|| {
                self.external_api_effect_for_receiver_with_arity(
                    Some(&key.owner.type_name),
                    &key.member.0,
                    Some(key.arity),
                )
            })
    }

    pub fn external_api_effect_for_receiver_with_arity(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
    ) -> Option<&ExternalApiEffectFact> {
        self.find_external_api_effect(
            receiver_type,
            method_name,
            arg_count,
            Some(FactProvenance::UserOverride),
        )
        .or_else(|| {
            self.find_external_api_effect(
                None,
                method_name,
                arg_count,
                Some(FactProvenance::UserOverride),
            )
        })
        .or_else(|| self.find_external_api_effect(receiver_type, method_name, arg_count, None))
        .or_else(|| self.find_external_api_effect(None, method_name, arg_count, None))
    }

    pub fn external_api_effect_for_call(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_types: &[JavaType],
    ) -> Option<&ExternalApiEffectFact> {
        let index = ExternalFactLookupIndex::new(self);
        index.external_api_effect_for_call(receiver_type, method_name, arg_types)
    }

    fn find_external_api_effect(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
        provenance: Option<FactProvenance>,
    ) -> Option<&ExternalApiEffectFact> {
        if let Some(receiver_type) = receiver_type
            && let Some(effect) = self.find_exact_receiver_external_api_effect(
                receiver_type,
                method_name,
                arg_count,
                provenance,
            )
        {
            return Some(effect);
        }
        if provenance.is_none() {
            if let Some(effect) = arg_count.and_then(|arg_count| {
                self.external_api_effect_facts.iter().rev().find(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_optional_type_name_matches(
                            fact.receiver_type.as_deref(),
                            receiver_type,
                        )
                        && fact.arg_count == Some(arg_count)
                        && fact.return_type.is_some()
                })
            }) {
                return Some(effect);
            }
            if let Some(effect) = self.external_api_effect_facts.iter().rev().find(|fact| {
                external_method_name_matches(&fact.method_name, method_name)
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
                    && (arg_count.is_none() || fact.arg_count.is_none())
                    && fact.return_type.is_some()
            }) {
                return Some(effect);
            }
        }
        arg_count
            .and_then(|arg_count| {
                self.external_api_effect_facts.iter().rev().find(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_optional_type_name_matches(
                            fact.receiver_type.as_deref(),
                            receiver_type,
                        )
                        && fact.arg_count == Some(arg_count)
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
            .or_else(|| {
                self.external_api_effect_facts.iter().rev().find(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_optional_type_name_matches(
                            fact.receiver_type.as_deref(),
                            receiver_type,
                        )
                        && (arg_count.is_none() || fact.arg_count.is_none())
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
    }

    fn find_exact_receiver_external_api_effect(
        &self,
        receiver_type: &str,
        method_name: &str,
        arg_count: Option<usize>,
        provenance: Option<FactProvenance>,
    ) -> Option<&ExternalApiEffectFact> {
        arg_count
            .and_then(|arg_count| {
                self.external_api_effect_facts.iter().rev().find(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && fact.receiver_type.as_deref() == Some(receiver_type)
                        && fact.arg_count == Some(arg_count)
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
            .or_else(|| {
                self.external_api_effect_facts.iter().rev().find(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && fact.receiver_type.as_deref() == Some(receiver_type)
                        && (arg_count.is_none() || fact.arg_count.is_none())
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
    }

    pub fn external_api_argument_effect_for_receiver(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        parameter_index: usize,
    ) -> Option<&ExternalApiArgumentEffectFact> {
        self.external_api_argument_effect_for_receiver_with_arity(
            receiver_type,
            method_name,
            None,
            parameter_index,
        )
    }

    pub fn external_api_argument_effect_for_receiver_with_arity(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
        parameter_index: usize,
    ) -> Option<&ExternalApiArgumentEffectFact> {
        let arity_matches = |fact: &ExternalApiArgumentEffectFact| match arg_count {
            Some(arg_count) => fact.arg_count == Some(arg_count),
            None => fact.arg_count.is_none(),
        };
        let generic_arity_matches =
            |fact: &ExternalApiArgumentEffectFact| arg_count.is_some() && fact.arg_count.is_none();
        self.external_api_argument_effect_facts
            .iter()
            .rev()
            .find(|fact| {
                external_method_name_matches(&fact.method_name, method_name)
                    && arity_matches(fact)
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
                    && fact.parameter_index == parameter_index
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_api_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_method_name_matches(&fact.method_name, method_name)
                            && arity_matches(fact)
                            && fact.receiver_type.is_none()
                            && fact.parameter_index == parameter_index
                            && fact.provenance == FactProvenance::UserOverride
                    })
            })
            .or_else(|| {
                self.external_api_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_method_name_matches(&fact.method_name, method_name)
                            && arity_matches(fact)
                            && external_optional_type_name_matches(
                                fact.receiver_type.as_deref(),
                                receiver_type,
                            )
                            && fact.parameter_index == parameter_index
                    })
            })
            .or_else(|| {
                self.external_api_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_method_name_matches(&fact.method_name, method_name)
                            && arity_matches(fact)
                            && fact.receiver_type.is_none()
                            && fact.parameter_index == parameter_index
                    })
            })
            .or_else(|| {
                self.external_api_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_method_name_matches(&fact.method_name, method_name)
                            && generic_arity_matches(fact)
                            && external_optional_type_name_matches(
                                fact.receiver_type.as_deref(),
                                receiver_type,
                            )
                            && fact.parameter_index == parameter_index
                            && fact.provenance == FactProvenance::UserOverride
                    })
            })
            .or_else(|| {
                self.external_api_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_method_name_matches(&fact.method_name, method_name)
                            && generic_arity_matches(fact)
                            && fact.receiver_type.is_none()
                            && fact.parameter_index == parameter_index
                            && fact.provenance == FactProvenance::UserOverride
                    })
            })
            .or_else(|| {
                self.external_api_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_method_name_matches(&fact.method_name, method_name)
                            && generic_arity_matches(fact)
                            && external_optional_type_name_matches(
                                fact.receiver_type.as_deref(),
                                receiver_type,
                            )
                            && fact.parameter_index == parameter_index
                    })
            })
            .or_else(|| {
                self.external_api_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_method_name_matches(&fact.method_name, method_name)
                            && generic_arity_matches(fact)
                            && fact.receiver_type.is_none()
                            && fact.parameter_index == parameter_index
                    })
            })
            .or_else(|| {
                if arg_count.is_some() {
                    return None;
                }
                self.unambiguous_explicit_arity_external_api_argument_effect(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_optional_type_name_matches(
                            fact.receiver_type.as_deref(),
                            receiver_type,
                        )
                        && fact.parameter_index == parameter_index
                        && fact.provenance == FactProvenance::UserOverride
                })
            })
            .or_else(|| {
                if arg_count.is_some() {
                    return None;
                }
                self.unambiguous_explicit_arity_external_api_argument_effect(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && fact.receiver_type.is_none()
                        && fact.parameter_index == parameter_index
                        && fact.provenance == FactProvenance::UserOverride
                })
            })
            .or_else(|| {
                if arg_count.is_some() {
                    return None;
                }
                self.unambiguous_explicit_arity_external_api_argument_effect(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_optional_type_name_matches(
                            fact.receiver_type.as_deref(),
                            receiver_type,
                        )
                        && fact.parameter_index == parameter_index
                })
            })
            .or_else(|| {
                if arg_count.is_some() {
                    return None;
                }
                self.unambiguous_explicit_arity_external_api_argument_effect(|fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && fact.receiver_type.is_none()
                        && fact.parameter_index == parameter_index
                })
            })
    }

    fn unambiguous_explicit_arity_external_api_argument_effect(
        &self,
        matches: impl Fn(&ExternalApiArgumentEffectFact) -> bool,
    ) -> Option<&ExternalApiArgumentEffectFact> {
        let mut selected = None;
        for fact in self.external_api_argument_effect_facts.iter().rev() {
            if fact.arg_count.is_none() || !matches(fact) {
                continue;
            }
            let Some(current) = selected else {
                selected = Some(fact);
                continue;
            };
            if !external_api_argument_effects_agree(current, fact) {
                return None;
            }
        }
        selected
    }

    pub fn external_api_argument_effect_for_key(
        &self,
        key: &CallableKey,
        parameter_index: usize,
    ) -> Option<&ExternalApiArgumentEffectFact> {
        if key.kind != CallableKind::Method || parameter_index >= key.arity {
            return None;
        }
        self.external_api_argument_effect_facts
            .iter()
            .rev()
            .find(|fact| {
                fact.callable_key().is_some_and(|fact_key| {
                    fact_key.owner == key.owner
                        && fact_key.member == key.member
                        && fact_key.kind == key.kind
                        && fact.parameter_index == parameter_index
                })
            })
            .or_else(|| {
                self.external_api_argument_effect_for_receiver_with_arity(
                    Some(&key.owner.type_name),
                    &key.member.0,
                    Some(key.arity),
                    parameter_index,
                )
            })
    }

    pub fn external_ffi_argument_handover_for_receiver(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        parameter_index: usize,
        parameter_type: &JavaType,
    ) -> Option<&ExternalFfiHandoverFact> {
        self.external_ffi_handover_facts
            .iter()
            .rev()
            .find(|fact| {
                fact.method_name == method_name
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
                    && fact.slot == ExternalFfiHandoverSlot::Argument { parameter_index }
                    && fact
                        .parameter_type
                        .as_ref()
                        .is_none_or(|fact_ty| fact_ty == parameter_type)
            })
            .or_else(|| {
                self.external_ffi_handover_facts.iter().rev().find(|fact| {
                    fact.method_name == method_name
                        && fact.receiver_type.is_none()
                        && fact.slot == ExternalFfiHandoverSlot::Argument { parameter_index }
                        && fact
                            .parameter_type
                            .as_ref()
                            .is_none_or(|fact_ty| fact_ty == parameter_type)
                })
            })
    }

    pub fn external_ffi_receiver_handover_for_receiver(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        receiver_java_ty: &JavaType,
    ) -> Option<&ExternalFfiHandoverFact> {
        self.external_ffi_handover_facts
            .iter()
            .rev()
            .find(|fact| {
                fact.method_name == method_name
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
                    && fact.slot == ExternalFfiHandoverSlot::Receiver
                    && fact
                        .parameter_type
                        .as_ref()
                        .is_none_or(|fact_ty| fact_ty == receiver_java_ty)
            })
            .or_else(|| {
                self.external_ffi_handover_facts.iter().rev().find(|fact| {
                    fact.method_name == method_name
                        && fact.receiver_type.is_none()
                        && fact.slot == ExternalFfiHandoverSlot::Receiver
                        && fact
                            .parameter_type
                            .as_ref()
                            .is_none_or(|fact_ty| fact_ty == receiver_java_ty)
                })
            })
    }

    pub fn external_constructor_argument_effect_for(
        &self,
        type_name: &str,
        arg_count: usize,
        parameter_index: usize,
    ) -> Option<&ExternalConstructorArgumentEffectFact> {
        self.external_constructor_argument_effect_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_matches(&fact.type_name, type_name)
                    && fact.arg_count == arg_count
                    && fact.parameter_index == parameter_index
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_constructor_argument_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.type_name, type_name)
                            && fact.arg_count == arg_count
                            && fact.parameter_index == parameter_index
                    })
            })
    }

    pub fn external_conversion_adapter_for(
        &self,
        from_type: &str,
        to_type: &str,
    ) -> Option<&ExternalConversionAdapterFact> {
        self.external_conversion_adapter_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_matches(&fact.from_type, from_type)
                    && external_type_name_matches(&fact.to_type, to_type)
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_conversion_adapter_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.from_type, from_type)
                            && external_type_name_matches(&fact.to_type, to_type)
                    })
            })
    }

    pub fn facade_abi_for_key(&self, key: &CallableKey) -> Option<&FacadeAbiFact> {
        self.facade_abi_facts
            .iter()
            .rev()
            .find(|fact| &fact.key == key && fact.provenance == FactProvenance::UserOverride)
            .or_else(|| {
                self.facade_abi_facts
                    .iter()
                    .rev()
                    .find(|fact| &fact.key == key)
            })
    }

    pub fn facade_abi_for_call(
        &self,
        key: &CallableKey,
        arg_types: &[JavaType],
    ) -> Option<&FacadeAbiFact> {
        self.facade_abi_facts
            .iter()
            .rev()
            .filter(|fact| callable_key_header_matches(&fact.key, key))
            .filter(|fact| callable_key_params_match_java_types(&fact.key.params, arg_types))
            .max_by_key(|fact| {
                (
                    fact.provenance == FactProvenance::UserOverride,
                    callable_key_param_specificity(&fact.key.params),
                )
            })
    }

    pub fn external_crate_api_mapping_for_receiver(
        &self,
        receiver_type: &str,
        method_name: &str,
        arg_count: Option<usize>,
    ) -> Option<&ExternalCrateApiMappingFact> {
        self.external_crate_api_mapping_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_directly_matches(&fact.receiver_type, receiver_type)
                    && fact.method_name == method_name
                    && fact.arg_count == arg_count
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_crate_api_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_directly_matches(&fact.receiver_type, receiver_type)
                            && fact.method_name == method_name
                            && fact.arg_count.is_none()
                            && fact.provenance == FactProvenance::UserOverride
                    })
            })
            .or_else(|| {
                self.external_crate_api_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.receiver_type, receiver_type)
                            && fact.method_name == method_name
                            && fact.arg_count == arg_count
                            && fact.provenance == FactProvenance::UserOverride
                    })
            })
            .or_else(|| {
                self.external_crate_api_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.receiver_type, receiver_type)
                            && fact.method_name == method_name
                            && fact.arg_count.is_none()
                            && fact.provenance == FactProvenance::UserOverride
                    })
            })
            .or_else(|| {
                self.external_crate_api_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_directly_matches(&fact.receiver_type, receiver_type)
                            && fact.method_name == method_name
                            && fact.arg_count == arg_count
                    })
            })
            .or_else(|| {
                self.external_crate_api_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_directly_matches(&fact.receiver_type, receiver_type)
                            && fact.method_name == method_name
                            && fact.arg_count.is_none()
                    })
            })
            .or_else(|| {
                self.external_crate_api_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.receiver_type, receiver_type)
                            && fact.method_name == method_name
                            && fact.arg_count == arg_count
                    })
            })
            .or_else(|| {
                self.external_crate_api_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.receiver_type, receiver_type)
                            && fact.method_name == method_name
                            && fact.arg_count.is_none()
                    })
            })
    }

    pub fn external_crate_constructor_mapping_for(
        &self,
        type_name: &str,
        arg_count: usize,
    ) -> Option<&ExternalCrateConstructorMappingFact> {
        self.external_crate_constructor_mapping_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_directly_matches(&fact.type_name, type_name)
                    && fact.arg_count == arg_count
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_crate_constructor_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.type_name, type_name)
                            && fact.arg_count == arg_count
                            && fact.provenance == FactProvenance::UserOverride
                    })
            })
            .or_else(|| {
                self.external_crate_constructor_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_directly_matches(&fact.type_name, type_name)
                            && fact.arg_count == arg_count
                    })
            })
            .or_else(|| {
                self.external_crate_constructor_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.type_name, type_name)
                            && fact.arg_count == arg_count
                    })
            })
    }

    pub fn external_crate_rust_type_name_for(&self, type_name: &str) -> Option<&str> {
        self.external_crate_type_mapping_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_directly_matches(&fact.type_name, type_name)
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_crate_type_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| external_type_name_matches(&fact.type_name, type_name))
            })
            .map(|fact| fact.rust_type_name.as_str())
            .or_else(|| {
                self.external_crate_constructor_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_directly_matches(&fact.type_name, type_name)
                            && fact.rust_type_name.is_some()
                            && fact.provenance == FactProvenance::UserOverride
                    })
                    .or_else(|| {
                        self.external_crate_constructor_mapping_facts
                            .iter()
                            .rev()
                            .find(|fact| {
                                external_type_name_matches(&fact.type_name, type_name)
                                    && fact.rust_type_name.is_some()
                            })
                    })
                    .and_then(|fact| fact.rust_type_name.as_deref())
            })
    }

    pub fn external_crate_constructor_rust_type_name_for(&self, type_name: &str) -> Option<&str> {
        self.external_crate_constructor_mapping_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_directly_matches(&fact.type_name, type_name)
                    && fact.rust_type_name.is_some()
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_crate_constructor_mapping_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.type_name, type_name)
                            && fact.rust_type_name.is_some()
                    })
            })
            .and_then(|fact| fact.rust_type_name.as_deref())
    }

    pub fn external_rust_result_for_receiver(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
    ) -> Option<&ExternalRustResultFact> {
        self.find_external_rust_result(receiver_type, method_name, arg_count, true)
            .or_else(|| self.find_external_rust_result(None, method_name, arg_count, true))
            .or_else(|| {
                self.find_external_rust_result(receiver_type, method_name, arg_count, false)
            })
            .or_else(|| self.find_external_rust_result(None, method_name, arg_count, false))
    }

    fn find_external_rust_result(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
        user_override_only: bool,
    ) -> Option<&ExternalRustResultFact> {
        self.external_rust_result_facts.iter().rev().find(|fact| {
            fact.method_name == method_name
                && fact.arg_count == arg_count
                && (!user_override_only || fact.provenance == FactProvenance::UserOverride)
                && external_optional_type_name_matches(fact.receiver_type.as_deref(), receiver_type)
        })
    }

    pub fn external_rust_return_type_for_receiver(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
    ) -> Option<&ExternalRustReturnTypeFact> {
        self.find_external_rust_return_type(receiver_type, method_name, arg_count, true)
            .or_else(|| self.find_external_rust_return_type(None, method_name, arg_count, true))
            .or_else(|| {
                self.find_external_rust_return_type(receiver_type, method_name, arg_count, false)
            })
            .or_else(|| self.find_external_rust_return_type(None, method_name, arg_count, false))
    }

    fn find_external_rust_return_type(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
        user_override_only: bool,
    ) -> Option<&ExternalRustReturnTypeFact> {
        self.external_rust_return_type_facts
            .iter()
            .rev()
            .find(|fact| {
                fact.method_name == method_name
                    && fact.arg_count == arg_count
                    && (!user_override_only || fact.provenance == FactProvenance::UserOverride)
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
            })
    }

    pub fn external_constructor_rust_result_for(
        &self,
        type_name: &str,
        arg_count: usize,
    ) -> Option<&ExternalConstructorRustResultFact> {
        self.external_constructor_rust_result_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_matches(&fact.type_name, type_name)
                    && fact.arg_count == arg_count
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.external_constructor_rust_result_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.type_name, type_name)
                            && fact.arg_count == arg_count
                    })
            })
    }

    pub fn external_constructor_for(
        &self,
        type_name: &str,
        arg_types: &[JavaType],
    ) -> Option<&ExternalConstructorFact> {
        self.find_external_constructor_for(type_name, arg_types, true)
            .or_else(|| self.find_external_constructor_for(type_name, arg_types, false))
    }

    pub fn external_constructor_for_key(
        &self,
        key: &CallableKey,
    ) -> Option<&ExternalConstructorFact> {
        if key.kind != CallableKind::Constructor {
            return None;
        }
        self.external_constructor_facts
            .iter()
            .rev()
            .find(|fact| &fact.callable_key() == key)
            .or_else(|| {
                let arg_types = vec![JavaType::Unknown("_".to_string()); key.arity];
                self.external_constructor_for(&key.owner.type_name, &arg_types)
            })
    }

    pub fn external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&ExternalFieldFact> {
        self.find_exact_external_field_for(type_name, field_name)
            .or_else(|| self.find_external_field_for(type_name, field_name, true))
            .or_else(|| self.find_external_field_for(type_name, field_name, false))
    }

    pub fn external_field_value_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&ExternalFieldValueFact> {
        self.external_field_value_facts
            .iter()
            .rev()
            .find(|fact| {
                fact.field.type_name == type_name
                    && external_field_name_matches(&fact.field.field_name, field_name)
            })
            .or_else(|| {
                self.external_field_value_facts.iter().rev().find(|fact| {
                    external_type_name_directly_matches(&fact.field.type_name, type_name)
                        && external_field_name_matches(&fact.field.field_name, field_name)
                })
            })
            .or_else(|| {
                self.external_field_value_facts.iter().rev().find(|fact| {
                    external_type_name_matches(&fact.field.type_name, type_name)
                        && external_field_name_matches(&fact.field.field_name, field_name)
                })
            })
    }

    pub fn external_field_for_key(&self, key: &CallableKey) -> Option<&ExternalFieldFact> {
        if !matches!(
            key.kind,
            CallableKind::StaticField
                | CallableKind::InstanceField
                | CallableKind::FieldGetter
                | CallableKind::FieldSetter
        ) {
            return None;
        }
        self.external_field_facts
            .iter()
            .rev()
            .find(|fact| &fact.callable_key() == key)
            .or_else(|| self.external_field_for(&key.owner.type_name, &key.member.0))
    }

    fn find_external_constructor_for(
        &self,
        type_name: &str,
        arg_types: &[JavaType],
        direct_match_only: bool,
    ) -> Option<&ExternalConstructorFact> {
        self.external_constructor_facts.iter().rev().find(|fact| {
            let type_matches = if direct_match_only {
                external_type_name_directly_matches(&fact.type_name, type_name)
            } else {
                external_type_name_matches(&fact.type_name, type_name)
            };
            type_matches
                && (fact.arg_types == arg_types
                    || fact.provenance == FactProvenance::UserOverride
                    || (fact.provenance == FactProvenance::ExternalLibraryPack
                        && fact.arg_types.len() == arg_types.len()))
        })
    }

    fn find_external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
        direct_match_only: bool,
    ) -> Option<&ExternalFieldFact> {
        self.external_field_facts.iter().rev().find(|fact| {
            let type_matches = if direct_match_only {
                external_type_name_directly_matches(&fact.field.type_name, type_name)
            } else {
                external_type_name_matches(&fact.field.type_name, type_name)
            };
            type_matches && external_field_name_matches(&fact.field.field_name, field_name)
        })
    }

    fn find_exact_external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&ExternalFieldFact> {
        self.external_field_facts.iter().rev().find(|fact| {
            fact.field.type_name == type_name
                && external_field_name_matches(&fact.field.field_name, field_name)
        })
    }

    pub fn external_library_pack_for_type(
        &self,
        type_name: &str,
    ) -> Option<&ExternalLibraryPackFact> {
        self.external_library_packs
            .iter()
            .rev()
            .find(|pack| external_pack_directly_owns_type(pack, type_name))
            .or_else(|| {
                self.external_library_packs
                    .iter()
                    .rev()
                    .find(|pack| external_pack_owns_type(pack, type_name))
            })
            .or_else(|| {
                self.external_library_packs
                    .iter()
                    .rev()
                    .find(|pack| self.external_pack_has_member_for_type(pack, type_name))
            })
    }

    fn external_pack_has_member_for_type(
        &self,
        pack: &ExternalLibraryPackFact,
        type_name: &str,
    ) -> bool {
        if !type_name.contains('.') {
            return false;
        }
        self.external_api_effect_facts.iter().any(|fact| {
            fact.receiver_type.as_deref().is_some_and(|receiver_type| {
                external_type_name_matches(receiver_type, type_name)
                    && external_pack_owns_type(pack, receiver_type)
            })
        }) || self.external_constructor_facts.iter().any(|fact| {
            external_type_name_matches(&fact.type_name, type_name)
                && external_pack_owns_type(pack, &fact.type_name)
        }) || self.external_field_facts.iter().any(|fact| {
            external_type_name_matches(&fact.field.type_name, type_name)
                && external_pack_owns_type(pack, &fact.field.type_name)
        })
    }

    pub fn unsupported_external_api_for_receiver(
        &self,
        receiver_type: &str,
        method_name: &str,
    ) -> Option<&UnsupportedExternalApiFact> {
        self.unsupported_external_api_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_directly_matches(&fact.receiver_type, receiver_type)
                    && fact.method_name == method_name
            })
            .or_else(|| {
                self.unsupported_external_api_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.receiver_type, receiver_type)
                            && fact.method_name == method_name
                    })
            })
    }

    pub fn unsupported_external_constructor_for(
        &self,
        type_name: &str,
        arg_count: usize,
    ) -> Option<&UnsupportedExternalConstructorFact> {
        self.unsupported_external_constructor_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_directly_matches(&fact.type_name, type_name)
                    && fact.arg_count == arg_count
            })
            .or_else(|| {
                self.unsupported_external_constructor_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.type_name, type_name)
                            && fact.arg_count == arg_count
                    })
            })
    }

    pub fn unsupported_external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&UnsupportedExternalFieldFact> {
        self.unsupported_external_field_facts
            .iter()
            .rev()
            .find(|fact| {
                external_type_name_directly_matches(&fact.field.type_name, type_name)
                    && external_field_name_matches(&fact.field.field_name, field_name)
            })
            .or_else(|| {
                self.unsupported_external_field_facts
                    .iter()
                    .rev()
                    .find(|fact| {
                        external_type_name_matches(&fact.field.type_name, type_name)
                            && external_field_name_matches(&fact.field.field_name, field_name)
                    })
            })
    }

    pub fn recursion_facts_for(&self, type_name: &str, method_name: &str) -> Vec<&RecursionFact> {
        self.recursion_facts
            .iter()
            .filter(|fact| method_key_matches(&fact.method, type_name, method_name))
            .collect()
    }

    pub fn constructor_recursion_facts_for(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Vec<&ConstructorRecursionFact> {
        self.constructor_recursion_facts
            .iter()
            .filter(|fact| constructor_key_matches(&fact.constructor, type_name, rust_name))
            .collect()
    }

    pub fn method_effect_for(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<&MethodEffectFact> {
        self.method_effect_facts
            .iter()
            .rev()
            .find(|fact| {
                method_key_matches(&fact.method, type_name, method_name)
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                self.method_effect_facts
                    .iter()
                    .rev()
                    .find(|fact| method_key_matches(&fact.method, type_name, method_name))
            })
    }

    pub fn method_has_thrown_exceptions(&self, type_name: &str, method_name: &str) -> bool {
        self.method_effect_for(type_name, method_name)
            .is_some_and(|fact| self.thrown_types_include_checked_exception(&fact.thrown))
    }

    pub fn thrown_types_include_checked_exception(&self, thrown: &[JavaType]) -> bool {
        thrown.iter().any(|ty| {
            java_type_name(ty).and_then(|name| self.exception_kind_for(name))
                != Some(ExceptionKind::Unchecked)
        })
    }

    pub fn has_method_effects(&self) -> bool {
        self.method_effect_facts.iter().any(|fact| {
            fact.receiver_mutability != ReceiverMutabilityFact::Unknown
                || fact.return_nullability != NullabilityState::Unknown
                || !fact.thrown.is_empty()
        })
    }

    pub fn has_method_throw_effects(&self) -> bool {
        self.method_effect_facts
            .iter()
            .any(|fact| !fact.thrown.is_empty())
    }

    pub fn has_control_flow_fact(
        &self,
        type_name: &str,
        method_name: &str,
        kind: ControlFlowFactKind,
    ) -> bool {
        self.control_flow_facts.iter().any(|fact| {
            method_key_matches(&fact.method, type_name, method_name) && fact.kind == kind
        })
    }

    pub fn dispatch_requirements_for(&self, type_name: &str) -> Vec<DispatchRequirementKind> {
        let mut requirements = Vec::new();
        for fact in &self.dispatch_requirement_facts {
            if external_type_name_matches(&fact.type_name, type_name)
                && !requirements.contains(&fact.requirement)
            {
                requirements.push(fact.requirement);
            }
        }
        requirements
    }

    pub fn dispatch_requirement_facts_for(&self, type_name: &str) -> Vec<&DispatchRequirementFact> {
        self.dispatch_requirement_facts
            .iter()
            .filter(|fact| external_type_name_matches(&fact.type_name, type_name))
            .collect()
    }

    pub fn exception_kind_for(&self, type_name: &str) -> Option<ExceptionKind> {
        self.exception_facts
            .iter()
            .rev()
            .find(|fact| {
                exception_type_name_matches(&fact.type_name, type_name)
                    && fact.kind != ExceptionKind::Unknown
            })
            .or_else(|| {
                self.exception_facts
                    .iter()
                    .rev()
                    .find(|fact| exception_type_name_matches(&fact.type_name, type_name))
            })
            .map(|fact| fact.kind)
    }

    pub fn catch_facts_for(&self, type_name: &str, method_name: &str) -> Vec<&CatchFact> {
        self.catch_facts
            .iter()
            .filter(|fact| method_key_matches(&fact.method, type_name, method_name))
            .collect()
    }

    pub fn type_capabilities_for(&self, java_ty: &JavaType) -> TypeCapabilities {
        crate::effective_facts::EffectiveFacts::new(self).type_capabilities_for(java_ty)
    }

    pub fn sequence_element_type_for(&self, java_ty: &JavaType) -> Option<JavaType> {
        crate::effective_facts::EffectiveFacts::new(self).sequence_element_type_for(java_ty)
    }

    pub fn is_copy_type(&self, java_ty: &JavaType) -> bool {
        crate::effective_facts::EffectiveFacts::new(self).is_copy_type(java_ty)
    }

    pub fn is_clone_type(&self, java_ty: &JavaType) -> bool {
        crate::effective_facts::EffectiveFacts::new(self).is_clone_type(java_ty)
    }

    pub fn explicit_clone_capability_for(&self, java_ty: &JavaType) -> Option<bool> {
        crate::effective_facts::EffectiveFacts::new(self).explicit_clone_capability_for(java_ty)
    }

    pub fn is_string_like_type(&self, java_ty: &JavaType) -> bool {
        crate::effective_facts::EffectiveFacts::new(self).is_string_like_type(java_ty)
    }

    pub fn can_recover_owned_from_borrowed(&self, java_ty: &JavaType) -> bool {
        crate::effective_facts::EffectiveFacts::new(self).can_recover_owned_from_borrowed(java_ty)
    }

    pub fn owned_recovery_capability_for(&self, java_ty: &JavaType) -> OwnedRecoveryCapability {
        crate::effective_facts::EffectiveFacts::new(self).owned_recovery_capability_for(java_ty)
    }

    pub fn owned_recovery_capability_fact_for(
        &self,
        java_ty: &JavaType,
    ) -> Option<&OwnedRecoveryCapabilityFact> {
        crate::effective_facts::EffectiveFacts::new(self)
            .owned_recovery_capability_fact_for(java_ty)
    }

    pub fn object_safe_trait_fact_for_name(&self, type_name: &str) -> Option<bool> {
        crate::effective_facts::EffectiveFacts::new(self).object_safe_trait_fact_for_name(type_name)
    }

    pub fn object_safe_trait_fact_for(&self, java_ty: &JavaType) -> Option<bool> {
        crate::effective_facts::EffectiveFacts::new(self).object_safe_trait_fact_for(java_ty)
    }

    pub fn object_safety_fact_for_name(&self, type_name: &str) -> Option<&ObjectSafetyFact> {
        crate::effective_facts::EffectiveFacts::new(self).object_safety_fact_for_name(type_name)
    }

    pub fn object_safety_violation_for_name(
        &self,
        type_name: &str,
    ) -> Option<ObjectSafetyViolation> {
        crate::effective_facts::EffectiveFacts::new(self)
            .object_safety_violation_for_name(type_name)
    }

    pub fn has_object_representation_facts(&self, java_ty: &JavaType) -> bool {
        crate::effective_facts::EffectiveFacts::new(self).has_object_representation_facts(java_ty)
    }

    pub fn extends_type_for(&self, java_ty: &JavaType) -> Option<JavaType> {
        crate::effective_facts::EffectiveFacts::new(self).extends_type_for(java_ty)
    }

    pub fn implemented_types_for(&self, java_ty: &JavaType) -> Vec<JavaType> {
        crate::effective_facts::EffectiveFacts::new(self).implemented_types_for(java_ty)
    }

    pub fn cast_type_capability_policy(
        &self,
        actual: &JavaType,
        target: &JavaType,
    ) -> crate::effective_facts::CastTypeCapabilityPolicy {
        crate::effective_facts::EffectiveFacts::new(self)
            .cast_type_capability_policy(actual, target)
    }

    pub fn instanceof_target_capability_policy(
        &self,
        target: &JavaType,
    ) -> crate::effective_facts::InstanceOfTargetCapabilityPolicy {
        crate::effective_facts::EffectiveFacts::new(self)
            .instanceof_target_capability_policy(target)
    }

    pub fn is_final_type(&self, java_ty: &JavaType) -> bool {
        crate::effective_facts::EffectiveFacts::new(self).is_final_type(java_ty)
    }

    pub fn hierarchy_edges_for_type(&self, java_ty: &JavaType) -> Vec<HierarchyEdgeFact> {
        crate::effective_facts::EffectiveFacts::new(self).hierarchy_edges_for_type(java_ty)
    }

    pub fn reachable_hierarchy_edges_for_type(&self, java_ty: &JavaType) -> Vec<HierarchyEdgeFact> {
        self.reachable_hierarchy_edges_for_type_with_stats(java_ty)
            .edges
    }

    pub fn hierarchy_subtype_policy(
        &self,
        subtype: &JavaType,
        supertype: &JavaType,
    ) -> HierarchySubtypePolicy {
        let result = self.reachable_hierarchy_edges_for_type_with_stats(subtype);
        let reaches = match (java_type_name(subtype), java_type_name(supertype)) {
            (Some(sub_name), Some(super_name)) if sub_name == super_name => true,
            (Some(_), Some(super_name)) => result
                .edges
                .iter()
                .any(|edge| edge.super_name == super_name),
            _ => false,
        };
        HierarchySubtypePolicy {
            subtype: subtype.clone(),
            supertype: supertype.clone(),
            reaches,
            diagnostics: result.diagnostics,
        }
    }

    pub fn reachable_hierarchy_edges_for_type_with_stats(
        &self,
        java_ty: &JavaType,
    ) -> HierarchyReachabilityResult {
        let Some(sub_name) = java_type_name(java_ty).map(str::to_string) else {
            return HierarchyReachabilityResult {
                edges: Vec::new(),
                stats: WorklistStats::default(),
                diagnostics: Vec::new(),
            };
        };
        let initial_edges = self.direct_hierarchy_edges_for_type(java_ty);
        let domain = self.hierarchy_reachability_domain(&sub_name);
        let policy = WorklistSaturationPolicy::new(domain.iteration_budget());
        let mut result = saturate_worklist(
            initial_edges.clone(),
            initial_edges,
            policy,
            |edges, edge, inserter| {
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
                        edges.push(reachable_edge.clone());
                        inserter.insert(reachable_edge);
                    }
                }
            },
            |stats| HierarchyReachabilityDiagnostic {
                code: "hierarchy_reachability_budget_exhausted",
                message: format!(
                    "hierarchy reachability exceeded finite worklist budget after {} processed items",
                    stats.processed_items
                ),
            },
        );
        result.state.sort_by(|left, right| {
            left.super_name
                .cmp(&right.super_name)
                .then_with(|| left.sub_name.cmp(&right.sub_name))
        });
        HierarchyReachabilityResult {
            edges: result.state,
            stats: result.stats,
            diagnostics: result.diagnostics,
        }
    }

    pub fn hierarchy_reachability_domain(&self, root_type_name: &str) -> FiniteFactDomain {
        let type_count = self.hierarchy_type_names(root_type_name).len();
        FiniteFactDomain::new(type_count, type_count)
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
        names.extend(self.hierarchy_fact_type_names());
        names
    }

    fn hierarchy_fact_type_names(&self) -> HashSet<String> {
        let mut names = HashSet::new();
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
        crate::effective_facts::EffectiveFacts::new(self).closed_hierarchy_fact_for_name(type_name)
    }

    pub fn closed_hierarchy_variants_for(
        &self,
        type_name: &str,
        direct_subtypes: &[String],
    ) -> Option<Vec<String>> {
        crate::effective_facts::EffectiveFacts::new(self)
            .closed_hierarchy_variants_for(type_name, direct_subtypes)
    }

    pub fn sealed_permitted_subtypes_for(&self, type_name: &str) -> Option<Vec<String>> {
        crate::effective_facts::EffectiveFacts::new(self)
            .sealed_permitted_subtypes_for(type_name)
            .map(|fact| fact.value)
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

pub(crate) fn external_pack_owns_type(pack: &ExternalLibraryPackFact, type_name: &str) -> bool {
    external_pack_directly_owns_type(pack, type_name)
        || external_type_name_directly_matches(&pack.package_prefix, type_name)
}

fn external_pack_directly_owns_type(pack: &ExternalLibraryPackFact, type_name: &str) -> bool {
    type_name == pack.package_prefix
        || type_name
            .strip_prefix(&pack.package_prefix)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

pub fn external_type_name_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    let fact_aliases = external_type_name_aliases(fact_type_name);
    let query_aliases = external_type_name_aliases(query_type_name);
    fact_aliases.iter().any(|fact_alias| {
        query_aliases
            .iter()
            .any(|query_alias| query_alias == fact_alias)
    })
}

fn callable_key_header_matches(fact: &CallableKey, query: &CallableKey) -> bool {
    external_type_name_matches(&fact.owner.type_name, &query.owner.type_name)
        && fact.member == query.member
        && fact.kind == query.kind
        && (fact.staticness == query.staticness
            || fact.staticness == Staticness::Unknown
            || query.staticness == Staticness::Unknown)
        && fact.arity == query.arity
        && (fact.generic_arity.is_none()
            || query.generic_arity.is_none()
            || fact.generic_arity == query.generic_arity)
}

fn callable_key_params_match_java_types(params: &[TypePattern], arg_types: &[JavaType]) -> bool {
    params.len() == arg_types.len()
        && params
            .iter()
            .zip(arg_types)
            .all(|(pattern, arg_ty)| type_pattern_matches_java_type(pattern, arg_ty))
}

fn callable_key_param_specificity(params: &[TypePattern]) -> usize {
    params
        .iter()
        .map(|pattern| match pattern {
            TypePattern::Any => 0,
            TypePattern::TypeParam(_) => 1,
            TypePattern::Erased(_) => 2,
            TypePattern::Exact(_) => 3,
        })
        .sum()
}

pub fn callable_type_pattern_specificity(pattern: &TypePattern) -> usize {
    callable_key_param_specificity(std::slice::from_ref(pattern))
}

pub fn callable_type_patterns_specificity(patterns: &[TypePattern]) -> usize {
    callable_key_param_specificity(patterns)
}

fn type_pattern_matches_java_type(pattern: &TypePattern, java_ty: &JavaType) -> bool {
    if let Some(element_ty) = crate::types::optional_element_type(java_ty)
        && type_pattern_matches_java_type(pattern, &element_ty)
    {
        return true;
    }
    match pattern {
        TypePattern::Any | TypePattern::TypeParam(_) => true,
        TypePattern::Exact(type_key) => type_key_matches_java_type(&type_key.type_name, java_ty),
        TypePattern::Erased(type_key) => {
            let erased = erase_java_type_for_pattern(java_ty);
            type_key_matches_java_type(&type_key.type_name, &erased)
        }
    }
}

pub fn type_pattern_matches_external_java_type(pattern: &TypePattern, java_ty: &JavaType) -> bool {
    type_pattern_matches_java_type(pattern, java_ty)
}

pub fn type_patterns_match_external_java_types(
    patterns: &[TypePattern],
    java_types: &[JavaType],
) -> bool {
    callable_key_params_match_java_types(patterns, java_types)
}

fn erase_java_type_for_pattern(java_ty: &JavaType) -> JavaType {
    match java_ty {
        JavaType::Named { name, .. } => JavaType::Named {
            name: name.clone(),
            args: Vec::new(),
        },
        JavaType::Array(element) => JavaType::Array(Box::new(erase_java_type_for_pattern(element))),
        other => other.clone(),
    }
}

fn type_key_matches_java_type(type_name: &str, java_ty: &JavaType) -> bool {
    let normalized = type_name.trim();
    if let Some(element_name) = normalized.strip_suffix("[]") {
        return matches!(
            java_ty,
            JavaType::Array(element) if type_key_matches_java_type(element_name, element)
        );
    }
    match java_ty {
        JavaType::Primitive(primitive) => primitive_type_name_matches(normalized, *primitive),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            external_type_name_matches(normalized, name)
        }
        JavaType::Array(_) => false,
        JavaType::Void => matches!(normalized, "void" | "Void" | "()"),
        JavaType::String => {
            external_type_name_matches(normalized, "String")
                || external_type_name_matches(normalized, "java.lang.String")
        }
        JavaType::Wildcard(_) => matches!(normalized, "Object" | "java.lang.Object"),
    }
}

fn primitive_type_name_matches(type_name: &str, primitive: crate::java_ir::PrimitiveType) -> bool {
    let java_name = match primitive {
        crate::java_ir::PrimitiveType::Boolean => "boolean",
        crate::java_ir::PrimitiveType::Byte => "byte",
        crate::java_ir::PrimitiveType::Short => "short",
        crate::java_ir::PrimitiveType::Char => "char",
        crate::java_ir::PrimitiveType::Int => "int",
        crate::java_ir::PrimitiveType::Long => "long",
        crate::java_ir::PrimitiveType::Float => "float",
        crate::java_ir::PrimitiveType::Double => "double",
    };
    type_name.eq_ignore_ascii_case(java_name)
}

fn external_optional_type_name_matches(
    fact_type_name: Option<&str>,
    query_type_name: Option<&str>,
) -> bool {
    match (fact_type_name, query_type_name) {
        (Some(fact_type_name), Some(query_type_name)) => {
            external_type_name_matches(fact_type_name, query_type_name)
        }
        (None, None) => true,
        _ => false,
    }
}

fn external_method_name_matches(fact_method_name: &str, query_method_name: &str) -> bool {
    let fact_aliases = external_method_name_aliases(fact_method_name);
    external_method_name_aliases(query_method_name)
        .iter()
        .any(|query_alias| {
            fact_aliases
                .iter()
                .any(|fact_alias| fact_alias == query_alias)
        })
}

fn external_api_argument_effects_agree(
    first: &ExternalApiArgumentEffectFact,
    second: &ExternalApiArgumentEffectFact,
) -> bool {
    first.parameter_type == second.parameter_type
        && first.parameter_nullability == second.parameter_nullability
        && first.argument_mutability == second.argument_mutability
        && first.alias_policy == second.alias_policy
        && first.provenance == second.provenance
}

fn external_type_name_aliases(type_name: &str) -> Vec<String> {
    let mut aliases = external_direct_type_name_aliases(type_name);
    let canonical = crate::stdlib::canonical_external_receiver_type_key(type_name);
    if let Some(simple) = package_qualified_simple_alias(type_name) {
        aliases.extend([
            simple.to_string(),
            crate::stdlib::canonical_external_receiver_type_key(simple),
            crate::parser::to_snake_case(simple),
        ]);
    }
    if let Some(canonical_simple) = package_qualified_simple_alias(&canonical) {
        aliases.extend([
            canonical_simple.to_string(),
            crate::stdlib::canonical_external_receiver_type_key(canonical_simple),
            crate::parser::to_snake_case(canonical_simple),
        ]);
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn exception_type_name_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    if external_type_name_matches(fact_type_name, query_type_name) {
        return true;
    }
    let fact_aliases = nested_type_suffix_aliases(fact_type_name);
    let query_aliases = nested_type_suffix_aliases(query_type_name);
    fact_aliases.iter().any(|fact_alias| {
        query_aliases
            .iter()
            .any(|query_alias| query_alias == fact_alias)
            || external_type_name_matches(fact_alias, query_type_name)
    }) || query_aliases
        .iter()
        .any(|query_alias| external_type_name_matches(fact_type_name, query_alias))
}

fn nested_type_suffix_aliases(type_name: &str) -> Vec<String> {
    let segments = type_name.split('.').collect::<Vec<_>>();
    let Some(first_type_segment) = segments
        .iter()
        .position(|segment| segment.chars().next().is_some_and(char::is_uppercase))
    else {
        return Vec::new();
    };
    if first_type_segment + 1 >= segments.len() {
        return Vec::new();
    }
    let suffix = segments[first_type_segment..].join(".");
    let simple = segments.last().copied().unwrap_or(&suffix);
    vec![
        suffix.clone(),
        crate::stdlib::canonical_external_receiver_type_key(&suffix),
        crate::parser::to_snake_case(&suffix),
        simple.to_string(),
        crate::stdlib::canonical_external_receiver_type_key(simple),
        crate::parser::to_snake_case(simple),
    ]
}

fn package_qualified_simple_alias(type_name: &str) -> Option<&str> {
    let (prefix, simple) = type_name.rsplit_once('.')?;
    if prefix
        .split('.')
        .any(|segment| segment.chars().next().is_some_and(char::is_lowercase))
    {
        Some(simple)
    } else {
        None
    }
}

fn external_type_name_directly_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    let fact_aliases = external_direct_type_name_aliases(fact_type_name);
    let query_aliases = external_direct_type_name_aliases(query_type_name);
    fact_aliases.iter().any(|fact_alias| {
        query_aliases
            .iter()
            .any(|query_alias| query_alias == fact_alias)
    })
}

fn external_direct_type_name_aliases(type_name: &str) -> Vec<String> {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(type_name);
    let snake = crate::parser::to_snake_case(type_name);
    let mut aliases = vec![type_name.to_string(), canonical, snake];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn external_field_name_matches(fact_field_name: &str, query_field_name: &str) -> bool {
    let fact_aliases = external_field_name_aliases(fact_field_name);
    let query_aliases = external_field_name_aliases(query_field_name);
    fact_aliases.iter().any(|fact_alias| {
        query_aliases
            .iter()
            .any(|query_alias| query_alias == fact_alias)
    })
}

fn external_field_name_aliases(field_name: &str) -> Vec<String> {
    let canonical = crate::parser::to_snake_case(field_name);
    let mut aliases = vec![field_name.to_string(), canonical];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn method_key_matches(method: &MethodKey, type_name: &str, method_name: &str) -> bool {
    external_type_name_matches(&method.type_name, type_name)
        && (method.method_name == method_name
            || crate::parser::to_snake_case(&method.method_name) == method_name)
}

fn constructor_key_matches(constructor: &ConstructorKey, type_name: &str, rust_name: &str) -> bool {
    external_type_name_matches(&constructor.type_name, type_name)
        && constructor.rust_name == rust_name
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
