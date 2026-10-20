use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::fact_saturation::{
    FiniteFactDomain, WorklistSaturationPolicy, WorklistStats, saturate_worklist,
};
use crate::java_ir::JavaType;
use crate::slot_shape::TypeCapabilities;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactSet {
    /// Selects the generated `JavaString` carrier for a translation unit.
    ///
    /// This is deliberately a whole-unit representation choice: a Java String
    /// may cross fields, methods, collections, and erased-object boundaries,
    /// so changing only the comparison expression would leave a mixed Rust
    /// carrier and silently lose reference identity.
    pub java_string_identity_representation: bool,
    /// How values of each source type are *held* — by value, or behind a shared
    /// handle.
    ///
    /// It lives here rather than on `RepresentationPlan` for one reason: the
    /// Rust type of a source class is decided in **49 places** that take
    /// `(shape, facts)` and only 11 that also take the representation plan, and
    /// `item_assembly.rs` — struct fields, parameters, return types — is in the
    /// first group. A storage decision only lowering could see would let the
    /// two disagree about a type's Rust spelling, which does not compile.
    ///
    /// Step 3 of
    /// [0002b](../claude_audits/0002b-shared-representation-design.md).
    pub storage_strategies: crate::storage_strategy::StoragePlan,
    pub notes: Vec<String>,
    pub captures: CaptureFacts,
    pub hierarchy_overrides: Vec<HierarchyOverride>,
    pub closed_hierarchy_facts: Vec<ClosedHierarchyFact>,
    pub sealed_permits_facts: Vec<SealedPermitsFact>,
    pub object_safety_facts: Vec<ObjectSafetyFact>,
    pub source_interface_upcast_facts: Vec<SourceInterfaceUpcastFact>,
    pub generic_parameter_variance_facts: Vec<GenericParameterVarianceFact>,
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
    pub source_equals_override_facts: Vec<SourceEqualsOverrideFact>,
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
    pub disable_builtin_external_packs: bool,
}

