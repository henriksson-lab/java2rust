use crate::facts::{
    DispatchRequirementKind, ExternalFactLookupIndex, ExternalLibraryPackFact, FactProvenance,
    FactSet, HierarchyEdgeFact, HierarchyOverride, HierarchyStrategy, ObjectSafetyFact,
    ObjectSafetyViolation, OwnedRecoveryCapability, OwnedRecoveryCapabilityFact,
    TypeCapabilityOverride,
};
use crate::java_ir::JavaType;
use crate::slot_shape::TypeCapabilities;
use crate::types::{
    contains_wildcard_type, is_known_raw_generic_type, is_optional_or_result_type,
    is_raw_generic_counterpart, is_unchecked_generic_counterpart,
};
use std::{cell::RefCell, ops::Range, rc::Rc};

#[derive(Default)]
struct TypeFactLookupSession {
    facts_address: usize,
    capabilities: Option<crate::facts::ExternalTypeNameLookupIndex>,
    hierarchy: Option<crate::facts::ExternalTypeNameLookupIndex>,
    parsed_source_names: Option<std::collections::HashSet<String>>,
}

thread_local! {
    static TYPE_FACT_LOOKUP_SESSION: RefCell<Option<TypeFactLookupSession>> = const {
        RefCell::new(None)
    };
}

/// Keep indexes only while the selected FactSet is immutably borrowed. The
/// session owns names and row numbers, never references or dereferenced raw
/// pointers. Other FactSets still use the ordinary scan. Nesting and unwinding
/// restore the previous session, so allocation reuse cannot leak old facts.
pub(crate) fn with_type_fact_lookup_session<R>(facts: &FactSet, f: impl FnOnce() -> R) -> R {
    TYPE_FACT_LOOKUP_SESSION.with(|slot| {
        let previous = slot.replace(Some(TypeFactLookupSession {
            facts_address: std::ptr::from_ref(facts) as usize,
            ..Default::default()
        }));
        struct Restore<'a> {
            slot: &'a RefCell<Option<TypeFactLookupSession>>,
            previous: Option<TypeFactLookupSession>,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.slot.replace(self.previous.take());
            }
        }
        let _restore = Restore { slot, previous };
        f()
    })
}

#[derive(Clone, Copy)]
enum TypeFactKind {
    Capabilities,
    Hierarchy,
}

struct TypeFactCandidates {
    indices: Option<Rc<[usize]>>,
    range: Range<usize>,
}

impl Iterator for TypeFactCandidates {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        let index = self.range.next()?;
        Some(
            self.indices
                .as_ref()
                .map_or(index, |indices| indices[index]),
        )
    }
}

