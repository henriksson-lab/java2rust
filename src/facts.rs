use std::collections::HashSet;

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
    pub external_crate_api_mapping_facts: Vec<ExternalCrateApiMappingFact>,
    pub external_crate_constructor_mapping_facts: Vec<ExternalCrateConstructorMappingFact>,
    pub external_constructor_facts: Vec<ExternalConstructorFact>,
    pub external_field_facts: Vec<ExternalFieldFact>,
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
    ExternalCrateApiMapping,
    ExternalCrateConstructorMapping,
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
    pub role: ExternalApiRole,
    pub receiver_mutability: ReceiverMutabilityFact,
    pub return_type: Option<JavaType>,
    pub return_nullability: NullabilityState,
    pub thrown: Vec<JavaType>,
    pub provenance: FactProvenance,
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
    pub parameter_index: usize,
    pub parameter_type: Option<JavaType>,
    pub parameter_nullability: NullabilityState,
    pub argument_mutability: ReceiverMutabilityFact,
    pub alias_policy: ExternalArgumentAliasPolicy,
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
pub struct ExternalCrateConstructorMappingFact {
    pub type_name: String,
    pub arg_count: usize,
    pub rust_type_name: Option<String>,
    pub rust_name: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFieldFact {
    pub field: FieldKey,
    pub field_type: Option<JavaType>,
    pub provenance: FactProvenance,
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
        self.external_crate_api_mapping_facts
            .extend(other.external_crate_api_mapping_facts);
        self.external_crate_constructor_mapping_facts
            .extend(other.external_crate_constructor_mapping_facts);
        self.external_constructor_facts
            .extend(other.external_constructor_facts);
        self.external_field_facts.extend(other.external_field_facts);
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
        self.external_api_argument_effect_facts
            .iter()
            .rev()
            .find(|fact| {
                fact.method_name == method_name
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
                        fact.method_name == method_name
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
                        fact.method_name == method_name
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
                        fact.method_name == method_name
                            && fact.receiver_type.is_none()
                            && fact.parameter_index == parameter_index
                    })
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
        self.find_external_field_for(type_name, field_name, true)
            .or_else(|| self.find_external_field_for(type_name, field_name, false))
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

    pub fn external_library_pack_for_type(
        &self,
        type_name: &str,
    ) -> Option<&ExternalLibraryPackFact> {
        self.external_library_packs
            .iter()
            .rev()
            .find(|pack| external_pack_owns_type(pack, type_name))
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
            .is_some_and(|fact| !fact.thrown.is_empty())
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
            .find(|fact| external_type_name_matches(&fact.type_name, type_name))
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

fn external_pack_owns_type(pack: &ExternalLibraryPackFact, type_name: &str) -> bool {
    type_name == pack.package_prefix
        || type_name
            .strip_prefix(&pack.package_prefix)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

pub(crate) fn external_type_name_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    let fact_aliases = external_type_name_aliases(fact_type_name);
    let query_aliases = external_type_name_aliases(query_type_name);
    fact_aliases.iter().any(|fact_alias| {
        query_aliases
            .iter()
            .any(|query_alias| query_alias == fact_alias)
    })
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
    fact_method_name == query_method_name
        || crate::parser::to_snake_case(fact_method_name)
            == crate::parser::to_snake_case(query_method_name)
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