impl Default for FactSet {
    fn default() -> Self {
        Self {
            java_string_identity_representation: false,
            storage_strategies: crate::storage_strategy::StoragePlan::default(),
            notes: Vec::new(),
            captures: CaptureFacts::default(),
            hierarchy_overrides: Vec::new(),
            closed_hierarchy_facts: Vec::new(),
            sealed_permits_facts: Vec::new(),
            object_safety_facts: Vec::new(),
            source_interface_upcast_facts: Vec::new(),
            generic_parameter_variance_facts: Vec::new(),
            type_capability_overrides: Vec::new(),
            owned_recovery_capability_facts: Vec::new(),
            field_nullability_facts: Vec::new(),
            trivial_accessor_facts: Vec::new(),
            external_api_effect_facts: Vec::new(),
            external_api_argument_effect_facts: Vec::new(),
            external_ffi_handover_facts: Vec::new(),
            external_constructor_argument_effect_facts: Vec::new(),
            external_conversion_adapter_facts: Vec::new(),
            external_crate_api_mapping_facts: Vec::new(),
            external_crate_type_mapping_facts: Vec::new(),
            external_crate_constructor_mapping_facts: Vec::new(),
            facade_abi_facts: Vec::new(),
            external_rust_result_facts: Vec::new(),
            external_rust_return_type_facts: Vec::new(),
            external_constructor_rust_result_facts: Vec::new(),
            external_constructor_facts: Vec::new(),
            external_field_facts: Vec::new(),
            external_field_value_facts: Vec::new(),
            source_equals_override_facts: Vec::new(),
            method_effect_facts: Vec::new(),
            recursion_facts: Vec::new(),
            constructor_recursion_facts: Vec::new(),
            control_flow_facts: Vec::new(),
            unresolved_target_facts: Vec::new(),
            dispatch_requirement_facts: Vec::new(),
            exception_facts: Vec::new(),
            catch_facts: Vec::new(),
            external_library_packs: Vec::new(),
            unsupported_external_api_facts: Vec::new(),
            unsupported_external_constructor_facts: Vec::new(),
            unsupported_external_field_facts: Vec::new(),
            disable_builtin_external_packs: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExternalFactLookupIndex<'a> {
    facts: &'a FactSet,
    external_api_by_method_alias: RefCell<Option<HashMap<String, Vec<usize>>>>,
    external_api_candidates_by_query: RefCell<HashMap<String, Rc<[usize]>>>,
    external_field_by_name_alias: RefCell<Option<HashMap<String, Vec<usize>>>>,
    external_field_value_by_name_alias: RefCell<Option<HashMap<String, Vec<usize>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExternalApiParameterMatch {
    matches: bool,
    saw_parameter_facts: bool,
    specificity: usize,
}

#[derive(Debug, Clone)]
pub struct HierarchyFactLookupIndex<'a> {
    facts: &'a FactSet,
    reachable_by_sub_name: RefCell<HashMap<String, HierarchyReachabilityResult>>,
}

#[cfg(test)]
thread_local! {
    static HIERARCHY_LOOKUP_INDEX_BUILD_COUNT: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
pub(crate) fn reset_hierarchy_lookup_index_build_count() {
    HIERARCHY_LOOKUP_INDEX_BUILD_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn hierarchy_lookup_index_build_count() -> usize {
    HIERARCHY_LOOKUP_INDEX_BUILD_COUNT.with(std::cell::Cell::get)
}

impl<'a> ExternalFactLookupIndex<'a> {
    pub fn new(facts: &'a FactSet) -> Self {
        Self {
            facts,
            external_api_by_method_alias: RefCell::new(None),
            external_api_candidates_by_query: RefCell::new(HashMap::new()),
            external_field_by_name_alias: RefCell::new(None),
            external_field_value_by_name_alias: RefCell::new(None),
        }
    }

    fn ensure_external_api_alias_index(&self) {
        if self.external_api_by_method_alias.borrow().is_some() {
            return;
        }
        let mut by_alias = HashMap::<String, Vec<usize>>::new();
        for (index, fact) in self.facts.external_api_effect_facts.iter().enumerate() {
            for alias in external_method_name_aliases(&fact.method_name).iter() {
                by_alias.entry(alias.clone()).or_default().push(index);
            }
        }
        *self.external_api_by_method_alias.borrow_mut() = Some(by_alias);
    }

    fn ensure_external_field_alias_index(&self) {
        if self.external_field_by_name_alias.borrow().is_some() {
            return;
        }
        let mut by_alias = HashMap::<String, Vec<usize>>::new();
        for (index, fact) in self.facts.external_field_facts.iter().enumerate() {
            for alias in external_field_name_aliases(&fact.field.field_name) {
                by_alias.entry(alias).or_default().push(index);
            }
        }
        *self.external_field_by_name_alias.borrow_mut() = Some(by_alias);
    }

    fn ensure_external_field_value_alias_index(&self) {
        if self.external_field_value_by_name_alias.borrow().is_some() {
            return;
        }
        let mut by_alias = HashMap::<String, Vec<usize>>::new();
        for (index, fact) in self.facts.external_field_value_facts.iter().enumerate() {
            for alias in external_field_name_aliases(&fact.field.field_name) {
                by_alias.entry(alias).or_default().push(index);
            }
        }
        *self.external_field_value_by_name_alias.borrow_mut() = Some(by_alias);
    }

    pub fn facts(&self) -> &'a FactSet {
        self.facts
    }

    /// Visits only external API facts whose Java/Rust method aliases can
    /// match `method_name`. Callers still own receiver and arity policy; this
    /// is the shared candidate index, not a second resolution implementation.
    pub(crate) fn visit_external_api_effect_candidates(
        &self,
        method_name: &str,
        mut visit: impl FnMut(&'a ExternalApiEffectFact),
    ) {
        for index in self.external_api_candidate_indices(method_name).iter() {
            visit(&self.facts.external_api_effect_facts[*index]);
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
        .filter(|fact| {
            receiver_type.is_none_or(|owner| {
                self.facts
                    .model_fact_applies_to_owner(owner, fact.provenance)
            })
        })
    }

    pub(crate) fn external_api_effect_for_mapped_receiver_with_arity(
        &self,
        method_name: &str,
        arg_count: usize,
        receiver_matches: impl Fn(&str) -> bool,
    ) -> Option<&'a ExternalApiEffectFact> {
        // Candidate indices are stored newest-first for normal override lookup.
        // This compatibility lookup historically scanned the fact set forwards,
        // so reverse them here to preserve its first-match precedence exactly.
        self.external_api_candidate_indices(method_name)
            .iter()
            .rev()
            .map(|index| &self.facts.external_api_effect_facts[*index])
            .find(|fact| {
                fact.method_name == method_name
                    && fact.arg_count.is_none_or(|arity| arity == arg_count)
                    && fact.receiver_type.as_deref().is_some_and(&receiver_matches)
            })
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

    pub fn external_field_value_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&'a ExternalFieldValueFact> {
        self.find_exact_external_field_value_for(type_name, field_name)
            .or_else(|| self.find_external_field_value_for(type_name, field_name, true))
            .or_else(|| self.find_external_field_value_for(type_name, field_name, false))
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

    pub fn external_rust_crate_mapping_pack_for_type(
        &self,
        type_name: &str,
    ) -> Option<&'a ExternalLibraryPackFact> {
        self.facts
            .external_library_packs
            .iter()
            .rev()
            .find(|pack| {
                pack.mode == ExternalLibraryMode::RustCrateMapping
                    && external_pack_directly_owns_type(pack, type_name)
            })
            .or_else(|| {
                self.facts.external_library_packs.iter().rev().find(|pack| {
                    pack.mode == ExternalLibraryMode::RustCrateMapping
                        && external_pack_owns_type(pack, type_name)
                })
            })
            .or_else(|| {
                self.external_pack_with_member_for_type(type_name)
                    .filter(|pack| pack.mode == ExternalLibraryMode::RustCrateMapping)
            })
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
            let receiver_ambiguous = self.external_api_effect_simple_owner_alias_is_ambiguous(
                receiver_type,
                method_name,
                |fact| fact.arg_count == arg_count && fact.return_type.is_some(),
                None,
            );
            if let Some(effect) = arg_count.and_then(|arg_count| {
                self.find_external_api_effect_candidate(method_name, |fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_api_effect_receiver_matches_for_fact(
                            fact,
                            receiver_type,
                            receiver_ambiguous,
                            self.external_pack_directly_owns_receiver(receiver_type),
                        )
                        && fact.arg_count == Some(arg_count)
                        && fact.return_type.is_some()
                })
            }) {
                return Some(effect);
            }
            let receiver_ambiguous = self.external_api_effect_simple_owner_alias_is_ambiguous(
                receiver_type,
                method_name,
                |fact| {
                    (arg_count.is_none() || fact.arg_count.is_none()) && fact.return_type.is_some()
                },
                None,
            );
            if let Some(effect) = self.find_external_api_effect_candidate(method_name, |fact| {
                external_method_name_matches(&fact.method_name, method_name)
                    && external_api_effect_receiver_matches_for_fact(
                        fact,
                        receiver_type,
                        receiver_ambiguous,
                        self.external_pack_directly_owns_receiver(receiver_type),
                    )
                    && (arg_count.is_none() || fact.arg_count.is_none())
                    && fact.return_type.is_some()
            }) {
                return Some(effect);
            }
        }
        arg_count
            .and_then(|arg_count| {
                let receiver_ambiguous = self.external_api_effect_simple_owner_alias_is_ambiguous(
                    receiver_type,
                    method_name,
                    |fact| fact.arg_count == Some(arg_count),
                    provenance,
                );
                self.find_external_api_effect_candidate(method_name, |fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_api_effect_receiver_matches_for_fact(
                            fact,
                            receiver_type,
                            receiver_ambiguous,
                            self.external_pack_directly_owns_receiver(receiver_type),
                        )
                        && fact.arg_count == Some(arg_count)
                        && provenance.is_none_or(|provenance| fact.provenance == provenance)
                })
            })
            .or_else(|| {
                let receiver_ambiguous = self.external_api_effect_simple_owner_alias_is_ambiguous(
                    receiver_type,
                    method_name,
                    |fact| arg_count.is_none() || fact.arg_count.is_none(),
                    provenance,
                );
                self.find_external_api_effect_candidate(method_name, |fact| {
                    external_method_name_matches(&fact.method_name, method_name)
                        && external_api_effect_receiver_matches_for_fact(
                            fact,
                            receiver_type,
                            receiver_ambiguous,
                            self.external_pack_directly_owns_receiver(receiver_type),
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
        let receiver_ambiguous = self.external_api_effect_simple_owner_alias_is_ambiguous(
            receiver_type,
            method_name,
            |fact| fact.arg_count == Some(arg_count),
            provenance,
        );
        for index in self
            .external_api_candidate_indices(method_name)
            .iter()
            .copied()
        {
            let fact = &self.facts.external_api_effect_facts[index];
            if !external_method_name_matches(&fact.method_name, method_name)
                || !external_api_effect_receiver_matches_for_fact(
                    fact,
                    receiver_type,
                    receiver_ambiguous,
                    self.external_pack_directly_owns_receiver(receiver_type),
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
        let arity_matches = |fact: &ExternalApiArgumentEffectFact| {
            arg_count.is_none() || fact.arg_count == arg_count || fact.arg_count.is_none()
        };
        find_external_api_argument_effect_for_receiver(
            &self.facts.external_api_argument_effect_facts,
            receiver_type,
            method_name,
            arity_matches,
            parameter_index,
            Some(FactProvenance::UserOverride),
        )
        .or_else(|| {
            find_external_api_argument_effect_for_receiver(
                &self.facts.external_api_argument_effect_facts,
                receiver_type,
                method_name,
                arity_matches,
                parameter_index,
                None,
            )
        })
        .filter(|fact| {
            receiver_type.is_none_or(|owner| {
                self.facts
                    .model_fact_applies_to_owner(owner, fact.provenance)
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
        for index in self
            .external_api_candidate_indices(method_name)
            .iter()
            .copied()
        {
            let fact = &self.facts.external_api_effect_facts[index];
            if predicate(fact) {
                return Some(fact);
            }
        }
        None
    }

    fn external_pack_directly_owns_receiver(&self, receiver_type: Option<&str>) -> bool {
        receiver_type.is_some_and(|receiver_type| {
            self.facts
                .external_library_packs
                .iter()
                .any(|pack| external_pack_directly_owns_type(pack, receiver_type))
        })
    }

    fn external_api_effect_simple_owner_alias_is_ambiguous(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        signature_matches: impl Fn(&ExternalApiEffectFact) -> bool,
        provenance: Option<FactProvenance>,
    ) -> bool {
        let Some(receiver_type) = receiver_type else {
            return false;
        };
        package_simple_owner_alias_is_ambiguous(
            self.external_api_candidate_indices(method_name)
                .iter()
                .filter_map(|index| {
                    let fact = &self.facts.external_api_effect_facts[*index];
                    (external_method_name_matches(&fact.method_name, method_name)
                        && signature_matches(fact)
                        && provenance.is_none_or(|provenance| fact.provenance == provenance))
                    .then_some(fact.receiver_type.as_deref())
                    .flatten()
                }),
            receiver_type,
        )
    }

    fn external_api_candidate_indices(&self, method_name: &str) -> Rc<[usize]> {
        if let Some(cached) = self
            .external_api_candidates_by_query
            .borrow()
            .get(method_name)
            .cloned()
        {
            return cached;
        }
        let mut seen = HashSet::new();
        let mut indices = Vec::new();
        self.ensure_external_api_alias_index();
        let by_alias = self.external_api_by_method_alias.borrow();
        let by_alias = by_alias
            .as_ref()
            .expect("external API alias index initialized");
        for alias in external_method_name_aliases(method_name).iter() {
            if let Some(alias_indices) = by_alias.get(alias) {
                for index in alias_indices {
                    if seen.insert(*index) {
                        indices.push(*index);
                    }
                }
            }
        }
        indices.sort_by(|left, right| right.cmp(left));
        let indices = Rc::<[usize]>::from(indices);
        self.external_api_candidates_by_query
            .borrow_mut()
            .insert(method_name.to_string(), indices.clone());
        indices
    }

    fn find_external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
        direct_match_only: bool,
    ) -> Option<&'a ExternalFieldFact> {
        if !direct_match_only
            && self.external_field_simple_owner_alias_is_ambiguous(type_name, field_name)
        {
            return None;
        }
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

    fn external_field_simple_owner_alias_is_ambiguous(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> bool {
        let mut package_owners = HashSet::new();
        for index in self.external_field_candidate_indices(field_name) {
            let fact = &self.facts.external_field_facts[index];
            if !external_field_name_matches(&fact.field.field_name, field_name) {
                continue;
            }
            let Some(simple_owner) = package_qualified_simple_alias(&fact.field.type_name) else {
                continue;
            };
            if external_type_name_directly_matches(simple_owner, type_name) {
                package_owners.insert(fact.field.type_name.as_str());
                if package_owners.len() > 1 {
                    return true;
                }
            }
        }
        false
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

    fn find_external_field_value_for(
        &self,
        type_name: &str,
        field_name: &str,
        direct_match_only: bool,
    ) -> Option<&'a ExternalFieldValueFact> {
        if !direct_match_only
            && self.external_field_value_simple_owner_alias_is_ambiguous(type_name, field_name)
        {
            return None;
        }
        for index in self.external_field_value_candidate_indices(field_name) {
            let fact = &self.facts.external_field_value_facts[index];
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

    fn external_field_value_simple_owner_alias_is_ambiguous(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> bool {
        let mut package_owners = HashSet::new();
        for index in self.external_field_value_candidate_indices(field_name) {
            let fact = &self.facts.external_field_value_facts[index];
            if !external_field_name_matches(&fact.field.field_name, field_name) {
                continue;
            }
            let Some(simple_owner) = package_qualified_simple_alias(&fact.field.type_name) else {
                continue;
            };
            if external_type_name_directly_matches(simple_owner, type_name) {
                package_owners.insert(fact.field.type_name.as_str());
                if package_owners.len() > 1 {
                    return true;
                }
            }
        }
        false
    }

    fn find_exact_external_field_value_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&'a ExternalFieldValueFact> {
        for index in self.external_field_value_candidate_indices(field_name) {
            let fact = &self.facts.external_field_value_facts[index];
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
        self.ensure_external_field_alias_index();
        let by_alias = self.external_field_by_name_alias.borrow();
        let by_alias = by_alias
            .as_ref()
            .expect("external field alias index initialized");
        for alias in external_field_name_aliases(field_name) {
            if let Some(alias_indices) = by_alias.get(&alias) {
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

    fn external_field_value_candidate_indices(&self, field_name: &str) -> Vec<usize> {
        let mut seen = HashSet::new();
        let mut indices = Vec::new();
        self.ensure_external_field_value_alias_index();
        let by_alias = self.external_field_value_by_name_alias.borrow();
        let by_alias = by_alias
            .as_ref()
            .expect("external field-value alias index initialized");
        for alias in external_field_name_aliases(field_name) {
            if let Some(alias_indices) = by_alias.get(&alias) {
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
        // This fallback is rare: direct pack ownership is checked first.
        // Building a complete member-to-pack cross-index in `new` made every
        // ordinary API lookup pay for it, even when no pack lookup followed.
        let indices = self
            .facts
            .external_library_packs
            .iter()
            .enumerate()
            .filter_map(|(index, pack)| {
                self.external_pack_has_member_for_type(pack, type_name)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        if !type_name.contains('.') && indices.len() != 1 {
            return None;
        }
        indices
            .into_iter()
            .next_back()
            .map(|index| &self.facts.external_library_packs[index])
    }

    fn external_pack_has_member_for_type(
        &self,
        pack: &ExternalLibraryPackFact,
        type_name: &str,
    ) -> bool {
        // Simple imported names may legitimately resolve through a member's
        // simple alias.  Once the query is package-qualified, however, the
        // package is semantic: `other.Item` must not be claimed because a pack
        // happens to contain `a.Item`.
        let member_type_matches = |member_type: &str| {
            if type_name.contains('.') {
                external_type_name_directly_matches(member_type, type_name)
            } else {
                external_type_name_matches(member_type, type_name)
            }
        };
        self.facts.external_api_effect_facts.iter().any(|fact| {
            fact.receiver_type.as_deref().is_some_and(|receiver_type| {
                member_type_matches(receiver_type) && external_pack_owns_type(pack, receiver_type)
            })
        }) || self.facts.external_constructor_facts.iter().any(|fact| {
            member_type_matches(&fact.type_name) && external_pack_owns_type(pack, &fact.type_name)
        }) || self.facts.external_field_facts.iter().any(|fact| {
            member_type_matches(&fact.field.type_name)
                && external_pack_owns_type(pack, &fact.field.type_name)
        }) || self
            .facts
            .external_crate_api_mapping_facts
            .iter()
            .any(|fact| {
                member_type_matches(&fact.receiver_type)
                    && external_pack_owns_type(pack, &fact.receiver_type)
            })
            || self
                .facts
                .external_crate_type_mapping_facts
                .iter()
                .any(|fact| {
                    member_type_matches(&fact.type_name)
                        && external_pack_owns_type(pack, &fact.type_name)
                })
            || self
                .facts
                .external_crate_constructor_mapping_facts
                .iter()
                .any(|fact| {
                    member_type_matches(&fact.type_name)
                        && (fact
                            .pack_name
                            .as_deref()
                            .is_some_and(|pack_name| pack.pack_name == pack_name)
                            || external_pack_owns_type(pack, &fact.type_name))
                })
    }
}

impl<'a> HierarchyFactLookupIndex<'a> {
    pub fn new(facts: &'a FactSet) -> Self {
        #[cfg(test)]
        HIERARCHY_LOOKUP_INDEX_BUILD_COUNT.with(|count| count.set(count.get() + 1));
        Self {
            facts,
            // Exception and subtype analysis normally queries only a small
            // fraction of the modeled type universe. Saturating every JDK and
            // source hierarchy eagerly makes index construction quadratic in
            // the corpus size. Keep the exact legacy worklist semantics, but
            // run them on demand and memoize each queried subtype.
            reachable_by_sub_name: RefCell::new(HashMap::new()),
        }
    }

    pub fn reachable_hierarchy_edges_for_type_with_stats(
        &self,
        java_ty: &JavaType,
    ) -> HierarchyReachabilityResult {
        let Some(sub_name) = java_type_name(java_ty) else {
            return HierarchyReachabilityResult {
                edges: Vec::new(),
                stats: WorklistStats::default(),
                diagnostics: Vec::new(),
            };
        };
        if let Some(reachable) = self.reachable_by_sub_name.borrow().get(sub_name).cloned() {
            return reachable;
        }
        let reachable = self
            .facts
            .reachable_hierarchy_edges_for_type_with_stats(java_ty);
        self.reachable_by_sub_name
            .borrow_mut()
            .insert(sub_name.to_string(), reachable.clone());
        reachable
    }

    pub fn reachable_hierarchy_edges_for_type(&self, java_ty: &JavaType) -> Vec<HierarchyEdgeFact> {
        self.reachable_hierarchy_edges_for_type_with_stats(java_ty)
            .into_parts()
            .0
    }

    pub fn hierarchy_subtype_policy(
        &self,
        subtype: &JavaType,
        supertype: &JavaType,
    ) -> HierarchySubtypePolicy {
        let result = self.reachable_hierarchy_edges_for_type_with_stats(subtype);
        let reaches = match (java_type_name(subtype), java_type_name(supertype)) {
            (Some(sub_name), Some(super_name)) if sub_name == super_name => true,
            (Some(sub_name), Some(super_name)) => hierarchy_edges_reach_with_platform(
                self.facts,
                sub_name,
                super_name,
                result.edges(),
            ),
            _ => false,
        };
        HierarchySubtypePolicy::new(reaches)
    }
}

/// Extend recorded source edges through the shared JDK subtype authority. A
/// source definition or explicit user hierarchy overrides platform fallback.
/// Both indexed and direct queries use this exact boundary policy.
fn hierarchy_edges_reach_with_platform(
    facts: &FactSet,
    subtype: &str,
    supertype: &str,
    edges: &[HierarchyEdgeFact],
) -> bool {
    if edges.iter().any(|edge| edge.super_name == supertype) {
        return true;
    }
    // Most queries are entirely source-defined. Reject them before consulting
    // provenance indexes, and reuse the existing indexed override lookup for
    // the remaining platform boundary rather than rescanning all overrides.
    let mut candidates = std::iter::once(subtype)
        .chain(edges.iter().map(|edge| edge.super_name.as_str()))
        .filter(|name| crate::stdlib::is_known_java_platform_subtype(name, supertype));
    let Some(first) = candidates.next() else {
        return false;
    };
    let effective = crate::effective_facts::EffectiveFacts::new(facts);
    if !effective.allows_platform_hierarchy_fallback(supertype) {
        return false;
    }
    std::iter::once(first)
        .chain(candidates)
        .any(|name| effective.allows_platform_hierarchy_fallback(name))
}

fn external_method_name_aliases(method_name: &str) -> Rc<[String]> {
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<String, Rc<[String]>>> =
            std::cell::RefCell::new(HashMap::new());
    }
    if let Some(cached) = CACHE.with(|cache| cache.borrow().get(method_name).cloned()) {
        return cached;
    }
    let aliases = Rc::<[String]>::from(external_method_name_aliases_uncached(method_name));
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .insert(method_name.to_string(), aliases.clone())
    });
    aliases
}

fn external_method_name_aliases_uncached(method_name: &str) -> Vec<String> {
    let mut aliases = crate::java_naming::java_method_name_aliases(method_name);
    aliases.sort();
    aliases.dedup();
    aliases
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyReachabilityResult {
    edges: Vec<HierarchyEdgeFact>,
    stats: WorklistStats,
    diagnostics: Vec<HierarchyReachabilityDiagnostic>,
}

impl HierarchyReachabilityResult {
    pub fn edges(&self) -> &[HierarchyEdgeFact] {
        &self.edges
    }

    pub fn stats(&self) -> WorklistStats {
        self.stats
    }

    pub fn diagnostics(&self) -> &[HierarchyReachabilityDiagnostic] {
        &self.diagnostics
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<HierarchyEdgeFact>,
        WorklistStats,
        Vec<HierarchyReachabilityDiagnostic>,
    ) {
        (self.edges, self.stats, self.diagnostics)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchySubtypePolicy {
    reaches: bool,
}

impl HierarchySubtypePolicy {
    pub fn new(reaches: bool) -> Self {
        Self { reaches }
    }

    pub fn reaches(&self) -> bool {
        self.reaches
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyReachabilityDiagnostic {
    code: &'static str,
    message: String,
}

impl HierarchyReachabilityDiagnostic {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_parts(self) -> (&'static str, String) {
        (self.code, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeKey {
    type_name: String,
}

impl TypeKey {
    pub fn named(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.type_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberName(String);

impl MemberName {
    pub fn named(member_name: impl Into<String>) -> Self {
        Self(member_name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallableKind {
    Method,
    Constructor,
    StaticField,
    InstanceField,
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

impl TypePattern {
    pub fn exact(type_name: impl Into<String>) -> Self {
        Self::Exact(TypeKey::named(type_name))
    }

    pub fn erased(type_name: impl Into<String>) -> Self {
        Self::Erased(TypeKey::named(type_name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallableKey {
    owner: TypeKey,
    member: MemberName,
    kind: CallableKind,
    staticness: Staticness,
    arity: usize,
    params: Vec<TypePattern>,
    generic_arity: Option<usize>,
}

impl CallableKey {
    pub fn owner_type(&self) -> &str {
        self.owner.as_str()
    }

    pub fn member_name(&self) -> &str {
        self.member.as_str()
    }

    pub fn kind(&self) -> CallableKind {
        self.kind
    }

    pub fn staticness(&self) -> Staticness {
        self.staticness
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn params(&self) -> &[TypePattern] {
        &self.params
    }

    pub fn generic_arity(&self) -> Option<usize> {
        self.generic_arity
    }

    pub(crate) fn with_owner_type(mut self, owner: impl Into<String>) -> Self {
        self.owner = TypeKey::named(owner);
        self
    }

    pub fn method(
        owner: impl Into<String>,
        method: impl Into<String>,
        staticness: Staticness,
        arity: usize,
    ) -> Self {
        Self {
            owner: TypeKey::named(owner),
            member: MemberName::named(method),
            kind: CallableKind::Method,
            staticness,
            arity,
            params: vec![TypePattern::Any; arity],
            generic_arity: None,
        }
    }

    pub fn resolved_method(
        call: &crate::java_ir::ResolvedCall,
        staticness: Staticness,
        arity: usize,
    ) -> Self {
        Self::method(
            call.declaring_type.clone(),
            call.rust_name.clone(),
            staticness,
            arity,
        )
    }

    pub fn method_with_params(
        owner: impl Into<String>,
        method: impl Into<String>,
        staticness: Staticness,
        params: Vec<TypePattern>,
    ) -> Self {
        let arity = params.len();
        Self {
            owner: TypeKey::named(owner),
            member: MemberName::named(method),
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
            owner: TypeKey::named(owner),
            member: MemberName::named("<init>"),
            kind: CallableKind::Constructor,
            staticness: Staticness::Static,
            arity,
            params,
            generic_arity: None,
        }
    }

    pub fn resolved_constructor(
        constructor: &crate::java_ir::ResolvedConstructor,
        params: Vec<TypePattern>,
    ) -> Self {
        Self::constructor(constructor.declaring_type.clone(), params)
    }

    pub fn field(
        owner: impl Into<String>,
        field: impl Into<String>,
        kind: CallableKind,
        staticness: Staticness,
    ) -> Self {
        Self {
            owner: TypeKey::named(owner),
            member: MemberName::named(field),
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
    type_name: String,
    field_name: String,
}

impl FieldKey {
    pub fn named(type_name: impl Into<String>, field_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            field_name: field_name.into(),
        }
    }

    pub fn source_field(ty: &crate::java_ir::TypeDecl, field: &crate::java_ir::FieldDecl) -> Self {
        Self::named(ty.name.clone(), field.name.clone())
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn field_name(&self) -> &str {
        &self.field_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodKey {
    type_name: String,
    method_name: String,
}

impl MethodKey {
    pub fn named(type_name: impl Into<String>, method_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            method_name: method_name.into(),
        }
    }

    pub fn source_method(
        ty: &crate::java_ir::TypeDecl,
        method: &crate::java_ir::MethodDecl,
    ) -> Self {
        Self::named(ty.name.clone(), method.name.clone())
    }

    pub(crate) fn with_method_name(&self, method_name: impl Into<String>) -> Self {
        Self::named(self.type_name.clone(), method_name)
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn method_name(&self) -> &str {
        &self.method_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstructorKey {
    type_name: String,
    rust_name: String,
}

impl ConstructorKey {
    pub fn named(type_name: impl Into<String>, rust_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            rust_name: rust_name.into(),
        }
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn rust_name(&self) -> &str {
        &self.rust_name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullabilityState {
    DefinitelyNonNull,
    MaybeNull,
    DefinitelyNull,
    Unknown,
}

pub(crate) fn external_parameter_nullability_policy(state: NullabilityState) -> NullabilityState {
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
    pub pack_name: Option<String>,
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

impl FacadeSlot {
    pub fn contains_shared_borrow(&self) -> bool {
        match self {
            FacadeSlot::Borrowed => true,
            FacadeSlot::Optional(inner) | FacadeSlot::Slice(inner) => {
                inner.contains_shared_borrow()
            }
            FacadeSlot::Owned
            | FacadeSlot::Index
            | FacadeSlot::MutBorrowed
            | FacadeSlot::DynTrait(_)
            | FacadeSlot::CrateHandle(_) => false,
        }
    }

    pub fn contains_mut_borrow(&self) -> bool {
        match self {
            FacadeSlot::MutBorrowed => true,
            FacadeSlot::Optional(inner) | FacadeSlot::Slice(inner) => inner.contains_mut_borrow(),
            FacadeSlot::Owned
            | FacadeSlot::Index
            | FacadeSlot::Borrowed
            | FacadeSlot::DynTrait(_)
            | FacadeSlot::CrateHandle(_) => false,
        }
    }
}

pub fn facade_slot_for_rust_type(ty: &str) -> FacadeSlot {
    let ty = ty.trim();
    if ty == "usize" {
        return FacadeSlot::Index;
    }
    if let Some(inner) = crate::types::raw_rust_option_inner(ty) {
        return FacadeSlot::Optional(Box::new(facade_slot_for_rust_type(inner)));
    }
    if let Some(inner) = ty.strip_prefix("&mut ") {
        if let Some(slice) = inner.strip_prefix('[').and_then(|ty| ty.strip_suffix(']')) {
            return FacadeSlot::Slice(Box::new(facade_slot_for_rust_type(&format!(
                "&mut {slice}"
            ))));
        }
        return FacadeSlot::MutBorrowed;
    }
    if let Some(inner) = ty.strip_prefix('&') {
        let inner = inner.trim();
        if let Some(slice) = inner.strip_prefix('[').and_then(|ty| ty.strip_suffix(']')) {
            return FacadeSlot::Slice(Box::new(facade_slot_for_rust_type(slice)));
        }
        return FacadeSlot::Borrowed;
    }
    if let Some(trait_name) = crate::types::raw_rust_box_dyn_trait(ty) {
        return FacadeSlot::DynTrait(trait_name.to_string());
    }
    if rust_type_is_crate_handle(ty) {
        return FacadeSlot::CrateHandle(ty.to_string());
    }
    FacadeSlot::Owned
}

fn rust_type_is_crate_handle(ty: &str) -> bool {
    ty.contains("::")
        && !ty.starts_with("std::")
        && !ty.starts_with("core::")
        && !ty.starts_with("alloc::")
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
        JavaType::Named { name, .. } => TypePattern::erased(name.clone()),
        JavaType::Primitive(primitive) => {
            TypePattern::exact(format!("{primitive:?}").to_ascii_lowercase())
        }
        JavaType::String => TypePattern::exact("String"),
        JavaType::Array(element) => TypePattern::erased(format!(
            "{}[]",
            external_type_pattern_key_for_java_type(element)
        )),
        JavaType::Intersection(bounds) => bounds
            .first()
            .map(external_type_pattern_for_java_type)
            .unwrap_or(TypePattern::Any),
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
        JavaType::Intersection(bounds) => bounds
            .first()
            .map(external_type_pattern_key_for_java_type)
            .unwrap_or_else(|| "Object".to_string()),
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

fn join_method_receiver_mutability<'a>(
    effects: impl IntoIterator<Item = &'a MethodEffectFact>,
) -> Option<ReceiverMutabilityFact> {
    let mut inferred = None;
    let mut user_override = None;
    for effect in effects {
        if effect.provenance == FactProvenance::UserOverride {
            user_override = Some(effect.receiver_mutability);
            continue;
        }
        inferred = Some(match (inferred, effect.receiver_mutability) {
            (Some(ReceiverMutabilityFact::MutatesReceiver), _)
            | (_, ReceiverMutabilityFact::MutatesReceiver) => {
                ReceiverMutabilityFact::MutatesReceiver
            }
            (Some(ReceiverMutabilityFact::ReadOnly), _) | (_, ReceiverMutabilityFact::ReadOnly) => {
                ReceiverMutabilityFact::ReadOnly
            }
            _ => ReceiverMutabilityFact::Unknown,
        });
    }
    user_override.or(inferred)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEqualsOverrideFact {
    pub type_name: String,
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

impl HierarchyOverride {
    /// Whether this representation is only a fallback when no parsed source
    /// definition is available. Keep planning and effective fact lookup on the
    /// same precedence policy; explicit user overrides remain authoritative.
    pub(crate) fn is_source_fallback(&self) -> bool {
        matches!(
            self.provenance,
            FactProvenance::ExternalLibraryPack | FactProvenance::BuiltinExternalModel
        ) || (self.provenance != FactProvenance::UserOverride
            && matches!(self.strategy, HierarchyStrategy::ErasedObject))
    }
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
    code: &'static str,
    message: String,
}

impl FactDiagnostic {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_parts(self) -> (&'static str, String) {
        (self.code, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HierarchyEdgeFact {
    pub super_name: String,
    pub sub_name: String,
}

/// A boxed adapter generated from a source interface's instantiated ancestry.
/// Unlike a name-only hierarchy edge, this preserves the generic ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInterfaceUpcastFact {
    pub source_type: JavaType,
    pub target_type: JavaType,
    pub type_params: Vec<String>,
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
pub struct GenericParameterVarianceFact {
    pub type_name: String,
    pub parameter_index: usize,
    pub consumed: bool,
    pub produced: bool,
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
    MethodReference,
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
    /// Resolver declaration identity; external capture facts may omit it.
    pub binding: Option<crate::binding_id::BindingId>,
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
    /// A lambda or bound reference retains its lexical Java receiver, not a
    /// snapshot of that receiver's current fields.
    This,
    OuterThis,
}

impl FactSet {
    pub fn external_lookup_index(&self) -> ExternalFactLookupIndex<'_> {
        ExternalFactLookupIndex::new(self)
    }

    pub fn hierarchy_lookup_index(&self) -> HierarchyFactLookupIndex<'_> {
        HierarchyFactLookupIndex::new(self)
    }

    /// How values of `type_name` are held. `Value` for anything unknown, so a
    /// caller cannot mistake "not analysed" for "shared".
    pub fn storage_strategy_for(
        &self,
        type_name: &str,
    ) -> crate::storage_strategy::StorageStrategy {
        self.storage_strategies.strategy(type_name)
    }

    /// Where values of `type_name` live; see
    /// [`crate::storage_strategy::StoragePlacement`].
    pub fn storage_placement_for(
        &self,
        type_name: &str,
    ) -> crate::storage_strategy::StoragePlacement {
        self.storage_strategies.placement(type_name)
    }

    /// Which pointer `type_name`'s shared handle must be; see
    /// [`crate::storage_strategy::SharedHandle`].
    pub fn shared_handle_for(&self, type_name: &str) -> crate::storage_strategy::SharedHandle {
        self.storage_strategies.handle(type_name)
    }

    pub fn constructor_self_escapes_for(&self, type_name: &str) -> bool {
        self.storage_strategies
            .facts(type_name)
            .is_some_and(|facts| facts.constructor_self_escapes())
    }

    pub fn extend_from(&mut self, other: FactSet) {
        self.disable_builtin_external_packs |= other.disable_builtin_external_packs;
        self.java_string_identity_representation |= other.java_string_identity_representation;
        self.storage_strategies
            .extend_from(other.storage_strategies);
        self.notes.extend(other.notes);
        self.captures.owners.extend(other.captures.owners);
        self.hierarchy_overrides.extend(other.hierarchy_overrides);
        self.closed_hierarchy_facts
            .extend(other.closed_hierarchy_facts);
        self.sealed_permits_facts.extend(other.sealed_permits_facts);
        self.object_safety_facts.extend(other.object_safety_facts);
        self.source_interface_upcast_facts
            .extend(other.source_interface_upcast_facts);
        self.generic_parameter_variance_facts
            .extend(other.generic_parameter_variance_facts);
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
        self.source_equals_override_facts
            .extend(other.source_equals_override_facts);
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
        let direct = |fact: &&FieldNullabilityFact| {
            external_type_name_directly_matches(&fact.field.type_name, type_name)
                && external_field_name_matches(&fact.field.field_name, field_name)
        };
        self.field_nullability_facts
            .iter()
            .rev()
            .find(|fact| {
                direct(fact) && fact.provenance == FieldNullabilityProvenance::ExternalOverride
            })
            .or_else(|| {
                self.field_nullability_facts
                    .iter()
                    .rev()
                    .find(|fact| direct(fact))
            })
            .or_else(|| {
                if field_nullability_simple_owner_alias_is_ambiguous(
                    &self.field_nullability_facts,
                    type_name,
                    field_name,
                ) {
                    return None;
                }
                self.field_nullability_facts.iter().rev().find(|fact| {
                    external_type_name_matches(&fact.field.type_name, type_name)
                        && external_field_name_matches(&fact.field.field_name, field_name)
                        && fact.provenance == FieldNullabilityProvenance::ExternalOverride
                })
            })
            .or_else(|| {
                if field_nullability_simple_owner_alias_is_ambiguous(
                    &self.field_nullability_facts,
                    type_name,
                    field_name,
                ) {
                    return None;
                }
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
            .find(|fact| method_key_directly_matches(&fact.method, type_name, method_name))
            .or_else(|| {
                if trivial_accessor_simple_owner_alias_is_ambiguous(
                    &self.trivial_accessor_facts,
                    type_name,
                    method_name,
                ) {
                    return None;
                }
                self.trivial_accessor_facts
                    .iter()
                    .rev()
                    .find(|fact| method_key_matches(&fact.method, type_name, method_name))
            })
    }

    pub(crate) fn external_api_effect_for(
        &self,
        method_name: &str,
    ) -> Option<&ExternalApiEffectFact> {
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

    pub fn external_api_effect_for_receiver_with_arity(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
    ) -> Option<&ExternalApiEffectFact> {
        let index = ExternalFactLookupIndex::new(self);
        index.external_api_effect_for_receiver_with_arity(receiver_type, method_name, arg_count)
    }

    pub(crate) fn external_api_effect_for_receiver_with_arity_or_rust_name(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
    ) -> Option<&ExternalApiEffectFact> {
        self.external_api_effect_for_receiver_with_arity(receiver_type, method_name, arg_count)
            .or_else(|| {
                receiver_type.and_then(|receiver_type| {
                    find_external_crate_api_mapping_for_rust_name(
                        &self.external_crate_api_mapping_facts,
                        receiver_type,
                        method_name,
                        arg_count,
                    )
                    .and_then(|mapping| {
                        self.external_api_effect_for_receiver_with_arity(
                            Some(&mapping.receiver_type),
                            &mapping.method_name,
                            arg_count.or(mapping.arg_count),
                        )
                    })
                    .or_else(|| {
                        find_external_crate_api_mapping_for_rust_name(
                            &self.external_crate_api_mapping_facts,
                            receiver_type,
                            method_name,
                            None,
                        )
                        .and_then(|mapping| {
                            self.external_api_effect_for_receiver_with_arity(
                                Some(&mapping.receiver_type),
                                &mapping.method_name,
                                arg_count.or(mapping.arg_count),
                            )
                        })
                    })
                })
            })
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
        find_external_api_argument_effect_for_receiver(
            &self.external_api_argument_effect_facts,
            receiver_type,
            method_name,
            arity_matches,
            parameter_index,
            Some(FactProvenance::UserOverride),
        )
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
            find_external_api_argument_effect_for_receiver(
                &self.external_api_argument_effect_facts,
                receiver_type,
                method_name,
                arity_matches,
                parameter_index,
                None,
            )
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
            find_external_api_argument_effect_for_receiver(
                &self.external_api_argument_effect_facts,
                receiver_type,
                method_name,
                generic_arity_matches,
                parameter_index,
                Some(FactProvenance::UserOverride),
            )
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
            find_external_api_argument_effect_for_receiver(
                &self.external_api_argument_effect_facts,
                receiver_type,
                method_name,
                generic_arity_matches,
                parameter_index,
                None,
            )
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
                    && !external_api_argument_effect_simple_owner_alias_is_ambiguous(
                        &self.external_api_argument_effect_facts,
                        receiver_type,
                        method_name,
                        |_| true,
                        parameter_index,
                        Some(FactProvenance::UserOverride),
                    )
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
                    && !external_api_argument_effect_simple_owner_alias_is_ambiguous(
                        &self.external_api_argument_effect_facts,
                        receiver_type,
                        method_name,
                        |_| true,
                        parameter_index,
                        None,
                    )
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
        .filter(|fact| {
            receiver_type
                .is_none_or(|owner| self.model_fact_applies_to_owner(owner, fact.provenance))
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

    pub(crate) fn external_api_argument_effect_for_key(
        &self,
        key: &CallableKey,
        parameter_index: usize,
    ) -> Option<&ExternalApiArgumentEffectFact> {
        if key.kind() != CallableKind::Method || parameter_index >= key.arity() {
            return None;
        }
        self.external_api_argument_effect_facts
            .iter()
            .rev()
            .find(|fact| {
                fact.callable_key().is_some_and(|fact_key| {
                    fact_key.owner_type() == key.owner_type()
                        && fact_key.member_name() == key.member_name()
                        && fact_key.kind() == key.kind()
                        && fact.parameter_index == parameter_index
                })
            })
            .or_else(|| {
                self.external_api_argument_effect_for_receiver_with_arity(
                    Some(key.owner_type()),
                    key.member_name(),
                    Some(key.arity()),
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
        find_external_ffi_handover_for_receiver(
            &self.external_ffi_handover_facts,
            receiver_type,
            method_name,
            ExternalFfiHandoverSlot::Argument { parameter_index },
            parameter_type,
        )
    }

    pub fn external_ffi_receiver_handover_for_receiver(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        receiver_java_ty: &JavaType,
    ) -> Option<&ExternalFfiHandoverFact> {
        find_external_ffi_handover_for_receiver(
            &self.external_ffi_handover_facts,
            receiver_type,
            method_name,
            ExternalFfiHandoverSlot::Receiver,
            receiver_java_ty,
        )
    }

    pub fn external_constructor_argument_effect_for(
        &self,
        type_name: &str,
        arg_count: usize,
        parameter_index: usize,
    ) -> Option<&ExternalConstructorArgumentEffectFact> {
        find_external_constructor_argument_effect_for(
            &self.external_constructor_argument_effect_facts,
            type_name,
            arg_count,
            parameter_index,
            FactProvenance::UserOverride,
        )
        .or_else(|| {
            find_external_constructor_argument_effect_for(
                &self.external_constructor_argument_effect_facts,
                type_name,
                arg_count,
                parameter_index,
                FactProvenance::ExternalLibraryPack,
            )
        })
    }

    pub fn external_conversion_adapter_for(
        &self,
        from_type: &str,
        to_type: &str,
    ) -> Option<&ExternalConversionAdapterFact> {
        find_external_conversion_adapter_for(
            &self.external_conversion_adapter_facts,
            from_type,
            to_type,
            FactProvenance::UserOverride,
        )
        .or_else(|| {
            find_external_conversion_adapter_for(
                &self.external_conversion_adapter_facts,
                from_type,
                to_type,
                FactProvenance::ExternalLibraryPack,
            )
        })
    }

    /// Source declarations own their ABI. A familiar external leaf alias must
    /// not override that declaration; explicit user mappings remain deliberate.
    pub(crate) fn model_fact_applies_to_owner(
        &self,
        owner: &str,
        provenance: FactProvenance,
    ) -> bool {
        crate::effective_facts::fact_precedence_key(provenance)
            >= crate::effective_facts::fact_precedence_key(FactProvenance::ParsedSource)
            || !crate::effective_facts::EffectiveFacts::new(self)
                .has_parsed_source_type_capability_for_name(owner)
    }

    pub(crate) fn facade_abi_for_key(&self, key: &CallableKey) -> Option<&FacadeAbiFact> {
        self.facade_abi_facts
            .iter()
            .rev()
            .find(|fact| &fact.key == key && fact.provenance == FactProvenance::UserOverride)
            .or_else(|| {
                self.facade_abi_facts.iter().rev().find(|fact| {
                    &fact.key == key
                        && self.model_fact_applies_to_owner(key.owner_type(), fact.provenance)
                })
            })
    }

    pub fn facade_abi_for_external_method(
        &self,
        receiver_type: &str,
        method_name: &str,
        arg_count: usize,
    ) -> Option<&FacadeAbiFact> {
        self.facade_abi_for_key(&CallableKey::method(
            receiver_type,
            method_name,
            Staticness::Unknown,
            arg_count,
        ))
    }

    pub fn facade_abi_for_call(
        &self,
        key: &CallableKey,
        arg_types: &[JavaType],
    ) -> Option<&FacadeAbiFact> {
        find_facade_abi_for_call(
            self.facade_abi_facts.iter().rev().filter(|fact| {
                callable_key_header_directly_matches(&fact.key, key)
                    && self.model_fact_applies_to_owner(key.owner_type(), fact.provenance)
            }),
            arg_types,
        )
        .or_else(|| {
            if callable_key_simple_owner_alias_is_ambiguous(
                self.facade_abi_facts.iter().map(|fact| &fact.key),
                key,
            ) {
                return None;
            }
            find_facade_abi_for_call(
                self.facade_abi_facts.iter().rev().filter(|fact| {
                    callable_key_header_matches(&fact.key, key)
                        && self.model_fact_applies_to_owner(key.owner_type(), fact.provenance)
                }),
                arg_types,
            )
        })
    }

    pub fn facade_abi_for_external_call(
        &self,
        receiver_type: &str,
        method_name: &str,
        arg_count: usize,
        arg_types: &[JavaType],
    ) -> Option<&FacadeAbiFact> {
        self.facade_abi_for_call(
            &CallableKey::method(receiver_type, method_name, Staticness::Unknown, arg_count),
            arg_types,
        )
    }

    pub fn external_crate_api_mapping_for_receiver(
        &self,
        receiver_type: &str,
        method_name: &str,
        arg_count: Option<usize>,
    ) -> Option<&ExternalCrateApiMappingFact> {
        find_external_crate_api_mapping_for_receiver(
            &self.external_crate_api_mapping_facts,
            receiver_type,
            method_name,
            arg_count,
            Some(FactProvenance::UserOverride),
        )
        .or_else(|| {
            find_external_crate_api_mapping_for_receiver(
                &self.external_crate_api_mapping_facts,
                receiver_type,
                method_name,
                None,
                Some(FactProvenance::UserOverride),
            )
        })
        .or_else(|| {
            find_external_crate_api_mapping_for_receiver(
                &self.external_crate_api_mapping_facts,
                receiver_type,
                method_name,
                arg_count,
                None,
            )
        })
        .or_else(|| {
            find_external_crate_api_mapping_for_receiver(
                &self.external_crate_api_mapping_facts,
                receiver_type,
                method_name,
                None,
                None,
            )
        })
    }

    pub fn external_crate_constructor_mapping_for(
        &self,
        type_name: &str,
        arg_count: usize,
    ) -> Option<&ExternalCrateConstructorMappingFact> {
        find_external_crate_constructor_mapping_for(
            &self.external_crate_constructor_mapping_facts,
            type_name,
            arg_count,
            Some(FactProvenance::UserOverride),
        )
        .or_else(|| {
            find_external_crate_constructor_mapping_for(
                &self.external_crate_constructor_mapping_facts,
                type_name,
                arg_count,
                None,
            )
        })
    }

    pub fn external_crate_rust_type_name_for(&self, type_name: &str) -> Option<&str> {
        find_external_crate_type_mapping_for(
            &self.external_crate_type_mapping_facts,
            type_name,
            Some(FactProvenance::UserOverride),
        )
        .or_else(|| {
            find_external_crate_type_mapping_for(
                &self.external_crate_type_mapping_facts,
                type_name,
                None,
            )
        })
        .map(|fact| fact.rust_type_name.as_str())
        .or_else(|| {
            find_external_crate_constructor_type_mapping_for(
                &self.external_crate_constructor_mapping_facts,
                type_name,
                Some(FactProvenance::UserOverride),
            )
            .or_else(|| {
                find_external_crate_constructor_type_mapping_for(
                    &self.external_crate_constructor_mapping_facts,
                    type_name,
                    None,
                )
            })
            .and_then(|fact| fact.rust_type_name.as_deref())
        })
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
            .filter(|fact| {
                receiver_type
                    .is_none_or(|owner| self.model_fact_applies_to_owner(owner, fact.provenance))
            })
    }

    fn find_external_rust_result(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: Option<usize>,
        user_override_only: bool,
    ) -> Option<&ExternalRustResultFact> {
        find_external_rust_result_for_receiver(
            &self.external_rust_result_facts,
            receiver_type,
            method_name,
            arg_count,
            user_override_only,
        )
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
        find_external_rust_return_type_for_receiver(
            &self.external_rust_return_type_facts,
            receiver_type,
            method_name,
            arg_count,
            user_override_only,
        )
    }

    pub fn external_constructor_rust_result_for(
        &self,
        type_name: &str,
        arg_count: usize,
    ) -> Option<&ExternalConstructorRustResultFact> {
        find_external_constructor_rust_result_for(
            &self.external_constructor_rust_result_facts,
            type_name,
            arg_count,
            FactProvenance::UserOverride,
        )
        .or_else(|| {
            find_external_constructor_rust_result_for(
                &self.external_constructor_rust_result_facts,
                type_name,
                arg_count,
                FactProvenance::ExternalLibraryPack,
            )
        })
        .or_else(|| {
            find_external_constructor_rust_result_for(
                &self.external_constructor_rust_result_facts,
                type_name,
                arg_count,
                FactProvenance::BuiltinExternalModel,
            )
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

    pub fn external_field_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&ExternalFieldFact> {
        let index = ExternalFactLookupIndex::new(self);
        index.external_field_for(type_name, field_name)
    }

    pub fn external_field_value_for(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&ExternalFieldValueFact> {
        let index = ExternalFactLookupIndex::new(self);
        index.external_field_value_for(type_name, field_name)
    }

    fn find_external_constructor_for(
        &self,
        type_name: &str,
        arg_types: &[JavaType],
        direct_match_only: bool,
    ) -> Option<&ExternalConstructorFact> {
        if !direct_match_only
            && external_constructor_simple_owner_alias_is_ambiguous(
                &self.external_constructor_facts,
                type_name,
                arg_types,
            )
        {
            return None;
        }
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

    pub fn external_library_pack_for_type(
        &self,
        type_name: &str,
    ) -> Option<&ExternalLibraryPackFact> {
        let index = ExternalFactLookupIndex::new(self);
        index.external_library_pack_for_type(type_name)
    }

    pub fn external_rust_crate_mapping_pack_for_type(
        &self,
        type_name: &str,
    ) -> Option<&ExternalLibraryPackFact> {
        let index = ExternalFactLookupIndex::new(self);
        index.external_rust_crate_mapping_pack_for_type(type_name)
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
                if unsupported_external_api_simple_owner_alias_is_ambiguous(
                    &self.unsupported_external_api_facts,
                    receiver_type,
                    method_name,
                ) {
                    return None;
                }
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
                if unsupported_external_constructor_simple_owner_alias_is_ambiguous(
                    &self.unsupported_external_constructor_facts,
                    type_name,
                    arg_count,
                ) {
                    return None;
                }
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
                if unsupported_external_field_simple_owner_alias_is_ambiguous(
                    &self.unsupported_external_field_facts,
                    type_name,
                    field_name,
                ) {
                    return None;
                }
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
        let direct = self
            .recursion_facts
            .iter()
            .filter(|fact| method_key_directly_matches(&fact.method, type_name, method_name))
            .collect::<Vec<_>>();
        if !direct.is_empty()
            || method_key_simple_owner_alias_is_ambiguous(
                self.recursion_facts.iter().map(|fact| &fact.method),
                type_name,
                method_name,
            )
        {
            return direct;
        }
        self.recursion_facts
            .iter()
            .filter(|fact| method_key_matches(&fact.method, type_name, method_name))
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
                method_key_directly_matches(&fact.method, type_name, method_name)
                    && fact.provenance == FactProvenance::UserOverride
            })
            .or_else(|| {
                if method_key_simple_owner_alias_is_ambiguous(
                    self.method_effect_facts.iter().map(|fact| &fact.method),
                    type_name,
                    method_name,
                ) {
                    return None;
                }
                self.method_effect_facts.iter().rev().find(|fact| {
                    method_key_matches(&fact.method, type_name, method_name)
                        && fact.provenance == FactProvenance::UserOverride
                })
            })
            .or_else(|| {
                self.method_effect_facts.iter().rev().find(|fact| {
                    method_key_directly_matches(&fact.method, type_name, method_name)
                        && self.model_fact_applies_to_owner(type_name, fact.provenance)
                })
            })
            .or_else(|| {
                if method_key_simple_owner_alias_is_ambiguous(
                    self.method_effect_facts.iter().map(|fact| &fact.method),
                    type_name,
                    method_name,
                ) {
                    return None;
                }
                self.method_effect_facts.iter().rev().find(|fact| {
                    method_key_matches(&fact.method, type_name, method_name)
                        && self.model_fact_applies_to_owner(type_name, fact.provenance)
                })
            })
    }

    /// Joins receiver-mutability evidence independently of the other method
    /// effect dimensions. Summary and primary-source analysis can contribute
    /// separate facts for one method; a later `Unknown` summary must not hide
    /// a concrete mutating body.
    pub fn method_receiver_mutability_for(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<ReceiverMutabilityFact> {
        let direct = self
            .method_effect_facts
            .iter()
            .filter(|fact| method_key_directly_matches(&fact.method, type_name, method_name))
            .collect::<Vec<_>>();
        if !direct.is_empty() {
            return join_method_receiver_mutability(direct);
        }
        if method_key_simple_owner_alias_is_ambiguous(
            self.method_effect_facts.iter().map(|fact| &fact.method),
            type_name,
            method_name,
        ) {
            return None;
        }
        join_method_receiver_mutability(
            self.method_effect_facts
                .iter()
                .filter(|fact| method_key_matches(&fact.method, type_name, method_name)),
        )
    }

    pub fn source_equals_override_for(&self, type_name: &str) -> Option<&SourceEqualsOverrideFact> {
        self.source_equals_override_facts
            .iter()
            .rev()
            .find(|fact| external_type_name_directly_matches(&fact.type_name, type_name))
            .or_else(|| {
                if source_equals_override_simple_owner_alias_is_ambiguous(
                    &self.source_equals_override_facts,
                    type_name,
                ) {
                    return None;
                }
                self.source_equals_override_facts
                    .iter()
                    .rev()
                    .find(|fact| external_type_name_matches(&fact.type_name, type_name))
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
            method_key_directly_matches(&fact.method, type_name, method_name) && fact.kind == kind
        }) || (!method_key_simple_owner_alias_is_ambiguous(
            self.control_flow_facts.iter().map(|fact| &fact.method),
            type_name,
            method_name,
        ) && self.control_flow_facts.iter().any(|fact| {
            method_key_matches(&fact.method, type_name, method_name) && fact.kind == kind
        }))
    }

    pub fn dispatch_requirements_for(&self, type_name: &str) -> Vec<DispatchRequirementKind> {
        let mut requirements = Vec::new();
        for fact in &self.dispatch_requirement_facts {
            if external_type_name_directly_matches(&fact.type_name, type_name)
                && !requirements.contains(&fact.requirement)
            {
                requirements.push(fact.requirement);
            }
        }
        if !requirements.is_empty()
            || dispatch_requirement_simple_owner_alias_is_ambiguous(
                &self.dispatch_requirement_facts,
                type_name,
            )
        {
            return requirements;
        }
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
        let direct = self
            .dispatch_requirement_facts
            .iter()
            .filter(|fact| external_type_name_directly_matches(&fact.type_name, type_name))
            .collect::<Vec<_>>();
        if !direct.is_empty()
            || dispatch_requirement_simple_owner_alias_is_ambiguous(
                &self.dispatch_requirement_facts,
                type_name,
            )
        {
            return direct;
        }
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
                external_type_name_directly_matches(&fact.type_name, type_name)
                    && fact.kind != ExceptionKind::Unknown
            })
            .or_else(|| {
                if exception_simple_owner_alias_is_ambiguous(&self.exception_facts, type_name) {
                    return None;
                }
                self.exception_facts.iter().rev().find(|fact| {
                    exception_type_name_matches(&fact.type_name, type_name)
                        && fact.kind != ExceptionKind::Unknown
                })
            })
            .or_else(|| {
                self.exception_facts
                    .iter()
                    .rev()
                    .find(|fact| external_type_name_directly_matches(&fact.type_name, type_name))
            })
            .or_else(|| {
                if exception_simple_owner_alias_is_ambiguous(&self.exception_facts, type_name) {
                    return None;
                }
                self.exception_facts
                    .iter()
                    .rev()
                    .find(|fact| exception_type_name_matches(&fact.type_name, type_name))
            })
            .map(|fact| fact.kind)
    }

    pub fn catch_facts_for(&self, type_name: &str, method_name: &str) -> Vec<&CatchFact> {
        let direct = self
            .catch_facts
            .iter()
            .filter(|fact| method_key_directly_matches(&fact.method, type_name, method_name))
            .collect::<Vec<_>>();
        if !direct.is_empty()
            || method_key_simple_owner_alias_is_ambiguous(
                self.catch_facts.iter().map(|fact| &fact.method),
                type_name,
                method_name,
            )
        {
            return direct;
        }
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

    pub fn generic_parameter_is_input_only(&self, type_name: &str, parameter_index: usize) -> bool {
        let matching = self.generic_parameter_variance_facts.iter().filter(|fact| {
            fact.parameter_index == parameter_index
                && external_type_name_matches(&fact.type_name, type_name)
        });
        let mut found = false;
        for fact in matching {
            found = true;
            if !fact.consumed || fact.produced {
                return false;
            }
        }
        found
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

    pub fn extends_type_name_for(&self, type_name: &str) -> Option<String> {
        let java_ty = JavaType::Named {
            name: type_name.to_string(),
            args: Vec::new(),
        };
        match self.extends_type_for(&java_ty)? {
            JavaType::String => Some("String".to_string()),
            JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
            _ => None,
        }
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
            .into_parts()
            .0
    }

    pub fn hierarchy_subtype_policy(
        &self,
        subtype: &JavaType,
        supertype: &JavaType,
    ) -> HierarchySubtypePolicy {
        let result = self.reachable_hierarchy_edges_for_type_with_stats(subtype);
        let reaches = match (java_type_name(subtype), java_type_name(supertype)) {
            (Some(sub_name), Some(super_name)) if sub_name == super_name => true,
            (Some(sub_name), Some(super_name)) => {
                hierarchy_edges_reach_with_platform(self, sub_name, super_name, result.edges())
            }
            _ => false,
        };
        HierarchySubtypePolicy::new(reaches)
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
        let result = saturate_worklist(
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
            |stats| {
                HierarchyReachabilityDiagnostic::new(
                    "hierarchy_reachability_budget_exhausted",
                    format!(
                        "hierarchy reachability exceeded finite worklist budget after {} processed items",
                        stats.processed_items()
                    ),
                )
            },
        );
        let (mut edges, stats, diagnostics) = result.into_parts();
        edges.sort_by(|left, right| {
            left.super_name
                .cmp(&right.super_name)
                .then_with(|| left.sub_name.cmp(&right.sub_name))
        });
        HierarchyReachabilityResult {
            edges,
            stats,
            diagnostics,
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
            .map(crate::effective_facts::EffectiveFact::into_value)
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

pub(crate) fn external_type_name_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    if fact_type_name == query_type_name {
        return true;
    }
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<String, HashMap<String, bool>>> =
            std::cell::RefCell::new(HashMap::new());
    }
    let (left, right) = if fact_type_name <= query_type_name {
        (fact_type_name, query_type_name)
    } else {
        (query_type_name, fact_type_name)
    };
    if let Some(cached) = CACHE.with(|cache| {
        cache
            .borrow()
            .get(left)
            .and_then(|matches| matches.get(right))
            .copied()
    }) {
        return cached;
    }
    let fact_aliases = external_type_name_aliases(fact_type_name);
    let query_aliases = external_type_name_aliases(query_type_name);
    let matches = fact_aliases.iter().any(|fact_alias| {
        query_aliases
            .iter()
            .any(|query_alias| query_alias == fact_alias)
    });
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry(left.to_string())
            .or_default()
            .insert(right.to_string(), matches)
    });
    matches
}

/// Alias-union index for repeated membership queries against a stable set of
/// Java type names. This is equivalent to asking whether any stored name
/// matches the query with [`external_type_name_matches`], but avoids the
/// stored-set cross product at each call site.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExternalTypeNameSetIndex {
    aliases: HashSet<String>,
}

/// Alias-to-row index for an immutable ordered fact collection. Candidate
/// rows retain original order, including distinct facts with identical names.
/// Use the same alias expansion as pairwise external name matching, rather
/// than approximating it with a simple-name or canonical-name key.
#[derive(Debug)]
pub(crate) struct ExternalTypeNameLookupIndex {
    by_alias: HashMap<String, Vec<usize>>,
    by_query: RefCell<HashMap<String, Rc<[usize]>>>,
}

impl ExternalTypeNameLookupIndex {
    pub(crate) fn new<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        let mut by_alias = HashMap::<String, Vec<usize>>::new();
        for (index, name) in names.into_iter().enumerate() {
            for alias in external_type_name_aliases(name).iter() {
                by_alias.entry(alias.clone()).or_default().push(index);
            }
        }
        Self {
            by_alias,
            by_query: RefCell::new(HashMap::new()),
        }
    }

    pub(crate) fn matching_indices(&self, name: &str) -> Rc<[usize]> {
        if let Some(indices) = self.by_query.borrow().get(name) {
            return indices.clone();
        }
        let mut indices = Vec::new();
        for alias in external_type_name_aliases(name).iter() {
            if let Some(candidates) = self.by_alias.get(alias) {
                indices.extend_from_slice(candidates);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        let indices = Rc::<[usize]>::from(indices);
        self.by_query
            .borrow_mut()
            .insert(name.to_string(), indices.clone());
        indices
    }
}

impl ExternalTypeNameSetIndex {
    pub(crate) fn new<'a>(names: impl IntoIterator<Item = &'a String>) -> Self {
        let mut aliases = HashSet::new();
        for name in names {
            aliases.extend(external_type_name_aliases(name).iter().cloned());
        }
        Self { aliases }
    }

    pub(crate) fn matches(&self, name: &str) -> bool {
        external_type_name_aliases(name)
            .iter()
            .any(|alias| self.aliases.contains(alias))
    }
}

fn callable_key_header_matches(fact: &CallableKey, query: &CallableKey) -> bool {
    callable_key_non_owner_header_matches(fact, query)
        && external_type_name_matches(fact.owner_type(), query.owner_type())
}

// Reject unrelated members before computing owner aliases. These scalar
// predicates are shared by direct and alias fallback lookup; neither candidate
// ordering nor source/override precedence changes.
fn callable_key_non_owner_header_matches(fact: &CallableKey, query: &CallableKey) -> bool {
    fact.member_name() == query.member_name()
        && fact.kind() == query.kind()
        && (fact.staticness() == query.staticness()
            || fact.staticness() == Staticness::Unknown
            || query.staticness() == Staticness::Unknown)
        && fact.arity() == query.arity()
        && (fact.generic_arity().is_none()
            || query.generic_arity().is_none()
            || fact.generic_arity() == query.generic_arity())
}

fn callable_key_header_directly_matches(fact: &CallableKey, query: &CallableKey) -> bool {
    callable_key_non_owner_header_matches(fact, query)
        && external_type_name_directly_matches(fact.owner_type(), query.owner_type())
}

fn callable_key_simple_owner_alias_is_ambiguous<'a>(
    keys: impl Iterator<Item = &'a CallableKey>,
    query: &CallableKey,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        keys.filter_map(|key| {
            (key.member_name() == query.member_name()
                && key.kind() == query.kind()
                && (key.staticness() == query.staticness()
                    || key.staticness() == Staticness::Unknown
                    || query.staticness() == Staticness::Unknown)
                && key.arity() == query.arity()
                && (key.generic_arity().is_none()
                    || query.generic_arity().is_none()
                    || key.generic_arity() == query.generic_arity()))
            .then_some(key.owner_type())
        }),
        query.owner_type(),
    )
}

fn find_facade_abi_for_call<'a>(
    facts: impl Iterator<Item = &'a FacadeAbiFact>,
    arg_types: &[JavaType],
) -> Option<&'a FacadeAbiFact> {
    facts
        .filter(|fact| callable_key_params_match_java_types(fact.key.params(), arg_types))
        .max_by_key(|fact| {
            (
                fact.provenance == FactProvenance::UserOverride,
                callable_key_param_specificity(fact.key.params()),
            )
        })
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
        TypePattern::Exact(type_key) => type_key_matches_java_type(type_key.as_str(), java_ty),
        TypePattern::Erased(type_key) => {
            let erased = erase_java_type_for_pattern(java_ty);
            type_key_matches_java_type(type_key.as_str(), &erased)
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
        JavaType::Intersection(bounds) => bounds
            .iter()
            .any(|bound| type_key_matches_java_type(normalized, bound)),
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

fn external_optional_type_name_directly_matches(
    fact_type_name: Option<&str>,
    query_type_name: Option<&str>,
) -> bool {
    match (fact_type_name, query_type_name) {
        (Some(fact_type_name), Some(query_type_name)) => {
            external_type_name_directly_matches(fact_type_name, query_type_name)
        }
        (None, None) => true,
        _ => false,
    }
}

fn external_method_name_matches(fact_method_name: &str, query_method_name: &str) -> bool {
    if fact_method_name == query_method_name {
        return true;
    }
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

/// Every spelling an external type name can be matched under.
///
/// Memoized because it is a **pure function of its argument** that sits on the
/// hottest path in the translator: `external_type_name_matches` is called once
/// per external-pack member during a linear scan, and it recomputed the alias
/// list — `to_snake_case`, `canonical_external_receiver_type_key`, and several
/// `String` allocations — for *both* names on every comparison.
///
/// Profiled on a seven-method file with one context-summary source: 38% of
/// samples under `external_type_name_matches`, with `to_snake_case` (14.6%),
/// `canonical_external_receiver_type_key` (13.8%) and ~25% in
/// `malloc`/`free`/`realloc` as the self-time leaders. The names repeat
/// constantly across the scan, so almost all of that work was recomputation.
///
/// Safe to cache per thread precisely because it depends on nothing but the
/// string: no facts, no program, no configuration.
fn external_type_name_aliases(type_name: &str) -> Rc<[String]> {
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<String, Rc<[String]>>> =
            std::cell::RefCell::new(HashMap::new());
    }
    if let Some(cached) = CACHE.with(|cache| cache.borrow().get(type_name).cloned()) {
        return cached;
    }
    let aliases = Rc::<[String]>::from(external_type_name_aliases_uncached(type_name));
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .insert(type_name.to_string(), aliases.clone())
    });
    aliases
}

fn external_type_name_aliases_uncached(type_name: &str) -> Vec<String> {
    let mut aliases = external_direct_type_name_aliases(type_name).to_vec();
    let canonical = crate::stdlib::canonical_external_receiver_type_key(type_name);
    aliases.extend(nested_type_suffix_aliases(type_name));
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

fn nested_type_leaf_alias(type_name: &str) -> bool {
    let Some((prefix, _simple)) = type_name.rsplit_once('.') else {
        return false;
    };
    !prefix
        .split('.')
        .any(|segment| segment.chars().next().is_some_and(char::is_lowercase))
}

fn external_type_name_directly_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    if fact_type_name == query_type_name {
        return true;
    }
    let fact_aliases = external_direct_type_name_aliases(fact_type_name);
    let query_aliases = external_direct_type_name_aliases(query_type_name);
    fact_aliases.iter().any(|fact_alias| {
        query_aliases
            .iter()
            .any(|query_alias| query_alias == fact_alias)
    })
}

fn external_direct_type_name_aliases(type_name: &str) -> Rc<[String]> {
    // Direct matching intentionally excludes leaf/nested suffix aliases.
    // Its spelling calculation is nevertheless just as pure as the broader
    // alias calculation, and is shared by every method-effect lookup.
    thread_local! {
        static CACHE: RefCell<HashMap<String, Rc<[String]>>> = RefCell::new(HashMap::new());
    }
    if let Some(aliases) = CACHE.with(|cache| cache.borrow().get(type_name).cloned()) {
        return aliases;
    }
    let canonical = crate::stdlib::canonical_external_receiver_type_key(type_name);
    let snake = crate::parser::to_snake_case(type_name);
    let mut aliases = vec![type_name.to_string(), canonical, snake];
    aliases.sort();
    aliases.dedup();
    let aliases = Rc::<[String]>::from(aliases);
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .insert(type_name.to_string(), aliases.clone())
    });
    aliases
}

fn external_field_name_matches(fact_field_name: &str, query_field_name: &str) -> bool {
    if fact_field_name == query_field_name {
        return true;
    }
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

fn method_key_directly_matches(method: &MethodKey, type_name: &str, method_name: &str) -> bool {
    external_type_name_directly_matches(&method.type_name, type_name)
        && (method.method_name == method_name
            || crate::parser::to_snake_case(&method.method_name) == method_name)
}

fn method_key_matches(method: &MethodKey, type_name: &str, method_name: &str) -> bool {
    external_type_name_matches(&method.type_name, type_name)
        && (method.method_name == method_name
            || crate::parser::to_snake_case(&method.method_name) == method_name)
}

fn method_key_simple_owner_alias_is_ambiguous<'a>(
    methods: impl Iterator<Item = &'a MethodKey>,
    type_name: &str,
    method_name: &str,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        methods.filter_map(|method| {
            (method.method_name == method_name
                || crate::parser::to_snake_case(&method.method_name) == method_name)
                .then_some(method.type_name.as_str())
        }),
        type_name,
    )
}

fn source_equals_override_simple_owner_alias_is_ambiguous(
    facts: &[SourceEqualsOverrideFact],
    type_name: &str,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().map(|fact| fact.type_name.as_str()),
        type_name,
    )
}

fn dispatch_requirement_simple_owner_alias_is_ambiguous(
    facts: &[DispatchRequirementFact],
    type_name: &str,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().map(|fact| fact.type_name.as_str()),
        type_name,
    )
}

fn exception_simple_owner_alias_is_ambiguous(facts: &[ExceptionFact], type_name: &str) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().map(|fact| fact.type_name.as_str()),
        type_name,
    )
}

fn field_nullability_simple_owner_alias_is_ambiguous(
    facts: &[FieldNullabilityFact],
    type_name: &str,
    field_name: &str,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            external_field_name_matches(&fact.field.field_name, field_name)
                .then_some(fact.field.type_name.as_str())
        }),
        type_name,
    )
}

fn trivial_accessor_simple_owner_alias_is_ambiguous(
    facts: &[TrivialAccessorFact],
    type_name: &str,
    method_name: &str,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (fact.method.method_name == method_name
                || crate::parser::to_snake_case(&fact.method.method_name) == method_name)
                .then_some(fact.method.type_name.as_str())
        }),
        type_name,
    )
}

fn external_constructor_simple_owner_alias_is_ambiguous(
    facts: &[ExternalConstructorFact],
    type_name: &str,
    arg_types: &[JavaType],
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (fact.arg_types == arg_types
                || fact.provenance == FactProvenance::UserOverride
                || (fact.provenance == FactProvenance::ExternalLibraryPack
                    && fact.arg_types.len() == arg_types.len()))
            .then_some(fact.type_name.as_str())
        }),
        type_name,
    )
}

fn find_external_constructor_argument_effect_for<'a>(
    facts: &'a [ExternalConstructorArgumentEffectFact],
    type_name: &str,
    arg_count: usize,
    parameter_index: usize,
    provenance: FactProvenance,
) -> Option<&'a ExternalConstructorArgumentEffectFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            fact.type_name == type_name
                && fact.arg_count == arg_count
                && fact.parameter_index == parameter_index
                && fact.provenance == provenance
        })
        .or_else(|| {
            facts.iter().rev().find(|fact| {
                external_type_name_directly_matches(&fact.type_name, type_name)
                    && fact.arg_count == arg_count
                    && fact.parameter_index == parameter_index
                    && fact.provenance == provenance
            })
        })
        .or_else(|| {
            if external_constructor_argument_effect_simple_owner_alias_is_ambiguous(
                facts,
                type_name,
                arg_count,
                parameter_index,
                provenance,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_type_name_matches(&fact.type_name, type_name)
                    && fact.arg_count == arg_count
                    && fact.parameter_index == parameter_index
                    && fact.provenance == provenance
            })
        })
}

fn external_constructor_argument_effect_simple_owner_alias_is_ambiguous(
    facts: &[ExternalConstructorArgumentEffectFact],
    type_name: &str,
    arg_count: usize,
    parameter_index: usize,
    provenance: FactProvenance,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (fact.arg_count == arg_count
                && fact.parameter_index == parameter_index
                && fact.provenance == provenance)
                .then_some(fact.type_name.as_str())
        }),
        type_name,
    )
}

fn find_external_api_argument_effect_for_receiver<'a>(
    facts: &'a [ExternalApiArgumentEffectFact],
    receiver_type: Option<&str>,
    method_name: &str,
    arity_matches: impl Fn(&ExternalApiArgumentEffectFact) -> bool + Copy,
    parameter_index: usize,
    provenance: Option<FactProvenance>,
) -> Option<&'a ExternalApiArgumentEffectFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            external_method_name_matches(&fact.method_name, method_name)
                && arity_matches(fact)
                && external_optional_type_name_directly_matches(
                    fact.receiver_type.as_deref(),
                    receiver_type,
                )
                && fact.parameter_index == parameter_index
                && provenance.is_none_or(|provenance| fact.provenance == provenance)
        })
        .or_else(|| {
            if external_api_argument_effect_simple_owner_alias_is_ambiguous(
                facts,
                receiver_type,
                method_name,
                arity_matches,
                parameter_index,
                provenance,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_method_name_matches(&fact.method_name, method_name)
                    && arity_matches(fact)
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
                    && fact.parameter_index == parameter_index
                    && provenance.is_none_or(|provenance| fact.provenance == provenance)
            })
        })
}

fn external_api_argument_effect_simple_owner_alias_is_ambiguous(
    facts: &[ExternalApiArgumentEffectFact],
    receiver_type: Option<&str>,
    method_name: &str,
    arity_matches: impl Fn(&ExternalApiArgumentEffectFact) -> bool,
    parameter_index: usize,
    provenance: Option<FactProvenance>,
) -> bool {
    let Some(receiver_type) = receiver_type else {
        return false;
    };
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (external_method_name_matches(&fact.method_name, method_name)
                && arity_matches(fact)
                && fact.parameter_index == parameter_index
                && provenance.is_none_or(|provenance| fact.provenance == provenance))
            .then_some(fact.receiver_type.as_deref())
            .flatten()
        }),
        receiver_type,
    )
}

fn external_api_effect_receiver_matches_for_fact(
    fact: &ExternalApiEffectFact,
    receiver_type: Option<&str>,
    broad_match_is_ambiguous: bool,
    external_pack_directly_owns_receiver: bool,
) -> bool {
    if optional_external_type_name_directly_matches(fact.receiver_type.as_deref(), receiver_type) {
        return true;
    }
    if fact.provenance == FactProvenance::ExternalLibraryPack
        && !external_pack_directly_owns_receiver
        && receiver_type.is_some_and(|receiver_type| !receiver_type.contains('.'))
        && fact
            .receiver_type
            .as_deref()
            .is_some_and(nested_type_leaf_alias)
    {
        return false;
    }
    !broad_match_is_ambiguous
        && external_optional_type_name_matches(fact.receiver_type.as_deref(), receiver_type)
}

fn find_external_crate_api_mapping_for_receiver<'a>(
    facts: &'a [ExternalCrateApiMappingFact],
    receiver_type: &str,
    method_name: &str,
    arg_count: Option<usize>,
    provenance: Option<FactProvenance>,
) -> Option<&'a ExternalCrateApiMappingFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            external_crate_api_mapping_signature_matches(fact, method_name, arg_count, provenance)
                && external_type_name_directly_matches(&fact.receiver_type, receiver_type)
        })
        .or_else(|| {
            if external_crate_api_mapping_simple_owner_alias_is_ambiguous(
                facts,
                receiver_type,
                method_name,
                arg_count,
                provenance,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_crate_api_mapping_signature_matches(
                    fact,
                    method_name,
                    arg_count,
                    provenance,
                ) && external_type_name_matches(&fact.receiver_type, receiver_type)
            })
        })
}

fn external_crate_api_mapping_signature_matches(
    fact: &ExternalCrateApiMappingFact,
    method_name: &str,
    arg_count: Option<usize>,
    provenance: Option<FactProvenance>,
) -> bool {
    fact.method_name == method_name
        && fact.arg_count == arg_count
        && provenance.is_none_or(|provenance| fact.provenance == provenance)
}

fn external_crate_api_mapping_simple_owner_alias_is_ambiguous(
    facts: &[ExternalCrateApiMappingFact],
    receiver_type: &str,
    method_name: &str,
    arg_count: Option<usize>,
    provenance: Option<FactProvenance>,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            external_crate_api_mapping_signature_matches(fact, method_name, arg_count, provenance)
                .then_some(fact.receiver_type.as_str())
        }),
        receiver_type,
    )
}

fn find_external_crate_api_mapping_for_rust_name<'a>(
    facts: &'a [ExternalCrateApiMappingFact],
    receiver_type: &str,
    rust_name: &str,
    arg_count: Option<usize>,
) -> Option<&'a ExternalCrateApiMappingFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            external_crate_api_rust_name_mapping_signature_matches(fact, rust_name, arg_count)
                && external_type_name_directly_matches(&fact.receiver_type, receiver_type)
        })
        .or_else(|| {
            if external_crate_api_rust_name_mapping_simple_owner_alias_is_ambiguous(
                facts,
                receiver_type,
                rust_name,
                arg_count,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_crate_api_rust_name_mapping_signature_matches(fact, rust_name, arg_count)
                    && external_type_name_matches(&fact.receiver_type, receiver_type)
            })
        })
}

fn external_crate_api_rust_name_mapping_signature_matches(
    fact: &ExternalCrateApiMappingFact,
    rust_name: &str,
    arg_count: Option<usize>,
) -> bool {
    fact.rust_name == rust_name && fact.arg_count == arg_count
}

fn external_crate_api_rust_name_mapping_simple_owner_alias_is_ambiguous(
    facts: &[ExternalCrateApiMappingFact],
    receiver_type: &str,
    rust_name: &str,
    arg_count: Option<usize>,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            external_crate_api_rust_name_mapping_signature_matches(fact, rust_name, arg_count)
                .then_some(fact.receiver_type.as_str())
        }),
        receiver_type,
    )
}

fn find_external_crate_constructor_mapping_for<'a>(
    facts: &'a [ExternalCrateConstructorMappingFact],
    type_name: &str,
    arg_count: usize,
    provenance: Option<FactProvenance>,
) -> Option<&'a ExternalCrateConstructorMappingFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            external_crate_constructor_mapping_signature_matches(fact, arg_count, provenance)
                && external_type_name_directly_matches(&fact.type_name, type_name)
        })
        .or_else(|| {
            if external_crate_constructor_mapping_simple_owner_alias_is_ambiguous(
                facts, type_name, arg_count, provenance,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_crate_constructor_mapping_signature_matches(fact, arg_count, provenance)
                    && external_type_name_matches(&fact.type_name, type_name)
            })
        })
}

fn find_external_crate_constructor_type_mapping_for<'a>(
    facts: &'a [ExternalCrateConstructorMappingFact],
    type_name: &str,
    provenance: Option<FactProvenance>,
) -> Option<&'a ExternalCrateConstructorMappingFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            fact.rust_type_name.is_some()
                && provenance.is_none_or(|provenance| fact.provenance == provenance)
                && external_type_name_directly_matches(&fact.type_name, type_name)
        })
        .or_else(|| {
            if external_crate_constructor_type_mapping_simple_owner_alias_is_ambiguous(
                facts, type_name, provenance,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                fact.rust_type_name.is_some()
                    && provenance.is_none_or(|provenance| fact.provenance == provenance)
                    && external_type_name_matches(&fact.type_name, type_name)
            })
        })
}