fn type_fact_candidates(facts: &FactSet, kind: TypeFactKind, name: &str) -> TypeFactCandidates {
    let indices = TYPE_FACT_LOOKUP_SESSION.with(|slot| {
        let mut session = slot.borrow_mut();
        let session = session
            .as_mut()
            .filter(|session| session.facts_address == std::ptr::from_ref(facts) as usize)?;
        let index = match kind {
            TypeFactKind::Capabilities => session.capabilities.get_or_insert_with(|| {
                crate::facts::ExternalTypeNameLookupIndex::new(
                    facts
                        .type_capability_overrides
                        .iter()
                        .map(|fact| fact.type_name.as_str()),
                )
            }),
            TypeFactKind::Hierarchy => session.hierarchy.get_or_insert_with(|| {
                crate::facts::ExternalTypeNameLookupIndex::new(
                    facts
                        .hierarchy_overrides
                        .iter()
                        .map(|fact| fact.type_name.as_str()),
                )
            }),
        };
        Some(index.matching_indices(name))
    });
    let count = indices.as_ref().map_or_else(
        || match kind {
            TypeFactKind::Capabilities => facts.type_capability_overrides.len(),
            TypeFactKind::Hierarchy => facts.hierarchy_overrides.len(),
        },
        |indices| indices.len(),
    );
    TypeFactCandidates {
        indices,
        range: 0..count,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EffectiveFacts<'a> {
    facts: &'a FactSet,
    external_index: Option<&'a ExternalFactLookupIndex<'a>>,
    nested_collection_views: &'a [(JavaType, crate::slot_shape::ViewShape)],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveFact<T> {
    value: T,
    provenance: FactProvenance,
}

impl<T> EffectiveFact<T> {
    pub fn new(value: T, provenance: FactProvenance) -> Self {
        Self { value, provenance }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn provenance(&self) -> FactProvenance {
        self.provenance
    }

    pub fn into_value(self) -> T {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastTypeCapabilityPolicy {
    actual: JavaType,
    target: JavaType,
    optional_or_result: bool,
    raw_or_wildcard: bool,
    unchecked_generic: bool,
}

impl CastTypeCapabilityPolicy {
    pub fn new(
        actual: JavaType,
        target: JavaType,
        optional_or_result: bool,
        raw_or_wildcard: bool,
        unchecked_generic: bool,
    ) -> Self {
        Self {
            actual,
            target,
            optional_or_result,
            raw_or_wildcard,
            unchecked_generic,
        }
    }

    pub fn actual(&self) -> &JavaType {
        &self.actual
    }

    pub fn target(&self) -> &JavaType {
        &self.target
    }

    pub fn optional_or_result(&self) -> bool {
        self.optional_or_result
    }

    pub fn raw_or_wildcard(&self) -> bool {
        self.raw_or_wildcard
    }

    pub fn unchecked_generic(&self) -> bool {
        self.unchecked_generic
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceOfTargetCapabilityPolicy {
    target: JavaType,
    primitive_target: bool,
    raw_or_wildcard_target: bool,
}

impl InstanceOfTargetCapabilityPolicy {
    pub fn new(target: JavaType, primitive_target: bool, raw_or_wildcard_target: bool) -> Self {
        Self {
            target,
            primitive_target,
            raw_or_wildcard_target,
        }
    }

    pub fn target(&self) -> &JavaType {
        &self.target
    }

    pub fn primitive_target(&self) -> bool {
        self.primitive_target
    }

    pub fn raw_or_wildcard_target(&self) -> bool {
        self.raw_or_wildcard_target
    }
}

impl<'a> EffectiveFacts<'a> {
    /// Where values of `type_name` live.
    pub fn storage_placement_for(
        &self,
        type_name: &str,
    ) -> crate::storage_strategy::StoragePlacement {
        self.facts.storage_placement_for(type_name)
    }

    /// Which pointer `type_name`'s shared handle must be.
    pub fn shared_handle_for(&self, type_name: &str) -> crate::storage_strategy::SharedHandle {
        self.facts.shared_handle_for(type_name)
    }

    /// How values of `type_name` are held; see [`FactSet::storage_strategy_for`].
    pub fn storage_strategy_for(
        &self,
        type_name: &str,
    ) -> crate::storage_strategy::StorageStrategy {
        self.facts.storage_strategy_for(type_name)
    }

    pub fn new(facts: &'a FactSet) -> Self {
        Self {
            nested_collection_views: &[],
            facts,
            external_index: None,
        }
    }

    pub(crate) fn has_parsed_source_definition(&self, ty: &JavaType) -> bool {
        capability_type_name(ty)
            .is_some_and(|name| self.has_parsed_source_type_capability_for_name(name))
    }

    pub(crate) fn parsed_source_controls_native_carrier(&self, ty: &JavaType) -> bool {
        self.has_parsed_source_definition(ty)
            && !self.type_capability_overrides_for(ty).any(|fact| {
                fact.provenance == FactProvenance::UserOverride
                    && (fact.sequence_like.is_some() || fact.string_like == Some(true))
            })
    }

    pub fn with_external_index(
        facts: &'a FactSet,
        external_index: &'a ExternalFactLookupIndex<'a>,
    ) -> Self {
        Self {
            facts,
            external_index: Some(external_index),
            nested_collection_views: &[],
        }
    }

    pub(crate) fn with_nested_collection_views<'b>(
        self,
        views: &'b [(JavaType, crate::slot_shape::ViewShape)],
    ) -> EffectiveFacts<'b>
    where
        'a: 'b,
    {
        EffectiveFacts {
            facts: self.facts,
            external_index: self.external_index,
            nested_collection_views: if views.is_empty() {
                self.nested_collection_views
            } else {
                views
            },
        }
    }

    pub(crate) fn nested_collection_view(
        &self,
        ty: &JavaType,
    ) -> Option<&crate::slot_shape::ViewShape> {
        self.nested_collection_views
            .iter()
            .find_map(|(key, view)| (key == ty).then_some(view))
    }

    pub fn generic_parameter_is_input_only(&self, type_name: &str, parameter_index: usize) -> bool {
        self.facts
            .generic_parameter_is_input_only(type_name, parameter_index)
    }

    pub fn uses_java_string_identity_representation(&self) -> bool {
        self.facts.java_string_identity_representation
    }

    pub fn hierarchy_overrides(&self) -> &'a [HierarchyOverride] {
        &self.facts.hierarchy_overrides
    }

    pub(crate) fn allows_platform_hierarchy_fallback(&self, name: &str) -> bool {
        !self.has_parsed_source_type_capability_for_name(name)
            && !self
                .hierarchy_overrides_for_name(name)
                .any(|fact| fact.provenance == FactProvenance::UserOverride)
    }

    pub fn external_library_pack_for_type(
        &self,
        type_name: &str,
    ) -> Option<&'a ExternalLibraryPackFact> {
        match self.external_index {
            Some(index) => index.external_library_pack_for_type(type_name),
            None => self.facts.external_library_pack_for_type(type_name),
        }
    }

    pub fn external_rust_crate_mapping_pack_for_type(
        &self,
        type_name: &str,
    ) -> Option<&'a ExternalLibraryPackFact> {
        match self.external_index {
            Some(index) => index.external_rust_crate_mapping_pack_for_type(type_name),
            None => self
                .facts
                .external_rust_crate_mapping_pack_for_type(type_name),
        }
    }

    pub fn external_crate_rust_type_name_for(&self, type_name: &str) -> Option<&'a str> {
        self.facts.external_crate_rust_type_name_for(type_name)
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
        self.type_capabilities_for(java_ty).extends().cloned()
    }

    pub fn implemented_types_for(&self, java_ty: &JavaType) -> Vec<JavaType> {
        self.type_capabilities_for(java_ty).implements().to_vec()
    }

    pub fn cast_type_capability_policy(
        &self,
        actual: &JavaType,
        target: &JavaType,
    ) -> CastTypeCapabilityPolicy {
        CastTypeCapabilityPolicy::new(
            actual.clone(),
            target.clone(),
            is_optional_or_result_type(actual) || is_optional_or_result_type(target),
            contains_wildcard_type(actual)
                || contains_wildcard_type(target)
                || is_raw_generic_counterpart(actual, target),
            is_unchecked_generic_counterpart(actual, target),
        )
    }

    pub fn instanceof_target_capability_policy(
        &self,
        target: &JavaType,
    ) -> InstanceOfTargetCapabilityPolicy {
        InstanceOfTargetCapabilityPolicy::new(
            target.clone(),
            matches!(target, JavaType::Primitive(_)),
            contains_wildcard_type(target)
                || matches!(target, JavaType::Wildcard(_))
                || is_known_raw_generic_type(target),
        )
    }

    pub fn object_safe_trait_fact_for_name(&self, type_name: &str) -> Option<bool> {
        self.object_safe_trait_fact_for_name_with_provenance(type_name)
            .map(EffectiveFact::into_value)
    }

    pub fn object_safe_trait_fact_for_name_with_provenance(
        &self,
        type_name: &str,
    ) -> Option<EffectiveFact<bool>> {
        let override_fact = self
            .facts
            .type_capability_overrides
            .iter()
            .filter(|override_fact| {
                object_safety_type_name_matches(&override_fact.type_name, type_name)
            })
            .filter_map(|override_fact| {
                override_fact
                    .object_safe_trait
                    .map(|value| EffectiveFact::new(value, override_fact.provenance))
            })
            .max_by_key(effective_fact_precedence_key);
        let source_fact = self
            .facts
            .object_safety_facts
            .iter()
            .filter(|fact| object_safety_type_name_matches(&fact.type_name, type_name))
            .map(|fact| EffectiveFact::new(fact.object_safe, fact.provenance))
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
            .filter(|fact| object_safety_type_name_matches(&fact.type_name, type_name))
            .max_by_key(|fact| fact_precedence_key(fact.provenance))
    }

    pub fn object_safety_violation_for_name(
        &self,
        type_name: &str,
    ) -> Option<ObjectSafetyViolation> {
        let object_safe = self.object_safe_trait_fact_for_name_with_provenance(type_name)?;
        if *object_safe.value() {
            return None;
        }
        self.object_safety_fact_for_name_with_effective_precedence(type_name)
            .filter(|fact| fact.provenance == object_safe.provenance())
            .and_then(|fact| fact.violations.first().cloned())
            .or(Some(ObjectSafetyViolation::ExplicitNonObjectSafeFact {
                provenance: object_safe.provenance(),
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
            .map(EffectiveFact::into_value)
    }

    pub fn closed_hierarchy_fact_for_name_with_provenance(
        &self,
        type_name: &str,
    ) -> Option<EffectiveFact<bool>> {
        self.facts
            .closed_hierarchy_facts
            .iter()
            .filter(|fact| type_name_matches(&fact.type_name, type_name))
            .map(|fact| EffectiveFact::new(fact.closed, fact.provenance))
            .max_by_key(effective_fact_precedence_key)
    }

    pub fn sealed_permitted_subtypes_for(
        &self,
        type_name: &str,
    ) -> Option<EffectiveFact<Vec<String>>> {
        self.facts
            .sealed_permits_facts
            .iter()
            .filter(|fact| type_name_matches(&fact.type_name, type_name))
            .map(|fact| EffectiveFact::new(fact.permitted_subtypes.clone(), fact.provenance))
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
        self.type_capabilities_for(java_ty).is_final_type()
    }

    pub fn has_object_representation_facts(&self, java_ty: &JavaType) -> bool {
        self.object_safe_trait_fact_for(java_ty).is_some()
            || capability_type_name(java_ty).is_some_and(|type_name| {
                self.hierarchy_overrides_for_name(type_name)
                    .any(|override_fact| {
                        !fallback_override_conflicts_with_parsed_source(
                            override_fact,
                            type_name,
                            self.facts,
                        )
                    })
                    || self.closed_hierarchy_fact_for_name(type_name).is_some()
            })
    }

    pub fn has_parsed_source_type_capability_for_name(&self, type_name: &str) -> bool {
        if let Some(found) = TYPE_FACT_LOOKUP_SESSION.with(|slot| {
            let mut session = slot.borrow_mut();
            let session = session.as_mut().filter(|session| {
                session.facts_address == std::ptr::from_ref(self.facts) as usize
            })?;
            let names = session.parsed_source_names.get_or_insert_with(|| {
                self.facts
                    .type_capability_overrides
                    .iter()
                    .filter(|capability| capability.provenance == FactProvenance::ParsedSource)
                    .flat_map(|capability| {
                        [
                            capability.type_name.clone(),
                            crate::java_naming::rust_name_for_java_named_type(
                                &capability.type_name,
                            ),
                        ]
                    })
                    .collect()
            });
            Some(names.contains(type_name))
        }) {
            return found;
        }
        self.facts
            .type_capability_overrides
            .iter()
            .any(|capability| {
                capability.provenance == FactProvenance::ParsedSource
                    && source_type_name_matches(&capability.type_name, type_name)
            })
    }

    pub(crate) fn has_external_final_erased_facade_for_name(&self, name: &str) -> bool {
        if self.has_parsed_source_type_capability_for_name(name) {
            return false;
        }
        let erased = JavaType::Named {
            name: name.to_string(),
            args: Vec::new(),
        };
        (matches!(
            self.object_representation_override_strategy_for(&erased),
            Some(HierarchyStrategy::ErasedObject)
        ) && self.is_final_type(&erased))
            || crate::external_packs::builtin_external_final_erased_object_type(name)
    }

    pub fn object_representation_override_strategy_for(
        &self,
        java_ty: &JavaType,
    ) -> Option<&'a HierarchyStrategy> {
        // Modeled functional interfaces have a concrete runtime facade whose
        // Rc-backed callable performs dispatch. Derived open-hierarchy facts
        // must not erase that ABI merely because source implementors exist.
        if self.native_functional_carrier(java_ty).is_some() {
            return Some(&HierarchyStrategy::ConcreteStruct);
        }
        let type_name = capability_type_name(java_ty)?;
        self.hierarchy_overrides_for_name(type_name)
            .filter(|override_fact| {
                !fallback_override_conflicts_with_parsed_source(
                    override_fact,
                    type_name,
                    self.facts,
                )
            })
            .max_by_key(|override_fact| {
                (
                    fact_precedence_key(override_fact.provenance),
                    hierarchy_strategy_specificity_rank(&override_fact.strategy),
                )
            })
            .map(|override_fact| &override_fact.strategy)
    }

    pub(crate) fn native_functional_carrier(
        &self,
        java_ty: &JavaType,
    ) -> Option<crate::stdlib::FunctionalInterfaceShape> {
        let name = capability_type_name(java_ty)?;
        // Most queried types are not SAMs. Reject them before any source or
        // override lookup, retaining the cheap path for ordinary slot queries.
        let shape = crate::stdlib::functional_interface_shape(
            &crate::stdlib::canonical_external_receiver_type_key(name),
        )?;
        if self.has_parsed_source_type_capability_for_name(name)
            || self.hierarchy_overrides_for_name(name).any(|fact| {
                fact.provenance == FactProvenance::UserOverride
                    && fact.strategy != HierarchyStrategy::ConcreteStruct
            })
        {
            return None;
        }
        Some(shape)
    }

    /// Modeled cursors own shared operation/state handles internally; cloning
    /// preserves cursor identity without an outer allocation wrapper.
    pub(crate) fn native_list_cursor_element_type(&self, java_ty: &JavaType) -> Option<JavaType> {
        self.native_cursor_element_type_for_names(
            java_ty,
            &["ListIterator", "java.util.ListIterator", "JavaListIterator"],
        )
    }

    /// Cursor element domains flow independently of the cursor's own handle.
    pub(crate) fn native_cursor_element_type(&self, java_ty: &JavaType) -> Option<JavaType> {
        self.native_cursor_element_type_for_names(
            java_ty,
            &[
                "Iterator",
                "java.util.Iterator",
                "JavaIterator",
                "ListIterator",
                "java.util.ListIterator",
                "JavaListIterator",
            ],
        )
    }

    fn native_cursor_element_type_for_names(
        &self,
        java_ty: &JavaType,
        names: &[&str],
    ) -> Option<JavaType> {
        let ty = crate::types::optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
        let JavaType::Named { name, args } = ty else {
            return None;
        };
        if args.len() != 1
            || !names.contains(&crate::stdlib::canonical_external_receiver_type_key(&name).as_str())
            || self.has_parsed_source_type_capability_for_name(&name)
            || self.hierarchy_overrides_for_name(&name).any(|fact| {
                fact.provenance == FactProvenance::UserOverride
                    && fact.strategy != HierarchyStrategy::ConcreteStruct
            })
        {
            return None;
        }
        Some(args[0].clone())
    }

    pub(crate) fn native_iterable_element_type(&self, java_ty: &JavaType) -> Option<JavaType> {
        let effective =
            crate::types::optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
        let JavaType::Named { name, args } = &effective else {
            return None;
        };
        if crate::stdlib::canonical_external_receiver_type_key(name) != "Iterable"
            || args.len() != 1
        {
            return None;
        }
        self.native_functional_carrier(&effective)?;
        Some(crate::types::generic_read_type(&args[0]))
    }

    pub(crate) fn native_iterable_list_projection_element(
        &self,
        actual: &JavaType,
        target: &JavaType,
    ) -> Option<JavaType> {
        let element = self.native_iterable_element_type(actual)?;
        let target = crate::types::optional_element_type(target).unwrap_or_else(|| target.clone());
        let JavaType::Named { name, args } = &target else {
            return None;
        };
        if crate::stdlib::canonical_external_receiver_type_key(name) != "List"
            || self.parsed_source_controls_native_carrier(&target)
            || !(args.is_empty()
                || (args.len() == 1 && {
                    let target = crate::types::generic_read_type(&args[0]);
                    target == element
                        || crate::types::optional_element_type(&element).as_ref() == Some(&target)
                }))
        {
            return None;
        }
        Some(element)
    }

    /// Common authority for the native payloads implemented by the retained
    /// collection runtime, preserving parsed source precedence.
    pub(crate) fn retained_native_collection_kind(
        &self,
        java_ty: &JavaType,
    ) -> Option<crate::stdlib::JavaCollectionKind> {
        let ty = crate::types::optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
        if self.parsed_source_controls_native_carrier(&ty) {
            return None;
        }
        let kind = crate::stdlib::java_collection_kind_for_type(&ty)?;
        matches!(
            kind,
            crate::stdlib::JavaCollectionKind::List
                | crate::stdlib::JavaCollectionKind::Set
                | crate::stdlib::JavaCollectionKind::InsertionOrderedSet
        )
        .then_some(kind)
    }

    /// These sequence values carry concrete factory policy (null acceptance,
    /// List membership and iteration behavior), even in an unaliased local.
    /// Queue alone is not enough: priority/blocking backends remain distinct.
    pub(crate) fn native_collection_requires_runtime_policy(&self, java_ty: &JavaType) -> bool {
        let ty = crate::types::optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
        self.retained_native_collection_kind(&ty).is_some()
            && matches!(&ty, JavaType::Named { name, .. } | JavaType::Unknown(name)
                if matches!(crate::stdlib::canonical_external_receiver_type_key(name).rsplit('.').next().unwrap_or(name),
                    "Deque" | "ArrayDeque" | "LinkedList"))
    }

    /// Map payloads have two child domains; they are not the one-element
    /// collection runtime. Hash/linked maps can share a canonical retained
    /// map ABI while retaining insertion order. Sorted/comparator backends
    /// keep their separate contract until explicitly supported.
    pub(crate) fn retained_native_map_kind(
        &self,
        java_ty: &JavaType,
    ) -> Option<crate::stdlib::JavaCollectionKind> {
        let ty = crate::types::optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
        if self.parsed_source_controls_native_carrier(&ty) {
            return None;
        }
        let kind = crate::stdlib::java_collection_kind_for_type(&ty)?;
        matches!(
            kind,
            crate::stdlib::JavaCollectionKind::Map
                | crate::stdlib::JavaCollectionKind::InsertionOrderedMap
        )
        .then_some(kind)
    }

    /// Allocation retention is shared policy; payload rendering and operations
    /// still distinguish one-element collections from two-element maps.
    pub(crate) fn retained_native_container_kind(
        &self,
        java_ty: &JavaType,
    ) -> Option<crate::stdlib::JavaCollectionKind> {
        self.retained_native_collection_kind(java_ty)
            .or_else(|| self.retained_native_map_kind(java_ty))
    }

    fn hierarchy_overrides_for_name(
        &self,
        type_name: &str,
    ) -> impl Iterator<Item = &'a HierarchyOverride> {
        type_fact_candidates(self.facts, TypeFactKind::Hierarchy, type_name)
            .map(|index| &self.facts.hierarchy_overrides[index])
            .filter(move |override_fact| type_name_matches(&override_fact.type_name, type_name))
    }

    pub fn owned_recovery_capability_for(&self, java_ty: &JavaType) -> OwnedRecoveryCapability {
        if let Some(fact) = self.owned_recovery_capability_fact_for(java_ty) {
            return fact.capability.clone();
        }
        if let Some(element_ty) = crate::types::optional_element_type(java_ty) {
            return match self.owned_recovery_capability_for(&element_ty) {
                OwnedRecoveryCapability::Copy
                | OwnedRecoveryCapability::ImmutableSharedHandle { .. }
                | OwnedRecoveryCapability::ExplicitClone { .. }
                | OwnedRecoveryCapability::CloneBoxAdapter { .. }
                | OwnedRecoveryCapability::FactoryCopy { .. } => {
                    OwnedRecoveryCapability::ExplicitClone {
                        java_ty: java_ty.clone(),
                    }
                }
                OwnedRecoveryCapability::ConservativeClone => {
                    OwnedRecoveryCapability::ConservativeClone
                }
                OwnedRecoveryCapability::NonCloneable
                | OwnedRecoveryCapability::FacadeOwnedExternalHandle { .. }
                | OwnedRecoveryCapability::FfiOwnedHandle { .. } => {
                    OwnedRecoveryCapability::NonCloneable
                }
            };
        }
        if self.is_copy_type(java_ty) {
            return OwnedRecoveryCapability::Copy;
        }
        if matches!(java_ty, JavaType::String) {
            return OwnedRecoveryCapability::ExplicitClone {
                java_ty: java_ty.clone(),
            };
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

    pub fn owned_recovery_capability_fact_for(
        &self,
        java_ty: &JavaType,
    ) -> Option<&'a OwnedRecoveryCapabilityFact> {
        let type_name = capability_type_name(java_ty)?;
        self.facts
            .owned_recovery_capability_facts
            .iter()
            .filter(|fact| type_name_matches(&fact.type_name, type_name))
            .max_by_key(|fact| fact_precedence_key(fact.provenance))
    }

    pub fn is_string_like_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).is_string_like()
    }

    pub fn is_copy_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).is_copy()
    }

    pub fn is_clone_type(&self, java_ty: &JavaType) -> bool {
        self.type_capabilities_for(java_ty).is_clone()
    }

    pub fn explicit_clone_capability_for(&self, java_ty: &JavaType) -> Option<bool> {
        self.explicit_clone_capability_for_with_provenance(java_ty)
            .map(EffectiveFact::into_value)
    }

    pub fn explicit_clone_capability_for_with_provenance(
        &self,
        java_ty: &JavaType,
    ) -> Option<EffectiveFact<bool>> {
        self.type_capability_overrides_for(java_ty)
            .filter_map(|override_fact| {
                override_fact
                    .clone
                    .map(|value| EffectiveFact::new(value, override_fact.provenance))
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
        self.type_capabilities_for(java_ty).sequence_like().cloned()
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
        let candidates = type_name
            .map(|name| type_fact_candidates(self.facts, TypeFactKind::Capabilities, name));
        candidates
            .into_iter()
            .flatten()
            .map(|index| &self.facts.type_capability_overrides[index])
            .filter(move |override_fact| {
                type_name
                    .is_some_and(|type_name| type_name_matches(&override_fact.type_name, type_name))
            })
    }
}

fn effective_fact_max_by_precedence<T>(
    left: Option<EffectiveFact<T>>,
    right: Option<EffectiveFact<T>>,
) -> Option<EffectiveFact<T>> {
    match (left, right) {
        (Some(left), Some(right)) => {
            if fact_precedence_key(right.provenance()) > fact_precedence_key(left.provenance()) {
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
    fact_precedence_key(fact.provenance())
}

pub(crate) fn fact_precedence_key(provenance: FactProvenance) -> u8 {
    match provenance {
        FactProvenance::ConservativeFallback => 0,
        FactProvenance::CorpusAnalysis => 1,
        FactProvenance::BuiltinExternalModel | FactProvenance::ExternalLibraryPack => 2,
        FactProvenance::ParsedSource => 3,
        FactProvenance::UserOverride => 4,
    }
}

fn hierarchy_strategy_specificity_rank(strategy: &HierarchyStrategy) -> u8 {
    match strategy {
        HierarchyStrategy::ConcreteStruct => 0,
        HierarchyStrategy::DynTrait { .. }
        | HierarchyStrategy::ClosedEnum { .. }
        | HierarchyStrategy::ErasedObject => 1,
    }
}

fn type_name_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    crate::facts::external_type_name_matches(fact_type_name, query_type_name)
}

// External models deliberately accept leaf aliases. Source ownership must not:
// a source `Character` is not evidence for `java.lang.Character`. Retain the
// full owner when comparing its emitted (flattened) Rust spelling.
fn source_type_name_matches(source_name: &str, query_name: &str) -> bool {
    source_name == query_name
        || crate::java_naming::rust_name_for_java_named_type(source_name) == query_name
}

#[cfg(test)]
mod source_identity_tests {
    use super::source_type_name_matches;

    #[test]
    fn source_identity_does_not_import_external_leaf_aliases() {
        assert!(source_type_name_matches("Character", "Character"));
        assert!(!source_type_name_matches(
            "Character",
            "java.lang.Character"
        ));
        assert!(!source_type_name_matches(
            "other.Character",
            "java.lang.Character"
        ));
        assert!(source_type_name_matches(
            "other.Character",
            "OtherCharacter"
        ));
        assert!(source_type_name_matches("Outer.Inner", "Outer_Inner"));
    }
}

fn fallback_override_conflicts_with_parsed_source(
    override_fact: &HierarchyOverride,
    query_type_name: &str,
    facts: &FactSet,
) -> bool {
    override_fact.is_source_fallback()
        && type_fact_candidates(facts, TypeFactKind::Capabilities, query_type_name).any(|index| {
            let capability = &facts.type_capability_overrides[index];
            capability.provenance == FactProvenance::ParsedSource
                && type_name_matches(&capability.type_name, query_type_name)
        })
}

#[cfg(test)]
mod type_fact_lookup_session_tests {
    use super::*;

    #[test]
    fn parsed_source_name_index_retains_exact_emitted_identity_and_provenance() {
        let mut facts = facts_for(false);
        let mut source = facts.type_capability_overrides[0].clone();
        source.provenance = FactProvenance::ParsedSource;
        for name in [
            "Character",
            "other.Token",
            "Outer.Inner",
            "Outer$Inner",
            "Result",
        ] {
            source.type_name = name.to_string();
            facts.type_capability_overrides.push(source.clone());
        }
        source.type_name = "ExternalOnly".to_string();
        source.provenance = FactProvenance::UserOverride;
        facts.type_capability_overrides.push(source);
        let names = [
            "Character",
            "java.lang.Character",
            "other.Token",
            "Token",
            "OtherToken",
            "Outer.Inner",
            "Outer_Inner",
            "Outer$Inner",
            "Inner",
            "Result",
            "JavaResult",
            "ExternalOnly",
            "Missing",
        ];
        let expected = names.map(|name| {
            EffectiveFacts::new(&facts).has_parsed_source_type_capability_for_name(name)
        });
        with_type_fact_lookup_session(&facts, || {
            for (name, expected) in names.into_iter().zip(expected) {
                assert_eq!(
                    EffectiveFacts::new(&facts).has_parsed_source_type_capability_for_name(name),
                    expected,
                    "{name}"
                );
            }
            assert!(TYPE_FACT_LOOKUP_SESSION.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .unwrap()
                    .parsed_source_names
                    .is_some()
            }));
        });
        for capability in &mut facts.type_capability_overrides {
            if capability.type_name == "Character" {
                capability.type_name = "Changed".to_string();
            }
        }
        with_type_fact_lookup_session(&facts, || {
            assert!(
                !EffectiveFacts::new(&facts)
                    .has_parsed_source_type_capability_for_name("Character")
            );
            assert!(
                EffectiveFacts::new(&facts).has_parsed_source_type_capability_for_name("Changed")
            );
        });
    }

    #[test]
    fn native_functional_carriers_keep_source_and_user_precedence() {
        let mut facts = facts_for(false);
        for name in ["IntSupplier", "Comparator", "BiFunction", "Runnable"] {
            let ty = JavaType::Named {
                name: name.to_string(),
                args: Vec::new(),
            };
            facts.hierarchy_overrides.push(HierarchyOverride {
                type_name: name.to_string(),
                strategy: HierarchyStrategy::ErasedObject,
                provenance: FactProvenance::CorpusAnalysis,
            });
            let effective = EffectiveFacts::new(&facts);
            assert!(effective.native_functional_carrier(&ty).is_some());
            assert_eq!(
                effective.object_representation_override_strategy_for(&ty),
                Some(&HierarchyStrategy::ConcreteStruct)
            );
            facts.hierarchy_overrides.push(HierarchyOverride {
                type_name: name.to_string(),
                strategy: HierarchyStrategy::ErasedObject,
                provenance: FactProvenance::UserOverride,
            });
            let effective = EffectiveFacts::new(&facts);
            assert!(effective.native_functional_carrier(&ty).is_none());
            assert_eq!(
                effective.object_representation_override_strategy_for(&ty),
                Some(&HierarchyStrategy::ErasedObject)
            );
            facts.hierarchy_overrides.pop();
            let mut source = facts.type_capability_overrides[1].clone();
            source.type_name = name.to_string();
            facts.type_capability_overrides.push(source);
            assert!(
                EffectiveFacts::new(&facts)
                    .native_functional_carrier(&ty)
                    .is_none()
            );
            facts.type_capability_overrides.pop();
        }
    }

    #[test]
    fn deque_runtime_policy_preserves_source_precedence_and_backend_boundaries() {
        let mut facts = facts_for(false);
        let ty = |name: &str| JavaType::Named {
            name: name.into(),
            args: vec![JavaType::String],
        };
        for name in ["Deque", "java.util.ArrayDeque", "LinkedList"] {
            assert!(
                EffectiveFacts::new(&facts).native_collection_requires_runtime_policy(&ty(name))
            );
        }
        for name in ["Queue", "PriorityQueue", "ArrayBlockingQueue", "ArrayList"] {
            assert!(
                !EffectiveFacts::new(&facts).native_collection_requires_runtime_policy(&ty(name))
            );
        }
        let mut source = facts.type_capability_overrides[1].clone();
        source.type_name = "ArrayDeque".into();
        facts.type_capability_overrides.push(source);
        assert!(
            !EffectiveFacts::new(&facts)
                .native_collection_requires_runtime_policy(&ty("ArrayDeque"))
        );
    }

    #[test]
    fn native_list_cursor_contract_preserves_source_and_user_precedence() {
        let mut facts = facts_for(false);
        let ty = JavaType::Named {
            name: "ListIterator".into(),
            args: vec![JavaType::String],
        };
        assert_eq!(
            EffectiveFacts::new(&facts).native_list_cursor_element_type(&ty),
            Some(JavaType::String)
        );
        let mut source = facts.type_capability_overrides[1].clone();
        source.type_name = "ListIterator".into();
        facts.type_capability_overrides.push(source);
        assert!(
            EffectiveFacts::new(&facts)
                .native_list_cursor_element_type(&ty)
                .is_none()
        );
        facts.type_capability_overrides.pop();
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: "ListIterator".into(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::UserOverride,
        });
        assert!(
            EffectiveFacts::new(&facts)
                .native_list_cursor_element_type(&ty)
                .is_none()
        );
    }

    #[test]
    fn read_only_cursor_contract_keeps_qualified_elements_and_source_precedence() {
        let mut facts = facts_for(false);
        let ty = JavaType::Named {
            name: "Iterator".into(),
            args: vec![crate::types::optional_java_type(JavaType::String)],
        };
        assert_eq!(
            EffectiveFacts::new(&facts).native_cursor_element_type(&ty),
            Some(crate::types::optional_java_type(JavaType::String))
        );
        assert!(
            EffectiveFacts::new(&facts)
                .native_list_cursor_element_type(&ty)
                .is_none()
        );
        let mut source = facts.type_capability_overrides[1].clone();
        source.type_name = "Iterator".into();
        facts.type_capability_overrides.push(source);
        assert!(
            EffectiveFacts::new(&facts)
                .native_cursor_element_type(&ty)
                .is_none()
        );
        facts.type_capability_overrides.pop();
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: "Iterator".into(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::UserOverride,
        });
        assert!(
            EffectiveFacts::new(&facts)
                .native_cursor_element_type(&ty)
                .is_none()
        );
    }

    fn facts_for(copy: bool) -> FactSet {
        let mut facts = FactSet::default();
        for (type_name, provenance) in [
            ("example.Token", FactProvenance::ConservativeFallback),
            ("example.Token", FactProvenance::ParsedSource),
            ("other.Token", FactProvenance::ExternalLibraryPack),
            ("example.Token", FactProvenance::UserOverride),
        ] {
            facts
                .type_capability_overrides
                .push(TypeCapabilityOverride {
                    type_name: type_name.to_string(),
                    copy: Some(copy),
                    clone: Some(true),
                    string_like: None,
                    sequence_like: None,
                    implements: Vec::new(),
                    extends: None,
                    final_type: None,
                    object_safe_trait: None,
                    provenance,
                });
        }
        for (strategy, provenance) in [
            (
                HierarchyStrategy::ErasedObject,
                FactProvenance::BuiltinExternalModel,
            ),
            (
                HierarchyStrategy::ConcreteStruct,
                FactProvenance::ParsedSource,
            ),
            (
                HierarchyStrategy::DynTrait {
                    trait_name: "TokenView".to_string(),
                },
                FactProvenance::UserOverride,
            ),
            (
                HierarchyStrategy::DynTrait {
                    trait_name: "LastTokenView".to_string(),
                },
                FactProvenance::UserOverride,
            ),
        ] {
            facts.hierarchy_overrides.push(HierarchyOverride {
                type_name: "example.Token".to_string(),
                strategy,
                provenance,
            });
        }
        facts
    }

    fn snapshot(
        facts: &FactSet,
        name: &str,
    ) -> (
        Vec<TypeCapabilityOverride>,
        TypeCapabilities,
        Option<HierarchyStrategy>,
        bool,
    ) {
        let effective = EffectiveFacts::new(facts);
        let ty = JavaType::Named {
            name: name.to_string(),
            args: Vec::new(),
        };
        (
            effective
                .type_capability_overrides_for(&ty)
                .cloned()
                .collect(),
            effective.type_capabilities_for(&ty),
            effective
                .object_representation_override_strategy_for(&ty)
                .cloned(),
            effective.has_parsed_source_type_capability_for_name(name),
        )
    }

    #[test]
    fn scoped_type_fact_index_matches_ordered_scan_and_precedence() {
        let facts = facts_for(false);
        let names = [
            "example.Token",
            "Token",
            "example_token",
            "other.Token",
            "Missing",
        ];
        let expected = names.map(|name| snapshot(&facts, name));
        with_type_fact_lookup_session(&facts, || {
            for (name, expected) in names.into_iter().zip(&expected) {
                assert_eq!(&snapshot(&facts, name), expected, "{name}");
                assert!(
                    type_fact_candidates(&facts, TypeFactKind::Capabilities, name)
                        .indices
                        .is_some()
                );
            }
            assert_eq!(
                snapshot(&facts, "example.Token").2,
                Some(HierarchyStrategy::DynTrait {
                    trait_name: "LastTokenView".to_string()
                })
            );
        });
        assert!(TYPE_FACT_LOOKUP_SESSION.with(|slot| slot.borrow().is_none()));
    }

    #[test]
    fn scoped_type_fact_index_restores_nested_sessions_and_does_not_cache_mutations() {
        let mut outer = facts_for(false);
        let inner = facts_for(true);
        let original = snapshot(&outer, "Token");
        let other = snapshot(&inner, "Token");
        with_type_fact_lookup_session(&outer, || {
            assert_eq!(snapshot(&outer, "Token"), original);
            assert_eq!(snapshot(&inner, "Token"), other);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_type_fact_lookup_session(&inner, || {
                    assert_eq!(snapshot(&inner, "Token"), other);
                    assert_eq!(snapshot(&outer, "Token"), original);
                    panic!("test session restoration");
                });
            }));
            assert!(panic.is_err());
            assert_eq!(snapshot(&outer, "Token"), original);
            assert!(
                type_fact_candidates(&outer, TypeFactKind::Capabilities, "Token")
                    .indices
                    .is_some()
            );
            assert!(
                type_fact_candidates(&inner, TypeFactKind::Capabilities, "Token")
                    .indices
                    .is_none()
            );
        });
        assert!(TYPE_FACT_LOOKUP_SESSION.with(|slot| slot.borrow().is_none()));
        outer.type_capability_overrides[0].type_name = "Changed".to_string();
        outer.type_capability_overrides[3].copy = Some(true);
        outer
            .type_capability_overrides
            .push(outer.type_capability_overrides[3].clone());
        let changed = snapshot(&outer, "Token");
        assert_ne!(original, changed);
        with_type_fact_lookup_session(&outer, || assert_eq!(snapshot(&outer, "Token"), changed));
    }
}

fn object_safety_type_name_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    type_name_matches(fact_type_name, query_type_name)
        || fact_type_name
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == query_type_name)
        || query_type_name
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == fact_type_name)
}

fn apply_type_capability_override(
    capabilities: &mut TypeCapabilities,
    override_fact: &TypeCapabilityOverride,
) {
    if let Some(copy) = override_fact.copy {
        capabilities.set_copy(copy);
    }
    if let Some(clone) = override_fact.clone {
        capabilities.set_clone(clone);
    }
    if let Some(string_like) = override_fact.string_like {
        capabilities.set_string_like(string_like);
    }
    if let Some(sequence_like) = &override_fact.sequence_like {
        capabilities.set_sequence_like(sequence_like.clone());
    }
    if !override_fact.implements.is_empty() {
        capabilities.set_implements(override_fact.implements.clone());
    }
    if let Some(extends) = &override_fact.extends {
        capabilities.set_extends(extends.clone());
    }
    if let Some(final_type) = override_fact.final_type {
        capabilities.set_final_type(final_type);
    }
    if let Some(object_safe_trait) = override_fact.object_safe_trait {
        capabilities.set_object_safe_trait(object_safe_trait);
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