fn external_crate_constructor_mapping_signature_matches(
    fact: &ExternalCrateConstructorMappingFact,
    arg_count: usize,
    provenance: Option<FactProvenance>,
) -> bool {
    fact.arg_count == arg_count && provenance.is_none_or(|provenance| fact.provenance == provenance)
}

fn external_crate_constructor_mapping_simple_owner_alias_is_ambiguous(
    facts: &[ExternalCrateConstructorMappingFact],
    type_name: &str,
    arg_count: usize,
    provenance: Option<FactProvenance>,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            external_crate_constructor_mapping_signature_matches(fact, arg_count, provenance)
                .then_some(fact.type_name.as_str())
        }),
        type_name,
    )
}

fn external_crate_constructor_type_mapping_simple_owner_alias_is_ambiguous(
    facts: &[ExternalCrateConstructorMappingFact],
    type_name: &str,
    provenance: Option<FactProvenance>,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (fact.rust_type_name.is_some()
                && provenance.is_none_or(|provenance| fact.provenance == provenance))
            .then_some(fact.type_name.as_str())
        }),
        type_name,
    )
}

fn find_external_crate_type_mapping_for<'a>(
    facts: &'a [ExternalCrateTypeMappingFact],
    type_name: &str,
    provenance: Option<FactProvenance>,
) -> Option<&'a ExternalCrateTypeMappingFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            provenance.is_none_or(|provenance| fact.provenance == provenance)
                && external_type_name_directly_matches(&fact.type_name, type_name)
        })
        .or_else(|| {
            if external_crate_type_mapping_simple_owner_alias_is_ambiguous(
                facts, type_name, provenance,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                provenance.is_none_or(|provenance| fact.provenance == provenance)
                    && external_type_name_matches(&fact.type_name, type_name)
            })
        })
}

fn external_crate_type_mapping_simple_owner_alias_is_ambiguous(
    facts: &[ExternalCrateTypeMappingFact],
    type_name: &str,
    provenance: Option<FactProvenance>,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            provenance
                .is_none_or(|provenance| fact.provenance == provenance)
                .then_some(fact.type_name.as_str())
        }),
        type_name,
    )
}

fn find_external_ffi_handover_for_receiver<'a>(
    facts: &'a [ExternalFfiHandoverFact],
    receiver_type: Option<&str>,
    method_name: &str,
    slot: ExternalFfiHandoverSlot,
    parameter_type: &JavaType,
) -> Option<&'a ExternalFfiHandoverFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            external_ffi_handover_signature_matches(fact, method_name, slot, parameter_type)
                && fact.receiver_type.as_deref() == receiver_type
        })
        .or_else(|| {
            facts.iter().rev().find(|fact| {
                external_ffi_handover_signature_matches(fact, method_name, slot, parameter_type)
                    && optional_external_type_name_directly_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
            })
        })
        .or_else(|| {
            if external_ffi_handover_simple_owner_alias_is_ambiguous(
                facts,
                receiver_type,
                method_name,
                slot,
                parameter_type,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_ffi_handover_signature_matches(fact, method_name, slot, parameter_type)
                    && external_optional_type_name_matches(
                        fact.receiver_type.as_deref(),
                        receiver_type,
                    )
            })
        })
        .or_else(|| {
            facts.iter().rev().find(|fact| {
                external_ffi_handover_signature_matches(fact, method_name, slot, parameter_type)
                    && fact.receiver_type.is_none()
            })
        })
}

fn external_ffi_handover_signature_matches(
    fact: &ExternalFfiHandoverFact,
    method_name: &str,
    slot: ExternalFfiHandoverSlot,
    parameter_type: &JavaType,
) -> bool {
    fact.method_name == method_name
        && fact.slot == slot
        && fact
            .parameter_type
            .as_ref()
            .is_none_or(|fact_ty| fact_ty == parameter_type)
}

fn external_ffi_handover_simple_owner_alias_is_ambiguous(
    facts: &[ExternalFfiHandoverFact],
    receiver_type: Option<&str>,
    method_name: &str,
    slot: ExternalFfiHandoverSlot,
    parameter_type: &JavaType,
) -> bool {
    let Some(receiver_type) = receiver_type else {
        return false;
    };
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            external_ffi_handover_signature_matches(fact, method_name, slot, parameter_type)
                .then_some(fact.receiver_type.as_deref())
                .flatten()
        }),
        receiver_type,
    )
}

fn find_external_rust_result_for_receiver<'a>(
    facts: &'a [ExternalRustResultFact],
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
    user_override_only: bool,
) -> Option<&'a ExternalRustResultFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            external_rust_result_signature_matches(fact, method_name, arg_count, user_override_only)
                && fact.receiver_type.as_deref() == receiver_type
        })
        .or_else(|| {
            facts.iter().rev().find(|fact| {
                external_rust_result_signature_matches(
                    fact,
                    method_name,
                    arg_count,
                    user_override_only,
                ) && optional_external_type_name_directly_matches(
                    fact.receiver_type.as_deref(),
                    receiver_type,
                )
            })
        })
        .or_else(|| {
            if external_rust_result_simple_owner_alias_is_ambiguous(
                facts,
                receiver_type,
                method_name,
                arg_count,
                user_override_only,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_rust_result_signature_matches(
                    fact,
                    method_name,
                    arg_count,
                    user_override_only,
                ) && external_optional_type_name_matches(
                    fact.receiver_type.as_deref(),
                    receiver_type,
                )
            })
        })
}

fn external_rust_result_signature_matches(
    fact: &ExternalRustResultFact,
    method_name: &str,
    arg_count: Option<usize>,
    user_override_only: bool,
) -> bool {
    fact.method_name == method_name
        && fact.arg_count == arg_count
        && (!user_override_only || fact.provenance == FactProvenance::UserOverride)
}

fn external_rust_result_simple_owner_alias_is_ambiguous(
    facts: &[ExternalRustResultFact],
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
    user_override_only: bool,
) -> bool {
    let Some(receiver_type) = receiver_type else {
        return false;
    };
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            external_rust_result_signature_matches(fact, method_name, arg_count, user_override_only)
                .then_some(fact.receiver_type.as_deref())
                .flatten()
        }),
        receiver_type,
    )
}

fn find_external_rust_return_type_for_receiver<'a>(
    facts: &'a [ExternalRustReturnTypeFact],
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
    user_override_only: bool,
) -> Option<&'a ExternalRustReturnTypeFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            external_rust_return_type_signature_matches(
                fact,
                method_name,
                arg_count,
                user_override_only,
            ) && fact.receiver_type.as_deref() == receiver_type
        })
        .or_else(|| {
            facts.iter().rev().find(|fact| {
                external_rust_return_type_signature_matches(
                    fact,
                    method_name,
                    arg_count,
                    user_override_only,
                ) && optional_external_type_name_directly_matches(
                    fact.receiver_type.as_deref(),
                    receiver_type,
                )
            })
        })
        .or_else(|| {
            if external_rust_return_type_simple_owner_alias_is_ambiguous(
                facts,
                receiver_type,
                method_name,
                arg_count,
                user_override_only,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_rust_return_type_signature_matches(
                    fact,
                    method_name,
                    arg_count,
                    user_override_only,
                ) && external_optional_type_name_matches(
                    fact.receiver_type.as_deref(),
                    receiver_type,
                )
            })
        })
}

fn external_rust_return_type_signature_matches(
    fact: &ExternalRustReturnTypeFact,
    method_name: &str,
    arg_count: Option<usize>,
    user_override_only: bool,
) -> bool {
    fact.method_name == method_name
        && fact.arg_count == arg_count
        && (!user_override_only || fact.provenance == FactProvenance::UserOverride)
}

fn external_rust_return_type_simple_owner_alias_is_ambiguous(
    facts: &[ExternalRustReturnTypeFact],
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
    user_override_only: bool,
) -> bool {
    let Some(receiver_type) = receiver_type else {
        return false;
    };
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            external_rust_return_type_signature_matches(
                fact,
                method_name,
                arg_count,
                user_override_only,
            )
            .then_some(fact.receiver_type.as_deref())
            .flatten()
        }),
        receiver_type,
    )
}

fn find_external_constructor_rust_result_for<'a>(
    facts: &'a [ExternalConstructorRustResultFact],
    type_name: &str,
    arg_count: usize,
    provenance: FactProvenance,
) -> Option<&'a ExternalConstructorRustResultFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            fact.type_name == type_name
                && fact.arg_count == arg_count
                && fact.provenance == provenance
        })
        .or_else(|| {
            facts.iter().rev().find(|fact| {
                external_type_name_directly_matches(&fact.type_name, type_name)
                    && fact.arg_count == arg_count
                    && fact.provenance == provenance
            })
        })
        .or_else(|| {
            if external_constructor_rust_result_simple_owner_alias_is_ambiguous(
                facts, type_name, arg_count, provenance,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_type_name_matches(&fact.type_name, type_name)
                    && fact.arg_count == arg_count
                    && fact.provenance == provenance
            })
        })
}

fn external_constructor_rust_result_simple_owner_alias_is_ambiguous(
    facts: &[ExternalConstructorRustResultFact],
    type_name: &str,
    arg_count: usize,
    provenance: FactProvenance,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (fact.arg_count == arg_count && fact.provenance == provenance)
                .then_some(fact.type_name.as_str())
        }),
        type_name,
    )
}

fn find_external_conversion_adapter_for<'a>(
    facts: &'a [ExternalConversionAdapterFact],
    from_type: &str,
    to_type: &str,
    provenance: FactProvenance,
) -> Option<&'a ExternalConversionAdapterFact> {
    facts
        .iter()
        .rev()
        .find(|fact| {
            fact.from_type == from_type && fact.to_type == to_type && fact.provenance == provenance
        })
        .or_else(|| {
            facts.iter().rev().find(|fact| {
                external_type_name_directly_matches(&fact.from_type, from_type)
                    && external_type_name_directly_matches(&fact.to_type, to_type)
                    && fact.provenance == provenance
            })
        })
        .or_else(|| {
            if external_conversion_adapter_simple_endpoint_alias_is_ambiguous(
                facts, from_type, to_type, provenance,
            ) {
                return None;
            }
            facts.iter().rev().find(|fact| {
                external_type_name_matches(&fact.from_type, from_type)
                    && external_type_name_matches(&fact.to_type, to_type)
                    && fact.provenance == provenance
            })
        })
}

fn external_conversion_adapter_simple_endpoint_alias_is_ambiguous(
    facts: &[ExternalConversionAdapterFact],
    from_type: &str,
    to_type: &str,
    provenance: FactProvenance,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (fact.provenance == provenance && external_type_name_matches(&fact.to_type, to_type))
                .then_some(fact.from_type.as_str())
        }),
        from_type,
    ) || package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (fact.provenance == provenance
                && external_type_name_matches(&fact.from_type, from_type))
            .then_some(fact.to_type.as_str())
        }),
        to_type,
    )
}

fn optional_external_type_name_directly_matches(
    fact_type_name: Option<&str>,
    query_type_name: Option<&str>,
) -> bool {
    match (fact_type_name, query_type_name) {
        (Some(fact_type_name), Some(query_type_name)) => {
            external_type_name_directly_matches(fact_type_name, query_type_name)
        }
        (None, None) => true,
        _ => false,
    }
}

fn unsupported_external_constructor_simple_owner_alias_is_ambiguous(
    facts: &[UnsupportedExternalConstructorFact],
    type_name: &str,
    arg_count: usize,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts
            .iter()
            .filter_map(|fact| (fact.arg_count == arg_count).then_some(fact.type_name.as_str())),
        type_name,
    )
}

fn unsupported_external_api_simple_owner_alias_is_ambiguous(
    facts: &[UnsupportedExternalApiFact],
    receiver_type: &str,
    method_name: &str,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            (fact.method_name == method_name).then_some(fact.receiver_type.as_str())
        }),
        receiver_type,
    )
}

fn unsupported_external_field_simple_owner_alias_is_ambiguous(
    facts: &[UnsupportedExternalFieldFact],
    type_name: &str,
    field_name: &str,
) -> bool {
    package_simple_owner_alias_is_ambiguous(
        facts.iter().filter_map(|fact| {
            external_field_name_matches(&fact.field.field_name, field_name)
                .then_some(fact.field.type_name.as_str())
        }),
        type_name,
    )
}

fn package_simple_owner_alias_is_ambiguous<'a>(
    owners: impl Iterator<Item = &'a str>,
    type_name: &str,
) -> bool {
    let mut package_owners = HashSet::new();
    for owner in owners {
        let Some(simple_owner) = package_qualified_simple_alias(owner) else {
            continue;
        };
        if external_type_name_directly_matches(simple_owner, type_name) {
            package_owners.insert(owner);
            if package_owners.len() > 1 {
                return true;
            }
        }
    }
    false
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
        diagnostics.push(FactDiagnostic::new(
            "type_capability_override_conflict",
            format!(
                "type capability override for {} conflicts with source-proven implements facts",
                user_override.type_name
            ),
        ));
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
        diagnostics.push(FactDiagnostic::new(
            "type_capability_override_conflict",
            format!(
                "type capability override for {} sets {field}={override_value}, conflicting with source-proven {field}={source_value}",
                override_fact.type_name
            ),
        ));
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
        diagnostics.push(FactDiagnostic::new(
            "type_capability_override_conflict",
            format!(
                "type capability override for {} sets {field}={override_value:?}, conflicting with source-proven {field}={source_value:?}",
                override_fact.type_name
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::PrimitiveType;

    #[test]
    fn callable_header_prefilter_preserves_direct_and_alias_matching() {
        let mut keys = Vec::new();
        for owner in [
            "pkg.Outer.Inner",
            "Outer.Inner",
            "Outer_Inner",
            "other.Inner",
        ] {
            for member in ["read", "write"] {
                for staticness in [
                    Staticness::Static,
                    Staticness::Instance,
                    Staticness::Unknown,
                ] {
                    for arity in [0, 1] {
                        for kind in [CallableKind::Method, CallableKind::Constructor] {
                            for generic_arity in [None, Some(0), Some(1)] {
                                let mut key = CallableKey::method(owner, member, staticness, arity);
                                key.kind = kind;
                                key.generic_arity = generic_arity;
                                keys.push(key);
                            }
                        }
                    }
                }
            }
        }
        for fact in &keys {
            for query in &keys {
                // Keep an independent copy of the previous owner-first rule.
                let reference = |owner_matches: bool| {
                    owner_matches
                        && fact.member_name() == query.member_name()
                        && fact.kind() == query.kind()
                        && (fact.staticness() == query.staticness()
                            || fact.staticness() == Staticness::Unknown
                            || query.staticness() == Staticness::Unknown)
                        && fact.arity() == query.arity()
                        && (fact.generic_arity().is_none()
                            || query.generic_arity().is_none()
                            || fact.generic_arity() == query.generic_arity())
                };
                assert_eq!(
                    callable_key_header_matches(fact, query),
                    reference(external_type_name_matches(
                        fact.owner_type(),
                        query.owner_type()
                    )),
                    "alias {fact:?} {query:?}"
                );
                assert_eq!(
                    callable_key_header_directly_matches(fact, query),
                    reference(external_type_name_directly_matches(
                        fact.owner_type(),
                        query.owner_type()
                    )),
                    "direct {fact:?} {query:?}"
                );
            }
        }
    }

    #[test]
    fn hierarchy_lookup_index_saturates_only_queried_types_and_memoizes_them() {
        let mut facts = FactSet::default();
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: "pkg.Child".to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: Some(JavaType::Named {
                    name: "pkg.Parent".to_string(),
                    args: Vec::new(),
                }),
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::ParsedSource,
            });
        let child = JavaType::Named {
            name: "pkg.Child".to_string(),
            args: Vec::new(),
        };
        let index = facts.hierarchy_lookup_index();

        assert!(index.reachable_by_sub_name.borrow().is_empty());
        let first = index.reachable_hierarchy_edges_for_type_with_stats(&child);
        assert_eq!(index.reachable_by_sub_name.borrow().len(), 1);
        let second = index.reachable_hierarchy_edges_for_type_with_stats(&child);
        assert_eq!(index.reachable_by_sub_name.borrow().len(), 1);
        assert_eq!(first, second);
    }

    #[test]
    fn receiver_mutability_join_does_not_let_unknown_summary_hide_mutating_body() {
        let mut facts = FactSet::default();
        for receiver_mutability in [
            ReceiverMutabilityFact::MutatesReceiver,
            ReceiverMutabilityFact::Unknown,
        ] {
            facts.method_effect_facts.push(MethodEffectFact {
                method: MethodKey::named("Worker", "run"),
                receiver_mutability,
                return_nullability: NullabilityState::Unknown,
                thrown: Vec::new(),
                provenance: FactProvenance::ParsedSource,
            });
        }

        assert_eq!(
            facts.method_receiver_mutability_for("Worker", "run"),
            Some(ReceiverMutabilityFact::MutatesReceiver)
        );
    }

    #[test]
    fn external_api_effect_rust_name_fallback_refuses_ambiguous_simple_owner_alias() {
        let mut facts = FactSet::default();
        for (owner, java_method, return_type) in [
            ("a.Item", "a_java_method", JavaType::String),
            (
                "b.Item",
                "b_java_method",
                JavaType::Primitive(PrimitiveType::Int),
            ),
        ] {
            facts
                .external_crate_api_mapping_facts
                .push(ExternalCrateApiMappingFact {
                    receiver_type: owner.to_string(),
                    method_name: java_method.to_string(),
                    arg_count: Some(1),
                    rust_name: "shared_rust_name".to_string(),
                    provenance: FactProvenance::ExternalLibraryPack,
                });
            facts.external_api_effect_facts.push(ExternalApiEffectFact {
                method_name: java_method.to_string(),
                receiver_type: Some(owner.to_string()),
                arg_count: Some(1),
                parameter_patterns: None,
                role: ExternalApiRole::Getter,
                receiver_mutability: ReceiverMutabilityFact::ReadOnly,
                return_type: Some(return_type),
                return_nullability: NullabilityState::DefinitelyNonNull,
                thrown: Vec::new(),
                provenance: FactProvenance::ExternalLibraryPack,
            });
        }

        assert_eq!(
            facts
                .external_api_effect_for_receiver_with_arity_or_rust_name(
                    Some("a.Item"),
                    "shared_rust_name",
                    Some(1),
                )
                .and_then(|fact| fact.return_type.as_ref()),
            Some(&JavaType::String)
        );
        assert_eq!(
            facts
                .external_api_effect_for_receiver_with_arity_or_rust_name(
                    Some("b.Item"),
                    "shared_rust_name",
                    Some(1),
                )
                .and_then(|fact| fact.return_type.as_ref()),
            Some(&JavaType::Primitive(PrimitiveType::Int))
        );
        assert!(
            facts
                .external_api_effect_for_receiver_with_arity_or_rust_name(
                    Some("Item"),
                    "shared_rust_name",
                    Some(1),
                )
                .is_none()
        );
    }

    #[test]
    fn mapped_receiver_effect_lookup_preserves_forward_fact_precedence() {
        let mut facts = FactSet::default();
        for return_type in [JavaType::String, JavaType::Primitive(PrimitiveType::Int)] {
            facts.external_api_effect_facts.push(ExternalApiEffectFact {
                method_name: "value".to_string(),
                receiver_type: Some("example.JavaOwner".to_string()),
                arg_count: Some(0),
                parameter_patterns: None,
                role: ExternalApiRole::Getter,
                receiver_mutability: ReceiverMutabilityFact::ReadOnly,
                return_type: Some(return_type),
                return_nullability: NullabilityState::DefinitelyNonNull,
                thrown: Vec::new(),
                provenance: FactProvenance::ExternalLibraryPack,
            });
        }

        let index = facts.external_lookup_index();
        let effect = index
            .external_api_effect_for_mapped_receiver_with_arity("value", 0, |receiver| {
                receiver == "example.JavaOwner"
            })
            .expect("mapped receiver should resolve");
        assert_eq!(effect.return_type, Some(JavaType::String));
    }

    #[test]
    fn external_type_name_set_index_matches_pairwise_alias_membership() {
        let names = [
            "org.jgrapht.Graph".to_string(),
            "Outer.Inner".to_string(),
            "java.io.InputStream".to_string(),
        ];
        let index = ExternalTypeNameSetIndex::new(&names);
        for query in [
            "Graph",
            "org_jgrapht_graph",
            "Outer$Inner",
            "JavaIoInputStream",
            "UnrelatedType",
        ] {
            assert_eq!(
                index.matches(query),
                names
                    .iter()
                    .any(|name| external_type_name_matches(name, query)),
                "indexed membership diverged for {query}"
            );
        }
    }

    #[test]
    fn external_type_name_lookup_index_matches_ordered_pairwise_rows() {
        let names = [
            "example.BoxedThing",
            "BoxedThing",
            "example.Outer.Inner",
            "other.BoxedThing",
            "example.BoxedThing",
        ];
        let index = ExternalTypeNameLookupIndex::new(names);
        for query in [
            "BoxedThing",
            "example.BoxedThing",
            "example_boxed_thing",
            "Outer$Inner",
            "Inner",
            "example.Outer.Inner",
            "Missing",
        ] {
            let expected = names
                .iter()
                .enumerate()
                .filter_map(|(index, name)| {
                    external_type_name_matches(name, query).then_some(index)
                })
                .collect::<Vec<_>>();
            let actual = index.matching_indices(query);
            assert_eq!(actual.as_ref(), expected.as_slice(), "{query}");
            assert!(Rc::ptr_eq(&actual, &index.matching_indices(query)));
        }
    }

    #[test]
    fn direct_type_alias_memo_preserves_qualified_owner_distinctions() {
        for name in [
            "example.BoxedThing",
            "other.BoxedThing",
            "Outer.Inner",
            "Outer$Inner",
        ] {
            let mut expected = vec![
                name.to_string(),
                crate::stdlib::canonical_external_receiver_type_key(name),
                crate::parser::to_snake_case(name),
            ];
            expected.sort();
            expected.dedup();
            let actual = external_direct_type_name_aliases(name);
            assert_eq!(actual.as_ref(), expected.as_slice());
            assert!(Rc::ptr_eq(
                &actual,
                &external_direct_type_name_aliases(name)
            ));
        }
        assert!(!external_type_name_directly_matches(
            "example.BoxedThing",
            "other.BoxedThing"
        ));
        assert!(!external_type_name_directly_matches(
            "example.BoxedThing",
            "BoxedThing"
        ));
    }
}
